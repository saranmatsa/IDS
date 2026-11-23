# IDS Phase 015 — ids-query: Detailed Research Results

**Topic:** Create the `ids-query` crate: Query decomposition across partitions, nodes, regions (distributed query planning and execution).
**Methodology:** 30 targeted web searches (Q001-Q030) executed in one parallel batch. Each result classified ESTABLISHED / HYPOTHESIS / SPECULATION and checked against constraints (carrier-agnostic via ids-carrier::Endpoint; allowed crypto {ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305}; ids-000-aa master crate; research only).

Constraint note: every finding below satisfies the carrier-agnostic and crypto constraints — none requires TCP/IP, libp2p, HTTP, DNS, etc.; none requires crypto outside the allowed set.

---

## Q001 — Distributed query planning decomposition across partitions, nodes, regions

**Classification:** ESTABLISHED  
**Key finding:** Citus (distributed PostgreSQL) uses a coordinator/worker model: the coordinator stores shard metadata, partitions each query into independent fragments per shard, assigns fragments to workers, oversees execution, and merges results. Query decomposition is coordinator-driven; workers scan local shards in parallel; result merge happens on the way up.  
**Source:** https://docs.citusdata.com/en/latest/develop/reference_processing.html  
**Constraint check:** Satisfied — decomposition logic is carrier-agnostic; results ship as opaque bytes.

## Q002 — Scatter-gather execution pattern distributed database query

**Classification:** ESTABLISHED  
**Key finding:** MongoDB mongos routes targeted queries (with shard key) to specific shards; queries without the shard key become scatter-gather broadcast to all shards, then results are merged. Scatter-gather is efficient for full-collection scans but inefficient for routine targeted lookups. Aerospike clients execute scatter-gather in parallel across nodes with chunked streaming and cursors.  
**Sources:** https://www.mongodb.com/docs/manual/core/distributed-queries ; https://aerospike.com/blog/working-with-query-result-streams/  
**Constraint check:** Satisfied — scatter-gather targets opaque Endpoint IDs, no network assumptions.

## Q003 — Distributed aggregation count sum min max group by two phase aggregation

**Classification:** ESTABLISHED  
**Key finding:** Distributed aggregation uses two phases: local partial aggregation per partition/node (map), then hash-based redistribution of partial states on GROUP BY keys followed by final total aggregation (reduce). R2 SQL "scatter-gather aggregations" compute pre-aggregates on workers and combine at coordinator; "shuffling aggregations" add HAVING/ORDER BY via local finalization + k-way merge. SkyWalking BanyanDB: map on data nodes, reduce on liaison nodes.  
**Sources:** https://blog.cloudflare.com/r2-sql-aggregations ; https://skywalking.apache.org/docs/skywalking-banyandb/latest/concept/distributed-measure-aggregation ; https://drill.apache.org/docs/guidelines-for-optimizing-aggregation  
**Constraint check:** Satisfied — partial states are opaque Vec<u8> over Endpoint.

## Q004 — Partition map ownership metadata distributed query routing

**Classification:** ESTABLISHED  
**Key finding:** Every partitioned system must answer "which machine owns this key" via routing metadata (directory/token range table). Stale routing (one epoch behind) sends writes to a node that already handed the range away, causing divergence; ownership epochs convert racy handoffs into detectable errors. Cassandra uses consistent hashing token ranges; vnodes split ranges into many smaller ranges per node.  
**Sources:** https://mercemay.com/articles/databases/replication-and-sharding/shard-routing-and-ownership-metadata/ ; https://cassandra.apache.org/doc/latest/cassandra/architecture/dynamo.html ; https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/architecture/archDataDistributeHashing.html  
**Constraint check:** Satisfied — ownership metadata maps keys to opaque Endpoint IDs.

## Q005 — Distributed index design partitioned tables query execution

**Classification:** ESTABLISHED  
**Key finding:** Distributed indexes have three components: PARTITION (distributes index rows), CLUSTERING (orders rows within partition), INCLUDE (covering columns to avoid table round-trips). Yugabyte: local secondary indexes are async (eventual consistency); global indexes require distributed index maintenance. Partition pruning uses index partition metadata to skip irrelevant partitions.  
**Source:** https://www.yugabyte.com/blog/design-indexes-query-performance-distributed-database/  
**Constraint check:** Satisfied — index lookups target Endpoint-scoped partitions.

## Q006 — Cross regional distributed query execution latency optimization

**Classification:** ESTABLISHED  
**Key finding:** Cross-region latency dominates query time; the goal is to minimize data movement — route queries to the region where data lives and avoid cross-region shuffles. ClickHouse cross-region optimization tunes data locality, query routing, and partition pruning. CockroachDB locality-aware query optimization: cross-region exchanges become the primary latency driver; local-first planning and region-aligned joins avoid cross-region shuffles.  
**Sources:** https://oneuptime.com/blog/post/2026-03-31-clickhouse-optimize-cross-region-query-performance/view ; https://www.cockroachlabs.com/blog/multi-region-database-architecture-sql-placement-locality  
**Constraint check:** Satisfied — region routing via ids-routing/Endpoint, no network assumptions.

## Q007 — Intermediate result combination distributed query reduce phase

**Classification:** ESTABLISHED  
**Key finding:** Intermediate results are combined via k-way merge of sorted partial streams (coordinator merges row-by-row, picks winner by sort order, supports early LIMIT termination) or via partial-state aggregation merge (sum of counts, etc.). Vectorwise "recycling" reuses materialized intermediate results across queries via a recycler cache with subsumption and proactive rewriting; materialization cost vs pipelined trade-off is workload-dependent.  
**Sources:** https://blog.cloudflare.com/r2-sql-aggregations ; https://ir.cwi.nl/pub/21352/21352A.pdf  
**Constraint check:** Satisfied — intermediate results are opaque bytes shipped over Endpoint.

## Q008 — Apache Dremel distributed query execution architecture tree

**Classification:** ESTABLISHED  
**Key finding:** Dremel executes queries via a multi-level serving tree: root receives queries, reads tablet metadata, routes to intermediate servers; leaves scan tablets in parallel; on the way up, intermediate servers perform parallel aggregation of partial results. Root rewrites `SELECT A, COUNT(B) FROM T GROUP BY A` into per-tablet partial aggregates then final `GROUP BY`. Supports broadcast join (small tables shipped to leaves) and shuffle-based repartitioned joins. Serving tree parallelizes scheduling and aggregation and tolerates stragglers/faults.  
**Sources:** https://cacm.acm.org/research/dremel-interactive-analysis-of-web-scale-datasets/ ; https://www.vldb.org/pvldb/vol3/R29.pdf  
**Constraint check:** Satisfied — tree levels route to opaque nodes/Endpoints.

## Q009 — Apache Drill distributed query engine planning execution

**Classification:** ESTABLISHED  
**Key finding:** Drillbit receiving a query becomes the Foreman (drives the query): parses SQL into logical plan, converts to physical plan with distributed operators, distributes work across cluster for data locality. Embarrassingly parallel operators (projection, filter) run independently per partition with no inter-executor coordination. Aggregates split into partial aggregate per partition + final aggregate.  
**Sources:** https://drill.apache.org/docs/drill-query-execution/ ; https://howqueryengineswork.com/15-distributed-query.html  
**Constraint check:** Satisfied — Foreman/Executor model maps to coordinator/Endpoint.

## Q010 — Presto Trino distributed query execution coordinator workers

**Classification:** ESTABLISHED  
**Key finding:** Presto coordinator parses statements, plans queries, manages workers; creates a logical model of stages translated into connected tasks on workers. Coordinator is the "brain" clients connect to. Workers execute tasks. Coordinator tracks worker activity and query execution. No transport assumptions exposed at the planning layer.  
**Source:** https://prestodb.github.io/docs/current/overview/concepts.html  
**Constraint check:** Satisfied — coordinator/worker split is logical; transport via Endpoint.

## Q011 — Apache Calcite distributed query planning optimizer

**Classification:** ESTABLISHED  
**Key finding:** Calcite is a query optimization framework (parser, validator, cost-based optimizer) without storage or execution — add data via adapters. Uses relational algebra, transformation rules, cost model. `RelOptPlanner.chooseDelegate()` negotiates which schema has most knowledge for distributed queries. Extensible Volcano/Cascades-style optimizer; Calcite-based engines (Ignite) build distributed execution on top.  
**Sources:** https://calcite.apache.org/docs ; https://calcite.apache.org/javadocAggregate/org/apache/calcite/plan/RelOptPlanner.html  
**Constraint check:** Satisfied — Calcite is planner-only; execution transport is pluggable.

## Q012 — Volcano optimizer Cascades distributed query optimization

**Classification:** ESTABLISHED  
**Key finding:** Volcano/Cascades are extensible rule-based optimizers: logical transformation rules + implementation rules (logical→physical operators) + MEMO structure + cost-based dynamic programming. Cascades replaces Volcano's recursion with stack-based task exploration, mixes generation and cost estimation, uses ordering by "promise". Microsoft SQL Server's optimizer uses Cascades; Calcite adopts Volcano.  
**Sources:** https://www.microsoft.com/en-us/research/wp-content/uploads/2024/12/Extensible-Query-Optimizers-in-Practice.pdf ; https://www.cs.cmu.edu/~15721-f24/notes/03_QO2.pdf  
**Constraint check:** Satisfied — optimizer is pure computation, carrier-agnostic.

## Q013 — Predicate pushdown distributed queries partition pruning

**Classification:** ESTABLISHED  
**Key finding:** Predicate pushdown pushes selection operators as close to the data source as possible to reduce transmission/computation cost. Applicable cases push predicates to storage layer; inapplicable cases (e.g., predicates over non-pushable expressions) are not pushed. TiDB documents PPD cases precisely. Partition pruning uses partition metadata to skip irrelevant partitions.  
**Source:** https://docs.pingcap.com/tidb/stable/predicate-push-down  
**Constraint check:** Satisfied — pushdown targets partition-scoped Endpoints.

## Q014 — Google Spanner distributed query execution across regions

**Classification:** ESTABLISHED  
**Key finding:** Spanner divides data into splits that can move independently and be assigned to servers in different regions. Query execution plans are sets of steps for obtaining results; efficient plans are hard because splits are region-distributed. Spanner uses declarative SQL; execution plan determines how results are obtained across splits.  
**Source:** https://cloud.google.com/spanner/docs/query-execution-plans  
**Constraint check:** Satisfied — split-to-server mapping via Endpoint, no network assumptions.

## Q015 — CockroachDB distributed SQL query planning execution

**Classification:** ESTABLISHED  
**Key finding:** CockroachDB each node parses SQL → AST → logical plan (semantic analysis) → physical plan → executes. Logical planning includes join reordering, etc. SQL layer is PostgreSQL wire-compatible. Distributed execution ships plan fragments to nodes owning the data.  
**Source:** https://docs.cockroachlabs.com/docs/v26.3/architecture/sql-layer  
**Constraint check:** Satisfied — plan fragments target opaque node Endpoints.

## Q016 — Distributed sort external merge sort query execution

**Classification:** ESTABLISHED  
**Key finding:** Distributed sort sorts local partitions then merges sorted streams (k-way merge). PostgreSQL Sort node chooses quicksort/external merge based on memory; distributed sort extends this across partitions with streaming merge. Bitonic sort is a massively parallel sort primitive for top-k. Local sort + merge + rebuild reduces top-k candidates in parallel.  
**Sources:** http://airbyte.com/blog/postgresql-query-plans-for-sorting-data ; https://spiral.imperial.ac.uk/server/api/core/bitstreams/00d931d0-ec66-4a4b-b5c1-f8aa214e37a4/content  
**Constraint check:** Satisfied — sorted runs are opaque streams over Endpoint.

## Q017 — Distributed group by hash aggregation two phase

**Classification:** ESTABLISHED  
**Key finding:** Drill performs GROUP BY in 1 or 2 phases using Hash Aggregate or Streaming Aggregate. Default is 2-phase: local partial aggregation per minor fragment, hash-based redistribution on GROUP BY keys, then total aggregation per fragment. Efficient when grouping keys have duplicates (reduction); 1-phase better when little reduction.  
**Source:** https://drill.apache.org/docs/guidelines-for-optimizing-aggregation  
**Constraint check:** Satisfied — partial aggregates are opaque states over Endpoint.

## Q018 — Distinct count distributed aggregation approximate

**Classification:** ESTABLISHED  
**Key finding:** Exact COUNT(DISTINCT) doesn't scale across workers (overlapping records). Options: pull all distinct to one machine (doesn't scale), map/reduce (scales but slow), or approximation sketches (HyperLogLog) with provable error bounds. Elasticsearch cardinality aggregation uses HyperLogLog++ with precision_threshold (default 3000, ~c*8 bytes memory, error grows above threshold). Snowflake HLL: ~1.62% relative error.  
**Sources:** https://learn.microsoft.com/en-us/postgresql/citus/hyperloglog-count-distinct?view=citus-14 ; https://www.elastic.co/docs/reference/aggregations/search-aggregations-metrics-cardinality-aggregation  
**Constraint check:** Satisfied — HLL states are opaque byte sketches over Endpoint; allowed hash is blake3/sha256.

## Q019 — Distributed query execution fault tolerance straggler speculation

**Classification:** ESTABLISHED  
**Key finding:** BigQuery/Dremel: deterministic and idempotent tasks allow restarts; straggler detection re-executes slow tasks redundantly when execution exceeds a threshold. Cassandra speculative execution (driver-side): pre-emptively re-send idempotent queries to another node after a latency threshold (p99.9); whichever replies first wins; cancelled requests still interact with server.  
**Sources:** https://15721.courses.cs.cmu.edu/spring2024/notes/17-bigquery.pdf ; https://docs.datastax.com/en/developer/java-driver/latest/manual/core/speculative_execution/index.html  
**Constraint check:** Satisfied — idempotent query fragments re-dispatched to alternate Endpoints.

## Q020 — Query decomposition message passing carrier agnostic distributed system

**Classification:** HYPOTHESIS  
**Key finding:** Carrier-agnostic message passing over opaque datagrams (Endpoint + Vec<u8>) is an architectural stance (HX-241 / IDS), not a widely documented distributed-query pattern in the literature. The decomposition→fragment→dispatch→gather pattern is well established, but expressing every inter-node hop strictly through an opaque `ids-carrier::Endpoint` without any transport assumptions is a design constraint rather than an established prior art. No counter-evidence found; feasibility rests on IDs's carrier abstraction being total.  
**Source:** n/a (architectural constraint from prompts.txt/CRATES.md)  
**Constraint check:** Satisfied by design — the constraint is self-imposed; no finding violates it.

## Q021 — Distributed conjunctive query evaluation semi join

**Classification:** ESTABLISHED  
**Key finding:** Semi-join is the fundamental operation for reducing data volume transferred between sites in distributed query processing. Semi-join reduces a relation by keeping only tuples that join with another relation, shipped as a small bitmap/value set before the full relation. Bitmap augmentation augments join attributes with bitmaps. Reduces network transfer dramatically in distributed joins.  
**Sources:** http://carleton.ca/scs/wp-content/uploads/sites/260/TR-78.pdf ; https://arxiv.org/html/1710.07907v1  
**Constraint check:** Satisfied — semi-join reduction ships small opaque sets over Endpoint.

## Q022 — MapReduce distributed query execution map reduce phases

**Classification:** ESTABLISHED  
**Key finding:** MapReduce executes distributed queries as map (local per-partition computation) + shuffle (hash-partition intermediate by key) + reduce (per-key aggregation). Shuffle persistence layer decouples scheduling from execution; intermediate outputs held in memory, spilling to disk only when necessary; worker failure → re-queue unit of work. Deterministic/idempotent tasks ensure restart correctness.  
**Source:** https://15721.courses.cs.cmu.edu/spring2024/notes/17-bigquery.pdf  
**Constraint check:** Satisfied — map/reduce tasks target Endpoint-scoped partitions; shuffle data is opaque bytes.

## Q023 — Distributed top-k query execution sorted merge

**Classification:** ESTABLISHED  
**Key finding:** Distributed top-k uses threshold algorithms (TA, NRA) and variants (TPUT three-phase uniform threshold, TPAT, HT hybrid). KLEE framework trades result quality vs performance/bandwidth. Massively parallel top-k: local bitonic sort per partition, merge to halve candidate sets, rebuild; limits per-partition candidates to k before merge. Sorted access to index lists is ~20× faster than random access for large disk-resident lists.  
**Sources:** https://cs.ucsb.edu/sites/default/files/documents/2005-14.pdf ; http://www.vldb.org/archives/website/2005/program/paper/thu/p637-michel.pdf ; https://spiral.imperial.ac.uk/server/api/core/bitstreams/00d931d0-ec66-4a4b-b5c1-f8aa214e37a4/content  
**Constraint check:** Satisfied — top-k candidates are opaque bounded streams over Endpoint.

## Q024 — Query shipping versus data shipping distributed execution data locality

**Classification:** ESTABLISHED  
**Key finding:** Function shipping (worker executes query, returns result) vs data shipping (coordinator reads raw data, executes locally) is a fundamental trade-off. Function shipping minimizes data movement but requires compute at data; data shipping is simpler but moves more data. "To Ship or Not to (Function) Ship" quantifies costs; hybrid chooses per-worker. Data locality favors shipping the function to the data.  
**Source:** https://arxiv.org/html/1807.11149v1  
**Constraint check:** Satisfied — both are query fragments over opaque Endpoint-scoped data.

## Q025 — Distributed query scheduling locality aware replica selection

**Classification:** ESTABLISHED  
**Key finding:** Replica selection for each shard should pick the replica that answers fastest; queue length is an inaccurate workload indicator under dynamic pruning. Query efficiency prediction (expected workload) improves scheduling accuracy and reduces waiting/completion times. Locality-aware scheduling routes to the replica/region holding the data.  
**Source:** http://www.dcs.gla.ac.uk/~craigm/publications/freire12scheduling.pdf  
**Constraint check:** Satisfied — replica selection maps shard→Endpoint via ownership metadata.

## Q026 — Consistent hashing partition ownership range routing

**Classification:** ESTABLISHED  
**Key finding:** Consistent hashing maps nodes and keys onto a ring; a key's owner is the first node clockwise from the key's position. Adding/removing nodes moves only a small fraction of keys. Vnodes (Cassandra) give each node multiple small ranges for better balance. Rendezvous hashing scores every server per key (good for small server sets, no ring). Fixed partitions map key→partition→server table (good for logs/queues).  
**Sources:** https://cassandra.apache.org/doc/latest/cassandra/architecture/dynamo.html ; https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/architecture/archDataDistributeHashing.html ; https://eli.thegreenplace.net/2025/consistent-hashing  
**Constraint check:** Satisfied — partition ownership maps to opaque Endpoint IDs.

## Q027 — Intermediate result materialization distributed query pipelines

**Classification:** ESTABLISHED  
**Key finding:** Intermediate result materialization vs pipelining is a core trade-off: materialization enables reuse/caching (recycling) but consumes memory; pipelining avoids it. Vectorwise recycles materialized intermediates via a cache with subsumption and proactive rewriting; DISS combines decomposition, pipelining, and intermediate result sharing. Intermittent query processing materializes chosen intermediate states for delta reprocessing within memory budgets.  
**Sources:** https://ir.cwi.nl/pub/21352/21352A.pdf ; https://www.vldb.org/pvldb/vol12/p1427-tang.pdf ; http://store.computer.org/csdl/proceedings-article/icde/1986/07266210/12OmNxd4too  
**Constraint check:** Satisfied — materialized intermediates are opaque byte blobs over Endpoint.

## Q028 — Distributed query failure partial results graceful degradation

**Classification:** ESTABLISHED  
**Key finding:** Vespa graceful query coverage degradation: when a query cannot complete within timeout (overload, complexity), the system can return partial results with degraded coverage (percentage of documents evaluated) rather than failing outright. Coverage vs timeout is tunable; fail-partial rather than fail-total. Dremel honors a min-tablet-scanned percentage parameter to speed up execution under partial failure.  
**Sources:** https://docs.vespa.ai/en/performance/graceful-degradation.html ; https://www.vldb.org/pvldb/vol3/R29.pdf  
**Constraint check:** Satisfied — partial results are opaque aggregates over Endpoint.

## Q029 — Region aware query routing data residency compliance

**Classification:** ESTABLISHED  
**Key finding:** Data residency/domiciling constrains replicas to approved geographies (super regions, restricted replica placement). Region-locked routing sends each request only to endpoints inside the allowed region, covering fallback/load-balancing too (fail closed if no in-region endpoint). Locality-aware optimization incorporates cross-region latency as a first-class cost input to prefer local plans.  
**Sources:** https://www.cockroachlabs.com/blog/multi-region-database-architecture-sql-placement-locality ; https://tetrate.io/learn/ai/data-residency-routing ; https://openrouter.ai/docs/guides/features/in-region-routing  
**Constraint check:** Satisfied — region routing via ids-routing/Endpoint, no network assumptions.

## Q030 — Distributed stream query execution continuous queries

**Classification:** ESTABLISHED  
**Key finding:** Continuous queries run over dynamic (streaming) tables and never terminate, producing dynamic result tables semantically equivalent to batch re-execution on snapshots. Flink dynamic tables / BigQuery continuous queries / Hazelcast stream SQL: stream processing jobs are long-running, backpressured, and restartable. Distributed stream query execution is a recognized pattern distinct from batch.  
**Sources:** https://nightlies.apache.org/flink/flink-docs-master/docs/concepts/sql-table-concepts/dynamic_tables/ ; https://docs.cloud.google.com/bigquery/docs/continuous-queries-introduction ; https://docs.hazelcast.com/hazelcast/latest/sql/querying-streams  
**Constraint check:** Satisfied — stream query fragments target Endpoint-scoped partitions; results stream as opaque bytes.

---

## Evidence synthesis

| Classification | Count |
|----------------|-------|
| ESTABLISHED | 29 |
| HYPOTHESIS | 1 (Q020 — carrier-agnostic Endpoint-only transport is an IDS architectural stance) |
| SPECULATION | 0 |

**Prior art relevant to ids-query:** Citus coordinator/fragment/merge, MongoDB mongos scatter-gather/targeted, Aerospike scatter-gather streaming, Dremel serving tree, Drill foreman/fragments, Presto coordinator/tasks/stages, Calcite optimizer framework, Volcano/Cascades, Spanner split-region plans, CockroachDB per-node planning, two-phase aggregation (map partial + reduce final), HyperLogLog distinct count, consistent hashing/vnodes/token ranges, semi-join data reduction, function vs data shipping, top-k threshold algorithms, speculative execution/straggler speculation, graceful partial-result degradation, continuous queries.

**Constraint gaps:** None identified — every finding is expressible over `ids-carrier::Endpoint` using only allowed crypto. Q020 is a deliberate design constraint, not a gap.
