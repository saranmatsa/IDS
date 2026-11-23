## Phase 033 — Create the ids-lifecycle crate: Dataset lifecycle: creation → active → archive → expiry → retention → legal hold → deletion → reclamation.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 033 creates **ids-lifecycle**: the IDS crate for dataset lifecycle management across creation → active → archive → expiry → retention → legal hold → deletion → reclamation. It owns lifecycle policies, retention periods, legal holds, expiry enforcement, deletion coordination, reclamation, tombstone management, space amplification reduction, and snapshot compatibility.

**Scope (from prompts.txt entry 33):**
- Crate: `ids-lifecycle`
- Purpose: Dataset lifecycle: creation → active → archive → expiry → retention → legal hold → deletion → reclamation.
- Contents: Lifecycle policies, retention periods, legal holds, expiry, deletion coordination, reclamation, tombstone management, space amplification reduction, snapshot compatibility.
- Guideline: try to use separate crates instead of more folders in existing ones → `ids-lifecycle` is its own crate, not a folder inside another crate.

**Architectural constraints (non-negotiable, from prompts.txt):**
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts.
- Research only — no code, no source modifications, only the plan document.

**Key engineering decisions (preview, justified in §11):**
- `ids-lifecycle` depends only on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`; it is not a storage engine or network crate.
- Lifecycle modeled as a deterministic state machine (creation → active → archive → expiry → retention → legal hold → deletion → reclamation) with guard conditions on every transition; illegal transitions rejected, never silently coerced.
- Legal hold overrides every disposition: an active hold suspends expiry/deletion/reclamation regardless of retention state (verified by retention-enforcement research — legal hold evaluated first, overrides everything).
- Tombstones used for deletion (LSM/CRDT-style): deletion writes a tombstone, propagated before permanent drop during compaction; tombstone GC only after all peers acknowledge past the deletion (causal stability), preventing zombie resurrection.
- Space amplification reduction via tombstone compaction/reclaim sweep bounded by quota; reclamation marks blocks free but does not delete snapshot-referenced content (content-addressed storage, snapshot compatibility).
- Snapshot compatibility: every snapshot holds an implicit GC hold on referenced CAS blocks; deleting a snapshot releases its hold; snapshots immovable during lifecycle transitions that touch referenced data.
- Retention enforcement is deterministic and reversible-by-review: the evaluator emits disposition actions (RETAIN/REVIEW/PURGE/ARCHIVE) and routes purges through a delayed queue — never deletes directly.

---

### 2. Architecture Overview

`ids-lifecycle` sits below the orchestrator/scheduler and above `ids-carrier::Endpoint`. It is **not** a storage engine — it is a local lifecycle policy engine that evaluates dataset state transitions and emits bytes to ship over whatever carrier IDS uses.

```
┌─────────────────────────────────────────────────────────────┐
│              ORCHESTRATOR / ids-scheduler                   │
│  admits lifecycle actions based on ids-lifecycle decisions  │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              Phase 033: ids-lifecycle (NEW CRATE)           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ state    │ │ policy   │ │ retention│ │ hold     │       │
│  │ machine  │ │ engine   │ │ periods  │ │ manager  │       │
│  │ create   │ │ rules    │ │ TTL      │ │ legal    │       │
│  │ active   │ │ conditions│ │ expiry   │ │ release  │       │
│  │ archive  │ │ guard    │ │ action   │ │ override │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ expiry   │ │ deletion │ │ reclaim  │ │ tombstone│       │
│  │ enforcer │ │ coordinator│ │ sweep   │ │ manager  │       │
│  │ deadline │ │ consent   │ │ quota    │ │ GC       │       │
│  │ queue    │ │ gate     │ │ bounded  │ │ stability│       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ snapshot │ │ carrier  │ │ audit    │ │ types    │       │
│  │ compat   │ │ endpoint │ │ log      │ │ error    │       │
│  │ hold     │ │ adapter  │ │ append   │ │ contracts│       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│             ids-000-aa (master crate, sealed traits)        │
│  DatasetId, DatasetState, LifecyclePolicyId, RetentionId,   │
│  HoldId, TombstoneId, SnapshotId + sealed trait contracts   │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              ids-carrier::Endpoint                          │
│  CarrierType + Vec<u8>  (the ONLY traffic channel)          │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              ids-crypto / ids-storage                       │
│  ed25519, xchacha20poly1305, chacha20poly1305, sha256,      │
│  blake3 | lifecycle journal, tombstone log, snapshot index  │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a lifecycle decision:**
1. Scheduler (or time-driven sweeper) asks `ids-lifecycle` to evaluate dataset `d` against its policy.
2. `state machine` reads current state (creation/active/archive/expiry/retention/hold/deletion/reclamation).
3. `policy engine` evaluates lifecycle rules + guard conditions for the candidate transition.
4. `retention` checks retention period vs reference event (created/published/last_accessed) and computes disposition date.
5. `hold` checked FIRST: if a legal hold is active, disposition is forced to RETAIN regardless of retention state (legal hold overrides expiry/deletion/reclamation).
6. `expiry` checks deadline (`creation_time + lifetime` keyed to creation time, never RTT); expired → candidate for disposition.
7. `deletion` coordinator requires consent gate: no active holds, retention elapsed, no snapshot references blocking; writes tombstone before permanent drop.
8. `tombstone` manager propagates tombstone; GC only after causal stability (all peers acked past the deletion).
9. `reclaim` sweep reclaims space bounded by quota; snapshot-referenced content never reclaimed (snapshot compatibility).
10. Decision (transition / hold / retain / review / purge / reject + reason) returned; control-plane traffic between nodes travels only via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

---

### 3. Detailed Research Dimensions

#### 3.1 Lifecycle State Machine — creation → active → archive → expiry → retention → legal hold → deletion → reclamation

Each dataset exists in exactly one lifecycle state at any time; all transitions follow deterministic guard rules. Based on SDLP-style lifecycle research (seven-state deterministic object lifecycle), every transition has explicit preconditions and postconditions; illegal transitions are rejected, never silently coerced.

- **creation** — dataset being provisioned/ingested; mutable; creation timestamp recorded (all deadlines keyed to creation time, never measured RTT).
- **active** — dataset in regular use for analysis/operations; receives writes; retention clock may start at creation, publish, or last_accessed (reference event configurable per policy).
- **archive** — data no longer actively used but retained for legal/regulatory/business reasons; moved to cheaper storage tier; read-only or restricted-write.
- **expiry** — dataset passed its expiry deadline; data no longer useful; candidate for disposition but not yet deleted.
- **retention** — minimum retention period enforced from reference event; data kept until disposition date + grace window.
- **legal hold** — suspends all disposition (expiry, deletion, reclamation) until explicitly released; evaluated first and overrides every other rule.
- **deletion** — permanent removal; requires consent gate (no active hold, retention elapsed, no blocking snapshot references); tombstone written before drop.
- **reclamation** — space reclamation sweep; reclaims tombstoned/expired blocks not referenced by snapshots; bounded by quota.

Research confirms lifecycle is nonlinear: data may be revived from archive back to active use, and states are not strictly linear (National Academies data lifecycle: "data may move from one state to another in a nonlinear manner"). The state machine must allow active ← archive revival.

#### 3.2 Retention Periods & Policy Enforcement

Retention defined per data category (e.g., transaction records: 7 years; session logs: 30 days), applied consistently across every system holding that data. Based on retention-enforcement research (policy-as-code model):

| Field | Meaning | Drives |
|-------|---------|--------|
| `policy_id` | Stable rule identifier, cited in audit log | ties every action to its rule |
| `category` | Data class the rule matches | selects datasets |
| `retention_period` | Minimum duration from reference event | disposition date |
| `reference_event` | Clock start: created / published / last_accessed | which timestamp |
| `disposition` | Action after period: RETAIN / REVIEW / PURGE / ARCHIVE | emitted action |
| `legal_hold` | Hold reference freezing disposition | forces RETAIN when active |
| `grace_days` | Buffer after disposition date before action | prevents same-day churn |

Enforcement is **deterministic** (same dataset + same policy → same action, auditable/reproducible) and **reversible-by-review** (evaluator emits a disposition action and routes purges through a delayed queue — never deletes directly; a misconfigured rule surfaces as a flagged review, not silent data loss).

Retention clocks observed in production systems: Google Cloud Datastore TTL (deleted within ~24h of expiry, non-instantaneous, lower priority), Firestore TTL, Spanner TTL (within 72h), Azure Blob lifecycle (daily runs, delete after daysAfterModificationGreaterThan), Snowflake storage lifecycle policies (archive to COOL/COLD then expire), OpenSearch Serverless (24h–3650d retention). TTL does not immediately invalidate data — expired-but-not-yet-deleted data remains visible until the sweep runs, matching ids-lifecycle's tombstone approach.

#### 3.3 Legal Hold

A legal hold (litigation hold) preserves data relevant to anticipated/ongoing litigation; once a duty to preserve exists, routine retention/destruction schedules must be suspended. Based on legal-hold research (Zubulake v. UBS Warburg standard; Exterro legal-hold guidance):

- Hold placed by legal authority; covers all records or a scoped subset (e.g., invoices only).
- Hold remains until explicitly released; release requires pre-determined authority, confirmation no other hold covers the data, and legal sign-off.
- Release lifts the suspension on routine disposition — it does not delete anything. Retention obligations underneath still apply after release.
- Overlapping holds merge into one continuous protection stretch; a dataset under any active hold is retained regardless of retention state.
- FRCP Rule 37(e) governs sanctions when preserved information is lost.

Implementation rule: `hold_active(dataset)` evaluated before every disposition; if true → RETAIN, disposition suspended; hold release returns dataset to its underlying retention schedule.

#### 3.4 Expiry & Archive Tiering

Expiry is keyed to creation time (`creation_time + lifetime`), never to measured RTT — matching delay-tolerant research (lifetime = ms past creation; no RTT-bound timers). Archive tiering moves data to cheaper storage (cool → cold → archive) based on age/access patterns:

- Azure Blob: tier to cool after 30 days, archive after 90 days; archive minimum 180 days or early-deletion charge; rehydration up to 15 hours.
- Oracle Object Storage: move to Archive tier after N days, delete after M days; up to 1000 rules per bucket.
- Snowflake: archive to COOL (instant retrieval) or COLD (up to 48h retrieval); archive period before expiry.
- Microsoft Fabric OneLake: conditions on daysAfterCreationGreaterThan / daysAfterModificationGreaterThan / daysAfterLastAccessTimeGreaterThan; actions tier to cool/cold/archive or delete.

ids-lifecycle models expiry as a deadline queue; expired datasets are candidates for disposition (archive / retain / purge) subject to hold and retention gates.

#### 3.5 Deletion Coordination & Tombstone Management

Distributed deletion does not immediately drop data; instead a tombstone marks the deletion and is propagated to replicas before permanent removal during compaction. Based on Cassandra/DSE tombstone research and Lethe (delete-aware LSM engine):

- Delete inserts a tombstone invalidating older instances of the key.
- Tombstone propagates to replicas before permanent drop during compaction/sweep.
- Tombstone GC only after causal stability (all peers acked past the deletion) — prevents "zombie records" (deleted data resurrecting when an offline peer replays old state).
- Offline/never-acking members block GC — requires membership management or a grace/quarantine window.
- Tombstone GC must re-parent survivors before removing stable tombstones to avoid corrupting order (sequence-CRDT GC lesson applied to dataset tombstones).

Deletion coordination: a deletion is consented only when (a) no active legal hold, (b) retention period elapsed, (c) no snapshot references the data, (d) expiry deadline passed. Consent is recorded in the deletion journal before the tombstone is written.

#### 3.6 Reclamation & Space Amplification Reduction

Space amplification = ratio of physical bytes used to logical live data. Tombstones and old versions inflate space until reclaimed. Based on LSM compaction research:

- Compaction merges sorted runs, drops tombstoned/deleted entries, reduces read and space amplification at the cost of write amplification.
- KV-separated LSM-trees (WiscKey, BlobDB, Titan, TerakDB) reduce index size but increase storage overhead; compaction overhead is a key trade-off.
- Scavenger+ revisits space-time tradeoffs in KV-separated LSM-trees.

Reclamation sweep: marks tombstoned blocks free, runs within quota bounds; snapshot-referenced content excluded (content-addressed storage — mark-and-sweep GC enumerates all snapshots, collects reachable blocks, sweeps unreached). Reclamation is bounded (quota-enforced) to prevent unbounded I/O spikes during sweeps.

#### 3.7 Snapshot Compatibility

Snapshots are point-in-time, immutable, content-addressed views. Based on dittofs/cloudstic research:

- Every snapshot holds an implicit GC hold on referenced CAS blocks (manifest-on-disk = held).
- Deleting a snapshot releases its hold; blocks reclaimed only when unreferenced by all snapshots.
- Snapshot state transitions: creating → ready (or creating → failed); failed snapshots are retained for inspection, not silently swept.
- Restore creates a pre-restore safety snapshot first; restore-in-progress marker survives crashes for automatic rollback.

Lifecycle rule: no deletion/reclamation may remove data referenced by any existing snapshot. Snapshot compatibility is a hard guard condition on the deletion consent gate.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Premature deletion under active legal hold | Hold evaluated FIRST before every disposition; hold forces RETAIN; hold release requires authority + no-other-hold check + legal sign-off |
| Zombie records (deleted data resurrected by offline peer) | Tombstone GC only after causal stability (all-peer ack); quarantine/grace window for dead members |
| Illegal state transition coerced silently | Deterministic state machine; every transition has guard conditions; illegal transitions rejected, never coerced |
| Retention period bypassed | Retention enforced per data category, consistently across systems; disposition actions logged before action |
| Space amplification unbounded (tombstone bloat) | Bounded reclamation sweep with quota; tombstone compaction thresholds; GC at causal stability |
| Snapshot data deleted by reclamation | Snapshot implicit GC hold; reclamation excludes snapshot-referenced blocks; snapshot compatibility guard on deletion consent |
| Clock skew corrupting expiry | Deadlines keyed to dataset creation time, never RTT or wall-clock validity; monotonic clock for internal scheduling |
| Replay of stale lifecycle actions | Action IDs + dedup log; idempotency keys on lifecycle operations; nonce/replay cache bounded with TTL |
| Unauthorized lifecycle mutation | ed25519-signed lifecycle actions; capability/grant check before state transition; audit log append-only |

---

### 5. Integration with Existing Crates

Phase 033 depends on and integrates with:

| Crate | Role in Phase 033 |
|-------|-------------------|
| `ids-000-aa` | `DatasetId`, `DatasetState`, `LifecyclePolicyId`, `RetentionId`, `HoldId`, `TombstoneId`, `SnapshotId` + sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — sole traffic channel for lifecycle signals between nodes |
| `ids-crypto` | ed25519 signatures on lifecycle actions; sha256/blake3 for content addressing; xchacha20poly1305/chacha20poly1305 for encrypted lifecycle metadata |
| `ids-storage` | Lifecycle journal, tombstone log, snapshot index, disposition queue persistence |

**Integration points:**
- `ids-carrier::Endpoint` is the only carrier boundary — lifecycle state transitions and disposition signals are shipped as `Vec<u8>` payloads, carrier-agnostic.
- `ids-000-aa` provides canonical types and sealed trait contracts that `ids-lifecycle` implements; no type duplication.
- `ids-storage` persists the lifecycle journal, tombstone log, snapshot index, and disposition queue; recovery replays the journal.
- `ids-crypto` signs lifecycle actions (ed25519) and hashes content (sha256/blake3) for snapshot/tombstone addressing.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — `ids-lifecycle` is carrier-agnostic over `ids-carrier::Endpoint`.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-lifecycle` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`; serde, thiserror, bytes, blake3, sha2, tokio — sync primitives only, no `tokio::net`)

```
ids-lifecycle/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol defaults (retention defaults, grace windows, tombstone thresholds)
├── error.rs                        # LifecycleError (thiserror, comprehensive)
├── features.rs                     # Feature flags (durable, metrics, fuzzing)
│
├── state_machine/                  # Deterministic lifecycle state machine
│   ├── mod.rs
│   ├── states.rs                   # DatasetState enum: creation, active, archive, expiry, retention, hold, deletion, reclamation
│   ├── transitions.rs              # Allowed transitions + guard conditions
│   ├── guard.rs                    # Guard-condition evaluation (hold, retention, snapshot refs, expiry)
│   ├── transition_id.rs            # Unique transition identifier
│   └── state_machine_tests.rs      # State-machine property tests
│
├── policy/                         # Lifecycle policy engine
│   ├── mod.rs
│   ├── policy.rs                   # LifecyclePolicy: rules, retention, disposition
│   ├── rules.rs                    # Lifecycle rule evaluation
│   ├── condition.rs                # Transition guard conditions
│   ├── rule_set.rs                 # Rule set per data category
│   ├── policy_loader.rs            # Loading/exchanging policy sets
│   ├── policy_types.rs             # PolicyId, Category, Disposition, ReferenceEvent
│   ├── policy_proptests.rs         # Property tests: policy invariants
│   └── policy_fuzz.rs              # Fuzzing for malformed policies
│
├── retention/                      # Retention periods & clock
│   ├── mod.rs
│   ├── retention.rs                # Retention period evaluation
│   ├── clock.rs                    # Reference event: created/published/last_accessed
│   ├── disposition_date.rs         # Disposition date computation + grace window
│   ├── retention_enforcer.rs       # Evaluator: RETAIN/REVIEW/PURGE/ARCHIVE
│   ├── retention_types.rs          # RetentionId, RetentionPeriod, ReferenceEvent
│   ├── retention_proptests.rs      # Property tests: deterministic disposition
│   └── retention_fuzz.rs           # Fuzzing for clock-skew edge cases
│
├── hold/                           # Legal hold management
│   ├── mod.rs
│   ├── hold_manager.rs             # Place/release/evaluate holds
│   ├── hold_record.rs              # HoldId, scope (all_records / scoped), placed_on, released_on
│   ├── hold_override.rs            # Hold overrides disposition (evaluated first)
│   ├── hold_release.rs             # Release authority + no-other-hold check
│   ├── hold_types.rs               # HoldId, MatterId, HoldScope
│   ├── hold_proptests.rs           # Property tests: hold overrides all dispositions
│   └── hold_fuzz.rs                # Fuzzing for overlapping holds
│
├── expiry/                         # Expiry enforcement
│   ├── mod.rs
│   ├── expiry.rs                   # Expiry deadline: creation_time + lifetime
│   ├── deadline_queue.rs           # Expiry deadline queue (creation-time keyed)
│   ├── expiry_enforcer.rs          # Expired → candidate for disposition
│   ├── expiry_types.rs             # ExpiryId, Lifetime
│   └── expiry_proptests.rs         # Property tests: no RTT-bound deadlines
│
├── deletion/                       # Deletion coordination
│   ├── mod.rs
│   ├── deletion.rs                 # Deletion consent gate
│   ├── consent.rs                  # Consent: no hold, retention elapsed, no snapshot refs, expiry passed
│   ├── coordinator.rs              # Deletion coordinator across nodes
│   ├── deletion_journal.rs         # Durable consent journal
│   ├── deletion_types.rs           # DeletionId, ConsentDecision
│   └── deletion_proptests.rs       # Property tests: consent soundness
│
├── tombstone/                      # Tombstone management
│   ├── mod.rs
│   ├── tombstone.rs                # Tombstone: invalidates dataset/block references
│   ├── tombstone_manager.rs        # Propagate tombstone, GC at causal stability
│   ├── tombstone_log.rs            # Append-only tombstone log
│   ├── gc.rs                       # Tombstone GC: all-peer ack, quarantine window
│   ├── stability.rs                # Causal stability (per-peer frontiers, MVV min)
│   ├── tombstone_types.rs          # TombstoneId
│   └── tombstone_proptests.rs      # Property tests: no zombie resurrection
│
├── reclaim/                        # Space reclamation & amplification reduction
│   ├── mod.rs
│   ├── reclaim.rs                  # Reclamation sweep (bounded by quota)
│   ├── sweep.rs                    # Sweep: mark tombstoned blocks free, exclude snapshot refs
│   ├── quota.rs                    # Reclamation quota (bounded I/O)
│   ├── amplification.rs            # Space amplification measurement
│   ├── reclaim_types.rs            # ReclaimId, ReclaimQuota
│   └── reclaim_proptests.rs        # Property tests: snapshot refs never reclaimed
│
├── snapshot/                       # Snapshot compatibility
│   ├── mod.rs
│   ├── snapshot.rs                 # Snapshot: point-in-time, immutable, content-addressed
│   ├── snapshot_hold.rs            # Implicit GC hold on referenced CAS blocks
│   ├── snapshot_index.rs           # Snapshot catalog (self-healing)
│   ├── compatibility.rs            # Snapshot compatibility guard on deletion/reclamation
│   ├── snapshot_types.rs           # SnapshotId, SnapshotState (creating/ready/failed)
│   └── snapshot_proptests.rs       # Property tests: snapshot refs protected
│
├── disposition/                    # Disposition queue & actions
│   ├── mod.rs
│   ├── queue.rs                    # Disposition queue: RETAIN/REVIEW/PURGE/ARCHIVE
│   ├── action.rs                   # Disposition action (reversible-by-review)
│   ├── dispatcher.rs               # Routes purges through delayed queue, never deletes directly
│   ├── disposition_types.rs        # Disposition, Action
│   └── disposition_proptests.rs    # Property tests: reversible-by-review
│
├── carrier/                        # Carrier-agnostic traffic adapter
│   ├── mod.rs
│   ├── endpoint.rs                 # ids-carrier::Endpoint binding (CarrierType + Vec<u8>)
│   ├── envelope.rs                 # Lifecycle action envelope (framing-independent)
│   ├── serializer.rs               # Lifecycle action → byte payload
│   └── deserializer.rs             # Byte payload → lifecycle action
│
├── types/                          # Composed types
│   ├── mod.rs
│   ├── dataset_ref.rs              # DatasetId + state + policy refs
│   ├── lifecycle_event.rs          # Lifecycle event (transition, hold, dispose)
│   └── event_id.rs                 # Event identifier + dedup
│
├── audit/                          # Audit log
│   ├── mod.rs
│   ├── audit_log.rs                # Append-only audit log of lifecycle decisions
│   ├── event_recorder.rs           # Structured lifecycle events
│   ├── audit_types.rs              # AuditEvent
│   └── audit_proptests.rs          # Property tests: append-only invariant
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_state_machine.rs
│   ├── fuzz_policy.rs
│   ├── fuzz_hold.rs
│   ├── fuzz_tombstone.rs
│   └── fuzz_reclaim.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── lifecycle_tests.rs          # Full lifecycle transitions
│   ├── hold_tests.rs               # Legal hold override + release
│   ├── retention_tests.rs          # Retention + disposition
│   ├── tombstone_tests.rs          # Tombstone GC + causal stability
│   ├── reclaim_tests.rs            # Reclamation + space amplification
│   ├── snapshot_tests.rs           # Snapshot compatibility
│   ├── deletion_tests.rs           # Deletion consent + coordination
│   ├── expiry_tests.rs             # Expiry deadline queue
│   ├── integration_tests.rs        # End-to-end over ids-carrier::Endpoint
│   ├── adversarial_tests.rs        # Hold bypass, illegal transition, zombie replay
│   └── chaos_tests.rs              # Offline members, partition healing, churn
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── policy_bench.rs             # Policy eval throughput
│   ├── retention_bench.rs          # Disposition date computation
│   ├── tombstone_bench.rs          # Tombstone GC latency
│   └── reclaim_bench.rs            # Reclamation sweep throughput
│
├── spec/                           # Formal specifications
│   ├── state_machine_spec.md       # Lifecycle state machine + guards
│   ├── retention_spec.md           # Retention enforcement + hold override
│   ├── tombstone_spec.md           # Tombstone GC + causal stability
│   └── snapshot_spec.md            # Snapshot compatibility invariant
│
├── docs/
│   ├── architecture.md             # ids-lifecycle architecture
│   ├── lifecycle_guide.md          # When to use which state/disposition
│   ├── hold_guide.md               # Legal hold placement & release
│   └── integration_guide.md        # Integrating with ids-scheduler/ids-storage
│
└── test_vectors/                   # Deterministic test vectors
    ├── lifecycle_vectors.json
    ├── hold_vectors.json
    ├── tombstone_vectors.json
    ├── reclaim_vectors.json
    └── snapshot_vectors.json
```

**Why ~90–1000 files:** the directive calls for a separate crate rather than more folders in existing ones, so `ids-lifecycle` carries a deep module tree: 13 concern domains (state_machine, policy, retention, hold, expiry, deletion, tombstone, reclaim, snapshot, disposition, carrier, types, audit) each with core + implementation + tests; plus fuzzing, benchmarks, formal spec, docs, and generated test vectors. Each concern domain expands combinatorially (8 lifecycle states × 4 dispositions × hold/expiry/retention gates × tombstone GC × snapshot compatibility × test permutations), so the file count scales into the ~90–1000 range with per-configuration modules and generated test vectors. No stubs — every file has full error handling, documentation, and unit tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Lifecycle State Machine
```
state ∈ {creation, active, archive, expiry, retention, hold, deletion, reclamation}
transition(from, to, guard) admitted ⟺ guard(dataset, policy, now) = true
illegal transitions rejected, never silently coerced
nonlinear: active ← archive revival permitted
```

#### 7.2 Retention Enforcement (legal hold evaluated first)
```
evaluate(dataset, policy, today):
  if hold_active(dataset): return RETAIN, "legal hold active"
  anchor = reference_date(dataset, policy.reference_event)
  disposition_date = anchor + policy.retention_period
  effective_date = disposition_date + policy.grace_days
  if today < effective_date: return RETAIN
  return policy.disposition   // REVIEW / PURGE / ARCHIVE
```
Reversible-by-review: PURGE routed through delayed queue, never deleted directly.

#### 7.3 Deletion Consent Gate
```
consent(dataset):
  hold_active(dataset)      → reject (hold)
  retention_not_elapsed     → reject (retention)
  snapshot_references(data) → reject (snapshot compatibility)
  expiry_not_passed         → reject (expiry)
  else: write consent to deletion journal, issue tombstone
```

#### 7.4 Tombstone GC (causal stability)
```
stable_frontier[r] = min over all peers of frontier[r]
version v stable ⟺ ∀ replicas: frontier[replica] ≥ v   // MVV componentwise min
tombstone t GC-able ⟺ t's deletion is causally stable
offline member blocks GC → quarantine/grace window
```

#### 7.5 Reclamation (space amplification reduction)
```
reclaim(quota):
  candidates = tombstoned blocks NOT referenced by any snapshot
  reclaim up to quota bytes; mark free
  snapshot-referenced content excluded (implicit GC hold)
```

#### 7.6 Expiry Deadline (creation-time keyed)
```
expiry(dataset) = dataset.creation_time + dataset.lifetime
expired ⟺ now >= expiry(dataset)
// No RTT appears in any deadline; absolute deadline keyed to creation time.
```

---

### 8. Dependencies

| Crate | Purpose | Notes |
|-------|---------|-------|
| `ids-000-aa` | Canonical types + sealed trait contracts | Master crate |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) | Sole traffic channel |
| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | Allowlist only |
| `ids-storage` | Lifecycle journal, tombstone log, snapshot index | |
| `thiserror` | Error types | |
| `serde` | Serialization | |
| `blake3`, `sha2` | Hashing | Already in workspace |
| `tokio` | Async runtime, sync primitives only | NO `tokio::net` |
| `bytes` | Buffer handling | |
| `criterion` | Benchmarks | |
| `proptest` | Property-based testing | |

**Prohibited:** no `std::net`, `tokio::net`, libp2p, or any IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh crate. All traffic via `ids-carrier::Endpoint`.

**Allowed crypto only:** ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3. No AES, RSA, ECDH, SHA-1, MD5, etc.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| State machine | deterministic transitions; illegal rejected | unit + property test, 100k iterations |
| Legal hold override | hold forces RETAIN; blocks expiry/deletion/reclaim | unit + adversarial test |
| Hold release | authority + no-other-hold + legal sign-off | state-machine test |
| Retention enforcement | disposition = RETAIN until elapsed + grace | property test |
| Disposition reversible-by-review | PURGE via delayed queue, never direct delete | integration test |
| Expiry deadline | creation_time + lifetime; no RTT-bound timers | unit test |
| Deletion consent | all 4 gates (hold, retention, snapshot, expiry) | unit test |
| Tombstone GC | only after causal stability (all-peer ack) | adversarial test (offline peer replay) |
| Zombie prevention | no resurrection after GC | adversarial test |
| Snapshot compatibility | snapshot-referenced blocks never reclaimed | unit + reclaim test |
| Reclamation bounded | within quota; space amplification reduced | bench |
| Carrier-only traffic | zero `std::net`/`tokio::net` usage | build + grep audit |
| Crypto allowlist | only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency audit |
| Property tests | 100,000+ iterations | proptest |
| Idempotency | lifecycle action replay returns original result | idempotency test |

---

### 10. Remaining Uncertainties

1. **Retention clock source:** reference event (created/published/last_accessed) requires a clock abstraction; all deadlines keyed to dataset creation time — a minimal internal clock trait or `time`-like dependency is needed.
2. **Durable lifecycle backend:** optional persistent journal (sled-like via `ids-storage`) vs. pure in-memory — durability requirements for multi-day retention are undefined.
3. **Causal stability with permanent offline members:** tombstone GC freezes if a member never acks; needs membership management or quarantine/grace window — trade-off between metadata bloat and resurrection risk.
4. **Wire format for lifecycle signals over `ids-carrier::Endpoint`:** CBOR vs bincode vs plain scalar encoding; TBD.
5. **Scheduler integration:** synchronous policy check in admission path vs async deferral with durable timer for deferred disposition.
6. **Formal verification scope:** TLA+/PlusCal spec for state-machine invariants and hold-override; effort TBD.
7. **Idempotency key retention window** for the lifecycle journal (bounded retention vs replay safety).
8. **Reclamation quota defaults:** bounded sweep quota needs workload-specific tuning; defaults are placeholders pending measurement.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-lifecycle` crate | Prompt guideline: separate crates instead of more folders in existing ones |
| Carrier-agnostic; `ids-carrier::Endpoint` only | I-CARR-001 architecture; no networking crates |
| Deterministic state machine (8 states) | Predictable transitions; prevents invalid lifecycle operations (SDLP pattern) |
| Legal hold evaluated FIRST, overrides all | Retention-enforcement research: hold suspends disposition regardless of state |
| Tombstones + GC at causal stability | Prevents zombie resurrection; offline members block → quarantine window |
| Snapshot implicit GC hold | Content-addressed storage: snapshot refs protect blocks from reclamation |
| Reversible-by-review disposition | Evaluator emits actions; purges routed through delayed queue, never direct delete |
| Expiry keyed to creation time, never RTT | Correct over arbitrary delays; avoids timeout misfire (RFC 3449 findings) |
| Allowed crypto: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | Prompt allowlist |
| Bounded reclamation sweep with quota | Prevents unbounded I/O during space-amplification reduction |

---

### 12. Sources consulted (30 targeted searches)

- UCF CRCV dataset management — dataset lifecycle states (creating/stable/temporary/archive/archived), squash before archive, 30-day/6-month offline automation
- YouMustArchitect data lifecycle management — Create → Active → Archive → Delete; legal hold pauses progression at any stage
- Amorphic dataset lifecycle — S3 transition/expiration rules; expiration days from upload; GLACIER/DEEP_ARCHIVE constraints
- Crystalloids data lifecycle management — ingestion to deletion; retention policy per data type; secure deletion under GDPR
- Redivis dataset lifecycle — archival stops usage without removal; deletion preserves reference; 7-day undelete window
- Oracle Object Storage lifecycle management — up to 1000 rules; tier to Archive, delete after days; last-modification-time UTC
- Azure Blob lifecycle management — tier to cool/archive, delete after daysAfterModificationGreaterThan; daily runs
- Snowflake storage lifecycle policies — archive to COOL/COLD, expire after archive period; retrieval time trade-offs
- ICO UK GDPR storage limitation — Article 5(1)(e); erasure when no longer necessary; archiving exceptions
- Google Cloud Datastore TTL — deletion within ~24h of expiry; non-instantaneous; lower priority
- Azure immutable storage / WORM — legal hold vs time-based retention; version-level and container-level WORM
- Dell PowerScale SmartLock — WORM state; compliance vs privileged delete
- Lexology tracking and releasing litigation holds — authority to release; no-other-hold check; legal sign-off
- Exterro legal hold chapter — Zubulake standard; duty to preserve; spoliation; release procedures
- Venio Systems how to release a legal hold — release lifts suspension, does not delete; retention layer underneath persists
- Snowflake-Labs data retention and purge — DATA_RETENTION_HOLD_RECORDS; hold checks before deletion
- Data.gov DCAT-US v3 implementation guide — data governance across lifecycle; data asset definition
- Splunk retention and deletion lifecycle — raw table vs promoted dataset retention; independent lifecycle controls
- Microsoft Purview retention — retain-only / delete-only / retain-then-delete; adaptive protection
- Microsoft Fabric OneLake lifecycle — daysAfterCreation/Modification/LastAccess conditions; tier to cool/cold/archive
- AWS OpenSearch Serverless lifecycle — MinIndexRetention 24h–3650d; NoMinIndexRetention indefinite
- Automating data retention policy enforcement — policy-as-code; legal hold overrides everything; deterministic + reversible-by-review
- EON data lifecycle management — retention matrix with legal basis; data deletion matrix best practice
- Cassandra/DSE deletes and tombstones — tombstone propagation; compaction drops tombstones
- Lethe delete-aware LSM engine — tombstone persistence guarantees; delete-aware compaction
- Scavenger+ KV-separated LSM space-time tradeoffs — space amplification vs compaction cost
- dittofs snapshots — manifest-on-disk = GC hold; implicit hold; snapshot state creating/ready/failed
- Cloudstic content-addressable storage — Merkle HAMT; structural sharing; mark-and-sweep GC; snapshot holds
- National Academies data lifecycle — nonlinear data states; archive/revival; data states concept
- SDLP lifecycle state machine draft — seven deterministic states; guard conditions; illegal transitions rejected
- ITAR/EAR-adjacent chain of custody — defensible eDiscovery reporting; authenticity of ESI

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 033.
