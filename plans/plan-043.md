## Phase 043 — Create the ids-chaos crate: Controlled fault injection for resilience validation.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 640-654)

```
43. Research only do not read plan.md **[Phase 043 — Create the ids-chaos crate: Controlled fault injection for resilience validation.]**

Crate: ids-chaos
Purpose: Controlled fault injection for resilience validation.
Contents: Fault injection harness, packet loss/reordering/duplication, network corruption, partition injection, node churn, resource exhaustion, Byzantine behavior simulation, chaos experiment execution, resilience metrics collection.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 043 — Create the ids-chaos crate: Controlled fault injection for resilience validation.]**

Create plan-043.md in C:/idfs/plans/ for the ids-chaos crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 043 creates `ids-chaos`, the IDS crate that owns controlled fault injection for resilience validation. It provides a fault injection harness covering packet loss/reordering/duplication, network corruption, partition injection, node churn, resource exhaustion, Byzantine behavior simulation, chaos experiment execution, and resilience metrics collection. The crate is a **resilience-validation harness**, not a runtime fault injector for production: it injects faults into controlled test scenarios (benchmarks, conformance runs, soak tests, chaos game days) and collects metrics so IDS maintainers can verify the system degrades gracefully rather than fails catastrophically.

`ids-chaos` sits above `ids-000-aa` (canonical types + sealed trait contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). All injected-fault traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; consumed via type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-chaos` crate does NOT yet exist at `C:/idfs/ids-chaos/`. `ids-000-aa` already declares canonical types `NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType` ∈ {ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption}, `IdsTime`, `CarrierType`, `Endpoint` — all referenced by this crate but never redefined. `ids-000-aa` also holds placeholder `src/chaos/` and `src/fault/` directories (empty) and a sealed `FaultTolerance` contract (`src/contracts/fault.rs`) — referenced, never redefined here.

Research basis: 30 parallel searches returned Gremlin (enterprise fault injection platform, application-level fault injection via ALFI, halt-and-rollback experiments); Chaos Mesh (Kubernetes-native chaos platform, CRD-defined experiments: pod-kill, network-delay/loss/duplication/corruption/partition, I/O delay/errno, time, stress, kernel; controller-manager + chaos-daemon + sidecar architecture); LitmusChaos (CNCF chaos project, Operator + CRDs: ChaosEngine/ChaosExperiment/ChaosResult/ChaosSchedule/Workflow; ChaosCenter UI); Pumba (Docker/containerd/Podman network chaos via Linux netem/iptables: delay, loss, duplicate, corrupt, rate limiting; marker/stateful loss via 4-state Markov model); Toxiproxy (TCP proxy with "toxics": latency, bandwidth, timeout, reset_peer, slo; HTTP control API; deterministic-by-default for CI); s3chaos (RustFS fault injection + recovery verification: disk I/O errors, network partitions, pod kills, resource stress, quorum loss; smoke/chaos/DM scenario suites; artifact run roots); chaos-engineering-rs (cross-platform Rust CLI/library for fault injection; injector/target/schedule/duration/SLO/target-count/parallelism/blast-radius policy bounded); malcolm (Rust chaos library: seeded deterministic replay, power-law/Pareto fault distributions, Lyapunov sensitivity scoring, Byzantine fault primitives, no_std-compatible core, zero unsafe); faultkit (Rust: fail the Nth call to a specific operation, zero-cost when inactive, probabilistic/persistent/count-based triggers); chaos-rs (Rust: deterministic block-layer simulator for crash-consistency/fault injection: torn/lost writes, bit flips, corrupt blocks); madsim / moonpool (Rust deterministic simulation testing for distributed systems: logical time, seeded RNG, simulated network/disk, replay bit-for-bit); Jepsen (Clojure fault-injection framework: nemesis injects partitions, process pauses/crashes, clock errors, power loss, disk errors; histories + checkers for linearizability); Netflix Chaos Monkey / Simian Army / FIT (random instance termination, availability-zone/region outages, failure injection testing); AWS FIS (managed fault injection: termination, failover, CPU/memory stress, throttling, latency, packet loss; stop conditions/rollback); SequenceFI (non-intrusive temporal fault injection for microservices: temporal guards from traces, message-level send/receive observation); QUANTAS 2 (Byzantine simulator: crash, equivocation, message rewriting, message withholding, local-computation replacement via programmable hooks); ByzzBench (BFT protocol testing: controlled nondeterminism in concurrency/network/process faults); aws.amazon.com / Azure blogs on chaos engineering (steady state → hypothesis → inject → validate; FIS integration with CI/CD); Microsoft/BoundedContext chaos testing (deterministic vs nondeterministic chaos; injector in separate process); dsh-tool-chaos (deterministic fault injection: deny/error/delay/abort/block actions; seeded, replayable, dry-run by default); fault injection experiment design (what/where/when/how; model-based framework IEEE 5261017); stress-ng (CPU/memory/IO/VM stressors for resource exhaustion injection).

---

### 2. Architecture Overview

`ids-chaos` is layered so each fault class is an independent module behind a single experiment orchestrator. It is a **guard-and-injector** layer: experiments declare a fault scenario, the harness validates blast-radius policy, injects the fault at the carrier/Endpoint boundary or into a subsystem seam, runs the scenario, observes metrics, then rolls the fault back. `ids-chaos` does not store records itself; it wraps or intercepts datagrams flowing to/from `ids-carrier::Endpoint` and injects faults into copies, never mutating production state without an explicit, bounded experiment scope.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    EXPERIMENT DRIVER (ids-testing, ids-bench, soak runs)     │
│  Declares scenario: fault type, target endpoint/node, rate, duration, scope │
├──────────────────────────────────────────────────────────────────────────────┤
│                 Phase 043: ids-chaos (THIS CRATE — harness layer)            │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ ││
│  │ fault injector │ │ experiment     │ │ blast radius   │ │ resilience   │ ││
│  │ (loss/reorder/ │ │ executor       │ │ policy (scope, │ │ metrics      │ ││
│  │  dup/corrupt/  │ │ (setup→inject→ │ │  rate, duration│ │ collector    │ ││
│  │  partition/    │ │  observe→rollback)│ │  max blasts)   │ │ (latency/    │ ││
│  │  churn/exhaust│ │                │ │                │ │  error/      │ ││
│  │  /byzantine)   │ │                │ │                │ │  recovery)   │ ││
│  └────────────────┴────────────────┴────────────────┴──────────────┘ ││
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ ││
│  │ deterministic  │ │ rollback/      │ │ carrier        │ │ audit log    │ ││
│  │ replay (seed)  │ │ recovery       │ │ adapter        │ │ (signed)     │ ││
│  │                │ │ (restore state)│ │ (Endpoint only)│ │              │ ││
│  └────────────────┴────────────────┴────────────────┴──────────────┘ ││
└──────────────────────────────────────────────────────────────────┬───────────┘
                                                                   │ opaque Vec<u8> datagrams
┌────────────────────────────────────────────────────────────────▼────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType ≤ 1024 B addr, Vec<u8>}, payload)      │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a chaos experiment:** (1) Driver submits a `ChaosScenario` (fault type, target `Endpoint`, rate, duration, blast-radius budget, seed) to the experiment executor. (2) The executor checks blast-radius policy (max concurrent blasts, max targets, duration ceiling, excluded endpoints) before any injection. (3) The fault injector wraps the carrier adapter: outbound/inbound `Vec<u8>` datagrams to the target `Endpoint` pass through fault primitives (loss, reorder, duplicate, corrupt, partition, delay) or subsystem seams (node churn: pause/resume a node; resource exhaustion: memory/CPU/disk pressure via stress-ng-style stressors; Byzantine: equivocation, message rewriting, withholding). (4) Resilience metrics collector records latency, error rate, recovery time, and whether steady-state invariants held during injection. (5) On experiment end (or abort via stop condition), rollback restores all fault state; the fault log is sealed with an ed25519 signature and hash-chained. (6) All traffic remains opaque `Vec<u8>` to `ids-carrier::Endpoint`; `ids-chaos` never touches a carrier directly.

**Fault classes:**
- **Packet loss** — Bernoulli or 4-state Markov (bursty) drop of datagrams to a target `Endpoint`; independent egress/ingress control.
- **Packet reordering** — hold-back buffer releases datagrams out of order within a reorder window.
- **Packet duplication** — deliver a datagram N times (1 ≤ N ≤ cap).
- **Network corruption** — bit-flip / byte overwrite / checksum-invalid payloads (BLAKE3/SHA-256 mismatch detectable by `ids-integrity`).
- **Partition injection** — drop all datagrams between a node/endpoint set and the rest (simulated link failure).
- **Node churn** — pause/resume a node (simulated crash/reboot), add/remove endpoints from membership view.
- **Resource exhaustion** — CPU starvation, memory pressure, disk-full, file-descriptor exhaustion (stress-ng-style stressors, no real hardware damage).
- **Byzantine behavior simulation** — equivocation (two conflicting messages at same position), message rewriting, message withholding, replay of old datagrams.
- **Latency injection** — bounded delay on datagrams (fixed + jitter), simulating long RTT / link degradation.
- **Experiment lifecycle** — setup → inject → observe → rollback, each phase audited; stop conditions (SLO breach) auto-abort and roll back.

---

### 3. Detailed Research Dimensions

#### 3.1 Fault injection harness design (what/where/when/how)

Fault injection answers four questions per experiment: **what** fault model (loss, reorder, duplicate, corrupt, partition, churn, exhaustion, Byzantine), **where** to inject (which `Endpoint`/node/subsystem seam), **when** to activate (rate, duration, schedule, trigger condition), and **how** to observe/rollback (metrics collection, stop conditions, state restoration). The IEEE model-based framework (A Model Based Framework for Specifying and Executing Fault Injection Experiments, 5261017) structures experiments around these four axes; `ids-chaos` adopts them verbatim as its scenario schema. Every scenario carries a seed for deterministic replay (s3chaos smoke/chaos/DM suites; malcolm seeded replay; dsh-tool-chaos seeded, replayable, dry-run-by-default), so a failure that appears under seed S replays bit-for-bit until fixed. The injector runs in a separate process/thread boundary from the system under test (boundedcontext chaos-testing practice) so a crashing injector cannot corrupt the target.

#### 3.2 Packet loss / reordering / duplication (network fault primitives)

Gremlin's Packet Loss experiment drops a percentage of packets (or corrupts them) targeted by host/port, with correlation for burstiness; Chaos Mesh `network-loss` / `network-duplication` / `network-delay` / `network-corrupt` / `network-partition` operate per-pod via CRDs; Pumba `netem` manipulates outgoing traffic with Linux `tc` (delay, loss, corruption, duplication, rate limiting) and `iptables` for incoming, including 4-state Markov loss (burst/gap states). `ids-chaos` expresses the same primitives over `ids-carrier::Endpoint` datagrams: a fault layer intercepts `send_datagram`/`receive_datagram` and applies Bernoulli/Markov loss, reorder-window buffering, duplication count, and bit-flip corruption. No `std::net`/`tokio::net`/IP/TCP/UDP — only opaque `Vec<u8>` to `Endpoint`.

#### 3.3 Network corruption (bit flip, byte overwrite, checksum mismatch)

`ids-chaos` corruption faults flip bits or overwrite bytes in datagram payloads (probability p per byte, or targeted offset range). Because payloads are content-addressed (`Checksum` in `ids-000-aa` over BLAKE3/SHA-256) and `ids-integrity` runs end-to-end checksums/Merkle scrubbing, corrupted datagrams are detectable on receipt; the chaos harness measures whether the receiver detects, discards, and recovers from corruption without cascading failure. This mirrors Prooflog / ATL right-to-erasure patterns (payload may be unreadable but hash/tombstone remain) and the ixdp bitflip fault-injection tool (single-bit flip at random payload position).

#### 3.4 Partition injection (simulated link / node failure)

Partition injection drops all datagrams between a set of endpoints and the rest of the system for the experiment duration, simulating network partition or node isolation. Chaos Mesh `network-partition` blocks 100% ingress/egress via network policy; Jepsen nemeses cut networks into halves or isolate random nodes. In `ids-chaos`, a partition fault wraps the carrier adapter for the target `Endpoint` set and drops datagrams in both directions; the harness observes whether the partitioned side degrades gracefully (local reads/writes where policy permits) and whether healing (partition removal) triggers convergence without divergence or zombie resurrection. Regional isolation (IDS §18.3) is the production analogue.

#### 3.5 Node churn (pause/resume, add/remove endpoints)

Node churn simulates nodes joining, pausing (crash), resuming (reboot), or leaving the cluster. ACM "Minimizing churn in distributed systems" (1159913.1159931) studies churn cost (dropped messages, inconsistency, bandwidth); `ids-chaos` models churn as endpoint lifecycle events: `pause(endpoint)` drops all traffic and freezes processing; `resume(endpoint)` restarts it; `add/remove(endpoint)` changes the membership view seen by `ids-membership`/`ids-discovery`. The harness measures recovery: missed updates, replayed operations, reconvergence. `ids-failure-detector` (SWIM/phi accrual) suspicion states are the natural observation target.

#### 3.6 Resource exhaustion (CPU, memory, disk, file descriptors)

Resource exhaustion simulates running out of a physical subsystem: CPU saturation, memory pressure (OOM), disk-full, fd exhaustion. stress-ng provides 220 stressors (CPU, VM, IO, FS, scheduler); the Resilium Labs chaos newsletter documents memory/CPU/disk exhaustion as common, easy injections that simulate denial-of-service locally. `ids-chaos` implements bounded resource-pressure faults (no real hardware damage; bounded duration; rollback restores resources) and observes graceful degradation: backpressure (`ids-backpressure`), circuit breakers (`ids-circuit-breaker`), bulkheads (`ids-bulkhead`), load shedding. The exhaust fault is scoped to one node/endpoint and bounded by blast-radius policy.

#### 3.7 Byzantine behavior simulation (equivocation, rewriting, withholding)

Byzantine faults model malicious or arbitrary-failure nodes: equivocation (same sender, same position, conflicting content), message rewriting (field-level tampering), message withholding (drop selective datagrams), local-computation replacement. QUANTAS 2 provides explicit fault strategies attachable to peers via programmable hooks (direct unicast, multicast, receive, local computation); ByzzBench controls nondeterminism in concurrency/network/process faults for BFT testing; Kleppmann's "Making CRDTs Byzantine fault tolerant" (PaPoC '22) shows convergence needs deterministic validation predicates under Byzantine participants. `ids-chaos` simulates Byzantine endpoints at the `ids-carrier::Endpoint` boundary (opaque datagrams can be rewritten/dropped/duplicated by the injector) and measures whether `ids-byzantine`/`ids-authenticated-broadcast` (ed25519-signed, Dolev-Strong) contain the damage. Byzantine simulation here is strictly test-harness scope — no production Byzantine node behavior.

#### 3.8 Chaos experiment execution (lifecycle, stop conditions, rollback)

An experiment follows the chaos-engineering scientific method (AWS/Azure/Harness principles): identify steady state + SLOs; hypothesize; inject controlled failure with minimal blast radius; validate whether steady state holds. Harness (Chaos Studio / Harness) structures experiments as faults + probes + actions, with a resilience score (fault weight × probe success %). Gremlin provides automatic halt and rollback when metrics exceed SLIs/SLOs; Chaos Mesh rolls back injected faults on experiment end/abort. `ids-chaos` adopts: scenario = fault(s) + probes (steady-state checks) + stop conditions (SLO breach auto-aborts, rolls back); executor runs setup → inject → observe → rollback; every phase is audited and sealed. Rollback restores datagram paths, node state, and resource pressure to pre-experiment.

#### 3.9 Resilience metrics collection (latency, error rate, recovery, steady-state)

Metrics quantify resilience: operation latency (p50/p95/p99) during fault, error rate, recovery time (MTTR — mean time to recovery), steady-state violation count, throughput under fault, bytes lost/duplicated/corrupted, partition healing time. AWS FIS integrates with CloudWatch alarms; Chaos Mesh exposes experiment status/results via dashboard; s3chaos records run artifacts (qualification-plan.json, qualification-result.json, validation-summary.tsv, logs, health observations). `ids-chaos` writes a resilience metrics record per experiment run: fault type, target, rate, duration, seed, start/end `IdsTime`, SLOs, probes passed/failed, recovery time, steady-state violations, and a sealed ed25519-signed audit entry. These feed `ids-observability` (Phase 044) and `ids-bench` (Phase 048) dashboards.

#### 3.10 Deterministic replay and fault distribution (seeded, power-law/Pareto, Lyapunov)

Reproducibility is non-negotiable: every experiment uses a seed so the exact fault sequence replays (madsim/moonpool deterministic simulation; malcolm seeded deterministic replay; faultkit count-based triggers). Fault timing can follow power-law/Pareto distributions (real-world failures are bursty, not Poisson — malcolm) and Lyapunov sensitivity scoring ranks which faults most destabilize the system (guide experiment prioritization). SequenceFI shows temporal guards derived from traces prevent premature/multiple injections. `ids-chaos` scenarios are seeded, replayable, dry-run-capable by default (dsh-tool-chaos pattern), with optional probabilistic distributions for soak runs.

---

### Threat Model

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|
| Chaos experiment harms production system | Availability / data integrity | Blast-radius policy (max targets, rate, duration, excluded endpoints); dry-run by default; stop conditions auto-abort + rollback; injector isolated from target |
| Injector itself crashes / hangs | Test harness reliability | Separate process/thread boundary; watchdog timeout on inject phase; rollback on injector exit |
| Fault state leaks after experiment | Residual degradation | Rollback restores datagram paths/node state/resource pressure; sealed audit log proves restoration |
| Non-deterministic faults hide bugs | Reproducibility | Seeded RNG; deterministic replay bit-for-bit; dry-run mode reveals exact injection plan before execution |
| Overly aggressive exhaustion faults | Hardware/OS stress | Bounded duration; no real hardware damage; scoped to one endpoint; stress-ng-style stressors capped |
| Byzantine simulation mistaken for production behavior | Trust model confusion | Clearly test-harness scope only; Byzantine faults never enabled outside experiment; `ids-byzantine` is the production counterpart |
| Metrics collection overhead skews results | Measurement fidelity | Lightweight probes; metrics sampled, not traced per-datagram by default; overhead budget per experiment |
| Carrier bypass (faults not applied to all paths) | Coverage gap | All carrier-agnostic traffic funnels through `ids-carrier::Endpoint` adapter; fault layer sits at the adapter boundary; audit proves which paths were faulted |

The carrier is never trusted: injected faults are applied to opaque `Vec<u8>` datagrams at the `ids-carrier::Endpoint` boundary, so they survive any carrier re-framing, and the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the fault-injection trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-chaos ||
|-------|------|---------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Endpoint`, `OpRef`, `OrderingPos`, `Error` — consumed, never redefined; `src/chaos/`/`src/fault/` placeholder dirs and `FaultTolerance` contract referenced |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | Fault injection happens at the `Endpoint` adapter boundary; all faulted traffic is `Vec<u8>` datagrams; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Seals experiment audit logs; signs fault scenarios; content hashes for corrupted-datagram detection |
| `ids-testing` (Phase 047) | Property-based, adversarial, conformance testing; chaos tests | Consumes `ids-chaos` harness for chaos/soak test scenarios |
| `ids-bench` (Phase 048) | Performance benchmarking | Resilience metrics from chaos runs feed bench dashboards (recovery time, latency under fault) |
| `ids-observability` (Phase 044) | Metrics collection, tracing, audit trail | Chaos experiment metrics and sealed audit entries flow into observability |
| `ids-failure-detector` (Phase 007) | SWIM / phi accrual failure detection | Observation target during churn/partition experiments |
| `ids-circuit-breaker` (Phase 027) / `ids-bulkhead` / `ids-backpressure` | Failure isolation and graceful degradation | Validated during resource-exhaustion and partition experiments |
| `ids-integrity` (Phase 030) | End-to-end checksums, Merkle trees, scrubbing | Detects corruption faults; `Checksum` reuse |
| `ids-membership` / `ids-discovery` | Node/H-unit/region membership and gossip discovery | Membership-view changes during churn experiments |
| `ids-byzantine` / `ids-authenticated-broadcast` | BFT consensus, authenticated broadcast | Production counterpart to Byzantine simulation; validated against simulated Byzantine endpoints |
| `ids-replication` | Replication streams and checkpoints | Partition/churn experiments measure replication lag and recovery |

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-chaos/` (a separate crate, not a folder inside an existing crate; one file per concern):

```
ids-chaos/
  Cargo.toml
  README.md
  src/
    lib.rs                      # crate root, feature gates, public re-exports
    errors.rs                   # ChaosError (thiserror): injection, policy, rollback, metric errors
    types.rs                    # ChaosScenario, FaultType, BlastRadius, ExperimentResult, ResilienceMetric
    executor.rs                 # experiment executor: setup → inject → observe → rollback
    scenario.rs                 # scenario parsing/validation (fault type, target, rate, duration, seed, stop conditions)
    policy.rs                   # blast-radius policy (max targets, rate, duration, excluded endpoints)
    injector.rs                 # fault injection entrypoint, dispatches to fault modules
    faults/
      mod.rs
      loss.rs                   # Bernoulli / Markov packet loss
      reorder.rs                # packet reordering buffer
      duplicate.rs              # packet duplication
      corrupt.rs                # bit-flip / byte overwrite corruption
      partition.rs              # endpoint set partition (drop both directions)
      churn.rs                  # node pause/resume/add/remove
      exhaustion.rs             # CPU/memory/disk/fd pressure
      byzantine.rs              # equivocation, rewriting, withholding, replay
      latency.rs                # bounded delay (fixed + jitter)
    carrier_adapter.rs          # wraps ids-carrier::Endpoint send/receive with fault layer
    replay.rs                   # seeded deterministic replay (seed → fault sequence)
    metrics/
      mod.rs
      collector.rs              # resilience metrics: latency, error rate, recovery time, steady-state
      recorder.rs               # per-experiment metrics record
      reporter.rs               # output for ids-observability / ids-bench
    audit.rs                    # sealed audit log (hash chain + ed25519 signature) per experiment
    rollback.rs                 # state restoration to pre-experiment
    contracts/                  # sealed trait contracts mirroring ids-000-aa style (~10 files)
      injector_contract.rs, scenario_contract.rs, policy_contract.rs, metrics_contract.rs,
      audit_contract.rs, rollback_contract.rs, fault_module_contract.rs, executor_contract.rs,
      carrier_adapter_contract.rs, mod.rs
    invariants/                 # compile-time/runtime invariants (~8 files)
      blast_radius_bounded.rs, dry_run_by_default.rs, rollback_restores.rs, seed_determinism.rs,
      carrier_agnostic.rs, crypto_allowlist.rs, no_production_leak.rs, stop_condition_works.rs
  tests/                        # (~25 files)
  benches/                      # (~8 files)
  docs/                         # (~14 files)
```
Total: ~90–1000 files across `src/`, `src/faults/`, `src/metrics/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, each concern a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. No stubs: every fault module has policy, injection, and rollback logic plus tests.

---

### 7. Key Algorithms & Data Structures

- **ChaosScenario** = { fault: FaultType, target: Endpoint, rate: f64, duration: IdsTime, seed: u64, blast_radius: BlastRadius, stop_conditions: Vec<StopCondition>, probes: Vec<Probe> }
- **BlastRadius** = { max_targets: u64, max_rate: f64, max_duration: IdsTime, excluded_endpoints: Vec<Endpoint>, dry_run: bool }
- **FaultType** ∈ {Loss, Reorder, Duplicate, Corrupt, Partition, Churn, Exhaustion, Byzantine, Latency}
- **Loss algorithm:** Bernoulli drop with probability p; optional 4-state Markov (receive / burst-receive / burst-loss / gap-loss) for bursty loss; correlation parameter.
- **Reorder algorithm:** hold-back buffer; release datagrams out of order within window W; sequence by arrival index.
- **Duplicate algorithm:** deliver datagram N times (1 ≤ N ≤ cap); independent per-datagram probability.
- **Corrupt algorithm:** bit-flip at random payload position with probability p per byte; optional targeted offset range; detectable via `Checksum` (BLAKE3/SHA-256) mismatch.
- **Partition algorithm:** drop all datagrams (both directions) between endpoint set S and complement; duration-bounded; auto-heal on experiment end.
- **Churn algorithm:** pause(endpoint) freezes processing + drops traffic; resume(endpoint) restarts; add/remove changes membership view.
- **Exhaustion algorithm:** bounded CPU/memory/disk/fd pressure via in-process stressors; duration capped; rollback releases pressure.
- **Byzantine algorithm:** equivocation (two conflicting datagrams at same position), rewrite (field tampering), withhold (selective drop), replay (re-send old datagram).
- **Latency algorithm:** fixed delay + jitter per datagram; bounded so experiments terminate.
- **Executor:** setup (capture baseline + dry-run reveal) → inject (activate fault layer at carrier adapter) → observe (collect metrics, check stop conditions each tick) → rollback (restore all fault state; seal audit).
- **Replay:** seed → PRNG → deterministic fault sequence; bit-for-bit reproducible.
- **Audit:** hash-chained log of experiment phases; ed25519-signed seal on completion/abort.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Endpoint`, `OpRef`, `OrderingPos`, `Error`) and sealed trait contracts (`FaultTolerance`, `Carrier`).
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of scenarios, metrics, audit entries.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No real hardware-damaging stressors (bounded, in-process simulation only).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|---|-----------|-------------|
| AC-43.1 | Fault injection harness covers all 9 fault classes | Unit test per fault class: loss, reorder, duplicate, corrupt, partition, churn, exhaustion, Byzantine, latency each injects and recovers |
| AC-43.2 | All traffic via `ids-carrier::Endpoint`; zero `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Grep audit across `src/` |
| AC-43.3 | Only allowed crypto (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) in crate | Grep audit of crypto surface |
| AC-43.4 | Deterministic replay: same seed → identical fault sequence | Run scenario twice with same seed; compare injected-fault traces byte-identical |
| AC-43.5 | Dry-run by default: injection plan revealed before activation | Scenario with `dry_run: true` produces no datagram modification |
| AC-43.6 | Blast-radius policy enforced: max targets/rate/duration, excluded endpoints never faulted | Inject beyond budget → rejected; excluded endpoint → no fault applied |
| AC-43.7 | Stop conditions auto-abort and roll back | SLO breach during inject → experiment aborts, state restored, audit sealed |
| AC-43.8 | Rollback restores pre-experiment state | Post-rollback datagram paths/node state/resource pressure equal baseline (diff = 0) |
| AC-43.9 | Resilience metrics collected per experiment | Metrics record: latency p50/p95/p99, error rate, recovery time, steady-state violations, sealed with ed25519 |
| AC-43.10 | Corruption detectable via `Checksum` mismatch | Corrupted datagram fails BLAKE3/SHA-256 verification on receipt |
| AC-43.11 | Partition heals without divergence | Post-partition removal: replicas converge; no zombie resurrection |
| AC-43.12 | Byzantine simulation contained | Simulated Byzantine endpoint cannot cause safety violation outside experiment scope |

---

### 10. Remaining Uncertainties

1. **Fault seam granularity:** Inject at the `ids-carrier::Endpoint` boundary (datagram level) vs. deeper subsystem seams (e.g., inside `ids-store` write path). Datagram-level is carrier-agnostic by construction; subsystem-level gives finer control but requires per-crate hooks. Decision deferred to integration with Phase 047 `ids-testing`.
2. **Resource exhaustion implementation:** In-process bounded stressors (CPU loop, memory allocation cap, disk fill simulation) vs. external process injection. In-process is safer and portable; external is more realistic. In-process chosen for this crate; external delegation noted as future work.
3. **Stop-condition latency:** How quickly an SLO breach is detected and abort triggered (tick interval trade-off: responsiveness vs. overhead). Tunable parameter, unmeasured.
4. **Byzantine simulation scope:** Which Byzantine behaviors (equivocation/rewrite/withhold/replay) are modeled vs. left to `ids-byzantine` production testing. All four simulated here at harness scope; validation against production BFT is integration work.
5. **Metrics overhead:** Per-datagram metrics collection could dominate latency under high-rate faults. Sampling budget per experiment to be validated.
6. **Interaction with `ids-failure-detector`:** Churn/partition experiments will trigger SWIM/phi-accrual suspicion; the expected suspicion/convergence behavior under chaos is undefined until `ids-failure-detector` contract is fixed.
7. **Experiment concurrency:** Whether multiple experiments can run simultaneously against different endpoints; blast-radius accounting across concurrent experiments is un-designed.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate crate `ids-chaos`, not a folder inside an existing crate | User directive: separate crates for new capabilities; keeps fault-injection harness isolated from production crates |
| Fault injection at `ids-carrier::Endpoint` adapter boundary | Carrier-agnostic by construction; opaque `Vec<u8>` datagrams; no network stack in trust base |
| Seeded deterministic replay + dry-run by default | Reproducibility (s3chaos, malcolm, dsh-tool-chaos); safety (plan revealed before activation) |
| Blast-radius policy + stop conditions + automatic rollback | Prevents chaos experiments from harming production (Gremlin halt-and-rollback; Chaos Mesh rollback) |
| 9 fault classes covering the prompt's contents list | Directly maps to ids-chaos Contents: loss/reorder/dup, corruption, partition, churn, exhaustion, Byzantine, experiment execution, metrics |
| Resilience metrics per experiment (latency, error rate, recovery, steady-state) | Feeds `ids-observability` and `ids-bench`; quantifies resilience |
| Sealed audit log (hash chain + ed25519) per experiment | Tamper-evident record of what was injected, when, and that rollback occurred |
| Only allowed crypto; no networking stack | Complies with IDS carrier-agnostic and crypto allowlist rules |
| One file per concern (faults/, metrics/, contracts/, invariants/) | Matches "~90-1000 files in subfolders" format and the separate-modules directive |

---

### Sources

[^1]: Gremlin — Fault Injection platform (hosts, containers, services, serverless; halt and rollback; observability integration): https://gremlin.com/technologies/fault-injection
[^2]: Gremlin — Application-Level Fault Injection (ALFI), targeted chaos experiments via language library: https://infoq.com/news/2018/10/gremlin-alfi
[^3]: Chaos Mesh — Cloud-native Chaos Engineering platform for Kubernetes; CRDs: PodChaos, NetworkChaos (delay/loss/duplication/corruption/partition), IOChaos, TimeChaos, StressChaos, KernelChaos; controller-manager + chaos-daemon + sidecar: https://chaos-mesh.org/docs
[^4]: LitmusChaos — CNCF chaos project; Operator + CRDs (ChaosEngine, ChaosExperiment, ChaosResult, ChaosSchedule, Workflow); ChaosCenter UI: https://docs.litmuschaos.io/
[^5]: Pumba — Chaos testing and network emulation for Docker/containerd/Podman via Linux netem/iptables (delay, loss, duplicate, corrupt, rate; 4-state Markov loss): https://github.com/alexei-led/pumba
[^6]: Toxiproxy — TCP proxy with "toxics" (latency, bandwidth, timeout, reset_peer, slo); HTTP control API; deterministic for CI: https://github.com/Shopify/toxiproxy
[^7]: s3chaos — RustFS fault injection + recovery verification (disk I/O, network partitions, pod kills, resource stress, quorum loss; scenario suites; artifact run roots): https://github.com/rustfs/s3chaos
[^8]: chaos-engineering-rs — Cross-platform Rust CLI/library for fault injection; injector/target/schedule/duration/SLO/target-count/parallelism/blast-radius policy bounded: https://github.com/NLlemain/chaos-engineering-rs
[^9]: malcolm — Rust chaos library: seeded deterministic replay, power-law/Pareto fault distributions, Lyapunov sensitivity scoring, Byzantine primitives, no_std core, zero unsafe: https://lib.rs/crates/malcolm
[^10]: faultkit — Rust: fail the Nth call to an operation; zero-cost when inactive; probabilistic/persistent/count triggers: https://docs.rs/faultkit
[^11]: chaos-rs — Rust deterministic block-layer simulator for crash-consistency/fault injection (torn/lost writes, bit flips, corrupt blocks): https://github.com/Alpy16/chaos-rs
[^12]: madsim — Magical Deterministic Simulator for distributed systems in Rust (logical time, seeded RNG, simulated network/disk, bit-for-bit replay): https://github.com/madsim-rs/madsim
[^13]: moonpool — Deterministic simulation testing for distributed systems in Rust (provider traits, logical time, simulated network/disk): https://github.com/PierreZ/moonpool
[^14]: Jepsen — Clojure fault-injection framework; nemesis (partitions, process pauses/crashes, clock errors, power loss, disk errors); histories + checkers (linearizability): https://jepsen.io/
[^15]: Netflix Chaos Monkey / Simian Army / FIT — random instance termination, availability-zone/region outages, failure injection testing; stop conditions/rollback: https://netflix.github.io/chaosmonkey/
[^16]: AWS Fault Injection Simulator (FIS) — managed fault injection (termination, failover, CPU/memory stress, throttling, latency, packet loss); CloudWatch stop conditions; CI/CD integration: https://aws.amazon.com/fis/
[^17]: AWS / Azure chaos engineering blogs — steady state → hypothesis → inject → validate; FIS in CD pipelines: https://aws.amazon.com/blogs/architecture/verify-the-resilience-of-your-workloads-using-chaos-engineering
[^18]: SequenceFI — Non-intrusive temporal fault injection for microservices; temporal guards from traces; message-level send/receive observation: https://arxiv.org/html/2607.20050
[^19]: QUANTAS 2 — Abstract/Concrete and Byzantine simulator; crash, equivocation, message rewriting, withholding, local-computation replacement via hooks: https://arxiv.org/html/2605.08547
[^20]: ByzzBench — BFT protocol testing benchmark; controlled nondeterminism in concurrency/network/process faults: https://github.com/joaomlneto/byzzbench
[^21]: stress-ng — CPU/memory/IO/VM/FS/scheduler stressors (220 stressors) for resource exhaustion injection: https://www.mankier.com/1/stress-ng
[^22]: ACM "Minimizing churn in distributed systems" — churn cost (dropped messages, inconsistency, bandwidth): https://dl.acm.org/doi/10.1145/1159913.1159931
[^23]: IEEE 5261017 — Model-based framework for specifying/executing fault injection experiments (what/where/when/how): https://ieeexplore.ieee.org/document/5261017
[^24]: dsh-tool-chaos — Deterministic fault injection (deny/error/delay/abort/block); seeded, replayable, dry-run by default: https://github.com/cyanseek/dsh-tool-chaos
[^25]: BoundedContext — Chaos testing; deterministic vs nondeterministic chaos; injector in separate process: https://boundedcontext.com/chaos-testing
[^26]: Resilium Labs chaos newsletter — failure injection history; resource exhaustion (CPU/memory/disk) as common injection: https://newsletter.resiliumlabs.com/p/chaos-engineering-part-3-61579e41edd8
[^27]: ixdp — Fault injection: corrupting/dropping packets (bitflip at random payload position): https://blog.inlab.net/posts/ixdp-fault-injection-corrupting-packets
[^28]: Gremlin — Packet Loss experiment docs (percentage loss/correlation, host/port targeting): https://gremlin.com/docs/fault-injection-experiments-packetloss
[^29]: Harness Chaos Studio — experiments = faults + probes + actions; resilience score (fault weight × probe success %); halt/rollback: https://developer.harness.io/resilience-testing/chaos-testing/experiments/chaos-studio-experiments
[^30]: udptest — Go fault-injection proxy for UDP (loss, duplicate, reorder, delay, swallow direction) with DirectionStats: https://pkg.go.dev/github.com/XGC-Team/xgc2-xrpc/go/udpx/udptest

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, modules, fault classes, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 043.
