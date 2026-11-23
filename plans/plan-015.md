## Phase 015 — Create the ids-query crate: Query decomposition across partitions, nodes, regions: Research Findings & Implementation Blueprint

**Status:** Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

Verbatim prompt (C:/idfs/prompts.txt lines 220-234):
"Phase 015 — Create the ids-query crate: Query decomposition across partitions, nodes, regions."
Crate: ids-query
Purpose: Query decomposition across partitions, nodes, regions.
Contents: Query planner, targeted query execution, partition maps, indexes, predicates, ownership metadata, scatter-gather execution, distributed aggregation (count, sum, min, max, group, filter, sort), cross-regional queries, intermediate result combination.

CRATES.md entry 15: "ids-query — Distributed query planning and execution. Purpose: Query decomposition across partitions, nodes, regions."

**Carrier-agnostic:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).
**Allowed crypto only:** ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-query` consumes type-level surface and primitive IDs only).
**`ids-000-aa` is the master crate** holding every canonical type and sealed trait contract; `ids-query` depends on `ids-000-aa` (plus `ids-crypto` types).
Research only — no code, no source modifications, only this plan document.

Detailed findings are recorded in `C:/idfs/plans/results.md` (one entry per query Q001-Q030, each classified ESTABLISHED/HYPOTHESIS/SPECULATION and constraint-checked). Summary: 29 ESTABLISHED, 1 HYPOTHESIS (the carrier-agnostic Endpoint-only stance is an IDS architectural constraint, not prior art), 0 SPECULATION.

---

### 1. Executive Summary

Phase 015 creates `ids-query`, the IDS crate for distributed query planning and execution. It owns the query path: **plan a query, decompose it into per-partition/per-node/per-region fragments, execute via scatter-gather, combine intermediate results, and return a final result**. Contents per CRATES.md §15 and prompts.txt entry 15: query planner, targeted query execution, partition maps, indexes, predicates, ownership metadata, scatter-gather execution, distributed aggregation (count, sum, min, max, group, filter, sort), cross-regional queries, intermediate result combination.

Research basis: 30 parallel searches returned Citus coordinator/worker fragment planning and result merge, MongoDB mongos targeted vs scatter-gather routing, Aerospike scatter-gather streaming, Dremel serving-tree execution with parallel aggregation on the way up, Drill foreman/fragment model, Presto coordinator/tasks/stages, Calcite optimizer framework, Volcano/Cascades cost-based optimization, Spanner split-region execution plans, CockroachDB per-node planning, two-phase aggregation (local partial + final combine), HyperLogLog approximate distinct count, consistent hashing/vnodes/token ranges for partition ownership, semi-join data reduction, function vs data shipping, top-k threshold algorithms, speculative execution/straggler speculation, graceful partial-result degradation, continuous queries.

A skeleton `ids-query` crate does NOT yet exist at `C:/idfs/ids-query/`. This plan specifies creation from scratch; no source modifications are made in this phase.

---

### 2. Architecture Overview

`ids-query` sits above `ids-000-aa` (types + sealed trait contracts + invariants + errors) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It is layered so planning, decomposition, execution, and combination are independent modules that compose:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / CLIENT LAYER (ids-*)                │
│  submit(query) · receive(final result) · progress callbacks         │
├─────────────────────────────────────────────────────────────────────┤
│                 Phase 015: ids-query (THIS CRATE)                   │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐ │\n│  │   planner   │ │  decomposer  │ │   fragment      │ │ scatter │ │
│  │ (SQL/DSL →  │ │ (partition-  │ │   executor      │ │ gather  │ │
│  │  logical/   │ │  aware       │ │ (per-Endpoint   │ │ executor│ │
│  │  physical   │ │  fragment    │ │  fragment +     │ │ (merge) │ │
│  │  plan)      │ │  plan)       │ │  predicate push)│ │         │ │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘ │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐ │
│  │ aggregation │ │ intermediate │ │ cross-region    │ │ index   │ │
│  │  (count,    │ │  result      │ │  query          │ │ lookup  │ │
│  │   sum, min, │ │  combination │ │  (region routing│ │ (planner│ │
│  │   max, group│ │  (k-way merge│ │  aware, residency │ │  aid)   │ │
│  │   filter,   │ │  + combine)  │ │  constraints)   │ │         │ │
│  │   sort)     │ │              │ │                 │ │         │ │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘ │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
┌──────────────────────────────▼──────────────────────────────────────┐
│              PARTITION / NODE / REGION ENDPOINTS (via ids-carrier)  │
│  ids-read · ids-write · ids-storage · region replicas · …          │
└─────────────────────────────────────────────────────────────────────┘

CARRIER BOUNDARY (no std::net / tokio::net): ids-carrier::Endpoint only
```

**Data flow:** client submits a query → planner produces a logical plan → decomposer maps plan fragments to partitions/nodes/regions via ownership metadata + partition maps → scatter executor dispatches fragments to target Endpoints → each target executes locally (predicate pushdown, index use, local aggregation) → results stream back as opaque `Vec<u8>` → gather executor combines intermediate results (partial-aggregate merge, k-way sorted merge, top-k reduction) → final result returned to client.

**Design rules (non-negotiable):**
- Query decomposition is partition/node/region aware; a fragment targets one or more Endpoints.
- Predicates are pushed down to the fragment's owner so only results (not raw rows) cross Endpoints (function shipping preferred over data shipping).
- All inter-Endpoint traffic is opaque `Vec<u8>` over `ids-carrier::Endpoint`; `ids-query` never addresses by IP/port.
- Crypto: signatures over query fragments (ed25519), result confidentiality (xchacha20poly1305/chacha20poly1305), fragment/result integrity (blake3/sha256). No other algorithms.
- Distributed aggregation uses two-phase: local partial aggregate per fragment + final combine at the coordinator/gatherer; approximate distinct count via HyperLogLog over blake3/sha256 hashes when exactness is not required.
- Cross-regional queries route through region-aware planning; residency constraints are hard filters (fail closed if no in-region Endpoint).
- Partial results / graceful degradation: return partial coverage rather than total failure when a region is unreachable (configurable).
- Idempotent query fragments (re-dispatch on straggler/timeout to an alternate Endpoint); speculative execution for tail-latency.

---

### 3. Detailed Research Dimensions

#### 3.1 Query decomposition — coordinator/fragment model

Citus-style decomposition: a coordinator holds shard/partition metadata, partitions each query into independent fragments per shard, assigns fragments to workers, oversees execution, and merges results. Each fragment runs independently on its shard; no inter-fragment coordination during execution; result merge happens on the way up. Dremel's serving tree generalizes this: a root routes queries down a tree; intermediate servers perform parallel aggregation of partial results on the way up; leaves scan local tablets. Drill's foreman model: the node receiving a query drives it end-to-end, distributing work across cluster for data locality.

**Decision for ids-query:** adopt a coordinator/decomposer model where one Endpoint (or a designated planner Endpoint) owns decomposition, fragments target data-owner Endpoints, and results merge on the way up through a gatherer. Planner may be co-located with the client or a dedicated Endpoint.

#### 3.2 Targeted vs scatter-gather execution

MongoDB mongos: queries including the shard key route to specific shards (targeted); queries without the shard key become scatter-gather broadcast to all shards, then results merged. Scatter-gather is efficient for full-collection scans but inefficient for routine targeted lookups. Aerospike clients execute scatter-gather in parallel across nodes with chunked streaming and cursors.

**Decision for ids-query:** support both — targeted execution when the predicate references a partition/ownership key (route to owning Endpoint only), scatter-gather when it does not (broadcast fragment to all relevant Endpoints, merge results). Ownership metadata decides which.

#### 3.3 Partition maps and ownership metadata

Every partitioned system must answer "which machine owns this key" via routing metadata. Stale routing (one epoch behind) sends requests to a node that already handed the range away, causing divergence; ownership epochs convert racy handoffs into detectable errors. Cassandra uses consistent hashing token ranges; vnodes split each node's range into many small ranges for better balance and minimal movement on topology change. Rendezvous hashing scores every server per key (good for small server sets, no ring).

**Decision for ids-query:** partition map = mapping from `PartitionId` → set of owning `Endpoint`s with an epoch; ownership metadata versioned with epochs so stale maps cause detectable errors, not silent divergence. Consistent hashing (with vnodes) as the default partitioning scheme, pluggable.

#### 3.4 Indexes and predicate pushdown

Distributed indexes have PARTITION (distributes index rows), CLUSTERING (orders rows within partition), INCLUDE (covering columns to avoid table round-trips). Local secondary indexes are async/eventual; global indexes require distributed maintenance. Predicate pushdown pushes selection operators as close to the data source as possible, reducing transmission/computation cost. Partition pruning uses partition metadata to skip irrelevant partitions.

**Decision for ids-query:** index module helps the planner choose indexes and prune partitions; predicates are pushed into fragments so only matching results cross Endpoints. Index lookups themselves target partition-scoped Endpoints.

#### 3.5 Distributed aggregation — two-phase combine

Two-phase aggregation: local partial aggregate per partition/node (map), then hash-redistribute partial states on GROUP BY keys and final total-aggregate (reduce). R2 SQL scatter-gather aggregations compute pre-aggregates on workers and combine at coordinator; shuffling aggregations add HAVING/ORDER BY via local finalization + k-way merge. SkyWalking BanyanDB: map on data nodes, reduce on liaison nodes.

**Decision for ids-query:** aggregation module implements two-phase combine for count/sum/min/max (decomposable, duplicate-insensitive → trivial combine); average requires (sum, count) pairs; group-by uses hash redistribution of partial states on the group keys; filter and sort are embarrassingly parallel per partition with a final k-way merge for sort.

#### 3.6 Approximate distinct count

Exact COUNT(DISTINCT) does not scale across workers (overlapping records; pulling all distinct to one machine doesn't scale). Options: HyperLogLog / HyperLogLog++ sketches with provable error bounds, fixed memory (~c*8 bytes for precision threshold c), ~1.62% relative error at default precision. Elasticsearch cardinality aggregation and Snowflake HLL both use this. Combine is associative (HLL_COMBINE merges states), making it distributed-friendly.

**Decision for ids-query:** offer exact count/sum/min/max and an approximate distinct-count via HyperLogLog over blake3/sha256 hashes, with a configurable error/precision trade-off. Allowed hashes only.

#### 3.7 Cross-regional queries and data residency

Cross-region latency dominates query time; the goal is to minimize data movement — route queries to the region where data lives and avoid cross-region shuffles. Locality-aware optimization incorporates cross-region latency as a first-class cost input to prefer local plans. Data residency/domiciling constrains replicas to approved geographies; region-locked routing sends each request only to endpoints inside the allowed region, covering fallback/load-balancing (fail closed if no in-region endpoint).

**Decision for ids-query:** cross-region module adds region-aware planning: prefer local-region execution, only consult remote regions if needed; residency is a hard filter; all cross-region fragment dispatch is via Endpoint, region metadata from ids-routing/ids-000-aa.

#### 3.8 Intermediate result combination

Intermediate results combine via partial-aggregate merge (sum of counts, max of maxes, etc.) or k-way sorted merge of result streams (coordinator reads row-by-row from each worker, picks winner by sort order, supports early LIMIT termination). Vectorwise-style recycling reuses materialized intermediate results across queries with subsumption and proactive rewriting; materialization vs pipelining is a workload trade-off.

**Decision for ids-query:** gather module combines intermediate results per operator: aggregate merge for group-by, k-way merge for sort, top-k reduction (keep top-k per partition then merge). Optional intermediate-result caching keyed by fragment hash.

#### 3.9 Fault tolerance — stragglers, speculation, partial results

BigQuery/Dremel: deterministic and idempotent tasks allow restarts; straggler detection re-executes slow tasks redundantly past a threshold. Cassandra speculative execution: pre-emptively re-send idempotent queries to another node after a p99.9 latency threshold; whichever replies first wins. Vespa graceful degradation: return partial results with degraded coverage rather than total failure when a query can't complete in time.

**Decision for ids-query:** fragments are idempotent (re-dispatch on straggler/timeout to an alternate Endpoint); speculative re-execution for tail latency; graceful degradation returns partial coverage when regions are unreachable (configurable coverage threshold).

#### 3.10 Query shipping vs data shipping

Function shipping (worker executes query, returns result) vs data shipping (coordinator reads raw data, executes locally) is a fundamental trade-off: function shipping minimizes data movement but needs compute at data; data shipping is simpler but moves more data. Data locality favors shipping the function to the data.

**Decision for ids-query:** default to function shipping (fragments execute at data-owner Endpoints, only results return); data shipping available as an opt-in for small result sets where shipping raw data is cheaper.

#### 3.11 Top-k execution

Distributed top-k uses threshold algorithms (TA, NRA) and variants (TPUT three-phase uniform threshold); KLEE trades result quality vs performance/bandwidth. Massively parallel top-k: local top-k per partition (bounded to k), then merge reduces candidate sets; sorted index access ~20× faster than random access for large lists.

**Decision for ids-query:** sort+limit pushed to fragments as local top-k; gatherer merges per-partition top-k streams into a global top-k (k-way merge with early termination).

#### 3.12 Continuous queries

Continuous queries run over dynamic (streaming) tables and never terminate, producing dynamic results semantically equivalent to batch re-execution on snapshots (Flink dynamic tables, BigQuery continuous queries, Hazelcast stream SQL). Distinct from batch; long-running, backpressured, restartable.

**Decision for ids-query:** continuous-query support is a documented future dimension (out of scope for Phase 015 MVP); the planner/fragment/executor structure is designed to be extensible to streaming.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Fragment interception/tampering in transit | ed25519 signature over fragment + payload; blake3/sha256 integrity; verification at target before execution |
| Result confidentiality | xchacha20poly1305 / chacha20poly1305 encryption of result payloads between Endpoints |
| Replay of a fragment | Nonce + per-Endpoint sequence; replay cache bounded with TTL eviction |
| Stale partition map → wrong owner | Ownership epochs; epoch mismatch is a detectable error, triggers map refresh |
| Straggler/timeout stalls query | Idempotent fragment re-dispatch to alternate Endpoint; speculative execution; configurable timeouts |
| Partial region failure → total failure | Graceful degradation: return partial coverage rather than fail; configurable coverage threshold |
| Distinct-count accuracy | HyperLogLog approximate with explicit error bounds; exact mode available |
| Metadata bomb in partition map | Size/depth budgets on ownership metadata before parsing |
| Cross-region data residency violation | Region is a hard filter; fail closed if no in-region Endpoint |
| Unauthorized query execution | Query fragment signed by client key; target verifies signature + grant (ed25519) |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 015 |
|-------|-------------------|
| `ids-000-aa` | `NodeId`, `RegionId`, `PartitionId`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, sealed trait contracts, invariants, errors — reused verbatim |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); all traffic rides here; no direct socket/transport imports |
| `ids-crypto` | ed25519 sign/verify, xchacha20poly1305, chacha20poly1305, sha256, blake3 — type-level + primitive IDs; no new algorithms |
| `ids-read` | Local read path consumed by fragments executing at data-owner Endpoints |
| `ids-write` | Not a dependency (query is read-only); write path out of scope |
| `ids-routing` | Region-aware / locality-aware Endpoint resolution for cross-regional and targeted routing |
| `ids-broadcast` | Future: reliable/causal/total-order delivery of fragments if needed |
| `ids-durability-policy` | Future: durability conditions per query |

**Integration points:**
- `ids-query` produces fragment payloads (opaque `Vec<u8>`) dispatched via `ids-carrier::Endpoint`; targets execute locally using `ids-read`.
- Partition maps and ownership metadata derive from `ids-000-aa` types (`PartitionId`, `RegionId`, `Endpoint`, epochs).
- Region routing for cross-regional queries delegates to `ids-routing`; no `ids-query` knowledge of geography beyond `RegionId`.
- All fragment/result signing and encryption uses `ids-crypto` primitives only.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-query` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-read`, `ids-routing`; serde, thiserror, bytes, blake3, zeroize)

```
ids-query/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports, QueryEngine orchestrator
├── constants.rs                    # Protocol defaults (timeouts, batch sizes, coverage thresholds)
├── error.rs                        # QueryError (thiserror): planning, decomposition, execution, aggregation
├── features.rs                     # Feature flags (approximate, continuous, fuzzing)
│
├── planner/                        # Query planning: SQL/DSL → logical/physical plan
│   ├── mod.rs
│   ├── planner.rs                  # Planner orchestrator
│   ├── logical_plan.rs             # Logical plan (relational operators)
│   ├── physical_plan.rs            # Physical plan (fragmentable operators)
│   ├── parser.rs                   # Query parser (DSL/SQL subset)
│   ├── validator.rs                # Semantic validation against schema/catalog
│   ├── optimizer.rs                # Rule-based optimizer (predicate pushdown, projection pruning)
│   ├── cost_model.rs               # Cost model incl. cross-region latency
│   ├── plan_fragment.rs            # PlanFragment (unit of execution, targets Endpoint set)
│   ├── planner_types.rs            # PlanId, Operator, Trait structures
│   └── planner_proptests.rs        # Property-based planning tests
│
├── decomposition/                  # Query decomposition: plan → fragments → Endpoints
│   ├── mod.rs
│   ├── decomposer.rs               # Decomposer orchestrator
│   ├── partition_map.rs            # PartitionId → Endpoint set + epoch
│   ├── ownership.rs                # Ownership metadata, epoch versioning, staleness detection
│   ├── router.rs                   # Maps fragments to target Endpoints (targeted vs scatter-gather)
│   ├── fragment_assigner.rs        # Assigns fragments to Endpoints, load awareness
│   ├── targeted.rs                 # Targeted execution (partition-key predicate → owning Endpoint)
│   ├── scatter_gather.rs           # Scatter-gather (broadcast fragment to all relevant Endpoints)
│   ├── decomposition_types.rs      # FragmentId, Assignment, RoutingDecision structures
│   └── decomposition_proptests.rs
│
├── executor/                       # Fragment execution at target Endpoints
│   ├── mod.rs
│   ├── fragment_executor.rs        # Executes a PlanFragment locally
│   ├── predicate_pushdown.rs       # Pushes selection into fragment (function shipping)
│   ├── index_lookup.rs             # Index-assisted scan (partition-scoped)
│   ├── partition_scan.rs           # Local partition scan
│   ├── local_aggregate.rs          # Local partial aggregate (count/sum/min/max/group)
│   ├── local_sort.rs               # Local sort + top-k
│   ├── local_filter.rs             # Local filter
│   ├── data_shipping.rs            # Opt-in data shipping path
│   └── executor_types.rs           # FragmentResult, ScanCursor structures
│
├── gatherer/                       # Intermediate result combination
│   ├── mod.rs
│   ├── gatherer.rs                 # Gatherer orchestrator (merge on the way up)
│   ├── aggregate_merge.rs          # Merge partial aggregates (sum of counts, max of maxes…)
│   ├── kway_merge.rs               # K-way sorted merge of result streams
│   ├── topk_reduce.rs              # Top-k reduction across partitions
│   ├── intermediate_cache.rs       # Optional cached intermediate results keyed by fragment hash
│   ├── combination_types.rs        # PartialResult, CombinedResult structures
│   └── gatherer_proptests.rs
│
├── aggregation/                    # Distributed aggregation operators
│   ├── mod.rs
│   ├── count.rs                    # Distributed count
│   ├── sum.rs                      # Distributed sum
│   ├── min_max.rs                  # Distributed min/max
│   ├── group_by.rs                 # Distributed group-by (two-phase)
│   ├── filter.rs                   # Distributed filter (embarrassingly parallel)
│   ├── sort.rs                     # Distributed sort (local sort + k-way merge)
│   ├── hll_distinct.rs             # Approximate distinct count (HyperLogLog over blake3/sha256)
│   ├── avg.rs                      # Distributed average (sum+count pair)
│   ├── aggregation_types.rs        # AggregateState, CombineMode structures
│   └── aggregation_proptests.rs
│
├── cross_region/                   # Cross-regional queries & residency
│   ├── mod.rs
│   ├── region_router.rs            # Region-aware fragment routing
│   ├── residency_filter.rs         # Hard residency filter (fail closed)
│   ├── locality_cost.rs            # Cross-region latency cost input
│   ├── cross_region_types.rs       # RegionId, ResidencyConstraint structures
│   └── cross_region_proptests.rs
│
├── fault_tolerance/                # Stragglers, speculation, graceful degradation
│   ├── mod.rs
│   ├── straggler_detector.rs       # Detects slow fragments past threshold
│   ├── speculative_executor.rs     # Speculative re-execution of idempotent fragments
│   ├── replay_cache.rs             # Nonce + sequence replay cache (bounded, TTL)
│   ├── graceful_degradation.rs     # Partial coverage vs total failure
│   ├── idempotence.rs              # Idempotent fragment keys (re-dispatch safe)
│   └── fault_tolerance_types.rs
│
├── security/                       # Fragment/result signing & encryption
│   ├── mod.rs
│   ├── fragment_signer.rs          # ed25519 signing of query fragments
│   ├── fragment_verifier.rs        # Signature verification at target before execution
│   ├── result_encryptor.rs         # xchacha20poly1305/chacha20poly1305 result encryption
│   ├── result_decryptor.rs
│   ├── integrity.rs                # blake3/sha256 fragment+result integrity
│   ├── nonce_manager.rs            # Nonce issuance + single-use enforcement
│   └── security_types.rs
│
├── transport/                      # Carrier binding (opaque only)
│   ├── mod.rs
│   ├── endpoint_binding.rs         # ids-carrier::Endpoint send/receive binding
│   ├── datagram.rs                 # Opaque Vec<u8> datagram wrapper
│   ├── transport_types.rs
│   └── transport_proptests.rs
│
├── config/                         # Configuration
│   ├── mod.rs
│   ├── query_config.rs             # Query defaults (timeouts, batch sizes)
│   ├── aggregation_config.rs       # Aggregation defaults
│   ├── fault_tolerance_config.rs   # Speculative/degradation thresholds
│   └── security_config.rs          # Signing/encryption config
│
├── metrics/                        # Metrics & tracing
│   ├── mod.rs
│   ├── query_metrics.rs            # Query lifecycle metrics
│   ├── fragment_metrics.rs         # Fragment dispatch/execution metrics
│   ├── aggregation_metrics.rs      # Aggregation combine metrics
│   └── fault_tolerance_metrics.rs
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_planner.rs
│   ├── fuzz_decomposition.rs
│   ├── fuzz_executor.rs
│   ├── fuzz_gatherer.rs
│   └── fuzz_security.rs
│
├── tests/                          # Integration & adversarial tests
│   ├── mod.rs
│   ├── planner_tests.rs            # Plan generation & optimization
│   ├── decomposition_tests.rs      # Targeted vs scatter-gather routing
│   ├── executor_tests.rs           # Fragment execution + predicate pushdown
│   ├── gatherer_tests.rs           # Intermediate result combination
│   ├── aggregation_tests.rs        # Two-phase combine correctness
│   ├── cross_region_tests.rs       # Region routing + residency
│   ├── fault_tolerance_tests.rs    # Straggler/speculation/degradation
│   ├── security_tests.rs           # Signing/verification/encryption
│   ├── integration_tests.rs        # End-to-end over ids-carrier Endpoint
│   ├── adversarial_tests.rs        # Stale map, replay, tamper, partial failure
│   ├── determinism_tests.rs        # Same input → identical result
│   └── property_tests.rs
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── planner_bench.rs
│   ├── decomposition_bench.rs
│   ├── aggregation_bench.rs        # Two-phase combine bandwidth
│   ├── gatherer_bench.rs           # K-way merge latency
│   └── cross_region_bench.rs
│
├── spec/                           # Normative specifications
│   ├── query_plan_spec.md          # Query plan format & lifecycle
│   ├── decomposition_spec.md       # Fragment → Endpoint mapping
│   ├── aggregation_spec.md         # Distributed aggregation combine rules
│   ├── cross_region_spec.md        # Cross-regional & residency rules
│   └── threat_model.md             # Threat model document
│
├── docs/                           # Documentation
│   ├── architecture.md
│   ├── integration_guide.md        # Integrating with ids-read/ids-routing/ids-carrier
│   ├── query_language.md           # Supported DSL/SQL subset
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── plan_vectors.json
    ├── aggregation_vectors.json
    └── cross_region_vectors.json
```

**Why ~200–300 files:** one file per responsibility — planner, decomposer, partition map, ownership, router, executor per operator, gatherer per combination mode, aggregation per function, cross-region, fault tolerance, security, transport binding, tests, fuzz, benchmarks, specs — reflects the genuine state-machine and decomposition complexity of distributed query planning/execution. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Query plan & fragment

```
Query { id: PlanId, sql/dsl, logical_plan, physical_plan }
PlanFragment { id: FragmentId, operator_tree, target_endpoints: Vec<Endpoint>,
               predicate_pushdown: bool, idempotent: bool, nonce: [u8;32] }
FragmentResult { fragment_id, partial_state: Vec<u8>, coverage: f32, blake3_digest }
```

#### 7.2 Partition map & ownership (epoch-versioned)

```
PartitionMap { entries: Map<PartitionId, PartitionOwnership> }
PartitionOwnership { endpoints: Vec<Endpoint>, epoch: u64, version: u64 }
// lookup(partition) -> owning Endpoint set; epoch mismatch → detectable error → refresh
```

#### 7.3 Two-phase aggregation combine

```
// Phase 1 (per fragment): partial = aggregate(local_rows)
// Phase 2 (gatherer): combined = combine(partial_1, …, partial_n)
// count: sum; sum: sum; min: min; max: max; avg: (sum,count) → sum/count
// group-by: hash-redistribute partial states on group keys, then total aggregate
// distinct: HLL_COMBINE(partial sketches) → HLL_ESTIMATE
```

#### 7.4 Scatter-gather vs targeted routing

```
// targeted: predicate references partition key → route fragment to owning Endpoint only
// scatter-gather: no partition key → broadcast fragment to all relevant Endpoints,
//                 gatherer merges results (dedup if overlapping replicas)
```

#### 7.5 K-way sorted merge (sort + top-k)

```
// each fragment returns its local top-k sorted stream
// gatherer opens a stream per fragment, compares current row by sort order,
// emits winner, advances that stream; early termination on LIMIT
```

#### 7.6 Speculative re-execution

```
// fragment dispatched to primary Endpoint; if no response by p99.9 threshold,
// re-dispatch idempotent fragment to alternate Endpoint; first valid result wins;
// cancelled request still executes at primary (idempotent, no side effects)
```

#### 7.7 Graceful degradation

```
// coverage = fraction of target Endpoints that returned valid results
// if coverage >= threshold → return partial result with degraded coverage flag
// else → fail the query (configurable)
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (NodeId, RegionId, PartitionId, Endpoint, CarrierType, …), sealed trait contracts, invariants, errors |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); all traffic |
| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 |
| `ids-read` | Local read path consumed by fragments at data-owner Endpoints |
| `ids-routing` | Region-aware / locality-aware Endpoint resolution |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Hashing (workspace dependency; allowed) |
| `zeroize` | Key material zeroing |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-query` is carrier-agnostic over `ids-carrier::Endpoint`. No crypto outside the allowed set.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-015.1 | Query decomposition | A query referencing a partition key decomposes into one fragment targeting the owning Endpoint; non-partitioned queries become scatter-gather over all relevant Endpoints (unit test) |
| AC-015.2 | Targeted execution | Query with partition-key predicate executes only at the owner; no broadcast to unrelated Endpoints (trace assertion) |
| AC-015.3 | Scatter-gather merge | Full-collection query returns combined results from all Endpoints; duplicates from overlapping replicas deduped (integration test) |
| AC-015.4 | Predicate pushdown | Only matching rows/results cross Endpoints; raw rows are not shipped unless data-shipping opt-in (integration test) |
| AC-015.5 | Two-phase aggregation | count/sum/min/max combine correctly across fragments under reorder/loss/dup (property test, 100k iterations) |
| AC-015.6 | Group-by | Distributed group-by matches single-node result on identical data (integration test) |
| AC-015.7 | Approximate distinct count | HyperLogLong result within configured error bound of exact count (unit test) |
| AC-015.8 | Cross-region routing | Query prefers local-region execution; residency constraint fails closed when no in-region Endpoint (unit test) |
| AC-015.9 | Sort + top-k | Distributed sort produces globally sorted stream; top-k early termination returns k rows (integration test) |
| AC-015.10 | Fault tolerance | Idempotent fragment re-dispatched to alternate Endpoint on straggler/timeout; first valid result wins (chaos test) |
| AC-015.11 | Speculative execution | Tail-latency query completed by speculative replica; cancelled primary still executes idempotently (chaos test) |
| AC-015.12 | Graceful degradation | Partial result returned with degraded coverage when a region is unreachable, above threshold (chaos test) |
| AC-015.13 | Security | Fragment signed ed25519; target verifies before execution; tampered fragment rejected; result encrypted xchacha20poly1305 (security test) |
| AC-015.14 | Replay resistance | Replayed fragment rejected by nonce+sequence replay cache (bounded, TTL) (security test) |
| AC-015.15 | Carrier agnosticism | `ids-query` compiles and behaves identically with zero direct dependencies on TCP/IP, UDP, QUIC, HTTP, DNS, libp2p; all traffic via `ids-carrier::Endpoint` (build + audit) |
| AC-015.16 | Crypto constraint | Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 used; no other algorithms (dependency audit) |
| AC-015.17 | Determinism | Same query + same partition map → identical result (determinism test) |

---

### 10. Remaining Uncertainties

1. **Query language scope:** Which DSL/SQL subset does `ids-query` support natively (SELECT/WHERE/GROUP BY/ORDER BY/LIMIT vs full SQL)? Defer detailed grammar to implementation; planner designed to be extensible.
2. **Planner Endpoint selection:** Is the planner co-located with the client, a dedicated Endpoint, or elected? All three work; decision deferred to deployment topology.
3. **Continuous queries:** Streaming/continuous query support is out of scope for Phase 015 MVP; the fragment/executor structure is designed to be extensible but not specified here.
4. **Intermediate cache eviction:** Optional cached intermediate results keyed by fragment hash; eviction policy (size/age/ttl) unresolved.
5. **Coverage threshold default:** The graceful-degradation coverage threshold is workload-dependent; default is a placeholder pending measurement.
6. **HLL precision default:** Approximate distinct-count precision threshold (memory vs error trade-off) needs workload tuning.
7. **Join support:** Distributed joins (broadcast join vs shuffle join) are documented as a future dimension; not in Phase 015 contents.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Coordinator/decomposer + gatherer model (Citus/Dremel/Drill) | Decomposition is coordinator-driven; workers scan local partitions in parallel; results merge on the way up; matches IDS partition/node/region model |
| Targeted execution when partition-key predicate present; scatter-gather otherwise | MongoDB mongos precedent: targeted is efficient, scatter-gather is the fallback for unkeyed scans |
| Predicate pushdown / function shipping default | Minimizes data movement across Endpoints; only results cross the carrier boundary |
| Two-phase aggregation (partial + combine) | count/sum/min/max/group decomposable; combine is associative; bounded coordinator memory |
| Approximate distinct count via HyperLogLog (blake3/sha256) | Exact COUNT(DISTINCT) doesn't scale across workers; HLL has fixed memory and provable error bounds |
| Epoch-versioned partition map + ownership metadata | Stale maps cause detectable errors, not silent divergence; consistent hashing with vnodes as default partitioning |
| Region-aware planning + hard residency filter | Cross-region latency is a first-class cost input; residency fails closed |
| Idempotent fragments + speculative re-execution + graceful degradation | Fault tolerance for stragglers, timeouts, and partial region failure without total query failure |
| All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Carrier-agnostic absolute rule (CRATES.md §9); no IP/TCP/UDP/QUIC/HTTP/DNS/libp2p |
| Crypto restricted to ed25519 / xchacha20poly1305 / chacha20poly1305 / sha256 / blake3 | Allowed-crypto mandate (CRATES.md; prompts.txt) |
| New `ids-query` crate, not subfolder | Separate crate per IDS convention (CRATES.md §15); keeps existing crates focused |

---

### 12. Sources

- Citus Query Processing (coordinator/worker, fragments, result merge): https://docs.citusdata.com/en/latest/develop/reference_processing.html
- MongoDB Distributed Queries (targeted vs scatter-gather, mongos): https://www.mongodb.com/docs/manual/core/distributed-queries
- Aerospike Query Result Streams (scatter-gather streaming, cursors): https://aerospike.com/blog/working-with-query-result-streams/
- Dremel: Interactive Analysis of Web-Scale Datasets (serving tree, parallel aggregation): https://cacm.acm.org/research/dremel-interactive-analysis-of-web-scale-datasets/
- Dremel VLDB 2010 (serving tree, tablet scan, partial aggregation): https://www.vldb.org/pvldb/vol3/R29.pdf
- Apache Drill Query Execution (foreman, fragments, data locality): https://drill.apache.org/docs/drill-query-execution/
- Presto Concepts (coordinator, tasks, stages, workers): https://prestodb.github.io/docs/current/overview/concepts.html
- Apache Calcite (optimizer framework, chooseDelegate for distributed): https://calcite.apache.org/docs
- Extensible Query Optimizers in Practice (Volcano/Cascades): https://www.microsoft.com/en-us/research/wp-content/uploads/2024/12/Extensible-Query-Optimizers-in-Practice.pdf
- CMU 15-721 Cascades/Volcano notes: https://www.cs.cmu.edu/~15721-f24/notes/03_QO2.pdf
- TiDB Predicate Push Down: https://docs.pingcap.com/tidb/stable/predicate-push-down
- Google Spanner Query Execution Plans (split-region): https://cloud.google.com/spanner/docs/query-execution-plans
- CockroachDB SQL Layer (per-node planning): https://docs.cockroachlabs.com/docs/v26.3/architecture/sql-layer
- CMU 15-721 BigQuery/Dremel (shuffle, deterministic idempotent tasks, stragglers): https://15721.courses.cs.cmu.edu/spring2024/notes/17-bigquery.pdf
- PostgreSQL Sort (quicksort/external merge): http://airbyte.com/blog/postgresql-query-plans-for-sorting-data
- Bitonic top-k massively parallel sort: https://spiral.imperial.ac.uk/server/api/core/bitstreams/00d931d0-ec66-4a4b-b5c1-f8aa214e37a4/content
- Drill Aggregation Optimization (two-phase hash aggregate): https://drill.apache.org/docs/guidelines-for-optimizing-aggregation
- Citus HyperLogLog distinct count: https://learn.microsoft.com/en-us/postgresql/citus/hyperloglog-count-distinct?view=citus-14
- Elasticsearch Cardinality (HyperLogLog++): https://www.elastic.co/docs/reference/aggregations/search-aggregations-metrics-cardinality-aggregation
- Cassandra Consistent Hashing / Dynamo: https://cassandra.apache.org/doc/latest/cassandra/architecture/dynamo.html
- DataStax Consistent Hashing (vnodes, token ranges): https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/architecture/archDataDistributeHashing.html
- Consistent hashing explainer: https://eli.thegreenplace.net/2025/consistent-hashing
- Cassandra speculative execution (driver): https://docs.datastax.com/en/developer/java-driver/latest/manual/core/speculative_execution/index.html
- Vespa Graceful Query Coverage Degradation: https://docs.vespa.ai/en/performance/graceful-degradation.html
- Shard Routing and Ownership Metadata (epochs, staleness): https://mercemay.com/articles/databases/replication-and-sharding/shard-routing-and-ownership-metadata/
- Vectorwise Recycling in Pipelined Query Evaluation (intermediate reuse): https://ir.cwi.nl/pub/21352/21352A.pdf
- Intermittent Query Processing (materialized intermediates): https://www.vldb.org/pvldb/vol12/p1427-tang.pdf
- Decomposition/pipelining/intermediate result sharing (ICDE 1986): http://store.computer.org/csdl/proceedings-article/icde/1986/07266210/12OmNxd4too
- Efficient Processing of Distributed Top-k Queries (threshold algorithms): https://cs.ucsb.edu/sites/default/files/documents/2005-14.pdf
- KLEE Distributed Top-k Framework: http://www.vldb.org/archives/website/2005/program/paper/thu/p637-michel.pdf
- To Ship or Not to (Function) Ship (function vs data shipping): https://arxiv.org/html/1807.11149v1
- Scheduling Queries Across Replicas (replica selection, efficiency prediction): http://www.dcs.gla.ac.uk/~craigm/publications/freire12scheduling.pdf
- ClickHouse Cross-Region Query Performance: https://oneuptime.com/blog/post/2026-03-31-clickhouse-optimize-cross-region-query-performance/view
- CockroachDB Multi-Region Architecture (locality-aware, data domiciling): https://www.cockroachlabs.com/blog/multi-region-database-architecture-sql-placement-locality
- Data Residency Routing (fail closed, in-region only): https://tetrate.io/learn/ai/data-residency-routing
- OpenRouter In-Region Routing: https://openrouter.ai/docs/guides/features/in-region-routing
- Flink Dynamic Tables / Continuous Queries: https://nightlies.apache.org/flink/flink-docs-master/docs/concepts/sql-table-concepts/dynamic_tables/
- BigQuery Continuous Queries: https://docs.cloud.google.com/bigquery/docs/continuous-queries-introduction
- Hazelcast Stream Processing in SQL: https://docs.hazelcast.com/hazelcast/latest/sql/querying-streams
- IDS CRATES.md entry 15 (ids-query): C:/idfs/CRATES.md line 140
- IDS prompts.txt entry 15 (lines 220-234): C:/idfs/prompts.txt

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 015.
