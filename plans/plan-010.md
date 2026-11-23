## Phase 010 — Create the ids-distribution crate (data partitioning and distribution): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no source modifications, no repository changes made. This document is the blueprint for implementation only.**

---

### 1. Executive Summary

Phase 010 creates the **ids-distribution** crate — the engine that divides a dataset into independently placeable units (partitions) and tracks their distribution across nodes, H-units, and regions. It owns partition boundaries, key-to-partition mapping, split/merge behavior, partition metadata, partition ownership, and logical-to-physical mapping stability during migration. The crate sits between `ids-placement` (which decides *where* a unit goes) and `ids-replication`/`ids-routing`/`ids-read` (which consume placement output); it is the layer that turns a logical dataset into a set of placeable, movable partition objects.

**Core engineering decisions (research-verified):**

- **Partition as the unit of distribution:** a dataset is divided into partitions; each partition is independently placeable and owns a disjoint key-range or hash-bucket range. Consistent with Dynamo, Bigtable tablets, TiKV regions, CockroachDB ranges, Couchbase vBuckets, MongoDB chunks — all treat a bounded unit as the atom of placement.
- **Logical → physical indirection via a versioned partition map:** logical partition boundaries are stable identities; physical location (node/H-unit/region) is a separate, versioned mapping that changes on split/merge/move without changing the logical partition id. Matches Couchbase vBucket map (doc ID → vBucket → server) and NodeDB vShards (data hashed to vShard → catalog → owner). Enables live migration and rebalancing without client-visible key remapping.
- **Key mapping strategies:** consistent/rendezvous hashing (uniform spread, ~1/N remap on membership change), range partitioning (ordered scans, contiguous [start,end) ranges like Bigtable/TiKV/CockroachDB), and directory-based mapping (explicit boundary table for residency/uneven shards). Strategy chosen per dataset policy, not globally.
- **Split/merge with hysteresis:** size-triggered split when a partition exceeds target S; merge when adjacent partitions stay below ~0.5×S for a sustained period; hysteresis (split at >1.5×S, merge <0.5×S) prevents split/merge thrash. Split point sampled at key median; daughters get roughly equal ranges/load. Atomic publication: prepare children from a consistent snapshot, capture post-snapshot writes, atomically publish new map epoch, fence old owner, retain redirect/rollback metadata until old routers drain, delete parent only after no reader references it. Merge is the reverse with one publication point.
- **Single authoritative owner at a time:** each partition has one write owner; ownership transfer fences the old owner, transfers a consistent snapshot plus in-flight mutation replay, then atomically publishes the new owner (LEAP-style owner table transfer; shard-growth handoff pattern). Reads served during migration via redirect to both source and target during cutover.
- **Logical-to-physical mapping stability during migration:** partition logical id never changes across a move; only the catalog entry (physical location) is updated atomically. Tombstone/redirect retained briefly so in-flight requests resolve. No key remapping on the client side.
- **New crate `ids-distribution`** per user's separate-crates rule; depends only on `ids-000-aa` (types/sealed contracts), `ids-placement` (placement decisions), `ids-topology` (failure domains), `ids-membership` (node lifecycle), `ids-placement-policy`, `ids-consistency-profile`, `ids-redundancy-mode`, `ids-crypto` (allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305), `ids-partition-map` (logical→physical map crate, sibling).
- All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

---

### 2. Architecture Overview

The distribution crate sits between the placement layer and the replication/routing/read layer. It is **not** a network protocol — it is a local, deterministic partition lifecycle engine that produces partition metadata, map epochs, and ownership records consumed by `ids-replication`, `ids-routing`, `ids-read`, and `ids-query`.

```
┌──────────────────────────────────────────────────────────────────────┐
│                    CONSUMER LAYER                                    │
│  ids-replication · ids-routing · ids-read · ids-query · ids-write   │
├──────────────────────────────────────────────────────────────────────┤
│              Phase 010: ids-distribution (NEW CRATE)                 │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────────────────────────┐  │
│  │   Partition  │ │  Key Mapping │ │   Partition Metadata        │  │
│  │   Boundaries │ │  Resolver    │ │   (epoch, version, owner)   │  │
│  └──────────────┘ └──────────────┘ └─────────────────────────────┘  │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────────────────────────┐  │
│  │  Split/Merge │ │   Ownership  │ │ Logical→Physical Stability  │  │
│  │   Controller │ │  Transfer    │ │   During Migration          │  │
│  └──────────────┘ └──────────────┘ └─────────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────┤
│             ids-placement · ids-partition-map · ids-topology         │
│   PlacementDecision · logical↔physical map · failure domains         │
├──────────────────────────────────────────────────────────────────────┤
│             ids-000-aa (canonical types, sealed contracts)           │
│   PartitionId · DatasetId · NodeId · RegionId · HUnitId · ...        │
└──────────────────────────────────────────────────────────────────────┘
```

**Data flow of partition lifecycle:**

1. Dataset created with redundancy mode + consistency profile → `ids-placement-policy` defines replica/fragment counts and required domains → `ids-distribution` creates initial partition(s) spanning the full keyspace, records partition metadata (boundaries, epoch 1, version 1, owner unset).
2. Write/read arrives keyed by `RecordId`/`DatasetId` → Key Mapping Resolver hashes/ranges the key → identifies target partition → consults versioned partition map → resolves owning node/H-unit/region → `ids-routing` directs the request.
3. Partition grows beyond split threshold → Split Controller selects median split key from consistent snapshot → creates daughter partitions [start, mid) and [mid, end) → assigns new partition ids → captures writes after snapshot via dual-application/log → brings daughters to cutover frontier → atomically publishes new map epoch → fences parent writes → retains redirect until old routers drain → deletes parent state.
4. Adjacent partitions below merge threshold → Merge Controller validates adjacency + same owner domain → combines into one partition → single publication point preserves both children through the cutover.
5. Rebalance/move triggered by `ids-placement` or `ids-selfreorg`/`ids-mobility` → Ownership Transfer: fence old owner, transfer consistent snapshot + mutation replay, atomically publish new owner in map, retain redirect/rollback until drain complete → logical partition id unchanged, only physical location updated.
6. Partition metadata (epoch, version, boundaries, owner, location, checksums) persisted via `ids-partition-map`; consumers subscribe to map version changes.

---

### 3. Detailed Research Dimensions

#### 3.1 Partition Boundaries & Key Mapping (hash, range, directory)

**Consistent/rendezvous hashing:** keys and nodes mapped onto a hash ring/space; a key resolves to the first node clockwise (consistent hashing) or the highest-weight node (rendezvous/HRW). Adding/removing one node remaps only ~1/N of keys (Karger et al., MIT; Dynamo paper). Uniform spread, no hot spots under random keys, no central directory. Limitation: range scans inefficient (keys scattered); heterogeneous capacity needs virtual nodes or weighted HRW. Couchbase uses CRC32(doc ID) → 1024 fixed vBuckets → vBucket map → server; uniform 1/1024 share per vBucket. Used as baseline for equality lookups.

**Range partitioning:** contiguous key ranges [start_key, end_key) map to partition owners; Bigtable tablets, TiKV regions, CockroachDB ranges, HBase regions all use this. Good for ordered/range scans and prefix queries; range queries hit only relevant partitions. Risk: sequential keys (timestamps, monotonically increasing IDs) concentrate writes on the last range → hot spots; mitigated by hashed shard keys or key prefix salting. Split/merge natural (adjacent ranges). Needs metadata management of boundaries (Bigtable METADATA hierarchy, CockroachDB meta1→meta2→ranges).

**Directory/virtual-bucket mapping:** fixed logical bucket count (e.g., 1024 vBuckets or 4096 logical shards as in Aerospike), hashed from key, then a versioned table maps each bucket to a physical owner. Indirection decouples logical partition identity from physical location → live migration without remapping keys (Couchbase, NodeDB vShards). Supports residency, uneven shards, tenant moves. Price: metadata dependency — routers need a strongly managed, versioned map and safe cache invalidation on move begin/end. Best for IDS: combine logical bucket/virtual-partition layer with per-bucket versioned ownership.

**Hybrid per dataset:** policy selects strategy — hash for uniform equality workloads, range for scan-heavy workloads, directory for residency/uneven/tenant-isolated datasets.

#### 3.2 Split/Merge Behavior

**Split triggers:** size-based (partition exceeds target S, e.g. tens of MB–tens of GB depending on engine — Bigtable 100–200 MB tablets, MongoDB 64 MB chunks, TiKV 96 MB regions, YugabyteDB 512 MB low phase / 10 GB high phase / 100 GB force split) and load-based (QPS/write-throughput exceeds threshold even below size target, preempting hot ranges). Hysteresis: split at >1.5×S, merge only below ~0.5×S to prevent split/merge thrash ("region explosion" from poor boundaries).

**Split algorithm:** sample key distribution within parent → choose median split key → create daughters [start, mid) and [mid, end) with roughly equal key ranges/load → assign new partition ids → record parent→children lineage in metadata.

**Merge triggers:** adjacent partitions both below ~0.5×S and low load for sustained period; small adjacent regions merged (TiKV max-merge-region-size/keys, split-merge-interval cooldown; newly split regions not merged immediately).

**Atomic publication (both split and merge):**
1. Choose boundary + durable split/merge identity.
2. Create child state from consistent parent snapshot.
3. Capture writes after snapshot via log or dual-application protocol.
4. Bring children to parent's cutover frontier.
5. Atomically publish new map/epoch and fence parent writes.
6. Retain redirect + rollback metadata until old routers drain.
7. Delete obsolete parent state only after no reader or rollback path references it.

Merge is the reverse: one publication point preserves both children through cutover.

#### 3.3 Partition Metadata

Each partition carries: partition id (stable logical identity), dataset id, key boundaries [start_key, end_key) or hash-bucket set, epoch (version bumped on split/merge/owner change), version (data-generation counter), owning node/H-unit/region, replica/fragment placement set, redundancy mode, consistency profile, size, key count, creation time, last-split/merge timestamp, checksums, lineage (parent id for split children, children ids for merged parent), redirect/rollback pointers during migration.

Metadata is versioned: every split/merge/owner change publishes a new map epoch; consumers cache and invalidate on epoch change. Partition map is the authoritative source of logical→physical mapping (see `ids-partition-map` crate).

#### 3.4 Distribution Across Nodes/H-units/Regions

Distribution = placement + replication + ownership. `ids-placement` produces `PlacementDecision` (node/H-unit/region set per partition); `ids-distribution` records the decision in the partition map as the physical location; `ids-replication` places replicas/fragments across failure domains per `ids-redundancy-mode`; `ids-routing` resolves requests to the owning replica.

Failure-domain awareness (from `ids-topology`): replicas/fragments of one partition spread across independent failure domains (power, facility, network segment, operator, jurisdiction) so one correlated failure disables at most one replica. Correlated-failure modeling (arborescence/dependency DAG DP, COCOA) checks placement before acceptance.

Capacity awareness (from `ids-resource`): component-wise fit test (storage/CPU/memory/network/IOPS vectors); headroom reserved for recovery/rebalance bursts.

#### 3.5 Partition Ownership

Each partition has exactly one write owner (leader/primary) at any time; replicas/fragments are followers/secondaries. Ownership is recorded in the partition map with an epoch; changes require atomic publication.

**Ownership transfer protocol** (LEAP-style + shard-growth handoff):
1. Requester (or controller) initiates transfer; partitioner/old owner identified from map.
2. Old owner fenced: no new writes accepted after fence point; in-flight writes drained.
3. Consistent snapshot transferred to new owner; mutations after snapshot replayed (idempotent reapplication keyed by OpRef/OrderingPos).
4. Catch-up verified; cutover boundary established under controlled handoff.
5. New map epoch published atomically: new owner authoritative, old owner drops the partition.
6. Redirect/rollback retained until old routers/clients drain; reads may consult both source and target during cutover.

No two nodes own the same partition concurrently (split-brain prevented by single-epoch map + fencing, coordinated with `ids-lease`/`ids-byzantine` for fencing epochs).

#### 3.6 Logical-to-Physical Mapping Stability During Migration

The logical partition id is immutable across moves; only the catalog entry (physical node/H-unit/region) changes. Indirection (vBucket/vShard pattern) means clients key on logical partition id, resolve via current map, and never rehash keys on topology change. During migration: writes fenced to old owner until cutover; target catches up via snapshot+replay; new epoch published; old owner retains redirect briefly; in-flight requests resolved through redirect. Cutover verified before old state deleted. This preserves logical identity, access policy, and history during region/node moves (integrates with `ids-mobility`).

#### 3.7 Partition-Level Integrity & Checksums

Each partition's metadata includes per-partition checksums (blake3/sha256 over boundaries + content digest) verified at split/merge/move; integration with `ids-integrity` for end-to-end checksums, Merkle trees per partition, replica audits, silent bit-rot detection. Partition-level integrity verification before ownership transfer acceptance.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Split point chosen in sparse key region → uneven daughters, repeated splits, "partition explosion" | Sample key distribution; median split key; hysteresis (split >1.5×S, merge <0.5×S); reject boundaries violating min/max size |
| Split/merge thrash under oscillating load | Hysteresis + cooldown window (`split-merge-interval`); minimum time between split/merge on same partition |
| Two owners for one partition (split-brain) | Single authoritative epoch in versioned map; fencing before transfer; atomic publication; `ids-lease`/`ids-byzantine` fencing epochs |
| Logical partition id changes on move → client remapping | Immutable logical id; indirection via versioned partition map; only physical location updated |
| Data loss during split/merge (writes after snapshot lost) | Dual-application/log capture post-snapshot writes; replay to children; catch-up to frontier before cutover |
| Over-capacity partition after split/merge | Component-wise capacity fit test per daughter/merged partition via `ids-resource`; reject if any dimension exceeds headroom |
| Correlated failure disables multiple replicas of one partition | Failure-domain independence at named level; correlation-graph DP check before placement; replicas across independent domains |
| Stale router cache routes to old owner after move | Redirect retained until drain; map epoch bump invalidates caches; clients re-resolve on redirect/moved response |
| Unauthorized region receives partition | Residency/jurisdiction constraints from `ids-placement-policy` checked before any placement; `ids-security` authorization |
| Metadata corruption (map/epoch) | Checksums (blake3/sha256) on every metadata payload; versioned map with fail-closed on mismatch; multiple historical snapshots |
| Rebalance churn / thundering herd on membership change | Bounded move fraction (~1/N per node change, AIStore HRW); cooldown windows; `ids-fault` circuit-breaker/bulkhead gating |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 010 ||
||-------|-------------------|-------------------|
| `ids-000-aa` | Canonical types: `PartitionId`, `DatasetId`, `NodeId`, `RegionId`, `HUnitId`, `RecordId`, `OpRef`, `OrderingPos`, `PlacementDecision`, `RedundancyMode`, `ConsistencyProfile`, `FailureDomain`; sealed `trait Distribution` contract |
| `ids-placement` | Produces `PlacementDecision` (where each partition goes); consumed as input to partition placement |
| `ids-partition-map` | Authoritative logical→physical partition map; versioned epochs; owned jointly with `ids-distribution` |
| `ids-topology` | FailureDomain hierarchy; domain independence verification; correlated-failure model |
| `ids-membership` | Node states; only active/degraded nodes eligible as partition owners |
| `ids-placement-policy` | Dataset policy: replica/fragment counts, required domains, redundancy mode |
| `ids-consistency-profile` | Durability/acknowledgement semantics per profile (affects partition replica requirements) |
| `ids-redundancy-mode` | Full replication vs erasure coding per dataset (determines fragment count per partition) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — partition map references endpoints; distribution never touches carriers directly |
| `ids-crypto` | Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (signed partition metadata/audit) |
| `ids-integrity` | Per-partition checksums, Merkle trees, integrity verification at split/merge/move |
| `ids-mobility` / `ids-selfreorg` | Drive region/node moves and placement revision; consume ownership-transfer protocol |

**Integration points:**
- `ids-placement` → `ids-distribution`: placement decisions become physical locations in the partition map.
- `ids-replication` consumes partition ownership + placement to place replicas/fragments.
- `ids-routing` resolves requests to partition owner using the versioned map.
- `ids-read`/`ids-write` target partitions by key → map → owner.
- `ids-query` decomposes queries across partitions using partition boundaries + ownership metadata.
- `ids-mobility`/`ids-selfreorg` trigger ownership transfer + map epoch changes.
- Carrier-agnostic: distribution never imports `std::net`/`tokio::net`/libp2p; all traffic via `ids-carrier::Endpoint`.

---

### 6. Proposed File Structure (~120–200 files, one new crate)

**Crate:** `ids-distribution` (new crate; depends on ids-000-aa, ids-placement, ids-partition-map, ids-topology, ids-membership, ids-placement-policy, ids-consistency-profile, ids-redundancy-mode, ids-crypto, ids-integrity; thiserror, serde, blake3)

```
ids-distribution/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Distribution constants (split/merge thresholds, hysteresis)
├── error.rs                        # DistributionError (thiserror, comprehensive)
├── features.rs                     # Feature flags (simulation, fuzzing, benchmarks)
│
├── partition/                      # Partition lifecycle core
│   ├── mod.rs
│   ├── types.rs                    # Partition, PartitionId, PartitionBoundaries
│   ├── partition_id.rs             # Stable logical partition identity
│   ├── boundaries.rs               # Key-range [start,end) or hash-bucket set
│   ├── lineage.rs                  # Parent/children lineage (split/merge genealogy)
│   ├── partition_state.rs          # Partition state machine (creating/active/splitting/merging/moving/retired)
│   ├── partition_proptests.rs
│   └── partition_fuzz.rs
│
├── key_mapping/                    # Key → partition resolution
│   ├── mod.rs
│   ├── resolver.rs                 # KeyMappingResolver trait + implementations
│   ├── hash_mapping.rs             # Consistent/rendezvous hashing (HRW)
│   ├── range_mapping.rs            # Range-based key partitioning
│   ├── directory_mapping.rs        # Directory/virtual-bucket versioned mapping
│   ├── mapping_selector.rs         # Picks strategy per dataset policy
│   └── mapping_proptests.rs
│
├── metadata/                       # Partition metadata & versioned map
│   ├── mod.rs
│   ├── partition_metadata.rs       # Epoch, version, owner, location, size, checksums
│   ├── map_epoch.rs                # Map epoch versioning + invalidation
│   ├── metadata_store.rs           # Persistent partition metadata (via ids-partition-map)
│   ├── checksum.rs                 # Per-partition blake3/sha256 checksums
│   └── metadata_proptests.rs
│
├── split_merge/                    # Split & merge controller
│   ├── mod.rs
│   ├── split_controller.rs         # Split triggers (size/load), median split key
│   ├── merge_controller.rs         # Merge triggers (adjacent, below threshold)
│   ├── split_plan.rs               # Durable split identity, daughters, cutover frontier
│   ├── merge_plan.rs               # Merge plan preserving both children
│   ├── atomic_publish.rs           # Atomic map-epoch publication + fencing
│   ├── hysteresis.rs               # Split/merge hysteresis (1.5×S / 0.5×S)
│   ├── cooldown.rs                 # Split-merge interval cooldown
│   └── split_merge_proptests.rs
│
├── ownership/                      # Partition ownership & transfer
│   ├── mod.rs
│   ├── owner.rs                    # Single write owner per partition
│   ├── ownership_transfer.rs       # Fence → snapshot → replay → cutover → publish
│   ├── fence.rs                    # Owner fencing (no new writes after fence point)
│   ├── cutover.rs                  # Cutover boundary verification
│   ├── redirect.rs                 # Redirect/rollback pointers during migration
│   ├── drain.rs                    # Old-router/client drain tracking
│   └── ownership_proptests.rs
│
├── distribution/                   # Distribution across nodes/H-units/regions
│   ├── mod.rs
│   ├── distributor.rs              # Places partitions per PlacementDecision
│   ├── placement_consumer.rs       # Consumes ids-placement decisions
│   ├── replica_set.rs              # Replica/fragment set per partition across domains
│   ├── domain_spread.rs            # Failure-domain independence for partition replicas
│   └── distribution_proptests.rs
│
├── migration/                      # Logical→physical stability during migration
│   ├── mod.rs
│   ├── move_coordinator.rs         # Coordinates partition move (region/node change)
│   ├── snapshot_transfer.rs        # Consistent snapshot + mutation replay
│   ├── catchup.rs                  # Catch-up to cutover frontier
│   ├── logical_stability.rs        # Immutable logical id across moves
│   └── migration_proptests.rs
│
├── integrity/                      # Partition-level integrity (integration)
│   ├── mod.rs
│   ├── partition_checksum.rs       # Checksum over boundaries + content digest
│   ├── merkle_partition.rs         # Per-partition Merkle tree
│   ├── integrity_verify.rs         # Verify before ownership transfer acceptance
│   └── integrity_proptests.rs
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_split_merge.rs
│   ├── fuzz_mapping.rs
│   ├── fuzz_ownership.rs
│   └── fuzz_migration.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── partition_tests.rs          # Partition lifecycle state machine
│   ├── mapping_tests.rs            # Hash/range/directory key resolution
│   ├── split_merge_tests.rs        # Split/merge behavior + hysteresis
│   ├── ownership_tests.rs          # Single owner, transfer protocol
│   ├── map_epoch_tests.rs          # Versioned map + cache invalidation
│   ├── distribution_tests.rs       # Placement across nodes/H-units/regions
│   ├── migration_tests.rs          # Logical id stability during moves
│   ├── integrity_tests.rs          # Per-partition checksums + Merkle
│   ├── integration_tests.rs        # End-to-end dataset → partitions → owners
│   ├── adversarial_tests.rs        # Split-brain, sparse boundaries, thrash
│   ├── determinism_tests.rs        # Same inputs → same partition map everywhere
│   ├── property_tests.rs           # Proptest properties
│   └── chaos_tests.rs              # Node churn during split/merge/move
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── mapping_bench.rs            # Key→partition resolution latency
│   ├── split_merge_bench.rs        # Split/merge planning benchmarks
│   ├── ownership_bench.rs          # Transfer cutover latency
│   └── map_epoch_bench.rs          # Map publication/invalidation overhead
│
├── spec/                           # Formal specifications
│   ├── distribution_spec.md        # Partition lifecycle & invariants
│   ├── split_merge_spec.md         # Split/merge atomic publication spec
│   └── tla/                        # TLA+/PlusCal (in ids-doc)
│
├── docs/                           # Documentation
│   ├── architecture.md             # Distribution crate architecture
│   ├── integration_guide.md        # Integrating with placement/replication/routing
│   └── mapping_guide.md            # When to use hash/range/directory mapping
│
└── test_vectors/                   # Deterministic test vectors
    ├── split_vectors.json
    ├── merge_vectors.json
    ├── mapping_vectors.json
    └── ownership_transfer_vectors.json
```

**Why ~120–200 files:** one file per responsibility — partition lifecycle + state machine, 3 key-mapping strategies + selector, metadata + map epoch + checksums, split/merge controller + plans + atomic publish + hysteresis + cooldown, ownership + transfer + fence + cutover + redirect + drain, distribution + replica set + domain spread, migration + snapshot transfer + catch-up + logical stability, integrity + Merkle, fuzzing, property/integration/adversarial/determinism/chaos tests, benchmarks, formal spec, docs, test vectors. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Partition lifecycle state machine
```
creating → active → {splitting | merging | moving | retired}
active --(split trigger)--> splitting --(atomic publish)--> active (daughters) + retired (parent)
active --(merge trigger)--> merging --(atomic publish)--> active (merged) + retired (children)
active --(move trigger)--> moving --(cutover publish)--> active (new owner) + retired (old owner)
```

#### 7.2 Key mapping resolution
```
resolve(key: RecordId, dataset: DatasetId) -> PartitionId:
  strategy = mapping_selector(dataset.policy)   // hash / range / directory
  bucket = strategy.hash_or_range(key)          // hash bucket or range index
  return partition_map.lookup(dataset, bucket, current_epoch)
```
Hash: consistent/rendezvous (~1/N remap on membership change). Range: ordered [start,end) boundary lookup. Directory: versioned bucket→owner table (indirection, no client remapping).

#### 7.3 Split with hysteresis
```
split_candidate(partition):
  if partition.size > 1.5 × S: return Split
  if partition.size < 0.5 × S and low_load(sustained): return Merge candidate
  else: NoOp
split_point = median_key(partition.keys, sample)
daughters = [start, split_point), [split_point, end)
```

#### 7.4 Atomic publication (split/merge/move)
```
publish(epoch': MapEpoch):
  1. prepare children/target from consistent snapshot
  2. capture post-snapshot writes (log/dual-application)
  3. bring to cutover frontier
  4. atomically publish map epoch', fence old owner
  5. retain redirect/rollback until drain
  6. delete obsolete state after no reader references it
```

#### 7.5 Ownership transfer
```
transfer(partition, new_owner):
  fence(old_owner)                      // no new writes
  snapshot = consistent_snapshot(old_owner)
  replay(mutations_after_snapshot)      // idempotent, keyed by OpRef/OrderingPos
  verify_catchup(target = frontier)
  publish_map_epoch(new_owner)          // atomic, single publication
  retain_redirect(old_owner) until drain
  old_owner.drops(partition)
```

#### 7.6 Per-partition checksum
```
checksum(partition) = blake3(boundaries || content_digest || epoch || owner)
```
Verified at split/merge/move; integrated with `ids-integrity` Merkle trees.

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
| `ids-000-aa` | Canonical types + sealed `trait Distribution` contract |
| `ids-placement` | `PlacementDecision` input for partition placement |
| `ids-partition-map` | Authoritative logical→physical partition map, versioned epochs |
| `ids-topology` | FailureDomain hierarchy, domain independence, correlation model |
| `ids-membership` | Node lifecycle states (eligible owners only) |
| `ids-placement-policy` | Dataset policy (replicas/fragments, domains, redundancy mode) |
| `ids-consistency-profile` | Durability/acknowledgement semantics per profile |
| `ids-redundancy-mode` | Full replication vs erasure coding per dataset |
| `ids-crypto` | Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-integrity` | Per-partition checksums, Merkle trees, integrity verification |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `blake3` | Hashing (allowed crypto) |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-distribution` is a partition lifecycle engine; all traffic via `ids-carrier::Endpoint`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-010.1 | Deterministic partitioning | Same dataset+key+map+policy → identical partition assignment on every replica (property test, 100k iterations) |
|| AC-010.2 | Logical id stability | Partition logical id immutable across split/merge/move; only physical location changes (migration test) |
|| AC-010.3 | Single write owner | Exactly one owner per partition at any epoch; no split-brain (ownership transfer test + adversarial) |
|| AC-010.4 | Split correctness | Daughters partition [start,mid) ∪ [mid,end) = parent range; no gaps/overlaps (unit + property) |
|| AC-010.5 | Merge correctness | Merged partition = union of adjacent children; both children preserved through one publication point (unit + adversarial) |
|| AC-010.6 | Hysteresis | No split/merge oscillation: split >1.5×S, merge <0.5×S, cooldown enforced (chaos/load-oscillation test) |
|| AC-010.7 | Atomic publication | Map epoch publication is atomic; old owner fenced before new owner authoritative (transfer test) |
|| AC-010.8 | Key mapping coverage | Every key resolves to exactly one partition; no key unmapped (mapping test, all strategies) |
|| AC-010.9 | Distribution across domains | Partition replicas/fragments spread across independent failure domains (domain-independence test) |
|| AC-010.10 | Capacity fit | Partition accepted only if every capacity dimension fits (component-wise test via ids-resource) |
|| AC-010.11 | Integrity verification | Per-partition checksum verified at split/merge/move; mismatch → reject transfer (integrity test) |
|| AC-010.12 | Cache invalidation | Map epoch bump invalidates router caches; stale reads get redirect/moved (migration test) |
|| AC-010.13 | Carrier agnosticism | Compiles/behaves identically with zero std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh deps |
|| AC-010.14 | Allowed crypto only | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 used; nothing else |
|| AC-010.15 | Adversarial tests pass | Split-brain, sparse boundaries, thrash, stale-cache, corruption tests all fail-safe |

---

### 10. Remaining Uncertainties

1. **Initial partition count per dataset:** fixed logical bucket count (e.g., 1024/4096) vs. dynamic single-starting-partition that auto-splits — no decision yet; both supported behind mapping strategy interface.
2. **Split/merge threshold defaults:** size S and load thresholds are workload-dependent; defaults are placeholders pending measurement (Bigtable 100–200 MB, TiKV 96 MB, MongoDB 64 MB, YugabyteDB 512 MB/10 GB reference points).
3. **Map epoch publication mechanism:** atomic publication via `ids-partition-map` vs. separate epoch service — interface defined, implementation target TBD.
4. **Ownership transfer catch-up protocol:** dual-application vs. log-replay for post-snapshot mutations; idempotent replay boundaries keyed by OpRef/OrderingPos need `ids-write`/`ids-commit-state` integration details.
5. **Partition metadata size growth:** lineage + redirect/rollback pointers accumulate; retention/GC policy for split/merge history undefined.
6. **Signed partition metadata:** whether map epochs/partition metadata carry ed25519 signatures (via `ids-crypto`) for audit is a product decision.
7. **Formal verification scope:** TLA+/PlusCal spec for partition invariants (single owner, atomic publication, logical id stability) deferred to `ids-doc`.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
| Partition as unit of distribution | Atom of placement in Dynamo/Bigtable/TiKV/CockroachDB/Couchbase; independently placeable, movable |
| Logical → physical indirection (versioned map) | Live migration/rebalance without client key remapping (Couchbase vBucket, NodeDB vShards) |
| Key mapping per dataset policy (hash/range/directory) | Hash for uniform equality, range for scans, directory for residency/uneven shards |
| Split/merge with hysteresis (1.5×S / 0.5×S) | Prevents partition explosion and split/merge thrash (dynamic partition management research) |
| Atomic publication + fencing | Single authoritative epoch; no split-brain; old owner fenced before new owner authoritative |
| Single write owner per partition | Simplifies consistency; ownership transfer via fence → snapshot → replay → cutover → publish |
| Immutable logical id across moves | Access policy, history, and client addressing preserved during region/node moves (`ids-mobility`) |
| Per-partition checksums + Merkle (ids-integrity) | Integrity at split/merge/move; bit-rot detection |
| New `ids-distribution` crate | User's separate-crates rule; clean dependency boundary between placement and replication/routing |
| Allowed crypto only | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; nothing else |
| Carrier-agnostic via `ids-carrier::Endpoint` | No `std::net`/`tokio::net`/libp2p/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 010.

**Sources consulted (30 targeted searches):**
- Consistent Hashing (Karger et al., MIT; ByteByteGo course) — https://bytebytego.com/courses/system-design-interview/design-consistent-hashing
- Consistent Hashing and Virtual Nodes for Stable Partition Mapping — https://systemoverflow.com/learn/partitioning-sharding/hash-partitioning/consistent-hashing-and-virtual-nodes-for-stable-partition-mapping
- Rendezvous hashing / HRW (Wikipedia; IEEE/ACM ToN HRW98 paper) — https://en.wikipedia.org/wiki/Highest_random_weight_hashing ; https://microsoft.com/en-us/research/wp-content/uploads/2017/02/HRW98.pdf
- Consistent Hashing with Bounded Loads (SODA'18; Google Research blog; AAAI-21) — https://research.google/blog/consistent-hashing-with-bounded-loads/ ; https://ojs.aaai.org/index.php/AAAI/article/view/16517
- CRUSH failure domains (Ceph; Weil et al. SC'06) — https://ceph.com/en/news/blog/2026/slicing-the-pie ; https://docs.ceph.com/en/latest/rados/operations/crush-map
- Bigtable (OSDI'06; tablets, split at 100-200 MB, METADATA hierarchy) — https://research.google.com/archive/bigtable-osdi06.pdf
- Bigtable CS261 notes (lexicographic ranges, tablet split by server) — https://read.seas.harvard.edu/~kohler/class/cs261-f11/bigtable.html
- Dynamic Partition Management: Splitting, Merging, Rebalancing (hysteresis 1.5×S/0.5×S) — https://systemoverflow.com/learn/partitioning-sharding/range-partitioning/dynamic-partition-management-splitting-merging-and-rebalancing
- Apache Cassandra Basics (ring, token ranges, partitioner) — https://cassandra.apache.org/cassandra-basics
- Cassandra ByteOrderedPartitioner (ordered partitioning) — https://docs.datastax.com/en/cassandra/2.0/cassandra/architecture/architecturePartitionerBOP_c.html
- Couchbase vBuckets (1024 fixed, CRC32(doc ID) → vBucket → vBucket map → server; active/replica) — https://docs.couchbase.com/server/current/learn/buckets-memory-and-storage/vbuckets.html
- vBuckets and vBucket Maps (bucket partitions, indirection) — https://developer.couchbase.com/documentation/server/4.5/architecture/core-data-access-vbuckets-bucket-partition.html
- TiKV Region (range [start,end), 96 MB target, split >144 MB, merge <20 MB, PD metadata, RegionEpoch) — https://tikv.github.io/tikv-dev-guide/understanding-tikv/scalability/region.html
- TiKV Region Merge Config (max-merge-region-size/keys, split-merge-interval) — https://tikv.org/docs/4.0/tasks/configure/region-merge
- Multi-Raft design (TiKV range sharding, ask_split/report_split) — https://tidb.io/blog/design-and-implementation-of-multi-raft
- CockroachDB Distribution Layer (ranges, meta1→meta2→ranges, pointer rebalance not data copy) — https://cockroachlabs.com/docs/v25.3/architecture/distribution-layer
- Spanner (shards across Paxos state machines, automatic resharding, directories/fragments) — https://cs.cornell.edu/courses/cs5414/2017fa/papers/Spanner.pdf
- Dynamo (consistent hashing, quorum, object versioning, gossip) — https://cs.cornell.edu/courses/cs5414/2017fa/papers/dynamo.pdf
- Kafka Basic Operations (partition count, hash(key)%N, replication factor) — https://kafka.apache.org/44/operations/basic-kafka-operations
- HDFS Architecture (rack-aware replica placement, failure domains) — https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/HdfsDesign.html
- YugabyteDB Auto-split (512 MB low phase, 10 GB high phase, 100 GB force split) — https://yugabyte.com/blog/distributed-sql-sharding-how-many-tablets-size
- OceanBase Auto Split (SIZE threshold, auto_split_tablet_size) — https://en.oceanbase.com/docs/common-oceanbase-database-10000000003454806
- MongoDB Sharding Internals (chunks, config server routing table, auto-balancer, auto-split, zones) — https://github.com/mongodb/mongo/wiki/Sharding-Internals/cbb11801713a058741bd46847ffed591f6211207
- Azure Sharding Pattern (lookup/directory, range-based, hash-based strategies) — https://learn.microsoft.com/en-us/azure/architecture/patterns/sharding
- Martin Fowler Key-Range Partitions — https://martinfowler.com/articles/patterns-of-distributed-systems/key-range-partitions.html
- LEAP Ownership Transfer (owner table, partitioner, transfer request/response/inform) — https://mwhittaker.github.io/papers/html/lin2016towards.html
- NodeDB vShards (virtual shards, catalog, atomic ownership transfer) — https://nodedb.dev/docs/architecture/vshards
- Shard Growth 20→200 (versioned routing, snapshot+replay, cutover, publish after target ready) — https://system-design-atlas.com/topics/shard-growth-20-200.html
- System Design Patterns: Partitioning Strategies (logical boundaries, split publication, replica placement) — https://github.com/babushkai/system-design-patterns/blob/main/02-distributed-databases/05-partitioning-strategies.md
- oteldb rebalance (ownership handoff not data copy, HRW ring, one-in/one-out) — https://pkg.go.dev/github.com/oteldb/storage/cluster/rebalance
- Topology-Aware Placement (Ondat TAP, failure domains via labels, best-effort) — https://docs.ondat.io/docs/concepts/tap
- Designing Data-Intensive Applications Ch.6 Partitioning (partition terminology, hash/range, fixed/dynamic/proportional) — https://github.com/ps06756/Designing-Data-Intensive-Applications/blob/main/chapter-06-partitioning.md
- Partitioning, Sharding, Skew, and Rebalancing (independently placeable units, split/merge/move/reshard) — https://swe.servos.dev/books/performance-engineering-system-design-handbook/part-03-reusable-mechanisms-and-trade-offs/chapter-026-partitioning-sharding-skew-and-rebalancing