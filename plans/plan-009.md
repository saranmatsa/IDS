## Phase 009 — Create the ids-placement crate (capacity-aware and failure-correlation-aware placement): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no source modifications, no repository changes made. This document is the blueprint for implementation only.**

---\

### 1. Executive Summary

Phase 009 creates the **ids-placement** crate — the engine that decides where data lives in the IDS distributed store. It is a placement engine with hard constraints and soft objectives: capacity-aware placement (storage, CPU, memory, network, IOPS), failure-domain-aware placement (power, facility, network segment, operator, jurisdiction), failure-correlation-aware placement (avoid correlated loss), recovery-cost-aware placement, dynamic rebalancing, multiple placement strategies (hash, range, geographical, workload-aware, capacity-aware, hybrid), and hard constraint enforcement (residency, authorization, durability, capacity, data type, retention).

**Core engineering decisions (research-verified):**
- Placement is a **pure function of the data key/id, the cluster topology map, and the placement policy** — deterministic across replicas, no central allocator (CRUSH pattern: Weil et al., SC'06; avoids single point of failure and full-mapping bookkeeping).
- **Hard constraints are validated first, soft objectives rank candidates** (flexi-constraint/Constraint Relaxation pattern: arXiv 2507.13895; failed hard constraint = placement impossible, not degraded).
- **Failure-domain independence is failure-mode-specific** (MerceMay analysis): replicas spread across independent failure domains are independent only for the named mode; correlated-failure modeling uses a directed dependency graph/arborescence (COCOA/arXiv 1701.01539 DP algorithms) to avoid placing replicas on branches that a single failure can wipe out.
- **Capacity tracking is multi-dimensional** (storage, CPU, memory, network, IOPS); fit test is component-wise vector comparison (MerceMay bin-packing analysis): free cores alone do not imply room.
- **New crate `ids-placement`** per user's separate-crates rule; depends only on `ids-000-aa` (types/sealed contracts), `ids-topology` (failure domains), `ids-resource` (capacity metrics), `ids-membership` (node lifecycle), `ids-placement-policy` (dataset policy), `ids-consistency-profile` (durability), `ids-redundancy-mode` (replication vs erasure), `ids-crypto` (allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305).
- All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

---\

### 2. Architecture Overview

The placement engine sits between the topology/membership layer and the distribution/replication layer. It is **not** a network protocol — it is a local, deterministic decision engine that produces `PlacementDecision` (canonical type in `ids-000-aa`) consumed by `ids-distribution`, `ids-replication`, `ids-routing`, and `ids-read`.

```
┌─────────────────────────────────────────────────────────────┐
│                   CONSUMER LAYER                            │
│  ids-distribution · ids-replication · ids-routing · ids-read│
├─────────────────────────────────────────────────────────────┤
│              Phase 009: ids-placement (NEW CRATE)           │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │  Strategy   │ │  Constraint │ │  Capacity & Failure │   │
│  │  Selector   │ │  Enforcer   │ │  Domain Model         │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │   Rebalancer│ │   Recovery  │ │  Objective Scorer   │   │
│  │  (dynamic)  │ │   Cost      │ │  (soft objectives)  │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│            ids-topology · ids-resource · ids-membership     │
│   FailureDomain hierarchy · capacity metrics · node lifecycle│
├─────────────────────────────────────────────────────────────┤
│             ids-000-aa (canonical types, sealed contracts)   │
│   PlacementDecision · FailureDomain · RegionId · HUnitId ... │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a placement decision:**
1. Request arrives: dataset id, redundancy mode, consistency profile, data type, retention, size, replicas/fragments needed.
2. **Constraint Enforcer** filters candidate nodes/H-units/regions against hard constraints (residency, authorization, durability, capacity headroom, data type, retention) — any unsatisfiable hard constraint → placement rejected, never degraded.
3. **Strategy Selector** picks strategy per policy: hash (consistent/rendezvous), range (key-range partitioning), geographical (proximity to consumers), workload-aware (access patterns), capacity-aware (multi-dim bin fit), hybrid (weighted blend).
4. **Failure-Domain Model** applies failure-domain independence at the required level; failure-correlation model (directed dependency graph) rejects placements where one correlated failure could disable too many replicas (COCOA DP objective).
5. **Objective Scorer** ranks survivors on soft objectives: least-loaded, locality, cost, rebalance churn minimization.
6. `PlacementDecision` emitted: deterministic node set + rationale + version stamp; consumed by distribution/replication; rebalancer re-evaluates on topology/capacity/membership changes.

---\

### 3. Detailed Research Dimensions

#### 3.1 Placement Strategies (hash, range, geographical, workload-aware, capacity-aware, hybrid)

**Hash / rendezvous hashing (HRW):** each object key is hashed against the cluster map; destination is a pure function of key + map. When one node joins/leaves, only ~1/N of objects move (AIStore rebalance docs, NVIDIA). Deterministic, no central directory. Limitation: uniform random assignment is not capacity-optimal under heterogeneous hardware — skewed weights produce unbalanced placement (MerceMay analysis). Used as the baseline strategy; capacity-aware variants layer on top.

**Range partitioning:** key ranges map to partition owners; placement moves whole ranges. Good for ordered scans and range queries; split/merge coordinated by `ids-distribution`. Less flexible for failure-domain spreading than hash.

**Geographical:** data placed near consumers/regions to minimize latency (Volley, USENIX NSDI'10: automated geo-placement correlating access logs to latency/cost/capacity models; Iridium, SIGCOMM CCR: joint data+task placement over WAN). Constraints: inter-DC bandwidth cost, per-region capacity caps, data residency laws.

**Workload-aware:** placement tracks access patterns (hot/cold), moves hot data to high-performance nodes (H2DP, TPDS'19: heterogeneity-aware placement across HDD/SSD; workload-driven placement for IMDBMS, HPI: cost models over access traces). Coarse-grain workload-aware replication improves throughput 25–80% (NC State TR-2017-2). Dynamic re-evaluation needed as patterns change at runtime.

**Capacity-aware:** multi-dimensional bin fit — CPU, memory, storage, network, IOPS vectors; component-wise fit test (MerceMay bin-packing: fit = vector comparison, not scalar sum). Greedy/first-fit/best-fit heuristics; NP-hard, so production uses heuristics with bounded moves (consistent hashing with bounded loads, SODA'18 Karger et al.; Google Research blog, Mirrokni/Thorup/Zadimoghaddam 2018: capacity C = ceil((1+eps)·n/k), overflow cascades to next bin).

**Hybrid:** weighted blend of strategies per dataset policy (e.g., 70% capacity-aware + 30% geographical). Policy-driven weights from `ids-placement-policy`.

#### 3.2 Capacity Tracking (storage, CPU, memory, network, IOPS)

Each node/H-unit/region exposes a capacity vector: total, used, reserved, headroom per dimension. Placement fits only if every dimension of the placement's demand ≤ remaining headroom (component-wise comparison). Headroom reserved for recovery/rebalance bursts (Alauda Ceph planning: keep headroom for recovery, rebalance, upgrades; expand before near-full).

Metrics sources: `ids-resource` crate (storage/CPU/memory/network monitoring, latency/queue/utilization/health). IOPS and throughput per storage device (Cloudera infra metrics: Disk IOPS, throughput; AWS FSx Lustre: DiskIopsUtilization per OST). Capacity planning forecasts growth from historical usage (IBM Storage Insights: 7-day minimum history; forecast next year).

Capacity is tracked as **usable** not raw (replication/erasure reduce effective space; Alauda: usable capacity examples for 3-replica). Headroom percentages configurable (e.g., 80% warning threshold).

#### 3.3 Failure-Domain-Aware Placement & Failure-Correlation Modeling

**Failure domain hierarchy:** root → region → datacenter → room → row → rack → chassis → host → device (Ceph CRUSH bucket types; Lightbits: each server labeled with a set of FDs, minimum 3 non-intersecting FDs; replicas never share a domain). Correlated factors: physical proximity, shared power source, shared networking, shared operator/jurisdiction (Ceph docs).

**Correlated failure model:** directed dependency graph/arborescence — nodes = failure events, edge u→v means failure of u triggers v. Goal: choose placements where correlated failures disable as few replicas as possible. Two DP algorithms: O(n + ρ·log ρ) for single file on tree; polynomial when skew is constant (COCOA, arXiv 1701.01539). Causal failure model generalization: node v cascades when ≥ℓ(v) incoming neighbors failed (arXiv 1503.02654).

**Domain independence is failure-mode-specific:** replicas on separate racks are independent against rack power loss but not against data-center-wide events (MerceMay). Effective replication factor = mode-specific. Placement must name the failure mode it protects against.

**Domain independence verification (IDS-topology integration):** power-domain separation, network-segment separation, operator separation, jurisdiction separation — verified before placement accepted.

#### 3.4 Recovery-Cost-Aware Placement

When a node dies, every object it held must be re-replicated; the replacement must not land in a domain already occupied by surviving replicas (MerceMay: rebuild traffic concentrates across rack boundary under HDFS 2+1). Rebuild traffic is the constraint that most often forces placement changes.

Recovery-cost-aware placement: pre-compute rebuild fan-in/fan-out per candidate; prefer placements where recovery can be served from local/cheap helpers (Google Colossus RS(6,3), Facebook RS(10,4), Azure RS(12,4); USENIX FAST'21 correlated SSD failure study). Recovery bandwidth cost modeled per failure domain (intra-cluster vs inter-cluster cost ratio; GRBC model for clustered regenerating codes). Balanced recovery load distribution algorithm (USENIX ATC'23 Shan: pick target nodes with used storage within (1+ε) of min, minimizing recovery load edge weights).

#### 3.5 Hard Constraint Enforcement (residency, authorization, durability, capacity, data type, retention)

Hard constraints remove candidates entirely (Cloudflare placement: regions/jurisdiction constraints; AWS Well-Architected data residency: geographic restrictions under GDPR Chapter V). Constraint satisfaction vs optimization separation: feasible-configs generation then Pareto selection (IEEE CLOUD 2018: hard+soft constraints, multiple conflicting objectives).

- **Residency / jurisdiction:** data may only be stored in permitted regions/countries (Google Spanner data residency: per-placement region selection; Northflank: residency attribute assigned at creation, propagated end-to-end).
- **Authorization:** placement only on nodes/operators authorized for the dataset (data-use policy, ownership policy).
- **Durability:** minimum replicas/fragments, erasure (n,k), required placement domains (HDFS EC policies: RS-6-3 default, RS-10-4; oneuptime durability comparison: topology + recovery window matter more than ratio).
- **Capacity:** component-wise fit test; headroom reservation.
- **Data type:** storage-type policies (HDFS storage types: SSD vs HDD fallback paths).
- **Retention:** retention period + legal hold interact with placement (WORM/immutable; retention-aware zone placement, RetainZ USENIX: group checkpoints retiring together for zone reclamation).

Failed hard constraint → placement rejected (never silently degraded). Soft objectives then rank feasible candidates.

#### 3.6 Dynamic Rebalancing

Triggered by topology change (node join/leave/maintenance/decommission), capacity skew, or workload shift. AIStore global rebalance: decentralized, each target recomputes HRW destination for local objects, moves only objects whose owner changed (~1/N of objects per single node change). Trigger conditions: load imbalance thresholds, cooldown periods, min-load thresholds (GreptimeDB region balancer: acceptable_load_ratio, min_load_threshold, cooldown). Rebalance vs resilver distinction (AIStore): rebalance = placement correction; resilver = redundancy restoration after failure.

Rebalance scope: full vs partial (bounded by cost budget; StorPool balancer scores and constraint violations). Serves reads during migration (AIStore). Migration bounded to avoid thundering herd (ids-fault integration: circuit breakers, bulkheads, backpressure).

---\

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Correlated failure wipes multiple replicas | Failure-domain independence at named level; correlation graph (arborescence) checked before placement; DP-optimal replica spread |
| Placement on over-capacity node | Component-wise capacity fit test; headroom reservation; reject before placement |
| Hard constraint silently violated | Hard constraints validated first; unsatisfiable = reject, never degrade; audit log of rejections |
| Rebalance churn / thundering herd | Bounded rebalance budget, cooldown windows, ~1/N move fraction (AIStore), circuit-breaker gating via ids-fault |
| Deterministic placement diverges across replicas | Pure function of key+map+policy; same inputs → same decision on every replica; version-stamped decisions |
| Recovery overloads surviving nodes | Recovery-cost-aware target selection; bounded recovery fan-in; capacity-bounded recovery (ids-recovery) |
| Placement leaks data to unauthorized region | Residency/authorization constraints checked before any candidate scored; jurisdiction filtering (Cloudflare pattern) |
| Correlated power/network failure across "independent" domains | Domain independence verification (power, network segment, operator, jurisdiction) from ids-topology |

---\

### 5. Integration with Existing Crates

|| Crate | Role in Phase 009 ||
||-------|-------------------|-------------------|
|| `ids-000-aa` | Canonical types: `PlacementDecision`, `FailureDomain`, `RegionId`, `HUnitId`, `NodeId`, `DatasetId`, `PartitionId`, `RedundancyMode`, `ConsistencyProfile`; sealed `trait Placement` contract |
|| `ids-topology` | FailureDomain hierarchy, topology map, correlated-failure modeling, domain independence verification |
|| `ids-resource` | Capacity metrics: storage/CPU/memory/network/IOPS per node, utilization, health |
|| `ids-membership` | Node states (candidate/validating/active/degraded/draining/failed/retired); only eligible nodes placed on |
|| `ids-placement-policy` | Dataset-specific policy: replica/fragment counts, required domains, redundancy mode |
|| `ids-consistency-profile` | Durability/acknowledgement semantics per profile |
|| `ids-redundancy-mode` | Full replication vs erasure coding per dataset |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — placement decisions reference endpoints, no direct carrier use |
|| `ids-crypto` | Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) for signed placement decisions/audit |

**Integration points:**
- `ids-distribution` consumes `PlacementDecision` to assign partitions to nodes.
- `ids-replication` uses placement output to place replicas/fragments across failure domains.
- `ids-routing` uses placement for replica-aware, locality-aware request resolution.
- `ids-read` uses placement for closest-replica selection and fallback.
- `ids-selfreorg` and `ids-mobility` drive placement revision on topology/membership/region changes.
- `ids-fault` gates rebalancing under circuit-breaker/bulkhead/backpressure.
- Carrier-agnostic: placement never touches `std::net`/`tokio::net`/libp2p; all traffic via `ids-carrier::Endpoint`.

---\

### 6. Proposed File Structure (~120–200 files, one new crate)

**Crate:** `ids-placement` (new crate; depends on ids-000-aa, ids-topology, ids-resource, ids-membership, ids-placement-policy, ids-consistency-profile, ids-redundancy-mode, ids-crypto; thiserror, serde, blake3)

```
ids-placement/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Placement constants (headroom defaults, rebalance thresholds)
├── error.rs                        # PlacementError (thiserror, comprehensive)
├── features.rs                     # Feature flags (simulation, fuzzing, benchmarks)
│
├── strategy/                       # Placement strategies
│   ├── mod.rs
│   ├── trait.rs                    # PlacementStrategy trait (select candidates)
│   ├── hash_strategy.rs            # Consistent/rendezvous hashing (HRW)
│   ├── range_strategy.rs           # Key-range partitioning
│   ├── geographical_strategy.rs    # Proximity to consumers/regions
│   ├── workload_aware.rs           # Access-pattern-driven placement
│   ├── capacity_aware.rs           # Multi-dimensional bin fit
│   ├── hybrid_strategy.rs          # Weighted blend of strategies
│   └── strategy_selector.rs        # Picks strategy per dataset policy
│
├── constraint/                     # Hard constraint enforcement
│   ├── mod.rs
│   ├── enforcer.rs                 # Validates candidates against hard constraints
│   ├── residency.rs                # Residency/jurisdiction filtering
│   ├── authorization.rs            # Authorization checks (data-use/ownership policy)
│   ├── durability.rs               # Durability requirement checks (replicas/domains)
│   ├── capacity.rs                 # Component-wise capacity fit test
│   ├── data_type.rs                # Data type / storage type constraints
│   ├── retention.rs                # Retention + legal hold constraints
│   ├── constraint_types.rs         # Constraint, ConstraintResult structures
│   └── constraint_proptests.rs     # Property tests: reject never degrade
│
├── failure_domain/                 # Failure-domain & correlation model
│   ├── mod.rs
│   ├── domain_model.rs             # FailureDomain hierarchy integration
│   ├── independence.rs             # Domain independence verification (power, network, operator, jurisdiction)
│   ├── correlation.rs              # Correlated failure graph (arborescence/dependency DAG)
│   ├── correlation_dp.rs           # DP-optimal replica spread (COCOA-style)
│   ├── domain_types.rs             # FailureDomainRef, DomainLevel structures
│   └── domain_proptests.rs
│
├── capacity/                       # Capacity tracking & bin packing
│   ├── mod.rs
│   ├── tracker.rs                  # Capacity vector per node/H-unit/region
│   ├── fit_test.rs                 # Component-wise multi-dimensional fit
│   ├── bin_pack.rs                 # Greedy/first-fit/best-fit heuristics
│   ├── headroom.rs                 # Headroom reservation for recovery/rebalance
│   ├── capacity_types.rs           # CapacityVector, Demand, Headroom structures
│   └── capacity_proptests.rs
│
├── decision/                       # PlacementDecision assembly
│   ├── mod.rs
│   ├── assembler.rs                # Builds PlacementDecision from scored candidates
│   ├── scorer.rs                   # Soft-objective scoring (load, locality, cost, churn)
│   ├── rationale.rs                # Decision rationale + version stamp
│   └── decision_types.rs
│
├── rebalance/                      # Dynamic rebalancing
│   ├── mod.rs
│   ├── trigger.rs                  # Rebalance triggers (topology, capacity, workload)
│   ├── planner.rs                  # Rebalance plan (which objects move, to where)
│   ├── budget.rs                   # Rebalance budget (move fraction, cooldown)
│   ├── migrator.rs                 # Migration coordination (reads during migration)
│   └── rebalance_proptests.rs
│
├── recovery_cost/                  # Recovery-cost-aware placement
│   ├── mod.rs
│   ├── cost_model.rs               # Rebuild fan-in/fan-out cost per candidate
│   ├── target_selector.rs          # Pick recovery targets minimizing cost
│   ├── bandwidth.rs                # Intra/inter-domain bandwidth cost model
│   └── recovery_cost_proptests.rs
│
├── types/                          # Composed placement types
│   ├── mod.rs
│   ├── placement_request.rs        # PlacementRequest (dataset, redundancy, size, ...)
│   ├── placement_result.rs         # PlacementResult (decision + rejections)
│   └── replica_set.rs              # Replica/fragment set across domains
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_placement.rs
│   ├── fuzz_constraints.rs
│   └── fuzz_rebalance.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── strategy_tests.rs           # Each strategy produces valid placements
│   ├── constraint_tests.rs         # Hard constraint rejection behavior
│   ├── failure_domain_tests.rs     # Domain independence & correlation
│   ├── capacity_tests.rs           # Component-wise fit, headroom
│   ├── rebalance_tests.rs          # Trigger, plan, bounded moves
│   ├── recovery_cost_tests.rs      # Recovery target selection
│   ├── integration_tests.rs        # End-to-end placement → decision
│   ├── adversarial_tests.rs        # Correlated failure, overload, skew
│   ├── determinism_tests.rs        # Same inputs → same decision everywhere
│   ├── property_tests.rs           # Proptest properties
│   └── chaos_tests.rs              # Node churn during placement
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── placement_bench.rs          # Placement decision latency
│   ├── constraint_bench.rs         # Constraint enforcement overhead
│   ├── rebalance_bench.rs          # Rebalance planning benchmarks
│   └── fit_test_bench.rs           # Capacity fit test benchmarks
│
├── spec/                           # Formal specifications
│   ├── placement_spec.md           # Placement algorithm & invariants
│   ├── constraint_spec.md          # Hard/soft constraint semantics
│   └── tla/                        # TLA+/PlusCal (in ids-doc)
│
├── docs/                           # Documentation
│   ├── architecture.md             # Placement engine architecture
│   ├── integration_guide.md        # Integrating with distribution/replication/routing
│   └── strategy_guide.md           # When to use which strategy
│
└── test_vectors/                   # Deterministic test vectors
    ├── placement_vectors.json
    ├── constraint_vectors.json
    └── rebalance_vectors.json
```

**Why ~120–200 files:** one file per responsibility — 6 strategies + selector, 7 constraint modules + enforcer, failure-domain model + correlation + independence + DP, capacity tracker + fit + bin-pack + headroom, decision assembler + scorer + rationale, rebalance trigger/planner/budget/migrator, recovery cost model + target selector + bandwidth, composed types, fuzzing, property/integration/adversarial/determinism/chaos tests, benchmarks, formal spec, docs, test vectors. No stubs.

---\

### 7. Key Algorithms & Data Structures

#### 7.1 Placement decision pipeline
```
decide(req: PlacementRequest) -> PlacementDecision:
  candidates = all eligible nodes (membership: active/degraded only)
  candidates = constraint_enforcer.filter(candidates, req.hard_constraints)
  if candidates.empty(): return Rejected(unsatisfiable_constraints)
  survivors = strategy_selector.select(req, candidates)   // per-strategy candidate set
  ranked = objective_scorer.rank(survivors, req.soft_objectives)
  failure_domain.verify_independence(ranked.top_k, req.required_domains)
  correlation_graph.check(ranked.top_k, req.correlation_model)
  return PlacementDecision(ranked.best, rationale, version)
```

#### 7.2 Component-wise capacity fit test
```
fit(node, demand) = ∀ d ∈ {storage, cpu, memory, network, iops}:
                    node.remaining[d] >= demand[d]
```

#### 7.3 Failure-domain independence (arborescence DP, COCOA-style)
- Failure model = directed rooted tree/arborescence; edge u→v: failure of u triggers v.
- Objective: place ρ replicas on leaves minimizing max replicas disabled by any single failure event.
- DP: O(n + ρ·log ρ) for single file; polynomial when replica-count skew is constant.

#### 7.4 Consistent hashing with bounded loads
- Hash key and nodes to unit circle; key → first bin clockwise; if full, cascade to next non-full bin.
- Bin capacity C = ceil((1+eps)·n/k); expected moves within factor of optimum on add/remove.

#### 7.5 Rebalance trigger
```
rebalance if: max_util − min_util > imbalance_threshold
              OR node join/leave/maintenance/decommission
              OR workload skew exceeds threshold
bounded by: move_fraction ≤ budget, cooldown between runs
```

---\

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | Canonical types + sealed `trait Placement` contract ||
|| `ids-topology` | FailureDomain hierarchy, topology map, correlation model ||
|| `ids-resource` | Capacity metrics (storage/CPU/memory/network/IOPS) ||
|| `ids-membership` | Node lifecycle states (eligible nodes only) ||
|| `ids-placement-policy` | Dataset-specific placement policy ||
|| `ids-consistency-profile` | Durability/acknowledgement semantics ||
|| `ids-redundancy-mode` | Replication vs erasure coding per dataset ||
|| `ids-crypto` | Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 ||
|| `serde` + `serde_bytes` | Serialization ||
|| `thiserror` | Error types ||
|| `blake3` | Hashing (allowed crypto) ||

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-placement` is a pure decision engine; all traffic via `ids-carrier::Endpoint`.

---\

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-009.1 | Deterministic placement | Same key+map+policy → identical `PlacementDecision` on every replica (property test, 100k iterations) |
|| AC-009.2 | Hard constraint rejection | Unsatisfiable hard constraint → placement rejected, never silently degraded (adversarial test) |
|| AC-009.3 | Failure-domain independence | Replicas placed across distinct failure domains at the required level; no two replicas share a domain that fails together (unit + adversarial) |
|| AC-009.4 | Correlated-failure avoidance | Placement minimizes replicas disabled by any single correlated failure event (DP-optimal on tree model) |
|| AC-009.5 | Capacity fit (multi-dimensional) | Placement accepted iff every capacity dimension (storage/CPU/memory/network/IOPS) fits (component-wise test) |
|| AC-009.6 | Capacity-aware bin packing | Heterogeneous nodes filled proportionally to capacity; no overload under skew (bench) |
|| AC-009.7 | Recovery-cost-aware | Recovery targets minimize rebuild fan-in/fan-out and cross-domain bandwidth (cost model bench) |
|| AC-009.8 | Dynamic rebalancing | Triggered on topology/capacity/workload change; ~1/N objects move per single-node change; bounded by budget + cooldown |
|| AC-009.9 | Strategy correctness | Each strategy (hash/range/geographical/workload/capacity/hybrid) produces valid placements per its contract (unit tests) |
|| AC-009.10 | Carrier agnosticism | Compiles and behaves identically with zero `std::net`/`tokio::net`/libp2p/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh dependencies |
|| AC-009.11 | Allowed crypto only | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 used; no other crypto primitives |
|| AC-009.12 | Adversarial tests pass | Correlated failure, overload, skew, constraint-conflict tests all fail-safe |

---\

### 10. Remaining Uncertainties

1. **Strategy weighting for hybrid:** exact weight blend per dataset class (capacity vs geography vs workload) is workload-dependent; defaults are placeholders pending measurement.
2. **Correlation graph source:** static config, learned from failure history, or operator-declared — all must be supported behind one interface; no decision yet.
3. **Capacity refresh cadence:** how often capacity vectors are updated from `ids-resource`; staleness tolerance for placement decisions undefined.
4. **Rebalance cost model:** exact move-cost formula (bytes × distance × priority) and budget defaults need workload tuning.
5. **Placement decision versioning:** how `PlacementDecision` versions interact with `ids-partition-map` versioning and migration tracking is undefined.
6. **Signed placement decisions:** whether `PlacementDecision` carries an ed25519 signature (via `ids-crypto`) for audit is a product decision, not a research conclusion.
7. **Geographical proximity data source:** consumer location / region latency data source undefined; geographical strategy needs an input interface.
8. **Formal verification scope:** TLA+/PlusCal spec for placement invariants (domain independence, constraint rejection) deferred to `ids-doc`.

---\

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Pure function of key+map+policy (CRUSH pattern) | Deterministic, no central allocator, no full-mapping bookkeeping (Weil et al. SC'06; AIStore HRW) |
|| Hard constraints first, soft objectives rank | Failed hard constraint = reject, never degrade (Constraint Relaxation pattern, arXiv 2507.13895) |
|| Multi-dimensional component-wise capacity fit | Free cores alone don't imply room; vector comparison per dimension (MerceMay bin-packing analysis) |
|| Failure-domain independence is failure-mode-specific | Replicas on separate racks are independent against rack power loss, not DC-wide events (MerceMay) |
|| Correlation graph via arborescence DP | Minimize replicas disabled by one correlated failure; O(n + ρ·log ρ) (COCOA, arXiv 1701.01539) |
|| Recovery-cost-aware target selection | Rebuild traffic is the constraint that most often forces placement changes (MerceMay) |
|| Rebalance ~1/N objects per node change | Decentralized, bounded churn (AIStore global rebalance) |
|| New `ids-placement` crate | User's separate-crates rule; clean dependency boundary; keeps existing crates focused |
|| Allowed crypto only | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; nothing else |
|| Carrier-agnostic via `ids-carrier::Endpoint` | No `std::net`/`tokio::net`/libp2p/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |

---\

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 009.

**Sources consulted (30 targeted searches):**
- CRUSH: Controlled, Scalable, Decentralized Placement of Replicated Data (Weil et al., SC'06) — https://ceph.com/assets/pdfs/weil-crush-sc06.pdf
- Ceph CRUSH Maps & failure domains — https://docs.ceph.com/en/latest/rados/operations/crush-map
- Consistent Hashing with Bounded Loads (SODA'18; Google Research blog) — https://dl.acm.org/doi/10.5555/3174304.3175309 ; https://research.google/blog/consistent-hashing-with-bounded-loads/
- Revisiting Consistent Hashing with Bounded Loads (AAAI-21) — https://ojs.aaai.org/index.php/AAAI/article/view/16517
- Algorithms for Optimal Replica Placement Under Correlated Failure (COCOA, arXiv 1701.01539) — https://arxiv.org/html/1701.01539v4
- Algorithms for Replica Placement in High-Availability Storage (arXiv 1503.02654) — https://arxiv.org/html/1503.02654v4
- On Correlated Failures in Survivable Storage Systems (CMU-CS-02-129) — http://pdl.cmu.edu/ftp/PASIS/CMU-CS-02-129.pdf
- An In-Depth Study of Correlated Failures in Production SSD-Based Data Centers (USENIX FAST'21) — https://www.usenix.org/system/files/fast21-han.pdf
- Explore Data Placement Algorithm for Balanced Recovery Load Distribution (USENIX ATC'23) — https://www.usenix.org/system/files/atc23-shan.pdf
- Cost aware fault recovery in clouds (IFIP IM'2013) — https://dl.ifip.org/db/conf/im/im2013/IsraelR13.pdf
- Global repair bandwidth cost of regenerating codes (IET) — https://researchonline.jcu.edu.au/70817/1/2021_Global%20repair_IET_cmu2.12289.pdf
- HDFS Architecture / Rack Awareness / Block Placement Policies — https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/HdfsDesign.html ; https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-common/RackAwareness.html
- AIStore Global Rebalance (HRW, ~1/N moves) — https://docs.nvidia.com/aistore/rebalance
- Failure-Domain Placement: Keeping Replicas Out of the Same Blast Radius (MerceMay) — https://mercemay.com/articles/distributed-systems/replication/failure-domain-placement-and-correlated-loss/
- Availability in Globally Distributed Storage Systems (USENIX OSDI'10, Ford) — https://static.usenix.org/events/osdi10/tech/full_papers/Ford.pdf
- Volley: Automated Data Placement for Geo-Distributed Cloud Services (USENIX NSDI'10) — https://resources.mpi-inf.mpg.de/departments/d5/teaching/ws10_11/cc/presentations/20101214-martin-beyer-volley.pdf
- Iridium: Low Latency Geo-distributed Data Analytics (SIGCOMM CCR) — https://dl.acm.org/doi/10.1145/2829988.2787505
- WANify: Gauging and Balancing Runtime WAN Bandwidth (arXiv 2508.12961) — https://arxiv.org/html/2508.12961v1
- Application Placement with Constraint Relaxation (arXiv 2507.13895) — https://arxiv.org/html/2507.13895v1
- Optimal Cloud Resource Selection Considering Hard and Soft Constraints (IEEE CLOUD 2018) — https://www.computer.org/csdl/proceedings-article/cloud/2018/723501a831/13xI8B3IkY6
- Discrete-choice Multi-agent Optimization: Decentralized Hard Constraint Satisfaction (arXiv 2305.16993) — https://arxiv.org/html/2305.16993v1
- Smash: Flexible, Fast, Resource-Efficient Placement (PACM'23 / CACM) — https://dl.acm.org/doi/10.1145/3589977 ; https://users.soe.ucsc.edu/~qian/papers/Smash.pdf
- Categorization for constraint-based placement of object replicas (US9659080B1, Google) — https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9659080
- A Holistic Heterogeneity-Aware Data Placement (TPDS'19, H2DP) — https://shuibing9420.github.io/assets/pdf/holistic-place-tpds19.pdf
- Workload-Aware Data Placement for Cloud Computing (NCSU TR-2017-2) — https://techrep.csc.ncsu.edu/2017/TR-2017-2.pdf
- Workload-Driven Data Placement for Tierless In-Memory Database Systems (HPI) — https://hpi.de/oldsite/fileadmin/user_upload/fachgebiete/plattner/publications/papers/2023/Hurdelhey23WorkloadDrivenDataPlacement.pdf
- Cost-Aware Region-Level Data Placement (IEEE) — https://shuibing9420.github.io/assets/pdf/Cost-Aware_Region-Level_Data_Placement_in_Multi-Tiered_Parallel_I_O_Systems.pdf
- Capability-Aware Data Placement for Heterogeneous Active Storage Systems — https://shuibing9420.github.io/assets/pdf/Li-2016-Capability-Aware%20Data%20Placement%20for%20He.pdf
- Data residency (Google Spanner / Cloud SQL / Assured Workloads / AWS Well-Architected) — https://docs.cloud.google.com/spanner/docs/data-residency ; https://docs.aws.amazon.com/pdfs/wellarchitected/latest/data-residency-hybrid-cloud-services-lens/data-residency-hybrid-cloud-services-lens.pdf
- Data lifecycle / retention-aware placement (WEKA, OneLake, Azure Blob, RetainZ USENIX) — https://docs.weka.io/weka-system-overview/data-storage ; https://learn.microsoft.com/en-us/fabric/onelake/onelake-lifecycle-management ; https://arxiv.org/html/2610.05554v1
- HDFS Erasure Coding durability comparison (RS-6-3, RS-10-4) — https://oneuptime.com/blog/post/2026-09-04-choose-hdfs-erasure-coding-policy-durability/view
- Data placement across racks and sites (Scality) — https://www.scality.com/blog/data-placement?hs_amp=true
- Decibel: Isolation and Sharing in Disaggregated Rack-Scale Storage (USENIX NSDI'17, Mirador placement engine) — https://www.usenix.org/system/files/conference/nsdi17/nsdi17-nanavati.pdf
- GreptimeDB Region Balancer (rebalance thresholds, cooldown) — https://docs.greptime.com/enterprise/autopilot/region-balancer
- StorPool Rebalancing the cluster (balancer scores, constraint violations) — https://kb.storpool.com/admin-guide/rebalancing-the-cluster.html
- Infrastructure metrics (CPU, memory, disk IOPS, network) — https://docs.cloudera.com/observability/cloud/cluster-management/topics/obs-rtm-monitor-infrastructure.html