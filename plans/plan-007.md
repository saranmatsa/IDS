## Phase 007 — Create the ids-failure-detector crate: Chandra-Toueg failure detector properties — completeness and accuracy.

**Status:** Research complete (30 targeted searches executed across SWIM gossip failure detection, phi accrual suspicion, Chandra-Toueg completeness/accuracy classes, suspected-vs-proven failure, heartbeat-loss-vs-GC-pause semantics, and detector oscillation dimensions). No code written, no files modified. This document is the blueprint for implementation.

---

### 1. Executive Summary

Create `ids-failure-detector` (IDS crate #7), a carrier-agnostic failure-detection crate built on two engines:

- **SWIM gossip-based detection** (Scalable Weakly-consistent Infection-style Membership, Das et al.): each node periodically probes one random peer per protocol period, follows up with indirect (k-prober) confirmation, disseminates suspicion via gossip piggybacked on ping/ack, and moves members through alive → suspected → dead states. Suspicion precedes confirmed failure specifically to bound false positives under slow carriers or overloaded nodes.
- **Phi accrual suspicion** (Hayashibara et al., SRDS 2004): a continuous suspicion level φ = -log10(1 − F(timeSinceLastHeartbeat)) computed from a sliding window of heartbeat inter-arrival times, replacing binary timeouts with an application-selectable suspicion threshold.

The crate exposes Chandra-Toueg failure-detector properties — strong/weak completeness and strong/weak accuracy — as explicit, tunable contracts, and cleanly separates **suspected** failure (unconfirmed hint) from **proven** failure (confirmation evidence). All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere. Cryptography is limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (via `ids-crypto`). Timing uses `ids-time::Idstime` (the IDS proprietary time basis), never wall-clock. Research only — no code, no source modifications.

### 2. Architecture Overview

The crate is organized as layered, testable modules above a carrier abstraction:

```
┌─────────────────────────────────────────────────────────────┐
│                   CONSUMERS (ids-*)                         │
│  ids-membership · ids-fault · ids-recovery · ids-chaos ·    │
│  ids-observability                                            │
├─────────────────────────────────────────────────────────────┤
│              ids-failure-detector (Phase 007)               │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐ │
│  │   SWIM       │  │   Phi        │  │ Chandra-Toueg      │ │
│  │   engine     │  │   accrual    │  │   properties       │ │
│  │  probe ·     │  │  φ score ·   │  │  completeness ·    │ │
│  │  gossip ·    │  │  threshold · │  │  accuracy ·        │ │
│  │  dissemination│ │  window ·   │  │  suspected vs      │ │
│  └──────┬───────┘  │  pause gate  │  │  proven failure    │ │
│         │          └──────┬───────┘  │  oscillation       │ │
│  ┌──────▼─────────────────▼──────────┤ damping ·          │ │
│  │   Detection engine + verdict state│  hysteresis        │ │
│  │   suspected ↔ proven ↔ alive ↔ dead│                    │ │
│  └──────┬────────────────────────────┘                    │ │
│  ┌──────▼──────────────────────────────────────────────┐ │ │
│  │  Carrier transport: ids-carrier::Endpoint only      │ │ │
│  │  ed25519-signed suspicion/confirm datagrams         │ │ │
│  │  blake3/sha256 digests · optional chacha20poly1305  │ │ │
│  └────────────────────────────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────────┘
                         ▲
                    ids-carrier::Endpoint
                  (CarrierType + Vec<u8>)
```

**Request flow:** local detector schedules a SWIM probe → sends a `Ping` datagram over `ids-carrier::Endpoint` → `Ping-Ack` returns → on silence, dispatch `k` indirect probers → combine responses into suspicion evidence → feed inter-arrival times into the phi accrual detector → compute φ against threshold Φ → emit `suspected` verdict → run completeness/accuracy evaluation → if confirmed (indirect-prober agreement or STONITH evidence via `ids-stonith`), emit `proven failure` → oscillation damping/hysteresis before state transitions → publish failure-detector events to consumers (ids-membership, ids-fault, ids-recovery, ids-observability).

Existing-crate inspection relevant to this phase:

| Crate | Role | Relevance to ids-failure-detector |
|-------|------|-----------------------------------|
| `ids-000-aa` | Canonical types + sealed trait contracts (master crate) | `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `FailureDomain`, error contracts, prelude — all detector types derive from here |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`), send/receive datagrams | The only traffic path; all suspicion/heartbeat/confirmation datagrams are `Endpoint` payloads |
| `ids-crypto` | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signed suspicion/confirmation messages, member-state digests, optional payload encryption, replay nonces |
| `ids-time` | Proprietary `Idstime` basis (not wall-clock/HLC) | Phi inter-arrival measurements keyed to `Idstime` ticks; drift tolerance absorbed here |
| `ids-topology` | `FailureDomain` hierarchy, correlated failure modeling | Suspicion grouping by failure domain; correlated-failure awareness prevents domain-wide oscillation |
| `ids-swim` (#75) / `ids-phi-accrual` (#76) | Dedicated SWIM / phi-accrual crates (later phases) | ids-failure-detector is created earlier; it contains SWIM + phi modules internally and can later delegate to #75/#76 |
| `ids-membership` (#8) | Node lifecycle states incl. `failed` | Consumes suspected/proven verdicts to drive candidate→active→failed→retired transitions |
| `ids-fault` (#27) | Circuit breakers, bulkheads, graceful degradation | Considers detector verdicts when tripping per-peer circuit breakers |
| `ids-recovery` (#25) | Multi-level failure recovery | Triggered on proven failure (node/H-unit/region) |
| `ids-chaos` (#43) | Fault injection | Injects carrier silence, delay, reordering to validate detector behavior |
| `ids-observability` (#44) | Metrics/tracing | Exports phi values, suspicion rates, oscillation events, false-positive/negative counters |

### 3. Detailed Research Dimensions

#### 3.1 SWIM gossip-based failure detection

**Sources:** SWIM paper (Das et al., Cornell Quicksilver), Wikipedia SWIM Protocol, Tarantool SWIM suspicion implementation, Apple swift-cluster-membership, Lifeguard (arxiv:1707.00788), "Building a SWIM Gossip Membership Protocol from Scratch in Rust".

- **Separation of failure detection and membership dissemination:** SWIM decouples "is this node alive?" from membership change propagation; both piggyback on the same ping/ack datagrams so no extra traffic is needed.
- **Protocol period T:** each member picks one target per period (random selection, or round-robin per brianstorti/SWIM hardening notes), sends `Ping`; target replies `Ping-Ack`.
- **Indirect probing (k probers):** on `Ping` silence, the detector asks k randomly chosen peers to probe the silent member; agreement among indirect probers upgrades suspicion toward confirmation. This is the SWIM confirmation mechanism that trades detection latency for lower false-positive rate (Lifeguard: suspicion trades increased detection latency for lower false-positive rate).
- **Suspicion before death:** a member failing k pings moves `alive → suspected`, not straight to `dead`. Suspicion is disseminated via gossip (piggybacked on ping/ack/indirect-ping), so the whole cluster converges on the suspicion in O(log N) periods without a central coordinator.
- **Status machine:** `alive → suspected → (confirmed) dead`, with possible `suspected → alive` refutation when a ping-ack arrives (incarnation-based refutation: a node returning under the same incarnation refutes stale suspicion; per IDS invariant `dead_node_identity`, a dead node never returns with its old identity — a fresh ID is assigned).
- **Scalability:** load per member stays O(1) regardless of cluster size; detection completes in expected constant time per the SWIM paper.

#### 3.2 Phi accrual suspicion

**Sources:** Hayashibara et al. "The φ Accrual Failure Detector" (IS-RR-2004-010, SRDS 2004); Akka/Pekko Phi Accrual Failure Detector docs; Cassandra `phi_convict_threshold` deep dive; digitalis.io Cassandra failure-detection analysis; arpitbhayani.me phi accrual exposition; docs.rs `phi-accrual-failure-detector`.

- **Accrual abstraction:** instead of a binary alive/dead timeout, the detector outputs a continuous suspicion level φ; the consumer chooses the conviction threshold Φ (the paper's sweet spot: 8 ≤ Φ).
- **Formula:** φ = -log10(1 − F(timeSinceLastHeartbeat)), where F is the CDF of the inter-arrival-time distribution (normal approximation estimated from a sliding window of the last N heartbeat intervals — the paper keeps the last 1000 intervals).
- **Adaptive to conditions:** mean/variance are re-estimated continuously so φ dynamically reflects current network/carrier conditions rather than a fixed timeout.
- **Pause survival:** a configurable margin of potentially lost/delayed heartbeats is accepted before anomaly so that GC pauses and transient carrier slowdowns do not immediately convict (Pekko docs, docs.rs margin note).
- **Load-aware adjustment:** phi rises when observed inter-arrival grows; under load the estimator widens, avoiding conviction of merely-slow peers (Akka docs: "survive sudden abnormalities, such as garbage collection pauses and transient network failures").
- **IDS time binding:** inter-arrival times are measured in `ids-time::Idstime` ticks, not wall-clock, honoring the IDS proprietary time basis and its drift tolerance.

#### 3.3 Chandra-Toueg completeness and accuracy properties

**Sources:** Chandra & Toueg, "Unreliable Failure Detectors for Reliable Distributed Systems" (JACM 43(2):225–267, 1996); UT Austin CS380D failure-detector notes; Yale PineWiki FailureDetectors; NYU failure-detector notes; Wikipedia Failure detector.

Failure detectors are characterized by two abstract properties:

- **Completeness** (suspicion of faulty processes):
  - *Strong completeness:* every faulty process is eventually permanently suspected by every correct process.
  - *Weak completeness:* every faulty process is eventually permanently suspected by at least one correct process.
- **Accuracy** (suspicion of correct processes):
  - *Strong accuracy:* no correct process is ever suspected (perpetual).
  - *Weak accuracy:* at most one correct process is suspected (perpetual), or some correct process is never suspected.
  - *Eventual strong accuracy:* after some time t, no correct process is suspected.
  - *Eventual weak accuracy:* after some time t, some correct process is never suspected.
- **Classes:** P (perfect: strong completeness + strong accuracy), S (strong: strong completeness + eventual strong accuracy), ◊S (eventually strong), ◊W (eventually weak — weakest sufficient for consensus), FP. The distinction between strong and weak completeness is largely spurious in practice (Yale notes), but the crate exposes both as explicit, selectable contracts per dataset/consistency profile.
- **IDS posture:** no perfect failure detector can exist in an asynchronous system with unreliable carriers (impossibility: crash vs. slow carrier indistinguishable). ids-failure-detector targets class ◊S-style behavior with tunable completeness/accuracy knobs.

#### 3.4 Suspected vs proven failure

**Sources:** Chandra-Toueg JACM 1996 (suspicion is a hint, not a fact); EPFL Nestmann "Unreliable Failure Detectors via Operational Semantics"; Yale PineWiki ("the output of a failure detector is merely a hint at the actual state of the world"); Tarantool SWIM (suspected ≠ dead).

- **Suspected:** an unconfirmed hint — φ ≥ Φ or SWIM indirect-prober silence. No state change in membership/placement/replication may rest on suspicion alone; suspicion triggers preparatory actions only (e.g., avoid new writes to the node, increase replication factor targets elsewhere).
- **Proven:** requires confirmation evidence — indirect-prober quorum agreement, `Ping-Ack` failure across k probers, or STONITH/fencing evidence via `ids-stonith`/`ids-fencing-epoch`. Only a proven failure transitions a node to `failed` in ids-membership and triggers ids-recovery.
- **Refutation:** a successful `Ping-Ack` from a suspected node refutes suspicion and returns it to `alive` (subject to the `dead_node_identity` invariant — old identity never reused).

#### 3.5 Heartbeat loss vs GC pause distinction

**Sources:** Pekko Phi Accrual Failure Detector docs; rahulsuryawanshi.com failure detection (GC pause vs crash); Arpit Bhayani heartbeats blog; ITUOnline heartbeat false-positive/negative definitions.

- A missed heartbeat has multiple causes: node crash, carrier silence/delay, process stop-the-world GC pause, overload, scheduler stall. The detector must not equate "heartbeat absent" with "node dead."
- Phi accrual helps: during a GC pause, inter-arrival times grow but remain within the widening distribution tail for a bounded interval; φ rises gradually, and the configurable heartbeat-margin absorbs occasional pauses instead of convicting immediately.
- The crate exposes a classifier (research dimension, not a verdict): `heartbeat_loss` (phi persists above Φ past pause margin) vs `gc_pause_suspect` (phi spike bounded, self-heals on heartbeat resumption). Final proven failure still requires SWIM confirmation; GC-pause suspects that recover are refuted without triggering recovery.

#### 3.6 False positive / false negative handling

**Sources:** "The Neutralizer" (Computers in Engineering, cpe.1338 — false-positive/false-negative tightrope in distributed storage); IEEE clock-failure-detector failure-mode taxonomy (F1 false positive, F2 false negative); Lifeguard arxiv:1707.00788 (SWIM false positives under slow processing); Tarantool SWIM suspicion (false-positive protection).

- **False positive:** healthy node suspected/proven failed → unnecessary recovery, replica loss, traffic diversion. Mitigations: suspicion-before-death, phi margin, indirect-prober confirmation, damping/hysteresis, ed25519-authenticated refutation messages.
- **False negative:** failed node not suspected → stale reads, writes to dead node, split-brain risk. Mitigations: bounded protocol period T, anti-entropy gossip re-probing, eventual strong completeness target, cross-check with ids-membership/ids-topology failure-domain evidence.
- The crate reports both rates as observability metrics (ids-observability integration) and exposes configurable false-positive/false-negative budgets per failure domain.

#### 3.7 Detector oscillation (flapping)

**Sources:** Webalert alert-flapping damping/hysteresis guidance; Lifeguard SWIM false-positive sensitivity; Tarantool SWIM suspicion (overloaded node not processing ACKs → false suspicion).

- **Oscillation:** a node repeatedly flips `suspected ↔ alive` under marginal conditions (slow carrier, GC pauses, overload), cascading into membership/placement churn.
- **Countermeasures:** pending-duration (suspicion must persist N periods before transition), consecutive-check confirmation, hysteresis (separate suspect-threshold Φ_suspect and refute-threshold Φ_refute with a deadband), deadband around the phi threshold, per-domain damping so a whole failure domain doesn't flip together (ids-topology correlation), and an oscillation log that backs off re-probing for repeatedly flapping peers.

### 4. Threat Model

| Threat | Vector | Mitigation (design-level) |
|--------|--------|---------------------------|
| Spoofed suspicion/confirmation | Carrier delivers forged `Suspect`/`Confirm` datagrams | ed25519 signatures over suspicion/confirmation envelopes; unverified datagrams dropped |
| Replay of old suspicion | Captured datagram re-injected later | Nonce + Idstime timestamp in every datagram; replay cache with bounded window |
| False-positive storm | Slow carrier/GC pause triggers mass suspicion | Phi margin, indirect-prober confirmation, damping/hysteresis, suspicion-before-proven |
| False-negative silence | Partitioned carrier hides real failure | Bounded period T, gossip anti-entropy re-probing, failure-domain correlation via ids-topology |
| Oscillation cascade | Flapping node triggers membership churn | Deadband thresholds, pending duration, per-domain damping, oscillation backoff |
| Carrier flooding via suspicion | Adversary sends high-rate suspicion datagrams | Per-peer suspicion rate budgets, datagram size caps, ed25519 verification budget |
| Carrier misidentifies timeout as failure | No carrier-aware distinction | All failure reasoning happens inside the crate over opaque `Endpoint` datagrams; carrier never interprets payload |
| No networking assumptions | std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/wireless | Forbidden by design; `ids-carrier::Endpoint` is the sole traffic path |

All detection messages are authenticated with ed25519; optional payload confidentiality with chacha20poly1305/xchacha20poly1305; member-state integrity via blake3/sha256 digests. No other algorithms are permitted.

### 5. Integration with Existing Crates

ids-failure-detector is crate #7; per the dependency rule all crates 2–100 depend on `ids-000-aa`. `ids-crypto` is the only crate not depending on `ids-000-aa`. Integration points:

- **ids-000-aa:** `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `FailureDomain`, sealed error contracts, prelude. Detector verdict types are built on these.
- **ids-carrier:** `send_datagram`/`receive_datagram` over `Endpoint` — the only I/O the crate performs. No direct carrier imports beyond `ids-carrier::Endpoint`.
- **ids-crypto:** ed25519 sign/verify for signed suspicion/confirmation; blake3/sha256 for state digests; chacha20poly1305/xchacha20poly1305 for optional encryption; nonce management for replay protection.
- **ids-time:** `Idstime` ticks drive phi inter-arrival measurement and protocol period T; drift tolerance absorbed here, never wall-clock.
- **ids-topology:** `FailureDomain` placement informs correlated-suspicion grouping and domain-level damping.
- **ids-membership (#8):** suspected/proven verdicts feed node lifecycle transitions (active → degraded → failed → retired).
- **ids-fault (#27):** verdicts feed per-peer circuit-breaker decisions.
- **ids-recovery (#25):** proven failure triggers node/H-unit/region recovery.
- **ids-swim (#75) / ids-phi-accrual (#76):** created in later phases; ids-failure-detector contains equivalent SWIM + phi modules internally and may delegate to them later (engineering decision below).
- **ids-chaos (#43):** injects carrier silence/delay/reordering as chaos experiments targeting the detector.
- **ids-observability (#44):** exports φ values, suspicion/confirmation rates, false-positive/negative counters, oscillation events.

### 6. Proposed File Structure (~90–1000 files in subfolders)

Following the "separate crates instead of more folders in existing ones" rule and the HX-241 ~90–1000-file subfolder convention:

```
ids-failure-detector/
├── Cargo.toml
├── README.md
├── LICENSE
├── src/
│   ├── lib.rs
│   ├── types.rs          — DetectorVerdict, SuspicionLevel, PhiValue, FailureEvidence, ProbeResult
│   ├── error.rs          — sealed error contract
│   ├── event.rs          — FailureDetectorEvent (suspected/proven/refuted/oscillated)
│   ├── config.rs         — PhiConfig, SwimConfig, CompletenessProfile, AccuracyProfile
│   ├── clock.rs          — Idstime adapter (ids-time)
│   ├── prelude.rs
│   ├── swim/
│   │   ├── mod.rs
│   │   ├── member.rs           — Member record: NodeId, status, incarnation, endpoint
│   │   ├── member_list.rs      — Local membership table with statuses alive/suspected/dead/confirmed
│   │   ├── incarnation.rs      — Incarnation numbers for suspicion refutation
│   │   ├── probe.rs            — Ping / Ping-Ack / indirect ping construction
│   │   ├── scheduler.rs        — Protocol period T, random + round-robin target selection
│   │   ├── indirect.rs         — k indirect probers, confirmation quorum logic
│   │   ├── dissemination.rs    — Gossip suspicion dissemination, piggyback on ping/ack
│   │   ├── states.rs           — alive → suspected → dead state machine + refutation
│   │   └── datagram.rs         — SWIM datagram envelopes over Endpoint
│   ├── phi/
│   │   ├── mod.rs
│   │   ├── detector.rs         — Per-peer phi accrual state
│   │   ├── history.rs          — Sliding window of inter-arrival times (Idstime)
│   │   ├── distribution.rs     — Mean/variance estimation, normal CDF approximation
│   │   ├── phi_value.rs        — φ = -log10(1 − F(timeSinceLastHeartbeat))
│   │   ├── threshold.rs        — Conviction threshold Φ, load-based adjustment
│   │   ├── pause_gate.rs       — GC-pause / heartbeat-loss classifier
│   │   └── load.rs             — System-load phi adjustment inputs (ids-resource hints)
│   ├── properties/
│   │   ├── mod.rs
│   │   ├── completeness.rs     — Strong/weak completeness contracts
│   │   ├── accuracy.rs         — Strong/weak/eventual accuracy contracts
│   │   ├── fd_classes.rs       — P, S, ◊S, ◊W, FP class selection
│   │   ├── proven.rs           — Suspected vs proven failure, evidence rules
│   │   └── evaluation.rs       — Runtime completeness/accuracy evaluation
│   ├── detection/
│   │   ├── mod.rs
│   │   ├── engine.rs           — Combines SWIM + phi into verdicts
│   │   ├── suspect.rs          — Suspicion state machine
│   │   ├── verdict.rs          — Suspected / proven / alive verdicts
│   │   ├── oscillation.rs      — Damping, hysteresis, pending duration, deadband
│   │   └── classifier.rs       — Heartbeat loss vs GC pause classification
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── carrier.rs          — ids-carrier::Endpoint wrapper: send/receive opaque datagrams
│   │   ├── envelope.rs         — Signed suspicion/confirmation datagram envelopes
│   │   └── targeting.rs        — Peer selection over Endpoint (no IP/TCP/UDP)
│   ├── crypto/
│   │   ├── mod.rs
│   │   ├── signatures.rs       — ed25519 sign/verify for suspicion/confirmation
│   │   ├── hashing.rs          — blake3 + sha256 member-state digests
│   │   ├── nonce.rs            — Replay-protection nonces
│   │   └── seal.rs             — Optional chacha20poly1305/xchacha20poly1305 encryption
│   ├── state/
│   │   ├── mod.rs
│   │   ├── peer_state.rs       — Per-peer suspicion history and phi state
│   │   ├── suspicion_table.rs  — Cluster-wide suspicion map
│   │   ├── proven_failures.rs  — Confirmed failure registry
│   │   └── oscillation_log.rs  — Flap history for backoff
│   └── metrics/
│       ├── mod.rs
│       ├── fd_metrics.rs       — phi distribution, suspicion rate, false +/- counters
│       └── histogram.rs        — Inter-arrival time histogram
├── tests/ (~30 files)
│   ├── swim_protocol_tests.rs
│   ├── phi_accrual_tests.rs
│   ├── completeness_accuracy_tests.rs
│   ├── suspected_vs_proven_tests.rs
│   ├── gc_pause_vs_loss_tests.rs
│   ├── false_positive_negative_tests.rs
│   ├── oscillation_tests.rs
│   ├── replay_protection_tests.rs
│   ├── signature_verification_tests.rs
│   ├── carrier_agnostic_tests.rs   — Endpoint-only traffic, no std::net
│   ├── idstime_integration_tests.rs
│   ├── topology_integration_tests.rs
│   ├── membership_integration_tests.rs
│   ├── recovery_integration_tests.rs
│   ├── chaos_injection_tests.rs
│   ├── determinism_tests.rs
│   ├── fuzz/
│   │   ├── swim_fuzz.rs
│   │   └── phi_fuzz.rs
│   └── conformance/
│       ├── chandra_toueg_conformance.rs
│       ├── swim_conformance.rs
│       └── phi_conformance.rs
├── benches/ (~10 files)
│   ├── phi_calculation_bench.rs
│   ├── swim_detection_latency_bench.rs
│   ├── gossip_dissemination_bench.rs
│   ├── suspicion_throughput_bench.rs
│   └── signature_verification_bench.rs
├── docs/ (~15 files)
│   ├── architecture.md
│   ├── swim_protocol.md
│   ├── phi_accrual.md
│   ├── chandra_toueg_properties.md
│   ├── threshold_tuning.md
│   ├── carrier_agnostic_constraint.md
│   ├── crypto_constraints.md
│   ├── integration_with_ids_membership.md
│   ├── integration_with_ids_recovery.md
│   ├── oscillation_control.md
│   ├── observability.md
│   └── acceptance_criteria.md
└── targets/ (test fixtures, golden vectors)
```

Planned file count: ~15 manifest/docs + ~48 src + ~30 tests + ~10 benches + ~15 docs + fixtures ≈ 120 files, within the ~90–1000 subfolder target and organized by subsystem rather than cramming into existing crates.

### 7. Key Algorithms & Data Structures

1. **SWIM probe loop:** every protocol period T, scheduler selects target (random or round-robin), emits `Ping` over `ids-carrier::Endpoint`; on `Ping-Ack` within window, member stays `alive`; on silence, dispatch `k` indirect probers; indirect-prober agreement upgrades `suspected` toward confirmed.
2. **Gossip dissemination:** suspicion/alive/dead transitions piggyback on outgoing ping/ack datagrams; epidemic spread reaches O(log N) periods cluster-wide; anti-entropy re-probing repairs divergence.
3. **Phi accrual:** sliding window (last N inter-arrival samples in `Idstime` ticks); running mean/variance; normal CDF F; φ = -log10(1 − F(timeSinceLastHeartbeat)); conviction when φ ≥ Φ; margin absorbs occasional lost heartbeats (GC pauses).
4. **Chandra-Toueg evaluation:** configurable completeness class (strong/weak) and accuracy class (strong/weak/eventual); runtime evaluation against observed suspicion/proven sets.
5. **Suspected vs proven:** `Suspected` = φ ≥ Φ or SWIM indirect silence (hint only, no recovery); `Proven` = indirect-prober quorum agreement or STONITH evidence; only `Proven` advances ids-membership to `failed` and triggers ids-recovery.
6. **Oscillation control:** pending duration D, consecutive-check count C, hysteresis deadband (Φ_suspect > Φ_refute), per-domain damping, exponential backoff on repeat flappers.
7. **Authenticated envelopes:** ed25519 signature over (sender NodeId, payload, nonce, Idstime timestamp); blake3/sha256 digest of member-state for gossip anti-entropy; optional chacha20poly1305/xchacha20poly1305 envelope encryption.
8. **Data structures:** `MemberList` (NodeId → Member), `SuspicionTable` (NodeId → φ history), `ProvenFailureRegistry`, `OscillationLog`, `ReplayNonceCache`.

### 8. Dependencies

- `ids-000-aa` — types, sealed trait contracts, invariants, error contracts (required; master crate).
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` (required; sole traffic path).
- `ids-crypto` — ed25519, blake3, sha256, chacha20poly1305, xchacha20poly1305 (required; only allowed crypto set).
- `ids-time` — `Idstime` proprietary tick basis (required; no wall-clock).
- `ids-topology` — `FailureDomain` for correlated-suspicion grouping (optional feature-gated integration).

No `std::net`, `tokio::net`, libp2p, or any carrier crate beyond `ids-carrier`. Dev/test dependencies (proptest, criterion) limited to test-only targets with no networking. Cargo.toml will carry an explicit audit rule: prohibited-dependency check fails the build if any forbidden crate appears.

### 9. Measurable Acceptance Criteria

- **Carrier-agnostic:** zero references to `std::net`, `tokio::net`, libp2p, IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh; all traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Verified by dependency audit + source grep.
- **Crypto bound:** only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 appear in `ids-crypto`-bound code paths; any other crypto dependency fails CI.
- **Time basis:** all phi inter-arrival measurements and protocol period T keyed to `ids-time::Idstime`; no `std::time::SystemTime`/wall-clock in failure-reasoning paths.
- **SWIM:** probe period T configurable; expected detection within O(k) periods; gossip dissemination reaches quorum in O(log N) periods; suspicion precedes death (alive → suspected → confirmed).
- **Phi:** φ = -log10(1 − F(t)) implemented; sliding window size N configurable; threshold Φ configurable (paper sweet spot 8 ≤ Φ); heartbeat margin absorbs ≥1 missed heartbeat without conviction.
- **Completeness/accuracy:** strong/weak completeness and strong/weak/eventual accuracy exposed as selectable contracts; runtime evaluation reports property satisfaction per failure domain.
- **Suspected vs proven:** suspicion alone never triggers ids-recovery; proven failure requires indirect-prober quorum or STONITH evidence; refutation on `Ping-Ack` returns node to `alive` (respecting `dead_node_identity`).
- **Oscillation:** no more than C flips per window D before backoff; hysteresis deadband between suspect/refute thresholds enforced.
- **False rates:** false-positive and false-negative counters exposed via ids-observability; per-domain budgets configurable and enforced by test vectors.
- **Security:** all suspicion/confirmation datagrams ed25519-signed; replay nonces enforced; unsigned datagrams dropped.
- **Tests:** property-based (proptest), adversarial (chaos silence/delay/reordering), conformance (Chandra-Toueg classes, SWIM, phi), determinism, fuzz targets for swim + phi.

### 10. Remaining Uncertainties

- Final `Idstime` drift tolerance vs phi window sizing: how much clock drift the proprietary time basis absorbs before φ estimates distort (depends on ids-time phase-005 details not yet fixed).
- Whether ids-failure-detector internal SWIM/phi modules are later factored into `ids-swim` (#75) / `ids-phi-accrual` (#76) or remain internal; dependency ordering (crate 7 precedes crates 75/76) argues for internal implementation now, delegation later.
- Proven-failure evidence standard: indirect-prober quorum size k vs failure-domain STONITH via ids-stonith (#82) — to be coordinated with ids-membership (#8).
- GC-pause classifier thresholds (pause margin vs phi spike) need production telemetry from ids-observability before final values.
- Interaction with ids-identity (#4) RotatingNodeId: suspicion keyed to `StableNodeId` while RNI rotates; refutation across rotation windows needs care (idle-only rotation enforcement).

### 11. Engineering Decisions Summary

- **Compose SWIM + phi internally** in crate 7 rather than depend on later-numbered `ids-swim`/#75 and `ids-phi-accrual`/#76, preserving crate creation order; delegate later if factored out.
- **Carrier-agnostic datagram-only:** all detection traffic is opaque `Endpoint` payloads; the crate never opens sockets or interprets carrier topology.
- **ed25519-authenticated suspicion** with nonce + Idstime timestamps; optional chacha20poly1305/xchacha20poly1305 encryption; blake3/sha256 digests — nothing outside the allowed crypto set.
- **Idstime clock** for all timing; no wall-clock or HLC in detection paths.
- **Suspected ≠ proven:** suspicion prepares (avoid new writes), only proven failure triggers membership `failed` and recovery.
- **Oscillation control by default:** pending duration, hysteresis deadband, per-domain damping, flapper backoff — on by default, tunable.
- **Completeness/accuracy as contracts:** strong/weak variants exposed and selectable per dataset, acknowledging no perfect detector exists in an asynchronous, carrier-unreliable system.

### 12. Sources

1. Das et al., "SWIM: Scalable Weakly-consistent Infection-style Process Group Membership Protocol" — https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf
2. Wikipedia, "SWIM Protocol" — https://en.wikipedia.org/wiki/SWIM_Protocol
3. Brian Storti, "SWIM: The scalable membership protocol" — https://www.brianstorti.com/swim/
4. Hayashibara et al., "The φ Accrual Failure Detector" (IS-RR-2004-010 / SRDS 2004) — https://dspace.jaist.ac.jp/dspace/bitstream/10119/4784/1/IS-RR-2004-010.pdf
5. Akka, "Phi Accrual Failure Detector" (φ = -log10(1 − F(timeSinceLastHeartbeat))) — https://doc.akka.io/libraries/akka-core/current/typed/failure-detector.html
6. Apache Pekko, "Phi Accrual Failure Detector" — https://pekko.apache.org/docs/pekko/1.3/typed/failure-detector.html
7. Chandra & Toueg, "Unreliable Failure Detectors for Reliable Distributed Systems" (JACM 43(2):225–267, 1996) — https://courses.csail.mit.edu/6.852/08/papers/CT96-JACM.pdf
8. Yale PineWiki, "FailureDetectors" (completeness/accuracy classes) — https://www.cs.yale.edu/homes/aspnes/pinewiki/FailureDetectors.html
9. UT Austin CS380D notes (strong/weak completeness and accuracy definitions) — https://cs.utexas.edu/~lorenzo/corsi/cs380d/past/03F/notes/11-18.pdf
10. Lifeguard: Local Health Awareness for More Accurate Failure Detection (arxiv:1707.00788) — https://arxiv.org/html/1707.00788v2
11. Tarantool SWIM suspicion component (false-positive protection, suspected-before-dead) — https://lists.tarantool.org/tarantool-patches/0810d2237700cc59a23559f083cd238c1a88d8ec.1556116199.git.v.shpilevoy@tarantool.org
12. Tarantool SWIM module reference (alive/suspected/dead status) — https://tarantool.io/docs/tarantool/en/3_x/reference/reference_lua/swim/
13. Apple swift-cluster-membership (unreachable status between suspect and dead) — https://github.com/apple/swift-cluster-membership/blob/main/Sources/SWIM/SWIMInstance.swift
14. "Building a SWIM Gossip Membership Protocol from Scratch in Rust" (incarnation refutation, ChaCha20-Poly1305) — https://medium.com/@cb7chaitanya/building-a-swim-gossip-membership-protocol-from-scratch-in-rust-e82558d9fe99
15. Cassandra `phi_convict_threshold` deep dive (digitalis.io) — https://digitalis.io/post/understanding-phi-convict-threshold-in-apache-cassandra-a-deep-dive-into-failure-detection
16. "The Neutralizer: a self-configurable failure detector" (false-positive/false-negative tightrope) — https://doi.org/10.1002/cpe.1338
17. UT Austin gossip/anti-entropy notes — https://cs.utexas.edu/~lorenzo/corsi/cs395t/04S/notes/gossipnotes.pdf
18. Webalert, "Alert Flapping: Detection, Dampening, and Hysteresis" — https://web-alert.io/blog/alert-flapping-detection-taming-unstable-alerts
19. CRATES.md — IDS crate inventory (ids-failure-detector = crate #7; ids-swim #75; ids-phi-accrual #76; carrier-agnostic and crypto rules)
20. prompts.txt entry 7 (Phase 007 verbatim prompt) — C:/idfs/prompts.txt
21. HX-241 plan format references — C:/hx-241/plans/plan-077.md, plan-080.md, plan-121.md
