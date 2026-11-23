## Phase 052 — Create the ids-membership-state crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 775-789)

```
52. Research only do not read plan.md **[Phase 052 — Create the ids-membership-state crate: Node lifecycle state transitions.]**

Crate: ids-membership-state
Purpose: Node lifecycle state transitions.
Contents: Joining, active, degraded, draining, unreachable, failed, retired states, transition recording, placement/routing integration, state persistence, state recovery after restart.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 052 — Create the ids-membership-state crate: Node lifecycle state transitions.]**

Create plan-052.md in C:/idfs/plans/ for the ids-membership-state crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 052 creates `ids-membership-state`, the IDS crate that owns node lifecycle state transitions: **Joining → Active → {Degraded | Draining | Unreachable | Failed | Retired}**, with full transition recording, placement/routing integration, state persistence, and recovery after restart. It is the IDS counterpart of the membership concern: a node is either a full member (Active), transitioning in/out (Joining/Draining/Retired), impaired (Degraded/Unreachable), or gone (Failed). Every transition is an event recorded in an append-only, hash-chained transition log, and every state change is disseminated carrier-agnostically as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

`ids-membership-state` does not open sockets or drive any transport directly. It maintains the membership table (node ID → state + endpoint + timestamps), runs a per-peer failure detector (phi-accrual suspicion score, SWIM-style indirect probe confirmation), escalates Unreachable → Failed on timeout, recovers Unreachable → Active on heartbeat resumption, and enforces the `dead_node_identity` invariant from `ids-000-aa`: a Failed/Retired node never returns under its old identity — re-entry requires a fresh `NodeId`. Placement (`ids-placement-policy`, Phase 053) and routing consult membership state so no data is placed on, and no routes resolve through, non-Active members.

The crate implements the `ids-000-aa` sealed `Membership` trait contract (`src/contracts/membership.rs` in the master crate), which already declares `NodeLifecycleState { Candidate, Joining, Active, Leaving, Retired, Failed }`, `HUnitLifecycleState { Pending, Active, Draining, Retired, Failed }`, `NodeInfo`, `HUnitInfo`, `MembershipEngine`, `DrainManager`, `GracefulLeave`, `RejoinManager`, and `MembershipError` (with `InvalidTransition`, `DrainTimeout`, `QuorumNotMet`). `ids-membership-state` extends the node state set with **Degraded** and **Unreachable** and maps graceful `Leaving` onto **Draining**, providing the downstream implementation the contract points to.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed via `ids-crypto`; primitive IDs from `ids-000-aa`).

A skeleton `ids-membership-state` crate does NOT yet exist at `C:/idfs/ids-membership-state/`. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned SWIM (Das/Gupta/Motivala, DSN 2002 — randomized peer probing, suspect-before-dead, infection-style gossip dissemination, constant per-member load, deterministic detection bound); SWIM Suspicion subprotocol + Lifeguard extensions (arxiv:1707.00788 — suspicion trades detection latency for fewer false positives; buddy refutation); HashiCorp memberlist (alive/suspect/dead states, push/pull state sync, gossip fanout, TCP state exchange); HashiCorp Serf (join/leave intents via gossip, distinguish graceful leave from failure); Cassandra phi accrual failure detector (Hayashibara et al. 2004 — per-node sliding window of heartbeat inter-arrival times, continuous φ suspicion score, suspect vs convict thresholds, failed nodes periodically re-probed, permanent removal only via explicit add/remove); Akka cluster membership CRDT (joining/up/leaving/exiting/down/removed; unreachable is a flag orthogonal to state; leader-driven convergence via gossip); Raft joint consensus membership changes (two-phase C_old,new → C_new; quorum overlap); Kubernetes node drain/cordon/uncordon and v1.37 Node lifecycle conditions (DrainInProgress/Drained/Maintenance*); CockroachDB node decommission (membership decommissioning, replica rebalancing off the node); phi accrual implementations (Akka, Hazelcast, quickwit chitchat with `dead_node_grace_period`); CRDT/delta-CRDT monotonic merge (Shapiro et al. 2011; Almeida et al. 2018; did:crdt); event sourcing for recovery (Akka persistence — append events, snapshots, replay on restart); anti-entropy digest exchange and reconciliation (gossip systems).

---

### 2. Architecture Overview

`ids-membership-state` sits above `ids-000-aa` (canonical types, sealed `Membership` trait, invariants, errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — state machine, failure detection, transition recording, placement/routing integration, persistence, recovery — is an independent module. It is **not** a failure detector for the network stack; it is a node lifecycle state owner: every transition is locally validated, recorded, persisted, and gossiped before any placement/routing decision consumes it.

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (placement, routing, replication)      │
│  ids-placement-policy, ids-routing, ids-replication consult membership before use   │
├─────────────────────────────────────────────────────────────────────────────────────┤
│                Phase 052: ids-membership-state (THIS CRATE — node lifecycle)        │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────────────┐  │
│  │ node state      │ │ failure         │ │ transition      │ │ placement/route  │  │
│  │ machine         │ │ detection       │ │ recording       │ │ integration      │  │
│  │ (7 states,      │ │ (phi-accrual    │ │ (append-only    │ │ (exclude non-active│  │
│  │  Joining→Active│ │  per peer,      │ │  hash-chained log│ │  from placement,   │  │
│  │  →…→Retired)    │ │  SWIM confirm,  │ │  + ed25519 sig)│ │  resolve endpoints)│  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────────────┘  │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────────────┐  │
│  │ persistence     │ │ recovery        │ │ gossip/dissem.  │ │ contracts +      │  │
│  │ (snapshot +     │ │ (restart:       │ │ (state changes  │ │ invariants       │  │
│  │  transition     │ │  snapshot+replay│ │  as datagrams)  │ │ (sealed trait impl)│  │
│  │  log replay)    │ │  verify against │ │                 │ │                  │  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────────────┘  │
└─────────────────────────────────────────────────────────────────┬──────────────────┘
                                                                  │ opaque Vec<u8> datagrams
┌───────────────────────────────────────────────────────────────▼────────────────────┐
│                CARRIER BOUNDARY: ids-carrier::Endpoint only                       │
│  send_datagram(Endpoint{CarrierType, address: Vec<u8>}, payload: &[u8])            │
│  receive_datagram → (Endpoint, Vec<u8>)                                            │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS            │
└────────────────────────────────────────────────────────────────────────────────────┘
```

**Node lifecycle states (7, per Phase 052 prompt):**
- `Joining` — transient; node announced itself, awaiting quorum/activation; not yet placement-eligible.
- `Active` — normal operating state; placement-eligible, routable.
- `Degraded` — member but impaired (health below threshold); placement may avoid, routing may deprioritize; not terminal.
- `Draining` — graceful departure in progress; no new placement, in-flight work completes; precedes Retired.
- `Unreachable` — failure-detected (phi-accrual conviction + SWIM confirm); placement/routing exclude; recoverable → Active.
- `Failed` — confirmed failure after Unreachable timeout; terminal for that identity; `dead_node_identity` forbids return under same NodeId.
- `Retired` — explicit graceful decommission complete; terminal; replaced by fresh identity if the node returns.

**Transitions (event-driven, all recorded):**
`node_join` → Joining; `activate` (quorum met) → Active; `health_fault` → Degraded; `health_recover` → Active; `drain_request` → Draining; `drain_complete` → Retired; `drain_timeout` → Failed; `phi_convict + SWIM_confirm` → Unreachable; `heartbeat_resume` (Unreachable) → Active; `unreachable_ttl_expired` → Failed; `fail` (immediate, e.g. kernel panic observed) → Failed; `rejoin` after Failed/Retired → Joining with a **fresh NodeId** (old identity never reused).

**Data flow of a state transition:** local event (heartbeat miss, drain request, admin retire) → failure detector or state machine evaluates → transition validated against the allowed transition table → recorded in append-only transition log (node_id, from, to, trigger, `IdsTime`, ed25519 signature) → persisted (snapshot + journal) → gossiped as opaque datagram to `ids-carrier::Endpoint` → placement/routing consume the new state on convergence.

**Failure detection:** each node runs phi-accrual per peer over a sliding window of heartbeat inter-arrival times (Cassandra/Akka/Hazelcast pattern); φ crossing the suspect threshold marks `Unreachable` (not immediately Failed — mirrors Serf/memberlist suspect state); SWIM indirect probing (ask k random peers to probe the target) confirms before escalation; `Unreachable → Active` on heartbeat resumption within the grace window; `Unreachable → Failed` after `dead_node_grace_period`/TTL; `Failed` triggers placement evacuation and routing removal.

**Draining (graceful departure):** modeled on Kubernetes drain/cordon and CockroachDB decommission — node marked unschedulable for placement, in-flight workloads complete, replicas/data rebalanced off, then `Draining → Retired`; on `DrainTimeout` → `Failed`.

---

### 3. Detailed Research Dimensions

#### 3.1 Node lifecycle state machines and the 7-state model

Research-verified state sets from production systems:
- **SWIM** (Cornell DSN 2002): per-target states alive → suspect → dead; suspicion precedes death to reduce false positives; deterministic detection bound; constant per-member message load; infection-style (gossip) dissemination of membership changes piggybacked on ping/ack.
- **HashiCorp memberlist/Serf**: alive → suspect → dead; graceful `leave` intent vs failure-driven `dead`, distinguished at the gossip layer; join intent carries a Lamport clock for join/leave ordering; push/pull full-state sync periodically for convergence.
- **Akka cluster membership CRDT**: joining → up → leaving → exiting → removed; `down` marks crashed nodes; `removed` is a tombstone; crucially, **unreachable is a flag orthogonal to the state** — a node can be `up` but flagged unreachable, and returns to fully reachable only when all monitoring nodes see it reachable again. Leader confirms transitions after gossip convergence.
- **Cassandra**: alive → down (phi conviction); failed nodes are **not** auto-removed (outages are often transient); periodic re-probe; permanent membership change only via explicit `nodetool` add/remove — directly supports IDS's `dead_node_identity` invariant (dead node never returns with old identity; fresh ID assigned).

IDS design decision: a **7-state node FSM** (Joining, Active, Degraded, Draining, Unreachable, Failed, Retired) where Unreachable is a first-class state (not a flag) because placement and routing must exclude unreachable nodes deterministically, matching the `OpState`/`CommitState` style canonical states already in `ids-000-aa::types`. Degraded captures impaired-but-operational (health monitor from `ids-000-aa` recovery module reports Warning/Critical). Draining = graceful leave. Retired and Failed are terminal for an identity.

#### 3.2 Failure detection: phi-accrual + SWIM indirect probing

Binary timeouts (mark dead after fixed silence) cause false positives under GC pauses, cross-AZ latency, and slow message processing (the SWIM/Lifeguard finding: slow local processing declares healthy members faulty). The accrual alternative: a continuous suspicion score φ computed from a sliding window of heartbeat inter-arrival times; the application picks the threshold per SLA (Cassandra `phi_convict_threshold` default 8, 10–12 for EC2; Hazelcast default 10 ≈ 0.1% false-suspect rate). Each node computes its own liveness view — liveness is local, never a global consensus decision.

IDS design: per-peer phi-accrual detector (sliding window of heartbeat inter-arrival ms, mean/variance, φ = -log₁₀(probability of this delay being natural)). φ > suspect_threshold → `Unreachable`; φ stays below → back to `Active`. Before `Unreachable → Failed`, a SWIM-style **indirect probe**: the detector asks k random peers to probe the target; only if all fail within the window is failure declared — this matches memberlist/Serf ("if both our probe and the indirect probes fail, mark suspicious, then dead after refutation timeout") and dramatically cuts false positives. `Unreachable → Active` on heartbeat resume (recovery path); `Unreachable → Failed` after `unreachable_ttl_ms` (default aligned with `heartbeat_threshold_ms` in `ids-000-aa` MembershipConfig).

Sources: [^1] SWIM paper; [^3] Lifeguard/SWIM suspicion; [^4][^5] memberlist; [^6][^7] Serf; [^10][^11] Cassandra phi accrual; [^25] Hazelcast phi accrual; [^26] quickwit chitchat (scuttlebutt reconciliation + phi-accrual, `dead_node_grace_period`).

#### 3.3 Transition recording (append-only, hash-chained log)

Every state transition is recorded before it takes effect: `(transition_id, node_id, from, to, trigger, ids_time_ms, prev_hash, signature)` where `signature = ed25519(node_priv over the entry)` and the entry chain uses SHA-256/BLAKE3 of the previous entry. This gives an offline-verifiable audit trail of every lifecycle change — the membership analog of plan-050's session audit chain. Invalid transitions (e.g. `Active → Joining`, `Retired → Active` without rejoin-with-fresh-id, `Failed → Draining`) are rejected with `MembershipError::InvalidTransition`. The log supports replay-based recovery after restart and convergence cross-check between replicas: two replicas with the same transition sequence reach the same membership state (CALM/CRDT convergence principle — state-based monotonic merge; did:crdt shows signed CRDTs merge deterministically without coordination).

Sources: [^27] delta-CRDT (monotonic join-semilattice, idempotent merge); [^28] did:crdt (signed CRDTs, CALM, no coordinator); [^14][^15] Akka CRDT membership (monotonic merge, gossip convergence); [^29] CMU CRDT lecture (SEC conditions).

#### 3.4 Placement and routing integration

Placement (`ids-placement-policy`, Phase 053) decides where datasets/live data live; routing resolves logical node IDs to physical `ids-carrier::Endpoint`s. Both must consult membership state:
- Placement: **only `Active` members are placement targets**. `Degraded` is deprioritized (health severity-weighted); `Draining`, `Unreachable`, `Failed`, `Retired` are excluded. When a node enters `Draining`/`Unreachable`, placement triggers re-replication/rebalancing of its data (CockroachDB decommission pattern: replicas moved off before retirement).
- Routing: endpoint resolution returns endpoints only for `Active` members; `Unreachable`/`Failed` lookups fail fast instead of blackholing traffic.

Integration is read-only from membership's perspective: placement/routing subscribe to membership transition events (gossiped state changes); membership does not depend on placement or routing — one-directional dependency, preserving the acyclic dependency direction of `ids-000-aa` (no cycles).

Sources: [^17][^18][^19] Raft joint consensus membership changes (configuration as consensus state; placement must agree on membership); [^21][^22][^23] Kubernetes drain/cordon (cordon = unschedulable, drain = evict, then maintenance); [^24] CockroachDB decommission (membership decommissioning + replica rebalancing); [^12] IEEE blockspace/nodespace decoupling (placement layer separate from membership/nodespace).

#### 3.5 State persistence and recovery after restart

Membership state (table + transition log) must survive node restart. Pattern verified in research: **snapshot + append-only journal** (Akka persistence: events appended, never mutated; snapshots checkpoint; recovery = load latest snapshot + replay journal after snapshot index; snapshot compaction reduces replay cost). For membership: persist `NodeInfo` table + transition log entries; on restart, load snapshot, replay transitions with seq > snapshot seq, rebuild the in-memory state machine, resume phi-accrual windows (heartbeat history is volatile — restart resets suspicion, which is safe: Unreachable nodes stay Unreachable until re-probed, Failed nodes stay Failed).

Recovery correctness rules (non-negotiable): no partial replay on corruption; fail closed on hash/signature mismatch in the transition log; keep multiple historical snapshots; tombstone/transition entries are never compacted while any replica may still need them (causal-stability-style gate adapted to membership: compact only after all known replicas acked past the snapshot).

`dead_node_identity` (from `ids-000-aa` invariants): on rejoin after `Failed`/`Retired`, the node must present a fresh `NodeId`; the old identity is never accepted again. This prevents a compromised or stale node from resurrecting its old state.

Sources: [^14] Akka persistence/event sourcing (append events, snapshot, replay on restart); [^27] CRDT anti-entropy; [^1] SWIM deterministic detection bound (recovery timelines bounded).

#### 3.6 Gossip-based dissemination of membership changes

Membership changes are small and infrequent relative to data, so they piggyback on carrier datagrams: each `ids-carrier::Endpoint` send can carry a membership delta (state change entry) alongside or instead of payload. Dissemination follows the SWIM/infection model: periodic randomized peer selection, state merged idempotently (merge is commutative/associative/idempotent — duplicates and reordering harmless over an unordered carrier), convergence grows logarithmically with group size. Full state sync (memberlist push/pull) periodically repairs divergence. Because the carrier is untrusted and may duplicate/delay/reorder datagrams, membership messages carry sender signatures (ed25519) and sequence numbers so stale/dup transitions are idempotently ignored.

Sources: [^1] SWIM (infection-style dissemination, logarithmic latency); [^4][^5] memberlist (gossip + push/pull); [^6][^7] Serf (gossip layer); [^16] systemoverflow anti-entropy digests (digest exchange to locate divergent entries, transfer only missing).

#### 3.7 Deployment-scale concerns (false positives, churn, partitions)

- False positives: mitigated by suspicion-before-death (SWIM), indirect probing confirmation, phi-accrual adaptive thresholds per peer (Lifeguard reduces false positives ~50x per HashiCorp research).
- Churn: constant per-member message load independent of group size (SWIM property) — IDS scales to thousands of nodes without quadratic heartbeat cost.
- Partitions: each side progresses independently; on heal, gossip/anti-entropy converges (weak consistency acceptable; strong consistency layered by placement/routing consensus if needed, per SWIM's own suggestion).
- Graceful vs ungulates: `Leave`/`Drain` intent distinguished from failure (Serf pattern) so planned maintenance never triggers False Failed.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| False failure declaration (GC pause, latency spike) | Availability | Phi-accrual per-peer adaptive suspicion; SWIM indirect probe confirmation before Failed; Unreachable is recoverable → Active |
| Stale/duplicate transition replay (carrier duplicates datagrams) | State integrity | Transition log is idempotent (same transition_id applied twice = no-op); sequence numbers per node; ed25519 signatures on transition entries |
| Compromised node resurrects old identity after Failed/Retired | Identity integrity | `dead_node_identity`: rejoin only with fresh `NodeId`; old identities never accepted (ids-000-aa invariant) |
| Unauthorized state transition (forge node as Failed) | State integrity | Every transition entry ed25519-signed; replicas verify signature + transition table before applying |
| Transition log corruption / partial write | Persistence integrity | Hash-chained entries + checksums (SHA-256/BLAKE3); fail closed on mismatch; multiple snapshots retained |
| Placement routing traffic to Failed/Unreachable node | Data plane reliability | Placement/routing exclude non-Active; fail-fast endpoint resolution |
| Drain blocking forever (pods/workloads won't stop) | Availability | `DrainTimeout` → `Failed` (configurable); force-drain escape hatch via admin retires |
| Partition splits membership into diverging views | Convergence | Idempotent gossip merge + periodic full-state sync (memberlist push/pull) converges on heal |
| Carrier re-framing / byte corruption | Integrity | Transition payloads signed over content bytes; carrier-agnostic boundary carries opaque bytes, signatures survive re-framing |
| DoS via transition flood | Availability | Per-node transition rate caps; bounded transition log + compaction after all-replica ack |

The carrier is never trusted: membership bindings are signed over payload bytes, surviving re-framing by any carrier; the opaque `Endpoint` (`CarrierType` + `Vec<u8>`) boundary means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-membership-state |
|-------|------|-----------------------------------|
| `ids-000-aa` | Master crate: canonical types, sealed trait contracts, invariants, errors | Consumes `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `IdsTime`, `Checksum`, `Error`; implements sealed `Membership` trait from `src/contracts/membership.rs`; honors `dead_node_identity`, `no_global_master` invariants |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All membership traffic (state changes, failure probes, heartbeats) is datagrams to `Endpoint`; no direct carrier dependency beyond `Endpoint` |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Transition-entry signatures (ed25519), log hashing (sha256/blake3), heartbeat/auth encryption (xchacha20poly1305/chacha20poly1305) |
| `ids-placement-policy` (Phase 053) | Dataset placement decisions | Consumes membership state: places only on Active, excludes Draining/Unreachable/Failed/Retired, triggers rebalance on state change |
| `ids-routing` | Logical → physical endpoint resolution | Returns endpoints only for Active members; fail-fast on Unreachable/Failed |
| `ids-replication` | Replica synchronization | Uses membership for replica placement targets and failure-driven repair triggers |
| `ids-recovery` (contract in ids-000-aa) | Multi-level recovery (node/H-unit/region) | Failed/Unreachable nodes feed recovery failure events; `Recovery` trait consumes membership state |

**Integration points:**
- `ids-000-aa::contracts::Membership` — sealed trait: `join_node`, `leave_node`, `retire_node`, `node_state`, `nodes_in_state`, `is_member`, `member_count`, `active_member_count`; `ids-membership-state` provides the implementation engine.
- `ids-carrier::Endpoint` — all outbound membership datagrams (transition gossip, probe requests, heartbeats) addressed via `Endpoint`; inbound datagrams parsed into membership messages.
- `ids-000-aa::invariants::dead_node_identity` — enforced on every rejoin attempt.
- No dependency on `ids-discovery`, `ids-session`, or any prohibited crate; membership hooks placement/routing via trait-based event callbacks, so placement/routing stay membership-agnostic until they choose to consume state.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-membership-state/` — a separate crate, not a folder inside an existing crate (per the "separate crates instead of more folders" directive):

```
ids-membership-state/
├── Cargo.toml
├── lib.rs                          # crate root, public API surface, feature gates
├── errors.rs                       # MembershipStateError (InvalidTransition, DrainTimeout, IdentityReuse, ...)
├── types.rs                        # NodeLifecycleState (7 states), MembershipRecord, HeartbeatInfo, TransitionEntry (re-exports ids-000-aa types)
├── config.rs                       # MembershipConfig (heartbeat_threshold, suspect_threshold, unreachable_ttl, drain_timeout, grace_period)
├── state_machine.rs                # 7-state FSM, transition table, guard validation (InvalidTransition)
├── failure_detector.rs             # phi-accrual per-peer detector (sliding window, mean/variance, φ)
├── swim_probe.rs                   # SWIM indirect probing: k-random-peer confirmation before Failed
├── gossip.rs                       # infection-style dissemination of transition entries, idempotent merge
├── transition_log.rs               # append-only, hash-chained transition log (ed25519-signed entries)
├── persistence.rs                  # snapshot + journal persistence; load snapshot + replay journal
├── recovery.rs                     # recovery after restart: snapshot + log replay; dead_node_identity rejoin check
├── drain.rs                        # draining state: start, complete, drain_timeout → Failed
├── placement_integration.rs        # placement/routing event callbacks (Active-only placement, fail-fast routing)
├── heartbeat.rs                    # heartbeat emission/reception scheduling (carrier-agnostic datagrams)
├── carrier_adapter.rs              # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
├── metrics.rs                      # transition counts, state residency, suspicion scores, drain latency
├── contracts/                      # sealed trait contracts (~10 files)
│   ├── mod.rs, state_contract.rs, detector_contract.rs, log_contract.rs,
│   ├── persistence_contract.rs, recovery_contract.rs, probe_contract.rs,
│   ├── integration_contract.rs, carrier_contract.rs, crypto_contract.rs
├── invariants/                     # compile-time/runtime invariants (~10 files)
│   ├── mod.rs, valid_transition.rs, dead_node_identity.rs, monotonic_ids_time.rs,
│   ├── log_hash_chain.rs, no_network_stack.rs, crypto_allowlist.rs, active_only_placement.rs,
│   ├── terminal_state_immutable.rs, rejoin_fresh_id.rs, mod.rs
├── tests/                          # property/verification tests (~25 files)
│   ├── fsm_tests.rs, transition_log_tests.rs, failure_detector_tests.rs,
│   ├── swim_probe_tests.rs, gossip_merge_tests.rs, persistence_tests.rs,
│   ├── recovery_tests.rs, drain_tests.rs, integration_tests.rs,
│   ├── adversarial_tests.rs (forged transitions, identity reuse, partition heal),
│   ├── chaos_tests.rs (carrier delay/dup/reorder), property_tests.rs (100k+ iterations)
├── benches/                        # (~8 files)
│   ├── fsm_bench.rs, gossip_bench.rs, persistence_bench.rs, detection_bench.rs
├── docs/                           # (~12 files)
│   ├── architecture.md, fsm.md, failure_detection.md, persistence.md,
│   ├── integration_guide.md, threat_model.md, carrier_agnostic.md, crypto_allowlist.md
└── test_vectors/                   # deterministic test vectors (~6 files)
    ├── transition_vectors.json, fsm_vectors.json, gossip_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — FSM, failure detector, SWIM probe, gossip dissemination, transition log, persistence, recovery, drain, placement/routing integration, heartbeat, carrier adapter — each production-grade with error handling, docs, and tests; plus contracts/invariants subfolders mirroring `ids-000-aa` style. No stubs. The file count scales to ~1000 with per-module tests, fuzz targets, benches, docs, and test vectors at implementation time.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Node FSM and transition table

```
States: { Joining, Active, Degraded, Draining, Unreachable, Failed, Retired }
Terminal: { Failed, Retired }  (identity can never return; rejoin requires fresh NodeId)

Allowed transitions (trigger):
Joining   --activate(quorum met)--> Active
Active    --health_fault--> Degraded
Active    --drain_request--> Draining
Active    --phi_convict+SWIM_confirm--> Unreachable
Active    --fail(immediate)--> Failed
Degraded  --health_recover--> Active
Degraded  --drain_request--> Draining
Degraded  --fail--> Failed
Draining  --drain_complete--> Retired
Draining  --drain_timeout--> Failed
Draining  --fail--> Failed
Unreachable --heartbeat_resume(within TTL)--> Active
Unreachable --unreachable_ttl_expired--> Failed
Unreachable --fail--> Failed
Failed    --rejoin(fresh NodeId)--> Joining   (new identity; old NodeId forbidden)
Retired   --rejoin(fresh NodeId)--> Joining   (new identity; old NodeId forbidden)
```

`can_transition(from, to)` is a pure function — invalid transitions are compile-/runtime-rejected with `MembershipError::InvalidTransition { from, to }`.

#### 7.2 Phi-accrual failure detector

```
window = sliding buffer of last N heartbeat inter-arrival intervals (ms)
mean, var = running statistics over window
φ(now) = -log10( P(delay >= now - t_last | distribution estimated from window) )
  (logistic approximation to the cumulative normal, as in Akka/Hazelcast)
if φ > suspect_threshold → mark Unreachable
if heartbeat arrives while Unreachable → mark Active (recovery)
if Unreachable && now - t_last > unreachable_ttl → mark Failed
```

Per-peer state: each node tracks φ independently — liveness is local, matching Cassandra and Akka (unreachable flag is per-observer). Thresholds configurable per deployment (default φ ≈ 8–10).

#### 7.3 SWIM indirect probe confirmation

```
on φ conviction for target T:
  1. direct ping to T (datagram to ids-carrier::Endpoint(T))
  2. if no ack within probe_timeout: choose k random peers; send ping-req(T) to each
  3. if any peer reports T alive → refute, stay Active
  4. if all k fail within window → confirm Unreachable; gossip "T suspected" to cluster
  5. after suspect_ttl with no refutation from T → T = Failed (gossip "T dead")
```

Reduces false positives from transient/local network issues (the SWIM/Lifeguard core result).

#### 7.4 Transition log entry

```
TransitionEntry {
    transition_id: [u8; 32],        // blake3(node_id ∥ seq ∥ ids_time_ms)
    node_id: NodeId,
    from: NodeLifecycleState,
    to: NodeLifecycleState,
    trigger: TransitionTrigger,
    ids_time_ms: u64,
    prev_entry_hash: [u8; 32],      // sha256 of previous entry (chain)
    signature: [u8; 64],            // ed25519_sign(node_priv, entry bytes)
}
```

Recovery: `load latest snapshot` + `replay entries with seq > snapshot.seq`; fail closed on hash or signature mismatch.

#### 7.5 Gossip merge (idempotent)

```
merge(local, remote) = for each node_id: keep state with higher transition sequence;
  ties broken by deterministic node_id ordering
```
Merge is commutative, associative, idempotent — carrier loss/dup/reorder harmless; convergence bounded logarithmically (SWIM) with periodic full state sync for repair.

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `OpState`) and sealed `Membership` trait contract + invariants |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` + `serde_bytes` | Serialization of transition envelopes, membership records |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-052.1 | 7-state FSM (Joining, Active, Degraded, Draining, Unreachable, Failed, Retired); every transition validated against the transition table; 0 invalid transitions accepted | unit test enumerating all 49 pairs + property test, 100k iterations |
| AC-052.2 | Transition recording: 100% of state transitions appended to hash-chained, ed25519-signed append-only log with actor + `IdsTime` | log audit assertion on every transition |
| AC-052.3 | Idempotent merge: duplicate/reordered transition entries converge to identical state on all replicas | property test over all permutations of small histories |
| AC-052.4 | Failure detection: phi-accrual per-peer; silence beyond adaptive threshold → Unreachable (not immediately Failed); heartbeat resume → Active | unit test with synthetic heartbeat streams |
| AC-052.5 | SWIM confirmation: direct probe + k indirect probes all fail → Unreachable; any peer refutes → stays Active | unit test (simulated peers) |
| AC-052.6 | Unreachable → Failed after `unreachable_ttl_ms`; Failed → never Active under same NodeId | unit + adversarial test |
| AC-052.7 | Dead node identity: rejoin after Failed/Retired accepted only with fresh `NodeId`; old identity rejected 100% | adversarial test |
| AC-052.8 | Draining: no new placement targets while Draining; `drain_complete` → Retired; `drain_timeout` → Failed | integration test with placement mock |
| AC-052.9 | Placement integration: placement excludes non-Active members; routing resolves endpoints only for Active | integration test with placement/routing stubs |
| AC-052.10 | Persistence + restart recovery: snapshot + transition-log replay restores exact pre-restart state (all node states, φ windows reset safely) | round-trip test; state equality pre/post restart |
| AC-052.11 | Corrupt transition log: hash/signature mismatch → fail closed, no partial state applied | adversarial test (tampered entry) |
| AC-052.12 | Carrier-agnostic: all traffic via `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS | grep/audit of crate source; compiles with no networking deps |
| AC-052.13 | Crypto allowlist: only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 in crypto surface | dependency audit |
| AC-052.14 | Gossip dissemination: membership change reaches all replicas within O(log n) rounds under loss/dup/reorder | chaos test |
| AC-052.15 | Property tests: 100,000+ iterations; adversarial tests: partition, offline member, identity reuse | pass/fail |

---

### 10. Remaining Uncertainties

1. **State count granularity:** prompt specifies exactly 7 states (Joining, Active, Degraded, Draining, Unreachable, Failed, Retired); `ids-000-aa` contract has 6 (Candidate, Joining, Active, Leaving, Retired, Failed). Confirm whether `ids-000-aa::NodeLifecycleState` is extended to the 7-state superset or whether `ids-membership-state` maintains its own superset mapped down at the contract boundary.
2. **Failure detector thresholds:** φ suspect/convict defaults, `unreachable_ttl_ms`, probe fanout `k`, and probe timeouts are workload-dependent (LAN vs WAN vs cross-AZ); defaults are placeholders pending measurement.
3. **Placement/routing callback shape:** membership exposes transition events; the exact subscription trait (push vs pull, consistency requirements) is defined here but validated at Phase 053 (`ids-placement-policy`) integration.
4. **Persistence backend:** append-only file vs `ids-storage`-compatible snapshot/journal; durability requirements for multi-day member tables undefined.
5. **Rejoin after Failed with fresh NodeId:** the endpoint/key material binding for the new identity (does the new NodeId carry new ed25519 identity keys?) depends on `ids-000-aa` identity contract — to be confirmed.
6. **Compaction of transition log:** entries compacted only after all known replicas acked past a snapshot; behavior with permanently offline replicas (quarantine/grace window) unresolved.
7. **Convergence vs strong consistency:** weak eventual consistency (gossip) is the baseline; whether placement/routing require strong consistency (leader-based, Raft-style joint config) for membership changes is an open product decision.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-membership-state` (not a folder) | "Separate crates instead of more folders" directive; keeps `ids-000-aa` focused on contracts |
| 7-state FSM with Unreachable/Degraded as first-class states | Prompt-specified states; placement/routing need deterministic exclusion, not just an unreachable flag (Akka's flag model is insufficient for IDS's data-plane requirements) |
| Phi-accrual + SWIM indirect probe confirmation | Adaptive per-peer suspicion cuts false positives under GC pauses/latency variance (Cassandra/Akka/Hazelcast); indirect confirmation cuts false positives further (SWIM/Lifeguard ~50x per HashiCorp) |
| Append-only, hash-chained, ed25519-signed transition log | Offline-verifiable audit trail; replay-based recovery; tamper-evident (did:crdt signed-CRDT pattern) |
| Snapshot + journal persistence | Bounded restart recovery time (Akka persistence pattern); avoids unbounded replay |
| `dead_node_identity` — fresh NodeId on rejoin after Failed/Retired | `ids-000-aa` invariant; prevents identity resurrection after compromise or failure |
| Idempotent gossip merge (commutative/associative/idempotent) | Tolerates carrier loss/dup/reorder; no reliable ordered channel needed (carrier-agnostic) |
| All traffic via `ids-carrier::Endpoint`; signed payload bytes | Carrier never in trust base; survives re-framing |
| Only ed25519 / xchacha20poly1305 / sha256 / blake3 / chacha20poly1305 | IDS crypto allowlist (no exceptions) |
| Weak eventual consistency baseline; strong consistency layered by placement/routing if needed | Matches SWIM's own position (weak consistency scales; strong consistency layered on top) |

---

### 12. Sources

[^1]: SWIM: Scalable Weakly-consistent Infection-style Process Group Membership Protocol (Das, Gupta, Motivala, Cornell, DSN 2002) — randomized probing, suspect-before-dead, gossip dissemination, constant per-member load, deterministic detection bound. https://cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf\n[^2]: IEEE Xplore — SWIM publication record. https://ieeexplore.ieee.org/document/1028914\n[^3]: Lifeguard: SWIM-ing with Situational Awareness (HashiCorp Research, arxiv:1707.00788) — suspicion subprotocol reduces false positives ~50x; slow local processing causes false failure detection. https://arxiv.org/pdf/1707.00788\n[^4]: HashiCorp memberlist — gossip-based cluster membership and failure detection, alive/suspect/dead states, push/pull state sync. https://github.com/hashicorp/memberlist\n[^5]: memberlist pkg.go.dev — GossipInterval/GossipNodes, GossipToTheDeadTime, Lifeguard extensions. https://pkg.go.dev/github.com/hashicorp/memberlist@v0.3.0\n[^6]: HashiCorp Serf internals — gossip internals, join/leave intents distinguish graceful leave from failure. https://github.com/hashicorp/serf/blob/master/docs/internals/gossip.html.markdown\n[^7]: Consul gossip concept — Serf-based LAN/WAN gossip pools, membership + failure detection. https://developer.hashicorp.com/consul/docs/concept/gossip\n[^8]: HashiCorp Serf 0.7 release — TCP fallback probe, network tomography. https://hashicorp.com/en/blog/serf-0-7\n[^9]: Making Gossip More Robust with Lifeguard — buddy refutation, false-positive reduction. https://hashicorp.com/en/blog/making-gossip-more-robust-with-lifeguard\n[^10]: Cassandra failure detection and recovery — phi_convict_threshold, per-node accrual, failed nodes not auto-removed, explicit add/remove only. https://docs.datastax.com/en/cassandra-oss/3.x/cassandra/architecture/archDataDistributeFailDetect.html\n[^11]: Cassandra FailureDetector.java — phi computation, suspect vs convict thresholds. https://github.com/apache/cassandra (FailureDetector.java)\n[^12]: SWIM and Phi Accrual: two approaches to failure detection over gossip — comparison of SWIM states (alive/suspect/dead) vs phi accrual statistical detection. https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/swim-and-phi-accrual-two-approaches-to-failure-detection-over-gossip\n[^13]: Akka Cluster Membership Service — joining/up/leaving/exiting/down/removed; unreachable as flag orthogonal to state; leader confirms after gossip convergence. https://doc.akka.io/libraries/akka-core/current/typed/cluster-membership.html\n[^14]: Akka Cluster Specification (2.3.8) — member states, gossip-based convergence, seed nodes. http://doc.akka.io/docs/akka/2.3.8/common/cluster.html\n[^15]: Akka Persistence / Event Sourcing — append-only events, snapshots, replay on restart. https://doc.akka.io/libraries/guide/concepts/event-sourcing.html\n[^16]: etcd raft README — ProposeConfChange for cluster membership changes. https://github.com/etcd-io/raft/blob/main/README.md\n[^17]: OpenRaft joint consensus documentation — two-phase C_old,new → C_new membership changes, quorum overlap. https://docs.rs/openraft/0.10.0-alpha.37/openraft/docs/cluster_control/joint_consensus\n[^18]: Redpanda Raft group reconfiguration — learners, joint consensus, membership changes via replicated config messages. https://docs.redpanda.com/streaming/25.3/manage/raft-group-reconfiguration.md\n[^19]: JOINT-CONSENSUS.md (peterbourgon/raft) — Raft §6 cluster membership change analysis. https://github.com/peterbourgon/raft/blob/master/JOINT-CONSENSUS.md\n[^20]: ReCraft — Raft split/merge/membership change (arxiv:2504.14802). https://arxiv.org/html/2504.14802\n[^21]: Kubernetes — Safely Drain a Node (cordon/drain/uncordon, PDBs, graceful termination). https://kubernetes.io/docs/tasks/administer-cluster/safely-drain-node\n[^22]: Kubernetes — kubectl drain reference. https://kubernetes.io/docs/reference/kubectl/generated/kubectl_drain\n[^23]: Kubernetes v1.37 Node Lifecycle Conditions — DrainInProgress, Drained, MaintenancePlanned/InProgress, GracefulNodeShutdownInProgress. https://kubernetes.io/blog/2026/09/09/kubernetes-v1-37-node-lifecycle-conditions\n[^24]: CockroachDB Node Shutdown / decommission — membership decommissioning, replica rebalancing off node. https://cockroachlabs.com/docs/v24.2/node-shutdown\n[^25]: Hazelcast Phi Accrual Failure Detector — sliding window, φ threshold default 10 ≈ 0.1% false-suspect rate. https://docs.hazelcast.com/hazelcast/5.7/clusters/phi-accrual-detector\n[^26]: quickwit chitchat — cluster membership with scuttlebutt reconciliation + phi-accrual, `dead_node_grace_period`. https://github.com/quickwit-oss/chitchat\n[^27]: Efficient State-based CRDTs by Delta-Mutation (Almeida et al., arxiv:1410.2803) — δ-CRDT, monotonic join-semilattice, anti-entropy. https://arxiv.org/html/1410.2803v1\n[^28]: did:crdt — Signed CRDTs, CALM theorem, BLAKE3-256, causal delivery, no coordinator. https://arxiv.org/html/2606.16223v1\n[^29]: CMU 17-350 Lecture 10 — Eventual Consistency and CRDTs, SEC conditions (commutative/associative/idempotent merge, inflationary updates). https://cs.cmu.edu/~aldrich/courses/17-350/slides/10-crdts.pdf\n[^30]: Anti-Entropy, Digests, and Reconciliation — digest exchange to locate divergent entries, transfer only missing entries in gossip systems. https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems

**Research only — no code written, no files created, no repository modified.**
