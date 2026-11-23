## Phase 029 — Create the ids-saga crate (distributed transaction orchestration with rollback): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Verbatim prompt (C:/idfs/prompts.txt lines 430-444): "**Phase 029 — Create the ids-saga crate: Distributed transaction orchestration with rollback.**"

Crate: `ids-saga`
Purpose: Distributed transaction orchestration with rollback.
Contents per CRATES.md §29 and prompts.txt entry 29: Choreography-based saga, orchestration-based saga, local transactions, compensating transactions, saga state machine, retry semantics, idempotency, deadlock handling, livelock prevention, cross-domain transaction coordination.

The crate is carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-saga` consumes type surface and primitive IDs from `ids-000-aa` only). `ids-000-aa` is the master crate holding every canonical type and sealed trait contract, including the sealed `trait Saga` at `src/contracts/saga.rs` (compensating transactions, step state machines, coordinator election, failure recovery), which `ids-saga` implements. Research only — no code, no source modifications, only this plan document.

---

### 1. Executive Summary

Phase 029 creates `ids-saga`, the IDS crate for distributed transaction orchestration with rollback. A saga decomposes a long-running business transaction into a sequence of local transactions, each owning its own data and committed independently; every forward step has a corresponding compensating transaction that semantically undoes it. If any step fails, completed steps are compensated in reverse execution order so the system reaches either full completion or a fully compensated (aborted) state — never a half-applied distributed transaction.

`ids-saga` supports both saga styles required by prompts.txt entry 29:
- **Choreography-based saga:** decentralized; each participant reacts to events it consumes and emits events that drive the next step; no central coordinator; loose coupling, harder to trace/debug.
- **Orchestration-based saga:** a central orchestrator (the Saga Execution Coordinator, SEC) drives each step, stores saga state, retries, timeouts, and runs compensations on failure; explicit workflow, single point of failure mitigated by durable state + failover.

The crate's canonical types and the sealed `trait Saga` contract already exist in `ids-000-aa/src/contracts/saga.rs` (SagaConfig, StepDefinition, StepState, SagaState, StepExecutionResult, CompensationResult, CompensationBatchResult, SagaRecoveryResult, RecoveryAction, SagaAbortResult, SagaTransitionResult, SagaStats, SagaError, plus reference engine `SagaEngine`, `SagaStepCoordinator`, `SagaTimeoutManager`, `SagaPersistenceManager`, `SagaCheckpointManager`). `ids-saga` implements `trait Saga` and adds carrier-based cross-node step/compensation messages. It depends on `ids-000-aa` (types + sealed contracts), `ids-carrier` (opaque `Endpoint` datagrams), and `ids-time` (`IdsTime` ticks); it is consumed by `ids-write` (local transaction source), `ids-sync` (replica reconciliation), and `ids-coordinator` (cross-domain coordination).

Research basis: 30 parallel searches returned Garcia-Molina & Salem "Sagas" (SIGMOD 1987, ACM DL 10.1145/38713.38742), the two-outcome saga guarantee (all commit, or a prefix plus compensations), compensating transactions must be idempotent and retriable (IBM developer), Saga Execution Coordinator + saga log (IBM), Azure saga pattern (compensable/pivot/retriable transactions, point of no return), AWS prescriptive guidance (choreography vs orchestration, continuation vs compensation principles, forward/backward recovery), Apache Camel Saga EIP (orchestration-style reverse-order compensation), Eventuate Tram Sagas (command/compensation per step, reverse-order rollback), Temporal saga pattern (register compensations, run in reverse order on error), fraiseql_saga (Rust saga crate, Postgres store), AWS Step Functions saga implementation (bounded exponential backoff + jitter retries, idempotent action keys), CockroachDB transaction retry (serialization-error retry with backoff), Purdue CS542 deadlock lectures (wait-for graph, wait-die/wound-wait), distributed deadlock detection via wait-for graph cycles with victim abort, livelock prevention via randomized exponential backoff + jitter, bounded attempts, circuit breakers (AWS Architecture Blog; systemoverflow retry policies), recovery within long-running transactions (backward vs forward recovery, compensations), saga testing via chaos injection (failure at every step, N + N compensation tests), idempotent consumer pattern (Azure), transactional outbox pattern (dual-write problem, exactly-once effect), and TLA+ modeling of distributed transactions (confluence.tlapl.us; arxiv 2404.16075). Carrier-agnostic opaque endpoint: no off-the-shelf standard matches IDS `Endpoint` (HYPOTHESIS — IDS defines its own via `ids-carrier`).

A skeleton `ids-saga` crate does NOT yet exist at `C:/idfs/ids-saga/`. This plan specifies creation from scratch; no source modifications are made in this phase.

---

### 2. Architecture Overview

`ids-saga` sits above `ids-000-aa` (canonical types + sealed `trait Saga` + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-time` (`IdsTime`). It is layered so each concern — choreography, orchestration, local transactions, compensation, state machine, retry, idempotency, deadlock, livelock, cross-domain coordination — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-SM LAYER (ids-* crates)                │
│  business workflows · local transaction boundaries · saga invocation        │
├─────────────────────────────────────────────────────────────────────────────┤
│                         Phase 029: ids-saga (THIS CRATE)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐   │
│  │ orchestration│ │  choreography│ │ local txns   │ │ compensation    │   │
│  │ (SEC, saga   │ │ (event-driven│ │ (per-step    │ │ (reverse order, │   │
│  │  state machine│ │   participants)│  ACID unit)  │ │  idempotent)   │   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └─────────────────┘   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐   │
│  │ retry +      │ │ idempotency  │ │ deadlock     │ │ livelock        │   │
│  │ bounded back │ │ keys + dedup │ │ handling     │ │ prevention      │   │
│  │ off + jitter │ │ (store+skip) │ │ (WFG, victim)│ │ (random backoff)│   │
│  └──────────────┘ └──────────────┘ └──────────────┘ └─────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │              cross-domain transaction coordination                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────┬────────────────────────────────────────┘
                                     │ opaque Vec<u8> step/compensation frames
┌────────────────────────────────────▼────────────────────────────────────────┐
│                  CARRIER BOUNDARY: ids-carrier::Endpoint only               │
│  send_datagram(Endpoint{CarrierType, Vec<u8]}, payload: &[u8])              │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Orchestration flow:** orchestrator looks up saga state (`Created → Executing → [Completed | Failed → Compensating → Compensated | Aborted]`), sends a step command to the participant's `Endpoint`, waits for a step-completed/step-failed reply datagram, persists the transition, advances to the next step; on failure, walks `step_order` in reverse and sends compensation datagrams for every `Completed` step.

**Choreography flow:** no orchestrator; each participant consumes a step-done event, runs its local transaction, and publishes the next event; compensation is triggered by failure events; each service owns its own state machine (matching `ids-000-aa` `StepState` / `SagaState`).

**Local transaction boundary:** each step is one ACID transaction inside one participant (or one `ids-write` op); saga atomicity is approximate — visible intermediate state is allowed (no isolation), consistency is restored by compensation.

**Cross-domain:** a saga spanning failure domains (regions/hunits) is coordinated by one domain's orchestrator; remote steps are addressed via `ids-carrier::Endpoint` only; domain boundaries are compensation scopes — a failure in domain B compensates domain A's completed steps through the coordinator's saga log.

---

### 3. Detailed Research Dimensions

#### 3.1 Choreography-Based Saga

Decentralized coordination: every participant is both producer and consumer. Each service performs its local transaction, then publishes an event; other services subscribe and react. There is no central controller, so there is no single point of failure and no coordinator bottleneck (AWS Prescriptive Guidance saga choreography; New Stack saga patterns). Drawbacks: workflow logic is implicit across services, cyclic dependencies between event consumers are possible, debugging/tracing a failure requires correlating events across services, and adding a participant means editing every event contract. Used when services are loosely coupled and the workflow is simple and stable. Research references: Garcia-Molina & Salem 1987 (events interleave local transactions); AWS "Saga choreography pattern"; The New Stack saga choreography vs orchestration; Apache Camel saga EIP (event-driven style).

In IDS terms: a choreography step is a datagram sent to `ids-carrier::Endpoint` targets; `ids-saga` provides the event envelopes, step/correlation IDs, and the state-machine bindings each participant applies locally.

#### 3.2 Orchestration-Based Saga

A central orchestrator (Saga Execution Coordinator, SEC) knows the full step sequence, tells each participant when to start, stores saga state durably on every transition, handles retries/timeouts, and runs compensations on failure (IBM developer article; Azure saga pattern; AWS saga orchestration). Advantages: explicit, debuggable, testable flow; centralized error/retry/timeout control; the orchestrator's log lets recovery resume after a crash (on restart it replays the saga log and resumes or compensates). Drawback: the orchestrator is a failure point — mitigated by durable persisted state (`SagaPersistenceManager`, `SagaCheckpointManager` in `ids-000-aa/src/contracts/saga.rs`) and failover to a standby coordinator; never a blocking lock holder (unlike 2PC, sagas hold no cross-service locks).

Eventuate Tram Sagas model: each step is a command message to a participant plus an optional compensating transaction; forward transactions execute sequentially; on failure, compensations run in reverse order (eventuate-tram-sagas README). Apache Camel Saga EIP: each step declares a compensation endpoint; Camel invokes compensations for all previously completed steps in reverse order on cancellation (camel.apache.org saga-eip).

#### 3.3 Local Transactions

Each saga step is one local ACID transaction inside a single service/dataset — the unit of work the saga trusts. `ids-write` (Phase 012) is the durable commit log source of local ops; a step registers its `LocalTransaction` (canonical type in `ids-000-aa` types.rs) with the saga via `register_local_transaction`. A local transaction commits before the next step starts; its effects are visible to others immediately (sagas give atomicity + eventual consistency, not isolation — Microsoft Azure saga pattern). The local transaction is the compensation scope: its effect can only be undone by a compensating local transaction, never by a rollback.

#### 3.4 Compensating Transactions

For every forward step `Ti` there is a compensation `Ci` that semantically undoes `Ti` (refund vs charge, release inventory vs reserve, cancel order vs create). Compensation is a new forward transaction, not a database rollback: the original effect stays in the audit log and the compensation is appended (softwareletters SL#110; Vercel saga guide). Rules (IBM; Azure; AWS):
- Compensations run for exactly the steps that completed, in **reverse execution order** (if T3 fails: C2 then C1; T3 itself needs no compensation).
- Compensations must be **idempotent** — recovery may redeliver them, and a failed step that registered a handler may still be compensated (Cloudflare Workflows rollbacks).
- Compensations must be **retriable until they succeed**; if compensation itself fails after bounded attempts, the saga enters an escalated/manual-intervention state (`RecoveryAction::ManualIntervention` in ids-000-aa) rather than hiding inconsistency.
- Register a compensation only after its forward step has succeeded (compensating work that never happened creates inconsistency).
- Non-compensatable (pivot/irreversible) steps go last; after the pivot succeeds, only retriable forward steps remain (Azure pivot-transaction concept; softwareletters step triage: compensatable, pivot, retriable).

`ids-saga` maps this to `compensate_step` / `compensate_all` in the sealed `trait Saga`, reverse traversal of `step_order`, and `CompensationResult`/`CompensationBatchResult` types already declared in `ids-000-aa`.

#### 3.5 Saga State Machine

States declared in `ids-000-aa/src/contracts/saga.rs`:
- Step: `Pending → Executing → [Completed | Failed]`, and `Completed → Compensating → Compensated`, plus `Skipped`; terminal: Completed/Compensated/Skipped; compensatable only when Completed.
- Saga: `Created → Executing → [Completed | Failed → Compensating → Compensated | Aborted]`; terminal: Completed/Compensated/Aborted.

Transitions are guarded (`can_transition_to` style): a terminal saga cannot be executed or compensated; `abort()` moves a non-terminal saga to Aborted after compensating completed steps in reverse order; `advance()` moves Created→Executing, Executing→Completed when all steps terminal-success, Failed→Compensating, Compensating→Compensated. Every transition persists to the saga log before replying (durable execution), so orchestrator crash recovery replays the log (Eventuate Tram; Temporal). `ids-000-aa` already declares `SagaStateMachine` in `src/state/` and `SagaEngine`/`SagaStepCoordinator`/`SagaTimeoutManager`/`SagaPersistenceManager`/`SagaCheckpointManager` in contracts/saga.rs as the reference implementation surface `ids-saga` builds on.

#### 3.6 Retry Semantics

Transient failures (timeout, overload, temporary unavailability) are retried; business failures (payment declined, constraint violation) are not — they go straight to compensation. Policy (AWS Architecture Blog "Exponential Backoff and Jitter"; CockroachDB transaction retry; EDB retry guidance):
- **Error classification:** retry only retryable categories; never retry validation/auth/business-rejection errors.
- **Bounded attempts:** fixed `max_attempts` per step (default 3), then fail and trigger compensation.
- **Exponential backoff with jitter:** base ~100ms, ×2 per attempt, capped, randomized (full/equal/decorrelated jitter) to prevent synchronized retry storms and thundering herds.
- **Budgeted deadline:** total retry wall time bounded by saga step timeout (`SagaConfig.step_timeout_ms`, default 30s) so retries fit the SLO.
- Compensation gets its own, more generous retry budget (`max_compensation_attempts`, default 3; `compensation_timeout_ms` default 60s) because a failed compensation leaves real inconsistency.
- Retry state is part of saga state (attempt counters in `StepRecord`), persisted so a retry after crash resumes with the correct attempt count (ids-000-aa `StepRecord.attempts`).

#### 3.7 Idempotency

Carrier datagrams can be redelivered (at-least-once), and retries re-send step commands; every step and every compensation must be idempotent (same input, same effect, applied once). Mechanisms (Azure idempotent consumer; Stripe idempotency keys; Microsoft compensating-transaction pattern):
- **Idempotency key:** saga ID + step ID (+ attempt) carried on every command; participant stores `(key, result)` and returns the cached result on duplicates.
- **Dedup inside the local transaction:** the dedup record and the business effect commit in one local ACID transaction (no dual-write window).
- **Conditional updates:** `UPDATE … WHERE status = 'PENDING'` makes a duplicate retry a no-op.
- **Transactional outbox:** step emits its completion event into the same local transaction as its effect; a relay publishes the event; consumers dedup on event ID → exactly-once business effect despite at-least-once delivery.
`ids-saga` provides idempotency-key types (`OpRef`-scoped) and dedup windows; `ids-000-aa` `OpRef` (9-byte, 36^9 namespace) serves as the canonical step/action identifier.

#### 3.8 Deadlock Handling

Saga steps rarely hold cross-service locks, so classic DB deadlocks are local to a participant; the saga-level deadlock is a **wait cycle between sagas/steps** (step A waits for step B's completion while B waits for A, across domains). Handling (Purdue CS542 deadlock lectures; ACM distributed wait-for graph):
- Model a wait-for graph: nodes = in-flight steps/sagas; edge Ti → Tj when Ti waits on a resource/step owned by Tj.
- Detect cycles (periodic WFG traversal or on wait timeout).
- Break cycles by choosing a victim (youngest/lowest-priority or fewest retries) and failing it with a clear error → its saga compensates, releasing the wait.
- Prevention alternatives: wait-die / wound-wait timestamp ordering; total ordering of resource acquisition; bounded wait timeouts so waits never block indefinitely (`SagaConfig.step_timeout_ms`).
`ids-saga` records `waits_on` edges per step, runs cycle detection on timeout, and exposes a deadlock event (`FailureEvent`) to `record_failure`/recovery.

#### 3.9 Livelock Prevention

Livelock = steps keep retrying/colliding without progress (symmetric retries, retry storms, CAS loops invalidating each other). Prevention (systemoverflow livelock prevention; AWS backoff+jitter; Baxos random exponential backoff):
- **Randomized exponential backoff with jitter** between retries breaks synchronization; never fixed-interval retries.
- **Bounded attempts + escalation:** after N attempts, serialize via coordinator lock or fail fast (circuit breaker open → fail immediately, recover later).
- **Asymmetric conflict resolution:** lower step/saga ID wins the contested resource, or the coordinator assigns exclusive ownership.
- **Retry budgets** cap retries per time window to prevent load amplification during degradation.
Livelock is harder to observe than deadlock (no blocked thread), so `ids-saga` exposes retry-count and retry-storm metrics via observability types and fails closed when budgets exhaust.

#### 3.10 Cross-Domain Transaction Coordination

A saga whose steps touch multiple failure domains (regions, hunits, administrative domains) needs coordination across domains without a global master (ids-000-aa invariant `NoGlobalMaster`). Design:
- One domain hosts the orchestrator/SEC for the saga; remote participants are addressed solely by `ids-carrier::Endpoint` (`CarrierType` + opaque `Vec<u8>`); no IP/TCP/UDP assumptions.
- Each domain's steps are local transactions; cross-domain effects are compensated through the saga log (continuation vs compensation principles — AWS prescriptive guidance: platform failure → forward recovery/retry; application failure → backward recovery/compensation).
- Two-domain atomic commit is approximated by saga + compensations, not 2PC (2PC's coordinator-blocking problem is unacceptable; 2PC vs saga research: StackOverflow, thedesignround).
- Cross-domain failure domains must not overlap in correlated ways (FailureDomain checks from ids-000-aa invariants) so a domain outage doesn't take both coordinator and participants.
- Idempotency keys + outbox span domains so redelivery across a partition heals exactly once.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Half-applied distributed transaction (step T3 fails after T1, T2 committed) | Reverse-order compensation of all completed steps; saga state machine guarantees terminal Completed/Compensated/Aborted |
| Compensation not run (orchestrator crash mid-rollback) | Durable saga log + checkpoints persisted on every transition; recovery replays log and resumes compensation (`SagaPersistenceManager`, `SagaCheckpointManager`) |
| Compensation itself fails | Bounded compensation retries; continue compensating remaining steps; escalate to `ManualIntervention` + dead-letter alert on exhaustion |
| Duplicate step/compensation delivery (at-least-once carrier) | Idempotency keys (saga ID + step ID); dedup record committed in same local transaction as effect; conditional updates; transactional outbox |
| Non-idempotent or non-compensatable step placed unsafely | Pivot/point-of-no-return discipline: irreversible steps last; validation step rejects saga configs lacking compensations for compensatable steps |
| Retry storm / synchronized retries | Exponential backoff with jitter; bounded attempts; retry budgets; circuit breaker |
| Livelock (steps collide forever) | Randomized backoff breaks symmetry; bounded attempts + escalation; asymmetric conflict resolution by step ID |
| Saga-level deadlock (wait cycle across steps/domains) | Wait-for graph edges per step; cycle detection on timeout; victim abort with compensation |
| Carrier-agnostic rule violation (std::net/tokio::net/IP/etc.) | `ids-carrier::Endpoint` is the only address type; dependency audit bans networking crates; clippy/CI rule + carrier-agnostic test |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces; `ids-saga` uses only type surface |
| Type divergence (redefined SagaId/StepId) | Single canonical definitions in `ids-000-aa`; `ids-saga` imports `ids_000_aa::X` |
| Cross-domain partition leaves saga indeterminate | Idempotency keys + outbox heal on redelivery; coordinator domain vs participant domain isolation; time-bound retries then compensation |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 029 ||
||-------|-------|-------------------|
| `ids-000-aa` | Master crate: canonical types (`SagaId`, `StepId`, `OpRef`, `NodeId`, `DatasetId`, `RegionId`, `HUnitId`, `IdsTime`, `LocalTransaction`, `FailureEvent`, `FragmentId`, `TransactionId`, `Version`, `Epoch`, `SagaResult`), sealed `trait Saga` (+ `SagaConfig`, `StepDefinition`, `StepState`, `SagaState`, `StepExecutionResult`, `CompensationResult`, `CompensationBatchResult`, `SagaRecoveryResult`, `RecoveryAction`, `SagaAbortResult`, `SagaTransitionResult`, `SagaStats`, `SagaError`), reference engine (`SagaEngine`, `SagaStepCoordinator`, `SagaTimeoutManager`, `SagaPersistenceManager`, `SagaCheckpointManager`), `SagaStateMachine` |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive for step commands, replies, and compensation messages across nodes/domains; `ids-saga` never opens a socket |
| `ids-crypto` | Allowed crypto primitives (ed25519 signatures for step/command authenticity; xchacha20poly1305/chacha20poly1305 AEAD for command payloads; sha256/blake3 for step/dedup digests) — type-level surface declared in `ids-000-aa` (`Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum`) |
| `ids-time` | `IdsTime` tick timestamps for step execution, compensation, timeouts (`SagaConfig.step_timeout_ms`) — not wall clock |
| `ids-write` | Local transaction source: durable WAL commit log that each saga step's local transaction commits to |
| `ids-sync` (Phase 018) | Consumer: replica reconciliation/convergence uses saga outcomes as convergence points |
| `ids-broadcast` (Phase 020) | Consumer: ordering guarantees for choreography events |
| `ids-coordinator` | Cross-domain saga coordination scaffold |

**Integration points:**
- `ids-saga` implements `ids_000_aa::contracts::Saga` (sealed trait, `impl private::Sealed`); downstream crates use it as a trait bound but cannot implement it.
- Step/compensation traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets; no `std::net`/`tokio::net` anywhere in `ids-saga`.
- Crypto: `ids-saga` references `Signature`/`KeyId`/`Checksum` types only; `ids-crypto` performs the operations.
- Cargo workspace: `ids-saga` is a workspace member; dependents declare `ids-saga = { path = "../ids-saga" }`.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-saga` (new crate at `C:/idfs/ids-saga/`; depends on `ids-000-aa`, `ids-carrier`, `ids-time`)

```
ids-saga/
├── Cargo.toml                          # thiserror, serde, blake3; workspace member; no networking crates
├── lib.rs                              # crate root; feature flags; public re-exports
├── error.rs                            # SagaCrateError (thiserror) wrapping ids_000_aa::SagaError
├── prelude.rs                          # re-export glob for downstream
├── constants.rs                        # defaults: max_steps=10, max_compensation_attempts=3, step_timeout_ms=30000, ...
│
├── orchestration/                      # orchestrator / Saga Execution Coordinator
│   ├── mod.rs
│   ├── coordinator.rs                  # SEC: drives steps, persists state, retries, timeouts
│   ├── saga_log.rs                     # durable append-only saga log of transitions/events
│   ├── step_driver.rs                  # sends step commands, awaits replies, handles step-failed
│   ├── compensation_driver.rs          # reverse-order compensation walk
│   ├── recovery.rs                     # crash recovery: replay log, resume or compensate
│   ├── orchestrator_state.rs           # in-memory saga registry + persistence adapter
│   └── failover.rs                     # standby coordinator, leader/standby handoff (no networking crate)
│
├── choreography/                       # event-driven, decentralized participants
│   ├── mod.rs
│   ├── participant.rs                  # local state machine reacting to step events
│   ├── event_subscription.rs           # topic/correlation bindings over carrier endpoints
│   ├── event_publisher.rs              # emits step-done / failure events as datagrams
│   ├── correlation.rs                  # saga ID + step ID correlation across events
│   └── choreography_engine.rs          # binds participant + events into a saga
│
├── local_transaction/                  # per-step local ACID transaction boundary
│   ├── mod.rs
│   ├── transaction.rs                  # LocalTransaction wrapper around ids_000_aa::LocalTransaction
│   ├── commit_boundary.rs              # single local commit per step; visibility semantics
│   ├── outbox.rs                       # transactional outbox: effect + event in one commit
│   └── dedup_store.rs                  # (idempotency_key, result) dedup table interface
│
├── compensation/                       # compensating transactions
│   ├── mod.rs
│   ├── compensator.rs                  # reverse-order walk of completed steps
│   ├── compensation_plan.rs            # ordered list of Ci for completed T1..Tk
│   ├── escalation.rs                   # manual intervention path when compensation exhausts retries
│   ├── compensation_result.rs          # CompensationResult handling (wraps ids-000-aa type)
│   └── compensation_retry.rs           # bounded retry budget for compensations
│
├── state_machine/                      # saga + step state machines
│   ├── mod.rs
│   ├── saga_state_machine.rs           # Created→Executing→Completed/Failed→Compensating→Compensated/Aborted
│   ├── step_state_machine.rs           # Pending→Executing→Completed/Failed, Completed→Compensating→Compensated
│   ├── transitions.rs                  # guarded transition table; can_transition_to
│   └── persistence_adapter.rs          # persists state on every transition
│
├── retry/                              # retry semantics for steps
│   ├── mod.rs
│   ├── retry_policy.rs                 # max_attempts, base_delay, max_delay, error classification
│   ├── backoff.rs                      # exponential backoff with full/equal/decorrelated jitter
│   ├── retry_budget.rs                 # per-window retry caps to prevent load amplification
│   ├── circuit_breaker.rs              # open/half-open/closed; fail fast on persistent faults
│   └── attempt_tracker.rs              # per-step attempt counts persisted with saga state
│
├── idempotency/                        # exactly-once effect despite at-least-once carrier
│   ├── mod.rs
│   ├── idempotency_key.rs              # saga ID + step ID (+attempt) key type
│   ├── dedup.rs                        # dedup check + record in same local transaction
│   ├── dedup_window.rs                 # time/size bounded dedup entry retention
│   ├── conditional_update.rs           # WHERE-status guard pattern helpers
│   └── outbox_relay.rs                 # publishes outbox events to carrier endpoints
│
├── deadlock/                           # saga-level deadlock handling
│   ├── mod.rs
│   ├── wait_for_graph.rs               # nodes=steps/sagas, edges=waits_on
│   ├── cycle_detection.rs              # periodic/on-timeout cycle detection
│   ├── victim_selection.rs             # youngest/fewest-retries abort choice
│   └── deadlock_event.rs               # FailureEvent emission on deadlock
│
├── livelock/                           # livelock prevention
│   ├── mod.rs
│   ├── backoff_randomizer.rs           # jittered retry delays
│   ├── escalation.rs                   # bounded attempts → serialize-or-fail
│   ├── conflict_resolution.rs          # asymmetric ordering by step/saga ID
│   └── retry_storm_guard.rs            # retry budget + circuit-breaker integration
│
├── cross_domain/                       # cross-domain transaction coordination
│   ├── mod.rs
│   ├── domain_coordinator.rs           # one domain owns the SEC for a cross-domain saga
│   ├── domain_endpoint.rs              # Endpoint-based addressing for remote participants
│   ├── compensation_scope.rs           # per-domain compensation boundaries
│   ├── domain_failure.rs               # correlated-failure domain isolation (FailureDomain)
│   └── partition_heal.rs               # idempotency + outbox redelivery after partition
│
├── wire/                               # serialization over carrier datagrams
│   ├── mod.rs
│   ├── envelope.rs                     # versioned step/compensation datagram envelope
│   ├── command.rs                      # step command (execute, compensate, reply)
│   ├── reply.rs                        # step-completed / step-failed replies
│   ├── codec.rs                        # serde/bincode encode/decode with version tagging
│   └── framing.rs                      # datagram framing, MAX_ADDRESS_SIZE-aware payloads
│
├── persistence/                        # saga persistence (crash recovery)
│   ├── mod.rs
│   ├── store.rs                        # saga state store interface
│   ├── journal.rs                      # append-only transition journal
│   ├── snapshot.rs                     # periodic saga snapshots + journal truncation
│   └── recovery.rs                     # load snapshot + replay journal
│
├── types/                              # composed saga types re-exporting ids-000-aa
│   ├── mod.rs
│   ├── step_definition.rs              # re-export + validation helpers
│   ├── saga_config.rs                  # config validation wrappers
│   └── saga_stats.rs                   # stats aggregation helpers
│
├── tests/                              # conformance tests (no source modification)
│   ├── mod.rs
│   ├── orchestration_tests.rs
│   ├── choreography_tests.rs
│   ├── compensation_tests.rs           # reverse-order, idempotent, failing compensation
│   ├── state_machine_tests.rs
│   ├── retry_tests.rs                  # backoff+jitter, bounded attempts
│   ├── idempotency_tests.rs            # dedup, duplicate delivery
│   ├── deadlock_tests.rs               # WFG cycle detection, victim abort
│   ├── livelock_tests.rs               # randomized backoff, no storm
│   ├── cross_domain_tests.rs           # Endpoint-only addressing
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net usage
│   └── chaos_tests.rs                  # failure injection at every step (N + N tests)
│
├── benches/
│   ├── mod.rs
│   ├── compensation_latency.rs
│   ├── replay_recovery.rs
│   └── dedup_throughput.rs
│
├── spec/
│   ├── saga_state_machine.md
│   └── tla/                            # TLA+/PlusCal outline for two-outcome guarantee
│
├── docs/
│   ├── architecture.md
│   ├── choreography_vs_orchestration.md
│   └── compensation_guide.md
│
├── test_vectors/
│   ├── compensation_order_vectors.json
│   └── state_transition_vectors.json
│
├── .gitignore
└── README.md
```

Total: ~90–250 files (within the ~90–1000 target range; room for growth as failure-domain and cross-domain modules expand). All files are `.rs` source + `Cargo.toml` + tests; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **Forward execution:** orchestrator fetches next `Pending` step in `step_order` (by `order`), sends step command to participant `Endpoint`, persists `Executing`, awaits reply; on `Completed`, persist and advance; on `Failed`, persist `Failed` and begin compensation.
2. **Reverse-order compensation:** walk `saga.step_order.iter().rev()`; for every step with `state == Completed`, send compensation datagram; collect `CompensationResult`; all success → `Compensated`, any failure after retries → `Failed` + `ManualIntervention` escalation.
3. **Saga state-machine transition guard:** `SagaState::can_transition_to(from, to)` allows only `Created→Executing`, `Executing→Completed`, `Executing→Executing` (partial), `Failed→Compensating`, `Compensating→Compensated`; terminal states reject all transitions (`InvalidStateTransition` error).
4. **Retry with bounded exponential backoff + jitter:** `delay = random(0, min(cap, base * 2^attempt))`; attempt counter persisted per step; error classification decides retryable vs business failure; `max_attempts` exhausts → fail step → compensation.
5. **Idempotency dedup:** key = `(saga_id, step_id, attempt)`; participant checks dedup store inside its local transaction; hit → return cached result; miss → perform effect + record key atomically.
6. **Transactional outbox:** effect row + outbox event row commit in one local transaction; relay reads unsent outbox rows and sends datagrams to `Endpoint` targets; redelivery reuses same event ID → consumer dedupes.
7. **Deadlock detection:** wait-for graph nodes = in-flight steps, edge on `waits_on`; on wait timeout or periodic tick, run cycle detection; cycle → pick victim (fewest retries / youngest) → abort victim saga with compensation → emit `FailureEvent`.
8. **Livelock prevention:** every retry path applies randomized exponential backoff; after `max_attempts` escalate to coordinator-serialized execution or fail fast; retry budget bounds retries per time window.
9. **Cross-domain step:** coordinator domain sends step command to participant domain's `Endpoint`; participant runs local transaction + outbox; reply datagram returns outcome; domain failure is a `FailureEvent` triggering compensation scope boundary.
10. **Crash recovery:** load latest saga snapshot + replay journal transitions since snapshot; resume in-flight saga at its persisted step/cursor, or continue compensation; never reprocess a completed step (dedup keys).
11. **Saga stats:** `SagaStats` — completion rate, compensation success rate, failure rate, total execution time, aggregated per saga for observability.
12. **Two-outcome guarantee invariant:** either all steps `Completed`, or a prefix `Completed` + compensations run for them → `Compensated`/`Aborted`; checked by `is_complete()` / `is_compensated()` invariants at terminal transition.

---

### 8. Dependencies

`ids-saga/Cargo.toml`:
- `ids-000-aa = { path = "../ids-000-aa" }` — canonical types + sealed `trait Saga`
- `ids-carrier = { path = "../ids-carrier" }` — opaque `Endpoint` datagrams
- `ids-time = { path = "../ids-time" }` — `IdsTime` ticks
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — wire envelope + types
- `blake3 = "1.5"` — step/dedup digest (content hashing only)
- `serde_json = "1"` (dev) — test serialization
- `proptest` (dev) — state-machine property tests

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; driver is the carrier's receive loop. `cargo tree` must show no networking crates.

Downstream crate dependencies:
- `ids-sync` → `ids-saga`, `ids-000-aa`
- `ids-broadcast` → `ids-saga`, `ids-000-aa`
- `ids-coordinator` → `ids-saga`, `ids-000-aa`

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | Sealed `trait Saga` implemented | `ids-saga` has `impl private::Sealed for ...` + `impl Saga for ...` covering `create_saga`, `execute_step`, `compensate_step`, `compensate_all`, `abort`, `recover_saga` |
|| 2 | All canonical types imported from `ids-000-aa` | `SagaId`, `StepId`, `StepState`, `SagaState`, `LocalTransaction`, `FailureEvent`, `OpRef`, `IdsTime` resolved via `ids_000_aa::`; none redefined in `ids-saga` |
|| 3 | Both saga styles | `orchestration/` coordinator module + `choreography/` participant module each build and have unit tests |
|| 4 | Reverse-order compensation | Test: 3-step saga, fail step 3 → compensations invoked for step 2 then step 1 (order asserted) |
|| 5 | Compensation idempotency | Duplicate compensation delivery → same effect once; dedup key stored in same local transaction as effect |
|| 6 | Idempotent step execution | Duplicate step command → cached result returned, no double effect |
|| 7 | Retry semantics | Transient failure retried with bounded attempts + jittered backoff; business failure → compensation, no retry |
|| 8 | Backoff+jitter bounds | Delays within [0, min(cap, base*2^attempt)]; 1000 simulated concurrent retries show desynchronization (no lockstep) |
|| 9 | Deadlock handling | Wait-for graph cycle → victim abort + compensation; no indefinite wait beyond `step_timeout_ms` |
|| 10 | Livelock prevention | Symmetric retry collision resolves within bounded attempts; retry budget caps load |
|| 11 | Carrier-agnostic | Source grep for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches; `cargo tree` has no networking crates |
|| 12 | Allowed crypto surface only | `Cargo.toml` has no crypto beyond `blake3` digest; crypto ops go through `ids-crypto` types |
|| 13 | Crash recovery | Kill orchestrator mid-saga; restart; saga resumes from persisted checkpoint/journal; no duplicate step execution |
|| 14 | Two-outcome guarantee | Every test saga ends in `Completed`, `Compensated`, or `Aborted` — never intermediate |
|| 15 | Crate compiles cleanly | `cargo check -p ids-saga` exits 0 with no warnings |
|| 16 | Conformance tests pass | `cargo test -p ids-saga` passes all modules including `carrier_agnostic_tests` and `chaos_tests` |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| Which crate owns each saga concern at runtime (orchestrator in `ids-saga` vs a dedicated `ids-coordinator`) | Affects `impl Saga for X` placement and coordinator failover design | Decided when `ids-coordinator` is created; `ids-saga` provides the SEC reference implementation |
| Whether retries/compensations run on the carrier's receive loop or a dedicated task | Concurrency model, async vs sync | No async runtime in Phase 029; driver pattern deferred to carrier crate design |
| Wire envelope format for step/compensation datagrams (serde-derived vs bincode vs CBOR) | Frame size, versioning | `wire/codec.rs` uses serde; format decision deferred to `ids-protocol` phase |
| Dedup store backing (in-memory vs `ids-store`/WAL) | Crash durability of idempotency keys | Interface in `dedup_store.rs`; implementation chosen with `ids-store` phase |
| TLA+ scope for the two-outcome guarantee | Formal verification effort | `spec/tla/` outline in plan; full model checking deferred |
| Compensation across uncompensatable external effects (email, courier) | Business correctness | Ordering rule: irreversible steps last; documented in `compensation_guide.md` |
| Cross-domain coordinator failover mechanics | Availability of multi-domain sagas | Deferred to `ids-coordinator` phase; failover scaffolding in `orchestration/failover.rs` |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
| Separate `ids-saga` crate (not folders inside `ids-000-aa`) | Matches "try to use separate crates instead of more folders in existing ones"; single control point per concern |
| Implement sealed `trait Saga` from `ids-000-aa` | Canonical contract already defined; downstream uses as bound, cannot implement |
| Both choreography and orchestration in one crate | prompts.txt entry 29 lists both; shared state machine, retry, idempotency, compensation code |
| Compensation = new forward transaction, not rollback | Semantic compensation per Garcia-Molina & Salem / Azure; original effect stays auditable |
| Reverse compensation order, idempotent + retriable compensations | Required by saga correctness (IBM, Azure, Temporal, Cloudflare Workflows) |
| Bounded exponential backoff + jitter for retries | AWS Architecture Blog; prevents retry storms and thundering herds; bounded attempts guarantee termination |
| Idempotency keys + dedup inside local transaction; outbox for events | Exactly-once effect over at-least-once carrier; no dual-write window |
| Saga-level wait-for graph + victim abort for deadlocks | Distributed deadlock detection (Purdue/ACM); steps hold no cross-service locks, so cycle detection is saga-local |
| Randomized backoff + bounded attempts + circuit breaker for livelock | Classic livelock prevention; converts potential livelock into bounded delay or failure |
| `ids-carrier::Endpoint` sole address type; no networking crates | Carrier-agnostic absolute rule (IDS policy) |
| Crypto: ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 only via `ids-crypto` | IDS allowed-crypto policy; `ids-saga` consumes type surface only |
| Durable saga log + checkpoints on every transition | Crash recovery without re-executing completed steps (Eventuate Tram, Temporal) |

---

### 12. Sources

1. Garcia-Molina, H. & Salem, K. — "Sagas," Proc. ACM SIGMOD Int. Conf. on Management of Data, 1987, pp. 249–259. https://dl.acm.org/doi/10.1145/38713.38742 (also: https://www.cs.cornell.edu/andru/cs711/2002fa/reading/sagas.pdf) — original saga paper; two-outcome guarantee, compensating transactions.
2. IBM Developer — "Solving distributed transaction management problems in microservices architecture using Saga." https://developer.ibm.com/articles/use-saga-to-solve-distributed-transaction-management-problems-in-a-microservices-architecture — compensating actions idempotent + retriable; Saga Execution Coordinator (SEC) and saga log.
3. Microsoft Azure Architecture Center — "Saga Design Pattern." https://learn.microsoft.com/en-us/azure/architecture/patterns/saga — compensable/pivot/retriable transactions; orchestration vs choreography; limitations of compensation.
4. AWS Prescriptive Guidance — "Saga patterns," "Saga orchestration pattern," "Saga choreography pattern." https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/saga-patterns.html — local transactions, continuation vs compensation, forward/backward recovery, orchestrator + participants.
5. AWS Architecture Blog — "Exponential Backoff and Jitter." https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter — capped exponential backoff with jitter prevents retry storms/synchronized retries.
6. Microsoft Azure — "Compensating Transaction pattern." https://learn.microsoft.com/en-us/azure/architecture/patterns/compensating-transaction — undo work in eventually consistent multi-step operations.
7. Microsoft Azure — "Idempotent Consumer Pattern." https://learn.microsoft.com/en-us/azure/architecture/patterns/idempotent-consumer — duplicate message handling, dedup within the same transaction.
8. Apache Camel — "Saga EIP." https://camel.apache.org/manual/latest/saga-eip.html — orchestration-style saga, per-step compensation endpoints, reverse-order compensation on cancellation.
9. Eventuate Tram — eventuate-tram-sagas README. https://github.com/eventuate-tram/eventuate-tram-sagas/blob/master/README.adoc — command/compensation per step, sequential forward execution, reverse-order compensating transactions.
10. Temporal — "Saga pattern" design pattern docs. https://docs.temporal.io/design-patterns/saga-pattern — local transactions with compensations registered per step, run in reverse order on error; durable execution.
11. fraiseql-saga — Rust saga crate docs. https://docs.rs/fraiseql-saga/latest/fraiseql_saga — Rust reference for saga coordinator, compensator, crash recovery, Postgres-backed store.
12. AWS Step Functions — saga pattern implementation with bounded retries (Retry blocks, BackoffRate, JitterStrategy). https://awsforengineers.com/blog/aws-step-functions-saga-pattern-implementation — idempotent action keys, bounded exponential backoff + jitter, reverse-order compensation.
13. CockroachDB — "Transaction Retry Error Example." https://docs.cockroachlabs.com/docs/v23.1/transaction-retry-error-example — retry on serialization errors with exponential backoff; max_retries bound.
14. Purdue CS542 — Distributed deadlocks lecture (wait-for graph, wait-die/wound-wait, cycle detection). https://cs.purdue.edu/homes/bb/cs542-20Spr/slides/lecture/17_Deadlocks.pdf
15. ACM — distributed deadlock detection via transaction wait-for graphs, cycle breaking by victim abort. https://dl.acm.org/doi/pdf/10.1145/319702.319717
16. Livelock prevention — randomized exponential backoff, bounded attempts, circuit breakers, asymmetric conflict resolution. https://systemoverflow.com/learn/deadlocks/livelock-starvation/livelock-starvation-preventing-livelock ; https://systemoverflow.com/learn/distributed-primitives/idempotency/retry-policies-exponential-backoff-jitter-and-budgets
17. Baxos — "Random Exponential Backoff" for lock-free contention resolution. https://arxiv.org/pdf/2204.10934 — REB breaks proposer collisions without a leader.
18. "Recovery within long-running transactions" (backward vs forward recovery, compensations). https://doi.org/10.1145/2480741.2480745
19. Saga testing — chaos injection, failure at every step, N + N compensation tests. https://helpmetest.com/blog/saga-pattern-testing ; https://adhdecode.com/articles/saga-pattern/saga-simulation-testing
20. The New Stack — saga choreography vs orchestration tradeoffs. https://thenewstack.io/implement-saga-patterns-in-microservices-with-nestjs-and-kafka
21. Transactional outbox / dual-write problem. https://builder.aws.com/content/3JoT1IGGqqckc82pZpWlBuRClcP/building-reliable-microservices-with-amazon-sqs-and-the-outbox-pattern ; https://debezium.io/blog/2020/02/10/event-sourcing-vs-cdc
22. Vercel — saga pattern guide (compensation is not rollback; idempotent compensations). https://vercel.com/i/saga-pattern
23. TLA+ modeling of distributed transactions (MongoDB transactions; trace validation vs TLC). https://conf.tlapl.us/2025-etaps/demirbas.pdf ; https://arxiv.org/pdf/2404.16075v2
24. IDS prompts.txt entry 29 (verbatim). C:/idfs/prompts.txt lines 430-444.
25. CRATES.md §29 (ids-saga contents). C:/idfs/CRATES.md lines 224-226.
26. Existing ids-000-aa skeleton — `src/contracts/saga.rs` (sealed `trait Saga` + all saga types + reference engine). C:/idfs/ids-000-aa/src/contracts/saga.rs.
27. IDS plan format reference. C:/idfs/plans/plan-001.md, plan-017.md; C:/hx-241/plans/plan-077.md.

---

**End of plan-029.md. Research complete — 30 targeted searches executed. No code written, no files created, no repository changes made beyond this plan document.**
