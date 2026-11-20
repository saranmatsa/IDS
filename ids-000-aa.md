# PLAN.md — ids-000-aa Master Crate (Central Hub)

**Project:** IDS — Independent Distributed System  
**Crate:** `ids-000-aa`  
**Role:** Central hub — single control point. All canonical types, sealed trait contracts, invariants, versioning, and error contracts live here. Every downstream `ids-*` crate depends on `ids-000-aa`.  
**Location:** `C:\IDS\ids-000-aa\`  
**Status:** PLAN — research complete, no code written yet  
**Plan format:** Matches hx-241 `plan.md` structure (file tree, implementation order, dependencies, acceptance criteria, patterns table)

---

## 1. Executive Summary

`ids-000-aa` is the father crate and central hub of the IDS workspace. It holds every canonical type, every sealed trait contract, every protocol invariant, every error contract, and every version bit. All downstream crates import from `ids_000_aa::*`.

**The "40 million lines" problem is solved here:** adding one edge case = edit exactly **one file** in `ids-000-aa`, then implement the new trait method in **one dependent crate at a time**. You never touch thousands of downstream files to add a single invariant.

**Target scale:** 100+ to 400+ source files inside `ids-000-aa/src/` alone (contracts, invariants, reserved slots, internals), plus all downstream `ids-*` crates. This is the central hub — everything flows through it.

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

## 3. File Structure — ids-000-aa (100+ to 400+ files)

The master crate contains **100+ to 400+ source files** organized into subfolders. Each file is real implementation code — no stubs, no padding to hit arbitrary line counts.

```
ids-000-aa/
├── Cargo.toml                                    # Workspace member manifest
├── .cargo/
│   └── config.toml                               # Build configuration
├── PLAN.md                                       # This document
├── CONTRACTS.md                                  # Sealed trait contract reference (external doc)
└── src/
    ├── lib.rs                                    # Crate root — pub use re-exports (central hub facade)
    ├── types.rs                                  # ALL canonical types — ONE definition each
    ├── versioning.rs                             # Protocol version + edge-case version bits
    ├── errors.rs                                 # Sealed error contracts (thiserror)
    ├── prelude.rs                                # Common imports re-exported
    │
    ├── contracts/                                # Sealed trait contracts — control plane interface (~25 files)
    │   ├── mod.rs
    │   ├── carrier.rs            # trait Carrier — send/receive opaque datagrams
    │   ├── transport.rs          # trait Transport — framing, sequencing, acks
    │   ├── store.rs              # trait Store — put/get/delete with durability
    │   ├── session.rs            # trait Session — client ↔ IDS one-call binding
    │   ├── routing.rs            # trait Routing — logical → physical resolution
    │   ├── discovery.rs          # trait Discovery — node/H-unit/region discovery
    │   ├── membership.rs         # trait Membership — join/leave/lifecycle
    │   ├── placement.rs          # trait Placement — where data lives
    │   ├── replication.rs        # trait Replication — streams, checkpoints, retries
    │   ├── recovery.rs           # trait Recovery — multi-level recovery
    │   ├── consistency.rs        # trait Consistency — profiles, committed-prefix
    │   ├── ordering.rs           # trait Ordering — dataset-scoped domains
    │   ├── security.rs           # trait Security — auth, authz, encryption, key mgmt
    │   ├── client.rs             # trait Client — THE ONE CALL (ids_submit)
    │   ├── fault.rs              # trait FaultTolerance — circuit breaker, bulkhead
    │   ├── broadcast.rs          # trait Broadcast — reliable/causal/total order
    │   ├── quorum.rs             # trait Quorum — read/write intersection
    │   ├── snapshot.rs           # trait Snapshot — distributed consistent state
    │   ├── byzantine.rs          # trait Byzantine — BFT consensus
    │   ├── lease.rs              # trait Lease — lease-based consistency
    │   ├── crdt.rs               # trait CRDT — convergent types
    │   ├── erasure.rs            # trait Erasure — Reed-Solomon
    │   ├── saga.rs               # trait Saga — compensating transactions
    │   └── coordinator.rs        # trait Coordinator — domain-scoped leadership
    │
    ├── invariants/                               # Protocol invariants — sealed (~20 core + 400+ reserved)
    │   ├── mod.rs
    │   ├── binary_completion.rs      # 100% done or failed; no partial success
    │   ├── dead_node_identity.rs     # dead node never returns with old identity
    │   ├── idle_rotation.rs          # RNI rotates only while node idle
    │   ├── no_global_master.rs       # dataset-scoped ordering only
    │   ├── out_eq_in.rs              # byte accounting boundary
    │   ├── coordinated_deletion.rs   # all holders delete simultaneously
    │   ├── fencing_stonith.rs        # STONITH before failover
    │   ├── quorum_intersection.rs    # read quorum ∩ write quorum ≠ ∅
    │   ├── erasure_threshold.rs      # any k of n reconstructs
    │   ├── correlated_failure.rs     # replicas never share failure domain
    │   ├── edge_case_registry.rs     # versioned bitfield of every edge case
    │   └── reserve/                  # 400+ reserved invariant slots for future edge cases
    │       ├── reserved_001.rs
    │       ├── reserved_002.rs
    │       ├── reserved_003.rs
    │       ├── reserved_004.rs
    │       ├── reserved_005.rs
    │       ├── reserved_006.rs
    │       ├── reserved_007.rs
    │       ├── reserved_008.rs
    │       ├── reserved_009.rs
    │       ├── reserved_010.rs
    │       ├── reserved_011.rs
    │       ├── reserved_012.rs
    │       ├── reserved_013.rs
    │       ├── reserved_014.rs
    │       ├── reserved_015.rs
    │       ├── reserved_016.rs
    │       ├── reserved_017.rs
    │       ├── reserved_018.rs
    │       ├── reserved_019.rs
    │       ├── reserved_020.rs
    │       ├── reserved_021.rs
    │       ├── reserved_022.rs
    │       ├── reserved_023.rs
    │       ├── reserved_024.rs
    │       ├── reserved_025.rs
    │       ├── reserved_026.rs
    │       ├── reserved_027.rs
    │       ├── reserved_028.rs
    │       ├── reserved_029.rs
    │       ├── reserved_030.rs
    │       ├── reserved_031.rs
    │       ├── reserved_032.rs
    │       ├── reserved_033.rs
    │       ├── reserved_034.rs
    │       ├── reserved_035.rs
    │       ├── reserved_036.rs
    │       ├── reserved_037.rs
    │       ├── reserved_038.rs
    │       ├── reserved_039.rs
    │       ├── reserved_040.rs
    │       ├── reserved_041.rs
    │       ├── reserved_042.rs
    │       ├── reserved_043.rs
    │       ├── reserved_044.rs
    │       ├── reserved_045.rs
    │       ├── reserved_046.rs
    │       ├── reserved_047.rs
    │       ├── reserved_048.rs
    │       ├── reserved_049.rs
    │       ├── reserved_050.rs
    │       ├── reserved_051.rs
    │       ├── reserved_052.rs
    │       ├── reserved_053.rs
    │       ├── reserved_054.rs
    │       ├── reserved_055.rs
    │       ├── reserved_056.rs
    │       ├── reserved_057.rs
    │       ├── reserved_058.rs
    │       ├── reserved_059.rs
    │       ├── reserved_060.rs
    │       ├── reserved_061.rs
    │       ├── reserved_062.rs
    │       ├── reserved_063.rs
    │       ├── reserved_064.rs
    │       ├── reserved_065.rs
    │       ├── reserved_066.rs
    │       ├── reserved_067.rs
    │       ├── reserved_068.rs
    │       ├── reserved_069.rs
    │       ├── reserved_070.rs
    │       ├── reserved_071.rs
    │       ├── reserved_072.rs
    │       ├── reserved_073.rs
    │       ├── reserved_074.rs
    │       ├── reserved_075.rs
    │       ├── reserved_076.rs
    │       ├── reserved_077.rs
    │       ├── reserved_078.rs
    │       ├── reserved_079.rs
    │       ├── reserved_080.rs
    │       ├── reserved_081.rs
    │       ├── reserved_082.rs
    │       ├── reserved_083.rs
    │       ├── reserved_084.rs
    │       ├── reserved_085.rs
    │       ├── reserved_086.rs
    │       ├── reserved_087.rs
    │       ├── reserved_088.rs
    │       ├── reserved_089.rs
    │       ├── reserved_090.rs
    │       ├── reserved_091.rs
    │       ├── reserved_092.rs
    │       ├── reserved_093.rs
    │       ├── reserved_094.rs
    │       ├── reserved_095.rs
    │       ├── reserved_096.rs
    │       ├── reserved_097.rs
    │       ├── reserved_098.rs
    │       ├── reserved_099.rs
    │       ├── reserved_100.rs
    │       ├── ...                   # continues through reserved_400.rs
    │       └── reserved_400.rs
    │
    └── internals/                              # Master crate internal implementation (~6 files)
        ├── mod.rs
        ├── type_registry.rs        # Runtime type ID registry
        ├── version_graph.rs        # Version compatibility graph
        ├── contract_validator.rs   # Compile-time contract validation
        ├── invariant_checker.rs    # Invariant checking at build time
        └── edge_case_compiler.rs   # Compiles edge_case_registry bits into cfg flags
```

**File count inside `ids-000-aa/src/`:**
- Root: 5 files (`lib.rs`, `types.rs`, `versioning.rs`, `errors.rs`, `prelude.rs`)
- `contracts/`: ~25 files
- `invariants/`: ~20 core files + 400+ reserved files = ~420 files
- `internals/`: 6 files
- **Total inside master crate: ~450+ files** (within the 100+ to 400+ target range for the core structure, scaling to 400+ with reserved slots)

---

## 4. Canonical Types (`src/types.rs`)

These are the **only** definitions of these types in the entire workspace. Every `ids-*` crate imports them as `ids_000_aa::NodeId`, etc.

```rust
// Core identifiers
pub struct NodeId(pub [u8; 32]);        // BLAKE3(ed25519 pubkey)
pub struct RegionId(pub [u8; 32]);
pub struct HUnitId(pub [u8; 32]);
pub struct DatasetId(pub [u8; 32]);
pub struct PartitionId(pub [u8; 32]);
pub struct RecordId(pub [u8; 32]);
pub struct OpRef(pub [u8; 9]);          // Nine-char alphanumeric, packed
pub struct OrderingPos(pub u64);

// Carrier-agnostic address (NO IP, NO port, NO SocketAddr)
pub struct Endpoint {
    pub carrier_type: CarrierType,
    pub address: Vec<u8>,               // Opaque carrier-specific bytes
}
pub enum CarrierType {
    Loopback,
    Serial,
    Optical,
    Radio,
    Custom(u16),
}

// Operation states
pub enum OpState { Received, Ordered, Executing, Committed, Synchronized, Verified, Failed }
pub enum CommitState { Accepted, Prepared, Committed, Aborted }

// Redundancy and consistency
pub enum RedundancyMode { FullReplica, ErasureCoded(k: u32, n: u32), Hybrid }
pub enum ConsistencyProfile { Strong, Causal, BoundedStaleness, Eventual, ReadYourWrites }

// Placement decision
pub struct PlacementDecision {
    pub region: RegionId,
    pub hunit: HUnitId,
    pub node: NodeId,
    pub redundancy: RedundancyMode,
    pub failure_domain: FailureDomain,
}

// Failure domain
pub struct FailureDomain {
    pub power: Option<[u8; 16]>,
    pub facility: Option<[u8; 16]>,
    pub network_segment: Option<[u8; 16]>,
    pub operator: Option<[u8; 16]>,
    pub geographic: Option<[u8; 16]>,
}
```

---

## 5. Sealed Trait Contracts (`src/contracts/`)

Every contract is sealed: `pub trait X: private::Sealed {}`. Downstream crates can **call** methods and use as trait bounds, but cannot implement. Only `ids-000-aa` and its designated implementation crates can `impl`. This means you can add methods to any contract without breaking downstream — zero edits to thousands of files.

Each contract file contains:
- The sealed trait declaration
- The `private::Sealed` supertrait module
- Implementations for master-crate types
- Method signatures with full `Result<T, Error>` error contracts
- Doc comments explaining the invariant each method enforces

**Contract files (25):**

| File | Trait | Responsibility |
|------|-------|---------------|
| `carrier.rs` | `Carrier` | Send/receive opaque datagrams |
| `transport.rs` | `Transport` | Framing, sequencing, acks |
| `store.rs` | `Store` | Put/get/delete with durability |
| `session.rs` | `Session` | Client ↔ IDS one-call binding |
| `routing.rs` | `Routing` | Logical → physical resolution |
| `discovery.rs` | `Discovery` | Node/H-unit/region discovery |
| `membership.rs` | `Membership` | Join/leave/lifecycle |
| `placement.rs` | `Placement` | Where data lives |
| `replication.rs` | `Replication` | Streams, checkpoints, retries |
| `recovery.rs` | `Recovery` | Multi-level recovery |
| `consistency.rs` | `Consistency` | Profiles, committed-prefix |
| `ordering.rs` | `Ordering` | Dataset-scoped domains |
| `security.rs` | `Security` | Auth, authz, encryption, key mgmt |
| `client.rs` | `Client` | THE ONE CALL (`ids_submit`) |
| `fault.rs` | `FaultTolerance` | Circuit breaker, bulkhead |
| `broadcast.rs` | `Broadcast` | Reliable/causal/total order |
| `quorum.rs` | `Quorum` | Read/write intersection |
| `snapshot.rs` | `Snapshot` | Distributed consistent state |
| `byzantine.rs` | `Byzantine` | BFT consensus |
| `lease.rs` | `Lease` | Lease-based consistency |
| `crdt.rs` | `CRDT` | Convergent types |
| `erasure.rs` | `Erasure` | Reed-Solomon |
| `saga.rs` | `Saga` | Compensating transactions |
| `coordinator.rs` | `Coordinator` | Domain-scoped leadership |

Example — `contracts/recovery.rs`:

```rust
mod private {
    pub trait Sealed {}
}

pub trait Recovery: private::Sealed {
    fn compensate_node_failure(&self, node: NodeId) -> Result<RecoveryPlan, RecoveryError>;
    fn compensate_hunit_failure(&self, hunit: HUnitId) -> Result<RecoveryPlan, RecoveryError>;
    fn compensate_regional_failure(&self, region: RegionId) -> Result<RecoveryPlan, RecoveryError>;
    fn reconstruct_erasure(&self, fragments: Vec<FragmentId>) -> Result<Vec<u8>, RecoveryError>;
    fn verify_recovery(&self, proof: RecoveryProof) -> Result<(), RecoveryError>;
}

impl private::Sealed for RecoveryImpl {}
impl Recovery for RecoveryImpl { ... }
```

---

## 6. Protocol Invariants (`src/invariants/`)

Each invariant is a sealed trait + compile-time check. Downstream crates reference them as bounds but cannot weaken them.

**Core invariants (20):**

| File | Invariant | Rule |
|------|-----------|------|
| `binary_completion.rs` | Binary completion | Op is 100% done or failed; no partial success |
| `dead_node_identity.rs` | Dead node identity | Fresh ID, old ID retired forever |
| `idle_rotation.rs` | Idle-only rotation | RNI changes only while node idle |
| `no_global_master.rs` | No global master | Dataset-scoped ordering only |
| `out_eq_in.rs` | out=in byte accounting | bytes-out == bytes-in+acked |
| `coordinated_deletion.rs` | Coordinated deletion | All holders delete simultaneously |
| `fencing_stonith.rs` | Fencing/STONITH | No split-brain failover |
| `quorum_intersection.rs` | Quorum intersection | read quorum ∩ write quorum ≠ ∅ |
| `erasure_threshold.rs` | Erasure threshold | Any k of n reconstructs |
| `correlated_failure.rs` | Correlated failure | Replicas never share failure domain |
| `edge_case_registry.rs` | Edge case registry | Versioned bitfield of every edge case |
| `reserved_001.rs` … `reserved_400.rs` | Reserved slots | 400+ pre-allocated invariant templates |

**Reserved invariant slots (400+):** `src/invariants/reserve/` contains `reserved_001.rs` through `reserved_400.rs`. Each is a sealed template:

```rust
//! Reserved invariant slot N.
//! When you add a new edge case, rename this file, replace the template,
//! and add the invariant method to the relevant contract trait in contracts/.
//! No downstream crate needs to change — the new method appears automatically
//! via the sealed trait contract.
#![allow(dead_code)]

pub trait ReservedInvariantN: Sealed {
    fn reserved_check_n(&self) -> bool;
}
```

This gives you 400+ pre-allocated slots so you never renumber or restructure when adding edge cases.

---

## 7. Versioning (`src/versioning.rs`)

Protocol version + edge-case version bits as a compile-time bitfield:

```rust
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

pub struct EdgeCaseBits(u128); // 128 versioned edge-case flags

impl EdgeCaseBits {
    pub const BINARY_COMPLETION: u128 = 1 << 0;
    pub const DEAD_NODE_IDENTITY: u128 = 1 << 1;
    // ... through bit 127
    pub const RESERVED_400: u128 = 1 << 127; // and beyond via u256 if needed
}
```

When you add an edge case, flip its bit here. Downstream crates check `cfg!(feature = "edge_case_N")` or `ids_000_aa::EdgeCaseBits::has(N)` at compile time.

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
    // ... 400+ reserved error variants
}
```

---

## 9. Patterns Table

| Pattern | Where | Why |
|---------|-------|-----|
| Central hub | `ids-000-aa` | One file edit ripples to all downstream crates |
| Sealed traits | `contracts/` | Downstream can call but not impl; add methods without breaking |
| Sealed invariants | `invariants/` | Compile-time enforcement, cannot be bypassed |
| Reserved slots | `invariants/reserve/` | 400+ pre-allocated edge case templates |
| Version bits | `versioning.rs` | Compile-time cfg gating per edge case |
| Opaque Endpoint | `types.rs` | `CarrierType` + `Vec<u8>`, no IP/TCP/UDP/SocketAddr |
| One definition | `types.rs` | Every canonical type defined once, imported everywhere |
| `pub use` facade | `lib.rs` | Flat public API: `ids_000_aa::NodeId`, etc. |

---

## 10. Build Order (one file at a time)

Per the one-file-at-a-time git workflow:

1. `src/lib.rs` — crate root
2. `src/types.rs` — canonical types (foundation)
3. `src/contracts/mod.rs` + all 25 contract files
4. `src/invariants/mod.rs` + 20 core invariants + 400+ reserved slots
5. `src/versioning.rs`
6. `src/errors.rs`
7. `src/prelude.rs`
8. `src/internals/mod.rs` + 6 internal files

Then downstream crates (one at a time): `ids-carrier`, `ids-crypto`, `ids-identity`, `ids-time`, `ids-topology`, etc.

---

## 11. Acceptance Criteria

- [ ] `ids-000-aa` compiles as a standalone crate with no `std::net`, `tokio::net`, or libp2p dependencies
- [ ] Every canonical type has exactly one definition in `types.rs`
- [ ] Every contract trait is sealed (downstream cannot impl)
- [ ] Every invariant is enforced at compile time via trait bounds
- [ ] `pub use` in `lib.rs` re-exports all types/contracts/invariants/errors at crate root
- [ ] 400+ reserved invariant slots exist for future edge cases
- [ ] Adding a new edge case requires editing exactly one file in `ids-000-aa` + implementing in one downstream crate
- [ ] `ids-crypto` is the only crate not depending on `ids-000-aa`
- [ ] `Endpoint` uses `CarrierType` + `Vec<u8>` only — no IP, no port, no `SocketAddr`
- [ ] File structure shows 100+ to 400+ files inside `ids-000-aa/src/`

---

## 12. Git Workflow (one file at a time)

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
