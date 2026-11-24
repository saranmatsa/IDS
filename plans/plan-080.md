## Phase 080 — Create the ids-backpressure crate: Flow control under load. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1195-1209)

```
80. Research only do not read plan.md **[Phase 080 — Create the ids-backpressure crate: Flow control under load.]**

Crate: ids-backpressure
Purpose: Flow control under load.
Contents: Credit-based flow control, blocked signal emission, credit reservation, credit return, queue management, load shedding, adaptive rate limiting, congestion detection.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 080 — Create the ids-backpressure crate: Flow control under load.]**

Create plan-080.md in C:/idfs/plans/ for the ids-backpressure crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 080 creates `ids-backpressure`, the IDS crate for flow control under load. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by flow-control mechanism. `ids-backpressure` provides: credit-based flow control (per-endpoint credit windows, credit reservation, credit return), blocked signal emission (backpressure signals propagate upstream through opaque datagrams), queue management (bounded queues with configurable overflow policy), load shedding (drop/priority shed when overloaded), adaptive rate limiting (PID + gradient-based rate adjustment), and congestion detection (queue-delay, loss, ECN-style signals). All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address ≤ 1024 bytes); `ids-backpressure` never opens a socket, never imports a networking crate, and never touches `std::net`/`tokio::net`/libp2p/TCP/UDP/QUIC/HTTP/DNS or any carrier by name. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` only; `ids-backpressure` implements no crypto itself.

`ids-backpressure` is a policy/admission layer above `ids-000-aa` (types, sealed contracts, invariants, errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It does not send bytes itself; it returns credit grants, backpressure signals, admit/drop/shed decisions, and rate limits that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

**Core design:** a per-endpoint credit window (`Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`, credit-grant/credit-return/credit-reserve protocol) plus an adaptive rate limiter (PID/gradient on queue delay + RTT) plus a load shedder (criticality-ranked drop when queue depth crosses watermarks). Congestion is detected from queue sojourn time (CoDel-style, target ~5 ms), loss-rate, and ECN-style marks carried inside opaque datagram envelopes. Backpressure signals ("blocked" emission) travel as opaque datagrams on `ids-carrier::Endpoint`, never on any network stack.

**Scope boundary:** `ids-backpressure` admits, credits, sheds, and rates; it does not implement transport reliability, retransmission, or ordering (those live in `ids-transport`/`ids-carrier`). It never trusts the carrier: credit decisions and audit entries are signed over bytes, surviving any carrier re-framing. Skeleton `ids-backpressure` does NOT yet exist at `C:/idfs/ids-backpressure/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Reactive Streams (demand signaling `request(n)`, non-blocking backpressure, publisher bounded by demand), DLEP RFC 9893 (credit-window flow control, per-flow credit windows, Credit Window Grant/Status/Request data items), Kung & Morris IEEE Network 1995 / ATM credit-based flow control (`Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`, credit cell per forwarded cell, receiver-oriented adaptive buffer allocation), Flink credit-based flow control (1 buffer = 1 credit, channel credits, backlog reporting), Google SRE load shedding (observe local load, drop fraction of traffic, stack-rank by criticality), Netflix concurrency-limits `Gradient2Limit` (adaptive concurrency from RTT gradient vs EWMA base RTT), Uber Cinnamon (PID rate limiter, dynamic priority threshold, load shedding), BBR (bandwidth-delay product, min RTT, PROBE_BW/PROBE_RTT), CoDel RFC 8289 (queue sojourn time as congestion signal, TARGET 5 ms, control law), RED (random early detection, average queue size thresholds), ECN (explicit congestion notification marking), HTTP/2 RFC 9113 (WINDOW_UPDATE credit-based flow control, 65,535 octet initial window), SCTP/WebRTC advertised receiver window credit, tokio mpsc bounded channels with `reserve`/`Permit` (credit-like admission), actor bounded mailboxes (drop-new/drop-head/reject), Little's law (L = λW), token bucket / leaky bucket.

---

### 2. Architecture Overview

`ids-backpressure` sits above `ids-000-aa`, `ids-carrier`, and `ids-crypto`. It is an admission/flow-control layer: given an inbound datagram destined for an `ids-carrier::Endpoint`, it decides (a) whether the sender holds sufficient credit, (b) whether to admit, delay, or shed the datagram, (c) what rate limit applies, and (d) what backpressure/blocked signal to emit upstream. It never sends bytes itself; it returns `CreditDecision`, `AdmitDecision`, `ShedDecision`, `RateLimit`, and `BackpressureSignal` that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (flow-control users)               │
│  ids-store, ids-replication, ids-fragment — send datagrams with credit tag     │
├─────────────────────────────────────────────────────────────────────────────────┤
│               Phase 080: ids-backpressure (THIS CRATE — policy/admission)      │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌──────────┐ │ │\\n│  │ credit window    │ │ credit           │ │ queue          │ │ load       │ │ │\\n│  │ (per endpoint)   │ │ reservation/     │ │ management     │ │ shedding     │ │ │\\n│  │ Crd_Bal formula  │ │ credit return    │ │ bounded queues   │ │ (criticality │ │ │\\n│  │ grant/return/reserve│ │                  │ │ overflow policy  │ │ ranked drop) │ │ │\\n└──────────────────┘ └──────────────────┘ └──────────────────┘ └──────────┘ │ │\\n│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │                │ │ │\\n│  │ adaptive rate    │ │ congestion       │ │ blocked signal   │ │                │ │ │\\n│  │ limiter (PID/    │ │ detection (delay │ │ emission (opaque │ │                │ │ │\\n│  │  gradient/token bucket)│ │  sojourn/loss/ECN)│ │  datagram)      │ │                │ │ │\\n│  └──────────────────┘ └──────────────────┘ └──────────────────┘ │                │ │\\n│  ┌──────────────────┐ ┌──────────────────┐ │                │                │ │ │\\n│  │ audit/logging    │ │ crypto surface   │ │                │                │ │ │\\n│  │ (signed+chained) │ │ (allowed set only)│ │                │                │ │ │\\n│  └──────────────────┘ └──────────────────┘ │                │                │ │\\n└─────────────────────────────────────────────┬──────────────────┴──────────────┘\\n                                              │ opaque Vec<u8> datagrams\\n┌─────────────────────────────────────────────▼──────────────────┐\\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only      │\\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload)│\\n│  receive_datagram → (Endpoint, Vec<u8>)                        │\\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC /   │\\n│  HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN / mesh        │\\n└────────────────────────────────────────────────────────────────┘
```

**Data flow of a credit-gated send:** caller submits `Send{endpoint, payload, priority, credit_cost}` → `ids-backpressure` checks the endpoint's credit window: if `Crd_Bal ≥ credit_cost`, reserve credit, decrement `Crd_Bal`, return `Admit{reserved_credit, rate_limit, deadline}` → caller sends the datagram as opaque bytes to `ids-carrier::Endpoint` → on receiver processing/forwarding, receiver emits `CreditReturn{fwd_cnt}` as opaque datagram → sender updates `Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`. If `Crd_Bal < credit_cost`, return `Backpressure{blocked_until, signal}` → caller emits a blocked/backpressure signal upstream as opaque datagram and waits.

**Data flow of load shedding:** when queue depth crosses high watermark, `load_shedder` ranks incoming datagrams by criticality and drops/sheds lower-priority traffic; emits `ShedDecision{dropped, reason, backpressure_signal}`; when depth falls below low watermark, resume admission (XON/XOFF-style hysteresis).

**Congestion detection:** per-endpoint queue sojourn time, loss rate, and ECN-style marks inside envelopes feed `congestion_detector`; when sojourn > TARGET for interval, enter drop state and increase drop probability (CoDel control law `t + INTERVAL / sqrt(count)`); when below target, cancel drops.

**Crypto used:** ed25519 for signing credit/backpressure/audit decisions; sha256/blake3 for credit-window IDs, envelope hashing, audit-chain links; xchacha20poly1305/chacha20poly1305 to encrypt credit state / backpressure state at rest when confidentiality required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Credit-based flow control (credit window, grant/return/reserve)

**Source:** DLEP RFC 9893 (credit-window flow control, per-flow credit windows, Credit Window Grant/Status/Request/Initialization/Association data items); Kung & Morris IEEE Network 1995 (ATM credit-based flow control, `Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`, credit cell per forwarded cell, receiver-oriented adaptive buffer allocation); Flink credit-based flow control (1 buffer = 1 credit, channel credits, backlog reporting, backpressure on a logical channel only).

Credit-based flow control: before forwarding any datagram, the sender must hold credits for the endpoint's credit window. The receiver sends credits indicating available buffer space. Each sent datagram decrements `Crd_Bal` by its cost; each processed/forwarded datagram returns a credit. `Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cint)` guarantees the receiver buffer never overflows: overflow is impossible even under abrupt stop/start because buffer ≥ Rate × RTT (one BDP). Lost/delayed feedback does not hurt — the sender uses the previous allowance; transmission can be stopped completely (rate → 0). Credit windows may be shared or dedicated per endpoint/destination. `ids-backpressure` implements `CreditWindow{Buf_Alloc, Tx_Cnt, Fwd_Cnt, Crd_Bal, MaxSize}` per endpoint, with `credit_reserve(cost)`, `credit_return(fwd_cnt)`, `credit_grant(increment)` operations. Credit values are octet counts (include framing overhead, as in DLEP where credit value includes MAC headers).

#### 3.2 Blocked signal emission (backpressure signal)

**Source:** Reactive Streams spec (demand signaling `request(n)`, non-blocking backpressure, publisher never signals more than demand); Actor bounded mailboxes (bounded mailbox caps consumer memory; backpressure expressed through the messaging protocol, e.g. `bufferedAmountLow` threshold, dead-letter on reject).

When `Crd_Bal` hits 0 or queue depth crosses the high watermark, `ids-backpressure` emits a `BackpressureSignal{endpoint, reason, blocked_until, credit_needed}` as an opaque `Vec<u8>` datagram to `ids-carrier::Endpoint`. The upstream sender slows/stops emission until credits return or the signal clears. Blocked emission is non-blocking: the producer hands the signal to the carrier boundary and is not stalled on a thread. Backpressure propagates hop-by-hop through opaque datagrams, never through `std::net`/`tokio::net`.

#### 3.3 Queue management (bounded queues, overflow policy)

**Source:** Actor bounded mailbox patterns (drop-new/drop-head/reject, capacity sizing by worst-case burst and latency budget); Datadog Observability Pipelines (buffer on-full behavior: block vs drop-newest); Flink network stack (exclusive vs floating buffers, channel credits, backlog).

`ids-backpressure` manages bounded per-endpoint queues. Overflow policy is configurable: `DropNew` (discard incoming, preserve queued — critical work), `DropHead` (discard oldest, newest wins — telemetry), `Reject` (refuse and dead-letter — refusal on record), `Block` (propagate backpressure). Queue capacity is sized by worst-case burst size, per-message memory (`capacity × bytes`), and latency budget (`capacity / drain_rate`). Queue depth feeds the load shedder and congestion detector.

#### 3.4 Load shedding (criticality-ranked drop under overload)

**Source:** Google SRE load shedding (observe local load like CPU/memory/queue length; when above safe level, drop a fraction of incoming traffic; stack-rank requests by criticality, shed least-critical first); Netflix concurrency-limits (adaptive concurrency limit sheds excess load); RAJOMON (market-based overload control, tokens + prices).

`ids-backpressure` implements a load shedder: observe local load (queue depth, CPU, memory), and when it crosses a safe threshold, drop a fraction of incoming datagrams. Criticality is established per traffic class (control-plane vs data-plane, high-priority vs bulk). Shedding starts at `drop_fraction = 1/10` and ramps up as load stays high, ramps down as load returns to safe. Shedded datagrams are recorded in the audit log; `ShedDecision{dropped, reason, criticality, backpressure_signal}` is emitted.

#### 3.5 Adaptive rate limiting (PID / gradient / token bucket / leaky bucket)

**Source:** Uber Cinnamon (PID rate limiter, dynamic priority threshold, self-correction); Netflix `Gradient2Limit` (adaptive concurrency from RTT gradient vs EWMA base RTT); APNIC rate vs credit-based flow control (absolute vs differential rate adjustment, XON/XOFF); token bucket / leaky bucket (GeeksforGeys, Wikipedia, Cloudflare); Uber-go ratelimit (leaky bucket).

`ids-backpressure` implements adaptive rate limiting: a PID controller adjusts the sending rate based on error = target_latency − measured_latency (queue sojourn); a gradient-based limiter (Netflix style) adjusts concurrency from RTT gradient vs smoothed base RTT; token bucket (`rate` + `burst`) permits bursty traffic with a long-term average cap; leaky bucket smooths to a constant rate. Rate is adjusted dynamically (differential: speed-up/slow-down by Δ%) rather than absolute replacement, to avoid overreaction. `RateLimit{rate, burst, priority_threshold, adjustment_interval}` per endpoint.

#### 3.6 Congestion detection (queue delay, loss, ECN)

**Source:** CoDel RFC 8289 (queue sojourn time as congestion signal, TARGET 5 ms, control law `t + INTERVAL / sqrt(count)`, parameterless AQM); RED (random early detection, average queue size min/max thresholds, mark/drop probability); ECN (explicit congestion notification, CE marking instead of dropping); BBR (bandwidth-delay product, min RTT, BDP-based inflight bound); RFC 8085 UDP usage guidelines (UDP applications must employ congestion control to prevent collapse).

`ids-backpressure` detects congestion per endpoint from: (1) queue sojourn time (dequeue − enqueue) — CoDel-style, if min sojourn > TARGET for INTERVAL, enter drop state; (2) loss rate — lost datagrams / sent datagrams; (3) ECN-style marks inside opaque datagram envelopes (carrier-agnostic: marks are envelope fields, not IP ECN). Congestion state drives the rate limiter (reduce rate) and load shedder (increase drop fraction). BDP = bandwidth × min_RTT bounds in-flight data; `ids-backpressure` tracks BDP per endpoint to size credit windows.

#### 3.7 Credit reservation and return

Credit reservation: before constructing a datagram, the sender calls `credit_reserve(endpoint, cost)`; if `Crd_Bal ≥ cost`, credit is reserved (`Crd_Bal −= cost`, reservation recorded with timeout). If the datagram is never sent, the reservation is released (`credit_return` of the reserved amount). Credit return: when the receiver processes/forwards a datagram, it sends `CreditReturn{fwd_cnt, endpoint}`; sender updates `Fwd_Cnt` and recomputes `Crd_Bal`. Credit grant: receiver proactively grants additional credits (`CreditWindowGrant{additional_credits, endpoint}`) when buffer availability exceeds the window. All credit messages are opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

#### 3.8 Determinism and reproducibility

Every flow-control decision is recorded with `ids-time` ticks (monotonic, never wall-clock for ordering) and `NodeId`. Runs are deterministic given the same seed, payload pattern, and congestion scenario. Run manifest records git revision, dirty state, toolchain, host, config, and results (schema-versioned JSON). No wall-clock or RTT-derived timeouts for correctness; `ids-time` only for timeouts and measurement.

#### 3.9 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek (RFC 8032 EdDSA); chacha20poly1305 docs (RFC 8439 AEAD + XChaCha20Poly1305 192-bit nonce); sha2 RustCrypto (SHA-256); BLAKE3-team README (BLAKE3-256, SIMD, keyed hash, KDF, XOF).

`ids-backpressure` uses: ed25519 for signing credit/backpressure/audit decisions; sha256 for credit-window IDs and audit-chain links; blake3 for fast envelope/content hashing and credit-state checkpoint chaining; xchacha20poly1305 to encrypt credit/backpressure state at rest (24-byte nonce, random nonces safe); chacha20poly1305 for session/state payloads (12-byte nonce, counter-based). No other crypto. `ids-crypto` exposes primitive IDs; `ids-backpressure` references them via `ids-000-aa` only.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Buffer overflow / receiver memory exhaustion | Receiver memory | Credit window bounds in-flight data: `Crd_Bal ≥ 0` enforced; buffer ≥ Rate × RTT (one BDP); no datagram sent without credit |
| Lost/delayed credit feedback | Flow-control liveness | Sender uses previous allowance on lost/delayed feedback; bounded liability; credit windows re-initialized on re-sync |
| Credit theft / forgery | Decision integrity | ed25519-signed credit/backpressure decisions; signed audit checkpoints |
| Congestion collapse | Network stability | Congestion detection (sojourn/loss/ECN) reduces rate + sheds load; BDP bounds in-flight |
| Queue-induced tail latency | Latency SLO | CoDel-style drop on persistent sojourn > TARGET; bounded queues prevent unbounded buffering |
| Load-shedding unfairness | Request fairness | Criticality-ranked shedding; least-critical traffic shed first; audit records every shed |
| Rate oscillation / instability | Rate stability | Differential (not absolute) rate adjustment; PID damping; gradient vs smoothed base RTT |
| Carrier duplicating/delaying datagrams | Replay/ordering | Replay protection at flow-control layer; idempotent credit return tolerates duplicates |
| Prohibited crate in dependency tree | Carrier-agnostic rule | CI dependency audit; `cargo tree` asserts no std::net/tokio::net/libp2p/QUIC/HTTP/DNS crate |
| Disallowed crypto algorithm | Crypto policy | Only the five allowed primitives; `ids-crypto` is sole crypto surface; allowlist unit test |
| Clock manipulation for congestion | Congestion correctness | Logical/monotonic `ids-time` only; never wall-clock for flow-control decisions |
| Credit-window DoS (reservation exhaustion) | Availability | Reservations carry timeouts; unreserved after `reservation_ttl`; per-endpoint reservation caps |

The carrier is never trusted: credit and backpressure decisions are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-backpressure |
|-------|------|-------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; sealed contract for credit/backpressure surface |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All flow-control traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing credit/backpressure decisions; encrypting credit state at rest; hashing envelopes and credit windows |
| `ids-time` | Monotonic IDS standard time source | All flow-control timeouts and measurements keyed to `ids-time` ticks, never wall-clock |
| `ids-store` | Primary record store (flow-control user) | Tags outbound datagrams with credit cost; consults `ids-backpressure` before sending |
| `ids-replication` | Replica synchronization | Replication datagrams credit-gated; backpressure signals slow replication under load |
| `ids-fragment` | Record fragments/shards | Fragment datagrams governed by the parent record's flow-control policy |
| `ids-fault` | Fault detection / recovery | Fault events trigger load shedding and rate-limit reduction |
| `ids-observability` | Metric/trace sinks | Flow-control telemetry (credit balance, shed count, rate, congestion state) |

Integration points: `ids-backpressure` drives decisions through public contracts declared in `ids-000-aa`; it does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-backpressure/` (a separate crate, not a folder inside an existing crate):

```
ids-backpressure/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-time, thiserror, serde, blake3; dev: proptest, criterion
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # Flow-control defaults: BDP, TARGET, watermarks, reservation TTL
├── error.rs                            # BackpressureError (thiserror): credit, queue, shed, rate, congestion errors
├── features.rs                         # Feature flags: audit, crypto-at-rest, nightly
│
├── credit/                             # Credit-based flow control
│   ├── mod.rs
│   ├── window.rs                       # CreditWindow: Buf_Alloc, Tx_Cnt, Fwd_Cnt, Crd_Bal, MaxSize per endpoint
│   ├── reservation.rs                  # Credit reservation with TTL; release on timeout
│   ├── grant.rs                        # CreditWindowGrant/Status/Request (DLEP-style)
│   ├── return.rs                       # Credit return: Fwd_Cnt update, Crd_Bal recompute
│   ├── accounting.rs                   # Per-endpoint credit accounting (octets incl. framing overhead)
│   └── types.rs                        # CreditDecision, CreditCost structures
│
├── signal/                             # Blocked / backpressure signal emission
│   ├── mod.rs
│   ├── emitter.rs                      # Emits BackpressureSignal as opaque datagram to Endpoint
│   ├── backpressure.rs                 # BackpressureSignal {endpoint, reason, blocked_until, credit_needed}
│   ├── blocked.rs                      # Blocked state machine per endpoint
│   └── types.rs                        # Signal types
│
├── queue/                              # Queue management
│   ├── mod.rs
│   ├── bounded_queue.rs                # Bounded per-endpoint queue
│   ├── overflow.rs                     # Overflow policy: DropNew/DropHead/Reject/Block
│   ├── depth.rs                        # Queue depth tracking; high/low watermark
│   ├── sizing.rs                       # Capacity sizing: burst, bytes, latency budget
│   └── types.rs                        # QueueResult structures
│
├── shed/                               # Load shedding
│   ├── mod.rs
│   ├── shedder.rs                      # Criticality-ranked drop; drop fraction ramping
│   ├── criticality.rs                  # Traffic criticality ranking (control vs data vs bulk)
│   ├── fraction.rs                     # Drop fraction: ramps up/down with load
│   └── types.rs                        # ShedDecision structures
│
├── rate/                               # Adaptive rate limiting
│   ├── mod.rs
│   ├── limiter.rs                      # Rate limiter: PID + gradient + token bucket + leaky bucket
│   ├── pid.rs                          # PID controller: error = target_latency − measured_latency
│   ├── gradient.rs                     # Gradient-based concurrency from RTT gradient vs EWMA base RTT
│   ├── token_bucket.rs                 # Token bucket (rate + burst)
│   ├── leaky_bucket.rs                 # Leaky bucket (constant-rate smoothing)
│   └── types.rs                        # RateLimit structures
│
├── congestion/                         # Congestion detection
│   ├── mod.rs
│   ├── detector.rs                     # Congestion detector: sojourn/loss/ECN
│   ├── sojourn.rs                      # Queue sojourn time (CoDel-style, TARGET 5 ms)
│   ├── loss.rs                         # Loss-rate detection
│   ├── ecn.rs                          # ECN-style marks inside opaque envelopes
│   ├── bdp.rs                          # Bandwidth-delay product tracking per endpoint
│   └── types.rs                        # CongestionState structures
│
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── credit_contract.rs, signal_contract.rs, queue_contract.rs, shed_contract.rs,
│   │   rate_contract.rs, congestion_contract.rs, carrier_contract.rs
│
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── credit_nonnegative.rs, credit_bounded_by_bdp.rs, queue_depth_bounded.rs,
│   │   shed_criticality_ordered.rs, rate_monotonic_adjust.rs, carrier_agnostic.rs,
│   │   crypto_allowlist.rs, mod.rs
│
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs, mod.rs
│
├── tests/                              # Property/verification tests (~30 files)
├── benches/                            # (~8 files)
├── spec/                               # Normative specifications (~6 files)
│   ├── credit_spec.md, signal_spec.md, queue_spec.md, shed_spec.md, rate_spec.md, congestion_spec.md
├── docs/                               # Documentation (~8 files)
│   ├── architecture.md, runbook.md, adding_mechanism.md, crypto_policy.md
├── baselines/                          # Stored baseline results for regression comparison
└── test_vectors/                       # Deterministic inputs and expected outputs
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `spec/`, `docs/`, `baselines/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Credit window
```
CreditWindow { Buf_Alloc, Tx_Cnt, Fwd_Cnt, Crd_Bal, MaxSize }
Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)   (credit balance, never negative)
credit_reserve(endpoint, cost): if Crd_Bal ≥ cost { Crd_Bal −= cost; record reservation } else → Backpressure
credit_return(endpoint, fwd_cnt): Fwd_Cnt += fwd_cnt; Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)
credit_grant(endpoint, increment): Crd_Bal += increment (capped at MaxSize)
```
Guarantee: no datagram sent without credit ⇒ receiver buffer never overflows ⇒ zero loss due to buffer overflow (credit-based advantage over rate-based).

#### 7.2 Blocked / backpressure signal emission
```
BackpressureSignal { endpoint, reason, blocked_until, credit_needed }
emitted as opaque Vec<u8> datagram to ids-carrier::Endpoint when Crd_Bal = 0 or queue depth ≥ high watermark
non-blocking: producer hands signal to carrier boundary, not stalled on a thread
```

#### 7.3 Load shedding
```
observe load L (queue depth, CPU, memory)
if L > safe_threshold: drop_fraction = min(1, drop_fraction × ramp_up); start at 1/10
if L < safe_threshold: drop_fraction = max(floor, drop_fraction × ramp_down)
shed least-critical traffic first (criticality-ranked)
ShedDecision { dropped, reason, criticality, backpressure_signal }
```

#### 7.4 Adaptive rate limiting
```
PID: error = target_latency − measured_sessions; rate += Kp·error + Ki·∫error + Kd·d(error)
Gradient2Limit: concurrency adjusts from RTT gradient vs EWMA base RTT
Token bucket: tokens += rate·Δt (capped at burst); conform iff tokens ≥ cost
Leaky bucket: water += cost; leak at constant rate; conform iff water ≤ capacity
Differential adjustment (speed-up/slow-down by Δ%), not absolute replacement
```

#### 7.5 Congestion detection (CoDel-style)
```
cur_qdelay = dequeue_time − enqueue_time
track min sojourn over interval (~1 RTT)
if min_sojourn > TARGET (5 ms) for INTERVAL: enter drop state; next_drop = t + INTERVAL / sqrt(count)
if min_sojourn ≤ TARGET: cancel drops, reset interval
loss_rate = lost / sent; ECN marks from envelopes → congestion state
BDP = bandwidth × min_RTT; bounds in-flight data per endpoint
```

#### 7.6 Audit chain
Each flow-control decision (admit/reserve/return/shed/rate-change/congestion) = SHA-256/BLAKE3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Monotonic time source for all flow-control timeouts and measurements |
| `serde` | Serialization of credit envelopes, backpressure signals, audit entries |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-backpressure` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-80.1 | Credit window per endpoint: `Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`, never negative | CreditWindow unit tests; invariant `credit_nonnegative` |
| AC-80.2 | Credit reservation/return/grant protocol | reservation grants when `Crd_Bal ≥ cost`; return recomputes `Crd_Bal`; grant caps at `MaxSize` |
| AC-80.3 | Blocked signal emission when `Crd_Bal = 0` or queue ≥ high watermark | `BackpressureSignal` emitted as opaque datagram to `ids-carrier::Endpoint`; non-blocking |
| AC-80.4 | Bounded queue with configurable overflow policy (DropNew/DropHead/Reject/Block) | overflow policy unit tests; capacity sized by burst/bytes/latency |
| AC-80.5 | Load shedding: criticality-ranked drop, fraction ramps 1/10 → up/down with load | shedder tests; least-critical dropped first; audit records every shed |
| AC-80.6 | Adaptive rate limiting: PID + gradient + token bucket + leaky bucket | rate limiter converges to target latency; differential adjustment (no oscillation) |
| AC-80.7 | Congestion detection: queue sojourn (CoDel TARGET 5 ms), loss rate, ECN marks | detector enters/exits drop state; `t + INTERVAL / sqrt(count)` control law |
| AC-80.8 | BDP bounds in-flight data per endpoint | `BDP = bandwidth × min_RTT`; credit window sized ≥ one BDP |
| AC-80.9 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | dependency audit test (`cargo tree`) + source grep — zero matches |
| AC-80.10 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | allowlist unit test passes |
| AC-80.11 | Every flow-control decision in append-only, hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime` | audit chain verifiable offline |
| AC-80.12 | Determinism: same seed + scenario ⇒ identical decisions | determinism tests pass |
| AC-80.13 | No wall-clock for flow-control correctness; `ids-time` only | monotonic clock usage enforced |

---

### 10. Remaining Uncertainties

1. **Credit granularity:** credits in octets (incl. framing overhead, DLEP-style) vs per-datagram credits; unquantified for IDS payload sizes.
2. **Credit window defaults per endpoint:** `Buf_Alloc`, `MaxSize`, reservation TTL are deployment-tuned; validated at integration with `ids-store`/`ids-replication`.
3. **Load-shedding criticality taxonomy:** exact traffic classes and drop-fraction ramp curve are workload-dependent.
4. **PID tuning constants (Kp/Ki/Kd) and gradient EWMA alpha:** latency-vs-stability tradeoff; unquantified on target host.
5. **Congestion TARGET/INTERVAL:** CoDel TARGET 5 ms / INTERVAL ~1 RTT is Internet-default; IDS carrier latency characteristics may require retuning.
6. **Credit feedback on lossy/unreliable carriers:** lost credit datagrams must not stall the sender indefinitely; re-sync protocol details pending.
7. **Interaction with `ids-transport` (reliability/ordering):** credit flow control is orthogonal to transport reliability; exact handshake to be validated at integration.
8. **Audit signing overhead per decision:** ed25519 sign per credit decision may be heavy at high throughput; batching/checkpoint interval TBD.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-backpressure` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear flow-control boundary |
| Credit-based flow control (`Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`) | Receiver-oriented, guarantees zero buffer-overflow loss, bounded by one BDP (Kung & Morris; Flink; DLEP RFC 9893) |
| Blocked signal as opaque datagram to `ids-carrier::Endpoint` | Non-blocking backpressure; carrier-agnostic; no `std::net`/`tokio::net` |
| Bounded queues with DropNew/DropHead/Reject/Block | Bounded memory; overflow policy per traffic criticality (actor mailbox patterns) |
| Load shedding: criticality-ranked, drop fraction 1/10 ramping | Google SRE load shedding; shed least-critical first under overload |
| Adaptive rate limiting: PID + gradient + token bucket + leaky bucket | Uber Cinnamon / Netflix Gradient2Limit; dynamic rate from queue latency + RTT; differential adjustment avoids overreaction |
| Congestion detection: CoDel-style sojourn (TARGET 5 ms) + loss + ECN | RFC 8289; queue delay as congestion signal; parameterless |
| BDP bounds in-flight data | BBR; credit window ≥ one BDP keeps link loss-free at full use |
| Audit: SHA-256/BLAKE3 hash chain + ed25519-signed checkpoints | Offline-verifiable flow-control decision log; tamper evidence |
| Logical/monotonic `ids-time` only; no wall-clock for decisions | Clock manipulation cannot break flow control |
| All traffic via `ids-carrier::Endpoint`; crate returns decisions, never sends | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; via `ids-crypto` primitive IDs | Complies with allowed-crypto rule |

---

### 12. Sources

1. DLEP RFC 9893 — Credit-Based Flow Control Messages and Data Items (credit windows, grant/status/request). https://ietf.org/ietf-ftp/rfc/rfc9893.html
2. Kung & Morris, IEEE Network 1995 — Credit-based flow control for ATM (`Crd_Bal = Buf_Alloc − (Tx_Cnt − Fwd_Cnt)`, receiver-oriented adaptive buffer allocation). https://eecs.harvard.edu/~htk/publication/1995-ieee-network-kung-morris.pdf
3. APNIC Blog — Rate-based vs credit-based flow control (credit rule: buffer ≥ Rate × RTT = one BDP; XON/XOFF vs credit). https://blog.apnic.net/2025/10/28/rate-based-versus-credit-based-flow-control-part-2-notes-on-flow-control
4. MERL TR95-03 — Rate-based vs credit-based flow control duality (ABR traffic, credit cells per forwarded cell). https://merl.com/publications/docs/TR95-03.pdf
5. ScienceDirect — Flow control in ATM networks: a survey (credit-based vs rate-based, per-VC queueing). https://sciencedirect.com/science/article/pii/S0140366498001558
6. Flink — A Deep-Dive into Flink's Network Stack (1 buffer = 1 credit, channel credits, backlog, per-channel backpressure). https://flink.apache.org/2019/06/05/a-deep-dive-into-flinks-network-stack
7. Reactive Streams — Non-blocking backpressure spec (`request(n)` demand, publisher bounded by demand). https://github.com/reactive-streams/reactive-streams-jvm
8. Akka Streams — Back-pressure basics (demand signaling, bounded by demand, drop/tear-down strategies). https://doc.akka.io/libraries/akka-core/current/stream/stream-flows-and-basics.html
9. Google SRE — Using load shedding to survive a success disaster (drop fraction of traffic, criticality stack ranking). https://cloud.google.com/blog/products/gcp/using-load-shedding-to-survive-a-success-disaster-cre-life-lessons
10. Netflix concurrency-limits — `Gradient2Limit` (adaptive concurrency from RTT gradient vs EWMA base RTT). https://github.com/Netflix/concurrency-limits
11. Uber Cinnamon — PID adaptive rate limiting (dynamic priority threshold, load shedding, self-correction). https://github.com/jfrz38/rate-limit-pid-controller
12. Token bucket — Wikipedia / GeeksforGeys (rate + burst, conform, shaping vs policing). https://en.wikipedia.org/wiki/Token_bucket
13. Leaky bucket — docs.rs tokio-rate-limit / uber-go ratelimit (constant-rate smoothing). https://docs.rs/tokio-rate-limit/latest/src/tokio_rate_limit/algorithm/leaky_bucket.rs.html
14. tokio mpsc — bounded channels with `reserve`/`Permit` (credit-like admission). https://github.com/tokio-rs/tokio/blob/master/tokio/src/sync/mpsc/bounded.rs
15. Actor mailbox backpressure — tellus bounded mailbox (drop-new/drop-head/reject, capacity sizing). https://heikoseeberger.de/2026-10-22-bounded-mailbox
16. ProtoActor — Backpressure and flow control (bounded mailboxes, work pulling). https://proto.actor/docs/ProtoActor/backpressure
17. CoDel RFC 8289 — Controlled Delay AQM (sojourn time as congestion signal, TARGET 5 ms, control law). https://datatracker.ietf.org/doc/html/rfc8289
18. RED — Random Early Detection (average queue size thresholds, mark/drop probability). https://geeksforgeeks.org/computer-networks/random-early-detection-red-queue-discipline
19. ECN — Explicit Congestion Notification (CE marking instead of dropping). https://documentation.nokia.com/sr/26-10/7750-sr/books/qos/congestion-notification.html
20. BBR — Bottleneck Bandwidth and RTT congestion control (BDP, min RTT, PROBE_BW/PROBE_RTT). https://github.com/google/bbr
21. RFC 8085 — UDP Usage Guidelines (congestion control required for UDP apps). https://datatracker.ietf.org/doc/html/rfc8085
22. HTTP/2 RFC 9113 — Flow control via WINDOW_UPDATE (credit-based, 65,535 octet initial window). https://datatracker.ietf.org/doc/html/rfc9113
23. SCTP/WebRTC — Advertised receiver window credit (data channels, flow control). https://webrtcforthecurious.com/docs/07-data-communication
24. Little's Law — L = λW (queueing theory, occupancy = arrival rate × wait). https://blog.danslimmon.com/2022/06/07/using-littles-law-to-scale-applications
25. ed25519-dalek — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
26. chacha20poly1305 — ChaCha20Poly1305 / XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
27. sha2 RustCrypto — SHA-256. https://docs.rs/sha2
28. BLAKE3-team — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3
29. IDS prompts.txt entry 80 (verbatim). C:/idfs/prompts.txt — Phase 080 crate definition, contents, constraints.
30. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, Cargo.toml.

---

**End of plan-080.md. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made beyond this plan document. Ready for implementation phase.**
