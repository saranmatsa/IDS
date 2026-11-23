## Phase 008 — Create the ids-membership crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files modified. This document is the blueprint for implementation. Research only — no source modifications.**

---

### 1. Executive Summary

Phase 008 creates the **ids-membership** crate — authenticated, versioned joins/leaves coordinated with placement and ownership for the IDS distributed system. The crate manages the full node/H-unit/region membership lifecycle: states (candidate/validating/active/degraded/draining/failed/retired), H-unit membership, regional cluster membership, membership versioning, admission control, credential validation, capacity advertisement, health reporting, and removal procedures. All traffic rides `ids-carrier::Endpoint` (CarrierType + Vec<u8>) — carrier-agnostic by construction. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

**Core design decision:** A versioned membership view (monotonic epoch/version per change) held as a join-semilattice state that converges via SWIM-style gossip with phi-accrual suspicion, anti-entropy digest reconciliation (BLAKE3), and admission gated by ed25519-signed credentials validated against ids-identity (StableNodeId + RotatingNodeId). Nodes progress candidate → validating → active via admission control; removal transitions active → degraded → draining → (failed or retired). Membership versions coordinate with ids-placement and ids-topology so ownership changes follow admission, never precede it.

**Separate crate rule:** ids-membership is its own crate (not a folder inside ids-000-aa), depending on ids-000-aa, ids-carrier, ids-crypto, ids-identity, ids-time, ids-topology, ids-failure-detector. Consistent with CRATES.md dependency rule (all crates depend on ids-000-aa; ids-crypto does not).

**Carrier-agnostic:** Zero std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. The trait `trait Membership` in ids-000-aa (join/leave/lifecycle) is implemented here; all datagrams flow through `ids-carrier::send_datagram`/`receive_datagram`.

---

### 2. Architecture Overview

The membership layer is the cluster's source of truth for "who is in, what state, and at what version." It sits above the carrier transport and below placement/routing/ownership.

```
┌─────────────────────────────────────────────────────────────────┐
│                    OWNERSHIP / PLACEMENT LAYER                  │
│            (ids-placement, ids-routing, ids-distribution)       │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│                      ids-membership (THIS CRATE)                │
│  ┌──────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │ Admission    │ │ Versioned    │ │ Lifecycle state        │ │
│  │ Controller   │ │ Membership   │ │ machine (per node/HU)  │ │
│  └──────────────┘ └──────────────┘ └────────────────────────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │ Credential   │ │ Capacity     │ │ Health reporter        │ │
│  │ Validator    │ │ Advertisement│ │ (heartbeat/phi-accrual)│ │
│  └──────────────┘ └──────────────┘ └────────────────────────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │ Gossip       │ │ Anti-Entropy │ │ Removal / Drain        │ │
│  │ Dissemination│ │ Digest (BLAKE3│ │ Procedure (drain/retire│ │
│  │ (SWIM-style) │ │  Merkle-like)│ │  / decommission)       │ │
│  └──────────────┘ └──────────────┘ └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│             ids-carrier · ids-000-aa · ids-crypto · ids-identity│
│             ids-time · ids-topology · ids-failure-detector       │
└─────────────────────────────────────────────────────────────────┘
```

**Data flow — authenticated join:** prospective node sends `JoinRequest` (signed with its admission credential, carrying StableNodeId + RNI + capacity advertisement) via `ids-carrier::Endpoint`. Admission controller validates signature (ed25519) against ids-identity trust root, checks capacity quotas (ids-resource boundary), and if admitted increments the membership version and emits `MemberAdded(v)` gossiped via SWIM-style rumor mongering. The node transitions candidate → validating → active after a validating window (ids-time-based) and successful health checks.

**Data flow — graceful leave/drain:** node sends `LeaveIntent` (signed); state moves active → draining; placement evacuates replicas (coordinated with ids-placement/ids-replication); once drained and ownership transferred, state → retired; `MemberRemoved(v)` gossiped with incremented version. Failed nodes: phi-accrual suspicion → confirmed failure → state → failed → removal after a tombstone window.

**Membership versioning:** every join/leave/fail/retire bumps a monotonic `MembershipVersion` (u64, tied to ids-time commit ticks). Views are versioned snapshots; placement and routing consume the latest committed version so ownership never races membership.

---

### 3. Detailed Research Dimensions

#### 3.1 Node States and Lifecycle State Machine

States (from CRATES.md §8 / prompts.txt entry 8): candidate, validating, active, degraded, draining, failed, retired.

- **candidate:** node has sent JoinRequest, awaiting admission decision. No traffic served, no replicas assigned.
- **validating:** admitted provisionally; running health + credential re-check window (ids-time bounded). Limited read-only traffic possible; no writes.
- **active:** full member; serves reads/writes, holds replicas, eligible for placement/routing.
- **degraded:** member but below health threshold (phi-accrual suspicion, resource pressure); placement avoids new assignments; existing replicas retained.
- **draining:** graceful departure in progress; no new writes; replicas being evacuated to other members; coordination with ids-replication/ids-placement.
- **failed:** confirmed failure (phi threshold crossed + indirect probe consensus, or STONITH via ids-fencing-epoch/ids-stonith); replicas marked for recovery via ids-recovery.
- **retired:** permanently removed; `MemberRemoved` gossipped; tombstone retained for anti-entropy convergence window.

Transitions are versioned: each transition increments `MembershipVersion` and is gossipped. Invalid transitions rejected (e.g., active → retired without drain, candidate → active without admission).

Reference patterns: HashiCorp Serf member states (alive/suspect/left/failed), graceful leave intents (Serf internals), Kubernetes node drain/cordon (evacuation before removal), CockroachDB node decommission (replica streaming before exit), OR-Set add-wins semantics for concurrent join/leave of the same node identity.

#### 3.2 H-Unit Membership and Regional Cluster Membership

Hierarchy: Region → H-Unit → Node. A node belongs to exactly one H-unit; an H-unit belongs to exactly one region (failure-domain alignment with ids-topology §6). Regional cluster membership = the set of H-units (and their nodes) active in a region.

- H-unit membership: add/remove H-units at region scope; versioned per region; cross-region H-unit moves require mobility coordination (ids-mobility, out of scope for this crate).
- Regional membership view: per-region `RegionMembership { region_id: RegionId, h_units: Map<HUnitId, HUnitState>, version: MembershipVersion }`.
- Failure-domain awareness: placement constraints (power, facility, network, operator, jurisdiction) consulted before admitting a node so domain independence is preserved (ids-topology integration).
- No cross-region membership merge in this crate — regions are autonomous membership domains; cross-region consistency handled by ids-consistency/ids-replication.

Reference: AWS/Azure/GCP region-zone failure-domain hierarchy, Service Fabric hierarchical fault domains, Mesos region/zone domains, Simplyblock failure-domain placement contract.

#### 3.3 Membership Versioning

Every membership change (join, leave, fail, retire, degrade, drain, capacity update) produces a new `MembershipVersion` (monotonic u64). Views are immutable snapshots keyed by version.

- Version bump source: ids-time commit ticks or a local logical counter per region; never wall-clock for ordering.
- Placement/routing consume the version at decision time; a placement made against version N must not be applied after version N+k without re-validation (stale-view guard).
- Gossip carries `(version, view_delta)`; receivers apply only if `delta.version > local.version` (idempotent, out-of-order tolerant).
- Version gaps trigger anti-entropy pull (fetch missing view deltas).

Reference: Cassandra gossip vector clocks (generation, version), Raft configuration change joint consensus (USENIX ATC'14), OpenRaft adaptive membership (quorum overlap), Serf Lamport clock for join/leave ordering.

#### 3.4 Admission Control

Admission decides whether a candidate node joins. Gated on:

1. **Credential validation:** ed25519 signature over JoinRequest verified against ids-identity trust root (StableNodeId bound to provisioned admission key). Fail-closed on missing/invalid credential (AIStore node-join auth pattern: bootstrap trust problem — node not yet in cluster map, primary must authenticate before admitting).
2. **Capacity check:** node's advertised capacity (storage/CPU/memory/network/IOPS) vs. region quotas (ids-resource boundary); reject if region over-subscribed.
3. **Failure-domain check:** node's declared domain (power/facility/network/operator/jurisdiction) preserves domain independence (ids-topology).
4. **Duplicate identity:** StableNodeId/RNI not already an active member (OR-Set semantics: concurrent join of same identity = one wins, idempotent).

Admission output: `Admit(v)` or `Reject(v, reason)`, both versioned and gossipped. Rejected candidates remain candidate or are ejected.

#### 3.5 Credential Validation

- JoinRequest signed with node's admission credential (ed25519).
- Credential checked against ids-identity: StableNodeId + RNI validity window; revoked credentials → immediate reject (emergency revocation path, ids-identity).
- Replay protection: nonce + monotonic sequence per node (ids-time bounded window), rejecting stale joins.
- No TLS/PKI — carrier-agnostic; authentication is message-level (ed25519 over payload), transport-independent.

#### 3.6 Capacity Advertisement

Each active node periodically gossips `CapacityReport { node_id, storage_free, cpu, memory, network_bw, iops, timestamp }`. Consumed by ids-placement for capacity-aware placement and by admission for quota enforcement. Reports are versioned and bounded in size (no unbounded growth).

#### 3.7 Health Reporting

- Heartbeat: each node emits signed health beacons on the carrier; peers track last-heard (ids-time based).
- Phi-accrual suspicion (from ids-failure-detector): phi = -log10(1 - F(timeSinceLastHeartbeat)) computed from inter-arrival distribution; suspicion threshold configurable.
- Health states feed node state transitions: active → degraded (suspicion) → failed (confirmed).
- Health reports piggyback on gossip (SWIM: membership updates piggybacked on ping/ack).
- Local health awareness (Lifeguard, arxiv:1707.00788): node's own load/GC pause considered to reduce false positives.

#### 3.8 Removal Procedures

- **Graceful leave:** LeaveIntent → draining → evacuate replicas (coordinate ids-replication/ids-placement) → retired → MemberRemoved(v).
- **Failure removal:** phi threshold + indirect probe confirmation → failed → tombstone window → MemberRemoved(v) after recovery attempts exhausted (ids-recovery integration).
- **Forced retire:** operator-initiated retirement (security/compliance); bypasses drain if data already replicated elsewhere; immediate MemberRemoved(v).
- **Dead node identity:** dead node never returns with old identity; fresh ID assigned (ids-000-aa invariant `dead_node_identity`).

Reference: Cassandra nodetool decommission (stream data before exit), Kubernetes drain (cordon + evict), CockroachDB decommission, Serf leave intents.

#### 3.9 Gossip-Based Dissemination (SWIM-style)

Membership updates (joins, leaves, failures, capacity, health) disseminated via randomized peer probing + gossip payloads (SWIM, DSN 2002 paper; HashiCorp memberlist/Serf).

- Each round: probe one random peer (direct ping); piggyback membership view delta; indirect probes (ping-req via k others) on suspicion.
- Rumor mongering: new updates pushed hot; anti-entropy: periodic full-state sync with random peer for convergence.
- Eventually consistent: weakly consistent views converge logarithmically with group size; constant message load per member independent of group size.
- No central coordinator (IDS-B design principle: no global master).

#### 3.10 Anti-Entropy & Digest Reconciliation

Periodic full-state exchange with a random peer; Merkle-like digest (BLAKE3) over membership view to locate divergence in O(log n) rather than full-state transfer; ship only missing deltas. Tombstones retained for convergence window then GC'd at causal stability (all peers acked past a version).

Reference: Dynamo Merkle tree anti-entropy, ConflictSync digest-driven sync, scuttlebutt anti-entropy, OR-Set tombstone GC at causal stability.

#### 3.11 Coordination with Placement and Ownership

Membership is the input; placement is the consumer.

- Placement consumes the latest committed membership version; a node not in active state receives no new replicas.
- On member removal (drain/failed/retired), placement re-homes that node's partitions/replicas (ids-recovery).
- Ownership changes (partition ownership) only committed against a membership version where all owning nodes are active.
- Joint-consensus-style safety: membership change + ownership change must not create disjoint majorities (Raft joint consensus pattern).

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Unauthenticated node joins (Sybil) | ed25519-signed JoinRequest validated against ids-identity trust root; fail-closed |
| Replay of old JoinRequest/LeaveIntent | Nonce + monotonic sequence; ids-time bounded replay window |
| Credential theft / compromise | Emergency revocation via ids-identity; revoked credentials rejected prospectively |
| False failure detection (healthy node marked failed) | Phi-accrual + indirect probe confirmation + Lifeguard local-health awareness; suspicion before failure |
| Premature removal (node evicted before data evacuated) | Drain state requires replica evacuation confirmation before retired |
| Stale placement against old membership version | Placement gates on committed membership version; stale-view guard |
| Gossip partition / divergent views | Versioned views + anti-entropy digest reconciliation; converge on heal |
| Membership version rollback / replay | Monotonic u64 versions; reject deltas with version <= local |
| Capacity advertisement DoS (inflated claims) | Quotas enforced by ids-resource; claims cross-checked against observed usage |
| Removal during active writes | Drain state blocks new writes; in-flight writes completed or aborted per consistency profile |
| Byzantine membership messages | Out of scope for Phase 008 (deferred to ids-byzantine); authenticated (ed25519) but not BFT |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 008 |
|-------|-------------------|
| `ids-000-aa` | Canonical types (NodeId, RegionId, HUnitId, Endpoint, CarrierType, OpState, FailureDomain), sealed `trait Membership` contract, invariants (dead_node_identity, no_global_master, binary_completion), error contracts |
| `ids-carrier` | `Endpoint` (CarrierType + Vec<u8>), `send_datagram`/`receive_datagram` — sole traffic path |
| `ids-crypto` | ed25519 sign/verify for credentials; sha256/blake3 for digests; chacha20poly1305/xchacha20poly1305 for optional payload encryption |
| `ids-identity` | StableNodeId, RotatingNodeId (RNI), validity windows, revocation — credential trust root |
| `ids-time` | Idstime ticks for versioning, validating windows, heartbeat timing, replay windows (never wall-clock for ordering) |
| `ids-topology` | FailureDomain hierarchy; domain independence check at admission |
| `ids-failure-detector` | SWIM gossip detection + phi-accrual suspicion (feeds health → state transitions) |
| `ids-placement` | Consumer of membership version; capacity-aware placement; re-homing on removal |
| `ids-replication` | Replica evacuation during drain; recovery on failure |
| `ids-recovery` | Node/H-unit/region failure recovery; tombstone windows |
| `ids-resource` | Capacity quota boundary for admission |

**Integration points:**
- Implements `trait Membership` from ids-000-aa (join/leave/lifecycle).
- All datagrams via `ids-carrier::Endpoint`; zero direct carrier deps.
- Credentials verified via ids-identity; health via ids-failure-detector; placement gated by membership version.

---

### 6. Proposed File Structure (~90–200 files, all production-grade)

**Crate:** `ids-membership` (depends on ids-000-aa, ids-carrier, ids-crypto, ids-identity, ids-time, ids-topology, ids-failure-detector; serde, thiserror, blake3, bytes, parking_lot, dashmap)

```
ids-membership/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (validating window, phi threshold, tombstone TTL)
├── error.rs                        # MembershipError (thiserror, comprehensive)
├── features.rs                     # Feature flags (gossip, anti_entropy, testing)
│
├── types/                          # Core membership types
│   ├── mod.rs
│   ├── node_id.rs                  # NodeId, HUnitId, RegionId re-export wrappers
│   ├── membership_version.rs       # Monotonic MembershipVersion (u64)
│   ├── node_state.rs               # NodeState enum (candidate/validating/active/degraded/draining/failed/retired)
│   ├── node_entry.rs               # NodeEntry: state, version, capacity, health, timestamps
│   ├── h_unit.rs                   # HUnitId, HUnitState, HUnitMembership
│   ├── region.rs                   # RegionId, RegionMembership, per-region view
│   ├── membership_view.rs          # Immutable versioned snapshot of cluster membership
│   ├── view_delta.rs               # Incremental view change (join/leave/fail/etc.)
│   └── capacity.rs                 # CapacityReport (storage, cpu, memory, network, iops)
│
├── admission/                      # Admission control
│   ├── mod.rs
│   ├── admission_controller.rs     # Join/leave admission decisions
│   ├── credential_validator.rs     # ed25519 credential validation vs ids-identity
│   ├── capacity_checker.rs         # Quota/failure-domain checks
│   ├── join_request.rs             # JoinRequest (signed, capacity advertisement)
│   ├── leave_intent.rs             # LeaveIntent (signed)
│   ├── admit_reject.rs             # Admit(v) / Reject(v, reason)
│   └── replay_guard.rs             # Nonce + sequence replay window
│
├── lifecycle/                      # Node/H-unit/region state machine
│   ├── mod.rs
│   ├── state_machine.rs            # NodeState transitions + guards
│   ├── transitions.rs              # Per-transition validation (active→draining, etc.)
│   ├── drain_manager.rs            # Graceful drain: evacuate replicas, confirm, retire
│   ├── removal_procedure.rs        # Failure/forced retire removal flow
│   ├── tombstone.rs                # Tombstone tracking + convergence-window GC
│   └── h_unit_lifecycle.rs         # H-unit add/remove at region scope
│
├── versioning/                     # Membership versioning
│   ├── mod.rs
│   ├── version_clock.rs            # Monotonic version bump source (ids-time based)
│   ├── view_snapshot.rs            # Immutable view snapshot per version
│   ├── stale_view_guard.rs         # Reject placement/routing on stale versions
│   └── version_gap.rs              # Gap detection → anti-entropy pull
│
├── gossip/                         # SWIM-style gossip dissemination
│   ├── mod.rs
│   ├── gossip_protocol.rs          # Random peer probing + piggybacking
│   ├── rumor_mongering.rs          # Hot-update push with die-after-threshold
│   ├── indirect_probe.rs           # k-peer indirect confirmation on suspicion
│   ├── memberlist.rs               # Local member list with states
│   └── ping_ack.rs                 # Ping/ping-ack/health piggyback
│
├── anti_entropy/                   # Digest-based state reconciliation
│   ├── mod.rs
│   ├── digest.rs                   # BLAKE3 digest over membership view
│   ├── merkle_digest.rs            # Merkle-like tree for divergence location
│   ├── reconciliation.rs           # Pull missing deltas on version gap
│   ├── tombstone_gc.rs             # GC at causal stability (all-peer ack)
│   └── full_state_sync.rs          # Periodic full-state exchange with random peer
│
├── health/                         # Health reporting
│   ├── mod.rs
│   ├── health_reporter.rs          # Signed health beacons via carrier
│   ├── heartbeat_tracker.rs        # Last-heard tracking per peer (ids-time)
│   ├── phi_suspicion.rs            # Phi-accrual suspicion (delegates to ids-failure-detector)
│   ├── local_health.rs             # Lifeguard-style local health awareness
│   └── health_state.rs             # Health → node state mapping
│
├── carrier/                        # Carrier binding (adapter, not implementation)
│   ├── mod.rs
│   ├── endpoint_mapper.rs          # Membership message → ids-carrier::Endpoint
│   ├── datagram_codec.rs           # Encode/decode membership datagrams (CBOR/bincode)
│   └── carrier_adapter.rs          # send_datagram/receive_datagram wiring
│
├── crypto/                         # Cryptographic helpers (allowed set only)
│   ├── mod.rs
│   ├── credential_signer.rs        # ed25519 sign/verify for join/leave
│   ├── nonce_manager.rs            # Nonce + sequence issuance and validation
│   └── digest_hasher.rs            # sha256/blake3 digests
│
├── coordination/                   # Placement/ownership coordination
│   ├── mod.rs
│   ├── placement_gate.rs           # Gate placement on committed membership version
│   ├── ownership_validator.rs      # Validate ownership changes against membership version
│   └── versioned_view_consumer.rs  # Placement/routing consume latest committed view
│
├── persistence/                    # Optional state persistence
│   ├── mod.rs
│   ├── view_store.rs               # Persist membership view snapshots
│   ├── version_log.rs              # Append-only version log for restart recovery
│   └── recovery.rs                 # Restart: load latest snapshot + replay log
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_join.rs
│   ├── fuzz_leave.rs
│   ├── fuzz_gossip.rs
│   └── fuzz_state_machine.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── join_tests.rs               # Authenticated join flow
│   ├── leave_tests.rs              # Graceful drain + retire
│   ├── failure_tests.rs            # Phi suspicion → failed → removal
│   ├── gossip_tests.rs             # Convergence under loss/dup/reorder
│   ├── version_tests.rs            # Monotonic version + stale-view guard
│   ├── admission_tests.rs          # Credential validation + capacity check
│   ├── anti_entropy_tests.rs       # Digest reconciliation correctness
│   ├── coordination_tests.rs       # Placement gating on membership version
│   ├── adversarial_tests.rs        # Replay, stale credential, partition heal
│   ├── chaos_tests.rs              # Node churn, offline members
│   └── property_tests.rs           # Proptest semilattice/version properties
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── gossip_bench.rs
│   ├── anti_entropy_bench.rs
│   ├── admission_bench.rs
│   └── version_bench.rs
│
├── spec/                           # Formal specifications
│   ├── membership_state.md         # State machine spec
│   ├── versioning_spec.md          # Version monotonicity + convergence
│   └── tla/                        # TLA+/PlusCal (in ids-doc)
│
├── docs/                           # Documentation
│   ├── architecture.md
│   ├── admission_guide.md
│   ├── lifecycle_guide.md
│   └── integration_guide.md
│
└── test_vectors/                   # Deterministic test vectors
    ├── join_vectors.json
    ├── leave_vectors.json
    ├── gossip_vectors.json
    └── version_vectors.json
```

**Why ~90–200 files:** each module has a single responsibility (types, admission, lifecycle, versioning, gossip, anti-entropy, health, carrier binding, crypto, coordination, persistence, tests, fuzz, benchmarks, spec, docs, test vectors). Every file holds production-grade code with error handling, documentation, and tests. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Node State Machine

```
candidate --(admit)--> validating --(health pass + validating window)--> active
active        --(suspicion)--> degraded --(health recover)--> active
active        --(leave intent)--> draining --(evacuation complete)--> retired
active/degraded --(failure confirmed)--> failed --(tombstone window)--> removed
failed        --(recovery success)--> active   (re-join with fresh identity per invariant)
```

Guards: no active→retired without drain; no candidate→active without admission; transitions increment `MembershipVersion`.

#### 7.2 Versioned Membership View

```
MembershipView {
    version: MembershipVersion,        // monotonic u64
    nodes: Map<NodeId, NodeEntry>,
    h_units: Map<HUnitId, HUnitState>,
    regions: Map<RegionId, RegionMembership>,
    tombstones: Set<NodeId>,           // removed nodes within convergence window
}
ViewDelta { version, kind (Join/Leave/Fail/Retire/Degrade/Drain/Capacity), node_id, payload }
apply(local_view, delta): if delta.version > local.version { merge delta; local.version = delta.version }
```

Idempotent: applying the same delta twice is a no-op. Out-of-order tolerated: deltas with version <= local.version ignored; gaps trigger anti-entropy pull.

#### 7.3 SWIM Gossip Round

```
each round:
  1. pick random peer; send Ping(view_delta) + piggyback health
  2. on Ack: peer alive; merge peer's delta into local view
  3. on timeout: indirect probe → ping-req to k random peers about target
  4. if all indirect fail: mark suspect → gossip suspect → phi-accrual confirm → failed
```

Constant message load per member; failure detection expected constant time; dissemination latency O(log n).

#### 7.4 Phi-Accrual Suspicion

```
phi = -log10(1 - F(timeSinceLastHeartbeat))
F = CDF of inter-arrival times (normal approx, mean/std from history)
suspicion if phi > threshold; confirmation via indirect probes
```

#### 7.5 Anti-Entropy Digest Reconciliation

```
digest = BLAKE3(view_state)   // Merkle-like tree over node entries
exchange digests with random peer; equal → in sync
differ → descend tree to locate divergent entries → ship missing deltas
tombstone GC only at causal stability (all peers acked past version v)
```

#### 7.6 Admission Decision

```
admit(join_request):
  1. verify ed25519 sig over JoinRequest vs ids-identity trust root
  2. check nonce + sequence (replay guard)
  3. check StableNodeId/RNI not already active (OR-Set idempotency)
  4. check capacity vs region quota (ids-resource boundary)
  5. check failure-domain independence (ids-topology)
  6. if all pass: Admit(v) → version bump → NodeEntry(candidate) → validating
  7. else: Reject(v, reason)
```

#### 7.7 Graceful Drain

```
drain(node):
  state → draining; block new writes
  for each replica on node: stream to target (ids-replication), update placement (ids-placement)
  when all replicas evacuated: state → retired; MemberRemoved(v) gossipped
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types, sealed `trait Membership`, invariants, errors |
| `ids-carrier` | `Endpoint`, `send_datagram`/`receive_datagram` |
| `ids-crypto` | ed25519 sign/verify, sha256, blake3, chacha20poly1305/xchacha20poly1305 |
| `ids-identity` | StableNodeId, RotatingNodeId, validity windows, revocation |
| `ids-time` | Idstime ticks (versioning, windows, timing) |
| `ids-topology` | FailureDomain hierarchy, domain independence |
| `ids-failure-detector` | SWIM + phi-accrual suspicion |
| `ids-placement` | Consumer of membership version (integration boundary) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `blake3` | Digest hashing |
| `bytes` | Buffer handling |
| `parking_lot` / `dashmap` | Concurrent member list indexes |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — all traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-008.1 | Authenticated join: only ed25519-signed JoinRequests admitted; invalid credentials rejected | Unit test (valid sig admitted, invalid/tampered sig rejected) |
| AC-008.2 | Replay protection: replayed JoinRequest/LeaveIntent rejected | Test replay beyond nonce window |
| AC-008.3 | Node state transitions respect guards (no active→retired without drain) | State machine unit tests, all transitions |
| AC-008.4 | Membership versions monotonic; stale deltas rejected | Property test (100k iterations) |
| AC-008.5 | Gossip convergence under loss/dup/reorder: all peers reach same view | Chaos test (SWIM-style) |
| AC-008.6 | Phi-accrual suspicion → failed transition with indirect probe confirmation | Failure injection test |
| AC-008.7 | Graceful drain evacuates replicas before retired | Integration test with ids-replication/ids-placement |
| AC-008.8 | Anti-entropy digest reconciliation converges divergent views | Test partition heal |
| AC-008.9 | Placement gated on committed membership version; stale view rejected | Coordination test |
| AC-008.10 | Carrier agnostic: compiles with zero std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS deps | Dependency audit + build check |
| AC-008.11 | Crypto restricted to ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | Dependency audit |
| AC-008.12 | H-unit and region membership versioned per region | Unit test |
| AC-008.13 | Dead node never returns with old identity; fresh ID required | Invariant test (ids-000-aa dead_node_identity) |
| AC-008.14 | Property tests: 100,000+ iterations | proptest |
| AC-008.15 | Adversarial tests pass: replay, stale credential, partition heal, churn | pass/fail |

---

### 10. Remaining Uncertainties

1. **Validating window duration:** ids-time-based provisional window length for candidate→validating→active — workload-dependent, needs tuning.
2. **Phi-accrual threshold and inter-arrival distribution:** defaults are placeholders; production tuning required per deployment latency profile.
3. **Tombstone convergence window:** how long removed-node tombstones are retained before GC — trade-off between metadata bloat and resurrection risk; needs membership-aware causal stability.
4. **Persistence backend:** optional sled/view-store persistence vs. pure in-memory — durability requirements for multi-day storage undefined.
5. **Cross-region membership:** regions are autonomous; cross-region H-unit moves deferred to ids-mobility; coordination boundary not yet specified.
6. **Capacity claim verification:** advertised capacity cross-checked against observed usage — verification frequency and enforcement model undefined.
7. **Formal verification scope:** TLA+/PlusCal spec for state machine and version convergence; extent (core vs. gossip + anti-entropy) undecided.
8. **Wire format:** CBOR vs bincode for membership datagrams — schema evolution needs decision.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-membership` (not subfolder) | CRATES.md separate-crate rule; clean dependency boundary |
| Versioned membership view (monotonic u64) | Idempotent, out-of-order-tolerant gossip; placement/routing gate on version |
| SWIM-style gossip + phi-accrual suspicion | Constant per-member load; expected constant detection time; logarithmic dissemination (DSN 2002; HashiCorp Serf/memberlist) |
| Admission gated by ed25519 credential vs ids-identity | Solves bootstrap trust problem (AIStore node-join auth pattern); fail-closed |
| Graceful drain before retire | Prevents data loss during removal (Cassandra decommission, K8s drain, CockroachDB) |
| OR-Set idempotent join/leave of same identity | Concurrent join/leave of same node identity converges (add-wins semantics) |
| Anti-entropy digest (BLAKE3 Merkle-like) | Locate divergence in O(log n) vs full-state (Dynamo, ConflictSync) |
| No RTT-bound timers; ids-time keyed | Correct over variable delays (carrier-agnostic) |
| Tombstone GC at causal stability (all-peer ack) | Prevents zombie resurrection |
| Reject op-based broadcast as primary | Requires reliable exactly-once causal broadcast — contradicts carrier-agnostic design |
| Allowed crypto only: ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | IDS crypto policy |

---

### 12. Sources

- SWIM: Scalable Weakly-consistent Infection-style Process Group Membership (DSN 2002, Das/Gupta/Motivala) — IEEE Xplore 10.1109/DSN.2002.1028914
- HashiCorp memberlist (SWIM-based gossip membership, failure detection) — github.com/hashicorp/memberlist
- HashiCorp Serf internals (gossip, leave/join intents, Lamport clocks) — github.com/hashicorp/serf/docs/internals/gossip
- Lifeguard: Local Health Awareness for More Accurate Failure Detection (arxiv:1707.00788, HashiCorp)
- Raft cluster membership / joint consensus (USENIX ATC'14, Ongaro/Ousterhout) — usenix.org/conference/atc14/atc14-paper-ongaro.pdf
- OpenRaft adaptive membership (quorum overlap, skip joint consensus) — blog.openacid.com/distributed/raft-adaptive-config-change
- Flexible Paxos: Quorum Intersection Revisited (arxiv:1608.06696)
- OR-Set / CRDTs (Shapiro et al. 2011, inria-00555588) — observed-remove set, add-wins semantics
- Apache Cassandra / Dynamo gossip + Merkle anti-entropy + hinted handoff — cassandra.apache.org, dynamo.pdf (cs.cornell.edu)
- HashiCorp Consul gossip (Serf LAN/WAN pools) — developer.hashicorp.com/consul/docs/concept/gossip
- AIStore Node-Join Authentication (bootstrap trust problem, credential permission) — docs.nvidia.com/aistore/auth_node_join
- Chandra-Toueg unreliable failure detectors (completeness/accuracy, JACM 1996) — dl.acm.org/doi/10.1145/226643.226647
- Phi accrual failure detector (Akka/Pekko; inter-arrival distribution) — doc.akka.io/libraries/akka-core/current/typed/failure-detector
- Rust memberlist crate (gossip membership, async runtime agnostic) — crates.io/crates/memberlist, github.com/al8n/memberlist
- Byzantine eventual consistency / Sybil-resistant admission (Kleppmann arxiv:2012.00472)
- Failure domains (AWS/Azure/GCP region-zone; Service Fabric; Mesos; Simplyblock) — multiple docs
- Cassandra decommission / graceful node removal — compactionstrategy.org, docs.cockroachlabs.com
- Kubernetes node drain/cordon — kubernetes.io/docs
- Group membership in the epidemic style (Golding & Taylor, UCSC-CRL-92-13)
- CRDT survey / optimized OR-Set (Riak DT ORSWOT, version-vector tombstone-free)
- CRDT convergence proofs (Isabelle/Coq mechanized proofs; type-checking CRDT convergence 2023)

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 008.
