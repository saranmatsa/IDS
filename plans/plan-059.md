## Phase 059 — Create the ids-redundancy-mode crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

-----

### Verbatim prompt (C:/idfs/prompts.txt lines 880-894)

```
59. Research only do not read plan.md **[Phase 059 — Create the ids-redundancy-mode crate: Full replication vs erasure coding per dataset.]**

Crate: ids-redundancy-mode
Purpose: Full replication vs erasure coding per dataset.
Contents: Full replication mode, erasure coding mode, hybrid redundancy, dataset-specific redundancy, H-unit-aware redundancy, replica/fragment placement across failure domains.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 059 — Create the ids-redundancy-mode crate: Full replication vs erasure coding per dataset.]**

Create plan-059.md in C:/idfs/plans/ for the ids-redundancy-mode crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

-----

### 1. Executive Summary

Phase 059 creates `ids-redundancy-mode`, the IDS crate that owns per-dataset redundancy method selection: full replication vs erasure coding, hybrid coexistence of both modes, dataset-specific redundancy objectives, H-unit-aware redundancy, and replica/fragment placement across failure domains. `ids-redundancy-mode` is a pure decision-and-placement layer: it does not encode, decode, move, or store data itself — it selects the redundancy mode per dataset, determines how many replicas or (k, m) fragments to place, picks failure-domain-diverse placement targets, and exposes the redundancy plan to `ids-distribution`, `ids-replication`, `ids-recovery`, and `ids-placement` for execution. All traffic it influences rides as opaque `Vec<u8>` datagrams over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate itself never imports a network stack.

Redundancy mode is a per-dataset property, not a global constant. Hot/performance-sensitive datasets use full replication (3 copies typical) for fast local reads and trivial single-copy rebuild. Cold/capacity-sensitive datasets use systematic Reed-Solomon erasure coding (e.g., RS(10,4), RS(6,3)) for 1.33×–1.5× storage overhead vs 3× for triple replication, tolerating any m fragment losses per stripe, at the cost of k-fragment read amplification on rebuild and read-modify-write on small writes. Hybrid redundancy lets both modes coexist in one dataset (hot shards replicated, cold shards erasure-coded), and H-unit-aware redundancy constrains placement so no two copies share an H-unit, rack, power circuit, or network switch.

`ids-redundancy-mode` does NOT yet exist at `C:/idfs/ids-redundancy-mode/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `CarrierType`, `RedundancyMode`, `Checksum`, plus sealed trait contracts (`Placement`, `Replication`, `Erasure`, `Durability`, `Topology`, `FaultTolerance`) and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Reed-Solomon systematic (k+m) erasure coding over GF(2^8) with any-k-of-n reconstruction (MDS property); ISA-L Cauchy/Vandermonde generator matrices and `reed_solomon_erasure` Rust crate (GF(2^8)/0x11d); RAID 1 mirroring vs RAID 5/6 trade-offs (storage 2x vs 1.33×–1.5×, rebuild from one copy vs k fragments); HDFS rack-aware replica placement (local rack + remote rack, 2-rack spread); Ceph CRUSH failure domains (host/rack/row/room/datacenter, at most one slice per domain); Cassandra NetworkTopologyStrategy (per-DC replica counts, rack-aware placement); Dynamo N/R/W quorum replication with consistent hashing; Amazon S3 11-nines durability across 3+ AZs with erasure coding; Facebook f4 warm storage reducing effective replication factor from 3.6 to 2.8/2.1 via RS; Microsoft Azure WAS LRC (local reconstruction codes) reducing repair reads from k to r ≪ k; pyramid codes trading MDS distance for locality; regenerating codes minimizing repair bandwidth; locally repairable codes (LRC) minimizing helper-node count; EVENODD/RDP/Liberation XOR-based RAID-6 codes; comparison papers showing erasure coding wins for cold data, replication wins for hot data; hybrid replication→erasure-coding transitioning (ELECT/F4 pattern: replicate hot, EC cold, transition offline); bitrot detection via per-shard checksum sidecars; MTTDL data-loss probability for k-of-n codes under node failure/repair distributions.

-----

### 2. Architecture Overview

`ids-redundancy-mode` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives for integrity). It is a decision-and-placement layer: it never touches payload bytes, never encodes/decodes, never opens a socket, and never imports `std::net`, `tokio::net`, libp2p, or any transport. It consumes dataset identity, topology state, policy version, resource measurements, and operation context; it emits a redundancy plan (mode, replica/fragment count, placement targets, reconstruction threshold, verification requirements) consumed by `ids-distribution`, `ids-replication`, `ids-recovery`, and `ids-placement`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (redundancy consumers)          │
│  ids-distribution, ids-replication, ids-recovery, ids-placement — apply plan │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 059: ids-redundancy-mode (THIS CRATE — decision/placement)│
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ │ │
│  │ mode selection  │ │ dataset policy  │ │ placement engine│ │ H-unit    │ │ │
│  │ (full vs EC vs  │ │ (per-dataset    │ │ (failure-domain │ │ aware     │ │ │
│  │  hybrid)        │ │  objectives)    │ │  diversity)     │ │ redundancy│ │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └───────────┘ │ │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ │ │
│  │ replica/fragment│ │ EC (k,m) config │ │ integrity plan  │ │ contracts │ │ │
│  │  count planner  │ │ (parity shards, │ │ (checksums,     │ │ + invariants│ │ │
│  │                 │ │  stripe width)  │ │  sidecars)      │ │           │ │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └───────────┘ │ │
│  ┌───────────────────────────────────────────────────────────────────────┐ │ │
│  │  audit/logging (every decision signed+chained, sha256/blake3 chain)   │ │ │
│  └───────────────────────────────────────────────────────────────────────┘ │ │
└──────────────────────────────────────────────────────────────────────────────┘
                                                         │ opaque Vec<u8> datagrams
┌────────────────────────────────────────────────────────▼────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])  │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a redundancy decision:** `ids-distribution` (or `ids-placement`) submits a dataset identity + policy intent → `ids-redundancy-mode` evaluates dataset-specific redundancy objectives against topology state, capacity, failure-domain diversity, and H-unit membership → emits `RedundancyPlan{mode, replica_count|ec_k_m, placement_targets[], threshold, verification}` → consumer crate executes placement/copy/encode over `ids-carrier::Endpoint` datagrams → `ids-redundancy-mode` records the decision in the append-only, hash-chained audit log.

**Redundancy modes:**
- **Full replication mode:** N complete copies of the dataset (or partition), each on a distinct failure domain. Any single copy serves reads; loss of N−1 copies tolerable; rebuild = copy one surviving replica. Storage overhead N×. Best for hot, read-heavy, low-latency datasets.
- **Erasure coding mode:** systematic Reed-Solomon (k, m): data split into k equal fragments, m parity fragments computed over GF(2^8), n=k+m fragments stored across distinct failure domains; any k of n reconstruct. Storage overhead (k+m)/k. Best for cold, capacity-sensitive datasets; rebuild reads k fragments (k× bandwidth vs replication's 1×).
- **Hybrid redundancy:** full replication + erasure coding coexist per dataset (per-shard or per-partition mode selection). Hot shards replicated, cold shards erasure-coded; transitions between modes online (replication→EC encoding after data cools; EC→replication on hotness increase).
- **Dataset-specific redundancy:** each dataset declares its own objective (durability target, read latency, storage budget, failure tolerance); `ids-redundancy-mode` evaluates each policy independently, never forcing one global config.
- **H-unit-aware redundancy:** placement constrained so copies/fragments of one dataset never share an H-unit (and optionally rack, power, network, operator domain); H-unit failure compensated by another eligible H-unit when capacity and policy permit.

-----

### 3. Detailed Research Dimensions

#### 3.1 Full Replication Mode

Full replication stores N complete, independent copies of the data (or partition), each on a distinct node/H-unit/region. Reads can be served from any replica; writes fan out to all N copies (or a write quorum W of N with read quorum R, R+W>N for consistency per Dynamo quorum technique). Tolerates loss of any N−1 copies without data loss. Rebuild after a failure copies one surviving replica — minimal bandwidth (one object's worth) and no computation.

Research-verified characteristics:
- **Storage overhead:** N× (3× for triple replication, the HDFS/Cassandra default).
- **Read performance:** excellent — any replica can serve, read load balances across copies, locality-aware reads preferred (HDFS reads from same-rack replica first).
- **Write performance:** write must reach W replicas before acknowledgment; write amplification N× (or W× with quorum); small writes are cheap (no read-modify-write parity update).
- **Rebuild:** cheapest of all redundancy methods — copy c bytes from any one surviving replica to the replacement node.
- **Consistency:** replica divergence handled by read repair, anti-entropy, hinted handoff, version vectors (Dynamo pattern); last-write-wins or vector-clock conflict resolution.

Best fit: hot datasets, low-latency reads, small objects, workloads where storage is cheap relative to performance. IDS-A §37 (hybrid redundancy) and `hybrid_redundancy.rs` in `ids-000-aa/src/invariants/` reference full replication + erasure coding coexisting per dataset.

#### 3.2 Erasure Coding Mode

Systematic Reed-Solomon (k, m) erasure coding: data object partitioned into k equal data fragments; m parity fragments computed over GF(2^8) via a Cauchy or Vandermonde generator matrix; n=k+m fragments stored across distinct failure domains. Any k of n fragments reconstruct the original (MDS property). Storage overhead (k+m)/k — e.g., RS(10,4) = 1.4× overhead vs 3× for triple replication, tolerating any 4 fragment losses per stripe. RS(6,3) = 1.5×. RS(4,2) = 1.5×.

Research-verified characteristics:
- **Storage overhead:** (k+m)/k — 40–50% for typical configs vs 200% for triple replication.
- **Fault tolerance:** tolerates any m fragment losses per stripe (data or parity).
- **Read performance:** full reads need all k data fragments (or any k of n); degraded reads of a missing fragment require reconstruction from k others — k× bandwidth vs replication's 1×.
- **Write performance:** small writes incur read-modify-write or reconstruct-write penalty (read existing fragments, recompute parity, write back) — write amplification up to k+1×; stripe-sized writes avoid RMW.
- **Rebuild:** reconstructs lost fragment from k surviving fragments — k× bandwidth and GF(2^8) computation vs replication's 1× copy.
- **Integrity risk:** parity shards are cold (never read on normal serving) — silent bitrot in parity poisons reconstruction; requires per-fragment checksum sidecars + scrubbing (SeaweedFS EC bitrot detection pattern).

RS over GF(2^8): addition = XOR, multiplication via log/antilog tables; generator matrix rows 0..k identity (systematic), rows k..k+m from Cauchy/Vandermonde so any k×k submatrix is invertible. Rust crates: `reed_solomon_erasure` (GF(2^8)/0x11d, klauspost/Backblaze-compatible), `rusty_erasure` (ISA-L-compatible pure Rust), `sia_reed_solomon` (AVX2/GFNI/NEON). ISA-L `gf_gen_cauchy1_matrix` / `gf_gen_rs_matrix` produce invertible Cauchy matrices.

EVENODD/RDP/Liberation XOR-based RAID-6 codes (m=2) avoid GF arithmetic entirely (XOR + cyclic shifts) — optimal XOR count, prime-number disk constraints. Regenerating codes (MSR/MBR) minimize repair bandwidth; LRC/pyramid codes minimize helper nodes (locality r ≪ k) for single-fragment repair. Azure WAS uses LRC; Facebook f4 uses RS for warm blobs.

Best fit: cold/capacity-sensitive datasets, large objects, storage-constrained environments, workloads tolerant of higher rebuild/read amplification.

#### 3.3 Hybrid Redundancy

Hybrid redundancy lets full replication and erasure coding coexist within the same dataset — different shards, partitions, or temperature zones use different modes. Microsoft Azure pattern: objects enter replicated (hot), later erasure-coded and replicas deleted as they cool. Facebook f4: warm blobs use EC to cut effective replication factor from 3.6 to 2.8/2.1 while tolerating disk, host, rack, and datacenter failures. USENIX FAST'24 ELECT: LSM-tree KV store with hotness-aware redundancy transitioning — hot KV pairs replicated, cold SSTables erasure-coded, cold data offloaded to a cold tier; 56.1% less hot-tier storage than pure replication with similar normal read/write performance.

Research-verified considerations:
- **Transitioning:** replication→EC requires encoding all replicas then deleting extras (encoding-aware replication placement avoids cross-rack downloads during encoding — CMU HDFS paper); EC→replication requires decoding then copying.
- **Granularity:** per-object, per-shard, per-partition, or per-SSTable mode selection; metadata tracks each unit's mode and placement.
- **Temperature tracking:** access-frequency/recency scoring decides hot vs cold; hysteresis avoids mode-flapping.
- **Consistency during transition:** both copies must be valid and verifiable before the old mode's copies are retired; checksum sidecars validate before delete.

Best fit: datasets with mixed access patterns (hot head, long cold tail), tiered storage (hot/warm/cold), capacity-plus-performance requirements.

#### 3.4 Dataset-Specific Redundancy

Each dataset declares its own redundancy objective: durability target (nines), read-latency SLO, storage budget, maximum tolerated failures, failure-domain scope (node/H-unit/rack/region). `ids-redundancy-mode` evaluates each policy independently — no global forced configuration. IDS CAPABILITIES §41/§42/§43 define hybrid redundancy, dataset-specific redundancy, and H-unit-aware redundancy as explicit system responsibilities; CRATES.md entry 59 defines ids-redundancy-mode contents as "Full replication mode, erasure coding mode, hybrid redundancy, dataset-specific redundancy, H-unit-aware redundancy, replica/fragment placement across failure domains."

Per-dataset policy fields: `mode ∈ {FullReplication, ErasureCoding, Hybrid}`, `replica_count` or `(k, m)`, `required_failure_domains`, `min_redundancy_state`, `recovery_requirements`, `verification_requirements`, `temperature_threshold` (for hybrid), `allowed_regions`, `allowed_hunits`.

#### 3.5 H-Unit-Aware Redundancy

H-units are intermediate operational groups of storage nodes inside a regional database cluster (IDS CAPABILITIES §34, IDS-A §36). H-unit-aware redundancy constrains replica/fragment placement so copies of one dataset never share an H-unit — an H-unit failure loses at most one copy/fragment per dataset. Compensation: another eligible H-unit assumes serving/storage/reconstruction if capacity, redundancy, and policy permit (IDS-A §36, `hunit_compensation.rs` invariant).

Placement constraints extend recursively: H-unit → rack → power circuit → network switch → region (topological failure domains). `topology_diversity.rs` invariant: different locations, H-units, regions, failure domains per copy. CRUSH-style placement (Ceph): at most one slice per failure domain; failure domain = bucket type (host/rack/row/room/datacenter). HDFS rack-aware: with RF=3, one replica local rack, one remote rack, third different node in remote rack (2-rack spread); BlockPlacementPolicyRackFaultTolerant places on 3 different racks.

Best fit: any redundancy mode where correlated failure (whole H-unit lost) must not cause data loss or extended unavailability.

#### 3.6 Replica/Fragment Placement Across Failure Domains

Placement selects target nodes/H-units/regions for each replica or fragment. Key algorithms:
- **Consistent hashing (with virtual nodes):** maps partition/fragment ID to node ring position; only k/n keys remap on node add/remove (Cassandra vnodes, Dynamo, Karger et al.). Improves load distribution; vnodes handle heterogeneity (weighted by capacity).
- **Rack/failure-domain awareness:** place copies on distinct failure domains at the configured level (H-unit, rack, region). HDFS: local rack + remote rack; Ceph CRUSH: at most one slice per failure domain.
- **Capacity-aware:** track usable/reserved capacity per node/H-unit/region; avoid assignments exceeding declared capacity with safety margins (`ids-000-aa/src/placement/capacity_tracker.rs`).
- **Topology-aware:** prefer nearest replica for reads (near-to-far routing, region affinity); fallback to next-closest eligible replica on failure (`read_fallback.rs` invariant).

Placement outputs a `PlacementPlan{targets: Vec<(FragmentIndex, NodeId/HUnitId/RegionId)>, failure_domains: Vec<DomainId>}` consumed by `ids-placement` and `ids-distribution` for actual datagram shipping over `ids-carrier::Endpoint`.

-----

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|----------------|--------------------------|
| Correlated failure wipes all copies (same H-unit/rack/power) | Durability | H-unit-aware placement: at most one copy/fragment per failure domain; topology diversity invariant |
| Silent bitrot in cold parity shards poisons reconstruction | Integrity | Per-fragment checksum sidecars (sha256/blake3), scrub before rebuild, RS arbitration of corrupt shards |
| Erasure-coded rebuild reads k fragments (k× bandwidth, latency) | Availability/performance | Prefer local fragments for degraded reads; LRC/pyramid locality for single-fragment repair; schedule rebuild off-peak |
| Small-write penalty on erasure-coded data (read-modify-write) | Performance | Stripe-sized writes avoid RMW; hybrid mode keeps hot data replicated |
| Replica divergence under concurrent writes | Consistency | Version vectors + quorum (R+W>N); conflict policy per dataset (last-writer-wins / MV-register) |
| Placement overcommits capacity (node/H-unit full) | Availability | Capacity-aware placement with reserved recovery headroom; rejection before commit |
| Mode transition leaves data under-protected mid-transition | Durability | Both modes valid + checksum-verified before retiring old copies; atomic plan commit |
| H-unit failure exceeds compensation capacity | Durability | H-unit compensation via eligible peer H-unit; degradation mode documented; regional fallback |
| Carrier duplicates/delays/reorders placement datagrams | Correctness | Idempotent placement application; sequence numbers; carrier-agnostic boundary (opaque Endpoint) |

The carrier is never trusted: placement decisions are signed over payload bytes, surviving re-framing by any carrier; no network stack is in the trust base.

-----

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-redundancy-mode ||
|-------|-------|------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `CarrierType`, `RedundancyMode`, `Checksum`, `Error`; sealed traits `Placement`, `Replication`, `Erasure`, `Durability`, `Topology`, `FaultTolerance`; invariants `hybrid_redundancy`, `erasure_threshold`, `correlated_failure`, `hunit_compensation`, `topology_diversity` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All redundancy-related traffic (placement plans, fragment directives) is datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Checksum sidecars (sha256/blake3), signed redundancy plans (ed25519) |
| `ids-placement` | Data placement engine (capacity, failure domain, H-unit/region selection) | Consumes `RedundancyPlan`; emits concrete node/H-unit targets |
| `ids-distribution` | Data distribution across nodes/H-units/regions | Executes replica/fragment placement per plan |
| `ids-replication` | Replica synchronization (streams, checkpoints, retries) | Full-replication copy operations; rebuild from surviving replica |
| `ids-recovery` | Multi-level recovery (node/H-unit/region) | Erasure fragment reconstruction; redundancy restoration after recovery |
| `ids-erasure` (Phase planned) | Erasure encode/decode (Reed-Solomon, regenerating codes) | Consumes `(k, m)` config; performs GF(2^8) encode/decode |
| `ids-durability-policy` (Phase 058) | Per-dataset durability requirements | Feeds durability targets into redundancy mode selection |

-----

### 6. Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-redundancy-mode/` (a separate crate, not a folder inside an existing crate):

```
ids-redundancy-mode/
  Cargo.toml
  src/
    lib.rs                      # Crate root, public API surface, feature gates
    errors.rs                   # RedundancyError, PlacementError, ModeError
    types.rs                    # RedundancyMode, RedundancyPlan, EcConfig, ReplicaConfig (re-exports ids-000-aa types)
    mode_selection.rs           # Full replication vs erasure coding vs hybrid decision per dataset
    dataset_policy.rs           # Per-dataset redundancy objectives and policy evaluation
    placement_engine.rs         # Failure-domain-diverse replica/fragment placement
    hunit_aware.rs              # H-unit-aware redundancy constraints and compensation
    replica_planner.rs          # Replica count planner (N for full replication)
    ec_config.rs                # Erasure coding (k, m) configuration: parity shards, stripe width
    integrity_plan.rs           # Checksum sidecar plan, scrub schedule, verification
    carrier_adapter.rs          # Opaque Vec<u8> datagram framing to ids-carrier::Endpoint
    contracts/                  # Sealed trait contracts (~10 files)
      redundancy_contract.rs, placement_contract.rs, mode_contract.rs,
      ec_contract.rs, replica_contract.rs, hunit_contract.rs,
      integrity_contract.rs, policy_contract.rs, audit_contract.rs, mod.rs
    invariants/                 # Compile-time/runtime invariants (~12 files)
      correlated_failure.rs, topology_diversity.rs, hunit_compensation.rs,
      erasure_threshold.rs, hybrid_redundancy.rs, capacity_bounded.rs,
      placement_diversity.rs, mode_transition_atomic.rs, checksum_sidecar.rs,
      carrier_agnostic.rs, crypto_allowlist.rs, mod.rs
  tests/                        # (~25 files)
  benches/                      # (~8 files)
  docs/                         # (~14 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

-----

### 7. Key Algorithms & Data Structures

#### 7.1 Redundancy Mode Selection

```
select_mode(dataset, policy, topology, capacity) -> RedundancyMode
  if policy.hot && capacity.permits(replica_count): FullReplication
  if policy.cold && capacity.permits(ec_overhead): ErasureCoding
  if policy.mixed: Hybrid (per-shard/partition mode by temperature)
  else: policy.default (FullReplication for latency-critical, ErasureCoding for capacity-critical)
```

#### 7.2 Full Replication Plan

```
ReplicaPlan { replica_count: u32, targets: Vec<NodeId>, failure_domains: Vec<DomainId>, quorum: (R, W) }
read: any replica; prefer local failure domain (near-to-far)
write: fan out to W of N; ack on W (R+W>N for consistency)
rebuild: copy c bytes from one surviving replica
storage overhead: N×
tolerates: any N−1 copy losses
```

#### 7.3 Erasure Coding Plan (systematic Reed-Solomon over GF(2^8))

```
EcPlan { k: u32, m: u32, n=k+m, stripe_size: u64, fragment_size: u64, targets: Vec<NodeId>, failure_domains: Vec<DomainId> }
encode: data[k fragments] × G[(k+m)×k] → n fragments (first k = data verbatim, last m = parity)
decode: any k of n fragments → solve linear system over GF(2^8) → reconstruct
GF(2^8): addition = XOR; multiplication via log/antilog tables (0x11d primitive polynomial)
generator: Cauchy or Vandermonde; any k×k submatrix invertible (MDS)
storage overhead: (k+m)/k
tolerates: any m fragment losses per stripe
rebuild: reconstruct lost fragment from k fragments (k× bandwidth, GF compute)
integrity: per-fragment sha256/blake3 checksum sidecar; scrub before rebuild
```

Typical configs: RS(10,4) = 1.4× overhead, any 4 losses; RS(6,3) = 1.5×; RS(4,2) = 1.5×; RS(3,2) = 1.67×. k+m ≤ 255 (GF(2^8) field size).

#### 7.4 Hybrid Redundancy Plan

```
HybridPlan { shards: Vec<(ShardId, Mode, Plan)>, temperature_threshold, transition_policy }
hot shards: FullReplication(N=3)
cold shards: ErasureCoding(RS(10,4))
transition: replicate→EC after cool period; EC→replicate on heat increase; both modes verified before switch
```

#### 7.5 Dataset-Specific Redundancy Policy

```
DatasetPolicy { dataset_id, mode, replica_count | (k,m), required_failure_domains, min_redundancy_state, recovery_requirements, verification_requirements, temperature_threshold, allowed_regions, allowed_hunits }
evaluated per-dataset; no global forced config
```

#### 7.6 H-Unit-Aware Placement

```
place(dataset_plan, topology) -> PlacementPlan
  group nodes by H-unit → rack → region → power/network domain
  select targets so each copy/fragment lands in a distinct failure domain at configured level
  if H-unit fails: compensation via eligible peer H-unit (capacity + redundancy + policy)
  topology diversity: no two copies share H-unit/rack/power/switch/region unless policy allows
```

Placement algorithms: consistent hashing with virtual nodes (baseline distribution), rack/fault-domain-aware selection (constrain to distinct domains), capacity-aware filtering (usable ≥ reserved + safety margin), near-to-far read preference.

-----

### 8. Dependencies

|| Crate | Purpose ||
|-------|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `CarrierType`, `RedundancyMode`, `Checksum`, `Error`) and sealed trait contracts (`Placement`, `Replication`, `Erasure`, `Durability`, `Topology`, `FaultTolerance`) + invariants |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — opaque datagram transport only |
| `ids-crypto` | Allowed primitives only: ed25519 (signed plans), sha256/blake3 (checksum sidecars, fragment integrity) |
| `ids-placement` | Placement engine (capacity, failure domain, H-unit/region selection) — consumer of redundancy plans |
| `serde` | Serialization of redundancy plans, policies, placement outputs |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No erasure-coding math crate in this decision/placement crate (encode/decode lives in `ids-erasure`; this crate only selects `(k, m)` and placement).

-----

### 9. Measurable Acceptance Criteria

|| Criterion | Target | Measurement ||
|-----------|----------|-------------|--------------|
| Per-dataset mode selection | full replication / erasure coding / hybrid chosen per dataset policy | unit test per dataset class |
| Full replication tolerance | tolerates any N−1 replica losses; rebuild = 1× object copy | unit test + fault-injection test |
| Erasure coding (k,m) | any k of n=k+m fragments reconstruct; tolerates any m losses | unit test (matrix invertibility), RS(10,4)/RS(6,3) configs |
| Storage overhead | replication N× vs EC (k+m)/k; EC ≤ 1.5× for typical cold configs | calculation/test |
| H-unit-aware placement | no two copies/fragments of one dataset share an H-unit | placement test with synthetic topology |
| Failure-domain diversity | copies spread across distinct H-unit/rack/region per policy | placement test |
| Dataset-specific policy | each dataset evaluated independently; no global forced config | unit test (multiple datasets, different modes) |
| Hybrid coexistence | hot shards replicated, cold shards EC within one dataset | unit test (shard-level mode map) |
| Mode transition atomicity | old and new copies both valid+checksummed before switch | transition test |
| Checksum sidecars | per-fragment sha256/blake3 checksum; scrub detects bitrot before rebuild | unit test |
| Carrier agnosticism | zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS; all traffic via `ids-carrier::Endpoint` | grep/audit |
| Crypto allowlist | only ed25519, sha256, blake3 (xchacha20poly1305/chacha20poly1305 if encryption needed) | dependency audit |
| Signed decisions | every redundancy decision recorded in append-only, hash-chained, ed25519-signed audit log | audit-log test |
| Placement capacity | never assigns beyond node/H-unit declared capacity + safety margin | capacity test |
| Rebuild bandwidth | EC rebuild reads k fragments vs replication 1×; documented ratio | bench/calculation |

-----

### 10. Remaining Uncertainties

1. **Erasure-code parametrization per dataset class:** which (k,m) defaults for hot/warm/cold datasets (RS(10,4) vs RS(6,3) vs RS(4,2)) — product decision, not research conclusion.
2. **Hybrid transition granularity:** per-object, per-shard, per-partition, or per-SSTable; affects metadata size and transition complexity.
3. **Temperature tracking:** access-frequency/recency scoring algorithm and hysteresis thresholds to prevent mode-flapping — workload-dependent.
4. **H-unit compensation eligibility:** what makes an H-unit "eligible" (capacity headroom, redundancy state, policy) — intersects with `ids-recovery` and `ids-placement` integration.
5. **Checksum sidecar storage:** where sidecars live (per-volume, per-fragment, separate crate) and scrub cadence — overlaps `ids-integrity` / `ids-security`.
6. **Quorum vs N for full replication:** whether writes require all N or W<N with read-quorum consistency (R+W>N) — consistency profile interacts with `ids-consistency-profile` (Phase 060).
7. **Erasure-code math crate selection:** `reed_solomon_erasure` vs `rusty_erasure` vs `sia_reed_solomon` lives in `ids-erasure`, but this crate's `(k,m)` advice must match the chosen crate's GF(2^8) matrix format — defer to `ids-erasure` phase.
8. **Failure-domain level granularity:** H-unit vs rack vs region vs power/switch — policy-driven, deployment-specific.

-----

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate crate `ids-redundancy-mode` | "Separate crates instead of more folders" directive; clean decision/placement boundary |
| Decision/placement only — no encode/decode | Keeps crate small, carrier-agnostic, no GF(2^8) math dependency; `ids-erasure` handles codec |
| Per-dataset redundancy mode (not global) | Matches IDS CAPABILITIES §41/§42/§43 and CRATES.md entry 59 contents |
| Systematic Reed-Solomon over GF(2^8) for EC | MDS property (any k of n), widely implemented (ISA-L, klauspost, rust crates), Cauchy/Vandermonde invertible matrices |
| Full replication = N copies, EC = (k+m)/k overhead | Established RAID/storage fact: 3× vs 1.33×–1.5× |
| H-unit-aware placement with at most one copy per failure domain | IDS-A §36 hunit_compensation; `correlated_failure`, `topology_diversity`, `hunit_compensation` invariants |
| Hybrid hot-replicated / cold-EC | Azure f4 / ELECT FAST'24 pattern: replication for hot, EC for cold; 56.1% storage savings |
| Checksum sidecars + scrub before rebuild | SeaweedFS EC bitrot detection: cold parity shards rot silently and poison RS reconstruction |
| Carrier-agnostic via `ids-carrier::Endpoint` only | Absolute IDS rule; no network stack in trust base |
| Crypto limited to ed25519/sha256/blake3 | IDS allowed-crypto list; signed plans + checksum sidecars |

-----

### Sources

[^1]: Reed-Solomon erasure coding — systematic (k+m), any k of n reconstruct, GF(2^8), MDS property: https://medium.com/@thisis-Shitanshu/erasure-coding-with-images-a8af65642345
[^2]: ISA-L erasure_code.h — GF(2^8) encode/decode, Cauchy matrix generation `gf_gen_cauchy1_matrix`: https://github.com/intel/isa-l/blob/ad8dce15/include/erasure_code.h
[^3]: reed_solomon_erasure Rust crate — GF(2^8)/0x11d Reed-Solomon encoder/decoder: https://docs.rs/reed-solomon-erasure
[^4]: rusty_erasure — pure-Rust ISA-L-compatible Reed-Solomon, Cauchy/Vandermonde/klauspost matrices: https://lib.rs/crates/rusty_erasure-core
[^5]: sia_reed_solomon — GF(2^8) RS with AVX2/GFNI/NEON runtime dispatch: https://docs.rs/crate/sia_reed_solomon/latest
[^6]: PPR — Partial-parallel-repair, RS(k,m) reconstruction reads k chunks vs replication's 1×: https://doi.org/10.1145/2901318.2901328
[^7]: OceanStore — erasure-coded systems use order-of-magnitude less bandwidth/storage than replication for similar durability: https://oceanstore.cs.berkeley.edu/publications/papers/pdf/erasure_iptps.pdf
[^8]: RAID standard levels — RAID 1 mirroring (2×) vs RAID 5/6 (1/n, 2/n overhead), read-modify-write penalty: https://en.wikipedia.org/wiki/Standard_RAID_levels
[^9]: VMware vSAN RAID 5/6 erasure coding — 1.33× overhead for FTT=1 vs 2× for RAID-1 mirroring: https://techdocs.broadcom.com/us/en/vmware-cis/vcf/vcf-9-0-and-later/9-1/vsan-deployment-administration-and-monitoring/administering-vmware-vsan/increasing-space-efficiency-in-a-vsan-cluster/using-raid-5-6-erasure-coding-in-vsan-cluster.html
[^10]: EVENODD/RDP/Liberation XOR-based RAID-6 codes (GF(2^8)-free, prime-disk constraints): https://cs.princeton.edu/courses/archive/spring13/cos598C/atc12-final181.pdf
[^11]: Plank FAST'13 tutorial — array codes, XOR-only vs GF arithmetic, EVENODD/RDP: https://web.eecs.utk.edu/~jplank/plank/papers/2013-02-11-FAST-Tutorial.pdf
[^12]: HDFS rack-aware replica placement — local rack + remote rack, 2-rack spread, rack failure tolerance: https://hadoop.apache.org/docs/r1.2.1/hdfs_design.html
[^13]: HDFS BlockPlacementPolicies — RackFaultTolerant places on 3 different racks; node groups/upgrade domains: https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/HdfsBlockPlacementPolicies.html
[^14]: HDFS-14786 — availability-zone-aware placement across AZs/racks: https://issues.apache.org/jira/browse/HDFS-14786
[^15]: Ceph CRUSH failure domains — at most one slice per domain (host/rack/row/room/datacenter): https://ceph.io/en/news/blog/2026/slicing-the-pie
[^16]: Ceph Tentacle FastEC — stripe unit, partial reads/writes, erasure-coded pool layout: https://wiki.ceph.com/en/news/blog/2025/tentacle-fastec-performance-updates
[^17]: Cassandra NetworkTopologyStrategy — per-DC replica counts, rack-aware placement, vnodes: https://docs.datastax.com/en/cassandra/3.0/cassandra/architecture/archDataDistributeDistribute.html
[^18]: Dynamo — N/R/W quorum replication, consistent hashing, vector clocks, read repair, hinted handoff: https://cs.cornell.edu/courses/cs5414/2017fa/papers/dynamo.pdf
[^19]: Apache Kudu rack awareness — location strings, no majority in one location: https://kudu.apache.org/docs/administration.html
[^20]: Comparing cost and performance of replication and erasure coding — EC for cold, replication for hot; hybrid Azure pattern (replicate on entry, EC later): https://arxiv.org/html/1308.1887v2
[^21]: ELECT FAST'24 — hybrid redundancy, hot replicated / cold EC, LSM-tree transitioning, 56.1% storage savings: https://usenix.org/system/files/fast24-ren.pdf
[^22]: Facebook f4 — warm blob storage, effective replication factor 3.6 → 2.8/2.1 via RS, disk/host/rack/datacenter failure tolerance: https://web.eecs.umich.edu/~mosharaf/Readings/f4.pdf
[^23]: Amazon S3 — 11-nines durability, 3+ AZ replication, erasure coding, end-to-end checksums: https://docs.aws.amazon.com/whitepapers/latest/aws-storage-services-overview/durability-and-availability.html
[^24]: Azure WAS LRC — local reconstruction codes, locality r ≪ k, reduces repair reads: https://cs.princeton.edu/courses/archive/spring13/cos598C/atc12-final181.pdf
[^25]: Locally repairable codes / pyramid codes — locality vs MDS distance trade-off, optimal (r,d)-codes: https://dl.acm.org/doi/10.1145/3381832; https://arxiv.org/html/1806.04437v1
[^26]: Regenerating codes — minimize repair bandwidth (MSR/MBR): https://arxiv.org/pdf/1101.0133
[^27]: EVENODD/RDP/X-Code — optimal XOR-count RAID-6 array codes: https://arxiv.org/pdf/2306.08763
[^28]: Consistent hashing with virtual nodes — k/n remap on node change, heterogeneity handling: https://bytebytego.com/courses/system-design-interview/design-consistent-hashing
[^29]: SeaweedFS EC bitrot detection — per-shard CRC32C sidecar, scrub before rebuild, RS arbitration: https://github.com/seaweedfs/seaweedfs/wiki/EC-Bitrot-Detection
[^30]: Silent data corruption detection — checksums per fragment, reconstruct-and-verify on mismatch: https://scality.com/blog/silent-data-corruption
[^31]: Data loss probability for erasure-coded systems under failure/repair distributions (MTTDL): https://arxiv.org/pdf/1409.7286
[^32]: IDS CAPABILITIES.md — §41 hybrid redundancy, §42 dataset-specific redundancy, §43 H-unit-aware redundancy, H-unit role in placement/redundancy/compensation: C:/idfs/CAPABILITIES.md
[^33]: IDS CRATES.md — entry 59 ids-redundancy-mode contents (full replication, erasure coding, hybrid, dataset-specific, H-unit-aware, placement across failure domains): C:/idfs/CRATES.md
[^34]: ids-000-aa PLAN.md — master crate canonical types, sealed trait contracts (Placement, Replication, Erasure, Durability, Topology, FaultTolerance), invariants (hybrid_redundancy, erasure_threshold, correlated_failure, hunit_compensation, topology_diversity): C:/idfs/ids-000-aa.md

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 059.