## Phase 054 — Create the ids-partition-map crate: Logical to physical partition mapping.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 805-819)

```
54. Research only do not read plan.md **[Phase 054 — Create the ids-partition-map crate: Logical to physical partition mapping.]**

Crate: ids-partition-map
Purpose: Logical to physical partition mapping.
Contents: Partition boundaries, key-range mapping, partition ownership, partition versioning, partition migration tracking, split/merge history.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 054 — Create the ids-partition-map crate: Logical to physical partition mapping.]**

Create plan-054.md in C:/idfs/plans/ for the ids-partition-map crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 054 creates `ids-partition-map`, the IDS crate responsible for logical-to-physical partition mapping: given a dataset-scoped logical partition (key-range or hash-bucket), resolve the physical node/H-unit/region that owns it, track partition versioning across topology changes, record split/merge history, and track partition migration progress. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by mapping domain.

The crate owns the partition map data structure itself — distinct from `ids-placement` (where data should live), `ids-distribution` (dataset division into independently placeable units), `ids-routing` (logical → physical request resolution), and `ids-mobility` (region handoff). `ids-partition-map` is the authoritative, versioned map: `PartitionId → KeyRange → Owner(Endpoint) → Epoch`, plus mutation history (splits, merges, migrations). All lookups are deterministic functions of the map version and the partition key; every mapping change bumps a monotonic epoch so stale readers are detectable.

**Core design:** a versioned partition map stored as an append-only sequence of map versions, each version an immutable snapshot of partition → (key-range, owner endpoint, placement) entries. A key-range index (sorted boundary list) enables O(log n) range lookup by key. Partition assignment to owners uses consistent-hashing family algorithms (jump consistent hash, rendezvous/HRW, AnchorHash/MementoHash) selected per dataset policy, so adding/removing owners remaps only a minimal fraction of partitions. Partition ownership changes carry fencing tokens/epochs (IDS-A fencing-epoch pattern) so stale owners cannot write after a handoff. Split and merge are recorded as versioned map transitions with child/parent partition IDs and boundary points, keeping history until all readers have drained past the cutover version.

`ids-partition-map` never touches a network stack. All cross-node map traffic rides opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa`. Timestamps/deadlines use `ids-time::IdsTime` (monotonic, never wall-clock for ordering).

Skeleton `ids-partition-map` does NOT exist yet at `C:/idfs/ids-partition-map/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract (`PartitionId`, `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `RecordId`, `OrderingPos`, `Endpoint`, `CarrierType`, `Epoch`, `IdsTime`, `PlacementDecision`, `FailureDomain`). Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Azure Cosmos DB logical→physical partition mapping (many logical partitions → fewer physical partitions, hash-based distribution), Kafka partition leader/ISR ownership and preferred-replica leadership balancing, Cassandra/DynamoDB consistent hashing with virtual nodes (vnodes), jump consistent hash (Lamping & Veach, O(log n), no memory, sequential bucket ids), rendezvous/HRW hashing (highest random weight, O(n) lookup, no ring metadata, good for small clusters), AnchorHash (minimal disruption + balance under arbitrary add/remove, ~1.69 avg hash lookups under 50% failure), MementoHash (stateful, minimal memory, handles random failures, no fixed cluster capacity), DxHash/BinomialHash (consistent hashing variants), CRUSH (hierarchical placement rules, failure-domain separation), MongoDB chunk auto-split/merge/balancer (128 MB chunks, migration thresholds, shardVersion bumping, config-server authoritative routing table), CockroachDB range leases + epoch-based fencing (leaseholder = write owner, epoch invalidation on node liveness change), fencing tokens / lease-based ownership transfer (quiesce → drain → checkpoint → activate), hot-partition mitigation (salting, time bucketing, load-based splitting, adaptive capacity), bounded-load consistent hashing (capacity constraints, overflow forwarding), shard map manager pattern (global + local + client cache with invalidation watchers).

---

### 2. Architecture Overview

`ids-partition-map` is the mapping layer between logical partitioning and physical placement. It sits above `ids-000-aa` canonical types and below/alongside `ids-placement`, `ids-distribution`, `ids-routing`, `ids-metadata`, `ids-mobility`, `ids-lease`, and `ids-topology`. It does not decide *where* a partition should go (that is `ids-placement`); it records *where each partition currently is*, at which version, and how the mapping has evolved.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    REQUEST ROUTER / ids-routing (consumer)                  │
│  resolve_logical(dataset_id, record_id) → Endpoint                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                 │ logical (dataset, partition/key)
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              ids-partition-map (THIS CRATE)                                 │
│  ┌──────────────┐  ┌────────────────┐  ┌──────────────────────────────┐   │
│  │  Map Store   │  │  Key-Range     │  │  Assignment (consistent hash)│   │
│  │  (versioned) │  │  Index         │  │  jump / HRW / AnchorHash     │   │
│  └──────┬───────┘  └──────┬─────────┘  └───────────┬──────────────────┘   │
│         │                 │                         │                      │
│  ┌──────▼─────────────────▼─────────────────────────▼──────────┐          │
│  │  Partition Map Version  V  (immutable snapshot)              │          │
│  │  PartitionId → {key_range, owner_endpoint, epoch, state}     │          │
│  └──────┬────────────────────────────────────────────────────────┘          │
│         │ owner changes / splits / merges / migrations                      │
│  ┌──────▼────────────────────────────────────────────────────────────────┐  │
│  │  History Log: split/merge/migration records (append-only, versioned)  │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                 │ opaque Vec<u8> datagrams
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType, address: Vec<u8> ≤ 1024 B}, payload)   │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow — lookup:** caller supplies `(dataset_id, partition_key)`; `ids-partition-map` hashes the key (blake3/sha256 via ids-crypto) or compares against key-range boundaries, selects the partition `PartitionId`, resolves the owner `Endpoint` at the current map version, returns it with the version/epoch so the caller can detect staleness. The lookup is read-only, deterministic, and requires no network round trip once the map version is cached locally.

**Data flow — map update (split/merge/migrate/rehash):** `ids-placement` proposes a new placement; `ids-partition-map` validates it against invariants (no gaps/overlaps in key ranges, owner capacity, failure-domain diversity via ids-topology), writes a new map version, appends a history record (split/merge/migrate), bumps the partition epoch, and invalidates stale cached copies via the map epoch. Ownership handoffs carry fencing tokens so the previous owner is fenced before the new owner serves writes (pattern from CockroachDB range leases + lease-machines handoff).

**No end-to-end path assumption:** the crate is a local deterministic mapping engine. It produces bytes (map versions, history records) that other crates ship over whatever carrier IDS uses. It never opens a socket, never parses an IP address, never depends on a carrier by name.

---

### 3. Detailed Research Dimensions

#### 3.1 Logical vs physical partition mapping

A **logical partition** is a dataset-scoped unit of data identified by `PartitionId` and a key range (or hash bucket). A **physical partition** is the actual storage owner: a `NodeId`/`Endpoint` (plus H-unit/region placement from `ids-topology`/`ids-placement`). The logical→physical map decouples the data layout from the physical infrastructure: many logical partitions can map to fewer physical nodes (Azure Cosmos DB virtual-partition pattern), and the mapping can change (rebalance, failover, migration) without applications changing how they address data.

IDS conventions from `ids-000-aa`/`types.rs`: `PartitionId(pub [u8;32])`, `NodeId(pub [u8;32])`, `RegionId`, `HUnitId`, `DatasetId`, `Endpoint{CarrierType, address: Vec<u8>}`, `PlacementDecision{region, hunit, node, redundancy, failure_domain}`, `FailureDomain{power, facility, network_segment, operator, geographic}`. The map key type is `PartitionId`; the value includes `Endpoint`, `Epoch`, `PlacementDecision`, and partition state.

Design decision: map is keyed by `PartitionId` (not raw key), with a separate key-range index for key→partition resolution. This matches IDS's existing `PartitionId` canonical type and keeps the map stable under key-range re-splits (partition IDs are stable; only ranges move).

#### 3.2 Key-range mapping

Each partition owns a half-open key interval `[lower, upper)` over a normalized key space (raw record keys or blake3/sha256 hashes of keys, depending on dataset partition strategy: range vs hash). Boundaries are stored as `Vec<u8>`; lookup is a binary search over a sorted boundary list → O(log n) partition selection. Invariants (non-negotiable): no gaps, no overlaps, union covers the full key space, boundaries strictly increasing.

Split picks a boundary point inside an overloaded partition's range and creates two children `[lower, split)` and `[split, upper)`, each getting a new `PartitionId`; the parent is marked `Splitting` then `Retired` after all readers drain past the cutover version. Merge is the inverse: adjacent partitions with the same owner policy combine into one, parents marked `Merging` then `Retired`. Both operations are versioned: a new map epoch is published atomically (all-or-nothing entry replacement) so readers never observe a partially split/merged map.

Reference patterns: MongoDB chunk auto-split (128 MB default, splitAt/splitFind, shardVersion bumped on migration, config-server authoritative routing table), Azure Cosmos DB partition split/merge (elastic operations, logical partitions stable through physical split), RASK range-conscious split (split point minimizes range fragmentation while balancing entry count), B+-tree median split (fence keys, suffix truncation) as the on-disk analogue for boundary selection.

#### 3.3 Partition ownership

Every partition has exactly one owning `Endpoint` at a given map epoch (the owner serves reads/writes for that partition; replicas are handled by `ids-replication`/`ids-crdt`, not this crate). Ownership is expressed as `Endpoint` + `Epoch` (fencing epoch). A reader that presents a stale epoch is redirected or rejected; a writer must hold the current fencing token (IDS-000-aa `FencingToken`, `ids-lease` lease-based fencing) or the write is refused at the storage boundary (Twitter fencing pattern: only the newest valid owner mutates).

Ownership transfer (migration/handoff) follows a quiesce → drain → checkpoint → activate sequence under a newer fencing token (lease-machines `handoff_machines` pattern): source stops accepting new writes, drains in-flight ops, checkpoints state, target activates under the new epoch; only after target acknowledges does the map publish the new owner. The old owner is fenced by epoch comparison at the resource boundary.

Reference patterns: Kafka partition leader + ISR (one leader per partition, preferred-replica leadership balancing, leader only from ISR), CockroachDB range leases (leaseholder = write owner; epoch-based leases invalidated when node liveness epoch changes), fencing tokens / lease-based ownership (monotonic token, stale writes rejected at resource boundary).

#### 3.4 Partition versioning

The map itself is versioned with a monotonic `Epoch` (ids-000-aa `Epoch(pub u64)`). Each map publication increments the epoch. Each partition also carries a per-partition epoch/version that increments on ownership change, split, merge, or migration. Readers cache a map version; if a reader's cached epoch is older than the current epoch, its cached mappings may be stale and must be refreshed. Clients/routers carry the map epoch with routed requests; receivers check authority: if current, execute; if stale-but-forwardable, redirect with newer metadata; if ambiguous during transition, reject with retryable error rather than accept under the wrong generation (pattern from system-design partition-map ownership).

Version storage: append-only sequence of immutable map versions; old versions retained until no reader references them (gc at causal stability / reader drain, similar to CRDT tombstone GC). This supports rollback/redo of a failed split/merge by replaying to a prior version.

#### 3.5 Partition migration tracking

A migration moves a partition's ownership (and data, coordinated by `ids-replication`/`ids-mobility`) from a source `Endpoint` to a target `Endpoint`. `ids-partition-map` records a `MigrationRecord`: partition id, source, target, start `IdsTime`, fencing token, state (`Quiesce | Drain | Checkpoint | Activate | Complete | Abort`), and progress (bytes/counts drained, checkpoints transferred). The map entry stays pointing at the source until `Activate`, then flips to the target under the new epoch. On failure, the record supports abort/resume: the source remains owner, the target is told to discard partial state.

Progress is tracked so the control plane can observe lag and so a crashed migrator can resume from the last checkpoint rather than restarting. Migration records are append-only and versioned; completed migrations are compacted after all readers have drained past the cutover epoch.

#### 3.6 Split/merge history

Every split and merge is appended to a history log: `{op: Split|Merge, epoch, parent_ids, child_ids, boundary, owner_before, owner_after, ids_time}`. The history is append-only and never rewritten (immutable audit trail). It enables: reconstructing a partition's lineage (which parent a record came from), debugging routing anomalies, and replaying map evolution for tests. History entries are keyed by epoch and indexed by partition id for fast lineage queries. Retention: kept until all nodes have acknowledged the cutover version (causal stability), then compacted; never deleted while any reader might reference the old mapping.

#### 3.7 Assignment algorithms (consistent-hashing family)

How partitions map to owners when the owner set changes:

- **Jump consistent hash** (Lamping & Veach): O(log n) lookup, zero per-bucket memory, minimal key movement when bucket count changes, but requires sequential bucket numbering (0..n) and cannot handle arbitrary removal or heterogeneous capacity. Best for homogeneous, append-only owner sets.
- **Rendezvous / HRW** (highest random weight): O(n) hash computations per lookup, no ring metadata, perfect minimal disruption under arbitrary add/remove, uniform distribution, easy capacity weighting. Best for small/mid owner sets (under ~100 nodes) where lookup volume is moderate.
- **AnchorHash**: minimal disruption + balance under arbitrary add/remove, ~1.69 avg hash lookups even under 50% adversarial removal, low memory (few bytes per resource), stateful (depends on history of removals). Best for dynamic owner sets requiring strong consistency.
- **MementoHash**: stateful, minimal memory, handles random failures, no fixed cluster capacity, optimal lookup when removals are LIFO; scales to millions of nodes. Best for very large, churn-prone owner sets.

IDS decision: support jump hash (default, homogeneous), HRW (weighted, small clusters), and AnchorHash/MementoHash (dynamic, large clusters) as pluggable `PartitionAssignment` strategies behind one trait, selected per dataset in `ids-placement-policy`. Consistent hashing with virtual nodes is considered but rejected as primary (ring metadata overhead, rebalancing complexity) in favor of the simpler jump/HRW/AnchorHash family; vnodes remain an option for heterogeneous capacity.

#### 3.8 Hotspots and skew mitigation

Hash-based mapping spreads keys uniformly but cannot prevent a single popular key/range from becoming a hot partition. Mitigations (policy, not core): key salting (composite `bucket#key` key), time bucketing (finite-lifespan time ranges), load-based splitting (split below normal size threshold when QPS exceeds limit), adaptive capacity (shift throughput to hot partitions temporarily), hysteresis/cooldown on rebalancing to avoid thrash. `ids-partition-map` exposes per-partition load signals (read/write counts via `ids-observability`) so `ids-placement`/`ids-selfreorg` can trigger splits, but the crate itself does not auto-split (that is a policy decision).

#### 3.9 Map caching and invalidation

Clients/routers cache the map locally. Invalidation model: global shard map (authoritative, versioned) + local shard map (per-node redundant copy) + application cache (Azure SQL shard-map-manager pattern). On a map epoch change, caches are invalidated; on a per-partition migration, only affected partition entries are refreshed. Negative entries (key not in any partition) are not cached long-term; a read that fails against a cached location drops that shard from the map and re-checks on a short window (SeaweedFS EC shard-map pattern) to avoid paying cross-party recovery on stale locations.

#### 3.10 Determinism and reproducibility

All map operations are deterministic functions of (map version, partition key, owner set, assignment algorithm, seed). Same input → same mapping on every replica, so every node can compute the owner locally without a coordinator. This is essential for carrier-agnostic operation: no central lookup service, no network round trip per request.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Stale owner writes after handoff (split-brain) | Fencing token/epoch per partition; resource boundary rejects writes not carrying the current epoch (ids-lease + ids-000-aa FencingToken) |
| Stale cached map routes writes to wrong owner | Carry map epoch with routed requests; receivers reject/redirect on epoch mismatch; never downgrade epoch |
| Gaps/overlaps in key ranges after split/merge | Validate invariants (no gaps, no overlaps, full coverage) before publishing a new map version; fail closed |
| Owner failure leaves partition unowned | Failure detection via ids-failure-detector; placement engine reassigns; map marks partition `Degraded` until new owner confirmed |
| Hot partition overload | Load-based split policy; adaptive capacity; salting/time-bucketing at the key-encoding layer (ids-distribution/ids-query) |
| Map version corruption / torn write | Append-only immutable versions; blake3/sha256 checksum per version; fail closed on checksum mismatch |
| Migration loses data (source deleted before target acked) | Migration record state machine; source retains data until target acknowledges `Complete`; abort on failure |
| Replay of old map/history | Monotonic epochs; readers never accept an epoch lower than their cached highest; fencing tokens single-use per ownership grant |
| Prohibited crate / crypto sneaking in | ids-crypto is the sole crypto surface; only the five allowed primitives; CI dependency audit forbids std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |

Security risks: stale owner (mitigated by fencing), cache poisoning (mitigated by epoch validation), metadata bomb (map size/depth budgets at ingestion), key-range DoS (oversized boundary lists rejected).

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 054 |
|-------|-------------------|
| `ids-000-aa` | Canonical types (`PartitionId`, `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `RecordId`, `OrderingPos`, `Endpoint`, `CarrierType`, `Epoch`, `IdsTime`, `PlacementDecision`, `FailureDomain`, `FencingToken`), sealed trait contracts, invariants; master control point |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B), `send_datagram`/`receive_datagram`; the only carrier boundary ids-partition-map touches |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; ids-partition-map consumes primitive IDs, never implements crypto |
| `ids-time` | Monotonic IDS standard time for map epochs, migration deadlines, fencing (never wall-clock for ordering) |
| `ids-placement` | Proposes where partitions should live; ids-partition-map records the resulting mapping |
| `ids-distribution` | Dataset division into partitions; supplies initial partition boundaries and key ranges |
| `ids-routing` | Consumer: resolve_logical(dataset, key) → Endpoint using the partition map |
| `ids-metadata` | Hierarchical metadata; map versions stored/versioned as metadata |
| `ids-topology` | Failure-domain diversity validation for owner placement (power/facility/network/operator/geographic separation) |
| `ids-lease` / `ids-fencing-epoch` | Lease + fencing token issuance for ownership transfers; epoch invalidation on node liveness change |
| `ids-mobility` | Dataset/partition region handoff; consumes migration records |
| `ids-replication` / `ids-crdt` | Replicas of a partition (not ownership); out of scope for this crate |
| `ids-failure-detector` | Detects owner failure → triggers reassignment |
| `ids-observability` | Per-partition load signals (read/write counts) for split/merge policy |
| `ids-selfreorg` | Triggers rebalancing that produces new map versions |
| `ids-testing` | Property-based/adversarial harness patterns; ids-partition-map is measured, not the test harness |

**Integration points:** ids-partition-map exposes read-only lookup (`resolve_partition(dataset_id, key) → (PartitionId, Endpoint, Epoch)`) and versioned map publication (called by placement/mobility/selfreorg). All cross-node map traffic is opaque `Vec<u8>` to `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders, one new crate)

**Crate:** `ids-partition-map` (new crate; depends on ids-000-aa, ids-carrier, ids-crypto, ids-time, ids-topology + serde, thiserror, bytes; dev-only: proptest, tempfile; optional `bench` feature for criterion/divan)

```
ids-partition-map/
├── Cargo.toml                          # Crate manifest
├── lib.rs                              # Crate root, feature gates, public re-exports
├── constants.rs                        # Protocol defaults (boundary limits, cache TTLs)
├── error.rs                            # PartitionMapError (thiserror): mapping, invariant, version errors
├── features.rs                         # Feature flags (bench, fuzz, std/no-std)
│
├── map/                                # Versioned partition map core
│   ├── mod.rs
│   ├── partition_map.rs                # Immutable map version: PartitionId → entry
│   ├── map_version.rs                  # MapVersion { epoch, entries, checksum, ids_time }
│   ├── entry.rs                        # PartitionEntry { key_range, owner, epoch, state, placement }
│   ├── version_sequence.rs             # Append-only sequence of map versions
│   ├── snapshot.rs                     # Map snapshot for cache / export
│   ├── checksum.rs                     # blake3/sha256 checksum per map version
│   └── gc.rs                           # Retain versions until all readers drain past cutover epoch
│
├── keyrange/                           # Key-range mapping and boundary index
│   ├── mod.rs
│   ├── key_range.rs                    # KeyRange { lower, upper, inclusive/exclusive }
│   ├── boundary_index.rs               # Sorted boundary list; binary search → partition
│   ├── invariant.rs                    # No gaps, no overlaps, full coverage validation
│   ├── split_point.rs                  # Split boundary selection (median, range-conscious)
│   └── coverage.rs                     # Full key-space coverage check
│
├── assignment/                         # Partition → owner assignment strategies
│   ├── mod.rs
│   ├── strategy.rs                     # PartitionAssignment trait
│   ├── jump_hash.rs                    # Jump consistent hash (O(log n), zero memory)
│   ├── rendezvous.rs                   # Rendezvous/HRW hashing (O(n), no ring metadata)
│   ├── anchor_hash.rs                  # AnchorHash (minimal disruption + balance, stateful)
│   ├── memento_hash.rs                 # MementoHash (minimal memory, random-failure tolerant)
│   ├── virtual_nodes.rs                # Optional consistent-hashing vnodes (heterogeneous capacity)
│   └── strategy_selector.rs            # Per-dataset policy → strategy
│
├── ownership/                          # Partition ownership and fencing
│   ├── mod.rs
│   ├── owner.rs                        # Owner { endpoint, epoch, fencing_token }
│   ├── ownership_transfer.rs           # Quiesce → drain → checkpoint → activate handoff
│   ├── fencing.rs                      # Fencing token validation at resource boundary
│   ├── lease_holder.rs                 # Leaseholder = write owner (CockroachDB range-lease pattern)
│   └── epoch.rs                        # Per-partition epoch management
│
├── migration/                          # Partition migration tracking
│   ├── mod.rs
│   ├── migration_record.rs             # {partition, source, target, start, token, state, progress}
│   ├── migration_state.rs              # Quiesce | Drain | Checkpoint | Activate | Complete | Abort
│   ├── migration_tracker.rs            # In-flight migrations, resume from checkpoint
│   └── migration_gc.rs                 # Compact completed migrations after reader drain
│
├── history/                            # Split/merge history log
│   ├── mod.rs
│   ├── history_entry.rs                # {op, epoch, parent_ids, child_ids, boundary, ids_time}
│   ├── split_record.rs                 # Partition split record
│   ├── merge_record.rs                 # Partition merge record
│   ├── lineage.rs                      # Partition lineage reconstruction (parent chain)
│   └── history_log.rs                  # Append-only, versioned history storage
│
├── cache/                              # Local map cache + invalidation
│   ├── mod.rs
│   ├── map_cache.rs                    # Local in-memory cache of map versions
│   ├── invalidation.rs                 # Epoch-based invalidation (global + per-partition)
│   ├── watcher.rs                      # Invalidation watchers (remote-invalidation pattern)
│   └── negative_cache.rs               # Short-lived negative entries (key → no partition)
│
├── routing/                            # Read-only logical → physical resolution
│   ├── mod.rs
│   ├── resolver.rs                     # resolve_partition(dataset_id, key) → (PartitionId, Endpoint, Epoch)
│   ├── redirect.rs                     # Stale-epoch redirect with newer metadata
│   └── authority.rs                    # Authority check: current / stale-forwardable / ambiguous
│
├── policy/                             # Hotspot and split/merge policy hooks
│   ├── mod.rs
│   ├── load_signal.rs                  # Per-partition read/write load (from ids-observability)
│   ├── split_policy.rs                 # Load-based / size-based split triggers
│   ├── merge_policy.rs                 # Cold-partition merge triggers
│   └── rebalance_hook.rs               # Hook for ids-selfreorg to propose new versions
│
├── wire/                               # Serialization of map versions / history
│   ├── mod.rs
│   ├── format.rs                       # Versioned wire envelope (CBOR/bincode via ids-000-aa contracts)
│   ├── cbor.rs                         # CBOR encoding (deterministic)
│   └── checksum.rs                     # Payload checksums (sha256/blake3)
│
├── crypto/                             # Allowed-crypto usage boundary
│   ├── mod.rs
│   ├── hashing.rs                      # blake3 / sha256 key hashing for partition selection
│   └── signing.rs                      # ed25519 signing of map versions (optional, via ids-crypto)
│
├── fuzz/                               # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_map.rs
│   ├── fuzz_keyrange.rs
│   ├── fuzz_assignment.rs
│   └── fuzz_migration.rs
│
├── tests/                              # Integration & adversarial tests
│   ├── mod.rs
│   ├── map_tests.rs                    # Versioned map round-trip
│   ├── keyrange_tests.rs               # Boundary invariant checks
│   ├── assignment_tests.rs             # Minimal disruption under add/remove
│   ├── ownership_tests.rs              # Fencing, handoff, stale-owner rejection
│   ├── migration_tests.rs              # Migration state machine, abort/resume
│   ├── split_merge_tests.rs            # Split/merge history, lineage
│   ├── cache_tests.rs                  # Invalidation, negative cache
│   ├── integration_tests.rs            # End-to-end resolve_partition
│   ├── adversarial_tests.rs            # Torn writes, gap/overlap, replay
│   ├── property_tests.rs               # Proptest invariants
│   └── determinism_tests.rs            # Same input → same mapping on all replicas
│
├── benches/                            # Benchmarks
│   ├── mod.rs
│   ├── lookup_bench.rs                 # Key-range lookup latency
│   ├── map_publish_bench.rs            # Map version publication cost
│   ├── assignment_bench.rs             # Jump/HRW/AnchorHash/Memento lookup cost
│   └── migration_bench.rs              # Migration record overhead
│
├── spec/                               # Normative specifications
│   ├── partition_map_spec.md           # Map version lifecycle spec
│   ├── keyrange_spec.md                # Key-range boundary spec
│   ├── assignment_spec.md              # Assignment algorithm spec
│   ├── ownership_spec.md               # Fencing + handoff spec
│   ├── migration_spec.md               # Migration state machine spec
│   └── threat_model.md                 # Threat model document
│
├── docs/                               # Documentation
│   ├── architecture.md                 # Map architecture & layering
│   ├── integration_guide.md            # Integrating with ids-routing/ids-placement/ids-mobility
│   ├── api_docs.md                     # Public API documentation
│   └── design_rationale.md             # Why jump/HRW/AnchorHash over vnode rings
│
└── test_vectors/                       # Deterministic test vectors
    ├── map_vectors.json
    ├── keyrange_vectors.json
    ├── assignment_vectors.json
    └── migration_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — map versions, key-range index, boundary invariants, split-point selection, four assignment strategies + selector, ownership transfer + fencing, migration state machine + tracker + GC, split/merge history + lineage, cache + invalidation + watchers, resolver + authority, policy hooks, wire format, crypto boundary, fuzz, tests, benches, specs, docs, test vectors. The count scales with the number of assignment-strategy variants and policy hooks; the core is ~90 files, the full production build approaches the upper bound. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Versioned partition map

```
MapVersion {
    epoch: Epoch,                          // monotonic, ids-000-aa Epoch
    entries: BTreeMap<PartitionId, PartitionEntry>,
    checksum: [u8; 32],                    // blake3 over canonical encoding
    ids_time: IdsTime,
}
PartitionEntry {
    id: PartitionId,
    key_range: KeyRange,                   // [lower, upper)
    owner: Endpoint,                       // ids-carrier::Endpoint
    owner_epoch: Epoch,                    // fencing epoch for this owner
    placement: PlacementDecision,          // region/hunit/node/redundancy/failure_domain
    state: PartitionState,                 // Active | Splitting | Merging | Moving | Degraded | Retired
}
```

Lookup: `resolve_partition(dataset_id, key)` → hash key (blake3/sha256) or boundary-binary-search → `PartitionId` → entry at current `MapVersion`. Returns `(PartitionId, Endpoint, owner_epoch)`. All replicas computing from the same `MapVersion` get the same answer (determinism).

#### 7.2 Key-range lookup

```
boundaries: Vec<(Vec<u8> lower_bound, PartitionId)>  // sorted ascending
lookup(key): binary search for greatest lower_bound ≤ key; return that partition
Invariant: boundaries cover the full key space, strictly increasing, no gaps/overlaps
Split at point p: replace parent range [l, u) with [l, p) and [p, u); new child PartitionIds
Merge adjacent [a, b) + [b, c): single [a, c); parents retired
```

#### 7.3 Assignment algorithms

**Jump consistent hash:** `jump(key_u64, num_buckets) → bucket ∈ [0, num_buckets)`; O(log n), no memory; minimal movement when bucket count changes; requires sequential bucket ids.

**Rendezvous / HRW:** `weight(key, node_i) = hash(key || node_i)`; owner = argmax weight; O(n) per lookup; no ring metadata; arbitrary add/remove remaps only affected partitions; capacity weighting trivial.

**AnchorHash:** keys hashed into successively smaller working sets until one resource remains; minimal disruption + balance under arbitrary add/remove; ~1.69 avg lookups under 50% removal; few bytes per resource; stateful (depends on removal history).

**MementoHash:** Jump core + memory of removed buckets; minimal memory, handles random failures, no fixed cluster capacity; optimal when removals are LIFO.

#### 7.4 Ownership transfer (handoff)

```
start(source_token): scope the transfer under source's fencing token
quiesce: source stops accepting new writes
drain: in-flight ops complete or time out (IdsTime-bounded, not RTT)
checkpoint: source state transferred to target
activate: target becomes owner under newer fencing token; map flips owner
abort: on any failure, source remains owner, target discards partial state
Fencing: every write carries owner_epoch; resource boundary rejects epoch < current
```

#### 7.5 Split / merge

```
split(parent, boundary p):
  1. choose p (median or range-conscious to minimize fragmentation)
  2. create children c1=[l,p), c2=[p,u); new PartitionIds
  3. publish new MapVersion (epoch+1): parent→Retired, children→Active
  4. append SplitRecord {parent, c1, c2, p, epoch, ids_time} to history
merge(c1=[a,b), c2=[b,c)):
  1. create parent p=[a,c); new PartitionId
  2. publish new MapVersion: c1,c2→Retired, p→Active
  3. append MergeRecord {c1, c2, p, b, epoch, ids_time}
```

#### 7.6 Migration record

```
MigrationRecord {
    partition_id: PartitionId,
    source: Endpoint,
    target: Endpoint,
    fencing_token: FencingToken,
    start_ids_time: IdsTime,
    state: Quiesce | Drain | Checkpoint | Activate | Complete | Abort,
    drained_bytes: u64,
    checkpoint_bytes: u64,
}
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (PartitionId, NodeId, Endpoint, Epoch, IdsTime, PlacementDecision, FailureDomain, FencingToken), sealed trait contracts |
| `ids-carrier` | Opaque Endpoint (CarrierType + Vec<u8>); only carrier boundary |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Monotonic IdsTime for epochs, deadlines, fencing |
| `ids-topology` | FailureDomain for owner placement validation |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3`, `sha256` | Key hashing and map-version checksums (via ids-crypto primitive IDs) |
| `ed25519` | Optional map-version signing (via ids-crypto) |
| `proptest` | Property-based testing |
| `criterion` / `divan` | Benchmarks (optional feature) |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS). No `std::net`/`tokio::net`. No crypto beyond the five allowed primitives. `ids-crypto` does not depend on `ids-000-aa` (circular-dependency rule); `ids-partition-map` depends on both.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-054.1 | Deterministic lookup | Same (map version, key, owner set, algorithm) → same PartitionId + Endpoint on every replica; unit + determinism test |
| AC-054.2 | Key-range invariants | No gaps, no overlaps, full coverage; invariant check rejects invalid maps before publication |
| AC-054.3 | Lookup complexity | O(log n) boundary lookup (binary search); bench |
| AC-054.4 | Minimal disruption | Adding/removing one owner remaps only ~1/N of partitions (jump hash / AnchorHash / MementoHash); assignment test |
| AC-054.5 | Fencing | Write with epoch < current owner epoch rejected at boundary; stale owner cannot mutate after handoff |
| AC-054.6 | Split correctness | Children cover parent's range exactly; parent retired; history record appended; lineage reconstructable |
| AC-054.7 | Merge correctness | Parents retired; single parent covers union; history record appended |
| AC-054.8 | Migration state machine | Quiesce→Drain→Checkpoint→Activate→Complete; abort on failure leaves source as owner |
| AC-054.9 | Map versioning | Each publication bumps epoch; stale-epoch readers redirected/rejected; never downgrade epoch |
| AC-054.10 | Cache invalidation | Epoch change invalidates local cache; per-partition migration invalidates only affected entries |
| AC-054.11 | Carrier agnosticism | Compiles with zero std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS; all traffic via ids-carrier::Endpoint |
| AC-054.12 | Allowed crypto only | Dependency audit confirms only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 |
| AC-054.13 | Determinism test vectors | Same seed + scenario → identical map versions and history |
| AC-054.14 | Adversarial tests | Torn writes, gap/overlap injection, stale-map replay all fail-safe |

---

### 10. Remaining Uncertainties

1. **Assignment-strategy default:** jump hash (homogeneous, append-only) vs AnchorHash (dynamic, arbitrary churn) as the default for IDS; depends on expected cluster churn profile. Decision deferred to `ids-placement-policy` owners.
2. **Key space normalization:** whether partitions map raw record keys or blake3/sha256(key) hashes; affects range-query efficiency (hashing destroys ordering). Range datasets need raw-key ranges; hash datasets need hashed buckets. Both must be supported.
3. **Map version storage backend:** in-memory only vs optional durable backend (sled via ids-storage); durability requirements for map versions across restarts undefined.
4. **Reader-drain GC:** how long to retain old map versions/history before compaction; depends on reader count and churn; causal-stability gate needed but reader-tracking mechanism undefined.
5. **Split/merge trigger policy:** load-based vs size-based vs manual; thresholds and hysteresis are workload-specific, not research conclusions.
6. **Map distribution:** how map versions are pushed to clients/routers (gossip, pull, push) — out of scope for this crate (consumed by ids-routing/ids-metadata); integration point provisional.
7. **Fencing token issuance:** ids-lease/ids-fencing-epoch provides tokens; the exact handoff protocol (when epoch becomes authoritative) needs product decision, not research conclusion.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-partition-map` crate | Project rule: separate crates for new capabilities; keeps ids-distribution/ids-placement/ids-routing focused |
| Map keyed by PartitionId + separate key-range index | Stable partition IDs under range re-splits; matches ids-000-aa canonical type |
| Versioned immutable map + monotonic Epoch | Deterministic lookups; stale-reader detection; atomic split/merge publication |
| Jump / HRW / AnchorHash / MementoHash as pluggable strategies | Minimal disruption under owner changes; no single algorithm fits all cluster sizes/churn profiles |
| No virtual-node ring as primary | Ring metadata overhead and rebalancing complexity; vnodes remain optional for heterogeneous capacity |
| Fencing token + per-partition owner epoch | Prevents stale-owner writes after handoff (CockroachDB range-lease + Twitter fencing pattern) |
| Handoff = quiesce→drain→checkpoint→activate | Safe ownership transfer without data loss (lease-machines pattern) |
| Append-only split/merge history + lineage | Audit trail; partition lineage reconstruction; replayable map evolution |
| blake3/sha256 checksum per map version; fail closed on mismatch | Detect torn/corrupt map writes |
| ids-time::IdsTime for all epochs/deadlines | Never wall-clock for ordering (IDS standard time) |
| All traffic via ids-carrier::Endpoint | Carrier-agnostic; no network-stack dependencies |

---

### 12. Sources consulted (30 targeted searches)

- Azure Cosmos DB partitioning overview (logical → physical partitions, hash-based distribution, many logical → fewer physical): https://learn.microsoft.com/en-us/azure/cosmos-db/partitioning-overview
- Azure Cosmos DB partitioning / logical and physical shards: https://learn.microsoft.com/en-us/Azure/documentdb/partitioning
- Azure Architecture Center sharding pattern (virtual partitioning, lookup strategy, shard map database): https://learn.microsoft.com/en-us/azure/architecture/patterns/sharding
- Azure SQL Database elastic scale shard map management (GSM/LSM/application cache): https://learn.microsoft.com/en-us/azure/azure-sql/database/elastic-scale-shard-map-management
- Kafka design: replication, partition leader, ISR, preferred replica, leader election: https://kafka.apache.org/44/design/design
- Confluent Kafka replication and committed messages: https://docs.confluent.io/kafka/design/replication.html
- Kafka basic operations (replication factor, partition count, preferred replica election): https://kafka.apache.org/10/operations/basic-kafka-operations
- KIP-1151 minimal movement replica balancing: https://wiki.apache.org/confluence/spaces/KAFKA/pages/345378006/KIP-1151
- Cassandra architecture: Dynamo consistent hashing, token ring, vnodes: https://cassandra.apache.org/doc/5.0/cassandra/architecture/dynamo.html
- Cassandra partitioning: consistent hashing, token ring, vnodes: https://axonops.com/docs/data-platforms/cassandra/architecture/distributed-data/partitioning/
- Jump consistent hash (Lamping & Veach, arXiv:1406.2294): https://arxiv.org/pdf/1406.2294
- go-jump implementation: https://github.com/dgryski/go-jump/blob/master/jump.go
- Rendezvous hashing / HRW (Wikipedia): https://en.wikipedia.org/wiki/Highest_random_weight_hashing
- Weighted HRW (IETF draft): https://ietf.org/archive/id/draft-ietf-bess-weighted-hrw-02.html
- Consistent vs rendezvous hashing comparison: https://en.wikipedia.org/wiki/Rendezvous_hashing
- Hash-based assignment strategies (consistent vs rendezvous, vnodes): https://systemoverflow.com/learn/partitioning-sharding/rebalancing/hash-based-assignment-strategies-consistent-and-rendezvous-hashing
- AnchorHash (arXiv:1812.09674 / IEEE): https://arxiv.org/pdf/1812.09674.pdf
- AnchorHash Rust crate: https://docs.rs/anchorhash/latest/anchorhash/struct.AnchorHash.html
- MementoHash (arXiv:2306.09783): https://arxiv.org/pdf/2306.09783
- DxHash (arXiv:2107.07930): https://arxiv.org/html/2107.07930v2
- BinomialHash (arXiv:2406.19836): https://arxiv.org/pdf/2406.19836.pdf
- Consistent hashing with bounded loads (Google Research / arxiv:1608.01350): https://research.google/blog/consistent-hashing-with-bounded-loads
- Revisiting consistent hashing with bounded loads (arxiv:1908.08762): https://arxiv.org/html/1908.08762v2
- CRUSH: Controlled Replication Under Scalable Hashing (Weil et al., SC06): https://ceph.com/assets/pdfs/weil-crush-sc06.pdf
- CRUSH Maps (Ceph docs): https://docs.ceph.com/en/latest/rados/operations/crush-map/
- DHT / Chord lookup (Princeton COS418, Cornell CS6410): https://cs.princeton.edu/courses/archive/fall22/cos418/docs/L9-dht-chord.pdf
- MongoDB sharding internals (config server, routing table cache, shardVersion, moveChunk, balancer): https://github.com/mongodb/mongo/wiki/Sharding-Internals/ea79e3172b630d02be84d20c804c25fd20fced28
- MongoDB sharded cluster balancer / chunk migration: https://docs.mongodb.org/master/core/sharding-chunk-migration
- MongoDB data partitioning with chunks (auto-split, 128 MB, automerger): https://docs.mongodb.org/manual/core/sharding-chunk-splitting
- CockroachDB range leases RFC: https://github.com/cockroachdb/cockroach/blob/master/docs/RFCS/20160210_range_leases.md
- CockroachDB replication layer (leaseholder, epoch-based leases): https://cockroachlabs.com/docs/v25.4/architecture/replication-layer
- Fencing in distributed systems (Twitter approach, monotonic fencing tokens): https://positioniseverything.net/fencing-in-distributed-systems-twitters-approach
- lease-machines / handoff-machines (ownership transfer, fencing tokens): https://docs.rs/handoff-machines/latest/handoff_machines
- Hot partition problem and skew mitigation (salting, time bucketing, load-based splitting): https://systemoverflow.com/learn/partitioning-sharding/range-partitioning/hot-partition-problem-monotonic-keys-and-skew-mitigation
- Dynamic partition management (auto split/rebalance/adaptive capacity): https://systemoverflow.com/learn/partitioning-sharding/hotspot-handling/dynamic-partition-management-auto-splitting-rebalancing-and-adaptive-capacity
- RASK range-conscious split / range fragmentation: https://arxiv.org/html/2601.14129v1
- SeaweedFS EC shard-map stale-mark re-check pattern: https://github.com/seaweedfs/seaweedfs/commit/cd5013f1168f34beabc1e86f9c6d6a282430b42e
- BeeGFS client metadata caching / invalidation watchers: https://doc.beegfs.io/8.4/advanced_topics/client_meta_caching.html
- B+-tree split (fence keys, suffix truncation, Lehman-Yao): https://cs.cit.tum.de/fileadmin/w00cfj/dis/papers/btrees-are-back.pdf
- Azure Cosmos DB activity logs for elastic split/merge operations: https://learn.microsoft.com/en-us/azure/Cosmos-db/monitor-activity-logs-elastic-operations

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 054.
