## Phase 034 — Create the ids-compute crate: Analytics and transformations near data.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 505-519)

"34. Research only do not read plan.md **[Phase 034 — Create the ids-compute crate: Analytics and transformations near data.]**"

Crate: ids-compute
Purpose: Analytics and transformations near data.
Contents: Compute placement, authorization constraints, resource quotas, data-use policy enforcement, result compaction (return compact results, not entire datasets), execution isolation, compute-to-data scheduling.

"Create plan-034.md in C:/idfs/plans/ for the ids-compute crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources."

Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).

Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

ids-000-aa is the master crate with all canonical types and sealed trait contracts.

Research only — no code, no source modifications, only the plan document.

---

### 1. Executive Summary

Phase 034 creates `ids-compute`, the IDS crate for compute-to-data execution: running analytics and transformations at the data's location rather than moving data to compute. It owns compute placement, authorization constraints, resource quotas, data-use policy enforcement, result compaction (return compact results, not entire datasets), execution isolation, and compute-to-data scheduling. The crate operates as a local compute engine that ships workload code to data nodes, executes near the stored data, and returns only compact results — preserving data sovereignty, privacy, and bandwidth. `ids-compute` is carrier-agnostic: all traffic rides opaque `Vec<u8>` datagrams over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address). Allowed crypto only: ed25519 (signed compute policies and result attestations), xchacha20poly1305/chacha20poly1305 (workload payload encryption), sha256/blake3 (content addressing, job digests). All types and sealed trait contracts come from `ids-000-aa` (the master crate). Research only — no code written, no files created beyond this plan document.

---

### 2. Architecture Overview

`ids-compute` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — compute placement, authorization constraints, resource quotas, data-use policy enforcement, result compaction, execution isolation, compute-to-data scheduling — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-* LAYER (compute consumers)             │
│  ids-analytics, ids-sync, ids-replication, ids-write — submit compute jobs  │
├─────────────────────────────────────────────────────────────────────────────┤
│                    Phase 034: ids-compute (THIS CRATE)                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │\n│  │ compute      │ │ authorization│ │ resource     │ │ data-use     │    │ │\n│  │ placement    │ │ constraints  │ │ quotas       │ │ policy       │    │ │\n│  │ (data-local) │ │ (capability) │ │ (cgroup/RLIMIT│ │ (ODRL-style) │    │ │\n│  │              │ │              │ │ enforcement)  │ │ enforcement) │    │ │\n│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │\n│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │\n│  │ result       │ │ execution    │ │ compute-to-    │ │ scheduler    │    │ │\n│  │ compaction   │ │ isolation    │ │ data scheduling│ │ (DAG/queue)  │    │ │\n│  │ (aggregates) │ │ (Wasm/TEE)   │ │ (push-down)    │ │              │    │ │\n│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │\n│  ┌─────────────────────────────────────────────────────────────────────┐ │ │\n│  │ job lifecycle: submit → place → authorize → quota → isolate →     │ │ │\n│  │ execute → compact → return result (Vec<u8>)                       │ │ │\n│  └─────────────────────────────────────────────────────────────────────┘ │ │\n└─────────────────────────────────────────────────────────────┬───────────────┘\n                                                              │ opaque Vec<u8> datagrams\n┌─────────────────────────────────────────────────────────────▼───────────────┐\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │\n│  send_datagram(Endpoint{CarrierType, Vec<u8>≤1024}, payload: &[u8])        │\n│  receive_datagram → (Endpoint, Vec<u8>)                                    │\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │\n└─────────────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `OrderingPos`, `Version`, `SnapshotId`, `JobId`, `ComputeNodeId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`) imported from `ids_000_aa::X`, never redefined.
- `ids-compute` implements the sealed `trait Compute` contract from `ids-000-aa/src/contracts/` (place, authorize, quota, enforce_policy, compact, isolate, schedule) as declared in the master crate; downstream crates use it as a trait bound, cannot implement it.
- All compute metadata (job specs, policies, results) transported as opaque bytes via `ids-carrier::Endpoint`. No IP addresses, ports, sockets.
- Crypto surface: ed25519 (signed compute policies, result attestations), xchacha20poly1305/chacha20poly1305 (workload payload encryption), sha256/blake3 (job digests, content addressing). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Compute-to-data: code ships to data; results return compact. Never ship entire datasets through `Endpoint`.
- Execution isolation: untrusted workloads run in sandboxed contexts (Wasm-style capability model documented; TEE/attestation optional layer).
- Resource quotas: CPU/memory/IO/pids limits per job via cgroup-style constraints; enforced at placement/admission.
- Data-use policy enforcement: ODRL-style permissions/prohibitions/duties evaluated before execution; workload rejected if policy violated.
- Result compaction: aggregation/pushdown/filtering applied before return; only compact results leave the data node.

---

### 3. Detailed Research Dimensions

#### 3.1 Compute-to-Data Execution Model

Research basis:
- Compute-over-data (Expanso / Bacalhau): instead of moving massive datasets to a central processing hub, send lightweight compute jobs directly to the data's source. Job runs locally; only small lightweight results sent back. Reduces network overhead, latency, egress costs; improves privacy/governance since sensitive data never leaves its environment.
- Data gravity: larger datasets make every copy more expensive; compute, applications, services pull toward data concentrations. Scheduling/placement should send jobs to environments already holding their data.
- Bacalhau architecture: orchestrator schedules tasks based on resource availability, data location, job requirements; compute nodes pull images, mount/fetch input data locally, run tasks in isolated environments. Single binary, orchestrator + compute node roles. NATS as messaging backbone (note: IDS replaces this with `ids-carrier::Endpoint`).
- Data sovereignty: process sensitive data within security boundaries without requiring it to leave premises. Cross-organizational computation: allow specific vetted computations on protected datasets without exposing raw data.
- Implication for `ids-compute`: job spec = lightweight code + input references + expected output shape; placement = schedule on node holding input data; return = compact result only.

#### 3.2 Compute Placement and Data Locality Scheduling

Research basis:
- Data-locality-aware task scheduling (DynDL, Appl. Sci. 2018): schedule computational tasks near input data considering server free time, data placements, transfer costs. Offline algorithm quadratic time, optimal; online variant adapts. 30% better processing time vs non-locality scheduling.
- Network-aware scheduling (Corral, SIGMETRICS 2017): jointly optimize data placement and compute task placement; offline planner + cluster scheduler; most small jobs run within one rack.
- Locality-aware reduce task scheduling (LoNARS, IEEE 2015): combine data locality with network traffic awareness; schedule reduce tasks near map task outputs.
- NEAL (IEEE TPDS 2021): cross-layer network-aware locality scheduling for join/aggregation; reduces communication time.
- GEODIS (2017): geo-distributed data center job scheduling; data locality + inter-DC transfer; heuristic reduces makespan 44% vs state of the art.
- Push-down optimization (arxiv 2601.17546): execute transformation logic inside the database engine rather than ETL tool; source-side/target-side/full push-down; filters/projections/aggregations close to data origin.
- SQL pushdown (Starburst/BigQuery): move filters, aggregations, projections into source systems; column pruning + filter pushdown reduce data scanned/transferred.
- Implication for `ids-compute`: placement module selects compute nodes by data locality score; supports push-down of filters/projections/aggregations into storage layer before result return.

#### 3.3 Authorization Constraints (Capability-Based Access Control)

Research basis:
- SciTokens (ACM PEARC '18): capability-based access to remote scientific data. Identity tokens authenticate users at submit time; capability tokens grant specific data access rights to jobs, limiting exposure if stolen. OAuth-compliant, short-lived access tokens delivered to jobs at execution time.
- Capability-based authorization for HEP: restricted token gives access to limited set of authorizations (e.g., read certain paths from certain hosts), not full identity authority.
- SADRA (arxiv 2609.31119): distributed capability-based access control for resource-disaggregated architectures; linked process/compute/resource capabilities; delegation attenuates authority; resource-side enforcement rejects stale capabilities.
- OPA/ABAC (OPA docs, osohq): attribute-based access control using subject/resource/environment attributes; Rego policy language; decouples policy decision from enforcement.
- XACML (OASIS/NIST SP 800-178): XML-based ABAC standard; subject/resource/action/environment attributes; rules/policies/policiesets with combining algorithms (permit/deny-unless-permit).
- Implication for `ids-compute`: authorization module uses capability tokens (signed, scoped, short-lived) plus ODRL-style policy attributes; enforcement point intercepts compute requests before execution; all policy decisions versioned and auditable.

#### 3.4 Resource Quotas

Research basis:
- Linux cgroups (v1/v2): limit CPU time (cpu.max, quota/period), memory (memory.max, swap), IO bandwidth, pids; hard limits via quota/period, soft via memory.high; OOM kill on memory breach.
- Kubernetes ResourceQuotas: per-namespace aggregate limits on cpu/memory requests/limits, hugepages, pod/job counts; every pod must specify requests/limits when quota enforced.
- Nomad resource quotas: aggregate usage per namespace; allocations queued when exhausted.
- Anyscale/Ray quotas: max CPUs, instances, GPUs, accelerator types per cloud/project/user.
- Sliding-window quotas (Hadoop YARN arxiv 2606.06145): limit average allocation over configured windows; exact admission check at job submission.
- Implication for `ids-compute`: quota module enforces per-job and per-namespace CPU/memory/IO/pids limits at admission; cgroup-style hard limits; sliding-window accounting for recurring analytics; rejection/queuing when quota exhausted.

#### 3.5 Data-Use Policy Enforcement (ODRL / Usage Control)

Research basis:
- IDS Knowledge Base (Usage Control, PIP/PXP interfaces): policy restricts data usage to specific purposes/locations/time intervals; Purpose PIP resolves app-declared purposes against policy; location PIP computes part-of/subset functions; time PIP resolves current date/time; PXP executes policy-demanded actions (logging).
- ODRL (W3C): permissions (action allowed), prohibitions (action disallowed), duties (obligation when permission exercised), constraints (conditions refining when rules apply). Big Data profile (ODRL DUC): data ownership across processing phases; enforcement via CSP/process algebra translated to extended automata checked against Spark/Flink logging.
- Continuous usage control enforcement (SDS@ESWC 2024): ODRL extended with time-dependent constraints; continuous monitoring of digital asset usage; dynamic environments.
- Open Digital Rights Enforcement (ODRE): ODRL extended with enforcing features; descriptive → enforceable policies; algorithm checks restrictions, invokes actions.
- DAVE (arxiv 2602.17413): usage policy-enforcing spokesperson; virtual redaction at query time; ODRL policies integrated with Eclipse Dataspace Components; asset-level policy enforcement.
- Interoperable and Continuous Usage Control (arxiv): time-dependent ODRL constraints; enforcement at policy decision/enforcement points.
- Implication for `ids-compute`: data-use policy module expresses permissions/prohibitions/duties/constraints in ODRL-compatible form; evaluates purpose, location, time, recipient attributes before admitting a compute job; duties (logging, deletion after retention) attached to job execution.

#### 3.6 Result Compaction (Return Compact Results, Not Entire Datasets)

Research basis:
- Compute-over-data principle: only lightweight results travel back; raw data stays in place.
- Partial Partial Aggregates (arxiv 2603.26698): push only the local compute phase of an aggregate below joins; data reduction before shuffle; avoid transferring full datasets.
- Differential privacy / federated analytics (Mayfly, arxiv 2412.07962): on-device windowing + contribution bounding; streaming DP; only privatized aggregates revealed; hierarchical aggregation with increasing retention; no raw data persists.
- Data clean rooms (Databricks/Snowflake/BigQuery): partners analyze shared data in place; raw records never exposed; outputs constrained to aggregations or governed tables; data egress controls prevent raw export.
- Result compaction frameworks (TruvaG3/Haystack): shrink results to fit budget while preserving signal; full result never lost (kept in step memory), digest/summary is a view for reasoning.
- Implication for `ids-compute`: compaction module applies aggregation, filtering, projection, sampling, DP noise before return; enforces maximum result size; egress controls prevent raw dataset leakage through compute jobs.

#### 3.7 Execution Isolation (Untrusted Workloads)

Research basis:
- WebAssembly sandboxing (Wasmtime/Wasmer): bounds-checked memory, validated control flow, explicit host capabilities, Harvard architecture; deny-by-default; fast instantiation (fresh instance per task); no raw syscall access; only linked imports.
- WaVe (IEEE 2023): verifiably secure WASI runtime; static verification of memory/file/network isolation; verifies runtime sets up sandbox correctly.
- gVisor (Google 2018): userspace application kernel (Sentry) intercepts syscalls; Gofer processes handle I/O; reduced attack surface (350→68 host syscalls); defense-in-depth with seccomp/namespaces/cgroups.
- Kata Containers / microVMs (GKE Sandbox): lightweight VMs with dedicated guest kernel per pod; full hardware virtualization isolation; for untrusted code requiring full kernel access.
- 2cVM (arxiv 2606.10615): AMD SEV-SNP + Wasm component model; Commitment Manifest enumerates participants/components/permitted channels/authorized outputs; locked to VM attestation; mutual isolation between co-resident workloads.
- WASI capability model: filesystem, network, etc. access only via explicitly granted capabilities; deny-by-default.
- Implication for `ids-compute`: isolation module documents Wasm-style capability sandbox as default execution context for untrusted workloads; TEE/attestation optional for higher-assurance; no ambient access to filesystem/network/host; capabilities declared and granted explicitly.

#### 3.8 Compute-to-Data Scheduling

Research basis:
- Kubernetes Jobs/CronJobs: finite run-to-completion workloads; completions/parallelism scaling; backoffLimit/activeDeadlineSeconds; CronJob crontab schedule with concurrency policy (Allow/Forbid/Replace), startingDeadlineSeconds, history limits.
- Airflow DAG scheduler: DAG = tasks + dependencies + schedule; triggers task instances when dependencies complete; pool limits/concurrency; data-interval scheduling; asset-aware scheduling (trigger on dataset updates).
- Temporal/durable execution: distributed workflows, stateful, retry/timeout semantics; strong second across orchestrator benchmarks.
- Argo Workflows: Kubernetes-native, ephemeral pods, burst/batch CPU workloads; near-zero idle cost.
- Corral (SIGMETRICS 2017): offline planner + cluster scheduler; joint data/compute placement; small jobs within one rack.
- Implication for `ids-compute`: scheduler module manages job lifecycle (submit → place → authorize → quota → isolate → execute → compact → return); DAG of dependent compute jobs; cron-style recurring analytics; backpressure and retries; all scheduling decisions local (no network calls — state exchanged via `Endpoint` datagrams only).

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Data exfiltration via compute job (job returns entire dataset) | Result compaction: enforce max result size; aggregation/filter/projection before return; egress controls; only compact results leave data node |
| Unauthorized compute on data (no permission) | Capability-based authorization + ODRL data-use policy evaluation before admission; reject on policy violation |
| Malicious/untrusted workload escapes sandbox | Execution isolation: Wasm-style deny-by-default capability model; TEE optional; no ambient filesystem/network/host access |
| Resource starvation / DoS via runaway jobs | Resource quotas: CPU/memory/IO/pids hard limits at admission; sliding-window accounting; reject/queue when exhausted |
| Replay of stale job grant / capability | Nonce + per-job capability token; grantor rejects duplicate/out-of-order; signed grants (ed25519) prevent forgery |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; CI grep for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Workload result tampering in transit | ed25519-signed result attestations; optional xchacha20poly1305/chacha20poly1305 payload encryption |
| Job placement on untrusted node | Compute placement respects trust domains; attestation (TEE) optional for high-assurance placement; node reputation via `ids-000-aa` types |
| Policy drift / continuous usage violation | Usage control: continuous monitoring; obligations (logging, deletion) attached to job; re-evaluation on attribute change |
| Scheduling storms / thundering herd | Jittered submission times; exponential backoff on retry; circuit breakers; rate-limited job admission |
| Memory-side corruption / page cache | Documented boundary: compute isolation does not cover host memory outside sandbox; ECC/TTE recommended out of crate scope |
| Compaction lossy vs fidelity tradeoff | Configurable compaction level; full result retained at data node; compact result is a view for reasoning |

---

### 5. Integration with Existing Crates

Phase 034 depends on and integrates with:

| Crate | Role in Phase 034 |
|-------|-------------------|
| `ids-000-aa` (master) | Canonical types (`NodeId`, `DatasetId`, `PartitionId`, `RecordId`, `OrderingPos`, `Version`, `SnapshotId`, `JobId`, `ComputeNodeId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`), sealed `trait Compute` contract, invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits`), unified `Error` |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤1024); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-compute` |
| `ids-crypto` (Phase 003) | ed25519 (signed policies/attestations), xchacha20poly1305/chacha20poly1305 (workload encryption), sha256/blake3 (job digests/content addressing); consumed only |
| `ids-storage` | Persistence backend for job specs, policy stores, quota accounting, compaction results |
| `ids-policy` / data-use | ODRL-style policy evaluation; authorization constraints |
| `ids-quota` | Resource quota accounting (CPU/memory/IO/pids) |
| `ids-time` (crate 5) | `IdsTime` tick basis for job scheduling, cron triggers, quota windows |
| `ids-sync` | Consumer: compute jobs for cross-replica analytics; result compaction before sync |
| `ids-replication` | Data sources for compute placement; results shipped via carrier |
| `ids-write` | Change source triggering compute jobs on new data |

**Integration points:**
- `ids-compute` implements `ids_000_aa::contracts::Compute` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-compute` never touches sockets.
- Job specs/policies/results via sha256/blake3 digests; policies optionally signed with ed25519; workloads optionally encrypted with xchacha20poly1305/chacha20poly1305.
- Depends on `ids-000-aa` only for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (per CRATES.md dependency rule) — `ids-compute` depends on both.
- Cargo workspace: `ids-compute = { path = "../ids-compute" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-compute` (to be created at `C:/idfs/ids-compute/`)

```
ids-compute/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto; thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # JobSpec, JobId, ComputeNodeId, PlacementDecision, QuotaLimit, PolicyDecision, CompactionResult, IsolationLevel
│   ├── errors.rs                       # ComputeError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Compute (sealed trait)
│   │
│   ├── placement/                      # Compute placement (~8 files)
│   │   ├── mod.rs
│   │   ├── locality_scorer.rs          # score nodes by data locality
│   │   ├── node_selector.rs            # select compute node for job
│   │   ├── trust_domain.rs             # trust domains for placement
│   │   ├── attestation_check.rs        # optional TEE attestation for placement
│   │   ├── pushdown.rs                 # push filters/projections/aggregations to storage
│   │   ├── data_affinity.rs            # data affinity hints
│   │   ├── placement_cache.rs          # cache placement decisions
│   │   └── scheduler.rs                # placement scheduler integration
│   │
│   ├── authorization/                  # Authorization constraints (~8 files)
│   │   ├── mod.rs
│   │   ├── capability_token.rs         # signed, scoped, short-lived capability
│   │   ├── capability_issuer.rs        # issue capability tokens (ed25519)
│   │   ├── capability_verifier.rs      # verify capability before execution
│   │   ├── abac.rs                     # attribute-based access control (subject/resource/env)
│   │   ├── policy_evaluator.rs         # ODRL-style permission/prohibition/duty evaluation
│   │   ├── enforcement_point.rs        # intercept compute requests before admission
│   │   └── audit_log.rs                # authorization decisions logged
│   │
│   ├── quota/                          # Resource quotas (~8 files)
│   │   ├── mod.rs
│   │   ├── quota_limit.rs              # CPU/memory/IO/pids limits
│   │   ├── admission_controller.rs     # reject/queue when quota exhausted
│   │   ├── sliding_window.rs           # sliding-window accounting for recurring jobs
│   │   ├── cgroup_constraints.rs       # cgroup-style hard limits (documented)
│   │   ├── quota_accounting.rs         # track usage per namespace/job
│   │   ├── backpressure.rs             # backpressure when near quota
│   │   ├── reset_schedule.rs           # quota window reset
│   │   └── reject_policy.rs            # reject vs queue policy
│   │
│   ├── policy/                         # Data-use policy enforcement (~8 files)
│   │   ├── mod.rs
│   │   ├── odrl_policy.rs              # ODRL permission/prohibition/duty/constraint
│   │   ├── purpose_pip.rs              # purpose PIP (resolve app-declared purposes)
│   │   ├── location_pip.rs             # location PIP (part-of/subset)
│   │   ├── time_pip.rs                 # time PIP (current date/time)
│   │   ├── duty_tracker.rs             # duties (logging, deletion after retention)
│   │   ├── continuous_monitor.rs       # continuous usage monitoring; re-evaluate on attribute change
│   │   ├── policy_store.rs             # persist policies
│   │   └── reject_on_violation.rs      # reject job on policy violation
│   │
│   ├── compaction/                     # Result compaction (~8 files)
│   │   ├── mod.rs
│   │   ├── aggregator.rs               # aggregation before return (sum, count, avg, histogram)
│   │   ├── filter_project.rs           # filter/projection pushdown into compaction
│   │   ├── sampler.rs                  # statistical sampling for large results
│   │   ├── dp_noise.rs                 # differential privacy noise on released aggregates
│   │   ├── max_size_enforcer.rs        # enforce maximum result size
│   │   ├── egress_control.rs           # prevent raw dataset export through compute
│   │   ├── digest.rs                   # sha256/blake3 digest of compact result
│   │   └── view_vs_full.rs             # compact result is view; full result retained at data node
│   │
│   ├── isolation/                      # Execution isolation (~8 files)
│   │   ├── mod.rs
│   │   ├── wasm_sandbox.rs             # Wasm-style deny-by-default capability sandbox (documented)
│   │   ├── capability_grant.rs         # explicitly granted capabilities (filesystem, etc.)
│   │   ├── deny_by_default.rs          # no ambient access
│   │   ├── tee_attestation.rs          # optional TEE/attestation for high-assurance
│   │   ├── isolation_level.rs          # IsolationLevel enum (none/wasm/tee)
│   │   ├── instance_per_task.rs        # fresh isolation instance per task (fast instantiate)
│   │   ├── hostcall_filter.rs          # filter allowed hostcalls/syscalls
│   │   └── memory_boundary.rs          # documented memory isolation boundary
│   │
│   ├── scheduling/                     # Compute-to-data scheduling (~8 files)
│   │   ├── mod.rs
│   │   ├── job_lifecycle.rs            # submit → place → authorize → quota → isolate → execute → compact → return
│   │   ├── dag_scheduler.rs            # DAG of dependent compute jobs
│   │   ├── cron_scheduler.rs           # cron-style recurring analytics
│   │   ├── backoff_retry.rs            # exponential backoff on failure
│   │   ├── concurrency_policy.rs       # Allow/Forbid/Replace for overlapping runs
│   │   ├── deadline.rs                 # active deadline seconds per job
│   │   ├── queue.rs                    # job admission queue
│   │   └── state_machine.rs            # JobState (Submitted, Placed, Authorized, Quotaed, Isolated, Running, Compacted, Returned, Failed)
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper for compute metadata
│   ├── codec/                          # wire encoding of compute messages (~5 files)
│   │   ├── mod.rs
│   │   ├── compute_frame.rs            # version-tagged frame header
│   │   ├── job_spec_codec.rs           # serde encode/decode job specs
│   │   ├── policy_codec.rs             # policy serialization
│   │   ├── result_codec.rs             # compact result serialization
│   │   └── digest.rs                   # sha256/blake3 message digests
│   │
│   ├── validation/                     # input validation rules
│   ├── observability/                  # compute event types (job submitted, executed, compacted, rejected)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── placement_tests.rs              # data locality scoring; pushdown
│   ├── authorization_tests.rs          # capability token; ABAC; policy rejection
│   ├── quota_tests.rs                  # admission control; sliding window; backpressure
│   ├── policy_tests.rs                 # ODRL permission/prohibition/duty; continuous monitor
│   ├── compaction_tests.rs             # aggregation; filter/project; DP noise; max size; egress control
│   ├── isolation_tests.rs              # wasm sandbox; deny-by-default; capability grant
│   ├── scheduling_tests.rs             # job lifecycle; DAG; cron; backoff; state machine
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~200–320 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Compute Placement (Data Locality)

```
locality_score(node, job) = w_data * data_locality(node, job.inputs)
                          + w_trust * trust_domain_match(node, job.trust_domain)
                          + w_resource * free_resource_ratio(node, job.quota)
                          - w_transfer * estimated_transfer_cost(job.inputs, node)

placement_decision = argmax_node(locality_score(node, job))
```

Push-down: filters/projections/aggregations converted to storage-side operations before data is read; only preprocessed relevant data extracted.

#### 7.2 Authorization (Capability + ABAC)

```
capability_token = { job_id: JobId, holder: NodeId, dataset_ids: Vec<DatasetId>,
                     actions: Vec<Action>, expires_at: IdsTime, signature: ed25519::Signature }

abac_decision(subject_attrs, resource_attrs, action, env_attrs) → Permit | Deny | Indeterminate
ODRL: permission ∨ prohibition ∨ duty ∨ constraint evaluated before admission
```

#### 7.3 Resource Quotas

```
quota_check(job, namespace) → Allow | Queue | Reject
  if usage + job.request > hard_limit → Reject
  if usage + job.request > soft_limit → Queue (backpressure)
  else Allow

sliding_window: track resource rates over window W; exact admission check at submission
```

#### 7.4 Data-Use Policy Enforcement (ODRL)

```
policy_decision(job, dataset) → Permit | Deny
  check purpose ∈ allowed_purposes (purpose PIP)
  check location ⊆ allowed_location (location PIP)
  check time ∈ allowed_time_window (time PIP)
  if deny → reject job; if permit → attach duties (logging, deletion)
continuous_monitor: re-evaluate on attribute change; enforce duties post-execution
```

#### 7.5 Result Compaction

```
compact_result = aggregate(filter(project(raw_data, columns), predicate), group_by, agg_fn)
if size(compact_result) > max_size → apply sampling or DP noise
egress_control: raw dataset never leaves data node; only compact result returned
digest = sha256/blake3(compact_result) for attestation
```

#### 7.6 Execution Isolation

```
isolation_level: None | Wasm | TEE
wasm_sandbox: deny-by-default; capabilities granted explicitly; fresh instance per task;
              hostcall filter allows only granted resources; memory bounded
tee_attestation: optional remote attestation before releasing sensitive data to workload
```

#### 7.7 Compute-to-Data Scheduling (Job Lifecycle)

```
job_state_machine: Submitted → Placed → Authorized → Quotaed → Isolated → Running → Compacted → Returned
                                                              ↘ Failed (any transition)

scheduling: DAG of dependent jobs; cron triggers; backoff/retry; concurrency policy (Allow/Forbid/Replace);
            active deadline per job; admission queue with backpressure
```

#### 7.8 Data Structures

```
JobSpec { job_id: JobId, code_ref: Vec<u8>, input_refs: Vec<DatasetId>, quota: QuotaLimit,
          policy: Policy, isolation_level: IsolationLevel, expected_output_shape: OutputShape,
          deadline: IdsTime, signature: Option<ed25519::Signature} }

PlacementDecision { job_id: JobId, selected_node: ComputeNodeId, locality_score: f64,
                    pushdown_ops: Vec<PushdownOp>, ids_time: IdsTime }

CapabilityToken { job_id: JobId, holder: NodeId, dataset_ids: Vec<DatasetId>,
                  actions: Vec<Action>, expires_at: IdsTime, sig: ed25519::Signature }

QuotaLimit { cpus: f64, memory_bytes: u64, io_bps: u64, pids: u32, window_secs: u64 }

PolicyDecision { permitted: bool, duties: Vec<Duty>, constraints: Vec<Constraint>, ids_time: IdsTime }

CompactionResult { result_bytes: Vec<u8>, digest: [u8; 32], algorithm: Algo,
                   size_bytes: u64, compacted_from: u64, ids_time: IdsTime }

JobState { state: JobStateEnum, transition_history: Vec<(JobStateEnum, IdsTime)> }
```

---

### 8. Dependencies

`ids-compute/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Compute`, invariants, versioning, errors
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 (consumed only)
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — compute message serialization
- `blake3 = "1.5"` — job digests/content addressing (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling storage/policy/quota crates referenced via adapters; persistence delegation is a design option, not a hard dependency in this phase.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | Compute placement specified | `src/placement/` module; data-locality scoring; push-down of filters/projections/aggregations |
| 2 | Authorization constraints | `src/authorization/` module; capability tokens (ed25519); ABAC; enforcement point |
| 3 | Resource quotas | `src/quota/` module; CPU/memory/IO/pids limits; sliding-window accounting; reject/queue |
| 4 | Data-use policy enforcement | `src/policy/` module; ODRL permission/prohibition/duty/constraint; PIPs (purpose/location/time) |
| 5 | Result compaction | `src/compaction/` module; aggregation; filter/project; sampling; DP noise; max size; egress control |
| 6 | Execution isolation | `src/isolation/` module; Wasm-style deny-by-default; capability grant; optional TEE |
| 7 | Compute-to-data scheduling | `src/scheduling/` module; job lifecycle state machine; DAG; cron; backoff; concurrency policy |
| 8 | Sealed `trait Compute` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Compute` |
| 9 | Canonical types from ids-000-aa only | `types.rs` imports all canonical types from `ids_000_aa`; no redefinitions |
| 10 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`; all traffic via `ids_carrier::Endpoint` |
| 11 | Crypto allowlist | Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 referenced; no RSA/ECDSA/AES-GCM |
| 12 | No code written / no source modifications | Plan document only; `ids-compute/` crate not created; no edits to existing crates |
| 13 | Conformance test plan enumerated | `tests/` list covers placement, authorization, quota, policy, compaction, isolation, scheduling, carrier-agnostic, crypto allowlist |
| 14 | File structure within range | ~200–320 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|-------------|--------|-----------------|
| Compute granularity (job vs task vs function) | Affects `src/scheduling/` design and overhead | Job = unit of scheduling; task = unit of execution; function = unit of isolation |
| Persistence backend for job specs and quota accounting | Which crate owns durable compute metadata | Delegate to `ids-storage`/quota crate; adapter pattern |
| Isolation mechanism selection (Wasm vs TEE vs container) | Affects `src/isolation/` design and TCB | Wasm default for untrusted; TEE optional high-assurance layer; container out of scope |
| Result compaction lossy vs fidelity tradeoff | Determines acceptable data reduction | Configurable compaction level; full result retained at data node; compact result is view |
| Policy language (ODRL vs XACML vs Rego) | Affects `src/policy/` expressiveness | ODRL core (permissions/prohibitions/duties/constraints) as baseline; extensible |
| Sliding-window vs fixed-window quota accounting | Affects `src/quota/` admission behavior | Sliding-window for recurring analytics; fixed for one-off jobs |
| Scheduling backpressure under overload | Performance impact | Admission queue with backpressure; jittered submission; exponential backoff |
| ed25519-signed compute policies: mandatory or optional | Authenticity vs accidental-misuse scope | Optional layer; core compute is local execution enforcement |
| Push-down support heterogeneity across storage backends | Placement complexity | Capability negotiation per storage connector; fallback to local compaction |
| Offline compute allowance for partitioned holders | Bounded offline finality | Possible extension, not in scope |

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-compute` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; single-responsibility boundary |
| Compute-to-data: code ships to data, compact results return | Data gravity; reduces network transfer, egress cost, latency; preserves data sovereignty/privacy |
| Capability-based authorization + ODRL data-use policies | Fine-grained, context-aware access decisions; scope-limited capability tokens reduce blast radius if stolen |
| Resource quotas with cgroup-style hard limits + sliding-window accounting | Prevent resource starvation/DoS; fair sharing across namespaces; recurring analytics support |
| Result compaction before return | Only compact results leave data node; prevents raw dataset exfiltration through compute jobs |
| Execution isolation: Wasm-style deny-by-default sandbox | Bounds-checked memory, validated control flow, explicit host capabilities; fast per-task instantiation |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic absolute rule; no `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
| Allowed crypto: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| `ids-000-aa` holds sealed `trait Compute` and canonical types | Master crate contract; `ids-compute` implements, never redefines |
| No code in this phase | Research only per prompts.txt entry 34 |

---

### 12. Sources

1. Expanso — "What Is Compute Over Data Architecture?" — compute over data flips traditional model: send compute to data, not data to compute; data sovereignty; reduced egress. https://expanso.io/newsroom/compute-over-data-architecture
2. Expanso — "Top Benefits of Compute Over Data Explained" — data gravity; decoupled compute/storage; only code + small results move across network. https://expanso.io/newsroom/benefits-of-compute-over-data
3. Bacalhau README / Architecture / Key Concepts — distributed compute orchestrator; orchestrator schedules by data locality; compute nodes run isolated; data sovereignty; cross-organizational computation. https://github.com/bacalhau-project/bacalhau/blob/main/README.md; https://bacalhau.org/docs/overview/architecture; https://bacalhau.org/docs/overview/key-concepts
4. DynDL: Scheduling Data-Locality-Aware Tasks (Appl. Sci. 2018) — dynamic data transfer costs; offline algorithm quadratic/optimal; 30% better processing time. https://doi.org/10.3390/app8112216
5. Corral: Network-Aware Scheduling for Data-Parallel Jobs (SIGMETRICS 2017) — joint data/compute placement; offline planner + cluster scheduler. https://dl.acm.org/doi/10.1145/2829988.2787488
6. LoNARS: Locality and Network-Aware Reduce Task Scheduling (IEEE 2015) — data locality + network traffic awareness. https://ieeexplore.ieee.org/document/7017949
7. NEAL: Network-Aware Locality Scheduling (IEEE TPDS 2021) — cross-layer scheduling for join/aggregation; reduces communication time. https://research.tudelft.nl/files/88261151/neal_IEEE_TPDS_20210125.pdf
8. GEODIS: Data Locality-Aware Job Scheduling in Geo-Distributed Data Centers (2017) — heuristic reduces makespan 44%. https://link.springer.com/article/10.1007/s00607-017-0564-7
9. Push Down Optimization for Distributed Multi-Cloud Data Integration (arxiv 2601.17546) — source/target/full push-down; transformation logic inside database engine. https://arxiv.org/html/2601.17546
10. What is Query Pushdown? (Starburst) — filters/aggregations/projections pushed into source systems; column pruning + filter pushdown. https://starburst.io/blog/what-is-query-pushdown
11. Introduction to Federated Queries (BigQuery) — SQL pushdown; delegate operations to external data source. https://docs.cloud.google.com/bigquery/docs/federated-queries-intro
12. SciTokens: Capability-Based Access to Remote Scientific Data (ACM PEARC '18) — capability tokens grant specific data access to jobs; short-lived; OAuth-compliant. https://dl.acm.org/doi/10.1145/3332186.3333258
13. Capability-Based Authorization for HEP — restricted tokens for limited authorizations (read certain paths from certain hosts). https://epj-conferences.org/10.1051/epjconf/201921404014
14. SADRA: Sound Capability-Based Access Control for Resource-Disaggregated Architectures (arxiv 2609.31119) — linked process/compute/resource capabilities; delegation attenuates; resource-side enforcement. https://arxiv.org/html/2609.31119v1
15. Open Policy Agent (OPA) — general-purpose policy engine; Rego; ABAC with subject/resource/environment attributes. https://openpolicyagent.org/docs
16. ABAC with Open Policy Agent (OPA) — attribute-based access control; dynamic, context-aware decisions. https://www-osohq-com/learn/abac-with-open-policy-agent-opa
17. XACML (OASIS/NIST SP 800-178) — XML-based ABAC standard; subject/resource/action/environment; combining algorithms. https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=920189; https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-178.pdf
18. IDS Knowledge Base — Usage Control; PIP/PXP interfaces; purpose/location/time PIPs; ODRL policies. https://docs.internationaldataspaces.org/ids-knowledgebase/ids-g/usagecontrol/specification/concepts/t7_pip_and_pip_interfaces
19. ODRL Profile: Big Data (W3C) — ODRL DUC; data ownership across processing phases; enforcement via CSP/automata. https://w3c.github.io/odrl/profile-bigdata
20. Interoperable and Continuous Usage Control Enforcement in Dataspaces (SDS@ESWC 2024) — ODRL extended with time-dependent constraints; continuous monitoring. https://penni.wu.ac.at/papers/SDS@ESWC%202024%20Interoperable%20and%20Continuous%20Usage%20Control%20Enforcement%20in%20Dataspaces.pdf
21. Open Digital Rights Enforcement Framework (ODRE) — descriptive → enforceable ODRL policies; algorithm checks restrictions. https://sciencedirect.com/science/article/pii/S0167404824005881
22. DAVE: Policy-Enforcing LLM Spokesperson (arxiv 2602.17413) — virtual redaction at query time; ODRL + Eclipse Dataspace Components. https://arxiv.org/html/2602.17413
23. Partial Partial Aggregates (arxiv 2603.26698) — push local compute phase below joins; data reduction before shuffle. https://arxiv.org/html/2603.26698
24. Mayfly: Federated Analytics over Ephemeral On-Device Data Streams (arxiv 2412.07962) — on-device windowing + contribution bounding; streaming DP; only privatized aggregates revealed. https://arxiv.org/pdf/2412.07962v1
25. Differential Privacy: Have Your Data and Hide It Too (Cloudflare) — DP adds noise to aggregates; privacy-utility tradeoff; ε parameter. https://blog.cloudflare.com/have-your-data-and-hide-it-too-an-introduction-to-differential-privacy
26. Databricks Clean Rooms — governed environment; raw records never exposed; outputs constrained to aggregations/governed tables; data egress controls. https://databricks.com/blog/2022/06/28/introducing-data-cleanrooms-for-the-lakehouse
27. Snowflake Data Clean Rooms — ML Jobs; data never leaves account; hash-based approval; governed workloads. https://snowflake.com/en/blog/ml-jobs-snowflake-data-clean-rooms
28. BigQuery Data Clean Rooms — publish/subscribe model; linked datasets; egress controls prevent raw export. https://docs.cloud.google.com/bigquery/docs/data-clean-rooms
29. WebAssembly Security (Wasmtime docs) — sandboxed by design; bounds-checked memory; validated control flow; only linked imports; fast per-instance instantiation. https://github.com/bytecodealliance/wasmtime/blob/main/docs/security.md
30. WaVe: Verifiably Secure WebAssembly Sandboxing Runtime (IEEE 2023) — static verification of memory/file/network isolation. https://ieeexplore.ieee.org/document/10179357
31. gVisor: Sandboxed Container Runtime (Google 2018) — userspace application kernel (Sentry); Gofer I/O processes; 350→68 host syscalls. https://cloudplatform.googleblog.com/2018/05/Open-sourcing-gVisor-a-sandboxed-container-runtime.html
32. Kata Containers / MicroVMs (GKE Sandbox) — lightweight VMs with dedicated guest kernel per pod; full hardware virtualization. https://cloud.google.com/kubernetes-engine/docs/concepts/sandbox-pods
33. Two-Way Confidential VMs (2cVM) (arxiv 2606.10615) — AMD SEV-SNP + Wasm component model; Commitment Manifest; mutual isolation. https://arxiv.org/html/2606.10615
34. WebAssembly Component Model — language-agnostic composition; WIT interfaces; capability-based I/O isolation. https://component-model.bytecodealliance.org/
35. Kubernetes Jobs — finite run-to-completion workloads; completions/parallelism; backoffLimit; activeDeadlineSeconds. https://kubernetes.io/docs/concepts/workloads/controllers/job
36. Kubernetes CronJobs — crontab-style schedule; concurrency policy; startingDeadlineSeconds; history limits. https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs
37. Apache Airflow Scheduler / DAGs — task dependencies + schedule; pool limits; data-interval; asset-aware scheduling. https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html; https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html
38. Orchestrators Benchmark (Airflow/Argo/Windmill/Temporal) — comparison of scheduling latency, idle cost, breaking point. https://github.com/Jesshuan/Orchestrators-benchmark
39. cgroups / Control Groups (Apptainer/Kernel docs) — CPU/memory/IO/pids limits; cgroups v1/v2; hard limits via quota/period. https://apptainer.org/docs/user/main/cgroups.html; https://kernel-internals.org/cgroups
40. Kubernetes Resource Quotas — per-namespace aggregate limits on cpu/memory; every pod must specify requests/limits. https://kubernetes.io/docs/concepts/policy/resource-quotas
41. Data Sharing Agreement / Data Use Agreement templates — purpose, permissible use, term, ownership, safeguards. https://contractstandards.com/public/contracts/data-sharing-agreement; https://admindatahandbook.mit.edu/print/v1.0/handbook_ch3_DUA.pdf
42. Homomorphic Encryption for Privacy-Preserving Data Aggregation in Data Spaces (PMC) — HE enables computation on encrypted data; noise budget; FHE limitations. https://pmc.ncbi.nlm.nih.gov/articles/PMC13234483
43. Zero-Knowledge Cloud Analytics (zk-Analytics, ACM) — verifiable aggregation/query execution via ZKPs; input integrity, confidentiality, computational integrity. https://dl.acm.org/doi/pdf/10.1145/3789240.3829157
44. Zero-Knowledge Proof Based Verifiable Machine Learning (Survey) — ZKP certifies training/testing/inference results without revealing data/model. https://arxiv.org/html/2502.18535v2
45. Pinocchio: Nearly Practical Verifiable Computation (IEEE SP 2013) — succinct proofs of computation correctness; public verifiable computation. https://ieee-security.org/TC/SP2013/papers/4977a238.pdf
46. Enigma: Decentralized Computation Platform with Guaranteed Privacy (arxiv 1506.03471) — secure MPC; compute on secret-shared data without revealing inputs. https://arxiv.org/html/1506.03471v1
47. Conclave: Secure Multi-Party Computation on Big Data (arxiv 1902.06288) — hybrid MPC-cleartext query compiler for relational analytics. https://arxiv.org/pdf/1902.06288
48. IDs prompts.txt entry 34 (verbatim) — C:/idfs/prompts.txt lines 505-519 — crate definition, contents, constraints.
49. CRATES.md entry 34 — C:/idfs/CRATES.md — ids-compute: analytics and transformations near data.
50. ids-000-aa skeleton — C:/idfs/ids-000-aa/ — canonical types, sealed trait contracts, invariants, versioning, errors.

---

**End of plan-034.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**

Constraint compliance verified:
- New separate crate `ids-compute` (not folders in existing crates): yes.
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`): yes.
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305: yes.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts: yes.
- Research only — no code, no source modifications, only the plan document: yes.
- Plan matches HX-241 plan format (Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure, Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources): yes.
- Output file: `C:/idfs/plans/plan-034.md`: yes.