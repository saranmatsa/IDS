## Phase 078 — Create the ids-circuit-breaker crate: Failure isolation via circuit breakers. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1165-1179)

```
78. Research only do not read plan.md **[Phase 078 — Create the ids-circuit-breaker crate: Failure isolation via circuit breakers.]**

Crate: ids-circuit-breaker
Purpose: Failure isolation via circuit breakers.
Contents: Closed/open/half-open states, failure threshold, success threshold, timeout, backoff strategy, per-peer circuit breakers, global circuit breakers, handshake circuit breakers.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 078 — Create the ids-circuit-breaker crate: Failure isolation via circuit breakers.]**

Create plan-078.md in C:/idfs/plans/ for the ids-circuit-breaker crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 078 creates `ids-circuit-breaker`, the IDS crate for failure isolation via circuit breakers. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by breaker scope (per-peer, global, handshake). It implements the classic three-state circuit breaker (CLOSED → OPEN → HALF-OPEN → CLOSED) with configurable failure threshold, success threshold, timeout/reset timeout, and backoff strategy. Per-peer circuit breakers isolate one failing peer while breakers for healthy peers stay closed; global circuit breakers trip on aggregate failure rate or when every per-peer breaker is open; handshake circuit breakers protect protocol handshakes from hanging on unresponsive peers. All protected traffic rides opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate never touches `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh, and never sends bytes itself — it returns an admit/reject/probe decision that the caller frames to an `Endpoint`.

**Core design:** a state machine per circuit keyed by `NodeId` (per-peer) plus a global aggregator. In CLOSED, calls pass through and outcomes are recorded; on crossing the failure threshold (consecutive failures or sliding-window failure rate) the breaker trips to OPEN and fails fast for the reset timeout; after the timeout it enters HALF-OPEN and admits a limited number of probe calls; all-probe success closes the circuit, any probe failure re-opens it with backoff applied. Backoff strategies: fixed, exponential, and exponential-with-jitter (full/equal/decorrelated) to avoid thundering herds on recovery. Failure classification is predicate-based so only breaker-worthy outcomes (timeouts, transport failures, 5xx-equivalent errors) count; everything else passes through untyped. Observability emits state (closed/open/half-open), success/failure/rejection counters, and transition events.

**Scope boundary:** `ids-circuit-breaker` decides; it does not execute, send, or retry. Callers (ids-store, ids-replication, ids-fragment, ids-session, etc.) route every outbound datagram through the breaker for the target `Endpoint`, then act on the decision. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` (breaker state-transition audit entries are signed/hash-chained; crypto ops live in `ids-crypto`).

A skeleton `ids-circuit-breaker` crate does NOT yet exist at `C:/idfs/ids-circuit-breaker/`. `ids-000-aa` is the master crate holding canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`) and sealed trait contracts, consumed but never redefined. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Martin Fowler's Circuit Breaker bliki (3 states, fail-fast, monitor alert), Azure Architecture Center Circuit Breaker pattern (open/half-open flows, short-circuit, retry vs breaker distinction), Resilience4j `CircuitBreakerConfig` (sliding window count-based/time-based, failure rate threshold 50% default, success threshold, wait duration, minimum calls), SmallRye Fault Tolerance circuit breaker (rolling window, `delay`/open→half-open, `successThreshold` probe count), arXiv 1609.05830 "Circuit Breakers, Discovery, and API Gateways in Microservices" (client-side vs proxy deployment, fail fast, stability), AWS "Exponential Backoff and Jitter" (full/equal/decorrelated jitter, thundering herd), AWS Step Functions + DynamoDB circuit breaker (Michael Nygard *Release It* popularization, callee recovery detection), docs.rs `breaker` Rust crate (state machine, sliding-window failure-rate tripping, consecutive-failure tripping, half-open probe permits with stampede protection, backoff strategies Fixed/Exponential/ExponentialJitter, typed errors, failure predicate), tower-resilience-circuitbreaker (Rust, async, metrics, health check), pyresilience CircuitBreaker (consecutive failures, recovery timeout, success threshold, doubled-timeout backoff on repeated failure), per-replica vs global breaker layering (per-replica ejects one degraded node; global trips only when all per-peers are open), OpenTelemetry circuit breaker observability metrics (state gauge, transitions, rejection rate, flapping alert), Go `bjaus/breaker` (failure threshold, success threshold, open duration, half-open requests, OnReject hook), *Release It* (Michael Nygard, stability patterns: circuit breaker, timeouts, bulkheads, handshaking), Hystrix circuit breaker (rolling counters, circuit open → short-circuit → fallback).

---

### 2. Architecture Overview

`ids-circuit-breaker` sits above `ids-000-aa` (types, sealed contracts, errors), `ids-carrier` (opaque `Endpoint` datagram transport), `ids-crypto` (allowed primitives), and `ids-time` (monotonic ticks for timeouts). It is a decision layer: given a call targeted at a peer (`NodeId` → `Endpoint`), it returns `Admit | RejectFast | Probe` and records outcomes. It never opens a socket, never dials an address, and never sends bytes — the caller frames the datagram to `ids-carrier::Endpoint` only when the breaker admits.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                    CALLER LAYER (ids-store, ids-replication, …)                   │
│  build datagram → call breaker.admit(peer: NodeId) → send or reject             │
├──────────────────────────────────────────────────────────────────────────────────┤
│               Phase 078: ids-circuit-breaker (THIS CRATE — decision layer)      │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────┐   │
│  │ breaker_registry │ │ per_peer         │ │ global           │ │ handshake│   │
│  │ (keyed by NodeId)│ │ breakers (one    │ │ breaker (aggregate│ │ breaker  │   │
│  │                  │ │ per peer target) │ │ failure rate)    │ │ (handshake│  │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ │ deadline)│   │
│  ┌──────────────────┐ ┌──────────────────┐ └──────────────────┘ └──────────┘   │
│  │ backoff_strategy │ │ failure_classifier│ │                  │           │   │
│  │ (fixed/exp/jitter│ │ (predicate-based)│ │                  │           │   │
│  │  state machine)  │ │                  │ │                  │           │   │
│  └──────────────────┘ └──────────────────┘ │                  │           │   │
│  ┌──────────────────┐ ┌──────────────────┐ │                  │           │   │
│  │ metrics          │ │ audit            │ │                  │           │   │
│  │ (state, counts,  │ │ (signed+chained  │ │                  │           │   │
│  │  transitions)    │ │  state changes)  │                  │           │   │
│  └──────────────────┘ └──────────────────┘ │                  │           │   │
└───────────────────────────────────────────┬──────────────────┴──────────────────┘
                                            │ Admit → caller sends opaque Vec<u8>
                                            │ RejectFast → caller fails fast
                                            ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                        │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])   │
│  receive_datagram → (Endpoint, Vec<u8>)                                          │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS /        │
│  Wi-Fi / Bluetooth / cellular / VPN / mesh                                       │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Per-peer circuit lifecycle:** `CLOSED` (pass through, count outcomes; threshold reached → OPEN) → `OPEN` (reject fast, reset timeout running; timeout elapsed → HALF-OPEN) → `HALF-OPEN` (admit `success_threshold` probe calls; all succeed → CLOSED; any fail → OPEN with backoff-applied reset timeout).

**Global circuit breaker:** aggregates per-peer state; trips to OPEN when the fleet-wide failure rate exceeds its threshold or every per-peer breaker for a scope is open; callers consult the global breaker first (cheap gate), then the per-peer breaker.

**Handshake circuit breaker:** a per-handshake breaker keyed by handshake ID (`HandshakeId` from `ids-000-aa`); bounds the handshake deadline with `IdsTime` monotonic ticks; trips on handshake timeout/failure so one unresponsive peer cannot stall the handshake pipeline.

**Crypto used:** ed25519 to sign breaker state-transition audit entries; sha256/blake3 to hash-chain the audit log and derive breaker IDs; xchacha20poly1305/chacha20poly1305 to encrypt breaker state at rest where confidentiality is required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 Circuit breaker state machine (CLOSED / OPEN / HALF-OPEN)

**Source:** Martin Fowler Circuit Breaker bliki; Azure Architecture Center Circuit Breaker pattern; SmallRye Fault Tolerance circuit breaker reference; pyresilience CircuitBreaker; Go `bjaus/breaker`.

The breaker is a three-state finite state machine. `CLOSED` is the initial state: calls execute, outcomes (success/failure) are recorded, and crossing the failure threshold trips the breaker to `OPEN`. In `OPEN`, every call is rejected immediately without executing the protected operation (fail fast); after the reset timeout elapses, the breaker moves to `HALF-OPEN` to probe recovery. In `HALF-OPEN`, a limited number of probe calls are admitted; if all probes succeed the breaker returns to `CLOSED`, otherwise it returns to `OPEN` (typically with backoff applied to the next reset timeout). This is the reference lifecycle `ids-circuit-breaker` implements for every circuit.

#### 3.2 Failure threshold: consecutive failures vs sliding-window failure rate

**Source:** Resilience4j `CircuitBreakerConfig` (failureRateThreshold, slidingWindowSize, slidingWindowType COUNT_BASED/TIME_BASED, minimumNumberOfCalls); SmallRye circuit breaker (rolling window ratio); async-concurrency tuning guide (consecutive count vs rate threshold simulation); Go `500px/circuitbreaker` (ConsecutiveBreaker vs RateBreaker); `breaker` Rust crate docs (trips on whichever fires first: consecutive failures or sliding-window failure rate).

Two tripping policies are supported, configured per circuit:
- **Consecutive failures:** opens after `failure_threshold` failures in a row; any success resets the streak. Simple, fast, detects a hard-down dependency; fragile under alternating fail/success (degraded dependency never trips).
- **Sliding-window failure rate:** opens when `failures / outcomes ≥ failure_rate_threshold` within a window of `window_size` outcomes (count-based or time-based), once `minimum_calls` outcomes are recorded. Scale-independent: 5% failure at 10 req/s and at 10k req/s are treated identically. Recommended default for high-throughput IDS paths.

`ids-circuit-breaker` supports both policies (whichever fires first, matching the `breaker` Rust crate design), with defaults: `failure_threshold = 5` consecutive, `failure_rate_threshold = 0.50` (50%), `minimum_calls = 10`, `window_size = 100` outcomes.

#### 3.3 Success threshold and half-open probe logic

**Source:** SmallRye (`successThreshold`: number of probe invocations, default 1); Resilience4j (`permittedNumberOfCallsInHalfOpenState`, `automaticTransitionFromOpenToHalfOpenEnabled`); pyresilience (`success_threshold` consecutive successes to close); Go `bjaus/breaker` (`successThreshold`, `halfOpenRequests`); Tencent Cloud circuit breaker policy (success threshold ≤ probing requests).

In `HALF-OPEN`, the breaker admits at most `half_open_max_calls` concurrent probes (stampede protection: the rest are rejected instantly without counting as failures). The circuit closes when `success_threshold` probes succeed; any probe failure re-opens the circuit. `ids-circuit-breaker` defaults: `success_threshold = 2`, `half_open_max_calls = 3`. Probes are the only calls allowed through an OPEN→recovering breaker, preventing recovery traffic from re-overloading a just-recovering peer.

#### 3.4 Timeout / reset timeout and backoff strategy

**Source:** Resilience4j (`waitDurationInOpenState`, default 30s; `resetTimeout`); opossum (`resetTimeout: 30000` ms, timeout default 10s); AWS "Exponential Backoff and Jitter" (full/equal/decorrelated jitter; capped exponential backoff); pyresilience (doubled timeout on repeated failure, 2h TTL so stale breakers self-heal); arrow-kt circuit breaker (`resetTimeout` multiplied by `exponentialBackoffFactor` up to `maxResetTimeout`); `breaker` Rust crate (backoff strategies Fixed, Exponential, ExponentialJitter applied per trip via attempt counter).

- `reset_timeout`: time the breaker stays OPEN before moving to HALF-OPEN. Default 30 s.
- **Backoff strategy** applied per trip: `Fixed` (constant reset timeout), `Exponential` (reset timeout multiplied by factor per consecutive trip, capped at `max_reset_timeout`), `ExponentialJitter` (exponential with full/equal/decorrelated jitter so recovering peers don't retry in lockstep). Repeated trips lengthen the open duration (10s → 15s → 22.5s …) until success, then reset. A per-circuit TTL (e.g. 2 h of inactivity) auto-closes stale breakers so historical failures cannot penalize future traffic.
- A per-call `timeout` (optional, via `ids-time` ticks, never wall-clock) counts timed-out calls as failures.

#### 3.5 Per-peer circuit breakers (isolation / bulkhead per peer)

**Source:** arXiv 1609.05830 (client-side circuit breakers: one breaker per external service per client); A2A circuit breaker architecture (per-peer breakers create bulkheads; one failing peer cannot consume resources needed for healthy peers); database-read-replicas.com (per-replica breaker ejects one degraded node while healthy peers keep serving).

One independent breaker instance per peer target (`NodeId` → `Endpoint`). The breaker for a failing peer opens and sheds that peer's calls; breakers for healthy peers stay closed and traffic continues unaffected. This bounds the blast radius to the actually-failing peer and prevents a single bad dependency from saturating shared resources. Per-peer breakers are the default scope in `ids-circuit-breaker`.

#### 3.6 Global circuit breakers (aggregate / derived all-open signal)

**Source:** Resilient Circuit documentation (distributed, shared state across instances); database-read-replicas.com (global breaker trips only when every per-replica breaker is open → degraded read mode); Polly distributed circuit breaker issue (state shared between instances); `blackblacktigerHQ/resilience-circuitbreaker` (Redis-shared circuits per resource key, tenant-aware keys).

A global breaker sits above the per-peer layer and trips on an aggregate signal (fleet-wide failure rate, or every per-peer breaker in a scope open). It switches the whole affected path into a degraded mode (fallback target, load shedding). Running only a global breaker is a coarse-grained mistake: one slow peer would trip traffic for the entire fleet. `ids-circuit-breaker` implements both layers; callers consult global first (cheap gate), then per-peer.

#### 3.7 Handshake circuit breakers

**Source:** *Release It* stability patterns (handshaking, defensive integration points); arrow-kt (one request as test in half-open); Webhook architecture circuit breaker patterns (TLS handshake failure classified separately as unreachable/non-self-healing).

A handshake circuit breaker protects protocol handshakes: one breaker per handshake (`HandshakeId`), bounded by a handshake deadline (`IdsTime` monotonic). On handshake timeout or repeated failure it trips, so one unresponsive peer cannot stall the handshake pipeline or exhaust handshake slots. After the reset timeout it probes with a single lightweight handshake attempt. Handshake breakers are keyed separately from data-plane per-peer breakers so a failing data path does not interfere with control/handshake paths and vice versa.

#### 3.8 Circuit breaker vs retry vs bulkhead

**Source:** Azure Architecture Center (breaker prevents calls likely to fail; retry expects eventual success; they combine — retry through a breaker); *Release It* (circuit breaker, timeouts, bulkheads, handshaking as complementary stability patterns); async-concurrency guide (circuit breaker + bulkhead).

Circuit breakers stop calls to a likely-failing dependency; retries re-attempt with expectation of eventual success; bulkheads isolate resources (thread pools, connection pools). They compose: retry logic sits *through* the breaker (breaker trips → retries fail fast). `ids-circuit-breaker` defines the breaker boundary; retry/backoff and bulkhead concerns live in companion crates (ids-bulkhead, Phase 079) and are referenced, not implemented, here.

#### 3.9 Observability: state, transitions, rejections

**Source:** OneUptime circuit breaker monitoring (OpenTelemetry metrics: state gauge 0=closed/1=open/2=half-open, transitions, rejection rate, flapping alert); breaker_machines observability (opened/closed/half_opened/rejected/success/failure events); stackpractices circuit breaker monitoring (Prometheus metrics per service/endpoint).

`ids-circuit-breaker` emits per-circuit metrics: current state, total/success/failure/rejected call counts, state-transition count and timestamps, open duration, probe outcomes. Alerts: breaker stuck open beyond N reset timeouts; flapping (rapid OPEN↔HALF-OPEN cycling); sustained high rejection rate. Metrics are structured over `ids-000-aa` types (`NodeId`, circuit ID) and feed `ids-observability`.

#### 3.10 Rust circuit breaker implementations and sealed-trait style

**Source:** docs.rs `breaker` crate (state machine, sliding-window failure-rate tripping, consecutive-failure tripping, half-open probe permits, backoff strategies, typed errors, failure predicate, Tower layer); tower-resilience-circuitbreaker (async Rust, `state_sync()`, `is_open()`, `metrics()`); docs.rs/axum-conf circuit breaker (per-target organization: one breaker per database, one per external HTTP service).

`ids-circuit-breaker` follows the sealed-trait contract style of `ids-000-aa`: `trait Breaker: private::Sealed` with `admit(&self, key: CircuitKey) -> Decision`, `record(&self, key, outcome)`, `state(&self) -> CircuitState`. Downstream crates use as bounds but cannot implement. Sync state inspection (`state_sync`, `is_open`) for health checks; async metrics via `ids-time`-keyed snapshots.

#### 3.11 Failure classification (what counts as a breaker failure)

**Source:** Resilience4j (`recordExceptions`/`ignoreExceptions`); opencode/gomodel resilience config (`CIRCUIT_BREAKER_FAILURE_ON_STATUSES`: 429, 5xx; client cancellation ignored; local timeouts count); async-concurrency guide (timeouts and 5xx count as failures; declines don't).

A `FailurePredicate` classifies outcomes: transport timeout, transport error, and protocol-level error responses count as failures; business-logic rejections (e.g. invalid payload) and caller cancellations do not. Timeouts are measured against `ids-time` ticks, never wall-clock. The predicate is configurable per circuit so idempotent control-plane calls and latency-sensitive data-plane calls can classify differently.

#### 3.12 Distributed circuit breaker coordination considerations

**Source:** arXiv 1609.05830 (client-side vs proxy deployment); Resilient Circuit (shared PostgreSQL state, coordinated failure handling, fail-open on storage read failure); Polly distributed breaker (shared state between instances so other nodes break earlier).

`ids-circuit-breaker` is per-node local state by design (no shared storage, no carrier dependency). Breaker state is not synchronized across nodes — each node trips independently on its own observed failures. This matches IDS's carrier-agnostic, no-central-coordinator philosophy. Coordination (if ever needed) is an integration-layer concern outside this crate; this plan explicitly does not implement distributed breaker state.

#### 3.13 Carrier-agnostic enforcement (Endpoint only)

**Source:** IDS prompts.txt entry 78; existing `ids-000-aa` skeleton (`Endpoint { CarrierType, address: Vec<u8> }`, `MAX_ADDRESS_SIZE = 1024`); HX-241 carrier-agnostic socket abstraction research.

`ids-circuit-breaker` imports no networking crate. `CircuitKey` = `NodeId` (peer identity from `ids-000-aa`), not an address; the breaker's decision is applied before the caller sends to `ids-carrier::Endpoint`. CI test asserts zero references to `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` in the crate.

#### 3.14 Crypto surface for breaker decisions (allowed set only)

**Source:** IDS allowed-crypto rule (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305); RFC 8032 Ed25519; RFC 7539 ChaCha20-Poly1305; libsodium XChaCha20-Poly1305; NIST FIPS 180-4 SHA-256; BLAKE3 spec.

- ed25519: sign breaker state-transition audit entries (proof that a trip/close decision was made locally).
- sha256: breaker ID derivation, audit-chain links.
- blake3: fast hash-chaining of audit entries, circuit key hashing.
- xchacha20poly1305: encrypt breaker state at rest (random 192-bit nonces).
- chacha20poly1305: encrypt session/breaker payloads (96-bit nonces, counter-based, bounded use).
No other crypto. `ids-crypto` is the sole implementation surface; `ids-circuit-breaker` references primitive IDs via `ids-000-aa`.

#### 3.15 Integration surface with ids-000-aa types

**Source:** `ids-000-aa` canonical types and sealed trait contracts (types.rs, contracts/, errors.rs); existing IDS plan format precedents (plan-001, plan-048, plan-060).

Consumed types: `NodeId` (circuit key / peer identity), `Endpoint` (carrier target the decision is applied to), `CarrierType`, `IdsTime` (monotonic ticks for timeouts and reset deadlines), `Checksum` (audit chain), `Error` (unified error contract, wrapped as `BreakerError` variants). Sealed trait contracts mirrored: `trait Breaker: private::Sealed` declared here; impl crates (`ids-carrier`-adjacent consumers) implement at integration time.

---

### Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Cascading failure: one failing peer drains caller resources | System stability | Per-peer breaker isolates the failing peer; healthy peers' breakers stay closed |
| Global breaker tripping all traffic due to one slow peer | Availability | Global breaker trips only on aggregate rate or all-per-peers-open; per-peer layer is the first decision |
| Recovery stampede re-overloading a just-recovered peer | Peer recovery | HALF-OPEN admits at most `half_open_max_calls` probes; rest rejected instantly |
| Threshold too sensitive (flapping on noise) or too lax (sends traffic into outage) | Correctness | Rate-based threshold with `minimum_calls`; defaults tuned above normal error rates (50%) |
| Stale breaker penalizing traffic long after recovery | Liveness | Per-circuit TTL auto-closes after inactivity; backoff caps max reset timeout |
| Clock manipulation for timeout/deadline | Correctness | `ids-time` monotonic ticks only; no wall-clock for breakers |
| Replay/forgery of breaker state transitions | Decision integrity | ed25519-signed, sha256/blake3 hash-chained audit log; offline verifiable |
| Breaker keyed by network address (couples to carrier) | Carrier-agnosticism | Keyed by `NodeId`; decision applied to `ids-carrier::Endpoint` by caller; crate never addresses sockets |
| Disallowed crypto algorithm | Policy compliance | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto; allowlist test |
| Breaker rejecting calls it should admit (false positive) | Availability | `minimum_calls` gate prevents tripping on low traffic; failure predicate excludes business rejections |
| Carrier duplicating/delaying datagrams | Replay/ordering | Breaker is idempotent on outcomes; duplicates counted once per admit call |

The carrier is never trusted: breaker decisions are local observations over outcomes the caller reports; no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-circuit-breaker ||
|-------|-------|-----------------------------------|-----------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error` consumed, never redefined; `trait Breaker: private::Sealed` contract declared here |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | Breaker decision is applied before caller sends to an `Endpoint`; crate never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Sign/hash-chain breaker audit entries; encrypt breaker state at rest |
| `ids-time` | Monotonic IDS standard time source | Reset timeouts, handshake deadlines, backoff scheduling — never wall-clock |
| `ids-store` | Primary record store (breaker user) | Wraps writes/reads to a peer `Endpoint` with that peer's breaker |
| `ids-replication` | Replica synchronization (breaker user) | Replica-to-peer datagrams gated by per-peer breakers |
| `ids-fragment` | Record fragments/shards (breaker user) | Fragment send path gated by target-peer breaker |
| `ids-session` (Phase 050) | Client-session binding | Handshake circuit breaker protects session handshakes with `HandshakeId` keys |
| `ids-bulkhead` (Phase 079) | Resource isolation (companion) | Composes with circuit breaker: breaker fails fast, bulkhead caps concurrent calls |
| `ids-observability` | Metric/trace sinks | Consumes breaker state/transition/rejection metrics |

**Integration points:** `ids-circuit-breaker` declares the sealed `Breaker` contract and breaker types; consumers call `admit(key)` then `record(key, outcome)` around each protected operation, and route admitted traffic only to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond exposed contracts.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-circuit-breaker/` (a separate crate, not a folder inside an existing crate):

```
ids-circuit-breaker/
├── Cargo.toml                          # serde, thiserror, ids-000-aa, ids-carrier, ids-crypto, ids-time; workspace member
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # Breaker defaults: failure_threshold, success_threshold, reset_timeout, window_size
├── error.rs                            # BreakerError (thiserror): open, reject, classifier, config errors
├── features.rs                         # Feature flags: audit, metrics, async, nightly
│
├── types.rs                            # CircuitKey(NodeId), CircuitId, Decision(Admit/RejectFast/Probe), Outcome(Success/Failure), CircuitState
├── state_machine.rs                    # Three-state breaker core: Closed/Open/HalfOpen transitions
├── breaker.rs                          # Per-circuit breaker struct: admit/record/state, failure predicate, thresholds
├── breaker_registry.rs                 # Registry keyed by NodeId; create/get/remove per-peer breakers
├── failure_predicate.rs                # Classify outcomes as breaker-worthy (timeout, transport error) or not
├── outcome_recorder.rs                 # Records call outcomes; computes streaks and windowed rates
│
├── per_peer/                           # Per-peer circuit breakers (one per NodeId target)
│   ├── mod.rs
│   ├── per_peer_breaker.rs             # Wraps breaker + NodeId; admit/record scoped to peer
│   ├── peer_map.rs                     # HashMap<NodeId, Breaker> with eviction/TTL
│   ├── isolation.rs                    # Blast-radius analysis: one peer open ⇒ others unaffected
│   └── types.rs                        # PerPeerBreakerResult structures
│
├── global/                             # Global circuit breaker (aggregate signal)
│   ├── mod.rs
│   ├── global_breaker.rs               # Aggregates per-peer state; fleet-wide failure rate; all-open derived signal
│   ├── aggregation.rs                  # Roll up per-peer outcomes into global rate
│   ├── degraded_mode.rs                # Global-open → degraded path / fallback target
│   └── types.rs                        # GlobalBreakerResult structures
│
├── handshake/                          # Handshake circuit breakers
│   ├── mod.rs
│   ├── handshake_breaker.rs            # Per-handshake breaker keyed by HandshakeId
│   ├── deadline.rs                     # IdsTime-bounded handshake deadline
│   ├── probe.rs                        # Single lightweight probe attempt on recovery
│   └── types.rs                        # HandshakeBreakerResult structures
│
├── backoff/                            # Backoff strategies for reset timeout
│   ├── mod.rs
│   ├── strategy.rs                     # BackoffStrategy trait: Fixed / Exponential / ExponentialJitter
│   ├── fixed.rs                        # Constant reset timeout
│   ├── exponential.rs                  # Factor-multiplied, capped reset timeout
│   ├── jitter.rs                       # Full/equal/decorrelated jitter (no rng crate; ids-time-derived)
│   └── types.rs                        # BackoffConfig, AttemptCounter structures
│
├── metrics/                            # Observability
│   ├── mod.rs
│   ├── collector.rs                    # State gauge, success/failure/rejection counters per circuit
│   ├── transitions.rs                  # Transition counters and timestamps
│   ├── alerts.rs                       # Stuck-open, flapping, high-rejection alert rules
│   └── types.rs                        # BreakerMetrics structures
│
├── audit/                              # Signed, hash-chained audit log of state transitions
│   ├── mod.rs
│   ├── audit_entry.rs                  # Transition record: from→to, reason, NodeId, IdsTime
│   ├── audit_chain.rs                  # sha256/blake3 chain of entries
│   ├── signer.rs                       # ed25519-signed checkpoints
│   ├── verifier.rs                     # Offline verification of chain + signatures
│   └── types.rs                        # AuditEntry structures
│
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── breaker_contract.rs             # trait Breaker: private::Sealed (admit/record/state)
│   ├── registry_contract.rs            # trait BreakerRegistry
│   ├── predicate_contract.rs           # trait FailurePredicate
│   ├── backoff_contract.rs             # trait BackoffStrategy
│   ├── global_contract.rs              # trait GlobalBreaker
│   ├── handshake_contract.rs           # trait HandshakeBreaker
│   └── metric_contract.rs              # trait BreakerMetrics
│
├── invariants/                         # Compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS references
│   ├── crypto_allowlist.rs             # Only the five allowed algorithms reachable
│   ├── breaker_state_sound.rs          # Legal transitions only (Closed→Open→HalfOpen→Closed)
│   ├── threshold_bounds.rs             # failure_threshold ≥ 1, success_threshold ≥ 1, reset_timeout bounded
│   ├── monotonic_time_only.rs          # ids-time ticks, no wall-clock
│   ├── probe_limit.rs                  # half_open_max_calls bounds concurrent probes
│   ├── key_is_nodeid.rs                # CircuitKey is NodeId, never a network address
│   ├── ttl_self_heal.rs                # Stale breakers auto-close after inactivity TTL
│   ├── audit_chain_sound.rs            # Hash chain + signature verification invariant
│   └── mod.rs
│
├── tests/                              # Property/verification tests (~30 files)
│   ├── mod.rs
│   ├── state_machine_tests.rs
│   ├── breaker_tests.rs
│   ├── per_peer_tests.rs
│   ├── global_tests.rs
│   ├── handshake_tests.rs
│   ├── backoff_tests.rs
│   ├── predicate_tests.rs
│   ├── metrics_tests.rs
│   ├── audit_tests.rs
│   ├── carrier_agnostic_tests.rs       # Greps source for banned networking references — zero matches
│   ├── crypto_allowlist_tests.rs       # Only allowed algorithms reachable
│   ├── isolation_tests.rs              # One peer open does not affect others
│   ├── stampede_tests.rs               # Probe limit enforced under concurrency
│   ├── ttl_tests.rs                    # Stale breaker auto-closes
│   └── property_tests.rs               # proptest properties for state machine
│
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── admit_latency.rs                # admit() latency under load
│   ├── state_transition.rs             # Transition throughput
│   ├── per_peer_scaling.rs             # Breaker registry scaling with peer count
│   └── ...
│
├── docs/                               # (~14 files)
│   ├── architecture.md
│   ├── runbook.md                      # Tuning thresholds, timeouts, backoff
│   ├── adding_a_breaker.md             # Adding a new breaker scope
│   ├── crypto_policy.md                # Allowed-crypto allowlist policy
│   ├── observability.md                # Metrics and alerts
│   └── ...
│
├── spec/                               # (~6 files)
│   ├── state_machine_spec.md
│   ├── threshold_spec.md
│   ├── backoff_spec.md
│   ├── carrier_agnostic_spec.md
│   ├── audit_spec.md
│   └── ...
│
└── test_vectors/                       # (~6 files)
    ├── state_transitions.json          # Expected transitions for outcome sequences
    ├── threshold_vectors.json          # Trip points for consecutive/rate policies
    └── backoff_vectors.json            # Expected reset timeouts per strategy
```

Total: ~90–1000 files across `src/`, `src/per_peer/`, `src/global/`, `src/handshake/`, `src/backoff/`, `src/metrics/`, `src/audit/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, `spec/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. All files are `.rs` source + `Cargo.toml` + tests; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **Three-state breaker** — `CircuitState { Closed, Open, HalfOpen }`; `admit(key)` returns `Decision { Admit | RejectFast | Probe }`; closed passes through and records outcomes; open rejects fast until reset timeout; half-open admits probes only.

2. **Failure counting (two policies)** — consecutive-failure streak (opens at `failure_threshold` in a row; success resets) and sliding-window failure rate (`failures / min(window_size, outcomes) ≥ failure_rate_threshold` once `minimum_calls` recorded); breaker trips on whichever fires first.

3. **Half-open probe with stampede protection** — at most `half_open_max_calls` concurrent probes; excess calls rejected instantly without counting as failures; `success_threshold` consecutive probe successes → CLOSED; any probe failure → OPEN with backoff-applied reset timeout.

4. **Backoff strategies** — `Fixed` (constant `reset_timeout`), `Exponential` (`reset_timeout × factor` per consecutive trip, capped at `max_reset_timeout`), `ExponentialJitter` (full/equal/decorrelated jitter so recovering peers don't retry in lockstep); per-trip attempt counter; TTL auto-closes stale breakers.

5. **Per-peer breaker map** — `HashMap<NodeId, Breaker>`; one breaker per peer target; per-peer outcomes never leak into other peers' breakers (blast-radius isolation).

6. **Global breaker aggregation** — roll up per-peer outcomes into fleet-wide failure rate; global trips on rate threshold or derived all-open signal; callers consult global gate first, then per-peer.

7. **Handshake breaker with deadline** — per-`HandshakeId` breaker bounded by `IdsTime` monotonic deadline; handshake timeout/failure trips; single probe on recovery; separate key space from data-plane breakers.

8. **Failure predicate** — `classify(outcome) -> BreakerWorthy | NotCounted`: timeouts and transport errors count; business rejections and caller cancellations do not; timeouts keyed to `ids-time`.

9. **Breaker metrics** — per-circuit state gauge, success/failure/rejected counters, transition timestamps, open duration; alert rules for stuck-open and flapping.

10. **Audit chain** — each state transition = `{from, to, reason, NodeId, IdsTime}` entry, blake3/sha256 hash-chained, periodic ed25519-signed checkpoints; offline verifiable.

---

### 8. Dependencies

|| Crate | Purpose ||
|-------|---------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` type only — decision is applied before caller sends to an `Endpoint` |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Monotonic ticks for reset timeouts, handshake deadlines, backoff |
| `serde` | Serialization of breaker envelopes, audit entries, metrics |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS implementations in or under `ids-circuit-breaker`. No crypto beyond the allowed list; all crypto via `ids-crypto`. No async runtime (sync state machine; consumers drive timing through `ids-time`).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|-----------|-----------|-------------|
| AC-78.1 | Three-state machine: CLOSED → OPEN on threshold, OPEN → HALF-OPEN after reset timeout, HALF-OPEN → CLOSED on `success_threshold` probe successes, HALF-OPEN → OPEN on any probe failure | State machine unit tests cover all transitions and illegal-transition rejection |
| AC-78.2 | Failure threshold: consecutive-failure policy trips after `failure_threshold` in a row; rate-based policy trips at `failure_rate_threshold` within `window_size` once `minimum_calls` recorded | Threshold tests with synthetic outcome sequences |
| AC-78.3 | Success threshold: `success_threshold` consecutive probe successes in HALF-OPEN close the circuit | Probe-sequence tests |
| AC-78.4 | Reset timeout: breaker stays OPEN for `reset_timeout` before HALF-OPEN; `ids-time`-keyed, not wall-clock | Timed tests with monotonic tick source |
| AC-78.5 | Backoff strategies: Fixed/Exponential/ExponentialJitter produce configured reset timeouts; jitter spreads retry times | Backoff unit tests with deterministic RNG source |
| AC-78.6 | Per-peer isolation: one peer's breaker OPEN does not affect other peers' decisions | Per-peer tests with independent outcome streams |
| AC-78.7 | Global breaker: trips on aggregate failure rate or all-per-peers-open; callers consult global gate first | Global aggregation tests |
| AC-78.8 | Handshake breaker: trips on handshake timeout/failure; single probe on recovery; keyed by `HandshakeId` separate from data-plane | Handshake breaker tests |
| AC-78.9 | Stampede protection: >`half_open_max_calls` concurrent calls in HALF-OPEN rejected instantly without counting as failures | Concurrency test |
| AC-78.10 | Stale breaker TTL: breaker auto-closes after inactivity TTL | TTL tests |
| AC-78.11 | Failure predicate: timeouts/transport errors count; business rejections/cancellations do not | Predicate classification tests |
| AC-78.12 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh in the crate | Dependency audit test (`cargo tree`) + source grep test passes |
| AC-78.13 | Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 reachable | Allowlist unit test passes |
| AC-78.14 | Audit: every state transition hash-chained + ed25519-signed checkpoints; offline verification passes | Audit chain verification tests |
| AC-78.15 | Sealed trait: `trait Breaker: private::Sealed` usable as bound, unimplementable downstream | Contract sealing test |
| AC-78.16 | Crate compiles cleanly: `cargo check -p ids-circuit-breaker` exits 0 with no warnings | Build check |
| AC-78.17 | Tests pass: `cargo test -p ids-circuit-breaker` passes all unit/property/audit/carrier-agnostic tests | Test run |

---

### 10. Remaining Uncertainties

1. **Concrete thresholds per circuit:** defaults (`failure_threshold=5`, `failure_rate_threshold=0.50`, `minimum_calls=10`, `window_size=100`, `success_threshold=2`, `reset_timeout=30s`) are starting points; production tuning requires observed error-rate baselines from `ids-store`/`ids-replication` integration.
2. **Backoff factor and cap:** exponential factor (1.5–2.0) and `max_reset_timeout` unquantified; trade-off between recovery speed and peer protection.
3. **Half-open probe count vs success threshold:** whether `half_open_max_calls` equals `success_threshold` or allows multiple windows (per Tencent Cloud config where attempts < required successes spans multiple sleep windows).
4. **Distributed breaker state:** whether breaker state should ever be shared across nodes (Redis/PostgreSQL-backed) — explicitly out of scope for Phase 078; per-node local state is the design.
5. **Interaction with `ids-bulkhead` (Phase 079):** breaker fails fast; bulkhead caps concurrency; exact composition order (bulkhead around breaker vs breaker around bulkhead) TBD at integration.
6. **Interaction with `ids-session` (Phase 050):** handshake breaker keyed by `HandshakeId`; session-handshake handshake off TBD at integration.
7. **Metrics export format:** Prometheus/OpenTelemetry vs IDS-native `ids-observability` sink; to be decided at integration.
8. **Per-call timeout source:** whether per-call timeout uses `ids-time` ticks directly or a deadline wrapper passed by the caller.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate `ids-circuit-breaker` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear failure-isolation boundary |
| Three-state machine (Closed/Open/HalfOpen) | Industry-standard pattern (Fowler, Azure, Resilience4j, SmallRye); fail fast + automatic recovery detection |
| Key circuits by `NodeId`, not network address | Carrier-agnostic; breaker is a peer-scoped decision applied before sending to `ids-carrier::Endpoint` |
| Per-peer breakers as default scope | Isolates one failing peer; blast-radius bounded (A2A architecture, per-replica breaker research) |
| Global breaker on aggregate/all-open signal | Complements per-peer layer without the "one slow node trips the fleet" anti-pattern |
| Rate-based + consecutive failure thresholds | Rate-based is scale-independent (50% default); consecutive catches hard-down dependencies fast |
| `success_threshold` probes + `half_open_max_calls` stampede protection | Half-open probes test recovery without re-overloading (SmallRye, bjaus/breaker) |
| Backoff: Fixed/Exponential/ExponentialJitter | Jitter breaks retry synchrony (AWS backoff-and-jitter research); cap prevents runaway open durations |
| `ids-time` monotonic ticks only, no wall-clock | Clock manipulation cannot corrupt timeouts or deadlines |
| Failure predicate for classification | Business rejections/cancellations must not trip the breaker (Resilience4j record/ignore exceptions) |
| Per-node local state, no distributed sync | Matches IDS no-central-coordinator philosophy; carrier-agnostic; no shared storage dependency |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence for trip/close decisions |
| Allowed crypto only via `ids-crypto` primitive IDs | Complies with allowed-crypto rule |
| Sealed `Breaker` trait via private supertrait | Rust API Guidelines C-SEALED; consistent with `ids-000-aa` contracts |

---

### 12. Sources

[^1]: Martin Fowler, "Circuit Breaker" bliki — three states (closed/open/half-open), fail fast, monitor alert. https://martinfowler.com/bliki/CircuitBreaker.html
[^2]: Microsoft Learn, "Circuit Breaker Pattern" (Azure Architecture Center) — open/half-open flows, short-circuit, retry vs breaker distinction. https://learn.microsoft.com/en-us/azure/architecture/patterns/circuit-breaker
[^3]: Resilience4j, `CircuitBreakerConfig` — sliding window count/time-based, failureRateThreshold, successThreshold, waitDuration, minimumNumberOfCalls. https://github.com/resilience4j/resilience4j/blob/master/resilience4j-circuitbreaker/src/main/java/io/github/resilience4j/circuitbreaker/CircuitBreakerConfig.java
[^4]: SmallRye Fault Tolerance, circuit breaker reference — rolling window, delay open→half-open, successThreshold probe count. https://github.com/smallrye/smallrye-fault-tolerance/blob/6.6.1/doc/modules/ROOT/pages/reference/circuit-breaker.adoc
[^5]: arXiv 1609.05830, "Circuit Breakers, Discovery, and API Gateways in Microservices" — fail fast, client-side vs proxy deployment, stability. https://arxiv.org/html/1609.05830v2
[^6]: AWS Architecture Blog, "Exponential Backoff and Jitter" — full/equal/decorrelated jitter, thundering herd. https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter
[^7]: AWS, "Using the circuit breaker pattern with AWS Step Functions and Amazon DynamoDB" — Michael Nygard *Release It* popularization, callee recovery detection. https://aws.amazon.com/blogs/compute/using-the-circuit-breaker-pattern-with-aws-step-functions-and-amazon-dynamodb
[^8]: docs.rs, `breaker` crate — Rust state machine, sliding-window failure-rate tripping, consecutive-failure tripping, half-open probe permits, backoff strategies, typed errors, failure predicate. https://docs.rs/crate/breaker/latest
[^9]: tower-resilience, `tower-resilience-circuitbreaker` — async Rust circuit breaker, state inspection, metrics. https://github.com/joshrotenberg/tower-resilience
[^10]: pyresilience, CircuitBreaker — consecutive failures, recovery timeout, success threshold, doubled timeout on repeated failure, TTL self-heal. https://pyresilience.readthedocs.io/en/latest/core/circuitbreaker
[^11]: database-read-replicas.com, "Circuit Breaker Patterns for Degraded Replicas" — per-replica vs global breaker layering, all-open derived signal. https://database-read-replicas.com/replication-lag-consistency-management/circuit-breaker-patterns-for-degraded-replicas
[^12]: OneUptime, "How to Monitor Circuit Breaker State Changes with OpenTelemetry Metrics" — state gauge, transitions, rejection rate, flapping alert. https://oneuptime.com/blog/post/2026-02-06-monitor-circuit-breaker-state-changes-opentelemetry-metrics/view
[^13]: opossum (nodeshift/opossum) — Node.js circuit breaker, resetTimeout, halfOpen, fail fast. https://github.com/nodeshift/opossum
[^14]: Go `bjaus/breaker` — failure threshold, success threshold, open duration, half-open requests, OnReject hook. https://pkg.go.dev/github.com/bjaus/breaker
[^15]: async-concurrency.com, "Tuning Circuit Breaker Thresholds from Metrics" — consecutive count vs rate threshold simulation; 50% rate threshold guidance. https://async-concurrency.com/resilience-cancellation-error-handling/circuit-breakers-and-bulkheads/tuning-circuit-breaker-thresholds-from-metrics
[^16]: Go `500px/circuitbreaker` — ConsecutiveBreaker vs RateBreaker, error rate over sliding window. https://pkg.go.dev/github.com/500px/circuitbreaker
[^17]: letsbuildsolutions.com, "Designing a Circuit Breaker: Failure Detection, State Machines, and Cascading Failure Prevention" — failure rate vs consecutive count, retries count as separate failures. https://letsbuildsolutions.com/blog/system-design/designing-a-circuit-breaker-failure-detection-state-machines-and-cascading-failure-prevention-in-distributed-systems
[^18]: gomodel.mintlify.app, "Resilience" — circuit breaker config: failure_threshold, success_threshold, timeout, slow_call_threshold, scope per provider. https://gomodel.mintlify.app/advanced/resilience
[^19]: Tencent Cloud, "Circuit Breaker Policy" — circuit duration, probing requests, success threshold. https://tencentcloud.com/document/product/1310/81860
[^20]: arrow-kt, "Circuit breaker" — resetTimeout multiplied by exponentialBackoffFactor up to maxResetTimeout. https://arrow-kt.io/learn/resilience/circuitbreaker
[^21]: systemdesignmastery.dev, "Circuit Breakers" — rate-based threshold as gold standard for high-traffic systems; failure_threshold/success_threshold defaults. https://systemdesignmastery.dev/week-02-failure-first-design/day-03-circuit-breakers
[^22]: bindian0509 system-design repo — circuit breaker parameter table (failure_threshold 5-10, success_threshold 2-5, reset_timeout 30-60s, monitoring_window). https://github.com/bindian0509/system-design/blob/master/distributed-system-architectural-patterns/03-resilience-patterns/circuit-breaker.md
[^23]: Webhook Architecture, "Circuit Breaker Patterns for Webhooks" — half-open probe requests, TLS handshake failure classified separately. https://webhook-architecture.com/resilient-delivery-retry-strategies/circuit-breaker-patterns/
[^24]: Michael Nygard, *Release It!* — stability patterns: circuit breaker, timeouts, bulkheads, handshaking. (Book reference; cited via AWS/Microsoft summaries above.)
[^25]: IDS project documents at C:/idfs: prompts.txt entry 78 (lines 1165-1179), plan-001.md, plan-048.md, plan-060.md (IDS plan format precedent), existing ids-000-aa skeleton at C:/idfs/ids-000-aa/.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document.
