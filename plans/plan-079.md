## Phase 079 — Create the ids-bulkhead crate: Resource isolation to prevent cascade failure. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1180-1194)

```
79. Research only do not read plan.md **[Phase 079 — Create the ids-bulkhead crate: Resource isolation to prevent cascade failure.]**

Crate: ids-bulkhead
Purpose: Resource isolation to prevent cascade failure.
Contents: Thread pool isolation, connection pool isolation, memory isolation, CPU isolation, failure domain isolation, blast radius limitation.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 079 — Create the ids-bulkhead crate: Resource isolation to prevent cascade failure.]**

Create plan-079.md in C:/idfs/plans/ for the ids-bulkhead crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 079 creates `ids-bulkhead`, the IDS crate for resource isolation to prevent cascade failure. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by isolation dimension: thread-pool isolation, connection-pool isolation, memory isolation, CPU isolation, failure-domain isolation, and blast-radius limitation. `ids-bulkhead` implements the bulkhead pattern — isolating resources into independent compartments so that failure or overload in one compartment cannot drain resources needed by other compartments. All protected operations ride opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate never touches `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh, and never sends bytes itself — it returns an admit/reject/defer decision that the caller frames to an `Endpoint`.

**Core design:** a registry of named bulkheads, each backed by a bounded resource pool with admission control. Each bulkhead enforces a hard concurrency limit (semaphore-style permits) plus an optional bounded wait queue. Admission strategies: immediate rejection (fail-fast), bounded FIFO queue with timeout, priority-based admission with reserved capacity, and load shedding. Bulkheads compose with circuit breakers (ids-circuit-breaker, Phase 078): the breaker fails fast, the bulkhead caps concurrency. Composition order is circuit-breaker → bulkhead → call so doomed calls never consume a bulkhead permit.

**Scope boundary:** `ids-bulkhead` decides admission; it does not execute, send, or retry. Callers (ids-store, ids-replication, ids-fragment, ids-session, ids-circuit-breaker consumers) route every outbound datagram through the relevant bulkhead for the target resource pool, then act on the decision. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` (bulkhead audit entries are signed/hash-chained; crypto ops live in `ids-crypto`).

A skeleton `ids-bulkhead` crate does NOT yet exist at `C:/idfs/ids-bulkhead/`. `ids-000-aa` is the master crate holding canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`, `FailureDomain`) and sealed trait contracts, consumed but never redefined. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Michael Nygard's *Release It!* bulkhead pattern (2007, watertight-compartment ship analogy); Wikipedia Bulkhead pattern (failure compartmentalization, combined with circuit breakers/retries); Microsoft Azure Architecture Center bulkhead pattern (cell-based architecture, per-consumer connection pools, QoS levels); systemoverflow bulkhead pattern (isolating failures through resource partitioning, blast radius); Hystrix bulkhead (per-dependency thread pools vs semaphore isolation); resilience4j Bulkhead (SemaphoreBulkhead maxConcurrentCalls/maxWaitDuration; FixedThreadPoolBulkhead maxThreadPoolSize/coreThreadPoolSize/queueCapacity/keepAliveDuration; default maxConcurrentCalls 25, maxWaitDuration 0); bulkhead vs circuit breaker composition order (breaker first so bulkhead capacity isn't wasted on doomed requests); admission-control strategies (immediate rejection, bounded queue, timeout admission, priority admission, load shedding); bounded-queue backpressure (tail drop vs head drop, Little's Law L=λ×W, coordinated omission); M/M/1 queueing theory and saturation; adaptive concurrency limiting (AIMD, Vegas, Gradient2 — Envoy adaptive concurrency filter, tower-resilience); priority-based rejection with reserved capacity; weighted concurrency + strict FIFO bounded waiting (go-bulkhead); Rust implementations: reliakit-bulkhead (no_std, zero dependencies, try_acquire/release, no RAII guard, saturating arithmetic), tower-resilience-bulkhead (tokio-based BulkheadLayer, max_concurrent_calls, max_wait_duration), tokio::sync::Semaphore async concurrency limiting; rate-limiting vs concurrency-limiting distinction.

---

### 2. Architecture Overview

`ids-bulkhead` sits above `ids-000-aa` (types, sealed contracts, errors), `ids-carrier` (opaque `Endpoint` datagram transport), `ids-crypto` (allowed primitives), and `ids-time` (monotonic ticks for timeouts). It is an admission-control layer: given an operation targeted at a named resource pool, it returns `Admit | Reject | Defer` and records pool occupancy. It never opens a socket, never dials an address, and never sends bytes — the caller frames the datagram to `ids-carrier::Endpoint` only when the bulkhead admits.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                    CALLER LAYER (ids-store, ids-replication, …)                         │
│  build datagram → call bulkhead.admit(pool) → send or reject/defer                     │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                    Phase 079: ids-bulkhead (THIS CRATE — admission layer)               │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐  │
│  │ bulkhead_registry│ │ thread_pool      │ │ connection_pool  │ │ memory           │  │
│  │ (keyed by PoolId)│ │ isolation        │ │ isolation        │ │ isolation        │  │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └──────────────────┘  │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐  │
│  │ cpu              │ │ failure_domain   │ │ blast_radius     │ │ admission        │  │
│  │ isolation        │ │ isolation        │ │ limitation       │ │ control          │  │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ │ (reject/queue/   │  │
│  ┌──────────────────┐ ┌──────────────────┐ └──────────────────┘ │  priority/shed)  │  │
│  │ backoff_strategy │ │ metrics          │                      │                  │  │
│  │ (fixed/exp/jitter│ │ (occupancy,      │                      │                  │  │
│  │  retry attempts) │ │ rejections, waits│                      │                  │  │
│  └──────────────────┘ └──────────────────┘                      │                  │  │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │                  │  │
│  │ audit            │ │ priority/reserve │ │ cpu_limiter      │ │                  │  │
│  │ (signed+chained  │ │ capacity         │ │ (token bucket)   │ │                  │  │
│  │  decisions)      │ │                  │ │ mem_limiter      │ │                  │  │
│  └──────────────────┘ └──────────────────┘ │ (watermark)      │                  │  │
└───────────────────────────────────────────┬──────────────────┴──────────────────┘
                                            │ Admit → caller sends opaque Vec<u28>
                                            │ Reject → caller fails fast
                                            │ Defer  → caller queues/retry later
                                            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                               │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])          │
│  receive_datagram → (Endpoint, Vec<u8>)                                                 │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS /               │
│  Wi-Fi / Bluetooth / cellular / VPN / mesh                                             │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

**Per-bulkhead admission lifecycle:** `OPEN` (admit up to `max_concurrent`; queue up to `max_queued` if configured; reject beyond) → on permit release, admit next queued waiter (FIFO) → saturated pool rejects new arrivals immediately (or after `max_wait`). Each resource dimension (thread pool, connection pool, memory, CPU, failure domain) is a separate named bulkhead so overload in one dimension starves none of the others.

**Failure-domain isolation:** each bulkhead is tagged with a `FailureDomain` (from `ids-000-aa`); correlated-failure domains share no permits, so a domain-wide failure drains only that domain's pool.

**Crypto used:** ed25519 to sign bulkhead admission/audit entries; sha256/blake3 to hash-chain the decision log and derive bulkhead IDs; xchacha20poly1305/chacha20poly1305 to encrypt bulkhead state at rest where confidentiality is required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Bulkhead pattern origin and ship analogy

**Source:** Michael Nygard *Release It!* (2007, stability patterns incl. bulkhead); Wikipedia Bulkhead pattern; Microsoft Azure Architecture Center bulkhead pattern; systemoverflow bulkhead pattern.

The bulkhead pattern is named after the watertight compartments of a ship's hull: a hull breach floods only the breached compartment, not the whole vessel. In software, resources are partitioned into independent compartments so that failure, latency, or resource exhaustion in one compartment cannot drain resources needed by other compartments. Without bulkheads, a single slow dependency (e.g. a slow database query) can exhaust all threads in a shared pool, blocking unrelated requests that never touch that dependency — a classic cascade failure. `ids-bulkhead` implements this compartmentalization per resource dimension.

#### 3.2 Thread pool isolation

**Source:** Hystrix bulkhead (per-dependency thread pools, semaphore isolation); resilience4j FixedThreadPoolBulkhead (maxThreadPoolSize, coreThreadPoolSize, queueCapacity, keepAliveDuration); systemoverflow bulkhead (separate thread pools per dependency, 50 threads each).

Each downstream dependency gets its own thread pool. Requests to resource-pool A execute on Pool A; requests to Pool B execute on Pool B. When Pool A is exhausted, new A requests are rejected/queued within Pool A only; Pool B continues operating. Per-pool queue size trades latency for throughput: longer queues absorb bursts but increase wait time; for latency-sensitive paths use small queues (5–10) or no queue (fail immediately when pool exhausted). `ids-bulkhead` models thread-pool isolation as a bounded-concurrency bulkhead per pool keyed by `PoolId`.

#### 3.3 Connection pool isolation

**Source:** Microsoft Azure Architecture Center bulkhead (per-service connection pools; one failing service affects only its own pool); Hystrix bulkhead (connection-pool per dependency).

A consumer that calls multiple services is assigned a separate connection pool per service. If one service begins to fail, only its connection pool is affected; the consumer continues using other services. `ids-bulkhead` models connection-pool isolation identically to thread-pool isolation — a bounded pool of "connection slots" per target pool, with admission control deciding whether a new connection attempt may proceed.

#### 3.4 Memory isolation

**Source:** resource-governor / resource-bulkhead research (partition memory so one workload cannot consume all memory); geeksforgeeks bulkhead resource-bulkhead (partition system resources incl. memory).

Memory is a shared, unbounded-by-default resource: one workload allocating unboundedly can trigger OOM for all workloads. Memory isolation partitions memory into per-bulkhead budgets (watermark-based: soft limit triggers load shed, hard limit rejects). `ids-bulkhead` exposes a `mem_limiter` per bulkhead with watermarks; allocations beyond the hard watermark are rejected before touching storage/downstream services.

#### 3.5 CPU isolation

**Source:** resource-bulkhead research (partition CPU so one workload cannot saturate CPU); lean-ctx process-guard (fixed concurrent-slot limit).

CPU isolation limits the number of CPU-bound operations running concurrently per bulkhead (a token-bucket or semaphore over CPU slots). A CPU-hungry workload cannot starve latency-sensitive work because each bulkhead caps its own concurrent CPU operations. `ids-bulkhead` exposes a `cpu_limiter` per bulkhead backed by bounded permits; excess CPU work is queued or rejected.

#### 3.6 Failure domain isolation

**Source:** ids-000-aa `FailureDomain` type and `correlated_failure` invariant; correlated-failure modeling; failure-domain boundaries containing failures.

A failure domain is a boundary (power, facility, network segment, operator, geography) inside which failures are correlated. Bulkheads tagged with distinct `FailureDomain`s share no permits and no state, so a domain-wide failure drains only that domain's pool. `ids-bulkhead` integrates `FailureDomain` from `ids-000-aa` so domain isolation is enforced at the type level.

#### 3.7 Blast radius limitation

**Source:** blast-radius research (bulkhead limits blast radius to one compartment); Azure well-architected reliability (segmentation contains faults to one bulkhead).

Blast radius = the set of components affected by a single failure. Bulkheads bound blast radius to the compartment where the failure occurs. `ids-bulkhead` quantifies blast radius per bulkhead (max concurrent ops × max queued ops) and enforces it as a hard cap; no single overload can exceed its compartment's budget.

#### 3.8 Bulkhead variants: semaphore vs thread-pool/bounded-queue

**Source:** resilience4j Bulkhead (SemaphoreBulkhead vs FixedThreadPoolBulkhead); systemoverflow bulkhead implementation (thread pools, connection pools, semaphores); stackoverflow Hystrix bulkhead Q&A (thread isolation vs semaphore isolation).

Two canonical implementations:
- **Semaphore bulkhead:** limits concurrent executions on the calling thread; acquire/release a permit; full ⇒ immediate reject (or wait up to `maxWaitDuration`). Low overhead, no extra threads.
- **Thread-pool/queue bulkhead:** bounded queue + dedicated thread pool; caller submits and returns; pool exhaustion ⇒ reject. Caller thread is freed immediately.

`ids-bulkhead` supports both variants behind a sealed `AdmissionPolicy` trait; the semaphore variant is the default (no runtime threads, carrier-agnostic friendly).

#### 3.9 Admission control strategies

**Source:** onenoughtone bulkhead admission control; systemoverflow bulkhead failure handling/rejection strategies; go-bulkhead (RejectImmediately vs Wait with FIFO).

When a bulkhead is full, the system must decide:
- **Immediate rejection (fail-fast):** reject instantly, sub-millisecond; caller retries/uses fallback. Best for latency-sensitive paths.
- **Bounded queue:** queue up to `max_queued`, then reject. Absorbs brief spikes but adds latency; queue timeout must be shorter than caller timeout.
- **Timeout-based admission:** wait up to `max_wait` for a permit; reject on timeout.
- **Priority-based admission:** higher-priority requests admitted first; low-priority rejected first near capacity; reserved capacity for VIP/health checks.
- **Load shedding:** proactively reject before saturation based on leading indicators (rising latency, error rate).

`ids-bulkhead` supports all five strategies via `AdmissionPolicy`; default = immediate rejection with optional bounded queue.

#### 3.10 Head-of-line blocking and lane isolation

**Source:** gojek engineering per-bank consumer lanes (bulkhead killed head-of-line blocking); per-bank lanes with priority vs bulk traffic.

Without lane separation, bursty "bulk" traffic starves sporadic "priority" traffic sharing the same consumers. Full isolation per lane (per bank, transfer type, priority) contains failure completely and allows independent throughput tuning. `ids-bulkhead` supports per-lane bulkheads so priority vs bulk traffic never contend.

#### 3.11 Backpressure, bounded queues, tail drop vs head drop

**Source:** readpipe network queues (tail drop vs backpressure; head drop better than tail drop in many cases); ironclad backpressure/flow control (bounded buffer forces backpressure; block/reject/drop-oldest); task-queues backpressure strategies; leaky-bucket overflow.

An unbounded queue absorbs everything until OOM; a bounded queue exerts backpressure. Overflow choices: block producer, reject work, or drop. Drop variants: tail drop (newest), head drop (oldest — better for stale work), priority-based (sacrifice low priority first), random. Queue depth should be sized from max acceptable latency: `capacity = max_latency × drain_rate` (Little's Law). Reject early and cheaply beats rejecting after expensive work. `ids-bulkhead` makes queue depth a deliberate, configurable number and offers tail/head/priority drop.

#### 3.12 Queueing theory: M/M/1 saturation and Little's Law

**Source:** MIT urban_or M/M/1 finite capacity; LibreTexts queueing (saturation λ vs μ); arxiv Erlang Redux M/M/m (iron law R=S+W; Little's Law Q=λR).

Little's Law: at fixed throughput, bounding queue length strictly bounds latency. As λ approaches μ (saturation), queue length and wait time grow without bound. Finite-capacity queues reach steady state even when oversaturated — excess users are turned away (blocking probability = probability queue is full). `ids-bulkhead` uses finite-capacity pools precisely so saturation turns into controlled rejection, not unbounded latency.

#### 3.13 Bulkhead vs circuit breaker composition

**Source:** onenoughtone bulkhead vs circuit breaker; systemoverflow bulkhead vs circuit breakers; Azure circuit breaker vs bulkhead; Release It stability patterns.

- **Circuit breaker** detects failures and stops calls (fails fast on known-bad destination).
- **Bulkhead** contains damage while detection happens (limits concurrency even on slow-but-successful calls).
- Composition order matters: circuit-breaker → bulkhead → call. If bulkhead is evaluated first, a circuit-open call still consumes a permit, wasting capacity under sustained failure.
- A circuit breaker takes 10–30s to trip; without bulkheads that window can exhaust all resources. `ids-bulkhead` explicitly composes with ids-circuit-breaker (Phase 078) in breaker-first order.

#### 3.14 Adaptive concurrency limiting

**Source:** Envoy adaptive concurrency filter (minRTT, headroom, AIMD/gradient); tower-resilience adaptive limiter (AIMD/Vegas/Gradient2); resile adaptive limiter (TCP-Vegas style, RTT-based multiplicative decrease/additive increase).

Static bulkheads pick a fixed number; adaptive limiters tune the concurrency limit in real time from observed latency/error. `ids-bulkhead` provides static bulkheads by default and documents an optional `AdaptiveLimiter` extension (AIMD/Vegas) for dynamic environments; the adaptive layer is explicitly future work, not Phase 079 scope.

#### 3.15 Rust bulkhead implementations and no_std style

**Source:** reliakit-bulkhead (no_std, zero deps, no unsafe, try_acquire/release, Copy type, saturating arithmetic); tower-resilience-bulkhead (tokio-based BulkheadLayer); tokio::sync::Semaphore docs (fair FIFO permits, acquire_owned for task boundaries).

- `reliakit-bulkhead`: counting semaphore, no blocking/queueing/clock/allocation, `no_std`, `Copy`, permits paired manually (no RAII). Ideal carrier-agnostic foundation.
- `tower-resilience-bulkhead`: tokio async, `max_concurrent_calls` + `max_wait_duration`, callbacks on permit/reject.
- `tokio::sync::Semaphore`: fair FIFO async semaphore; permits movable across tasks via `acquire_owned`.

`ids-bulkhead` follows the `reliakit-bulkhead` style: `no_std`-compatible core, zero dependencies beyond `ids-000-aa`/`ids-time`/`ids-crypto`, no unsafe, saturating arithmetic, manual permit pairing. Async waiting (if any) is opt-in via a feature gated on `ids-time` deadlines, never `tokio::net`.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
||--------|-----------|-------------------------------|--------------------------|
|| Cascade failure: one overloaded dependency drains shared resources | System stability | Per-resource-pool bulkhead isolates the overloaded pool; healthy pools keep admitting |
|| Bulkhead saturation wastes capacity on doomed calls | Availability | Circuit-breaker → bulkhead → call order (breaker first); doomed calls never acquire a permit |
|| Recovery stampede re-overloading a just-recovered pool | Peer/pool recovery | Bounded wait queue + max_wait; FIFO admission; no permit hoarding |
|| Threshold too sensitive (flapping) or too lax (sends traffic into outage) | Correctness | Configurable max_concurrent/max_queued/max_wait per bulkhead; defaults tuned above normal load |
|| Unbounded queue → OOM and unbounded latency | Stability/Liveness | Hard queue capacity; deliberate bound sized via Little's Law; no unbounded queues |
|| Memory/CPU starvation by one workload | Stability | Separate mem_limiter/cpu_limiter bulkheads with hard watermarks |
|| Failure domain drains unrelated domains | Stability | FailureDomain-tagged bulkheads share no permits/state |
|| Clock manipulation for wait/deadline | Correctness | ids-time monotonic ticks only; no wall-clock |
|| Replay/forgery of admission decisions | Decision integrity | ed25519-signed, sha256/blake3 hash-chained audit log; offline verifiable |
|| Disallowed crypto algorithm | Policy compliance | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto; allowlist test |
|| Bulkhead keyed by network address (couples to carrier) | Carrier-agnosticism | Keyed by PoolId/FailureDomain; decision applied to ids-carrier::Endpoint by caller; crate never addresses sockets |
|| Bulkhead rejecting calls it should admit (false positive) | Availability | max_queued + max_wait absorb bursts; failure classification excludes non-overload rejections |
|| Carrier duplicating/delaying datagrams | Replay/ordering | Bulkhead is idempotent on occupancy; duplicates counted once per admit |

The carrier is never trusted: bulkhead decisions are local observations of pool occupancy; no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-bulkhead ||
||-------|------|-----------------------------|-----------------------------|
|| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`, `FailureDomain` consumed, never redefined; `trait Bulkhead: private::Sealed` contract declared here |
|| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | Bulkhead decision applied before caller sends to an `Endpoint`; crate never sends directly |
|| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Sign/hash-chain admission audit entries; encrypt bulkhead state at rest |
|| `ids-time` | Monotonic IDS standard time source | Wait timeouts, deadline scheduling, backoff — never wall-clock |
|| `ids-circuit-breaker` (Phase 078) | Failure isolation via circuit breakers | Composes: breaker-first → bulkhead caps concurrency; breaker fails fast, bulkhead caps concurrent calls |
|| `ids-store` | Primary record store (bulkhead user) | Wraps writes/reads to a pool with that pool's bulkhead |
|| `ids-replication` | Replica synchronization (bulkhead user) | Replica-to-peer datagrams gated by target-pool bulkhead |
|| `ids-fragment` | Record fragments/shards (bulkhead user) | Fragment send path gated by target-pool bulkhead |
|| `ids-session` (Phase 050) | Client-session binding | Session-handshake path gated by handshake-pool bulkhead |
|| `ids-backpressure` (Phase 080) | Flow control under load | Composes: bulkhead caps concurrency, backpressure signals queue full |
|| `ids-observability` | Metric/trace sinks | Consumes bulkhead occupancy/rejection/wait metrics |

**Integration points:** `ids-bulkhead` declares the sealed `Bulkhead` contract and bulkhead types; consumers call `admit(pool)` then `release(pool)` around each protected operation, and route admitted traffic only to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond exposed contracts.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-bulkhead/` (a separate crate, not a folder inside an existing crate):

```
ids-bulkhead/
├── Cargo.toml                          # serde, thiserror, ids-000-aa, ids-carrier, ids-crypto, ids-time; workspace member
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # Bulkhead defaults: max_concurrent, max_queued, max_wait
├── error.rs                            # BulkheadError (thiserror): full, reject, config errors
├── features.rs                         # Feature flags: audit, metrics, async-wait, no_std, nightly
│
├── types.rs                            # PoolId, FailureDomain tag, Decision(Admit/Reject/Defer), Occupancy
├── bulkhead.rs                         # Per-bulkhead core: admit/release/occupancy, admission policy
├── bulkhead_registry.rs                # Registry keyed by PoolId; create/get/remove bulkheads
├── admission_policy.rs                 # AdmissionPolicy trait: reject/queue/defer/priority/shed
├── occupancy_tracker.rs                # Tracks in-flight + queued count; computes saturation
├── wait_queue.rs                       # Bounded FIFO wait queue with max_wait deadline
│
├── thread_pool/                        # Thread-pool isolation bulkheads
│   ├── mod.rs
│   ├── thread_pool_bulkhead.rs         # Bounded thread-slot pool per PoolId
│   ├── slot_allocator.rs               # Allocates/frees thread slots
│   ├── isolation.rs                    # Blast-radius: one pool saturated ⇒ others unaffected
│   └── types.rs                        # ThreadPoolBulkheadResult structures
│
├── connection_pool/                    # Connection-pool isolation bulkheads
│   ├── mod.rs
│   ├── connection_pool_bulkhead.rs     # Bounded connection-slot pool per PoolId
│   ├── slot_tracker.rs                 # Tracks active connections per pool
│   ├── isolation.rs                    # One failing service ⇒ only its pool affected
│   └── types.rs                        # ConnectionPoolBulkheadResult structures
│
├── memory/                             # Memory isolation bulkheads
│   ├── mod.rs
│   ├── mem_limiter.rs                  # Watermark-based memory budget per bulkhead
│   ├── watermark.rs                    # Soft/hard watermarks; load-shed at soft, reject at hard
│   ├── types.rs                        # MemLimiterResult structures
│   └── budget.rs                       # Memory budget accounting (bytes)
│
├── cpu/                                # CPU isolation bulkheads
│   ├── mod.rs
│   ├── cpu_limiter.rs                  # Token-bucket/semaphore CPU-slot limiter per bulkhead
│   ├── slot_limiter.rs                 # Bounded concurrent CPU operations
│   ├── types.rs                        # CpuLimiterResult structures
│   └── token_bucket.rs                 # CPU token bucket (no std::net; ids-time-derived ticks)
│
├── failure_domain/                     # Failure-domain isolation bulkheads
│   ├── mod.rs
│   ├── domain_bulkhead.rs              # Bulkhead tagged with FailureDomain; no shared permits
│   ├── domain_map.rs                   # FailureDomain → bulkhead mapping; correlated-failure boundary
│   ├── isolation.rs                    # Domain-wide failure drains only that domain's pool
│   └── types.rs                        # DomainBulkheadResult structures
│
├── blast_radius/                       # Blast-radius limitation
│   ├── mod.rs
│   ├── radius_limiter.rs               # Hard cap: max concurrent × max queued per compartment
│   ├── budget.rs                       # Per-compartment budget accounting
│   ├── enforcement.rs                  # Enforce blast-radius cap at admission
│   └── types.rs                        # BlastRadiusResult structures
│
├── backoff/                            # Backoff strategies for retries after rejection
│   ├── mod.rs
│   ├── strategy.rs                     # BackoffStrategy trait: Fixed / Exponential / ExponentialJitter
│   ├── fixed.rs                        # Constant retry delay
│   ├── exponential.rs                  # Factor-multiplied, capped retry delay
│   ├── jitter.rs                       # Full/equal/decorrelated jitter (ids-time-derived, no rng crate)
│   └── types.rs                        # BackoffConfig, AttemptCounter structures
│
├── metrics/                            # Observability
│   ├── mod.rs
│   ├── collector.rs                    # Occupancy, rejection, wait-time counters per bulkhead
│   ├── saturation.rs                   # Saturation gauge and alert thresholds
│   ├── alerts.rs                       # Stuck-full, high-rejection, long-wait alert rules
│   └── types.rs                        # BulkheadMetrics structures
│
├── audit/                              # Signed, hash-chained audit log of admission decisions
│   ├── mod.rs
│   ├── audit_entry.rs                  # Decision record: admit/reject/defer, PoolId, IdsTime
│   ├── audit_chain.rs                  # sha256/blake3 chain of entries
│   ├── signer.rs                       # ed25519-signed checkpoints
│   ├── verifier.rs                     # Offline verification of chain + signatures
│   └── types.rs                        # AuditEntry structures
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── bulkhead_contract.rs            # trait Bulkhead: private::Sealed (admit/release/occupancy)
│   ├── registry_contract.rs            # trait BulkheadRegistry
│   ├── admission_policy_contract.rs    # trait AdmissionPolicy
│   ├── limiter_contract.rs             # trait ResourceLimiter (mem/cpu)
│   ├── wait_queue_contract.rs          # trait WaitQueue
│   ├── backoff_contract.rs             # trait BackoffStrategy
│   ├── domain_contract.rs              # trait FailureDomainBulkhead
│   └── metric_contract.rs              # trait BulkheadMetrics
│
├── invariants/                         # Compile-time/runtime invariants (~14 files)
│   ├── mod.rs
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS references
│   ├── crypto_allowlist.rs             # Only the five allowed algorithms reachable
│   ├── bounded_capacity.rs             # max_concurrent ≥ 1, max_queued bounded, max_wait bounded
│   ├── monotonic_time_only.rs          # ids-time ticks, no wall-clock for waits/deadlines
│   ├── queue_finite.rs                 # Wait queue always bounded (no unbounded queues)
│   ├── domain_isolation.rs             # Distinct FailureDomain bulkheads share no permits/state
│   ├── no_unbounded_memory.rs          # Memory limiter has hard watermark
│   ├── permit_pairing.rs               # Every admit matched by exactly one release
│   ├── blast_radius_cap.rs             # Occupancy never exceeds compartment budget
│   ├── admission_order.rs              # Breaker-first composition invariant (ids-circuit-breaker)
│   ├── audit_chain_sound.rs            # Hash chain + signature verification invariant
│   ├── key_is_pool_id.rs               # PoolId key, never a network address
│   ├── ttl_eviction.rs                 # Idle bulkheads evicted after TTL
│   └── mod.rs
│
├── tests/                              # Property/verification tests (~30 files)
│   ├── mod.rs
│   ├── bulkhead_tests.rs
│   ├── registry_tests.rs
│   ├── admission_policy_tests.rs
│   ├── thread_pool_tests.rs
│   ├── connection_pool_tests.rs
│   ├── memory_tests.rs
│   ├── cpu_tests.rs
│   ├── failure_domain_tests.rs
│   ├── blast_radius_tests.rs
│   ├── backoff_tests.rs
│   ├── wait_queue_tests.rs
│   ├── metrics_tests.rs
│   ├── audit_tests.rs
│   ├── carrier_agnostic_tests.rs       # Greps source for banned networking references — zero matches
│   ├── crypto_allowlist_tests.rs       # Only allowed algorithms reachable
│   ├── isolation_tests.rs              # One pool full does not affect others
│   ├── stampede_tests.rs               # Queue limit enforced under concurrency
│   ├── ttl_tests.rs                    # Idle bulkhead eviction
│   └── property_tests.rs               # proptest properties for occupancy/permit pairing
│
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── admit_latency.rs                # admit() latency under load
│   ├── occupancy_scaling.rs            # Occupancy tracking scaling with pool count
│   ├── rejection_rate.rs               # Rejection throughput under saturation
│   ├── wait_queue.rs                   # Wait queue enqueue/dequeue latency
│   └── ...
│
├── docs/                               # (~14 files)
│   ├── architecture.md
│   ├── runbook.md                      # Tuning max_concurrent/max_queued/max_wait per pool
│   ├── adding_a_bulkhead.md            # Adding a new isolation dimension
│   ├── crypto_policy.md                # Allowed-crypto allowlist policy
│   ├── observability.md                # Metrics and alerts
│   └── ...
│
├── spec/                               # (~6 files)
│   ├── admission_spec.md
│   ├── capacity_spec.md
│   ├── backoff_spec.md
│   ├── carrier_agnostic_spec.md
│   ├── audit_spec.md
│   └── ...
│
└── test_vectors/                       # (~6 files)
    ├── admission_vectors.json          # Expected decisions for occupancy sequences
    ├── capacity_vectors.json           # Saturation points per policy
    └── backoff_vectors.json            # Expected retry delays per strategy
```

Total: ~90–1000 files across `src/`, `src/thread_pool/`, `src/connection_pool/`, `src/memory/`, `src/cpu/`, `src/failure_domain/`, `src/blast_radius/`, `src/backoff/`, `src/metrics/`, `src/audit/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, `spec/`, `test_vectors/`, organized by isolation dimension so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. All files are `.rs` source + `Cargo.toml` + tests; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **Bulkhead admission** — `admit(pool) -> Decision { Admit | Reject | Defer }`; `release(pool)` frees a permit; occupancy = in-flight + queued; admit succeeds iff occupancy < max_concurrent (or queue has space for Defer).
2. **Semaphore bulkhead** — bounded permits; acquire on admit, release on complete; full ⇒ immediate reject (default) or bounded wait; no RAII guard (permit paired manually), `no_std`-friendly.
3. **Thread-pool/queue bulkhead** — bounded queue + permit pool; caller submits and returns; pool exhaustion ⇒ reject; caller thread freed immediately.
4. **Admission policies** — immediate rejection (fail-fast), bounded FIFO queue with `max_wait` deadline, timeout-based admission, priority-based admission with reserved capacity, load shedding; selected per bulkhead via `AdmissionPolicy`.
5. **Per-pool registry** — `HashMap<PoolId, Bulkhead>`; one bulkhead per resource pool; per-pool occupancy never leaks into other pools (blast-radius isolation).
6. **Failure-domain tagging** — each bulkhead tagged with `FailureDomain`; distinct domains share no permits/state; domain-wide failure drains only that domain's pool.
7. **Memory limiter** — watermark-based: soft watermark triggers load shed, hard watermark rejects; budget tracked in bytes; no unbounded allocation.
8. **CPU limiter** — token bucket over `ids-time` ticks; bounded concurrent CPU operations; excess queued/rejected.
9. **Backoff strategies** — `Fixed` (constant delay), `Exponential` (factor × delay per retry, capped), `ExponentialJitter` (full/equal/decorrelated jitter so retries don't synchronize); per-retry attempt counter.
10. **Wait queue** — bounded FIFO with `max_queued` capacity and `max_wait` deadline; head-drop/tail-drop/priority-drop overflow policies; waiters admitted in FIFO order on permit release.
11. **Bulkhead metrics** — per-bulkhead occupancy gauge, success/rejection/wait counters, saturation ratio; alert rules for stuck-full and high rejection.
12. **Audit chain** — each admission decision = `{decision, PoolId, FailureDomain, IdsTime}` entry, sha256/blake3 hash-chained, periodic ed25519-signed checkpoints; offline verifiable.

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`, `FailureDomain`) and sealed trait contracts |
|| `ids-carrier` | `Endpoint` type only — decision applied before caller sends to an `Endpoint` |
|| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
|| `ids-time` | Monotonic ticks for wait timeouts, deadlines, backoff |
|| `serde` | Serialization of bulkhead envelopes, audit entries, metrics |
|| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS implementations in or under `ids-bulkhead`. No crypto beyond the allowed list; all crypto via `ids-crypto`. No async runtime (sync admission state machine; consumers drive timing through `ids-time`; optional async-wait feature gated on `ids-time` deadlines only).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|-------------|
|| AC-79.1 | Bulkhead admission: `admit` returns Admit while occupancy < max_concurrent, Reject at saturation (immediate-rejection policy) | Admission unit tests with synthetic occupancy |
|| AC-79.2 | Queue policy: bounded FIFO queue admits up to `max_queued` waiters, rejects beyond; `max_wait` deadline enforced via `ids-time` | Wait-queue unit tests with monotonic tick source |
|| AC-79.3 | Priority admission: high-priority requests admitted first; reserved capacity preserved for VIP/health checks | Priority admission tests |
|| AC-79.4 | Load shedding: proactive rejection before saturation based on leading indicators | Shed-load tests at configurable threshold |
|| AC-79.5 | Thread-pool isolation: one pool's saturation does not affect other pools' admission | Per-pool tests with independent occupancy streams |
|| AC-79.6 | Connection-pool isolation: one failing service's pool exhausted ⇒ only that pool affected | Connection-pool tests |
|| AC-79.7 | Memory isolation: allocations beyond hard watermark rejected; soft watermark triggers load shed | Memory limiter watermark tests |
|| AC-79.8 | CPU isolation: concurrent CPU ops capped per bulkhead; excess queued/rejected | CPU limiter concurrency tests |
|| AC-79.9 | Failure-domain isolation: distinct `FailureDomain` bulkheads share no permits/state; domain failure drains only that pool | Failure-domain tests |
|| AC-79.10 | Blast-radius cap: occupancy never exceeds per-compartment budget (max concurrent × max queued) | Blast-radius enforcement tests |
|| AC-79.11 | Backoff strategies: Fixed/Exponential/ExponentialJitter produce configured delays; jitter spreads retry times | Backoff unit tests with deterministic tick source |
|| AC-79.12 | Stampede protection: concurrent calls beyond queue limit rejected instantly without counting as failures | Concurrency test |
|| AC-79.13 | Permit pairing: every `admit` matched by exactly one `release`; no leaks under panic/cancellation | Permit-pairing property tests |
|| AC-79.14 | Idle bulkhead TTL: idle bulkheads evicted after inactivity TTL | TTL tests |
|| AC-79.15 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh in the crate | Dependency audit (`cargo tree`) + source grep test passes |
|| AC-79.16 | Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 reachable | Allowlist unit test passes |
|| AC-79.17 | Audit: every admission decision hash-chained + ed25519-signed checkpoints; offline verification passes | Audit chain verification tests |
|| AC-79.18 | Sealed trait: `trait Bulkhead: private::Sealed` usable as bound, unimplementable downstream | Contract sealing test |
|| AC-79.19 | Crate compiles cleanly: `cargo check -p ids-bulkhead` exits 0 with no warnings | Build check |
|| AC-79.20 | Tests pass: `cargo test -p ids-bulkhead` passes all unit/property/audit/carrier-agnostic tests | Test run |

---

### 10. Remaining Uncertainties

1. **Concrete limits per pool:** defaults (`max_concurrent`, `max_queued`, `max_wait`) are starting points; production tuning requires observed load baselines from `ids-store`/`ids-replication` integration.
2. **Async-wait feature:** whether `max_wait` > 0 requires an async runtime or can be implemented with `ids-time`-deadline polling; opt-in feature gating TBD.
3. **Adaptive limiter:** whether to include an optional AIMD/Vegas adaptive concurrency limiter (future work; explicitly out of Phase 079 scope).
4. **Interaction with `ids-circuit-breaker` (Phase 078):** breaker-first composition order confirmed, but exact wrapping (bulkhead around breaker vs breaker around bulkhead per call site) TBD at integration.
5. **Interaction with `ids-backpressure` (Phase 080):** bulkhead caps concurrency, backpressure signals queue full; combined admission+backpressure flow TBD at integration.
6. **Metrics export format:** Prometheus/OpenTelemetry vs IDS-native `ids-observability` sink; to be decided at integration.
7. **Memory/CPU accounting granularity:** whether limits are per-bulkhead or per-pool-plus-domain; cross-product cardinality impact on registry size.
8. **Overflow drop policy default:** tail-drop vs head-drop vs priority-drop as the default when queue is full; head-drop better for stale work, tail-drop simpler.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate `ids-bulkhead` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear resource-isolation boundary |
|| Semaphore bulkhead as default variant | Low overhead, no runtime threads, carrier-agnostic-friendly; matches `reliakit-bulkhead` no_std style |
|| Admission policies: reject/queue/defer/priority/shed | Covers fail-fast, burst absorption, timed wait, priority reservation, proactive shed (onenoughtone/systemoverflow/go-bulkhead research) |
|| Bounded queues only, sized via Little's Law | Unbounded queues → OOM + unbounded latency; finite capacity turns saturation into controlled rejection (M/M/1, Little's Law research) |
|| Key bulkheads by `PoolId`, not network address | Carrier-agnostic; decision applied before sending to `ids-carrier::Endpoint` |
|| Per-pool bulkheads as default scope | Isolates one overloaded pool; blast-radius bounded to compartment |
|| FailureDomain-tagged bulkheads | Domain-wide failures drain only that domain's pool; type-level integration with `ids-000-aa` |
|| Circuit-breaker → bulkhead → call order | Prevents bulkhead capacity wasted on doomed calls; breaker fails fast, bulkhead caps concurrency (onenoughtone/systemoverflow research) |
|| `ids-time` monotonic ticks only, no wall-clock | Clock manipulation cannot corrupt waits or deadlines |
|| Backoff: Fixed/Exponential/ExponentialJitter | Jitter breaks retry synchrony (AWS backoff-and-jitter research); cap prevents runaway delays |
|| Permit pairing manual (no RAII), `no_std`-compatible core | Keeps crate minimal, allocation-free in steady state, usable from sync and async contexts |
|| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence for admit/reject/defer |
|| Allowed crypto only via `ids-crypto` primitive IDs | Complies with allowed-crypto rule |
|| Sealed `Bulkhead` trait via private supertrait | Rust API Guidelines C-SEALED; consistent with `ids-000-aa` contracts |

---

### 12. Sources

[^1]: Michael Nygard, *Release It!: Design and Deploy Production-Ready Software* — bulkhead pattern origin (2007), stability patterns. (Cited via AWS/Microsoft summaries below.)
[^2]: Wikipedia, "Bulkhead pattern" — watertight-compartment ship analogy, failure compartmentalization, combined with circuit breakers/retries. https://en.wikipedia.org/wiki/Bulkhead_pattern
[^3]: Microsoft Learn, "Bulkhead Pattern" (Azure Architecture Center) — cell-based architecture, per-consumer connection pools, QoS levels, isolate cascading failures. https://learn.microsoft.com/en-us/azure/architecture/patterns/bulkhead
[^4]: systemoverflow, "Bulkhead Pattern: Isolating Failures Through Resource Partitioning" — separate resource pools per dependency, blast radius, bulkhead vs circuit breaker. https://systemoverflow.com/learn/resilience-patterns/bulkhead-pattern/bulkhead-pattern-isolating-failures-through-resource-partitioning
[^5]: Netflix Hystrix, "How it Works" — per-dependency thread pools, semaphore isolation, bulkhead pattern. https://github.com/Netflix/Hystrix/wiki/How-it-Works
[^6]: Resilience4j, Bulkhead documentation — SemaphoreBulkhead (maxConcurrentCalls, maxWaitDuration), FixedThreadPoolBulkhead (maxThreadPoolSize, coreThreadPoolSize, queueCapacity, keepAliveDuration); defaults maxConcurrentCalls=25, maxWaitDuration=0. https://resilience4j.readme.io/docs/bulkhead
[^7]: onenoughtone, "Bulkhead Pattern" — admission-control strategies: immediate rejection, bounded queue, timeout admission, priority admission, load shedding; bulkhead vs circuit breaker comparison. https://onenoughtone.com/learn/hld-fault-tolerance-patterns-bulkhead-pattern/2
[^8]: onenoughtone, "Bulkhead Pattern — Page 5" — bulkheads prevent cascade failures; circuit breakers prevent futile calls; healthy→slow→failed→recovering phases; composition order. https://onenoughtone.com/learn/hld-fault-tolerance-patterns-bulkhead-pattern/5
[^9]: systemoverflow, "Bulkhead Failure Handling: Rejection Strategies and Fallbacks" — immediate rejection, bounded queue, fallback, priority-based rejection with reserved capacity. https://systemoverflow.com/learn/resilience-patterns/bulkhead-pattern/bulkhead-failure-handling-rejection-strategies-and-fallbacks
[^10]: systemoverflow, "Bulkhead Implementation: Thread Pools, Connection Pools, and Semaphores" — per-dependency thread pools, queue size latency/throughput tradeoff. https://systemoverflow.com/learn/resilience-patterns/bulkhead-pattern/bulkhead-implementation-thread-pools-connection-pools-semaphores
[^11]: readpipe, "Things we (finally) know about network queues" — tail drop vs backpressure; head drop better than tail drop; AQM. https://readpipe.org/posts/32d0598e-0757-46e4-bcca-3896319af01d
[^12]: ironclad.academy, "Backpressure & Flow Control" — bounded buffer forces backpressure; block/reject/drop; tail/head/priority drop; Little's Law. https://ironclad.academy/system-design/articles/backpressure-and-flow-control
[^13]: task-queues.com, "Backpressure Strategies for Fast Producers" — bounded queue, reject-publish vs drop-head; bounded capacity. https://task-queues.com/queue-fundamentals-architecture/producer-consumer-pattern-design/backpressure-strategies-for-fast-producers
[^14]: MIT OpenCourseWare, 1.041 Queueing Models — M/M/1 saturation, λ vs μ, finite capacity. https://web.mit.edu/1.041/www/lectures/L8-queuing-models-2026sp.pdf
[^15]: arxiv.org, "Erlang Redux: An Ansatz Method for Solving the M/M/m Queue" — iron law R=S+W, Little's Law Q=λR. https://arxiv.org/html/2008.06823v1
[^16]: Envoy Proxy, "Adaptive Concurrency" filter — minRTT, headroom, AIMD/gradient concurrency limiting. https://envoyproxy.io/docs/envoy/latest/configuration/http/http_filters/adaptive_concurrency_filter
[^17]: tower-resilience, `tower-resilience-bulkhead` — BulkheadLayer (max_concurrent_calls, max_wait_duration); adaptive limiter AIMD/Vegas/Gradient2. https://docs.rs/crate/tower-resilience-bulkhead
[^18]: reliakit-bulkhead — Rust no_std counting semaphore, try_acquire/release, Copy, no unsafe, no deps, saturating arithmetic. https://lib.rs/crates/reliakit-bulkhead
[^19]: tokio::sync::Semaphore — fair FIFO async semaphore, acquire_owned for task boundaries. https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html
[^20]: gojek engineering, "Per-Bank Consumer Lanes" — bulkhead killed head-of-line blocking; per-lane isolation (bank, transfer type, priority). https://medium.com/gojekengineering/per-bank-consumer-lanes-how-we-solved-head-of-line-blocking-with-the-bulkhead-pattern-ef4220f9ad4f
[^21]: go-bulkhead (faustbrian) — RejectImmediately vs Wait, bounded FIFO, weighted concurrency, priority reservation. https://pkg.go.dev/github.com/faustbrian/go-bulkhead
[^22]: resile — adaptive concurrency limiter (TCP-Vegas style, RTT-based multiplicative decrease/additive increase). https://github.com/cinar/resile
[^23]: AWS Architecture Blog, "Exponential Backoff and Jitter" — full/equal/decorrelated jitter, thundering herd. https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter
[^24]: Azure Well-Architected Framework, reliability design patterns — bulkhead segmentation contains faults to one bulkhead. https://learn.microsoft.com/en-us/azure/well-architected/reliability/design-patterns
[^25]: IDS project documents at C:/idfs: prompts.txt entry 79 (lines 1180-1194), plan-001.md, plan-048.md, plan-060.md, plan-078.md (IDS plan format precedent), existing ids-000-aa skeleton at C:/idfs/ids-000-aa/.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document.
