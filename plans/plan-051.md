## Phase 051 — Create the ids-metadata crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 760-774)

```
51. Research only do not read plan.md **[Phase 051 — Create the ids-metadata crate: Metadata for regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments.]**

Crate: ids-metadata
Purpose: Metadata for regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments.
Contents: Hierarchical metadata model, versioned state, explicit relationships, logical identity stability during physical changes, metadata caching/invalidation/version checks.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 051 — Create the ids-metadata crate: Metadata for regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments.]**

Create plan-051.md in C:/idfs/plans/ for the ids-metadata crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 051 creates `ids-metadata`, the IDS crate that owns all metadata for regions, clusters, H-units, nodes, datasets, partitions, replicas, and fragments. It is a **separate crate** (not a folder inside an existing crate), per the directive "try to use separate crates instead of more folders in existing ones." `ids-metadata` defines the hierarchical metadata model, versioned state, explicit relationships, logical identity stability during physical changes, and metadata caching/invalidation/version checks. It does not open sockets or drive any transport directly: all metadata traffic is shipped as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

The metadata hierarchy is eight levels deep: **Region → Cluster → H-Unit → Node → Dataset → Partition → Replica → Fragment**. Every level is a first-class metadata object with a stable logical ID, a content-addressed checksum, a version vector, and an explicit relationship graph linking it to parents, children, and peers. `ids-metadata` is read-optimized and cache-aware: metadata objects carry versions, caches invalidate on version change, and version checks gate all reads/writes so stale metadata can never be silently applied.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-metadata` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-metadata` crate does NOT yet exist at `C:/idfs/ids-metadata/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Zarr hierarchy (tree of groups/arrays, NodePath → NodeMetadata mapping), Holt (embedded Rust metadata engine on a persistent Adaptive Radix Tree for path-shaped keys: prefix scans, delimiter rollups, conditional creates, atomic renames), tree-ds (generic Rust tree with LCA, graft/prune), ed25519-dalek (Ed25519 keygen/sign/verify with batch verification), chacha20poly1305 (ChaCha20Poly1305 + XChaCha20Poly1305 AEAD, RFC 8439, 24-byte extended nonce), blake3 (official Rust BLAKE3, SIMD, incremental hashing, XOF output), sha2 (Rust SHA-256/SHA-512 family), IDSA Knowledge Base catalog patterns (DCAT catalogs of datasets/distributions/offers; central vs federated catalog variants), GreptimeDB Metasrv (catalog/schema/table/region/node metadata, leader election, cache invalidation broadcasts), Milvus external catalog (namespace-isolated metadata, zero client cache → no invalidation/dirty reads), persistent identifiers with content-centric resolution targets (location-independent targets), Facebook "Cache made consistent" (version field for conflict resolution and invalidation), Confluent Schema Registry Avro (schema versioning, compatibility types), IPFS content addressing / CID / multihash (content-based identifiers, sha-256 default), Rust typestate pattern (state machines via type-level states), SirixDB (append-only, bitemporal, structural sharing, stable node IDs across revisions), barn (append-only versioned KV on object store, transaction log + checkpoints), event sourcing (append-only event store, replay to materialize state), Replikativ (metadata as CRDT for eventual consistency), Merkle-CRDTs (Merkle-DAGs meet CRDTs), CompassDB/Roindex (perfect hash, LSM-tree KV), jsonschema (Rust JSON Schema validator), ULID/Rusty ULID (128-bit lexicographically sortable time-based IDs), Apache Ignite data partitioning (partitions distributed across zone, replicas stored on separate nodes, rendezvous hashing, metastorage), Hazelcast data partitioning (partition owner, primary + backup replicas), MetaDB (hierarchical namespace mapped to flat schema, shard boundaries aligned with directory tree, containment colocation), RDF triplestores (subject-predicate-object triples, quads, named graphs for metadata), thiserror (Rust error derive macro), Apache Ignite/Hazelcast partitioning (consistent distribution), btrfs/ZFS checksumming (sha256/blake3 inline and detached checksums), SirixDB append-only versioning + event sourcing (versioned document stores, transaction logs, checkpoints).

---

### 2. Architecture Overview

`ids-metadata` sits above `ids-000-aa` (canonical types + sealed trait contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — hierarchical metadata model, versioned state, explicit relationships, logical identity stability, caching/invalidation/version checks — is an independent module. `ids-metadata` is a metadata registry-and-state layer: it does not store application payloads; it issues metadata IDs, versions metadata objects, maintains the relationship graph, and gates all metadata traffic before it reaches `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│                 APPLICATION / ids-* LAYER (metadata consumers)                       │
│  ids-store, ids-fragment, ids-replication — read/write metadata objects              │
├──────────────────────────────────────────────────────────────────────────────────────┤
│               Phase 051: ids-metadata (THIS CRATE — metadata registry & state)       │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────┐   │ │\n│  │ hierarchy        │ │ versioning       │ │ relationships    │ │ identity     │   │ │\n│  │ (region→cluster→ │ │ (version vectors,│ │ (containment,    │ │ (stable IDs    │   │ │\n│  │  hunit→node→…→   │ │  semver tags,    │ │  reference edges)│ │  across moves) │   │ │\n│  │  fragment)       │ │  version checks) │ │                  │ │              │   │ │\n│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └──────────────┘   │ │\n│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────┐   │ │\n│  │ caching          │ │ invalidation     │ │ checksums        │ │ carrier      │   │ │\n│  │ (metadata cache, │ │ (version-change  │ │ (sha256/blake3   │ │ adapter        │   │ │\n│  │  TTL + version)  │ │  invalidation)   │  content addr.)    │ │ (opaque Vec<u8>│   │ │\n│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─datagrams) │   │ │\n│  ┌──────────────────┐ ┌──────────────────┐                                  │   │ │\n│  │ contracts +      │ │ audit/logging    │                                  │   │ │\n│  │ invariants       │ │ (hash-chained)   │                                  │   │ │\n│  └──────────────────┘ └──────────────────┘                                  │   │ │\n└───────────────────────────────────────────────────────────────┬───────────────────┘\n                                                                │ opaque Vec<u8> datagrams\n┌───────────────────────────────────────────────────────────────▼───────────────────┐\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only                        │\n│  send_datagram(Endpoint{CarrierType ≤1024B addr, Vec<u8>}, payload: &[u8])     │\n│  receive_datagram → (Endpoint, Vec<u8>)                                          │\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS         │\n└──────────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of metadata registration:** caller submits a metadata-request datagram → `ids-metadata` assigns a stable logical ID (UUID v7 / ULID-style time-ordered ID), computes content checksum (blake3 primary, sha256 secondary), writes the metadata object into the versioned state (version vector + semver tag), records explicit relationship edges (parent/child/containment/reference), updates indexes, returns `MetadataCreateResponse{object_id, version, checksum, relationships}` as an opaque datagram to `ids-carrier::Endpoint`.

**Data flow of a metadata read:** caller sends `MetadataRead{object_id, expected_version}` → `ids-metadata` checks the local metadata cache first (version-match hit returns cached object; version mismatch → invalidate + refetch), validates the version check against the authoritative versioned state, verifies checksum (sha256/blake3) on the returned bytes, returns the metadata object datagram to `ids-carrier::Endpoint`.

**Metadata hierarchy (eight levels):** Region → Cluster → H-Unit → Node → Dataset → Partition → Replica → Fragment. Each level has: `ObjectId` (stable logical ID), `ParentId` (optional, roots are regions), `Level`, `Attributes` (key/value metadata), `Version` (semver + version vector), `Checksum` (content hash), `Relationships` (explicit edge set). Decommissioning/moving a physical node does not change its logical ID — identity is stable across physical changes (relocation, replacement, re-IP), consistent with persistent-identifier research (location-independent targets).

**Versioned state:** every metadata object carries a version vector (per-replica/per-writer Lamport-style counters) plus a human-readable semver tag. Writes require `expected_version` (optimistic concurrency); a mismatch rejects the write — this is the version check. Reads may specify `min_version` / `exact_version`; stale cache entries are invalidated on version change (Facebook cache-consistency pattern: version field enables conflict resolution and invalidation).

**Explicit relationships:** two edge kinds — **containment** (region contains cluster; cluster contains H-unit; …; replica contains fragment) forming the eight-level tree, and **reference** (dataset references partitions; fragment references source data; cross-hierarchy links). Relationships are first-class, versioned, and hash-chained in the audit log so the relationship graph is reconstructible at any version.

**Caching/invalidation:** metadata cache keyed by `ObjectId` with `(version, checksum)` entry; reads validate cache entry version against state before serving; any version bump invalidates all dependent cache entries (prefix-style invalidation for subtree reads, per Holt-style path-shaped prefix scans); TTL as secondary bound, never the only freshness mechanism.

**Crypto used:** ed25519 for signing metadata object envelopes and relationship edges; sha256/blake3 for content checksums and ID derivation; xchacha20poly1305/chacha20poly1305 to encrypt sensitive metadata at rest (e.g., location hints, credential references) when confidentiality is required.

---

### 3. Detailed Research Dimensions

#### 3.1 Hierarchical Metadata Model (Region → Cluster → H-Unit → Node → Dataset → Partition → Replica → Fragment)

Eight containment levels, each a first-class metadata object:
- **Region** — top-level administrative/fault domain; root of the hierarchy, no parent.
- **Cluster** — a group of nodes within a region; contained by exactly one region.
- **H-Unit** — hardware unit (rack/chassis/blade grouping) within a cluster; contained by one cluster.
- **Node** — compute/storage node within an H-unit; contained by one H-unit.
- **Dataset** — named logical data collection owned by a node; contained by one node.
- **Partition** — shard of a dataset; contained by one dataset.
- **Replica** — physical copy of a partition on a node; contained by one partition, located on one node (cross-reference to node level).
- **Fragment** — sub-piece of a replica (chunk/erasure fragment); contained by one replica.

Research anchors: Zarr hierarchy (tree of Groups/Arrays; `NodePath → NodeMetadata` mapping; `Hierarchy::tree` string representation) shows metadata objects keyed by path in a tree; Holt (persistent Adaptive Radix Tree for path-shaped metadata: prefix scans, delimiter rollups, conditional creates, atomic renames, directory-style emptiness checks) confirms metadata workloads are path-shaped, not random KV; tree-ds (generic Rust tree: LCA, graft/prune, subtree enumeration) provides the primitive for containment operations. Design decision: model the hierarchy as a content-addressed tree where each object's ID is derived from its attributes (blake3), so moving/subtree operations produce new versions while the logical ID stays stable.

#### 3.2 Versioned State & Version Checks

Every metadata object carries: (a) a **semver tag** (MAJOR.MINOR.PATCH, semver crate semantics: MAJOR = incompatible, MINOR = backwards-compatible add, PATCH = bug fix), and (b) a **version vector** (per-writer Lamport counters) for concurrent-writer detection. Writes carry `expected_version`; the store rejects writes whose `expected_version` ≠ current version (optimistic concurrency / CAS). Reads carry `min_version` / `exact_version` / `latest`; serving stale metadata is a correctness violation.

Research anchors: semver crate (Cargo's SemVer interpretation); Confluent Schema Registry Avro (every schema versioned with unique id + version number; compatibility types govern evolution — adopted here for metadata schema evolution); Facebook "Cache made consistent" (version field enables conflict resolution: older data should not overwrite newer; version = invalidation signal); SirixDB (append-only, bitemporal, structural sharing; stable node IDs across revisions; snapshot isolation) — metadata is append-only versioned state, never overwritten in place; barn (append-only versioned KV with transaction log + checkpoints + optimistic concurrency via PutIfAbsent). Decision: metadata state is append-only; each mutation writes a new versioned object and a transaction-log entry; checkpoints compact old versions.

#### 3.3 Explicit Relationships (Containment vs Reference)

Relationships are first-class versioned edges, not implicit naming conventions:
- **Containment edges**: region→cluster, cluster→hunit, hunit→node, node→dataset, dataset→partition, partition→replica, replica→fragment (tree; each non-root object has exactly one parent).
- **Reference edges**: dataset→partitions, replica→node (location), fragment→replica, fragment→source-data-hint, cross-cluster replication links.

Research anchors: RDF triplestore (subject→predicate→object triples; quads add named graph; named graphs store metadata about triples; triple attributes = key/value per triple) — relationships as explicit triples; IPLD/Content-addressed Merkle DAGs (CIDs identify content; DAG links represent relationships between content-addressed data; UnixFS for directories/files) — Merkle-DAG model for relationship graphs; MetaDB (containment hierarchy mapped to flat schema; shard boundaries aligned with directory tree so single-directory operations stay in one shard) — containment colocation for metadata locality. Decision: store relationships as `(subject_id, predicate, object_id, version)` triples in a relationship index; support parent/child traversal and LCA queries (tree-ds primitives).

#### 3.4 Logical Identity Stability During Physical Changes

Logical IDs (RegionId, ClusterId, HUnitId, NodeId, DatasetId, PartitionId, ReplicaId, FragmentId) are stable for the lifetime of the entity regardless of physical relocation, replacement, or re-addressing. A node moved from H-unit A to H-unit B keeps its `NodeId`; only its `ParentId` and location attributes change, producing a new version.

Research anchors: "On Constructing Persistent Identifiers with Persistent Resolution Targets" (PIDs embed location-independent targets; maintenance-free PIDs via content-centric networking); IPFS CID (content-based, not location-based: same content = same CID anywhere); UUID v7 / ULID (time-ordered 128-bit IDs, lexicographically sortable; Rust `uuid` crate v7 per RFC 9562; `ulid` crate 48-bit timestamp + 80-bit random). Decision: IDs are UUID v7 / ULID-style time-ordered identifiers (stable, sortable, collision-resistant), content checksums are blake3 (primary) + sha256 (secondary), never derived from volatile network addresses (carrier-agnostic: no IP dependence).

#### 3.5 Metadata Caching, Invalidation & Version Checks

Metadata is read-heavy; a local cache per metadata consumer is essential. Cache entry = `(object_id, version, checksum, object_bytes, expiry)`. On read: cache hit with matching version → serve; version mismatch → invalidate entry, refetch from authoritative state; checksum mismatch → reject (corruption/tampering) and signal audit. Subtree reads (e.g., "all datasets under cluster X") use prefix-style invalidation: any version bump anywhere in the subtree invalidates the subtree cache entry. TTL is a secondary bound; version change is the primary invalidation signal.

Research anchors: Holt (path-shaped metadata: prefix scans, delimiter rollups, directory-style emptiness checks — adopted for subtree cache invalidation); Facebook "Cache made consistent" (version field as invalidation signal; TTL-only caches are out of scope for invalidation); GreptimeDB Metasrv (broadcasts cache invalidations to Frontends/Datanodes/Flownodes on metadata change — adopted as the invalidation broadcast pattern over `ids-carrier::Endpoint` datagrams); Milvus external catalog (zero client cache → no invalidation/dirty reads — considered and rejected for `ids-metadata` because carrier-agnostic datagram latency makes cache-less reads too expensive; bounded cache with version checks chosen instead).

---

### 4. Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|---|---|---|
| Stale metadata silently applied | Metadata correctness | Version checks (`expected_version` on writes, `min_version`/`exact_version` on reads); cache invalidation on version change |
| Corrupted metadata bytes | Metadata integrity | sha256/blake3 content checksum verified on every read and before cache insert; fail closed on mismatch |
| Tampered metadata object / relationship | Metadata authenticity | ed25519 signatures over metadata envelopes and relationship edges; verify before apply |
| Unbounded metadata cache growth | Availability | Bounded cache with LRU + TTL eviction; version-change invalidation takes priority |
| Subtree cache poisoning after hierarchy change | Cache correctness | Prefix-style invalidation on any version bump within the subtree |
| Replayed old metadata version | Metadata integrity | Version vectors + semver ordering; stale-version reads rejected |
| Logical ID collision | Identity stability | UUID v7 / ULID time-ordered IDs + blake3-derived content IDs; duplicate-ID detection on registration |
| Sensitive metadata exposure at rest | Confidentiality | xchacha20poly1305/chacha20poly1305 encryption of sensitive attributes (location hints, credential refs) |
| Carrier duplicating/reordering datagrams | Replay / ordering | Carrier-agnostic: sequence + nonce in metadata envelopes; replay window per object (DTLS 1.3 / RFC 4303 ESP sliding-window pattern) |
| Relationship graph inconsistency | Metadata correctness | Hash-chained relationship log; relationship edits versioned and atomic per object |

The carrier is never trusted: metadata envelopes are signed and checksummed over payload bytes, surviving re-framing by any carrier; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-metadata |
|---|---|---|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; adds `ClusterId`, `HUnitId`, `DatasetId`, `PartitionId`, `ReplicaId`, `FragmentId`, `MetadataId`, `Version`, `RelationshipEdge` |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All metadata traffic is datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signatures (metadata envelopes), checksums, encryption of sensitive attributes |
| `ids-store` | Primary record store (metadata consumer) | All record metadata registered/read via `ids-metadata` |
| `ids-fragment` | Record fragments/shards (metadata consumer) | Fragment metadata (location, checksum, version) managed by `ids-metadata` |
| `ids-replication` | Replica synchronization | Replica metadata (placement, version, checksum) exchanged via `ids-metadata` |
| `ids-session` (Phase 050) | Client-session binding | Session-bound metadata reads/writes gated by session token + ed25519 PoP |

**Integration points:**
- `ids-000-aa` sealed trait contracts define the metadata object traits (versionable, checksummable, relational, hierarchical); `ids-metadata` implements them.
- All cross-crate metadata traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-metadata` never imports any networking crate.
- Cache-invalidation broadcasts are sent as datagrams to interested endpoints (GreptimeDB Metasrv broadcast pattern).

---

### 6. Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-metadata/` (a separate crate, not a folder inside an existing crate):

```
ids-metadata/
├── Cargo.toml
├── lib.rs                          # crate root, feature gates, public re-exports
├── constants.rs                    # protocol defaults (cache TTL, version vector width)
├── error.rs                        # MetadataError (thiserror, comprehensive)
├── features.rs                     # feature flags (encryption, fuzzing, durable)
│
├── hierarchy/                      # eight-level containment model
│   ├── mod.rs
│   ├── types.rs                    # Level enum, HierarchyPath, HierarchyError
│   ├── region.rs                   # Region metadata object
│   ├── cluster.rs                  # Cluster metadata object
│   ├── hunit.rs                    # H-Unit metadata object
│   ├── node.rs                     # Node metadata object
│   ├── dataset.rs                  # Dataset metadata object
│   ├── partition.rs                # Partition metadata object
│   ├── replica.rs                  # Replica metadata object
│   ├── fragment.rs                 # Fragment metadata object
│   ├── object.rs                   # MetadataObject base (id, attrs, version, checksum)
│   ├── root.rs                     # region root handling (no parent)
│   ├── subtree.rs                  # subtree enumeration, LCA, prefix ops
│   ├── move.rs                     # logical-move semantics (ID stable, parent changes)
│   └── index.rs                    # hierarchy index (level → id → object)
│
├── versioning/                     # versioned state & version checks
│   ├── mod.rs
│   ├── version.rs                  # Version (semver tag + version vector)
│   ├── version_vector.rs           # per-writer Lamport counters, merge, compare
│   ├── semver_tag.rs               # MAJOR.MINOR.PATCH parsing/compare
│   ├── expected_version.rs         # optimistic concurrency gate (CAS)
│   ├── read_guard.rs               # min_version / exact_version / latest read policies
│   ├── append_log.rs               # append-only version log (never overwrite)
│   ├── checkpoint.rs               # version checkpoints + compaction
│   └── history.rs                  # object version history traversal
│
├── relationships/                  # explicit relationship graph
│   ├── mod.rs
│   ├── edge.rs                     # RelationshipEdge (subject, predicate, object, version)
│   ├── kinds.rs                    # Containment vs Reference edge kinds
│   ├── graph.rs                    # relationship graph storage + traversal
│   ├── containment.rs              # containment-edge rules (single parent, acyclic)
│   ├── reference.rs                # reference-edge rules (location, cross-links)
│   ├── traversal.rs                # parent/child/descendant/ancestor queries
│   ├── lca.rs                      # lowest common ancestor (tree-ds pattern)
│   └── log.rs                      # hash-chained relationship edit log
│
├── identity/                       # logical identity stability
│   ├── mod.rs
│   ├── object_id.rs                # MetadataId / typed IDs (UUID v7 / ULID-style)
│   ├── stability.rs                # ID-stability guarantees across physical moves
│   ├── derivation.rs               # content-derived IDs (blake3 of attrs)
│   ├── collision.rs                # duplicate-ID detection
│   └── registry.rs                 # ID → object registry
│
├── caching/                        # metadata cache + invalidation
│   ├── mod.rs
│   ├── cache.rs                    # MetadataCache (object_id → (version, checksum, bytes))
│   ├── entry.rs                    # CacheEntry (version, checksum, expiry, hit count)
│   ├── invalidation.rs             # version-change + prefix-subtree invalidation
│   ├── policy.rs                   # TTL + LRU eviction bounds
│   ├── broadcast.rs                # invalidation broadcast via ids-carrier::Endpoint
│   ├── stats.rs                    # hit/miss/eviction metrics
│   └── guard.rs                    # read-through guard (cache → state → cache)
│
├── checksums/                      # content integrity
│   ├── mod.rs
│   ├── digest.rs                   # blake3 primary + sha256 secondary
│   ├── verify.rs                   # checksum verification on read/write
│   └── content_id.rs               # blake3-derived content identifiers
│
├── encryption/                     # at-rest encryption of sensitive attributes
│   ├── mod.rs
│   ├── envelope.rs                 # xchacha20poly1305 / chacha20poly1305 sealed envelope
│   ├── key_wrap.rs                 # attribute key wrapping
│   └── sensitive.rs                # sensitive-attribute marker type
│
├── carrier_adapter.rs              # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
├── contracts/                      # sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── metadata_contract.rs
│   ├── versionable_contract.rs
│   ├── checksummable_contract.rs
│   ├── hierarchical_contract.rs
│   ├── relational_contract.rs
│   ├── cacheable_contract.rs
│   ├── identity_contract.rs
│   ├── edition_contract.rs
│   └── error_contract.rs
├── invariants/                     # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── id_stability.rs             # ID unchanged across physical moves
│   ├── single_parent.rs            # containment acyclic + single parent
│   ├── version_monotonic.rs        # version vectors only advance
│   ├── checksum_match.rs           # checksum verified before cache insert
│   ├── cache_version_match.rs      # cache entry version == state version
│   ├── crypto_allowlist.rs         # only ed25519/xchacha20/sha256/blake3/chacha20
│   ├── carrier_agnostic.rs         # no std::net/tokio::net/libp2p references
│   ├── relationship_acyclic.rs     # reference-edge cycle constraints
│   └── checksum_on_read.rs         # every served object checksummed
│
├── tests/                          # property/verification tests (~25 files)
│   ├── mod.rs
│   ├── hierarchy_tests.rs
│   ├── versioning_tests.rs
│   ├── relationship_tests.rs
│   ├── caching_tests.rs
│   ├── invalidation_tests.rs
│   ├── checksum_tests.rs
│   ├── encryption_tests.rs
│   ├── identity_tests.rs
│   ├── replay_tests.rs
│   ├── carrier_adapter_tests.rs
│   └── property_tests.rs           # proptest properties
├── fuzz/                           # fuzzing harnesses (~6 files)
│   ├── mod.rs
│   ├── fuzz_hierarchy.rs
│   ├── fuzz_versioning.rs
│   ├── fuzz_relationships.rs
│   ├── fuzz_caching.rs
│   └── fuzz_envelope.rs
├── benches/                        # benchmarks (~6 files)
│   ├── mod.rs
│   ├── hierarchy_bench.rs
│   ├── versioning_bench.rs
│   ├── cache_bench.rs
│   └── checksum_bench.rs
├── spec/                           # normative specifications (~8 files)
│   ├── metadata_model_spec.md
│   ├── versioning_spec.md
│   ├── relationships_spec.md
│   ├── caching_spec.md
│   ├── identity_spec.md
│   ├── checksum_spec.md
│   ├── threat_model.md
│   └── carrier_boundary_spec.md
├── docs/                           # documentation (~12 files)
│   ├── architecture.md
│   ├── integration_guide.md
│   ├── metadata_object_guide.md
│   ├── versioning_guide.md
│   ├── caching_guide.md
│   ├── api_docs.md
│   └── design_rationale.md
└── test_vectors/                   # deterministic test vectors (~8 files)
    ├── hierarchy_vectors.json
    ├── version_vectors.json
    ├── relationship_vectors.json
    ├── checksum_vectors.json
    └── envelope_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — eight hierarchy levels plus object/subtree/move/index, versioning (version, vector, semver, CAS guard, read policies, append log, checkpoint, history), relationships (edge, kinds, graph, containment, reference, traversal, LCA, log), identity, caching (cache, entry, invalidation, policy, broadcast, stats, guard), checksums, encryption, contracts, invariants, tests, fuzz, benches, specs, docs, test vectors. The range reflects that each module can be split further (per-level files, per-invariant files) without merging concerns into folders inside other crates. No stubs; every file holds production-grade design with error handling, documentation, and tests at implementation time.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Metadata Object
```
MetadataObject {
    id: MetadataId,                 // UUID v7 / ULID-style time-ordered stable ID
    level: Level,                   // Region | Cluster | HUnit | Node | Dataset | Partition | Replica | Fragment
    parent_id: Option<MetadataId>,  // None only for Region roots
    attributes: BTreeMap<String, AttributeValue>,
    version: Version,               // semver tag + version vector
    checksum: Checksum,             // blake3 primary + sha256 secondary
    relationships: Vec<RelationshipEdge>,
    signature: Option<[u8; 64]>,    // ed25519 over canonical encoding
}
```

#### 7.2 Version Check (optimistic concurrency)
```
write(obj, expected_version):
    current = state.get(obj.id)
    if current.version != expected_version: reject(VersionMismatch)
    new = obj with version = current.version.next()
    state.append(new)                    // append-only, never overwrite
    log.transaction(new.version)         // transaction-log entry (barn/SirixDB pattern)
    invalidate_cache(obj.id, subtree(obj.id))
```

#### 7.3 Read with Cache + Version Check + Checksum
```
read(id, min_version):
    entry = cache.get(id)
    if entry && entry.version >= min_version && entry.checksum == state_checksum(id):
        return entry.bytes                       // cache hit, version-validated
    obj = state.get(id)                          // authoritative
    require(obj.version >= min_version)          // version check
    verify(obj.checksum, obj.bytes)              // blake3/sha256 verification
    cache.insert(id, obj)                        // (version, checksum, bytes)
    return obj.bytes
```

#### 7.4 Subtree Cache Invalidation
```
invalidate_subtree(root_id):
    for id in descendants(root_id):              // Holt prefix-scan over hierarchy index
        cache.remove(id)                         // version bump anywhere invalidates subtree
    broadcast_invalidation(root_id)              // datagram via ids-carrier::Endpoint (GreptimeDB pattern)
```

#### 7.5 Containment Edge Rule
```
add_containment(parent, child):
    require(child.parent_id == None || child.parent_id == parent.id)  // single parent
    require(acyclic(parent, child))                                   // no cycles
    edge = RelationshipEdge(parent.id, Contains, child.id, version)
    log.relationship(edge)                                            // hash-chained
```

#### 7.6 Content Checksum & ID Derivation
- `checksum = blake3(canonical_bytes(obj))` primary; `sha256` secondary for cross-algorithm verification (btrfs/ZFS pattern: inline + detached checksums).
- `MetadataId` may be content-derived (`blake3(attributes)`) for immutable metadata or UUID v7 / ULID for mutable entities requiring stable identity.

#### 7.7 Replay Protection (per metadata object)
Sliding-window `(seq, nonce)` bitmap per object (DTLS 1.3 / RFC 4303 ESP pattern): sender increments `seq` per mutation and attaches fresh random `nonce`; receiver accepts only in-window, unseen nonces; duplicates silently discarded; bounded replay cache with TTL eviction.

---

### 8. Dependencies

| Crate | Purpose |
|---|---|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) + new metadata IDs; sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` + `serde_bytes` | Serialization of metadata envelopes, version vectors, relationship edges |
| `thiserror` | Error types |
| `semver` | Semantic version tags for metadata object versions |
| `uuid` (v7) / `ulid` | Time-ordered stable logical IDs |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-051.1 | Eight-level hierarchy modeled (Region→Cluster→HUnit→Node→Dataset→Partition→Replica→Fragment) | unit test: each level is a distinct metadata object with correct parent/child rules |
| AC-051.2 | Logical ID stable across physical move | move node between H-units; assert `NodeId` unchanged, only `ParentId`/location versioned |
| AC-051.3 | Version check rejects stale writes | write with wrong `expected_version` → 100% rejected (VersionMismatch) |
| AC-051.4 | Read version check enforced | read with `min_version` above current → rejected; below → served |
| AC-051.5 | Content checksum verified on every read | corrupted bytes → 100% rejected with ChecksumMismatch, never served |
| AC-051.6 | Cache invalidation on version change | bump version → all cache entries for object + subtree invalidated within one invalidation cycle |
| AC-051.7 | Containment rules: single parent, acyclic | 100% of invalid containment edges rejected |
| AC-051.8 | Explicit relationships versioned & hash-chained | relationship edits produce new versions; log verifiable end-to-end |
| AC-051.9 | All metadata traffic via `ids-carrier::Endpoint` | zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS in crate |
| AC-051.10 | Only allowed crypto appears | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only |
| AC-051.11 | Metadata signature verification | unsigned/tampered envelopes rejected; 0 accepted without valid ed25519 signature |
| AC-051.12 | Replay protection | duplicate `(seq, nonce)` rejected; sliding window tolerates in-window reordering |
| AC-051.13 | Determinism | same seed + same operation sequence → identical metadata state and versions |
| AC-051.14 | Property tests pass | proptest: hierarchy invariants, version monotonicity, checksum match, 100k+ iterations |

---

### 10. Remaining Uncertainties

1. **ID scheme:** UUID v7 (RFC 9562, time-ordered) vs ULID (48-bit timestamp + 80-bit random) vs content-derived blake3 IDs — trade-off between sortability, immutability, and content-addressing not yet decided; all three are candidates behind one `MetadataId` type.
2. **Version vector width:** per-writer Lamport counters bounded how? Unbounded vectors grow with writer count; needs a compaction/pruning policy (stable versions truncated after all readers ack).
3. **Cache invalidation broadcast reliability:** carrier-agnostic datagrams may be lost; invalidation broadcast needs ack/retry or read-side version validation as the ultimate fallback (read-side validation already mandated — acceptable).
4. **Encryption scope:** which metadata attributes are encrypted at rest (xchacha20poly1305/chacha20poly1305) vs plaintext (e.g., public IDs, checksums) — classification policy pending.
5. **Cross-crate metadata convergence:** `ids-replication` must converge replica metadata across nodes; replication strategy (state transfer vs log replay) to be validated at integration.
6. **Relationship edge cardinality:** containment is single-parent by design; reference edges may be many-to-many — cycle constraints on reference edges need a defined policy (DAG vs arbitrary graph).
7. **Checksum dual-algorithm overhead:** storing both blake3 and sha256 per object doubles hash cost; whether both are mandatory or sha256 is optional/compatibility-only is unresolved.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate crate `ids-metadata`, not a folder | User directive: separate crates instead of more folders in existing ones |
| Eight explicit hierarchy levels as first-class objects | Matches prompt purpose: regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments |
| Logical IDs stable across physical changes | Persistent-identifier research: location-independent targets; carrier-agnostic (no IP-derived IDs) |
| Append-only versioned state + semver + version vectors | Confluent Schema Registry versioning; SirixDB append-only snapshots; Facebook version-field conflict resolution |
| Optimistic concurrency via `expected_version` CAS | barn/PutIfAbsent pattern; prevents lost updates under concurrent writers |
| Cache invalidation primarily by version change, TTL secondary | Facebook "Cache made consistent": TTL-only caches provide no invalidation; Holt prefix-scan for subtree invalidation |
| Content checksums: blake3 primary + sha256 secondary | btrfs/ZFS inline+detached checksum pattern; blake3 for performance, sha256 for cross-algorithm assurance |
| ed25519 signatures on metadata envelopes | Authenticity surviving carrier re-framing; ed25519-dalek batch verification available |
| All traffic via `ids-carrier::Endpoint` datagrams | Carrier-agnostic mandate; no network stack in the crate |
| Only ed25519 / xchacha20poly1305 / sha256 / blake3 / chacha20poly1305 | Allowed-crypto mandate |
| thiserror error type + per-module error handling | Library crate pattern: callers match on variants |
| Sliding-window `(seq, nonce)` replay protection | DTLS 1.3 / RFC 4303 ESP pattern; carrier may duplicate datagrams |
| Relationships as versioned, hash-chained triples | RDF triplestore explicit subject-predicate-object; IPLD Merkle-DAG relationship graph |

---

### 12. Sources

- Zarr hierarchy (tree of Groups/Arrays; NodePath → NodeMetadata; Hierarchy::tree): https://docs.rs/crate/zarrs/latest/target-redirect/src/zarrs/hierarchy.rs.html
- Holt — embedded Rust metadata engine on persistent Adaptive Radix Tree for path-shaped metadata (prefix scans, delimiter rollups, conditional creates, atomic renames): https://docs.rs/crate/holt/latest; https://users.rust-lang.org/t/holt-0-3-2-an-embedded-persistent-art-for-path-shaped-metadata/140228
- tree-ds — generic Rust tree (LCA, graft/prune, subtree ops): https://github.com/clementwanjau/tree-ds
- ed25519-dalek — Ed25519 keygen/sign/verify, batch verification: https://docs.rs/ed25519-dalek/latest/ed25519_dalek
- chacha20poly1305 — ChaCha20Poly1305 + XChaCha20Poly1305 AEAD (RFC 8439, 24-byte extended nonce): https://docs.rs/chacha20poly1305/latest/chacha20poly1305
- blake3 — official Rust BLAKE3 (SIMD, incremental hashing, XOF): https://github.com/blake3-team/blake3; https://docs.rs/blake3/latest/blake3
- sha2 — Rust SHA-256/SHA-512 family: https://docs.rs/sha2/latest/sha2
- IDSA Knowledge Base — catalog patterns (DCAT catalogs of datasets/distributions/offers; central vs federated): https://kb.internationaldataspaces.org/external/ram/Pattern/Catalogs
- GreptimeDB Metasrv — metadata/coordination service, cache invalidation broadcasts: https://docs.greptime.com/nightly/contributor-guide/metasrv/overview
- Milvus external catalog — namespace-isolated metadata, zero client cache: https://github.com/zilliztech/milvus-catalog
- Persistent identifiers with content-centric resolution targets: https://arxiv.org/pdf/2610.32247
- Facebook "Cache made consistent" — version field for conflict resolution/invalidation: https://engineering.fb.com/2022/06/08/core-infra/cache-made-consistent
- Confluent Schema Registry — schema versioning + compatibility types: https://docs.confluent.io/platform/7.3/schema-registry/avro.html
- IPFS content addressing — CID, multihash, content-based identifiers: https://github.com/ipfs/ipfs-docs/blob/main/docs/concepts/content-addressing.md
- Rust typestate pattern — type-level state machines: https://docs.rs/typestate/latest/typestate
- SirixDB — append-only, bitemporal, structural sharing, stable node IDs: https://github.com/sirixdb/sirix
- barn — append-only versioned KV (transaction log, checkpoints, optimistic concurrency): https://pkg.go.dev/github.com/org-runink/runi@v1.0.0/barn
- Event Sourcing pattern — append-only event store, replay to materialize: https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing
- Replikativ — metadata as CRDT for eventual consistency: https://replikativ.github.io/replikativ
- Merkle-CRDTs (Merkle-DAGs meet CRDTs): https://arxiv.org/html/2004.00107v2
- CompassDB/Roindex — perfect hash, LSM-tree KV index: https://arxiv.org/html/2406.18099v1
- jsonschema — Rust JSON Schema validator: https://docs.rs/jsonschema/latest/jsonschema
- ULID / rusty_ulid — 128-bit lexicographically sortable time-based IDs: https://docs.rs/ulid/latest/ulid/struct.Ulid.html; https://github.com/huxi/rusty_ulid
- Apache Ignite data partitioning — partitions, replicas on separate nodes, rendezvous distribution, metastorage: https://ignite.apache.org/docs/ignite3/latest/administrators-guide/storage/data-partitions
- Hazelcast data partitioning — partition owner, primary + backup replicas: https://docs.hazelcast.com/hazelcast/5.1-snapshot/overview/data-partitioning
- MetaDB — hierarchical namespace mapped to flat schema, containment colocation: https://arxiv.org/pdf/2610.32247
- RDF triplestore — subject-predicate-object triples, quads, named graphs for metadata: https://ontotext.com/knowledgehub/fundamentals/what-is-rdf-triplestore
- thiserror — Rust error derive macro for libraries: https://docs.rs/crate/thiserror/latest
- UUID v7 (RFC 9562) in Rust uuid crate: https://docs.rs/uuid/latest/uuid/struct.Uuid.html
- btrfs/ZFS checksumming — sha256/blake3 inline and detached checksums: https://btrfs.readthedocs.io/en/latest/Checksumming.html; https://github.com/openzfs/openzfs-docs/blob/master/docs/Basic%20Concepts/Checksums.rst

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 051.
