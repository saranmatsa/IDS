## Phase 081 — Create the ids-graceful-degradation crate: Ordered fallback states between health and failure.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1210-1224)

```
81. Research only do not read plan.md **[Phase 081 — Create the ids-graceful-degradation crate: Ordered fallback states between health and failure.]**

Crate: ids-graceful-degradation
Purpose: Ordered fallback states between health and failure.
Contents: Degradation hierarchy states, fallback behavior, recovery path, capability reduction, availability tradeoff, operator notification of degraded state.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 081 — Create the ids-graceful-degradation crate: Ordered fallback states between health and failure.]**

Create plan-081.md in C:/idfs/plans/ for the ids-graceful-degradation crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 081 creates `ids-graceful-degradation`, a separate IDS crate (not a folder inside an existing crate, per the project rule of separate crates for new capabilities). It defines ordered fallback states between health and failure: a degradation hierarchy with named, ordered intermediate states, each trading capability for availability, plus fallback behavior per state, a recovery path back to health, capability-reduction bookkeeping, availability tradeoff documentation, and operator notification of degraded state.

The crate is a policy/decision layer: it observes health signals (from ids-health or equivalent), computes the current degradation state, decides which capabilities are reduced or disabled, and emits a `DegradationDecision` (state transition + capability mask + operator notice). It never sends bytes itself; every protection-relevant message rides opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). All traffic is carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed via `ids-crypto` primitive IDs from `ids-000-aa`/`ids-crypto`; never implemented in this crate). `ids-000-aa` is the master crate holding every canonical type (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpState`, `Endpoint`, `FailureDomain`, …) and sealed trait contracts; this crate imports them as `ids_000_aa::X`.

Research basis: 30 parallel searches returned the ordered-degradation-hierarchy model (named intermediate states between healthy and down, each trading capability for availability), circuit-breaker three-state machine (Closed/Open/Half-Open) from Nygard's Release It! and Azure/AWS guidance, fallback chains (ordered alternatives: fresh → cached → simplified → static default → error), load shedding (reject early, reject cheap, tiered by priority), retry storms / thundering herd and exponential backoff + jitter prevention, graceful-degradation mode from Azure self-preservation and Google Cloud reliability guidance, fail-safe defaults (Saltzer & Schroeder 1975; de-energize-to-safe), degraded mode entry/exit rules with hysteresis (Atlas of Engineering), operator notification patterns, anti-patterns (silent degradation; fallback hard to prove/test — AWS Builder Library), and degrade-from-the-edges-inward tiering.

A skeleton `ids-graceful-degradation` crate does NOT yet exist at `C:/idfs/ids-graceful-degradation/`. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-graceful-degradation` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a degradation-state machine with an ordered hierarchy of states and a fallback engine that maps each state to a capability mask. It observes health/failure signals, computes transitions, and emits `DegradationDecision`s that callers frame into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. It never sends or receives datagrams directly.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CALLER LAYER (ids-*, operators, health monitors)         │
│  ids-store, ids-replication, ids-session — report health, consume decisions │
├─────────────────────────────────────────────────────────────────────────────┤
│              Phase 081: ids-graceful-degradation (THIS CRATE — policy)     │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐│
│  │ degradation      │ │ state machine    │ │ fallback engine  │ │ recovery││
│  │ hierarchy        │ │ (Healthy→…→Failed│ │ (per-state fallback│ │ path    ││
│  │ (ordered states) │ │  ordered chain)  │ │   chain)         │ │ (hysteresis││
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘│
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐│
│  │ capability       │ │ operator         │ │ audit log        │ │ carrier ││
│  │ reduction mask   │ │ notification     │ │ (signed+chained) │ │ adapter ││
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘│
└──────────────────────────────────────────┬──────────────────────────────────┘
                                           │ opaque Vec<u8> datagrams
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload)      │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS /   │
│  Wi-Fi / Bluetooth / cellular / VPN / mesh                                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Ordered degradation hierarchy** (states between full health and complete failure, each trading capability for availability — research-verified pattern from Sujeet Jaiswal's graceful-degradation catalog and the HLD Handbook):

| Level | State | Behavior | Capability reduction | Availability tradeoff |
|-------|-------|----------|----------------------|----------------------|
| 0 | Healthy | Full functionality, real-time data, all writes accepted | None | None (baseline) |
| 1 | Degraded | Serve cached/stale data where fresh is unavailable; non-critical features on | Staleness | Higher availability under dependency slowness |
| 2 | Limited | Disable Tier-3 (non-critical) features; critical path preserved | Functionality | Stability preserved for core flows |
| 3 | Minimal | Read-only mode; reject writes; serve from local/cached replicas | Writes lost | Reads preserved during write-path failure |
| 4 | Static | Return default/static responses; no personalization or dynamic data | Personalization | Uptime preserved with no dynamic backend |
| 5 | Failed | Reject all operations; emit terminal failure to callers | Everything | Safe fail-safe default; operator intervention required |

**Data flow:** health signal arrives → state machine evaluates transition rules → if state changes, fallback engine computes capability mask for the new state → operator notification emitted → `DegradationDecision{from_state, to_state, capability_mask, reason, timestamp, operator_notice}` returned → caller frames it into opaque datagrams to `ids-carrier::Endpoint` targets. Recovery path: state only exits downward when hysteresis/confirmation conditions are met (persist ≥ T_confirm, no fault recurrence), preventing rapid chatter.

**Key research-verified patterns applied:**
- Ordered intermediate states with explicit recovery paths (Sujeet Jaiswal; HLD Handbook).
- Circuit-breaker semantics per dependency (Closed/Open/Half-Open) to isolate failing components (Nygard; Azure/AWS).
- Fallback chain per capability: fresh → cached → simplified → static default → error (sujeet.pro; Tetrate; nRouter).
- Load shedding by priority tier; reject non-critical first (Google SRE; HLD Handbook; Uber Cinnamon).
- Fail-safe defaults: on uncertainty, transition to a predefined safe state and signal operators (arc42; Saltzer & Schroeder).
- Degraded-mode entry/exit with hysteresis to prevent chatter (Atlas of Engineering degraded-mode entry/exit rules).
- Operator notification on every state transition (Azure self-preservation degraded UX; HLD Handbook banners).
- Anti-pattern awareness: silent degradation is dangerous; fallback code paths are rarely exercised and must be tested (AWS Builder Library "Avoiding fallback"; Google SRE "code path you never use").

---

### 3. Detailed Research Dimensions

Each dimension maps to the 30 targeted searches executed. Sources cited inline; full list in Section 12.

#### 3.1 Ordered degradation hierarchy (named intermediate states between Healthy and Failed)

**Sources:** sujeet.pro "Graceful Degradation" (ordered intermediate states between full health and complete failure, each trading capability for availability); HLD Handbook "Graceful Degradation: When Partial Service Beats No Service" (critical-path tiering, three-tier model); antimetal glossary (explicit ranking of functionality by importance; fallbacks, feature flags, load shedding).

The crate defines an ordered chain of degradation states `Healthy → Degraded → Limited → Minimal → Static → Failed`. Each state has: a name, an ordinal severity, a capability mask (which capabilities are on/off), entry conditions, exit conditions, entry/exit actions, and an operator-notice template. States are totally ordered by severity so there is always a well-defined "next worse" and "next better" state. Degradation proceeds from the edges inward: non-critical capabilities shed first; the critical path is the last to degrade.

#### 3.2 Fallback behavior per state (fallback chains)

**Sources:** sujeet.pro fallback chains (fresh → cached → simplified → static default → error); Tetrate Agent Router fallback policy (ordered backends, advance on recoverable error); nRouter provider fallback chains (ordering is deterministic; advance conditional on retryable-only failures; exhaustion is an error); Frontier Engineering circuit-breaker + fallback chain (degrade through progressively simpler alternatives).

Each capability in the system has a fallback chain: an ordered list of fallback behaviors, tried in order on failure. The chain terminates when an attempt succeeds; if every fallback is exhausted, the result is an explicit error (never a silent stub). Failure classification decides whether to advance: retryable failures (timeout, 5xx-equivalent, capacity) advance; terminal failures (malformed request, auth failure) fail fast. Each fallback level is tagged with its quality impact so degradation is observable and honest.

#### 3.3 Recovery path (return from degraded to healthy)

**Sources:** Azure self-preservation degraded mode (failure detection → automated initiation → alternative paths); Atlas of Engineering degraded mode (entry conditions explicit; exit requires evidence the degraded condition cleared + hysteresis/persistence rules; transition time budget T_rev = T_detect + T_decide + T_transition ≤ T_req); Google SRE cascading failures (degraded code path must be exercised regularly or it rots).

The recovery path is the reverse transition chain: `Failed → Static → Minimal → Limited → Degraded → Healthy`. Exit from a degraded state requires: (a) the failure condition cleared, (b) a confirmation window `T_confirm` elapsed without recurrence (hysteresis, prevents chatter), (c) a bounded transition time `T_rev` checked against a requirement `T_req`. Recovery actions are logged and notified to operators. The crate enforces that recovery cannot skip states (must traverse the ordered chain back up) to avoid re-exciting a flapping dependency.

#### 3.4 Capability reduction (what is shed at each state)

**Sources:** HLD Handbook critical-path tiering (Google's four fixed criticality levels CRITICAL_PLUS/CRITICAL/SHEDDING_PLUS/SHEDDING; tie tiers to SLOs); alivedise BEE-12005 graceful degradation (critical vs non-critical feature classification; per-feature fallback); Defcon (Meta graceful feature degradation; essential vs fungible features; knob-driven runtime degradation).

Every capability (read path, write path, replication, rebalancing, anti-entropy, telemetry verbosity, personalization, etc.) is assigned to a tier. At each degradation state, a capability-reduction mask specifies which tiers are disabled or degraded. Tier 0 (critical) is disabled only at `Failed`; Tier 3 (non-critical) is disabled at `Limited`. Reduction is deterministic: given the same state, the same capabilities are reduced. No ad-hoc shedding.

#### 3.5 Availability tradeoff (documented per state)

**Sources:** antimetal graceful degradation vs failover (degradation handles capability failure where no equivalent exists; failover handles infrastructure failure with redundancy); AWS REL05-BP01 (transform hard dependencies into soft dependencies; component functions minimally impeded); Google Cloud graceful degradation (system continues with reduced performance/accuracy; resumes full functionality when load eases).

For each state the crate documents: what capability is lost, what SLA/SLO no longer applies, what error callers receive, and what the operator must do. The tradeoff is explicit: availability is bought with reduced capability, never silently. This documentation is part of the crate's public contract so downstream crates (ids-store, ids-replication) can rely on it.

#### 3.6 Operator notification of degraded state

**Sources:** Azure self-preservation (degraded UX: notification mechanism tells users what functionality remains/changed; banners by severity); HLD Handbook (Tier 3 shed → no banner; Tier 2 shed → yellow banner; database-down → red banner with status page link); New Relic four considerations (shed workload, time shift, reduce quality, add capacity — user-impact ordering).

Every state transition emits an operator notification: from-state, to-state, reason, capabilities affected, estimated recovery, and recommended action. Notifications are severity-graded (info for Degraded, warning for Limited/Minimal, critical for Failed) and routed through the carrier as opaque datagrams so downstream observability crates can consume them. Silent degradation is treated as a bug.

#### 3.7 Circuit-breaker integration per dependency

**Sources:** Martin Fowler Circuit Breaker (Closed/Open/Half-Open; failure threshold trips; half-open permits trial requests); Azure circuit breaker (Closed → Open on threshold; Half-Open after timeout; closed again on success); AWS circuit breaker (prevents retrying a failing callee; detects recovery).

Each dependency wrapped by the crate has a circuit breaker. Closed: calls proceed; failures counted. Open: calls short-circuit, fallback used immediately. Half-Open: limited trial requests; success closes, failure re-opens. The breaker state feeds the degradation state machine: an open breaker on a critical dependency can push the system down one degradation level.

#### 3.8 Load shedding by priority tier

**Sources:** Google SRE cascading failures (load shedding drops load as server approaches overload; per-task throttling; queue-length limiting); HLD Handbook load shedding (reject at earliest layer with cheap 503 + Retry-After; shed non-critical first; criticality propagates across RPC hops); Uber Cinnamon (priority-tagged requests; PID-controller rejection threshold).

When overload is detected, the crate sheds load by rejecting the lowest-criticality requests first with a fast, cheap response. Shedding decisions use the same tiered criticality as the degradation hierarchy so shedding and degradation are consistent. Health checks are given priority over other requests (New Relic).

#### 3.9 Retry storm / thundering herd prevention

**Sources:** Azure retry storm antipattern (limit attempts; pause with exponential backoff; circuit breaker; honor retry-after); Google SRE (randomized exponential backoff with jitter; retry budget 10%; cap amplification); gronex (jitter desynchronizes clients; retry budget; circuit breaker + retry budget combined).

The crate bounds retries: capped exponential backoff with full jitter, a per-operation deadline, and a retry budget tied to successful request volume. When the budget is exhausted, retries are shed. Circuit breakers and retry budgets are used together: the breaker stops traffic entirely past a hard threshold; the budget prevents amplification during partial degradation.

#### 3.10 Fail-safe defaults and safe-state transitions

**Sources:** arc42 Fail-Safe Defaults (define explicit safe state per component/mode; unrecognized inputs denied by default; atomic transition; alert operators); Saltzer & Schroeder 1975 (fail-safe defaults; base decisions on permission not exclusion); Nguyen "Designing Defaults That Fail Safely" (name the failure mode; what does the user see if dependency is down).

Every state transition defines a safe state: if the crate cannot determine the correct action, it degrades rather than proceeding optimistically. Transitions are atomic where possible (no partial states that are neither operational nor safe). Safe states are documented explicitly so operators know what to expect and what recovery steps are required.

#### 3.11 Degradation hysteresis and anti-chatter

**Sources:** Atlas of Engineering degraded mode (exit requires evidence condition cleared + `t_valid ≥ T_confirm`; prevents rapid mode chatter); Google SRE cascading failures (degraded mode won't be exercised in steady state; exercise it regularly or it rots).

The state machine enforces hysteresis: a state change downward requires the fault condition to persist; a change upward requires the healthy condition to persist for `T_confirm`. This prevents flapping when a dependency hovers near a threshold. The crate also schedules periodic exercises of degraded paths (canary) so the rarely-used code stays working.

#### 3.12 Degradation vs failover distinction

**Sources:** antimetal (failover replaces failed component with healthy equivalent — full functionality preserved; degradation reduces functionality when no equivalent exists); New Relic (networks re-route traffic if path unavailable; enterprise apps shed/reduce quality).

The crate is explicitly about degradation (capability reduction when no redundant equivalent exists), not failover (switching to a healthy replica). Failover is handled by ids-stonith / ids-replication; this crate owns the degradation policy when failover is exhausted or unavailable.

#### 3.13 Audit trail for degradation decisions

**Sources:** MongoDB causal consistency (read/write concerns); AWS REL05-BP01 (failure modes seen as normal operation; predictable and recoverable states); Azure self-preservation (audit/logging of degraded mode).

Every degradation decision (enter state, exit state, shed load, trip breaker) is recorded in an append-only, hash-chained audit log with `NodeId` + `IdsTime`, sha256/blake3 chaining, and periodic ed25519-signed checkpoints. Offline-verifiable; tamper-evident. This makes degradation visible and auditable — addressing the "silent degradation is dangerous" anti-pattern.

#### 3.14 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Sources:** RFC 8032 (ed25519); RFC 7539 (ChaCha20-Poly1305, 96-bit nonce); libsodium (XChaCha20-Poly1305, 192-bit nonce); NIST FIPS 180-4 (SHA-256); BLAKE3 spec.

The crate uses: ed25519 for signing degradation decisions and audit checkpoints; sha256 for profile/state IDs and audit-chain links; blake3 for fast content hashing of degradation state and capability masks; xchacha20poly1305 for encrypting degradation state at rest (random 192-bit nonces); chacha20poly1305 for encrypting session/state payloads (12-byte nonces). All via `ids-crypto` primitive IDs; no other crypto.

#### 3.15 Carrier-agnostic transport

**Sources:** HX-241 carrier-agnostic socket abstraction; IDS prompts.txt entry 1 (Endpoint = CarrierType + Vec<u8>; no IP/TCP/UDP).

All degradation notifications, audit events, and state transitions are shipped as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). The crate imports no networking crate and never references `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
||--------|-----------|-------------------------------|--------------------------|
|| Silent degradation (operators unaware) | Operability / trust | Every state transition emits operator notification; silent degradation treated as bug; audit log |
|| Degradation chatter (rapid state flapping) | Stability | Hysteresis: entry requires persistence; exit requires `T_confirm` without recurrence |
|| Fallback code path rot (rarely exercised code breaks) | Reliability | Periodic canary exercise of degraded paths; degradation tests in CI |
|| Fallback harder to prove/test than primary path | Reliability | Fallback paths are simpler than primary (AWS guidance); covered by property tests |
|| Retry storm amplifying overload | Availability | Capped exponential backoff + jitter; retry budget; circuit breaker |
|| Circuit breaker trips too early/late | Stability | Tunable failure threshold, reset timeout, success threshold; documented defaults |
|| Operator override disables fail-safe | Safety | Fail-safe defaults documented; overrides require signed governance decision, audited |
|| Clock manipulation affecting recovery timing | Causality correctness | Monotonic `IdsTime` only for timeouts/hysteresis; no wall-clock for state decisions |
|| Partial state transition (neither safe nor operational) | Safety | Atomic transitions where possible; intermediate states documented |
|| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
|| Key extraction (degradation signing key) | Decision authenticity | Keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
|| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent state application; replay cache per degradation decision |
|| Degradation below safe threshold | Safety | Each state documents minimum viable capability; `Failed` is the safe default |
|| Crypto beyond allowed list | Policy compliance | CI allowlist test; only five primitives reachable |

The carrier is never trusted: degradation decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-graceful-degradation ||
||-------|------|--------------------------|--------------------------------------|
|| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpState`, `Endpoint`, `FailureDomain`, `Error` — consumed, never redefined |
|| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All degradation notifications/audit ship to `Endpoint`; crate returns decisions, never sends directly |
|| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing decisions/audit; encrypting degradation state at rest; hashing state/capability masks |
|| `ids-health` | Health/failure signal source (probe results, dependency health) | Feeds health signals into the degradation state machine |
|| `ids-store` | Primary record store (degradation consumer) | Reads capability mask; disables/reduces capabilities per degradation state |
|| `ids-replication` | Replica synchronization | Respects degradation state: reduces replication frequency under degradation |
|| `ids-session` | Client-session binding | Sessions carry degradation context; operator notifications routed per session |
|| `ids-fault` | Fault injection / failure detection | Fault events can trigger degradation transitions |
|| `ids-backpressure` | Backpressure / load shedding | Load-shedding decisions coordinated with degradation tiers |
|| `ids-observability` | Metric/trace sinks | Degradation state and transitions exported as metrics/traces |

**Integration points:** `ids-graceful-degradation` is a pure policy crate: it consumes health signals and produces `DegradationDecision`s; it never opens a transport. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-graceful-degradation` (new crate at `C:/idfs/ids-graceful-degradation/`; separate crate, not a folder inside an existing crate):

```
ids-graceful-degradation/
├── Cargo.toml                          # serde, thiserror, blake3; workspace member
├── lib.rs                              # crate root, public API, feature gates
├── constants.rs                        # Degradation defaults: T_confirm, T_rev, tier budget
├── error.rs                            # DegradationError (thiserror): transition, fallback, audit errors
├── features.rs                         # Feature flags: audit, canary, crypto, notify
│
├── hierarchy/                          # Ordered degradation hierarchy states
│   ├── mod.rs
│   ├── state.rs                        # DegradationState enum: Healthy..Failed (6 states)
│   ├── hierarchy.rs                    # Ordered chain + next-worse / next-better
│   ├── level.rs                        # State level/severity + entry/exit conditions
│   ├── mask.rs                         # Capability-reduction mask per state
│   ├── types.rs                        # DegradationLevel, CapabilityId, CapabilityTier
│   └── transition.rs                   # Transition rules, hysteresis, T_confirm
│
├── fallback/                           # Fallback behavior per state
│   ├── mod.rs
│   ├── chain.rs                        # FallbackChain: ordered fallback behaviors
│   ├── link.rs                         # FallbackLink: single fallback + quality impact
│   ├── selector.rs                     # Select fallback by state + capability
│   ├── fresh_cached_static.rs          # Standard chain: fresh → cached → simplified → static → error
│   ├── classify.rs                     # Retryable vs terminal failure classification
│   └── types.rs                        # FallbackResult, QualityImpact structures
│
├── recovery/                           # Recovery path (degraded → healthy)
│   ├── mod.rs
│   ├── path.rs                         # Recovery chain: reverse of degradation chain
│   ├── hysteresis.rs                   # Exit confirmation window T_confirm
│   ├── budget.rs                       # Transition time budget T_rev ≤ T_req
│   ├── canary.rs                       # Periodic exercise of degraded paths
│   └── types.rs                        # RecoveryResult structures
│
├── decision/                           # Degradation decision emission
│   ├── mod.rs
│   ├── decision.rs                     # DegradationDecision: transition + mask + notice
│   ├── emitter.rs                      # Emit decisions to caller (carrier-framed by caller)
│   ├── notify.rs                       # Operator notification (severity-graded)
│   └── types.rs                        # DegradationDecision, OperatorNotice structures
│
├── breaker/                            # Circuit breaker per dependency
│   ├── mod.rs
│   ├── breaker.rs                      # Three-state machine: Closed/Open/Half-Open
│   ├── config.rs                       # Failure threshold, reset timeout, success threshold
│   └── types.rs                        # CircuitState, BreakerEvent structures
│
├── shed/                               # Load shedding by priority tier
│   ├── mod.rs
│   ├── shedder.rs                      # Reject lowest-criticality first with cheap response
│   ├── tier.rs                         # Request criticality tier + health-check priority
│   └── types.rs                        # SheddingDecision structures
│
├── retry/                              # Retry storm prevention
│   ├── mod.rs
│   ├── policy.rs                       # Capped exponential backoff + jitter + deadline + budget
│   ├── budget.rs                       # Retry budget tied to successful request volume
│   └── types.rs                        # RetryDecision structures
│
├── safe_state/                         # Fail-safe defaults
│   ├── mod.rs
│   ├── default.rs                      # Safe state per component/mode; atomic transitions
│   └── types.rs                        # SafeState structures
│
├── audit/                              # Signed, hash-chained audit log
│   ├── mod.rs
│   ├── entry.rs                        # Audit entry: decision, NodeId, IdsTime
│   ├── chain.rs                        # sha256/blake3 hash chain
│   ├── checkpoint.rs                   # Periodic ed25519-signed checkpoint
│   ├── signer.rs                       # Signing with ed25519
│   ├── verifier.rs                     # Offline verification
│   └── types.rs                        # AuditEntry, AuditChain structures
│
├── carrier_adapter.rs                  # Opaque Vec<u8> datagram framing to ids-carrier::Endpoint
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── degradation_contract.rs         # trait DegradationPolicy (sealed)
│   ├── fallback_contract.rs            # trait FallbackEngine (sealed)
│   ├── recovery_contract.rs            # trait RecoveryPath (sealed)
│   ├── breaker_contract.rs             # trait CircuitBreaker (sealed)
│   ├── notify_contract.rs              # trait OperatorNotifier (sealed)
│   ├── audit_contract.rs               # trait AuditLogger (sealed)
│   ├── shed_contract.rs                # trait LoadShedder (sealed)
│   └── carrier_contract.rs             # trait CarrierAdapter (sealed)
│
├── invariants/                         # Compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── state_ordering.rs               # States totally ordered; no skipping
│   ├── hysteresis.rs                   # Entry/exit persistence rules
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/IP/TCP/etc.
│   ├── crypto_allowlist.rs             # Only five allowed primitives
│   ├── mask_determinism.rs             # Same state ⇒ same capability mask
│   ├── atomic_transition.rs            # Transitions atomic where possible
│   ├── notification_on_transition.rs   # Every transition notifies
│   ├── safe_default.rs                 # Fail-safe default defined per mode
│   ├── audit_chain_sound.rs            # Hash chain + signed checkpoints
│   └── canary_coverage.rs              # Degraded paths exercised periodically
│
├── tests/                              # Property/verification tests (~30 files)
│   ├── hierarchy_tests.rs
│   ├── fallback_tests.rs               # Chain exhaustion returns error, never silent stub
│   ├── recovery_tests.rs               # Hysteresis; T_confirm prevents chatter
│   ├── decision_tests.rs               # Decision schema + notification
│   ├── breaker_tests.rs                # Closed/Open/Half-Open transitions
│   ├── shed_tests.rs                   # Tiered shedding; health-check priority
│   ├── retry_tests.rs                  # Backoff+jitter+budget; no storm
│   ├── safe_state_tests.rs             # Fail-safe defaults; atomic transitions
│   ├── audit_tests.rs                  # Chain verification; checkpoint verification
│   ├── carrier_agnostic_tests.rs       # Zero prohibited-dep references
│   ├── crypto_allowlist_tests.rs       # Only five primitives reachable
│   ├── determinism_tests.rs            # Same state ⇒ same mask/decision
│   ├── chatter_tests.rs                # No rapid flapping near thresholds
│   ├── canary_tests.rs                 # Degraded paths exercised in tests
│   └── property_tests.rs               # proptest properties for masks/chains
│
├── benches/                            # (~8 files)
│   ├── transition_bench.rs             # State transition latency
│   ├── fallback_bench.rs               # Fallback chain resolution latency
│   ├── shedding_bench.rs               # Load-shedding decision latency
│   └── audit_bench.rs                  # Audit entry + checkpoint latency
│
├── spec/                               # Normative specifications (~6 files)
│   ├── hierarchy_spec.md               # Degradation hierarchy contract
│   ├── fallback_spec.md                # Fallback chain contract
│   ├── recovery_spec.md                # Recovery path + hysteresis contract
│   ├── decision_spec.md                # DegradationDecision contract
│   ├── carrier_agnostic_spec.md        # Carrier-agnostic boundary contract
│   └── crypto_policy.md                # Allowed-crypto allowlist policy
│
├── docs/                               # Documentation (~10 files)
│   ├── architecture.md                 # Degradation architecture and layering
│   ├── runbook.md                      # Operator runbook for degraded states
│   ├── adding_a_capability.md          # Adding a new tiered capability
│   ├── states.md                       # Per-state behavior and tradeoffs
│   └── integration.md                  # Integration with ids-store/ids-replication
│
└── test_vectors/                       # Deterministic inputs and expected outputs
    ├── state_transitions.json          # Valid/invalid transitions
    ├── fallback_vectors.json           # Expected fallback per state+capability
    └── audit_vectors.json              # Expected chain/checkpoint outputs
```

**Why ~90–1000 files:** one file per responsibility — hierarchy, fallback, recovery, decision, breaker, shedding, retry, safe-state, audit, contracts, invariants, tests, benches, specs, docs, test vectors. Each file holds production-grade code with error handling, documentation, and tests. Reflects the genuine state-machine + policy complexity of a degradation crate. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Degradation state machine (ordered chain)

```
DegradationState { level: u8 (0..5), name, severity, capability_mask, entry_conditions, exit_conditions }
Chain: Healthy(0) → Degraded(1) → Limited(2) → Minimal(3) → Static(4) → Failed(5)
next_worse(state)  = state.level + 1  (capped at Failed)
next_better(state) = state.level - 1  (capped at Healthy)
can_transition_to(a, b) = |a.level - b.level| == 1  (no skipping states)
```

Transitions downward require fault persistence; transitions upward require `T_confirm` without recurrence (hysteresis).

#### 7.2 Fallback chain resolution

```
FallbackChain { links: [FallbackLink; N] }  // ordered by quality, best first
execute(input): for link in links:
  if link.breaker.is_open(): continue
  try link.fn(input) → result
    if success: return { result, source: link.name, degraded: true }
    if failure is terminal: return error (do not advance)
    if failure is retryable: continue to next link
return error { exhausted: true, last_error }  // never a silent stub
```

Each link tagged with `QualityImpact` (documented degradation). Circuit breaker per link skips open breakers.

#### 7.3 Circuit breaker (three-state machine)

```
state: Closed | Open | HalfOpen
Closed:  call proceeds; failure_count++ ; if ≥ threshold → Open, start reset_timeout
Open:    calls short-circuited; fallback used; after reset_timeout → HalfOpen
HalfOpen: limited trial requests; success → Closed; failure → Open
```

Parameters: `failure_threshold`, `reset_timeout`, `success_threshold` (tunable, documented defaults).

#### 7.4 Capability reduction mask

```
CapabilityMask { bit per CapabilityId; tier: u8 }
apply(state): mask = default_mask AND NOT disabled_tiers(state)
disabled_tiers(Degraded) = {}; (Limited) = {Tier3}; (Minimal) = {Tier2, Tier3, writes}; ...
```

Deterministic: same state ⇒ same mask. Tier 0 (critical) disabled only at `Failed`.

#### 7.5 Load shedding by tier

```
shed(request): if system.overloaded():
  reject lowest criticality tier first with cheap 503 + Retry-After
  health-check requests never shed
return request  // admitted
```

Criticality propagates: the tier of the calling request sets the tier of downstream calls (per HLD Handbook).

#### 7.6 Retry policy (storm prevention)

```
retry(attempt): delay = min(base * 2^attempt, cap) * random_jitter(0..1)
if attempt ≥ max_attempts or deadline exceeded: shed (fail fast)
if retry_budget exhausted: shed
```

Jitter desynchronizes clients; budget caps amplification (Google SRE).

#### 7.7 Audit chain

```
entry_i = { decision, node_id, ids_time, prev_hash, payload_hash }
chain_hash_i = blake3(entry_i)
checkpoint_k = ed25519_sign(chain_hash at checkpoint interval)
```

Offline-verifiable: recompute chain, verify signatures.

#### 7.8 Operator notification

```
notify(transition): { from, to, reason, capabilities_affected, severity, recommended_action }
severity: Degraded=info, Limited=warning, Minimal=warning, Static=critical, Failed=critical
```

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------||
|| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpState`, `Endpoint`, `FailureDomain`, `Error`) and sealed trait contracts |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly |
|| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
|| `ids-health` | Health/failure signals feeding the state machine |
|| `ids-store`, `ids-replication`, `ids-session`, `ids-fault`, `ids-backpressure`, `ids-observability` | Consumers of degradation decisions (via public contracts only) |
|| `serde` | Serialization of degradation envelopes, decisions, audit entries |
|| `thiserror` | Error types |
|| `blake3` | Content hashing of degradation state and capability masks (type-level surface; ops via ids-crypto) |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list; `ids-graceful-degradation` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|-------------|
|| AC-81.1 | Six ordered degradation states defined: Healthy, Degraded, Limited, Minimal, Static, Failed | `src/hierarchy/state.rs` defines `DegradationState` enum with 6 levels, totally ordered |
|| AC-81.2 | No state skipping: transitions only between adjacent levels | `can_transition_to` enforces `|level_a - level_b| == 1`; unit tests verify |
|| AC-81.3 | Fallback chain per capability: fresh → cached → simplified → static default → error | `src/fallback/chain.rs` resolves ordered links; exhaustion returns error, never silent stub |
|| AC-81.4 | Circuit breaker per dependency: Closed/Open/Half-Open | `src/breaker/breaker.rs` implements three-state machine; threshold/reset/success configurable |
|| AC-81.5 | Recovery path is reverse chain with hysteresis (T_confirm) | Exit requires `T_confirm` without recurrence; unit tests verify no chatter |
|| AC-81.6 | Capability-reduction mask deterministic per state | Same state ⇒ same mask; Tier 0 (critical) disabled only at Failed |
|| AC-81.7 | Operator notification on every state transition | `notify` emits severity-graded notice; test asserts notification per transition |
|| AC-81.8 | Load shedding by priority tier; health checks never shed | `src/shed/shedder.rs` rejects lowest tier first; health-check priority test |
|| AC-81.9 | Retry storm prevention: capped backoff + jitter + deadline + budget | `src/retry/policy.rs`; property test: no unbounded retry amplification |
|| AC-81.10 | Fail-safe defaults: on uncertainty, degrade and signal | `src/safe_state/default.rs`; safe state documented per mode |
|| AC-81.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
|| AC-81.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | CI allowlist test passes |
|| AC-81.13 | Every degradation decision in append-only, hash-chained, ed25519-signed audit log | `src/audit/chain.rs` + `checkpoint.rs`; offline verifier test passes |
|| AC-81.14 | Crate compiles cleanly | `cargo check -p ids-graceful-degradation` exits 0 with no warnings |
|| AC-81.15 | Conformance tests pass | `cargo test -p ids-graceful-degradation` passes all hierarchy/fallback/recovery/breaker/shed/retry/audit tests |
|| AC-81.16 | Carrier-agnostic assertion test passes | Greps source for prohibited terms — zero matches |
|| AC-81.17 | Canary coverage: degraded paths exercised in test/CI | Canary tests run and pass |

---

### 10. Remaining Uncertainties

- **Concrete per-state capability masks:** which capabilities belong to which tier and are disabled at which state — deployment-tuned; to be validated at integration with `ids-store`/`ids-replication`.
- **T_confirm / T_rev defaults:** hysteresis confirmation window and transition time budget are workload-dependent; unquantified pending integration.
- **Fallback chain per capability:** the exact ordered fallback list (fresh/cached/simplified/static) per capability must be defined by product teams; this crate provides the mechanism, not the policy content.
- **Circuit-breaker parameter defaults:** failure_threshold, reset_timeout, success_threshold need production tuning.
- **Load-shedding threshold:** the overload signal (queue length, CPU, latency) that triggers shedding is not yet defined.
- **Interaction with ids-health:** health signal format and frequency feeding the state machine to be confirmed at integration.
- **Audit checkpoint interval:** how often ed25519-signed checkpoints are written; trades audit granularity against cost.
- **Canary exercise frequency:** how often degraded paths are exercised in production without causing real degradation.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate `ids-graceful-degradation` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear policy boundary |
|| Six-state ordered chain (Healthy→…→Failed) | Named intermediate states between health and failure; each trades capability for availability (research-verified) |
|| No state skipping (adjacent transitions only) | Prevents re-exciting flapping dependencies; well-defined next-worse/next-better |
|| Fallback chain: fresh → cached → simplified → static default → error | Ordered alternatives preserve forward progress with reduced quality (Tetrate/nRouter research) |
|| Exhaustion = error, never silent stub | Silent degradation is dangerous (AWS/Azure guidance); every path must be observable |
|| Circuit breaker per dependency (Closed/Open/Half-Open) | Isolates failing components; prevents cascade (Nygard; Azure/AWS) |
|| Recovery path = reverse chain + hysteresis (T_confirm) | Prevents chatter; exit requires evidence condition cleared (Atlas of Engineering) |
|| Capability mask deterministic per state | Same state ⇒ same reduction; operators and downstream crates can rely on it |
|| Tier 0 (critical) disabled only at Failed | Critical path preserved as long as possible (degrade from edges inward) |
|| Operator notification on every transition | Silent degradation treated as bug; severity-graded notices (Azure/HLD Handbook) |
|| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable, tamper-evident degradation log |
|| Carrier-agnostic via `ids-carrier::Endpoint`; crate returns decisions, never sends | Preserves IDS architecture; no network stack in trust base |
|| Allowed crypto only; via `ids-crypto` primitive IDs | Complies with IDS crypto policy |
|| Retry: capped backoff + jitter + deadline + budget | Prevents retry storms / thundering herd (Google SRE; Azure) |
|| Fail-safe defaults; atomic transitions | On uncertainty, degrade safely (arc42; Saltzer & Schroeder) |
|| Canary exercise of degraded paths | Prevents fallback code-path rot (Google SRE: "code path you never use") |

---

### 12. Sources

1. Sujeet Jaiswal, "Graceful Degradation" — ordered intermediate states between full health and complete failure; degradation hierarchy; fallback chains (fresh → cached → simplified → static default → error). https://sujeet.pro/articles/system-design/core-distributed-patterns/graceful-degradation.html ; https://sujeet.pro/articles/graceful-degradation
2. HLD Handbook, "Graceful Degradation: When Partial Service Beats No Service" — critical-path tiering; load shedding; feature flags as kill switches; cached fallbacks; static fallbacks; operator notification banners. https://hld.handbook.academy/curriculum/reliability-and-operations/graceful-degradation
3. Michael Nygard, "Release It!" / Martin Fowler, "Circuit Breaker" — three-state circuit breaker (Closed/Open/Half-Open); failure threshold; half-open probe. https://martinfowler.com/bliki/CircuitBreaker.html
4. Azure Architecture Center, "Circuit Breaker Pattern" — Closed/Open/Half-Open transitions; timeout; forced open/close; observability. https://learn.microsoft.com/en-us/azure/architecture/patterns/circuit-breaker
5. AWS Prescriptive Guidance, "Circuit Breaker Pattern" — prevent retrying failing callee; detect recovery; service-agnostic implementation. https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/circuit-breaker.html
6. Google SRE, "Addressing Cascading Failures" — load shedding; graceful degradation; randomized exponential backoff + jitter; retry budget; degraded code path must be exercised. https://sre.google/sre-book/addressing-cascading-failures/
7. Google Cloud Architecture Center, "Design for graceful degradation" — system continues with reduced performance/accuracy; throttling; drop excess requests early; handle partial errors. https://docs.cloud.google.com/architecture/framework/reliability/graceful-degradation
8. Azure Well-Architected, "Self-healing and self-preservation" — graceful degradation mode; failure detection and automated initiation; degraded UX notification; alternative paths. https://learn.microsoft.com/en-us/azure/well-architected/reliability/self-preservation
9. Azure Well-Architected, "Retry Storm Antipattern" — limit attempts; pause with exponential backoff; circuit breaker; honor retry-after. https://learn.microsoft.com/en-us/azure/architecture/antipatterns/retry-storm
10. Antimetal, "Graceful Degradation" — explicit ranking of functionality; degradation vs failover distinction; silent degradation is dangerous. https://antimetal.com/glossary/graceful-degradation
11. Atlas of Engineering, "Degraded Mode, Fallback Operation and Validation" — entry conditions explicit; exit requires evidence + hysteresis `t_valid ≥ T_confirm`; transition time budget `T_rev ≤ T_req`. https://atlasofengineering.com/industrial-and-management-engineering/degraded-mode
12. arc42 Quality Model, "Fail-Safe Defaults" — explicit safe state per component/mode; unrecognized inputs denied by default; atomic transition; alert operators (Saltzer & Schroeder 1975). https://quality.arc42.org/approaches/fail-safe-defaults
13. Tetrate Agent Router, "Improve resilience with fallbacks" — fallback policy as ordered backends; advance on recoverable error; exhaustion is an error. https://docs.tetrate.ai/v0.4.0/agent-router-service/guides/improve-resilience-with-fallbacks
14. nRouter, "LLM Provider Fallback Chains" — deterministic ordering; advance conditional on retryable-only; exhaustion is an error; response reports truth. https://nrouter.ai/blog/engineering/provider-fallback-chains
15. Frontier Engineering, "Circuit Breaker and Fallback Chain" — degrade through progressively simpler alternatives; named tiers; quality impact. https://www.frontierengg.com/learn/patterns/reliability-guardrails/circuit-breaker-and-fallback
16. alivedise, "BEE-12005: Graceful Degradation" — critical vs non-critical features; per-feature fallback; degradation hierarchy; feature flags as runtime kill switches. https://alivedise.github.io/backend-engineering-essentials/resilience/graceful-degradation
17. Defcon (Meta), "Preventing Overload with Graceful Feature Degradation" — essential vs fungible features; knob-driven runtime degradation; anti-entropy convergence. https://www.usenix.org/system/files/osdi23-meza.pdf
18. RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA), ed25519. https://www.rfc-editor.org/rfc/rfc8032
19. RFC 7539 — ChaCha20 and Poly1305 for IETF Protocols. https://www.rfc-editor.org/rfc/rfc7539
20. Libsodium — XChaCha20-Poly1305 construction (192-bit nonce). https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
21. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
22. BLAKE3 — cryptographic hash function. https://github.com/BLAKE3-team/BLAKE3
23. Saltzer & Schroeder, "The Protection of Information in Computer Systems" (1975) — fail-safe defaults design principle. (Fail-safe defaults.)
24. AWS Builder Library, "Avoiding fallback in distributed systems" — fallback hard to prove/test; favor continuously-exercised code paths. https://aws.amazon.com/builders-library/avoiding-fallback-in-distributed-systems
25. AWS Well-Architected, "REL05-BP01 Implement graceful degradation" — transform hard dependencies into soft dependencies; failure modes as normal operation; predictable recoverable states. https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/rel_mitigate_interaction_failure_graceful_degradation.html

---

**End of plan-081.md.** Research only — no code written, no files created, no repository changes made beyond this plan document.