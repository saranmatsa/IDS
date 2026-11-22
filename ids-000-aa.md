# PLAN.md — ids-000-aa Master Crate (Central Hub)

**Project:** IDS — Independent Distributed System  
**Crate:** `ids-000-aa`  
**Role:** Central hub / father crate. Every canonical type, sealed trait contract, protocol invariant, error contract, and version bit lives here. All downstream `ids-*` crates depend on `ids-000-aa`.  
**Location:** `C:\IDS\ids-000-aa\`  
**Status:** PLAN — research complete (IDS.md, IDS-A.md, IDS-B.md, CAPABILITIES.md, DIFFERENCES.md, NOVEL.md, EDGE-CASES.md, DOMAINs.md, edge-case position/response docs reviewed), no code written yet  
**Format:** Matches hx-241 `plan.md` structure

---

## 1. Executive Summary

`ids-000-aa` is the father crate and central hub of the IDS workspace. It holds every canonical type, every sealed trait contract, every protocol invariant, every error contract, and every version bit. Every `pub` item added here is instantly available as `ids_000_aa::Thing` in all downstream `ids-*` crates — zero downstream edits for contract changes.

**Adding one edge case = edit exactly one file in `ids-000-aa`, then implement the new trait method in one dependent crate at a time.** You never touch thousands of downstream files to add a single invariant. This is the sealed-trait/type hub pattern: the contract lives in `ids-000-aa`, implementations live downstream.

**Target scale:** 335+ source files inside `ids-000-aa/src/` alone, plus all downstream `ids-*` crates. This is the central hub — everything flows through it.

**Carrier-agnostic absolute rule (no exceptions):** No `std::net`, `SocketAddr`, `IpAddr`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh anywhere. `ids-000-aa` defines only opaque `Endpoint` (`CarrierType` + `Vec<u8>`).

---

## 2. Architecture Overview

```
IDS WORKSPACE — CENTRAL HUB MODEL
==================================

                           ids-000-aa  (CENTRAL HUB / FATHER CRATE)
                           ┌─────────────────────────────────┐
                           │  src/lib.rs                     │
                           │  src/types.rs                   │
                           │  src/versioning.rs              │
                           │  src/errors.rs                  │
                           │  src/prelude.rs                 │
                           │  src/contracts/                 │
                           │  src/invariants/                │
                           │  src/internals/                 │
                           │  src/codec/                     │
                           │  src/state/                     │
                           │  src/validation/                │
                           │  src/recovery/                  │
                           │  src/placement/                 │
                           │  src/routing/                   │
                           │  src/security/                  │
                           │  src/time/                      │
                           │  src/identity/                  │
                           │  src/quorum/                    │
                           │  src/broadcast/                 │
                           │  src/crdt/                      │
                           │  src/erasure/                   │
                           │  src/snapshot/                  │
                           │  src/fault/                     │
                           │  src/byzantine/                 │
                           │  src/saga/                      │
                           │  src/lease/                     │
                           │  src/chaos/                     │
                           │  src/observability/             │
                           └─────────────────────────────────┘
                                     │
                    pub use re-export → ids_000_aa::Thing
                                     │
          ┌────────┬────────┬────────┼────────┬────────┬────────┐
          ▼        ▼        ▼        ▼        ▼        ▼        ▼
    ids-carrier  ids-crypto  ids-identity  ids-time  ids-topology  ids-membership
    ids-placement  ids-distribution  ids-discovery  ids-write  ids-read  ids-query
    ids-routing  ids-replication  ids-sync  ids-consistency  ids-broadcast  ids-quorum
    ids-crdt  ids-erasure  ids-lease  ids-recovery  ids-snapshot  ids-fault
    ids-byzantine  ids-saga  ids-integrity  ids-security  ids-resource  ids-lifecycle
    ids-compute  ids-federation  ids-accounting  ids-hotcold  ids-mobility  ids-selfreorg
    ids-immutable  ids-reclaim  ids-backup  ids-chaos  ids-observability  ids-client
    ids-cli  ids-testing  ids-bench  ids-doc

Dependency rule: ALL downstream crates → ids-000-aa ONLY.
ids-crypto is the ONLY crate that does NOT depend on ids-000-aa (avoids circular dependency).
No crate depends on another ids-* crate except through ids-000-aa sealed trait contracts.
```

---

## 3. File Structure — ids-000-aa (335 files, actual code not stubs)

```
ids-000-aa/
├── Cargo.toml                                    # Workspace member manifest
├── .cargo/
│   └── config.toml                               # Build configuration
├── PLAN.md                                       # This document
├── CONTRACTS.md                                  # Sealed trait contract reference (separate doc)
└── src/
    ├── lib.rs                                    # Crate root — pub use re-exports (central hub facade)
    ├── types.rs                                  # ALL canonical types — ONE definition each
    ├── versioning.rs                             # Protocol version + edge-case version bits
    ├── errors.rs                                 # Sealed error contracts (thiserror)
    ├── prelude.rs                                # Common imports re-exported
    │
    ├── contracts/                                # Sealed trait contracts — control plane interface (30 files)
    │   ├── mod.rs                                # Declares all contract submodules
    │   ├── carrier.rs            # trait Carrier — send/receive opaque datagrams (carrier-agnostic)
    │   ├── transport.rs          # trait Transport — framing, sequencing, acks, delivery semantics
    │   ├── store.rs              # trait Store — put/get/delete with durability conditions
    │   ├── session.rs            # trait Session — client ↔ IDS one-call binding (ids_submit)
    │   ├── routing.rs            # trait Routing — logical → physical resolution (locality-aware)
    │   ├── discovery.rs          # trait Discovery — node/H-unit/region discovery (gossip)
    │   ├── membership.rs         # trait Membership — join/leave/lifecycle (candidate→active→retired)
    │   ├── placement.rs          # trait Placement — where data lives (capacity-aware, failure-domain)
    │   ├── replication.rs        # trait Replication — streams, checkpoints, retries
    │   ├── recovery.rs           # trait Recovery — multi-level (node/H-unit/region)
    │   ├── consistency.rs        # trait Consistency — profiles, committed-prefix, MVCC
    │   ├── ordering.rs           # trait Ordering — dataset-scoped domains (no global master)
    │   ├── security.rs           # trait Security — auth, authz, encryption, key management
    │   ├── client.rs             # trait Client — THE ONE CALL (ids_submit)
    │   ├── fault.rs              # trait FaultTolerance — circuit breaker, bulkhead, backpressure
    │   ├── broadcast.rs          # trait Broadcast — reliable/causal/total order/atomic
    │   ├── quorum.rs             # trait Quorum — read/write quorum intersection
    │   ├── snapshot.rs           # trait Snapshot — distributed consistent state (Chandy-Lamport)
    │   ├── byzantine.rs          # trait Byzantine — BFT consensus, authenticated broadcast
    │   ├── lease.rs              # trait Lease — lease-based consistency, fencing epochs
    │   ├── crdt.rs               # trait CRDT — operation-based and state-based
    │   ├── erasure.rs            # trait Erasure — Reed-Solomon, regenerating codes
    │   ├── saga.rs               # trait Saga — compensating transactions
    │   ├── coordinator.rs        # trait Coordinator — domain-scoped leadership, STONITH
    │   ├── durability.rs         # trait Durability — acknowledgement conditions
    │   ├── topology.rs           # trait Topology — failure domain mapping
    │   ├── integration.rs        # trait Integration — cross-crate subsystem binding
    │   ├── protocol.rs           # trait Protocol — wire format versioning
    │   ├── wire.rs               # trait Wire — native frame format
    │   ├── framing.rs            # trait Framing — packet framing
    │   ├── sequencing.rs         # trait Sequencing — packet ordering
    │   └── acks.rs               # trait Acks — acknowledgement tracking
    │
    ├── invariants/                         # Protocol invariants — sealed, enforceable at compile time (40 files)
    │   ├── mod.rs                                # Declares all invariant submodules
    │   ├── binary_completion.rs      # 100% done or failed; no partial success terminal state (IDS edge-case position #2)
    │   ├── dead_node_identity.rs     # dead node never returns with old identity; fresh ID assigned (IDS edge-case position #1)
    │   ├── idle_rotation.rs          # RNI rotates only while node idle; active requests keep stable ID (IDS-B + EDGE-CASES #2)
    │   ├── no_global_master.rs       # dataset-scoped ordering only; no permanent global ordering master (IDS-A §40-41)
    │   ├── out_eq_in.rs              # byte accounting boundary: bytes-out == bytes-in+acked (IDS-EDGE-CASES-RESPONSES #3)
    │   ├── coordinated_deletion.rs   # all holders delete simultaneously within milliseconds or not at all (IDS edge-case position #6)
    │   ├── fencing_stonith.rs        # STONITH/fencing before any failover; split-brain prevention (DOMAINS Layer 14, 16)
    │   ├── quorum_intersection.rs    # read quorum ∩ write quorum ≠ ∅ (CAPABILITIES quorum section)
    │   ├── erasure_threshold.rs      # any k of n chunks reconstructs; tolerates n-k failures (IDS-A §37)
    │   ├── correlated_failure.rs     # replicas never share power/facility/network/operator domain (IDS-EDGE-CASES-RESPONSES #7)
    │   ├── hunit_compensation.rs     # H-unit failure compensation through another H-unit (IDS-A §36)
    │   ├── regional_autonomy.rs      # regional clusters operate independently for local operations (IDS §9)
    │   ├── hybrid_redundancy.rs      # full replication + erasure coding coexist per dataset (IDS-A §37)
    │   ├── operation_lifecycle.rs    # received→ordered→executing→committed→synchronized→verified→failed (IDS-A §42)
    │   ├── durability_ack.rs         # durability-aware acknowledgement; no overstating uncompleted replication (IDS-A §42)
    │   ├── ordered_read_visibility.rs # reads respect committed position prefix (IDS-A §44)
    │   ├── node_roster.rs            # client receives fixed node roster at join; stable (IDS-EDGE-CASES-DESIGN-POSITIONS #3)
    │   ├── region_affinity.rs        # client does not abruptly change regions; near-to-far routing (IDS-EDGE-CASES-DESIGN-POSITIONS #5)
    │   ├── identity_rotation.rs      # stable internal identity + rotating RNI; mapping protected (IDS-B §3-5)
    │   ├── ids_standard_time.rs      # proprietary time basis for ordering/commit, not wall-clock (IDS-EDGE-CASES-RESPONSES #2)
    │   ├── daily_integrity.rs        # daily integrity checks; "fishy" state detection (IDS-EDGE-CASES-RESPONSES #3)
    │   ├── db_retirement.rs          # temporary DB retirement during RNI rotation; returns online after window (IDS-EDGE-CASES-RESPONSES #4)
    │   ├── read_fallback.rs          # automatic fallback to next closest eligible replica (IDS-EDGE-CASES-RESPONSES #5)
    │   ├── topology_diversity.rs     # different locations, H-units, regions, failure domains per copy (IDS-EDGE-CASES-RESPONSES #7)
    │   ├── write_fanout.rs           # ~30 databases written simultaneously for durability (IDS-EDGE-CASES-RESPONSES #4)
    │   ├── custody_graph.rs          # data custody and responsibility graph (DIFFERENCES §11)
    │   ├── checksum_boundary.rs      # end-to-end checksum verification boundary (CAPABILITIES §49)
    │   ├── replica_freshness.rs      # replica freshness validation for reads (IDS-A §7.2)
    │   ├── commit_prefix.rs          # committed-prefix tracking for ordered reads (CAPABILITIES §64)
    │   ├── version_vector.rs         # vector clock for causality tracking (DOMAINS Layer 12)
    │   ├── membership_epoch.rs       # membership change epoch tracking (CAPABILITIES §85)
    │   ├── placement_policy.rs       # placement policy enforcement (IDS §5)
    │   ├── replication_lag.rs        # replication lag observability (CAPABILITIES §45)
    │   ├── recovery_scope.rs         # multi-level recovery scope classification (IDS-A §45)
    │   ├── data_lifecycle.rs         # data lifecycle state enforcement (IDS §16)
    │   ├── snapshot_consistency.rs   # snapshot consistency verification (CAPABILITIES §110-111)
    │   ├── compute_to_data.rs        # compute-to-data execution boundary (CAPABILITIES §119)
    │   ├── hot_cold_tier.rs          # hot/cold storage tier transition rules (CAPABILITIES §104)
    │   ├── mobility_handoff.rs       # dataset mobility across regions handoff (CAPABILITIES §116)
    │   └── federation_trust.rs       # multi-owner federation trust boundary (CAPABILITIES §101)
    │
    ├── internals/                          # Master crate internal implementation (not re-exported downstream) (20 files)
    │   ├── mod.rs
    │   ├── type_registry.rs        # Runtime type ID registry (NodeId, RegionId, etc. → type IDs)
    │   ├── version_graph.rs        # Version compatibility graph (protocol versions ↔ edge-case bits)
    │   ├── contract_validator.rs   # Compile-time contract validation (sealed trait bounds)
    │   ├── invariant_checker.rs    # Invariant checking at build time (all invariants satisfied)
    │   ├── edge_case_compiler.rs   # Compiles edge_case_registry bits into cfg flags for downstream
    │   ├── op_ref_allocator.rs     # Nine-character operation reference allocator (36^9 namespace)
    │   ├── ordering_position_clock.rs # Ordering position monotonic clock per dataset domain
    │   ├── durability_tracker.rs   # Tracks durability state per operation
    │   ├── replication_stream_mgr.rs # Manages replication stream lifecycle
    │   ├── sync_checkpoint_mgr.rs   # Manages synchronization checkpoints
    │   ├── recovery_plan_builder.rs # Builds multi-level recovery plans
    │   ├── consistency_profile_mgr.rs # Manages consistency profile selection
    │   ├── broadcast_topology.rs   # Broadcast message topology (reliable/causal/total order)
    │   ├── quorum_calculator.rs    # Calculates read/write quorum sets
    │   ├── snapshot_coordinator.rs # Coordinates distributed snapshots
    │   ├── byzantine_vote_mgr.rs   # Manages Byzantine consensus votes
    │   ├── lease_tracker.rs        # Tracks lease grants and renewals
    │   ├── crdt_merge_engine.rs    # CRDT merge engine (op-based and state-based)
    │   ├── erasure_codec.rs        # Erasure codec (Reed-Solomon, regenerating)
    │   ├── saga_coordinator.rs     # Saga compensating transaction coordinator
    │   └── coordinator_epoch.rs    # Coordinator epoch/term management
    │
    ├── codec/                              # Native wire format encoding/decoding (15 files)
    │   ├── mod.rs                                # Declares all codec submodules
    │   ├── frame_header.rs     # Native frame header encoding
    │   ├── frame_body.rs       # Native frame body encoding
    │   ├── wire_format.rs      # Complete wire format specification
    │   ├── datagram.rs         # Opaque Datagram type (CarrierType + Vec<u8>)
    │   ├── endpoint_codec.rs   # Endpoint encode/decode (opaque address bytes)
    │   ├── op_ref_codec.rs     # OpRef encode/decode (9-char alphanumeric)
    │   ├── ordering_pos_codec.rs # OrderingPos encode/decode
    │   ├── node_id_codec.rs    # NodeId encode/decode
    │   ├── region_id_codec.rs  # RegionId encode/decode
    │   ├── hunit_id_codec.rs   # HUnitId encode/decode
    │   ├── dataset_id_codec.rs # DatasetId encode/decode
    │   ├── partition_id_codec.rs # PartitionId encode/decode
    │   ├── record_id_codec.rs  # RecordId encode/decode
    │   └── carrier_type_codec.rs # CarrierType encode/decode
    │
    ├── state/                              # State machines for all lifecycle states (20 files)
    │   ├── mod.rs                                # Declares all state submodules
    │   ├── op_state_machine.rs     # OpState transitions (received→ordered→executing→committed→synchronized→verified→failed)
    │   ├── commit_state_machine.rs # CommitState transitions (accepted→prepared→committed→aborted)
    │   ├── node_state_machine.rs   # NodeState transitions (candidate→validating→active→degraded→draining→failed→retired)
    │   ├── hunit_state_machine.rs  # HUnitState transitions (forming→operational→degraded→compensating→retired)
    │   ├── region_state_machine.rs # RegionState transitions (registering→active→isolated→draining→restored→removed)
    │   ├── session_state_machine.rs # Session lifecycle state machine
    │   ├── membership_state_machine.rs # Membership state machine (join/leave/lifecycle)
    │   ├── placement_state_machine.rs # Placement decision state machine
    │   ├── replication_state_machine.rs # Replication stream state machine
    │   ├── recovery_state_machine.rs # Multi-level recovery state machine
    │   ├── consistency_state_machine.rs # Consistency profile state machine
    │   ├── ordering_state_machine.rs # Ordering domain state machine
    │   ├── broadcast_state_machine.rs # Broadcast delivery state machine
    │   ├── quorum_state_machine.rs # Quorum calculation state machine
    │   ├── snapshot_state_machine.rs # Distributed snapshot state machine
    │   ├── byzantine_state_machine.rs # Byzantine consensus state machine
    │   ├── lease_state_machine.rs  # Lease grant/renew/expiry state machine
    │   ├── crdt_state_machine.rs   # CRDT merge state machine
    │   ├── erasure_state_machine.rs # Erasure encode/decode state machine
    │   ├── saga_state_machine.rs   # Saga transaction state machine
    │   └── coordinator_state_machine.rs # Coordinator leadership state machine
    │
    ├── validation/                         # Cross-cutting validation logic (15 files)
    │   ├── mod.rs                                # Declares all validation submodules
    │   ├── op_validation.rs      # Operation validation before execution
    │   ├── placement_validation.rs # Placement decision validation
    │   ├── routing_validation.rs # Routing decision validation
    │   ├── replication_validation.rs # Replication stream validation
    │   ├── recovery_validation.rs # Recovery plan validation
    │   ├── consistency_validation.rs # Consistency profile validation
    │   ├── ordering_validation.rs # Ordering domain validation
    │   ├── security_validation.rs # Security policy validation
    │   ├── broadcast_validation.rs # Broadcast message validation
    │   ├── quorum_validation.rs   # Quorum set validation
    │   ├── snapshot_validation.rs # Snapshot consistency validation
    │   ├── byzantine_validation.rs # Byzantine behavior validation
    │   ├── erasure_validation.rs  # Erasure fragment validation
    │   └── integrity_validation.rs # End-to-end integrity validation
    │
    ├── recovery/                           # Multi-level recovery implementation (15 files)
    │   ├── mod.rs                                # Declares all recovery submodules
    │   ├── node_recovery.rs      # Single node failure recovery
    │   ├── hunit_recovery.rs     # H-unit failure compensation
    │   ├── regional_recovery.rs  # Regional failure compensation
    │   ├── erasure_reconstruct.rs # Erasure-coded fragment reconstruction
    │   ├── replica_copy.rs       # Replica-based data copy
    │   ├── capacity_bounded.rs   # Capacity-bounded recovery planning
    │   ├── ownership_transfer.rs # Ownership-aware recovery transfer
    │   ├── redundancy_restore.rs # Redundancy restoration after recovery
    │   ├── integrity_check.rs    # Post-recovery integrity verification
    │   ├── plan_builder.rs       # Recovery plan construction
    │   ├── scope_classifier.rs   # Recovery scope classification (node/H-unit/region)
    │   ├── pending_ops.rs        # Pending operation reconciliation during recovery
    │   ├── reconciliation.rs     # Replica reconciliation after recovery
    │   └── verification.rs       # Final recovery verification
    │
    ├── placement/                          # Data placement engine (15 files)
    │   ├── mod.rs                                # Declares all placement submodules
    │   ├── engine.rs         # Core placement engine
    │   ├── capacity_tracker.rs # Tracks node/H-unit/region capacity
    │   ├── failure_domain.rs # Failure domain modeling (power/facility/network/operator/geographic)
    │   ├── policy_parser.rs  # Placement policy parser
    │   ├── hunit_selector.rs # H-unit selection for placement
    │   ├── region_selector.rs # Region selection for placement
    │   ├── node_selector.rs  # Node selection within H-unit
    │   ├── rebalancer.rs     # Dynamic rebalancing on node addition
    │   ├── migration_planner.rs # Partition migration planner
    │   ├── hot_cold_tier.rs  # Adaptive hot/cold/warm storage tiering
    │   ├── recovery_cost.rs  # Recovery-cost-aware placement
    │   ├── diversity_checker.rs # Failure-domain diversity checker
    │   ├── topology_aware.rs # Topology-aware placement
    │   └── reservation.rs    # Capacity reservation tracking
    │
    ├── routing/                            # Data-aware request routing (15 files)
    │   ├── mod.rs                                # Declares all routing submodules
    │   ├── resolver.rs         # Logical → physical request resolver
    │   ├── locality_aware.rs   # Locality-aware routing (geography, health, load)
    │   ├── replica_aware.rs    # Replica-aware routing
    │   ├── near_to_far.rs      # Near-to-far routing (client region affinity)
    │   ├── fallback.rs         # Closest-replica fallback
    │   ├── table.rs            # Routing table management
    │   ├── path_selector.rs    # Path selection algorithm
    │   ├── loop_prevention.rs  # Routing loop prevention
    │   ├── convergence.rs      # Routing convergence
    │   ├── expiration.rs       # Route expiration/revalidation
    │   ├── locality_metrics.rs # Locality metrics collection
    │   ├── workload_aware.rs   # Workload-aware routing
    │   ├── resource_aware.rs   # Resource-aware routing
    │   └── multi_path.rs       # Multi-path routing support
    │
    ├── security/                           # Security subsystem (15 files)
    │   ├── mod.rs                                # Declares all security submodules
    │   ├── auth.rs             # Authentication (mutual)
    │   ├── authz.rs            # Authorization (dataset/operation/identity-level)
    │   ├── encryption.rs       # Data encryption (in transit + at rest)
    │   ├── key_mgmt.rs         # Cryptographic key management
    │   ├── key_rotation.rs     # Key rotation policies
    │   ├── key_revocation.rs   # Key revocation
    │   ├── audit.rs            # Audit trail (tamper-evident)
    │   ├── regional_boundary.rs # Regional security boundaries
    │   ├── nonce_mgr.rs        # Nonce management
    │   ├── replay_protection.rs # Replay attack protection
    │   ├── channel_binding.rs  # Constant-time channel binding comparison
    │   ├── ephemeral_key.rs    # Ephemeral public key validation (reject all-zeros)
    │   ├── address_limit.rs    # Endpoint address size limits (1024 bytes)
    │   └── zero_nonce.rs       # Zero nonce rejection (nonce.0 != 0)
    │
    ├── time/                               # IDS standard time (10 files)
    │   ├── mod.rs                                # Declares all time submodules
    │   ├── ids_time.rs         # IDS standard time type (proprietary, not wall-clock)
    │   ├── tick_source.rs      # Tick source for time basis
    │   ├── drift_tolerance.rs  # Clock drift tolerance
    │   ├── sync.rs             # Cross-region time synchronization
    │   ├── ordering_ts.rs      # Ordering timestamps
    │   ├── commit_ts.rs        # Commit timestamps
    │   ├── durability_window.rs # Durability windows
    │   ├── cross_region.rs     # Cross-region time coordination
    │   └── clock_drift.rs      # Clock drift absorption
    │
    ├── identity/                           # Stable internal identity + rotating RNI (10 files)
    │   ├── mod.rs                                # Declares all identity submodules
    │   ├── stable_id.rs        # Stable internal node identity
    │   ├── rotating_id.rs      # Rotating network-facing identifier (RNI)
    │   ├── mapping.rs          # RNI → stable ID mapping (protected)
    │   ├── validity_window.rs  # RNI validity window
    │   ├── transition.rs       # RNI transition state (scheduled/active/superseded/expired/revoked)
    │   ├── resolution.rs       # Authenticated RNI resolution
    │   ├── revocation.rs       # RNI revocation
    │   ├── protected_mapping.rs # Protected mapping access
    │   └── idle_enforcement.rs # Idle-only rotation enforcement
    │
    ├── quorum/                             # Quorum systems (10 files)
    │   ├── mod.rs                                # Declares all quorum submodules
    │   ├── read_quorum.rs        # Read quorum selection
    │   ├── write_quorum.rs       # Write quorum selection
    │   ├── intersection.rs     # Quorum intersection check (read ∩ write ≠ ∅)
    │   ├── calculator.rs       # Quorum capacity/fault tolerance calculator
    │   ├── flex_paxos.rs       # Flexible Paxos quorum intersection
    │   ├── capacity.rs         # Quorum capacity calculation
    │   ├── fault_tolerance.rs  # Quorum fault tolerance
    │   ├── domain.rs           # Quorum domain management
    │   ├── membership.rs       # Quorum membership tracking
    │   └── validator.rs        # Quorum validator
    │
    ├── broadcast/                          # Broadcast protocols (10 files)
    │   ├── mod.rs                                # Declares all broadcast submodules
    │   ├── reliable.rs         # Reliable broadcast
    │   ├── causal.rs           # Causal broadcast
    │   ├── total_order.rs      # Total order broadcast (atomic broadcast)
    │   ├── atomic.rs           # Atomic broadcast
    │   ├── fifo_total.rs       # FIFO-total order broadcast
    │   ├── sm_replication.rs   # State machine replication
    │   ├── agreement.rs        # Uniform agreement
    │   ├── integrity.rs        # Uniform integrity
    │   ├── validity.rs         # Validity property
    │   └── termination.rs      # Termination property
    │
    ├── crdt/                               # Conflict-free replicated data types (10 files)
    │   ├── mod.rs                                # Declares all CRDT submodules
    │   ├── op_based.rs         # Operation-based CRDTs
    │   ├── state_based.rs      # State-based CRDTs
    │   ├── delta_crdt.rs       # Delta-CRDTs
    │   ├── g_counter.rs        # Grow-only counter
    │   ├── g_set.rs            # Grow-only set
    │   ├── two_phase_set.rs    # Two-phase set
    │   ├── lww_register.rs     # Last-writer-wins register
    │   ├── or_set.rs           # Observed-remove set
    │   ├── pn_counter.rs       # PN-counter
    │   └── convergence.rs      # Deterministic convergence verification
    │
    ├── erasure/                            # Erasure coding (10 files)
    │   ├── mod.rs                                # Declares all erasure submodules
    │   ├── reed_solomon.rs     # Reed-Solomon (n,k) codes
    │   ├── systematic.rs       # Systematic form encoding
    │   ├── regenerating.rs     # Regenerating codes
    │   ├── lrc.rs              # Locally recoverable codes
    │   ├── encode.rs           # Encoding
    │   ├── decode.rs           # Decoding (any k of n)
    │   ├── reconstruct.rs      # Reconstruction
    │   ├── ppr.rs              # Partial Parallel Repair
    │   └── fragment_mgr.rs     # Fragment identity/placement tracking
    │
    ├── snapshot/                           # Distributed snapshots (10 files)
    │   ├── mod.rs                                # Declares all snapshot submodules
    │   ├── chandy_lamport.rs   # Chandy-Lamport algorithm
    │   ├── marker.rs           # Marker message handling
    │   ├── state_record.rs     # Process state recording
    │   ├── channel_state.rs    # In-flight message channel state
    │   ├── snapshot_id.rs      # Snapshot identifier
    │   ├── coordinator.rs      # Snapshot coordinator
    │   ├── savepoint.rs        # Savepoint management
    │   ├── checkpoint.rs       # Checkpoint management
    │   └── restore.rs          # State restoration from snapshot
    │
    ├── fault/                              # Fault tolerance (10 files)
    │   ├── mod.rs                                # Declares all fault submodules
    │   ├── circuit_breaker.rs  # Circuit breaker (closed/open/half-open)
    │   ├── bulkhead.rs         # Bulkhead isolation
    │   ├── backpressure.rs     # Credit-based backpressure
    │   ├── load_shedding.rs    # Load shedding
    │   ├── graceful_degrade.rs # Graceful degradation hierarchy
    │   ├── retry_storm.rs      # Retry storm prevention
    │   ├── thundering_herd.rs  # Thundering herd prevention
    │   ├── isolation.rs        # Failure isolation
    │   └── degradation_hierarchy.rs # Degradation hierarchy states
    │
    ├── byzantine/                          # Byzantine fault tolerance (10 files)
    │   ├── mod.rs                                # Declares all byzantine submodules
    │   ├── pbft.rs             # PBFT consensus
    │   ├── hotstuff.rs         # HotStuff consensus
    │   ├── authenticated_broadcast.rs # Authenticated Byzantine broadcast (Dolev-Strong)
    │   ├── sybil.rs            # Sybil resistance
    │   ├── eclipse.rs          # Eclipse resistance
    │   ├── replay.rs           # Replay attack protection
    │   ├── traffic_manip.rs    # Traffic manipulation detection
    │   ├── dos.rs              # DoS resistance
    │   └── quarantine.rs       # Byzantine node isolation/quarantine
    │
    ├── saga/                               # Compensating transactions (10 files)
    │   ├── mod.rs                                # Declares all saga submodules
    │   ├── choreography.rs     # Choreography-based saga
    │   ├── orchestration.rs    # Orchestration-based saga
    │   ├── local_tx.rs         # Local transaction management
    │   ├── compensate.rs       # Compensating transaction logic
    │   ├── idempotency.rs      # Idempotency key management
    │   ├── state_machine.rs    # Saga state machine
    │   ├── coordinator.rs      # Saga orchestrator
    │   ├── deadlock.rs         # Deadlock detection
    │   └── livelock.rs         # Livelock prevention
    │
    ├── lease/                              # Lease-based consistency (10 files)
    │   ├── mod.rs                                # Declares all lease submodules
    │   ├── grant.rs            # Lease grant
    │   ├── renew.rs            # Lease renewal
    │   ├── fencing_token.rs    # Fencing token management
    │   ├── epoch.rs            # Epoch/term management
    │   ├── stonith.rs          # STONITH execution
    │   ├── expiry.rs           # Lease expiry handling
    │   ├── renewal.rs          # Lease renewal scheduling
    │   ├── authority.rs        # Authority epoch management
    │   └── split_brain.rs      # Split-brain prevention
    │
    ├── chaos/                              # Fault injection / chaos engineering (10 files)
    │   ├── mod.rs                                # Declares all chaos submodules
    │   ├── injector.rs         # Fault injection engine
    │   ├── packet_loss.rs      # Packet loss injection
    │   ├── reorder.rs          # Packet reordering injection
    │   ├── duplicate.rs        # Packet duplication injection
    │   ├── corruption.rs       # Network corruption injection
    │   ├── partition.rs        # Network partition injection
    │   ├── churn.rs            # Node churn simulation
    │   ├── resource_exhaust.rs # Resource exhaustion simulation
    │   ├── byzantine_sim.rs    # Byzantine behavior simulation
    │   └── harness.rs          # Chaos experiment harness
    │
    └── observability/                      # Metrics, tracing, audit (10 files)
        ├── mod.rs                                # Declares all observability submodules
        ├── metrics.rs          # Metrics collection
        ├── tracing.rs          # Distributed tracing
        ├── audit.rs            # Tamper-evident audit trail
        ├── telemetry.rs        # Telemetry pipeline
        ├── control_plane.rs    # Control plane observability
        ├── data_plane.rs       # Data plane observability
        ├── sli.rs              # Service level indicators
        ├── slo.rs              # Service level objectives
        └── dashboard.rs        # Observability dashboard
```

**File count inside `ids-000-aa/src/`:** 335 files
- Root: 5 files
- `contracts/`: 30 files
- `invariants/`: 40 files
- `internals/`: 20 files
- `codec/`: 15 files
- `state/`: 20 files
- `validation/`: 15 files
- `recovery/`: 15 files
- `placement/`: 15 files
- `routing/`: 15 files
- `security/`: 15 files
- `time/`: 10 files
- `identity/`: 10 files
- `quorum/`: 10 files
- `broadcast/`: 10 files
- `crdt/`: 10 files
- `erasure/`: 10 files
- `snapshot/`: 10 files
- `fault/`: 10 files
- `byzantine/`: 10 files
- `saga/`: 10 files
- `lease/`: 10 files
- `chaos/`: 10 files
- `observability/`: 10 files

---

## 4. Canonical Types (`src/types.rs`)

These are the **only** definitions of these types in the entire workspace. Every `ids-*` crate imports them as `ids_000_aa::NodeId`, etc.

```rust
// Core identifiers
pub struct NodeId(pub [u8; 32]);        // BLAKE3(ed25519 pubkey) — stable internal identity (IDS-B §3.1)
pub struct RegionId(pub [u8; 32]);
pub struct HUnitId(pub [u8; 32]);
pub struct DatasetId(pub [u8; 32]);
pub struct PartitionId(pub [u8; 32]);
pub struct RecordId(pub [u8; 32]);
pub struct OpRef(pub [u8; 9]);          // Nine-char alphanumeric, packed (IDS-A §38, 47: 36^9 namespace)
pub struct OrderingPos(pub u64);        // Ordering position, separate from OpRef (IDS-A §38.3)

// Carrier-agnostic address (NO IP, NO port, NO SocketAddr)
pub struct Endpoint {
    pub carrier_type: CarrierType,
    pub address: Vec<u8>,               // Opaque carrier-specific bytes (hx-inbound pattern)
}
pub enum CarrierType {
    Loopback,
    Serial,
    Optical,
    Radio,
    Custom(u16),
}

// Rotating network-facing identifier (IDS-B §3.2)
pub struct RotatingNodeId(pub [u8; 16]); // RNI — changes per rotation policy, maps to stable NodeId

// Operation states (IDS-A §42, CAPABILITIES §61)
pub enum OpState { Received, Ordered, Executing, Committed, Synchronized, Verified, Failed }
pub enum CommitState { Accepted, Prepared, Committed, Aborted }

// Redundancy and consistency (IDS-A §37, CAPABILITIES §41-42, 51)
pub enum RedundancyMode { FullReplica, ErasureCoded { k: u32, n: u32 }, Hybrid }
pub enum ConsistencyProfile { Strong, Causal, BoundedStaleness, Eventual, ReadYourWrites }

// Placement decision (IDS §5, CAPABILITIES §21-28)
pub struct PlacementDecision {
    pub region: RegionId,
    pub hunit: HUnitId,
    pub node: NodeId,
    pub redundancy: RedundancyMode,
    pub failure_domain: FailureDomain,
}

// Failure domain (DOMAINS Layer 3, IDS-EDGE-CASES-RESPONSES #7)
pub struct FailureDomain {
    pub power: Option<[u8; 16]>,
    pub facility: Option<[u8; 16]>,
    pub network_segment: Option<[u8; 16]>,
    pub operator: Option<[u8; 16]>,
    pub geographic: Option<[u8; 16]>,
}

// Node lifecycle state (CAPABILITIES §67, IDS §13)
pub enum NodeState { Candidate, Validating, Active, Degraded, Draining, Failed, Retired }

// H-unit state (IDS-A §35, CAPABILITIES §3)
pub enum HUnitState { Forming, Operational, Degraded, Compensating, Retired }

// Region state (IDS §9, CAPABILITIES §87)
pub enum RegionState { Registering, Active, Isolated, Draining, Restored, Removed }

// Durability condition (IDS-A §42)
pub enum DurabilityCondition { Received, Accepted, Executed, LocalCommitted, ReplicasAcked, CrossRegionCompleted }

// Ids standard time (IDS-EDGE-CASES-RESPONSES #2)
pub struct IdsTime(pub u64); // Proprietary time basis
```

---

## 5. Sealed Trait Contracts (`src/contracts/`)

Every contract is sealed: `pub trait X: private::Sealed {}`. Downstream crates can **call** methods and use as trait bounds, but cannot implement. Only `ids-000-aa` and its designated implementation crates can `impl`. Adding a method to any contract here is non-breaking for downstream — zero edits to thousands of files.

Each contract file contains:
- The sealed trait declaration
- The `private::Sealed` supertrait module
- Method signatures with full `Result<T, Error>` error contracts
- Doc comments explaining the invariant each method enforces

### Contract specifications (30 files, drawn from IDS docs)

**`carrier.rs` — `trait Carrier`** (carrier-agnostic transport)
- `send_datagram(endpoint: &Endpoint, data: &[u8]) -> Result<(), CarrierError>`
- `receive_datagram() -> Result<(Endpoint, Vec<u8>), CarrierError>`
- `carrier_type() -> CarrierType`
- `endpoint_address_size_limit() -> usize` (default 1024 bytes, from IDS edge-case hardening)

**`transport.rs` — `trait Transport`**
- `frame(payload: Vec<u8>) -> Result<Vec<u8>, TransportError>`
- `deframe(bytes: Vec<u8>) -> Result<Vec<u8>, TransportError>`
- `sequence(frame_id: u64) -> Result<(), TransportError>`
- `acknowledge(sequence_id: u64) -> Result<(), TransportError>`
- `delivery_semantics() -> DeliverySemantics`

**`store.rs` — `trait Store`**
- `put(key: &[u8], value: Vec<u8>, durability: DurabilityCondition) -> Result<CommitState, StoreError>`
- `get(key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>`
- `delete(key: &[u8]) -> Result<CommitState, StoreError>`
- `durability_condition() -> DurabilityCondition`

**`session.rs` — `trait Session`** (THE ONE CALL binding)
- `ids_submit(op: Operation) -> Result<OpRef, SessionError>`
- `session_create(client_id: NodeId) -> Result<SessionId, SessionError>`
- `session_expire(session_id: SessionId) -> Result<(), SessionError>`
- `session_heartbeat(session_id: SessionId) -> Result<(), SessionError>`

**`routing.rs` — `trait Routing`**
- `resolve_logical(dataset_id: &DatasetId, record_id: &RecordId) -> Result<Endpoint, RoutingError>`
- `locality_aware_select(candidates: Vec<Endpoint>, profile: ConsistencyProfile) -> Result<Endpoint, RoutingError>`
- `replica_aware_fallback(primary: Endpoint, profile: ConsistencyProfile) -> Result<Endpoint, RoutingError>`

**`discovery.rs` — `trait Discovery`**
- `discover_node(hint: Option<&Endpoint>) -> Result<Vec<NodeId>, DiscoveryError>`
- `discover_hunit(hunit_id: &HUnitId) -> Result<Vec<NodeId>, DiscoveryError>`
- `discover_region(region_id: &RegionId) -> Result<Vec<HUnitId>, DiscoveryError>`
- `gossip_propagate(announcement: Announcement) -> Result<(), DiscoveryError>`

**`membership.rs` — `trait Membership`**
- `join(node_id: NodeId, region: RegionId, hunit: HUnitId, capacity: Capacity) -> Result<(), MembershipError>`
- `leave(node_id: NodeId) -> Result<(), MembershipError>`
- `lifecycle_transition(node_id: NodeId, new_state: NodeState) -> Result<(), MembershipError>`
- `capacity_advertise(node_id: NodeId, capacity: Capacity) -> Result<(), MembershipError>`

**`placement.rs` — `trait Placement`**
- `place_dataset(dataset_id: &DatasetId, policy: PlacementPolicy) -> Result<PlacementDecision, PlacementError>`
- `place_partition(partition_id: &PartitionId, policy: PlacementPolicy) -> Result<PlacementDecision, PlacementError>`
- `capacity_aware_select(candidates: Vec<NodeId>, required: u64) -> Result<Vec<NodeId>, PlacementError>`
- `failure_domain_aware(replicas: Vec<NodeId>) -> Result<bool, PlacementError>`

**`replication.rs` — `trait Replication`**
- `replicate_stream(dataset_id: &DatasetId, changes: ChangeStream) -> Result<(), ReplicationError>`
- `checkpoint(dataset_id: &DatasetId, version: Version) -> Result<CheckpointId, ReplicationError>`
- `retry_sync(checkpoint: CheckpointId) -> Result<(), ReplicationError>`

**`recovery.rs` — `trait Recovery`**
- `compensate_node_failure(node: NodeId) -> Result<RecoveryPlan, RecoveryError>`
- `compensate_hunit_failure(hunit: HUnitId) -> Result<RecoveryPlan, RecoveryError>`
- `compensate_regional_failure(region: RegionId) -> Result<RecoveryPlan, RecoveryError>`
- `reconstruct_erasure(fragments: Vec<FragmentId>) -> Result<Vec<u8>, RecoveryError>`
- `verify_recovery(proof: RecoveryProof) -> Result<(), RecoveryError>`
- `restore_redundancy(dataset_id: &DatasetId) -> Result<(), RecoveryError>`

**`consistency.rs` — `trait Consistency`**
- `profile(dataset_id: &DatasetId) -> ConsistencyProfile`
- `committed_prefix(domain: OrderingDomain) -> Result<OrderingPos, ConsistencyError>`
- `ordered_read_visibility(position: OrderingPos) -> Result<bool, ConsistencyError>`
- `mvcc_snapshot(dataset_id: &DatasetId) -> Result<SnapshotId, ConsistencyError>`

**`ordering.rs` — `trait Ordering`**
- `dataset_domain(dataset_id: &DatasetId) -> OrderingDomain`
- `assign_position(domain: &OrderingDomain, op_ref: OpRef) -> Result<OrderingPos, OrderingError>`
- `no_global_master() -> bool` (always true — dataset-scoped only)
- `concurrent_write_resolve(domain: &OrderingDomain, ops: Vec<OpRef>) -> Result<Vec<OpRef>, OrderingError>`

**`security.rs` — `trait Security`**
- `authenticate(identity: NodeId, credential: Credential) -> Result<AuthToken, SecurityError>`
- `authorize(token: AuthToken, dataset: DatasetId, op: OpType) -> Result<(), SecurityError>`
- `encrypt(data: Vec<u8>, key_id: KeyId) -> Result<Vec<u8>, SecurityError>`
- `decrypt(data: Vec<u8>, key_id: KeyId) -> Result<Vec<u8>, SecurityError>`
- `rotate_key(key_id: KeyId) -> Result<KeyId, SecurityError>`
- `revoke_key(key_id: KeyId) -> Result<(), SecurityError>`

**`client.rs` — `trait Client`** (THE ONE CALL)
- `ids_submit(op: Operation) -> Result<OpResult, ClientError>`
- `op_reference(op: &Operation) -> OpRef`
- `idempotency_key(op: &Operation) -> IdempotencyKey`

**`fault.rs` — `trait FaultTolerance`**
- `circuit_breaker_state(peer: NodeId) -> Result<CircuitState, FaultError>`
- `bulkhead_isolate(domain: FailureDomain) -> Result<(), FaultError>`
- `backpressure_apply(endpoint: Endpoint, credit: u64) -> Result<(), FaultError>`
- `graceful_degrade(level: DegradationLevel) -> Result<(), FaultError>`

**`broadcast.rs` — `trait Broadcast`**
- `reliable_broadcast(message: Message) -> Result<(), BroadcastError>`
- `causal_broadcast(message: Message, deps: Vec<MessageId>) -> Result<(), BroadcastError>`
- `total_order_broadcast(message: Message) -> Result<(), BroadcastError>`
- `atomic_broadcast(message: Message) -> Result<(), BroadcastError>`

**`quorum.rs` — `trait Quorum`**
- `read_quorum(replicas: Vec<NodeId>, r: u32) -> Result<Vec<NodeId>, QuorumError>`
- `write_quorum(replicas: Vec<NodeId>, w: u32) -> Result<Vec<NodeId>, QuorumError>`
- `intersection_check(read: &[NodeId], write: &[NodeId]) -> Result<bool, QuorumError>`

**`snapshot.rs` — `trait Snapshot`**
- `chandy_lamport_initiate(initiator: NodeId) -> Result<SnapshotId, SnapshotError>`
- `marker_send(channel: ChannelId) -> Result<(), SnapshotError>`
- `state_record(node_id: NodeId, state: NodeState) -> Result<(), SnapshotError>`
- `channel_state_record(channel: ChannelId, messages: Vec<Message>) -> Result<(), SnapshotError>`

**`byzantine.rs` — `trait Byzantine`**
- `bft_consensus(proposal: Value, replicas: Vec<NodeId>) -> Result<Value, ByzantineError>`
- `authenticated_broadcast(message: Message, signature: Signature) -> Result<(), ByzantineError>`
- `sybil_resistance(identity: NodeId) -> Result<bool, ByzantineError>`
- `eclipse_resistance(victim: NodeId) -> Result<bool, ByzantineError>`
- `replay_protection(op_ref: OpRef) -> Result<bool, ByzantineError>`

**`lease.rs` — `trait Lease`**
- `grant_lease(node_id: NodeId, epoch: Epoch, duration: Duration) -> Result<LeaseId, LeaseError>`
- `renew_lease(lease_id: LeaseId) -> Result<(), LeaseError>`
- `fencing_token_increment(lease_id: LeaseId) -> Result<FencingToken, LeaseError>`
- `fencing_token_check(token: FencingToken) -> Result<bool, LeaseError>`
- `stonith_execute(node_id: NodeId) -> Result<(), LeaseError>`

**`crdt.rs` — `trait CRDT`**
- `op_based_merge(state: CRDTState, op: CRDTOp) -> Result<CRDTState, CRDTError>`
- `state_based_merge(state_a: CRDTState, state_b: CRDTState) -> Result<CRDTState, CRDTError>`
- `delta_crdt_apply(state: CRDTState, delta: CRDTDelta) -> Result<CRDTState, CRDTError>`
- `convergence_check(replicas: Vec<CRDTState>) -> Result<bool, CRDTError>`

**`erasure.rs` — `trait Erasure`**
- `reed_solomon_encode(data: Vec<u8>, k: u32, n: u32) -> Result<Vec<Vec<u8>>, ErasureError>`
- `reed_solomon_decode(fragments: Vec<Vec<u8>>, k: u32) -> Result<Vec<u8>, ErasureError>`
- `regenerating_repair(failed_node: NodeId, helpers: Vec<NodeId>) -> Result<Vec<u8>, ErasureError>`
- `locally_recoverable_repair(locality_group: Vec<NodeId>) -> Result<Vec<u8>, ErasureError>`

**`saga.rs` — `trait Saga`**
- `choreography_saga(steps: Vec<LocalTransaction>) -> Result<SagaId, SagaError>`
- `orchestration_saga(orchestrator: NodeId, steps: Vec<LocalTransaction>) -> Result<SagaId, SagaError>`
- `compensate_transaction(saga_id: SagaId, step: StepId) -> Result<(), SagaError>`
- `idempotency_check(key: IdempotencyKey) -> Result<bool, SagaError>`

**`coordinator.rs` — `trait Coordinator`**
- `domain_leader_elect(domain: OrderingDomain) -> Result<NodeId, CoordinatorError>`
- `fencing_epoch_issue(domain: OrderingDomain) -> Result<Epoch, CoordinatorError>`
- `stonith_coordinate(victim: NodeId) -> Result<(), CoordinatorError>`
- `cross_domain_transaction(domains: Vec<OrderingDomain>, ops: Vec<Operation>) -> Result<TransactionId, CoordinatorError>`

**`durability.rs` — `trait Durability`**
- `ack_condition(op_state: OpState, replicas: u32, required: u32) -> DurabilityCondition`
- `overstatement_rejected(claimed: DurabilityCondition, actual: DurabilityCondition) -> bool`

**`topology.rs` — `trait Topology`**
- `failure_domain_check(replicas: Vec<NodeId>) -> Result<bool, TopologyError>`
- `power_domain_distinct(a: NodeId, b: NodeId) -> bool`
- `facility_distinct(a: NodeId, b: NodeId) -> bool`
- `operator_distinct(a: NodeId, b: NodeId) -> bool`

**`integration.rs` — `trait Integration`**
- `bind_subsystems() -> Result<(), IntegrationError>`
- `cross_crate_type_check() -> Result<(), IntegrationError>`

**`protocol.rs` — `trait Protocol`**
- `protocol_version() -> ProtocolVersion`
- `edge_case_bits() -> EdgeCaseBits`

**`wire.rs` — `trait Wire`**
- `native_frame_format() -> WireFormat`
- `opaque_endpoint() -> Endpoint`

**`framing.rs` — `trait Framing`**
- `frame_packet(data: Vec<u8>) -> Result<Vec<u8>, FramingError>`
- `deframe_packet(bytes: Vec<u8>) -> Result<Vec<u8>, FramingError>`

**`sequencing.rs` — `trait Sequencing`**
- `sequence_packet(packet_id: u64) -> Result<(), SequencingError>`
- `ack_packet(sequence_id: u64) -> Result<(), SequencingError>`

**`acks.rs` — `trait Acks`**
- `track_ack(packet_id: u64) -> Result<(), AckError>`
- `ack_state(packet_id: u64) -> AckState`

---

## 6. Protocol Invariants (`src/invariants/`)

Each invariant is a sealed trait + compile-time check. Downstream crates reference them as bounds but cannot weaken them. These are drawn from IDS.md, IDS-A.md, IDS-B.md, edge-case position/response docs.

### Invariant specifications (40 files)

**`binary_completion.rs`** — 100% done or failed; no partial success terminal state (IDS-EDGE-CASES-DESIGN-POSITIONS §2)
```rust
pub trait BinaryCompletion: Sealed {
    fn is_fully_done(&self, op_state: OpState) -> bool;
    fn reject_partial(&self, progress: u64, total: u64) -> Result<(), InvariantError>;
}
```

**`dead_node_identity.rs`** — dead node never returns with old identity; fresh ID assigned (IDS-EDGE-CASES-DESIGN-POSITIONS §1)
```rust
pub trait DeadNodeIdentity: Sealed {
    fn retire_identity(&self, node_id: NodeId) -> Result<(), InvariantError>;
    fn assign_fresh_identity(&self, returning_node: NodeId) -> Result<NodeId, InvariantError>;
    fn is_old_id_retired(&self, node_id: NodeId) -> bool;
}
```

**`idle_rotation.rs`** — RNI rotates only while node idle; active requests keep stable ID (IDS-B §5, EDGE-CASES §2)
```rust
pub trait IdleRotation: Sealed {
    fn can_rotate(&self, node_id: NodeId, node_state: NodeState) -> bool;
    fn rotation_transition_window(&self) -> Duration;
    fn active_request_protects(&self, node_id: NodeId) -> bool;
}
```

**`no_global_master.rs`** — dataset-scoped ordering only; no permanent global ordering master (IDS-A §40-41)
```rust
pub trait NoGlobalMaster: Sealed {
    fn is_dataset_scoped(&self, domain: OrderingDomain) -> bool;
    fn rejects_global_sequence(&self) -> bool;
    fn domain_independent(&self, domain_a: OrderingDomain, domain_b: OrderingDomain) -> bool;
}
```

**`out_eq_in.rs`** — byte accounting boundary: bytes-out == bytes-in+acked (IDS-EDGE-CASES-RESPONSES #3)
```rust
pub trait OutEqIn: Sealed {
    fn bytes_out(&self, endpoint: Endpoint) -> u64;
    fn bytes_in_acked(&self, endpoint: Endpoint) -> u64;
    fn verify_boundary(&self, endpoint: Endpoint) -> Result<(), InvariantError>;
}
```

**`coordinated_deletion.rs`** — all holders delete simultaneously within milliseconds or not at all (IDS edge-case position #6)
```rust
pub trait CoordinatedDeletion: Sealed {
    fn deletion_broadcast(&self, dataset_id: DatasetId, holders: Vec<NodeId>) -> Result<DeletionId, DeletionError>;
    fn millisecond_sync(&self, holders: Vec<NodeId>) -> Result<(), DeletionError>;
    fn no_staggered_retention(&self, dataset_id: DatasetId) -> bool;
}
```

**`fencing_stonith.rs`** — STONITH/fencing before any failover; split-brain prevention (DOMAINS Layer 14, 16)
```rust
pub trait FencingStonith: Sealed {
    fn fence_before_failover(&self, victim: NodeId) -> Result<(), FencingError>;
    fn stonith_execute(&self, node_id: NodeId) -> Result<(), FencingError>;
    fn split_brain_prevented(&self, domain: OrderingDomain) -> bool;
}
```

**`quorum_intersection.rs`** — read quorum ∩ write quorum ≠ ∅ (CAPABILITIES quorum section)
```rust
pub trait QuorumIntersection: Sealed {
    fn read_set(&self, replicas: &[NodeId], r: u32) -> Vec<NodeId>;
    fn write_set(&self, replicas: &[NodeId], w: u32) -> Vec<NodeId>;
    fn intersection_non_empty(&self, read: &[NodeId], write: &[NodeId]) -> bool;
}
```

**`erasure_threshold.rs`** — any k of n chunks reconstructs; tolerates n-k failures (IDS-A §37)
```rust
pub trait ErasureThreshold: Sealed {
    fn coding_params(&self, dataset_id: DatasetId) -> (u32, u32); // (k, n)
    fn can_reconstruct(&self, available: u32, k: u32) -> bool;
    fn failure_tolerance(&self, n: u32, k: u32) -> u32; // n - k
}
```

**`correlated_failure.rs`** — replicas never share power/facility/network/operator domain (IDS-EDGE-CASES-RESPONSES #7)
```rust
pub trait CorrelatedFailure: Sealed {
    fn failure_domain_check(&self, replicas: Vec<NodeId>) -> Result<bool, InvariantError>;
    fn power_domain_distinct(&self, a: NodeId, b: NodeId) -> bool;
    fn facility_distinct(&self, a: NodeId, b: NodeId) -> bool;
    fn operator_distinct(&self, a: NodeId, b: NodeId) -> bool;
}
```

**`hunit_compensation.rs`** — H-unit failure compensation through another H-unit (IDS-A §36)
```rust
pub trait HUnitCompensation: Sealed {
    fn identify_affected_data(&self, hunit: HUnitId, failed_node: NodeId) -> Result<Vec<PartitionId>, RecoveryError>;
    fn select_compensation_hunit(&self, failed_hunit: HUnitId, capacity: u64) -> Result<HUnitId, RecoveryError>;
    fn restore_redundancy(&self, source: HUnitId, destination: HUnitId) -> Result<RecoveryPlan, RecoveryError>;
}
```

**`regional_autonomy.rs`** — regional clusters operate independently for local operations (IDS §9)
```rust
pub trait RegionalAutonomy: Sealed {
    fn local_operation_allowed(&self, region: RegionId, dataset: DatasetId) -> bool;
    fn cross_region_required(&self, op: Operation) -> bool;
    fn region_isolated_during_outage(&self, region: RegionId, other: RegionId) -> bool;
}
```

**`hybrid_redundancy.rs`** — full replication + erasure coding coexist per dataset (IDS-A §37)
```rust
pub trait HybridRedundancy: Sealed {
    fn redundancy_mode(&self, dataset_id: DatasetId) -> RedundancyMode;
    fn full_replica_count(&self, dataset_id: DatasetId) -> u32;
    fn erasure_coded_params(&self, dataset_id: DatasetId) -> (u32, u32);
    fn mode_switch_safe(&self, dataset_id: DatasetId, from: RedundancyMode, to: RedundancyMode) -> bool;
}
```

**`operation_lifecycle.rs`** — received→ordered→executing→committed→synchronized→verified→failed (IDS-A §42)
```rust
pub trait OperationLifecycle: Sealed {
    fn state_transition(&self, current: OpState, next: OpState) -> Result<(), LifecycleError>;
    fn is_terminal(&self, state: OpState) -> bool;
    fn commit_state(&self, op_state: OpState) -> CommitState;
}
```

**`durability_ack.rs`** — durability-aware acknowledgement; no overstating uncompleted replication (IDS-A §42)
```rust
pub trait DurabilityAck: Sealed {
    fn ack_condition(&self, op_state: OpState, replicas: u32, required: u32) -> DurabilityCondition;
    fn overstatement_rejected(&self, claimed: DurabilityCondition, actual: DurabilityCondition) -> bool;
}
```

**`ordered_read_visibility.rs`** — reads respect committed position prefix (IDS-A §44)
```rust
pub trait OrderedReadVisibility: Sealed {
    fn committed_position(&self, domain: OrderingDomain) -> OrderingPos;
    fn read_visible(&self, position: OrderingPos, committed: OrderingPos) -> bool;
    fn gap_rejected(&self, position: OrderingPos, committed: OrderingPos) -> bool;
}
```

**`node_roster.rs`** — client receives fixed node roster at join; stable (IDS-EDGE-CASES-DESIGN-POSITIONS §3)
```rust
pub trait NodeRoster: Sealed {
    fn roster_assign(&self, client_id: NodeId, nodes: Vec<NodeId>) -> Result<(), RosterError>;
    fn roster_stable(&self, client_id: NodeId) -> bool;
    fn roster_update_formal(&self, client_id: NodeId, change: RosterChange) -> Result<(), RosterError>;
}
```

**`region_affinity.rs`** — client does not abruptly change regions; near-to-far routing (IDS-EDGE-CASES-DESIGN-POSITIONS §5)
```rust
pub trait RegionAffinity: Sealed {
    fn client_region(&self, client_id: NodeId) -> RegionId;
    fn near_to_far(&self, client: NodeId, candidates: Vec<Endpoint>) -> Vec<Endpoint>;
    fn region_change_formal_only(&self, client_id: NodeId) -> bool;
}
```

**`identity_rotation.rs`** — stable internal identity + rotating RNI; mapping protected (IDS-B §3-5)
```rust
pub trait IdentityRotation: Sealed {
    fn stable_id(&self, rni: RotatingNodeId) -> Option<NodeId>;
    fn rni_to_stable(&self, rni: RotatingNodeId) -> Result<NodeId, RotationError>;
    fn mapping_protected(&self) -> bool;
    fn transition_window(&self, old_rni: RotatingNodeId, new_rni: RotatingNodeId) -> Duration;
}
```

**`ids_standard_time.rs`** — proprietary time basis for ordering/commit, not wall-clock (IDS-EDGE-CASES-RESPONSES #2)
```rust
pub trait IdsStandardTime: Sealed {
    fn now(&self) -> IdsTime;
    fn drift_tolerance(&self) -> Duration;
    fn sync_across_regions(&self, regions: Vec<RegionId>) -> Result<(), TimeError>;
    fn ordering_timestamp(&self, op: OpRef) -> IdsTime;
}
```

**`daily_integrity.rs`** — daily integrity checks; "fishy" state detection (IDS-EDGE-CASES-RESPONSES #3)
```rust
pub trait DailyIntegrity: Sealed {
    fn out_eq_in_check(&self) -> Result<(), IntegrityError>;
    fn fishy_state_detect(&self, node_id: NodeId) -> Result<Anomaly, IntegrityError>;
    fn quarantine_suspect(&self, node_id: NodeId) -> Result<(), IntegrityError>;
}
```

**`db_retirement.rs`** — temporary DB retirement during RNI rotation; returns online after window (IDS-EDGE-CASES-RESPONSES #4)
```rust
pub trait DbRetirement: Sealed {
    fn retire_temporarily(&self, db_id: NodeId, window: Duration) -> Result<(), RetirementError>;
    fn return_online(&self, db_id: NodeId) -> Result<(), RetirementError>;
    fn window_closed(&self, db_id: NodeId) -> bool;
}
```

**`read_fallback.rs`** — automatic fallback to next closest eligible replica (IDS-EDGE-CASES-RESPONSES #5)
```rust
pub trait ReadFallback: Sealed {
    fn fallback_target(&self, primary: Endpoint, candidates: Vec<Endpoint>, profile: ConsistencyProfile) -> Result<Endpoint, FallbackError>;
    fn closest_eligible(&self, candidates: Vec<Endpoint>) -> Result<Endpoint, FallbackError>;
    fn strict_consistency_forbid_fallback(&self, profile: ConsistencyProfile) -> bool;
}
```

**`topology_diversity.rs`** — different locations, H-units, regions, failure domains per copy (IDS-EDGE-CASES-RESPONSES #7)
```rust
pub trait TopologyDiversity: Sealed {
    fn placement_diverse(&self, replicas: Vec<NodeId>) -> Result<bool, DiversityError>;
    fn failure_domain_distinct(&self, replicas: Vec<NodeId>) -> Result<bool, DiversityError>;
    fn correlated_risk_reduced(&self, replicas: Vec<NodeId>) -> f64;
}
```

**`write_fanout.rs`** — ~30 databases written simultaneously for durability (IDS-EDGE-CASES-RESPONSES #4)
```rust
pub trait WriteFanout: Sealed {
    fn fanout_count(&self, dataset_id: DatasetId) -> u32;
    fn simultaneous_write(&self, targets: Vec<NodeId>, data: Vec<u8>) -> Result<Vec<CommitState>, FanoutError>;
    fn binary_completion_check(&self, results: Vec<CommitState>) -> bool; // all 100% or failed
}
```

**`custody_graph.rs`** — data custody and responsibility graph (DIFFERENCES §11)
```rust
pub trait CustodyGraph: Sealed {
    fn record_responsibility(&self, asset: DatasetId, version: Version, domain: RegionId) -> Result<(), GraphError>;
    fn custody_transfer(&self, from: RegionId, to: RegionId, asset: DatasetId) -> Result<(), GraphError>;
    fn authority_epoch(&self, asset: DatasetId) -> Epoch;
    fn fencing_epoch(&self, asset: DatasetId) -> Epoch;
}
```

**`checksum_boundary.rs`** — end-to-end checksum verification boundary (CAPABILITIES §49)
```rust
pub trait ChecksumBoundary: Sealed {
    fn verify_checksum(&self, data: &[u8], expected: Checksum) -> Result<(), IntegrityError>;
    fn boundary_breach(&self, endpoint: Endpoint) -> Result<(), IntegrityError>;
}
```

**`replica_freshness.rs`** — replica freshness validation for reads (IDS-A §7.2)
```rust
pub trait ReplicaFreshness: Sealed {
    fn is_fresh(&self, replica: NodeId, version: Version) -> bool;
    fn staleness_bound(&self, profile: ConsistencyProfile) -> Duration;
}
```

**`commit_prefix.rs`** — committed-prefix tracking for ordered reads (CAPABILITIES §64)
```rust
pub trait CommitPrefix: Sealed {
    fn highest_contiguous(&self, domain: OrderingDomain) -> OrderingPos;
    fn gap_detected(&self, position: OrderingPos) -> bool;
}
```

**`version_vector.rs`** — vector clock for causality tracking (DOMAINS Layer 12)
```rust
pub trait VersionVector: Sealed {
    fn compare(&self, v1: Version, v2: Version) -> Ordering;
    fn merge(&self, v1: Version, v2: Version) -> Version;
}
```

**`membership_epoch.rs`** — membership change epoch tracking (CAPABILITIES §85)
```rust
pub trait MembershipEpoch: Sealed {
    fn current_epoch(&self) -> Epoch;
    fn epoch_transition(&self, new_epoch: Epoch) -> Result<(), MembershipError>;
}
```

**`placement_policy.rs`** — placement policy enforcement (IDS §5)
```rust
pub trait PlacementPolicy: Sealed {
    fn enforce(&self, decision: PlacementDecision) -> Result<(), PlacementError>;
    fn validate_constraints(&self, policy: &PlacementPolicy) -> Result<(), PlacementError>;
}
```

**`replication_lag.rs`** — replication lag observability (CAPABILITIES §45)
```rust
pub trait ReplicationLag: Sealed {
    fn measure_lag(&self, source: NodeId, replica: NodeId) -> Duration;
    fn lag_exceeds_threshold(&self, threshold: Duration) -> bool;
}
```

**`recovery_scope.rs`** — multi-level recovery scope classification (IDS-A §45)
```rust
pub trait RecoveryScope: Sealed {
    fn classify(&self, failure: FailureEvent) -> RecoveryScopeLevel;
    fn select_resources(&self, scope: RecoveryScopeLevel) -> Vec<NodeId>;
}
```

**`data_lifecycle.rs`** — data lifecycle state enforcement (IDS §16)
```rust
pub trait DataLifecycle: Sealed {
    fn lifecycle_state(&self, dataset: DatasetId) -> LifecycleState;
    fn transition_allowed(&self, from: LifecycleState, to: LifecycleState) -> bool;
}
```

**`snapshot_consistency.rs`** — snapshot consistency verification (CAPABILITIES §110-111)
```rust
pub trait SnapshotConsistency: Sealed {
    fn verify_consistency(&self, snapshot: SnapshotId) -> Result<(), SnapshotError>;
    fn point_in_time_valid(&self, timestamp: IdsTime) -> bool;
}
```

**`compute_to_data.rs`** — compute-to-data execution boundary (CAPABILITIES §119)
```rust
pub trait ComputeToData: Sealed {
    fn execution_allowed(&self, compute: ComputeRequest, data: DatasetId) -> bool;
    fn result_compaction(&self, result: ComputeResult) -> Result<CompactResult, ComputeError>;
}
```

**`hot_cold_tier.rs`** — hot/cold storage tier transition rules (CAPABILITIES §104)
```rust
pub trait HotColdTier: Sealed {
    fn tier_transition(&self, dataset: DatasetId, from: Tier, to: Tier) -> Result<(), TierError>;
    fn identity_preserved(&self, dataset: DatasetId) -> bool;
}
```

**`mobility_handoff.rs`** — dataset mobility across regions handoff (CAPABILITIES §116)
```rust
pub trait MobilityHandoff: Sealed {
    fn controlled_handoff(&self, dataset: DatasetId, from: RegionId, to: RegionId) -> Result<(), MobilityError>;
    fn identity_preserved(&self, dataset: DatasetId) -> bool;
}
```

**`federation_trust.rs`** — multi-owner federation trust boundary (CAPABILITIES §101)
```rust
pub trait FederationTrust: Sealed {
    fn trust_established(&self, domain_a: RegionId, domain_b: RegionId) -> bool;
    fn credential_valid(&self, credential: Credential) -> bool;
}
```

---

## 7. Versioning (`src/versioning.rs`)

Protocol version + edge-case version bits as a compile-time bitfield:

```rust
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

pub struct EdgeCaseBits(u128);

impl EdgeCaseBits {
    pub const BINARY_COMPLETION: u128 = 1 << 0;
    pub const DEAD_NODE_IDENTITY: u128 = 1 << 1;
    pub const IDLE_ROTATION: u128 = 1 << 2;
    pub const NO_GLOBAL_MASTER: u128 = 1 << 3;
    pub const OUT_EQ_IN: u128 = 1 << 4;
    pub const COORDINATED_DELETION: u128 = 1 << 5;
    pub const FENCING_STONITH: u128 = 1 << 6;
    pub const QUORUM_INTERSECTION: u128 = 1 << 7;
    pub const ERASURE_THRESHOLD: u128 = 1 << 8;
    pub const CORRELATED_FAILURE: u128 = 1 << 9;
    pub const HUNIT_COMPENSATION: u128 = 1 << 10;
    pub const REGIONAL_AUTONOMY: u128 = 1 << 11;
    pub const HYBRID_REDUNDANCY: u128 = 1 << 12;
    pub const OPERATION_LIFECYCLE: u128 = 1 << 13;
    pub const DURABILITY_ACK: u128 = 1 << 14;
    pub const ORDERED_READ_VISIBILITY: u128 = 1 << 15;
    pub const NODE_ROSTER: u128 = 1 << 16;
    pub const REGION_AFFINITY: u128 = 1 << 17;
    pub const IDENTITY_ROTATION: u128 = 1 << 18;
    pub const IDS_STANDARD_TIME: u128 = 1 << 19;
    pub const DAILY_INTEGRITY: u128 = 1 << 20;
    pub const DB_RETIREMENT: u128 = 1 << 21;
    pub const READ_FALLBACK: u128 = 1 << 22;
    pub const TOPOLOGY_DIVERSITY: u128 = 1 << 23;
    pub const WRITE_FANOUT: u128 = 1 << 24;
    pub const CUSTODY_GRAPH: u128 = 1 << 25;
    pub const CHECKSUM_BOUNDARY: u128 = 1 << 26;
    pub const REPLICA_FRESHNESS: u128 = 1 << 27;
    pub const COMMIT_PREFIX: u128 = 1 << 28;
    pub const VERSION_VECTOR: u128 = 1 << 29;
    pub const MEMBERSHIP_EPOCH: u128 = 1 << 30;
    pub const PLACEMENT_POLICY: u128 = 1 << 31;
    // bits 32-127 available for future edge cases via compile-time cfg

    pub fn has(&self, bit: u128) -> bool { (self.0 & bit) != 0 }
    pub fn enable(&mut self, bit: u128) { self.0 |= bit; }
}
```

Downstream crates check `cfg!(feature = "edge_case_N")` or `ids_000_aa::EdgeCaseBits::has(N)` at compile time.

---

## 8. Error Contracts (`src/errors.rs`)

Sealed error types via `thiserror`. Each downstream crate wraps `ids_000_aa::Error` variants:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("carrier {carrier:?} not available")]
    CarrierUnavailable { carrier: CarrierType },
    #[error("node {node:?} not found in membership")]
    NodeNotFound { node: NodeId },
    #[error("quorum not satisfied: need {need}, have {have}")]
    QuorumNotSatisfied { need: u32, have: u32 },
    #[error("invariant violated: {invariant}")]
    InvariantViolated { invariant: &'static str },
    #[error("binary completion required: operation {op:?} not 100% done")]
    BinaryCompletionRequired { op: OpRef },
    #[error("dead node identity retired: {node:?}")]
    DeadNodeIdentityRetired { node: NodeId },
    #[error("idle rotation violation: node {node:?} is active")]
    IdleRotationViolation { node: NodeId },
    #[error("global master rejected: domain {domain:?}")]
    GlobalMasterRejected { domain: OrderingDomain },
    #[error("byte accounting mismatch: out {out}, in {in_}")]
    ByteAccountingMismatch { out: u64, in_: u64 },
    #[error("coordinated deletion incomplete: holders {holders:?}")]
    CoordinatedDeletionIncomplete { holders: Vec<NodeId> },
    #[error("fencing/STONITH required for {node:?}")]
    FencingRequired { node: NodeId },
    #[error("split-brain detected in domain {domain:?}")]
    SplitBrainDetected { domain: OrderingDomain },
    #[error("erasure threshold not met: available {available}, need {k}")]
    ErasureThresholdNotMet { available: u32, k: u32 },
    #[error("correlated failure domain: replicas share {domain:?}")]
    CorrelatedFailureDomain { domain: FailureDomain },
    #[error("custody graph error: {message}")]
    CustodyGraphError { message: String },
    // Additional error variants for each invariant and contract
}
```

---

## 9. Build Order (one file at a time)

Per the one-file-at-a-time git workflow:

1. `src/lib.rs` — crate root
2. `src/types.rs` — canonical types (foundation)
3. `src/contracts/mod.rs` + all 30 contract files
4. `src/invariants/mod.rs` + all 40 invariant files
5. `src/internals/mod.rs` + 20 internal files
6. `src/codec/mod.rs` + 15 codec files
7. `src/state/mod.rs` + 20 state files
8. `src/validation/mod.rs` + 15 validation files
9. `src/recovery/mod.rs` + 15 recovery files
10. `src/placement/mod.rs` + 15 placement files
11. `src/routing/mod.rs` + 15 routing files
12. `src/security/mod.rs` + 15 security files
13. `src/time/mod.rs` + 10 time files
14. `src/identity/mod.rs` + 10 identity files
15. `src/quorum/mod.rs` + 10 quorum files
16. `src/broadcast/mod.rs` + 10 broadcast files
17. `src/crdt/mod.rs` + 10 crdt files
18. `src/erasure/mod.rs` + 10 erasure files
19. `src/snapshot/mod.rs` + 10 snapshot files
20. `src/fault/mod.rs` + 10 fault files
21. `src/byzantine/mod.rs` + 10 byzantine files
22. `src/saga/mod.rs` + 10 saga files
23. `src/lease/mod.rs` + 10 lease files
24. `src/chaos/mod.rs` + 10 chaos files
25. `src/observability/mod.rs` + 10 observability files
26. `src/versioning.rs`
27. `src/errors.rs`
28. `src/prelude.rs`

Then downstream crates (one at a time): `ids-carrier`, `ids-crypto`, `ids-identity`, `ids-time`, `ids-topology`, etc.

---

## 10. Acceptance Criteria

- [ ] `ids-000-aa` compiles as a standalone crate with no `std::net`, `tokio::net`, or libp2p dependencies
- [ ] Every canonical type has exactly one definition in `types.rs`
- [ ] Every contract trait is sealed (downstream cannot impl)
- [ ] Every invariant is enforced at compile time via trait bounds
- [ ] `pub use` in `lib.rs` re-exports all types/contracts/invariants/errors at crate root
- [ ] Edge case registry has versioned bits for all invariants
- [ ] Adding a new edge case requires editing exactly one file in `ids-000-aa` + implementing in one downstream crate
- [ ] `ids-crypto` is the only crate not depending on `ids-000-aa`
- [ ] `Endpoint` uses `CarrierType` + `Vec<u8>` only — no IP, no port, no `SocketAddr`
- [ ] File structure shows 335+ files inside `ids-000-aa/src/`

---

## 11. Git Workflow (one file at a time)

```
git add ids-000-aa/src/types.rs
git commit -m "ids-000-aa: types.rs — add canonical NodeId/RegionId/Endpoint types — master type definitions"
git push origin main
git log --oneline -1
git status   # must be clean
```

Then next file. Never batch. Never skip push. Never skip verification.

---

---


Read `ids-000-aa.md` in the repository root completely. Read only the additional files necessary to implement ids-000-aa. Create any missing directories. Report the ordered implementation file list and briefly state the implementation approach. Then immediately begin implementing Phase 36, following every rule in `rulebook.md`.


1. **One file at a time:** write → run `count_loc.sh` → verify 2400/1600/800 LOC targets → `git add -A` → `git commit -m "ids-000-aa: component — what changed — why"` → `git push origin main` → verify `git log --oneline -1` and `git status` show clean state → only then next file. Never batch files. Never skip push. Never skip verification.

2. **File rewrites:** When rewriting an existing file, always `rm` it first, then `write_file` from scratch. Never use `patch` on files with escaped Rust string literals. Use `patch` only for small, simple targeted edits on clean files.

3. **LOC verification:** Run `C:\IDS\count_loc.sh <file>` after writing every substantive file and before committing it. If targets aren't met, patch in more production code (old_string/new_string). Never estimate LOC manually or claim compliance without running the script.

4. **Stop on errors:** If any write, lint, commit, or push fails, stop and report it. Do not continue until resolved.

5. **Execute autonomously:** Do not wait for prompts between files. After each file is committed, pushed, and verified clean, immediately proceed to the next file in the ordered list.

Continue until Phase 36 is fully implemented, tested, committed, pushed, and verified, or a genuine blocker prevents further progress. Do not stop at planning. Do not skip files. Do not claim unverified results.

**End of PLAN.md for ids-000-aa.** This crate only — the central hub father crate. Edit here, ripple downstream, never touch thousands of files for a single contract change.
