## Phase 030 — Create the ids-integrity crate: Data integrity verification and bit rot detection.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 451-454)
"Phase 030 — Create the ids-integrity crate: Data integrity verification and bit rot detection."

Crate: `ids-integrity`
Purpose: Data integrity verification and bit rot detection.
Contents: End-to-end checksums, content hashes, Merkle trees, version comparisons, replica audits, partition-level integrity verification, continuous integrity scrubbing, silent bit rot detection, checksum mismatch handling, ZFS-style end-to-end protection.

"try to use separate crates instead of more folders in existing ones"

Create plan-030.md in C:/idfs/plans/ for the ids-integrity crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.

---

### 1. Executive Summary

Phase 030 creates `ids-integrity`, the IDS crate for data integrity verification and bit rot detection. It owns end-to-end checksums, content hashes, Merkle trees, version comparisons, replica audits, partition-level integrity verification, continuous integrity scrubbing, silent bit rot detection, checksum mismatch handling, and ZFS-style end-to-end protection. Every stored record, partition, and replica is checksummed; background scrubs recompute checksums against stored values to find silent corruption before reads hit it; mismatches are reported, reconstructed from redundant copies, and rewritten to healthy media.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-integrity` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-integrity` crate does NOT yet exist at `C:/idfs/ids-integrity/`. `ids-000-aa` already declares canonical types `Checksum` (`src/types.rs:1973`), `Anomaly` / `AnomalyType` (`src/types.rs:1934` — ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption), plus `SnapshotId`, `FragmentId`, `RecoveryProof` — all referenced by this crate but never redefined. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned ZFS end-to-end checksums (Bonwick 2005; OpenZFS docs; UWisc FAST'10 fault injection), ZFS/Btrfs scrub and self-healing, Merkle trees as self-validating block-pointer trees, anti-entropy Merkle reconciliation (Dynamo/Cassandra/ScyllaDB; Merkle Search Trees IEEE 9049566), dm-verity/fs-verity block-level integrity, end-to-end argument (Saltzer/Reed/Clark 1984), silent data corruption studies (CERN Xyratex; NetApp 1.5M drives/41 months/400k+ mismatches; Bairavasundaram et al. SOSP), bit half-life and petabyte-scale integrity, content-addressable storage (Git SHA-1/SHA-256), content-defined chunking (Rabin fingerprint; restic), SHA-256 vs CRC32 collision probability, T10 DIF/DIX end-to-end data integrity field, fixity checking / digital preservation auditing, fs-verity, cold-storage bit rot scrubbing, erasure coding (Reed-Solomon) repair.

---

### 2. Architecture Overview

`ids-integrity` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — checksums, content hashes, Merkle trees, version comparison, replica audit, partition verification, scrubbing, bit rot detection, mismatch handling — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-* LAYER (integrity consumers)           │
│  ids-store, ids-sync, ids-replication, ids-write — consume integrity proofs │
├─────────────────────────────────────────────────────────────────────────────┤
│                    Phase 030: ids-integrity (THIS CRATE)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │
│  │ end-to-end   │ │ content      │ │ Merkle       │ │ version      │    │ │
│  │ checksums    │ │ hashes       │ │ trees        │ │ comparison   │    │ │
│  │ (Checksum)   │ │ (BLAKE3/     │ │ (per block/  │ │ (OrderingPos/│    │ │
│  │              │ │ sha256)      │ │ record/part)│ │ Version)     │    │ │
│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │
│  │ replica      │ │ partition    │ │ integrity    │ │ bit rot      │    │ │
│  │ audits       │ │ verification │ │ scrubbing    │ │ detection    │    │ │
│  │ (Anomaly)    │ │ (per part)   │ │ (continuous) │ │ (silent)     │    │ │
│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │
│  ┌─────────────────────────────────────────────────────────────────────┐ │ │
│  │ checksum mismatch handling · reconstruction · verify · rewrite     │ │ │
│  └─────────────────────────────────────────────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────────┬───────────────┘
                                                              │ opaque Vec<u8> datagrams
┌─────────────────────────────────────────────────────────────▼───────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType, Vec<u8>≤1024}, payload: &[u8])        │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `OrderingPos`, `Version`, `SnapshotId`, `Checksum`, `Anomaly`, `AnomalyType`, `FragmentId`, `RecoveryProof`, `Endpoint`, `CarrierType`) imported from `ids_000_aa::X`, never redefined.
- `ids-integrity` implements the sealed `trait Integrity` contract from `ids-000-aa/src/contracts/` (checksums, scrub, audit, verify) as declared in the master crate; downstream crates use it as a trait bound, cannot implement it.
- All integrity metadata (checksums, digests, proofs) is transported as opaque bytes via `ids-carrier::Endpoint`. No IP addresses, ports, sockets.
- Crypto surface: sha256/blake3 for content hashes and checksums; ed25519 (optional) for signing integrity proofs/audit records; xchacha20poly1305/chacha20poly1305 only if checksum payloads need encryption (integrity crate does not encrypt by default). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Integrity is dataset-scoped; no global integrity master (invariant `no_global_master` in `ids-000-aa`).
- Operation lifecycle reuses `ids-000-aa` `OpState` and `CommitState`; integrity findings are `Anomaly` records raised through `ids-000-aa` error/annex channels.

---

### 3. Detailed Research Dimensions

#### 3.1 Silent Data Corruption and Bit Rot

Bit rot = random bit flips on storage media with no announced error. Research basis:
- CERN/Xyratex study (Kelemen 2007): ~3000 nodes, ~5 weeks, 500 errors on 100 nodes; 80% were 64K-region corruptions correlated with disk drop-out; single-bit errors correlated with ECC memory errors. Silent corruption is a fact of life.
- NetApp study (Bairavasundaram et al., SOSP): 1.53M disk drives over 41 months; >400,000 checksum mismatches; 8.5% of nearline drives and 1.9% of enterprise drives developed corruption; a large fraction went undetected until RAID restoration.
- Bit half-life model ("Keeping Bits Safe", CACM): at current enterprise storage error rates (~1e-9 bytes corrupted per 6 months at CERN), reaching petabyte-for-a-century durability requires ~1e9 improvement — hence checksums + scrubbing + redundancy are mandatory.
- Silent corruption is invisible to SMART/URE reporting; only application-level or filesystem-level checksums detect it. RAID scrub alone cannot identify which copy is correct.

#### 3.2 End-to-End Checksums (ZFS-style)

Research basis:
- ZFS end-to-end data integrity (Bonwick 2005; OpenZFS docs; UWisc FAST'10): checksums stored in parent block pointers, not with the block itself — fault isolation between data and checksum. Every block checksummed by its parent; the whole pool is a self-validating Merkle tree rooted at the uberblock. Detects bit rot, phantom writes, misdirected reads/writes, DMA parity errors, driver bugs.
- On every read: recompute block checksum, compare to parent-block-pointer checksum. On mismatch with redundancy (mirror/RAID-Z), fetch good copy, return it, rewrite bad block. Without redundancy: detect and report exactly which file/block is affected.
- Scrub walks all data, verifies each block checksum, repairs where redundancy allows. Finds latent corruption in unread (cold) data.

#### 3.3 Content Hashes

Research basis:
- Content-addressable storage (Git; IPFS; Nix; Docker): object address = hash of content. Same content → same hash → dedup + integrity. Git uses SHA-1 (transitioning to SHA-256); if content doesn't match its name, corruption detected.
- Allowed IDS hashes: sha256 (FIPS 180-4), blake3 (tree hash, incremental, fast, 256-bit default output). CRC32 is weaker (1/(2^32) undetected error probability) and not in the allowlist; sha256/blake3 give cryptographic-strength accidental-corruption detection.

#### 3.4 Merkle Trees

Research basis:
- Merkle tree = binary hash tree; leaves = data block/content hashes; internal nodes = hash of children. Root match ⇒ whole dataset matches (collision-resistant assumption); root mismatch ⇒ descend into differing branches; locate k divergent leaves in O(k·log n) bandwidth vs O(n) full transfer.
- Dynamo/Cassandra/ScyllaDB anti-entropy: Merkle trees per replica; compare roots; descend only mismatching subtrees; stream only divergent ranges. Cassandra uses depth-15 compact trees (2^15 leaves).
- Merkle Search Trees (IEEE 9049566): preserve key order for sequences/maps; important when integrity verification must align with ordered key spaces.
- dm-verity/fs-verity (Linux kernel): block device / file Merkle trees verified on every read; root hash as trust anchor; EIO/SIGBUS on mismatch. Read-only authenticity model.

#### 3.5 Version Comparisons

Every record carries `OrderingPos` (u64, monotonic per dataset domain) and `Version` (u64, MVCC) from `ids-000-aa`. Integrity verification compares versions across replicas: divergent `OrderingPos`/`Version` for the same `RecordId` indicates replica drift; checksum comparison on top detects whether the drift is silent corruption or legitimate concurrent update.

#### 3.6 Replica Audits

An audit compares replica states for a dataset/partition: exchange checksum roots or Merkle roots; mismatch triggers per-range descent to locate corrupted records; `Anomaly` records (ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption) raised per `ids-000-aa`. Audit scope: full replica, partition subset, or record range. Audit results are reproducible and version-pinned.

#### 3.7 Partition-Level Integrity Verification

Dataset is partitioned (`PartitionId`). Integrity verification runs per partition: each partition has a checksum root and per-record/content hashes. Verification can target a single partition (fast, scoped) or the whole dataset. Partition checksums feed a dataset-level Merkle-like aggregation for O(log partitions) divergence location.

#### 3.8 Continuous Integrity Scrubbing

Research basis:
- ZFS scrub (OpenZFS docs): walks all pool data, verifies checksums, repairs via redundancy. Recommended monthly for pools with unread data.
- Btrfs scrub (btrfs docs): per-device validation pass; detects checksum mismatch, superblock errors, header errors, disk read errors; repairs from redundant copies.
- Ceph scrubbing (IBM Ceph docs): daily light scrub (metadata comparison across PG replicas), weekly deep scrub (bit-for-bit object comparison).
- SeaweedFS EC Bitrot Scrub: verifies cold parity shards against per-block checksums; catches corruption before rebuild.
- Scrub is continuous/throttled background work; completes a full pass over a defined period (weeks/quarter); competes with client traffic — rate-limited.
- Scrub cycle = maximum undetected corruption window for cold data; shorter cycles cost more I/O.

#### 3.9 Silent Bit Rot Detection

Detection = recomputed checksum ≠ stored checksum on read or scrub. No error announced by drive/firmware; only checksum mismatch reveals it. Detection surfaces `Anomaly{anomaly_type: ChecksumFailure, description, node_id, ids_time}`. Detection without repair only tells you what you lost; repair requires redundancy (replica/erasure/parity).

#### 3.10 Checksum Mismatch Handling

Research basis:
- ZFS: transient mismatch → retry read; persistent mismatch with redundancy → fetch good copy, return to app, rewrite bad block, count error in pool status.
- Scality object storage: transfer checksum (upload), per-block checksum (read), background scrub (all data), application-level fixity check (end-to-end). On mismatch: treat fragment as missing, reconstruct from good copies/fragments, verify result, write to healthy media, log event (cluster of mismatches on one drive = early hardware-failure warning).
- Handling steps: detect → quarantine affected record/block → reconstruct from redundant copy or erasure fragments → verify reconstructed checksum → rewrite to healthy media → record anomaly → alert. If no redundancy: report only, never silently return bad data (fail closed).

#### 3.11 ZFS-Style End-to-End Protection

Data and checksum stored separately (checksum in parent block pointer). Entire structure self-validating to the root. Protects against bit rot, phantom writes, misdirected reads/writes, DMA parity errors, driver bugs, accidental overwrite. Limitation: page-cache/memory corruption not covered by on-disk checksums (UWisc FAST'10 observation); memory-side protection (ECC RAM) is orthogonal and out of scope for this crate beyond documenting the boundary.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Silent bit rot on media (unannounced bit flips) | End-to-end checksums per block/record; continuous scrubbing recomputes and compares |
| Checksum stored with data (self-consistent corruption) | Checksum stored separately (parent/pointer or sidecar metadata), not inline with data |
| Corruption in cold/unread data | Periodic scrub reads everything; scrub cycle bounds undetected window |
| No redundancy to repair from | Detect and report (fail closed); never return bad data; anomaly logged |
| Checksum mismatch on read | Retry once (transient); on persistent mismatch with redundancy, reconstruct from good copy, verify, rewrite |
| Replica drift masquerading as corruption | Version/OrderingPos comparison before checksum verdict; drift ≠ corruption |
| Corrupted checksum metadata itself | Checksum-of-checksums / Merkle root; sidecar redundancy; multiple historical roots |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; CI grep for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Scrub storms / I/O contention | Throttled continuous scrub; configurable rate; window-based full-pass scheduling |
| Undetected hash collision (accidental) | sha256/blake3: ~1/(2^256) accidental collision — negligible vs cosmic-ray bit-flip odds |
| Version skew / wire incompatibility | `ProtocolVersion` + `EdgeCaseBits` from ids-000-aa; integrity frames version-tagged; mismatched peers reject before verification |
| Memory-side corruption (page cache) | Documented boundary: on-disk checksums do not cover RAM; ECC RAM recommended (out of crate scope) |
| Erasure fragment corruption undetected until rebuild | Per-fragment checksums; EC scrub verifies cold parity shards |

---

### 5. Integration with Existing Crates

Phase 030 depends on and integrates with:

|| Crate | Role in Phase 030 ||
||-------|-------|-------------------|
| `ids-000-aa` (master) | Canonical types (`NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `OrderingPos`, `Version`, `SnapshotId`, `Checksum`, `Anomaly`, `AnomalyType`, `FragmentId`, `RecoveryProof`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`), sealed `trait Integrity` contract, invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits`), unified `Error` |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤1024); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-integrity` |
| `ids-crypto` (Phase 003) | sha256/blake3 hashing (content hashes/checksums), ed25519 (signed integrity proofs), xchacha20poly1305/chacha20poly1305 (optional encryption); consumed only |
| `ids-scrub` / storage crates | Persistence backend for checksum stores, scrub schedules, anomaly logs |
| `ids-replication` (Phase 017) | Replica sources for audit/repair; reconstructed data shipped via carrier |
| `ids-write` (Phase 012) | Change source; checksums computed at commit time, stored with the op |
| `ids-store` | Consumer: per-record/content checksums on stored objects |
| `ids-sync` | Consumer: Merkle root exchange for replica convergence verification |
| `ids-partition` / placement | Partition-level integrity scope |
| `ids-time` (crate 5) | `IdsTime` tick basis for scrub schedules and anomaly timestamps |

**Integration points:**
- `ids-integrity` implements `ids_000_aa::contracts::Integrity` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-integrity` never touches sockets.
- Checksums/content hashes via sha256/blake3 from `ids-crypto`; integrity proofs optionally signed with ed25519.
- Depends on `ids-000-aa` only for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (per CRATES.md dependency rule) — `ids-integrity` depends on both.
- Cargo workspace: `ids-integrity = { path = "../ids-integrity" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-integrity` (to be created at `C:/idfs/ids-integrity/`)

```
ids-integrity/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto; thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ChecksumRecord, ContentHash, MerkleRoot, IntegrityScope, IntegrityLevel, ScrubState, MismatchVerdict
│   ├── errors.rs                       # IntegrityError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Integrity (sealed trait)
│   │
│   ├── checksum/                       # End-to-end checksums (~7 files)
│   │   ├── mod.rs
│   │   ├── block_checksum.rs           # per-block/record checksum (sha256/blake3)
│   │   ├── sidecar.rs                  # checksum stored separately from data (parent pointer / sidecar)
│   │   ├── record_checksum.rs          # per-RecordId checksum
│   │   ├── partition_checksum.rs       # per-PartitionId aggregated checksum
│   │   ├── dataset_checksum.rs         # dataset-level checksum root
│   │   ├── verify.rs                   # recompute + compare on read/scrub
│   │   └── retry.rs                    # transient-mismatch retry policy
│   │
│   ├── content_hash/                   # Content hashes (~6 files)
│   │   ├── mod.rs
│   │   ├── sha256_hasher.rs            # SHA-256 content hash
│   │   ├── blake3_hasher.rs            # BLAKE3 content hash (incremental/tree)
│   │   ├── address.rs                  # content-address = hash of content
│   │   ├── dedup.rs                    # same-content → same hash → dedup
│   │   └── verify.rs                   # content hash verification
│   │
│   ├── merkle/                         # Merkle trees (~8 files)
│   │   ├── mod.rs
│   │   ├── tree.rs                     # binary hash tree construction
│   │   ├── leaf.rs                     # leaf = block/content hash
│   │   ├── node.rs                     # internal node = hash of children
│   │   ├── root.rs                     # Merkle root as trust anchor
│   │   ├── proof.rs                    # inclusion/consistency proof (path to root)
│   │   ├── diff.rs                     # locate divergent leaves in O(k·log n)
│   │   └── search_tree.rs              # Merkle Search Tree option (order-preserving)
│   │
│   ├── version/                        # Version comparisons (~5 files)
│   │   ├── mod.rs
│   │   ├── ordering_pos.rs             # OrderingPos-based version comparison
│   │   ├── mvcc_version.rs             # Version-based MVCC comparison
│   │   ├── drift_detector.rs           # divergent OrderingPos/Version for same RecordId
│   │   └── replica_version.rs          # per-replica version vector summary
│   │
│   ├── audit/                          # Replica audits (~7 files)
│   │   ├── mod.rs
│   │   ├── audit_scope.rs              # full replica / partition / record range
│   │   ├── root_exchange.rs            # exchange checksum/Merkle roots between replicas
│   │   ├── descent.rs                  # descend mismatching branches to locate bad records
│   │   ├── anomaly_record.rs           # raise Anomaly (ByteMismatch/ReplicaDrift/ChecksumFailure/StateCorruption)
│   │   ├── report.rs                   # audit report, version-pinned, reproducible
│   │   └── scheduler.rs                # audit schedule (on-demand / periodic)
│   │
│   ├── partition/                      # Partition-level integrity verification (~6 files)
│   │   ├── mod.rs
│   │   ├── partition_scope.rs          # verify single partition
│   │   ├── partition_root.rs           # per-partition checksum root
│   │   ├── partition_verify.rs         # verify all records in a partition
│   │   ├── aggregation.rs              # partition roots → dataset-level aggregation
│   │   └── scoped.rs                   # scoped verification API
│   │
│   ├── scrub/                          # Continuous integrity scrubbing (~8 files)
│   │   ├── mod.rs
│   │   ├── scrub_schedule.rs           # throttled continuous scrub; full-pass window (weeks/quarter)
│   │   ├── scrub_walker.rs             # walk all data, recompute checksums, compare
│   │   ├── rate_limiter.rs             # I/O throttle vs client traffic
│   │   ├── scrub_state.rs              # ScrubState (idle/running/paused/completed/failed)
│   │   ├── cold_data.rs                # prioritize rarely-read/cold data
│   │   ├── repair_coordinator.rs       # on mismatch: reconstruct, verify, rewrite
│   │   └── progress.rs                 # scrub progress reporting
│   │
│   ├── bitrot/                         # Silent bit rot detection (~5 files)
│   │   ├── mod.rs
│   │   ├── detector.rs                 # recomputed ≠ stored checksum = bit rot
│   │   ├── silent_corruption.rs        # no error announced; checksum-only detection
│   │   ├── anomaly_map.rs              # anomaly → node/dataset/partition mapping
│   │   └── threshold.rs                # corruption-rate thresholds and alerting
│   │
│   ├── mismatch/                       # Checksum mismatch handling (~7 files)
│   │   ├── mod.rs
│   │   ├── verdict.rs                  # MismatchVerdict (transient/persistent/repairable/unrepairable)
│   │   ├── quarantine.rs               # quarantine affected record/block
│   │   ├── reconstruct.rs              # reconstruct from redundant copy / erasure fragments
│   │   ├── verify_rewrite.rs           # verify reconstructed data, rewrite to healthy media
│   │   ├── fail_closed.rs              # never return bad data; report anomaly
│   │   ├── log_event.rs                # log mismatch event (cluster = hardware warning)
│   │   └── policy.rs                   # mismatch handling policy per consistency profile
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper for integrity metadata
│   ├── codec/                          # wire encoding of integrity messages (~4 files)
│   │   ├── mod.rs
│   │   ├── integrity_frame.rs          # version-tagged frame header
│   │   ├── checksum_codec.rs           # serde/bincode encode/decode
│   │   └── digest.rs                   # sha256/blake3 message digests
│   │
│   ├── validation/                     # input validation rules
│   ├── observability/                  # integrity event types (scrubbed, audited, mismatch, repaired)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── checksum_tests.rs               # end-to-end checksum verify on read/scrub
│   ├── content_hash_tests.rs           # sha256/blake3 content hash + dedup
│   ├── merkle_tests.rs                 # root match/mismatch; diff location
│   ├── version_comparison_tests.rs     # OrderingPos/Version drift detection
│   ├── audit_tests.rs                  # replica audit root exchange + descent
│   ├── partition_tests.rs              # partition-level verification
│   ├── scrub_tests.rs                  # throttled scrub walk; progress
│   ├── bitrot_tests.rs                 # silent bit rot detection
│   ├── mismatch_tests.rs               # mismatch handling: quarantine/reconstruct/verify/rewrite
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~150–220 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

#### 7.1 End-to-End Checksum (ZFS-style)
```
block.data → sha256/blake3(block.data) = block.checksum
block.checksum stored in parent block pointer / sidecar — NOT with block.data
on read(block): recompute sha256/blake3(block.data); compare to stored block.checksum
on mismatch: retry once (transient); persistent → reconstruct from redundancy, verify, rewrite
```
Fault isolation: data and checksum never stored together, so firmware bugs that corrupt both together are still detectable (they'd have to corrupt both consistently).

#### 7.2 Content Hash
```
object.address = sha256(object.content) or blake3(object.content)
same content → same hash → dedup + integrity
verification: recompute hash of retrieved content; compare to address
```

#### 7.3 Merkle Tree
```
leaf_i = hash(block_i)
internal_j = hash(left_child || right_child)
root = hash of root internal node
replica A root == replica B root ⇒ datasets match
roots differ → descend only differing subtrees → locate k divergent leaves in O(k·log n)
inclusion proof: path from leaf to root (sibling hashes at each level)
```

#### 7.4 Version Comparison (Drift Detection)
```
record R on replica A: (OrderingPos_A, Version_A)
record R on replica B: (OrderingPos_B, Version_B)
OrderingPos_A != OrderingPos_B ⇒ replica drift (legitimate or corruption)
checksum(R_A) == checksum(R_B) ⇒ drift is legitimate concurrent update
checksum(R_A) != checksum(R_B) ⇒ silent corruption / ByteMismatch anomaly
```

#### 7.5 Replica Audit Descent
```
exchange roots (checksum root or Merkle root) between replicas
roots match → audit passed, no descent
roots differ → descend into child ranges; compare per-range hashes
leaf mismatch → raise Anomaly{ChecksumFailure | ByteMismatch}, quarantine record
```

#### 7.6 Continuous Scrub
```
schedule: throttled, full-pass over window W (weeks/quarter)
walk: for each block/record, recompute checksum, compare to stored
mismatch → mismatch handling pipeline (quarantine → reconstruct → verify → rewrite)
progress: bytes scrubbed, mismatches found, repairs done, rate (bytes/sec)
```

#### 7.7 Mismatch Handling Pipeline
```
detect(mismatch) → quarantine(record) → reconstruct(from redundant copy or EC fragments)
→ verify(reconstructed.checksum == expected) → rewrite(healthy media)
→ log_anomaly(Anomaly{node_id, anomaly_type, description, ids_time})
→ if cluster of mismatches on one node/disk → hardware-failure warning
if no redundancy: fail closed, report only, never return bad data
```

#### 7.8 Data Structures
```
ChecksumRecord { record_id: RecordId, checksum: Checksum, algorithm: Algo, ids_time: IdsTime }
ContentHash { hash: [u8; 32], algorithm: Algo, content_len: u64 }
MerkleRoot { root: [u8; 32], leaf_count: u64, depth: u32, ids_time: IdsTime }
IntegrityScope { dataset_id: DatasetId, partition_id: Option<PartitionId>, record_range: Option<(RecordId, RecordId)> }
ScrubState { state, bytes_scrubbed, mismatches, repairs, rate_bps, started_at, ids_time }
MismatchVerdict { transient, persistent, repairable, unrepairable }
Anomaly { node_id, anomaly_type: AnomalyType, description, ids_time }  // from ids-000-aa
```

---

### 8. Dependencies

`ids-integrity/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Integrity`, invariants, versioning, errors
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — sha256, blake3, ed25519, xchacha20poly1305, chacha20poly1305 (consumed only)
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — integrity message serialization
- `blake3 = "1.5"` — content hashing (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling storage/replication crates referenced via adapters; persistence delegation is a design option, not a hard dependency in this phase.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | End-to-end checksums specified | `src/checksum/` module present; checksum stored separately from data (parent pointer/sidecar) |
|| 2 | Content hashes | `src/content_hash/` module; sha256 + blake3; same-content → same hash → dedup |
|| 3 | Merkle trees | `src/merkle/` module; root match ⇒ match; descent locates k divergent leaves in O(k·log n) |
|| 4 | Version comparisons | `src/version/` module; OrderingPos/Version drift detection vs checksum verdict |
|| 5 | Replica audits | `src/audit/` module; root exchange + descent; Anomaly raised on mismatch |
|| 6 | Partition-level verification | `src/partition/` module; per-partition checksum root + scoped verify |
|| 7 | Continuous scrubbing | `src/scrub/` module; throttled; full-pass window; progress reporting |
|| 8 | Bit rot detection | `src/bitrot/` module; recomputed ≠ stored = silent bit rot; anomaly logged |
|| 9 | Mismatch handling | `src/mismatch/` module; quarantine → reconstruct → verify → rewrite; fail closed |
|| 10 | Sealed `trait Integrity` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Integrity` |
|| 11 | Canonical types from ids-000-aa only | `types.rs` imports all canonical types from `ids_000_aa`; no redefinitions |
|| 12 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`; all traffic via `ids_carrier::Endpoint` |
|| 13 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM |
|| 14 | No code written / no source modifications | Plan document only; `ids-integrity/` crate not created; no edits to existing crates |
|| 15 | Conformance test plan enumerated | `tests/` list covers checksums, content hash, Merkle, version, audit, partition, scrub, bit rot, mismatch, carrier-agnostic, crypto allowlist |
|| 16 | File structure within range | ~150–220 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
||-------------|--------|-----------------|
| Checksum granularity (block vs record vs partition) | Affects `src/checksum/` design and overhead | Block-level for large objects, record-level for fine-grained; partition-level aggregation |
| Persistence backend for checksum stores | Which crate owns durable checksum metadata | Delegate to `ids-scrub`/storage crate; adapter pattern |
| Repair source selection (which replica/fragment is correct) | Determines reconstruction correctness | Version/OrderingPos comparison + quorum of checksums; majority vote among redundant copies |
| Scrub rate vs client traffic trade-off | Performance impact | Configurable rate limiter; window-based full pass; benchmarks in implementation |
| Memory-side corruption boundary | Checksums don't cover RAM/page cache | Document boundary; ECC RAM recommended out-of-crate |
| ed25519-signed integrity proofs: mandatory or optional | Authenticity vs accidental-corruption scope | Optional layer; core integrity is accidental-corruption detection |
| Merkle Search Tree vs plain Merkle tree | Order-preserving verification for sequences | Plain Merkle for blocks/records; MST option for ordered key spaces |
| Erasure-coded fragment integrity | Per-fragment checksums + EC scrub | Follow `ids-erasure` crate boundaries (deferred phase) |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|
| Separate `ids-integrity` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; single-responsibility boundary |
| Checksum stored separately from data (parent pointer / sidecar) | ZFS end-to-end principle: fault isolation between data and checksum; detects self-consistent corruption |
| sha256 + blake3 as allowed hashes | IDS crypto allowlist; cryptographic-strength accidental-corruption detection; blake3 for incremental/tree hashing |
| Merkle trees for scalable reconciliation | O(k·log n) divergence location vs O(n) full transfer; Dynamo/Cassandra lineage |
| Continuous throttled scrubbing | Finds latent bit rot in cold data; bounds undetected window; ZFS/Btrfs/Ceph precedent |
| Version/OrderingPos comparison before checksum verdict | Distinguishes legitimate replica drift from silent corruption |
| Fail-closed mismatch handling | Never return bad data; reconstruct from redundancy or report only |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic absolute rule |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| No code in this phase | Research only per prompts.txt entry 30 |

---

### 12. Sources

1. ZFS End-to-End Data Integrity — blogs.oracle.com/bonwick/zfs-end-to-end-data-integrity — checksums in parent block pointers, self-validating Merkle tree, bit rot/phantom write/misdirected read detection.
2. OpenZFS Checksums — openzfs.github.io/openzfs-docs — end-to-end checksums, self-healing, scrub and resilver.
3. OpenZFS Scrub and Resilver — openzfs.github.io/openzfs-docs — scrub walks all data, verifies checksums, repairs via redundancy; latent bit rot detection.
4. ZFS Corruption Analysis (FAST'10) — research.cs.wisc.edu/wind/Publications/zfs-corruption-fast10.pdf — checksum detection of all disk corruptions; memory-corruption limitation.
5. End-to-End Arguments in System Design — Saltzer, Reed, Clark (MIT LCS, ACM TOCS 1984) — end-to-end check and retry; function placement principle.
6. Keeping Bits Safe: How Hard Can It Be? — CACM — bit half-life; CERN and NetApp silent corruption studies; petabyte-scale durability requirements.
7. CERN Silent Corruptions (Kelemen 2007) — openlab-archive CERN/Xyratex — 500 errors on 100 nodes; 64K-region corruptions; single-bit errors correlated with ECC memory.
8. An Analysis of Data Corruption in the Storage Stack (SOSP) — Bairavasundaram et al. — 1.53M drives, 41 months, >400k checksum mismatches; nearline/enterprise drive corruption rates.
9. Content-Addressable Storage / Git internals — git-scm / progit — content hash as object address; integrity via hash-name match.
10. Git hash-function transition — git/git Documentation/technical/hash-function-transition.txt — SHA-1 → SHA-256; integrity checking via content hash.
11. BLAKE3 — blake3.pdf / RFC-style spec — tree hash, incremental, 256-bit default output.
12. SHA-256 — NIST FIPS 180-4 — approved secure hash standard.
13. CRC32 vs SHA-256 collision probability — security.stackexchange / crypto.stackexchange — 1/(2^256) accidental collision negligible.
14. Merkle Trees in Anti-Entropy — Dynamo/Cassandra/ScyllaDB — root comparison, subtree descent, O(k·log n) bandwidth.
15. Cassandra Anti-Entropy Repair — docs.datastax.com — Merkle trees per replica, depth-15 compact trees, divergent range streaming.
16. Merkle Search Trees — ieeexplore.ieee.org/document/9049566 — order-preserving hash tree for sequences/maps.
17. dm-verity — docs.kernel.org/admin-guide/device-mapper/verity.html — block-device Merkle tree integrity; root hash trust anchor; EIO on mismatch.
18. fs-verity — docs.kernel.org/filesystems/fsverity.html — file-level Merkle tree; constant-time measurement; EIO/SIGBUS on corrupted reads.
19. T10 DIF/DIX Data Integrity Field — mkp.net/pubs/dix-1.1.pdf — end-to-end protection information from host to disk; guard/app/ref tags.
20. Fixity Checking (Digital Preservation) — unimelb library guide — checksum generation, storage, routine comparison; bit rot detection.
21. Btrfs Scrub — btrfs.readthedocs.io — per-device validation pass; checksum mismatch, superblock, header, read-error detection; repair from redundant copies.
22. Ceph Data Integrity / Scrubbing — ibm.com/docs/en/storage-ceph — daily light scrub + weekly deep scrub; bit-for-bit object comparison.
23. SeaweedFS EC Bitrot Scrub — seaweedfs.com/docs/ec_bitrot_scrub — per-block checksum verification of cold parity shards; pre-rebuild corruption detection.
24. Scality Silent Data Corruption — scality.com/blog/silent-data-corruption — transfer/read/scrub/fixity check layers; mismatch handling (reconstruct, verify, rewrite, log).
25. ZFS vs Btrfs Data Protection — botmonster.com — per-block checksumming; self-healing via redundancy; copies property.
26. Content-Defined Chunking (Rabin fingerprint) — restic / Wikipedia — variable-size chunking; shift-resistant boundaries.
27. Reed-Solomon Erasure Coding Repair — arxiv.org/pdf/2007.15253 — repair bandwidth for distributed storage fragments.
28. IDS prompts.txt entry 30 (verbatim) — C:/idfs/prompts.txt lines 451-454 — crate definition, contents, constraints.
29. CRATES.md entry 30 — C:/idfs/CRATES.md — ids-integrity: end-to-end checksums and scrubbing.
30. ids-000-aa skeleton — C:/idfs/ids-000-aa/ — types.rs (`Checksum` §1973, `Anomaly`/`AnomalyType` §1934, `SnapshotId`, `FragmentId`, `RecoveryProof`), contracts, invariants, versioning, errors.

---

**End of plan-030.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**