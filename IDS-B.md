# IDS-B — Dynamic Node Identity and Cryptographic Request Handling

**Project:** IDS — Independent Distributed Storage  
**Document:** IDS-B.md  
**Relationship:** Continuation of IDS-A.md  
**Scope:** Dynamic rotation of node-facing identifiers, request-trace protection at database ingress, and periodic rotation of associated identifiers  
**Status:** CONCEPTUAL — EXPERIMENTAL DESIGN  
**Source:** Handwritten architecture note supplied for this document, together with the clarification that node identifiers remain necessary for IDS request handling but should not remain permanently exposed in a traceable form.

---

## 1. Purpose

This document records an additional IDS mechanism: a storage node must remain identifiable to the infrastructure so that IDS can route requests, maintain node membership, resolve storage responsibility, and coordinate operations. However, the same externally visible node identifier should not necessarily remain unchanged indefinitely. A persistent identifier can become a stable observation point through which an outside observer may correlate a node's activity over time.

The proposed mechanism separates the node's enduring internal identity from the identifier presented or used in a particular network-facing context. The internal identity remains stable for infrastructure management. The network-facing identifier can be changed periodically according to a rotation policy.

The handwritten proposal contains three connected ideas:

1. A request reaches a database (DB) component, and its trace is directed into cryptographic processing as soon as it reaches that component.
2. A node identifier is still given or used; the system does not remove node identification from request handling.
3. Identifiers or numbers, and the handwritten term “RDs,” are shifted periodically — potentially daily or at another interval — to reduce the opportunity for tracking.

The central design principle is therefore **stable internal node identity with periodically changing network-facing identity**.

## 2. Relationship to IDS-A

IDS-A establishes a hierarchy of regional database clusters, H-units, and storage nodes. It also specifies that node metadata includes a node identifier, H-unit identifier, regional database identifier, capacity information, operational state, assigned data, redundancy information, health information, and recovery state. IDS-A further describes operation references, dataset-scoped ordering, write coordination, read resolution, and failure compensation.

This proposal does not replace those mechanisms. It adds an identity-privacy layer around the way a node is identified during network communication and request handling.

The distinction is important:

- IDS still needs to know which actual node is responsible for a request.
- IDS still needs stable metadata relationships between a node, its H-unit, its regional DB, and its assigned data.
- Other participants do not necessarily need to see one permanent identifier for that node on every interaction.
- Changing a network-facing identifier must not accidentally create a new physical node, erase membership, change data ownership, or invalidate committed operations.

The proposal is an addition to the existing IDS identity model, not a replacement for it.

## 3. Identity Separation

IDS-B introduces a conceptual separation between an internal node identity and a rotating node identifier.

### 3.1 Stable internal identity

Each participating storage node retains a stable internal identity. IDS uses this identity to maintain its authoritative infrastructure records. It is the reference through which the system associates the node with its H-unit, regional database, capacity, health, data placement, redundancy relationships, and recovery responsibilities.

The stable identity is not intended to be routinely exposed to every remote participant. Access to it should be limited to the authenticated infrastructure components that require it.

### 3.2 Rotating network-facing identifier

A rotating node identifier (RNI) is a temporary or time-bounded identifier associated with the stable internal node identity. It can be used in network-facing discovery, addressing, or request-routing contexts where a node identifier is required.

The RNI changes according to a configured rotation policy. A new RNI is associated with the same internal node identity. The physical node and its IDS responsibilities remain unchanged.

### 3.3 Three identity scopes

```text
                    STORAGE NODE
                         |
             +-----------+-----------+
             |                       |
             v                       v
      STABLE INTERNAL ID       NETWORK-FACING ID
      (persistent)             (rotates periodically)
             |                       |
             |                 RNI generation
             |                       |
             +----------+------------+
                        |
                        v
               AUTHENTICATED MAPPING
                        |
                        v
                IDS REQUEST HANDLING
```

The diagram represents a conceptual relationship. The mapping must be protected and available only to authorized IDS components.

A third scope is the request or operation identity. IDS-A already distinguishes operation references from node identifiers and ordering positions. A request's operation reference identifies that operation; it must not be confused with the current RNI of the storage node handling it.

| Identity or reference | Function | Expected lifetime |
|---|---|---|
| Stable internal node identity | Infrastructure identity and authoritative metadata association | Persistent, subject to node retirement |
| Rotating node identifier (RNI) | Network-facing identification or routing alias | Bounded by rotation policy |
| Operation reference | Identifies an individual operation | Stable for that operation's lifecycle and required audit/retention period |
| H-unit identifier | Identifies the node's H-unit | Stable unless infrastructure membership is formally changed |
| Regional DB identifier | Identifies the regional database context | Stable unless infrastructure topology changes |
| Dataset identifier | Identifies the dataset being accessed | Stable according to dataset lifecycle |

## 4. Request Arrival and Cryptographic Processing

The handwritten note places cryptographic processing immediately after a request reaches the DB. This document records that ordering as a design requirement to investigate: the request's identifying trace or routing metadata should enter the relevant cryptographic protection step at the DB ingress boundary, before it is unnecessarily propagated through later request-handling stages.

The word “trace” in the note is interpreted here as the request-associated identifying or routing information that could expose a stable node identity or communication relationship. It does not mean that IDS should destroy the operational information needed to process the request. Authorized components must still be able to authenticate the request, resolve its destination, apply access controls, and route it correctly.

### 4.1 Conceptual ingress flow

```text
CLIENT / PEER
     |
     |  request
     v
REGIONAL DATABASE (DB) INGRESS
     |
     v
CAPTURE REQUEST CONTEXT
     |
     v
CRYPTOGRAPHIC PROTECTION
     |   - protect sensitive identity/routing metadata
     |   - authenticate or validate the relevant context
     |   - avoid forwarding unnecessary stable identifiers
     v
IDENTITY RESOLUTION
     |
     v
MAP CURRENT RNI TO AUTHORIZED INTERNAL NODE
     |
     v
IDS REQUEST HANDLING
     |
     v
H-UNIT RESOLUTION
     |
     v
STORAGE NODE / DATA OPERATION
```

This is a conceptual sequence, not a selected cryptographic protocol. The exact cryptographic construction, key management, token format, and placement of individual verification operations remain open design questions.

### 4.2 Preserve request functionality

Cryptographic processing must protect identifying information without preventing IDS from doing its work. The request-handling path still needs sufficient authenticated information to determine:

- whether the requester is authorized;
- which dataset or resource is being requested;
- which ordering domain applies, if the operation requires ordering;
- which current node-facing identifier is being referenced;
- which internal node is authorized to handle the request;
- which H-unit and regional DB contain the relevant responsibility;
- which redundancy and durability policies apply.

The system should expose only the information required at each stage. A component that only needs to forward a request should not automatically receive the permanent internal identity if a temporary identifier or protected token is sufficient.

### 4.3 What cryptographic processing does not mean

The note's reference to cryptography should not be interpreted as a complete protocol specification. Encryption alone does not guarantee anonymity, and encrypting a node identifier does not automatically prevent correlation through timing, traffic volume, network addresses, repeated communication patterns, or other metadata. Authentication, authorization, confidentiality, integrity, key rotation, and metadata minimization are related but distinct requirements.

IDS-B records the architectural direction: apply cryptographic protection to request-associated identifying information at the DB ingress stage, while preserving authorized identity resolution for request execution.

## 5. Periodic Identifier Rotation

A node should not have to expose the same network-facing identifier indefinitely. IDS can periodically replace the RNI associated with a stable internal node identity.

### 5.1 Illustrative rotation

```text
STABLE INTERNAL NODE: NODE-0001

Period 1:  RNI-184729
     |
     | rotation event
     v
Period 2:  RNI-928415
     |
     | rotation event
     v
Period 3:  RNI-572193
     |
     | rotation event
     v
Period 4:  RNI-306841

All four RNIs refer to the same authenticated internal node,
subject to their validity windows and protected mapping records.
```

The example identifiers are illustrative only. They do not prescribe a specific identifier length, character set, generation algorithm, or uniqueness mechanism.

### 5.2 Rotation does not mean node replacement

When an RNI changes:

- the physical storage node remains the same;
- the stable internal identity remains the same;
- the node's H-unit membership remains the same unless separately changed;
- the regional DB association remains the same unless separately changed;
- storage capacity and assigned data remain associated with the same internal node;
- redundancy relationships and recovery responsibilities remain intact;
- existing committed operation references do not change;
- dataset ordering positions do not change.

Only the network-facing identifier and the protected mapping that resolves it are updated.

### 5.3 Rotation intervals

The handwritten note explicitly considers periodic changes and gives daily rotation as an example. The broader discussion also considers seconds, minutes, hours, days, and months. IDS should therefore treat the interval as a configurable policy rather than fixing one interval for every node.

| Candidate interval | Possible use | Design consideration |
|---|---|---|
| Seconds | Very short-lived aliases or high-sensitivity sessions | High update and synchronization frequency |
| Minutes | Short-lived network-facing identifiers | Requires efficient propagation and transition handling |
| Hours | Frequent operational rotation | Balances identifier lifetime and coordination load |
| Daily | The explicit example in the handwritten note | Simpler initial policy to investigate |
| Weeks | Lower-frequency rotation | Longer period during which an alias remains reusable |
| Months | Long-lived operational alias | Less rotation and less frequent coordination |

These are candidate intervals, not established performance results. IDS may eventually support different policies for different node classes, regions, or communication contexts. A node's policy should be explicit and consistently enforced by the components that resolve its current identifier.

## 6. Rotation of Associated Numbers and “RDs”

The handwritten note says: “Shift numbers and RDs periodically,” and indicates that this may be done daily to avoid tracking. The abbreviation “RDs” is preserved exactly as written because the source note does not define its expansion.

For the purposes of this addendum, the proposal is recorded as follows:

1. The network-facing node number or identifier can be changed periodically.
2. The associated “RDs” mentioned in the note may also be shifted periodically if they are identifiers or values exposed in a way that permits persistent correlation.
3. The rotation policy may use a daily interval or another configured period.
4. Any associated rotation must preserve the internal mappings required for correct request handling.

The precise meaning of “RDs,” their role in IDS, whether they are externally visible, and whether they should rotate together with RNIs remain to be specified. This document does not silently assign an expansion or invent a data model for that abbreviation.

If the relevant RDs are infrastructure identifiers that must remain stable for internal consistency, they should not be changed merely because the RNI changes. If they are externally visible aliases or request-facing references whose persistence creates tracking risk, a separate rotation policy can be evaluated. The distinction must be established before implementation.

## 7. Rotation and Request Continuity

A rotating identifier must not make a valid request fail simply because the identifier changes while the request is travelling through the network.

A transition mechanism is therefore required. During a rotation event, IDS may need to recognize the new RNI and, for a limited transition period, resolve the immediately previous RNI for requests that were already issued or are still in flight.

```text
                 ROTATION TIME
                       |
          +------------+------------+
          |                         |
          v                         v
     NEW RNI ACTIVE            PREVIOUS RNI
                              TRANSITION WINDOW
          |                         |
          +------------+------------+
                       |
                       v
             AUTHENTICATED RESOLUTION
                       |
                       v
              SAME INTERNAL NODE
```

The transition window should be bounded. An old identifier should not remain valid indefinitely, because indefinite validity would weaken the purpose of rotation. The duration and exact acceptance rules are implementation decisions.

The system should distinguish at least these states for a rotating identifier:

- scheduled;
- active;
- superseded but temporarily resolvable;
- expired;
- revoked.

A request carrying an expired or revoked identifier should not be accepted merely because the identifier once referred to a valid node. The system must validate the identifier's validity period and its cryptographic or authenticated association.

## 8. Proposed End-to-End Request Flow

The following sequence combines the handwritten diagram and the existing IDS request model.

```text
(1) CLIENT / PEER
       |
       | request addressed using current network-facing identity
       v
(2) REGIONAL DB INGRESS
       |
       | request reaches DB
       v
(3) CRYPTOGRAPHIC PROCESSING
       |
       | protect identifying trace / routing context
       | validate required authentication material
       v
(4) RNI RESOLUTION
       |
       | current RNI -> stable internal node identity
       v
(5) IDS REQUEST HANDLING
       |
       | resolve dataset and operation requirements
       v
(6) REGIONAL DB / H-UNIT RESOLUTION
       |
       v
(7) STORAGE NODE
       |
       | execute authorized read/write or other operation
       v
(8) RESPONSE PATH
       |
       | return result without unnecessarily exposing
       | permanent internal identity
       v
(9) CLIENT / PEER
```

The flow does not replace IDS-A's write ordering, durability, redundancy, read consistency, or failure compensation. For example, a write still receives its operation reference and is processed according to its dataset's ordering and durability policies. The operation reference remains distinct from the rotating node identifier.

## 9. Relationship to IDS-A Metadata

IDS-A Section 46.1 requires node metadata to include a node identifier, H-unit identifier, regional DB identifier, capacity, operational state, assigned data, replica and fragment information, health information, and recovery state. IDS-B refines the identity part of that metadata by distinguishing the stable internal identity from the current network-facing RNI.

A conceptual node record may therefore look like this:

```text
NODE RECORD
|
+-- Stable internal node identity
+-- Current rotating node identifier (RNI)
+-- RNI validity start
+-- RNI validity end
+-- Previous RNI transition state (if applicable)
+-- H-unit identifier
+-- Regional DB identifier
+-- Storage capacity / available capacity
+-- Operational state
+-- Assigned data
+-- Replica / encoded-fragment information
+-- Health information
+-- Recovery state
+-- Rotation policy identifier
```

The permanent internal identity remains the authoritative key for the infrastructure record. The RNI is a time-bounded alias associated with that record. The mapping must be protected against unauthorized reading or modification.

## 10. Security and Privacy Objectives

The intended privacy benefit is to make long-term tracking based solely on one persistent node-facing identifier more difficult. An observer who sees an RNI during one validity period should not automatically be able to infer the next RNI or link it to the same node.

The design should pursue the following objectives:

1. **Identifier unlinkability:** A new RNI should not be trivially derivable from the previous RNI by an outside observer.
2. **Authenticated resolution:** Only authorized IDS components should be able to resolve an RNI to the stable internal identity.
3. **Integrity:** An attacker should not be able to substitute an RNI mapping and redirect requests to an unauthorized node.
4. **Confidentiality:** Sensitive identity mappings and request metadata should be protected in transit and at rest where appropriate.
5. **Bounded validity:** Each RNI should have a defined validity period and lifecycle state.
6. **Replay resistance:** Expired or previously used request material should not be accepted as a fresh authorized request.
7. **Continuity:** Legitimate requests in flight during rotation should have a defined transition path.
8. **Metadata minimization:** Components should receive only the identity information needed for their function.

These are design objectives, not claims that the proposed mechanism already achieves them. Identifier rotation is one privacy measure. It does not by itself conceal a node's IP address, geographic location, traffic volume, timing patterns, or communication relationships. Those are separate network and metadata privacy questions.

## 11. Operational Requirements

A working implementation will need to define how new RNIs are generated, distributed, activated, resolved, expired, and revoked. It will also need to define how rotation behaves when nodes are offline, when regional DBs are temporarily disconnected, and when a node is recovering after failure.

At minimum, the system needs:

- a secure RNI generation mechanism;
- uniqueness controls within the relevant namespace and validity period;
- authenticated binding between each RNI and its internal node identity;
- a rotation schedule or policy;
- a way to distribute updated mappings to authorized resolvers;
- a bounded old-identifier transition period;
- revocation and emergency rotation procedures;
- protection against replay and unauthorized mapping changes;
- auditable internal rotation events without exposing permanent identities unnecessarily;
- recovery behaviour when a node misses a scheduled rotation while offline.

Rotation should not be treated as a reason to alter H-unit membership or storage placement. If a node is moved to another H-unit, that is a separate infrastructure membership operation and should be recorded independently.

## 12. Failure and Edge Cases

### 12.1 Node is offline during rotation

If a node is offline when its RNI is due to rotate, IDS needs a policy for whether the new identifier is activated immediately, on reconnection, or after a controlled re-registration. The old identifier should not be accepted indefinitely. The node must re-establish authenticated participation before receiving a valid active RNI.

### 12.2 Request arrives during rotation

A request may arrive while the old RNI is being superseded. The resolver should apply a deterministic rule: accept the current RNI, or accept the immediately previous RNI only if it remains within the defined transition window and passes authentication and validity checks.

### 12.3 Old identifier is replayed

A previously valid RNI must not by itself authorize a new operation. Request authentication, freshness checks, authorization, and operation-level replay protection remain necessary.

### 12.4 Mapping service is unavailable

If a DB ingress component cannot securely resolve an RNI, it should not guess the underlying node. IDS needs a defined failure response or a trusted, appropriately replicated mapping-resolution path. This issue is particularly relevant to IDS's regional and geographically distributed architecture.

### 12.5 Node is permanently retired

When a node is permanently retired, its active RNI must be revoked and its mapping must no longer authorize requests. The stable internal identity remains in the historical infrastructure record according to IDS retention and audit requirements, but it must not be treated as an active participant.

### 12.6 H-unit or regional DB changes

If a node changes H-unit or regional DB association, the infrastructure metadata must be updated through the normal membership-management process. RNI rotation alone must not silently change those relationships.

## 13. Design Decisions Still Open

The handwritten note establishes the direction but does not settle every implementation detail. The following decisions remain open:

| Question | Current status |
|---|---|
| Should the stable internal node identity ever be exposed outside trusted IDS components? | Minimize exposure; exact boundary to be specified |
| What is the exact RNI format and length? | Unspecified |
| How is each RNI generated? | Unspecified |
| Which cryptographic construction protects the request trace? | Unspecified |
| What exactly does “RDs” mean in the handwritten note? | Abbreviation retained; expansion unresolved |
| Do RDs rotate with RNIs or under a separate policy? | Unspecified |
| What is the default rotation interval? | Daily is an example; no final default selected |
| How long is the previous-RNI transition window? | Unspecified |
| How are mappings distributed across regional DBs? | Unspecified |
| How does rotation work during a WAN partition? | Requires design |
| How are replayed or expired identifiers rejected? | Required; mechanism unspecified |
| What metadata is visible to peers and intermediaries? | Requires a visibility model |

These open points should be resolved through protocol design and experiments rather than being treated as already implemented capabilities.

## 14. Experimental Evaluation Plan

The mechanism should be evaluated as part of IDS request handling. Useful experiments include:

1. **Basic rotation:** Confirm that one node can change its RNI while retaining the same stable internal identity and infrastructure metadata.
2. **Request continuity:** Send requests immediately before, during, and after rotation and verify the defined transition behaviour.
3. **Old-identifier rejection:** Confirm that an expired or revoked RNI cannot authorize new requests.
4. **Mapping integrity:** Attempt unauthorized mapping changes and verify that they are rejected.
5. **Replay testing:** Replay captured request material and verify that freshness and authorization controls prevent duplicate unauthorized execution.
6. **Regional propagation:** Measure how quickly authorized regional DB components learn the new RNI mapping.
7. **WAN partition:** Rotate an identity while regional connectivity is interrupted, then verify safe reconciliation after reconnection.
8. **Offline node:** Keep a node disconnected across a rotation event and test its rejoining procedure.
9. **Tracking analysis:** Compare how easily an observer can correlate activity before and after rotation using only the network-visible identifiers. Separately test whether timing, traffic patterns, or other metadata still reveal correlation.
10. **Operational overhead:** Measure mapping updates, cryptographic processing cost, request latency, storage requirements, and control-plane traffic under different rotation intervals.

Results should distinguish successful request processing from actual privacy improvement. A rotation test that proves only that an identifier changes does not prove that observers cannot correlate the node.

## 15. Summary of the Proposal

IDS-B adds dynamic node-facing identity to the existing IDS architecture. It preserves the node identity required for infrastructure management and request handling while allowing the identifier visible to network participants to change periodically.

When a request reaches a regional DB, its identifying or routing trace is intended to enter cryptographic protection at the DB ingress stage. IDS then resolves the currently valid network-facing identifier through an authenticated mapping to the stable internal node identity and continues normal request handling. The request can proceed through dataset resolution, ordering requirements, regional DB and H-unit resolution, and storage-node execution.

The handwritten proposal also calls for periodically shifting “numbers and RDs,” with daily rotation given as one possible interval. The meaning of “RDs” remains unresolved and is intentionally preserved as written pending clarification.

The key architectural separation is:

```text
STABLE INTERNAL IDENTITY
          |
          | protected mapping
          v
ROTATING NETWORK-FACING IDENTITY
          |
          | request reaches DB
          v
CRYPTOGRAPHIC REQUEST-TRACE PROCESSING
          |
          v
AUTHENTICATED IDENTITY RESOLUTION
          |
          v
EXISTING IDS REQUEST HANDLING
          |
          v
REGIONAL DB -> H-UNIT -> STORAGE NODE
```

**Core principle:** Keep the node identifiable to IDS; avoid making one permanent network-facing identifier the enduring handle by which outside observers can track it.

---

**End of IDS-B.md**
