## Phase 039 — Create the ids-selfreorg crate: Revise H-unit membership, placement, replicas in response to changes.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 580-594)

```
39. Research only do not read plan.md **[Phase 039 — Create the ids-selfreorg crate: Revise H-unit membership, placement, replicas in response to changes.]**
```
Crate: `ids-selfreorg`
Purpose: Revise H-unit membership, placement, replicas in response to changes.
Contents: H-unit membership revision, placement revision, replica/fragment revision, recovery assignment revision, regional responsibility revision, invariant preservation during reorganisation, topology change handling.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 039 — Create the ids-selfreorg crate: Revise H-unit membership, placement, replicas in response to changes.]**

Create plan-039.md in C:/idfs/plans/ for the ids-selfreorg crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.

---

### 1. Executive Summary

Phase 039 creates `ids-selfreorg`, the IDS crate for infrastructure self-reorganisation: revising H-unit membership, placement, replicas, fragments, recovery assignments, and regional responsibilities in response to topology, workload, and membership changes while preserving defined invariants. It owns H-unit membership revision (add/remove/retire H-units and member nodes), placement revision (recompute which H-unit/region holds each dataset/partition/replica), replica/fragment revision (adjust replica counts, fragment groups, redundancy mode), recovery assignment revision (reassign recovery owners and plans after failures), regional responsibility revision (move responsibilities between regions), invariant preservation during reorganisation (quorum overlap, redundancy floor, durability conditions, no-split-brain), and topology change handling (node join/leave, H-unit formation/retirement, region split/merge, link degradation).

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-selfreorg` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

`ids-selfreorg` is a skeleton crate that does NOT yet exist at `C:/idfs/ids-selfreorg/`. `ids-000-aa` already declares canonical types `RegionId`, `HUnitId`, `NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `PlacementDecision`, `RedundancyMode`, `FailureDomain`, `FailureEvent`, `RecoveryScope`, `RecoveryPlanId`, `RecoveryStepId`, `Endpoint`, `CarrierType`, `HUnitState`, `HUnitInfo`, `RegionInfo`, `NodeInfo`, `OpState`, `CommitState`, `DurabilityCondition`, `Capacity`, `FailureEventType`, `RecoveryAction`, `RepairStrategy`, plus the sealed `trait Discovery` (`src/contracts/discovery.rs`) and `trait Recovery` (`src/contracts/recovery.rs`) — all referenced by this crate but never redefined. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Raft joint consensus membership changes (etcd/raft, openraft, ReCraft), quorum overlap and Add/RemoveServer vs joint configuration (Raft paper §4.3), consistent hashing / virtual nodes / token ownership transfer (fogfish/ring, Dynamo), SWIM failure detection and membership dissemination (Cornell Quicksilver), Akka cluster membership via gossip CRDT (leader determinism after convergence), Hadoop rack-aware replica placement and re-replication, Cassandra NetworkTopologyStrategy + virtual nodes + hinted handoff + anti-entropy Merkle repair, Aerospike dynamic replication factor and recluster, CockroachDB replication zones + locality-aware rebalancing + joint consensus for region failure, TigerGraph cluster repartition/replication factor changes, gluster volume rebalance on expand/shrink, anti-entropy Merkle tree reconciliation (Cassandra/Dynamo/Scalecube), joint consensus split-brain prevention (CockroachDB blog), zone-aware replica placement and impact-free rebalancing (LinkedIn Pinot), adaptive replica placement with load skew (Wiley concurrency), SWARM+ dynamic quorum q(t) from live membership, GeoLayer geo-distributed replica placement, RMP membership views for group reformation, HyParView gossip-based broadcast membership, Kubernetes pod topology spread constraints across failure domains, self-healing and replica replacement, hinted handoff for temporary node failure, grace period member removal, load-balanced replica placement and hotspot avoidance, cross-datacenter replica placement latency, and distributed invariant synthesis via IC3.

---

### 2. Architecture Overview

`ids-selfreorg` sits above `ids-000-aa` (types + sealed `trait Discovery`/`trait Recovery` contracts + invariants + versioning + errors) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It is layered so each self-reorganisation concern — membership revision, placement revision, replica/fragment revision, recovery assignment revision, regional responsibility revision, invariant preservation, topology change handling — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-* LAYER (self-reorg consumers)          │
│  ids-store, ids-replication, ids-mobility, ids-recovery — consume decisions │
├─────────────────────────────────────────────────────────────────────────────┤
│                Phase 039: ids-selfreorg (THIS CRATE)                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │
│  │ membership   │ │ placement    │ │ replica/     │ │ recovery     │    │ │
│  │ revision     │ │ revision     │ │ fragment     │ │ assignment   │    │ │
│  │ (add/remove/ │ │ (recompute   │ │ revision     │ │ revision     │    │ │
│  │  retire HU)  │ │  target HU)  │ │              │ │              │    │ │
│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │
│  │ regional     │ │ invariant    │ │ topology     │ │ decision     │    │ │
│  │ responsibility│ │ preservation │ │ change       │ │ recorder     │    │ │
│  │ revision     │ │ (quorum/     │ │ handling     │ │ (epoch + log)│    │ │
│  │              │ │  redundancy  │ │              │ │              │    │ │
│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │
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
- One canonical definition per type; canonical types (`RegionId`, `HUnitId`, `NodeId`, `DatasetId`, `PartitionId`, `PlacementDecision`, `RedundancyMode`, `FailureDomain`, `FailureEvent`, `RecoveryScope`, `RecoveryPlanId`, `RecoveryStepId`, `Endpoint`, `CarrierType`, `HUnitState`, `HUnitInfo`, `RegionInfo`, `NodeInfo`, `OpState`, `CommitState`, `DurabilityCondition`, `Capacity`, `FailureEventType`, `RecoveryAction`, `RepairStrategy`) imported from `ids_000_aa::X`, never redefined.
- `ids-selfreorg` implements the sealed `trait SelfReorg` contract from `ids-000-aa/src/contracts/` (membership, placement, replica, recovery-assignment, regional-responsibility revision) as declared in the master crate; downstream crates use it as a trait bound, cannot implement it.
- All reorganisation metadata (membership deltas, placement decisions, replica maps, recovery assignments) transported as opaque bytes via `ids-carrier::Endpoint`. No IP addresses, ports, sockets.
- Crypto surface: sha256/blake3 for membership/placement decision hashes and integrity; ed25519 (optional) for signing reorganisation decisions/proofs; xchacha20poly1305/chacha20poly1305 only if decision payloads need encryption (this crate does not encrypt by default). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Reorganisation is dataset/region-scoped; no global reorganisation master (invariant `no_global_master` in `ids-000-aa`).
- Operation lifecycle reuses `ids-000-aa` `OpState` and `CommitState`; reorganisation findings are raised through `ids-000-aa` error/annex channels.
- Logical identity remains stable across physical placement changes; metadata updated through controlled transition with epoch versions.

---

### 3. Detailed Research Dimensions

#### 3.1 H-Unit Membership Revision

H-units are intermediate subclusters between regions and nodes (IDS hierarchy: Region → regional database cluster → H-unit → node). Membership revision covers adding a node to an H-unit, creating a new H-unit, removing an H-unit, and changing H-unit state (`Forming` → `Operational` → `Degraded` → `Compensating` → `Retired` per `ids-000-aa` `HUnitState`). Research basis:
- Raft Add/RemoveServer (single-member change, quorum overlap preserved) vs joint consensus (multi-member change, two-phase) — CRaft/ReCraft generalize to arbitrary membership deltas while maintaining quorum overlap between consecutive configurations.
- SWIM failure detection: randomized peer-to-peer probing separates failure detection from membership dissemination; large-scale process groups with sub-linear detection time.
- Akka cluster membership: CRDT-based gossip state with monotonic merge; leader determined deterministically after gossip convergence; member status (joining/up/leaving/exiting/removed/down) transitions.
- H-unit capacity (`Capacity` type in ids-000-aa) constrains admission: a node joins an H-unit only if residual capacity admits it; capacity advertisement via discovery.

#### 3.2 Placement Revision

Placement revision recomputes which region/H-unit/node holds each dataset, partition, replica, or fragment when membership, capacity, or topology changes. Research basis:
- Consistent hashing with virtual nodes (tokens): each node claims ~Q/N shards of hash space; join "steals" shards consistently; leave redistributes shards to remaining nodes; deterministic so all nodes compute the same partition map; minimizes movement on view changes.
- Aerospike partition assignment: replication list = permutation of cluster succession list; first node = master, subsequent = replicas; objectives: deterministic, uniform distribution, minimized partition movement on view change.
- HDFS rack-aware placement: replicas spread across racks for fault tolerance; one replica local rack, others remote racks; re-replication when factor drops below target; block placement policy prefers same-rack writer then remote rack.
- Cassandra NetworkTopologyStrategy + virtual nodes: per-datacenter independent replica placement; vnodes automatically rebalance token ranges on join/leave/failure; dead node rebuild faster due to vnode granularity.
- CockroachDB replication zones + locality-aware rebalancing: replica allocator maintains configured replica count per range; locality or placement constraints can keep counts uneven to preserve survivability across zones; load-based rebalancing coexists with load-based splitting.
- Zone-aware replica placement and impact-free rebalancing (Pinot): mirror server sets across replica groups; placement repair preserves mirror sets, accommodates replacement nodes from arbitrary racks.

#### 3.3 Replica/Fragment Revision

Replica/fragment revision adjusts replica counts, fragment groups, redundancy mode (`FullReplica` / `ErasureCoded{k,n}` / `Hybrid` per ids-000-aa `RedundancyMode`), and fragment placement across failure domains. Research basis:
- Dynamic replication factor changes: Aerospike AP namespaces allow dynamic replication factor with `recluster`; SC mode requires quorum; TigerGraph repartition changes replication factor without adding/removing nodes; ScyllaDB cloud resize adds/removes nodes in multiples of replication factor.
- Erasure-coded fragment integrity: per-fragment checksums; EC scrub verifies cold parity shards before rebuild (SeaweedFS EC Bitrot Scrub); Reed-Solomon repair bandwidth optimization.
- Replica placement constraints: CockroachDB replica placement constraints and zones keep replica counts uneven where needed for survivability; locality-aware placement trades some load balance for failure-domain diversity.
- Hinted handoff (Cassandra): temporary node failure — missed writes stored by coordinator with target ID, replayed on recovery; anti-entropy Merkle tree repair as backstop for divergent replicas.

#### 3.4 Recovery Assignment Revision

Recovery assignment revision reassigns recovery owners and plans after failures (node/H-unit/region). Research basis:
- `trait Recovery` sealed contract in ids-000-aa: `detect_failures`, `create_plan`, `execute_plan`, `quarantine`, `restore_redundancy`, `reconcile_state`; `RepairStrategy` enum (CopyReplica, RestoreRedundancy, ReconstructErasure, TransferOwnership) with priority ordering; `RecoveryScope` (Node/HUnit/Region); `RecoveryPlan` with steps and status; `FailureEvent` typed by `FailureEventType`.
- Regional recovery coordinator: cross-region recovery, regional failure counting, region health tracking.
- Repair planner selects strategy by scope level: node→CopyReplica, H-unit→RestoreRedundancy, region→ReconstructErasure; resource-bounded active recoveries.
- Revision triggers recovery re-planning when placement changes invalidate current owners: new owner assumes in-flight recovery plans, quarantine releases restore redundancy.

#### 3.5 Regional Responsibility Revision

Regional responsibility revision moves responsibilities (dataset ownership, replica mastership, recovery coordination) between regions. Research basis:
- CockroachDB joint consensus for region failure: intermediate joint configuration requires agreement of both old and new configs, preventing split-brain during region outages; survives individual region outage.
- ReCraft split/merge: subclusters split into disjoint members and data ranges; joint configuration C_joint requires quorums from each subcluster; merge via 2PC among subclusters.
- ids-mobility (crate 38): preferred/authoritative region changes, logical identity preservation, controlled handoff between old/new owners, cutover verification — ids-selfreorg consumes mobility handoff outputs.
- Kubernetes topology spread constraints: spread pods across failure domains (regions, zones, nodes) with maxSkew/minDomains; ids-selfreorg's regional responsibility revision is the storage-layer analog.

#### 3.6 Invariant Preservation During Reorganisation

Invariant preservation ensures safety properties hold throughout reorganisation steps. Research basis:
- Quorum overlap property: Raft joint consensus maintains quorum overlap between C_old and C_new (AR-RPC one-node difference; JC two-phase). Generalized Raft reconfiguration (P2' proof) underlies ReCraft's wait-free multi-node changes.
- OpenRaft adaptive config change: skips joint consensus when old/new quorums already overlap (single entry); otherwise joint → uniform (two entries).
- Invariant synthesis via IC3 (arxiv 2605.24619): inductive invariants for distributed protocols incl. dynamic Raft reconfiguration; verifies safety across reconfiguration transitions.
- Grace period member removal: synchrony assumptions bound how long a removed member can still act; stale-metadata windows guarded by epoch versions.
- Durability conditions: `DurabilityCondition` in ids-000-aa gates replica/fragment changes; redundancy floor never breached mid-revision.

#### 3.7 Topology Change Handling

Topology change handling covers node join/leave, H-unit formation/retirement, region split/merge, link degradation, and carrier changes. Research basis:
- Fault-tolerant active rings for structured P2P overlays: neighbor pointers updated immediately on join/leave; active topology maintenance (Li/Misra/Plaxton) ensures immediate consistency of neighbor links.
- SWARM+ hierarchical multi-agent consensus: topology descriptor specifies hierarchy level, group id, peer set, parent reference; quorum q(t) = ceil((n_live(t)+1)/2) adapts dynamically without explicit reconfiguration.
- HyParView: active view + passive view; gossip-based broadcast; passive view promoted on active view failure; peer-to-peer periodic randomized probing.
- Gluster volume rebalance: expand/shrink volumes online; rebalance migrates data to new bricks; remove-brick triggers automatic rebalance.
- Cross-datacenter replica placement latency optimization: WAN-limited bandwidth trades replication for locality; GeoLayer layered graph for low-latency geo-distributed placement.
- Self-healing (Kubernetes): automatic replacement of failed pods/rescheduling on node unavailability; desired-state reconciliation.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Node/H-unit/region failure mid-revision | Epoch-versioned decisions; recovery assignment revision re-plans on failure; `trait Recovery` quarantine + restore_redundancy |
| Split brain during membership change | Quorum overlap (joint consensus when quorums don't overlap); single-entry transition when overlap exists; `ids-000-aa` sealed trait enforces at contract layer |
| Redundancy floor breached (too few replicas during revision) | `DurabilityCondition` gating; redundancy floor check before every revision step; abort if below floor |
| Stale topology/membership views | Gossip sequence numbers + timestamps; epoch versions on placement decisions; stale entries expired via `DiscoveryExpiration` |
| Replica/fragment data loss during movement | Reconstruct from redundant copies or erasure fragments before removing source; verify checksum (ids-integrity) before rewrite |
| Capacity overcommit on H-unit join | `Capacity` check against `HUnitInfo.available_capacity()`; admission control rejects over-full H-units |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; CI grep for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Revision storms / oscillation | Decision recorder with epoch log; cooldown windows; policy-driven thresholds; concurrent-request serialization |
| Reorganisation without observable completion | `RecoveryPlanId`-scoped plan tracking; completion conditions exposed; completion condition verified before next revision |
| Logical identity instability across physical moves | Dataset/partition `RecordId`/`OrderingPos` unchanged; only physical location (RegionId/HUnitId/NodeId) updated via `PlacementDecision` |
| Memory growth of decision/epoch logs | Bounded epoch log with TTL eviction; periodic compaction |

---

### 5. Integration with Existing Crates

Phase 039 depends on and integrates with:

| Crate | Role in Phase 039 |
|-------|-------------------|
| `ids-000-aa` (master) | Canonical types (`RegionId`, `HUnitId`, `NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `PlacementDecision`, `RedundancyMode`, `FailureDomain`, `FailureEvent`, `RecoveryScope`, `RecoveryPlanId`, `RecoveryStepId`, `Endpoint`, `CarrierType`, `HUnitState`, `HUnitInfo`, `RegionInfo`, `NodeInfo`, `OpState`, `CommitState`, `DurabilityCondition`, `Capacity`, `FailureEventType`, `RecoveryAction`, `RepairStrategy`), sealed `trait Discovery`, sealed `trait Recovery`, sealed `trait SelfReorg` (to be declared in master), invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits`), unified `Error` |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤1024); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-selfreorg` |
| `ids-crypto` (Phase 003) | sha256/blake3 (decision/placement hashes), ed25519 (signed reorganisation decisions), xchacha20poly1305/chacha20poly1305 (optional encryption); consumed only |
| `ids-discovery` (Phase 011) | H-unit/node/region discovery (`discover_hunits`, `discover_nodes`, `discover_regions`, `resolve_node`, `gossip_to`); feeds membership/placement inputs |
| `ids-recovery` (Phase 021) | Recovery plans, failure detection, replica reconstruction, quarantine — recovery assignment revision consumes and revises its plans |
| `ids-membership` (crate 8) | Node/H-unit/region membership and lifecycle (states, admission, capacity, removal) — membership revision consumes its state machine |
| `ids-placement` (crate 9) | Partition boundaries, key mapping, split/merge, distribution across nodes/H-units/regions — placement revision consumes its maps |
| `ids-replication` (Phase 017) | Replica/fragment sources and targets for replica/fragment revision |
| `ids-mobility` (crate 38) | Region changes with controlled handoff — regional responsibility revision consumes mobility outputs |
| `ids-integrity` (Phase 030) | Checksum verification of reconstructed/moved data before rewrite |
| `ids-time` (crate 5) | `IdsTime` tick basis for revision schedules, epoch timestamps, cooldown windows |
| `ids-ordering-domain` (crate 55) | Dataset-scoped ordering; revision preserves `OrderingPos`/`Version` stability |

**Integration points:**
- `ids-selfreorg` implements `ids_000_aa::contracts::SelfReorg` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-selfreorg` never touches sockets.
- Membership/placement/replica/recovery-assignment/regional-responsibility decisions signed with ed25519 via `ids-crypto`; decision integrity via sha256/blake3 digests.
- Depends on `ids-000-aa` only for types/contracts; Cargo workspace: `ids-selfreorg = { path = \"../ids-selfreorg\" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-selfreorg` (to be created at `C:/idfs/ids-selfreorg/`)

```
ids-selfreorg/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-discovery,
│                                       # ids-recovery, ids-membership, ids-placement, ids-replication,
│                                       # ids-mobility, ids-integrity, ids-time, ids-ordering-domain;
│                                       # thiserror, serde; no networking crates; allowed crypto only
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # SelfReorgDecision, RevisionEpoch, RevisionScope, RevisionType,
│   │                                   # PlacementRevision, MembershipDelta, ReplicaFragmentRevision,
│   │                                   # RecoveryAssignmentRevision, RegionalResponsibilityRevision
│   ├── errors.rs                       # SelfReorgError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::SelfReorg (sealed trait)
│   │
│   ├── membership/                     # H-unit membership revision (~9 files)
│   │   ├── mod.rs
│   │   ├── membership_delta.rs         # add/remove/retire node/H-unit deltas
│   │   ├── hunit_lifecycle.rs          # Forming→Operational→Degraded→Compensating→Retired
│   │   ├── admission.rs                # capacity-aware admission control
│   │   ├── capacity_check.rs           # residual capacity vs Capacity type
│   │   ├── membership_validator.rs     # validates delta against invariants
│   │   ├── membership_messages.rs      # datagram types for membership gossip
│   │   ├── membership_layer.rs         # composition over Endpoint
│   │   └── membership_scheduler.rs     # serializes concurrent membership changes
│   │
│   ├── placement/                      # Placement revision (~10 files)
│   │   ├── mod.rs
│   │   ├── placement_revision.rs       # recompute target H-region/H-unit/node
│   │   ├── consistent_hash.rs          # consistent hashing + virtual nodes/tokens
│   │   ├── token_assignment.rs         # token→shard ownership; steal on join, redistribute on leave
│   │   ├── partition_map.rs            # deterministic partition map (all nodes agree)
│   │   ├── rack_aware.rs               # rack/failure-domain-aware placement (HDFS-style)
│   │   ├── zone_placement.rs           # locality-aware zone placement (CockroachDB-style)
│   │   ├── movement_minimizer.rs       # minimizes partition movement on view change (Aerospike-style)
│   │   ├── placement_messages.rs       # datagram types
│   │   └── placement_layer.rs          # composition over Endpoint
│   │
│   ├── replica_fragment/               # Replica/fragment revision (~9 files)
│   │   ├── mod.rs
│   │   ├── replica_revision.rs         # adjust replica counts, redundancy mode
│   │   ├── fragment_group.rs           # fragment groups across failure domains
│   │   ├── redundancy_mode.rs          # FullReplica / ErasureCoded{k,n} / Hybrid transitions
│   │   ├── replication_factor.rs       # dynamic replication factor changes (Aerospike/TigerGraph-style)
│   │   ├── fragment_placement.rs       # fragment placement across H-units/regions
│   │   ├── hinted_handoff.rs           # temporary failure write buffering (Cassandra-style)
│   │   ├── fragment_messages.rs        # datagram types
│   │   └── replica_fragment_layer.rs   # composition over Endpoint
│   │
│   ├── recovery_assignment/            # Recovery assignment revision (~8 files)
│   │   ├── mod.rs
│   │   ├── recovery_owner_revision.rs  # reassign recovery owners after placement change
│   │   ├── plan_revision.rs            # revise in-flight RecoveryPlan on topology change
│   │   ├── owner_transfer.rs           # transfer recovery ownership between nodes/H-units
│   │   ├── quarantine_release.rs       # release quarantine, restore redundancy
│   │   ├── assignment_messages.rs      # datagram types
│   │   └── recovery_assignment_layer.rs# composition over Endpoint
│   │
│   ├── regional_responsibility/        # Regional responsibility revision (~8 files)
│   │   ├── mod.rs
│   │   ├── responsibility_revision.rs  # move responsibilities between regions
│   │   ├── region_split_merge.rs       # region split/merge handling (ReCraft-style)
│   │   ├── joint_config.rs             # joint consensus for region transitions (quorum overlap)
│   │   ├── topology_spread.rs          # spread responsibilities across failure domains (K8s-style)
│   │   ├── handoff_coordinator.rs      # controlled handoff old→new owner (ids-mobility)
│   │   ├── responsibility_messages.rs  # datagram types
│   │   └── regional_responsibility_layer.rs # composition over Endpoint
│   │
│   ├── invariant/                      # Invariant preservation (~8 files)
│   │   ├── mod.rs
│   │   ├── quorum_overlap.rs           # quorum overlap check (AR-RPC vs joint consensus)
│   │   ├── redundancy_floor.rs         # redundancy floor gating (DurabilityCondition)
│   │   ├── revision_invariant.rs       # invariants that hold across every revision step
│   │   ├── epoch_validator.rs          # epoch versioning for stale-view rejection
│   │   ├── graceful_removal.rs         # grace period for removed members
│   │   ├── invariant_checker.rs        # verifies invariant pre/post each revision
│   │   └── invariant_messages.rs       # datagram types
│   │
│   ├── topology/                       # Topology change handling (~9 files)
│   │   ├── mod.rs
│   │   ├── topology_change.rs          # node join/leave, H-unit form/retire, region split/merge
│   │   ├── link_degradation.rs         # carrier/link degradation response
│   │   ├── neighbor_maintenance.rs     # active neighbor pointer updates (Li/Misra/Plaxton)
│   │   ├── view_change.rs              # membership view dissemination (SWIM/Akka gossip CRDT)
│   │   ├── dynamic_quorum.rs           # adaptive quorum q(t) from live membership (SWARM+)
│   │   ├── topology_messages.rs        # datagram types
│   │   ├── self_healing.rs             # automatic replacement of failed responsibilities
│   │   └── topology_layer.rs           # composition over Endpoint
│   │
│   ├── decision/                       # Decision recording (~5 files)
│   │   ├── mod.rs
│   │   ├── decision_recorder.rs        # epoch-logged decision recording
│   │   ├── epoch_log.rs                # bounded epoch log with TTL compaction
│   │   ├── completion_condition.rs     # completion condition verification
│   │   └── cooldown.rs                 # revision cooldown windows / oscillation prevention
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper for reorg metadata
│   ├── codec/                          # wire encoding of reorganisation messages (~4 files)
│   │   ├── mod.rs
│   │   ├── reorg_frame.rs              # version-tagged frame header
│   │   ├── decision_codec.rs           # serde/bincode encode/decode
│   │   └── digest.rs                   # sha256/blake3 decision digests
│   │
│   ├── validation/                     # input validation rules (~3 files)
│   │   ├── mod.rs
│   │   ├── revision_validation.rs      # schema + size/depth budgets before semantics
│   │   └── selfreorg_predicates.rs     # deterministic validity predicates per revision class
│   │
│   ├── observability/                  # reorganisation event types (membership, placement, replica, region)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── membership_tests.rs             # H-unit add/remove/retire + lifecycle
│   ├── placement_tests.rs              # consistent hashing + partition map + movement minimization
│   ├── replica_fragment_tests.rs       # replica count + redundancy mode + fragment placement
│   ├── recovery_assignment_tests.rs    # recovery owner revision + plan revision
│   ├── regional_responsibility_tests.rs# region split/merge + responsibility move
│   ├── invariant_tests.rs              # quorum overlap + redundancy floor + epoch validation
│   ├── topology_tests.rs               # node join/leave + link degradation + self-healing
│   ├── decision_tests.rs               # epoch log + completion condition + cooldown
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~140–200 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Membership Revision (H-unit add/remove/retire)
```
delta = {add: [NodeId], remove: [NodeId], retire_hunit: [HUnitId]}
validate: admission check (capacity), lifecycle state (Forming/Operational/Degraded/Compensating/Retired)
apply: DiscoveryEngine.register_node/deregister_node/register_hunit; emit MembershipDelta via Endpoint
serialization: membership_scheduler serializes concurrent deltas; epoch versioned
```

#### 7.2 Placement Revision (consistent hashing + virtual nodes)
```
hash_space = 2^m; Q shards; T tokens per node
node claims ~Q/N shards; join steals shards consistently; leave redistributes to remaining nodes
deterministic partition map: all nodes compute identical RegionId/HUnitId/NodeId per (DatasetId, PartitionId)
rack-aware: spread replicas across failure domains; zone-aware: locality-aware uneven counts for survivability
movement minimization: Aerospike replication-list permutation minimizes moved partitions on view change
```

#### 7.3 Replica/Fragment Revision
```
replica_count = f(RedundancyMode): FullReplica→N copies; ErasureCoded{k,n}→n fragments, k needed; Hybrid→per-policy
transition: increase → allocate new replicas/fragments; decrease → remove excess after redundancy floor re-checked
fragment placement: across H-units/regions per failure-domain constraints; hinted_handoff for temporary failure
```

#### 7.4 Recovery Assignment Revision
```
on placement change: RecoveryOwnerRevision reassigns RecoveryPlan owners
plan_revision: in-flight RecoveryPlan steps retargeted to new owner; owner_transfer via Endpoint datagrams
quarantine_release: release scope, restore_redundancy via ids-recovery trait
```

#### 7.5 Regional Responsibility Revision (joint consensus + split/merge)
```
region split: disjoint members + data ranges per subcluster; C_joint requires quorums from each subcluster
region merge: 2PC among subclusters; C_new committed after all agree; data exchange then new leader
quorum overlap: if old/new quorums already overlap → single-entry transition; else joint→uniform (two entries)
handoff: controlled old→new owner with cutover verification (ids-mobility)
```

#### 7.6 Invariant Preservation
```
pre-revision: quorum_overlap_ok(old, new) ∧ redundancy_floor_ok() ∧ durability_condition_met()
post-revision: same invariants re-checked; epoch_validator rejects stale views
grace period: removed member retains read-only access for bounded window; no write authority
```

#### 7.7 Topology Change Handling
```
node join/leave: SWIM-style randomized probing; gossip CRDT merge (monotonic); leader deterministic after convergence
link degradation: carrier-agnostic — detected via Endpoint reachability, not IP/TCP; alternate CarrierType
self-healing: automatic replacement of failed responsibilities; desired-state reconciliation
dynamic quorum: q(t) = ceil((n_live(t)+1)/2) adapts without explicit reconfiguration (SWARM+)
```

#### 7.8 Data Structures
```
SelfReorgDecision { decision_id, epoch, revision_type, scope, payload_hash (sha256/blake3), signature (ed25519 opt), ids_time }
RevisionEpoch { epoch: u64, decided_at_ms: u64, decided_by: NodeId }
MembershipDelta { add_nodes: Vec<NodeId>, remove_nodes: Vec<NodeId>, retire_hunits: Vec<HUnitId>, epoch }
PlacementRevision { dataset_id, partition_id, old: PlacementDecision, new: PlacementDecision, epoch }
ReplicaFragmentRevision { partition_id, old_redundancy: RedundancyMode, new_redundancy: RedundancyMode, epoch }
RecoveryAssignmentRevision { plan_id: RecoveryPlanId, old_owner, new_owner, scope: RecoveryScope, epoch }
RegionalResponsibilityRevision { region_from, region_to, responsibilities, joint_config_opt, epoch }
CompletionCondition { decision_id, satisfied: bool, verified_at_ms, integrity_result }
```

---

### 8. Dependencies

`ids-selfreorg/Cargo.toml` (proposed):
- `ids-000-aa = { path = \"../ids-000-aa\" }` — types, sealed `trait SelfReorg`/`trait Discovery`/`trait Recovery`, invariants, versioning, errors
- `ids-carrier = { path = \"../ids-carrier\" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = \"../ids-crypto\" }` — sha256, blake3, ed25519, xchacha20poly1305, chacha20poly1305 (consumed only)
- `ids-discovery = { path = \"../ids-discovery\" }` — H-unit/node/region discovery inputs
- `ids-recovery = { path = \"../ids-recovery\" }` — recovery plans, failure detection, reconstruction
- `ids-membership = { path = \"../ids-membership\" }` — H-unit membership lifecycle state machine
- `ids-placement = { path = \"../ids-placement\" }` — partition maps, distribution across nodes/H-units/regions
- `ids-replication = { path = \"../ids-replication\" }` — replica/fragment sources and targets
- `ids-mobility = { path = \"../ids-mobility\" }` — region handoff outputs for regional responsibility revision
- `ids-integrity = { path = \"../ids-integrity\" }` — checksum verification of moved/reconstructed data
- `ids-time = { path = \"../ids-time\" }` — `IdsTime` tick basis
- `ids-ordering-domain = { path = \"../ids-ordering-domain\" }` — dataset-scoped ordering stability
- `thiserror = \"2.0\"` — error enum derive
- `serde = { version = \"1\", features = [\"derive\"] }` — decision message serialization
- `blake3 = \"1.5\"` — content hashing (type-level/use only)
- `serde_json = \"1\"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling infra crates referenced via adapters; persistence delegation is a design option, not a hard dependency in this phase.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | H-unit membership revision specified | `src/membership/` module; add/remove/retire node/H-unit deltas; HUnitState lifecycle |
| 2 | Placement revision specified | `src/placement/` module; consistent hashing + virtual nodes; deterministic partition map |
| 3 | Replica/fragment revision specified | `src/replica_fragment/` module; replica count adjustment; redundancy mode transitions |
| 4 | Recovery assignment revision specified | `src/recovery_assignment/` module; recovery owner reassignment; plan revision |
| 5 | Regional responsibility revision specified | `src/regional_responsibility/` module; region split/merge; responsibility move; joint config |
| 6 | Invariant preservation specified | `src/invariant/` module; quorum overlap; redundancy floor; epoch validation |
| 7 | Topology change handling specified | `src/topology/` module; node join/leave; link degradation; self-healing; dynamic quorum |
| 8 | Decision recording | `src/decision/` module; epoch-logged decisions; completion condition; cooldown |
| 9 | Sealed `trait SelfReorg` implemented | `src/contract.rs` implements `ids_000_aa::contracts::SelfReorg` |
| 10 | Canonical types from ids-000-aa only | `types.rs` imports all canonical types from `ids_000_aa`; no redefinitions |
| 11 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`; all traffic via `ids_carrier::Endpoint` |
| 12 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM |
| 13 | No code written / no source modifications | Plan document only; `ids-selfreorg/` crate not created; no edits to existing crates |
| 14 | Conformance test plan enumerated | `tests/` list covers membership, placement, replica/fragment, recovery assignment, regional responsibility, invariant, topology, decision, carrier-agnostic assertion, crypto allowlist assertion |
| 15 | File structure within range | ~140–200 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| `trait SelfReorg` not yet declared in `ids-000-aa` | `src/contract.rs` cannot reference it until declared | Declaration deferred to master crate phase; plan assumes its existence per sealed-contract pattern |
| Membership/placement revision trigger source | Whether revisions are event-driven (failure detected) or policy-driven (periodic evaluation) | Both specified; trigger router in `membership_scheduler`/`decision_recorder` |
| Consistent hashing token count T and shard count Q defaults | Affects rebalance granularity and movement | Configurable via `PlacementConfig`; tuned at implementation |
| Redundancy floor vs erasure-coded k/n during transition | Breaking floor mid-revision risks data loss | `DurabilityCondition` gate before every step; abort-and-retry on violation |
| Region split/merge data movement volume | Large moves can stall reorganisation | Movement minimizer + phased transfer; cutover verification via ids-mobility |
| Grace period duration for removed members | Too short → split-brain risk; too long → stale authority | Configurable; bounded by epoch version + heartbeat timeout from `ids-time` |
| Integration ordering with ids-mobility/ids-recovery | Regional responsibility revision depends on mobility handoff | Plan assumes ids-mobility (crate 38) and ids-recovery exist before this phase |
| Whether revision decisions are signed by default | Authenticity vs accidental-change scope | ed25519 signing optional; core revision is policy-driven state transition |

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-selfreorg` crate (not folders inside existing crates) | Matches user directive \"try to use separate crates instead of more folders in existing ones\"; single-responsibility boundary |
| H-unit membership, placement, replica/fragment, recovery assignment, regional responsibility as separate modules | Matches prompts.txt entry 39 contents exactly; each independently testable |
| All traffic via `ids-carrier::Endpoint` only | Carrier-agnostic absolute rule (CRATES.md, prompts.txt entry 39) |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| Quorum overlap (joint consensus when quorums don't overlap) | Raft joint consensus safety; generalized Raft reconfiguration (ReCraft/P2') |
| Consistent hashing + virtual nodes for placement | Deterministic partition map; minimal movement on view change (Aerospike/Dynamo lineage) |
| Epoch-versioned decisions + completion condition | Observable reorganisation; stale-view rejection; measurable acceptance |
| Logical identity stable across physical moves | Dataset/partition `RecordId`/`OrderingPos` unchanged; only physical location updated |
| No code in this phase | Research only per prompts.txt entry 39 |

---

### 12. Sources

1. Raft joint consensus membership changes — raft.github.io/raft.pdf — §4.3 membership changes; joint consensus; quorum overlap; Add/RemoveServer vs two-phase.
2. etcd/raft confchange — github.com/etcd-io/raft — EnterJoint/LeaveJoint; joint config validation; split-brain prevention.
3. OpenRaft dynamic membership — docs.rs/openraft — joint config → uniform config; adaptive skip when quorums overlap.
4. ReCraft split/merge/membership change — arxiv.org/html/2504.14802 — wait-free multi-node reconfig; split into disjoint subclusters; merge via 2PC.
5. Strengthened Fault Tolerance — arxiv.org/abs/2501.03715 — resilience 1/3→2/3 via confirmations.
6. SWIM failure detection — cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf — randomized peer-to-peer probing; separates detection from dissemination.
7. Akka cluster membership CRDT — doc.akka.io — gossip-based membership; monotonic merge; deterministic leader after convergence.
8. Consistent hashing virtual nodes — github.com/fogfish/ring — partition/allocation/routing; token→shard ownership; steal on join, redistribute on leave.
9. Dynamo consistent hashing — docs.riak.com/riak/kv/2.2.3/learn/dynamo — consistent hashing partition+replicate; gossip failure detection+membership; decentralized.
10. Cassandra NetworkTopologyStrategy + vnodes — docs.datastax.com — per-DC replica placement; vnode auto-rebalance on join/leave/failure; dead-node rebuild.
11. HDFS rack-aware placement — hadoop.apache.org — replicas across racks; re-replication when factor drops; block placement policy.
12. Aerospike dynamic replication factor — aerospike.com/docs — dynamic RF change + recluster; succession list; replication list permutation minimizes movement.
13. TigerGraph cluster repartition — docs.tigergraph.com — changes replication/partitioning factor without node add/remove; staging space formula.
14. ScyllaDB cloud resize — cloud.docs.scylladb.com — add/remove nodes in multiples of replication factor; default RF 3.
15. CockroachDB replication zones + locality — docs.cockroachlabs.com — replica allocator; locality constraints keep counts uneven for survivability; load-based rebalancing.
16. CockroachDB joint consensus region failure — cockroachlabs.com/blog — joint config survives region outage; prevents split-brain during region transitions.
17. Zone-aware placement + impact-free rebalancing — arxiv.org/pdf/2603.07382 (LinkedIn Pinot) — mirror server sets; placement repair preserves mirror sets; replacement nodes from arbitrary racks.
18. Anti-entropy Merkle tree repair — docs.datastax.com — Merkle trees per replica; root comparison; subtree descent; divergent range streaming.
19. Scalecube anti-entropy Merkle — github.com/scalecube/scalecube-prism — range-based Merkle tree; delta exchange; convergence backstop.
20. Hinted handoff — docs.datastax.com — coordinator stores missed writes with target ID; replays on recovery.
21. SWARM+ dynamic quorum — arxiv.org/html/2603.19431 — q(t)=ceil((n_live(t)+1)/2); hierarchical consensus; dynamic membership without explicit reconfig.
22. GeoLayer geo-distributed placement — arxiv.org/html/2509.02106 — latency-optimized replica placement across DCs; WAN bandwidth trade-offs.
23. RMP membership views — ntrs.nasa.gov — group reformation on site failure/network partition/join-leave; reliable multicast.
24. HyParView gossip membership — ieeexplore.ieee.org/document/4272993 — active/passive view; gossip-based broadcast; passive promoted on active failure.
25. Fault-tolerant active rings P2P — doi.org/10.1109/lcn.2005.69 — neighbor pointer updates on join/leave; active topology maintenance.
26. Kubernetes topology spread constraints — kubernetes.io/docs — spread across failure domains; maxSkew/minDomains; high availability.
27. Kubernetes self-healing — kubernetes.io/docs — automatic replacement of failed pods; rescheduling on node unavailability.
28. Adaptive replica placement — onlinelibrary.wiley.com/doi/10.1002/cpe.5675 — dynamic adaptation to access skew; load balancing.
29. Invariant synthesis IC3 — arxiv.org/html/2605.24619 — inductive invariants for distributed protocols incl. dynamic Raft reconfiguration.
30. IDS prompts.txt entry 39 — C:/idfs/prompts.txt lines 580-594 — crate definition, contents, constraints.
31. CRATES.md entry 39 — C:/idfs/CRATES.md — ids-selfreorg: infrastructure self-reorganisation.
32. CAPABILITIES.md §117 — C:/idfs/CAPABILITIES.md — infrastructure self-reorganisation capability definition, inputs/decisions/outputs, verification.
33. ids-000-aa types.rs — C:/idfs/ids-000-aa/src/types.rs — RegionId (§69), HUnitId (§118), PlacementDecision (§769), RedundancyMode (§659), HUnitState (§933), Endpoint (§470), CarrierType, Capacity, FailureDomain, FailureEvent, RecoveryScope, RecoveryPlanId, RecoveryStepId, DurabilityCondition, FailureEventType, RecoveryAction, RepairStrategy.
34. ids-000-aa contracts/discovery.rs — C:/idfs/ids-000-aa/src/contracts/discovery.rs — sealed `trait Discovery`; HUnitInfo, NodeInfo, RegionInfo, GossipState; DiscoveryError.
35. ids-000-aa contracts/recovery.rs — C:/idfs/ids-000-aa/src/contracts/recovery.rs — sealed `trait Recovery`; RecoveryPlan, RecoveryStep, FailureEvent, RepairStrategy, RecoveryScope, QuarantineManager, StateReconciler, ReplicaReconstructor, RecoveryScheduler, HealthMonitor, RegionalRecoveryCoordinator.
36. HX-241 plan format — C:/hx-241 (plan-077.md, plan-080.md, plan-121.md pattern) — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
37. Existing plan-030.md — C:/idfs/plans/plan-030.md — phase plan template followed by this document (ids-integrity, same carrier-agnostic/crypto-allowlist rules).
38. Existing plan-028.md — C:/idfs/plans/plan-028.md — phase plan template followed by this document (ids-byzantine, same structure).
39. ids-mobility crate 38 — C:/idfs/CRATES.md — region changes with controlled handoff; consumed by regional responsibility revision.
40. ids-membership crate 8 — C:/idfs/CRATES.md — node/H-unit/region membership and lifecycle; consumed by membership revision.

---

**End of plan-039.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**