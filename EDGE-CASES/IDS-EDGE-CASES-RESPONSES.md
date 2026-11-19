# IDS Edge-Case Responses — Operational Model and Failure Behavior

**Project:** IDS — Independent Distributed Storage  
**Document:** IDS-EDGE-CASES-RESPONSES.md  
**Location:** C:\IDS\EDGE-CASES\  
**Relationship:** Companion to EDGE-CASES.md, IDS.md, IDS-A.md, IDS-B.md  
**Status:** CONCEPTUAL — OPERATIONAL DESIGN RECORD  
**Source:** Direct architecture responses recorded for six edge-case questions covering partition behavior, time, integrity, identity rotation, fallback, and topology diversity.

---

# 1. H-UNIT PARTITION AND WRITE ACCEPTANCE

## 1.1 Writes during partition

When an H-unit loses contact with its region but remains internally connected, its nodes **do accept writes**. Writes are issued across nodes, across H-units, and across different regions simultaneously. The data is heavily segregated by design: storing everything in one region concentrates failure risk, so IDS distributes across different H-units and regions to lower the probability that a single failure event removes all access.

When many nodes in an H-unit die and the operation is no longer efficient, the H-unit becomes non-functional for that workload. The system does not force inefficient operations through a degraded H-unit.

## 1.2 Type-safe AI decision model for architecture control

IDS uses a type-safe AI type model (described as an 8B-scale decision model) for architecture control. Key properties:

- **Access model:** The model has access to computation and architecture control signals, but **zero access to stored data**. It sees control-plane state, not data-plane content.
- **Role:** It makes decisions about H-unit health, operation efficiency, node evacuation, and infrastructure reconfiguration when many nodes die or operations degrade.
- **Separation:** Data operations and architecture decisions are decoupled. The decision model never touches the bytes it is deciding where to place or how to move.

## 1.3 H-unit non-functionality threshold

An H-unit is treated as non-functional when:
- Too many member nodes have died;
- The remaining operation path is no longer efficient;
- The type-safe decision model determines local continuation is unsafe or uneconomic.

Non-functional does not mean destroyed. The H-unit can return to service when nodes are replaced or capacity is restored.

---

# 2. IDS STANDARD TIME

## 2.1 Time model

IDS uses an **IDS standard time** — a proprietary time basis distinct from wall-clock time, logical clocks, or hybrid logical clocks used by other distributed systems. The rationale: existing time models are "too messed up" for IDS's federated, multi-owner, geographically distributed requirements.

## 2.2 Implications

- Operation ordering, commit timestamps, and durability windows are expressed in IDS standard time, not local node wall-clock.
- Cross-region and cross-H-unit coordination uses IDS standard time as the reference frame.
- Clock drift between nodes is absorbed by the IDS time layer rather than treated as a consistency anomaly requiring external resolution.

The exact construction of IDS standard time (tick source, drift tolerance, synchronization mechanism) is specified elsewhere and is not treated as an off-the-shelf NTP or logical-clock problem.

---

# 3. INTEGRITY AND ANOMALY DETECTION

## 3.1 Out-equals-in byte accounting

IDS enforces an **out=in** property at the infrastructure level: the system knows how many bytes are sent and expects the same number to be received or acknowledged. This is not a checksum of content; it is a volume accounting boundary.

- Every write, replication, and recovery transfer records bytes-out at the source and bytes-in at the destination.
- A mismatch triggers investigation before the operation is declared complete.

## 3.2 Daily integrity checks

IDS performs daily checks on stored representations. The purpose is not to re-verify every byte every day, but to detect when "something feels fishy" — a silent mismatch, a stale replica, or an unexpected state change that volume accounting alone did not catch.

When the daily check or the out=in boundary detects an anomaly:
- The affected representation is flagged;
- The system does not silently proceed as if nothing happened;
- Recovery or re-verification is initiated according to the declared durability policy.

## 3.3 "Fishy" detection

IDS includes a heuristic or anomaly-detection path for states that do not match a clean failure mode but also do not look healthy. This covers:
- Partial writes where the source says complete but the destination volume is off;
- Replicas that respond quickly but with suspiciously old data;
- Nodes that report healthy but have not participated in a cross-check window.

The response to a "fishy" signal is operational: quarantine, re-verify, or route around the suspect node until the state is explained.

---

# 4. ROTATING IDENTIFIERS AND DATABASE RETIREMENT

## 4.1 Multi-database simultaneous writes

IDS writes to approximately **30 databases at once** for a single logical operation when the durability and placement policy requires wide distribution. This is not sequential replication to 30 targets; it is a concurrent write fan-out across the selected placement set.

## 4.2 Temporary retirement during rotation

When an identifier rotation (RNI change per IDS-B) or equivalent infrastructure change occurs:
- The affected database or node is **retired temporarily**, not immediately destroyed;
- It is removed from the active placement set;
- New writes are directed to the rotating replacements;
- The retired database comes back online later, once the rotation window has closed and its identity mapping is re-established.

This avoids cutover failures where a client or in-flight request addresses a database that no longer exists in its old form.

## 4.3 Relationship to IDS-B

This behavior complements IDS-B's rotating network-facing identifier model. The stable internal identity persists; the network-facing alias retires temporarily during the transition and returns when valid again.

---

# 5. READ FALLBACK

## 5.1 Automatic closest-replica fallback

When a client read cannot be served by the primary or preferred node:
- IDS automatically falls back to the **next closest** eligible replica or fragment source;
- "Closest" is evaluated by IDS standard time, topology distance, and current health — not purely by network hop count;
- The fallback is transparent to the client unless the client has requested a strict consistency profile that forbids fallback.

## 5.2 Fallback constraints

A fallback target must satisfy:
- Valid current RNI or stable internal identity resolution;
- Holding a verified copy or sufficient fragments for reconstruction;
- Not being in a temporary-retirement or drained state;
- Meeting the client's requested consistency profile.

If no target satisfies these, the read is degraded or refused rather than served from an untrusted source.

---

# 6. TOPOLOGY DIVERSITY

## 6.1 Different locations, different H-units, everything different

IDS placement does not assume similarity between infrastructure components. The design principle is:
- Different physical locations;
- Different H-units;
- Different regions;
- Different failure domains;
- Different administrative domains where federated.

The goal is to avoid correlated placement where two copies of the same logical data share a power domain, network segment, operator, or geographic event.

## 6.2 Consequence for failure probability

Because copies are spread across dissimilar infrastructure, the failure probability of the complete data service is lower than if all copies lived in one region or one H-unit. A single regional outage, H-unit failure, or node mortality event cannot remove all representations simultaneously.

---

# 7. CONSOLIDATED EDGE-CASE MATRIX

| # | Edge case | IDS response |
|---|-----------|--------------|
| 1 | H-unit partitioned from region, internally connected | Writes accepted across nodes/H-units/regions; data heavily segregated |
| 2 | Many nodes die, H-unit inefficient | H-unit becomes non-functional; type-safe AI decision model (8B, compute-only, no data access) governs architecture control and evacuation |
| 3 | Clock drift between nodes | IDS standard time replaces wall-clock and logical clocks for ordering and commit |
| 4 | Silent bit corruption after acknowledge | out=in byte accounting + daily integrity checks; "fishy" states trigger quarantine/re-verify |
| 5 | RNI rotation mid-operation | Database retired temporarily, returns online after rotation window |
| 6 | Primary read target unavailable | Automatic fallback to next closest eligible replica |
| 7 | Correlated infrastructure placement | Different locations, H-units, regions, and failure domains for every copy |

---

# 8. OPEN DESIGN NOTES

The following behaviors are described above but require formal specification before implementation:

- Exact definition of "too many nodes die" per H-unit;
- Input/output interface of the type-safe AI decision model (what control signals it consumes, what commands it emits);
- Construction of IDS standard time and its drift tolerance across regions;
- Daily check sampling strategy (full scan vs representative sample);
- "Fishy" anomaly thresholds and false-positive handling;
- Size of the temporary-retirement window during RNI rotation;
- Definition of "next closest" when multiple fallback candidates exist;
- Maximum fan-out count for simultaneous multi-database writes (currently ~30).

---

# END OF DOCUMENT
