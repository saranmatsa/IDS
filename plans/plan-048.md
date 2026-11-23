## Phase 048 — Create the ids-bench crate: Performance benchmarking.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 715-729)

```
48. Research only do not read plan.md **[Phase 048 — Create the ids-bench crate: Performance benchmarking.]**

Crate: ids-bench
Purpose: Performance benchmarking.
Contents: Read/write latency (p50/p95/p99), throughput, cross-domain bytes transferred, recovery completion time, durability restoration time, storage amplification, repair bandwidth, control-plane traffic, metadata convergence time, intent-compilation time, resource utilisation, cost per byte, energy per byte.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 048 — Create the ids-bench crate: Performance benchmarking.]**

Create plan-048.md in C:/idfs/plans/ for the ids-bench crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 048 creates `ids-bench`, the IDS performance-benchmarking crate. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by measurement domain. `ids-bench` measures read/write latency (p50/p95/p99), throughput, cross-domain bytes transferred, recovery completion time, durability restoration time, storage amplification, repair bandwidth, control-plane traffic, metadata convergence time, intent-compilation time, resource utilisation, cost per byte, and energy per byte.

**Core design:** a criterion.rs-driven microbenchmark harness (warmup → measurement → statistical analysis → regression comparison) with `std::hint::black_box` to prevent optimization, HDR histogram / t-digest / DDSketch percentile sketches for tail latency, and optional divan benchmarks as a cross-check. Every measurement driver speaks only opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address); `ids-bench` never touches a carrier directly and imports no networking crate. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` (ids-crypto does not depend on ids-000-aa to avoid the circular dependency; ids-bench depends on both).

**Scope boundary:** `ids-bench` measures; it does not tune. It produces reproducible numbers and regression signals for the implementation phases (ids-write, ids-read, ids-recovery, ids-erasure, ids-sync, ids-crdt, ids-query, …). Thresholds are configurable budgets, not hard-coded limits, so a slow platform degrades gracefully rather than failing the suite.

Skeleton `ids-bench` does NOT exist yet at `C:/idfs/ids-bench/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned criterion.rs (stats-driven microbenchmarks, black_box, outlier detection via Tukey IQR, 95% CI, bootstrap resampling, regression detection), divan (simpler harness, async support, instruction counting), bencher/libtest (nightly-only), HdrHistogram/hdrhistogram (p50/p95/p99, ~3–6 ns record cost, fixed memory, coordinated-omission correction), t-digest (MnO2; streaming, mergeable, tight tails), DDSketch (relative-error quantiles), ed25519-dalek criterion numbers (sign ~15 µs, verify ~40 µs), xchacha20poly1305 (~4.5 µs at 1 KiB, ~217 MiB/s), blake3 (~2–10 GiB/s), sha256 (~2.4 GiB/s), chacha20poly1305 (~1–3 GiB/s), sysinfo (process RSS/CPU), Welford online algorithm (streaming mean/variance), WAL-recovery benchmarks (~100–120 ms to replay 1M × 128 B records), LSM write amplification (10–30×), reed-solomon-simd (encode ~10 GiB/s, decode hundreds of MiB/s), clay codes MSR repair (37–67% repair-bandwidth savings vs Reed-Solomon), hotpath.rs CI regression guidance (wall-clock noise vs instruction-count stability), RAPL energy measurement, throughput-per-megabyte/energy-per-request methodology, rusty-comms IPC benchmark (p50/p95/p99, MiB/s per payload size).

---

### 2. Architecture Overview

`ids-bench` is a measurement layer above the IDS crates under test. It drives workloads through the public contracts declared in `ids-000-aa`, sends/receives only through `ids-carrier::Endpoint`, hashes/signs with `ids-crypto` primitives, and timestamps with `ids-time` (monotonic, never wall-clock for ordering). It does not store, route, or replicate data itself.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      BENCHMARK DRIVER (ids-bench, THIS CRATE)               │
│  cli.rs  ·  suite.rs  ·  workloads/  ·  reporters/  ·  config/              │
├─────────────────────────────────────────────────────────────────────────────┤
│  latency/        throughput/     bytes/         recovery/     durability/   │
│  p50/p95/p99     MiB/s + ops/s   cross-domain   completion    restoration   │
│  HDR/t-digest    warmup          bytes_out==in   WAL replay    erasure       │
│  Welford stats   batch sizes     + acked         harness      reconstruction│
├─────────────────────────────────────────────────────────────────────────────┤
│  amplification/  repair-band/    control-plane/  metadata/     intent/     │
│  storage         bandwidth       traffic ratio   convergence   compilation │
│  phys vs logical bytes repaired  ctrl/data      post-heal     intent→plan   │
│  ratio                                 time                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  resources/      cost-energy/    crypto-bench/   harness/      reports/    │
│  RSS/CPU/alloc   cost per byte   ed25519/xchacha criterion/divan  CSV/JSON │
│  per byte/energy blake3/sha256   chacha20poly1305 black_box      output    │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │ opaque Vec<u8> datagrams
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload)      │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one benchmark iteration:**
1. `suite.rs` selects a workload (read/write/repair/recovery/…), payload size, concurrency, duration; `workloads/` builds deterministic input (fixed seed, fixed byte patterns).
2. The driver submits operations through the crate-under-test's contract (e.g. `ids-store` write, `ids-read` get, `ids-erasure` encode/decode, `ids-recovery` replay). Each operation is timed with `ids-time` ticks.
3. Latency samples feed a concurrent HDR histogram (bounded range, fixed significant digits) plus a t-digest/DDSketch for mergeable cross-node aggregation; p50/p95/p99 extracted after warmup.
4. Byte counters record payload bytes + control-plane bytes per domain; cross-domain `bytes_transferred` checked against the `ids-000-aa` `out_eq_in` invariant (bytes-out == bytes-in + acked) within tolerance.
5. `resources/` samples process RSS and CPU% (sysinfo) and allocation counts; `cost-energy/` derives cost per byte and energy per byte (energy hooks optional, RAPL-style where available).
6. Crypto benches run ed25519 sign/verify, xchacha20poly1305, sha256, blake3, chacha20poly1305 across payload sizes (64 B … 1 MiB).
7. `reporters/` emits CSV/JSON plus criterion comparison against the previous run (regression signal, 95% CI).

All cross-node benchmark traffic is opaque `Vec<u8>` to `ids-carrier::Endpoint`; `ids-bench` never addresses IP/TCP/UDP/QUIC/HTTP/DNS or any carrier by name.

---

### 3. Detailed Research Dimensions

#### 3.1 Benchmark harness selection (criterion vs divan vs bencher)

**criterion.rs** — statistics-driven microbenchmarking (Rust port of Haskell Criterion). Four phases per benchmark: warmup (fill caches), measurement (repeated execution, timing samples), analysis (statistics), comparison (vs previous run, regression detection). Defaults: sample size 100, warm-up 3 s, measurement 5 s, bootstrap 100k resamples, 95% confidence, 5% significance, outlier classification via Tukey's IQR method. Works on stable Rust. De facto standard in the ecosystem; used by ed25519-dalek for its criterion benchmarks (signing ~15 µs, verification ~40 µs on an i7-10700K). `std::hint::black_box` prevents the compiler from constant-folding or deleting benchmarked work. `bench_with_input` / `BenchmarkId` detects O(n)/O(n²) complexity; `iter_batched` separates setup (unmeasured) from the operation under test.

**divan** — simpler harness with `#[divan::bench]`, `divan::black_box`, per-argument inputs, thread-contention (`threads` option), async benchmarks via a Tokio runtime inside the bench, allocation profiling, and hardware-adaptive measurement. Listed by CodSpeed as one of the three mainstream Rust harnesses alongside criterion and libtest-bench. Good as a cross-check harness because it measures differently (instruction-count-friendly, fast startup).

**bencher / libtest bench** — nightly-only (unstable `#[bench]`); neither actively developed. Rejected as the primary harness for an IDS crate targeting stable toolchains; usable only as a secondary, nightly-gated option.

**Verdict:** primary = criterion.rs (statistical rigor, regression detection, stable support); secondary = divan (cross-validation, async workloads, contention); nightly libtest bench not adopted.

#### 3.2 Latency histograms and percentiles (p50/p95/p99)

**HdrHistogram / hdrhistogram** — Gil Tene's HDR histogram port to Rust. Fixed log-linear buckets, constant-time `record` (~3–6 ns, no allocation per sample), fixed memory by dynamic range + significant digits, `value_at_percentile(0.50/0.95/0.99)`. Tail-accurate for bounded integer ranges (nanosecond latencies). Submillisecond cookbook numbers: record p99 ≈ 51 ns, percentile read ≈ 49 µs over ~20k populated counters for a 1 ns–1 ms range on EC2 t3.micro. Weakness: unbounded value ranges and merging across heterogeneous shapes need a sketch instead.

**t-digest (MnO2/t-digest; sketches)** — streaming quantile sketch; `record` O(1) amortised, quantile O(δ), mergeable across threads/nodes, tight at tails (p99/p99.9), compact state scales with compression factor not sample count. Use when value range is unbounded or percentiles must be merged across benchmark shards.

**DDSketch** — relative-error quantile sketch (logarithmic buckets); a 1% target means a true 100 ms p99 is estimated within [99, 101] ms; mergeable; good for cross-replica latency aggregation where relative error beats rank error.

**Coordinated omission** — on a fixed-rate load generator, one slow operation blocks every request that should have been issued during the stall; naive histograms report the tail the sampler saw, not the tail delivered. HDR's `record_with_expected_interval(value, interval)` backfills missing samples so percentiles reflect stalled requests. ids-bench must apply coordinated-omission correction for closed-loop drivers, or use open-loop drivers where it does not apply.

**Welford online algorithm** — single-pass, numerically stable running mean/variance (M2 update); O(1) memory, mergeable partitions (parallel → combine M2). Used for per-iteration statistics without storing every sample; complements histograms (not a replacement for percentiles).

**Verdict:** HDR histogram as the primary per-worker latency recorder (bounded ns–ms range, p50/p95/p99), t-digest/DDSketch as the mergeable cross-worker aggregation layer, Welford for streaming mean/variance, coordinated-omission correction on fixed-rate drivers.

#### 3.3 Throughput measurement

Throughput = bytes or operations per unit time, measured over the warmed-up steady state (not warmup), at multiple payload sizes (64 B, 256 B, 1 KiB, 4 KiB, 64 KiB, 256 KiB, 1 MiB, 4 MiB). Report MiB/s and ops/s per size, plus the size-throughput curve (knee detection). Reference patterns: rusty-comms IPC benchmark reports p50/p95/p99 plus MiB/s per message size; rustls reports bulk transfer MiB/s and handshakes/s; oxicrypto benchmarks AEAD/hash throughput at 1 KiB/64 KiB/1 MiB. ids-bench adopts the same per-size table format.

#### 3.4 Cross-domain bytes transferred

Counter every byte crossing a domain boundary (region → region, node → node, H-unit → H-unit): payload bytes and control bytes, sent and received. The `ids-000-aa` `out_eq_in` invariant (bytes-out == bytes-in + acked) gives a self-check: a benchmark run must account for every byte, or report the discrepancy as a measurement bug, not silence it. Cross-domain bytes transferred is reported per workload and per direction; amplification ratios (control bytes / payload bytes, repair bytes / logical bytes) derive from it.

#### 3.5 Recovery completion time and durability restoration time

**Recovery completion time:** time from failure detection signal to service resumption (replica caught up, WAL replayed, index rebuilt). Reference WAL-recovery benchmarks: ~78–122 ms to scan/replay 1M × 128 B records (~114 ms median), ~25M records/sec sequential WAL replay, group-commit append at hundreds of thousands of rec/s. ids-bench drives a crash/restart scenario (SIGKILL-style: truncate at last synced position) and measures replay time plus catch-up time.

**Durability restoration time:** time from failure to restored redundancy (re-replication or erasure reconstruction complete, quorum/region durability re-established). Measured for node, H-unit, and region failure scenarios with bounded recovery bandwidth (so recovery does not starve the data plane — ids-fault backpressure integration).

Both are absolute deadlines keyed to `ids-time` ticks, never RTT-derived.

#### 3.6 Storage amplification

Storage amplification = physical bytes stored / logical bytes ingested (includes WAL, replicas, erasure fragments, tombstones, index overhead, compaction headroom). Benchmark per workload: ingest N logical bytes, measure physical bytes on disk (via ids-store/ids-reclaim interfaces), compute ratio. Reference: LSM-trees suffer 10–30× write amplification under random workloads (repeated compaction); WAL-first engines trade amplification for durability. ids-bench reports amplification before/after compaction and after garbage collection so compaction efficacy is measurable.

#### 3.7 Repair bandwidth

Bytes transferred across the network to repair one failed node/fragment. Reed-Solomon (n,k) naive repair downloads k full chunks to reconstruct one; regenerating/MSR codes (clay codes) download a fraction per helper — measured 37–67% repair-bandwidth savings vs Reed-Solomon (configs (4,2,5), (9,3,11), (10,4,13)). reed-solomon-simd reference: encode ~10 GiB/s (32:32 shards), decode hundreds of MiB/s–1.4 GiB/s depending on shard count. ids-bench benchmarks single-node and concurrent-failure repair, reports bytes transferred and repair time, with repair bandwidth capped so foreground traffic is not starved (ids-fault/ids-backpressure integration).

#### 3.8 Control-plane traffic

Ratio of control-plane bytes (metadata, heartbeats, discovery gossip, Merkle digests, anti-entropy, replicas of control state) to data-plane bytes, measured per workload and per time window. Important because control traffic scales with node count and metadata volume, not payload size. ids-bench counts control bytes separately at the carrier boundary (opaque Vec<u8> frames tagged by frame type) and reports control/data ratio; a run is flagged if control traffic exceeds a configurable budget.

#### 3.9 Metadata convergence time and intent-compilation time

**Metadata convergence time:** after a partition heals or a node joins, time until all replicas agree on metadata (membership, placement, commit prefix). Driven through ids-membership/ids-sync/ids-crdt paths; measured as time-to-identical-state across replicas under controlled churn. Reference CRDT merge benchmarks: did:crdt criterion numbers — median merge latency 5–8 µs (small docs), 11–14 µs (medium), 70–88 µs (large), >140 K ops/s on small documents (Apple M2, criterion 0.5, release+LTO).

**Intent-compilation time:** from client `ids_submit(intent)` to compiled execution plan ready to execute (ids-query planning + ids-routing resolution). Measured end-to-end at the client handle; broken down into planning, routing, and validation sub-phases. Both metrics use deterministic inputs and fixed seeds so results are comparable across runs.

#### 3.10 Resource utilisation

Per-benchmark process RSS (peak and steady), CPU usage (% of one core and of machine), allocation counts, and (optionally) hardware events. sysinfo provides `Process::memory` (RSS) and `cpu_usage` (needs two refreshes). Allocation counting via `CountingAllocator`/dhat-style tools or a counting allocator wrapper for allocation-sensitive benches. Welford accumulates mean/variance of repeated runs. Resource numbers are reported per benchmark, never used as pass/fail without a configured budget (platform variance is high).

#### 3.11 Cost per byte and energy per byte

**Cost per byte:** derived from ids-accounting measurements — storage/bandwidth/compute/recovery effort attributed per byte written/read/transferred. ids-bench hooks the accounting counters and reports cost per byte per workload (write, read, repair, recovery).

**Energy per byte:** energy consumed per byte processed, measured where an energy interface exists (RAPL-style CPU/package energy, device energy where exposed; methodology from greensoftwarelab RAPL examples and throughput-per-megabyte studies measuring energy per request on Raspberry Pi 5). Reported as optional feature-gated benchmarks (`energy` feature) so the crate works on hosts without energy interfaces. NVMe energy-performance tools (nvme-energy-bench) show the storage-stack energy trade-off is real and workload-dependent; ids-bench reports it, does not prescribe targets.

#### 3.12 Crypto primitive benchmarks (allowed set only)

Within the allowed set — ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — ids-bench measures sign/verify (ed25519), encrypt/decrypt throughput and latency at 64 B … 1 MiB (xchacha20poly1305, chacha20poly1305), hash throughput (sha256, blake3). Indicative published numbers (hardware-dependent, to be re-measured on the target host): ed25519 sign ~15 µs / verify ~40 µs (i7-10700K, criterion); xchacha20poly1305 encrypt ~4.5 µs at 1 KiB (~217 MiB/s); blake3 ~2–10 GiB/s (parallel), ~2 GiB/s single-thread; sha256 ~2.4 GiB/s; chacha20poly1305 ~1–3 GiB/s. ids-bench records these per payload size and flags regressions vs the stored baseline; no other algorithm may be added without updating this plan and the crypto policy.

#### 3.13 Statistical methodology

- Warmup before measurement (criterion default 3 s; configurable).
- Sample size ≥ 100 measurements per bench; bootstrap CI at 95%.
- Outlier classification (Tukey IQR: samples < Q1 − 1.5·IQR or > Q3 + 1.5·IQR are outliers; mild/severe split).
- Regression detection: compare current sample vs stored previous run; report change with p-value (p ≤ 0.05 ⇒ significant).
- `black_box` on inputs and outputs to defeat constant folding and dead-code elimination; `iter_batched` to exclude setup.
- Determinism: fixed RNG seeds, fixed payload patterns, fixed contact/membership scenarios; same seed + same scenario → identical numbers (within measurement noise).
- Wall-clock noise warning: CI wall-clock can vary >2.5× on identical code; prefer instruction counts (Callgrind-style) or allocation counts for CI pass/fail, wall-clock for local trend (hotpath.rs finding).

#### 3.14 Determinism and reproducibility

Every bench writes a run manifest: Git revision + dirty state, Rust toolchain, OS/kernel, architecture, CPU model, logical CPU count, build profile, workload configuration, feature flags, and full results (schema-versioned JSON, camus-style). Results are comparable only on the same controlled host, device, build profile, and configuration — stated explicitly, no universal thresholds.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Optimized-away benchmark (compiler deletes the measured work) | `std::hint::black_box` on inputs/outputs; `iter_batched` setup exclusion; return values consumed |
| Warmup contamination (cold-start samples mixed into steady state) | Explicit warmup phase; discard warmup samples; report warmup vs measurement split |
| Coordinated omission hiding tail latency | `record_with_expected_interval` on fixed-rate drivers; open-loop drivers preferred |
| Wall-clock noise causing false regressions in CI | 95% CI + p-value; instruction-count/allocation metrics for pass/fail; wall-clock for trends only |
| Percentile sketch inaccuracy at tails | HDR for bounded ranges (significant digits), t-digest/DDSketch for unbounded/mergeable; accuracy targets stated per sketch |
| Cross-node percentile merge inconsistency | Same sketch type + same configuration on all nodes; merge before query |
| Byte accounting drift (lost bytes invisible) | `out_eq_in` invariant check at the end of every bytes-transfer bench; report discrepancy |
| Prohibited crate sneaking into the dependency tree | Dependency audit in CI; `cargo tree` check that no std::net/tokio::net/libp2p/QUIC/HTTP/DNS crate appears; ids-bench imports no networking crate |
| Disallowed crypto algorithm added | ids-crypto is the sole crypto surface; ids-bench references only the five allowed primitive IDs; CI test asserts algorithm allowlist |
| Energy/cost numbers compared across heterogeneous hosts | Run manifest records host + config; numbers comparable only within identical manifest; no cross-host thresholds |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 048 |
|-------|-------------------|
| `ids-000-aa` | Canonical types (`NodeId`, `RecordId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, …), sealed trait contracts, invariants incl. `out_eq_in` (bytes-out == bytes-in + acked); master control point, zero downstream edits for contract changes |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B address), `send_datagram`/`receive_datagram`; the only carrier boundary ids-bench touches |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; ids-bench consumes primitive IDs, never implements crypto |
| `ids-time` | Monotonic IDS standard time source for all latency/deadline measurements (never wall-clock for ordering) |
| `ids-store`, `ids-read` | Read/write workloads under measurement (latency p50/p95/p99, throughput, storage amplification) |
| `ids-recovery` | WAL replay / recovery completion and durability restoration workloads |
| `ids-erasure`, `ids-regenerating-codes`, `ids-ppr-reconstruction` | Repair-bandwidth workloads (encode/decode/repair bytes) |
| `ids-sync`, `ids-crdt`, `ids-membership` | Metadata convergence workloads (post-heal time-to-agreement) |
| `ids-query`, `ids-routing` | Intent-compilation workloads (submit → compiled plan) |
| `ids-fault`, `ids-backpressure` | Bounded recovery/repair bandwidth so foreground traffic is not starved |
| `ids-accounting` | Cost-per-byte counters (storage/bandwidth/compute/recovery effort per byte) |
| `ids-observability` | Metric/trace sinks for benchmark telemetry (optional) |
| `ids-testing` | Reuses property-based/adversarial harness patterns (proptest); ids-bench is the measurement counterpart, not the test harness |

**Integration points:** ids-bench drives public contracts declared in ids-000-aa; it does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what the contracts expose.

---

### 6. Proposed File Structure (~150–220 files in subfolders, one new crate)

**Crate:** `ids-bench` (new crate; depends on ids-000-aa, ids-carrier, ids-crypto, ids-time + criterion, divan (optional feature), hdrhistogram, t-digest/sketches, sysinfo, serde, thiserror, bytes; dev-only: proptest, tempfile; optional `energy` feature for RAPL-style energy hooks)

```
ids-bench/
├── Cargo.toml                          # Crate manifest; dev-dependencies only for benches
├── lib.rs                              # Crate root, feature gates (energy, async, nightly), public re-exports
├── constants.rs                        # Benchmark defaults: payload sizes, sample size, warmup, CI thresholds
├── error.rs                            # BenchError (thiserror): measurement, sketch, invariant, config errors
├── features.rs                         # Feature flags: criterion, divan, energy, async, nightly
│
├── suite/                              # Benchmark suite orchestration
│   ├── mod.rs
│   ├── suite.rs                        # Workload selection, run ordering, manifest emission
│   ├── config.rs                       # Run configuration (payload sizes, concurrency, duration, budgets)
│   ├── manifest.rs                     # Run manifest: git revision, toolchain, host, config
│   ├── runner.rs                       # Runs a workload, aggregates results, checks budgets
│   └── reporter.rs                     # CSV/JSON reporters, criterion-comparison output
│
├── latency/                            # Read/write latency: p50/p95/p99
│   ├── mod.rs
│   ├── runner.rs                       # Drives timed ops, records samples
│   ├── histogram.rs                    # HDR histogram wrapper (bounded range, significant digits)
│   ├── sketch.rs                       # t-digest / DDSketch mergeable aggregation
│   ├── percentiles.rs                  # p50/p95/p99/p99.9 extraction
│   ├── welford.rs                      # Welford online mean/variance
│   ├── coordinated_omission.rs         # record_with_expected_interval correction
│   └── types.rs                        # LatencySample, LatencyResult structures
│
├── throughput/                         # Throughput: MiB/s and ops/s per payload size
│   ├── mod.rs
│   ├── runner.rs                       # Open-loop throughput driver with warmup
│   ├── meter.rs                        # Byte/operation counter over a window
│   ├── curve.rs                        # Size → throughput curve, knee detection
│   └── types.rs                        # ThroughputResult structures
│
├── bytes/                              # Cross-domain bytes transferred
│   ├── mod.rs
│   ├── counter.rs                      # Sent/received byte counters per domain/direction
│   ├── control_plane.rs                # Control vs data plane byte split (frame-type tagged)
│   ├── invariant.rs                    # out_eq_in check: bytes-out == bytes-in + acked
│   └── types.rs                        # ByteTransferResult structures
│
├── recovery/                           # Recovery completion & durability restoration
│   ├── mod.rs
│   ├── runner.rs                       # Crash/restart scenario driver (SIGKILL-style)
│   ├── wal_replay.rs                   # WAL replay timing harness
│   ├── catchup.rs                      # Replica catch-up timing
│   ├── durability.rs                   # Redundancy-restoration timing (node/H-unit/region)
│   └── types.rs                        # RecoveryResult structures
│
├── amplification/                      # Storage amplification
│   ├── mod.rs
│   ├── measurer.rs                     # Logical vs physical byte measurement
│   ├── ratio.rs                        # Amplification ratio before/after compaction, GC
│   └── types.rs                        # AmplificationResult structures
│
├── repair_bandwidth/                   # Repair bandwidth measurement
│   ├── mod.rs
│   ├── runner.rs                       # Single-node / concurrent-failure repair driver
│   ├── bytes.rs                        # Bytes transferred for repair, per helper
│   ├── caps.rs                         # Repair bandwidth cap (foreground protection)
│   └── types.rs                        # RepairResult structures
│
├── control_plane/                      # Control-plane traffic
│   ├── mod.rs
│   ├── meter.rs                        # Control-plane byte meter at carrier boundary
│   ├── ratio.rs                        # Control/data byte ratio per workload
│   └── types.rs                        # ControlPlaneResult structures
│
├── metadata_convergence/               # Metadata convergence time
│   ├── mod.rs
│   ├── runner.rs                       # Partition-heal / node-join convergence driver
│   ├── agreement.rs                    # Time-to-identical-state across replicas
│   └── types.rs                        # ConvergenceResult structures
│
├── intent_compilation/                 # Intent → compiled plan time
│   ├── mod.rs
│   ├── runner.rs                       # ids_submit → plan-ready timing
│   ├── phases.rs                       # Planning / routing / validation sub-phase breakdown
│   └── types.rs                        # CompilationResult structures
│
├── resources/                          # Resource utilisation
│   ├── mod.rs
│   ├── sampler.rs                      # RSS, CPU% sampling (sysinfo) during a bench
│   ├── alloc.rs                        # Allocation counting (counting allocator / dhat)
│   ├── welford.rs                      # Streaming stats over repeated runs
│   └── types.rs                        # ResourceResult structures
│
├── cost_energy/                        # Cost per byte & energy per byte
│   ├── mod.rs
│   ├── cost.rs                         # Cost per byte from ids-accounting counters
│   ├── energy.rs                       # Energy-per-byte hooks (RAPL-style, feature-gated)
│   ├── nvme.rs                         # Optional storage energy (nvme-energy-bench patterns)
│   └── types.rs                        # CostEnergyResult structures
│
├── crypto_bench/                       # Allowed-crypto primitive benchmarks
│   ├── mod.rs
│   ├── ed25519.rs                      # Sign/verify latency at payload sizes
│   ├── xchacha20.rs                    # XChaCha20-Poly1305 encrypt/decrypt
│   ├── sha256.rs                       # SHA-256 throughput
│   ├── blake3.rs                       # BLAKE3 throughput (single + parallel)
│   ├── chacha20.rs                     # ChaCha20-Poly1305 encrypt/decrypt
│   ├── sizes.rs                        # Payload sizes: 64 B … 1 MiB
│   └── allowlist.rs                    # Compile-time assert: only the five algorithms
│
├── harness/                            # Benchmark harnesses (criterion + divan)
│   ├── mod.rs
│   ├── criterion.rs                    # criterion groups, black_box, iter_batched
│   ├── divan.rs                        # Optional divan benchmarks (cross-check)
│   └── nightly.rs                      # Nightly-gated libtest bench (not adopted; placeholder)
│
├── fuzz/                               # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_histogram.rs
│   ├── fuzz_sketch.rs
│   └── fuzz_invariant.rs               # out_eq_in under adversarial byte counts
│
├── tests/                              # Integration & adversarial tests
│   ├── mod.rs
│   ├── suite_tests.rs
│   ├── latency_tests.rs                # Percentile accuracy on known distributions
│   ├── throughput_tests.rs
│   ├── bytes_tests.rs                  # out_eq_in invariant enforcement
│   ├── recovery_tests.rs               # Crash/replay determinism
│   ├── repair_tests.rs                 # Repair bandwidth bounded
│   ├── control_plane_tests.rs
│   ├── convergence_tests.rs
│   ├── compilation_tests.rs
│   ├── resource_tests.rs
│   ├── crypto_bench_tests.rs           # Allowlist enforcement
│   ├── determinism_tests.rs            # Same seed + scenario ⇒ identical results
│   ├── dependency_audit_test.rs        # No prohibited crates in tree
│   └── property_tests.rs               # proptest properties for sketches/counters
│
├── benches/                            # Criterion benchmark binaries (measurement entries)
│   ├── mod.rs
│   ├── read_write.rs                   # Read/write latency + throughput
│   ├── cross_domain_bytes.rs           # Cross-domain bytes + invariant
│   ├── recovery.rs                     # Recovery completion + durability restoration
│   ├── amplification.rs                # Storage amplification
│   ├── repair_bandwidth.rs             # Repair bandwidth
│   ├── control_plane.rs                # Control-plane traffic ratio
│   ├── metadata_convergence.rs         # Metadata convergence time
│   ├── intent_compilation.rs           # Intent-compilation time
│   ├── resources.rs                    # Resource utilisation
│   ├── cost_energy.rs                  # Cost/energy per byte
│   └── crypto.rs                       # Allowed-crypto benchmarks
│
├── spec/                               # Normative specifications
│   ├── measurement_spec.md             # What each metric means and how it is measured
│   ├── statistical_spec.md             # CI, outlier, regression methodology
│   ├── determinism_spec.md             # Determinism and reproducibility contract
│   └── carrier_agnostic_spec.md        # Carrier-agnostic boundary contract
│
├── docs/                               # Documentation
│   ├── architecture.md                 # Bench architecture and layering
│   ├── runbook.md                      # How to run, read, and compare bench results
│   ├── adding_a_bench.md               # Adding a new measurement domain
│   └── crypto_policy.md                # Allowed-crypto allowlist policy
│
├── baselines/                          # Stored baseline results for regression comparison
│   ├── read_write_baseline.json
│   ├── crypto_baseline.json
│   └── recovery_baseline.json
│
└── test_vectors/                       # Deterministic inputs and expected outputs
    ├── payloads.json                   # Fixed payload patterns per size
    ├── scenario_vectors.json           # Crash/recovery scenarios
    └── sketch_vectors.json             # Expected quantiles on known distributions
```

**Why ~150–220 files:** one file per responsibility — twelve measurement domains, each with runner + types + result types; harness layer (criterion/divan); suite orchestration; resources; cost/energy; crypto allowlist; fuzz; tests; property tests; bench binaries; specs; docs; baselines; test vectors. Reflects the genuine measurement-and-statistics complexity of a cross-cutting benchmarking crate. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Latency histogram (HDR)
```
HdrHistogram { significant_digits: 1..=5, low, high, buckets[] }
record(value): O(1), no allocation, bucket index from log2 bucket + linear sub-bucket
value_at_percentile(p): scan buckets cumulatively until count ≥ p·total
p50 = value_at_percentile(0.50), p95 = 0.95, p99 = 0.99
Coordinated omission: record_with_expected_interval(value, interval) backfills stall samples
```

#### 7.2 Mergeable quantile sketch (t-digest / DDSketch)
```
TDigest { centroids[], compression δ }
record(x): O(1) amortised; centroid weight grows near tails
quantile(q): O(δ); merge(digests): combine + re-centroid, O(δ₁+δ₂)
DDSketch: logarithmic buckets, relative error ε; true q within [(1−ε)·v, (1+ε)·v]
```
Used to aggregate percentiles across benchmark workers/nodes.

#### 7.3 Welford online statistics
```
state = (n, mean, M2)
push(x): n += 1; delta = x − mean; mean += delta/n; M2 += delta·(x − mean)
variance = M2 / n (population) or M2 / (n−1) (sample)
merge(a, b): combine two partitions in O(1) (parallel mean/variance)
```

#### 7.4 Throughput meter
```
Meter { window_start, bytes, ops }
record(bytes_count, op_count)
throughput = bytes / elapsed  (MiB/s), ops / elapsed (ops/s), steady-state only
```

#### 7.5 out_eq_in invariant check
```
bytes_out == bytes_in + bytes_acked   (per domain, per direction, tolerance ε)
violation → measurement bug flagged, run result marked inconsistent
```
From `ids-000-aa` invariant `out_eq_in`.

#### 7.6 Repair bandwidth
```
repair_bytes = Σ bytes fetched from helpers; repair_time = completion − start
MSR target: repair_bytes ≈ (d·β) where β < full chunk (clay codes: 37–67% vs Reed-Solomon)
cap: repair bandwidth ≤ configured budget so foreground traffic is protected
```

#### 7.7 Convergence / compilation timing
```
t_start = ids-time tick at intent submission / partition heal
t_end   = ids-time tick at plan-ready / state agreement across all replicas
elapsed = t_end − t_start   (ids-time ticks, monotonic, never wall-clock)
```

#### 7.8 Cost / energy per byte
```
cost_per_byte   = accounting_effort_units / logical_bytes_processed
energy_per_byte = package_energy_delta_joules / logical_bytes_processed   (optional feature)
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types, sealed contracts, invariants (`out_eq_in`), `IdsTime` |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Monotonic time source for all measurements |
| `ids-store`, `ids-read`, `ids-recovery`, `ids-erasure`, `ids-regenerating-codes`, `ids-ppr-reconstruction`, `ids-sync`, `ids-crdt`, `ids-membership`, `ids-query`, `ids-routing`, `ids-fault`, `ids-backpressure`, `ids-accounting`, `ids-observability` | Workloads under measurement (via public contracts only) |
| `criterion` | Primary benchmark harness (statistical analysis, regression detection) |
| `divan` | Optional cross-check harness (async, contention, allocation profiling) |
| `hdrhistogram` | HDR latency histogram (p50/p95/p99) |
| `tdigest` / `sketches` | Mergeable quantile sketches (t-digest, DDSketch) |
| `sysinfo` | Process RSS and CPU usage sampling |
| `serde` + `serde_bytes` | Result serialization (CSV/JSON manifests) |
| `thiserror` | Error types |
| `bytes` | Buffer handling for payload datagrams |
| `proptest` | Property-based tests |
| `tempfile` | Test fixtures |

No networking crates: no `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS implementations in or under `ids-bench`. Crypto beyond the five allowed primitives is rejected; `ids-bench` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-48.1 | Read/write latency p50/p95/p99 reported per payload size (64 B … 1 MiB) | HDR histogram + sketch, post-warmup, 95% CI |
| AC-48.2 | Throughput reported in MiB/s and ops/s per payload size | Throughput meter, steady state after warmup |
| AC-48.3 | Cross-domain bytes transferred reported per direction/domain | Byte counters at carrier boundary |
| AC-48.4 | out_eq_in invariant holds: bytes-out == bytes-in + acked (tolerance ε) | Invariant check at end of every bytes-transfer bench; violations fail the run |
| AC-48.5 | Recovery completion time reported (crash → service resumed) | WAL replay + catch-up timing, ids-time keyed |
| AC-48.6 | Durability restoration time reported (node / H-unit / region failure) | Redundancy-restoration timing with bounded repair bandwidth |
| AC-48.7 | Storage amplification reported (physical / logical) per workload | Before/after compaction and GC |
| AC-48.8 | Repair bandwidth reported per failed node/fragment | Bytes transferred vs helpers; MSR target 37–67% savings vs naive RS (indicative) |
| AC-48.9 | Control-plane / data-plane byte ratio reported per workload | Frame-type-tagged byte meter |
| AC-48.10 | Metadata convergence time reported after partition heal / node join | Time-to-identical-state across replicas |
| AC-48.11 | Intent-compilation time reported (submit → plan ready) | Sub-phase breakdown: planning / routing / validation |
| AC-48.12 | Resource utilisation reported: peak RSS, CPU%, allocation counts | sysinfo sampler + counting allocator |
| AC-48.13 | Cost per byte and energy per byte reported (energy feature-gated) | ids-accounting counters; RAPL-style energy where available |
| AC-48.14 | Crypto benchmarks for ed25519 sign/verify, xchacha20poly1305, sha256, blake3, chacha20poly1305 at 64 B … 1 MiB | Throughput + latency per payload size; indicative targets: sign ≤ ~20 µs, verify ≤ ~50 µs, sha256 ≥ ~2 GiB/s, blake3 ≥ ~2 GiB/s single-thread, AEAD ≥ ~1 GiB/s (re-measure on target host) |
| AC-48.15 | Statistical rigor: 95% CI, outlier classification, regression vs baseline with p-value | Criterion analysis; p ≤ 0.05 ⇒ significant change |
| AC-48.16 | Determinism: same seed + scenario ⇒ identical results | Determinism tests pass |
| AC-48.17 | Carrier agnosticism: compiles/behaves identically with zero direct dependencies on TCP/IP, UDP, QUIC, HTTP, DNS, libp2p; all traffic via ids-carrier::Endpoint | Dependency audit test (`cargo tree`) passes |
| AC-48.18 | Crypto allowlist: only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 reachable | Allowlist unit test passes |
| AC-48.19 | Run manifest emitted per run (git revision, dirty state, toolchain, host, config, results) | Schema-versioned JSON |

---

### 10. Remaining Uncertainties

1. **Harness split between criterion and divan:** criterion is primary for statistical rigor; divan's exact role (async workloads only, or full cross-check) to be confirmed during implementation.
2. **Energy interface availability:** RAPL-style energy may not exist on target hosts; `energy` feature must degrade gracefully (feature off, not compile error).
3. **Baseline calibration:** indicative crypto/throughput numbers (ed25519 ~15 µs sign, sha256 ~2.4 GiB/s, etc.) are hardware-dependent and must be re-measured on the target host before any regression thresholds are set.
4. **out_eq_in tolerance ε:** the tolerance for byte-accounting discrepancies under loss/retransmission is workload-dependent; unquantified.
5. **Repair bandwidth caps:** MSR/clay-code savings (37–67%) are paper numbers; actual repair bandwidth under IDS topology constraints unmeasured.
6. **Storage amplification range:** depends on compaction/GC policy of ids-store/ids-reclaim; bench reports ratios, targets are product decisions.
7. **ids-time granularity:** monotonic tick resolution must be sufficient for sub-microsecond crypto benchmarks; clock source to be confirmed.
8. **Nightly libtest bench:** explicitly not adopted; revisited only if criterion/divan prove insufficient for a specific workload class.
9. **Cross-node sketch merge:** t-digest/DDSketch merge semantics across replicas need a single configured compression/ε; convergence of merged percentiles vs per-node percentiles untested.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-bench` crate, not a folder | Project rule: separate crates for new capabilities; keeps existing crates focused |
| criterion.rs as primary harness | Statistical confidence (CI, outliers, regression detection), stable Rust, ecosystem standard (ed25519-dalek uses it) |
| divan as optional cross-check | Different measurement style (fast startup, instruction-count-friendly, async, contention); guards against harness-specific bias |
| bencher/libtest bench rejected | Nightly-only, unmaintained; IDS targets stable toolchains |
| HDR histogram primary for p50/p95/p99 | Constant-time record (~3–6 ns), fixed memory, tail accuracy on bounded ranges; subms cookbook data |
| t-digest / DDSketch for cross-worker aggregation | Mergeable, tight tails (t-digest) or relative error (DDSketch) where HDR's fixed range is insufficient |
| Coordinated-omission correction on fixed-rate drivers | Prevents stall-masking tail illusions (subms cookbook / Gil Tene) |
| Welford online mean/variance | O(1) streaming stats without storing samples; mergeable partitions |
| All traffic via ids-carrier::Endpoint (`CarrierType` + `Vec<u8>`) | Carrier-agnostic absolute rule; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |
| Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS crypto policy; consumed via ids-crypto primitive IDs, never implemented in ids-bench |
| ids-time monotonic for all deadlines | No wall-clock or RTT-derived timeouts; correct under arbitrary delays |
| out_eq_in invariant as self-check | bytes-out == bytes-in + acked from ids-000-aa; catches lost byte accounting |
| Run manifest per run (git, toolchain, host, config) | Numbers comparable only within identical manifest (no universal thresholds, per camus/hotpath findings) |
| Regression detection via criterion comparison + p-value | 95% CI; wall-clock noise >2.5× on identical code → instruction counts for CI pass/fail |

---

### Sources

- criterion.rs documentation — statistics-driven microbenchmarking, black_box, four phases (warmup/measurement/analysis/comparison), Tukey IQR outlier detection, 95% CI, bootstrap: https://criterion-rs.github.io/book/print.html and https://bheisler.github.io/criterion.rs/book/getting_started.html
- HdrHistogram / hdrhistogram (Rust port of Gil Tene's HDR histogram) — p50/p95/p99, ~3–6 ns record, fixed memory: https://docs.rs/crate/hdrhistogram/7.5.4 ; https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html
- Sub-millisecond HDR histogram cookbook — record p99 ~51 ns, coordinated-omission `record_with_expected_interval`: https://submillisecond.com/cookbook/recipes/subms-hdr-histogram
- t-digest (MnO2/t-digest) — streaming quantiles, mergeable, tight tails: https://github.com/MnO2/t-digest
- DDSketch (quantile-sketch) — relative-error quantiles, mergeable: https://docs.rs/crate/quantile-sketch/latest
- Rust histogram benchmark (HDR vs t-digest vs DDSketch, recording throughput, percentile latency): https://github.com/drmingdrmer/rust-histogram-bench
- divan benchmark crate — `#[divan::bench]`, black_box, async, thread contention: https://docs.rs/divan/0.1.4 ; https://codspeed.io/docs/guides/how-to-benchmark-rust-with-divan
- bencher.dev — criterion, libtest bench (nightly), gungraun (instruction counts), iai options: https://bencher.dev/learn/benchmarking/rust/criterion
- ed25519-dalek criterion benchmarks — sign ~15 µs, verify ~40 µs (i7-10700K): https://docs.rs/crate/ed25519-dalek/latest
- xchacha20poly1305 criterion report — encrypt ~4.5 µs at 1 KiB (~217 MiB/s): https://ioi.network/assets/criterion/xchacha20poly1305_encrypt/1024/report
- oxicrypto bench — indicative throughput: SHA-256 ~1–3 GiB/s, BLAKE3 ~3–10 GiB/s, ChaCha20-Poly1305 / XChaCha20-Poly1305 ~1–3 GiB/s: https://github.com/cool-japan/oxicrypto/blob/master/crates/oxicrypto-bench/README.md
- checksum-benmarks (conradludgate) — SHA-256 ~2.4 GB/s, BLAKE3 ~2 GB/s single-thread / ~7.87 GB/s MT: https://github.com/conradludgate/checksum-benchmarks
- rusty-comms — IPC benchmark, p50/p95/p99, MiB/s per message size: https://github.com/redhat-performance/rusty-comms
- sysinfo — process RSS (`memory`) and CPU usage (`cpu_usage`): https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html
- Welford online algorithm — streaming mean/variance: https://docs.rs/anomstream-core/2026.4.1/x86_64-unknown-linux-gnu/src/anomstream_core/online_stats.rs.html ; https://docs.rs/scirs2-core/0.4.2/scirs2_core/streaming_stats
- WAL recovery benchmarks — ~78–122 ms for 1M × 128 B replay (~25M rec/s sequential), recovery-dominated-by-file-open finding: https://github.com/anirudhpundhir/wal-engine ; https://github.com/Shadylukin/Neumann/blob/main/docs/book/src/reference/benchmarks/tensor-store.md ; https://raw.githubusercontent.com/jamesgober/wal-db/main/docs/BENCHMARKS.md
- LSM write amplification 10–30× (Tidehunter, arxiv:2602.01873): https://arxiv.org/html/2602.01873v1
- reed-solomon-simd — encode ~10 GiB/s (32:32), decode hundreds of MiB/s–1.4 GiB/s: https://github.com/AndersTrier/reed-solomon-simd/blob/master/README.md
- clay codes (MSR repair) — 37–67% repair-bandwidth savings vs Reed-Solomon: https://github.com/t348575/energy-benchmark (repo context) / https://lib.rs/crates/clay-codes ; https://github.com/t348575/nvme-energy-bench
- hotpath.rs — CI regression: wall-clock noise >2.5×, instruction counts stable within 0.13%: https://hotpath.rs/blog/rust-performance-benchmarking
- greensoftwarelab RAPL — energy measurement framework: https://github.com/greensoftwarelab/RosettaExamples
- Throughput per megabyte / energy per request (Raspberry Pi 5, arxiv:2609.11932): https://arxiv.org/pdf/2609.11932
- nvme-energy-bench — NVMe energy-performance benchmarking: https://github.com/t348575/nvme-energy-bench
- did:crdt criterion merge benchmarks — 5–88 µs merge latency, >140 K ops/s small docs (Apple M2): https://arxiv.org/html/2606.16223
- std::hint::black_box — optimization prevention for benchmarks: https://doc.rust-lang.org/std/hint/fn.black_box.html
- camus benchmarks — schema-versioned JSON run manifests (git revision, toolchain, host, HDR percentiles): https://github.com/leiysky/camus/blob/main/docs/benchmarks.md
- IDS project documents at C:/idfs: CRATES.md (crate inventory, ids-000-aa master, ids-carrier Endpoint, ids-crypto allowed set, ids-bench as crate #48), IDS.md, prompts.txt entry 48 (lines 715-729), plans/plan-040.md (IDS plan format precedent)

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, measurement domains, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 048 (ids-bench).