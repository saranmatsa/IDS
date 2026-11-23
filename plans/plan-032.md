## Phase 032 — Create the ids-resource crate: Resource tracking and workload protection.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 032 creates **ids-resource**: the IDS crate for resource tracking and workload protection. It provides storage/CPU/memory/network monitoring, latency/queue/utilization/health measurements, per-client/per-dataset/per-operator/per-region quotas, workload isolation, backpressure, resource reservations, committed capacity optimization, and resource exhaustion prevention.

**Scope (from prompts.txt entry 32):**
- Crate: `ids-resource`
- Purpose: Resource tracking and workload protection.
- Contents: Storage/CPU/memory/network monitoring, latency/queue/utilization/health measurements, per-client/per-dataset/per-operator/per-region quotas, workload isolation, backpressure, resource reservations, committed capacity optimization, resource exhaustion prevention.
- Guideline: try to use separate crates instead of more folders in existing ones → `ids-resource` is its own crate, not a folder inside another crate.

**Architectural constraints (non-negotiable, from prompts.txt):**
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts.
- Research only — no code, no source modifications, only the plan document.

**Key engineering decisions (preview, justified in §11):**
- `ids-resource` depends only on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`; it is not a network crate.
- Quota enforcement: token bucket (rate), leaky bucket (steady shaping), escrow bounded counter (distributed capacity quotas, Σ grants ≤ limit, Δ = 0 strict → no overshoot without per-write round trip).
- Workload isolation: bulkhead concurrency pools per workload, weighted fair queuing / max-min fairness, cell-based fault isolation.
- Backpressure: bounded queues, reject-newest with Retry-After derived from queue depth / drain rate, circuit breaker for saturated dependencies.
- Reservations: Borg-style allocs + admission control; idempotent reserve/commit/release ledger for exactly-once resource accounting.
- Exhaustion prevention: PSI-based early detection, graceful feature degradation, load shedding triggered by p99 latency vs SLO.

---

### 2. Architecture Overview

`ids-resource` sits below the orchestrator/scheduler and above `ids-carrier::Endpoint`. It is **not** a network protocol — it is a local resource accounting and admission engine that emits bytes to ship over whatever carrier IDS uses.

```
┌─────────────────────────────────────────────────────────────┐
│              ORCHESTRATOR / ids-scheduler                   │
│  admits, defers, or rejects work based on ids-resource      │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              Phase 032: ids-resource (NEW CRATE)            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ monitor  │ │ measure  │ │   quota  │ │ isolate  │       │
│  │ storage  │ │ latency  │ │ token    │ │ bulkhead │       │
│  │ cpu      │ │ queue    │ │ bucket   │ │ wfq      │       │
│  │ memory   │ │ util     │ │ leaky    │ │ cell     │       │
│  │ network  │ │ health   │ │ escrow   │ │ slice    │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ backpress│ │ reserve  │ │ exhaust  │ │ carrier  │       │
│  │ sheduler │ │ alloc    │ │ oom guard│ │ endpoint │       │
│  │ breaker  │ │ capacity │ │ degrade  │ │ adapter  │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│             ids-000-aa (master crate, sealed traits)        │
│  ResourceId, QuotaId, WorkloadId, ClientId, DatasetId,      │
│  OperatorId, RegionId + sealed trait contracts              │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              ids-carrier::Endpoint                          │
│  CarrierType + Vec<u8>  (the ONLY traffic channel)          │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              ids-crypto / ids-storage                       │
│  ed25519, xchacha20poly1305, chacha20poly1305, sha256,      │
│  blake3 | quota ledger, reservation journal, snapshots      │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a work item:**
1. Scheduler asks `ids-resource` for an admission decision before admitting work for client `c`, dataset `d`, operator `o`, region `r`, workload `w`.
2. `monitor` samples current storage/CPU/memory/network usage (network = bytes handed to `ids-carrier::Endpoint`, no sockets).
3. `measure` updates latency/queue/utilization/health gauges and histograms (sharded atomic counters on the hot path).
4. `quota` evaluates per-axis quotas (token bucket for rate, escrow for capacity); hierarchical charges are all-or-none across bucket/tenant/root.
5. `isolate` checks the workload's bulkhead concurrency cap and WFQ share.
6. `backpressure` applies block/drop/signal; if the queue is full, reject with Retry-After derived from queue depth / drain rate.
7. `reserve` checks committed capacity (allocation = reserved resource set); admits iff Σ reserved + demand ≤ capacity on every resource dimension.
8. `exhaust` guards against resource exhaustion (hard caps, PSI-based early warning, graceful degradation).
9. Decision (admit / defer / reject + reason) is returned to the scheduler; control-plane traffic between nodes travels only via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

---

### 3. Detailed Research Dimensions

#### 3.1 Resource Monitoring — Storage / CPU / Memory / Network

**USE method** (Brendan Gregg): for every resource, monitor **U**tilization (% time busy), **S**aturation (queue depth / work waiting), and **E**rrors (error event count). CPU: `1 − avg(rate(node_cpu_seconds_total{mode="idle"}))`; Memory: used/total with available-bytes headroom; Disk/Storage: iops and bytes; Network: bytes in/out as observed at the `ids-carrier::Endpoint` boundary (byte counts only — no sockets, no protocol parsing).

**Pressure Stall Information (PSI)** (Linux kernel, cgroup v2): `some` pressure = some tasks stalled on a resource (early contention indicator); `full` pressure = all non-idle tasks stalled (severe shortage). Metrics `avg10`/`avg60`/`avg300` moving averages plus cumulative `total` in microseconds, for CPU, memory, and IO. `systemd-oomd` uses PSI to kill a cgroup before the kernel OOM killer fires — the model for ids-resource's early exhaustion guard. Cross-platform caveat: PSI requires Linux cgroup v2; on other platforms ids-resource falls back to poll-based utilization sampling (TBD, see §10).

**Sharded metrics:** hot-path counters/gauges/histograms use thread-sharded atomics (cache-line padded, aggregate on read) to avoid cross-core cache-line bouncing (fast-telemetry pattern). OpenTelemetry semantic conventions for instrument names and attributes (counter/gauge/histogram with labeled dimensions: client, dataset, operator, region, workload).

#### 3.2 Latency / Queue / Utilization / Health Measurements

- **Latency histograms:** fixed exponential buckets (e.g. 100µs … 100ms), streaming quantiles (summary with ~2% relative error), percentiles p50/p95/p99 reported per resource and per workload.
- **Queue depth:** number of pending work items per workload/queue; saturation signal = queue depth × service time vs drain rate; worst-case wait = queue depth / drain rate.
- **Utilization:** per-resource busy fraction over moving windows (10s/60s/300s), matching PSI averaging windows.
- **Health:** composite signal from utilization + saturation + error rate; drives load shedding as utilization approaches the hard limit (early rejection, not abrupt cutoff).

#### 3.3 Quotas — Per-Client / Per-Dataset / Per-Operator / Per-Region

Four quota axes, each independently configurable with rate + capacity + concurrency dimensions (Golem-style quota model):

- **Rate** — refills `value` tokens every `period`; capped at `max`. For API call rates, requests/sec per client.
- **Capacity** — fixed pool of tokens; consumed, never refilled. For storage budgets, byte quotas per dataset/region.
- **Concurrency** — pool of tokens returned on release. For parallel operation limits per operator.

**Algorithms:**
- **Token bucket (lock-free):** atomic CAS token accounting; tokens accumulate from elapsed monotonic time (scaled integer arithmetic, sub-token precision); burst = bucket capacity; admit iff tokens ≥ cost. 10M+ ops/sec achievable with lock-free sharding (tokio-rate-limit / leaky-bucket research).
- **Leaky bucket:** requests add to a finite-level bucket; leaks at a constant drain rate; overflow → reject. Strict output pacing; smooths bursts; pairs with bounded queue + backpressure.
- **Escrow bounded counter** (arxiv 1503.09052, `bcounter` design): global budget `Quota` lends local grants to nodes; a node admits locally iff `net_used + amount ≤ granted`; invariant `Σ grants ≤ limit` guarantees `Σ used ≤ limit` with no overshoot and no per-write round trip. `Δ = 0` gives strict enforcement; `Δ > 0` permits bounded overshoot (≤ Δ) for fewer false denials. False denials (node out of grant while others are idle) are accepted and healed by grant rebalancing.
- **Hierarchical quotas (`bcounter::BCounterMap`):** one counter per scope (bucket / tenant / root); a single write charges the whole path all-or-none — every level checked before any is charged; on shortage the scope is named and nothing is charged; server tops up from the scope's quota and retries.
- **Fenced, expiring leases** over a gossip/repair tree (Plumtree-style): grantor takes back a grant only after the holder relinquishes it slightly before the lease TTL, so take-back never double-lends (no overshoot); lease TTL longer than tree-repair time reclaims dead subtrees automatically; outstanding-grant ledger is durable so leader changes never re-lend budget.
- **Minimum viable quota:** if a quota is below `Y_min` for its workload, refuse it rather than enforce badly; options: raise `Y`, loosen the target (show tightest band reachable at `f_max`), or accept a soft best-effort limit.

**Enforcement actions:** `reject` (Err with optional estimated wait time), `throttle` (suspend until capacity available), `terminate` (kill with failure message). Acquisition tokens carry idempotency keys; `split`/`merge` guard against child-expected-use exceeding parent.

#### 3.4 Workload Isolation

- **Bulkhead pattern:** separate concurrency pool per workload (per-client/dataset/operator/region slice). A semaphore-like permit cap per pool; failure or latency in one pool never starves another (noisy-neighbor protection). Timeout + cancellation wrapper releases permits on hang/cancel.
- **Cell-based architecture / fault isolation:** partition into independent cells that do not share state; a failure in one cell affects only that cell's requests (AWS well-architected bulkhead/cell guidance).
- **Weighted Fair Queuing (WFQ) / max-min fairness:** each workload `i` with weight `w_i` receives `w_i / Σw` of capacity; guarantees proportional shares and isolation — a workload cannot affect others beyond its share. Virtual finish time ordering selects the next queue to serve.
- **cgroups-style slices:** workload slice = reserved set of CPU/memory/storage with MIN/MAX caps per dimension (SQL Server Resource Governor model: resource pools + workload groups, `MIN_CPU_PERCENT`/`MAX_CPU_PERCENT`, classification of sessions into groups).
- **Gang / co-scheduling:** co-allocated workloads reserve resources together; BoPF (Bounded Priority Fairness) gives latency-sensitive jobs short-term guarantees while preserving long-term fairness for throughput-sensitive jobs (arxiv 1912.03523).

#### 3.5 Backpressure

Bounded buffer + explicit signaling when full (block / drop / signal):

- **Block:** producer waits until buffer has space (blocks the caller — acceptable only when callers can absorb latency).
- **Drop:** reject-newest (tail-drop) with `429` + `Retry-After`; or drop-oldest (head-drop) for real-time feeds where stale data is worthless; or dead-letter routing for work that must not be lost.
- **Signal:** consumer requests exactly `n` items (pull-based flow control).

**Retry-After:** derived live from queue depth / drain rate, plus jitter, to avoid synchronized retry waves (thundering herd). `429 Too Many Requests` = this caller exceeded its allowance; `503 Service Unavailable` = system-wide saturation. `Retry-After` must never be a fixed constant.

**Circuit breaker:** FSM with Closed / Open / Half-Open states; trips after failure/latency threshold within a monitoring window; open state short-circuits calls immediately (fail fast, protects threads/connections); half-open admits a limited trial to test recovery (Azure pattern, Martin Fowler).

#### 3.6 Resource Reservations & Committed Capacity

- **Allocs (Borg model):** an alloc is a reserved set of resources on a machine, assigned whether or not used; used to hold resources between task stop/start and to pack tasks onto machines. Admission control + efficient task-packing + overcommitment + machine sharing with process-level performance isolation (Google Borg paper).
- **Kubernetes ResourceQuotas:** constraints limiting aggregate resource consumption per namespace; object-count limits by API kind; enforced at admission (k8s docs).
- **Committed capacity optimization:** track committed vs reserved vs used capacity; admission rejects when Σ reserved + demand > capacity on any dimension; overcommitment policy per workload class (how much overcommit is safe).

**Reservation ledger:** idempotent reserve → commit → release. Double-entry style: every reservation writes a matched reserve/commit pair; append-only journal; balance invariant (Σ reservations ≤ capacity) checked on commit; idempotency key prevents double-application on retry (open-ledger / idempotent-ledger patterns).

#### 3.7 Resource Exhaustion Prevention

- **Early OOM detection:** PSI memory `some`/`full` pressure triggers guards before the kernel OOM killer; `systemd-oomd` model selects a cgroup to terminate when limits are exceeded, rather than letting the kernel choose.
- **CWE-400 mitigations** (Uncontrolled Resource Consumption): design-in throttling, admission control, per-user rate limits, maximum connection/load limits, absolute timeouts (OWASP DoS cheat sheet).
- **Load shedding:** as utilization approaches the hard limit, reject a growing fraction of requests; trigger on p99 latency vs SLO, not average utilization (average can look healthy while p99 has already breached — Azure throttling pattern).
- **Graceful feature degradation:** turn off nonessential features so essential ones keep resources.
- **Connection/session limits:** max ingress data rate, absolute connection timeout, session inactivity timeout.

---

### 4. Threat Model & Mitigations

||| Threat | Mitigation ||
||--------|-----------|-------------|
| Noisy neighbor monopolizes CPU/memory/storage | Per-tenant quotas (4 axes) + bulkhead concurrency caps + cgroups-style hard MIN/MAX caps per slice |
| Unbounded queue growth → memory exhaustion | Bounded queues bounded by count AND bytes; reject-newest with Retry-After; queue depth as health metric; sustained growth = capacity mismatch |
| Cascading failure via saturated dependency | Circuit breaker (fail fast) + bulkheads isolate failure domains; never block on a dependency without timeout |
| Quota-splitting evasion (spread writes across clients/regions/nodes) | Hierarchical quotas charged all-or-none; `Σ grants ≤ limit` escrow invariant; `Δ = 0` strict path for hard billing/abuse limits; refuse quota below `Y_min` |
| Replay / double-charge of reservations | Idempotency keys on reserve/commit/release; append-only ledger; unique constraint on idempotency key; replay returns original result |
| Clock skew corrupting rate-limit windows | Monotonic steady clock for token-bucket refill; never wall-clock; HLC/Lamport only where ordering matters |
| Resource exhaustion DoS (CWE-400) | Admission control, throttling, hard caps, per-session/connection limits, load shedding on p99 vs SLO |
| Offline member blocks quota reclamation / GC | Fenced expiring leases with TTL > repair time; deliberate eviction, never timeout-only reclamation |
| Compromised workload reads another's metrics | Workload-scoped metric labels; no cross-tenant visibility into counters/gauges |
| Crypto outside allowlist | Only ed25519 (quota attestation signatures), xchacha20poly1305 / chacha20poly1305 (encryption), sha256 / blake3 (hashing, content addressing); dependency + grep audit |

---

### 5. Integration with Existing Crates

Phase 032 depends on and integrates with:

||| Crate | Role in Phase 032 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Canonical types + sealed trait contracts: `ResourceId`, `QuotaId`, `WorkloadId`, `ClientId`, `DatasetId`, `OperatorId`, `RegionId`; sealed traits `QuotaEnforcement`, `ResourceMonitor`, `BackpressureController`, `WorkloadIsolator`, `ReservationManager` |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — the ONLY traffic channel; all quota rebalancing/control signals traverse it |
|| `ids-crypto` | ed25519 signatures, xchacha20poly1305 / chacha20poly1305 encryption, sha256 / blake3 hashing |
|| `ids-storage` | Persistence of quota ledger, reservation journal, snapshots |
|| `ids-telemetry` | Metrics emission: counters, gauges, histograms with labeled dimensions |
|| `ids-scheduler` | Admission control — consults `ids-resource` before admitting work |

**Integration points:**
- `ids-resource` implements the sealed traits from `ids-000-aa`; the scheduler calls `admit(client, dataset, operator, region, workload, demand)` and receives `Admit / Defer(retry_after) / Reject(reason)`.
- All inter-node traffic (quota grant rebalancing, lease renewal, saturation signals) goes over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero use of `std::net`, `tokio::net`, or any networking crate.
- Quota decisions may be signed with ed25519 where attestation across trust boundaries is required.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh — preserving carrier-agnostic architecture (I-CARR-001).

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-resource` (separate crate per the "separate crates instead of more folders" guideline; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`).

```
ids-resource/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (sample intervals, GC thresholds, lease TTLs)
├── error.rs                        # ResourceError (thiserror, comprehensive)
│
├── monitor/                        # Resource sampling (storage/CPU/memory/network)
│   ├── mod.rs
│   ├── sampler.rs                  # Sample trait + poll loop
│   ├── storage.rs                  # Storage bytes / IOPS monitoring
│   ├── cpu.rs                      # CPU utilization + PSI
│   ├── memory.rs                   # Memory pressure + PSI
│   ├── network.rs                  # Network byte counts at ids-carrier::Endpoint (no sockets)
│   ├── health.rs                   # Health composite score
│   └── psi.rs                      # Pressure stall information abstraction
│
├── measure/                        # Latency / queue / utilization / health
│   ├── mod.rs
│   ├── latency.rs                  # Histograms, percentiles (p50/p95/p99)
│   ├── queue.rs                    # Queue depth, drain rate, worst-case wait
│   ├── utilization.rs              # Utilization % per resource over windows
│   ├── health.rs                   # Health signals
│   └── sharded.rs                  # Sharded atomic counters/gauges (hot path)
│
├── quota/                          # Per-client / per-dataset / per-operator / per-region quotas
│   ├── mod.rs
│   ├── engine.rs                   # Quota evaluation engine (4 axes)
│   ├── token_bucket.rs             # Lock-free token bucket (atomic CAS)
│   ├── leaky_bucket.rs             # Leaky bucket shaper + bounded queue
│   ├── escrow.rs                   # Escrow bounded counter (Σ grants ≤ limit)
│   ├── hierarchical.rs             # Hierarchical quota map (all-or-none path charge)
│   ├── axes.rs                     # Client/dataset/operator/region quota axes
│   ├── reservation.rs              # Idempotent reserve/commit/release ledger
│   └── enforcement.rs              # Reject / throttle / terminate actions
│
├── isolate/                        # Workload isolation
│   ├── mod.rs
│   ├── bulkhead.rs                 # Per-workload concurrency pools (permit caps)
│   ├── cell.rs                     # Cell-based fault isolation
│   ├── wfq.rs                      # Weighted fair queuing / max-min fairness
│   ├── slice.rs                    # Workload slice (cgroup-like MIN/MAX caps)
│   └── gang.rs                     # Gang / co-scheduling admission
│
├── backpressure/                   # Backpressure
│   ├── mod.rs
│   ├── controller.rs               # Backpressure controller (block / drop / signal)
│   ├── bounded.rs                  # Bounded buffer + overflow policies
│   ├── shed.rs                     # Load shedding (early rejection vs p99 SLO)
│   ├── retry_after.rs              # Retry-After from queue depth / drain rate + jitter
│   └── circuit.rs                  # Circuit breaker (closed / open / half-open)
│
├── reserve/                        # Resource reservations & committed capacity
│   ├── mod.rs
│   ├── alloc.rs                    # Reserved resource set (Borg alloc-style)
│   ├── capacity.rs                 # Committed capacity tracking + optimization
│   ├── admission.rs                # Admission control against reservations
│   └── overcommit.rs               # Overcommitment policy per workload class
│
├── exhaust/                        # Resource exhaustion prevention
│   ├── mod.rs
│   ├── guard.rs                    # Exhaustion guards (hard caps)
│   ├── oom.rs                      # Early OOM detection (PSI-based)
│   ├── degrade.rs                  # Graceful feature degradation
│   └── limit.rs                    # CWE-400 mitigations (limits, timeouts)
│
├── carrier/                        # Carrier-agnostic transport glue
│   ├── mod.rs
│   └── endpoint.rs                 # ids-carrier::Endpoint (CarrierType + Vec<u8>) adapters
│
├── types/                          # Composed types
│   ├── mod.rs
│   ├── quota_id.rs
│   ├── workload_id.rs
│   ├── resource_id.rs
│   ├── measurement.rs
│   └── decision.rs                 # Admit / Defer / Reject verdict
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_quota.rs
│   ├── fuzz_backpressure.rs
│   ├── fuzz_isolation.rs
│   ├── fuzz_reservation.rs
│   └── fuzz_monitor.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── quota_tests.rs
│   ├── isolation_tests.rs
│   ├── backpressure_tests.rs
│   ├── reservation_tests.rs
│   ├── exhaustion_tests.rs
│   ├── monitor_tests.rs
│   ├── measure_tests.rs
│   ├── adversarial_tests.rs        # Quota splitting, double-charge, clock skew
│   └── chaos_tests.rs              # Offline members, partition healing, churn
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── quota_bench.rs              # Quota eval throughput / latency
│   ├── monitor_bench.rs            # Sampling overhead
│   ├── backpressure_bench.rs       # Shedding / breaker latency
│   └── wfq_bench.rs                # Scheduling fairness
│
├── spec/                           # Formal specifications
│   ├── quota_invariant.md          # Σ grants ≤ limit, no overshoot
│   ├── backpressure_spec.md        # Block/drop/signal contracts
│   └── isolation_spec.md           # Bulkhead/WFQ isolation guarantees
│
├── docs/
│   ├── architecture.md             # ids-resource architecture
│   ├── quota_guide.md              # When to use which quota algorithm
│   └── integration_guide.md        # Integrating with ids-scheduler/ids-storage
│
└── test_vectors/                   # Deterministic test vectors
    ├── quota_vectors.json
    ├── backpressure_vectors.json
    ├── reservation_vectors.json
    └── isolation_vectors.json
```

**Why ~90–1000 files:** the directive calls for a separate crate rather than more folders in existing ones, so `ids-resource` carries a deep module tree: 9 concern domains (monitor, measure, quota, isolate, backpressure, reserve, exhaust, carrier, types) each with core + implementation + tests; plus fuzzing, benchmarks, formal spec, docs, and generated test vectors. Each concern domain expands combinatorially (4 resources × 4 quota axes × enforcement actions × overflow policies × test permutations), so the file count scales into the ~90–1000 range with per-configuration modules and generated test vectors. No stubs — every file has full error handling, documentation, and unit tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Token Bucket (rate quota)
```
state = (tokens: scaled u64, last_refill: Instant)
refill(now): tokens = min(capacity, tokens + (now − last_refill) × rate); last_refill = now
consume(cost): refill(); admitted ⟺ tokens ≥ cost; if admitted: tokens −= cost
```
Lock-free via atomic CAS on the token cell; sub-token precision with scaled integer arithmetic.

#### 7.2 Leaky Bucket (steady shaping + backpressure)
```
level += cost on arrival; if level > capacity → reject (overflow)
drain continuously at drain_rate; worst-case wait = level / drain_rate
```

#### 7.3 Escrow Bounded Counter (distributed capacity quota)
```
acquire(node, amount) admitted ⟺ node.net_used + amount ≤ node.granted
invariant: Σ grants ≤ limit (+ Δ); Δ = 0 ⇒ strict, overshoot-free enforcement
rebalance: grantor transfers grant to grantee via fenced, expiring lease (Plumtree tree)
```

#### 7.4 Hierarchical Charge (all-or-none)
```
charge(path scopes, amount): for each scope s in path: check s.granted ≥ s.used + amount
  if all pass: charge every scope; else: name the short scope, charge nothing, top up and retry
```

#### 7.5 Weighted Fair Queuing (isolation)
```
flow i share = w_i / Σw × capacity
virtual finish time F_i = max(now, last_F_i) + cost_i / w_i   (GPS-style)
serve next: flow with smallest F_i
isolation: flow i cannot exceed w_i/Σw of capacity regardless of others' behavior
```

#### 7.6 Backpressure Controller
```
queue_depth ≥ capacity → overflow policy:
  reject-newest: 429 + Retry-After(queue_depth / drain_rate + jitter)
  drop-oldest: evict head, admit tail (real-time feeds)
  dead-letter: durable replay queue (must-not-lose work)
circuit breaker: failures ≥ threshold in window → Open (short-circuit) → Half-Open (trial) → Closed
```

#### 7.7 Reservation Admission
```
admit(demand): for each resource dimension r:
  Σ reserved[r] + demand[r] ≤ capacity[r]  else reject
reserve → commit → release journal; idempotency key dedups retries
```

#### 7.8 Early Exhaustion Guard
```
PSI memory full > threshold → trigger guard
utilization ≥ hard limit → shed load (reject growing fraction, trigger on p99 vs SLO)
```

---

### 8. Dependencies

||| Crate | Purpose | Notes ||
||-------|---------|-------|-------|
|| `ids-000-aa` | Canonical types + sealed trait contracts | Master crate |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) | Sole traffic channel |
|| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | Allowlist only |
|| `ids-storage` | Quota ledger, reservation journal, snapshots | |
|| `thiserror` | Error types | |
|| `serde` | Serialization | |
|| `blake3`, `sha2` | Hashing | Already in workspace |
|| `tokio` | Async runtime, sync primitives only | NO `tokio::net` |
|| `bytes` | Buffer handling | |
|| `criterion` | Benchmarks | |
|| `proptest` | Property-based testing | |

**Prohibited:** no `std::net`, `tokio::net`, libp2p, or any IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh crate. All traffic via `ids-carrier::Endpoint`.

**Allowed crypto only:** ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3. No AES, RSA, ECDH, SHA-1, MD5, etc.

---

### 9. Measurable Acceptance Criteria

||| Criterion | Target | Measurement ||
||-----------|----------|-------------|
|| Token bucket | lock-free; burst ≤ capacity; sustained rate | unit + property test, 100k iterations |
|| Leaky bucket | steady output rate; overflow → reject | unit test |
|| Escrow quota | Σ grants ≤ limit (Δ = 0); no overshoot | property test |
|| Hierarchical charge | all-or-none across bucket/tenant/root | unit test |
|| Per-client quota | client cannot exceed its budget | adversarial test |
|| Bulkhead isolation | saturated workload A never blocks workload B | concurrency test |
|| WFQ fairness | flow share within ε of weight fraction | bench |
|| Backpressure | bounded queue; overflow → reject + Retry-After | unit test |
|| Circuit breaker | Open after threshold; Half-Open recovery | state-machine test |
|| Reservation admit | admitted iff Σ reserved + demand ≤ capacity (all dims) | unit test |
|| Exhaustion guard | hard cap enforced before OOM | guard test |
|| Metrics | counters/gauges/histograms emitted; p99 tracked | integration test |
|| Carrier-only traffic | zero `std::net`/`tokio::net` usage | build + grep audit |
|| Crypto allowlist | only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency audit |
|| Property tests | 100,000+ iterations | proptest |
|| Reservation idempotency | replay returns original result; no double-charge | idempotency test |

---

### 10. Remaining Uncertainties

1. **Quota rebalancing cadence:** event-driven early top-up (node lease runs low ahead of schedule) vs periodic sweep; lease TTL vs tree-repair time trade-off (escrow design, bcounter).
2. **Hierarchical depth:** how many levels (bucket → tenant → root) before rebalancing overhead dominates grant moves.
3. **Overcommitment policy:** how much committed-capacity overcommit is safe per workload class; measured empirically during implementation.
4. **PSI portability:** pressure stall information requires Linux cgroup v2; cross-platform fallback to poll-based utilization sampling is TBD.
5. **Wire format for quota signals over `ids-carrier::Endpoint`:** CBOR vs bincode vs plain scalar encoding; TBD.
6. **Scheduler integration:** synchronous quota check in the admission path vs async deferral with a durable timer (denied call releases its slot and waits — arxiv 2611.02926 backpressure gate pattern).
7. **Formal verification scope:** TLA+/PlusCal spec for quota invariants and backpressure contracts; sequence of verification effort TBD.
8. **Idempotency key retention window** for the reservation ledger (bounded retention vs replay safety).

---

### 11. Engineering Decisions Summary

||| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate `ids-resource` crate | Prompt guideline: separate crates instead of more folders in existing ones |
|| Carrier-agnostic; `ids-carrier::Endpoint` only | I-CARR-001 architecture; no networking crates |
|| Token bucket + leaky bucket + escrow | Covers rate limiting, steady shaping, and distributed capacity quotas without per-write round trips |
|| Escrow `Σ grants ≤ limit`, Δ = 0 | Overshoot-free enforcement (bcounter, arxiv 1503.09052) |
|| Bulkhead per workload | Failure isolation; noisy-neighbor protection |
|| WFQ / max-min fairness | Proportional capacity sharing with per-workload weights |
|| Idempotent reserve/commit/release ledger | Exactly-once resource accounting under retries |
|| PSI-based early exhaustion detection | `systemd-oomd` pattern; act before kernel OOM |
|| Circuit breaker for saturation | Fail fast on failing/saturated dependencies |
|| Reject-newest + Retry-After from queue depth | Backpressure without thundering herd |
|| Allowed crypto: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | Prompt allowlist |

---

### 12. Sources consulted (30 targeted searches)

- bcounter / escrow bounded counter for distributed capacity quotas (docs.rs bcounter; arxiv 1503.09052; Plumtree tree; fenced expiring leases)
- TiKV `resource_control` — resource groups, ControlledFuture, CPU/IO/compaction pressure scores, background quota adjustment
- Token bucket algorithm — lock-free atomic CAS, 10M+ ops/sec (docs.rs tokio-rate-limit; lib.rs ratelimit; gardal)
- Leaky bucket mechanics, overflow, backpressure policies (api-rate-limiting.com; docs.rs tokio-rate-limit)
- Cloudflare rate limiting at scale — leaky bucket trade-offs (blog.cloudflare.com)
- Bulkhead pattern in Rust — tokio::sync::Semaphore per pool, timeout + cancel (dev.to; Azure Architecture Center)
- cgroups v2 — cpu/memory/blkio controllers (Ubuntu docs; systemd.resource-control man page)
- Kubernetes ResourceQuotas per namespace (kubernetes.io docs)
- Pressure Stall Information (PSI) — CPU/memory/IO `some`/`full` pressure, avg10/60/300 (kernel docs; k8s understand-psi)
- systemd-oomd — PSI-based pre-OOM cgroup termination (freedesktop.org man page)
- OpenTelemetry metric semantic conventions — counter/gauge/histogram + attributes (opentelemetry-specification)
- Prometheus USE method / RED method — Utilization, Saturation, Errors (Tom Wilkie; node_exporter alerts; signoz guide)
- tokio::sync::Semaphore — concurrency limiting, fair permit distribution (docs.rs)
- Envoy rate limit service — domain + descriptors + gRPC RLS (envoyproxy.io; envoyproxy/ratelimit; Istio rate limit)
- Circuit breaker — Closed/Open/Half-Open FSM (Azure Architecture Center; Martin Fowler bliki; arxiv 1609.05830)
- Multi-tenant throttling, noisy neighbor — tiered throttling at scale (AWS API Gateway blog; Azure throttling pattern)
- Blast radius containment — cell-based architecture, fault isolation (drivenets CloudNets; stackresearch; AWS well-architected)
- Weighted fair queuing — GPS, max-min fairness, virtual finish time (Wikipedia; Princeton COS518; IEEE EWFQ)
- Gang scheduling / BoPF — bounded priority fairness, co-scheduling (arxiv 2401.09706; arxiv 1912.03523)
- Backpressure — bounded buffer, block/drop/signal strategies (battle-tested-patterns)
- SQL Server Resource Governor — resource pools, workload groups, MIN/MAX CPU percent (learn.microsoft.com)
- Rust metrics — sharded counters, histograms, summaries (fast-telemetry; metrics-lib; SkyWalking rust agent; ro11y)
- Idempotent double-entry ledger — reserve/commit accounting, idempotency keys (open-ledger; idempotent-ledger; tally; ledgerd)
- OWASP DoS cheat sheet / CWE-400 — uncontrolled resource consumption mitigations (owasp.org; cwe.mitre.org)
- Azure throttling pattern — per-principal limits, graceful degradation, load leveling, p99 trigger (learn.microsoft.com)
- Google Borg — admission control, allocs, overcommitment, process-level isolation (research.google.com pubs/43438)
- Dispatch control for priority multi-tenant workflows under shared rate limits (arxiv 2611.02926 backpressure admission gate)
- Sliding-window quotas in Apache Hadoop YARN (arxiv 2606.06145)