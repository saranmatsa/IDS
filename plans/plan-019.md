## Phase 019 — Create the ids-consistency crate (consistency profiles and committed-prefix tracking): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 019 creates `ids-consistency`, the IDS crate for explicit consistency contracts per dataset. It implements per-dataset consistency profiles (Strong, Causal, BoundedStaleness, Eventual, ReadYourWrites), committed-prefix tracking (a monotonic `OrderingPos` watermark per dataset), MVCC version creation and visibility checks, operation-state tracking (`received → ordered → executing → committed → synchronized → verified → failed`), and durability-aware acknowledgement.

Crate: `ids-consistency` (CRATES.md entry 19). Purpose: Explicit consistency contracts per dataset. Contents per prompts.txt entry 19: strong consistency, causal consistency, read-your-writes, bounded staleness, eventual consistency, consistency profiles table, committed-prefix tracking, operation-state tracking (received/ordered/executing/committed/synchronized/verified/failed), durability-aware acknowledgement.

Verbatim prompt (C:/idfs/prompts.txt lines 280-294): "Phase 019 — Create the ids-consistency crate: Explicit consistency contracts per dataset."

A skeleton `ids-consistency` crate does NOT yet exist at `C:/idfs/ids-consistency/`. `ids-000-aa` already declares `trait Consistency` (profiles, committed-prefix, MVCC) at `src/contracts/consistency.rs`; `ids-consistency` implements it. `ids-000-aa` already defines `ConsistencyProfile` (`Strong`, `Causal`, `BoundedStaleness`, `Eventual`, `ReadYourWrites`) at `src/types.rs`, `CommittedPrefixTracker`, `MVCCManager`, `VersionStore`, `ReadVisibilityManager`, `WriteOrderingManager`, `ConsistencyChecker`, `PrefixValidator`, `ConsistencyStats`, `ConsistencyMonitor` at `src/contracts/consistency.rs`, and `OpState` (`received → ordered → executing → committed → synchronized → verified → failed`) plus `CommitState` (`accepted → prepared → committed → aborted`). Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-consistency` consumes type-level surface and primitive IDs only). `ids-000-aa` is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-consistency` sits above `ids-000-aa` (types + sealed `trait Consistency` contract + invariants + versioning + errors) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It is layered so each consistency dimension is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / READ-WRITE LAYER (ids-* crates)         │
│  dataset reads/writes · visibility checks · profile selection · ack     │
├─────────────────────────────────────────────────────────────────────────┤
│              Phase 019: ids-consistency (THIS CRATE)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐   │
│  │ consistency  │ │ committed    │ │ MVCC /       │ │ op-state     │   │
│  │ profiles     │ │ prefix       │ │ versions     │ │ tracking     │   │
│  │ (5 profiles) │ │ tracking     │ │ (visibility) │ │ lifecycle    │   │
│  └──────────────┘ └──────────────┴────────────────┴──────────────┘   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ durability-aware ack · read visibility · profile table · prefix │   │
│  │ validator · committed-prefix GC · version store · read manager   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (ids-carrier::Endpoint only)               │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, &[u8])                   │
│  receive_datagram → (Endpoint, Vec<u8>)                                 │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP       │
└─────────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `OrderingPos`, `OpRef`, `DatasetId`, `PartitionId`, `RecordId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`, `DurabilityCondition`) imported from `ids_000_aa::X`, never redefined.
- `ids-consistency` implements the sealed `trait Consistency` from `ids-000-aa/src/contracts/consistency.rs`; downstream crates use it as a trait bound, cannot implement it.
- All message transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16) + opaque `Vec<u8>`). No IP addresses, ports, sockets.
- Crypto surface: ed25519 signatures (authentication of profile changes / prefix updates), sha256/blake3 (version digests, prefix commitments), xchacha20poly1305/chacha20poly1305 (optional payload encryption for version values). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Per-dataset consistency profiles stored in `ids-000-aa::ConsistencyProfileMgr`; this crate orchestrates and enforces them.
- Operation lifecycle reuses `ids-000-aa` `OpState` (`received → ordered → executing → committed → synchronized → verified → failed`) and `CommitState` (`accepted → prepared → committed → aborted`).
- Committed prefix is a monotonic `OrderingPos` watermark per dataset; regressions are rejected (`PositionOutOfOrder`).

---

### 3. Detailed Research Dimensions

#### 3.1 Consistency Profiles (per dataset)

**Strong consistency:** Linearizable reads — every read returns the most recent committed write; real-time ordering preserved between operations. Reads require quorum intersection (r + w > n). No fallback allowed — if the quorum cannot be reached, the read fails rather than serving stale data. Source: Azure Cosmos DB strong consistency (linearizability), Princeton COS 418 consistency models, Cambridge Distributed Systems notes.

**Causal consistency:** Causally-related operations observed by every replica in the same order; concurrent operations may be observed in different orders. Built on happens-before (Lamport): thread-of-execution, reads-from, transitivity. Strongest always-available convergent model; compatible with high availability under partitions. Source: Jepsen causal consistency, Springer Encyclopedia (Shapiro/Tomsic), ACM CSUR survey on causally consistent cloud systems.

**Bounded staleness:** Reads are guaranteed to be within a configured time or version bound of the latest committed write. Reads may be served from local replicas without blocking on the leaseholder, trading staleness for latency and partition availability. Source: Azure Cosmos DB bounded staleness, CockroachDB bounded staleness reads, Google Cloud Spanner TrueTime bounded-staleness reads.

**Eventual consistency:** In the absence of new updates, all replicas eventually converge to the same value. No ordering guarantees beyond convergence; weakest model, highest availability. Source: Dynamo (Cassandra), Jepsen eventual consistency, Shapiro et al. 2011 CRDTs.

**Read-your-writes:** Within a client session, a read always reflects all writes previously performed by that same session. Implemented via session tokens carrying the last-written version; reads are served only by replicas at or beyond that version. Source: Azure Cosmos DB session consistency, Jepsen read-your-writes, Scixa session guarantees, Terry et al. session guarantees.

**Consistency profiles table:** A per-dataset mapping `DatasetId → ConsistencyProfile`, with defaults (Strong), profile change tracking, per-profile capabilities (`requires_quorum`, `allows_fallback`), and version-gated enforcement. Source: IDS `ids-000-aa` `ConsistencyProfile` enum + `ConsistencyProfileMgr`; CRATES.md entry 60 (`ids-consistency-profile`).

#### 3.2 Committed-Prefix Tracking

A monotonic `OrderingPos` watermark per dataset representing the highest position known to be durably committed across the quorum. Every write records its position; the prefix advances only monotonically (regressions rejected with `PositionOutOfOrder`). Reads at positions ≤ committed prefix are visible; positions beyond are withheld until the prefix advances. This gives each dataset a consistent-prefix view: clients observe a valid ordered sequence of writes starting from the first committed update, possibly stale but never out of order. Source: Rialo consistent-prefix guide, Azure Cosmos DB consistent prefix, Jepsen consistent-prefix model, IDS `CommittedPrefixTracker` (update_prefix/get_prefix/advance_prefix).

#### 3.3 Operation-State Tracking

Each operation progresses through `OpState`: `received` (ingress accepted) → `ordered` (assigned an `OrderingPos`) → `executing` (being applied) → `committed` (quorum-acked, durable) → `synchronized` (replicated to required replicas) → `verified` (read-visible / checksum confirmed) → `failed` (terminal error). Parallel commit lifecycle `CommitState`: `accepted → prepared → committed → aborted`. Transitions are guarded by the dataset's consistency profile and durability condition; invalid transitions are rejected. Source: IDS `ids-000-aa` `OpState`/`CommitState` definitions; CRATES.md entry 19.

#### 3.4 Durability-Aware Acknowledgement

An acknowledgement is only meaningful when tied to a durability condition: e.g., `replicated` (quorum commit + local application, no stable-storage barrier) vs `persisted` (quorum commit backed by stable-storage copies + local application). The ack level is chosen per write operation and per dataset profile; strong profiles require `persisted` + quorum, weaker profiles may accept `replicated` or single-replica acks. Source: Apache Iggy cluster durability (VSR quorum commit / persisted prepare history), DynamoDB write completion semantics, Cassandra tunable consistency (R + W > N).

#### 3.5 MVCC Versions and Read Visibility

Multi-version concurrency control: each write creates a new version at an `OrderingPos`; reads specify an `at_position` and return the latest version ≤ that position. Visibility is determined by the consistency profile: Strong/BoundedStaleness/Session/Prefix require `version_position ≤ committed_prefix`; Eventual is always visible. Version conflicts (out-of-order position) are rejected. Source: IDS `ids-000-aa` `MVCCManager`/`VersionStore`/`ReadVisibilityManager` (check_visibility rules at consistency.rs lines 610-642), Dotted Version Vectors causality tracking.

#### 3.6 Causality and Ordering Metadata

Causal consistency requires tracking happens-before dependencies between operations. Vector clocks (one entry per replica) detect exact causality; dotted version vectors compact the representation; hybrid logical clocks (HLC) combine logical causality with physical-time proximity for timestamping. Timestamps from `ids-hlc` (crate 67) optional; `ids-time` (crate 5) `Idstime` tick basis for commit/ordering timestamps. Never raw wall-clock for validity decisions. Source: ACM Queue "Why Logical Clocks are Easy", Kulkarni HLC paper, YugabyteDB HLC, TiKV time blog, IDS `ids-vector-clock`/`ids-hlc`.

#### 3.7 Consistency Hierarchy and Trade-offs

Linearizability (Strong) > Sequential > Causal+ > Causal > Consistent Prefix > Eventual. Stronger models sacrifice availability and latency under partitions (CAP); weaker models improve availability at the cost of stale reads and conflict resolution. Per-dataset profiles let each dataset choose its point on the spectrum. Source: Princeton COS 418 consistency hierarchy, Jepsen consistency models, ACM CSUR CRDT survey, DynamoDB read consistency options.

#### 3.8 Quorum and Replication for Consistency

Read quorum `R`, write quorum `W`, replication factor `N`; consistent reads guaranteed when `R + W > N` (quorum intersection). Majority quorums, flexible quorum intersection (Flexible Paxos: only cross-phase intersection required). Quorum capacity and fault tolerance bounded per dataset. Source: Cassandra tunable consistency (R + W > N), Flexible Paxos (arXiv 1608.06696), quorum intersection theory.

#### 3.9 Fencing Tokens and Write Serialization

Monotonic fencing tokens issued per write epoch prevent stale writers from overwriting newer state after a lease/partition recovery. Each write carries a fencing token; storage rejects writes with tokens below the last seen. Prevents split-brain and stale-write resurrection. Source: Kleppmann DDIA fencing tokens, Hazelcast FencedLock, etcd/ZooKeeper zxid as fencing token, IDS `ids-lease` (crate 24) fencing epochs.

#### 3.10 Chain Replication and Total-Order Commit

Chain replication arranges replicas linearly: writes enter at the head and propagate to the tail; reads served by the tail; linearizable with committed-prefix semantics. Complements (does not replace) consensus-based total order; useful for single-leader datasets. Source: Unseel chain replication, Renesse & Schneider chain replication.

#### 3.11 Raft / Paxos Committed Prefix

Consensus logs advance a committed index / committed prefix; committing entry N implies all entries before N are committed (log matching property). Provides the committed-prefix foundation for strong consistency on top of `ids-broadcast`/`ids-total-order-broadcast`. Source: Ongaro Raft (ATC14), openraft commit-index, university.kurz.fyi Raft commitment, Paxos chosen-value semantics.

#### 3.12 Session Guarantees Formalism

The four client-facing session guarantees — read-your-writes, monotonic reads, monotonic writes, writes-follow-reads — are implemented via per-session version vectors (last-written version + last-read version). Read-your-writes and monotonic reads are incomparable; both are cheaper than full causal consistency. Source: Terry et al. session guarantees, Jepsen read-your-writes/monotonic-reads, Scixa session guarantees, MerceMay session guarantees.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Stale reads under weak profiles | Document per-profile staleness bounds; Strong profile rejects fallback; session tokens enforce read-your-writes ||
|| Profile downgrade / silent weakening | Profile changes are versioned and audited; `EdgeCaseBits` gating; downgrade requires explicit admin action ||
|| Committed-prefix regression | `PositionOutOfOrder` error on non-monotonic prefix updates; prefix advances only forward ||
|| Read visibility violations | `is_read_visible` enforced at every read; version_position ≤ committed_prefix check for Strong/BoundedStaleness/Session/Prefix ||
|| Operation state corruption | Invalid `OpState` transitions rejected; terminal `failed` state requires explicit recovery ||
|| Unacknowledged writes read as committed | Durability-aware ack: read only from positions covered by the required ack level (replicated/persisted) ||
|| Vector clock / causality metadata growth | Pruning / HLC fallback; bounded retention per dataset ||
|| Fencing token reuse / stale writer | Monotonic fencing tokens; storage rejects token ≤ last_seen; tied to lease epochs (`ids-lease`) ||
|| Quorum not reachable (Strong profile) | Read fails rather than serving stale; configurable quorum sizes per dataset; failure-domain-aware quorum ||
|| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; CI greps source for prohibited symbols ||
|| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer ||
|| Version skew / wire incompatibility | `ProtocolVersion` + `EdgeCaseBits` from ids-000-aa; consistency messages version-tagged; mismatched peers reject before visibility check ||
|| MVCC version store unbounded growth | Bounded version store (default 100k versions); eviction policy; GC at causal stability ||
|| Durability-ack spoofing | Ack level verified against durable storage state; persisted ack requires stable-storage confirmation ||

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 019 ||
||-------|-------|-------------------|
|| `ids-000-aa` (master) | Canonical types (`NodeId`, `OrderingPos`, `OpRef`, `DatasetId`, `PartitionId`, `RecordId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`, `DurabilityCondition`), sealed `trait Consistency`, invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits`), unified `Error`, `ConsistencyProfileMgr`, `CommittedPrefixTracker`, `MVCCManager`, `VersionStore`, `ReadVisibilityManager` ||
|| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-consistency` ||
|| `ids-crypto` (Phase 003) | ed25519 signing/verification, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 AEAD; consumed only ||
|| `ids-write` (crate 13) | WAL and write lifecycle that consumes consistency-committed positions ||
|| `ids-broadcast` (crate 19? no — crate 20) | Orders writes via broadcast; `ids-consistency` consumes committed positions from broadcast-delivered operations ||
|| `ids-ordering-domain` (crate 55) | Dataset-scoped ordering domains; per-domain `OrderingPos` assignment (no global master) ||
|| `ids-vector-clock` (crate 66) | Causality metadata for causal consistency and version dependency tracking ||
|| `ids-hlc` (crate 67) | Optional hybrid logical timestamps for ordering metadata ||
|| `ids-time` (crate 5) | `Idstime` commit/ordering tick basis ||
|| `ids-lease` (crate 24) | Fencing epochs and lease-based authority for write serialization ||
|| `ids-quorum` (crate 21) | Read/write quorum intersection (R + W > N) for strong/bounded-staleness profiles ||
|| `ids-consistency-profile` (crate 60) | Consistency profile selection and per-dataset profile management ||
|| `ids-fault` (crate 27) | Circuit breakers / backpressure for consistency ingress under load ||

**Integration points:**
- `ids-consistency` implements `ids_000_aa::contracts::Consistency` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-consistency` never touches sockets.
- Authenticated profile/prefix updates signed with ed25519 via `ids-crypto`; version integrity via sha256/blake3 digests.
- Depends on `ids-000-aa` only for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (per CRATES.md dependency rule) — `ids-consistency` depends on both.
- Cargo workspace: `ids-consistency = { path = "../ids-consistency" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-consistency` (to be created at `C:/idfs/ids-consistency/`)

```
ids-consistency/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-ordering-domain,
│                                       # ids-write, ids-vector-clock, ids-hlc, ids-time, ids-lease,
│                                       # ids-quorum, ids-consistency-profile, ids-fault;
│                                       # thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ConsistencyConfig, ProfileAssignment, AckLevel,
│   │                                   # DurabilityResult, VisibilityResult, PrefixUpdate
│   ├── errors.rs                       # ConsistencyError (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Consistency (sealed trait)
│   │
│   ├── profiles/                       # Consistency profile modules (~8 files)
│   │   ├── mod.rs
│   │   ├── profile_table.rs            # DatasetId -> ConsistencyProfile mapping
│   │   ├── strong.rs                   # Strong consistency enforcement (linearizable reads)
│   │   ├── causal.rs                   # Causal consistency enforcement (happens-before)
│   │   ├── bounded_staleness.rs        # Bounded staleness enforcement (time/version bound)
│   │   ├── eventual.rs                 # Eventual consistency enforcement (convergence only)
│   │   ├── read_your_writes.rs         # Read-your-writes enforcement (session tokens)
│   │   ├── profile_capabilities.rs     # requires_quorum, allows_fallback, as_str
│   │   └── profile_validator.rs        # PrefixValidator — validates profile transitions
│   │
│   ├── committed_prefix/               # Committed-prefix tracking (~6 files)
│   │   ├── mod.rs
│   │   ├── prefix_tracker.rs           # Monotonic OrderingPos watermark per dataset
│   │   ├── prefix_advance.rs           # Advance prefix on quorum ack; reject regressions
│   │   ├── prefix_validator.rs         # PositionOutOfOrder guard
│   │   ├── prefix_commitment.rs        # sha256/blake3 commitment over prefix state
│   │   └── prefix_gc.rs                # Committed-prefix GC at causal stability
│   │
│   ├── mvcc/                           # Multi-version concurrency control (~7 files)
│   │   ├── mod.rs
│   │   ├── version_store.rs            # Bounded version store (default 100k versions)
│   │   ├── version_create.rs           # Create version at OrderingPos; reject conflicts
│   │   ├── version_read.rs             # Read version at or before a position
│   │   ├── version_ref.rs              # VersionId / VersionRef types
│   │   ├── visibility.rs               # ReadVisibilityManager — profile-gated visibility
│   │   └── version_gc.rs               # Version garbage collection
│   │
│   ├── op_state/                       # Operation-state tracking (~6 files)
│   │   ├── mod.rs
│   │   ├── op_state_machine.rs         # received → ordered → executing → committed →
│   │   │                                 synchronized → verified → failed
│   │   ├── commit_state.rs             # accepted → prepared → committed → aborted
│   │   ├── state_transition.rs         # Guarded transitions by profile + durability
│   │   ├── op_tracker.rs               # Per-operation state tracking
│   │   └── op_lifecycle.rs             # Operation lifecycle hooks
│   │
│   ├── durability/                     # Durability-aware acknowledgement (~5 files)
│   │   ├── mod.rs
│   │   ├── ack_level.rs                # replicated vs persisted ack levels
│   │   ├── durability_condition.rs     # Per-dataset durability condition
│   │   ├── ack_tracker.rs              # Track ack levels per write
│   │   └── ack_verifier.rs             # Verify ack against durable storage state
│   │
│   ├── session/                        # Session guarantees (~5 files)
│   │   ├── mod.rs
│   │   ├── session_token.rs            # Session token (last-written + last-read version)
│   │   ├── read_your_writes.rs         # Read-your-writes enforcement per session
│   │   ├── monotonic_reads.rs          # Monotonic reads enforcement per session
│   │   └── session_manager.rs          # Per-session state (read-set / write-set)
│   │
│   ├── quorum/                         # Quorum integration (~4 files)
│   │   ├── mod.rs
│   │   ├── quorum_calc.rs              # R + W > N intersection check
│   │   ├── quorum_tracker.rs           # Per-dataset read/write quorum tracking
│   │   └── failure_domain_quorum.rs    # Failure-domain-aware quorum selection
│   │
│   ├── fencing/                        # Fencing tokens (~4 files)
│   │   ├── mod.rs
│   │   ├── fencing_token.rs            # Monotonic fencing token per write epoch
│   │   ├── fencing_checker.rs          # Reject writes with token <= last_seen
│   │   └── lease_epoch.rs              # Tie fencing to ids-lease epochs
│   │
│   ├── causality/                      # Causality metadata (~5 files)
│   │   ├── mod.rs
│   │   ├── vector_clock_adapter.rs     # Wrap ids-vector-clock (crate 66)
│   │   ├── dotted_version_vector.rs    # Compact causality representation
│   │   ├── hlc_adapter.rs              # Optional HLC timestamps (crate 67)
│   │   └── happens_before.rs           # Causality checks between operations
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper
│   ├── state/                          # State machines (~3 files)
│   │   ├── mod.rs
│   │   ├── consistency_state_machine.rs
│   │   └── visibility_state_machine.rs
│   ├── codec/                          # Wire encoding (~4 files)
│   │   ├── mod.rs
│   │   ├── consistency_frame.rs        # Version-tagged frame header
│   │   ├── message_codec.rs            # serde/bincode encode/decode
│   │   └── digest.rs                   # sha256/blake3 message/version digests
│   ├── validation/                     # Input validation rules
│   ├── observability/                  # Consistency events (profile change, prefix advance,
│   │                                   # visibility check, op state transition)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── strong_consistency_tests.rs
│   ├── causal_consistency_tests.rs
│   ├── bounded_staleness_tests.rs
│   ├── eventual_consistency_tests.rs
│   ├── read_your_writes_tests.rs
│   ├── committed_prefix_tests.rs       # monotonic prefix, regression rejection
│   ├── mvcc_tests.rs                   # version create/read/visibility
│   ├── op_state_tests.rs               # OpState lifecycle transitions
│   ├── durability_ack_tests.rs         # ack level verification
│   ├── session_tests.rs                # session token / read-your-writes / monotonic reads
│   ├── fencing_tests.rs                # fencing token monotonicity
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~110–160 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

1. **Per-dataset consistency profile table** — `HashMap<DatasetId, ConsistencyProfile>`; default `Strong`; `set_profile`/`get_profile`; change counting; profile capabilities (`requires_quorum`, `allows_fallback`).

2. **Committed-prefix tracking** — monotonic `OrderingPos` watermark per dataset; `update_prefix` rejects positions below current (`PositionOutOfOrder`); `advance_prefix` on quorum ack; prefix commitment via sha256/blake3 digest over `(dataset_id, position)`.

3. **Strong consistency visibility** — read visible iff `version_position.as_u64() <= committed_prefix.as_u64()`; requires quorum (`requires_quorum == true`); no fallback.

4. **Causal consistency visibility** — causally-related operations ordered via happens-before (vector clock / dotted version vector); concurrent operations unordered; read must observe all dependencies before returning.

5. **Bounded staleness visibility** — read served if within configured time/version bound of committed prefix; may be served from local replica; dynamic timestamp per read (CockroachDB `with_max_staleness` pattern).

6. **Eventual consistency visibility** — always visible (`true`); convergence only, no ordering guarantee; conflicts resolved later.

7. **Read-your-writes visibility** — session token carries last-written version per item; read served only by replica at or beyond that version; session affinity optional routing heuristic.

8. **Operation-state machine** — `received → ordered → executing → committed → synchronized → verified → failed`; transitions guarded by profile + durability; `CommitState`: `accepted → prepared → committed → aborted`.

9. **Durability-aware acknowledgement** — ack level `replicated` (quorum commit + local application) vs `persisted` (quorum commit + stable-storage copies + local application); read only from positions meeting the required ack level.

10. **MVCC version creation** — new version at `OrderingPos` per `(dataset, partition, record)`; reject positions ≤ existing last position (`VersionConflict`); bounded store with eviction.

11. **Read visibility check** — profile-gated: Strong/BoundedStaleness/Session/Prefix → `version_position ≤ committed_prefix`; Eventual → always visible.

12. **Fencing token** — monotonic token per write epoch; storage rejects writes with token ≤ last_seen; tied to `ids-lease` epochs.

13. **Version digest** — sha256/blake3 digest over `(dataset_id, partition_id, record_id, position, value)` for version integrity and committed-prefix commitments.

---

### 8. Dependencies

`ids-consistency/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Consistency`, invariants, versioning, errors, `ConsistencyProfileMgr`/`CommittedPrefixTracker`/`MVCCManager` etc.
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 (consumed only)
- `ids-ordering-domain = { path = "../ids-ordering-domain" }` — dataset-scoped ordering domains
- `ids-write = { path = "../ids-write" }` — WAL / write lifecycle consumer
- `ids-vector-clock = { path = "../ids-vector-clock" }` — causality metadata
- `ids-hlc = { path = "../ids-hlc" }` — optional hybrid logical timestamps
- `ids-time = { path = "../ids-time" }` — `Idstime` commit tick basis
- `ids-lease = { path = "../ids-lease" }` — fencing epochs
- `ids-quorum = { path = "../ids-quorum" }` — read/write quorum intersection
- `ids-consistency-profile = { path = "../ids-consistency-profile" }` — profile selection
- `ids-fault = { path = "../ids-fault" }` — circuit breakers / backpressure
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — message serialization
- `blake3 = "1.5"` — content hashing (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | Five consistency profiles specified | `src/profiles/{strong,causal,bounded_staleness,eventual,read_your_writes}/` modules present; each documents its guarantee ||
|| 2 | Consistency profiles table specified | `src/profiles/profile_table.rs` — `DatasetId → ConsistencyProfile` mapping; default `Strong`; change counting ||
|| 3 | Committed-prefix tracking specified | `src/committed_prefix/prefix_tracker.rs` — monotonic `OrderingPos` watermark; `PositionOutOfOrder` on regression ||
|| 4 | Operation-state tracking specified | `src/op_state/op_state_machine.rs` — `received → ordered → executing → committed → synchronized → verified → failed`; `CommitState: accepted → prepared → committed → aborted` ||
|| 5 | Durability-aware acknowledgement specified | `src/durability/ack_level.rs` — `replicated` vs `persisted` ack levels; read only from ack-covered positions ||
|| 6 | MVCC version + visibility specified | `src/mvcc/version_store.rs` + `visibility.rs` — version create at `OrderingPos`; profile-gated `is_read_visible` ||
|| 7 | Sealed `trait Consistency` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Consistency` ||
|| 8 | Canonical types from ids-000-aa only | `types.rs` imports `NodeId`, `OrderingPos`, `OpRef`, `DatasetId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile` from `ids_000_aa`; no redefinitions ||
|| 9 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` in source; all traffic via `ids_carrier::Endpoint` ||
|| 10 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM/etc. ||
|| 11 | No code written / no source modifications | Plan document only; `ids-consistency/` crate not created; no edits to existing crates ||
|| 12 | Conformance test plan enumerated | `tests/` list covers all 5 profiles, committed prefix, MVCC, op-state, durability ack, session, fencing, carrier-agnostic assertion, crypto allowlist assertion ||
|| 13 | File structure within range | ~110–160 files proposed (within ~90–1000 target) ||

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
||-------------|--------|-----------------|
|| Consistency profile granularity | Per-dataset vs per-partition vs per-record profiles | Default per-dataset (per `ConsistencyProfile`); finer granularity deferred ||
|| Committed-prefix advancement trigger | When does prefix advance (quorum ack vs full replication)? | Durability-condition-driven; tuned per profile ||
|| Bounded staleness bound source | Config vs dynamic per-read timestamp | Both supported; dynamic default (CockroachDB pattern) ||
|| Session token transport | How are session tokens passed between client and replicas? | Opaque `Vec<u8>` via `Endpoint`; token format deferred ||
|| MVGC version store eviction policy | Size/age/conflict-based eviction | Bounded store (100k default); LRU + conflict eviction ||
|| Fencing token vs OpState interaction | How fencing tokens integrate with operation lifecycle | Fencing checked at `received → ordered` transition ||
|| Profile downgrade safety | Downgrading Strong → Eventual may expose stale reads | Versioned profile changes + audit; downgrade requires explicit action ||
|| Integration timing with ids-broadcast | Committed positions from broadcast must exist before consistency tracks them | Phases ordered: ordering domain + broadcast before consistency enforcement ||
|| Whether consistency owns quorum or delegates | Crate boundaries | Delegate to ids-quorum (21); this crate consumes quorum results ||
|| GC at causal stability with offline members | Causal-stability GC freezes if a member never acks | Quarantine/grace window; deliberate eviction (cf. plan-077 tombstone GC issue) ||

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|
|| Separate `ids-consistency` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; enforces single-responsibility boundary ||
|| Five consistency profiles (Strong, Causal, BoundedStaleness, Eventual, ReadYourWrites) | Matches prompts.txt entry 19 contents exactly; covers full consistency spectrum ||
|| Committed-prefix tracking per dataset | Matches prompts.txt entry 19; consistent-prefix reads (stale but ordered) ||
|| Operation-state tracking reusing ids-000-aa OpState | Single canonical definition; `received → ordered → executing → committed → synchronized → verified → failed` ||
|| Durability-aware acknowledgement (replicated vs persisted) | Ack semantics tied to durability condition; strong profiles require `persisted` ||
|| All traffic via `ids-carrier::Endpoint` only | Carrier-agnostic absolute rule (CRATES.md, prompts.txt entry 19) ||
|| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries ||
|| Sealed `trait Consistency` from ids-000-aa | Downstream crates use as trait bound; cannot implement; single canonical contract ||
|| Per-dataset profiles (not per-record) | Matches `ConsistencyProfile` enum scope in `ids-000-aa/types.rs`; simpler enforcement ||
|| Quorum delegated to ids-quorum (21) | Quorum intersection is a separate concern (CRATES.md entry 21) ||
|| No code in this phase | Research only per prompts.txt entry 19 ||

---

### 12. Sources

1. Linearizability / strong consistency — Cambridge Distributed Systems notes (cl.cam.ac.uk/teaching/2526/ConcDisSys) — every operation takes effect atomically between invocation and response; "up-to-date" reads.
2. Princeton COS 418 Consistency Models (cs.princeton.edu) — linearizability vs sequential vs causal vs eventual hierarchy; RAFT provides linearizability.
3. Jepsen Causal Consistency (jepsen.io/consistency/models/causal) — causally-related operations in same order; concurrent operations unordered; sticky availability.
4. Jepsen Read Your Writes (jepsen.io/consistency/models/read-your-writes) — a process's subsequent read observes its own write; sticky available.
5. Jepsen Monotonic Reads (jepsen.io/consistency/models/monotonic-reads) — reads cannot go backwards within a process; totally available.
6. Azure Cosmos DB consistency levels (learn.microsoft.com) — Strong, Bounded Staleness, Session, Consistent Prefix, Eventual spectrum; session tokens; bounded staleness definition.
7. Scixa Session Guarantees (scixa.com) — read-your-writes, monotonic reads, monotonic writes, writes-follow-reads as client-facing contracts; version terms.
8. Terry et al. Session Guarantees (classic session guarantee work) — four guarantees; per-session version vectors (write-set / read-set).
9. Rialo Consistent Prefix (rialo.io) — consistent prefix: valid ordered sequence from first committed update; replicas at different points in same history.
10. DynamoDB Read Consistency (docs.aws.amazon.com) — eventually consistent vs strongly consistent reads; read-committed isolation.
11. Google Cloud Spanner / TrueTime (cloud.google.com/blog) — strong reads via TrueTime; bounded-staleness reads; external consistency.
12. CockroachDB Bounded Staleness Reads (docs.cockroachlabs.com) — bounded staleness read uses dynamic system-determined timestamp; served from local replicas.
13. Cassandra Tunable Consistency (cassandra.apache.org) — R + W > N quorum intersection; consistency levels ONE/TWO/THREE/QUORUM/ALL.
14. Shapiro et al. 2011 CRDTs (INRIA RR-7687) — strong eventual consistency; state-based vs op-based; convergence without coordination.
15. ACM CSUR CRDT Approaches (dl.acm.org/10.1145/3695249) — state-based vs op-based CRDTs; SEC as strongest AP guarantee.
16. Dotted Version Vectors (github.com/ricardobcl/Dotted-Version-Vectors) — compact causality tracking; dots separate from causal past.
17. ACM Queue "Why Logical Clocks are Easy" (queue.acm.org) — vector clocks; dotted vector clocks; version vectors.
18. Kulkarni Hybrid Logical Clock (uchicago.edu) — HLC combines logical causality with physical-time proximity.
19. YugabyteDB HLC (docs.yugabyte.com) — hybrid time = physical + logical; strict monotonicity; causality ordering.
20. Chain Replication (unseel.com; Renesse & Schneider) — head-to-tail writes, tail reads; linearizable; committed-prefix semantics.
21. Raft Log Replication & Commitment (ongaro.usenix.org/atc14; university.kurz.fyi; openraft) — commit index; committing N implies all before N committed; log matching property.
22. Paxos (mpaxos.com) — chosen value; majority acceptance; single-decree and multi-instance consensus.
23. Apache Iggy Cluster Durability (iggy.apache.org) — `replicated` vs `persisted` ack policies; quorum commit + stable storage.
24. ZooKeeper Zab / zxid (zookeeper.apache.org) — epoch + counter; monotonic transaction ID as fencing/commit ordering.
25. Kleppmann DDIA Fencing Tokens (ebrary.net; monolock) — monotonic fencing token per lock grant; reject stale writes.
26. Hazelcast FencedLock (docs.hazelcast.com) — fencing token increments per grant; external service rejects lower tokens.
27. IDS prompts.txt entry 19 (verbatim) — C:/idfs/prompts.txt lines 280-294 — crate definition, contents, constraints.
28. CRATES.md entry 19 — C:/idfs/CRATES.md — ids-consistency: consistency profiles and committed-prefix tracking.
29. CRATES.md entry 60 — C:/idfs/CRATES.md — ids-consistency-profile: consistency profile selection.
30. ids-000-aa skeleton — C:/idfs/ids-000-aa/ — types.rs (`ConsistencyProfile` enum line 717), contracts/consistency.rs (`trait Consistency`, `ConsistencyProfileMgr`, `CommittedPrefixTracker`, `MVCCManager`, `ReadVisibilityManager` visibility rules), OpState/CommitState lifecycle; HX-241 plan format — C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md; IDS plan format — C:/idfs/plans/plan-001.md, plan-020.md.

---

**End of plan-019.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**
