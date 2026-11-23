## Phase 025 — Create the ids-recovery crate (node/H-unit/region/region-wide failure recovery): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Verbatim prompt (C:/idfs/prompts.txt lines 370-384):
"25. Research only do not read plan.md **[Phase 025 — Create the ids-recovery crate: Node/H-unit/region/region-wide failure recovery.]** Crate: ids-recovery Purpose: Node/H-unit/region/region-wide failure recovery. Contents: Node failure recovery, H-unit failure compensation, cross-H-unit recovery, regional failure compensation, regional failure isolation, replica-based recovery, erasure-code reconstruction, capacity-bounded recovery, ownership-aware recovery, recovery verification, redundancy restoration, multi-level recovery state machine. Create plan-025.md in C:/idfs/plans/ for the ids-recovery crate. Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources. Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305. ids-000-aa is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only the plan document."

CRATES.md entry 25: "ids-recovery — Multi-level recovery and erasure reconstruction" — Purpose: Node/H-unit/region/region-wide failure recovery.

A skeleton `ids-recovery` crate does NOT yet exist at `C:/idfs/ids-recovery/`. `ids-000-aa` already declares the sealed `trait Recovery` at `src/contracts/recovery.rs` plus canonical recovery types (RecoveryPlan, RecoveryStep, RecoveryScope, FailureEvent, FailureEventType, RecoveryScopeLevel, RecoveryAction, RecoveryProof, FailureDetector, RepairPlanner, StateReconciler, ReplicaReconstructor, QuarantineManager, RecoveryScheduler, RecoveryStats, RecoveryTimeline, RecoveryCoordinator, ReplicaRepairStateTracker, RegionalRecoveryCoordinator, NodeRecoveryTracker, HUnitRecoveryTracker, RecoveryResourceManager) at `src/types.rs`. `ids-recovery` implements the sealed `trait Recovery` contract. Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-recovery` consumes type-level surface and primitive IDs only). `ids-000-aa` is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only this plan document.

---

### 1. Executive Summary

Phase 025 creates `ids-recovery`, the IDS crate for multi-level failure recovery across nodes, H-units, and regions. It owns the recovery lifecycle: failure detection (heartbeat timeouts), recovery planning (per-scope step graphs), plan execution (replica reconstruction, erasure repair, replica copy, ownership transfer), quarantine/release of suspect scopes, state reconciliation, redundancy restoration, and recovery verification via cryptographic proofs. Recovery is organized into four scope levels — Node, HUnit, Region, and RegionWide — each with its own escalation path and repair strategy. All traffic rides `ids-carrier::Endpoint` opaque datagrams; `ids-recovery` never opens a socket or references any transport-specific type. Recovery proofs are verified with BLAKE3/SHA-256 checksums and authenticated with ed25519; optional payload encryption uses XChaCha20-Poly1305/ChaCha20-Poly1305. Capacity bounds govern concurrent recovery to prevent thundering-herd rebuild storms. The crate is research-only in this phase; no code or source modifications are produced.

**Scope (from prompts.txt entry 25 and CRATES.md §25):** node failure recovery, H-unit failure compensation, cross-H-unit recovery, regional failure compensation, regional failure isolation, replica-based recovery, erasure-code reconstruction, capacity-bounded recovery, ownership-aware recovery, recovery verification, redundancy restoration, multi-level recovery state machine.

---

### 2. Architecture Overview

`ids-recovery` sits above `ids-000-aa` (canonical types + sealed `trait Recovery` contract + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each recovery concern — detection, planning, execution, verification, quarantine, redundancy — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-* LAYER                            │
│  (dataset owners request redundancy restoration, verification, rebalance)│
├─────────────────────────────────────────────────────────────────────────┤
│                    Phase 025: ids-recovery (THIS CRATE)                 │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │\n│  │ detection    │ │ planning     │ │ execution    │ │ verification │ │\n│  │ (heartbeat)  │ │ (per-scope   │ │ (reconstruct,│ │ (proofs,     │ │\n│  │ node/hunit/  │ │  step graph) │ │  copy, erase)  │ │  checksums)  │ │\n│  │ region)      │ └──────────────┴────────────────┴──────────────┘ │\n│  └──────────────┘                                                    │\n│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ │\n│  │ quarantine   │ │ redundancy   │ │ capacity     │ │ state machine│ │\n│  │ / release    │ │ restoration  │ │ bounding     │ │ (multi-level)│ │\n│  └──────────────┘ └──────────────┴────────────────┴──────────────┘ │\n└─────────────────────────────────────────────────────────────────────────┘
                                   │ opaque Vec<u8> datagrams
┌─────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only               │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, payload: &[u8])          │
│  receive_datagram → (Endpoint, Vec<u8>)                                 │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP       │
└─────────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `OrderingPos`, `Capacity`, `FailureEvent`, `FailureEventType`, `RecoveryScopeLevel`, `RecoveryAction`, `RecoveryProof`, `RedundancyMode`, `DurabilityCondition`, `Endpoint`, `CarrierType`, `Checksum`, `FragmentId`, `FragmentPlacement`, `IdsTime`) imported from `ids_000_aa::X`, never redefined.
- `ids-recovery` implements the sealed `trait Recovery` from `ids-000-aa/src/contracts/recovery.rs`; downstream crates use it as a trait bound, cannot implement it.
- All message transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16) + opaque `Vec<u8>`). No IP addresses, ports, sockets.
- Crypto surface: ed25519 signatures (recovery proof authentication, quarantine attestation), sha256/blake3 (fragment and state checksums, content addressing), xchacha20poly1305/chacha20poly1305 (optional recovery payload encryption). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Recovery scope levels: Node (1), HUnit (2), Region (3); escalation proceeds upward when lower-level repair is insufficient.
- Operation lifecycle reuses `ids-000-aa` `RecoveryPhase` (Detection → Planning → Execution → Verification → Completion) and `RecoveryAction` (ReconstructErasure, CopyReplica, TransferOwnership, RestoreRedundancy, VerifyIntegrity, QuarantineNode).
- Repair strategy priority (per `RepairPlanner`): CopyReplica (1) → RestoreRedundancy (2) → ReconstructErasure (3) → TransferOwnership (4).
- No global recovery master (invariant `no_global_master` in `ids-000-aa`); each domain manages its own recovery context.

---

### 3. Detailed Research Dimensions

#### 3.1 Multi-Level Recovery Scope Hierarchy (Node / HUnit / Region / RegionWide)

Research basis: hierarchical fault domains (zone/region two-level awareness, Mesos/DC-OS fault domain awareness; Google Cloud building blocks of reliability — regions contain zones; independent failure domains). Failure domain isolation restricts the effect of a failure to a limited number of components; components outside the boundary are unaffected.

- **Node level:** single NodeId fails (crash, disk failure, power failure). Repair: CopyReplica from surviving peers, or ReconstructErasure from erasure-coded fragments. Fastest, most local recovery.
- **HUnit level:** hardware unit (rack/chassis grouping within a region) fails; all nodes in the HUnit are unreachable. Repair: restore redundancy by re-placing replicas to other HUnits in the same region; cross-H-unit recovery if intra-region capacity is insufficient.
- **Region level:** entire RegionId fails (regional outage, network partition isolating a region). Repair: cross-region recovery — fail over to replicas in surviving regions, restore redundancy region-wide, update placement decisions.
- **Region-wide level:** correlated failure spanning multiple regions or the whole deployment. Repair: last-resort erasure-code reconstruction from globally dispersed fragments, ownership transfer, capacity-bounded staged restoration.

Decision: model scope as `RecoveryScope { Node(NodeId) | HUnit(HUnitId) | Region(RegionId) }` (already in `ids-000-aa`); add a `RegionWide` virtual scope concept for cross-region correlated failures handled by the `RegionalRecoveryCoordinator`. Escalation: node → hunit → region → region-wide, each with increasing repair cost and slower convergence.

#### 3.2 Failure Detection (Heartbeat Timeouts + Failure Event Classification)

Research basis: gossip-style failure detection (Cornell "Gossip Style Failure Detection" — random gossiping scales well, resilient to message loss; false-report probability independent of member count); heartbeat-based detection with timeout thresholds; Chandra-Toueg failure detectors (consensus reducible to atomic broadcast). Detection classifies failures into `FailureEventType`: NodeFailure, HUnitFailure, RegionalFailure, NetworkPartition, DiskFailure, PowerFailure.

Decision: heartbeat timeouts per scope (node/hunit/region) recorded via `FailureDetector` (already in `ids-000-aa`, default 30s timeout); a missed heartbeat emits a `FailureEvent` with `ids_time` (IDS standard time, not wall-clock); detection feeds the `RecoveryCoordinator` state machine (Detection → Planning → ...). No timeout-based eviction — eviction is deliberate, never automatic.

#### 3.3 Recovery Planning (Per-Scope Step Graphs)

Research basis: two-level/multi-level distributed recovery schemes (ACM "A case for two-level distributed recovery schemes" — differentiate probable vs. improbable failures, lower overhead for common cases); Derecho reliable efficient recovery (view changes, epoch termination, restart quorum); Raft reconfiguration (joint consensus, learner promotion, add-then-remove ordering).

Decision: `RecoveryPlan` (already in `ids-000-aa`) holds plan_id, failure_event, scope, ordered `Vec<RecoveryStep>`, status (Pending/InProgress/Completed/Failed/Cancelled), timestamps. Each `RecoveryStep` carries a `RecoveryAction`, scope level, target, attempt/max_attempts, status. Planner selects strategy by scope level: Node→CopyReplica, HUnit→RestoreRedundancy, Region→ReconstructErasure. Plan execution advances step-by-step; failed steps retry up to max_attempts then fail the plan.

#### 3.4 Replica-Based Recovery (Copy Replica / Restore Redundancy)

Research basis: replication-based fault tolerance (DSN 2014 — checkpoint-based recovery; reload state from last checkpoint); replica-based recovery copies a healthy replica to a replacement node; read quorum (R=1 in Derecho shards) lets any up-to-date replica serve state.

Decision: `CopyReplica` action copies full dataset state from a healthy donor node to a replacement node via `ids-carrier::Endpoint` datagrams; `RestoreRedundancy` re-establishes the target replication factor by placing new replicas on healthy nodes selected by capacity (`Capacity.can_serve`) and failure-domain placement (`PlacementDecision`). Donor selection prefers same-region, different-HUnit nodes to limit blast radius.

#### 3.5 Erasure-Code Reconstruction (k-of-n Fragment Repair)

Research basis: regenerating codes (Dimakis et al. — minimize repair bandwidth; exact minimum-repair-bandwidth cooperative regenerating codes); robust repair of Reed-Solomon codes under erroneous helper responses; fragment distribution across data owner/parity owner nodes; rebuilding lost data via computation from a recovery strip set.

Decision: `ReconstructErasure` action reconstructs lost fragments from surviving k-of-n erasure-coded fragments; helper nodes return fragment hashes (BLAKE3) first to detect erroneous responses before full fragment transfer; reconstruction writes to a new node and updates `FragmentPlacement`. Tolerates up to n−k simultaneous fragment losses. Verification via BLAKE3/SHA-256 checksum of reconstructed data against the original content hash.

#### 3.6 Ownership-Aware Recovery (Transfer Ownership)

Research basis: Raft group reconfiguration (add learners first, promote one at a time, remove after sync); ReCraft (wait-free membership changes, quorum overlap); Khepri failure recovery (Raft-driven, new leader elected, rejoining node catches up from common log point).

Decision: `TransferOwnership` action reassigns dataset leadership/ownership from a failed node to a healthy replica; ownership transfer requires the target to be caught up to the current `OrderingPos`; fencing epoch increment prevents split-brain; transfer is ownership-only, no data movement unless combined with CopyReplica.

#### 3.7 Capacity-Bounded Recovery (Throttling / Resource Limits)

Research basis: SRE throttling design pattern (coordinated overload rejection, per-caller quotas); token pools / DRB (bounded resource entitlements, fair-share convergence); recovery load bounded — clearing a backlog cannot exceed downstream safe capacity; bounded backoff with jitter.

Decision: `RecoveryResourceManager` (already in `ids-000-aa`) bounds concurrent active recoveries by `resource_limit`; excess recoveries are queued/rejected; per-recovery capacity requirements (`Capacity`) checked against donor node `can_serve` before starting; bounded exponential backoff between retry attempts; no unbounded parallel rebuilds that could saturate carrier bandwidth or donor capacity.

#### 3.8 Recovery Verification (Proofs + Checksums)

Research basis: content-addressed storage verification (BLAKE3 blob hashing, verified streaming via bao); recovery proof = dataset_id + recovered_fragments + checksum + ids_time (already `RecoveryProof` in `ids-000-aa`); integrity verification result per node (Checksum, verified flag, anomaly).

Decision: `VerifyIntegrity` action recomputes BLAKE3/SHA-256 checksum over recovered/replicated data and compares to the recorded content hash; a `RecoveryProof` is issued only when checksums match; mismatches trigger `Anomaly` (ByteMismatch, ChecksumFailure, StateCorruption, ReplicaDrift) and quarantine. Verification is mandatory before a plan reaches Completion.

#### 3.9 Quarantine and Release (Suspected Failure Isolation)

Research basis: Byzantine node isolation/quarantine; exclusion as last resort; identity-bound exclusion (not unauthenticated peer handles); post-compromise handling without breaking causality (decouple content trust from identity trust).

Decision: `QuarantineManager` (already in `ids-000-aa`) isolates a scope (node/hunit/region) with a `QuarantineReason` (SuspectedFailure, CorruptionDetected, Unresponsive, PolicyViolation); quarantined scopes are excluded from donor/recipient selection; release requires successful `VerifyIntegrity`; quarantine is deliberate, never timeout-based.

#### 3.10 Multi-Level Recovery State Machine

Research basis: multi-level recovery schemes (common case fast, worst case covered); recovery phase state machine (Detection, Planning, Execution, Verification, Completion — already `RecoveryPhase` in `ids-000-aa`); transition ordering enforced (can_transition requires order_index increase).

Decision: `RecoveryCoordinator` (already in `ids-000-aa`) drives the phase machine per active scope; `RegionalRecoveryCoordinator` handles cross-region recovery initiation; `NodeRecoveryTracker`/`HUnitRecoveryTracker` track per-node/hunit recovery state (Healthy, Degraded, Recovering, Failed, Quarantined); `RecoveryTimeline` records failure_detected → plan_created → plan_started → step_completed/failed → plan_completed/failed → quarantined/released/reconciled/reconstructed.

#### 3.11 Regional Failure Compensation and Isolation

Research basis: cell-based architecture (AWS — fault isolated boundaries limit blast radius; bulkheads contain hull breaches); region outage failover (Spanner failover — independent failure domains with separate Paxos quorums); regional load balancer failover; drain-a-region traffic shifting.

Decision: regional failure compensation redirects traffic/ownership to surviving regions before repair begins; regional failure isolation prevents the failed region's nodes from being selected as donors or recipients; cross-region recovery re-establishes redundancy in surviving regions; region health tracked (Healthy/Degraded/Critical/Failed — already `HealthStatus`).

#### 3.12 Cross-H-Unit Recovery

Decision: when an HUnit fails, cross-H-unit recovery places replacement replicas in other HUnits within the same region (intra-region, low latency); if intra-region capacity is insufficient, escalate to region-level recovery. Placement respects failure domain (different HUnit from surviving copies to avoid correlated loss).

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Rebuild storm (thundering herd) saturates carrier/donor capacity | Capacity-bounded recovery (`RecoveryResourceManager`); per-donor capacity check; bounded backoff; staged restoration |
| Donor node fails mid-rebuild | Retry with alternate replica; step-level max_attempts; plan-level failure escalation to higher scope |
| Corrupt replica used as donor | Verify integrity (BLAKE3/SHA-256) before and after copy; `VerifyIntegrity` action; quarantine corrupt donors |
| Erasure reconstruction with erroneous helper responses | Fragment hash pre-check (BLAKE3) before full transfer; robust repair tolerates e erroneous helpers |
| Split-brain during ownership transfer | Fencing epoch increment; target must be caught up to current `OrderingPos`; no global master |
| Quarantine blocks all donors (insufficient replicas) | `InsufficientReplicas` error; escalate scope (node→hunit→region→region-wide); alert for manual intervention |
| Recovery never completes (offline/permanent failure) | Deliberate eviction only, never timeout-based; quarantine with release condition; membership management required |
| Zombie/resurrected stale data | Recovery proofs bind to `ids_time` and content checksum; stale replicas rejected by `OrderingPos` comparison |
| Carrier loss during recovery datagrams | Carrier-agnostic datagrams may be lost; recovery is idempotent at step level; retransmission handled by caller/retry manager |
| Resource exhaustion from concurrent recoveries | `RecoveryResourceManager` hard limit; queued/rejected overflow; per-recovery capacity budgets |
| Region-wide correlated failure | Cross-region recovery; restore redundancy in surviving regions; failover before repair |

**Critical failure modes:** (1) donor exhaustion — all healthy replicas also fail during rebuild; mitigated by scope escalation and `InsufficientReplicas` error. (2) Erasure code below k survivors — data unrecoverable; mitigated by proactive redundancy restoration before hitting threshold. (3) Quarantine of a majority region — remaining regions cannot form quorum; mitigated by deliberate membership management, never automatic eviction.

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 025 ||
|-------|-------|-------------------|
| `ids-000-aa` | Master crate: canonical types (NodeId, RegionId, HUnitId, DatasetId, PartitionId, OrderingPos, Capacity, FailureEvent, FailureEventType, RecoveryScopeLevel, RecoveryAction, RecoveryProof, RedundancyMode, DurabilityCondition, Endpoint, CarrierType, Checksum, FragmentId, FragmentPlacement, IdsTime, HealthStatus), sealed `trait Recovery`, `RecoveryError`, sub-structures (RecoveryPlan, RecoveryStep, FailureDetector, RepairPlanner, StateReconciler, ReplicaReconstructor, QuarantineManager, RecoveryScheduler, RecoveryStats, RecoveryTimeline, RecoveryCoordinator, ReplicaRepairStateTracker, RegionalRecoveryCoordinator, NodeRecoveryTracker, HUnitRecoveryTracker, RecoveryResourceManager) |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive; all recovery traffic |
| `ids-crypto` | ed25519 (proof/signature), sha256/blake3 (checksums), xchacha20poly1305/chacha20poly1305 (optional encryption) |
| `ids-membership` | Node/HUnit/Region roster; failure-domain placement; donor selection |
| `ids-placement` | `PlacementDecision` (region/hunit/node/redundancy/failure_domain) for replica placement |
| `ids-write` | Source of committed ops (`OrderingPos`, `OpRef`) that recovery must preserve |
| `ids-snapshot` (Phase 026) | Checkpoints used as recovery restore points |
| `ids-fault` (Phase 027) | Circuit breakers/bulkheads that protect recovery from cascading load |
| `ids-broadcast` (Phase 020) | Ordered delivery of recovery coordination messages (optional) |

**Integration points:**
- `ids-recovery` implements `ids-000-aa::Recovery` sealed trait; downstream crates call `detect_failures`, `create_plan`, `execute_plan`, `quarantine`, `restore_redundancy`, `reconcile_state`.
- Failure events flow from `FailureDetector` (heartbeats) → `RecoveryCoordinator` (phase machine) → `RepairPlanner` (strategy) → step execution → `StateReconciler`/`ReplicaReconstructor` → `QuarantineManager`/`VerifyIntegrity` → `RecoveryStats`/`RecoveryTimeline`.
- All inter-node recovery traffic (fragment copy, parity transfer, state sync) uses `ids-carrier::Endpoint` datagrams; no direct carrier imports beyond `Endpoint` and `CarrierType`.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — preserves carrier-agnostic architecture.

---

### 6. Proposed File Structure (~90–150 files, one new crate, all in subfolders)

**Crate:** `ids-recovery` (new crate at `C:/idfs/ids-recovery/`; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; serde, thiserror, bytes, blake3, sha2, ed25519-dalek, chacha20poly1305, xchacha20poly1305, proptest/bolero for testing)

```
ids-recovery/
├── Cargo.toml                          # Crate manifest, workspace registration
├── lib.rs                              # Crate root, feature gates, public re-exports
├── constants.rs                        # Recovery constants (heartbeat timeouts, retry limits, capacity bounds)
├── error.rs                            # RecoveryError extensions (thiserror)
├── features.rs                         # Feature flags (verify, encrypt, fuzz, bench)
│
├── detection/                          # Failure detection (heartbeat timeouts)
│   ├── mod.rs
│   ├── heartbeat_monitor.rs            # Per-scope heartbeat recording + timeout detection
│   ├── failure_classifier.rs           # Classify FailureEventType (node/hunit/region/disk/power/partition)
│   ├── detection_scheduler.rs          # When to run detection rounds
│   ├── detection_types.rs              # HeartbeatRecord, DetectionConfig structures
│   └── detection_proptests.rs          # Property tests: timeout bounds, false-positive rate
│
├── planning/                           # Recovery planning (per-scope step graphs)
│   ├── mod.rs
│   ├── recovery_planner.rs             # Builds RecoveryPlan from FailureEvent
│   ├── step_graph.rs                   # Ordered step graph with dependencies
│   ├── strategy_selector.rs            # Selects RepairStrategy by scope level
│   ├── scope_escalator.rs              # Node→HUnit→Region→RegionWide escalation logic
│   ├── planning_types.rs               # PlanTemplate, StepTemplate structures
│   └── planning_proptests.rs           # Property tests: plan validity, escalation termination
│
├── execution/                          # Plan execution (step-by-step)
│   ├── mod.rs
│   ├── plan_executor.rs                # Executes RecoveryPlan steps, advances phase machine
│   ├── step_runner.rs                  # Runs a single RecoveryAction step
│   ├── retry_manager.rs                # Bounded retry with exponential backoff + jitter
│   ├── execution_types.rs              # ExecutionContext, StepResult structures
│   └── execution_proptests.rs          # Property tests: idempotence, retry bounds
│
├── replica_copy/                       # Replica-based recovery (copy replica / restore redundancy)
│   ├── mod.rs
│   ├── replica_copier.rs               # Copy full dataset state donor→replacement
│   ├── donor_selector.rs               # Select healthy donor by capacity + failure domain
│   ├── redundancy_restorer.rs          # Re-establish replication factor on healthy nodes
│   ├── replica_copy_types.rs           # CopyJob, TransferProgress structures
│   └── replica_copy_proptests.rs       # Property tests: copy integrity, donor capacity
│
├── erasure/                            # Erasure-code reconstruction (k-of-n fragment repair)
│   ├── mod.rs
│   ├── fragment_reconstructor.rs       # Reconstruct lost fragments from surviving k-of-n
│   ├── helper_validator.rs             # Fragment hash pre-check (BLAKE3) before transfer
│   ├── erasure_params.rs               # k-of-n parameters, failure tolerance n−k
│   ├── erasure_types.rs                # FragmentRepairJob, HelperResponse structures
│   └── erasure_proptests.rs            # Property tests: reconstruction correctness, tolerance bounds
│
├── ownership/                          # Ownership-aware recovery (transfer ownership)
│   ├── mod.rs
│   ├── ownership_transfer.rs           # Reassign dataset leadership to healthy replica
│   ├── fencing_manager.rs              # Fencing epoch increment, split-brain prevention
│   ├── catch_up_checker.rs             # Verify target caught up to current OrderingPos
│   ├── ownership_types.rs              # TransferJob, OwnershipRecord structures
│   └── ownership_proptests.rs          # Property tests: transfer safety, fencing monotonicity
│
├── verification/                       # Recovery verification (proofs + checksums)
│   ├── mod.rs
│   ├── integrity_verifier.rs           # BLAKE3/SHA-256 checksum verification of recovered data
│   ├── recovery_proof_issuer.rs        # Issue RecoveryProof on successful verification
│   ├── anomaly_detector.rs             # Detect ByteMismatch, ChecksumFailure, StateCorruption, ReplicaDrift
│   ├── verification_types.rs           # IntegrityResult, VerificationRecord structures
│   └── verification_proptests.rs       # Property tests: proof soundness, anomaly detection
│
├── quarantine/                         # Quarantine and release of suspect scopes
│   ├── mod.rs
│   ├── quarantine_manager.rs           # Isolate/release node/hunit/region scopes
│   ├── quarantine_policy.rs            # Release conditions (VerifyIntegrity required)
│   ├── quarantine_types.rs             # QuarantineRecord, QuarantineReason structures
│   └── quarantine_proptests.rs         # Property tests: release safety, exclusion bounds
│
├── capacity/                           # Capacity-bounded recovery (throttling)
│   ├── mod.rs
│   ├── recovery_resource_manager.rs    # Bound concurrent active recoveries
│   ├── capacity_budget.rs              # Per-recovery capacity requirements vs donor Capacity
│   ├── backoff_scheduler.rs            # Bounded exponential backoff with jitter
│   ├── capacity_types.rs               # ResourceAllocation, BudgetRecord structures
│   └── capacity_proptests.rs           # Property tests: budget soundness, no over-allocation
│
├── state_machine/                      # Multi-level recovery state machine
│   ├── mod.rs
│   ├── recovery_coordinator.rs         # Drive RecoveryPhase per scope (Detection→…→Completion)
│   ├── phase_transition.rs             # Enforce transition ordering (can_transition)
│   ├── scope_tracker.rs                # NodeRecoveryTracker / HUnitRecoveryTracker state
│   ├── timeline.rs                     # RecoveryTimeline (failure_detected→plan_completed→…)
│   ├── state_machine_types.rs          # PhaseRecord, TransitionRecord structures
│   └── state_machine_proptests.rs      # Property tests: phase ordering, terminal states
│
├── regional/                           # Regional failure compensation + isolation
│   ├── mod.rs
│   ├── regional_recovery_coordinator.rs # Cross-region recovery initiation
│   ├── failover_manager.rs             # Fail over to surviving regions before repair
│   ├── isolation_manager.rs            # Isolate failed region from donor/recipient selection
│   ├── regional_types.rs               # RegionRecoveryRecord, CrossRegionJob structures
│   └── regional_proptests.rs           # Property tests: isolation correctness, failover bounds
│
├── carrier/                            # Carrier-agnostic transport boundary
│   ├── mod.rs
│   ├── endpoint_mapper.rs              # Map logical NodeId/HUnitId/RegionId → ids-carrier::Endpoint
│   ├── datagram_envelope.rs            # Recovery datagram envelope (seq, scope, action, payload)
│   ├── carrier_adapter.rs              # Adapter over ids-carrier::Endpoint send/receive
│   └── carrier_types.rs                # EnvelopeId, DatagramMeta structures
│
├── crypto/                             # Allowed-crypto usage boundary
│   ├── mod.rs
│   ├── recovery_signer.rs              # ed25519 sign/verify for recovery proofs
│   ├── checksummer.rs                  # BLAKE3/SHA-256 checksum for fragments and state
│   ├── encryptor.rs                    # Optional XChaCha20-Poly1305/ChaCha20-Poly1305 payload encryption
│   ├── crypto_types.rs                 # ProofSignature, ChecksumRecord structures
│   └── crypto_proptests.rs             # Property tests: signature unforgeability, checksum collision resistance
│
├── types/                              # Composed recovery types (maps, reports)
│   ├── mod.rs
│   ├── recovery_report.rs              # Per-dataset recovery summary report
│   ├── redundancy_report.rs            # Redundancy restoration status per dataset
│   ├── health_rollup.rs                # Aggregate health across nodes/hunits/regions
│   └── types_proptests.rs              # Property tests: report consistency
│
├── fuzz/                               # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_detection.rs
│   ├── fuzz_planning.rs
│   ├── fuzz_execution.rs
│   ├── fuzz_replica_copy.rs
│   ├── fuzz_erasure.rs
│   └── fuzz_verification.rs
│
├── tests/                              # Integration & adversarial tests
│   ├── mod.rs
│   ├── detection_tests.rs              # Heartbeat timeout + failure classification
│   ├── planning_tests.rs               # Plan construction + escalation
│   ├── execution_tests.rs              # Step execution + retry
│   ├── replica_copy_tests.rs           # Donor selection + copy integrity
│   ├── erasure_tests.rs                # Fragment reconstruction + helper validation
│   ├── ownership_tests.rs              # Ownership transfer + fencing
│   ├── verification_tests.rs           # Checksum verification + proof issuance
│   ├── quarantine_tests.rs             # Quarantine + release
│   ├── capacity_tests.rs               # Resource bounds + backoff
│   ├── state_machine_tests.rs          # Phase transitions + timeline
│   ├── regional_tests.rs               # Failover + isolation
│   ├── carrier_tests.rs                # Datagram envelope + endpoint mapping
│   ├── crypto_tests.rs                 # Sign/verify + checksum + encrypt
│   ├── adversarial_tests.rs            # Donor failure mid-rebuild, corrupt replica, region-wide failure
│   ├── chaos_tests.rs                  # Node/HUnit/Region churn during recovery
│   ├── property_tests.rs               # Proptest properties across subsystems
│   └── replay_tests.rs                 # Recovery replay determinism
│
├── benches/                            # Benchmarks
│   ├── mod.rs
│   ├── detection_bench.rs
│   ├── planning_bench.rs
│   ├── replica_copy_bench.rs           # Copy bandwidth, time per GB
│   ├── erasure_bench.rs                # Reconstruction bandwidth, helper count
│   ├── verification_bench.rs           # Checksum + proof throughput
│   ├── state_machine_bench.rs          # Phase transition latency
│   └── capacity_bench.rs               # Throughput under concurrency bounds
│
├── spec/                               # Normative specifications
│   ├── recovery_spec.md                # Recovery lifecycle spec
│   ├── scope_spec.md                   # Node/HUnit/Region/RegionWide scope model
│   ├── strategy_spec.md                # Repair strategy selection spec
│   ├── verification_spec.md            # Proof + checksum verification spec
│   ├── threat_model.md                 # Threat model document
│   └── state_machine_spec.md           # Multi-level recovery state machine spec
│
├── docs/                               # Documentation
│   ├── architecture.md                 # Recovery architecture & layering
│   ├── integration_guide.md            # Integrating with ids-000-aa/ids-carrier/ids-crypto
│   ├── operations.md                   # Operational runbooks (quarantine release, failover)
│   ├── design_rationale.md             # Why separate crate, multi-level scope model
│   └── api_docs.md                     # Public API documentation
│
└── test_vectors/                       # Deterministic test vectors
    ├── recovery_plan_vectors.json
    ├── erasure_repair_vectors.json
    ├── checksum_vectors.json
    └── phase_transition_vectors.json
```

**Why ~90–150 files:** each module has a single responsibility; detection, planning, execution, replica copy, erasure, ownership, verification, quarantine, capacity, state machine, regional, carrier, crypto, types, plus fuzz, tests, benches, spec, docs, and test vectors. All files contain production-grade substance with error handling, documentation, and tests. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Multi-Level Recovery State Machine
```
RecoveryPhase: Detection(0) → Planning(1) → Execution(2) → Verification(3) → Completion(4)
can_transition(from, to) = from.order_index() < to.order_index() || from == to
Per active scope: RecoveryCoordinator drives phase transitions; RecoveryTimeline records each event.
```

#### 7.2 Failure Detection
```
record_heartbeat(scope_id, at_ms) → store last heartbeat
detect_failures(now_ms) → for each scope: if now_ms - last_hb > timeout_ms → FailureEvent
FailureEventType: NodeFailure | HUnitFailure | RegionalFailure | NetworkPartition | DiskFailure | PowerFailure
Default heartbeat_timeout_ms: 30000 (from ids-000-aa FailureDetector::default)
```

#### 7.3 Repair Strategy Selection by Scope
```
Node     → CopyReplica      (priority 1)
HUnit    → RestoreRedundancy (priority 2)
Region   → ReconstructErasure (priority 3)
RegionWide → TransferOwnership (priority 4)
Escalation: if lower-level repair yields InsufficientReplicas, escalate to next scope level.
```

#### 7.4 Erasure-Code Reconstruction (k-of-n)
```
Given n fragments, any k suffice to reconstruct.
Failure tolerance = n − k simultaneous losses.
Helper validation: each helper sends BLAKE3(fragment_hash) first; mismatch → exclude helper.
Reconstruct from k valid helpers; write to replacement node; verify BLAKE3(content_hash) matches original.
```

#### 7.5 Capacity-Bounded Recovery
```
acquire(plan_id) → reject if active_count >= resource_limit
donor_selection(candidate) → candidate.Capacity.can_serve(required) && !candidate.quarantined
backoff(delay) = base * multiplier^attempt, clamped to max_delay_ms, plus jitter
```

#### 7.6 Recovery Verification
```
checksum = BLAKE3(data) or SHA-256(data)
VerifyIntegrity: recompute checksum over recovered data; compare to recorded content_hash
match → issue RecoveryProof { dataset_id, recovered_fragments, checksum, ids_time }
mismatch → Anomaly (ByteMismatch/ChecksumFailure/StateCorruption/ReplicaDrift) → quarantine
Plan reaches Completion only after VerifyIntegrity passes.
```

#### 7.7 Ownership Transfer with Fencing
```
target must be caught up to current OrderingPos (target.pos >= current.pos)
fencing_epoch += 1; record fencing_epoch in transfer
no global master — transfer is per-dataset, coordinated via RecoveryCoordinator
```

---

### 8. Dependencies

| Crate | Purpose ||
|-------|---------|---------|
| `ids-000-aa` | Canonical types + sealed `trait Recovery` contract + error types |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — all recovery traffic |
| `ids-crypto` | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-membership` | Node/HUnit/Region roster, failure-domain placement |
| `ids-placement` | `PlacementDecision` for replica placement |
| `ids-write` | Committed op source (`OrderingPos`, `OpRef`) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Content checksums |
| `sha2` | SHA-256 checksums |
| `ed25519-dalek` | Recovery proof signatures |
| `chacha20poly1305` / `xchacha20poly1305` | Optional payload encryption |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-recovery` is carrier-agnostic over `ids-carrier::Endpoint` opaque bytes.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|---|-----------|-------------|
| AC-025.1 | Node failure recovery | Single node failure → CopyReplica restores replica; verify via IntegrityResult.verified == true |
| AC-025.2 | H-unit failure compensation | HUnit failure → RestoreRedundancy places replicas in other HUnits; placement satisfies failure_domain |
| AC-025.3 | Cross-H-unit recovery | Intra-region capacity insufficient → escalate to Region scope; cross-region placement |
| AC-025.4 | Regional failure compensation | Region failure → fail over to surviving regions before repair; regional isolation excludes failed region |
| AC-025.5 | Regional failure isolation | Failed region's nodes never selected as donors/recipients during regional recovery |
| AC-025.6 | Replica-based recovery | CopyReplica from healthy donor → target state matches donor (checksum equality) |
| AC-025.7 | Erasure-code reconstruction | k-of-n fragment reconstruction tolerates up to n−k helper failures; output checksum matches original |
| AC-025.8 | Capacity-bounded recovery | Concurrent active recoveries never exceed resource_limit; excess queued/rejected |
| AC-025.9 | Ownership-aware recovery | TransferOwnership only to target caught up to current OrderingPos; fencing_epoch incremented |
| AC-025.10 | Recovery verification | RecoveryProof issued only when BLAKE3/SHA-256 checksum matches; mismatch → Anomaly + quarantine |
| AC-025.11 | Redundancy restoration | Post-recovery, dataset replication factor restored to target; verify via redundancy report |
| AC-025.12 | Multi-level recovery state machine | Phase transitions strictly ordered Detection→Planning→Execution→Verification→Completion; no backward transitions |
| AC-025.13 | No timeout-based eviction | Quarantine/release is deliberate; no automatic eviction after heartbeat timeout |
| AC-025.14 | Carrier agnosticism | Zero direct dependencies on std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, mesh; all traffic via ids-carrier::Endpoint |
| AC-025.15 | Allowed crypto only | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 used; no other algorithms |
| AC-025.16 | File structure | ~90–150 files, all production-grade, organized in subfolders, no stubs |
| AC-025.17 | Property tests | proptest/bolero properties across all subsystems; 100k+ iterations |
| AC-025.18 | Adversarial tests | Donor failure mid-rebuild, corrupt replica donor, region-wide failure — all pass/fail defined |

---

### 10. Remaining Uncertainties

1. **Region-wide recovery scope:** CRATES.md and prompts mention "region-wide failure recovery" but `ids-000-aa` `RecoveryScope` only has Node/HUnit/Region. Need to decide whether RegionWide is a virtual scope handled by `RegionalRecoveryCoordinator` or a fourth enum variant added to `ids-000-aa` (contract change, requires master-crate coordination).
2. **Recovery heartbeat transport:** Heartbeats are recorded via `FailureDetector` but the transport mechanism (who sends heartbeats, over which `ids-carrier::Endpoint`) is not specified in `ids-000-aa`; must be designed without assuming any carrier semantics.
3. **Erasure-code library:** No erasure-coding crate is currently in the IDS dependency set; selection of a Reed-Solomon/regenerating-code implementation (pure Rust, no networking) is pending.
4. **Checkpoint integration:** `ids-snapshot` (Phase 026) will provide checkpoints as recovery restore points; the exact restore-from-checkpoint flow in `ids-recovery` is undefined until Phase 026 is planned.
5. **Cross-region failover trigger:** What triggers regional failover vs. in-region repair (threshold of failed nodes in a region, or region health == Critical/Failed) is a product decision.
6. **Recovery proof format:** `RecoveryProof` exists in `ids-000-aa` but the checksum algorithm (BLAKE3 vs SHA-256 vs both) and fragment inclusion rules need final specification.
7. **Capacity accounting:** How `Capacity` is measured per node (storage vs memory vs CPU vs network_bps) and refreshed during recovery is not yet defined.
8. **Quarantine release policy:** Release requires successful `VerifyIntegrity`, but who authorizes release (automatic vs manual) and appeal windows are policy decisions.

---

### 11. Engineering Decisions Summary

| Decision | Rationale ||
|----------|-----------|-------------|
| Separate `ids-recovery` crate | Keeps recovery as a distinct concern; `ids-000-aa` holds the sealed `trait Recovery` contract; implementation lives downstream |
| Multi-level scope model (Node/HUnit/Region/RegionWide) | Matches CRATES.md §25 and prompts.txt entry 25; hierarchical fault domains limit blast radius (AWS cell-based, Google Cloud failure domains) |
| Repair priority: CopyReplica → RestoreRedundancy → ReconstructErasure → TransferOwnership | Follows `RepairPlanner` in `ids-000-aa`; cheapest first, escalating cost/scope |
| Capacity-bounded recovery (`RecoveryResourceManager`) | Prevents rebuild storms; bounded backoff with jitter (SRE throttling pattern) |
| No timeout-based eviction | Eviction is deliberate, never automatic; quarantine requires deliberate release after `VerifyIntegrity` |
| Verification before Completion | `VerifyIntegrity` mandatory; `RecoveryProof` binds `ids_time` + content checksum; prevents zombie/resurrected data |
| Erasure-code helper hash pre-check | BLAKE3 fragment hash before full transfer; robust repair tolerates erroneous helpers |
| Carrier-agnostic over `ids-carrier::Endpoint` | Preserves IDS carrier-agnostic stance; no TCP/IP/UDP/QUIC/HTTP/DNS/libp2p |
| Allowed crypto only: ed25519/sha256/blake3/xchacha20poly1305/chacha20poly1305 | Matches IDS crypto policy (implemented in `ids-crypto`) |
| `ids-000-aa` as master crate | All canonical types and sealed trait contracts defined once; `ids-recovery` imports, never redefines |

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 025.

**Sources consulted (30 targeted searches):**
- ACM "A case for two-level distributed recovery schemes" (dl.acm.org/doi/10.1145/223586.223596) — multi-level recovery, common case fast
- Cornell DSN "Reliable, Efficient Recovery for Complex Services with Replicated Subsystems" (doi.org/10.1109/dsn48063.2020.00035) — Derecho view changes, epoch termination, restart quorum
- AWS Well-Architected "Reducing the Scope of Impact with Cell-Based Architecture" — fault isolated boundaries, bulkheads
- Google Cloud "Building blocks of reliability" — regions/zones as failure domains, independent failure domains
- Cornell CS514 "Gossip Style Failure Detection" (cs.cornell.edu) — scalable gossip failure detection, resilient to message loss
- arxiv:2504.14802 — ReCraft: wait-free Raft membership changes, quorum overlap
- RabbitMQ Khepri failure recovery — Raft-driven recovery, rejoining node catches up from common log point
- Redpanda Raft group reconfiguration — learners, add-then-remove ordering, quorum maintenance
- IEEE DSN 2014 "Replication-Based Fault-Tolerance for Large-Scale Graph Processing" — checkpoint-based recovery
- arxiv:2602.08257 — HEAL: online incremental recovery for leaderless distributed systems
- arxiv:10.1145/3241062 — Protocol-Aware Recovery for Consensus-Based Distributed Storage (Par)
- IEEE "Exact minimum-repair-bandwidth cooperative regenerating codes" — regenerating codes, repair bandwidth minimization
- arxiv:2606.05573 — Robust Repair of Reed-Solomon Codes under erroneous helper responses
- patents.google US08156368B2 — Rebuilding lost data via recovery strip sets, parity/ data owner nodes
- SRE Google "Handling Overload" / "Load Balancing with Client Side Throttling" — throttling, per-caller quotas, adaptive throttling
- arxiv:2504.14802 / ReCraft — membership change, subcluster-granularity operations
- IBM Cloud VPC HA/DR — regional failure, zone failure, control plane/data plane separation
- Azure Reliability Resource Manager — zone failures, region-wide failures, backup region failover
- IBM Storage Scale failover/failback — active/active site recovery, cluster configuration migration
- Spanner Failover and Resiliency Whitepaper — independent failure domains, separate Paxos quorums
- BLAKE3 content-addressed verification (snix.dev castore, decdn.org) — content addressing, verified streaming
- XChaCha20-Poly1305 / ChaCha20-Poly1305 Rust crates (docs.rs) — AEAD encryption primitives
- ed25519-dalek / FROST-Ed25519 — signature primitives for authentication
- Transport-layer agnosticism (qu3ry.net memory-native protocol) — semantic layer vs carriage layer separation over opaque bytes
- Merkle tree anti-entropy (scalecube-prism, Cassandra, Dynamo) — digest-driven divergence detection, localized repair
- CRDT convergence / SEC (Shapiro et al. 2011, plan-077 sources) — state convergence after recovery
- HX-241 plan formats: plan-077.md (hx-crdt), plan-080.md (hx-sync-security), plan-081.md (hx-sync-experiments) — plan structure template
- IDS CRATES.md §25, prompts.txt entry 25 (lines 370-384), ids-000-aa/src/contracts/recovery.rs, ids-000-aa/src/types.rs — canonical types and sealed trait contracts
