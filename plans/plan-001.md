## Phase 001 — Create the ids-000-aa crate (master crate / single control point): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 001 creates `ids-000-aa`, the master crate and single control point for the entire IDS workspace. It hosts every canonical type (`NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `PlacementDecision`, `RedundancyMode`, `ConsistencyProfile`, `FailureDomain`, plus dozens of additional types referenced by contracts and invariants), all sealed trait contracts (starting with `trait Carrier` — send/receive opaque datagrams via `CarrierType + Vec<u8>`, no IP/TCP/UDP), protocol invariants, version bits, and the unified error contract. Every downstream `ids-*` crate imports these as `ids_000_aa::X` and depends on `ids-000-aa` only. Adding one edge case = edit exactly one file here, then implement in one dependent crate at a time. Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (crypto itself lives in `ids-crypto`; this crate declares only the type-level crypto surface — `Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum`).

Research basis: 30 parallel searches returned the Shapiro et al. 2011 CRDT paper (INRIA RR-7687), the delta-CRDT paper (Almeida/Shoker/Baquero, arXiv:1603.01529), the Rust sealed-trait pattern (rust-lang API guidelines C-SEALED), thiserror library design guidance, NIST FIPS 180-4 (SHA-256), RFC 8032 (Ed25519), RFC 7539 (ChaCha20-Poly1305), libsodium XChaCha20-Poly1305 construction, quorum intersection theory (r + w > n), CRDT convergence proofs (CvRDT vs CmRDT, causal stability for tombstone GC), idempotency-key exactly-once semantics, vector clock vs hybrid logical clock ordering, durability acknowledgement levels, replication-vs-erasure-coding tradeoffs, consistency profiles (strong/causal/bounded staleness/eventual/read-your-writes), failure-domain correlated-failure modeling, hybrid redundancy (replica + erasure), workspace/monorepo patterns for Cargo, and serde newtype identifier patterns.

A skeleton `ids-000-aa` crate already exists at `C:/idfs/ids-000-aa/` with `src/types.rs` (all canonical types), `src/lib.rs` (module declarations + root re-exports), `src/contracts/mod.rs` + 30 contract modules (carrier, transport, store, session, routing, discovery, membership, placement, replication, recovery, consistency, ordering, security, client, fault, broadcast, quorum, snapshot, byzantine, lease, crdt, erasure, saga, coordinator, durability, topology, integration, protocol, wire, framing, sequencing, acks), `src/invariants/mod.rs` + ~40 invariant modules, `src/errors.rs`, `src/versioning.rs`, and `Cargo.toml` (thiserror, serde, blake3). Phase 001 research validates this skeleton and specifies the plan to complete/align it; no source modifications are made in this phase.

---

### 2. Architecture Overview

`ids-000-aa` is layer 0 of the IDS dependency stack. Every other `ids-*` crate depends on it and nothing else cross-crate at the type level. It is a pure data + contract crate: no `std::net`, `tokio::net`, or carrier implementation; all I/O is abstracted behind `trait Carrier` and `Endpoint` (`CarrierType` + `Vec<u8>`).

```
┌─────────────────────────────────────────────────────────────┐
│                   DOWNSTREAM ids-* CRATES                   │
│  ids-carrier · ids-crypto · ids-identity · ids-time · …     │
│  (each depends on ids-000-aa only)                          │
├─────────────────────────────────────────────────────────────┤
│              Phase 001: ids-000-aa (THIS CRATE)             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  types   │ │ contracts│ │ invariants│ │ versioning│       │
│  │ (all IDs │ │ 20+ sealed│ │ protocol  │ │ Protocol │       │
│  │  newtypes│ │ traits   │ │ invariants│ │ Version, │       │
│  │  + Enums)│ │ Carrier  │ │ ~40 rules │ │ EdgeCaseBits│      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │   errors │ │   codec  │ │   state  │ │   prelude │       │
│  │ unified  │ │ frame/   │ │ state    │ │ re-exports │       │
│  │ Error    │ │ datagram │ │ machines │ │           │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  internals│ │ recovery │ │ placement│ │ routing   │       │
│  │ type reg │ │ etc.     │ │ etc.     │ │ etc.      │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
├─────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (no std::net / tokio::net)      │
│  ids-carrier::Endpoint = CarrierType + Vec<u8>               │
│  trait Carrier: send_datagram(Endpoint, &[u8]) / receive     │
└─────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type. `NodeId`, `RegionId`, etc. are defined once in `src/types.rs` and re-exported at crate root; downstream crates never redefine them.
- Sealed traits via private supertrait (`pub trait Carrier: private::Sealed`). Downstream crates can call methods and use as bounds but cannot implement. Adding a method = edit one contract file, then implement in one dependent crate at a time.
- Newtype wrapper for every identifier (`pub struct NodeId(pub [u8; 32])`) with `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`, `ZERO` constant, `new`, `as_bytes`, `is_zero`, `to_hex`, `from_hex`, `Display`.
- `Endpoint` is the sole address type: `CarrierType` enum (Loopback, Serial, Optical, Radio, Custom(u16)) + opaque `Vec<u8>` capped at 1024 bytes. No IP, port, SocketAddr.
- Operation lifecycle: `received → ordered → executing → committed → synchronized → verified → failed` (`OpState`); commit lifecycle: `accepted → prepared → committed → aborted` (`CommitState`).
- Versioning: `ProtocolVersion` (semver) + `EdgeCaseBits` (u128 bitfield, one bit per protocol invariant) for compile-time wire-contract gating.

---

### 3. Detailed Research Dimensions

#### 3.1 Canonical Types — Identifier Newtypes
Each ID is a fixed-size byte array newtype (32-byte for node/region/hunit/dataset/partition/record/ordering-domain/checksum; 16-byte for rotating-node-id/session/snapshot/lease/message/transaction/fragment/key-id/auth-token; 9-byte `OpRef`; `u64` `OrderingPos`/`Version`/`Epoch`/`ChannelId`/`FencingToken`/`StepId`). All implement `Serialize`/`Deserialize` (serde), `Hash`, `Eq`, `Copy`, with `ZERO` const, `new`, `as_bytes`, `is_zero`, `to_hex`, `from_hex`, `Display`. Source: serde newtype patterns; Rust standard library conventions; existing `ids-000-aa/src/types.rs`.

#### 3.2 Sealed Trait Contracts
`trait Carrier: private::Sealed` with `send_datagram(&self, endpoint: &Endpoint, data: &[u8]) -> Result<(), CarrierError>` and `receive_datagram`. Additional sealed traits: `Transport` (frame/deframe/sequence/acknowledge/delivery semantics), `Store`, `Session`, `Routing`, `Discovery`, `Membership`, `Placement`, `Replication`, `Recovery`, `Consistency`, `Ordering`, `Security`, `Client`, `FaultTolerance`, `Broadcast`, `Quorum`, `Snapshot`, `Byzantine`, `Lease`, `CRDT`, `Erasure`, `Saga`, `Coordinator`, `Durability`, `Topology`, `Integration`, `Protocol`, `Wire`, `Framing`, `Sequencing`, `Acks`. Sealing follows the Rust API Guidelines sealed-trait pattern (private supertrait in `mod private`), confirmed by research. Downstream crates use as trait bounds but cannot implement.

#### 3.3 Protocol Invariants (~40 rules)
`BinaryCompletion`, `DeadNodeIdentity`, `IdleRotation`, `NoGlobalMaster`, `OutEqIn`, `CoordinatedDeletion`, `FencingStonith`, `QuorumIntersection`, `ErasureThreshold`, `CorrelatedFailure`, `HUnitCompensation`, `RegionalAutonomy`, `HybridRedundancy`, `OperationLifecycle`, `DurabilityAck`, `OrderedReadVisibility`, `NodeRoster`, `RegionAffinity`, `IdentityRotation`, `IdsStandardTime`, `DailyIntegrity`, `DbRetirement`, `ReadFallback`, `TopologyDiversity`, `WriteFanout`, `CustodyGraph`, `ChecksumBoundary`, `ReplicaFreshness`, `CommitPrefix`, `VersionVector`, `MembershipEpoch`, `PlacementPolicy`, `ReplicationLag`, `RecoveryScope`, `DataLifecycle`, `SnapshotConsistency`, `ComputeToData`, `HotColdTier`, `MobilityHandoff`, `FederationTrust`. Each is a compile-time/runtime constraint checked by `InvariantChecker` in `src/internals/`.

#### 3.4 Versioning
`ProtocolVersion` (MAJOR.MINOR.PATCH semver) + `EdgeCaseBits` (u128 bitfield, 40 bits = one per invariant) + `VersionRange`, `SemanticVersion`, `ProtocolFeature`, `ProtocolRegistry`, `CompatibilityMatrix`, `VersionGraph` (DAG of versions with upgrade/downgrade edges), `MigrationPath`, `VersionHistory`, `VersionNegotiation`, `EdgeCaseCompiler`. Wire-contract gating: increment `PROTOCOL_VERSION` when any field changes in bincode/serde-encoded messages; incompatible binaries must reject each other before registering peers or exchanging operations. Sources: HBase compatibility matrix, Numax wire versioning, Agent Host Protocol negotiation, qcp compatibility levels.

#### 3.5 Error Contract
Single `Error` enum (thiserror) with structured context variants per domain (`InvalidNodeIdLength`, `EndpointAddressTooLarge`, `InvalidOpRefLength`, `CarrierUnavailable`, etc.), each variant carrying recovery hints, `is_retryable()`, `is_fatal()`, `category()`, `plain_message()`. Downstream crates wrap `ids_000_aa::Error` variants; cannot extend the enum (sealed by thiserror enum design). Source: thiserror docs, Rust error-type design guidance.

#### 3.6 Carrier-Agnostic Addressing
`Endpoint` = `CarrierType` + `Vec<u8>` (max 1024 bytes). `CarrierType` enum: `Loopback`, `Serial`, `Optical`, `Radio`, `Custom(u16)`. This is the ONLY address type in the workspace. No `std::net`, `tokio::net`, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere. All traffic via `ids-carrier::Endpoint`. Source: HX-241 carrier-agnostic socket abstraction research; IDS prompts.txt entry 1.

#### 3.7 Crypto Surface (type-level only)
Allowed crypto: ed25519 (signatures), xchacha20poly1305 (AEAD, random 192-bit nonces), sha256 (hashing), blake3 (content addressing, fast hashing), chacha20poly1305 (AEAD, 96-bit nonces). `ids-000-aa` declares type-level surface only: `Signature([u8; 64])`, `KeyId([u8; 16])`, `AuthToken([u8; 32])`, `Credential { identity: NodeId, token: Vec<u8> }`, `Checksum([u8; 32])`. Actual crypto operations live in `ids-crypto` (Phase 003). Sources: RFC 8032 (Ed25519), RFC 7539 (ChaCha20-Poly1305), libsodium XChaCha20-Poly1305, NIST FIPS 180-4 (SHA-256), BLAKE3 spec.

#### 3.8 State Machines
`OpStateMachine`, `CommitStateMachine`, `NodeStateMachine` (Candidate→Validating→Active→Degraded→Draining→Failed→Retired), `HUnitStateMachine`, `RegionStateMachine`, `SessionStateMachine`, `MembershipStateMachine`, `PlacementStateMachine`, `ReplicationStateMachine`, `RecoveryStateMachine`, `ConsistencyStateMachine`, `OrderingStateMachine`, `BroadcastStateMachine`, `QuorumStateMachine`, `SnapshotStateMachine`, `ByzantineStateMachine`, `LeaseStateMachine`, `CRDTStateMachine`, `ErasureStateMachine`, `SagaStateMachine`, `CoordinatorStateMachine`. Each encodes legal transitions; terminal states flagged.

#### 3.9 Codec Layer
`FrameHeader`, `FrameBody`, `WireFormat`, `Datagram`, per-type codecs (`EndpointCodec`, `OpRefCodec`, `OrderingPosCodec`, `NodeIdCodec`, etc.) for serde/bincode wire encoding. Version-tagged frames for forward compatibility.

#### 3.10 Internals
`TypeRegistry`, `VersionGraph`, `ContractValidator`, `InvariantChecker`, `EdgeCaseCompiler`, `OpRefAllocator`, `OrderingPositionClock`, `DurabilityTracker`, `ReplicationStreamMgr`, `SyncCheckpointMgr`, `RecoveryPlanBuilder`, `ConsistencyProfileMgr`, `BroadcastTopology`, `QuorumCalculator`, `SnapshotCoordinator`, `ByzantineVoteMgr`, `LeaseTracker`, `CRDTMergeEngine`, `ErasureCodec`, `SagaCoordinator`, `CoordinatorEpoch`.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Type duplication across crates (divergent `NodeId` definitions) | Single canonical definition in `types.rs`; downstream crates import `ids_000_aa::X`; CI lint forbids redefinition |
| Downstream crate implementing sealed trait (breaks forward-compat) | Private supertrait `private::Sealed`; compiler enforces; rustdoc documents seal |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP, etc.) | `Endpoint` is sole address type; dependency audit bans networking crates; clippy/CI rule |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 in allowed list; `ids-crypto` enforces at implementation layer |
| Version skew / wire incompatibility | `ProtocolVersion` + `EdgeCaseBits`; `VersionNegotiation` rejects mismatched peers before operation exchange |
| Invalid identifier (wrong length, hex decode failure) | `from_hex`/`from_str` return `Result` with `Invalid*Length` variants; `MAX_ADDRESS_SIZE` cap on `Endpoint.address` |
| Error type leakage / uncontrolled enum extension | thiserror `Error` enum is the contract; downstream matches but cannot add variants; recovery hints on every variant |
| Invariant violation at runtime | `InvariantChecker` validates all 40 rules; `can_transition_to` guards state machines; `is_valid()` on `PlacementDecision` |
| Overflow in `OrderingPos`/`Version`/`Epoch` arithmetic | `saturating_add`/`saturating_sub` everywhere; `MAX` constants defined |
| `OpRef` collision (36^9 namespace) | Allocator guarantees uniqueness; collision detection variant in error contract |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 001 ||
||-------|-------|-------------------|
| `ids-000-aa` (this crate) | Master crate; all canonical types, sealed contracts, invariants, versioning, errors |
| `ids-carrier` (Phase 002) | Implements `trait Carrier`; provides concrete `Endpoint` usage; depends on `ids-000-aa` |
| `ids-crypto` (Phase 003) | Implements allowed crypto; consumes `Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum` types from `ids-000-aa` |
| `ids-identity` (Phase 004) | `StableNodeId`/`RotatingNodeId` built on `NodeId`; depends on `ids-000-aa` |
| `ids-time` (Phase 005) | `Idstime` type; `IdsTime` already defined here; depends on `ids-000-aa` |
| All remaining `ids-*` crates (Phases 006–100) | Each imports canonical types + contracts from `ids-000-aa` only |

**Integration points:**
- `ids-carrier` implements `Carrier` (send/receive opaque datagrams over `Endpoint`); `ids-000-aa` declares the sealed trait but provides no implementation.
- `ids-crypto` implements signing/verification/encryption/hashing using `Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum` from `ids-000-aa`.
- `ids-time` builds its proprietary `Idstime` on top of `IdsTime` (u64 ticks) defined here.
- No dependency on `std::net`, `tokio::net`, libp2p, or any carrier-specific crate anywhere in `ids-000-aa`.
- Cargo workspace: `ids-000-aa` is a workspace member; all downstream crates declare `ids-000-aa = { path = "../ids-000-aa" }`.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-000-aa` (existing at `C:/idfs/ids-000-aa/`; this plan completes/aligns it)

```
ids-000-aa/
├── Cargo.toml                          # thiserror, serde, blake3; workspace member
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ALL canonical types (NodeId … Checksum, ~200 types)
│   ├── errors.rs                       # unified Error enum + domain sub-errors + recovery hints
│   ├── versioning.rs                   # ProtocolVersion, EdgeCaseBits, VersionRange, …
│   ├── prelude.rs                      # re-export glob for downstream convenience
│   │
│   ├── contracts/                      # sealed trait contracts (~30 files)
│   │   ├── mod.rs                      # declares all contract submodules
│   │   ├── carrier.rs                  # trait Carrier (send/receive opaque datagrams)
│   │   ├── transport.rs                # trait Transport (framing, sequencing, acks)
│   │   ├── store.rs                    # trait Store
│   │   ├── session.rs                  # trait Session
│   │   ├── routing.rs                  # trait Routing
│   │   ├── discovery.rs                # trait Discovery
│   │   ├── membership.rs               # trait Membership
│   │   ├── placement.rs                # trait Placement
│   │   ├── replication.rs              # trait Replication
│   │   ├── recovery.rs                 # trait Recovery
│   │   ├── consistency.rs              # trait Consistency
│   │   ├── ordering.rs                 # trait Ordering
│   │   ├── security.rs                 # trait Security
│   │   ├── client.rs                   # trait Client
│   │   ├── fault.rs                    # trait FaultTolerance
│   │   ├── broadcast.rs                # trait Broadcast
│   │   ├── quorum.rs                   # trait Quorum
│   │   ├── snapshot.rs                 # trait Snapshot
│   │   ├── byzantine.rs                # trait Byzantine
│   │   ├── lease.rs                    # trait Lease
│   │   ├── crdt.rs                     # trait CRDT
│   │   ├── erasure.rs                  # trait Erasure
│   │   ├── saga.rs                     # trait Saga
│   │   ├── coordinator.rs              # trait Coordinator
│   │   ├── durability.rs               # trait Durability
│   │   ├── topology.rs                 # trait Topology
│   │   ├── integration.rs              # trait Integration
│   │   ├── protocol.rs                 # trait Protocol
│   │   ├── wire.rs                     # trait Wire
│   │   ├── framing.rs                  # trait Framing
│   │   ├── sequencing.rs               # trait Sequencing
│   │   └── acks.rs                     # trait Acks
│   │
│   ├── invariants/                     # ~40 protocol invariant modules
│   │   ├── mod.rs
│   │   ├── binary_completion.rs
│   │   ├── dead_node_identity.rs
│   │   ├── idle_rotation.rs
│   │   ├── no_global_master.rs
│   │   ├── out_eq_in.rs
│   │   ├── coordinated_deletion.rs
│   │   ├── fencing_stonith.rs
│   │   ├── quorum_intersection.rs
│   │   ├── erasure_threshold.rs
│   │   ├── correlated_failure.rs
│   │   ├── hunit_compensation.rs
│   │   ├── regional_autonomy.rs
│   │   ├── hybrid_redundancy.rs
│   │   ├── operation_lifecycle.rs
│   │   ├── durability_ack.rs
│   │   ├── ordered_read_visibility.rs
│   │   ├── node_roster.rs
│   │   ├── region_affinity.rs
│   │   ├── identity_rotation.rs
│   │   ├── ids_standard_time.rs
│   │   ├── daily_integrity.rs
│   │   ├── db_retirement.rs
│   │   ├── read_fallback.rs
│   │   ├── topology_diversity.rs
│   │   ├── write_fanout.rs
│   │   ├── custody_graph.rs
│   │   ├── checksum_boundary.rs
│   │   ├── replica_freshness.rs
│   │   ├── commit_prefix.rs
│   │   ├── version_vector.rs
│   │   ├── membership_epoch.rs
│   │   ├── placement_policy.rs
│   │   ├── replication_lag.rs
│   │   ├── recovery_scope.rs
│   │   ├── data_lifecycle.rs
│   │   ├── snapshot_consistency.rs
│   │   ├── compute_to_data.rs
│   │   ├── hot_cold_tier.rs
│   │   ├── mobility_handoff.rs
│   │   └── federation_trust.rs
│   │
│   ├── codec/                          # wire encoding
│   │   ├── mod.rs
│   │   ├── frame_header.rs
│   │   ├── frame_body.rs
│   │   ├── wire_format.rs
│   │   ├── datagram.rs
│   │   ├── endpoint_codec.rs
│   │   ├── op_ref_codec.rs
│   │   ├── ordering_pos_codec.rs
│   │   ├── node_id_codec.rs
│   │   ├── region_id_codec.rs
│   │   ├── hunit_id_codec.rs
│   │   ├── dataset_id_codec.rs
│   │   ├── partition_id_codec.rs
│   │   ├── record_id_codec.rs
│   │   └── carrier_type_codec.rs
│   │
│   ├── state/                          # state machines (~20 files)
│   │   ├── mod.rs
│   │   ├── op_state_machine.rs
│   │   ├── commit_state_machine.rs
│   │   ├── node_state_machine.rs
│   │   ├── hunit_state_machine.rs
│   │   ├── region_state_machine.rs
│   │   ├── session_state_machine.rs
│   │   ├── membership_state_machine.rs
│   │   ├── placement_state_machine.rs
│   │   ├── replication_state_machine.rs
│   │   ├── recovery_state_machine.rs
│   │   ├── consistency_state_machine.rs
│   │   ├── ordering_state_machine.rs
│   │   ├── broadcast_state_machine.rs
│   │   ├── quorum_state_machine.rs
│   │   ├── snapshot_state_machine.rs
│   │   ├── byzantine_state_machine.rs
│   │   ├── lease_state_machine.rs
│   │   ├── crdt_state_machine.rs
│   │   ├── erasure_state_machine.rs
│   │   ├── saga_state_machine.rs
│   │   └── coordinator_state_machine.rs
│   │
│   ├── internals/                      # compile-time enforcement
│   │   ├── mod.rs
│   │   ├── type_registry.rs
│   │   ├── version_graph.rs
│   │   ├── contract_validator.rs
│   │   ├── invariant_checker.rs
│   │   ├── edge_case_compiler.rs
│   │   ├── op_ref_allocator.rs
│   │   ├── ordering_position_clock.rs
│   │   ├── durability_tracker.rs
│   │   ├── replication_stream_mgr.rs
│   │   ├── sync_checkpoint_mgr.rs
│   │   ├── recovery_plan_builder.rs
│   │   ├── consistency_profile_mgr.rs
│   │   ├── broadcast_topology.rs
│   │   ├── quorum_calculator.rs
│   │   ├── snapshot_coordinator.rs
│   │   ├── byzantine_vote_mgr.rs
│   │   ├── lease_tracker.rs
│   │   ├── crdt_merge_engine.rs
│   │   ├── erasure_codec.rs
│   │   ├── saga_coordinator.rs
│   │   └── coordinator_epoch.rs
│   │
│   ├── validation/                     # input validation rules
│   ├── recovery/                       # recovery plan types
│   ├── placement/                      # placement decision types
│   ├── routing/                        # routing types
│   ├── security/                       # security principal types
│   ├── time/                           # IdsTime helpers
│   ├── identity/                       # identity type helpers
│   ├── quorum/                         # quorum math types
│   ├── broadcast/                      # broadcast message types
│   ├── crdt/                           # CRDT opaque state/op/delta types
│   ├── erasure/                        # erasure fragment types
│   ├── snapshot/                       # snapshot types
│   ├── fault/                          # fault types
│   ├── byzantine/                      # byzantine vote types
│   ├── saga/                           # saga step types
│   ├── lease/                          # lease types
│   ├── chaos/                          # chaos fault types
│   └── observability/                  # observability event types
│
├── tests/                              # conformance tests (no source modification)
│   ├── types_tests.rs
│   ├── contract_sealing_tests.rs
│   ├── invariant_tests.rs
│   ├── versioning_tests.rs
│   ├── error_tests.rs
│   └── carrier_agnostic_tests.rs     # asserts no std::net/tokio::net usage
│
├── .gitignore
└── README.md
```

Total: ~90–150 files (within the ~90–1000 target range; room for growth as invariants/contracts expand). All files are `.rs` source + `Cargo.toml` + tests; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **Identifier newtype construction** — `pub struct NodeId(pub [u8; 32])` with const `ZERO`, `new`, `as_bytes`, `is_zero`, `to_hex`, `from_hex` (hex parse with length validation), `Display`. Applied uniformly to all ~200 ID types.
2. **Sealed trait dispatch** — `pub trait Carrier: private::Sealed` where `private::Sealed` is unnameable downstream; compiler guarantees only `ids-000-aa` (and designated impl crates via `impl Sealed for X` inside the crate) can implement.
3. **State-machine transition guard** — `OpState::can_transition_to(next)` encodes `received → ordered → executing → committed → synchronized → verified → failed` with `Verified`/`Failed` terminal; `CommitState::is_terminal()` / `is_success()` for commit lifecycle.
4. **Failure-domain overlap check** — `FailureDomain::shares_domain(&self, other)` compares power/facility/network-segment/operator/geographic Option<[u8;16]> fields for correlated-failure detection.
5. **Redundancy failure tolerance** — `RedundancyMode::failure_tolerance()` returns `Some(n-k)` for erasure-coded, `Some(0)` for full replica/hybrid.
6. **Consistency quorum requirement** — `ConsistencyProfile::requires_quorum()` true for `Strong`/`BoundedStaleness`; `allows_fallback()` false only for `Strong`.
7. **Durability level ordering** — `DurabilityCondition::level()` (1–6) and `meets(required)` for acknowledgement gating.
8. **Endpoint validation** — `Endpoint::new()` rejects `address.len() > MAX_ADDRESS_SIZE (1024)`.
9. **OpRef parsing** — exactly 9 ASCII alphanumeric bytes; `36^9` namespace; `from_str` validates length.
10. **Ordering position arithmetic** — `OrderingPos::next()`/`prev()` with `saturating_add`/`saturating_sub`; `ZERO`/`MAX` constants.
11. **Saturating tick arithmetic** — `IdsTime::next()`/`saturating_sub()`/`elapsed_since()` for proprietary time basis (not wall-clock).
12. **Version bitfield** — `EdgeCaseBits: u128` with one bit per invariant (40 bits used); `VersionGraph` DAG for upgrade/downgrade edges; `VersionNegotiation` rejects mismatched peers.
13. **Error categorization** — `Error::is_retryable()`, `is_fatal()`, `category()`, `plain_message()`, `suggested_retry_delay_ms()` for structured recovery.
14. **Codec encode/decode** — `FrameHeader::encode()`/`decode()` (16-byte fixed header: sequence u64, flags u8, payload_length u16, checksum u32, reserved); `FramedData::encode()`/`decode()` with payload-length validation.
15. **Join-semilattice merge (CRDT surface)** — `CRDTState`/`CRDTOp`/`CRDTDelta` opaque wrappers; merge = least upper bound; referenced by `crate::crdt` contract (actual CRDT classes live in later phases).

---

### 8. Dependencies

`ids-000-aa/Cargo.toml` (existing):
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — serialization for all types
- `blake3 = "1.5"` — content-addressed hashing (type-level surface only; no crypto ops here)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, `dns`, `wi-fi`, `bluetooth`, `cellular`, `vpn`, or `mesh` dependencies. No async runtime. No networking stack.

Downstream crate dependencies (all on `ids-000-aa` only):
- `ids-carrier` → `ids-000-aa`
- `ids-crypto` → `ids-000-aa`
- `ids-identity` → `ids-000-aa`
- `ids-time` → `ids-000-aa`
- (all 100 crates follow the same pattern)

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | All canonical types from prompts.txt entry 1 defined exactly once | `src/types.rs` contains `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `PlacementDecision`, `RedundancyMode`, `ConsistencyProfile`, `FailureDomain` + all additional types referenced by contracts/invariants (~200 total) |
| 2 | Sealed `trait Carrier` declared | `src/contracts/carrier.rs` has `pub trait Carrier: private::Sealed` with `send_datagram`/`receive_datagram` |
| 3 | All ~30 contract modules declared | `src/contracts/mod.rs` lists carrier, transport, store, session, routing, discovery, membership, placement, replication, recovery, consistency, ordering, security, client, fault, broadcast, quorum, snapshot, byzantine, lease, crdt, erasure, saga, coordinator, durability, topology, integration, protocol, wire, framing, sequencing, acks |
| 4 | ~40 invariants declared | `src/invariants/mod.rs` lists all invariant modules from research dimension 3.3 |
| 5 | Versioning types present | `src/versioning.rs` has `ProtocolVersion`, `EdgeCaseBits` (u128), `VersionRange`, `SemanticVersion`, `ProtocolFeature`, `ProtocolRegistry`, `CompatibilityMatrix`, `VersionGraph` |
| 6 | Unified error enum | `src/errors.rs` has `Error` enum with `thiserror::Error`, recovery hints, `is_retryable()`, `is_fatal()`, `category()` |
| 7 | `Endpoint` is sole address type | `Endpoint { carrier_type: CarrierType, address: Vec<u8> }`, `MAX_ADDRESS_SIZE = 1024`, no `SocketAddr`/`IpAddr` anywhere |
| 8 | No prohibited dependencies | `Cargo.toml` contains only `thiserror`, `serde`, `blake3` (+ `serde_json` dev); `cargo tree` shows no networking crates |
| 9 | Crate compiles cleanly | `cargo check -p ids-000-aa` exits 0 with no warnings |
| 10 | Conformance tests pass | `cargo test -p ids-000-aa` passes all type/contract/invariant/versioning/error tests |
| 11 | Carrier-agnostic assertion test passes | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| Exact set of `ids-*` crates that implement each sealed trait (which impl crate owns `Carrier`, `Transport`, etc.) | Affects `impl Sealed for X` placement in internals | Decided per-phase as dependent crates are created; `ids-carrier` owns `Carrier` |
| Whether `EdgeCaseBits` uses all 128 u128 bits or a subset | Wire-format size | 40 bits allocated per invariant list; remaining reserved |
| Serialization format for wire frames (bincode vs CBOR vs serde-derived) | Frame size, versioning | `codec/` modules use serde; format decision deferred to `ids-protocol` (Phase 012) |
| Whether `OpRef` generator lives in `ids-000-aa` (`OpRefAllocator`) or a dedicated crate | Allocator visibility | `OpRefAllocator` in `src/internals/`; actual allocation algorithm TBD |
| Whether invariants are compile-time (const generics) or runtime checks | Enforcement strength | `InvariantChecker` runtime; compile-time enforcement evaluated per invariant |
| Whether `ids-000-aa` should expose `no_std` compatibility | Embedded carrier support | Not required for Phase 001; `std` used for `Vec`, `HashMap`, etc. |

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Single `ids-000-aa` master crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; enforces single control point |
| Newtype wrappers for every identifier | Prevents accidental mixing of `NodeId` and `RegionId`; zero-cost (`#[repr(transparent)]` compatible via newtype) |
| Sealed traits via private supertrait | Rust API Guidelines C-SEALED; allows adding methods non-breakingly; downstream can use as bounds but not implement |
| `Endpoint` = `CarrierType + Vec<u8>` only | Carrier-agnostic absolute rule; no IP/TCP/UDP/QUIC/HTTP/DNS or any networking stack |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Per IDS policy; `ids-000-aa` declares type surface only, `ids-crypto` implements |
| `thiserror` for unified `Error` enum | Structured context + recovery hints; downstream wraps but cannot extend |
| `saturating_add`/`saturating_sub` for all tick/position arithmetic | No overflow panics in distributed ordering/commit |
| `MAX_ADDRESS_SIZE = 1024` on `Endpoint.address` | Bounded memory; prevents address-based DoS |
| `OpRef` = exactly 9 bytes (36^9 namespace) | Per prompts.txt; fixed-size for wire efficiency |
| Workspace Cargo layout with path dependencies | `ids-000-aa` at `C:/idfs/ids-000-aa/`; downstream crates depend by path |

---

### 12. Sources

1. Shapiro, Preguiça, Baquero, Zawirski — "Conflict-free Replicated Data Types," INRIA RR-7687 (2011). https://www.lip6.fr/Marc.Shapiro/papers/RR-7687.pdf — CRDT convergence, join-semilattice, CvRDT/CmRDT.
2. Almeida, Shoker, Baquero — "Delta State Replicated Data Types," arXiv:1603.01529 (2016). https://arxiv.org/html/1603.01529 — δ-CRDT, delta-mutators, anti-entropy.
3. Rust API Guidelines — "Future proofing: Sealed traits (C-SEALED)." https://rust-lang.github.io/api-guidelines/future-proofing.html — sealed trait pattern via private supertrait.
4. thiserror crate documentation. https://docs.rs/thiserror — error enum design with `#[error]`, `#[from]`, recovery context.
5. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf — SHA-256 specification.
6. RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA), ed25519. https://www.rfc-editor.org/rfc/rfc8032 — Ed25519 signature algorithm.
7. RFC 7539 — ChaCha20 and Poly1305 for IETF Protocols. https://www.rfc-editor.org/rfc/rfc7539 — ChaCha20-Poly1305 AEAD, 96-bit nonce.
8. Libsodium documentation — XChaCha20-Poly1305 construction. https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction — 192-bit nonce, random nonces safe.
9. HBase version/compatibility matrix. https://hbase.apache.org/docs/upgrading/version-number — major/minor/patch compatibility semantics.
10. Numax wire versioning. https://gianiac.github.io/numax/design/wire-versioning — PROTOCOL_VERSION gating, incompatible peer rejection.
11. Agent Host Protocol versioning & negotiation. https://microsoft.github.io/agent-host-protocol/specification/versioning — protocol version negotiation handshake.
12. qcp protocol compatibility levels. https://docs.rs/qcp/latest/qcp/protocol/control/enum.Compatibility.html — compatibility level identifiers.
13. BLAKE3 — GitHub BLAKE3-team/BLAKE3. https://github.com/BLAKE3-team/BLAKE3 — cryptographic hash function, Merkle-tree parallelization.
14. Stripe / AWS idempotency-key documentation. https://stripe.com/blog/idempotency — idempotency keys for exactly-once semantics.
15. Quorum intersection theory (r + w > n). https://pl.cs.uni-kl.de/homepage/teaching/ss19/progdist/slides/09_quorum.pdf — read/write quorum intersection.
16. CRDT approaches survey (CvRDT vs CmRDT, causal stability). https://arxiv.org/html/2310.18220v2 — state vs op-based, tombstone GC at causal stability.
17. Vector clock vs hybrid logical clock ordering. https://hld.handbook.academy/curriculum/distributed-systems-theory/clocks-and-ordering/ — logical clocks for causal ordering.
18. Erasure coding vs replication survey. https://dl.acm.org/doi/full/10.1145/3708994 — redundancy tradeoffs, storage efficiency vs repair bandwidth.
19. Consistency levels summary. https://arxiv.org/html/2409.01576v2 — strong, causal, bounded staleness, eventual, read-your-writes; availability upper bounds.
20. Cargo workspaces. https://doc.rust-lang.org/cargo/reference/workspaces.html — workspace members, shared Cargo.lock, path dependencies.
21. Cargo workspace structure patterns. https://github.com/tachyon-beep/skillpacks/blob/main/plugins/axiom-rust-workspaces/skills/using-rust-workspaces/workspace-structure-patterns.md — layered/feature-grouped/domain-grouped topologies.
22. IDS prompts.txt entry 1 (verbatim). C:/idfs/prompts.txt — Phase 001 crate definition, contents, constraints.
23. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml.
24. HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
25. HX-241 carrier-agnostic socket abstraction. https://raw.githubusercontent.com/hxlabz/hx241-protocol/main/hx-inbound/src/receive/socket.rs — Carrier/Socket/Endpoint traits with no networking-stack references.

---

**End of plan-001.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**