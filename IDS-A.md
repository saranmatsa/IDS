# IDS — ARCHITECTURAL ADDENDUM

**Project:** HX-241 / XarnX  
**Document:** IDS-A.md  
**Scope:** Additions to the existing IDS.md  
**Status:** CONCEPTUAL — UNPROVEN  

This addendum continues the existing IDS specification. The preceding document ends at Section 34; this document begins at Section 35 and records only the architectural additions established afterward.

---

# 35. INTERMEDIATE HIERARCHICAL SUBCLUSTER ARCHITECTURE — H-UNITS

## 35.1 Introduction

IDS introduces an intermediate infrastructure level called an **H-unit** between regional database clusters and individual storage nodes.

The purpose of this level is to establish a structured relationship between multiple storage nodes and provide an additional organizational layer for data placement, redundancy, failure management, and recovery.

The updated infrastructure hierarchy is:

```text
CLIENTS
   |
   v
IDS
   |
   +-----------------------------+
   |              |              |
  DB1            DB2            DB3
 REGION A      REGION B       REGION C
   |              |              |
 H-UNITS        H-UNITS        H-UNITS
   |              |              |
STORAGE         STORAGE        STORAGE
NODES           NODES          NODES
```

A regional database cluster contains multiple H-units. Each H-unit contains multiple storage nodes.

For example:

```text
DB1 — REGION A
 |
 +-- H1
 |    +-- N117
 |    +-- N118
 |    +-- N119
 |
 +-- H2
 |    +-- N120
 |    +-- N121
 |    +-- N122
 |
 +-- H3
      +-- N123
      +-- N124
      +-- N125
```

H-units are not necessarily independent databases. They are intermediate subclusters within the regional database infrastructure.

## 35.2 Why H-Units Are Necessary

Individual storage nodes have finite capacities. Each node has a maximum amount of storage, processing capability, memory, and network throughput.

Consequently, IDS cannot treat the storage infrastructure as an unlimited collection of interchangeable machines. It must understand how nodes are organized, how much capacity they have, what information they maintain, and which other infrastructure components can assume their responsibilities.

The H-level provides this additional organization. Rather than managing every node only through the regional database cluster, IDS can organize nodes into identifiable subclusters. This creates a hierarchy through which storage responsibility and recovery can be managed.

## 35.3 H-Unit as an Infrastructure Management Domain

An H-unit represents a defined group of participating storage nodes. Its internal responsibilities include:

- Maintaining H-unit identity.
- Maintaining node membership information.
- Tracking individual node health.
- Tracking available and occupied storage capacity.
- Recording data placement.
- Maintaining redundancy relationships.
- Identifying unavailable nodes and affected data.
- Participating in failure compensation.
- Coordinating recovery operations.
- Reporting operational status to its regional database cluster.

H-units therefore provide more than a naming convention. They establish an intermediate level at which the system can organize storage resources and coordinate responsibility between groups of nodes.

## 35.4 H-Unit Independence

H-units should have identifiable operational boundaries. An H-unit must be distinguishable from other H-units within the same regional database cluster. Each H-unit has its own membership and capacity information.

The regional database cluster manages the relationship between these H-units. The system can identify which H-unit maintains particular data, which H-units contain eligible replacement capacity, and which H-units can participate in recovery.

## 35.5 H-Unit Expansion

A regional database cluster can expand by introducing additional H-units. Each H-unit can contain a different number of nodes according to its infrastructure configuration. The architecture does not require every H-unit to have identical storage capacity.

Heterogeneous infrastructure is permitted within the architecture.

---

# 36. H-UNIT FAILURE COMPENSATION

## 36.1 Fundamental Principle

H-units participate in compensating for storage-node failures. If an individual node permanently fails, IDS must identify the affected data and restore its availability or storage responsibility through other participating infrastructure.

The compensation mechanism can involve another H-unit.

For example, if N119 in H1 permanently fails, H2 can participate in compensating for the lost information.

```text
DB1
 |
 +-- H1
 |    +-- N117
 |    +-- N118
 |    +-- N119 [FAILED]
 |
 +-- H2
      +-- N120
      +-- N121
      +-- N122
           +-- PARTICIPATES IN COMPENSATION
```

The failure does not automatically mean that the information maintained by N119 is permanently lost. Its redundancy configuration determines how the system restores that information.

## 36.2 Failure Identification

IDS must distinguish between different node states. Possible states include:

- Operational.
- Temporarily unavailable.
- Degraded.
- Suspected failure.
- Permanently failed.
- Recovering.
- Rejoining.
- Retired.

The distinction between temporary unavailability and permanent failure is important. A node that temporarily loses communication should not automatically be treated as permanently destroyed.

IDS should maintain sufficient failure information to determine whether the node is unavailable, recovering, or permanently removed from the infrastructure.

## 36.3 Compensation Procedure

A proposed compensation sequence is:

1. N119 becomes unavailable.
2. H1 records the node's changed operational state.
3. The regional database cluster receives the failure information.
4. IDS identifies the data and storage responsibilities associated with N119.
5. IDS checks the applicable redundancy policy.
6. IDS identifies eligible replacement infrastructure.
7. H2 or another suitable H-unit participates in compensation.
8. Existing replicas or encoded fragments are located.
9. Missing information is recovered where necessary.
10. Replacement storage responsibility is assigned.
11. The restored information is verified.
12. The infrastructure's redundancy state is updated.

The exact recovery mechanism depends on the data's redundancy configuration.

## 36.4 Compensation Through Another H-Unit

The system can use a different H-unit to compensate for a failed node. H2 can supply an existing replica, participate in reconstruction, or receive reassigned data. H3 may also participate if its capacity and redundancy relationships make it an eligible destination.

The architecture should support compensation across multiple H-units rather than assuming that one particular H-unit must always handle another H-unit's failure.

## 36.5 H-Unit Capacity Awareness

Compensation requires available capacity. IDS must know whether a potential replacement H-unit has sufficient storage resources.

For example:

| H-unit | Available capacity | Operational state |
|---|---:|---|
| H1 | 120 GB | Operational |
| H2 | 450 GB | Operational |
| H3 | 80 GB | Operational |
| H4 | 700 GB | Operational |

If H1 loses a node containing 200 GB of information, H2 may be a suitable compensation destination. H3 may not have sufficient capacity.

This is why H-unit membership and capacity information are important parts of the architecture. The system should account for capacity before assigning replacement responsibility.

## 36.6 Recovery Verification

After compensation, IDS should verify that the recovered information corresponds to the original data.

Verification should include:

- Record integrity.
- Partition completeness.
- Replica consistency.
- Checksum verification.
- Version identification.
- Reconstructed-data verification.
- Placement metadata updates.

A recovery operation should not be considered complete merely because a replacement node has been assigned. The recovered information must be verified.

---

# 37. HYBRID REDUNDANCY ARCHITECTURE

## 37.1 Introduction

IDS adopts a hybrid redundancy direction. The architecture permits two major redundancy approaches to coexist:

1. Full replication.
2. Erasure coding and reconstruction-based redundancy.

These approaches do not have to be mutually exclusive across the entire system. Different datasets, partitions, or data classes can use different redundancy configurations.

The system should determine the appropriate redundancy policy according to the data's configured requirements.

## 37.2 Full Replication

Under full replication, a complete copy of the relevant information is maintained in multiple locations.

```text
DATASET A

H1: N117 — FULL COPY
H2: N120 — FULL COPY
H3: N123 — FULL COPY
```

If N117 fails, the system can retrieve the information from N120 or N123. Full replication provides directly accessible copies of the data and is suitable for data whose configured requirements favor straightforward availability and direct recovery.

## 37.3 Erasure Coding and Reconstruction

Under erasure coding, information is divided into encoded fragments. Additional redundancy fragments are generated so that the original information can be reconstructed from a sufficient subset of available fragments.

```text
ORIGINAL DATA
     |
  ENCODING
     |
     +---- Fragment A
     +---- Fragment B
     +---- Fragment C
     +---- Parity Fragment D
     +---- Parity Fragment E
```

These fragments can be distributed across different storage nodes and H-units. If a node containing one fragment fails, the original information can potentially be reconstructed using the remaining available fragments.

The exact number of fragments and the reconstruction threshold depend on the selected coding configuration.

## 37.4 Coexistence of Both Approaches

The IDS architecture does not require every dataset to use identical redundancy.

```text
IDS
 |
 +-- DATASET A — Full replication
 +-- DATASET B — Erasure coding
 +-- DATASET C — Full replication
 +-- DATASET D — Erasure coding
```

The redundancy policy can be associated with a dataset or a smaller data-placement unit. This permits different information to be maintained according to different redundancy requirements.

## 37.5 Hybrid Redundancy and H-Units

H-units provide an organizational level through which redundancy placement can be managed. A dataset can have copies or encoded fragments distributed across H1, H2, and H3.

If one node or H-unit becomes unavailable, the system determines whether the remaining redundancy is sufficient to maintain access or reconstruct the missing information.

The architecture should consider the failure domains of participating nodes. Placing multiple copies on nodes that fail together does not provide the same protection as distributing them across independent failure domains.

## 37.6 Redundancy Policy Definition

Each dataset or partition should have an identifiable redundancy policy. A policy may define:

- Redundancy method.
- Number of full replicas.
- Number of encoded fragments.
- Reconstruction threshold.
- Required placement domains.
- Regional distribution requirements.
- Recovery requirements.
- Verification requirements.
- Minimum acceptable redundancy state.

The architecture should allow different datasets to use different policies without requiring a change to the overall client-facing IDS interface.

---

# 38. DISTRIBUTED WRITE-ORDER MANAGEMENT

## 38.1 Introduction

IDS includes a write-order management mechanism intended to establish a consistent sequence for operations when ordering is required.

A client can submit a write request without knowing which physical node will execute it. IDS receives the operation, assigns an identifiable reference, resolves the relevant dataset and infrastructure, and processes the write according to its ordering requirements.

The system must distinguish between the identity of an operation and its position in an execution sequence. These are separate properties.

## 38.2 Operation References

Each write operation can receive a unique nine-character uppercase alphanumeric reference. The character set consists of 26 uppercase letters and 10 numerical digits, giving 36 possible characters per position.

For nine positions:

\[
36^9 = 101,559,956,668,416
\]

The total reference space is therefore **101,559,956,668,416 possible identifiers**, approximately 101.56 trillion.

Examples:

```text
A7K92M4PX
B3N81Q6RT
Z8F21L5WA
M4T72C9QH
```

Each identifier represents a distinct operation reference.

## 38.3 Identifier and Ordering Are Different

The nine-character identifier must not automatically be interpreted as a chronological sequence number.

For example:

```text
C1 → A7K92M4PX
C2 → B3N81Q6RT
```

The fact that A7K92M4PX appears lexicographically before B3N81Q6RT does not prove that C1's operation was received or committed first.

The identifier establishes operation identity. The ordering mechanism establishes execution or commit order.

IDS should maintain these as separate concepts.

A proposed operation record can contain:

```text
OPERATION
 |
 +-- Operation Reference
 +-- Dataset Identifier
 +-- Client Identifier
 +-- Request Information
 +-- Assigned Ordering Domain
 +-- Ordering Position
 +-- Operation State
 +-- Durability State
 +-- Redundancy Policy
```

The nine-character reference remains the stable identity of the operation. Its ordering position is separately determined.

## 38.4 Independent Ordering Domains

IDS should not automatically impose one global sequence across every dataset. Different datasets can maintain independent ordering domains.

```text
IDS
 |
 +-- DATASET A — Ordering Domain A
 +-- DATASET B — Ordering Domain B
 +-- DATASET C — Ordering Domain C
```

A write affecting Dataset A does not inherently need to wait for an unrelated write affecting Dataset B. Each dataset can maintain its own ordering requirements.

This avoids treating the entire IDS infrastructure as one enormous sequential write queue. The ordering scope should be determined by the relevant data-consistency requirements.

Where multiple datasets participate in one transaction, the system requires an additional coordination mechanism for that transaction.

## 38.5 Ordering Position

Each operation that requires ordered processing must receive a position within its applicable ordering domain.

```text
DATASET A
Operation 1 → Position 101
Operation 2 → Position 102
Operation 3 → Position 103
Operation 4 → Position 104
```

These positions describe the established operation order within that domain. They are not interchangeable with the nine-character operation references.

The architecture must maintain a reliable relationship between the operation reference and its ordering position.

---

# 39. CONCURRENT WRITE ARRIVALS — EDGE-CASE RESOLUTION

## 39.1 Design Classification

Simultaneous write arrivals are treated as a concurrency-resolution case. They are not intended to define the normal execution path for every write.

The normal operating condition is that writes arrive and are processed through their respective ordering domains. However, multiple clients can submit operations at effectively the same time.

IDS must determine how these operations are ordered when their arrival times do not establish an unambiguous sequence.

## 39.2 Example: Two Simultaneous Writes

Consider two clients.

C1 submits `WRITE("Hello")`. C2 submits `WRITE("World")`. Both requests arrive at IDS at approximately the same time.

The system cannot assume that the first request processed by one arbitrary physical machine represents the definitive order. Instead, the relevant ordering domain must establish a committed sequence.

```text
C1 → A7K92M4PX
C2 → B3N81Q6RT
```

The operation references are assigned independently of the final ordering decision.

The ordering mechanism then establishes, for example:

```text
POSITION 101 → A7K92M4PX
POSITION 102 → B3N81Q6RT
```

Alternatively, the order may be reversed if the coordination mechanism establishes that C2 precedes C1. The essential requirement is that all participating components apply the same established order.

## 39.3 Concurrent Arrival Procedure

A proposed procedure is:

1. C1 and C2 submit their requests.
2. IDS receives both requests.
3. Each operation receives a unique reference.
4. IDS identifies the relevant dataset.
5. IDS identifies the dataset's ordering domain.
6. The ordering mechanism detects that the requests require conflict resolution.
7. The participating coordination nodes establish an order.
8. The operations receive their respective ordering positions.
9. The writes are processed according to the established sequence.
10. The configured durability requirements are evaluated.
11. The operations are marked committed when their respective commit conditions are satisfied.
12. Readers observe the committed state according to the dataset's consistency policy.

The mechanism must produce one authoritative result for the ordering domain. Two participating nodes must not independently establish conflicting committed orders for the same operations.

## 39.4 Normal Path Versus Concurrency-Resolution Path

**Normal path:**

```text
WRITE REQUEST
     |
OPERATION REFERENCE
     |
DATASET RESOLUTION
     |
ORDERING DOMAIN
     |
WRITE EXECUTION
     |
DURABILITY CHECK
     |
COMMIT
```

**Concurrent-arrival path:**

```text
C1 WRITE ----+
             |
      CONCURRENCY RESOLUTION
             |
C2 WRITE ----+
             |
UNIQUE OPERATION REFERENCES
             |
DISTRIBUTED ORDER AGREEMENT
             |
ESTABLISHED OPERATION ORDER
             |
WRITE EXECUTION
             |
COMMIT
```

The second path exists to resolve simultaneous arrivals. It should not automatically become a requirement for every unrelated write throughout IDS.

## 39.5 Deterministic Observation

Once the order has been established, participating components must apply it consistently.

For example:

```text
POSITION 101 → WRITE A
POSITION 102 → WRITE B
```

A reader operating under an ordering guarantee must not observe B as committed while A is still absent if the dataset's consistency contract requires the committed prefix to be visible.

The reader must have a way to determine the committed position of the dataset.

The system should distinguish received operations, ordered operations, executing operations, committed operations, replicated operations, and fully synchronized operations. These states must not be treated as equivalent.

---

# 40. DISTRIBUTED COORDINATION WITHOUT A PERMANENT GLOBAL MASTER

## 40.1 Architectural Requirement

IDS is intended to operate across infrastructure that may be owned or operated by different parties. The architecture cannot assume that IDS owns every participating physical server.

Consequently, the design should not require one permanently available, centrally owned master machine to control all write ordering throughout the system.

The system should be capable of coordinating distributed operations through participating infrastructure. This is particularly relevant when the infrastructure contains geographically separated regional database clusters and independently managed storage nodes.

## 40.2 Separation of Identifier Generation and Coordination

Operation-reference generation and write-order coordination are separate responsibilities.

The nine-character reference identifies an operation. The coordination mechanism establishes its ordering position when ordering is required.

These responsibilities should not be combined into a single mandatory master-server function.

A distributed implementation could maintain reference allocation independently from dataset ordering.

## 40.3 Distributed Coordination Groups

Each ordering domain can have a participating coordination group.

```text
DATASET A
    |
ORDERING DOMAIN A
    |
    +-- Participant 1
    +-- Participant 2
    +-- Participant 3
```

The participants collectively establish an authoritative operation order.

The exact protocol remains an implementation and research decision. Possible research directions include:

- Quorum-based agreement.
- Replicated state machines.
- Distributed leader election.
- Consensus protocols.
- Partitioned coordination.
- Hierarchical coordination.
- Failure-aware membership management.

The architecture does not yet select one of these as its final implementation.

## 40.4 No Permanent System-Wide Ordering Master

The intended architecture avoids requiring every dataset and every regional cluster to depend on one permanent global ordering server.

Different ordering domains can operate independently. A coordination group responsible for Dataset A does not automatically control Dataset B.

This allows the system to separate unrelated write activity. It also means that a failure within one ordering domain should not automatically require unrelated datasets to stop processing their operations.

## 40.5 Temporary Coordination Leadership

A distributed agreement protocol may use an elected leader or coordinator for a particular ordering domain. This does not necessarily imply a permanent global master.

Leadership can be restricted to a specific domain and can change when the coordination membership changes.

The distinction is:

- **Permanent global master:** One central machine controls ordering throughout the complete system.
- **Domain-scoped coordination:** A group of participating nodes establishes operation order within its assigned dataset or partition.

IDS investigates the second architectural direction.

## 40.6 Infrastructure Ownership

IDS should treat physical infrastructure ownership as separate from logical participation.

A participating node can be owned by an external infrastructure operator while still participating in an IDS-managed storage or coordination arrangement.

The system therefore requires explicit mechanisms for node registration, participant identity, membership validation, operational health, capacity reporting, communication eligibility, failure detection, membership changes, and responsibility reassignment.

The system's logical coordination architecture should not assume that every participating machine belongs to one organization.

---

# 41. DATASET-SCOPED ORDERING

## 41.1 Independent Ordering

The ordering mechanism is designed around independent datasets rather than one mandatory system-wide sequence.

Each dataset can have its own ordering domain.

```text
IDS
 |
 +-- DATASET A
 |     +-- A-101
 |     +-- A-102
 |     +-- A-103
 |
 +-- DATASET B
 |     +-- B-201
 |     +-- B-202
 |     +-- B-203
 |
 +-- DATASET C
       +-- C-301
       +-- C-302
```

Operations in Dataset A are ordered according to Dataset A's ordering mechanism. Operations in Dataset B are ordered independently. Dataset C maintains its own ordering requirements.

There is no inherent need to establish one global ordering position shared by all three datasets.

## 41.2 Why Dataset Independence Matters

Consider:

```text
C1 → WRITE DATASET A
C2 → WRITE DATASET B
```

These operations affect unrelated datasets. Unless an explicit dependency exists, the system should not require the first operation to complete before the second can proceed.

The two ordering domains can progress independently. A system-wide sequential queue would introduce dependencies between operations that may have no logical relationship.

IDS instead investigates independent ordering domains.

## 41.3 Dataset-Level Ordering Guarantees

Each dataset must define the consistency and ordering guarantees expected from its operations.

A dataset may require ordered writes, read-after-write visibility, sequential observation of committed operations, conflict detection, version tracking, transactional operations, or concurrent updates.

The ordering mechanism must follow the dataset's defined requirements.

## 41.4 Future Partition-Level Ordering

Dataset-level ordering is the initial architectural scope. A future extension may introduce separate ordering domains for partitions within a dataset.

```text
DATASET A
 |
 +-- PARTITION A1 — ORDERING DOMAIN A1
 +-- PARTITION A2 — ORDERING DOMAIN A2
 +-- PARTITION A3 — ORDERING DOMAIN A3
```

This would permit independent ordering within separate partitions. Operations involving multiple partitions would require a mechanism for coordinating the affected domains.

Partition-level ordering remains a future design extension rather than an assumed current capability.

---

# 42. WRITE COMMIT AND DURABILITY CONDITIONS

## 42.1 Commit Is a Defined State

A write being received does not mean it has been committed. Likewise, assigning an operation reference does not mean that its data has been durably stored.

IDS must distinguish the stages of a write.

```text
RECEIVED
   |
IDENTIFIED
   |
ORDERED
   |
EXECUTING
   |
DURABILITY CONDITIONS
   |
COMMITTED
   |
SYNCHRONIZATION
```

Not every dataset necessarily uses identical durability conditions.

## 42.2 Durability Policy

A dataset's redundancy policy determines where information must be stored before IDS can acknowledge a successful write.

For example, one dataset may require a full replica in another H-unit. Another may require a configured number of encoded fragments. A third may require cross-regional redundancy before acknowledgement.

These are different durability policies. The commit mechanism must understand the policy associated with the relevant data.

## 42.3 Commit and Synchronization Are Separate

A write may be committed locally while designated remote replicas are still synchronizing, depending on the dataset's configured durability contract.

Therefore, IDS must not treat synchronization completion and commit as automatically identical.

The operation record should maintain distinct state information.

```text
OPERATION A7K92M4PX

Reference: A7K92M4PX
Ordering position: 101
Execution: Complete
Local durability: Complete
Required redundancy: Satisfied
Remote synchronization: Pending
Commit state: Committed
```

This is an illustrative state model. The exact relationship between commit and remote synchronization depends on the selected policy.

## 42.4 Ordered Commit Prefix

For an ordering domain, IDS should track the highest contiguous committed position.

```text
POSITION 101 → COMMITTED
POSITION 102 → COMMITTED
POSITION 103 → COMMITTED
POSITION 104 → PENDING
POSITION 105 → EXECUTING
```

The committed prefix is 103. A reader requiring the complete committed prefix must not treat position 105 as though every preceding operation has already committed.

This provides a basis for consistent ordered reads.

---

# 43. CLIENT WRITE EXAMPLE — UPDATED OPERATION MODEL

## 43.1 Initial Request

C1 submits `WRITE("Hello")`. C2 submits `WRITE("World")`. Both requests are received by IDS.

## 43.2 Operation Identification

IDS assigns operation references:

```text
C1 → A7K92M4PX
C2 → B3N81Q6RT
```

These references identify the two operations within the available identifier space, subject to the reference-generation mechanism's uniqueness guarantees.

## 43.3 Dataset Resolution

IDS determines which datasets the requests affect. If both operations affect Dataset A, they are assigned to Dataset A's ordering domain. If they affect different datasets, they can be processed independently.

## 43.4 Ordering Resolution

For simultaneous writes to Dataset A:

```text
DATASET A
A7K92M4PX → POSITION 101
B3N81Q6RT → POSITION 102
```

The order is established by the relevant coordination mechanism.

## 43.5 Infrastructure Resolution

IDS identifies regional database ownership, the relevant H-unit, eligible storage nodes, data placement, redundancy configuration, and required synchronization.

## 43.6 Write Execution

The relevant storage nodes execute the writes according to the established ordering positions. The system distributes the information according to the dataset's redundancy policy.

```text
DB1
 |
 +-- H1
 |    +-- N117: DATA PLACEMENT
 |    +-- N118: REDUNDANCY
 |
 +-- H2
      +-- N120: ADDITIONAL REDUNDANCY
```

This is an illustrative placement configuration, not a mandatory topology.

## 43.7 Completion

IDS evaluates the configured durability requirements. The operation is acknowledged according to its commit policy. The operation reference remains available for subsequent tracking and identification.

---

# 44. READ OPERATIONS UNDER THE UPDATED ARCHITECTURE

## 44.1 Unified Client Read

A client does not need to know which H-unit or physical node contains the requested information.

For example, C3 submits `READ(DATASET A)`. IDS resolves the dataset and determines the appropriate regional database cluster. The regional cluster identifies the relevant H-unit. The H-unit identifies an eligible storage node. The node returns the requested information.

## 44.2 Read Resolution

A proposed sequence is:

1. Client submits a read request.
2. IDS identifies the requested dataset.
3. IDS resolves the logical data identifier.
4. IDS identifies the responsible regional cluster.
5. IDS identifies the relevant H-unit.
6. The H-unit identifies an eligible node containing the information.
7. The node retrieves the information.
8. IDS evaluates the requested consistency requirements.
9. IDS returns the result.

## 44.3 Reading From Another Replica

If one node is unavailable, IDS can resolve an eligible alternative. For fully replicated data, it may retrieve a complete copy from another node. For erasure-coded data, it may retrieve the required fragments and reconstruct the original information.

The read mechanism therefore works with the hybrid redundancy architecture.

## 44.4 Ordered Reads

For a dataset using ordered writes, reads must respect the dataset's visibility rules.

For example:

```text
POSITION 101 → COMMITTED
POSITION 102 → COMMITTED
POSITION 103 → PENDING
```

The reader should not receive a result that incorrectly treats position 103 as committed. The system should maintain sufficient metadata to determine which operations are visible to the requested read.

---

# 45. REGIONAL FAILURE AND COMPENSATION

## 45.1 Regional Failure

The H-unit architecture handles organization and compensation within regional infrastructure. The same general redundancy philosophy extends to regional failures.

```text
IDS
 |
 +-- DB1 — REGION A
 +-- DB2 — REGION B
 +-- DB3 — REGION C
```

Suppose DB1 becomes permanently unavailable. The system identifies the datasets and partitions affected by the failure. Information maintained through configured cross-regional redundancy may remain accessible through DB2 or DB3.

## 45.2 Regional Compensation

Regional compensation may involve:

- Accessing an existing regional replica.
- Reconstructing data from encoded fragments.
- Reassigning regional ownership.
- Establishing replacement placement.
- Rebuilding required redundancy.
- Reconciling metadata.
- Verifying recovered information.

The exact procedure depends on the dataset's configured regional redundancy policy.

## 45.3 Relationship Between H-Level and Regional Recovery

The updated architecture provides two distinct compensation scopes.

**H-level compensation:** A failed storage node can be compensated for through another H-unit within the regional infrastructure.

**Regional compensation:** A failed regional database cluster can be compensated for through participating infrastructure in other regions, where the configured redundancy permits it.

These are separate failure-management levels.

---

# 46. OPERATIONAL STATE AND METADATA REQUIREMENTS

The introduction of H-units, hybrid redundancy, operation references, and independent ordering domains requires additional metadata.

## 46.1 Node Metadata

Node records should include:

- Node identifier.
- H-unit identifier.
- Regional database identifier.
- Storage capacity and available capacity.
- Current operational state.
- Assigned data.
- Replica information.
- Encoded-fragment information.
- Health information.
- Recovery state.

## 46.2 H-Unit Metadata

H-unit records should include:

- H-unit identifier.
- Regional database identifier.
- Participating nodes.
- Node membership states.
- Total and available storage capacity.
- Assigned data.
- Redundancy relationships.
- Failure-compensation relationships.
- Operational state.

## 46.3 Dataset Metadata

Dataset records should include:

- Dataset identifier.
- Logical ownership.
- Regional placement.
- Partition information.
- H-unit placement.
- Node placement.
- Redundancy policy.
- Ordering policy.
- Consistency requirements.
- Recovery requirements.

## 46.4 Operation Metadata

Each operation record should include:

- Nine-character operation reference.
- Client identity.
- Dataset identity.
- Request type.
- Ordering domain.
- Ordering position.
- Execution state.
- Commit state.
- Durability state.
- Redundancy state.
- Synchronization state.
- Operation version.

The operation reference is the stable identity used to associate these records.

## 46.5 Ordering-Domain Metadata

Ordering-domain records should include:

- Domain identifier.
- Associated dataset or partition.
- Participating coordination nodes.
- Current membership.
- Current committed position.
- Pending operations.
- Agreed operation order.
- Coordination state.
- Recovery information.

This metadata provides the information required to maintain ordered operation histories.

---

# 47. NINE-CHARACTER ALPHANUMERIC REFERENCE SPACE

## 47.1 Character Set

The proposed operation-reference format uses uppercase English letters and numerical digits.

```text
ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789
```

The number of available characters is 36. For a nine-character reference:

\[
36^9=101,559,956,668,416
\]

Therefore, the total number of possible references is **101,559,956,668,416**.

## 47.2 Reference Examples

```text
A7K92M4PX
B3N81Q6RT
Z8F21L5WA
M4T72C9QH
X9P31K6VB
```

## 47.3 Reference Assignment

Each operation receives a reference through the IDS reference-generation mechanism. The reference should remain associated with the operation throughout its lifecycle.

It can be used for operation identification, request tracking, retry recognition, metadata association, commit-status lookup, synchronization tracking, recovery tracking, and audit records.

## 47.4 Reference Uniqueness

The reference space provides a large number of possible identifiers. However, the available combination count does not itself guarantee uniqueness.

IDS must define a reference-generation mechanism that prevents duplicate active references and manages the possibility of previously generated references being encountered again.

Potential mechanisms include distributed allocation, randomized generation with collision detection, coordinated namespace allocation, deterministic generation using suitable inputs, and hybrid generation and validation.

The final generation mechanism remains to be specified.

## 47.5 Reference and Ordering Independence

The operation reference must not be confused with the ordering position.

```text
OPERATION REFERENCE: A7K92M4PX
ORDERING POSITION: 101
```

These fields serve different purposes. The reference identifies the operation. The ordering position identifies its place in a particular ordering domain.

---

# 48. CONCURRENCY MANAGEMENT AS AN EXCEPTIONAL OPERATING PATH

## 48.1 Design Intent

IDS is not intended to impose a universal sequential execution queue on all incoming operations.

Simultaneous requests are possible, but they do not mean every operation must be coordinated globally. The architecture should resolve concurrency within the scope where ordering actually matters.

## 48.2 Independent Operations

Consider:

```text
C1 → WRITE(DATASET A)
C2 → WRITE(DATASET B)
```

These operations affect different datasets. They can proceed independently according to their respective ordering and durability policies. The fact that they arrive simultaneously does not automatically create a conflict.

## 48.3 Conflicting Operations

Consider:

```text
C1 → UPDATE(RECORD X)
C2 → UPDATE(RECORD X)
```

Both operations affect the same record. The relevant ordering or conflict-resolution mechanism must determine how the updates are handled.

This may involve ordered execution, version checks, transaction coordination, conflict detection, explicit rejection and retry, or other dataset-specific semantics.

The system should not confuse simultaneous arrival with an actual data conflict.

## 48.4 Concurrency Resolution Scope

The relevant coordination scope should be as narrow as the consistency requirements permit.

Possible scopes include record, partition, dataset, and multi-dataset transaction.

The initial architecture defines independent dataset-level ordering. More granular ordering may be introduced later.

## 48.5 Exceptional Coordination

The concurrency-resolution mechanism should establish a definitive order when simultaneous operations require one.

The system should not require a single global master to perform this function. Instead, the relevant ordering domain can use distributed coordination among its participating nodes.

This allows the architecture to preserve independent operation across unrelated datasets.

---

# 49. UPDATED ARCHITECTURAL RESPONSIBILITY MAP

The additions establish more clearly defined responsibilities across IDS.

| Component | Primary responsibility |
|---|---|
| Clients | Submit read and write operations |
| IDS | Unified coordination and operation resolution |
| Reference mechanism | Generate operation identifiers |
| Ordering domain | Establish operation order where required |
| Regional DB cluster | Regional data organization and ownership |
| H-unit | Intermediate node organization and failure compensation |
| Storage node | Maintain and execute physical data operations |
| Redundancy mechanism | Maintain additional data protection |
| Recovery mechanism | Restore information and assigned responsibilities |
| Synchronization mechanism | Propagate and reconcile distributed changes |
| Metadata subsystem | Maintain logical and physical infrastructure state |

These responsibilities are distinct but interconnected.

A write operation may involve the reference mechanism, an ordering domain, a regional database cluster, an H-unit, multiple storage nodes, and the redundancy subsystem. The client continues interacting through the unified IDS interface.

---

# 50. UPDATED END-TO-END WRITE WORKFLOW

The complete write workflow incorporates the new architectural components.

```text
CLIENT
  |
  | WRITE REQUEST
  v
IDS
  |
  | Generate operation reference
  v
OPERATION IDENTIFICATION
  |
  | Identify dataset
  v
DATASET RESOLUTION
  |
  | Identify ordering requirements
  v
ORDERING DOMAIN
  |
  | Establish order when required
  v
REGIONAL DATABASE CLUSTER
  |
  | Resolve H-unit
  v
H-UNIT
  |
  | Select storage nodes
  v
STORAGE NODES
  |
  | Execute write
  v
HYBRID REDUNDANCY
  |
  | Replication or encoding
  v
DURABILITY EVALUATION
  |
  | Verify required conditions
  v
COMMIT
  |
  v
CLIENT ACKNOWLEDGEMENT
```

The exact sequence can vary according to the dataset's consistency and durability policies. The diagram represents overall responsibilities rather than a mandatory implementation sequence for every write.

---

# 51. UPDATED FAILURE WORKFLOW

The updated failure workflow incorporates H-unit compensation and hybrid redundancy.

```text
NODE FAILURE
     |
FAILURE DETECTION
     |
IDENTIFY AFFECTED DATA
     |
CHECK REDUNDANCY POLICY
     |
IDENTIFY ELIGIBLE H-UNITS
     |
SELECT COMPENSATION METHOD
     |
     +-----------------------+
     |                       |
FULL REPLICA            RECONSTRUCTION
     |                       |
RETRIEVE COPY            REBUILD DATA
     |                       |
     +-----------+-----------+
                 |
         VERIFY INFORMATION
                 |
        RESTORE DATA PLACEMENT
                 |
       UPDATE INFRASTRUCTURE
                 |
         RESTORE REDUNDANCY
```

The failure workflow should be observable through metadata and operational state.

---

# 52. ADDITIONAL VERIFICATION REQUIREMENTS

The original verification programme should be extended to cover the new architecture.

## Test 13 — H-Unit Membership

Create a regional database cluster containing multiple H-units. Verify that each H-unit maintains its own node membership and can be identified independently.

## Test 14 — H-Unit Capacity

Assign storage nodes with different capacities. Verify that IDS can determine the available capacity of each H-unit.

## Test 15 — Cross-H-Unit Compensation

Place data in H1. Simulate permanent failure of a storage node in H1. Verify that H2 can participate in restoring the affected information.

## Test 16 — Hybrid Redundancy

Create one dataset using full replication and another using erasure coding. Verify that both redundancy mechanisms operate within the same regional infrastructure.

## Test 17 — Reconstruction

Remove an encoded fragment. Verify that the original data can be reconstructed when the required fragments remain available.

## Test 18 — Operation Reference Generation

Generate nine-character uppercase alphanumeric references. Verify the required format and uniqueness guarantees.

## Test 19 — Independent Dataset Ordering

Create two independent datasets. Submit writes to both datasets simultaneously. Verify that unrelated operations can progress independently.

## Test 20 — Concurrent Write Resolution

Submit simultaneous writes to the same ordering domain. Verify that the coordination mechanism establishes one authoritative operation order.

## Test 21 — Operation Identity and Ordering Separation

Verify that the nine-character operation reference remains distinct from its ordering position.

## Test 22 — No Permanent Global Master Dependency

Test the operation of independent ordering domains without requiring one permanent system-wide master. Verify that the relevant coordination groups can establish their respective operation orders.

## Test 23 — Ordered Read Visibility

Submit multiple writes to one dataset. Verify that readers observe committed operations according to the dataset's ordering policy.

## Test 24 — Commit-State Accuracy

Verify that received, ordered, executing, committed, and synchronized operations are represented as distinct states.

## Test 25 — Recovery With Pending Operations

Simulate a failure while operations are pending. Verify that the recovery mechanism can determine which operations were committed and which require retry or reconciliation.

---

# 53. NEW RESEARCH AREAS

## 53.1 H-Unit Coordination

Investigate how H-units should coordinate with their regional database clusters and with other H-units. The research should define membership protocols, capacity reporting, health reporting, failure notifications, responsibility transfer, and compensation coordination.

## 53.2 Cross-H-Unit Redundancy

Investigate how redundancy should be distributed across H-units. Determine which placement configurations provide meaningful independence between failure domains.

## 53.3 Hybrid Redundancy Selection

Investigate how IDS should select between full replication and erasure coding. Consider data size, read frequency, write frequency, reconstruction cost, storage overhead, failure requirements, regional placement, and recovery time.

## 53.4 Distributed Operation References

Investigate generation mechanisms for nine-character references. The research should establish collision probability, uniqueness guarantees, allocation requirements, concurrent generation, retry behaviour, and reference lifecycle.

## 53.5 Dataset-Scoped Ordering

Investigate distributed ordering mechanisms that operate independently for different datasets. Compare centralized sequencing within one domain, distributed consensus, quorum-based ordering, replicated logs, hierarchical ordering, and partition-level ordering.

The objective is to determine an architecture that does not require one permanent global master.

## 53.6 Concurrency Resolution

Investigate efficient mechanisms for resolving simultaneous writes. The research should distinguish simultaneous request arrival, conflicting operations, independent operations, ordered execution, commit agreement, and read visibility.

## 53.7 Ownership-Independent Infrastructure

Investigate how IDS can operate across infrastructure owned by multiple independent parties. Research participant identity, node authentication, membership changes, capacity declarations, operational permissions, failure handling, and distributed coordination.

---

# 54. UPDATED ARCHITECTURAL PRINCIPLES

The following principles extend the original IDS architecture.

1. **Hierarchical infrastructure:** Regional database clusters contain H-units, and H-units contain storage nodes.
2. **Subcluster responsibility:** H-units are meaningful infrastructure management and failure-compensation domains.
3. **Cross-H-unit compensation:** A different H-unit can participate in compensating for a failed storage node.
4. **Capacity-aware placement:** Data placement and recovery must account for the finite capacity of individual nodes and H-units.
5. **Hybrid redundancy:** Full replication and erasure coding can coexist across different datasets or data-placement units.
6. **Independent operation identity:** Each write receives a distinct operation reference.
7. **Reference-order separation:** Operation references and ordering positions are separate fields with separate responsibilities.
8. **Dataset-scoped ordering:** Independent datasets can maintain independent ordering domains.
9. **Concurrency as a resolution case:** Simultaneous arrivals require ordering only where the relevant consistency rules demand it.
10. **Distributed coordination:** The architecture should not depend on one permanent system-wide master.
11. **Commit-state distinction:** Receiving, ordering, executing, committing, and synchronizing a write are separate operational stages.
12. **Policy-driven durability:** The configured redundancy and consistency requirements determine the conditions under which an operation can be acknowledged.
13. **Hierarchical recovery:** Node-level and regional-level failures have distinct compensation scopes.
14. **Unified client access:** Clients interact with IDS without directly managing H-units, nodes, or regional placement.

---

# 55. UPDATED FORMAL ARCHITECTURAL DEFINITION

IDS is an independent distributed data-operation architecture designed to connect geographically separated regional database clusters into one unified logical operating environment.

Each regional database cluster contains multiple H-units, and each H-unit contains multiple individual storage nodes.

The H-unit layer provides intermediate infrastructure organization, capacity awareness, redundancy relationships, and cross-subcluster failure compensation.

IDS manages data placement across this hierarchy and supports a hybrid redundancy architecture in which full replication and erasure coding can coexist according to configured data policies.

Every write operation can receive a nine-character uppercase alphanumeric reference drawn from a namespace containing 101,559,956,668,416 possible combinations.

The operation reference identifies the write independently of its ordering position.

Where ordering is required, IDS establishes an authoritative operation sequence within the relevant dataset's ordering domain.

Independent datasets can maintain independent ordering domains rather than depending on one mandatory global write sequence.

Simultaneous write arrivals are handled through a concurrency-resolution mechanism that establishes a consistent order when the operations require one.

The architecture investigates distributed coordination without requiring one permanent system-wide master server.

IDS also maintains distinct operational states for request reception, operation identification, ordering, execution, durability, commitment, synchronization, and recovery.

Its objective is to provide unified client-facing data operations across independently managed regional infrastructure while maintaining hierarchical organization, distributed redundancy, and coordinated data integrity.

---

# 56. UPDATED PROJECT POSITION

The additions establish four major architectural extensions to the original IDS proposal.

**First — H-unit infrastructure.** The system now contains an explicit intermediate subcluster level between regional database clusters and individual storage nodes.

**Second — Hybrid redundancy and compensation.** The system supports both full replication and reconstruction-based redundancy, with H-units participating in failure compensation.

**Third — Independent operation ordering.** Writes receive unique references, while their ordering positions are established separately within the relevant ordering domains.

**Fourth — Decentralized concurrency coordination.** The architecture investigates how simultaneous writes can be resolved without requiring one permanent global master.

These extensions expand the original IDS architecture without changing its foundational objective:

**ONE LOGICAL DATA-OPERATION SYSTEM. MULTIPLE INDEPENDENT REGIONAL DATABASE CLUSTERS. HIERARCHICAL STORAGE INFRASTRUCTURE. UNIFIED CLIENT OPERATIONS.**
