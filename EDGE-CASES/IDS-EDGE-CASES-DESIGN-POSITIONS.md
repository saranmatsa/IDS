# IDS Edge-Case Design Positions

**Project:** IDS — Independent Distributed Storage  
**Document:** IDS-EDGE-CASES-DESIGN-POSITIONS.md  
**Location:** C:\IDS\EDGE-CASES\  
**Relationship:** Companion to EDGE-CASES.md, IDS.md, IDS-A.md, IDS-B.md  
**Status:** CONCEPTUAL — OPERATIONAL DESIGN RECORD  
**Source:** Direct architecture responses recorded for six edge-case questions covering returned-node identity, binary error handling, client rosters, H-unit failure, client region affinity, and coordinated deletion.

---

# 1. RETURNED NODE IDENTITY REJECTION

## 1.1 A dead node does not return with its old identity

When a storage node dies, its data is considered deleted. Safety means the data is gone; if another party already possesses the dataset, IDS has failed because that party now owns data that is not theirs.

If the original physical node later comes back online, it does not resume with its old stable internal identity. It is treated as a fresh node:

- It receives a new node identity, not its previous one.
- It does not reclaim its former H-unit slot, data assignments, or custody obligations.
- Its old identity is retired and not reused for active infrastructure.

## 1.2 Rationale

A node that died and returned cannot be trusted to hold the current state of the data it once managed. Accepting its old identity would mean allowing a node with potentially stale, diverged, or unauthorized data to re-enter the responsibility graph. IDS prefers to provision a fresh identity and let the placement engine assign it new work from scratch.

---

# 2. BINARY ERROR HANDLING

## 2.1 Two states only

IDS error handling operates on a binary completion model:

| State | Meaning |
|-------|---------|
| 100% done | The operation completed across all required targets. |
| Not done | The operation failed. 99% is not done. Partial completion is treated as failure. |

## 2.2 No partial success

IDS does not recognize "mostly done," "degraded success," or "best-effort completion" as valid terminal states. If a write is fanned out to 30 databases and 29 acknowledge but 1 does not, the operation is not done. It is a failure and must be retried, re-planned, or rejected.

## 2.3 Improbability of edge cases

Because IDS requires 100% completion across its placement set, edge cases that depend on partial completion, dangling writes, or ambiguous commit states are treated as extremely improbable by design. The system is built so that these states should not occur in normal operation.

---

# 3. CLIENT NODE ROSTER ASSIGNMENT

## 3.1 Clients receive fixed node rosters

When a client joins IDS, it is assigned a roster of nodes. Example:

- 5 clients, 10 nodes.
- Each client receives the same 10 nodes.
- Those 10 nodes are spread across varying regions (different regions), but the node set is the same for each client.

## 3.2 Stable roster semantics

The client does not get random nodes. It gets a specific set. The roster is not recalculated per request; it is assigned at join time and remains stable unless the infrastructure changes formally.

## 3.3 Regional variation within a stable roster

The same 10 nodes can be topologically diverse — they may reside in different regions — but the client's view of its responsible nodes is fixed. IDS uses this roster for read/write routing rather than discovering nodes per operation.

---

# 4. H-UNIT FAILURE AND FALLBACK REPLICATION

## 4.1 H-unit failure is an extreme, improbable event

An H-unit going down is not an ordinary operational expectation. It matters enormously when it happens, but it is highly improbable because:

- The infrastructure is decentralized.
- A single organization or actor would need to destroy the same H-unit across its member nodes simultaneously.
- Such coordinated destruction is treated as an out-of-scope catastrophic event.

## 4.2 Fast replication to fallback H-units

When an H-unit does fail, replication to a fallback H-unit happens quickly. The fallback H-unit is selected because it has less current use and available capacity — "the data makes space for more."

Replication is not slow by design. IDS assumes the fallback path has headroom and can accept the displaced data promptly.

---

# 5. CLIENT REGION AFFINITY

## 5.1 Clients do not abruptly change regions

A client is assigned to write through specific nodes in a specific region. It does not abruptly switch from Region A to Region B because of a transient network event or a load-balancing whim.

## 5.2 Near-to-far, not random

When IDS needs to route a client's operation, it moves from near to far — checking the assigned nodes first, then expanding outward only if necessary. It is not random selection, and it is not an unpredictable hop to a different region.

The client's assigned region and nodes are stable. Region changes happen only through formal reassignment, not through implicit failover during a request.

---

# 6. SIMULTANEOUS DELETION ACROSS THE FEDERATION

## 6.1 IDS is a filesystem integrated into HX-241/Xarnx

IDS is not a conventional database with separate backup and deletion semantics. It is a filesystem designed to run inside the HX-241 / Xarnx ecosystem.

## 6.2 Coordinated millisecond deletion

When a deletion request is issued, every participant that holds the dataset receives the request at approximately the same time. All participants delete their copies simultaneously — within milliseconds.

There is no staggered deletion, no grace period where some regions retain the data while others have removed it, and no partial retention window.

## 6.3 Deletion is not an issue

Because deletion is a broadcast, synchronous operation across all holders, the problem of one domain keeping data it should not own does not arise. Everyone deletes at once, or the operation is not done (per the binary error-handling model in Section 2).

---

# 7. CONSOLIDATED DESIGN-POSITION MATRIX

| # | Edge case | IDS design position |
|---|-----------|---------------------|
| 1 | Dead node returns with old identity | Rejected; fresh node identity assigned; old identity retired |
| 2 | Partial write completion | Not done = failure; 99% is not 100%; binary completion only |
| 3 | Client node roster at join | Fixed roster of specific nodes across varying regions; stable |
| 4 | H-unit failure | Highly improbable; fast replication to fallback H-unit with spare capacity |
| 5 | Client changes regions mid-request | No abrupt region change; near-to-far routing; formal reassignment only |
| 6 | Deletion across federation | All participants delete simultaneously within milliseconds; no staggered retention |

---

# 8. OPEN DESIGN NOTES

The following behaviors are described above but require formal specification before implementation:

- The exact mechanism for retiring and replacing a node identity after death.
- How the client roster is communicated and updated if a node in the roster dies.
- The criteria for selecting a fallback H-unit (capacity, region distance, current load).
- The millisecond synchronization method for coordinated deletion across regions (IDS standard time, broadcast protocol, or similar).
- What happens when a client's entire assigned roster is unavailable — whether near-to-far expansion is permitted or the request fails.

---

# END OF DOCUMENT
