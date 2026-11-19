# IDS — ARCHITECTURAL DIFFERENCES, COMPARATIVE ANALYSIS, AND INFERRED APPLICATIONS

**Project:** IDS — Independent Distributed System  
**Parent system:** HX-241 / XarnX  
**Document:** `DIFFERENCES.md`  
**Research cut-off:** 18 November 2026  
**Status:** Comparative research and architectural inference; proposed mechanisms remain subject to implementation and verification.  
**Companion documents:** `IDS.md`, `IDS-A.md`, `NOVEL.md`  
**Primary question:** What is materially different about IDS, how does it compare with existing systems, and what new classes of use cases could follow if its proposed architecture works?

---

# CONTENTS

1. Executive conclusion
2. Purpose, scope, and interpretation rules
3. IDS in one architectural model
4. The essential distinction: a system that owns infrastructure versus a system that federates infrastructure
5. What IDS is not
6. Comparative method and evidence labels
7. At-a-glance comparison across the landscape
8. Detailed system-by-system comparisons
9. Architectural dimension comparison
10. The H-unit: what is already known and what IDS would need to add
11. Responsibility, custody, authority, and recovery
12. Ordering, concurrency, and consistency differences
13. Placement, capacity, and data/compute/network decisions
14. Operational workflows compared end to end
15. Use cases inferred from the architecture
16. Use-case portfolio: sector-by-sector analysis
17. Why the combined architecture could be transformative
18. What would make each proposed difference real
19. Technical gaps and unresolved design choices
20. Validation and benchmarking programme
21. Novelty boundary: safe claims versus unsupported claims
22. Strategic positioning and system evolution
23. Glossary
24. Research source register

---

# 1. EXECUTIVE CONCLUSION

IDS is best understood as a proposed **federated operating architecture for distributed data infrastructure**. Its baseline specification describes one client-facing data-operation environment spanning multiple geographically separated regional database clusters. Each regional cluster contains H-units, and each H-unit groups finite-capacity storage nodes. IDS resolves client operations against that hierarchy, manages data placement and access, and coordinates the operational consequences of replication, synchronization, migration, and failure.

The addendum extends this baseline with more explicit operational semantics: H-unit membership and capacity, node and regional compensation, operation references separated from ordering, dataset-scoped ordering, commit conditions, ordered commit prefixes, and concurrency coordination limited to operations that conflict. The novelty document then proposes a larger direction: autonomous infrastructure domains that can participate in a shared data service without surrendering local control, with intent-driven placement, explicit custody and responsibility, and recovery that transfers obligations rather than merely copying bytes.

The key comparative finding is that **almost every individual ingredient has prior art**. Hierarchical placement is well established. Multi-region replication is well established. Federated namespaces are well established. Declarative replica policies are well established. Autonomous repair is well established. Geo-partitioning, workload-aware placement, CRDT metadata synchronization, and local administrative authority also have precedents. Ceph's CRUSH hierarchy is a particularly close comparator for topology-aware placement; HDFS Federation is a close comparator for independent namespaces; Rucio is a close comparator for declarative replica obligations; Spanner, CockroachDB, YugabyteDB, and Cassandra demonstrate advanced distributed consistency and geo-placement; iRODS, MinIO, and CrossFS demonstrate data and metadata federation across sites or domains. [R1–R10]

Therefore, IDS must not claim that it invented distributed storage, federated databases, hierarchical storage, region-aware placement, selective replication, erasure coding, local autonomy, or automatic repair. Those are not credible standalone novelty claims.

The strongest potential difference is at the **system-operating level**:

> IDS aims to make a data service from independently administered infrastructure domains by explicitly coordinating the complete lifecycle of responsibility for data: admission, identity, placement, custody, permitted operations, ordering, durability, execution, and recovery.

That framing is materially different from simply saying “multiple databases appear as one database.” A federation router can hide namespace boundaries. A replicated database can maintain copies across regions. A storage cluster can place fragments across racks. A data-management system can restore a required replica count. IDS's larger ambition is to bind those concerns into a unified responsibility model across nested infrastructure and potentially independent operators.

This is a **candidate architectural contribution**, not a proven unique invention. To establish that contribution, IDS must define a protocol and data model that answer questions ordinary feature lists do not answer:

- Who is authoritative for a data object, partition, or operation at each stage?
- What exactly does a participating domain promise when it accepts a placement or custody obligation?
- Which local policy checks must pass before the domain commits resources?
- What evidence proves that a replica, fragment, or reconstructed object satisfies the declared durability requirement?
- When one node, H-unit, region, or operator exits, how are obligations transferred without creating two conflicting authorities or a period in which no one is responsible?
- Which operations can progress locally, and which require cross-domain agreement?
- How does IDS decide that a service requirement is satisfied when the underlying resource topology is changing?

The potential impact is large if these questions receive rigorous answers. IDS could become a platform for data continuity across multiple owners, not just multiple machines. It could support research consortia, scientific instruments, healthcare networks, industrial data ecosystems, public-sector archives, edge deployments, AI training and inference pipelines, and organizations that want to combine infrastructure without moving every dataset into one provider's account.

The correct conclusion is neither “IDS is already revolutionary” nor “IDS is just another database.” The correct conclusion is:

**IDS has a potentially powerful system-level direction. Its individual mechanisms overlap heavily with existing systems. Its differentiation must be earned by the formal interaction of autonomous domain admission, explicit data custody, hierarchical responsibility transfer, scoped coordination, and continuously verified service obligations.**

---

# 2. PURPOSE, SCOPE, AND INTERPRETATION RULES

## 2.1 What this document compares

This document compares IDS with distributed databases, distributed file systems, object stores, storage federations, scientific data-management systems, data-space architectures, and resource-orchestration systems. These categories are not interchangeable. Each solves a different subset of the overall problem.

A distributed database is principally concerned with storing and querying structured data while preserving defined transaction and consistency semantics. A distributed file system exposes file-oriented operations over distributed storage. An object store manages object identity, metadata, and byte retrieval. A data-management federation coordinates datasets and replicas across heterogeneous storage endpoints. A data-space architecture is concerned with controlled exchange and use of data among organizations. A compute orchestrator schedules workloads across machines or clusters. IDS's proposed scope intersects all of these, but the baseline IDS specification is still primarily a data-operation and storage architecture.

The distinction matters because it would be inaccurate to compare IDS with a database only on query latency, or with a storage fabric only on throughput, and then conclude that one is categorically superior. The correct comparison asks which responsibilities each architecture owns, which it delegates, what its trust boundary is, and what guarantees it provides.

## 2.2 Three scope levels must remain separate

The current project material contains a baseline architecture, subsequent additions, and a larger inferred direction. They must not be collapsed into one description as though every proposed capability already exists.

| Scope level | Meaning | Included elements | Status in this document |
|---|---|---|---|
| **Level A — Baseline IDS** | What the original `IDS.md` specifies | Client abstraction; regional DB clusters; H-units; storage nodes; data placement; retrieval; replication; synchronization; capacity awareness; expansion; regional operation | Existing conceptual specification; unproven implementation |
| **Level B — Architectural addendum** | What `IDS-A.md` adds | H-unit membership; node/H-unit metadata; cross-H-unit compensation; regional compensation; hybrid redundancy; operation references; dataset ordering; commit states; conflict-scoped coordination | Added specification; still conceptual and unproven |
| **Level C — Expanded research architecture** | What `NOVEL.md` proposes as the next frontier | Autonomous operator domains; local admission authority; intent-to-infrastructure compilation; data custody/responsibility graph; proof-backed durability; joint data/compute/network placement; federation of infrastructure services | Inferred and proposed extension; not part of a verified implementation |

Every comparison and use case below uses explicit labels:

- **Specified:** directly present in the current IDS documents.
- **Inferred:** logically follows from specified mechanisms if they are implemented correctly, but is not yet a written or proven guarantee.
- **Proposed:** an additional mechanism that would need a formal specification and implementation.
- **Potential impact:** a plausible consequence if the proposed mechanism is implemented and performs as intended.
- **Not established:** a claim that cannot currently be made from the documents or available evidence.

## 2.3 What “different” means

Difference is not a binary property. Two systems can share a technique but differ in its scope, authority model, interaction with other mechanisms, or failure semantics.

| Difference type | Question | Example in IDS assessment |
|---|---|---|
| Structural | Are the components arranged differently? | Regional clusters containing H-units, which in turn contain storage nodes |
| Administrative | Who owns and controls each component? | Current baseline has regional independence; full multi-owner autonomy is an expanded proposal |
| Operational | Which system decides what happens? | IDS resolves client requests across hierarchy; expanded IDS would negotiate domain-approved operations |
| Semantic | What does a successful operation mean? | Commit is separated from mere receipt, execution, and later synchronization |
| Failure-related | What happens when a component becomes unavailable or exits? | Node-, H-unit-, and region-level compensation; future operator-domain withdrawal recovery |
| Coordination-related | Where is agreement required? | Dataset-scoped ordering and conflict-specific coordination in the addendum |
| Economic | How are capacity and cost considered? | Capacity-aware placement is specified as a requirement; cost/energy-aware optimization is proposed |
| Ecosystem-level | What can participate? | Baseline storage nodes and regional DBs; expanded architecture could include independent organizations and multiple resource types |

## 2.4 Evidence and confidence

The source register identifies public technical documentation and research sources. Claims about other products should be read as descriptions of their documented architecture, not claims about every deployment configuration. Product documentation changes, and features may differ by edition or release.

This document is not a patentability opinion, legal freedom-to-operate analysis, or proof that a specific IDS mechanism has never been published. It is an engineering comparison intended to sharpen the research contribution and prevent obvious overclaims.

---

# 3. IDS IN ONE ARCHITECTURAL MODEL

## 3.1 Baseline topology

The baseline model is hierarchical:

```text
                         CLIENT APPLICATIONS
                                  |
                                  v
                       IDS CLIENT-FACING LAYER
                                  |
                    REQUEST / IDENTITY / POLICY RESOLUTION
                                  |
                +-----------------+-----------------+
                |                 |                 |
                v                 v                 v
           REGIONAL DB 1     REGIONAL DB 2     REGIONAL DB 3
             REGION A          REGION B          REGION C
                |                 |                 |
          +-----+-----+     +-----+-----+     +-----+-----+
          |     |     |     |     |     |     |     |     |
          H1    H2    H3    H1    H2    H3    H1    H2    H3
          |     |     |     |     |     |     |     |     |
        NODES NODES NODES NODES NODES NODES NODES NODES NODES
```

This hierarchy is more specific than a flat list of machines. The regional database cluster establishes a geographic and operational boundary. H-units group nodes inside a regional cluster. Nodes have finite storage, compute, memory, and network capacity. Data is distributed across these levels using placement and redundancy rules.

The H-unit is an intermediate subcluster, not simply a logical label and not necessarily a full database. The addendum assigns it membership, capacity, and compensation significance. A node failure inside H1 may require another H-unit such as H2 to serve, reconstruct, or otherwise compensate for the affected responsibility, depending on which replicas or encoded fragments exist and what the recovery policy permits.

## 3.2 Two axes of expansion

IDS distinguishes two forms of growth:

1. **Intra-region expansion:** add storage nodes or H-units to an existing regional database cluster.
2. **Inter-region expansion:** add a new regional database cluster and integrate it into the wider system.

These are not equivalent operations. Adding a node increases capacity inside an established regional boundary. Adding a region changes the geographic topology, possible failure domains, locality, jurisdiction, and cross-region coordination paths.

| Expansion action | Immediate effect | Metadata that must change | Main engineering issue |
|---|---|---|---|
| Add node to H-unit | Increases one H-unit's resources | Node identity, capacity, health, location, membership, placement eligibility | Rebalancing without overloading existing nodes |
| Add new H-unit | Increases regional subcluster capacity and failure separation | H-unit identity, node membership, H-level placement and recovery map | Avoiding concentration of data or correlated failure |
| Add regional DB cluster | Expands geographic footprint | Region identity, policy, ownership, routes, replica targets, coordination domains | Admission, locality, consistency, sovereignty, and region-level recovery |
| Remove node | Reduces local capacity | Placement map, replicas/fragments, responsibility records | Moving obligations before decommission |
| Remove H-unit | Removes a subcluster boundary | Membership, responsibility graph, replacement mapping | Ensuring loss of group does not silently remove independent copies |
| Withdraw region | Removes an entire failure and administrative domain | All relevant mappings, ownership, data placements, commit/replica states | Service continuity and safe transfer of every obligation |

## 3.3 The system's operational chain

A client should not need to address N119 in H1 directly. It asks IDS for a logical dataset or object. IDS resolves the relevant dataset, operation semantics, region, H-unit, node, and any required replicas. It checks the operation's authorization and consistency requirements, routes the operation, and returns a result whose meaning is defined by the commit and visibility model.

Conceptually:

```text
CLIENT REQUEST
   |
   v
IDENTIFY DATASET / OBJECT
   |
   v
RESOLVE OPERATION SEMANTICS
   |
   +--> authorization and policy
   +--> consistency / ordering domain
   +--> locality and residency constraints
   +--> durability target
   |
   v
RESOLVE RESPONSIBLE REGION
   |
   v
RESOLVE H-UNIT(S)
   |
   v
RESOLVE NODE(S) / SHARDS / REPLICAS
   |
   v
EXECUTE / REPLICATE / RECONSTRUCT AS REQUIRED
   |
   v
VERIFY COMMIT CONDITIONS
   |
   v
RETURN RESULT WITH OPERATION REFERENCE
```

The sequence is explanatory. The exact algorithm, message ordering, retry protocol, metadata consistency, and trust boundaries still need formal specification.

## 3.4 What the hierarchy does not automatically guarantee

A hierarchy by itself does not guarantee fault tolerance. Three nested boxes on a diagram do not create independent failure domains. If every H-unit shares the same storage controller, power supply, network switch, credentials, or administrator, the apparent separation may be correlated and illusory.

The hierarchy only contributes to resilience when placement rules map physical and administrative failure dependencies accurately. A node in H2 is not a safe replacement for a node in H1 if both are dependent on the same underlying fault source. This is why topology information, capacity records, ownership, and domain-specific policies matter.

The hierarchy also does not imply that every read or write must traverse every level. A well-designed system should use hierarchy to resolve responsibility and eligibility, then communicate directly with the relevant data path wherever possible. Metadata decisions and bulk data transfer should not be needlessly serialized through a central bottleneck.

---

# 4. THE ESSENTIAL DISTINCTION: OWNING INFRASTRUCTURE VERSUS FEDERATING IT

## 4.1 Two different operating assumptions

Many large distributed systems are deployed by one organization within one administrative control plane. That organization owns or controls the clusters, configures topology, sets replication policies, and has authority to repair or replace machines.

A federation is more difficult. The participants may be separate companies, universities, hospitals, cloud providers, research facilities, public agencies, or edge operators. They may have different owners, security boundaries, software versions, service levels, legal obligations, budgets, and willingness to accept external workloads.

The distinction is not whether machines are geographically separated. A single company can own servers in five continents. The distinction is whether the participants have independent authority and whether one participant can refuse, constrain, or terminate participation.

| Question | Single-operator cluster | Federated multi-owner system |
|---|---|---|
| Who controls node admission? | One operator or its delegated team | Each domain may retain its own admission authority |
| Who defines security policy? | Common organizational policy | Local policy plus agreed federation rules |
| Who can shut down a node? | Central operations authority | Local operator can withdraw its resources |
| Who pays for capacity? | Common budget or chargeback model | Multiple budgets and economic contracts |
| Who owns the data? | Usually one organizational owner or tenant | Data owners may differ from infrastructure owners |
| Who may inspect data? | Common organizational controls | Separate data and infrastructure permissions are required |
| Who repairs a failed node? | Local cluster operator | Local operator may repair; federation may need to transfer obligations elsewhere |
| Can a central controller command every participant? | Often yes within the organization | Not necessarily; local policy can reject actions |
| What happens during a dispute? | Internal escalation | Contract, policy, and trust mechanisms may be required |

## 4.2 Why “one logical system” is not enough

A global namespace makes resources appear connected. It does not establish that the owners have agreed to store a particular object, process a query, perform an expensive repair, or preserve data for a declared period.

A global routing layer can forward a request. It cannot manufacture local authority. A replica manager can schedule a copy. It cannot guarantee the destination operator has accepted the obligation. A consensus protocol can order operations among its participants. It cannot establish the legal or administrative permission to use every participant's resources.

The expanded IDS proposal addresses this gap by separating:

- **Federation identity:** which domain is participating?
- **Capability declaration:** which resources and service properties does it offer?
- **Admission authority:** which operations will it accept?
- **Data authority:** who may create, mutate, delete, or disclose a dataset?
- **Custody responsibility:** which domain promises to preserve a representation?
- **Execution authority:** where may code or queries run?
- **Evidence:** how is fulfilment of those promises demonstrated?

These are proposed additions. They are not fully specified by the baseline IDS documents.

## 4.3 The possible IDS operating model

```text
                  DATA SERVICE REQUEST
                           |
                           v
                 IDS INTENT RESOLVER
                           |
             +-------------+-------------+
             |             |             |
             v             v             v
        DOMAIN A AGENT DOMAIN B AGENT DOMAIN C AGENT
             |             |             |
        LOCAL POLICY   LOCAL POLICY   LOCAL POLICY
        & ADMISSION    & ADMISSION    & ADMISSION
             |             |             |
             v             v             v
        ACCEPT /       ACCEPT /       ACCEPT /
        DECLINE        DECLINE        DECLINE
             |             |             |
             +-------------+-------------+
                           |
                           v
              FEDERATED SERVICE PLAN
                           |
               CUSTODY / PLACEMENT / EXECUTION
                           |
                           v
              CONTINUOUS OBLIGATION MONITORING
                           |
                           v
              REPAIR / TRANSFER / RECONSTITUTION
```

The key difference is that an IDS coordinator would not be an omnipotent controller that directly commands every machine. It would negotiate or compile a plan from capabilities and constraints, while local domain agents authorize and execute the part within their own jurisdiction.

This is the architecture's most consequential proposed direction. It still needs a wire protocol, identity model, authorization semantics, negotiation algorithm, failure handling, and proof model.

---

# 5. WHAT IDS IS NOT

The following distinctions prevent scope confusion.

| Similar description | Why it is insufficient | More accurate IDS description |
|---|---|---|
| “A distributed database” | Too narrow for the intended federation and infrastructure responsibility model | Baseline: distributed data-operation architecture spanning regional DB clusters; expanded: federated data-infrastructure operating layer |
| “A new GFS” | GFS is a specific distributed file-system architecture with its own metadata and chunk model | IDS includes database-style regional clusters, H-units, ordering domains, and proposed multi-owner federation |
| “A Ceph replacement” | Ceph already provides mature object/block/file storage, CRUSH placement, replicas, and erasure coding | IDS would need to prove a different administrative and responsibility model, not merely different placement syntax |
| “A mesh network” | Network topology does not define data ownership, commit, durability, or recovery semantics | IDS may use XarnX transport, but its core responsibilities are data and infrastructure operations |
| “A P2P file-sharing system” | Peer discovery and transfer do not automatically provide service obligations, transaction semantics, or trusted custody | IDS may include peer domains but must define responsibility and authority at the data-service level |
| “A global namespace” | Name resolution does not guarantee availability, authority, consistency, or durability | IDS combines identity resolution with placement, policy, commit, and recovery responsibilities |
| “A storage marketplace” | A marketplace allocates resources economically but need not provide unified operations or data semantics | IDS could support resource offers, but the operating model is broader than buying storage |
| “A Kubernetes federation layer” | Compute scheduling across clusters is not the same as data custody and consistency | Expanded IDS could cooperate with orchestrators to place compute near data |
| “An automatic backup tool” | Backup is only one durability mechanism and usually does not coordinate live operations across domains | IDS treats durability and recovery as continuing service obligations |

IDS is also not currently a demonstrated implementation. `IDS.md`, `IDS-A.md`, and `NOVEL.md` are conceptual and research documents. This comparison evaluates the proposed architecture, not a benchmarked production system.

---

# 6. COMPARATIVE METHOD AND EVIDENCE LABELS

## 6.1 Comparison axes

Systems are compared on the following axes:

1. **Data model:** objects, files, blocks, tables, datasets, or mixed resources.
2. **Logical identity:** whether data identity is independent of its physical location.
3. **Topology:** whether hierarchy is explicit and what its levels represent.
4. **Placement:** whether data is placed by hash, policy, geography, workload, or negotiated capabilities.
5. **Administrative model:** one operator, tenant federation, or independent operators.
6. **Authority:** who can admit resources and authorize operations.
7. **Consistency:** what reads and writes are allowed to observe.
8. **Ordering:** whether ordering is global, per partition, per transaction, per dataset, or eventual.
9. **Durability:** replica count, erasure coding, policy targets, or other guarantees.
10. **Recovery:** whether repair is local, cluster-wide, cross-region, or cross-operator.
11. **Execution:** whether compute is scheduled near the data.
12. **Verification:** what proves that a declared service condition is actually fulfilled.

## 6.2 Evidence grades

| Grade | Interpretation | How it is used |
|---|---|---|
| A — Directly documented | Official documentation or primary paper expressly describes the feature | May be stated as a documented capability with a source |
| B — Strongly implied | Architectural description clearly entails the capability, but not every operational detail is stated | Describe as an architectural implication, not a product guarantee |
| C — IDS inference | A plausible consequence of proposed IDS mechanisms | Label as inferred or potential |
| D — Open question | The current documents do not define the needed mechanism | Present as research work, not a capability |

## 6.3 Important limits on product comparisons

Comparative tables can hide implementation-specific details. For example, “multi-region” can mean synchronous replicas in a consensus quorum, asynchronous copies, active-active object replication, or a region-local partition layout. These arrangements have different consequences for write latency, stale reads, outage behavior, and data loss exposure.

Likewise, “federation” may mean several independent namespaces accessible through a router, independent sites synchronizing objects, or organizations sharing data under usage policies. These are related but distinct.

The comparison therefore describes documented design patterns and then states the question IDS would need to answer to be different.

---

# 7. AT-A-GLANCE COMPARISON ACROSS THE LANDSCAPE

## 7.1 High-level architecture matrix

| System / family | Main abstraction | Distribution model | Main control model | Strongest overlap with IDS | What IDS would need to do differently |
|---|---|---|---|---|---|
| Google File System (GFS) | Files split into chunks | Chunkservers and replicas | Master-managed metadata | Logical names, distributed storage, replication, recovery | Replace neither GFS nor its file semantics; differentiate via regional federation and scoped responsibility |
| HDFS Federation | Multiple namespaces and block pools | Independent NameNodes over DataNodes | Federation of namespace services | Independent data namespaces and common storage participation | Add explicit data custody, nested H-unit compensation, and domain admission |
| Ceph / CRUSH | Objects mapped to placement groups and OSDs | Hierarchical failure-domain placement | Cluster map and MON/OSD system | H-like topology, replica/fragment distribution, capacity-aware placement | Make groups autonomous responsibility domains, not merely topology buckets |
| Apache Ozone | Keys, blocks, containers | OM / SCM / DataNodes | Cluster managers with replication pipelines | Namespace/storage separation, node grouping, capacity and repair | Extend beyond one cluster into independent regional and operator federation |
| Apache Cassandra | Partitioned keyspace | Replicas across nodes, racks, data centres | Peer cluster with tunable consistency | Distributed partitioning, replication, local/DC-aware behavior | Define more explicit multi-level responsibility and custody transfer |
| Google Spanner | Globally distributed relational database | Replicas across zones and regions | Synchronous quorum and transaction protocol | Geographic durability, strong consistency, region failover | Differentiate via autonomous multi-owner federation, not stronger consistency claims |
| CockroachDB | SQL ranges and transactions | Range replicas across localities | Raft-based replicated ranges | Multi-region placement and survival goals | Add independently controlled domain admission and custody guarantees |
| YugabyteDB | Tablets and geo-partitioned rows | Region/rack placements, Raft groups | Distributed SQL / transaction layer | Row-level geo-placement and replication topology | Add cross-operator resource federation and explicit service obligations |
| FoundationDB | Ordered transactional key-value store | Sharded key ranges and storage processes | Coordinated transactional architecture | Separation of layers and data engine roles | Not a direct competitor; IDS could use a database engine as a component |
| MinIO Site Replication | Buckets and objects | Peer site replicas | Site-level object replication | Independent sites and data synchronization | Add heterogeneous infrastructure placement, ordering semantics, and multi-level recovery |
| iRODS | Logical data objects and collections | Federated zones and storage resources | Metadata catalog and policy rule engine | Policy workflows and federated resource access | Add capacity, custody, consistency, and proof-backed obligation model |
| Rucio | Files, datasets, containers, RSEs | Replication rules across storage endpoints | Rule engine and transfer services | Declarative replica targets and automatic restoration | Extend into live data-operation semantics and domain-level admission |
| iStore | Global namespace over geo-distributed edge storage | Resource-aware placement and migration | Federated file-system model | Geographical federation and data-aware placement | Add explicit operator autonomy and transfer of durable responsibilities |
| CrossFS (2026) | Cross-domain file-system metadata | CRDT-based metadata synchronization | Metadata service across domains | Directly demonstrates cross-domain FS and metadata federation | Differentiate beyond synchronization: authority, custody, durability obligations, hierarchy |
| Gaia-X / data spaces | Data services and usage policies | Federated participants | Trust framework and policy enforcement | Trust, policy, and sovereign data sharing | Bind trust-policy layer to storage placement, operation lifecycle, recovery and proof |
| IPFS / Filecoin | Content-addressed data and storage deals | Peer retrieval / storage providers | Distributed discovery and economic deal mechanisms | Location-independent identifiers and multiple storage providers | Add transactional data services, scoped ordering, structured custody and verified service-level recovery |

## 7.2 Feature coverage matrix

Legend: **●** documented core emphasis; **◐** supported in some form or configuration; **○** not a primary function; **?** requires specific deployment or additional mechanisms. “IDS” column reflects the current specification plus clearly labeled expanded proposals, not an implemented system.

| Capability | IDS target | Ceph | HDFS Fed. | Spanner | CockroachDB | YugabyteDB | Rucio | iRODS | MinIO SR | CrossFS |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Logical client-facing data access | ● | ● | ● | ● | ● | ● | ● | ● | ● | ● |
| Multiple geographic domains | ● | ◐ | ◐ | ● | ● | ● | ● | ● | ● | ● |
| Explicit hierarchy of resource groups | ● | ● | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ |
| Independent namespace/control domains | ● | ? | ● | ○ | ◐ | ◐ | ● | ● | ● | ◐ |
| H-unit as intermediate subcluster | ● | ○* | ○* | ○ | ○ | ○ | ○ | ○ | ○ | ○ |
| Per-node capacity-aware placement | ● | ● | ◐ | ● | ● | ● | ◐ | ◐ | ◐ | ? |
| Replication / redundancy | ● | ● | ● | ● | ● | ● | ● | ● | ● | ● |
| Erasure coding | Proposed hybrid | ● | ● | ? | ? | ? | ◐ | ? | ●** | ◐ |
| Geo-partitioning / residency | ● | ◐ | ◐ | ● | ● | ● | ● | ● | ◐ | ◐ |
| Declarative replica obligations | Proposed | ◐ | ◐ | ◐ | ◐ | ◐ | ● | ◐ | ◐ | ◐ |
| Autonomous domain admission | Proposed | ○ | ○ | ○ | ○ | ○ | ◐ | ◐ | ◐ | ? |
| Explicit custody/responsibility graph | Proposed | ◐ | ◐ | ○ | ○ | ○ | ◐ | ◐ | ◐ | ? |
| Dataset-scoped operation ordering | Specified | ? | ? | ◐ | ◐ | ◐ | ○ | ○ | ◐ | ◐ |
| Conflict-specific coordination | Specified direction | ◐ | ◐ | ◐ | ◐ | ◐ | ○ | ◐ | ◐ | ◐ |
| Regional / multi-level recovery | ● | ● | ● | ● | ● | ● | ● | ◐ | ● | ◐ |
| Cross-operator withdrawal recovery | Proposed | ○ | ○ | ○ | ○ | ○ | ◐ | ◐ | ◐ | ? |
| Joint data + compute + network placement | Proposed | ○ | ○ | ◐ | ◐ | ◐ | ◐ | ◐ | ○ | ○ |
| Verified fulfillment of service obligations | Proposed | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ | ◐ |

\* Ceph and HDFS include intermediate groupings, but those are not automatically equivalent to the IDS H-unit's intended responsibility boundary.  
\** Product and release configuration matters; the table is not a guarantee that all MinIO distributions expose the same coding choices.

The table deliberately does not mark IDS as “better.” It identifies the desired coverage and the closest overlap. Most competitors have mature implementations of narrower capabilities, while IDS has an unproven proposal to connect a wider set of operational responsibilities.

## 7.3 Main difference clusters

| Difference cluster | Established prior art | Possible IDS distinction | Evidence needed |
|---|---|---|---|
| Nested hierarchy | Ceph buckets, HDFS blocks and DataNodes, Ozone containers and pipelines | H-unit acts as a responsibility and compensation domain with local capacity and admission semantics | H-unit state machine; membership protocol; failure-domain model |
| Region-level federation | HDFS Federation, iRODS zones, MinIO peer sites, iStore | Cross-region service built from independently governed domains | Domain protocol and local authority enforcement |
| Placement policy | CRUSH, Rucio rules, Yugabyte geo-partitioning | Translate service-level intent into multi-domain placement commitments | Formal intent language, solver/optimizer, accepted-plan record |
| Data responsibility | Replica metadata, ownership/access metadata, Rucio rules | Explicit graph of authority, custody, replicas/fragments, and repair duty | Graph model, safety invariants, transfer protocol |
| Coordination scope | Per-range or per-shard consensus, dataset queues, CRDT sync | Causality-scoped coordination across federated domains | Consistency model and cross-scope correctness proof |
| Recovery | Replica repair, erasure reconstruction, failover | Transfer responsibility across H, region, and operator levels | Recovery protocol, no-gap/no-double-authority proofs |
| Joint placement | Data-local compute and edge schedulers | Unified placement decision based on data, compute, network, policy, and custody | End-to-end benchmark against separated schedulers |

---

# 8. DETAILED SYSTEM-BY-SYSTEM COMPARISONS

This section describes the most important comparisons in greater detail. The aim is not to rank systems in the abstract. It is to identify where IDS overlaps with existing architecture, where the design boundary differs, and which claims would require a new mechanism.

## 8.1 Google File System (GFS)

### What GFS is designed to do

GFS is a distributed file system designed for large files, high throughput, and workloads in which components fail regularly. It divides files into chunks, distributes those chunks across chunkservers, and uses a master to maintain namespace and chunk metadata. Clients contact the master for metadata and then transfer bulk data directly with chunkservers. The original GFS paper describes the master operation log as establishing a global order of metadata operations. [R11]

### What overlaps with IDS

GFS and IDS share the fundamental abstraction that a logical data request must be translated into physical storage operations. Both need to know where pieces of data reside, which copies are available, and what recovery actions are needed when a storage node fails. Both separate a logical identity from the physical location of underlying data.

This overlap is fundamental to distributed storage. It does not indicate that IDS is derived from GFS or that GFS has the same regional/H-unit architecture.

### Where the design emphasis differs

GFS is a file-system design organized around a master metadata service and chunkservers. IDS is organized around regional database clusters, H-units, nodes, and client-level data operations. More importantly, the IDS addendum aims to avoid one permanent system-wide ordering authority by allowing independent ordering domains. The expanded IDS vision adds the more significant potential distinction: independent domains may be controlled by separate operators, and a federation-level plan must respect their admission policies.

GFS's master model is not an argument that centralized metadata is always wrong. A centralized metadata authority can be a useful trade-off when one operator can control the entire deployment and availability requirements permit it. IDS's question is different: how can a common data service operate when no one participant has the authority to command every region or resource owner?

### What IDS must not claim

IDS cannot claim novelty for distributed chunks, logical names, replica metadata, direct data paths, node-failure repair, or a separation between metadata and payload. Those are established design concepts.

### What could be distinctive

The potentially distinctive contribution is a formal way to bind each data representation and pending operation to a set of current responsibility holders, while allowing independent administrative domains to accept, reject, or later transfer those responsibilities. A concrete demonstration would have to show that data remains addressable and recoverable when an entire operator withdraws, not only when a process crashes.

| Dimension | GFS | IDS direction | Consequence |
|---|---|---|---|
| Core abstraction | Files and chunks | Data services, datasets/objects, regions, H-units and nodes | Different upper-level operating model |
| Metadata authority | Master-managed | Scoped/federated authority is the target | Avoid assuming one system-wide control authority |
| Ordering | Global metadata operation order in original design | Dataset-scoped ordering; future causality-scoped ordering proposed | Independent datasets need not share one sequence |
| Administrative boundary | GFS cluster under an operator | Potentially independent operators | Local admission and trust need explicit protocols |
| Failure model | Chunkserver/master failures | Node, H-unit, regional and proposed operator exit | Broader responsibility-transfer problem |

## 8.2 HDFS Federation and Router-Based Federation

### What HDFS Federation contributes

HDFS Federation allows multiple independent NameNodes and namespaces to exist in one HDFS cluster. The namespaces do not need to coordinate with each other, while DataNodes can provide block storage for multiple block pools. Router-based Federation adds a routing layer to make subclusters reachable through a more unified namespace view. [R1]

This is a close comparison because it demonstrates that federating independent data-control namespaces over shared physical storage is not a new direction by itself.

### Overlap with IDS

Both designs recognize that one namespace or metadata manager can become a scaling and administrative bottleneck. Both organize a wider data environment from multiple namespace or cluster components. Both aim to let a user access data without manually choosing a low-level storage location.

### Difference that may matter

HDFS Federation's federation model is a federation of namespace services. IDS's baseline is a federation of regional database clusters, with an intermediate H-unit layer inside each region. The H-unit adds a second organizational level between regional clusters and individual storage nodes. The IDS addendum also focuses on how a failed node's responsibility is compensated by another H-unit, and how region-level compensation differs from H-level compensation.

However, a nested topology is not sufficient to distinguish IDS from every other system. Ceph can represent multiple hierarchy levels; other storage systems use racks, zones, pools, containers, or failure domains. IDS must demonstrate that H-units are operational actors with their own membership, resource budget, local recovery coordination, and responsibility acceptance—not simply another name for a failure-domain bucket.

### What IDS could add

The strongest extension would be to make each regional cluster a federation participant that advertises a set of capabilities and constraints. The regional participant would not simply expose a namespace. It would declare which data it can own or host, which durability profiles it can meet, what geographic restrictions apply, and how much capacity it is willing to commit. That would change the federation from namespace routing into negotiated service construction.

| Dimension | HDFS Federation | IDS target |
|---|---|---|
| Independent components | NameNodes/namespaces | Regional DB domains and potentially operator domains |
| Shared physical storage | DataNodes can serve block pools | Nodes grouped inside H-units, regionally scoped |
| Unified access | Router-based namespace federation | Unified client-facing data-operation layer |
| Failure hierarchy | Node/block and NameNode concerns | Node → H-unit → region → proposed operator domain |
| Resource acceptance | Primarily cluster administration | Proposed local admission authority per domain |
| Main novelty opportunity | Not HDFS-style federation itself | Explicit service obligations and responsibility transfer across independently governed domains |

## 8.3 Ceph and CRUSH

### What Ceph does

Ceph's CRUSH placement model maps data to storage devices according to a cluster map. The map contains a hierarchy of buckets such as hosts, racks, rows, data centres, and regions, and rules specify where replicas or erasure-coded shards can be placed. Clients can compute object locations and communicate directly with OSDs rather than going through a central broker. [R2]

This is an exceptionally important comparator because an IDS H-unit is nested between a region and nodes, and Ceph already supports arbitrary internal hierarchy levels. The existence of intermediate groups therefore cannot be the novelty claim.

### What is already covered by Ceph

Ceph covers topology-aware placement, capacity weights, placement groups, replication, erasure coding, device classes, failure-domain separation, and dynamic cluster management. Its documentation describes peering, rebalancing, scrubbing, and recovery from faults without requiring a central controller to orchestrate every data operation. [R2]

### The H-unit distinction IDS needs

For IDS to be meaningfully different, H-units must carry semantics beyond topology. An H-unit would need to know:

- which nodes are current members and whether their membership is valid;
- what amount of capacity is genuinely available for new obligations;
- which data responsibilities it currently holds;
- what failure or isolation conditions it is experiencing;
- which other H-units are eligible to compensate for which data;
- what local policy permits it to accept;
- whether it can serve reads, reconstruct missing data, accept new writes, or only preserve existing replicas;
- what evidence is required before it can declare a recovery complete.

A Ceph bucket is a placement object in a topology map. An IDS H-unit is intended to become an operational subcluster in a responsibility and recovery model. The distinction is a hypothesis until implemented. A sufficiently extended Ceph deployment or another system may implement some analogous capabilities; novelty cannot be inferred from naming alone.

### Comparison

| Question | Ceph / CRUSH | IDS intended answer |
|---|---|---|
| What does a hierarchy level represent? | Physical or logical grouping used by placement rules | H-unit: membership, capacity, placement, and compensation domain |
| Who determines placement? | CRUSH rules and cluster map | IDS placement policy; expanded compiler may resolve intents |
| Are replicas placed across failure domains? | Yes, according to rules | Yes, as a target; exact policy model requires specification |
| Does a group have local admission authority? | Not the main abstraction of a CRUSH bucket | Proposed property of autonomous H-units/domains |
| Can a group compensate for another group? | Ceph recovery remaps replicas and shards | IDS proposes explicit inter-H responsibility compensation |
| Can independently owned operators participate? | Not the core CRUSH model | A major expanded direction |

### Critical test

Build a small test where H1 loses N119 permanently. H2 must determine whether it can accept the affected responsibility based on free capacity, independent failure-domain evidence, policy, and existing redundancy. IDS must distinguish (a) a source still being readable, (b) a repair operation being scheduled, (c) replacement data being transferred, and (d) the target durability obligation being restored and verified. If the implementation merely copies a replica to a spare node, it demonstrates ordinary repair, not a unique H-unit model.

## 8.4 Apache Ozone

Apache Ozone is a distributed object store with a separation between namespace management and block-space management. Ozone Manager manages namespace entities such as volumes, buckets, and keys. Storage Container Manager manages DataNodes, blocks, and containers, including replica tracking and recovery. DataNodes store the blocks, and clients can read and write those blocks directly. Ozone also supports replication pipelines and erasure coding. [R3]

### Why it is close

Ozone shows that distinct levels of metadata and storage management, grouped DataNodes, finite-capacity placement, replication units, and automated repair already exist in modern storage architecture. The fact that IDS has an IDS layer, regional DBs, H-units, and nodes is not enough to distinguish it from an architecture with managers, containers, pipelines, and DataNodes.

### Possible IDS difference

Ozone is primarily a storage cluster architecture. IDS could be different if it coordinates independently operated regional systems, each with a local control plane and admission boundary, into one service that tracks data authority, consistency, and custody across all participating domains. It could also make the difference between H-level repair and region-level repair explicit in a common state machine.

The right comparison is not “Ozone has containers, IDS has H-units.” The right comparison is whether the H-unit model yields stronger, measurable semantics for capacity allocation, admission, responsibility, and multi-level recovery than Ozone's existing management structures.

## 8.5 Apache Cassandra

Cassandra is a distributed wide-column database designed around partitioned data, replication, availability, and tunable consistency. It uses a peer-oriented architecture and can place replicas across racks and data centres through replication strategies. Applications choose consistency levels that determine how many replicas must acknowledge reads or writes. [R15]

### Overlap

IDS and Cassandra overlap in distributed partitioning, replication, locality, node failure handling, and the possibility of operating across data centres. Cassandra's consistency levels demonstrate that a system can expose tunable trade-offs instead of using one global consistency setting for every operation.

### Difference

Cassandra is a database with its own partition/key model, replication strategy, and consistency semantics. IDS is not intended merely to be another wide-column database. It proposes a hierarchical regional service that resolves data operations across independently grouped storage nodes and adds dataset-level ordering and commit semantics.

The addendum's dataset-scoped ordering should be compared carefully against Cassandra's per-partition ordering and consistency guarantees. If IDS only orders a dataset's writes, it may be broader than an individual partition and may introduce unnecessary serialization. If it instead evolves into key-range or partition-scoped lanes with explicit cross-lane dependencies, it can reduce coordination where data is independent. That is a research question, not an automatically superior design.

### What IDS needs to prove

- A dataset-level operation lane does not become a hotspot when dataset write rates increase.
- Independent datasets do not wait on one another unnecessarily.
- Cross-dataset transactions have a precise visibility and atomicity model.
- Concurrent writes are resolved deterministically or according to a declared conflict policy.
- The client can tell whether a result is committed, locally applied, replicated, or merely accepted.

## 8.6 Google Spanner

Spanner is a globally distributed relational database that supports regional, dual-region, and multi-region configurations. Its documented multi-region configurations use replicas across multiple regions, voting quorums, leader placement, and externally consistent transactions. A write must satisfy a quorum; region distribution provides higher availability and geographically distributed replicas, with associated latency trade-offs. [R4]

### Overlap

Spanner already demonstrates that global data services can preserve strong transactional semantics across regions. It provides data locality and regional failover while maintaining externally consistent transaction behavior. IDS cannot claim that global distribution, strong consistency, or multi-region availability is novel.

### Difference

Spanner is a managed database service and cluster architecture controlled within Google's service and configuration model. IDS's expanded direction is to coordinate separate infrastructure domains whose operators keep local authority. Spanner's consistency model is designed to make the database appear like one coherent transactional system; IDS proposes scope-sensitive ordering, where unrelated domains or datasets need not participate in the same ordering process.

This is not automatically a stronger property. Spanner intentionally pays coordination costs to provide a powerful transaction contract. IDS would be choosing a different trade-off. The comparison should ask which application classes benefit from independently scoped progress and which require serializable global transactions.

### Consequential design question

Can IDS preserve a clear application contract when one dataset is locally available, another is only asynchronously replicated, and a cross-dataset transaction requires coordination across regions? The system must not blur these states behind a generic “success” response.

| Dimension | Spanner | IDS research direction |
|---|---|---|
| Product class | Distributed relational database | Federated data-operation architecture |
| Global consistency | External consistency/serializable transactions | Scoped ordering and explicit consistency policies |
| Replication | Quorum-based, configured topology | Hybrid redundancy and domain-aware placement proposed |
| Leadership | Leader replicas/regions defined by configuration | No permanent system-wide ordering master is the design goal |
| Ownership | Cloud service / cluster configuration | Local autonomous domain operators are a proposed target |
| Recovery | Managed replica failover and quorum behavior | Hierarchical obligation transfer and proof-backed repair proposed |

## 8.7 CockroachDB

CockroachDB is a distributed SQL database with range-based distribution and Raft replication. Multi-region features allow locality configuration and survival goals such as preserving availability through a region failure. The system's guarantees and performance depend on the configured topology and transaction placement. [R16]

### Overlap

CockroachDB and IDS both need to place replicated pieces of a dataset across geographic regions, manage node and region failures, and choose where transactions run. Multi-region tables and survival goals show that geographical locality and resilience can be exposed as data-model or schema-level choices rather than only as infrastructure configuration.

### Difference

CockroachDB's central abstraction remains a SQL database and its range/transaction system. IDS's expanded proposal is not just a SQL deployment across regions. It would coordinate service requirements across multiple database clusters and independent owners, with explicit custody and admission relationships.

### What IDS must avoid

“Independent ordering domains” cannot simply be marketed as an original idea because distributed databases already shard data and coordinate at range or partition scope. The defensible question is whether IDS's scope model spans heterogeneous data services and independent authority boundaries, and whether its operation protocol preserves a useful consistency contract across those boundaries.

## 8.8 YugabyteDB

YugabyteDB partitions data into tablets and supports geographically distributed replication and row-level geo-partitioning. Its documentation describes pinning table partitions to regions through tablespaces, which can improve locality and support data-residency requirements. [R5]

### Overlap

This is a direct comparator for IDS's geo-placement direction. YugabyteDB can place subsets of relational data in different regions rather than forcing every dataset to have one identical geographic topology. It also supports multi-region replication and different kinds of placement constraints.

### Difference

IDS's possible differentiation is not “we can pin data to a region.” It would be the federation of separately administered regional data environments, where the system must negotiate the right to store, serve, or process data in each domain. YugabyteDB's capabilities are configured within a database cluster or managed deployment model; IDS aims at a broader multi-domain fabric.

### Research question

Can IDS compile a declarative service requirement—such as “keep the authoritative copy in jurisdiction A, allow two verified replicas in distinct failure domains, and place read-serving copies close to users in B and C”—into a legal and feasible plan across separately operated domains? The system would need to consider not only geographic labels but jurisdiction, capacity, availability, cost, trust, network path, and the domain's willingness to accept the obligation.

## 8.9 FoundationDB

FoundationDB provides an ordered transactional key-value store and supports a layered architecture in which higher-level data models and services can be built on top of the core transaction engine. Its technical documentation describes distributed roles such as coordinators, storage servers, transaction servers, and master processes. [R6]

### Overlap

FoundationDB is relevant because it illustrates separation of concerns and a minimal core with layers above it. IDS could use a transactional engine as one component rather than reimplementing every database primitive. The IDS client and policy layer could in principle target multiple storage backends.

### Difference

FoundationDB is not a federation of autonomous regional operators. It is a database engine with a deliberately minimal transactional key-value interface and layered extension model. IDS is concerned with the wider operational federation and placement of data services across separate domains.

### Strategic implication

IDS should not assume it must invent a new storage engine, transaction engine, or consensus protocol. If an established engine can serve as a regional persistence layer, IDS can focus its research on federation, obligation management, and cross-domain control. This may reduce implementation effort while making the actual systems contribution more visible.

## 8.10 MinIO multi-site replication

MinIO Site Replication configures multiple MinIO deployments as peer sites and synchronizes objects and associated configuration across sites. The current documentation describes replication of object changes, bucket configuration, policies, IAM-related state, encryption settings, and other metadata across peer sites. [R7]

### Overlap

MinIO demonstrates that independent object-store deployments can be connected through site replication. This directly overlaps with the notion of multiple regional clusters presenting a coordinated service.

### Difference

Site replication is primarily an object synchronization model. IDS's proposed model additionally requires explicit operation ordering, data ownership, placement decisions, H-unit resource grouping, cross-domain admission, and transfer of responsibility when one site is no longer able or willing to participate.

### Important qualification

IDS must be precise about whether a region is a peer that stores synchronized copies, an authority for a dataset, a serving location, or a temporary reconstruction participant. These roles are not interchangeable. Two sites containing copies do not automatically share the same authority to accept concurrent mutations or delete data.

## 8.11 iRODS

iRODS provides a data-virtualization and policy-oriented framework for managing logical data objects across storage resources and federated zones. It is relevant to IDS because it combines logical data identity, metadata catalogs, event-driven rules, storage abstraction, and cross-zone data management. [R8]

### Overlap

Both systems aim to hide physical storage complexity from users and manage workflows across multiple locations. Both can be policy-oriented rather than forcing every user to choose a physical file server.

### Difference

IDS's intended hierarchy and addendum focus more directly on finite node capacity, H-unit compensation, regional DB clusters, dataset ordering, and commit semantics. The expanded IDS vision would add an explicit graph of authority and custody, plus protocol-level resource admission and recovery obligations.

### What to learn from iRODS

iRODS shows the value of policy-driven data workflows and long-lived data management abstractions. IDS should avoid duplicating mature features unnecessarily. A strong implementation should clearly state whether iRODS-like rule processing is a compatible backend, an integration target, or functionality that IDS genuinely needs to implement differently.

## 8.12 Rucio

Rucio is a distributed scientific data-management system. It represents files and datasets, storage endpoints, replication rules, accounts, and transfer tasks. A rule can require a minimum number of replicas in a set of storage resources; a rule engine evaluates the requirement and creates transfers needed to satisfy it. [R9]

### Why Rucio is a major comparator

Rucio demonstrates declarative durability requirements at a level above individual disk writes. A user can specify a desired replica count and storage expression, and the rule engine works out which transfers will satisfy the rule. It also manages heterogeneous resources and large scientific datasets.

This is close to IDS's proposed “durability as a service obligation” model. IDS cannot claim that abstract replica requirements or automated repair of missing copies are new.

### What IDS could add

The potential difference is in the breadth and semantics of responsibility. A Rucio rule primarily expresses replica-placement obligations across Rucio Storage Elements. IDS could define an end-to-end service contract that includes which domain has authority, what node/H-unit/region is responsible, which consistency and read/write semantics apply, what evidence is accepted, whether compute may execute near the data, and how responsibilities are transferred if an operator exits.

### Comparative test

Give Rucio and IDS the same requirement: maintain three verified copies of a 100 TB scientific dataset across independent facilities, never place two copies inside the same power or administrative failure domain, keep the authoritative copy within a specified jurisdiction, and reconstruct lost replicas within a target time. Compare plan correctness, policy expression, recovery behavior, and operator withdrawal handling. Do not compare only number of replicas.

## 8.13 iStore and federated file-system research

iStore explores federation file systems over geographically distributed edge servers, including global namespace, resource-aware data placement, job-aware placement, and migration. Its work is a reminder that a global logical data view across distributed storage locations is already a researched idea. [R10]

IDS could differ by adding stronger operator autonomy, explicit responsibility transfer, and distinct ordering/commit semantics. But these are not automatic consequences of the global namespace. They need to become protocols and testable invariants.

## 8.14 CrossFS (2026)

CrossFS, published in ACM Transactions on Storage on 28 March 2026, is a cross-domain distributed file-system design that uses CRDT-based metadata synchronization and a hybrid synchronization model. Its stated purpose is to improve metadata consistency and efficiency across distributed and cross-domain environments. [R11]

### Why this changes the prior-art picture

CrossFS is a direct, recent counterexample to broad claims such as “IDS is novel because it synchronizes metadata across independent domains” or “IDS is the first cross-domain distributed file system.” Cross-domain storage and metadata synchronization are active research areas.

### Potential IDS distinction

CrossFS's reported focus is metadata management, synchronization efficiency, indexing, and file-system operations. IDS would need to show a different contribution at the service-responsibility level: local domain admission, custody, authority, durability commitments, transaction ordering, multi-level resource hierarchy, and recovery across domains that may be independently administered.

### What the paper means for IDS

The novelty claim must be narrow and comparative. The correct position is not that no one has built cross-domain storage. The question is whether IDS formalizes an operating protocol connecting data authority, resource admission, placement, scoped coordination, and responsibility-preserving recovery in a way that existing systems do not.

## 8.15 Gaia-X and International Data Spaces concepts

Gaia-X and related data-space architectures focus on participant identity, trust, credentials, policy, and controlled data exchange. The Gaia-X Trust Framework describes a process for validating trust criteria and for defining machine-readable rulebooks and policy enforcement. [R12]

### Overlap

The expanded IDS vision includes domains that control their own resources and policies. That immediately raises trust and authorization issues. Data-space frameworks demonstrate that federation requires more than network connectivity: participant identity, credentials, rulebooks, and enforcement matter.

### Difference

A data space can coordinate access and usage policies without itself becoming a distributed storage system. IDS would connect those policy decisions directly to storage placement, operation commit, replica custody, and recovery. It could become a resource-operating layer underneath or alongside a data-space governance framework.

### Critical separation

A domain's permission to expose a dataset is not the same as a domain's permission to host the dataset. A domain's permission to host bytes is not the same as permission to inspect them. A domain's permission to execute an approved computation is not the same as permission to export the raw data. IDS must represent these as distinct capabilities and permissions.

## 8.16 IPFS and Filecoin-style decentralized storage

Content-addressed systems separate the identity of content from the network location of the provider. Decentralized storage networks can discover providers or storage commitments without requiring all data to be held inside one conventional cluster. These systems are relevant to IDS's proposed topology-independent identity and multiple-provider direction. [R13–R14]

### Overlap

IDS's logical identity should not depend on the current node path. It could locate data across multiple resource owners and use content hashes or equivalent integrity identifiers to validate a copy. Decentralized storage demonstrates important patterns for content addressing, provider discovery, and storage commitments.

### Difference

Content-addressing alone does not provide the full semantics of a live database service. It does not by itself define mutable data ownership, transaction ordering, conditional updates, dataset-level visibility, domain-specific policy, or application query guarantees. Economic storage deals also differ from a general-purpose data-service contract.

### Possible synthesis

IDS could use content-addressed immutable chunks or content hashes as one layer, while maintaining a separate logical data identity and mutable dataset metadata. A custody graph could connect the logical asset to content-addressed representations, storage providers, and proof records. That is a possible integration, not a current IDS capability.

## 8.17 Edge-cloud orchestration and compute federations

Edge and cloud orchestrators can place workloads across devices, clusters, and regions, often using resource discovery, policy, latency, and service requirements. These systems overlap with the expanded IDS proposal for joint placement of data and compute.

The critical gap is that a compute scheduler usually reasons about whether a workload can run on a node; a data system reasons about where data lives and how it is replicated. A joint data-compute system must reason about both while preserving data authority and storage obligations. It should not place computation at a node that is cheap or nearby if that node is not permitted to process the data.

IDS's potential distinction is therefore not a generic scheduler. It is a common placement plan in which data access, execution rights, compute capacity, network cost, and recovery responsibilities are evaluated together.

## 8.18 Summary: the prior-art lesson

The comparison yields one clear lesson: the broad vision is not unprecedented in every component. Existing systems have already solved many subproblems, often exceptionally well. A credible IDS contribution has to live in the protocol and invariants that bind those subproblems into one federated service.

| Existing system demonstrates | IDS must move beyond |
|---|---|
| Hierarchical placement | Hierarchy with operational responsibility and admission semantics |
| Namespace federation | Federation with data authority and custody |
| Geo-replication | Service-level durability across independent domains |
| Declarative replica count | Verifiable and transferable obligations |
| Distributed consensus | Coordination restricted to actual semantic dependencies |
| Data sovereignty policies | Policy decisions tied to concrete placement and execution actions |
| Automatic repair | Safe responsibility transfer across nested and administrative failure boundaries |
| Compute locality | Joint data/compute/network placement respecting authority and policy |

---

# 9. ARCHITECTURAL DIMENSION COMPARISON

## 9.1 Data identity versus physical location

A distributed system often has an application-level name for an object and an internal mapping to physical shards, blocks, replicas, or files. This separation is a mature technique. IDS should treat it as a foundational property, not a novel invention.

The difference worth investigating is whether one logical identity can remain stable while the data's custodian, authoritative region, storage medium, node membership, or replica topology changes. That identity must also distinguish the logical dataset from its particular byte representations. A single dataset may have several replicas, compressed forms, erasure-coded fragments, cached forms, and derived datasets. These are related representations, not necessarily independent logical assets.

| Entity | What it identifies | Example | Stability expectation |
|---|---|---|---|
| Logical dataset ID | User-facing data asset | `dataset:climate-archive-2032` | Stable through migration and rebalancing |
| Object/record ID | A logical member of a dataset | `observation:station-14:sample-88` | Stable according to data model |
| Operation reference | One client operation | Nine-character reference format described in IDS-A | Unique within defined generation scope; not an ordering number |
| Physical representation ID | One concrete stored encoding/copy | Replica 2 or fragment 5 of 12 | May change with re-encoding or migration |
| Resource ID | Node, H-unit, region, domain | `DB2/H4/N88` | Stable only for the resource lifetime; replacement is a new identity |
| Responsibility record | Who currently owes what service obligation | Domain D7 must retain replica R2 | Must be versioned and auditable |

A critical distinction is that an operation reference is not a sequence number. The nine-character reference format in the addendum can correlate requests and logs, but it does not define commit order, causal order, or uniqueness across a distributed federation unless its generation and collision-resolution rules are specified.

## 9.2 Topology as physical structure versus topology as governance

Ceph and similar systems model topology to avoid correlated failures. IDS can use the same idea. The proposed expansion adds a further dimension: topology may also encode administrative ownership, local policy, and willingness to accept responsibility.

| Topology attribute | Physical meaning | Governance meaning | Why both matter |
|---|---|---|---|
| Node | Machine or storage process | May be operated by an owner or service provider | Node's real capacity and access boundary must be known |
| H-unit | Group of nodes | Possible local resource-management authority | May accept or decline compensation work |
| Region | Geographic data centre / cluster | May map to jurisdiction, operator, contract, and service policy | A copy in the same region may not satisfy independent-domain durability |
| Operator domain | Administrative trust boundary | Organization responsible for specific actions | Necessary for consent, audit, and exit handling |
| Network segment | Latency/failure domain | May imply security zone or access-control boundary | Connectivity and permission are different properties |

A placement algorithm that optimizes only physical geography may choose three machines in three racks, while all three are owned by one operator and depend on one identity provider. That can be sufficient for machine-failure resilience but insufficient for operator-level resilience. IDS must state which guarantee is intended.

## 9.3 Storage grouping versus responsibility grouping

A group of nodes can be used for convenient inventory or placement. A responsibility group additionally has state that determines which data and service properties it has accepted.

| Property | Inventory group | Responsibility group |
|---|---|---|
| Membership | Lists its members | Maintains validated membership and membership epochs |
| Capacity | Sums node capacities | Tracks capacity committed to obligations, reserve, and available headroom |
| Placement | May influence placement | Accepts or rejects specific placement obligations |
| Failure | Reports failed members | Determines which obligations are endangered and invokes transfer/reconstruction |
| Recovery | May assist a repair | Has explicit source/target role and evidence requirements |
| Authority | Mostly organizational | Enforces local admission and operational policy |
| Exit | Group can be deleted | Must transfer or discharge every outstanding responsibility |

The IDS H-unit must reach the right-hand column to support the expanded novelty direction. The baseline H-unit concept moves toward this but does not yet formalize all properties.

## 9.4 Replication versus custody

Replication answers: where are copies or fragments stored? Custody answers: who is accountable for preserving a representation under defined conditions?

A system can have three replicas but no clear account of whether all three are in independent failure domains. It can have a replica that has not caught up to the authoritative version. It can have a replica on a node that is being decommissioned. It can have a copy under a legal hold or a copy that is not allowed to be moved across a border. Therefore, a replica count alone is not a complete service guarantee.

| State | Meaning | Can the system claim durability target satisfied? |
|---|---|---|
| Planned | A placement target has been selected | No |
| Accepted | The target domain has accepted the obligation | Not yet; resources may not contain data |
| Transferring | Data is being copied or reconstructed | No |
| Stored | Bytes exist at destination | Not necessarily; integrity and version must be checked |
| Verified | Destination data matches the expected representation/version | Possibly, if the failure-domain and retention rules are also satisfied |
| Committed as custody | The destination is formally responsible under the policy | Yes, if all policy conditions are met |
| Releasing | Previous custodian is being removed from the responsibility set | Only after replacement is committed or policy explicitly permits risk |

This lifecycle is an IDS proposal that should be formalized. Rucio already demonstrates declarative replica requirements and automated transfers, so IDS must add something substantive beyond re-labeling a replication rule as custody. [R9]

## 9.5 Local autonomy versus global consistency

Autonomy and consistency can conflict. A domain cannot accept arbitrary local writes and also promise a single total order across every region under every network partition without paying coordination costs or restricting progress.

IDS's independent ordering domains are a way to scope coordination. The addendum describes dataset-level ordering and resolving conflicting operations in the relevant coordination domain. The expanded novelty document proposes moving further toward causality-scoped lanes and dependency graphs.

| Situation | Desired behavior | Coordination consequence |
|---|---|---|
| Two writes to independent datasets | Proceed independently | No reason to serialize globally |
| Two writes to same ordered dataset | Respect declared order | Coordination within that dataset's ordering scope |
| Two writes to same key/object | Resolve conflict according to data semantics | At least object/partition-level conflict handling |
| Cross-dataset transaction | Preserve declared atomicity or expose partial semantics | Explicit cross-domain dependency and coordination |
| Read from stale replica | Follow requested consistency level | Version/commit visibility check required |
| Region isolated from network | Continue only operations permitted by local policy and consistency contract | May require read-only, local-write, or degraded modes |

The research goal should not be “avoid consensus everywhere.” It should be “coordinate exactly where the application contract requires it, and preserve a precise contract when coordination is unavailable.”

## 9.6 Recovery versus reconstitution

Traditional recovery often asks whether a lost replica can be copied from a surviving replica. IDS's expanded model asks whether the service obligation can be reconstituted even when a whole administrative domain is no longer present.

The difference is subtle but important. A data service may depend on a particular region for authoritative writes, another region for read locality, and a third region for disaster recovery. If the authoritative region disappears, it is not enough to find bytes somewhere else. The system must establish which representation is current, who may assume write authority, how pending writes are treated, and how the old authority is fenced from returning and accepting conflicting writes.

| Failure | Ordinary repair response | Extended IDS recovery question |
|---|---|---|
| Disk failure | Rebuild from replicas/fragments | Which node/group accepts the responsibility and how is capacity reserved? |
| Node loss | Replace replica | Is the replacement in an independent failure domain? |
| H-unit loss | Rebalance within region | How do other H-units accept its data obligations without overcommit? |
| Region loss | Fail over or restore region replica | Which region becomes authority, and what is the status of pending operations? |
| Operator exit | Manual migration or contract termination | Can every data and service obligation be safely transferred before the operator leaves? |
| Policy revocation | Stop access or move data | Which copies must be deleted, retained, or re-placed, and who attests completion? |
| Network partition | Degrade availability or consistency | Which side may accept writes, which operations can progress, and how is split-brain prevented? |

## 9.7 Data/compute/network placement

Modern applications often move data to compute, compute to data, or replicate data near consumers. Each approach has costs. IDS's expanded proposal is to treat data location, compute placement, network path, policy, and authority as one optimization problem.

This does not mean every query should run on a storage node. It means the plan should consider where the data is, where the computation is permitted, what network transfer is needed, and what resources the domain has agreed to provide.

| Placement objective | Naive independent approach | Joint IDS objective |
|---|---|---|
| Low latency | Pick closest data replica | Pick legal data source plus permitted nearby compute and network path |
| Low cost | Store at cheapest provider | Include transfer, egress, repair, and compute cost over service lifetime |
| Data sovereignty | Pin bytes to a geography | Pin bytes and constrain where raw data, metadata, and derived data may be processed |
| AI training | Copy dataset to a GPU cluster | Compare data movement against compute-to-data or distributed training execution |
| Disaster recovery | Keep a second copy | Place copies across genuinely independent failure and operator domains |
| Energy efficiency | Choose low-power storage | Optimize storage, cooling, network transfer, compute energy, and carbon-aware scheduling |

## 9.8 Client interface and operational transparency

IDS should expose stable, understandable client operations while preserving meaningful control over semantics. “Transparent” must not mean hiding important state. A client should not have to know the IP address of every node, but it may need to specify that a write must be synchronously durable in two independent domains, or that a read may be stale by up to a defined interval.

A robust interface should separate convenience from semantics:

- Convenience: logical dataset IDs, simple SDKs, automatic placement, location resolution.
- Semantics: consistency level, durability condition, region constraints, retention, operation status, commit evidence.
- Diagnostics: operation reference, responsible domains, version, replication state, degraded mode, recovery progress.

A system that hides physical topology but also hides whether data is committed or durable creates false confidence. IDS's commit-state model is an advantage only if the API exposes it accurately.

---

# 10. THE H-UNIT: WHAT IS ALREADY KNOWN AND WHAT IDS WOULD NEED TO ADD

## 10.1 Why H-units matter

The H-unit is one of the clearest structural features in the current IDS documents. It sits between a regional database cluster and individual storage nodes. The user-defined example places N117–N119 inside H1, N120–N122 inside H2, and N123–N125 inside H3. The addendum explains that a permanent failure of a node in H1 can be compensated by another H-unit, if that H-unit has the required data or can reconstruct it.

The important insight is that a region is too broad to be the only unit of placement and failure management, while a node is too fine-grained to express every regional responsibility. An intermediate group creates a place to manage member health, capacity, and cross-group compensation.

However, this architecture sits close to existing concepts such as racks, CRUSH buckets, HDFS block pools, Ozone pipelines, placement groups, replication groups, and storage pools. Its novelty must be behavioral, not nominal.

## 10.2 Four possible H-unit maturity levels

| Level | H-unit behavior | What it proves | Novelty strength |
|---|---|---|---|
| H0 — Label | A name used to group nodes | Basic inventory organization | None by itself |
| H1 — Placement group | Placement algorithm spreads data among H-units | Failure-domain-aware grouping | Established class of mechanism |
| H2 — Recovery domain | H-unit tracks health/capacity and participates in cross-H recovery | Richer recovery organization | Potentially useful; compare against existing systems' recovery groups |
| H3 — Autonomous responsibility cell | H-unit admits or declines obligations, manages commitment budgets, proves custody/reconstruction, and transfers obligations | Operational autonomy and explicit responsibility semantics | Stronger candidate contribution; still requires prior-art review |

The current documents specify H-unit membership, capacity, metadata, and compensation as important design goals. The H3 behaviors are a proposed expansion that should not be silently treated as already specified.

## 10.3 A concrete H1 → H2 compensation example

Assume H1 contains N117, N118, and N119. Dataset D42 has fragments or replicas assigned across H1 and other eligible H-units. N119 permanently fails.

A weak implementation does this:

1. Detect N119 failure.
2. Pick any free node.
3. Copy bytes from a surviving replica.
4. Mark the repair done.

That is ordinary replica repair. It ignores correlated failures, capacity commitments, versions, and whether the target is allowed to host the data.

A stronger IDS implementation would need a sequence such as:

1. **Failure classification:** distinguish permanent node loss from transient unreachability, congestion, disk failure, or regional isolation.
2. **Responsibility impact:** enumerate datasets, fragments, write responsibilities, and in-flight operations affected by N119.
3. **Source qualification:** identify a verified source or a set of fragments sufficient for reconstruction.
4. **Candidate H-unit discovery:** identify H2/H3 candidates based on independent failure domains, free capacity, policy, network reachability, and current workload.
5. **Local admission:** each candidate H-unit evaluates whether it can accept the specific responsibility.
6. **Plan commitment:** reserve the required capacity and identify the exact version/encoding to transfer.
7. **Data movement or reconstruction:** copy a replica or reconstruct an encoded fragment.
8. **Integrity verification:** verify the expected content/version and encoding parameters.
9. **Durability re-evaluation:** determine whether the complete dataset's durability profile is now satisfied.
10. **Responsibility commit:** record which H-unit/node has assumed the new obligation.
11. **Old responsibility retirement:** retire the failed node's record while preserving audit history.
12. **Client visibility:** expose the affected service state and operation references without declaring recovery complete prematurely.

If IDS can formalize and verify this sequence, its H-unit model becomes materially more than a box in a diagram.

## 10.4 Finite capacity is a first-class constraint

Every node has finite capacity. But “free disk space” is not equivalent to “available capacity for a new obligation.” A node may need reserved space for repair, compaction, metadata, temporary reconstruction, and expected growth. It may also have limits on IOPS, bandwidth, CPU, memory, or encryption throughput.

An H-unit should calculate at least:

- raw capacity;
- usable capacity after formatting and system reservations;
- committed capacity;
- uncommitted capacity;
- repair reserve;
- expected growth reserve;
- resource-specific headroom;
- capacity blocked by policy or retention;
- temporary reconstruction headroom;
- capacity unavailable due to node health or degraded mode.

A simplistic policy such as “choose the H-unit with the most free disk” can generate a poor placement. The H-unit may be full of slow disks, have saturated network links, or share a power failure domain with the source. The actual capacity model must be multidimensional.

## 10.5 H-unit independence needs a definition

The word “independent” can refer to several properties:

| Independence type | Example | Can IDS assume it from H-unit identity alone? |
|---|---|---|
| Naming independence | H1 has a distinct ID from H2 | Yes, if IDs are unique |
| Membership independence | Nodes are not members of both H1 and H2 | Must be enforced |
| Failure independence | Failure of H1 does not cause failure of H2 | No; requires topology and failure evidence |
| Administrative independence | H1 and H2 have separate operators or delegated authority | No; requires explicit ownership metadata |
| Network independence | H1 and H2 do not rely on one shared network component | No; must be modeled |
| Power independence | Separate feeds or power domains | No; requires infrastructure facts |
| Credential independence | Compromise of one H-unit's credentials does not control another | No; requires security architecture |

This distinction is vital. A system can claim H1-to-H2 compensation only when H2 is sufficiently independent for the failure scenario being addressed.

## 10.6 H-units as local operational cells

The expanded direction could make H-units self-contained operational cells. An H-unit could expose a capability record that says what it can safely do, such as:

- accept a cold archive replica;
- serve a read replica;
- accept latency-sensitive writes;
- reconstruct a particular erasure-code profile;
- run a permitted analytics workload;
- preserve encrypted data without accessing plaintext;
- hold a time-limited recovery copy;
- serve as a temporary migration target.

This is a useful model because not every H-unit needs to offer every capability. It also prevents IDS from treating every storage node as interchangeable.

**Inferred impact:** a data service can be built from many specialized resource cells rather than one homogeneous cluster. This could reduce the cost of maintaining a global system, because each domain contributes only the capabilities it is designed to provide.

**Research status:** the capability-advertisement and negotiated-admission protocol is proposed, not established by the current H-unit spec.

---

# 11. RESPONSIBILITY, CUSTODY, AUTHORITY, AND RECOVERY

## 11.1 Four concepts that must not be collapsed

A major design weakness in many distributed-system descriptions is using words such as “owner,” “replica,” “master,” “responsible,” and “custodian” loosely. IDS should define them precisely.

| Concept | Meaning | Example |
|---|---|---|
| Data ownership | Legal or organizational authority over the dataset | University A owns a research dataset |
| Mutation authority | Permission to create or change the authoritative logical state | A service role may commit new observations |
| Storage custody | Duty to preserve a particular physical representation | Facility B holds a verified copy for 10 years |
| Serving responsibility | Duty or permission to answer reads under a declared consistency rule | Region C serves local reads from a committed replica |
| Recovery responsibility | Obligation to restore a lost durability or availability condition | H2 reconstructs a fragment after H1 failure |
| Execution authority | Permission to run approved code against the data | A compute domain may run a model but may not export raw records |

One domain may hold several roles, but the roles should be independently expressible. A storage provider should not automatically gain the right to inspect or mutate the data it stores.

## 11.2 A possible responsibility graph

The expanded IDS documents propose a Data Custody and Responsibility Graph. Conceptually, the graph can represent entities and the obligations connecting them.

```text
LOGICAL DATASET D42
      |
      +---- OWNED_BY ----> ORGANIZATION A
      |
      +---- MUTATION_AUTHORITY ----> SERVICE S1
      |
      +---- PLACEMENT_POLICY ----> POLICY P7
      |
      +---- REPRESENTED_AS ----> ENCODING E3
                                  |
                   +--------------+--------------+
                   |              |              |
                   v              v              v
               REPLICA R1     REPLICA R2      FRAGMENTS F1..F6
                   |              |              |
                 H1/N4          H2/N8           REGION C
                   |              |              |
             CUSTODIAN A    CUSTODIAN B     CUSTODIAN C
                   |              |              |
               VERIFICATION   VERIFICATION   RECONSTRUCTION
                 RECORD          RECORD          RECORD
                   \              |              /
                    +------ DURABILITY POLICY P7 ------+
```

This model separates the logical asset from its stored representations and separates physical existence from accepted custody. Each edge would need an explicit schema, version, validity period, and authorization model.

## 11.3 Why responsibility graphs could matter

A simple location map answers “where is the data?” A responsibility graph could answer:

- Which organization owns the dataset?
- Which identity is permitted to mutate it?
- Which representation is authoritative at a given version?
- Which regions hold read-serving replicas?
- Which H-unit is expected to reconstruct a missing fragment?
- Which operator has accepted the retention obligation?
- What happens if that operator withdraws?
- Which policy requires two independent operator domains?
- Which verification result supports the current durability status?

The graph becomes useful when the infrastructure changes without changing the logical data identity. A node can be replaced, an H-unit can be retired, a regional cluster can be decommissioned, or an operator can leave. The logical data asset remains, but the responsibility relationships change.

## 11.4 Responsibility transfer must be atomic at the obligation level

Suppose Domain A is the only custodian of a verified archive copy and notifies IDS that it will exit in 30 days. IDS should not merely mark the region “draining.” It must enumerate all obligations Domain A has accepted, determine which can be transferred, and prove that replacements have accepted them before Domain A is released.

A safe transition might look like:

```text
DOMAIN A: ACTIVE CUSTODIAN
        |
        v
WITHDRAWAL NOTICE / DRAINING
        |
        v
ENUMERATE OUTSTANDING OBLIGATIONS
        |
        v
FIND ELIGIBLE REPLACEMENT DOMAINS
        |
        v
LOCAL ADMISSION + CAPACITY RESERVATION
        |
        v
COPY / RECONSTRUCT / VERIFY
        |
        v
REPLACEMENT CUSTODY COMMITTED
        |
        v
DOMAIN A RESPONSIBILITY RELEASED
        |
        v
DOMAIN A REMOVED FROM ACTIVE FEDERATION
```

This avoids two unsafe states:

1. **Responsibility gap:** Domain A is released before any replacement has accepted the obligation.
2. **Authority duplication:** Domain A and its replacement both believe they have exclusive mutation authority after a transfer.

A protocol must define which obligations can be transferred in parallel and which require a fenced handoff. It must also handle a domain that disappears without providing notice.

## 11.5 Responsibility is not a substitute for cryptographic verification

A signed statement that a domain accepts a custody obligation does not prove that the data exists or that it remains intact. The service must combine authority records with data integrity checks, version tracking, and possibly challenge-response proofs or periodic scrubbing.

The evidence model must specify:

- what content or version is being attested;
- which encoding parameters are expected;
- who produced the verification record;
- when it was produced;
- which trust root validates the signer;
- whether the record can be revoked or superseded;
- how an auditor can reproduce or independently verify the result.

No single technique is universally sufficient. Full-copy hashes, Merkle structures, erasure-code reconstruction checks, storage proofs, and periodic scrubbing have different threat models and cost profiles.

## 11.6 Data deletion and retention are also obligations

A federated data service must handle deletion and retention correctly. If an owner requests deletion, the system must identify all active copies, derived copies, caches, replicas, fragments, and backup representations that fall within the deletion policy. If a legal hold applies, that obligation may prevent immediate erasure. If the system uses erasure coding, deleting one fragment is not necessarily equivalent to deleting the logical object.

The custody graph could make these relationships explicit. It would need to track data lineage and representation relationships, not just current replica locations.

This is an important potential difference from a conventional location map, but it is also a difficult research and systems problem. Deletion across disconnected domains cannot be honestly declared complete until each required participant has either confirmed deletion or a policy-defined exception has been recorded.

## 11.7 What must be implemented first

Before implementing a large graph database, IDS should define a minimal responsibility state machine. A record should contain at least:

- logical asset and version;
- physical representation identity;
- responsible domain/H-unit/node;
- role of responsibility;
- policy reference;
- accepted timestamp and expiry/renewal terms;
- capacity reservation identifier;
- verification status;
- previous and successor responsibility records;
- fencing or authority epoch where relevant.

The data structure can be a graph, relational tables, append-only event log, or a combination. The novelty is not that it is literally a graph. The novelty would be the protocol semantics and safety properties that the graph represents.

---

# 12. ORDERING, CONCURRENCY, AND CONSISTENCY DIFFERENCES

## 12.1 The original problem

In a distributed system, multiple clients may issue operations at the same time. Some operations are independent; others conflict. A system must define what it means for one write to happen before another, what a read may observe, how duplicate requests are handled, and when an operation is considered committed.

The IDS addendum separates operation identity from ordering. It proposes unique operation references and dataset-scoped ordering domains, with conflict handling restricted to operations that require coordination. It also distinguishes states such as received, ordered, executing, committed, and synchronized.

These are useful distinctions. They are not automatically a new consistency model. Distributed databases already provide partition- or range-scoped ordering, consensus groups, transaction isolation, and commit protocols. IDS must formalize what is new in the combined semantics.

## 12.2 The danger of one ordering lane per dataset

A dataset-scoped ordering lane is easy to reason about: writes to the same dataset have an explicit sequence. But it can create a bottleneck when one large dataset contains many independent partitions. It can force unrelated records to wait behind a slow or conflicted operation.

For example, a global environmental dataset might contain observations from millions of independent sensors. If every sensor update shares one global write sequence, the system may serialize activity that has no semantic dependency.

A future model may need a hierarchy of operation scopes:

- dataset scope for metadata and dataset-wide invariants;
- partition or key-range scope for independent data regions;
- object scope for conflicting changes to the same object;
- transaction scope for multi-object atomic operations;
- global scope only for genuinely system-wide invariants.

The expanded NOVEL document already points toward causality-scoped lanes and dependency graphs. That is a useful direction, but it must not be presented as implemented by the current addendum.

## 12.3 A possible causality graph

Each operation can declare or inherit dependencies. An operation is eligible to commit once the required predecessor operations are committed or otherwise resolved under its consistency policy.

```text
WRITE A1 ----> WRITE A2 ----> UPDATE INDEX A3

WRITE B1 ----> WRITE B2

WRITE C1 ----> WRITE C2 ----> CROSS-DATASET TX C3
                              ^
                              |
                         DEPENDS ON A3
```

In this example, A1 and B1 can progress independently. A3 depends on A2. C3 depends on both C2 and A3. The system does not need to impose a total order across all operations, but it must preserve the specified dependency graph.

A causality graph is only useful if its overhead stays manageable. A system cannot store unbounded dependency edges forever. It may compress, checkpoint, or summarize causality after a committed prefix becomes stable. These lifecycle rules are part of the design.

## 12.4 Operation reference versus sequence number

The nine-character alphanumeric reference introduced in IDS-A is a correlation identifier. It identifies an operation independently from its order position. This is sensible because generating a globally unique operation ID should not require globally coordinating every operation.

However, a small reference space needs careful collision handling. With nine characters, the total namespace depends on the alphabet size and case rules. If the alphabet is 36 characters, there are 36^9 possible values; if 62 characters are allowed, there are 62^9. The system still needs to define generation, randomness or deterministic partitioning, collision checks, and the uniqueness scope. A short human-readable ID is not automatically a globally unique distributed identifier.

| Identifier | Purpose | Must define |
|---|---|---|
| Operation reference | Trace one request through system | Uniqueness scope, collision handling, idempotency binding |
| Logical dataset ID | Address data independent of node location | Namespace, ownership, lifecycle and rename rules |
| Ordering token | Determine operation order in one lane | Authority, epoch, gaps, aborts, leader changes |
| Version ID | Identify committed data state | Monotonicity or causal semantics, branch handling |
| Responsibility epoch | Prevent stale custodians from acting after transfer | Fencing and conflict resolution |

## 12.5 Commit is not replication completion

IDS-A separates commit from later synchronization. This can be important because a write may become committed under a durability condition before every replica is fully synchronized.

The API should distinguish at least:

| State | Meaning | Client-facing interpretation |
|---|---|---|
| Accepted | IDS has received and validated the request envelope | Request has been admitted to processing; not durable |
| Ordered | The operation has a place in its required ordering scope | Position is known; not necessarily applied |
| Executing | Storage or transaction work is in progress | No final result yet |
| Locally applied | The target's local state has changed | Local progress only |
| Prepared | Required participants have prepared or reserved resources | May still abort or fail |
| Committed | The declared commit rule has been satisfied | Operation is logically successful under the stated guarantee |
| Replicating | Additional copies/fragments are being propagated | Durability may be below the desired steady-state policy if the commit rule allows it |
| Synchronized | Required participating replicas/domains have caught up | A broader replication condition has been reached |
| Verified | Integrity and version checks passed | Evidence supports correctness of stored state |
| Failed/aborted | Operation did not commit | Retry semantics and idempotency must be clear |

The key issue is not how many status labels are used. It is whether every status maps to a precise guarantee. A client should never receive a generic “success” that is interpreted as committed when the system only accepted the operation.

## 12.6 Consistency profiles should be explicit

IDS can support multiple profiles if it makes their guarantees clear. Examples:

| Profile | Read behavior | Write behavior | Typical use |
|---|---|---|---|
| Local acknowledged | Read from local committed state | Commit after local durable write | Low-latency local capture where later sync is acceptable |
| Quorum durable | Read according to quorum/version rule | Commit after required replica quorum | Important operational data |
| Regionally durable | Read from an authoritative or eligible regional copy | Commit after multiple independent intra-region failure domains | Regional production database |
| Cross-region durable | Read with defined freshness | Commit after cross-region condition | Critical system of record |
| Eventually replicated | Reads may lag | Local commit, asynchronous propagation | Telemetry and bulk ingestion |
| Read-only degraded | Reads from verified surviving copies | Writes disabled or restricted | Network partition or regional outage |
| Causally consistent | Reads respect declared dependencies | Writes commit within causal scope | Collaborative or distributed workflows |

These are proposed profiles, not current IDS guarantees. Each profile needs formal read/write definitions, failure behavior, and tests.

## 12.7 Concurrency resolution

The addendum says coordination should be exceptional and focused on conflicts. The system must define what counts as a conflict. Two updates to the same object may conflict; two increments to a counter may be mergeable; two updates to independent fields may be compatible; two operations that violate a dataset-wide invariant may conflict even if they touch different objects.

A generic “last write wins” rule is not adequate for all data. For a bank ledger, it can lose money. For sensor telemetry, a monotonic timestamp may be acceptable. For a collaborative document, multi-value registers or CRDTs may be appropriate. For a scientific dataset, immutable observations plus append-only correction records may be preferable.

The correct IDS design would allow the data model or dataset schema to define conflict semantics. The coordinator would only invoke a broader agreement protocol when the operation's semantic dependencies require it.

## 12.8 What would be genuinely different

The candidate difference is not dataset ordering alone. It is a **cross-domain coordination contract** that makes the scope of order align with the scope of data authority, custody, and policy. An operation may be local to one domain, depend on a replica held by another domain, or update a dataset that spans multiple operators. IDS would need to determine which participants must agree, what each participant is authorizing, and which commit evidence is sufficient.

The strongest validation would show that IDS:

- preserves required consistency under adversarial concurrency;
- permits unrelated partitions to progress independently;
- safely handles region partitions and leader/authority changes;
- does not create duplicate authoritative writers;
- recovers pending operations after a domain exits;
- exposes enough status for clients to know whether an operation committed.

---

# 13. PLACEMENT, CAPACITY, AND DATA/COMPUTE/NETWORK DECISIONS

## 13.1 Placement is more than choosing a node

A placement decision has multiple dimensions. At a minimum, IDS must consider capacity, reliability, data location, access patterns, write rate, read rate, hardware class, network connectivity, and redundancy. In a multi-owner federation, it must additionally consider domain admission, authorization, sovereignty, contractual limits, and the expected cost of maintaining the obligation.

A useful placement plan should distinguish hard constraints from soft objectives.

| Hard constraint | Example | May the optimizer violate it to improve latency? |
|---|---|---|
| Residency | Data must remain in jurisdiction J | No |
| Authorization | Domain D cannot host confidential class C | No |
| Durability | At least two copies across independent failure domains | No, unless client explicitly accepts degraded mode |
| Capacity | Target must reserve enough storage and bandwidth | No |
| Data type | Raw data cannot be processed outside a permitted enclave | No |
| Retention | Data must remain for 10 years | No, subject to applicable deletion/legal policy |
| Soft latency objective | Prefer p95 below 30 ms | May be traded if declared |
| Cost objective | Prefer lower storage and transfer cost | May be traded against latency or availability |
| Energy objective | Prefer lower energy use | May be traded against other stated priorities |

## 13.2 A service-intent example

A client might request:

> Store dataset D for at least five years. Keep the authoritative copy in Region A. Maintain two additional verified copies across distinct operator and power-failure domains. Serve reads locally in Regions B and C where possible. Do not allow raw data to be processed outside Region A. Reconstruct any lost required copy within six hours. Report degraded state immediately.

This is more expressive than “replication factor = 3.” It describes authority, retention, failure separation, read locality, processing restrictions, recovery time, and observability.

IDS's proposed intent-to-infrastructure compiler would translate the requirement into a plan. It would identify eligible domains, check capacity and policies, negotiate acceptance, determine replica or erasure-code configuration, allocate storage, define verification checkpoints, and monitor whether the obligations remain satisfied.

This is conceptually related to declarative placement in systems such as Rucio and policy-based data systems. The potential difference lies in compiling a wider operational service contract across independent domains, then maintaining and proving that contract over time.

## 13.3 Placement planning steps

1. Parse the request into formal requirements.
2. Resolve the logical dataset and owner.
3. Classify data sensitivity and permitted operations.
4. Determine the data authority and write model.
5. Identify the required consistency and durability profile.
6. Query domain capability advertisements.
7. Remove domains that violate hard policy constraints.
8. Estimate available capacity and reserve requirements.
9. Compute feasible replica/fragment layouts.
10. Estimate network transfer and repair paths.
11. Estimate compute placement if processing is required.
12. Obtain local admissions from candidate domains.
13. Select a plan using declared optimization weights.
14. Commit plan records and capacity reservations.
15. Execute data movement and verification.
16. Activate the data service only when the activation conditions are met.
17. Re-evaluate continuously as capacity, health, policies, and demand change.

A production compiler must be deterministic enough to audit but adaptive enough to respond to changing infrastructure. It must also explain why a plan was selected and why alternatives were rejected.

## 13.4 Capacity model

Capacity has more dimensions than total storage. The system needs to track:

- storage bytes by media class and durability tier;
- usable storage after system overhead;
- write bandwidth and sustained write IOPS;
- read bandwidth and read IOPS;
- CPU headroom for checksum, compression, encryption, and reconstruction;
- memory for metadata and caching;
- network ingress and egress;
- temporary space for compaction, migration, or reconstruction;
- reserved capacity for recovery;
- operational headroom and growth forecasts.

A placement algorithm that maximizes utilization can damage availability. When a node is at 99.5% capacity, it may technically have free bytes but no safe room for repair, compaction, or incoming writes. IDS should optimize for sustainable committed capacity, not merely raw occupancy.

## 13.5 Failure-domain model

A resilience policy should name the failure domains it intends to survive. These might include disk, node, H-unit, rack, power domain, network segment, region, operator, cloud account, identity provider, or jurisdictional event.

| Redundancy target | Example placement | What it protects against | What it does not necessarily protect against |
|---|---|---|---|
| Disk-level | Two drives on distinct devices | Drive failure | Node, controller, power or host failure |
| Node-level | Replicas on different nodes | One node failure | Shared rack or network outage |
| H-unit-level | Replicas across H1 and H2 | One H-unit failure | Common region/provider failure |
| Region-level | Copies across Region A and B | Regional outage | Shared operator/cloud dependency |
| Operator-level | Copies across two organizations | One operator's exit or outage | Shared software/identity/cyber vulnerability |
| Jurisdiction-level | Copies in separate legal jurisdictions | Certain local legal or regional events | Global legal changes or common control over data |

The system should not overstate independence. Two cloud regions may still share an operator, software stack, billing account, or global identity service. The service requirement should declare which risks it intends to separate.

## 13.6 Joint data/compute/network optimization

The expanded architecture suggests that data placement and workload placement should be planned together. This could be especially valuable where moving data is expensive or legally restricted.

For example, moving 500 TB of training data to a GPU cluster may create large transfer cost and delay. Running an approved data preprocessing job inside or near the storage domain may be cheaper. But raw-data restrictions may prohibit exporting the dataset, and the local facility may not have GPUs. The system must compare options rather than blindly moving either all data or all computation.

A joint cost model might include:

`TotalCost = StorageCost + TransferCost + ComputeCost + RepairCost + PolicyCost + EnergyCost + AvailabilityPenalty`

This is a conceptual objective function. A real implementation must define measurable units, weights, and constraints. Availability and sovereignty should normally be hard constraints rather than soft penalties.

## 13.7 What is genuinely different here?

Storage systems optimize data placement. Workload schedulers optimize compute placement. Network controllers optimize paths. A possible IDS contribution is an orchestrator that treats the service's data, compute, network, policy, and responsibility state as a single planning problem.

The claim would become meaningful only if IDS can demonstrate measurable benefits against a baseline that separately optimizes storage and compute. Relevant metrics include time-to-result, bytes transferred, egress cost, GPU idle time, recovery bandwidth, policy violations, and service availability.

---

# 14. OPERATIONAL WORKFLOWS COMPARED END TO END

This section examines what happens from the first client request to a final service state. It illustrates where IDS's proposed architecture might differ from a conventional distributed database or a replica-management service.

## 14.1 Write path

| Stage | Typical distributed database | Baseline IDS intent | Expanded IDS proposal |
|---|---|---|---|
| Request identity | Client request and transaction ID | Client request plus dataset resolution and operation reference | Operation bound to logical asset, authority, and policy context |
| Placement | Database's internal partition/replica map | Region → H-unit → node resolution | Intent compiler selects domain-approved target plan |
| Admission | Cluster/database authorization | Client authorization and infrastructure availability | Local domain admission for cross-domain responsibilities |
| Ordering | Database-specific transaction/partition order | Dataset-scoped ordering direction | Causality graph and conflict-specific scopes |
| Data write | Replicated transaction or storage write | Execute at resolved destination(s) | Execute only after necessary capacity and obligations are accepted |
| Commit | Database commit rules | Explicit commit condition; not equal to later synchronization | Commit record includes scope, policy, and evidence references |
| Replication | Database-specific | Selective replication and synchronization | Replicas are custody obligations with verification states |
| Response | Success/error and transaction result | Operation reference and commit state | Client-visible state includes degraded conditions and service evidence |

## 14.2 Read path

A read is not always “find the closest replica.” The system must know whether the replica is current enough, whether it is allowed to serve this client, whether the client asked for strong or stale reads, and whether it is in a verified state.

| Step | Required decision |
|---|---|
| Resolve ID | Which logical dataset/object does the client mean? |
| Authenticate | Is the client allowed to read this data? |
| Policy evaluation | Is this domain permitted to serve the data to this client? |
| Candidate selection | Which copies/fragments are available, healthy, and policy-compliant? |
| Freshness | Which versions satisfy the requested consistency profile? |
| Execution | Can one replica serve directly, or must fragments be reconstructed? |
| Verification | Does the returned data match the required version/integrity proof? |
| Response | What version and consistency guarantee should the client be told? |

An IDS client should be able to request “nearest verified read with at most 5 seconds of staleness,” rather than only “get data.”

## 14.3 Node-failure path

A node failure is the narrowest recovery case. IDS should detect the failure, distinguish transient from permanent status, calculate affected data, and select a replacement. It should not transfer data blindly if another replica is already stale or if the target violates placement rules.

The H-unit layer should identify the local consequences and whether another H-unit must accept a compensation responsibility. The region-level controller should know whether this node failure threatens a regional durability objective. The client should be informed when a data service is degraded, not only when a read fails.

## 14.4 H-unit failure path

An H-unit failure affects a group, not one device. The system should evaluate all responsibilities held by its nodes. If H1 is unavailable, H2 may serve data that it already has. H3 may reconstruct data from fragments. Another region may serve a copy if the region-level policy allows that and the data's residency rules permit it.

The system should not assume that every dataset in H1 has the same replacement path. Different datasets may have different replication counts, data classification, read/write rates, and recovery deadlines.

## 14.5 Regional failure path

A regional outage affects the regional database cluster and potentially its H-units, nodes, metadata, network, and local authority. The system must decide whether another region can serve reads, whether it can accept writes, and what happens to in-flight writes that may or may not have committed before the outage.

This is a different problem from node repair. A remote region may have the bytes but lack a valid authority epoch to accept new writes. IDS must define leader or authority fencing to prevent a recovered region from coming back and creating split-brain.

## 14.6 Operator withdrawal path

The largest difference between ordinary cluster repair and multi-owner federation appears when a domain deliberately leaves. This is not a machine crash. The operator may give notice, limit bandwidth, refuse new work, or terminate connectivity according to a contract.

The system needs a withdrawal protocol. It must identify all obligations; stop assigning new ones; find replacement domains; verify transfers; handle data deletion or retention; revoke credentials; update routes; and close audit records. Some obligations may be impossible to transfer within the requested period. The service must expose that condition honestly.

## 14.7 Policy-change path

A domain may change policy. For example, a facility may stop accepting a data classification, or a data owner may withdraw permission for a particular region to host a copy. The system must reevaluate all affected placements and jobs, not just future writes.

A policy change may require migration, deletion, re-encryption, or execution relocation. The responsibility graph would help identify affected representations. The system must define the precedence between retention obligations, deletion requests, legal holds, and security restrictions.

## 14.8 Recovery state machine

A generalized recovery state machine could be:

```text
NORMAL
  |
  v
DEGRADED_DETECTED
  |
  v
FAILURE_CLASSIFIED
  |
  v
IMPACT_AND_OBLIGATIONS_ENUMERATED
  |
  v
RECOVERY_PLAN_COMPUTED
  |
  v
TARGETS_ADMITTED_AND_RESERVED
  |
  v
DATA_TRANSFER_OR_RECONSTRUCTION
  |
  v
INTEGRITY_AND_VERSION_VERIFIED
  |
  v
DURABILITY_POLICY_RECHECKED
  |
  v
RESPONSIBILITY_TRANSFER_COMMITTED
  |
  v
RESTORED
```

Possible branches include `UNRECOVERABLE`, `POLICY_BLOCKED`, `SOURCE_UNTRUSTED`, `CAPACITY_UNAVAILABLE`, and `MANUAL_REVIEW_REQUIRED`. The system must preserve the reason rather than converting every failure into a generic retry.

---

