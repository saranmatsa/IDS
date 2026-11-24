## Phase 076 — Create the ids-phi-accrual crate: Phi accrual suspicion with adaptive thresholds. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1135-1149)

```
76. Research only do not read plan.md **[Phase 076 — Create the ids-phi-accrual crate: Phi accrual suspicion with adaptive thresholds.]**

Crate: ids-phi-accrual
Purpose: Phi accrual suspicion with adaptive thresholds.
Contents: Phi value calculation, inter-arrival time distribution, suspicion threshold, adaptive accuracy, GC pause handling, false positive/negative tuning.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 076 — Create the ids-phi-accrual crate: Phi accrual suspicion with adaptive thresholds.]**

Create plan-076.md in C:/idfs/plans/ for the ids-phi-accrual crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 076 creates `ids-phi-accrual`, the IDS crate implementing phi (φ) accrual suspicion with adaptive thresholds. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by statistical-monitoring concern.

`ids-phi-accrual` implements the accrual failure detector described by Hayashibara et al. (2004), "The φ Accrual Failure Detector": instead of a binary alive/dead timeout, it outputs a continuous suspicion level φ on a logarithmic scale. φ is computed as `-log10(P_later)`, where P_later is the probability that a heartbeat would arrive at least as late as the current silence, given the historical distribution of heartbeat inter-arrival times. Each unit of φ represents another factor of ten in implausibility: φ=1 ≈ 10% chance of a transient delay, φ=8 ≈ 1-in-100-million. The crate never declares a node dead on its own — it publishes φ continuously, and consumers (membership, lease, replication paths) apply their own thresholds.

**Core design per crate:** each monitored peer gets an independent `PhiDetector` instance holding:
- a bounded sliding window (ring buffer) of recent inter-arrival intervals (default 1000 samples, matching Cassandra and Akka),
- running mean and standard deviation estimated via Welford's online algorithm or recomputed from the window,
- timestamp of the last received heartbeat,
- configurable `acceptable_heartbeat_pause` (margin to survive GC pauses / scheduling hiccups; Akka default 3 s),
- configurable `min_std_deviation` floor (prevents φ exploding to infinity on a perfectly steady sender),
- configurable `first_heartbeat_estimate` (bootstrap for cold start; Cassandra seeds with 2× heartbeat interval),
- configurable `phi_threshold` (default 8; cloud/WAN deployments raise to 10–12).

The crate is **pure computation**: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. Heartbeats arrive as opaque `Vec<u8>` datagrams at the carrier boundary via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate records arrival timestamps and computes φ but never sends bytes itself. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — used only for signing/digesting suspicion checkpoints and audit entries; `ids-crypto` exposes primitive IDs, `ids-phi-accrual` consumes them via `ids-000-aa` only.

**Scope boundary:** `ids-phi-accrual` estimates suspicion; it does not evict, remap, or reconfigure. It publishes φ and lets the caller decide. Thresholds are configurable per peer/profile, not hard-coded, so a noisy path degrades gracefully rather than falsely convicting.

A skeleton `ids-phi-accrual` crate does NOT yet exist at `C:/idfs/ids-phi-accrual/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Hayashibara/Défago/Yared/Katayama 2004 SRDS paper (intercontinental link study, window size 1000, mistake rate vs window size), Akka `PhiAccrualFailureDetector.scala` (φ = -log10(1-CDF), logistic approximation `1/(1+exp(-y*(1.5976+0.070566*y*y)))`, error 0.00014 at ±3.16, acceptable-heartbeat-pause, firstHeartbeatEstimate, clock abstraction for testing), Apache Pekko docs (default threshold 8, cloud EC2 = 12), Cassandra `FailureDetector.java` + `ArrivalWindow` (bounded circular buffer 1000 intervals per endpoint, `PHI_FACTOR = 1/ln(10)`, MAX_INTERVAL clamped to rpc_timeout to avoid window poisoning during long partitions, seeds new peers with 2× gossip interval), Hazelcast docs (default threshold 10, sample size 200, min std dev millis floor), Rust crates `phi-accrual-failure-detector`, `atomr-remote`, `phi-accrual-detector`, `phi_detector` (EWMA variants, constant-memory two-value estimators, `PingWindow`/`NormalDist`), `suspect_machines` (Ruby, warming-up state when below min_samples, monotonic nanosecond clock), SWIM vs phi-accrual comparison (continuous suspicion vs binary suspect/dead with indirect probing), threshold tuning guidance (φ=8 = 1-in-100M false positive; φ=5 ≈ 1-in-100k; detection time 8–12 s LAN, 15–25 s WAN/GC), window-size tradeoffs (small W adapts fast but false-positives on gradual degradation; large W smooths but sluggish; sweet spot 500–2000), GC-pause handling (acceptable pause margin; never age out window entries during silence — that inflates variance and suppresses φ when most needed), adaptive-accrual alternatives (Satzger et al., no normal-distribution assumption).

---

### 2. Architecture Overview

`ids-phi-accrual` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a per-peer statistical estimation layer: every time a heartbeat datagram arrives from a peer, the caller records the arrival timestamp; on demand the crate returns φ for that peer. It never sends bytes itself — it consumes timestamps and publishes suspicion.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    CALLER LAYER (membership / lease / replication)            │
│  ids-membership, ids-lease, ids-replication — record heartbeat arrivals,      │
│  read φ, apply per-consumer threshold (aggressive φ>3 vs conservative φ>8)    │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 076: ids-phi-accrual (THIS CRATE — estimation)           │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────┐ ││
│  │  phi_detector    │ │  interval_window │ │  distribution    │ │  clock   │ ││
│  │  (φ = -log10 P)  │ │  (ring buffer    │ │  (normal approx, │ │  (monotonic│ ││
│  │  per peer        │ │   1000 samples)  │ │   Welford/EWMA)  │ │   ticks) │ ││
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └──────────┘ ││
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │            │ ││
│  │  thresholds      │ │  pause_margin    │ │  audit/checkpoint│ │            │ ││
│  │  (φ_threshold    │ │  (GC pause       │ │  (signed φ log   │ │            │ ││
│  │   per peer)      │ │   tolerance)     │ │   entries)       │ │            │ ││
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ │            │ ││
└──────────────────────────────────────────────────────────────────┬──────────┘│
                                       │ opaque Vec<u8> datagrams
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  Heartbeats arrive as opaque Vec<u8> to Endpoint{CarrierType, addr ≤1024 B} │
│  ids-phi-accrual receives arrival timestamps only; never sends, never opens  │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one heartbeat cycle:**
1. Peer P sends a heartbeat datagram (opaque `Vec<u8>`) via `ids-carrier::Endpoint`.
2. Caller (e.g. `ids-membership`) receives it, notes arrival tick `t_now` from `ids-time` (monotonic, never wall-clock), and calls `detector.record_heartbeat(P, t_now)`.
3. `ids-phi-accrual` computes interval `Δ = t_now - t_last`, appends to P's bounded ring window, updates running mean/variance, stores `t_last = t_now`.
4. On suspicion query `detector.phi(P, t_now)`: computes `elapsed = t_now - t_last`; if window empty → return 0.0 (warming-up); otherwise `y = (elapsed - mean - pause_margin) / stddev`; φ via logistic approximation of the normal CDF tail; φ grows monotonically with silence.
5. Caller compares φ against its own per-consumer threshold (lease path φ>3, membership eviction φ>8, alerting φ>12) and acts. `ids-phi-accrual` never evicts.

**Phi formula (research-verified):**
- `φ(t) = -log10(1 - F(t))` where `F` is the CDF of `N(mean, stddev)` estimated from historical inter-arrival times.
- `F(t)` approximated by the β Mathematics Handbook logistic form: `1 / (1 + exp(-y*(1.5976 + 0.070566*y*y)))`, `y = (t - mean)/stddev`, error 0.00014 at ±3.16.
- Cassandra variant: `φ = (1/ln(10)) * (elapsed / mean)` (exponential approximation, no σ) — noted as a divergent implementation; ids-phi-accrual follows the paper/Akka normal-distribution form.
- `acceptable_heartbeat_pause` shifts the mean: `effective_mean = mean + pause` so GC pauses don't trigger suspicion.
- `min_std_deviation` floor prevents divide-by-zero / infinite φ on steady senders.

**Crypto used:** ed25519 for signing φ checkpoint entries; sha256/blake3 for digesting suspicion logs and checkpoint chaining; xchacha20poly1305/chacha20poly1305 to encrypt per-peer suspicion state at rest when confidentiality required. No other crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources cited inline; full list in Section 13.

#### 3.1 Phi accrual failure detector (Hayashibara et al., 2004)

**Source:** Hayashibara, Défago, Yared, Katayama, "The φ Accrual Failure Detector," SRDS 2004 (IEEE), also available as JAIST IS_RR_2004_010 / HDY04.pdf. Analyzed over an intercontinental link for a week; φ performs equally well as other adaptive failure detectors with improved flexibility.

An accrual failure detector outputs a continuous suspicion level rather than binary trust/suspect. The φ detector samples heartbeat arrival times, maintains a sliding window of recent samples, estimates the distribution of future arrivals from past samples, and computes φ on a dynamically adjusted scale matching recent network conditions. Higher φ = higher chance the monitored process has crashed. If the process actually crashes, φ is guaranteed to accrue toward infinity. Applications set their own threshold per QoS requirements: low threshold → many false suspicions but quick detection; high threshold → fewer mistakes but slower detection.

Window size experiments: mistake rate improves as window size increases; curve flattens for large values (diminishing returns). Default window 1000 samples; analysis excluded warmup period (before window fills).

#### 3.2 Phi computation: normal CDF and logistic approximation

**Source:** Akka `PhiAccrualFailureDetector.scala`; Pekko docs; Apache Pekko `PhiAccrualFailureDetector`; `atomr-remote` Rust source; `phi_accrual_detector` Rust crate docs.

`φ = -log10(1 - F(timeSinceLastHeartbeat))`, `F` = CDF of normal distribution with mean and standard deviation estimated from historical heartbeat inter-arrival times.

Logistic approximation (β Mathematics Handbook): `F(y) ≈ 1 / (1 + exp(-y*(1.5976 + 0.070566*y*y)))`, `y = (x - mean)/stddev`. Error 0.00014 at ±3.16. Equivalent to `-log10(1 - CDF(y))`.

Akka adds two operational knobs:
- `acceptableHeartbeatPause`: margin (default 3 s) added to the mean so sudden GC pauses / network drops don't raise suspicion. Warning logged when heartbeat interval exceeds 2/3 of the pause.
- `firstHeartbeatEstimate`: bootstrap duration with high stddev when environment unknown at start.
- `Clock` abstraction returning milliseconds, faked for testing.

Threshold: default 8; cloud environments (EC2) raise to 12. Sweet spot 8 ≤ φ < 12 per Satzger et al.; detection time rises sharply beyond threshold 10–11.

#### 3.3 Inter-arrival time distribution and sliding window

**Source:** Cassandra `FailureDetector.java` + `ArrivalWindow`; Hazelcast docs (sample size 200, min std dev millis); systemdesignsimulator.org phi-accrual internals; padho-wiki phi accrual.

Each peer maintains a bounded sliding window (ring buffer) of recent inter-arrival intervals — default 1000 samples (Cassandra/Akka), 200 (Hazelcast). The window tracks a running sum so `mean = sum/count` is O(1); when full, oldest entry evicted. Mean reflects ~last 1000 seconds of heartbeat history at 1 s cadence.

Critical discipline: the window is updated **only on heartbeat arrival** — no entries added during silence, no entries aged out by time. Aging out entries during long silence artificially inflates variance and suppresses φ exactly when needed most.

Cassandra clamps `MAX_INTERVAL_IN_MS` to `rpc_timeout`: during a long partition, intervals longer than the RPC timeout are ignored rather than recorded, preventing the detector from adapting to a dead peer and delaying conviction.

Bootstrap: new peer seeded with `2 × heartbeat_interval` as a conservatively high initial mean so it isn't immediately suspected before the window fills.

#### 3.4 Welford online statistics vs recompute-from-window

**Source:** systemdesignsimulator.org; multiple implementations.

Running mean/variance updated online (Welford) or recomputed from the window when a sample is evicted. Welford: O(1) per sample, numerically stable; mergeable partitions. Recompute-from-window: O(window) on eviction but simple and exact. Both acceptable; window ≤ 1000 makes recompute cheap.

#### 3.5 GC pause handling and acceptable heartbeat pause

**Source:** Akka acceptableHeartbeatPause (default 3 s); dipach.github.io Cassandra analysis (stop-the-world GC pause → false positive vs slow detection tradeoff); systemoverflow SWIM vs phi-accrual (detection stretches to 15–25 s during 2–5 s GC pauses); padho-wiki (σ floor prevents infinite φ on first delayed packet).

The `acceptable_heartbeat_pause` is the key production knob: it represents the number of potentially lost/delayed heartbeats accepted before considering it an anomaly. Without it, any GC pause causes false suspicion. A minimum stddev floor (`min_std_deviation`) is equally important: a perfectly steady sender has σ → 0, and any silence shoots φ to infinity. Floor stddev (e.g. 1 ms, or 25% of first_heartbeat_estimate) bounds sensitivity.

#### 3.6 Cold start and warming-up state

**Source:** seuros/suspect_machines (warming-up: reports samples + required + silence until min_samples intervals); Akka firstHeartbeatEstimate; Cassandra seed 2× interval.

Until a peer has `min_samples` intervals, there is no basis for φ. Options: return φ=0 (treat as healthy, Akka behavior), return a `WarmingUp` state (suspect_machines), or bootstrap with `first_heartbeat_estimate` and a high stddev (Akka). A peer sending one heartbeat then vanishing stays warming up forever unless a warm-up ceiling is configured; `scan` names it once silence passes the ceiling.

#### 3.7 Threshold tuning: false positive vs detection latency

**Source:** Hazelcast docs (φ=1 ≈ 10% mistake, φ=2 ≈ 1%, φ=3 ≈ 0.1%, default 10); systemoverflow (φ=8 = 1-in-100M, LAN detection 8–12 s, WAN/GC 15–25 s); padho-wiki (φ=5 ≈ 1-in-100k; φ=8 crossing at ~1.85 s for μ=100 ms cadence); manuel.bernhardt.io (sweet spot 8 ≤ φ < 12; sharp detection-time increase beyond 10–11).

Per-consumer thresholds: same φ stream can feed an aggressive lease-renewal path (φ>3) and a conservative remove-from-membership path (φ>8) simultaneously. Two-tier alerting: auto-failover at φ=8, page humans at φ=10–12.

#### 3.8 Window size adaptation tradeoffs

**Source:** padho-wiki (W=50 chases jitter aggressively; W=5000 sluggish to regime changes; sweet spot 500–2000, upper bound for fast cadence ≤100 ms, lower bound for slow cadence ≥1 s); Hayashibara 2004 (mistake rate improves with window size, flattens for large values).

Small W: fast adaptation to current jitter, but gradual deterioration (slowly failing NIC adding 20 ms/packet) never raises σ enough → false positives accumulate. Large W: smooths jitter but slow to react to network recovery; cluster pays latency tax for a full window after recovery.

#### 3.9 Phi accrual vs SWIM vs fixed timeouts

**Source:** systemoverflow SWIM vs phi-accrual; dipach.github.io; learn.padho.ai.

Fixed timeout conflates estimation and decision. SWIM: alive/suspect/dead states, indirect probing via 3–5 random members, reduces false positives from transient issues but adds gossip rounds. Phi accrual: continuous suspicion, decouples monitoring from interpretation — multiple consumers apply different thresholds to the same stream. No indirect probing in the detector itself; that is a membership-layer concern (`ids-membership`, not `ids-phi-accrual`).

#### 3.10 Rust implementations and API patterns

**Source:** docs.rs `phi_accrual_failure_detector`, `phi_accrual_detector`, `phi_detector`, `atomr_remote` source; lib.rs `phi-accrual-failure-detector`.

Common Rust API: `heartbeat(timestamp)` records arrival; `phi(timestamp)` returns f64 suspicion; `is_available()` = φ < threshold; `Clock` trait abstracts time for deterministic testing; config fields: threshold, max_samples, min_std_deviation, acceptable_heartbeat_pause, first_heartbeat_estimate. `phi_detector` uses constant memory (two values: sum and count) instead of storing all pings.

#### 3.11 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek docs.rs (RFC 8032 EdDSA over Curve25519); chacha20poly1305 docs.rs (RFC 8439 AEAD + XChaCha20 192-bit nonce); sha2 RustCrypto (SHA-256); BLAKE3-team README (256-bit output, SIMD, keyed hash, KDF, XOF).

`ids-phi-accrual` uses:
- ed25519: signing φ checkpoint entries (tamper-evident suspicion log).
- sha256: checkpoint chaining, suspicion-entry hashing.
- blake3: fast content hashing for suspicion-state snapshots, parallel-friendly.
- xchacha20poly1305: encrypt per-peer suspicion state at rest (24-byte nonce, random nonces safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based bounded use).
All via `ids-crypto` primitive IDs from `ids-000-aa`; `ids-phi-accrual` implements no crypto itself.

#### 3.12 Determinism and testing

**Source:** sir-teo.github.io "The Heartbeat Is a Suspicion Score" (deterministic lab for a given seed; Clock abstraction faked for testing; default run 26 heartbeats before crash, φ crosses threshold after ~1.80 s silence with threshold 4.0 and 250 ms pause margin).

Deterministic testing requires a faked/injectable clock (`ids-time` monotonic ticks). Property tests: φ resets toward 0 after a heartbeat arrives; φ grows monotonically during silence; φ=0 when no heartbeats received; window eviction bound; min_stddev floor prevents infinite φ.

#### 3.13 Carrier-agnostic boundary

**Source:** IDS prompts.txt entry 76; plan-001/plan-048/plan-060 format; `ids-carrier::Endpoint` = CarrierType + Vec<u8> (max 1024 bytes); CarrierType: Loopback, Serial, Optical, Radio, Custom(u16).

`ids-phi-accrual` receives heartbeat arrival timestamps only. It never constructs an `Endpoint`, never calls `send_datagram`, and imports no networking crate. The caller wires heartbeats (which arrive over `ids-carrier::Endpoint`) into the detector. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| False positive suspicion (GC pause, scheduling hiccup) | Availability | `acceptable_heartbeat_pause` margin; `min_std_deviation` floor; per-consumer thresholds |
| Slow detection of real failures | Liveness | Threshold tuning per path (aggressive φ>3 for lease, conservative φ>8 for eviction); two-tier alerting |
| Window poisoning during long partition | Detection accuracy | Clamp max recorded interval to rpc_timeout (Cassandra pattern); never age out window during silence |
| Cold-start false suspicion | Availability | Bootstrap with `first_heartbeat_estimate` + high stddev; warming-up state until min_samples |
| σ → 0 on steady sender → infinite φ | False positives | `min_std_deviation` floor; never return infinite φ |
| Clock manipulation / wall-clock jump | Suspicion correctness | Monotonic `ids-time` ticks only; injectable `Clock` for tests |
| Audit/tampering of suspicion log | Accountability | ed25519-signed checkpoint entries; sha256/blake3 hash chain |
| Prohibited crate in dependency tree | Carrier-agnostic rule | CI dependency audit (`cargo tree`); imports no networking crate |
| Disallowed crypto algorithm | Crypto policy | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto; allowlist unit test |
| φ stream used as binary verdict | Semantic misuse | Detector never decides; publishes φ only; consumer applies threshold |
| Correlated multi-node failure | Membership accuracy | Out of scope for this crate (membership layer handles); φ is per-peer independent |

The carrier is never trusted: φ is computed from locally observed arrival timestamps, surviving any carrier re-framing or delay; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-phi-accrual ||
|-------|------|-------------------------------|--------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `IdsTime`, `CarrierType`, `Endpoint`, `Error` — consumed, never redefined; `Clock` built on `IdsTime` monotonic ticks |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram transport | Heartbeats arrive as opaque `Vec<u8>` over `Endpoint`; crate never sends or constructs endpoints |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing φ checkpoints, digesting suspicion state, encrypting per-peer state at rest |
| `ids-time` | Monotonic IDS standard time source | All interval/elapsed measurements keyed to `IdsTime` ticks; never wall-clock |
| `ids-membership` (later phase) | Cluster membership / failure detection primary consumer | Records heartbeat arrivals per peer, reads φ, applies per-consumer threshold |
| `ids-lease` (later phase) | Lease renewal path | Aggressive φ threshold (e.g. φ>3) on the same heartbeat stream |
| `ids-replication` (later phase) | Replica liveness for quorum decisions | Conservative φ threshold (e.g. φ>8) before evicting a replica |

**Integration points:** `ids-phi-accrual` is a pure library — `record_heartbeat(peer_id, tick)` and `phi(peer_id, tick) -> f64`. It holds no sockets, spawns no tasks, opens no files. All cross-peer traffic is the caller's responsibility via `ids-carrier::Endpoint`. No dependency on `ids-discovery`, `ids-session`, or `ids-cli`.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-phi-accrual` (new crate at `C:/idfs/ids-phi-accrual/`; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-time`):

```
ids-phi-accrual/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-time, serde, thiserror
├── lib.rs                              # Crate root, public API: PhiDetector, PhiConfig, SuspicionLevel
├── constants.rs                        # Defaults: window 1000, threshold 8, pause 3s, min_stddev, bootstrap
├── error.rs                            # PhiError (thiserror): window, config, clock errors
├── config.rs                           # PhiConfig: threshold, max_samples, min_stddev, pause, first_estimate
│
├── detector/                           # Per-peer phi detector core
│   ├── mod.rs
│   ├── phi_detector.rs                 # PhiDetector: record_heartbeat, phi, is_available, warming_up
│   ├── suspicion_level.rs              # SuspicionLevel (f64 wrapper), phi bands (low/elevated/suspect)
│   ├── per_peer.rs                     # Per-peer state map: NodeId -> PeerDetectorState
│   ├── registry.rs                     # DetectorRegistry: create/lookup/remove per-peer detectors
│   └── types.rs                        # PeerDetectorState, IntervalSample
│
├── interval_window/                    # Bounded sliding window of inter-arrival intervals
│   ├── mod.rs
│   ├── ring_window.rs                  # Fixed ring buffer, O(1) mean via running sum
│   ├── bounded_stats.rs                # BoundedStatsDeque: sum, count, mean, variance
│   ├── welford.rs                      # Welford online mean/variance
│   ├── recompute.rs                    # Recompute-from-window statistics
│   └── types.rs                        # WindowConfig, Sample
│
├── distribution/                       # Distribution fitting and CDF
│   ├── mod.rs
│   ├── normal_dist.rs                  # Normal distribution N(mean, stddev), CDF
│   ├── logistic_cdf.rs                 # β Math Handbook logistic approximation (error 0.00014)
│   ├── erf_cdf.rs                      # erf-based normal CDF (alternative)
│   └── types.rs                        # DistributionParams
│
├── phi_calc/                           # Phi value computation
│   ├── mod.rs
│   ├── phi.rs                          # phi(elapsed, mean, stddev, pause) -> f64
│   ├── tail_probability.rs             # P_later = 1 - CDF(elapsed), guard against p<=0 -> infinity
│   └── adjustment.rs                   # acceptable_heartbeat_pause adjustment to mean
│
├── clock/                              # Time abstraction
│   ├── mod.rs
│   ├── ids_time_clock.rs               # Clock backed by ids-time monotonic ticks
│   ├── fake_clock.rs                   # Fake/injectable clock for deterministic tests
│   └── trait.rs                        # Clock trait: timestamp(), elapsed()
│
├── thresholds/                         # Per-peer threshold configuration
│   ├── mod.rs
│   ├── threshold_config.rs             # Per-peer phi_threshold, recover_hysteresis
│   ├── bands.rs                        # Suspicion bands: healthy/elevated/suspect/dead
│   └── hysteresis.rs                   # Suspect/recover hysteresis to prevent flapping
│
├── crypto/                             # Allowed-crypto surface (type-level + IDs only)
│   ├── mod.rs
│   ├── checkpoint_signer.rs            # ed25519 signing of phi checkpoint entries
│   ├── suspicion_digest.rs             # sha256/blake3 hashing of suspicion state
│   ├── state_encryption.rs             # xchacha20poly1305/chacha20poly1305 encryption at rest
│   └── allowlist.rs                    # Compile-time assert: only the five algorithms
│
├── audit/                              # Signed, hash-chained suspicion log
│   ├── mod.rs
│   ├── checkpoint.rs                   # PhiCheckpoint: signed φ value + peer + tick
│   ├── audit_chain.rs                  # sha256/blake3 hash chain of checkpoints
│   ├── verifier.rs                     # Offline verification of audit chain
│   └── types.rs                        # CheckpointEntry
│
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── phi_detector_contract.rs        # trait PhiDetector: record/phi/is_available
│   ├── clock_contract.rs               # trait Clock (sealed)
│   ├── window_contract.rs              # trait IntervalWindow
│   ├── threshold_contract.rs           # trait ThresholdPolicy
│   ├── crypto_contract.rs              # trait SuspicionSigner
│   ├── audit_contract.rs               # trait AuditChain
│   └── registry_contract.rs            # trait DetectorRegistry
│
├── invariants/                         # Runtime/compile-time invariants (~6 files)
│   ├── mod.rs
│   ├── phi_monotonic_during_silence.rs # phi non-decreasing while no heartbeat
│   ├── phi_resets_after_heartbeat.rs   # phi -> 0 after heartbeat arrival
│   ├── window_size_bounded.rs          # window never exceeds max_samples
│   ├── min_stddev_floor.rs             # stddev >= min_std_deviation
│   ├── carrier_agnostic.rs             # no std::net / tokio::net references
│   └── crypto_allowlist.rs             # only allowed crypto algorithms
│
├── tests/                              # Property/verification tests (~22 files)
│   ├── mod.rs
│   ├── phi_calculation_tests.rs
│   ├── window_tests.rs
│   ├── threshold_tests.rs
│   ├── determinism_tests.rs            # fake clock, same seed => same phi
│   ├── gc_pause_tests.rs               # acceptable_pause suppresses suspicion
│   ├── cold_start_tests.rs             # warming-up until min_samples
│   ├── crypto_allowlist_tests.rs
│   ├── carrier_agnostic_tests.rs       # no networking crate in tree
│   ├── hysteresis_tests.rs             # suspect/recover no flapping
│   ├── audit_chain_tests.rs
│   └── property_tests.rs               # proptest properties
│
├── benches/                            # (~6 files)
│   ├── phi_calc_bench.rs
│   ├── window_update_bench.rs
│   └── threshold_crossing_bench.rs
│
├── spec/                               # Normative specifications (~8 files)
│   ├── phi_formula_spec.md             # φ = -log10(1-CDF), logistic approximation
│   ├── window_spec.md                  # Sliding window semantics, eviction, bootstrap
│   ├── threshold_spec.md               # Per-consumer thresholds, hysteresis
│   ├── pause_margin_spec.md            # acceptable_heartbeat_pause semantics
│   ├── determinism_spec.md             # Fake clock testing contract
│   ├── carrier_agnostic_spec.md        # Carrier boundary contract
│   ├── crypto_policy_spec.md           # Allowed-crypto allowlist
│   └── testing_spec.md                 # Determinism and property-test contract
│
├── docs/                               # Documentation (~10 files)
│   ├── architecture.md
│   ├── tuning_guide.md                 # Threshold/window/pause tuning per environment
│   ├── adding_a_peer.md
│   ├── integrating_with_membership.md
│   ├── integrating_with_lease.md
│   ├── false_positive_guide.md
│   ├── gc_pause_guide.md
│   ├── cold_start_guide.md
│   ├── crypto_policy.md
│   └── api_docs.md
│
└── test_vectors/                       # Deterministic inputs and expected outputs (~4 files)
    ├── heartbeat_vectors.json          # Fixed heartbeat sequences, expected phi
    ├── threshold_vectors.json          # Expected crossing points
    ├── window_vectors.json             # Expected window eviction behavior
    └── crypto_vectors.json             # Expected signed checkpoint outputs
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `spec/`, `docs/`, `test_vectors/`. Each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. No stubs; every file holds production-grade design with error handling, documentation, and tests (to be implemented in the implementation phase).

---

### 7. Key Algorithms & Data Structures

1. **Phi computation** — `φ(elapsed) = -log10(1 - F(elapsed))`; `F` = normal CDF `N(mean, stddev)` estimated from the interval window; logistic approximation `1/(1+exp(-y*(1.5976+0.070566*y*y)))`, `y=(elapsed-mean)/stddev`, error 0.00014 at ±3.16. Equivalent to `-log10(1-CDF(y))`. Guard: if tail probability `p <= 0`, return `f64::INFINITY` (saturated suspicion).

2. **Sliding interval window** — Fixed ring buffer of `max_samples` (default 1000) inter-arrival intervals. Running sum for O(1) mean; variance from sum of squares or Welford. On eviction, subtract outgoing sample from running stats. Window updated only on heartbeat arrival; never aged during silence.

3. **Welford online statistics** — `state = (n, mean, M2)`; `push(x): n+=1; delta=x-mean; mean+=delta/n; M2 += delta*(x-mean)`; `variance = M2/n`. Mergeable partitions for parallel stats.

4. **Acceptable heartbeat pause adjustment** — `effective_mean = mean + acceptable_pause`; `adjusted_elapsed = max(0, elapsed - pause_margin)`. Shifts the suspicion curve right so GC pauses don't trigger φ.

5. **Min stddev floor** — `stddev = max(computed_stddev, min_std_deviation)`. Prevents σ→0 → infinite φ on perfectly steady senders.

6. **Cold start / warming-up** — Until `min_samples` intervals collected: bootstrap with `first_heartbeat_estimate` mean and high stddev (25% of estimate), or report `WarmingUp{samples, required, silence}`. No φ guess before min_samples.

7. **Per-peer detector state** — `PeerDetectorState { window, last_tick: Option<Tick>, mean, stddev, phi_threshold }`. One `PhiDetector` per monitored peer (NodeId keyed), all independent.

8. **Threshold bands / hysteresis** — `Suspect` when `phi > suspect_threshold`; `Recover` when `phi < recover_threshold` (recover < suspect to prevent flapping). Bands: Healthy (φ<3), Elevated (3–8), Suspect (>8), Dead (configurable high).

9. **Suspicion audit chain** — Each φ checkpoint: `CheckpointEntry { peer: NodeId, tick: IdsTime, phi: f64, prev_hash: [u8;32] }`; chain via sha256/blake3; periodic ed25519-signed checkpoints; offline verifiable.

10. **Clock abstraction** — `Clock { timestamp() -> Tick; elapsed(before, after) -> Duration }`; implementations: `IdsTimeClock` (monotonic ids-time ticks) and `FakeClock` (deterministic tests).

---

### 8. Dependencies

`ids-phi-accrual/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `IdsTime`, `CarrierType`, `Endpoint`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` type surface only (heartbeats arrive over it; crate never sends).
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-time` — monotonic `IdsTime` ticks for all interval measurements.
- `serde` + `serde_derive` — serialization of checkpoint entries, config, suspicion state.
- `thiserror` — error types.
- `proptest` (dev) — property-based tests.
- `criterion` (dev) — phi calculation benchmarks.

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, `dns`, or any networking crate. No async runtime. No crypto beyond the allowed list; all crypto via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-76.1 | `PhiDetector::record_heartbeat(peer, tick)` appends interval to peer's bounded window | Window size ≤ max_samples (default 1000); O(1) append |
| AC-76.2 | `phi(peer, tick) = -log10(1 - F(elapsed))` with normal-CDF logistic approximation | Matches Akka/Pekko reference within logistic approximation error (0.00014 at ±3.16) |
| AC-76.3 | φ = 0 when no heartbeats received for a peer | Warming-up / no-history state returns 0.0 |
| AC-76.4 | φ grows monotonically during silence; resets toward 0 after a heartbeat arrives | Determinism test with fake clock |
| AC-76.5 | `acceptable_heartbeat_pause` suppresses suspicion during GC-scale pauses | Pause of ≤ pause_margin does not cross threshold |
| AC-76.6 | `min_std_deviation` floor prevents infinite φ on steady sender | Steady sender + silence → finite φ, not infinity |
| AC-76.7 | Cold start: bootstrap with `first_heartbeat_estimate`; warming-up until min_samples | No false suspicion before min_samples intervals |
| AC-76.8 | Per-peer independent φ: each peer has own window, mean, stddev | Peer A silence doesn't affect Peer B's φ |
| AC-76.9 | Threshold bands + hysteresis: suspect at φ_threshold, recover at lower threshold | No flapping near boundary |
| AC-76.10 | All traffic via `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test + source grep: zero matches |
| AC-76.11 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-76.12 | Audit chain: sha256/blake3 hash chain + ed25519-signed checkpoints, offline verifiable | Verifier test passes |
| AC-76.13 | Determinism: same fake-clock heartbeat sequence → identical φ sequence | Property test passes |
| AC-76.14 | Crate compiles cleanly: `cargo check -p ids-phi-accrual` exits 0 with no warnings | |
| AC-76.15 | Tests pass: `cargo test -p ids-phi-accrual` all green | |

---

### 10. Remaining Uncertainties

1. **Exact default `max_samples`:** 1000 (Cassandra/Akka) vs 200 (Hazelcast) — affects adaptation speed vs memory; to be validated at integration with `ids-membership`.
2. **`min_samples` for warming-up:** How many intervals before φ is trusted; Akka returns 0.0 with no history; suspect_machines requires min_samples. Undecided.
3. **Distribution choice:** Normal (paper/Akka) vs exponential (Cassandra `PHI_FACTOR * t/mean`) vs lognormal (some productions). Normal with logistic approximation selected; alternatives noted.
4. **`acceptable_heartbeat_pause` default:** Akka 3 s; deployment-dependent (GC pause profile). Tuning guide required.
5. **Per-peer threshold defaults:** Lease path φ>3, membership eviction φ>8, alerting φ>10–12 — deployment-tuned, validated at integration.
6. **Window eviction during long silence:** Never age out (discipline confirmed) vs bounded max interval clamp (Cassandra clamps to rpc_timeout). Both documented; decision pending integration.
7. **`ids-time` tick resolution:** Must be sufficient for sub-millisecond heartbeat cadences; clock source to be confirmed.
8. **Persistence of detector state across restarts:** Cold start after restart loses history; state persistence (encrypted at rest) out of scope for Phase 076, noted as future work.
9. **Correlated multi-node failure handling:** φ is per-peer independent; correlated failures (whole rack down) produce simultaneous φ rises — membership layer interprets, not this crate.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-phi-accrual` crate, not a folder | "Separate crates instead of more folders" directive; clear statistical-estimation boundary |
| φ = -log10(1-CDF) with normal distribution + logistic approximation | Hayashibara 2004 paper; Akka/Pekko reference implementation; error 0.00014 at ±3.16 |
| Bounded sliding window (default 1000 intervals) per peer | Matches Cassandra/Akka; O(1) mean via running sum; adapts to regime changes |
| Window updated only on heartbeat arrival; never aged during silence | Aging during silence inflates variance and suppresses φ when most needed (observed bug) |
| `acceptable_heartbeat_pause` margin + `min_std_deviation` floor | Survives GC pauses; prevents infinite φ on steady senders (Akka operational knobs) |
| `first_heartbeat_estimate` bootstrap + warming-up state | Conservative cold start; no false suspicion before min_samples |
| Per-peer independent `PhiDetector` | Each peer's heartbeat history differs; φ crosses threshold at different times (Cassandra observation) |
| Detector never decides; publishes φ only; consumer applies threshold | Decouples monitoring from interpretation; lease path φ>3 vs membership φ>8 on same stream |
| Monotonic `ids-time` ticks; injectable `Clock` for tests | Deterministic testing; clock manipulation cannot break suspicion |
| All traffic via `ids-carrier::Endpoint`; crate never sends bytes | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only: ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto | IDS crypto policy; type-level surface + primitive IDs only |
| Audit chain: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable, tamper-evident suspicion log |

---

### 12. Sources

1. Hayashibara, Défago, Yared, Katayama — "The φ Accrual Failure Detector," SRDS 2004 (IEEE); JAIST IS_RR_2004_010 / HDY04.pdf — accrual failure detector, continuous suspicion, sliding window, φ = -log10(1-CDF), window-size experiments.
2. Akka `PhiAccrualFailureDetector.scala` — https://github.com/akka/akka/blob/master/akka-remote/src/main/scala/akka/remote/PhiAccrualFailureDetector.scala — φ calculation, logistic approximation (β Math Handbook), acceptableHeartbeatPause, firstHeartbeatEstimate, Clock abstraction.
3. Apache Pekko docs — https://pekko.apache.org/docs/pekko/1.3/typed/failure-detector.html — φ = -log10(1-F), default threshold 8, cloud EC2 = 12, sweet spot 8 ≤ φ < 12.
4. Apache Cassandra `FailureDetector.java` + `ArrivalWindow` — https://github.com/apache/cassandra — bounded circular buffer 1000 intervals per endpoint, PHI_FACTOR=1/ln(10), MAX_INTERVAL clamped to rpc_timeout, seed 2× gossip interval.
5. Hazelcast docs — https://docs.hazelcast.com/hazelcast/5.7/clusters/phi-accrual-detector — default threshold 10, sample size 200, min std dev millis, φ=1≈10%/φ=2≈1%/φ=3≈0.1% mistake likelihood.
6. `atomr-remote` Rust source — https://docs.rs/atomr-remote/latest/src/atomr_remote/phi_accrual.rs.html — Rust PhiAccrualFailureDetector with threshold, max_samples, min_std_deviation, acceptable_heartbeat_pause, first_heartbeat_estimate.
7. `phi-accrual-failure-detector` Rust crate — https://docs.rs/phi-accrual-failure-detector — pluggable detector, Clock trait, ping/suspicion API.
8. `phi_detector` Rust crate — https://docs.rs/phi_detector — constant-memory two-value estimator, PingWindow/NormalDist.
9. `phi_accrual_detector` Rust crate — https://docs.rs/phi-accrual-detector — async MonitorInteraction trait, last_arrived_at, phi(current_time).
10. `suspect_machines` (Ruby) — https://github.com/seuros/suspect_machines — warming-up state, monotonic nanosecond clock, detector never declares dead.
11. systemoverflow — "SWIM and Phi Accrual: Two Approaches to Failure Detection Over Gossip" — continuous suspicion vs binary suspect/dead, indirect probing, detection times 8–12 s LAN / 15–25 s WAN-GC.
12. dipach.github.io — "gossip to tcm - part 1: phi accrual failure detection in Cassandra" — bounded window, window poisoning, seeding, two-tier alerting φ=8/φ=10-12.
13. padho.wiki — "Phi accrual failure detector" — φ bands, window-size tradeoffs (W=50/500/5000), σ floor, min_stddev, exponential vs normal variant.
14. manuel.bernhardt.io — "A new adaptive accrual failure detector for Akka" — Satzger et al. adaptive detector, no normal-distribution assumption, threshold semantics.
15. sir-teo.github.io — "The Heartbeat Is a Suspicion Score" — deterministic lab, fake clock, φ resets after heartbeat, threshold crossing measurements.
16. learn.padho.ai — "Phi accrual failure detector wiki" — decoupling detection latency from accuracy, per-consumer thresholds.
17. ed25519-dalek docs.rs — https://docs.rs/ed25519-dalek — RFC 8032 Ed25519 over Curve25519.
18. chacha20poly1305 docs.rs — https://docs.rs/chacha20poly1305 — RFC 8439 ChaCha20-Poly1305 AEAD + XChaCha20 192-bit nonce.
19. sha2 RustCrypto — https://docs.rs/sha2 — SHA-256 implementation (NIST FIPS 180-4).
20. BLAKE3-team BLAKE3 README — https://github.com/BLAKE3-team/BLAKE3 — BLAKE3-256, SIMD, keyed hash, KDF, XOF.
21. IDS prompts.txt entry 76 (verbatim) — C:/idfs/prompts.txt lines 1135-1149 — Phase 076 crate definition, contents, constraints.
22. Existing plans format — C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
23. `ids-carrier::Endpoint` = CarrierType + Vec<u8> (max 1024 bytes) — C:/idfs/ids-000-aa/ skeleton types.rs, contracts/carrier.rs.
24. `ids-000-aa` master crate — C:/idfs/ids-000-aa/ — canonical types, sealed trait contracts, invariants, errors, versioning.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 076 (ids-phi-accrual).
