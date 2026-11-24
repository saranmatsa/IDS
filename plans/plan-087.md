## Phase 087 — Create the ids-ppr-reconstruction crate: Distributed erasure-coded storage repair.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1300-1314)

```
87. Research only do not read plan.md **[Phase 087 — Create the ids-ppr-reconstruction crate: Distributed erasure-coded storage repair.]**

Crate: ids-ppr-reconstruction
Purpose: Distributed erasure-coded storage repair.
Contents: PPR algorithm, partial parallel reconstruction, reduction tree aggregation, m-PPR scheduling, concurrent failure handling, network pressure reduction.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 087 — Create the ids-ppr-reconstruction crate: Distributed erasure-coded storage repair.]**

Create plan-087.md in C:/idfs/plans/ for the ids-ppr-reconstruction crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 087 creates `ids-ppr-reconstruction`, the IDS crate for distributed erasure-coded storage repair. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by repair domain. The crate implements the design surface for Partial Parallel Repair (PPR): dividing a reconstruction operation into partial operations executed in parallel across storage nodes, aggregated through a reduction tree (binomial reduction tree), plus m-PPR scheduling for concurrent failures and network-pressure-reduction mechanics.

`ids-ppr-reconstruction` does not open sockets or drive any transport directly; it computes reconstruction schedules, partial-operation trees, helper selection, and bandwidth budgets, and ships every protection-relevant message as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-ppr-reconstruction` consumes primitive IDs from `ids-000-aa`/`ids-crypto` only).

**Core results from research** (Mitra, Panta, Ra, Bagchi, "Partial-Parallel-Repair (PPR): A Distributed Technique for Repairing Erasure Coded Storage," EuroSys 2016): PPR completes network transfer in ⌈log₂(k+1)⌉ timesteps vs k timesteps for conventional (k,m) Reed-Solomon repair, reducing maximum bandwidth per server by ~50–67% depending on code params ((6,3) 50%, (8,3) 62.5%, (10,4) 60%, (12,4) 66.6%) and reconstruction time by up to 59% for RS(12,4). m-PPR schedules multiple concurrent reconstructions to minimize contention for shared network/disk resources.

A skeleton `ids-ppr-reconstruction` crate does NOT yet exist at `C:/idfs/ids-ppr-reconstruction/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, plus sealed trait contracts and error types. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-ppr-reconstruction` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a **reconstruction scheduler/planner**: given a failed chunk/stripe and a set of surviving helper nodes, it (a) selects helpers, (b) decomposes the repair into partial operations, (c) arranges participants into a reduction tree, (d) schedules timesteps and bandwidth, (e) handles concurrent failures via m-PPR, and (f) caps network pressure so foreground traffic is not starved. It never sends bytes itself; it returns a `RepairPlan` that the caller executes through `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (repair drivers)                │
│  ids-erasure, ids-recovery, ids-rebuild — trigger repair with failed chunk   │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 087: ids-ppr-reconstruction (THIS CRATE — planner)       │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │ │\n│  │ helper          │ │ partial-op      │ │ reduction-tree    │ │ m-PPR    │ │ │\n│  │ selection       │ │ decomposition   │ │ (binomial)        │ │ scheduler│ │ │\n│  │ (k helpers, d   │ │ (sub-chunks,    │ │ ⌈log2(k+1)⌉       │ │ concurrent│ │ │\n│  │  helpers)       │ │ linear combos)  │ │ timesteps           │ │ failures │ │ │\n│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │ │\\n│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │ │\n│  │ bandwidth/      │ │ failure-state   │ │                 │                │ │ │\n│  │ pressure caps   │ │ tracker         │ │                 │                │ │ │\n│  │ (foreground     │ │ (per-chunk,     │ │                 │                │ │ │\n│  │  protection)    │ │ per-node)       │ │                 │                │ │ │\n│  └─────────────────┘ └─────────────────┘ │                 │                │ │\\n│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │ │\n│  │ repair plan     │ │ audit/logging   │ │                 │                │ │ │\n│  │ emission        │ │ (signed+chained)│ │                 │                │ │ │\n│  └─────────────────┘ └─────────────────┘ │                 │                │ │\\n└─────────────────────────────────────────┬──────────────────┴──────────────┘\\n                                          │ opaque Vec<u8> datagrams\\n┌─────────────────────────────────────────▼──────────────────┐\\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only  │\\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>},  │\\n│  payload: &[u8])                                           │\\n│  receive_datagram → (Endpoint, Vec<u8>)                    │\\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP /      │\\n│  QUIC / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN    │\\n└────────────────────────────────────────────────────────────┘
```

**Data flow of a single-chunk PPR repair:** driver submits `RepairRequest{failed_chunk, stripe_id, k, m}` → `ids-ppr-reconstruction` selects k helper nodes, decomposes into partial operations, arranges them in a binomial reduction tree with ⌈log₂(k+1)⌉ timesteps, computes per-node bandwidth and pressure caps, returns `RepairPlan{partial_ops, tree, timesteps, bandwidth_caps}` → caller executes partial ops as opaque datagrams to `ids-carrier::Endpoint` targets, collectors aggregate up the tree, final reconstructed chunk delivered to requestor.

**Data flow of concurrent-failure (m-PPR) repair:** driver submits multiple `RepairRequest`s → m-PPR scheduler interleaves/schedules reconstruction jobs to minimize contention for shared network/disk IO, returns per-job `RepairPlan`s with coordinated bandwidth allocations.

**Research-verified performance characteristics:**
- PPR network transfer: ⌈log₂(k+1)⌉ timesteps vs k timesteps conventional; total transfer time ~2×C/BN for small k examples.
- PPR per-server max traffic: each participating server transfers only ⌈log₂(k+1)⌉ chunks vs k chunks in traditional repair (e.g., 4 vs 8 for RS(8,3)).
- Reported gains: up to 59% reconstruction-time reduction for RS(12,4); 8.25× degraded-read throughput improvement; m-PPR 31–47% total repair-time reduction for simultaneous failures vs traditional RS repair.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 PPR — Partial Parallel Repair (Mitra et al., EuroSys 2016)

**Source:** Purdue DCSl / AT&T Labs Research, "Partial-Parallel-Repair (PPR): A Distributed Technique for Repairing Erasure Coded Storage," EuroSys 2016. https://engineering.purdue.edu/dcsl/publications/papers/2016/final_erasurecodedrepair_eurosys16_cameraready.pdf ; https://about.att.com/ecms/dam/sites/labs_research/content/publications/SDN_Partial-Parallel-Decoding_PPR.pdf

PPR is a distributed reconstruction technique for erasure-coded storage. It divides the reconstruction of a failed chunk into partial operations and schedules them to execute in parallel on multiple servers already involved in the reconstruction. A distributed protocol progressively combines partial results, overlaying a binomial reduction tree over the helper servers. Theoretically, PPR completes network transfer in ⌈log₂(k+1)⌉ timesteps vs k timesteps needed for a conventional (k,m) Reed-Solomon repair.

Key insight: conventional repair has the requestor fetch k chunks from k helpers, congesting the requestor's incoming link. PPR distributes bandwidth usage across helper links so no single link is overloaded; even the incoming link to the destination gets approximately as much traffic as the first aggregator server.

Reported advantages (Table 1 of the paper):
- (6,3): 50% network transfer reduction, 50% max BW/server reduction
- (8,3): 50% network transfer reduction, 62.5% max BW/server reduction
- (10,4): 60% network transfer reduction, 60% max BW/server reduction
- (12,4): 66.6% network transfer reduction, 66.6% max BW/server reduction

Experimental: up to 59% reconstruction-time reduction for RS(12,4); 8.25× degraded-read throughput improvement.

#### 3.2 m-PPR — Multiple Concurrent PPR

**Source:** Same EuroSys 2016 paper, Section 5.

In a data center, multiple chunk failures occur concurrently (transient failures, maintenance, upgrades, disk failures). Naive overlapping reconstructions contend for shared network and disk IO. m-PPR schedules multiple reconstruction jobs in parallel while minimizing competition for shared resources. Each repair job uses the main PPR algorithm; m-PPR apportions load across concurrent reconstructions. Evaluation: 31–47% total repair-time reduction for simultaneous failures vs traditional RS repair (RS(12,4), 64MB chunks, BIGSITE testbed with 83 chunk servers); benefit decreases with higher numbers of simultaneous failures.

#### 3.3 Reduction tree / binomial reduction tree aggregation

**Source:** PPR paper Section 4.2 ("Reduction in network transfer time"); HPC binomial reduction tree references.

PPR overlays a tree-like reduction structure (binomial reduction tree) over servers holding relevant chunks. Reconstruction takes logarithmic timesteps (⌈log₂(k+1)⌉) instead of linear (k). Partial results are progressively combined up the tree until the requestor holds the fully reconstructed chunk. This fully utilizes available network resources and significantly reduces repair time.

#### 3.4 Repair pipelining (ECPipe) and follow-up techniques

**Source:** Li et al., "Repair Pipelining for Erasure-Coded Storage," USENIX ATC 2017; "Repair Pipelining for Erasure-Coded Storage: Algorithms and Evaluation," arXiv:1908.01527.

ECPipe partitions a chunk into slices and pipelines repair across storage nodes in units of slices; achieves O(1) single-block repair time vs O(log k) for PPR and O(k) for conventional. ECPipe integrates as middleware into HDFS and QFS. Follow-ups: PPT/PPCT (Parallel Pipeline Tree / Cross-Tree, non-uniform traffic), SMFRepair/MSRepair (bandwidth-aware scheduling), RAN (aggregation repair with programmable switches), NetEC (in-network aggregation), SelectiveEC (balanced recovery load), FastPR (proactive repair via migration + reconstruction), CORE (concurrent failure recovery for regenerating codes), InaPR/FinRepair (in-network aggregation).

#### 3.5 Regenerating codes (MSR/MBR) and repair bandwidth

**Source:** Dimakis et al. regenerating codes; Goparaju et al. "Minimum Storage Regenerating Codes for All Parameters" (IEEE TIT 2017); ACM TOS survey 2025.

Regenerating codes minimize repair bandwidth (network traffic during repair) via sub-packetization. MSR codes minimize storage overhead then repair bandwidth; MBR minimize bandwidth then storage. For single-node failures (>98% of stripe failures in Facebook's warehouse cluster), MSR can reduce repair bandwidth by close to 50% vs RS. Clay codes support general (n,k) and are state-of-the-art MSR codes. PPR is compatible with repair-friendly codes and compounds savings.

#### 3.6 Concurrent failure handling

**Source:** CORE (Li, Lin, Lee, TC 2014 / arXiv:1302.3344); m-PPR section of EuroSys 2016; survey § "Failure repair."

Concurrent failures (t failed nodes, t ≤ n−k) require reconstructing multiple lost chunks. Conventional multi-block repair takes k + f − 1 timeslots with skewed bandwidth (requestor downlink congested). PPR + m-PPR distribute load. CORE extends regenerating codes to concurrent failures with 36–64% bandwidth savings (optimal cases). PPR's m-PPR minimizes contention across simultaneous reconstructions.

#### 3.7 Network pressure reduction

**Source:** PPR paper (key insight, Section 4); Rashmi et al. arXiv:1309.0186 (Facebook warehouse cluster: >100 TB/day recovery traffic); SelectiveEC; SMFRepair/MSRepair.

PPR reduces network pressure by (i) reducing total transfer time from k to ⌈log₂(k+1)⌉ timesteps, (ii) capping max bandwidth per server to ⌈log₂(k+1)⌉ chunks vs k, (iii) distributing traffic across helper links instead of concentrating on the requestor's downlink. m-PPR adds coordinated scheduling across concurrent jobs. SelectiveEC balances recovery load via bipartite-graph task selection; SMFRepair/MSRepair use idle nodes and bandwidth-aware link scheduling in heterogeneous networks.

#### 3.8 Carrier-agnostic transport

**Source:** IDS prompts.txt entry 1 / plan-001/048/060; ids-000-aa `trait Carrier` contract.

All traffic rides `ids-carrier::Endpoint` = `CarrierType` + `Vec<u8>` (max 1024 bytes). `CarrierType`: Loopback, Serial, Optical, Radio, Custom(u16). No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. `ids-ppr-reconstruction` returns repair plans; the caller frames partial operations as opaque `Vec<u8>` datagrams to endpoints.

#### 3.9 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** RFC 8032 (Ed25519); RFC 7539/8439 (ChaCha20-Poly1305, XChaCha20-Poly1305); NIST FIPS 180-4 (SHA-256); BLAKE3 spec.

`ids-ppr-reconstruction` uses:
- ed25519: signing repair plans, reduction-tree aggregation proofs, audit checkpoints.
- sha256/blake3: chunk/stripe checksums, reduction-tree node hashes, repair plan IDs, audit-chain links.
- xchacha20poly1305: encrypt repair state at rest (24-byte nonce, random-safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based).
No other crypto. Consumed via `ids-crypto` primitive IDs from `ids-000-aa`.

#### 3.10 Failure models and fault tolerance

**Source:** PPR paper (single chunk failure ≈99% of cases; f ≤ n−k concurrent); CORE (concurrent failure patterns); regenerating codes.

`ids-ppr-reconstruction` must handle: single-chunk degraded read, single-node full recovery, f ≤ n−k concurrent chunk failures in a stripe, partial helper availability, helper failure mid-repair (fallback to alternate helpers or re-schedule), straggler helpers (timeout and re-issue), correlated failures (same rack/region). Repair scope bounded by redundancy: t ≤ n−k or data unrecoverable.

---

### Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Requestor downlink congestion (conventional repair) | Repair latency | PPR reduction tree: ⌈log₂(k+1)⌉ timesteps, traffic distributed across helpers |
| Helper link overload / incast | Network pressure | Max per-server traffic capped to ⌈log₂(k+1)⌉ chunks; m-PPR coordinates concurrent jobs |
| Concurrent reconstruction contention | Repair performance | m-PPR scheduler apportions load across jobs, minimizes shared-resource competition |
| Helper failure mid-repair | Repair completion | Alternate helper selection; re-schedule partial ops; bounded retry |
| Straggler helper | Tail latency | Timeout + re-issue to alternate helper; cancellation tokens |
| Replay of repair plan / partial op | Decision integrity | ed25519-signed repair plans; nonce-bound partial ops; bounded replay cache |
| Clock manipulation for scheduling | Causality | Logical timesteps + IdsTime monotonic ticks; never wall-clock for scheduling |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent partial-op application; idempotent reduction-tree merge |
| Unauthorized repair initiation | Access control | Repair plan signed by authorized coordinator; ed25519 verification |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |
| Disallowed crypto in repair path | Policy compliance | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305; CI allowlist test |
| Carrier-agnostic rule violation | Architecture | `Endpoint` sole address type; dependency audit bans networking crates |

The carrier is never trusted: repair plans and partial-op messages are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-ppr-reconstruction ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All repair traffic flows to/from `Endpoint`; crate returns plans, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing plans/partial ops/audit; encrypting repair state at rest; hashing chunks and tree nodes |
| `ids-erasure` | Erasure coding encode/decode | PPR operates on erasure-coded chunks; consumes erasure fragment types |
| `ids-recovery` | Recovery orchestration | Triggers PPR repairs on failure detection; consumes `RepairPlan` |
| `ids-rebuild` | Node rebuild | Full-node recovery uses PPR per-chunk; m-PPR for concurrent chunks |
| `ids-fragment` | Record fragments/shards | Fragment reads/writes governed by parent record's redundancy; PPR repairs fragments |
| `ids-store` | Primary record store | Source of surviving chunks for repair; checksum verification on reconstructed chunks |

**Integration points:** `ids-ppr-reconstruction` is a pure planner: it emits `RepairPlan` structures; callers (`ids-recovery`, `ids-rebuild`) execute via `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what contracts expose. Never touches crate internals — public contracts only.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-ppr-reconstruction/` (a separate crate, not a folder inside an existing crate):

```
ids-ppr-reconstruction/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # crate root, public API surface
├── constants.rs                        # PPR defaults: k, m, timestep caps, bandwidth budgets
├── error.rs                            # RepairError (thiserror): scheduling, tree, bandwidth, crypto errors
├── types.rs                            # RepairPlan, PartialOp, TreeNode, Timestep, BandwidthCap (re-exports ids-000-aa types)
├── repair_request.rs                   # RepairRequest, failed chunk/stripe identification
├── helper_selection.rs                 # k-helper selection, d-helper alternates, topology diversity
├── ppr_decompose.rs                    # Partial operation decomposition from stripe + erasure params
├── reduction_tree.rs                   # Binomial reduction tree: ⌈log₂(k+1)⌉ timesteps, node hashes
├── timestep_scheduler.rs               # Timestep assignment, per-timestep partial-op batches
├── m_ppr.rs                            # m-PPR concurrent-job scheduler, load apportionment
├── bandwidth_caps.rs                   # Per-server/per-link bandwidth caps, foreground protection
├── pressure_controller.rs              # Network-pressure reduction: caps, throttling, backpressure
├── failure_tracker.rs                  # Per-chunk, per-node failure state during repair
├── straggler_handler.rs                # Timeout, re-issue, cancellation of slow helpers
├── repair_audit.rs                     # Signed repair plan emission, audit-chain entry
├── contracts/                          # sealed trait contracts (~12 files)
│   ├── mod.rs
│   ├── repair_planner.rs               # trait RepairPlanner
│   ├── ppr_contract.rs                 # trait PPR (partial-op decomposition + tree)
│   ├── m_ppr_contract.rs               # trait MPPR (concurrent scheduling)
│   ├── helper_contract.rs              # trait HelperSelection
│   ├── bandwidth_contract.rs           # trait BandwidthCaps
│   ├── pressure_contract.rs            # trait PressureController
│   ├── failure_contract.rs             # trait FailureTracker
│   ├── straggler_contract.rs           # trait StragglerHandler
│   ├── audit_contract.rs               # trait RepairAudit
│   ├── carrier_contract.rs             # trait RepairCarrierAdapter (decision carrier only)
│   ├── tree_contract.rs                # trait ReductionTree
│   └── timestep_contract.rs            # trait TimestepScheduler
├── invariants/                         # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── redundancy_scope.rs             # t ≤ n−k (data recoverable)
│   ├── tree_logarithmic.rs             # ⌈log₂(k+1)⌉ timestep bound
│   ├── bandwidth_cap_sound.rs          # per-server cap ≤ budget
│   ├── carrier_agnostic.rs             # no std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # only the five allowed algorithms
│   ├── idempotent_merge.rs             # reduction-tree merge idempotency
│   ├── monotonic_timesteps.rs          # logical timesteps, no wall-clock
│   ├── plan_signature.rs               # ed25519-signed repair plans
│   ├── audit_chain_sound.rs            # hash-chained audit entries
│   └── mod.rs
├── plan/                               # repair plan types (~6 files)
│   ├── mod.rs
│   ├── repair_plan.rs
│   ├── partial_op.rs
│   ├── tree_node.rs
│   ├── timestep.rs
│   ├── bandwidth_allocation.rs
│   └── plan_serializer.rs
├── tree/                               # reduction tree implementation (~6 files)
│   ├── mod.rs
│   ├── binomial_tree.rs
│   ├── tree_builder.rs
│   ├── tree_traversal.rs
│   ├── node_hasher.rs
│   └── merge_engine.rs
├── schedule/                           # timestep + m-PPR scheduling (~8 files)
│   ├── mod.rs
│   ├── timestep_assign.rs
│   ├── batch_builder.rs
│   ├── m_ppr_scheduler.rs
│   ├── load_apportion.rs
│   ├── job_queue.rs
│   ├── coordinator.rs
│   └── types.rs
├── pressure/                           # network pressure reduction (~6 files)
│   ├── mod.rs
│   ├── cap_enforcer.rs
│   ├── throttle.rs
│   ├── backpressure.rs
│   ├── budget_tracker.rs
│   ├── foreground_protector.rs
│   └── types.rs
├── failure/                            # failure tracking (~6 files)
│   ├── mod.rs
│   ├── chunk_state.rs
│   ├── node_state.rs
│   ├── straggler.rs
│   ├── retry.rs
│   ├── fallback.rs
│   └── types.rs
├── audit/                              # signed, hash-chained audit log (~6 files)
│   ├── audit_entry.rs
│   ├── audit_chain.rs
│   ├── checkpoint.rs
│   ├── signer.rs
│   ├── verifier.rs
│   └── mod.rs
├── carrier_adapter.rs                  # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
├── tests/                              # property/verification tests (~30 files)
│   ├── mod.rs
│   ├── ppr_tree_tests.rs
│   ├── timestep_tests.rs
│   ├── m_ppr_tests.rs
│   ├── bandwidth_cap_tests.rs
│   ├── pressure_tests.rs
│   ├── failure_tracker_tests.rs
│   ├── straggler_tests.rs
│   ├── audit_tests.rs
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net usage
│   ├── crypto_allowlist_tests.rs
│   ├── property_tests.rs               # proptest properties for tree/merge
│   └── integration_tests.rs
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── tree_bench.rs
│   ├── timestep_bench.rs
│   ├── m_ppr_bench.rs
│   ├── bandwidth_bench.rs
│   └── pressure_bench.rs
├── docs/                               # (~14 files)
│   ├── architecture.md
│   ├── ppr_algorithm.md
│   ├── m_ppr_scheduling.md
│   ├── bandwidth_caps.md
│   ├── failure_handling.md
│   ├── audit.md
│   ├── integration.md
│   ├── carrier_agnostic.md
│   ├── crypto_policy.md
│   ├── runbook.md
│   └── adding_a_repair_domain.md
└── spec/
    ├── ppr_spec.md
    ├── m_ppr_spec.md
    ├── tree_spec.md
    └── carrier_agnostic_spec.md
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/plan/`, `src/tree/`, `src/schedule/`, `src/pressure/`, `src/failure/`, `src/audit/`, `tests/`, `benches/`, `docs/`, `spec/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 PPR partial-op decomposition

```
decompose(stripe, failed_chunk, k_helpers, erasure_params) -> Vec<PartialOp>
  - Split each helper's surviving chunk into sub-chunks (sub-packetization)
  - Form linear combinations over GF(2^w) per erasure code coefficients
  - Each PartialOp: {source_helper, target_aggregator, sub_chunk_id, coefficients, payload: Vec<u8>}
  - Partial ops are independent → parallelizable across helpers
```

#### 7.2 Binomial reduction tree

```
BinomialTree { nodes: Vec<TreeNode>, root: NodeId, timesteps: ⌈log₂(k+1)⌉ }
  - Arrange k helpers + requestor as a binomial tree
  - Timestep t: nodes at distance t send partial results to parent
  - Each node transfers ≤ ⌈log₂(k+1)⌉ chunks total vs k in conventional repair
  - Root (requestor) reconstructs failed chunk from aggregated partial results
  - Merge at each node: idempotent, commutative, associative (linear combination)
```

#### 7.3 Timestep scheduling

```
assign_timesteps(tree, bandwidth_caps) -> Vec<TimestepBatch>
  - Timestep 0..T where T = ⌈log₂(k+1)⌉
  - Each batch contains partial ops whose links don't share a congested endpoint
  - Logical timesteps keyed to IdsTime ticks; never wall-clock
```

#### 7.4 m-PPR concurrent scheduling

```
m_ppr_schedule(jobs: Vec<RepairRequest>) -> Vec<ScheduledJob>
  - Apportion network/disk load across concurrent reconstruction jobs
  - Minimize competition for shared resources (per EuroSys 2016 §5)
  - Return per-job RepairPlan with coordinated bandwidth allocations
  - Benefit decreases as number of simultaneous failures grows
```

#### 7.5 Bandwidth caps / network pressure reduction

```
BandwidthCap { per_server: bytes/timestep, per_link: bytes/timestep, foreground_budget: bytes/s }
  - Per-server cap: ⌈log₂(k+1)⌉ chunks vs k conventional
  - Foreground traffic protected: repair bandwidth ≤ configured budget
  - Throttle/backpressure when links approach cap
```

#### 7.6 Failure handling

```
FailureTracker { chunk_state: Map<ChunkId, ChunkState>, node_state: Map<NodeId, NodeState> }
  - ChunkState: Pending / InProgress / Reconstructed / Failed / Fallback
  - On helper failure: select alternate, re-schedule partial op, bounded retry
  - On straggler: timeout, cancel, re-issue to alternate
  - Scope: t ≤ n−k concurrent failures; beyond → unrecoverable (signal ids-recovery)
```

#### 7.7 Audit chain

```
AuditEntry { op_id, node_id, timestamp(IdsTime), decision, prev_hash: [u8;32], signature: Signature }
  - sha256/blake3 hash chain + ed25519-signed checkpoints
  - Offline verifiable; tamper-evident repair decision log
```

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision/plan transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of repair plans, partial ops, audit entries.
- `thiserror` — error types.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No async runtime.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-87.1 | PPR decomposition produces partial ops executable in parallel across k helpers | `ppr_decompose` returns Vec<PartialOp> with independent source/target pairs |
| AC-87.2 | Reduction tree completes in ⌈log₂(k+1)⌉ timesteps vs k conventional | `binomial_tree` timestep count == ⌈log₂(k+1)⌉; verified for k=3,4,6,8,10,12 |
| AC-87.3 | Per-server max traffic ≤ ⌈log₂(k+1)⌉ chunks (vs k conventional) | `bandwidth_caps` enforces cap; tree traversal verifies per-node transfer count |
| AC-87.4 | m-PPR schedules concurrent reconstructions with coordinated bandwidth | `m_ppr_schedule` returns per-job plans; no two jobs over-allocate shared links |
| AC-87.5 | Network pressure reduction: total transfer time reduced vs conventional | Model: PPR ~2×C/BN for small k vs k×C/BN conventional; documented in spec |
| AC-87.6 | Helper failure mid-repair → alternate selection + re-schedule | `failure_tracker` + `straggler_handler` recover within bounded retries |
| AC-87.7 | Straggler helper → timeout + re-issue | `straggler_handler` enforces timeout bound; cancellation token |
| AC-87.8 | Failure scope t ≤ n−k enforced; beyond → unrecoverable signal | `redundancy_scope` invariant; returns unrecoverable error |
| AC-87.9 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) passes; source grep zero matches |
| AC-87.10 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-87.11 | Every repair plan ed25519-signed; audit chain sha256/blake3-hashed + ed25519 checkpoints | `repair_audit` signs; `audit_chain` verifies offline |
| AC-87.12 | Logical timesteps only (IdsTime monotonic); no wall-clock for scheduling | `timestep_scheduler` uses IdsTime ticks |
| AC-87.13 | Reduction-tree merge idempotent/commutative/associative | Property tests (proptest) pass |
| AC-87.14 | Crate compiles cleanly: `cargo check -p ids-ppr-reconstruction` exits 0 | Build passes |
| AC-87.15 | Conformance tests pass: `cargo test -p ids-ppr-reconstruction` | All tests green |

---

### 10. Remaining Uncertainties

1. **Concrete (n,k) defaults per deployment:** PPR gains vary by code params ((6,3) 50% … (12,4) 66.6%); defaults to be validated at integration with `ids-erasure`/`ids-recovery`.
2. **Sub-packetization level:** number of sub-chunks per chunk trades parallelism vs overhead; unquantified for IDS chunk sizes.
3. **m-PPR load-apportionment heuristic:** exact algorithm for coordinating concurrent jobs (greedy vs LP) to be selected at implementation.
4. **Bandwidth cap values:** per-server/per-link caps and foreground budget are deployment-tuned; SLO pending.
5. **Straggler timeout bound:** trades tail latency vs re-issue overhead; workload-dependent.
6. **Helper failure fallback chain:** how many alternates before declaring a chunk unrecoverable mid-repair; policy pending.
7. **Interaction with regenerating codes (MSR/Clay):** PPR compounds with repair-bandwidth-reducing codes; combined savings unquantified for IDS.
8. **Audit checkpoint frequency:** tradeoff between audit granularity and signing overhead; to be tuned.
9. **Cross-rack/region repair:** PPR reduction tree assumes helper reachability; cross-rack bandwidth constraints need network-aware tree construction (cf. PPT/PPCT, SMFRepair) — deferred.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-ppr-reconstruction` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear repair-planning boundary |
| PPR reduction tree with ⌈log₂(k+1)⌉ timesteps | EuroSys 2016: logarithmic vs linear transfer time; 50–67% bandwidth reduction |
| m-PPR for concurrent failures | EuroSys 2016 §5: schedules multiple reconstructions, minimizes shared-resource contention |
| Crate is a pure planner (emits RepairPlan, never sends bytes) | Preserves IDS architecture; no network stack in trust base; caller executes via ids-carrier::Endpoint |
| Binomial reduction tree aggregation | Distributes traffic across helper links; no single congested requestor downlink |
| Logical timesteps keyed to IdsTime; no wall-clock | Clock manipulation cannot break repair scheduling |
| ed25519-signed repair plans + sha256/blake3 audit chain | Tamper-evident, offline-verifiable repair decision log |
| Only allowed crypto; consumed via ids-crypto primitive IDs | Complies with allowed-crypto rule |
| Failure scope t ≤ n−k enforced | Beyond redundancy, data is unrecoverable; signal rather than fabricate recovery |
| Per-server bandwidth cap ⌈log₂(k+1)⌉ chunks | Caps network pressure; protects foreground traffic |
| Idempotent/commutative/associative tree merge | Tolerates carrier-duplicated datagrams and re-tries |

---

### 12. Sources

1. Mitra, Panta, Ra, Bagchi — "Partial-Parallel-Repair (PPR): A Distributed Technique for Repairing Erasure Coded Storage," EuroSys 2016. https://engineering.purdue.edu/dcsl/publications/papers/2016/final_erasurecodedrepair_eurosys16_cameraready.pdf — PPR algorithm, ⌈log₂(k+1)⌉ timesteps, binomial reduction tree, m-PPR, Table 1 gains (50–67%), up to 59% time reduction / 8.25× throughput.
2. Li, Li, Lee, Huang — "Repair Pipelining for Erasure-Coded Storage," USENIX ATC 2017. https://www.usenix.org/system/files/conference/atc17/atc17-li_runhui.pdf — ECPipe, O(1) single-block repair, slice pipelining, HDFS/QFS integration.
3. Li et al. — "Repair Pipelining for Erasure-Coded Storage: Algorithms and Evaluation," arXiv:1908.01527. https://arxiv.org/pdf/1908.01527v1 — PPR O(log k) vs pipelining O(1) vs conventional O(k); multi-block repair k+f−1 timeslots.
4. Shen, Cai, Cheng, Lee, Li, Hu, Shu — "A Survey of the Past, Present, and Future of Erasure Coding for Storage Systems," ACM TOS 2025. https://dl.acm.org/doi/full/10.1145/3708994 — PPR [126], ECPipe [106], PPT [12], SMFRepair [238], RepairBoost [116], PivotRepair [224], regenerating codes, LRCs.
5. Goparaju, Fazeli, Vardy — "Minimum Storage Regenerating Codes For All Parameters," IEEE TIT 2017. https://escholarship.org/content/qt7hk293h1/qt7hk293h1.pdf — MSR/MBR repair-bandwidth tradeoff, Clay codes general (n,k).
6. Rashmi, Shah, Gu, Kuang, Borthakur, Ramchandran — "A Solution to the Network Challenges of Data Recovery in Erasure-coded Distributed Storage Systems: A Study on the Facebook Warehouse Cluster," arXiv:1309.0186. https://arxiv.org/html/1309.0186 — >100 TB/day recovery traffic, piggybacking codes ~30% savings.
7. Li, Lin, Lee — "CORE: Augmenting Regenerating-Coding-Based Recovery for Single and Concurrent Failures," TC 2014 / arXiv:1302.3344. https://arxiv.org/html/1302.3344v2 — concurrent failure recovery, 36–64% bandwidth savings, relayer model.
8. Bai et al. — "Bandwidth-Aware Scheduling Repair Techniques in Erasure-Coded Clusters," IEEE TDSC 2022. https://store.computer.org/csdl/journal/td/2022/12/09720121/1BefencGkBG — SMFRepair/MSRepair, idle-node bypass, heterogeneous bandwidth.
9. Zhang et al. — "Parallel Pipeline Tree (PPT) / PPCT," ICPPS 2019. https://dl.acm.org/doi/10.1145/3337821.3337831 — non-uniform traffic tree repair, up to 89.2% time reduction over PPR.
10. Cheng et al. — "LESS is More for I/O-Efficient Repairs," USENIX FAST 2026. https://www.usenix.org/system/files/fast26-cheng.pdf — repair-friendly codes, I/O-efficient repair.
11. Xu et al. — "SelectiveEC: Selective Reconstruction in Erasure-coded Storage Systems," HotStorage 2020. https://files.usenix.org/s3fs-public/cdn/hotstorage20_paper_xu.pdf — balanced recovery load, bipartite-graph task selection.
12. Li, Lin, Lee — "Enabling Concurrent Failure Recovery for Regenerating-Coding-Based Storage Systems," TC 2014. https://www.cse.cuhk.edu.hk/~pclee/www/pubs/tc14core_main.pdf — CORE concurrent failures, relayer model.
13. Cheng et al. — "Parallelized In-Network Aggregation for Failure Repair," IEEE TON 2024. https://store.computer.org/csdl/journal/nt/2024/04/10445305/1URb2zKxLUY — Paint, programmable-switch aggregation repair.
14. RFC 8032 — EdDSA/ed25519. https://www.rfc-editor.org/rfc/rfc8032
15. RFC 7539 / RFC 8439 — ChaCha20-Poly1305 / XChaCha20-Poly1305. https://www.rfc-editor.org/rfc/rfc7539
16. NIST FIPS 180-4 — SHA-256. https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
17. BLAKE3-team — BLAKE3 specification. https://github.com/BLAKE3-team/BLAKE3
18. IDS prompts.txt entry 87 (verbatim). C:/idfs/prompts.txt lines 1300-1314.
19. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml.
20. HX-241 plan format references. C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 087 (ids-ppr-reconstruction).
