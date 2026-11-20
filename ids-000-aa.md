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

**Target scale:** 100+ to 400+ source files inside `ids-000-aa/src/` alone (contracts, invariants, internals), plus all downstream `ids-*` crates.

**Carrier-agnostic absolute rule (no exceptions):** No `std::net`, `SocketAddr`, `IpAddr`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh anywhere. `ids-000-aa` defines only opaque `Endpoint` (`CarrierType` + `Vec<u8>`).

---

## 2. Architecture Overview

```
IDS WORKSPACE — CENTRAL HUB MODEL
==================================

                           ids-000-aa  (CENTRAL HUB / FATHER CRATE)
                           ┌─────────────────────────────────┐
                           │  src/types.rs                   │
                           │  src/contracts/                 │
                           │  src/invariants/                │
                           │  src/versioning.rs              │
                           │  src/errors.rs                  │
                           │  src/prelude.rs                 │
                           │  src/internals/                 │
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

## 3. File Structure — ids-000-aa (100+ to 400+ files, actual code not stubs)

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
    ├── contracts/                                # Sealed trait contracts — control plane interface
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
    │   └── coordinator.rs        # trait Coordinator — domain-scoped leadership, STONITH
    │
    ├── invariants/                               # Protocol invariants — sealed, enforceable at compile time
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
    │   ├── edge_case_registry.rs     # versioned bitfield of every edge case handled
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
    │   └── custody_graph.rs          # data custody and responsibility graph (DIFFERENCES §11)
    │
    └── internals/                              # Master crate internal implementation (not re-exported downstream)
        ├── mod.rs
        ├── type_registry.rs        # Runtime type ID registry (NodeId, RegionId, etc. → type IDs)
        ├── version_graph.rs        # Version compatibility graph (protocol versions ↔ edge-case bits)
        ├── contract_validator.rs   # Compile-time contract validation (sealed trait bounds)
        ├── invariant_checker.rs    # Invariant checking at build time (all invariants satisfied)
        └── edge_case_compiler.rs   # Compiles edge_case_registry bits into cfg flags for downstream
```

**File count inside `ids-000-aa/src/`:**
- Root: 5 files (`lib.rs`, `types.rs`, `versioning.rs`, `errors.rs`, `prelude.rs`)
- `contracts/`: 25 files (one per sealed trait)
- `invariants/`: 29 files (core invariants + edge case registry + domain-specific invariants drawn from IDS docs)
- `internals/`: 6 files
- **Total inside master crate: ~65+ files**, scaling toward 100+ with additional contract/invariant files as edge cases are added

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
```

---

## 5. Sealed Trait Contracts (`src/contracts/`)

Every contract is sealed: `pub trait X: private::Sealed {}`. Downstream crates can **call** methods and use as trait bounds, but cannot implement. Only `ids-000-aa` and its designated implementation crates can `impl`. Adding a method to any contract here is non-breaking for downstream — zero edits to thousands of files.

Each contract file contains:
- The sealed trait declaration
- The `private::Sealed` supertrait module
- Method signatures with full `Result<T, Error>` error contracts
- Doc comments explaining the invariant each method enforces

### Contract specifications (drawn from IDS docs)

**`carrier.rs` — `trait Carrier`**
- `fn send_datagram(&self, endpoint: &Endpoint, data: &[u8]) -> Result<(), CarrierError>`
- `fn receive_datagram(&self) -> Result<(Endpoint, Vec<u8>), CarrierError>`
- `fn carrier_type(&self) -> CarrierType`
- `fn endpoint_address_size_limit(&self) -> usize` (default 1024 bytes, from IDS edge-case hardening)

**`transport.rs` — `trait Transport`**
- `fn frame(&self, payload: Vec<u8>) -> Result<Vec<u8>, TransportError>`
- `fn deframe(&self, bytes: Vec<u8>) -> Result<Vec<u8>, TransportError>`
- `fn sequence(&self, frame_id: u64) -> Result<(), TransportError>`
- `fn acknowledge(&self, sequence_id: u64) -> Result<(), TransportError>`
- `fn delivery_semantics(&self) -> DeliverySemantics` (at-least-once, at-most-once, exactly-once)

**`store.rs` — `trait Store`**
- `fn put(&self, key: &[u8], value: Vec<u8>, durability: DurabilityCondition) -> Result<CommitState, StoreError>`
- `fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>`
- `fn delete(&self, key: &[u8]) -> Result<CommitState, StoreError>`
- `fn durability_condition(&self) -> DurabilityCondition`

**`session.rs` — `trait Session`** (THE ONE CALL binding)
- `fn ids_submit(&self, op: Operation) -> Result<OpRef, SessionError>`
- `fn session_create(&self, client_id: NodeId) -> Result<SessionId, SessionError>`
- `fn session_expire(&self, session_id: SessionId) -> Result<(), SessionError>`
- `fn session_heartbeat(&self, session_id: SessionId) -> Result<(), SessionError>`

**`routing.rs` — `trait Routing`**
- `fn resolve_logical(&self, dataset_id: &DatasetId, record_id: &RecordId) -> Result<Endpoint, RoutingError>`
- `fn locality_aware_select(&self, candidates: Vec<Endpoint>, profile: ConsistencyProfile) -> Result<Endpoint, RoutingError>`
- `fn replica_aware_fallback(&self, primary: Endpoint, profile: ConsistencyProfile) -> Result<Endpoint, RoutingError>`

**`discovery.rs` — `trait Discovery`**
- `fn discover_node(&self, hint: Option<&Endpoint>) -> Result<Vec<NodeId>, DiscoveryError>`
- `fn discover_hunit(&self, hunit_id: &HUnitId) -> Result<Vec<NodeId>, DiscoveryError>`
- `fn discover_region(&self, region_id: &RegionId) -> Result<Vec<HUnitId>, DiscoveryError>`
- `fn gossip_propagate(&self, announcement: Announcement) -> Result<(), DiscoveryError>`

**`membership.rs` — `trait Membership`**
- `fn join(&self, node_id: NodeId, region: RegionId, hunit: HUnitId, capacity: Capacity) -> Result<(), MembershipError>`
- `fn leave(&self, node_id: NodeId) -> Result<(), MembershipError>`
- `fn lifecycle_transition(&self, node_id: NodeId, new_state: NodeState) -> Result<(), MembershipError>`
- `fn capacity_advertise(&self, node_id: NodeId, capacity: Capacity) -> Result<(), MembershipError>`

**`placement.rs` — `trait Placement`**
- `fn place_dataset(&self, dataset_id: &DatasetId, policy: PlacementPolicy) -> Result<PlacementDecision, PlacementError>`
- `fn place_partition(&self, partition_id: &PartitionId, policy: PlacementPolicy) -> Result<PlacementDecision, PlacementError>`
- `fn capacity_aware_select(&self, candidates: Vec<NodeId>, required: u64) -> Result<Vec<NodeId>, PlacementError>`
- `fn failure_domain_aware(&self, replicas: Vec<NodeId>) -> Result<bool, PlacementError>` (correlated failure check)

**`replication.rs` — `trait Replication`**
- `fn replicate_stream(&self, dataset_id: &DatasetId, changes: ChangeStream) -> Result<(), ReplicationError>`
- `fn checkpoint(&self, dataset_id: &DatasetId, version: Version) -> Result<CheckpointId, ReplicationError>`
- `fn retry_sync(&self, checkpoint: CheckpointId) -> Result<(), ReplicationError>` (bounded backoff, idempotent)

**`recovery.rs` — `trait Recovery`**
- `fn compensate_node_failure(&self, node: NodeId) -> Result<RecoveryPlan, RecoveryError>`
- `fn compensate_hunit_failure(&self, hunit: HUnitId) -> Result<RecoveryPlan, RecoveryError>`
- `fn compensate_regional_failure(&self, region: RegionId) -> Result<RecoveryPlan, RecoveryError>`
- `fn reconstruct_erasure(&self, fragments: Vec<FragmentId>) -> Result<Vec<u8>, RecoveryError>`
- `fn verify_recovery(&self, proof: RecoveryProof) -> Result<(), RecoveryError>`
- `fn restore_redundancy(&self, dataset_id: &DatasetId) -> Result<(), RecoveryError>`

**`consistency.rs` — `trait Consistency`**
- `fn profile(&self, dataset_id: &DatasetId) -> ConsistencyProfile`
- `fn committed_prefix(&self, domain: OrderingDomain) -> Result<OrderingPos, ConsistencyError>`
- `fn ordered_read_visibility(&self, position: OrderingPos) -> Result<bool, ConsistencyError>`
- `fn mvcc_snapshot(&self, dataset_id: &DatasetId) -> Result<SnapshotId, ConsistencyError>`

**`ordering.rs` — `trait Ordering`**
- `fn dataset_domain(&self, dataset_id: &DatasetId) -> OrderingDomain`
- `fn assign_position(&self, domain: &OrderingDomain, op_ref: OpRef) -> Result<OrderingPos, OrderingError>`
- `fn no_global_master(&self) -> bool` (always true — dataset-scoped only)
- `fn concurrent_write_resolve(&self, domain: &OrderingDomain, ops: Vec<OpRef>) -> Result<Vec<OpRef>, OrderingError>`

**`security.rs` — `trait Security`**
- `fn authenticate(&self, identity: NodeId, credential: Credential) -> Result<AuthToken, SecurityError>`
- `fn authorize(&self, token: AuthToken, dataset: DatasetId, op: OpType) -> Result<(), SecurityError>`
- `fn encrypt(&self, data: Vec<u8>, key_id: KeyId) -> Result<Vec<u8>, SecurityError>`
- `fn decrypt(&self, data: Vec<u8>, key_id: KeyId) -> Result<Vec<u8>, SecurityError>`
- `fn rotate_key(&self, key_id: KeyId) -> Result<KeyId, SecurityError>`
- `fn revoke_key(&self, key_id: KeyId) -> Result<(), SecurityError>`

**`client.rs` — `trait Client`** (THE ONE CALL)
- `fn ids_submit(&self, op: Operation) -> Result<OpResult, ClientError>`
- `fn op_reference(&self, op: &Operation) -> OpRef`
- `fn idempotency_key(&self, op: &Operation) -> IdempotencyKey`

**`fault.rs` — `trait FaultTolerance`**
- `fn circuit_breaker_state(&self, peer: NodeId) -> Result<CircuitState, FaultError>`
- `fn bulkhead_isolate(&self, domain: FailureDomain) -> Result<(), FaultError>`
- `fn backpressure_apply(&self, endpoint: Endpoint, credit: u64) -> Result<(), FaultError>`
- `fn graceful_degrade(&self, level: DegradationLevel) -> Result<(), FaultError>`

**`broadcast.rs` — `trait Broadcast`**
- `fn reliable_broadcast(&self, message: Message) -> Result<(), BroadcastError>`
- `fn causal_broadcast(&self, message: Message, deps: Vec<MessageId>) -> Result<(), BroadcastError>`
- `fn total_order_broadcast(&self, message: Message) -> Result<(), BroadcastError>`
- `fn atomic_broadcast(&self, message: Message) -> Result<(), BroadcastError>`

**`quorum.rs` — `trait Quorum`**
- `fn read_quorum(&self, replicas: Vec<NodeId>, r: u32) -> Result<Vec<NodeId>, QuorumError>`
- `fn write_quorum(&self, replicas: Vec<NodeId>, w: u32) -> Result<Vec<NodeId>, QuorumError>`
- `fn intersection_check(&self, read: &[NodeId], write: &[NodeId]) -> Result<bool, QuorumError>` (must be non-empty)

**`snapshot.rs` — `trait Snapshot`**
- `fn chandy_lamport_initiate(&self, initiator: NodeId) -> Result<SnapshotId, SnapshotError>`
- `fn marker_send(&self, channel: ChannelId) -> Result<(), SnapshotError>`
- `fn state_record(&self, node_id: NodeId, state: NodeState) -> Result<(), SnapshotError>`
- `fn channel_state_record(&self, channel: ChannelId, messages: Vec<Message>) -> Result<(), SnapshotError>`

**`byzantine.rs` — `trait Byzantine`**
- `fn bft_consensus(&self, proposal: Value, replicas: Vec<NodeId>) -> Result<Value, ByzantineError>`
- `fn authenticated_broadcast(&self, message: Message, signature: Signature) -> Result<(), ByzantineError>`
- `fn sybil_resistance(&self, identity: NodeId) -> Result<bool, ByzantineError>`
- `fn eclipse_resistance(&self, victim: NodeId) -> Result<bool, ByzantineError>`
- `fn replay_protection(&self, op_ref: OpRef) -> Result<bool, ByzantineError>`

**`lease.rs` — `trait Lease`**
- `fn grant_lease(&self, node_id: NodeId, epoch: Epoch, duration: Duration) -> Result<LeaseId, LeaseError>`
- `fn renew_lease(&self, lease_id: LeaseId) -> Result<(), LeaseError>`
- `fn fencing_token_increment(&self, lease_id: LeaseId) -> Result<FencingToken, LeaseError>`
- `fn fencing_token_check(&self, token: FencingToken) -> Result<bool, LeaseError>`
- `fn stonith_execute(&self, node_id: NodeId) -> Result<(), LeaseError>`

**`crdt.rs` — `trait CRDT`**
- `fn op_based_merge(&self, state: CRDTState, op: CRDTOp) -> Result<CRDTState, CRDTError>`
- `fn state_based_merge(&self, state_a: CRDTState, state_b: CRDTState) -> Result<CRDTState, CRDTError>`
- `fn delta_crdt_apply(&self, state: CRDTState, delta: CRDTDelta) -> Result<CRDTState, CRDTError>`
- `fn convergence_check(&self, replicas: Vec<CRDTState>) -> Result<bool, CRDTError>`

**`erasure.rs` — `trait Erasure`**
- `fn reed_solomon_encode(&self, data: Vec<u8>, k: u32, n: u32) -> Result<Vec<Vec<u8>>, ErasureError>`
- `fn reed_solomon_decode(&self, fragments: Vec<Vec<u8>>, k: u32) -> Result<Vec<u8>, ErasureError>`
- `fn regenerating_repair(&self, failed_node: NodeId, helpers: Vec<NodeId>) -> Result<Vec<u8>, ErasureError>`
- `fn locally_recoverable_repair(&self, locality_group: Vec<NodeId>) -> Result<Vec<u8>, ErasureError>`

**`saga.rs` — `trait Saga`**
- `fn choreography_saga(&self, steps: Vec<LocalTransaction>) -> Result<SagaId, SagaError>`
- `fn orchestration_saga(&self, orchestrator: NodeId, steps: Vec<LocalTransaction>) -> Result<SagaId, SagaError>`
- `fn compensate_transaction(&self, saga_id: SagaId, step: StepId) -> Result<(), SagaError>`
- `fn idempotency_check(&self, key: IdempotencyKey) -> Result<bool, SagaError>`

**`coordinator.rs` — `trait Coordinator`**
- `fn domain_leader_elect(&self, domain: OrderingDomain) -> Result<NodeId, CoordinatorError>`
- `fn fencing_epoch_issue(&self, domain: OrderingDomain) -> Result<Epoch, CoordinatorError>`
- `fn stonith_coordinate(&self, victim: NodeId) -> Result<(), CoordinatorError>`
- `fn cross_domain_transaction(&self, domains: Vec<OrderingDomain>, ops: Vec<Operation>) -> Result<TransactionId, CoordinatorError>`

---

## 6. Protocol Invariants (`src/invariants/`)

Each invariant is a sealed trait + compile-time check. Downstream crates reference them as bounds but cannot weaken them. These are drawn from IDS.md, IDS-A.md, IDS-B.md, edge-case position/response docs.

### Invariant specifications

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

**`out_eq_in.rs`** — byte accounting boundary: bytes-out == bytes-in+acked (IDS-EDGE-CASES-RESPONSES §3)
```rust
pub trait OutEqIn: Sealed {
    fn bytes_out(&self, endpoint: Endpoint) -> u64;
    fn bytes_in_acked(&self, endpoint: Endpoint) -> u64;
    fn verify_boundary(&self, endpoint: Endpoint) -> Result<(), InvariantError>;
}
```

**`coordinated_deletion.rs`** — all holders delete simultaneously within milliseconds or not at all (IDS edge-case position #6, IDS-EDGE-CASES-RESPONSES)
```rust
pub trait CoordinatedDeletion: Sealed {
    fn deletion_broadcast(&self, dataset_id: DatasetId, holders: Vec<NodeId>) -> Result<DeletionId, DeletionError>;
    fn millisecond_sync(&self, holders: Vec<NodeId>) -> Result<(), DeletionError>;
    fn no_staggered_retention(&self, dataset_id: DatasetId) -> bool;
}
```

**`fencing_stonith.rs`** — STONITH/fencing before any failover; split-brain prevention (DOMAINS Layer 14, 16; IDS-EDGE-CASES-RESPONSES)
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

**`edge_case_registry.rs`** — versioned bitfield of every edge case handled
```rust
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
    // bits 26-127 reserved for future edge cases (compile-time cfg)
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
    // bits 26-127 available for future edge cases via compile-time cfg
}

impl EdgeCaseBits {
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
3. `src/contracts/mod.rs` + all 25 contract files
4. `src/invariants/mod.rs` + all 29 invariant files
5. `src/versioning.rs`
6. `src/errors.rs`
7. `src/prelude.rs`
8. `src/internals/mod.rs` + 6 internal files

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
- [ ] All invariants drawn from IDS docs are represented as sealed traits

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

**End of PLAN.md for ids-000-aa.** This crate only — the central hub father crate. Edit here, ripple downstream, never touch thousands of files for a single contract change.
