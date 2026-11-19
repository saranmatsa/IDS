# EDGE-CASES.md

# IDS Edge-Case Questions and Architectural Responses

**Document purpose:** Record the six categories of edge-case questions raised during the IDS architecture discussion, the concrete scenarios behind them, and the answers supplied by the architecture's designer.

**Scope:** This document records the current architectural explanations. It is not a claim that every implementation detail has already been formally verified or benchmarked. It distinguishes between mechanisms explicitly described by the designer and extreme scenarios that belong in catastrophic-failure analysis rather than ordinary operational evaluation.

---

## 1. H-unit Communication Loss Combined with Node Failure

### Question raised

What happens if a node in one H-unit fails while that H-unit cannot communicate with another H-unit that might otherwise provide compensation?

The example discussed was:

- A node in H1 fails.
- H1 and H2 temporarily lose communication with each other.
- A request arrives that requires data normally served by the failed node.
- Another node or H-unit may be unable to coordinate with H1 during that network partition.

### Architectural response

The scenario must be evaluated against IDS's actual topology, redundancy placement, and request-routing behavior—not by assuming that a single H1–H2 communication interruption automatically removes every available copy.

The architecture distributes corresponding storage positions and data across many H-units and regions. The concrete topology used in the discussion was:

- 10 regions.
- 10 H-units in each region.
- 100 nodes in each H-unit.
- 100 H-units and 10,000 nodes in total.
- One corresponding `N2` position in each H-unit, giving 100 `N2` nodes across the complete topology.

The deliberately extreme example was the simultaneous failure of all 100 `N2` nodes while the other 9,900 nodes remained operational. That is not the same event as one node failing or one local communication path being interrupted.

For independent per-node failure probability `p`, the probability that all 100 corresponding `N2` nodes fail is:

\[
P = p^{100}
\]

The topology alone does not supply a numerical value for `p`; a measured or assumed individual-node failure rate is needed for a numerical probability. The exponent captures why the all-100 event becomes extraordinarily unlikely under an independent-failure model.

### Evaluation classification

- **Ordinary case:** one node fails; other eligible copies and routes remain available.
- **Degraded case:** a limited set of nodes or communication links fail; IDS routes through remaining eligible infrastructure.
- **Extreme correlated case:** failures or a common dependency affect many H-units at once. This belongs in a separate correlated-failure analysis.

A communication partition should be tested at realistic scope and duration. It should not be inflated into a simultaneous loss of every possible alternative unless that is the specific failure model being examined.

---

## 2. Node Identity Rotation During Request Processing

### Question raised

Could a node's identifier change while a request is actively being processed?

Potential consequences considered included:

- A request being addressed to an identity that changes mid-operation.
- A response or continuation referring to a stale identity.
- Confusion about which node is responsible for completing the operation.

### Architectural response

**Node identity does not rotate while the node is processing an active request.**

The identity-rotation rule is state-dependent:

1. A node processing a request retains its current identity.
2. The identity remains stable for the duration of that active work.
3. Once the node becomes idle, its identity may be changed according to the rotation policy.
4. The new identity is therefore applied during an idle period, not in the middle of an operation.

This directly removes the proposed active-request identity-change scenario from the normal operating model. It is incorrect to assess IDS as though identifiers can change at arbitrary moments regardless of node state.

### Evaluation classification

- **Active node:** identity remains unchanged.
- **Idle node:** eligible for identity rotation.
- **Transition:** rotation occurs only after the node has reached the idle state.

Implementation tests should verify the state transition and ensure that an active operation prevents identity rotation.

---

## 3. Permanent Failure of a Node Such as N119

### Question raised

If `N119` in H1 permanently fails, where does the data go? Does the system lose access to the data or require a complicated reconstruction process?

### Architectural response

The design described by the user has **99 other corresponding `N119` nodes holding the same data** across the other H-units in the topology.

The response is therefore straightforward:

1. Detect that the affected `N119` is unavailable.
2. Identify another available corresponding `N119` holding the required data.
3. Route the request through that available copy.
4. Continue serving the data without treating the failed physical node as the only possible source.

The key architectural point is that the failed node is not the sole holder of the data. The other corresponding copies are the alternatives.

This scenario should not be evaluated by asking how the system recovers data while ignoring the 99 other copies explicitly included in the design.

### Evaluation classification

- **Single `N119` failure:** use another available corresponding `N119`.
- **Several corresponding `N119` failures:** continue through any remaining eligible copies.
- **All 100 corresponding `N119` failures:** an extreme all-copy failure case, not the ordinary single-node failure case.

The statement that 99 other copies exist is specific to the data-placement arrangement described in this discussion. It should be represented in placement metadata and verified by implementation tests.

---

## 4. Regional Failure

### Question raised

What happens if an entire region becomes unavailable? What if multiple regions fail?

### Architectural response

The discussed topology has 10 regions. If one region fails, the other nine regions remain as alternative infrastructure.

The system should therefore treat a single-region outage as a regional failure and route eligible requests through the remaining available regions, subject to the relevant data-placement and availability rules.

A scenario in which all 10 regions are destroyed or become unavailable simultaneously is materially different. If the entire deployed infrastructure is physically destroyed, the service may cease operating. That is a catastrophic-loss scenario, not a reasonable baseline test for ordinary node or regional resilience.

### Evaluation classification

| Scenario | Architectural interpretation |
|---|---|
| One region unavailable | Nine other regions remain. |
| Several regions unavailable | Remaining regions provide the available infrastructure, subject to data placement and capacity. |
| All 10 regions unavailable | Total infrastructure loss; catastrophic business-continuity scenario. |

### Important distinction

A regional failure is not automatically a global failure. The architecture is geographically distributed precisely so that the loss of one region does not imply the loss of every other region.

Catastrophic scenarios involving a shared dependency or a physical event spanning multiple regions can be analyzed separately. They should not be presented as ordinary single-region failure behavior.

---

## 5. Concurrent Operations and Ordering

### Question raised

How does IDS handle concurrent writes or other operations that overlap in time?

Questions considered included:

- What if clients C1 and C2 issue operations concurrently?
- Does every operation require one global ordering sequence?
- How are conflicting operations resolved?
- What does a client observe when an operation has been received, ordered, executed, committed, or synchronized?

### Architectural response

The user had already specified the IDS ordering architecture. The earlier question therefore did not identify a new unanswered design problem; it revisited a mechanism already laid out in the IDS documents.

The architecture distinguishes operation references from ordering positions and uses scoped ordering rather than assuming that every operation throughout the entire infrastructure must pass through one permanent global ordering authority.

The documented operation lifecycle includes states such as:

- `received`
- `ordered`
- `executing`
- `committed`
- `synchronized`

The design also includes independent dataset ordering domains and explicit treatment of concurrent operations. Ordering scope is tied to the relevant data and operation context rather than automatically imposing one global sequence on unrelated datasets.

### Evaluation classification

This item is **already specified in the IDS architecture documents**. It should be assessed against those actual rules and invariants, not treated as though IDS has no ordering model.

Implementation-level validation should check that the documented ordering rules are preserved under concurrent requests, retries, partial failures, and recovery with pending operations.

---

## 6. What an H-unit Actually Is

### Question raised

Are H-units independent subsystems, or are they merely logical labels applied to groups of nodes?

### Architectural response

An H-unit is an **intermediate operational subcluster within a regional DB cluster**. It is not merely a name or passive grouping.

H-units have operational responsibilities involving:

- Membership.
- Node grouping.
- Capacity awareness.
- Data placement.
- Redundancy organization.
- Cross-H-unit compensation.
- Participation in failure handling and recovery.

The hierarchy is:

```text
IDS
└── Region
    └── Regional DB Cluster
        ├── H1
        │   ├── N1
        │   ├── N2
        │   └── ... N100
        ├── H2
        │   ├── N1
        │   ├── N2
        │   └── ... N100
        └── ... H10
```

The same regional structure is repeated across the 10 regions in the example topology.

An H-unit therefore has a real role in how IDS organizes nodes, tracks capacity, places data, maintains redundancy, and coordinates compensation. It is not simply an arbitrary label.

### Evaluation classification

This question was already answered by the architecture's hierarchy and responsibilities. Future analysis should use the operational definition of an H-unit rather than treating it as a purely logical tag.

---

# Consolidated Status

| # | Edge-case category | Answer supplied | Status in this discussion |
|---:|---|---|---|
| 1 | H-unit communication loss combined with node failure | Evaluate actual remaining routes and copies; the simultaneous loss of all 100 corresponding nodes is an extreme case. | Addressed at architectural level; correlated-failure modeling remains a separate analysis. |
| 2 | Node identity rotation during an active request | Identity changes only while the node is idle; active requests retain a stable identity. | Answered. |
| 3 | Permanent failure of N119 | 99 other corresponding N119 nodes hold the same data; route through another available copy. | Answered under the stated placement arrangement. |
| 4 | Regional failure | One failed region leaves nine other regions. Total loss of all regions is catastrophic infrastructure loss. | Answered at architectural level. |
| 5 | Concurrent operations and ordering | Ordering and operation lifecycle are already specified in IDS. | Already specified; validate against the documented rules. |
| 6 | H-unit definition | H-units are operational subclusters with membership, capacity, placement, redundancy, and compensation responsibilities. | Already defined. |

---

# Failure-Analysis Discipline for IDS

The six questions establish an important evaluation rule: **failure analysis must use the architecture as actually specified.** It should not silently remove existing redundancy, ignore routing alternatives, or assume that state-dependent rules do not exist.

A useful order of evaluation is:

1. Single-node failure.
2. Multiple-node failure within one H-unit.
3. Complete H-unit failure.
4. Regional network or infrastructure outage.
5. Multiple-region failure.
6. Correlated failures affecting shared dependencies.
7. Catastrophic physical destruction of the deployed infrastructure.

The ordering is a practical analysis sequence, not a claim that higher-numbered events are impossible. Ordinary operational resilience and catastrophic business-continuity planning are different analysis categories.

## Probability discipline

For independent events with individual probability `p`, the probability that all `n` specified nodes fail is:

\[
P = p^n
\]

For the example of all 100 corresponding `N2` nodes failing:

\[
P = p^{100}
\]

This is an independence-model calculation. Correlated failures require a different model because a common event can affect multiple nodes together. The topology determines the number of nodes and redundancy paths; measured failure rates and dependency data determine realistic numerical probabilities.

## Closing principle

IDS should be challenged with concrete failure scenarios, but each scenario must preserve the mechanisms the architecture actually contains:

- Do not assume an active node changes identity when rotation is idle-only.
- Do not treat one failed node as data loss when other corresponding copies exist.
- Do not treat one failed region as loss of all regions.
- Do not treat H-units as passive labels.
- Do not reopen ordering questions without consulting the ordering design already specified.

The purpose of edge-case analysis is to discover real gaps—not to manufacture failures by disregarding the architecture's existing answers.
