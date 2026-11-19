122 capability definitions • 9 capability domains • architecture diagrams

Scope: capabilities only. This document inventories capabilities already described or implied by the IDS architecture and additional capabilities identified for evaluation. It does not present these capabilities as implemented or verified. Existing/documented capabilities are numbered 1–98; researched extension candidates are numbered 99–122.

# Capability architecture map

                         IDS CAPABILITY ARCHITECTURE
                                      |
       +------------------------------+------------------------------+
       |                              |                              |
  INFRASTRUCTURE                 DATA OPERATIONS                 GOVERNANCE
       |                              |                              |
  Region / Cluster               Client Interface                Identity
  H-unit / Node                  Read / Write / Query             Authentication
  Membership / Capacity          Placement / Routing              Authorization
       |                              |                              |
       +------------------------------+------------------------------+
                                      |
                         DISTRIBUTION & DURABILITY
                                      |
                    Replication / Erasure Coding / Sync
                                      |
                         CONSISTENCY & COORDINATION
                                      |
                    Ordering / Commit / Concurrency
                                      |
                           RECOVERY & INTEGRITY
                                      |
                  Node / H-unit / Region / Verification
                                      |
                       FEDERATION & OPTIMISATION
                                      |
            Multi-owner / Adaptive Placement / Compute-to-Data

# Physical hierarchy represented by the capability set

CLIENTS
   |
IDS CLIENT INTERFACE
   |
IDS CONTROL + DATA OPERATIONS
   |
+--------------------+--------------------+--------------------+
|                    |                    |
REGION A             REGION B             REGION C
DB CLUSTER           DB CLUSTER           DB CLUSTER
|                    |                    |
+----------+         +----------+         +----------+
|          |         |          |         |          |
H1         H2        H1         H2        H1         H2
|          |         |          |         |          |
NODES      NODES     NODES      NODES     NODES      NODES

H-units are intermediate operational groups within a regional database cluster. They carry membership, capacity, placement, redundancy, and compensation responsibilities. They are not merely labels. A node failure in one H-unit may be compensated by another eligible H-unit when capacity, redundancy, and policy permit.

# A. Infrastructure and topology

Capabilities 1–10. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 1. Hierarchical infrastructure

### Capability definition

IDS organizes infrastructure as a hierarchy of regions, regional database clusters, H-units, and storage nodes. The hierarchy provides explicit scopes for placement, capacity, membership, failure handling, and operational responsibility.

### Operational behaviour

In an IDS deployment, hierarchical infrastructure is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that hierarchical infrastructure is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 2. Independent regional clusters

### Capability definition

Each region operates a distinct database cluster with its own membership, resources, health state, and locally managed data. Regional clusters participate in IDS through defined interfaces rather than being treated as interchangeable machines in one flat pool.

### Operational behaviour

In an IDS deployment, independent regional clusters is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that independent regional clusters is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 3. H-unit organization

### Capability definition

An H-unit is an intermediate operational group of storage nodes inside a regional database cluster. It is more than a naming label: it groups capacity, membership, placement, redundancy relationships, and recovery responsibilities.

### Operational behaviour

In an IDS deployment, h-unit organization is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit organization is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 4. H-unit responsibility management

### Capability definition

An H-unit can hold explicit responsibility for assigned data, fragments, replicas, or recovery tasks. IDS tracks which H-unit is responsible for each obligation and whether that obligation is currently satisfied.

### Operational behaviour

In an IDS deployment, h-unit responsibility management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit responsibility management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 5. H-unit expansion

### Capability definition

A regional cluster can add an H-unit as a distinct capacity and failure-management domain. The new group is registered, evaluated, assigned responsibilities, and integrated without requiring clients to address its physical nodes directly.

### Operational behaviour

In an IDS deployment, h-unit expansion is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit expansion is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 6. Regional expansion

### Capability definition

IDS can admit an additional regional database cluster into the overall infrastructure. The new region receives an identity, declares capabilities and constraints, joins relevant metadata and trust processes, and becomes eligible for selected data responsibilities.

### Operational behaviour

In an IDS deployment, regional expansion is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional expansion is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 7. Horizontal node expansion

### Capability definition

A regional cluster or H-unit can increase capacity by adding storage nodes. New nodes are discovered, validated, admitted, measured, and assigned work according to placement and resource policies.

### Operational behaviour

In an IDS deployment, horizontal node expansion is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that horizontal node expansion is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 8. Heterogeneous infrastructure

### Capability definition

IDS can represent nodes and clusters with different storage capacities, performance profiles, architectures, and availability characteristics. Placement and scheduling decisions can account for those differences instead of assuming uniform machines.

### Operational behaviour

In an IDS deployment, heterogeneous infrastructure is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that heterogeneous infrastructure is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 9. Independent infrastructure ownership

### Capability definition

The architecture can accommodate infrastructure operated by different organizations or administrative entities. Participation is represented through explicit identity, trust, policy, resource, and responsibility relationships.

### Operational behaviour

In an IDS deployment, independent infrastructure ownership is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that independent infrastructure ownership is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 10. Regional autonomy

### Capability definition

A region can continue eligible local operations without requiring every unrelated region to be reachable. Autonomy is bounded by data ownership, consistency policy, available replicas, and the dependencies of the requested operation.

### Operational behaviour

In an IDS deployment, regional autonomy is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional autonomy is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# B. Client access and data identity

Capabilities 11–20. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 11. Unified client interface

### Capability definition

Applications use a stable IDS-facing interface for logical data operations. The interface hides routine physical-location discovery while exposing operation outcomes and the consistency or durability level actually achieved.

### Operational behaviour

In an IDS deployment, unified client interface is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that unified client interface is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 12. Logical data addressing

### Capability definition

Clients identify datasets, partitions, records, or objects through logical identifiers rather than hard-coded node addresses. IDS resolves those identifiers to current physical locations using placement metadata.

### Operational behaviour

In an IDS deployment, logical data addressing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that logical data addressing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 13. Write operations

### Capability definition

Clients can submit new data through a common write path. IDS validates the request, resolves ownership and placement, coordinates execution, applies the configured durability policy, and returns an operation result.

### Operational behaviour

In an IDS deployment, write operations is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that write operations is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 14. Read operations

### Capability definition

Clients can retrieve data through logical identifiers. IDS resolves the relevant owner or eligible replica, evaluates consistency requirements, routes the request, validates the result as required, and returns the requested data.

### Operational behaviour

In an IDS deployment, read operations is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that read operations is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 15. Update operations

### Capability definition

Clients can modify existing records or objects through an operation that identifies the target and the intended change. IDS applies the dataset's concurrency and ordering rules and propagates the resulting state to designated replicas.

### Operational behaviour

In an IDS deployment, update operations is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that update operations is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 16. Delete operations

### Capability definition

Clients can request logical deletion. IDS records the deletion state, applies authorization and retention policy, propagates tombstones or equivalent deletion metadata, and coordinates physical reclamation when permitted.

### Operational behaviour

In an IDS deployment, delete operations is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that delete operations is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 17. Query operations

### Capability definition

Clients can submit structured queries against datasets without manually enumerating every storage node. IDS resolves the query scope and creates an execution plan over the relevant partitions and regions.

### Operational behaviour

In an IDS deployment, query operations is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that query operations is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 18. Describe and metadata requests

### Capability definition

Clients can request dataset schemas, logical properties, access conditions, version information, and selected operational metadata. The interface distinguishes public logical metadata from privileged infrastructure details.

### Operational behaviour

In an IDS deployment, describe and metadata requests is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that describe and metadata requests is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 19. Stable logical identities

### Capability definition

Dataset, partition, record, and object identifiers remain stable when physical placement changes. The identity layer separates the enduring logical name from mutable node, H-unit, and regional assignments.

### Operational behaviour

In an IDS deployment, stable logical identities is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that stable logical identities is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 20. Idempotent request handling

### Capability definition

A client can safely retry an operation using a stable idempotency key. IDS detects repeated submissions and returns or references the original outcome rather than accidentally applying the same logical operation multiple times.

### Operational behaviour

In an IDS deployment, idempotent request handling is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that idempotent request handling is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# C. Data placement and resource management

Capabilities 21–30. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 21. Distributed data placement

### Capability definition

IDS maps logical data units to physical regions, H-units, and nodes. Placement decisions account for ownership, capacity, redundancy, locality, policy, and the failure domains represented in topology metadata.

### Operational behaviour

In an IDS deployment, distributed data placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed data placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 22. Partition-aware placement

### Capability definition

Placement operates at a defined partition granularity. IDS can assign different partitions to different nodes or H-units while preserving the mapping from logical keys to their responsible partitions.

### Operational behaviour

In an IDS deployment, partition-aware placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that partition-aware placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 23. Policy-based placement

### Capability definition

Datasets can select placement policies such as hash, range, geographical, locality-aware, or hybrid placement. The policy is explicit and versioned so placement changes can be evaluated and reproduced.

### Operational behaviour

In an IDS deployment, policy-based placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that policy-based placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 24. Capacity-aware placement

### Capability definition

The placement engine considers usable capacity, reserved capacity, current utilization, expected growth, and safety margins. It avoids assigning obligations that exceed a node's or H-unit's declared and verified capacity.

### Operational behaviour

In an IDS deployment, capacity-aware placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that capacity-aware placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 25. H-unit capacity management

### Capability definition

IDS tracks capacity at the H-unit level as well as at individual nodes. It can aggregate eligible resources, account for reserved recovery headroom, and determine whether an H-unit can accept new data or repair work.

### Operational behaviour

In an IDS deployment, h-unit capacity management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit capacity management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 26. Heterogeneous resource placement

### Capability definition

Placement can account for differences in disk throughput, latency, memory, compute, network bandwidth, and failure characteristics. A data class can be matched to infrastructure whose capabilities satisfy its policy.

### Operational behaviour

In an IDS deployment, heterogeneous resource placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that heterogeneous resource placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 27. Dynamic rebalancing

### Capability definition

IDS can redistribute partitions or replicas when capacity, workload, topology, or policy changes. Rebalancing is planned as a controlled transition with progress tracking and correctness checks.

### Operational behaviour

In an IDS deployment, dynamic rebalancing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dynamic rebalancing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 28. Partition migration

### Capability definition

A partition can move between nodes, H-units, or regions while retaining its logical identity. IDS coordinates copying, verification, ownership transition, and retirement of the former placement.

### Operational behaviour

In an IDS deployment, partition migration is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that partition migration is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 29. Locality-aware routing

### Capability definition

Requests can be routed toward a nearby eligible copy or responsible region when doing so satisfies the requested consistency and authorization conditions. Locality is evaluated alongside freshness and workload.

### Operational behaviour

In an IDS deployment, locality-aware routing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that locality-aware routing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 30. Replica-aware routing

### Capability definition

The router knows which replicas are eligible for a particular read or operation. It selects among them using freshness, health, distance, load, and the consistency contract rather than treating every copy as equivalent.

### Operational behaviour

In an IDS deployment, replica-aware routing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that replica-aware routing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# D. Distributed operations and queries

Capabilities 31–38. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 31. Distributed write execution

### Capability definition

A logical write can be decomposed into coordinated actions across the nodes or regions required by its placement and durability policy. IDS tracks the stages and reports a result consistent with the actual commit condition.

### Operational behaviour

In an IDS deployment, distributed write execution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed write execution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 32. Distributed read execution

### Capability definition

A read can be served by one node or coordinated across several nodes when the requested data spans partitions or replicas. IDS resolves the required participants and assembles a valid response.

### Operational behaviour

In an IDS deployment, distributed read execution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed read execution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 33. Parallel reads

### Capability definition

Independent partition reads can execute concurrently. The query planner schedules work across eligible nodes and combines partial results while preserving the requested snapshot or consistency semantics.

### Operational behaviour

In an IDS deployment, parallel reads is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that parallel reads is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 34. Distributed query planning

### Capability definition

IDS can transform a logical query into a physical plan identifying relevant partitions, nodes, regions, filters, exchanges, and aggregation stages. The plan should minimize unnecessary data movement.

### Operational behaviour

In an IDS deployment, distributed query planning is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed query planning is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 35. Scatter-gather execution

### Capability definition

A query can fan out to a selected set of participants, collect partial results, and merge them into a response. Fan-out is limited to the partitions and replicas actually relevant to the query.

### Operational behaviour

In an IDS deployment, scatter-gather execution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that scatter-gather execution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 36. Distributed aggregation

### Capability definition

Aggregations such as counts, sums, grouping, and statistical summaries can be computed partially near the data and combined. The execution plan defines how partial results are merged and validated.

### Operational behaviour

In an IDS deployment, distributed aggregation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed aggregation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 37. Cross-regional queries

### Capability definition

A query may access data across multiple regional clusters when policy permits. IDS coordinates remote execution, transfer, partial-result handling, and region-specific access conditions.

### Operational behaviour

In an IDS deployment, cross-regional queries is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that cross-regional queries is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 38. Targeted query execution

### Capability definition

The query engine can avoid broadcasting requests to every node. It uses partition maps, indexes, predicates, and ownership metadata to select only the participants needed for a query.

### Operational behaviour

In an IDS deployment, targeted query execution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that targeted query execution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# E. Replication and synchronization

Capabilities 39–50. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 39. Data partitioning

### Capability definition

A dataset can be divided into independently placeable units. Partition boundaries, key mapping, split/merge behavior, and partition metadata are managed so operations can locate the relevant unit.

### Operational behaviour

In an IDS deployment, data partitioning is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that data partitioning is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 40. Independent replication

### Capability definition

Selected data units can have copies on multiple nodes or regions. Replication is governed by explicit placement, acknowledgement, freshness, and repair policies.

### Operational behaviour

In an IDS deployment, independent replication is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that independent replication is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 41. Hybrid redundancy

### Capability definition

IDS can use full replicas for some data and erasure-coded fragments for other data. The redundancy mode is selected according to data class, performance, durability, and resource policy.

### Operational behaviour

In an IDS deployment, hybrid redundancy is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that hybrid redundancy is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 42. Dataset-specific redundancy

### Capability definition

Different datasets can declare different redundancy objectives and mechanisms. IDS evaluates each policy separately rather than forcing every dataset into one global replication configuration.

### Operational behaviour

In an IDS deployment, dataset-specific redundancy is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset-specific redundancy is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 43. H-unit-aware redundancy

### Capability definition

Replica or fragment placement can be distributed across H-units so that one H-unit failure does not remove every required copy or fragment. Placement rules account for H-unit failure domains.

### Operational behaviour

In an IDS deployment, h-unit-aware redundancy is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit-aware redundancy is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 44. Incremental synchronization

### Capability definition

Nodes exchange changes since a known checkpoint or version instead of retransmitting the entire dataset. IDS tracks progress and detects when incremental history is no longer sufficient.

### Operational behaviour

In an IDS deployment, incremental synchronization is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that incremental synchronization is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 45. Replication streams

### Capability definition

Changes can be propagated through ordered or versioned streams associated with a dataset or partition. Streams support resumable delivery, acknowledgement tracking, and observability of replication lag.

### Operational behaviour

In an IDS deployment, replication streams is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that replication streams is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 46. Synchronization checkpoints

### Capability definition

A checkpoint records a verified point in a synchronization process. It allows participants to resume from a known state and provides a reference for reconciliation.

### Operational behaviour

In an IDS deployment, synchronization checkpoints is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that synchronization checkpoints is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 47. Synchronization retries

### Capability definition

Failed synchronization actions can be retried under bounded backoff and idempotent application rules. Retry state is visible so that transient interruption is distinguished from permanent failure.

### Operational behaviour

In an IDS deployment, synchronization retries is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that synchronization retries is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 48. Replica reconciliation

### Capability definition

When replicas diverge, IDS compares their versions or operation histories and applies the dataset's conflict and authority rules. Reconciliation must not silently overwrite a newer authoritative state.

### Operational behaviour

In an IDS deployment, replica reconciliation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that replica reconciliation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 49. Data integrity verification

### Capability definition

Checksums, hashes, authenticated metadata, or equivalent mechanisms can verify that stored or transferred data matches its expected content. Verification results are attached to the relevant data unit and operation.

### Operational behaviour

In an IDS deployment, data integrity verification is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that data integrity verification is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 50. Dataset-specific synchronization policies

### Capability definition

Each dataset can specify replication targets, allowed regions, lag objectives, checkpoint behavior, retry policy, and reconciliation rules. Policy changes are versioned and applied through controlled transitions.

### Operational behaviour

In an IDS deployment, dataset-specific synchronization policies is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset-specific synchronization policies is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# F. Consistency, ordering and concurrency

Capabilities 51–65. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 51. Dataset-specific consistency policies

### Capability definition

A dataset or data class can select an explicit consistency contract. Policies may differ in read freshness, write acknowledgement, ordering, conflict handling, and availability behavior.

### Operational behaviour

In an IDS deployment, dataset-specific consistency policies is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset-specific consistency policies is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 52. Concurrent-write resolution

### Capability definition

When multiple clients update the same ordering domain concurrently, IDS determines an authoritative outcome according to a declared rule such as serialized order, conflict detection, version checks, or application-defined merging.

### Operational behaviour

In an IDS deployment, concurrent-write resolution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that concurrent-write resolution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 53. Independent operation progress

### Capability definition

Operations in unrelated datasets or independent ordering domains can progress without waiting behind one global queue. Shared infrastructure constraints remain visible, but logical coordination is scoped to actual dependencies.

### Operational behaviour

In an IDS deployment, independent operation progress is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that independent operation progress is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 54. Dataset-scoped ordering

### Capability definition

Operations requiring a common order are assigned positions within a defined dataset, partition, key, or transaction domain. The scope is explicit so unrelated work can proceed independently.

### Operational behaviour

In an IDS deployment, dataset-scoped ordering is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset-scoped ordering is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 55. Operation identity independent of chronology

### Capability definition

Each operation receives a unique identity that supports tracking, retry, audit, and deduplication. The identity is not treated as a chronological sequence number unless a separate ordering mechanism establishes that meaning.

### Operational behaviour

In an IDS deployment, operation identity independent of chronology is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that operation identity independent of chronology is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 56. Nine-character operation references

### Capability definition

The current design exploration uses a fixed nine-character alphanumeric operation reference. The namespace size is 36^9 when using 26 uppercase letters and 10 digits; uniqueness still depends on a collision-safe allocation or detection strategy.

### Operational behaviour

In an IDS deployment, nine-character operation references is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that nine-character operation references is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 57. Distributed coordination

### Capability definition

Participants can coordinate membership, ownership, ordering, and commit decisions through a defined protocol. Coordination scope and failure behavior are specified rather than assumed from the presence of operation IDs.

### Operational behaviour

In an IDS deployment, distributed coordination is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that distributed coordination is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 58. No permanent global ordering master

### Capability definition

The intended architecture avoids requiring one permanently central system-wide ordering authority. It may still use scoped leaders, quorums, or consensus groups for specific data domains.

### Operational behaviour

In an IDS deployment, no permanent global ordering master is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that no permanent global ordering master is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 59. Domain-scoped leadership

### Capability definition

A temporary or scoped authority can coordinate a particular partition or dataset domain. Leadership is bounded by an epoch or term and is transferred safely during failure or membership changes.

### Operational behaviour

In an IDS deployment, domain-scoped leadership is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that domain-scoped leadership is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 60. Concurrency-scope isolation

### Capability definition

Conflicts are resolved within the smallest correct shared ordering domain. Independent keys or partitions can avoid unnecessary serialization when the data model permits it.

### Operational behaviour

In an IDS deployment, concurrency-scope isolation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that concurrency-scope isolation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 61. Explicit write lifecycle

### Capability definition

IDS represents write states such as received, validated, accepted, prepared, committed, replicated, and failed. The state model distinguishes progress from final success.

### Operational behaviour

In an IDS deployment, explicit write lifecycle is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that explicit write lifecycle is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 62. Durability-aware acknowledgement

### Capability definition

A client acknowledgement identifies the durability condition satisfied, such as local persistence, quorum persistence, or a configured regional redundancy threshold. It does not overstate uncompleted replication.

### Operational behaviour

In an IDS deployment, durability-aware acknowledgement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that durability-aware acknowledgement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 63. Ordered-read visibility

### Capability definition

A read can request visibility through a specified committed position or version. The serving node must demonstrate that it has applied the required prefix or route to a participant that has.

### Operational behaviour

In an IDS deployment, ordered-read visibility is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that ordered-read visibility is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 64. Committed-prefix tracking

### Capability definition

An ordering domain can record the highest contiguous committed position visible under its protocol. Gaps and unresolved operations are represented rather than skipped silently.

### Operational behaviour

In an IDS deployment, committed-prefix tracking is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that committed-prefix tracking is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 65. Operation-state tracking

### Capability definition

IDS retains the lifecycle state and relevant metadata of operations so clients, recovery processes, and administrators can determine whether an operation is pending, committed, retried, superseded, or failed.

### Operational behaviour

In an IDS deployment, operation-state tracking is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that operation-state tracking is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# G. Failure management and recovery

Capabilities 66–81. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 66. Node failure detection

### Capability definition

IDS monitors node reachability and health signals to identify suspected failures. The detector distinguishes a missed heartbeat or network partition from proof that a machine is permanently lost.

### Operational behaviour

In an IDS deployment, node failure detection is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that node failure detection is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 67. Node-state distinction

### Capability definition

Nodes have explicit lifecycle and health states such as joining, active, degraded, draining, unreachable, failed, and retired. Transitions are recorded and used by placement and routing.

### Operational behaviour

In an IDS deployment, node-state distinction is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that node-state distinction is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 68. H-unit failure compensation

### Capability definition

When a node or H-unit becomes unavailable, another eligible H-unit can assume serving, storage, or reconstruction responsibilities if redundancy, capacity, and policy permit.

### Operational behaviour

In an IDS deployment, h-unit failure compensation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that h-unit failure compensation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 69. Cross-H data recovery

### Capability definition

Recovery can use resources from multiple H-units rather than limiting repair to the failed node's original group. The system selects eligible sources and destinations and tracks the restoration of responsibility.

### Operational behaviour

In an IDS deployment, cross-h data recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that cross-h data recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 70. Capacity-bounded recovery

### Capability definition

Repair planning accounts for actual free capacity, reserved headroom, bandwidth, and workload. IDS does not assume that a surviving node can absorb unlimited data or reconstruction work.

### Operational behaviour

In an IDS deployment, capacity-bounded recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that capacity-bounded recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 71. Replica-based recovery

### Capability definition

If a valid replica remains, IDS can copy or stream data from it to a replacement destination. The source is checked for eligibility and freshness before it is treated as authoritative.

### Operational behaviour

In an IDS deployment, replica-based recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that replica-based recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 72. Erasure-code reconstruction

### Capability definition

If sufficient valid fragments remain, IDS can reconstruct missing data according to the configured coding scheme. It tracks fragment identity, placement, decode requirements, and verification of reconstructed output.

### Operational behaviour

In an IDS deployment, erasure-code reconstruction is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that erasure-code reconstruction is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 73. Multi-level recovery

### Capability definition

Recovery distinguishes node, H-unit, regional-cluster, and region-wide failure scopes. Each scope invokes an appropriate set of surviving resources and policies.

### Operational behaviour

In an IDS deployment, multi-level recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that multi-level recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 74. Regional failure compensation

### Capability definition

When a region is unavailable, another region can serve or reconstruct affected data only if the required replicas or fragments and permissions exist there.

### Operational behaviour

In an IDS deployment, regional failure compensation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional failure compensation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 75. Regional failure isolation

### Capability definition

A failure in one region should not automatically halt unrelated operations in healthy regions. Dependencies are evaluated per dataset and operation rather than through a blanket global stop.

### Operational behaviour

In an IDS deployment, regional failure isolation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional failure isolation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 76. Connectivity-failure handling

### Capability definition

IDS handles partitions and intermittent links by tracking reachability, operation state, replication lag, and authority epochs. It avoids treating network silence as conclusive proof of permanent data loss.

### Operational behaviour

In an IDS deployment, connectivity-failure handling is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that connectivity-failure handling is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 77. Returning-node reconciliation

### Capability definition

A node that returns after an outage compares its local state with authoritative versions and receives missing changes or relinquishes stale responsibility before re-entering service.

### Operational behaviour

In an IDS deployment, returning-node reconciliation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that returning-node reconciliation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 78. Incremental or full recovery

### Capability definition

Recovery can resume from a trusted checkpoint when history is complete, or fall back to a full copy/reconstruction when it is not. The choice is based on verified state and cost.

### Operational behaviour

In an IDS deployment, incremental or full recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that incremental or full recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 79. Ownership-aware recovery

### Capability definition

Repair restores not only bytes but also explicit logical responsibility for the data unit. Ownership, serving eligibility, and physical copies are updated consistently.

### Operational behaviour

In an IDS deployment, ownership-aware recovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that ownership-aware recovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 80. Recovery verification

### Capability definition

Recovered data is checked against expected checksums, versions, coding constraints, or operation history before being declared healthy.

### Operational behaviour

In an IDS deployment, recovery verification is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that recovery verification is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 81. Redundancy restoration

### Capability definition

After immediate service is restored, IDS replenishes the configured replica or fragment count and records when the dataset has returned to its target durability state.

### Operational behaviour

In an IDS deployment, redundancy restoration is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that redundancy restoration is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# H. Metadata, lifecycle and security

Capabilities 82–98. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 82. Hierarchical metadata

### Capability definition

Metadata represents regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments, ownership, and operations with explicit relationships and versioned state.

### Operational behaviour

In an IDS deployment, hierarchical metadata is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that hierarchical metadata is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 83. Placement directory

### Capability definition

A directory maps logical data identifiers to partitions and current eligible physical placements. It supports lookup, caching, invalidation, and version checks.

### Operational behaviour

In an IDS deployment, placement directory is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that placement directory is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 84. Topology metadata

### Capability definition

The system records infrastructure relationships, failure domains, network relationships, capabilities, and administrative boundaries used by placement and recovery decisions.

### Operational behaviour

In an IDS deployment, topology metadata is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that topology metadata is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 85. Membership management

### Capability definition

IDS tracks which nodes, H-units, and regional clusters are admitted participants. Membership changes are authenticated, versioned, and coordinated with placement and ownership transitions.

### Operational behaviour

In an IDS deployment, membership management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that membership management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 86. Node lifecycle management

### Capability definition

Node onboarding, activation, draining, maintenance, failure, rejoining, and retirement are represented as explicit transitions with defined effects on data responsibilities.

### Operational behaviour

In an IDS deployment, node lifecycle management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that node lifecycle management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 87. Regional lifecycle management

### Capability definition

Regional clusters can be registered, activated, degraded, isolated, drained, restored, or removed through controlled lifecycle transitions.

### Operational behaviour

In an IDS deployment, regional lifecycle management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional lifecycle management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 88. Dataset metadata

### Capability definition

Dataset metadata records schema or format information, partitioning, ownership, consistency, redundancy, retention, access policy, and relevant version state.

### Operational behaviour

In an IDS deployment, dataset metadata is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset metadata is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 89. Operation metadata

### Capability definition

Operation metadata records identity, target data, requester, timestamps or logical positions, lifecycle state, idempotency information, and durability outcome.

### Operational behaviour

In an IDS deployment, operation metadata is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that operation metadata is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 90. Resource monitoring

### Capability definition

IDS collects storage, CPU, memory, network, latency, queue, utilization, and health measurements needed for routing, placement, and recovery decisions.

### Operational behaviour

In an IDS deployment, resource monitoring is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that resource monitoring is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 91. Data lifecycle management

### Capability definition

Datasets and records can follow lifecycle policies such as creation, active use, archival, expiry, retention, legal hold, deletion, and reclamation.

### Operational behaviour

In an IDS deployment, data lifecycle management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that data lifecycle management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 92. Identity management

### Capability definition

Users, clients, services, nodes, H-units, regions, and operators have identities that can be authenticated and associated with permissions and trust relationships.

### Operational behaviour

In an IDS deployment, identity management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that identity management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 93. Authentication

### Capability definition

IDS verifies the identity of clients and infrastructure participants before allowing access or participation in privileged operations.

### Operational behaviour

In an IDS deployment, authentication is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that authentication is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 94. Authorization

### Capability definition

Policies determine which identities can read, write, update, delete, administer, replicate, or recover particular datasets and infrastructure resources.

### Operational behaviour

In an IDS deployment, authorization is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that authorization is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 95. Data encryption

### Capability definition

Data can be encrypted in transit and at rest using defined algorithms, key ownership, rotation, and access policies.

### Operational behaviour

In an IDS deployment, data encryption is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that data encryption is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 96. Regional security boundaries

### Capability definition

Policies can constrain data access, storage, processing, and replication by region or administrative boundary.

### Operational behaviour

In an IDS deployment, regional security boundaries is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that regional security boundaries is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 97. Auditability

### Capability definition

Security-sensitive and data-management operations produce tamper-evident or otherwise protected audit records suitable for investigation and compliance review.

### Operational behaviour

In an IDS deployment, auditability is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that auditability is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

## 98. Cryptographic key management

### Capability definition

IDS can integrate key generation, storage, rotation, revocation, distribution, and recovery workflows with explicit separation of key authority and data authority.

### Operational behaviour

In an IDS deployment, cryptographic key management is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that cryptographic key management is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Existing IDS-document capability: described or implied in the current IDS capability set. The degree of protocol detail varies.

# I. Extended federation and optimisation capabilities

Capabilities 99–122. This domain describes the functions exposed or coordinated by IDS at this architectural layer.

## 99. Control-plane/data-plane separation

### Capability definition

IDS can separate decisions and metadata management from bulk data movement and execution. The control plane manages identity, topology, ownership, placement, policy, and coordination; the data plane performs reads, writes, queries, replication, and reconstruction.

### Operational behaviour

In an IDS deployment, control-plane/data-plane separation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that control-plane/data-plane separation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 100. Federated infrastructure registry

### Capability definition

A registry can discover participating regions, H-units, operators, declared resources, capabilities, and service endpoints. Registry entries have provenance, freshness, and admission status.

### Operational behaviour

In an IDS deployment, federated infrastructure registry is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that federated infrastructure registry is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 101. Multi-owner trust architecture

### Capability definition

Independent operators can establish trust through authenticated identities, scoped credentials, policy evaluation, revocation, and verifiable participation records without assuming one administrative owner controls all infrastructure.

### Operational behaviour

In an IDS deployment, multi-owner trust architecture is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that multi-owner trust architecture is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 102. Data-use and ownership policies

### Capability definition

Policies can specify who owns, reads, writes, replicates, processes, exports, or deletes data, including permitted regions, operators, purposes, and restrictions on derived data.

### Operational behaviour

In an IDS deployment, data-use and ownership policies is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that data-use and ownership policies is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 103. Infrastructure accounting and resource negotiation

### Capability definition

IDS can measure contributed storage, bandwidth, compute, and recovery effort, then support quotas, reservations, commitments, and negotiated resource allocations between participants.

### Operational behaviour

In an IDS deployment, infrastructure accounting and resource negotiation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that infrastructure accounting and resource negotiation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 104. Adaptive hot/cold storage

### Capability definition

Data can move among hot, warm, and cold placement classes according to access patterns, latency objectives, cost, and lifecycle policy. Promotion and demotion preserve logical identity and verify the destination.

### Operational behaviour

In an IDS deployment, adaptive hot/cold storage is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that adaptive hot/cold storage is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 105. Hotspot detection and automatic partition balancing

### Capability definition

IDS can detect skewed request rates, overloaded partitions, and concentrated resource use. It can split, relocate, replicate, or reshape partitions under controlled migration procedures.

### Operational behaviour

In an IDS deployment, hotspot detection and automatic partition balancing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that hotspot detection and automatic partition balancing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 106. Recovery-cost-aware placement

### Capability definition

Placement can optimize not only normal access but also expected repair bandwidth, reconstruction CPU, network distance, correlated failure exposure, and time to restore redundancy.

### Operational behaviour

In an IDS deployment, recovery-cost-aware placement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that recovery-cost-aware placement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 107. Adaptive redundancy transformation

### Capability definition

A data unit can change between replication and erasure-coded layouts as workload or resource conditions change. Transformation preserves a valid recovery path until the new layout is verified.

### Operational behaviour

In an IDS deployment, adaptive redundancy transformation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that adaptive redundancy transformation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 108. Failure-correlation modelling

### Capability definition

The topology model can represent shared power, facility, network, operator, credential, software, and geographic dependencies so replicas are not mistakenly treated as independent when they share a common failure cause.

### Operational behaviour

In an IDS deployment, failure-correlation modelling is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that failure-correlation modelling is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 109. Continuous integrity scrubbing

### Capability definition

Background processes can periodically verify stored data and metadata, identify latent corruption, and repair damaged copies from trusted replicas or sufficient valid fragments.

### Operational behaviour

In an IDS deployment, continuous integrity scrubbing is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that continuous integrity scrubbing is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 110. Point-in-time snapshots

### Capability definition

A dataset can expose a stable read-only state corresponding to a defined logical point. Snapshot metadata identifies the captured versions and supports consistent reads or cloning.

### Operational behaviour

In an IDS deployment, point-in-time snapshots is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that point-in-time snapshots is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 111. Point-in-time recovery and incremental backups

### Capability definition

IDS can preserve recovery points and incremental backup histories so a dataset can be restored to a selected prior state. Backup copies are tracked separately from ordinary serving replicas.

### Operational behaviour

In an IDS deployment, point-in-time recovery and incremental backups is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that point-in-time recovery and incremental backups is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 112. Immutable records and retention enforcement

### Capability definition

Selected data can be protected against modification or premature deletion for a declared retention period or legal hold. Enforcement is applied across replicas, fragments, and derived copies within the system's control.

### Operational behaviour

In an IDS deployment, immutable records and retention enforcement is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that immutable records and retention enforcement is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 113. Storage compaction and reclamation

### Capability definition

IDS can compact fragmented storage, remove obsolete versions when safe, reclaim tombstones, expire eligible data, and reduce space amplification without violating snapshots or retention requirements.

### Operational behaviour

In an IDS deployment, storage compaction and reclamation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that storage compaction and reclamation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 114. Cooperative multi-H reconstruction

### Capability definition

Several H-units can jointly supply fragments, bandwidth, compute, and destination capacity for a single recovery task. The scheduler coordinates contributions and validates the reconstructed result.

### Operational behaviour

In an IDS deployment, cooperative multi-h reconstruction is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that cooperative multi-h reconstruction is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 115. Responsibility migration independent of physical migration

### Capability definition

IDS can transfer logical serving or ownership responsibility before every byte has moved, using epochs, fencing, read/write forwarding, and verified cutover to prevent two authorities from accepting conflicting writes.

### Operational behaviour

In an IDS deployment, responsibility migration independent of physical migration is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that responsibility migration independent of physical migration is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 116. Dataset mobility across regions

### Capability definition

A dataset or partition can change its preferred or authoritative region while preserving logical identity, history, access policy, and a controlled handoff between old and new owners.

### Operational behaviour

In an IDS deployment, dataset mobility across regions is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that dataset mobility across regions is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 117. Infrastructure self-reorganisation

### Capability definition

IDS can revise H-unit membership, placement, replicas, fragments, recovery assignments, and regional responsibilities in response to topology and workload changes while preserving defined invariants.

### Operational behaviour

In an IDS deployment, infrastructure self-reorganisation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that infrastructure self-reorganisation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 118. Continuously verifiable storage responsibility

### Capability definition

Participants can periodically demonstrate that assigned data exists, passes integrity checks, meets redundancy obligations, and remains available for eligible reads or recovery.

### Operational behaviour

In an IDS deployment, continuously verifiable storage responsibility is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that continuously verifiable storage responsibility is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 119. Compute-to-data execution

### Capability definition

Analytics and transformations can run near the data, returning compact results rather than moving entire datasets across regions. Execution is constrained by authorization, resource quotas, and data-use policy.

### Operational behaviour

In an IDS deployment, compute-to-data execution is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that compute-to-data execution is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 120. Federated dataset discovery

### Capability definition

Authorized clients and services can discover datasets across participating regions using searchable metadata, ownership information, schemas, and access conditions without exposing restricted contents.

### Operational behaviour

In an IDS deployment, federated dataset discovery is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that federated dataset discovery is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 121. Resource quotas and workload isolation

### Capability definition

IDS can enforce per-client, per-dataset, per-operator, and per-region limits for storage, concurrency, throughput, compute, and recovery bandwidth so one workload cannot consume all shared resources.

### Operational behaviour

In an IDS deployment, resource quotas and workload isolation is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that resource quotas and workload isolation is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

## 122. Recovery priority and bandwidth scheduling

### Capability definition

Repair work can be prioritized using redundancy remaining, data criticality, loss exposure, recovery time, available bandwidth, and competing workloads. Scheduling changes are observable and policy-controlled.

### Operational behaviour

In an IDS deployment, recovery priority and bandwidth scheduling is represented as an explicit system responsibility rather than an undocumented side effect. The responsible subsystem consumes the relevant dataset identity, topology state, policy version, resource measurements, and operation context. It evaluates the applicable constraints, selects an eligible action, records the decision, and exposes enough state for dependent components to proceed. The behaviour must remain consistent across the Region → regional database cluster → H-unit → node hierarchy. Where the capability changes physical placement or operational responsibility, logical data identity remains stable and metadata is updated through a controlled transition.

### Inputs, decisions, and outputs

Typical inputs include the target dataset or data unit, requester or participant identity, current ownership, topology and membership versions, available capacity, health and reachability observations, configured policy, and any relevant operation or version reference. The subsystem evaluates eligibility before acting; it must not infer that a node, replica, region, or operator is suitable merely because it is discoverable. Outputs include the selected participant or action, a recorded state transition, updated metadata where necessary, and a result that downstream routing, consistency, synchronization, or recovery components can interpret without guessing.

### Observable behaviour and verification

A deployment should expose measurable evidence that recovery priority and bandwidth scheduling is functioning. Evidence can include the selected target, policy and topology versions, resource reservation, operation state, completion condition, integrity result, and resulting metadata. Tests should cover normal operation, capacity pressure, concurrent requests, stale metadata, participant loss, retry, and recovery. A capability is not demonstrated merely because a request returns success; the system must show that the intended invariant held and that dependent components observe the same final state.

### Capability classification

Researched extension candidate: proposed addition to the IDS capability inventory; it requires explicit design and evaluation before being treated as part of the baseline.

# Capability interaction diagrams

## Write, ordering, and durability

CLIENT
  |
  v
REQUEST ID + DATASET RESOLUTION
  |
  v
OWNERSHIP + PARTITION LOOKUP
  |
  v
DATASET-SCOPED ORDERING / CONFLICT RULE
  |
  v
WRITE EXECUTION
  |
  +------> REPLICATION / ERASURE CODING
  |                    |
  |                    v
  |             DURABILITY EVALUATION
  |                    |
  +--------------------+
                       |
                       v
              COMMIT STATE + ACK

## H-unit and regional compensation

NODE / H-UNIT FAILURE
          |
          v
SCOPE + FAILURE DOMAIN IDENTIFICATION
          |
          v
DATA / FRAGMENT / OWNERSHIP IMPACT
          |
          v
ELIGIBLE SURVIVING H-UNITS OR REGIONS
          |
          v
CAPACITY + REDUNDANCY + POLICY CHECK
          |
          v
REPLICA COPY OR ERASURE RECONSTRUCTION
          |
          v
INTEGRITY + OWNERSHIP VERIFICATION
          |
          v
REDUNDANCY RESTORATION + METADATA UPDATE

## Control plane and data plane

                 IDS
                  |
       +----------+-----------+
       |                      |
  CONTROL PLANE           DATA PLANE
       |                      |
  Identity                 Read / Write
  Topology                 Query execution
  Ownership                Replication
  Placement                Erasure coding
  Policy                   Synchronization
  Ordering                 Reconstruction
  Membership               Integrity operations
       |                      |
       +----------+-----------+
                  |
          REGIONAL CLUSTERS
                  |
               H-UNITS
                  |
                NODES

## Responsibility migration

CURRENT OWNER
     |
NEW DESTINATION SELECTED
     |
DATA COPYING / STATE CATCH-UP
     |
DESTINATION VERIFIED
     |
NEW OWNERSHIP EPOCH ISSUED
     |
FENCING / FORWARDING / CUTOVER
     |
NEW OWNER SERVES REQUESTS
     |
OLD PHYSICAL COPIES RETIRED WHEN SAFE

## Federated participation

OPERATOR A          OPERATOR B          OPERATOR C
    |                   |                   |
IDENTITY + CAPABILITY DECLARATION
    |                   |                   |
TRUST / POLICY / ADMISSION EVALUATION
    |                   |                   |
    +-------------------+-------------------+
                        |
                FEDERATED REGISTRY
                        |
             DATASET-SCOPED RESPONSIBILITIES
                        |
              IDS OPERATIONS + AUDIT

# Capability index

## A. Infrastructure and topology

- 1. Hierarchical infrastructure

- 2. Independent regional clusters

- 3. H-unit organization

- 4. H-unit responsibility management

- 5. H-unit expansion

- 6. Regional expansion

- 7. Horizontal node expansion

- 8. Heterogeneous infrastructure

- 9. Independent infrastructure ownership

- 10. Regional autonomy

## B. Client access and data identity

- 11. Unified client interface

- 12. Logical data addressing

- 13. Write operations

- 14. Read operations

- 15. Update operations

- 16. Delete operations

- 17. Query operations

- 18. Describe and metadata requests

- 19. Stable logical identities

- 20. Idempotent request handling

## C. Data placement and resource management

- 21. Distributed data placement

- 22. Partition-aware placement

- 23. Policy-based placement

- 24. Capacity-aware placement

- 25. H-unit capacity management

- 26. Heterogeneous resource placement

- 27. Dynamic rebalancing

- 28. Partition migration

- 29. Locality-aware routing

- 30. Replica-aware routing

## D. Distributed operations and queries

- 31. Distributed write execution

- 32. Distributed read execution

- 33. Parallel reads

- 34. Distributed query planning

- 35. Scatter-gather execution

- 36. Distributed aggregation

- 37. Cross-regional queries

- 38. Targeted query execution

## E. Replication and synchronization

- 39. Data partitioning

- 40. Independent replication

- 41. Hybrid redundancy

- 42. Dataset-specific redundancy

- 43. H-unit-aware redundancy

- 44. Incremental synchronization

- 45. Replication streams

- 46. Synchronization checkpoints

- 47. Synchronization retries

- 48. Replica reconciliation

- 49. Data integrity verification

- 50. Dataset-specific synchronization policies

## F. Consistency, ordering and concurrency

- 51. Dataset-specific consistency policies

- 52. Concurrent-write resolution

- 53. Independent operation progress

- 54. Dataset-scoped ordering

- 55. Operation identity independent of chronology

- 56. Nine-character operation references

- 57. Distributed coordination

- 58. No permanent global ordering master

- 59. Domain-scoped leadership

- 60. Concurrency-scope isolation

- 61. Explicit write lifecycle

- 62. Durability-aware acknowledgement

- 63. Ordered-read visibility

- 64. Committed-prefix tracking

- 65. Operation-state tracking

## G. Failure management and recovery

- 66. Node failure detection

- 67. Node-state distinction

- 68. H-unit failure compensation

- 69. Cross-H data recovery

- 70. Capacity-bounded recovery

- 71. Replica-based recovery

- 72. Erasure-code reconstruction

- 73. Multi-level recovery

- 74. Regional failure compensation

- 75. Regional failure isolation

- 76. Connectivity-failure handling

- 77. Returning-node reconciliation

- 78. Incremental or full recovery

- 79. Ownership-aware recovery

- 80. Recovery verification

- 81. Redundancy restoration

## H. Metadata, lifecycle and security

- 82. Hierarchical metadata

- 83. Placement directory

- 84. Topology metadata

- 85. Membership management

- 86. Node lifecycle management

- 87. Regional lifecycle management

- 88. Dataset metadata

- 89. Operation metadata

- 90. Resource monitoring

- 91. Data lifecycle management

- 92. Identity management

- 93. Authentication

- 94. Authorization

- 95. Data encryption

- 96. Regional security boundaries

- 97. Auditability

- 98. Cryptographic key management

## I. Extended federation and optimisation capabilities

- 99. Control-plane/data-plane separation

- 100. Federated infrastructure registry

- 101. Multi-owner trust architecture

- 102. Data-use and ownership policies

- 103. Infrastructure accounting and resource negotiation

- 104. Adaptive hot/cold storage

- 105. Hotspot detection and automatic partition balancing

- 106. Recovery-cost-aware placement

- 107. Adaptive redundancy transformation

- 108. Failure-correlation modelling

- 109. Continuous integrity scrubbing

- 110. Point-in-time snapshots

- 111. Point-in-time recovery and incremental backups

- 112. Immutable records and retention enforcement

- 113. Storage compaction and reclamation

- 114. Cooperative multi-H reconstruction

- 115. Responsibility migration independent of physical migration

- 116. Dataset mobility across regions

- 117. Infrastructure self-reorganisation

- 118. Continuously verifiable storage responsibility

- 119. Compute-to-data execution

- 120. Federated dataset discovery

- 121. Resource quotas and workload isolation

- 122. Recovery priority and bandwidth scheduling

End of capability-only inventory.
