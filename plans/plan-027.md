## Phase 027 — Create the ids-fault crate: Research Findings & Implementation Blueprint

**Status:** Research complete (30 targeted searches executed). No code written, no files modified. This document is the blueprint for implementation.

---\

### 1. Executive Summary

Phase 027 creates `ids-fault`, IDS crate #27 (CRATES.md entry 27), dedicated to failure isolation and graceful degradation. The crate implements a composable resilience stack:

- **Circuit breakers** with three states (Closed / Open / Half-Open), failure thresholds, monitoring windows, reset timeouts, and per-node / global scope.
- **Bulkhead isolation** — resource pools partitioned per dependency/workload so failures stay in their compartment.
- **Load shedding** — bounded admission, priority-based shedding, CoDel-style queue delay control under overload.
- **Backpressure** — bounded queues, credit/pressure signals, pull-based consumption matched to downstream capacity.
- **Graceful degradation hierarchy** — ordered degradation levels (full → degraded → minimal → maintenance), recoverable in reverse order.
- **Retry storm / thundering herd prevention** — exponential backoff with full jitter, retry budgets, dead-letter queues, request coalescing/single-flight.
- **Handshake circuit breakers** — connection-establishment failure isolation separate from steady-state traffic breakers.
- **Per-node circuit breakers** (individual peer/dependency) and **global circuit breakers** (aggregate failure-domain scope).

Verbatim prompt (C:/idfs/prompts.txt lines 400-414): "Phase 027 — Create the ids-fault crate: Failure isolation and graceful degradation."

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305. `ids-000-aa` is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only this plan document.

Timing uses `ids-time::Idstime` (IDS proprietary time basis), never wall-clock. All failure-detector verdicts from `ids-failure-detector` (crate 7) feed `ids-fault` circuit-breaker decisions.

---\

### 2. Architecture Overview

`ids-fault` sits above `ids-000-aa` (types + sealed trait contracts) and `ids-carrier` (opaque `Endpoint` datagram send/receive). Consumers include `ids-membership`, `ids-recovery`, `ids-chaos`, `ids-observability`, and `ids-broadcast`. Each resilience primitive is an independent module that can be composed into protection pipelines:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   CONSUMERS (ids-* crates)                          │
│  ids-membership · ids-recovery · ids-chaos · ids-observability ·   │
│  ids-broadcast · ids-consistency-profile                             │
├─────────────────────────────────────────────────────────────────────┤
│              Phase 027: ids-fault (THIS CRATE)                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ circuit     │ │ bulkhead    │ │ load        │ │ backpressure│   │
│  │ breaker     │ │ isolation   │ │ shedding    │ │             │   │
│  │ (closed/    │ │ (resource   │ │ (admission  │ │ (bounded    │   │
│  │  open/      │ │  pools)     │ │  control,   │ │  queues,    │   │
│  │  half-open) │ │             │ │  priority)  │ │  credit)    │   │
│  └─────────────┘ └─────────────┴───────────────┴─────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ graceful    │ │ retry       │ │ handshake   │ │ degradation │   │
│  │ hierarchy   │ │ storm       │ │ circuit     │ │ levels      │   │
│  │ (levels 0-3)│ │ prevention  │ │ breaker     │ │             │   │
│  │             │ │ (backoff,   │ │             │ │             │   │
│  │             │ │  jitter,    │ │             │ │             │   │
│  │             │ │  budget)    │ │             │ │             │   │
│  └─────────────┘ └─────────────┴───────────────┴─────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (ids-carrier::Endpoint only)           │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, &[u8])               │
│  receive_datagram → (Endpoint, Vec<u8>)                             │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└─────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
1. All inter-node traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), never direct sockets.
2. Timing keyed to `ids-time::Idstime`; no wall-clock `std::time::Instant/SystemTime` for failure decisions.
3. Crypto limited to ed25519 (signed breaker state changes, handshake attestation), xchacha20poly1305 / chacha20poly1305 (optional payload encryption), sha256 / blake3 (digests, state commitments).
4. `ids-000-aa` provides canonical types (`NodeId`, `Endpoint`, `CarrierType`, `FailureDomain`) and sealed trait contracts; `ids-fault` implements them.
5. Each breaker is per-node (per `NodeId`) unless explicitly global; global breakers aggregate over failure domains.
6. Fail fast + fail safe: open circuits reject immediately with a typed error and optional fallback; never hang waiting.

**Request flow for a protected outbound call:**
1. Caller wraps operation in `CircuitBreaker::call(operation, fallback)`.
2. Breaker checks state: Closed → proceed; Open → fast-fail with fallback (no underlying call); Half-Open → admit limited probe count, reject rest.
3. On failure, record outcome in rolling window; if failure rate/threshold breached → trip to Open.
4. Open state starts reset timeout (Idstime-based); on expiry → Half-Open.
5. Half-Open probes: all succeed → Closed; any fail → re-open with backoff.
6. Bulkhead caps concurrency per dependency before the call; load shedder rejects excess when saturated; backpressure signals caller to slow down.

---\

### 3. Detailed Research Dimensions

#### 3.1 Circuit Breaker States and Transitions

Three-state finite state machine (Martin Fowler / Release It / Azure / Resilience4j):

| State | Behavior | Transition out |
|-------|----------|----------------|
| CLOSED | Requests pass through; failures counted in rolling window | failure rate ≥ threshold → OPEN |
| OPEN | All calls rejected immediately (fast-fail); fallback returned | reset timeout elapsed → HALF-OPEN |
| HALF-OPEN | Limited probe calls admitted | all probes succeed → CLOSED; any fail → OPEN |

Configuration parameters (research-verified typical values):
- `failure_threshold`: 5–10 consecutive failures, or 50% failure rate over window (Resilience4j default).
- `monitoring_window`: sliding window 10 seconds–1 minute, count-based or time-based.
- `reset_timeout` (sleep window): 30–60 seconds before trying Half-Open.
- `success_threshold` / probe count: 1–3 probe successes to close; 1 probe failure re-opens.
- `slow_call_threshold`: calls slower than X count as failures (e.g. 5 seconds).
- `permitted_calls_in_half_open`: 1–3.

Transitions must be atomic under concurrency (CAS on state + explicit half-open gate) to prevent thundering herds when many callers observe HALF_OPEN simultaneously.

Manual override: force-open / force-close for operators; reset clears counters.

#### 3.2 Thresholds and Monitoring Windows

- **Count-based window:** last N calls (e.g. 100); decision only after window is full (Resilience4j/SmallRye pattern).
- **Time-based window:** rolling time window (e.g. 10 s); `VecDeque<CallRecord>` with timestamped outcomes, old records evicted (tower-resilience pattern).
- **Failure rate threshold:** failures / total ≥ threshold (e.g. 50%).
- **Slow call rate:** calls exceeding `slow_call_duration_threshold` count as slow-call failures.
- Minimum calls before any tripping decision (avoid tripping on noise): e.g. 20 calls in window (Hystrix `requestVolumeThreshold`).

Thresholds are configurable budgets per breaker instance, not hard-coded constants, so misbehaving peers degrade gracefully.

#### 3.3 Backoff Strategies

Exponential backoff with full jitter is the industry standard for retry spacing (AWS Architecture Blog; systemoverflow retry-policies):

`delay = Random(0, min(cap, base × 2^attempt))`

- Base delay e.g. 100 ms; multiplier 2; cap e.g. 8–10 seconds.
- Full jitter randomizes each delay between 0 and the backoff cap, desynchronizing clients and preventing synchronized retry storms.
- Retry budget: retries consume at most ~10% additional requests per second; abandon retries when remaining deadline cannot fit another attempt.
- Max attempts bounded (e.g. 4 total = 1 original + 3 retries) to prevent infinite loops.
- Jitter variants: full jitter, equal jitter (base + random(0, base)), decorrelated jitter.

Backoff applies both to retry scheduling inside `ids-fault` and to circuit-breaker reset/backoff escalation when Half-Open probes keep failing (increasing reset timeout on repeated trips).

#### 3.4 Per-Node vs Global Circuit Breakers

- **Per-node circuit breaker:** one `CircuitBreaker` instance per `NodeId`/dependency. Tracks that peer's outcomes independently. Local-only state is faster and simpler; some instances may keep calling a failing peer while others have opened — acceptable, primary benefit is preventing a single instance from hammering a failing downstream.
- **Global circuit breaker:** aggregate state over a failure domain or the whole system. Circuit state stored/shared so all instances react consistently (Redis-shared pattern adapted to IDS: state propagated via `ids-carrier::Endpoint` datagrams, not a network service). If >X% of instances report OPEN, treat the service as globally failing.
- **Failure-domain scope:** breakers scoped by `FailureDomain` (from `ids-topology`) so correlated failures (region/H-unit down) trip domain-level breakers without affecting unrelated domains.
- Handshake circuit breaker: separate breaker for connection establishment (handshake failure isolation), distinct from steady-state traffic breakers, so handshake storms don't trip data-plane breakers.

#### 3.5 Bulkhead Isolation

Named after ship watertight compartments (Azure / Microsoft patterns). Isolate elements into pools so one failure doesn't cascade:

- **Thread-pool bulkhead:** separate thread pool per dependency/workload with fixed size and small/no queue. If one pool exhausts, others unaffected.
- **Semaphore bulkhead:** callers acquire a permit (zero timeout) before the call; no permit → reject/fail fast immediately. No timeout possible, but cheaper than thread pools.
- **Resource pools:** connections, workers, CPU, memory, queues, consumers partitioned per service.
- Trade-off: reduced average utilization (reserved idle capacity) vs predictable blast radius.

IDS adaptation: bulkheads keyed by `NodeId`/dependency, using semaphore-based concurrency caps (no threads — carrier-agnostic, async-friendly), plus per-dependency queue bounds.

#### 3.6 Load Shedding

Drops traffic as the server approaches overload to keep latency low for accepted requests (Google SRE "Addressing Cascading Failures"; Uber Cinnamon; AWS Builder):

- **Procrustean (static) shedding:** observe local measure (CPU, memory, queue length); drop a fraction when above safe level.
- **CoDel (controlled delay):** shed based on queue *waiting time*, not length; under pressure switch toward LIFO to favor fresh requests.
- **LIFO under overload:** old requests likely abandoned/retried by clients; failing them fast frees capacity for fresh, relevant requests.
- **Priority-aware shedding (Cinnamon):** request rank/tiers (t0–t5); shed lowest priority first; protect critical user-facing traffic.
- **Per-request TTL / deadline:** throw away doomed requests that have waited too long; propagate deadlines through the stack.
- **Enforced client timeouts on server:** prevent requests piling up behind unresponsive downstream.

IDS adaptation: admission control with bounded queue, LIFO-under-pressure, per-peer and per-class priority, deadline propagation via `ids-time::Idstime`.

#### 3.7 Backpressure

Matches inflow to downstream processing capacity to prevent overload, buffer overflow, cascading failures:

- **Bounded buffers:** queues with capacity; full queue triggers backpressure signal (reject, slow down, or shed).
- **Feedback loop:** receiver monitors load and signals sender to pause/slow.
- **Pull-based consumption:** downstream pulls at its own rate; sender cannot push faster than consumed.
- **Congestion avoidance:** controlling data rate avoids latency, loss, crashes.
- **Rate limiting / credit:** token-bucket or credit-based admission; sender only sends when credits available.

IDS adaptation: credit-based backpressure over `ids-carrier::Endpoint`, bounded delivery buffers with gap watermarks, pressure signals propagated as typed `Endpoint` payloads.

#### 3.8 Graceful Degradation Hierarchy

Reduces work under overload while preserving availability (Google SRE; HiLO product-level degradation; arika.dev levels):

| Level | Semantics |
|-------|-----------|
| 0 — Full | All features, live data, all services available |
| 1 — Degraded | Core user journey works; non-critical features disabled; stale data acceptable |
| 2 — Minimal | Read-only only; cached/pre-computed data; no writes |
| 3 — Maintenance | Static/status page only; system unavailable |

Degradation is **criticality-classified**: not all reads/writes deserve the same fallback. Critical traffic preserved to replicas first; sheddable traffic last. Recovery restores classes in order (critical → degradable → sheddable) behind a slowstart ramp to avoid re-spike. Degradation levels are pre-validated product states (design-time), not discovered at runtime.

#### 3.9 Retry Storms and Thundering Herd Prevention

- **Retry storm:** failed operation → clients retry → more load → more failures. Bounded attempts + exponential backoff + jitter + retry budgets prevent amplification. A 5-deep call stack with 3 retries per layer multiplies load by 243 without controls.
- **Thundering herd:** many clients wake/retry for the same event simultaneously (cache expiration, service recovery). Fixes: request coalescing/single-flight (one request per key, others wait), jitter before acting, `accept_mutex`-style exclusive wakeup, exponential backoff with full jitter.
- **Dead-letter queue:** repeatedly failing messages isolated after budget exhaustion so they don't poison healthy traffic.
- **Idempotency:** retries must not cause duplicate side effects; entry-ID dedup.

#### 3.10 Handshake Circuit Breakers

Separate breaker for connection/handshake establishment failures. Handshake storms (TLS churn, repeated connection attempts to a failing peer) are isolated from steady-state data-plane breakers so a handshake problem doesn't trip traffic breakers and vice versa. Handshake failure counted per peer; trips a handshake-specific breaker with its own threshold/backoff.

---\

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
|--------|-----------|-------------|
| Cascading failure from a slow/down dependency | Per-node circuit breaker trips → fast-fail + fallback; no retries into open circuit |
| Retry storm amplifying load on recovering service | Bounded attempts, exponential backoff + full jitter, retry budget (~10%), dead-letter queue |
| Thundering herd on recovery / cache expiry | Request coalescing (single-flight), jitter, exclusive wakeup, LIFO-under-pressure |
| One dependency exhausting shared resources (noisy neighbor) | Bulkhead isolation — semaphore/pool per dependency |
| Overload melting down server (RAM, health checks, latency) | Load shedding with priority tiers, CoDel queue delay, per-request TTL/deadline |
| Stale requests queued behind fresh ones under overload | LIFO discipline under pressure, per-request TTL discard |
| Circuit state inconsistency across instances | Failure-domain scoped global breakers propagated via `ids-carrier::Endpoint` datagrams |
| Wall-clock dependence for failure decisions | `ids-time::Idstime` only; no `SystemTime`/`Instant` for breaker logic |
| Unbounded retry / backoff growth | Max attempts cap, backoff cap, retry budget |
| Fallback invoked for critical traffic | Criticality classification; critical reads/writes never shed automatically |
| Recovery re-spike tripping breakers again | Slowstart ramp; restore classes in order behind circuit-breaker close |

**Security risks:** forged breaker-state messages mitigated by ed25519 signatures on state transitions; breaker exhaustion as DoS vector mitigated by bounded windows and per-peer quotas; never trust unsynchronized wall clocks for security decisions.

---\

### 5. Integration with Existing Crates

|| Crate | Role in Phase 027 ||
|-------|-------|-------------------|
| `ids-000-aa` | Canonical types + sealed trait contracts (master crate) | `NodeId`, `Endpoint`, `CarrierType`, `FailureDomain`, error contracts, prelude — all fault types derive from here |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`), send/receive datagrams | The only traffic path; all breaker state/fallback/handshake datagrams are `Endpoint` payloads |
| `ids-crypto` | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signed breaker state changes, handshake attestation, digests, optional payload encryption, replay nonces |
| `ids-time` | Proprietary `Idstime` basis (not wall-clock/HLC) | Reset timeouts, monitoring windows, backoff schedules keyed to `Idstime` ticks |
| `ids-topology` | `FailureDomain` hierarchy, correlated failure modeling | Breakers scoped by failure domain; correlated-failure awareness prevents domain-wide oscillation |
| `ids-failure-detector` (#7) | SWIM gossip + phi accrual; suspected vs proven failure | Detector verdicts feed per-peer circuit-breaker trip decisions; proven failure → force-open |
| `ids-membership` | Membership / view changes | Breakers keyed by member `NodeId`; on leave/join, breakers created/retired |
| `ids-recovery` (#25) | Multi-level recovery | Circuit-breaker open/close events trigger recovery scope decisions; degradation levels gate recovery aggressiveness |
| `ids-chaos` | Fault injection / chaos testing | Injects failures to validate breaker trips, shedding, degradation under controlled faults |
| `ids-observability` | Metrics, tracing, alerting | Breaker state transitions, shed counts, backpressure signals exported as metrics/alerts |
| `ids-broadcast` (#20) | Reliable/causal/total-order broadcast | Broadcast delivery gated by per-peer circuit breakers; backpressure applied to broadcast ingress |

**Integration points:**
- `ids-failure-detector` verdicts (`suspected`/`proven failure`) feed per-peer breaker trip decisions; `proven failure` can force-open a breaker.
- `ids-topology::FailureDomain` scopes global breakers; domain-wide correlated failure trips domain breaker, not node breakers.
- `ids-membership` creates/retires breakers on member join/leave; stale breakers garbage-collected after member departure.
- `ids-observability` scrapes breaker state, failure rate, shed count, backlog depth; alerts on open circuits and sustained shedding.
- `ids-chaos` injects latency/failure to validate breaker tuning in test environments.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — all traffic via `ids-carrier::Endpoint` (I-CARR-001).

---\

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-fault` (depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-time`, `ids-topology`; `thiserror`, `bytes`, `tracing`; `proptest`/`bolero` for testing; no `tokio`, no networking crates).

```
ids-fault/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (threshold defaults, timeouts, queue caps)
├── error.rs                        # FaultError (thiserror): breaker open, shed, bulkhead full, backpressure
│
├── circuit_breaker/                # Three-state circuit breakers
│   ├── mod.rs
│   ├── breaker.rs                  # CircuitBreaker state machine (Closed/Open/Half-Open)
│   ├── state.rs                    # CircuitState enum + transition guards (CAS)
│   ├── config.rs                   # BreakerConfig: threshold, window, timeout, probe count
│   ├── window.rs                   # Rolling outcome window (count-based + time-based)
│   ├── per_node.rs                 # Per-NodeId breaker registry
│   ├── global.rs                   # Global / failure-domain scoped breaker
│   ├── handshake.rs                # Handshake circuit breaker (separate from data-plane)
│   ├── fallback.rs                 # Fallback / default-response on open circuit
│   ├── manual_override.rs          # Force-open / force-close / reset operators
│   └── types.rs                    # BreakerId, BreakerEvent, Outcome
│
├── bulkhead/                       # Resource-pool isolation
│   ├── mod.rs
│   ├── bulkhead.rs                 # Bulkhead: concurrency cap per dependency
│   ├── semaphore.rs                # Semaphore-based permit acquisition (zero-timeout)
│   ├── pool.rs                     # Resource pool per workload/dependency
│   ├── config.rs                   # BulkheadConfig: max concurrent, queue size
│   └── types.rs                    # BulkheadId, Permit
│
├── load_shedding/                  # Admission control under overload
│   ├── mod.rs
│   ├── shedder.rs                  # Load shedder: accept/reject decision
│   ├── priority.rs                 # Priority tiers (t0–t5), criticality classification
│   ├── codels.rs                   # CoDel-style queue-delay shedding
│   ├── lifo.rs                     # LIFO-under-pressure queue discipline
│   ├── ttl.rs                      # Per-request TTL / deadline discard
│   ├── config.rs                   # SheddingConfig: threshold, fraction, TTL
│   └── types.rs                    # ShedDecision, PriorityClass
│
├── backpressure/                   # Inflow matching to downstream capacity
│   ├── mod.rs
│   ├── controller.rs               # Backpressure controller: credit/pressure signals
│   ├── credit.rs                   # Token-bucket / credit-based admission
│   ├── buffer.rs                   # Bounded delivery buffer with gap watermark
│   ├── signal.rs                   # Pressure signal type (slow/pause/reject)
│   ├── config.rs                   # BackpressureConfig: buffer size, credit rate
│   └── types.rs                    # Credit, PressureLevel
│
├── degradation/                    # Graceful degradation hierarchy
│   ├── mod.rs
│   ├── hierarchy.rs                # Degradation levels 0–3 state machine
│   ├── levels.rs                   # Per-level semantics (full/degraded/minimal/maintenance)
│   ├── criticality.rs              # Criticality classification of traffic classes
│   ├── recovery_order.rs           # Restore classes in order behind slowstart ramp
│   ├── config.rs                   # DegradationConfig: level thresholds, hysteresis
│   └── types.rs                    # DegradationLevel, TrafficClass
│
├── retry/                          # Retry storm prevention
│   ├── mod.rs
│   ├── policy.rs                   # RetryPolicy: max attempts, backoff, budget
│   ├── backoff.rs                  # Exponential backoff with full jitter
│   ├── budget.rs                   # Retry budget (~10% extra requests/sec)
│   ├── dead_letter.rs              # Dead-letter queue for exhausted retries
│   ├── coalescing.rs               # Request coalescing / single-flight per key
│   ├── config.rs                   # RetryConfig: base delay, cap, jitter, budget
│   └── types.rs                    # RetryId, BackoffSchedule
│
├── handshake_breaker/              # Handshake circuit breaker (connection establishment)
│   ├── mod.rs
│   ├── breaker.rs                  # HandshakeBreaker: separate from data-plane breaker
│   ├── config.rs                   # HandshakeBreakerConfig
│   └── types.rs
│
├── registry/                       # Central breaker/bulkhead registry
│   ├── mod.rs
│   ├── registry.rs                 # Registry of all breakers/bulkheads by NodeId/domain
│   ├── garbage_collector.rs        # Retire stale breakers after member departure
│   └── types.rs
│
├── wire/                           # Serialization over ids-carrier::Endpoint
│   ├── mod.rs
│   ├── envelope.rs                 # Versioned wire envelope for breaker state datagrams
│   ├── format.rs                   # CBOR/bincode encoding (deterministic)
│   └── checksum.rs                 # Payload checksums (sha256/blake3)
│
├── crypto/                         # Crypto surface for fault crate
│   ├── mod.rs
│   ├── signer.rs                   # ed25519 sign/verify for breaker state transitions
│   ├── digest.rs                   # sha256/blake3 digests
│   └── aead.rs                     # xchacha20poly1305 / chacha20poly1305 (optional)
│
├── time/                           # Idstime adapter
│   ├── mod.rs
│   └── idstime.rs                  # Adapter to ids-time::Idstime (no wall-clock)
│
├── types/                          # Composed fault types
│   ├── mod.rs
│   ├── node_fault.rs               # Per-node fault aggregate
│   ├── domain_fault.rs             # Failure-domain fault aggregate
│   └── event.rs                    # FaultEvent (trip, close, shed, degrade)
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_breaker.rs
│   ├── fuzz_shedder.rs
│   └── fuzz_bulkhead.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── breaker_tests.rs            # State transitions, thresholds, concurrency
│   ├── property_tests.rs           # Proptest/bolero semilattice/idempotence properties
│   ├── shedding_tests.rs           # Load shedding under overload
│   ├── bulkhead_tests.rs           # Pool exhaustion isolation
│   ├── backpressure_tests.rs       # Credit/pressure signals
│   ├── degradation_tests.rs        # Level transitions, recovery order
│   ├── retry_tests.rs              # Backoff, jitter, budget, dead-letter
│   ├── handshake_tests.rs          # Handshake breaker isolation
│   ├── chaos_tests.rs              # Node churn, offline members, correlated failure
│   └── carrier_tests.rs            # Endpoint-only traffic (no sockets)
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── breaker_bench.rs            # Transition latency, window cost
│   ├── shedding_bench.rs           # Shed decision throughput
│   └── backpressure_bench.rs       # Credit signal latency
│
├── spec/                           # Formal specifications
│   ├── breaker_fsm.md              # Three-state machine laws
│   ├── degradation_lattice.md      # Degradation level partial order
│   └── tla/                        # TLA+/PlusCal (reuses ids-000-aa spec area)
│
├── docs/                           # Documentation
│   ├── architecture.md             # Fault isolation architecture
│   ├── integration_guide.md        # Integrating with ids-* consumers
│   └── tuning_guide.md             # Threshold/backoff tuning guidance
│
└── test_vectors/                   # Deterministic test vectors
    ├── breaker_vectors.json
    ├── shedding_vectors.json
    └── degradation_vectors.json
```

**Why ~90–1000 files:** each module has a single responsibility; every file contains production-grade code with full error handling, documentation, and unit tests. The count reflects: circuit breakers (per-node/global/handshake), bulkheads, load shedding, backpressure, degradation hierarchy, retry/backoff/jitter/budget/dead-letter/coalescing, registry, wire format, crypto surface, Idstime adapter, composed types, fuzzing, property tests, integration/chaos/carrier tests, benchmarks, formal spec, docs, test vectors. No stubs.

---\

### 7. Key Algorithms & Data Structures

#### 7.1 Circuit Breaker State Machine

```
trait CircuitBreaker {
    fn call(&mut self, op, fallback) -> Result<T, FaultError>;
}

// Closed: pass through; record outcomes; failure rate ≥ threshold → Open
// Open: reject immediately with CircuitOpen + fallback; reset timeout → Half-Open
// Half-Open: admit ≤ probe_count calls; all succeed → Closed; any fail → Open

state: CLOSED | OPEN | HALF_OPEN
failure_count, success_count, total_count (rolling window)
last_failure_time: Idstime
open_at: Idstime
reset_timeout: Duration (Idstime-based)
failure_threshold: u32 / failure_rate: f64
success_threshold: u32
permitted_in_half_open: u32
```

Transition concurrency: CAS on `OPEN → HALF_OPEN`; explicit gate rejects all arrivals during Half-Open until probe completes (prevents thundering herd of 200 concurrent probe attempts).

#### 7.2 Rolling Outcome Window

- Count-based: ring buffer of last N outcomes; decision only after window full.
- Time-based: `VecDeque<(Idstime, Outcome)>`; evict records older than `monitoring_window`.
- Stats: total, failures, slow calls; failure rate = failures/total.

#### 7.3 Exponential Backoff with Full Jitter

```
delay(attempt) = Random(0, min(cap, base * 2^attempt))
```

- base e.g. 100 ms; multiplier 2; cap e.g. 8 s.
- Full jitter desynchronizes clients; prevents synchronized retry storms.

#### 7.4 Load Shedding Decision

```
if queue_len >= max_queue: reject (shed)
elif queue_wait_time > codels_threshold: reject LIFO-first (fresh requests prioritized)
elif priority_class < min_accepted_class: reject low priority first
else: admit (apply credit/backpressure)
```

Per-request TTL: discard requests that have waited > deadline.

#### 7.5 Backpressure Credit Model

```
credit: u64 (tokens replenished at credit_rate)
on send: if credit >= cost: credit -= cost; proceed
         else: signal pressure (slow/pause/reject) to caller
on ack: credit += refund
```

Bounded delivery buffer with gap watermark; pressure signal propagated as typed `Endpoint` payload.

#### 7.6 Degradation Hierarchy (lattice)

Levels form a partial order: Full(0) > Degraded(1) > Minimal(2) > Maintenance(3). Traffic class criticality determines which classes are shed at each level. Recovery restores in reverse order behind slowstart ramp.

#### 7.7 Retry Budget

Retries consume ≤ retry_budget (e.g. 10%) of total requests/sec per client. Abandon when remaining deadline < backoff delay. Dead-letter queue captures messages exceeding max attempts.

#### 7.8 Global Breaker Aggregation

```
domain_open if fraction(instances reporting OPEN) ≥ global_threshold
```

State propagated via `ids-carrier::Endpoint` datagrams (not a network service); TTL-based expiry prevents stale state from crashed instances.

---\

### 8. Dependencies

|| Crate | Purpose ||
|-------|---------|---------|
| `ids-000-aa` | NodeID, Endpoint, CarrierType, FailureDomain, error contracts, prelude |
| `ids-carrier` | Opaque `Endpoint` datagram send/receive (only traffic path) |
| `ids-crypto` | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Proprietary `Idstime` (no wall-clock) |
| `ids-topology` | FailureDomain hierarchy for breaker scoping |
| `ids-failure-detector` (#7) | suspected/proven failure verdicts feed breaker trips |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `tracing` | Instrumentation |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

No `tokio`, no networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS). All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---\

### 9. Measurable Acceptance Criteria

|| Criterion | Target | Measurement ||
|-----------|----------|-------------|-------------|
| Circuit breaker states | exactly Closed/Open/Half-Open | unit test, state-machine coverage |
| Closed → Open trip | failure rate ≥ threshold within window | unit test, 100k iterations |
| Open fast-fail | zero underlying calls while open; fallback returned | unit test |
| Open → Half-Open | after reset timeout (Idstime-based) | unit test with Idstime mock |
| Half-Open probe | ≤ N probe calls; success → Closed, fail → Open | unit test |
| Half-Open concurrency gate | CAS + gate prevents herd; exactly one probe | concurrency test (200 callers) |
| Per-node breaker isolation | peer A open doesn't affect peer B | unit test |
| Global / domain breaker | ≥X% instances OPEN → domain open | integration test |
| Bulkhead concurrency cap | ≤ max concurrent per dependency; excess rejected immediately | unit test |
| Bulkhead isolation | pool A exhausted doesn't affect pool B | unit test |
| Load shedding | reject fraction when queue/time above threshold | unit + overload test |
| Priority shedding | low priority shed before critical | unit test |
| CoDel / LIFO-under-pressure | waiting-time-based shed; fresh requests favored | bench |
| Per-request TTL discard | requests > deadline dropped | unit test |
| Backpressure credit | credit ≥ cost required; pressure signal on exhaustion | unit test |
| Bounded buffer | never exceeds capacity; gap watermark | unit test |
| Degradation levels | 0→1→2→3 transitions; recovery reverse order | state-machine test |
| Criticality classification | critical traffic preserved under shed | unit test |
| Exponential backoff + jitter | delay = Random(0, min(cap, base*2^attempt)) | unit test (distribution) |
| Jitter desynchronization | 1000 clients → <50% synchronized retries | chaos test |
| Retry budget | retries ≤ 10% extra requests/sec | unit test |
| Max attempts cap | bounded attempts, no infinite loop | unit test |
| Dead-letter queue | exhausted-retries messages isolated | unit test |
| Request coalescing | N concurrent misses for same key → 1 upstream call | unit test |
| Handshake breaker | separate from data-plane breaker; independent threshold | unit test |
| Idstime-only timing | no SystemTime/Instant in breaker logic | audit + build check |
| Carrier agnosticism | compiles/behaves identically with zero direct socket deps | build + dependency audit |
| Crypto surface | only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency audit |
| Property tests | 100,000+ iterations | proptest/bolero |
| Chaos tests | partition, offline member, correlated failure | pass/fail |
| Breaker transition latency | O(1) state change | bench |
| Shed decision throughput | decisions/sec | bench |

---\

### 10. Remaining Uncertainties

1. **Breaker state sharing mechanism:** global/domain breakers need state propagated via `ids-carrier::Endpoint` datagrams; the gossip/anti-entropy pattern for breaker state (push vs pull, convergence) is undefined.
2. **Threshold defaults:** failure threshold, monitoring window, reset timeout values are workload-dependent; no production telemetry yet to calibrate.
3. **Half-open probe count under high concurrency:** CAS + gate prevents herd, but probe admission policy (1 vs N probes, queueing vs reject) unresolved.
4. **Degradation level triggers:** which metrics (CPU, queue length, error rate) trigger level transitions, and hysteresis to prevent oscillation, unquantified.
5. **Backpressure credit rate:** credit replenishment rate vs sender rate is workload-dependent; needs load testing.
6. **Circuit-breaker ↔ failure-detector coupling:** whether `proven failure` force-opens breakers immediately or only influences thresholds is a product decision.
7. **Breaker garbage collection:** when to retire per-node breakers after member departure (never too early — late-arriving datagrams) unresolved.
8. **Fallback semantics:** typed fallback values vs generic error; fallback chaining (fallback-of-fallback) policy undefined.
9. **Formal verification scope:** TLA+/PlusCal spec for breaker FSM and degradation lattice; extending to bulkhead/shedding interaction is additional effort.
10. **Interaction with ids-chaos:** chaos injection must respect breaker state (don't inject into open circuits) — contract undefined.

---\

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Three-state breaker (Closed/Open/Half-Open) | Canonical pattern (Fowler/Release It/Azure/Resilience4j); fail-fast + auto-recovery |
| Per-node breakers primary, global/domain breakers aggregated | Local state faster/simpler; global for correlated failure domains |
| Idstime for all timing, never wall-clock | Wall-clock drift/skew corrupts failure decisions (IDS time basis) |
| Semaphore bulkhead (no threads) | Carrier-agnostic, async-friendly; zero-timeout permit → fail fast |
| Exponential backoff + full jitter | Industry standard (AWS); desynchronizes clients, prevents retry storms |
| Retry budget ~10% + max attempts cap | Prevents load amplification during incidents |
| Load shedding with priority tiers + CoDel/LIFO | Shed low-priority first; fresh requests favored under overload |
| Degradation levels 0–3 with criticality classification | Preserve critical traffic; predictable degradation states |
| Request coalescing / single-flight | Collapses N concurrent misses to 1 upstream call |
| Dead-letter queue for exhausted retries | Isolates poison messages from healthy traffic |
| New `ids-fault` crate | Clean dependency boundary; integrates ids-000-aa/ids-carrier/ids-crypto/ids-time/ids-topology/ids-failure-detector |
| Carrier agnostic via `ids-carrier::Endpoint` only | Preserves I-CARR-001; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |
| Allowed crypto only (ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305) | HX-241/IDS crypto policy |
| Reject direct socket dependencies | Contradicts carrier-agnostic design |

---\

### Sources

- Martin Fowler, "Circuit Breaker": https://martinfowler.com/bliki/CircuitBreaker.html
- Wikipedia, "Circuit breaker design pattern": https://en.wikipedia.org/wiki/Circuit_breaker_design_pattern
- Microsoft Azure Architecture Center, "Circuit Breaker Pattern": https://learn.microsoft.com/en-us/azure/architecture/patterns/circuit-breaker
- Microsoft Azure Architecture Center, "Bulkhead Pattern": https://learn.microsoft.com/en-us/azure/architecture/patterns/bulkhead
- Microsoft Azure Architecture Center, "Retry Storm Antipattern": https://learn.microsoft.com/en-us/azure/architecture/antipatterns/retry-storm
- AWS Architecture Blog, "Exponential Backoff and Jitter": https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter
- AWS Builder Center, "The Retry Storm": https://community.aws/content/3MERt2MYpFCgDGFTZLgHr3F33sp
- AWS Builder Center, "Using load shedding to avoid overload": https://builder.aws.com/content/3Eun1EEyX6p2e3VYNyRLSJzLuMV
- Google SRE Book, "Addressing Cascading Failures" (load shedding, graceful degradation, CoDel, LIFO): https://sre.google/sre-book/addressing-cascading-failures
- Google SRE Book, "Handling Overload": https://sre.google/sre-book/handling-overload/
- Resilience4j CircuitBreaker docs: https://resilience4j.readme.io/docs/circuitbreaker
- SmallRye Fault Tolerance circuit breaker reference: https://github.com/smallrye/smallrye-fault-tolerance/blob/6.4.3/doc/modules/ROOT/pages/reference/circuit-breaker.adoc
- Polly CircuitBreaker (raw GitHub wiki): https://raw.githubusercontent.com/wiki/App-vNext/Polly/Circuit-Breaker.md
- Netflix Hystrix "How it Works": https://github.com/Netflix/Hystrix/wiki/How-it-Works/dbf5913e8c4fe82c654f638e4e6ed75f7b7743eb
- Akka CircuitBreaker docs: https://github.com/akka/akka-core/blob/master/akka-docs/src/main/paradox/common/circuitbreaker.md
- tower-resilience circuitbreaker source (rolling window): https://docs.rs/tower-resilience-circuitbreaker/0.2.1/src/tower_resilience_circuitbreaker/circuit.rs.html
- resilient-rs resilience toolkit (retry/timeout/circuit breaker/rate limiter/bulkhead/fallback): https://github.com/resilient-rs/resilient
- rs-retry-kit (async retry with circuit breaker): https://github.com/philiprehberger/rs-retry-kit
- systemoverflow "Circuit Breakers, Bulkheads, and Failure Isolation": https://systemoverflow.com/learn/design-fundamentals/communication-patterns/circuit-breakers-bulkheads-and-failure-isolation-under-partial-degradation
- systemoverflow "Retry Policies: Exponential Backoff, Jitter, and Budgets": https://systemoverflow.com/learn/distributed-primitives/idempotency/retry-policies-exponential-backoff-jitter-and-budgets
- systemoverflow "Thundering Herd Problem": https://loadtest.app/thundering-herd-problem
- dev.to "The Thundering Herd Problem (and How to Actually Fix It)": https://dev.to/thdr/the-thundering-herd-problem-and-how-to-actually-fix-it-14aa
- Upstash "Thundering Herd Problem: Cache Stampede": https://upstash.com/kb/thundering-herd-problem-cache-stampede
- Redis.io "How to Tame the Thundering Herd Problem": https://redis.io/blog/how-to-tame-the-thundering-herd-problem
- arika.dev "Designing for Graceful Degradation": https://arika.dev/blog/architecture/graceful-degradation
- HiLO: "Towards Product-Level Graceful Degradation": https://arxiv.org/abs/2610.34844
- Uber Cinnamon load shedding: https://uber.com/us/en/blog/from-static-rate-limiting-to-intelligent-load-management
- NIST SP 800-204 Security Strategies for Microservices (circuit breakers): https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-204.pdf
- Envoy transient failures / circuit breaking: https://envoyproxy.io/docs/envoy/latest/faq/load_balancing/transient_failures
- Elixir Regulator (adaptive concurrency limits): https://github.com/elixir-toniq/regulator
- totoro-jam "Pattern: Circuit Breaker": https://totoro-jam.github.io/battle-tested-patterns/patterns/circuit-breaker
- Computing Series "Circuit Breakers": https://computingseries.com/s/book3/ch18_circuit_breakers
- throttle.com "Exponential Backoff and Jitter": https://throttle.com/guides/exponential-backoff-and-jitter
- medium "The Retry Storm You Built Yourself": https://medium.com/@ashokgudivada/the-retry-storm-you-built-yourself-eba971ad6ad3

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 027.
