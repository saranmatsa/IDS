# IDS — NOVELTY ASSESSMENT AND EXPANDED ARCHITECTURAL VISION

**Project:** IDS — Independent Distributed System  
**Parent ecosystem:** HX-241 / XarnX  
**Document:** `NOVEL.md`  
**Research date:** 18 November 2026  
**Sources:** Distributed databases, distributed file systems, storage federation, data grids, geo-distributed object stores, cross-domain systems, storage placement research, and data-sovereignty architectures.  
**Purpose:** Establish the defensible novelty position of IDS, identify the closest prior art, and expand IDS from a distributed data-operation architecture into a much broader federated data-infrastructure architecture.

---

# 1. EXECUTIVE FINDING

IDS should not be positioned as a new database, a new sharding algorithm, a new replication method, or a new erasure-code system. Those components already have extensive prior art.

The original IDS architecture already has a coherent system-level direction:

- One logical data-operation environment across geographically separated regional database clusters.
- Regional clusters that retain distinct operational identities.
- Storage nodes nested within regional infrastructure.
- H-units as an intermediate node-grouping and failure-compensation layer.
- Explicit data ownership and placement responsibilities.
- A hybrid choice between full replication and erasure coding.
- Independent ordering domains rather than one mandatory system-wide write sequence.
- Coordination without a permanently fixed, system-wide ordering master.
- Unified client operations over changing physical infrastructure.

Research shows that almost every one of those individual mechanisms has a close precedent. HDFS Federation, HDFS Router-based Federation, Ceph, iRODS, Rucio, iStore, MinIO Site Replication, Apache Ozone, YugabyteDB, CockroachDB, and other systems already cover important parts of this territory.

That does not eliminate IDS as a research direction. It changes where its novelty must reside.

## The defensible novelty position

**The strongest IDS contribution is a federated operating architecture for autonomous data infrastructure: a system in which independently administered domains contribute storage, compute, and network capabilities to a common logical data environment without surrendering their local operational authority. IDS translates high-level data-service requirements into domain-approved placement, custody, execution, ordering, durability, and recovery actions across a nested infrastructure graph.**

This is broader than federated database access. It is also broader than replicating data between clusters.

The proposed architectural distinction is the integration of five responsibilities into one operational model:

1. **Autonomous domain federation:** Each participating domain retains its own control plane, policies, identity, and admission authority.
2. **Intent-to-infrastructure compilation:** Clients and data owners describe the outcome they require; IDS resolves the intent into concrete cross-domain resource and data-placement actions.
3. **Explicit data custody and responsibility:** IDS tracks not only where bytes exist, but who owns the data, who may mutate it, which domains satisfy durability obligations, and who is responsible for recovery.
4. **Causality-scoped coordination:** Independent operations proceed independently; coordination is introduced only across the objects, partitions, or domains that have actual consistency dependencies.
5. **Multi-level autonomous recovery:** Node, H-unit, regional-cluster, operator-domain, and connectivity failures are handled as one connected responsibility-reassignment and data-reconstitution problem.

The individual ingredients are established. The research contribution is the precise architecture and protocol that make these five responsibilities work together across independently governed infrastructure.

This is the novelty direction IDS should develop and defend.

---

# 2. WHAT COUNTS AS NOVELTY IN IDS

Novelty must be assessed at several levels. Treating every feature as a separate invention would produce weak claims. Treating the entire project as unprecedented would be equally weak.

| Level | What is being claimed | IDS position |
|---|---|---|
| Primitive novelty | A new consensus algorithm, coding scheme, hash function, or storage primitive | Not the current IDS claim |
| Component novelty | A new placement engine, metadata structure, or recovery algorithm | Possible future research, but not established by the current specification alone |
| Architectural novelty | A different way to organize and connect known mechanisms | The current IDS direction |
| Protocol novelty | A specific protocol for autonomous domains to negotiate, execute, verify, and transfer data responsibilities | The strongest route to a concrete technical contribution |
| System novelty | A working implementation that combines the protocol and architecture into an operational system | The long-term engineering result |

A new name does not create novelty. Neither does combining a list of ordinary database features. A defensible claim must specify what the system does, how its mechanism differs from the closest systems, what guarantees it provides, and what tests demonstrate those guarantees.

IDS should therefore define its innovation around the operational model, not around superficial labels such as H-unit, dataset ID, or nine-character operation reference.

---

# 3. PRIOR-ART RESEARCH: THE CLOSEST SYSTEMS

The following research deliberately covers systems from different traditions. A comparison only against GFS would be insufficient because GFS is one historical distributed file system, not the complete modern design space.

## 3.1 Google File System (GFS)

GFS established a large-scale distributed file-system architecture using a master and many chunkservers. The master manages metadata and placement; clients obtain chunk locations and then communicate directly with chunkservers for data transfer.

The GFS paper explicitly states that its operation log defines a global total order of metadata operations. Its design also uses a single master process for mutations and system-level background activities, with replicated master state for recovery.

**Overlap with IDS:** logical data identity, distributed physical storage, replica locations, recovery, and operation ordering.

**Important difference:** GFS deliberately uses a single authoritative master for its metadata and global mutation order. IDS instead aims to separate independently scoped ordering domains and regional control responsibilities. This distinction matters, but the mere phrase “no permanent global master” is not sufficient novelty by itself; later distributed databases and decentralized placement systems already have domain- or shard-scoped coordination.

Primary source: [The Google File System, SOSP 2003](https://static.googleusercontent.com/media/research.google.com/en//archive/gfs-sosp2003.pdf).

## 3.2 Ceph and CRUSH

Ceph is one of the closest architectural comparisons for the storage hierarchy. Its CRUSH map models a physical hierarchy containing devices and intermediate buckets such as hosts, racks, rows, and data centers. Placement rules select replica or erasure-coded-fragment locations according to those failure domains.

Ceph also allows clients to compute object locations and communicate directly with storage daemons rather than routing all data through a central broker.

**Overlap with IDS:** hierarchical infrastructure, capacity-bearing storage nodes, failure-domain-aware placement, replication, erasure coding, and distributed access.

**Difference IDS must make meaningful:** an H-unit cannot be merely another name for a Ceph bucket. Its distinctive role must include a local operational boundary: membership, capacity, responsibility ownership, admission policy, recovery coordination, and the ability to accept or decline cross-H-unit compensation. The H-unit should be an operational participant, not only a topology label used by a placement function.

Primary sources: [Ceph CRUSH Maps](https://docs.ceph.com/en/latest/rados/operations/crush-map/) and [Ceph Architecture](https://docs.ceph.com/en/reef/architecture/).

## 3.3 HDFS Federation and Router-Based Federation

HDFS Federation is exceptionally relevant to IDS. Hadoop supports multiple independent NameNodes and namespaces. The namespaces do not need to coordinate with one another, while DataNodes can provide block storage for multiple block pools.

HDFS Router-based Federation adds a federation layer that makes subclusters accessible through a more transparent view. It routes requests to the appropriate subcluster, maintains namespace state, and provides a basis for data rebalancing.

**Overlap with IDS:** independent regional or logical data domains, independent namespace/control responsibilities, distributed physical storage, a federation layer, client transparency, and rebalancing.

**Difference IDS must make meaningful:** IDS is specified around independently managed regional database clusters with explicit logical data ownership, H-unit subclusters, policy-driven durability, and a cross-domain operation model. The proposed expansion goes further by allowing each domain to govern the resources it offers and the operations it accepts. A router over federated namespaces alone is not the IDS novelty claim.

Primary sources: [HDFS Federation](https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/Federation.html) and [HDFS Router-based Federation](https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs-rbf/HDFSRouterFederation.html).

## 3.4 iStore — Federation File Systems

The iStore research is a close precedent for geographically distributed storage federation. It describes a global namespace over geo-distributed edge servers, resource-aware data placement, job-aware placement decisions, data migration, and metadata optimizations.

**Overlap with IDS:** geographical federation, global logical access, resource-aware placement, migration, and data analysis across distributed sites.

**Difference IDS must make meaningful:** IDS needs an explicit model for autonomous administrative domains, data custody, operation authority, durability obligations, and multi-level responsibility transfer. iStore shows that “global namespace + geographically distributed servers + resource-aware placement” is already a known research direction.

Primary source: [IStore: Towards the Optimization of Federation File Systems, IEEE Access 2019 — Oak Ridge National Laboratory record](https://impact.ornl.gov/en/publications/istore-towards-the-optimization-of-federation-file-systems/).

## 3.5 Rucio — Distributed Scientific Data Management

Rucio manages data distributed across heterogeneous storage backends and data centres, often managed by different administrative teams. It provides unified data management, declarative replication rules, adaptive replication, quotas, and recovery. A user can express a requirement such as maintaining a particular number of copies across different geographical locations. When a replica disappears, the rule engine can create another replica to restore the declared policy.

**Overlap with IDS:** independent storage resources, heterogeneous infrastructure, declarative data-placement requirements, replica policy, automated recovery, quotas, and one operational interface.

**Difference IDS must make meaningful:** IDS is intended to coordinate the complete data-operation lifecycle, including write semantics, local ownership, operation ordering, compute execution, and recovery responsibility. Rucio is a particularly important warning against claiming that declarative placement and automatic replica restoration are novel on their own.

Primary sources: [What is Rucio?](https://rucio.github.io/documentation/started/what_is_rucio/) and [Replica Management with Replication Rules](https://rucio.github.io/documentation/started/concepts/replica_management/).

## 3.6 iRODS — Policy-Based Data Virtualization and Federated Zones

iRODS virtualizes data resources so that users can work with logical data objects independently of physical storage paths. It supports multiple replicas, metadata catalogs, event-triggered rule execution, and federation between independently administered zones.

**Overlap with IDS:** logical-to-physical location abstraction, metadata, policy-driven workflows, data virtualization, and federation across zones.

**Difference IDS must make meaningful:** IDS needs to define stronger infrastructure-level protocols for resource admission, placement commitments, consistency domains, and proof-backed data custody. The existence of federated zones and policy-driven actions means those ideas cannot be claimed as new in isolation.

Primary sources: [iRODS](https://irods.org/) and [iRODS Federation and Data Virtualization](https://www.irods.org/).

## 3.7 MinIO Multi-Site Replication

MinIO Site Replication configures multiple independent deployments as peer sites. Writes can be accepted at either site and replicated to the other sites. It supports deployment across racks, data centres, and geographic regions.

**Overlap with IDS:** independent storage deployments, peer sites, multi-site operation, and cross-site replication.

**Difference IDS must make meaningful:** IDS must go beyond a peer-replication arrangement by coordinating data ownership, heterogeneous resource capability, hierarchical placement, operation semantics, and recovery across independently controlled domains.

Primary source: [MinIO Site Replication](https://min.io/docs/minio/linux/operations/install-deploy-manage/multi-site-replication.html).

## 3.8 Apache Ozone

Apache Ozone separates namespace management from physical block-space management. Its architecture has an Ozone Manager, a Storage Container Manager, and DataNodes. Ozone write pipelines group DataNodes and support both Raft-based full replication and erasure coding. The Storage Container Manager chooses nodes based on availability, capacity, and network topology.

**Overlap with IDS:** a distinct metadata/control layer, node groups, write coordination, capacity-aware selection, replication, erasure coding, and recovery/reconstruction workflows.

**Difference IDS must make meaningful:** IDS's proposed H-units must operate as nested autonomous administrative and responsibility domains, not merely replication pipelines or storage containers. IDS also needs to federate multiple independently operated regional clusters rather than only scale one storage product.

Primary sources: [Apache Ozone Architecture](https://ozone.hadoop.apache.org/docs/core-concepts/architecture/overview/), [Ozone Write Pipelines](https://ozone.apache.org/docs/core-concepts/replication/write-pipelines/), and [Ozone Cluster Architectures](https://ozone.apache.org/docs/next/administrator-guide/configuration/cluster-architectures/).

## 3.9 Distributed SQL Systems: CockroachDB and YugabyteDB

CockroachDB partitions the keyspace into ranges, replicates ranges using Raft, and can split or merge ranges as workload and size change. YugabyteDB divides tables into tablets and replicates tablets through Raft groups. Its multi-region configurations support geo-partitioning, synchronous replication, asynchronous cross-cluster replication, and read replicas.

**Overlap with IDS:** independently replicated data partitions, local or regional placement, per-partition consensus groups, failure recovery, leader selection, and multiple geographic regions.

**Difference IDS must make meaningful:** dataset-scoped ordering is not inherently novel. Modern distributed databases already scope consensus and replication to ranges, tablets, or other shards. IDS should instead define a flexible consistency-domain graph in which ordering scope, data ownership, and physical placement are related but not permanently identical.

Primary sources: [CockroachDB Architecture](https://github.com/cockroachdb/cockroach/blob/master/docs/design.md), [YugabyteDB Architecture](https://docs.yugabyte.com/stable/architecture/), and [YugabyteDB Multi-Region Deployments](https://docs.yugabyte.com/stable/explore/multi-region-deployments/).

## 3.10 FoundationDB and Vitess

FoundationDB uses distinct process roles and a transaction architecture with coordinators, transaction logs, resolvers, proxies, and storage servers. It demonstrates how a system can separate responsibilities into specialised subsystems.

Vitess uses a topology service with global and cell-local topology information. The global topology stores shard and keyspace information; local topology stores cell-specific tablet and replication information.

**Overlap with IDS:** decoupled components, partitioned metadata, local/global control information, and independently scoped infrastructure responsibilities.

**Difference IDS must make meaningful:** IDS should make the boundary between global federation knowledge and locally authoritative domain knowledge explicit and enforce it through domain-to-domain protocols.

Primary sources: [FoundationDB Technical Overview](https://apple.github.io/foundationdb/technical-overview.html), [FoundationDB Architecture](https://apple.github.io/foundationdb/architecture.html), and [Vitess Topology Service](https://vitess.io/docs/24.0/concepts/topology-service/).

## 3.11 CrossFS — Cross-Domain Distributed File Systems (2026)

CrossFS, published in ACM Transactions on Storage in March 2026, is directly relevant because it addresses file-system operation across distributed administrative domains. It uses CRDT-based metadata synchronization, a hybrid-tree index, and adaptive caching/synchronization mechanisms.

**Overlap with IDS:** cross-domain storage, metadata synchronization, operation under network partitions, and a hybrid synchronization model.

**Difference IDS must make meaningful:** IDS should be concerned with a wider infrastructure control problem: who is permitted to host data, which domain assumes responsibility, how service obligations are decomposed across domains, how placement and compute are negotiated, and how recovery responsibilities are transferred. CrossFS is evidence that “cross-domain distributed storage” is already an active research area, not a blank space.

Primary source: [CrossFS: Improving Cross-Domain File System Performance with CRDT-Based Metadata Synchronization, ACM Transactions on Storage, published 28 March 2026](https://doi.org/10.1145/3777470).

## 3.12 SkyStore — Cross-Cloud Object Storage (VLDB 2025)

SkyStore addresses cost-optimized object storage across regions and cloud providers. It introduces a unified object/bucket API and dynamically replicates or evicts objects according to application access patterns and cloud cost differences.

**Overlap with IDS:** cross-cloud storage, unified API, dynamic placement, access-pattern-aware replication, cost-aware decisions, and availability.

**Difference IDS must make meaningful:** IDS should not merely choose a cheaper storage destination. It should coordinate autonomous provider domains, data-custody obligations, consistency scope, execution placement, and verified responsibility transfer under explicit owner policies.

Primary source: [SkyStore: Cost-Optimized Object Storage Across Regions and Clouds, IBM Research / VLDB 2025](https://research.ibm.com/publications/skystore-cost-optimized-object-storage-across-regions-and-clouds).

## 3.13 Patented Peer-to-Peer Global Metadata Federation

US patent publication US20220035769A1, later granted as US11947499B2, describes a peer-to-peer global namespace for storage metadata federations. It distributes metadata across geographically dispersed federation member nodes and continually reconciles the metadata without relying on one geographically assigned central metadata node.

**Overlap with IDS:** independent network domains, distributed metadata, global namespace, geo-distributed federation, and avoidance of a single central metadata point.

**Difference IDS must make meaningful:** this patent makes it especially important not to claim “a global metadata namespace across independent regions without a central authority” as a standalone invention. IDS must distinguish itself through its operational data-custody model, intent compilation, domain admission protocol, nested resource hierarchy, consistency scopes, and multi-level recovery process.

Primary sources: [US20220035769A1 — Google Patents](https://patents.google.com/patent/US20220035769A1) and [US11947499B2 — issued patent record](https://patents.justia.com/patent/11947499).

## 3.14 International Data Spaces and Data-Sovereignty Architectures

International Data Spaces (IDS-RAM) is a separate architecture family using the same acronym. It focuses on sovereign data exchange, federated data marketplaces, information models, and technically enforced data-usage policies.

**Overlap with the expanded IDS vision:** independent data owners, governed exchange, sovereignty, policy enforcement, and federation.

**Difference IDS must make meaningful:** IDS (Independent Distributed System) is intended to manage the physical and operational data infrastructure itself, not only trusted exchange of data assets. Nevertheless, the acronym collision is significant in research and public communication. Use the full name on first reference and distinguish the project from International Data Spaces Association / IDS-RAM.

Primary source: [International Data Spaces Reference Architecture](https://internationaldataspaces.org/offers/reference-architecture/).

## 3.15 Additional Research: Edge-Cloud Placement and Distributed Repair

Recent work on the cloud-edge continuum jointly optimizes function placement and data replication, showing that compute placement and data placement should be solved together rather than independently. Research on distributed erasure coding continues to produce new repair and storage-efficiency schemes.

**Overlap with expanded IDS:** data/compute co-placement, topology-aware scheduling, redundancy selection, and repair optimisation.

**Implication:** compute-aware data placement and efficient repair are valuable directions, but neither is new in isolation. IDS's opportunity is to integrate them into a sovereign, multi-domain operating protocol.

Sources: [Replication-Aware Placement of Functions and Data in the Edge-Cloud Continuum, September 2026](https://arxiv.org/abs/2609.18932) and [ZJC: Constructing Fully Local Repair in Erasure Codes for Distributed Cloud Storage, 2026](https://www.sciencedirect.com/science/article/pii/S1383762126000536).

---

# 4. NOVELTY MATRIX — WHAT IDS CAN AND CANNOT CLAIM

| IDS feature | Closest established precedents | Novelty assessment | What IDS must add to create a defensible distinction |
|---|---|---|---|
| One logical interface over distributed storage | HDFS Router Federation, iStore, Alluxio, Rucio, iRODS | Established | Make the interface an intent and service-contract interface, not merely a namespace or proxy |
| Multiple independent regional clusters | HDFS Federation, MinIO peer sites, iRODS zones, cross-domain storage systems | Established | Formalize autonomous administrative authority and domain admission |
| Hierarchical node grouping through H-units | Ceph CRUSH buckets, HDFS topology awareness, Ozone pipelines | Established as a structural pattern | Make H-units local operational domains with policies, capacity commitments, responsibility and repair autonomy |
| Hybrid replication and erasure coding | Ceph, Apache Ozone, many object stores | Established | Integrate selection and repair into a global policy/obligation model across independently governed domains |
| Cross-H-unit failure compensation | Ceph recovery, Ozone pipeline recovery, Rucio replica restoration | Established in principle | Make recovery a proof-checked cross-domain transfer of responsibility across multiple failure levels |
| Dataset-scoped write ordering | Sharded databases, Raft/Paxos replica groups, partitioned logs | Established | Define dynamic consistency scopes and cross-scope causality without an unnecessary global serialization layer |
| No permanent global master | Ceph CRUSH, peer-to-peer metadata federation patent, distributed consensus systems | Established | Define the exact division of global discovery, local authority, and inter-domain agreement |
| Nine-character operation references | UUIDs, ULIDs, random IDs, sequence IDs, transaction IDs | Not novel | Treat it as an implementation format only; establish collision and lifetime guarantees separately |
| Workload-aware placement | iStore, SkyStore, Rucio, Cassandra, cloud-edge scheduling research | Established | Optimize across owner policy, durability, compute locality, bandwidth, cost, energy, and domain admission together |
| Cross-domain metadata synchronization | CrossFS, patent US11947499B2, iRODS, MinIO | Established | Link metadata synchronization to custody, service obligations, and operational authority |
| Federated data sovereignty | IDS-RAM, data spaces, iRODS zones | Established as a domain | Couple sovereignty constraints to actual physical placement, execution, mutation and recovery enforcement |
| Integrated autonomous domain operating protocol | Individual portions exist across the systems above | Strongest IDS novelty target | Specify the intent language, domain-agent protocol, custody graph, commitment proofs, and multi-level re-planning algorithm |

## 4.1 The novelty conclusion

The baseline IDS document is an architectural synthesis of established distributed-system patterns. That is not a criticism; it is the accurate prior-art result.

The stronger IDS contribution is the expanded operating model defined in the following sections. The central invention target is not the presence of regional clusters, H-units, replication, or dataset ordering. It is the **protocol by which independent domains accept, execute, prove, and transfer obligations for data services while preserving local control**.

That is the proposition to formalize, implement, and benchmark.

---

# 5. EXPANDED VISION: IDS AS A FEDERATED DATA-INFRASTRUCTURE OPERATING SYSTEM

The original IDS proposal is framed primarily as a distributed data-operation architecture. The expanded vision is much larger.

IDS can become an operating layer for data across a federation of independently operated infrastructure—not only one organisation's database servers, but potentially multiple clouds, private data centres, edge sites, research clusters, institutional storage systems, enterprise environments, and specialised infrastructure providers.

The system should be capable of coordinating more than data location. It should coordinate the complete service lifecycle around data:

- Where the authoritative data is held.
- Which domains hold replicas or encoded fragments.
- Which infrastructure is permitted to process the data.
- Where a query or computation should execute.
- What consistency the operation requires.
- What durability must be satisfied before acknowledgement.
- Which party is responsible for a particular copy or fragment.
- What happens when a node, H-unit, region, provider, or network path fails.
- How the system restores the declared service after the topology changes.
- How the data owner retains authority while still using external infrastructure.

This shifts IDS from “a system that knows where the data is” toward “a system that can continuously assemble and maintain a data service from independently governed resources.”

## 5.1 The fundamental abstraction: Data Service Intent

A client or data owner should not have to specify physical server names, replica hostnames, or a fixed list of nodes.

Instead, the owner declares the service it needs.

Illustrative intent:

```yaml
service: research-dataset

identity:
  dataset: DS-RESEARCH-001
  owner: institution-A

placement:
  allowed_regions: [IN-SOUTH, SG, EU-WEST]
  prohibited_domains: [provider-Z]
  minimum_independent_domains: 3
  preferred_locality: IN-SOUTH

 durability:
  mode: hybrid
  minimum_recoverable_copies: 2
  allow_erasure_coding: true
  verify_after_repair: true

consistency:
  write_scope: partition
  read_policy: committed-prefix
  cross_partition_transactions: explicit-only

performance:
  read_latency_target_ms: 35
  write_ack_policy: policy-defined

operations:
  retention: owner-controlled
  audit: required
  recovery: automatic-when-authorized
```

This is only a conceptual representation. IDS must define a real schema, validation semantics, versioning, and deterministic policy evaluation.

The key shift is from static infrastructure configuration to declarative service requirements.

## 5.2 The Intent-to-Infrastructure Compiler

The Intent Compiler turns a Data Service Intent into executable infrastructure plans.

It performs the following transformations:

1. Parse the required service semantics.
2. Resolve the dataset owner and permitted operations.
3. Discover eligible infrastructure domains.
4. Request resource capability offers from those domains.
5. Reject offers that violate residency, security, trust, or ownership constraints.
6. Calculate storage, compute, bandwidth, and redundancy requirements.
7. Build a feasible placement and responsibility graph.
8. Assign local work to the participating domain agents.
9. Establish the required operation and durability commitments.
10. Monitor whether the realised infrastructure continues to satisfy the declared intent.

The compiler should not make an unsafe assumption that every domain is equally trusted or that every discovered node is available for every dataset.

A domain remains free to decline a proposed placement or resource request. IDS must then find another valid arrangement or report which requirement cannot currently be satisfied.

## 5.3 Independent Domain Agents

Each participating administrative domain should operate a local IDS Domain Agent.

The Domain Agent is the domain's interface to the wider federation. It does not surrender local control to a global IDS process.

The agent is responsible for:

- Publishing a signed description of available capabilities.
- Declaring the local domain's identity and trust attributes.
- Enforcing local admission and access policy.
- Reporting capacity and resource health.
- Accepting or declining placement proposals.
- Reserving local resources for accepted obligations.
- Executing approved storage or compute operations.
- Reporting operation receipts and durability evidence.
- Participating in recovery where its local policy permits.
- Revoking or modifying offers when local conditions change.

This establishes a clear control boundary:

```text
                    IDS FEDERATION
                          |
             +------------+------------+
             |            |            |
          DOMAIN A     DOMAIN B     DOMAIN C
             |            |            |
         DOMAIN AGENT DOMAIN AGENT DOMAIN AGENT
             |            |            |
         LOCAL POLICY LOCAL POLICY LOCAL POLICY
             |            |            |
         LOCAL NODES  LOCAL NODES  LOCAL NODES
```

The federation coordinates commitments. The local domain controls its own resources.

## 5.4 Capability Offers Instead of Unrestricted Node Discovery

A domain should advertise capabilities, not expose every internal machine and operational detail.

A capability offer could include:

- Available storage quantity and storage class.
- Supported durability mechanisms.
- Supported read/write semantics.
- Available compute classes.
- Network reachability and bandwidth envelope.
- Latency estimates.
- Geographic and jurisdictional attributes.
- Supported cryptographic and security properties.
- Capacity reservation limits.
- Failure-domain identifiers.
- Operational service history.
- Pricing or internal resource cost, where applicable.
- The domain's local policy and admission requirements.

The domain can expose only the information required for selection. Sensitive internal topology can remain private.

This avoids requiring one global controller to own or fully inspect every participating machine.

---

# 6. THE CENTRAL NOVELTY TARGET: A DATA CUSTODY AND RESPONSIBILITY GRAPH

This is the most important architectural expansion proposed in this document.

Traditional placement metadata answers questions such as “Which nodes contain this block?” IDS should represent a richer set of relationships.

For every data object, dataset, or partition, IDS should be able to determine:

- Who is the data owner?
- Which domain is authoritative for mutations?
- Which domains hold copies?
- Which nodes hold encoded fragments?
- Which resources are currently responsible for meeting durability requirements?
- Which domain is responsible for repair if a particular copy or fragment disappears?
- Which policy permits a domain to read, transform, replicate, export, or delete the data?
- Which operation history and consistency scope apply?
- What evidence proves that a copy, fragment, or repair completed correctly?

These relationships form a **Data Custody and Responsibility Graph (DCRG)**.

## 6.1 Graph entities

The graph contains at least the following entity classes:

| Entity | Meaning |
|---|---|
| Data Owner | The principal with defined authority over the logical data asset |
| Dataset / Object / Partition | Logical data identity independent of physical location |
| Authority Domain | The domain authorised to accept or commit relevant mutations |
| Administrative Domain | The independently governed infrastructure operator |
| Region | A geographical and jurisdictional placement boundary |
| H-Unit | A locally managed intermediate infrastructure group |
| Node / Storage Resource | The physical resource holding data or executing an operation |
| Replica | A complete data copy |
| Fragment | An encoded portion of data used for reconstruction |
| Compute Resource | A node or service allowed to process data |
| Network Path | A communication resource with properties and failure conditions |
| Obligation | A required property such as minimum durability or retention |
| Receipt / Proof | Evidence that a placement, write, repair, transfer, or deletion occurred |

## 6.2 Four graphs, one data-service identity

The implementation should distinguish four related graphs rather than flattening everything into one location table.

**Ownership graph:** establishes who owns data, who may mutate it, and who may grant access.

**Placement graph:** establishes where replicas, fragments, caches, and derived data physically reside.

**Responsibility graph:** establishes which domain is obligated to preserve, verify, repair, or return each data representation.

**Execution graph:** establishes where queries, transformations, indexing, training, analytics, or other compute operations may run.

All four graphs reference the same stable logical data identity.

This separation is important. A data owner is not necessarily the physical storage provider. The authority to mutate is not the same as the ability to read. A replica is not automatically an authoritative copy. A compute domain may process a dataset without becoming its owner.

## 6.3 Responsibility transfer

When a storage node fails, the question is not only “Where do we copy the data?” It is also “Which eligible domain now assumes the obligation to satisfy the missing durability or availability requirement?”

A transfer should have an explicit lifecycle:

1. Identify the failed resource and the obligations affected.
2. Determine which surviving representations are valid.
3. Evaluate the data owner's policy and all domain constraints.
4. Select an eligible replacement domain and resource.
5. Obtain local acceptance and reserve capacity.
6. Copy or reconstruct the data.
7. Verify content integrity and required metadata.
8. Produce a signed completion receipt.
9. Update the custody and responsibility graphs.
10. Release the old obligation only after the new one is proven satisfied.

The system should never mark a responsibility transfer complete merely because a replacement machine has been selected.

## 6.4 Why this graph is central

The original IDS specification already has node metadata, H-unit metadata, dataset metadata, operation metadata, and ordering-domain metadata. The DCRG unifies the semantics of those records and extends them across administrative domains.

It enables IDS to reason about the complete lifecycle of a data service rather than treating placement, replication, ownership, execution, and recovery as unrelated subsystems.

This is the core data model around which the expanded IDS vision should be designed.

---

# 7. H-UNITS EVOLVED: FROM GROUPED NODES TO AUTONOMOUS RESOURCE CELLS

The H-unit layer should be retained, but its role should become more substantial.

An H-unit should be defined as a **bounded operational resource cell** inside a regional domain. It groups nodes and maintains a local view of capacity, health, data responsibility, and eligible compensation paths.

A region can contain many H-units. A future design can also permit additional intermediate levels when the infrastructure becomes larger or more heterogeneous.

```text
GLOBAL IDS FEDERATION
        |
        +-- REGION / DOMAIN A
        |       |
        |       +-- H-UNIT A1
        |       |      +-- Storage nodes
        |       |      +-- Compute nodes
        |       |      +-- Cache resources
        |       |
        |       +-- H-UNIT A2
        |       |      +-- Storage nodes
        |       |      +-- Compute nodes
        |       |
        |       +-- H-UNIT A3
        |
        +-- REGION / DOMAIN B
                |
                +-- H-UNIT B1
                +-- H-UNIT B2
                +-- H-UNIT B3
```

## 7.1 H-unit responsibilities

An H-unit should have its own:

- Membership record.
- Capacity ledger.
- Health state.
- Resource capability description.
- Data placement map.
- Local redundancy state.
- Failure and recovery state.
- Admission policy.
- Local placement decision process.
- Compensation relationships with other H-units.
- Resource reservation mechanism.

## 7.2 Recursive hierarchy

The architecture should not assume that only three fixed levels will ever exist.

A large domain might require:

`Federation → Operator → Region → Availability Zone → H-Unit → Rack Group → Node → Device`.

A small deployment might only need:

`Federation → Domain → H-Unit → Node`.

The logical model should support recursive failure and responsibility boundaries without making every level mandatory.

## 7.3 H-unit autonomy

An H-unit should be able to optimise its own internal placement while respecting regional and global obligations. It should report the result to its parent domain and request external capacity only when local policy or local capacity requires it.

This gives IDS a hierarchical control model:

- Local decisions stay local where possible.
- Regional decisions coordinate H-units.
- Federation-level decisions coordinate domains.
- Global coordination occurs only when a service spans multiple domains.

This is stronger than a static tree that only labels the physical topology.

---

# 8. CONSISTENCY AND ORDERING: FROM DATASET QUEUES TO A CAUSALITY GRAPH

The existing IDS distinction between a nine-character operation reference and an ordering position should remain. The proposed expansion changes how ordering scope is represented.

The system should not force every operation into one global sequence. It should also not assume that every dataset needs one indivisible ordering domain.

Instead, IDS should represent consistency requirements at the narrowest scope that preserves the required semantics.

Possible scopes:

- Object or record.
- Partition.
- Dataset.
- H-unit-local metadata.
- Regional metadata.
- Multi-dataset transaction.
- Cross-domain transaction.

## 8.1 Causality-scoped operation lanes

An operation has a stable identity, but it can participate in one or more ordering lanes.

A lane is a consistency domain that establishes an authoritative order for a defined set of conflicting operations.

Two unrelated datasets can progress without waiting for each other. Two partitions can progress independently when no cross-partition invariant exists. A multi-dataset transaction can explicitly create a coordination edge across the lanes it touches.

```text
DATASET A / PARTITION 1:  A1 → A2 → A3
DATASET A / PARTITION 2:  B1 → B2 → B3
DATASET B:               C1 → C2 → C3

Cross-partition transaction:
                 T1
              /      \
      Partition 1   Partition 2
```

## 8.2 Dependency graph instead of global total order

The operation history can be represented as a directed acyclic dependency graph where edges encode required ordering or causality.

An edge means that operation Y depends on operation X being committed, observed, or otherwise resolved. Operations without a dependency edge do not need a system-wide ordering relationship.

The system must still prevent cycles, preserve atomicity where required, and define what readers are allowed to observe.

## 8.3 Elastic consistency domains

A future IDS implementation can investigate splitting or merging ordering lanes based on workload and dependency changes.

For example:

- A dataset with low write contention may be divided into independent partition lanes.
- A data object with high conflict rates may use a narrower, strongly ordered scope.
- A workload that begins to require cross-record invariants may temporarily coordinate a set of lanes.
- When that dependency disappears, the system can return to independent operation.

The split/merge operation must transfer the history, membership, and commit frontier without permitting two conflicting authorities to commit simultaneously.

## 8.4 Operation references are not an invention claim

The nine-character alphanumeric identifier has 36⁹ = 101,559,956,668,416 possible values. That namespace size is a format choice, not a uniqueness proof and not an architectural novelty.

The identifier must be separate from:

- Ordering position.
- Causal dependencies.
- Commit certificate.
- Durability evidence.
- Retry/idempotency identity.

IDS should define collision management and operation lifetime as engineering properties. Do not market the nine-character format as a novel distributed-systems mechanism.

---

# 9. DURABILITY AS A SERVICE OBLIGATION

Current IDS supports policy-selected replication and erasure coding. The expansion should treat durability as an explicit obligation attached to a data service.

An obligation could state:

- At least N independently administered domains must hold recoverable representations.
- At least M failure domains must remain independent.
- One copy must remain in a named jurisdiction.
- A write is acknowledged only after a defined number of durable fragments exist.
- A repair must complete within a defined recovery objective.
- A copy must be periodically integrity-checked.
- A deletion must propagate to all representations where legally and operationally required.

## 9.1 Adaptive redundancy

IDS can select between full replication, erasure coding, local parity, remote parity, caching, archival copies, or a combination according to policy and data characteristics.

The decision engine may consider:

- Object size.
- Mutation frequency.
- Read frequency.
- Rebuild bandwidth.
- Recovery time.
- Failure correlation.
- Storage cost.
- Energy cost.
- Network transfer cost.
- Access latency.
- Region and jurisdiction requirements.
- Domain trust and availability history.

## 9.2 Durability proof

A service should have a verifiable answer to the question: “Does the current placement still satisfy the declared durability obligation?”

The answer must be based on current state, not merely on a configured replication factor.

A durability proof can be a verifiable record that the required number of valid replicas or fragments exist across the required independent failure domains and that the storage agents have acknowledged their current responsibility.

The formal proof model is an IDS research task. It should account for stale copies, failed domains, correlated failures, incomplete repairs, and metadata loss.

## 9.3 Durability is not synonymous with replica count

Three copies on three nodes in one rack do not give the same failure protection as three copies across three independently powered and administered locations. IDS should measure failure-domain diversity and administrative independence, not only count storage copies.

---

# 10. DATA AND COMPUTE SHOULD BE PLACED TOGETHER

The larger IDS vision should extend beyond storage routing.

When a query or computation requires a large dataset, moving all the data to one central compute cluster is often unnecessary. IDS should be able to send an approved computation toward the data, execute partial work inside the data's permitted domain, and return only the result or approved intermediate data.

This can apply to:

- Distributed queries.
- Large-scale analytics.
- Scientific workloads.
- Machine-learning preprocessing.
- Model training over distributed datasets.
- Index building.
- Data transformation.
- Backup and integrity verification.
- Search and retrieval workloads.

## 10.1 Execution placement

The Execution Planner should evaluate:

- Where the input data resides.
- Which domains are allowed to process it.
- Available compute resources.
- Network transfer cost.
- Compute-to-data latency.
- Result size compared with input size.
- Security and jurisdiction policies.
- Whether partial execution is possible.
- Whether the operation is deterministic and restartable.

## 10.2 Compute-to-data execution

A domain can receive an executable task or constrained query plan instead of exporting an entire dataset. The domain executes the permitted workload locally and returns a result, summary, or approved data product.

This is particularly powerful when the data owner permits computation but does not permit raw data export.

## 10.3 Data, compute, and network as one placement problem

IDS should jointly optimise three connected placements:

1. Data placement.
2. Compute placement.
3. Network path selection.

Moving data closer to a compute resource can be worse than moving a lightweight computation closer to the data. The optimal decision depends on size, latency, cost, confidentiality, and workload semantics.

---

# 11. SOVEREIGNTY, TRUST, AND MULTI-OWNER OPERATION

The independent-domain model becomes meaningful only when ownership and control are technically enforceable.

The expanded IDS should support infrastructure operated by different organisations without requiring every operator to surrender its local policies to one central administrator.

## 11.1 Separate identities

The system should distinguish:

- Data owner identity.
- User/client identity.
- Administrative-domain identity.
- H-unit identity.
- Node identity.
- Execution identity.
- Operation identity.
- Resource-offer identity.

These identities have different privileges and lifecycles.

## 11.2 Policy dimensions

Data policies should express:

- Who may read.
- Who may write.
- Who may execute a query.
- Which transformations are allowed.
- Which domains may store data.
- Which jurisdictions are permitted.
- Which domains may hold replicas.
- Whether encrypted storage is mandatory.
- Whether key custody must remain with the owner.
- How long data must be retained.
- How deletion is verified.
- What audit records must be preserved.

## 11.3 Local admission authority

A global placement plan is a proposal, not a command that automatically overrides a domain's local policy.

The domain agent checks the request against local rules and accepts, rejects, or counteroffers resource availability.

This produces a federation in which local autonomy is a protocol property, not simply a statement in a project description.

## 11.4 Trust should be measurable

Trust is not a boolean field. The architecture should track verifiable properties such as identity validity, hardware attestation where available, policy compliance, historical reliability, integrity-check success, incident status, and audit completeness.

Any trust score should be explainable and should not replace hard policy constraints.

---

# 12. THE GLOBAL CONTROL PLANE SHOULD BE FEDERATED, NOT OMNIPOTENT

IDS should distinguish between a global view and global authority.

The federation needs enough shared knowledge to discover domains, map logical data identities, evaluate placement, and coordinate cross-domain obligations. It does not need unrestricted control over every local resource.

## 12.1 Three classes of state

**Global federation state:** domain identities, public capability offers, logical data identities, cross-domain ownership references, and accepted service obligations.

**Regional/domain state:** local topology, H-unit membership, local capacity, local resource admission, local placement, local health, and local policy decisions.

**Data-operation state:** operation references, ordering-lane positions, causal dependencies, commit states, durability proofs, synchronization states, and receipts.

These state classes have distinct update rates, authority sources, and consistency requirements.

## 12.2 No global hot path by default

A normal read from a local domain should not require a query to a distant global metadata service for every operation. Clients and domain agents should use securely cached or locally replicated metadata where safe.

Global coordination should be required when a data operation actually crosses a domain boundary or changes a global obligation.

## 12.3 Global failure should not imply universal failure

If the global discovery or planning component becomes unavailable, domains should continue permitted local operations from valid local state. Cross-domain operations may require a fresh agreement or a cached commitment according to policy.

This is an explicit availability and safety design, not an assumption that all partitions can always accept every write.

---

# 13. AUTONOMOUS RECOVERY: FROM NODE REPAIR TO INFRASTRUCTURE RECONSTITUTION

The original IDS failure model covers node, H-unit, and regional failures. The expanded model should include the failure of an entire operator domain, the loss of trust in a provider, long-term network partition, policy revocation, and loss of a resource class.

## 13.1 Recovery levels

| Failure level | Example | IDS response |
|---|---|---|
| Device | Disk or storage medium fails | Reconstruct or replace affected data representation |
| Node | Storage node permanently fails | Transfer responsibility to eligible nodes or H-units |
| H-unit | Entire subcluster is unavailable | Rebalance across surviving H-units or domains |
| Region | Regional cluster is lost | Reconstruct from cross-regional placements and restore region-level obligations |
| Operator domain | Provider or organisation exits federation | Move data and responsibilities to other accepted domains |
| Network partition | Domains cannot communicate | Continue only operations allowed by local consistency and durability policy |
| Trust revocation | Domain no longer satisfies security requirements | Freeze new placement, revoke access, migrate or re-protect data |
| Policy change | Data owner changes residency or retention rules | Replan placement, execution and deletion obligations |

## 13.2 Recovery as a graph-repair problem

The recovery planner evaluates the custody and responsibility graph, identifies broken obligations, and finds a new valid graph configuration.

Its goal is not simply to recreate an identical topology. It must restore the required service properties while minimising:

- Data movement.
- Recovery time.
- Cross-domain bandwidth.
- Storage overhead.
- Correlated failure risk.
- Operational disruption.
- Policy violations.

A new placement may be structurally different from the old placement while still satisfying the owner's obligations.

## 13.3 Repair with proof

Every recovery path should end with verification of the data, metadata, operation history, and durability requirements. The recovery service then issues receipts and updates the responsibility graph.

The system should maintain evidence of:

- Which source fragments or replicas were used.
- Which algorithm reconstructed the data.
- Which destination accepted responsibility.
- Which integrity checks passed.
- Which metadata/version state was restored.
- Which previous obligations were retired.

This turns recovery into a traceable protocol rather than an opaque background copy process.

---

# 14. GLOBAL RESOURCE FABRIC: STORAGE IS ONLY THE FIRST RESOURCE

A genuinely large IDS vision should not assume that all available infrastructure is storage-only.

The federation can represent resources in a common capability model:

- Persistent storage.
- Fast local storage.
- Memory and cache capacity.
- Archival and cold storage.
- Compute capacity.
- Accelerators.
- Network bandwidth and paths.
- Data indexing and search services.
- Replication and reconstruction services.
- Integrity verification services.

Each resource belongs to an administrative domain and carries constraints describing who may use it and for what purpose.

## 14.1 Storage media and tiers

IDS should support storage backends through pluggable resource adapters. The architecture should not be permanently coupled to SSDs, HDDs, or one object-store API.

Potential backends include:

- RAM and memory pools.
- NVMe and SSD storage.
- HDD arrays.
- Object storage.
- Distributed block stores.
- File systems.
- Tape and archival systems.
- Remote research storage.
- Future specialised storage media, provided they expose verifiable read/write/retention semantics.

## 14.2 A logical data asset can span multiple tiers

A data asset may have a frequently accessed local representation, a durable regional replica, erasure-coded long-term representations, and archival copies. IDS can maintain one logical identity while managing these physical representations under different policies.

## 14.3 Resource adapters

A resource adapter translates IDS operations into the native operations of a storage or compute backend. The adapter must expose the backend's real capabilities and limitations; it must not pretend that all backends offer identical semantics.

This makes IDS extensible without turning it into a wrapper that assumes every system is equivalent.

---

# 15. POTENTIAL CAPABILITIES THAT FOLLOW FROM THE EXPANDED ARCHITECTURE

The following capabilities become possible when the intent compiler, domain agents, custody graph, and resource fabric are implemented together.

## 15.1 Policy-defined global data services

A client requests a service outcome, not a machine address. IDS selects an eligible infrastructure arrangement and continuously checks it against the declared contract.

## 15.2 Cross-provider data continuity

A service can survive the loss or withdrawal of one operator by moving its data responsibility to other eligible domains.

## 15.3 Data sovereignty with operational federation

Data can be physically distributed and processed across multiple organisations while each owner retains explicit authority over access, mutation, export, retention, and deletion.

## 15.4 Automatic data/compute co-placement

IDS can send queries and permitted computations to the domains where the data resides, limiting unnecessary raw-data movement.

## 15.5 Dynamic consistency topology

Independent operations proceed independently. Conflicting operations coordinate at the narrowest scope necessary. Ordering lanes can be split or merged through a controlled protocol.

## 15.6 Durability as a continuously verified property

IDS can detect when real infrastructure no longer satisfies a service's durability requirements and reconfigure placement to restore them.

## 15.7 Resource-aware geographic expansion

New infrastructure can join the federation by advertising capabilities and accepting local obligations, rather than requiring a full redesign of the global architecture.

## 15.8 Cost-, latency-, and energy-aware placement

The scheduler can balance physical storage price, data transfer cost, compute cost, latency, energy usage, and owner policy.

## 15.9 Data migration without logical identity changes

Data can move between nodes, H-units, regions, and operators while logical references remain stable.

## 15.10 Federated recovery after provider exit

A domain can be removed from the federation and its assigned data services redistributed, provided the remaining domains can satisfy the required policies.

## 15.11 Execution without unrestricted data export

A domain can run approved code or queries locally and return permitted results rather than exporting the complete source dataset.

## 15.12 One data plane across heterogeneous infrastructure

A single logical service can span databases, distributed file systems, object stores, block storage, caches, and archives through explicit adapters and semantics.

## 15.13 Capacity contribution by independent operators

Operators can contribute available resources to selected data services while retaining local control of admission, quotas, policies, and withdrawal conditions.

## 15.14 Auditable lifecycle

Every write, placement, replication, repair, migration, execution, permission change, and deletion can be tied to an operation reference and a sequence of verifiable receipts.

## 15.15 Federated scientific and industrial data environments

Large research collaborations, universities, labs, healthcare networks, industrial groups, and geographically distributed enterprises could use IDS to coordinate data access and computation across separately managed institutions.

---

# 16. WHAT IDS COULD BECOME AT FULL SCALE

The most expansive interpretation of IDS is not “a database that has many nodes.”

It is a **federated operating system for distributed data infrastructure**.

At full scale, its federation could include:

- Independent cloud providers.
- Private data centres.
- Regional enterprise clusters.
- University and research infrastructure.
- Edge computing sites.
- Industrial systems.
- Scientific storage facilities.
- Archival infrastructure.
- Specialised compute and accelerator pools.
- New storage technologies introduced through resource adapters.

A data owner could define a service and its rules. IDS would discover capable infrastructure, obtain local domain acceptance, place data, schedule compute, enforce consistency semantics, maintain durability, and continuously repair the service when resources or domains disappear.

The owner would not need to operate every server. The providers would not need to surrender their infrastructure. IDS would coordinate the data service across them.

The essential architectural idea is:

**One logical data service, many autonomous infrastructure owners, explicit responsibility for every copy and operation.**

This is the scale of vision worth pursuing.

---

# 17. THE RESEARCH CONTRIBUTIONS IDS SHOULD FORMALISE

The following are the primary technical contributions to pursue. Each needs a formal model, a working implementation, and experiments against appropriate baselines.

## Contribution A — Autonomous Domain Participation Protocol

Define how an independent operator registers, publishes capabilities, accepts or declines obligations, reserves resources, executes approved actions, and withdraws from the federation safely.

Required artefacts:

- Domain identity and enrollment protocol.
- Capability schema.
- Resource-offer lifecycle.
- Admission and reservation protocol.
- Domain policy evaluation.
- Revocation and withdrawal procedure.
- Failure and rejoin protocol.

## Contribution B — Data Custody and Responsibility Graph

Define the graph model that binds ownership, authoritative mutation rights, physical placement, replica/fragment structure, compute permissions, durability obligations, and repair responsibility.

Required artefacts:

- Formal graph schema.
- Versioning and update rules.
- Consistency requirements for graph state.
- Custody-transfer protocol.
- Proof and receipt format.
- Recovery reconciliation algorithm.

## Contribution C — Intent-to-Infrastructure Compiler

Define a declarative service language and a deterministic process that compiles service requirements into admissible placements and execution plans.

Required artefacts:

- Intent syntax and schema.
- Policy semantics.
- Capability matching.
- Constraint solver or planning algorithm.
- Infeasibility explanation.
- Versioned plan representation.
- Plan lifecycle and rollback.

## Contribution D — Causality-Scoped Consistency Model

Define how operations are assigned to ordering lanes and how cross-lane dependencies are created, committed, observed, and retired.

Required artefacts:

- Ordering-domain formal model.
- Causal dependency representation.
- Commit frontier semantics.
- Cross-lane transaction procedure.
- Dynamic lane split/merge protocol.
- Reconfiguration safety proof.

## Contribution E — Multi-Level Responsibility Recovery

Define how IDS restores a set of data-service obligations after failures across nodes, H-units, regions, and entire operators.

Required artefacts:

- Failure-domain graph.
- Failure classification.
- Source selection and reconstruction planner.
- Destination eligibility and reservation.
- Integrity verification protocol.
- Responsibility reassignment proof.
- Recovery convergence properties.

## Contribution F — Joint Data/Compute/Network Placement

Define how the system selects both data placement and execution placement, taking into account data movement, compute capacity, latency, cost, policy, and failure risk.

Required artefacts:

- Unified resource model.
- Workload description.
- Placement objective function.
- Constraint and policy handling.
- Reconfiguration cost model.
- Benchmark suite.

---

# 18. THE EXPERIMENTS THAT WOULD ESTABLISH THE CONTRIBUTION

The novelty argument should be supported by measurable advantages, not by architectural diagrams alone.

| Experiment | Comparison | What it establishes |
|---|---|---|
| Multi-owner federation enrollment | Static central configuration vs domain-agent enrollment | Whether independent operators can join without surrendering local admission authority |
| Capability negotiation | Fixed placement vs capability-offer selection | Whether heterogeneous domain resources can be selected under explicit constraints |
| Custody transfer | Manual reconfiguration vs graph-driven transfer | Whether data responsibility can be reassigned safely and audibly |
| Multi-level failure | Single-node failure tests vs node/H-unit/region/operator failure tests | Whether one recovery model works across nested failure domains |
| Policy satisfaction | Best-effort replica count vs verified service obligations | Whether data residency, durability and trust constraints remain satisfied after reconfiguration |
| Intent compilation | Hand-authored placement vs intent-generated plan | Whether high-level service requirements can be compiled into valid executable plans |
| Cross-domain write | One global ordering path vs scoped ordering lanes | Whether unrelated domains avoid unnecessary coordination without violating consistency |
| Cross-lane transaction | No dependency coordination vs explicit dependency graph | Whether only genuinely related operations incur broader coordination |
| Joint placement | Storage-only placement vs joint data/compute/network placement | Whether moving compute to data reduces transfer, latency, or cost |
| Provider withdrawal | Static replicas vs domain-exit recovery | Whether services survive an operator leaving the federation |
| Metadata partition | Global metadata service vs local/global split | Whether local operations continue during global-control-plane interruption |
| Recovery verification | Copy-complete status vs content and obligation proof | Whether service restoration is based on verified correctness |

## 18.1 Required metrics

Measure at least:

- Read and write latency, including p50, p95, and p99.
- Throughput under varying numbers of domains and nodes.
- Cross-domain bytes transferred.
- Recovery completion time.
- Time to restore required durability.
- Storage amplification.
- Repair bandwidth.
- Control-plane traffic.
- Metadata convergence time.
- Intent-compilation time.
- Placement-plan churn.
- Resource utilisation.
- Cost per stored and served byte.
- Energy per stored and served byte where measurable.
- Number of domains required to coordinate each operation.
- Availability of unrelated local services during a domain failure.

## 18.2 Critical correctness properties

Tests must establish:

1. No domain stores or processes data without the applicable authority.
2. No accepted placement silently violates residency or trust constraints.
3. No data operation is reported committed before its declared commit conditions are met.
4. No responsibility is released before its replacement obligation is proven satisfied.
5. No stale replica is treated as authoritative without the required version and consistency checks.
6. No cross-domain recovery creates two conflicting authoritative owners.
7. A failed global component does not cause unrelated local operations to fail unless their policy requires it.
8. Cross-scope transactions preserve their stated atomicity and visibility properties.
9. Every completed repair is verified against the source data or a valid reconstruction proof.
10. Reconfiguration preserves stable logical data identity.

---

# 19. WHAT MUST NOT BE PRESENTED AS NOVEL

For clarity, the following items should not be marketed as original IDS inventions on their own:

- Multiple regions or data centres.
- Multiple independent databases.
- A global namespace.
- Sharding or partitioning.
- A hierarchical topology.
- H-unit-like grouping.
- Rack- or region-aware placement.
- Full replication.
- Erasure coding.
- Hybrid replication and erasure coding.
- Automated replica repair.
- Data rebalancing after node addition.
- Workload-aware placement.
- Locality-aware reads.
- Distributed query execution.
- Dataset- or partition-scoped consensus.
- The absence of a permanent global master.
- Unique operation identifiers.
- A nine-character alphanumeric reference format.
- Metadata synchronisation across sites.
- Federated access across administrative domains.
- Data sovereignty or usage policies.
- Moving compute toward data.

These are established ideas. Their presence is useful, but the novelty claim must concern the precise IDS mechanism that connects them.

---

# 20. FINAL NOVELTY STATEMENT

IDS is a proposed federated data-infrastructure operating architecture.

Its foundational architecture connects independently managed regional database clusters, hierarchical storage resources, and unified client operations. Its expanded architecture introduces autonomous administrative domains that advertise capabilities and retain local admission authority while participating in a common logical data-service federation.

The central IDS research contribution is a protocol model that converts data-service intent into accepted infrastructure commitments and maintains those commitments through explicit ownership, placement, custody, consistency, execution, durability, and recovery relationships.

A Data Custody and Responsibility Graph records where data representations reside, who controls them, which domains are responsible for preserving them, what operations are permitted, and what evidence proves that the required service properties are satisfied.

An intent-to-infrastructure compiler resolves high-level requirements into domain-approved resource plans. Independent domain agents enforce local policies. Causality-scoped operation lanes avoid imposing unnecessary global ordering. A multi-level recovery planner restores obligations across nodes, H-units, regions, and independent operators. Data, compute, and network resources are jointly considered when selecting where work should occur.

The resulting system is intended to provide:

**ONE LOGICAL DATA SERVICE. MANY AUTONOMOUS DOMAINS. EXPLICIT DATA CUSTODY. POLICY-DRIVEN PLACEMENT. CAUSALITY-SCOPED COORDINATION. VERIFIED RECOVERY.**

The system is not defined by owning all infrastructure. It is defined by coordinating infrastructure that remains independently owned and administered.

That is the expanded IDS vision and the novelty direction established by this prior-art assessment.

---

# 21. RESEARCH SOURCE REGISTER

The following sources form the research basis for the novelty assessment. Official technical documentation and primary research publications are prioritised.

1. **Google File System (GFS), SOSP 2003.** https://static.googleusercontent.com/media/research.google.com/en//archive/gfs-sosp2003.pdf
2. **Ceph — CRUSH Maps.** https://docs.ceph.com/en/latest/rados/operations/crush-map/
3. **Ceph — Architecture.** https://docs.ceph.com/en/reef/architecture/
4. **Apache Hadoop — HDFS Federation.** https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/Federation.html
5. **Apache Hadoop — HDFS Router-based Federation.** https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs-rbf/HDFSRouterFederation.html
6. **Khan et al. — IStore: Towards the Optimization of Federation File Systems, IEEE Access 2019.** https://impact.ornl.gov/en/publications/istore-towards-the-optimization-of-federation-file-systems/
7. **Rucio — What is Rucio?** https://rucio.github.io/documentation/started/what_is_rucio/
8. **Rucio — Replica Management with Replication Rules.** https://rucio.github.io/documentation/started/concepts/replica_management/
9. **iRODS — Data Virtualization and Federated Zones.** https://irods.org/
10. **MinIO — Site Replication.** https://min.io/docs/minio/linux/operations/install-deploy-manage/multi-site-replication.html
11. **Apache Ozone — Architecture.** https://ozone.hadoop.apache.org/docs/core-concepts/architecture/overview/
12. **Apache Ozone — Write Pipelines.** https://ozone.apache.org/docs/core-concepts/replication/write-pipelines/
13. **Apache Ozone — Cluster Architectures.** https://ozone.apache.org/docs/next/administrator-guide/configuration/cluster-architectures/
14. **CockroachDB — Architecture Design.** https://github.com/cockroachdb/cockroach/blob/master/docs/design.md
15. **YugabyteDB — Architecture.** https://docs.yugabyte.com/stable/architecture/
16. **YugabyteDB — Multi-Region Deployments.** https://docs.yugabyte.com/stable/explore/multi-region-deployments/
17. **FoundationDB — Technical Overview.** https://apple.github.io/foundationdb/technical-overview.html
18. **FoundationDB — Architecture.** https://apple.github.io/foundationdb/architecture.html
19. **Vitess — Topology Service.** https://vitess.io/docs/24.0/concepts/topology-service/
20. **CrossFS — Improving Cross-Domain File System Performance with CRDT-Based Metadata Synchronization, ACM Transactions on Storage, 28 March 2026.** https://doi.org/10.1145/3777470
21. **Liu et al. — SkyStore: Cost-Optimized Object Storage Across Regions and Clouds, VLDB 2025.** https://research.ibm.com/publications/skystore-cost-optimized-object-storage-across-regions-and-clouds
22. **US20220035769A1 — Peer-to-peer Global Namespace for Storage System Metadata Federations.** https://patents.google.com/patent/US20220035769A1
23. **US11947499B2 — Granted patent corresponding to the peer-to-peer global namespace disclosure.** https://patents.justia.com/patent/11947499
24. **International Data Spaces Association — Reference Architecture Model.** https://internationaldataspaces.org/offers/reference-architecture/
25. **Gu et al. — A Systematic Overview of Data Federation Systems, Semantic Web 2024.** https://journals.sagepub.com/doi/full/10.3233/SW-223201
26. **Zhang et al. — A Systematic Survey of Cross-Domain Data Sharing Architectures, Journal of Computer Science and Technology, July 2026.** https://jcst.ict.ac.cn/article/cstr/32374.14.s11390-026-5768-x
27. **d'Abate et al. — Replication-Aware Placement of Functions and Data in the Edge-Cloud Continuum, September 2026.** https://arxiv.org/abs/2609.18932
28. **ZJC — Constructing Fully Local Repair in Erasure Codes for Distributed Cloud Storage, 2026.** https://www.sciencedirect.com/science/article/pii/S1383762126000536
29. **Survey of Distributed File System Design Choices.** https://doi.org/10.1145/3465405
30. **Alluxio — Introduction and Data Orchestration.** https://documentation.alluxio.io/os-en

---

# 22. DOCUMENT POSITION

This document supersedes the earlier `Novel.md` and is the novelty and expanded-vision companion to `IDS.md` and `IDS-A.md`.

It does not rewrite the baseline architecture. It identifies the closest prior art, defines which parts of the current design are established patterns, and sets out the larger system architecture that IDS should pursue.

**IDS — Independent Distributed System**  
**From regional distributed databases to a federation of autonomous data infrastructure.**
