# CRATES.md

IDS crate inventory — 100 crates, `ids-000-aa` master control point.

**Rule:** `ids-000-aa` holds every canonical type and every sealed trait contract.
Any `pub` item added there is instantly available as `ids_000_aa::Thing` in all 99 dependent crates — zero downstream edits for contract changes.
Adding one edge case = edit one file in `ids-000-aa`, then implement the new trait method in one dependent crate at a time.

**Carrier-agnostic absolute rule (no exceptions):**
No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere.
`ids-carrier` exposes opaque `Endpoint` (`CarrierType` + `Vec<u8>`) only.

---

## 1. ids-000-aa — Master crate / single control point
**Purpose:** Canonical types, sealed trait contracts, invariants, versioning, error contracts.
**Contents:**
- `src/lib.rs` — re-exports of types, contracts, invariants, errors, prelude
- `src/types.rs` — `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `PlacementDecision`, `RedundancyMode`, `ConsistencyProfile`, `FailureDomain`
- `src/contracts/mod.rs` — declares all contract submodules
- `src/contracts/carrier.rs` — `trait Carrier` (send/receive opaque datagrams, no IP/TCP/UDP)
- `src/contracts/transport.rs` — `trait Transport` (framing, sequencing, acks)
- `src/contracts/store.rs` — `trait Store` (put/get/delete with durability)
- `src/contracts/session.rs` — `trait Session` (client ↔ IDS one-call binding)
- `src/contracts/routing.rs` — `trait Routing` (logical → physical resolution)
- `src/contracts/discovery.rs` — `trait Discovery` (node/H-unit/region discovery)
- `src/contracts/membership.rs` — `trait Membership` (join/leave/lifecycle)
- `src/contracts/placement.rs` — `trait Placement` (where data lives, capacity-aware)
- `src/contracts/replication.rs` — `trait Replication` (streams, checkpoints, retries)
- `src/contracts/recovery.rs` — `trait Recovery` (multi-level: node/H-unit/region)
- `src/contracts/consistency.rs` — `trait Consistency` (profiles, committed-prefix)
- `src/contracts/ordering.rs` — `trait Ordering` (dataset-scoped domains, no global master)
- `src/contracts/security.rs` — `trait Security` (auth, authz, encryption, key mgmt)
- `src/contracts/client.rs` — `trait Client` — THE ONE CALL (`ids_submit`)
- `src/contracts/fault.rs` — `trait FaultTolerance` (circuit breaker, bulkhead, backpressure)
- `src/contracts/broadcast.rs` — `trait Broadcast` (reliable/causal/total order/atomic)
- `src/contracts/quorum.rs` — `trait Quorum` (read/write quorum intersection)
- `src/contracts/snapshot.rs` — `trait Snapshot` (distributed consistent state)
- `src/contracts/byzantine.rs` — `trait Byzantine` (BFT consensus, authenticated broadcast)
- `src/contracts/lease.rs` — `trait Lease` (lease-based consistency, fencing epochs)
- `src/contracts/crdt.rs` — `trait CRDT` (convergent/commutative replicated types)
- `src/contracts/erasure.rs` — `trait Erasure` (Reed-Solomon, regenerating codes)
- `src/contracts/saga.rs` — `trait Saga` (compensating transactions)
- `src/contracts/coordinator.rs` — `trait Coordinator` (domain-scoped leadership, fencing/STONITH)
- `src/invariants/mod.rs` — declares all invariant submodules
- `src/invariants/binary_completion.rs` — 100% done or failed; no partial success terminal state
- `src/invariants/dead_node_identity.rs` — dead node never returns with old identity; fresh ID assigned
- `src/invariants/idle_rotation.rs` — RNI rotates only while node idle; active requests keep stable ID
- `src/invariants/no_global_master.rs` — dataset-scoped ordering only; no permanent global ordering master
- `src/invariants/out_eq_in.rs` — byte accounting boundary (bytes-out == bytes-in+acked)
- `src/invariants/coordinated_deletion.rs` — all holders delete within milliseconds or not at all
- `src/invariants/fencing_stonith.rs` — STONITH/fencing before any failover; split-brain prevention
- `src/invariants/quorum_intersection.rs` — read quorum ∩ write quorum ≠ ∅
- `src/invariants/erasure_threshold.rs` — any k of n chunks reconstructs; tolerates n-k failures
- `src/invariants/edge_case_registry.rs` — versioned bitfield of every edge case handled
- `src/versioning.rs` — Protocol version + edge-case version bits (compile-time cfg)
- `src/errors.rs` — Sealed error contracts (thiserror)
- `src/prelude.rs`

---

## 2. ids-carrier — Carrier-agnostic transport abstraction
**Purpose:** Native carrier interface, no existing networking stack.
**Contents:** `Endpoint` (`CarrierType` + `Vec<u8>`), `Datagram`, `CarrierType` enum, `send_datagram`, `receive_datagram`, carrier capability negotiation, carrier health, carrier-agnostic addressing.

---

## 3. ids-crypto — Cryptographic primitives
**Purpose:** Only allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
**Contents:** Key generation, signing, verification, symmetric encryption/decryption, hashing, key derivation, nonce management, key rotation, key revocation, constant-time comparisons.

---

## 4. ids-identity — Stable internal identity + rotating RNI
**Purpose:** IDS-B identity separation — stable internal ID with periodically rotating network-facing identifier.
**Contents:** `StableNodeId`, `RotatingNodeId` (RNI), `RNI mapping table`, validity windows, transition states (scheduled/active/superseded/expired/revoked), authenticated resolution, idle-only rotation enforcement, emergency revocation.

---

## 5. ids-time — IDS standard time
**Purpose:** Proprietary time basis for ordering/commit, not wall-clock or HLC.
**Contents:** `Idstime` type, tick source, drift tolerance, synchronization mechanism, cross-region reference frame, clock drift absorption, ordering timestamps, commit timestamps, durability windows.

---

## 6. ids-topology — Failure domains and correlated placement
**Purpose:** CRUSH-style placement, failure domain modeling (power, facility, network, operator, geographic).
**Contents:** `FailureDomain` hierarchy, topology map, correlated failure modeling, placement constraints, domain independence verification, power-domain separation, network-segment separation, operator separation, jurisdiction separation.

---

## 7. ids-failure-detector — Failure detection (SWIM, phi accrual)
**Purpose:** Chandra-Toueg failure detector properties — completeness and accuracy.
**Contents:** SWIM gossip-based detection, phi accrual suspicion, strong/weak completeness, strong/weak accuracy, suspected vs proven failure, heartbeat loss vs GC pause, false positive/negative handling, detector oscillation.

---

## 8. ids-membership — Node/H-unit/region membership and lifecycle
**Purpose:** Authenticated, versioned joins/leaves; coordinated with placement and ownership.
**Contents:** Node states (candidate/validating/active/degraded/draining/failed/retired), H-unit membership, regional cluster membership, membership versioning, admission control, credential validation, capacity advertisement, health reporting, removal procedures.

---

## 9. ids-placement — Capacity-aware and failure-correlation-aware placement
**Purpose:** Where data lives; placement engine with hard constraints and soft objectives.
**Contents:** Placement strategies (hash, range, geographical, workload-aware, capacity-aware, hybrid), dynamic rebalancing, capacity tracking (storage, CPU, memory, network, IOPS), failure-domain-aware placement, recovery-cost-aware placement, hard constraint enforcement (residency, authorization, durability, capacity, data type, retention).

---

## 10. ids-distribution — Data partitioning and distribution
**Purpose:** Dataset division into independently placeable units.
**Contents:** Partition boundaries, key mapping, split/merge behavior, partition metadata, distribution across nodes/H-units/regions, partition ownership, logical-to-physical mapping stability during migration.

---

## 11. ids-discovery — Node/H-unit/region discovery via gossip
**Purpose:** Gossip-based membership discovery without central coordinator.
**Contents:** Rumor mongering, anti-entropy, aggregate gossip, epidemic broadcast, membership convergence, discovery propagation, discovery expiration/refresh, churn handling.

---

## 12. ids-protocol — Native wire/message format
**Purpose:** HX-241-style native frame format, self-defined, carrier-agnostic.
**Contents:** Frame header, frame body, framing, sequencing, acknowledgements, fragmentation/reassembly, delivery semantics, stream/session semantics, flow control, loss detection, retransmission, protocol invariants.

---

## 13. ids-write — Write engine and write-ahead log
**Purpose:** Write lifecycle: received → ordered → executing → committed → synchronized.
**Contents:** WAL, append-only log, durable write path, write fan-out (up to ~30 databases simultaneously), idempotent writes, operation references, ordering positions, durability conditions, commit acknowledgements, write-order management, concurrent-write resolution.

---

## 14. ids-read — Read engine and replica selection
**Purpose:** Distributed reads with replica-aware selection and fallback.
**Contents:** Read lifecycle, closest-replica fallback, ordered-read visibility, replica freshness, consistency profile enforcement, parallel reads, scatter-gather reads, query aggregation, read repair, hinted handoff.

---

## 15. ids-query — Distributed query planning and execution
**Purpose:** Query decomposition across partitions, nodes, regions.
**Contents:** Query planner, targeted query execution, partition maps, indexes, predicates, ownership metadata, scatter-gather execution, distributed aggregation (count, sum, min, max, group, filter, sort), cross-regional queries, intermediate result combination.

---

## 16. ids-routing — Locality-aware and replica-aware routing
**Purpose:** Data-aware request resolution, not network packet routing.
**Contents:** Request resolver, locality-aware routing (geography, health, load, freshness, capacity, ownership, consistency), replica-aware routing, near-to-far routing, adaptive routing, routing tables, path selection, loop prevention.

---

## 17. ids-replication — Replication streams and checkpoints
**Purpose:** Change propagation, ordered/versioned streams, resumable delivery.
**Contents:** Independent replication, full replication, replication streams, acknowledgement tracking, replication lag observability, synchronization checkpoints, incremental synchronization, synchronization retries, bounded backoff, idempotent application.

---

## 18. ids-sync — Incremental sync and anti-entropy
**Purpose:** Replica reconciliation and convergence.
**Contents:** Incremental change streams, checkpoint management, missing update detection, retry procedures, version tracking, Merkle tree comparison, anti-entropy repair, read repair, replica reconciliation, integrity verification.

---

## 19. ids-consistency — Consistency profiles and committed-prefix tracking
**Purpose:** Explicit consistency contracts per dataset.
**Contents:** Strong consistency, causal consistency, read-your-writes, bounded staleness, eventual consistency, consistency profiles table, committed-prefix tracking, operation-state tracking (received/ordered/executing/committed/synchronized/verified/failed), durability-aware acknowledgement.

---

## 20. ids-broadcast — Reliable/causal/total order/atomic broadcast
**Purpose:** Message ordering guarantees across replicas.
**Contents:** Reliable broadcast, FIFO broadcast, causal broadcast, total order broadcast (atomic broadcast), FIFO-total order broadcast, state machine replication, uniform agreement, uniform integrity, validity, termination.

---

## 21. ids-quorum — Read/write quorum systems
**Purpose:** Quorum intersection for consistency.
**Contents:** Read quorum, write quorum, quorum intersection (r + w > n), majority quorums, Flexible Paxos quorum intersection, quorum capacity, quorum fault tolerance, read/write fault tolerance, quorum system optimization.

---

## 22. ids-crdt — Conflict-free replicated data types
**Purpose:** Optimistic replication with deterministic convergence.
**Contents:** Operation-based CRDTs, state-based CRDTs, delta-CRDTs, grow-only counter, grow-only set, two-phase set, last-writer-wins register, OR-set, PN-counter, operation-based vs state-based tradeoffs, merge commutativity/associativity/idempotency.

---

## 23. ids-erasure — Erasure coding (Reed-Solomon, regenerating, LRC)
**Purpose:** Reconstruction-based redundancy with storage efficiency.
**Contents:** Reed-Solomon (n,k) codes, systematic form, maximum distance separable (MDS), regenerating codes, locally recoverable codes (LRC), erasure-code reconstruction, decode latency, fragment placement across failure domains, partial parallel repair (PPR).

---

## 24. ids-lease — Lease-based consistency and fencing epochs
**Purpose:** Time-bounded authority to prevent split-brain.
**Contents:** Lease grant, lease renewal, lease expiry, fencing token increment, fencing token checking, STONITH integration, leadership epoch, term-based authority, lease renewal during partition, premature leadership loss prevention.

---

## 25. ids-recovery — Multi-level recovery and erasure reconstruction
**Purpose:** Node/H-unit/region/region-wide failure recovery.
**Contents:** Node failure recovery, H-unit failure compensation, cross-H-unit recovery, regional failure compensation, regional failure isolation, replica-based recovery, erasure-code reconstruction, capacity-bounded recovery, ownership-aware recovery, recovery verification, redundancy restoration, multi-level recovery state machine.

---

## 26. ids-snapshot — Distributed snapshots (Chandy-Lamport)
**Purpose:** Consistent global state for checkpointing and recovery.
**Contents:** Chandy-Lamport algorithm, marker messages, consistent global state, in-flight message recording, stable property detection, global snapshot, savepoint, checkpoint, incremental checkpoint, state restoration.

---

## 27. ids-fault — Circuit breakers, bulkheads, load shedding, graceful degradation
**Purpose:** Failure isolation and graceful degradation.
**Contents:** Circuit breaker states (closed/open/half-open), thresholds, backoff, per-node circuit breakers, global circuit breakers, handshake circuit breakers, bulkhead isolation, load shedding, backpressure, graceful degradation hierarchy, retry storms, thundering herd prevention.

---

## 28. ids-byzantine — Byzantine fault tolerance
**Purpose:** BFT consensus and authenticated broadcast for malicious nodes.
**Contents:** PBFT, HotStuff, Flexible BFT, Byzantine broadcast (Dolev-Strong), authenticated broadcast, Sybil resistance, eclipse resistance, malicious routing/discovery behavior, replay attacks, traffic manipulation, resource exhaustion, DoS resistance, Byzantine node isolation/quarantine.

---

## 29. ids-saga — Compensating transactions
**Purpose:** Distributed transaction orchestration with rollback.
**Contents:** Choreography-based saga, orchestration-based saga, local transactions, compensating transactions, saga state machine, retry semantics, idempotency, deadlock handling, livelock prevention, cross-domain transaction coordination.

---

## 30. ids-integrity — End-to-end checksums and scrubbing
**Purpose:** Data integrity verification and bit rot detection.
**Contents:** End-to-end checksums, content hashes, Merkle trees, version comparisons, replica audits, partition-level integrity verification, continuous integrity scrubbing, silent bit rot detection, checksum mismatch handling, ZFS-style end-to-end protection.

---

## 31. ids-security — Authentication, authorization, encryption, audit
**Purpose:** Security across complete infrastructure.
**Contents:** Identity management, authentication (mutual), authorization (dataset/operation/identity-level permissions), data encryption (in transit + at rest), key management (generation, rotation, revocation, distribution, recovery), regional security boundaries, auditability, tamper-evident audit records.

---

## 32. ids-resource — Monitoring, quotas, workload isolation
**Purpose:** Resource tracking and workload protection.
**Contents:** Storage/CPU/memory/network monitoring, latency/queue/utilization/health measurements, per-client/per-dataset/per-operator/per-region quotas, workload isolation, backpressure, resource reservations, committed capacity optimization, resource exhaustion prevention.

---

## 33. ids-lifecycle — Data lifecycle management
**Purpose:** Dataset lifecycle: creation → active → archive → expiry → retention → legal hold → deletion → reclamation.
**Contents:** Lifecycle policies, retention periods, legal holds, expiry, deletion coordination, reclamation, tombstone management, space amplification reduction, snapshot compatibility.

---

## 34. ids-compute — Compute-to-data execution
**Purpose:** Analytics and transformations near data.
**Contents:** Compute placement, authorization constraints, resource quotas, data-use policy enforcement, result compaction (return compact results, not entire datasets), execution isolation, compute-to-data scheduling.

---

## 35. ids-federation — Multi-owner trust and federated registry
**Purpose:** Infrastructure owned by independent parties.
**Contents:** Federated infrastructure registry, multi-owner trust architecture, authenticated identities, scoped credentials, policy evaluation, revocation, verifiable participation records, domain admission, capability advertisements, trust roots, provenance.

---

## 36. ids-accounting — Resource accounting and negotiation
**Purpose:** Measure contributed resources; quotas and commitments.
**Contents:** Storage/bandwidth/compute/recovery effort measurement, quotas, reservations, commitments, negotiated resource allocations, infrastructure accounting, billing events, cost per byte, energy per byte.

---

## 37. ids-hotcold — Adaptive hot/cold/warm storage tiering
**Purpose:** Data movement among placement classes by access patterns.
**Contents:** Hot tier, warm tier, cold tier, promotion/demotion, access pattern tracking, latency objectives, cost optimization, lifecycle policy integration, logical identity preservation during tier movement, destination verification.

---

## 38. ids-mobility — Dataset mobility across regions
**Purpose:** Dataset/partition region changes with controlled handoff.
**Contents:** Preferred/authoritative region changes, logical identity preservation, history preservation, access policy preservation, controlled handoff between old/new owners, cutover verification.

---

## 39. ids-selfreorg — Infrastructure self-reorganisation
**Purpose:** Revise H-unit membership, placement, replicas in response to changes.
**Contents:** H-unit membership revision, placement revision, replica/fragment revision, recovery assignment revision, regional responsibility revision, invariant preservation during reorganisation, topology change handling.

---

## 40. ids-immutable — Immutable records and retention enforcement
**Purpose:** Protect selected data against modification/deletion.
**Contents:** Immutable record protection, retention period enforcement, legal hold enforcement, enforcement across replicas/fragments/derived copies, tamper detection, retention expiry.

---

## 41. ids-reclaim — Storage compaction and reclamation
**Purpose:** Compact fragmented storage, reclaim space.
**Contents:** Fragmented storage compaction, obsolete version removal, tombstone expiry, eligible data expiration, space amplification reduction, snapshot/retention compatibility, compaction scheduling, reclamation verification.

---

## 42. ids-backup — Point-in-time backup and incremental backups
**Purpose:** Recovery points and incremental backup histories.
**Contents:** Snapshot metadata, captured versions, consistent reads, cloning, backup copies tracked separately from serving replicas, incremental backup history, point-in-time recovery, restore verification.

---

## 43. ids-chaos — Fault injection and chaos engineering
**Purpose:** Controlled fault injection for resilience validation.
**Contents:** Fault injection harness, packet loss/reordering/duplication, network corruption, partition injection, node churn, resource exhaustion, Byzantine behavior simulation, chaos experiment execution, resilience metrics collection.

---

## 44. ids-observability — Metrics, tracing, audit trail
**Purpose:** System state exposure and observability.
**Contents:** Metrics collection, distributed tracing, audit trail, telemetry, observability dashboard, control-plane vs data-plane separation observability, SLA/SLO tracking.

---

## 45 ids-client — The one-call IDS entry point
**Purpose:** Single client handle — `ids_submit(op)`.
**Contents:** `IDS` handle struct, `ids_submit` method, write/read/update/delete/query/describe operations, logical data addressing, idempotency key management, operation reference tracking, result assembly, client-facing interface abstraction (client never addresses region/H-unit/node directly).

---

## 46 ids-cli — CLI
**Purpose:** Command-line interface.
**Contents:** CLI commands, subcommands, argument parsing, output formatting, interactive and scriptable modes, configuration loading.

---

## 47 ids-testing — Test harness
**Purpose:** Property-based, adversarial, conformance testing.
**Contents:** Unit tests, integration tests, property-based tests (proptest), adversarial tests, conformance tests, test vectors, determinism, fuzzing harness, state recovery tests, chaos tests, soak tests.

---

## 48 ids-bench — Benchmark suite
**Purpose:** Performance benchmarking.
**Contents:** Read/write latency (p50/p95/p99), throughput, cross-domain bytes transferred, recovery completion time, durability restoration time, storage amplification, repair bandwidth, control-plane traffic, metadata convergence time, intent-compilation time, resource utilisation, cost per byte, energy per byte.

---

## 49 ids-doc — Specification and formal models
**Purpose:** Spec, TLA+, PlusCal formal models.
**Contents:** IDS Architecture Specification, TLA+ specs (session, stream, wire), PlusCal models, formal invariants, protocol conformance specification, determinism proofs, conformance suite.

---

## 50 ids-session — Session management
**Purpose:** Client-session binding and state.
**Contents:** Session creation, session expiration, session key management, session state machine, session recovery, client affinity, session stickiness, session token management, session replay protection.

---

## 51 ids-metadata — Hierarchical metadata
**Purpose:** Metadata for regions, clusters, H-units, nodes, datasets, partitions, replicas, fragments.
**Contents:** Hierarchical metadata model, versioned state, explicit relationships, logical identity stability during physical changes, metadata caching/invalidation/version checks.

---

## 52 ids-membership-state — Membership state machine
**Purpose:** Node lifecycle state transitions.
**Contents:** Joining, active, degraded, draining, unreachable, failed, retired states, transition recording, placement/routing integration, state persistence, state recovery after restart.

---

## 53 ids-placement-policy — Placement policy engine
**Purpose:** Dataset-specific placement policies.
**Contents:** Redundancy policy, number of full replicas, number of encoded fragments, reconstruction threshold, required placement domains, regional distribution requirements, recovery requirements, verification requirements, minimum acceptable redundancy state.

---

## 54 ids-partition-map — Partition mapping
**Purpose:** Logical to physical partition mapping.
**Contents:** Partition boundaries, key-range mapping, partition ownership, partition versioning, partition migration tracking, split/merge history.

---

## 55 ids-ordering-domain — Dataset-scoped ordering
**Purpose:** Ordering domains per dataset, no global master.
**Contents:** Ordering domain creation, domain membership, current committed position, pending operations, agreed operation order, domain-scoped leadership, cross-domain transaction coordination.

---

## 56 ids-operation-ref — Operation references
**Purpose:** Nine-character alphanumeric operation IDs separate from ordering positions.
**Contents:** 36^9 namespace, reference generation, uniqueness guarantees, collision detection, idempotency binding, retry recognition, commit-status lookup, synchronization tracking, recovery tracking, audit records.

---

## 57 ids-commit-state — Commit state machine
**Purpose:** Write state: accepted/ordered/executing/committed/replicating/synchronized/verified/failed.
**Contents:** Commit state transitions, durability conditions, commit prefix tracking, gap detection, abort semantics, state persistence, state recovery.

---

## 58 ids-durability-policy — Durability conditions
**Purpose:** Per-dataset durability requirements.
**Contents:** Local persistence, quorum persistence, regional redundancy threshold, cross-region durability, acknowledgement semantics, commit vs synchronization distinction.

---

## 59 ids-redundancy-mode — Redundancy method selection
**Purpose:** Full replication vs erasure coding per dataset.
**Contents:** Full replication mode, erasure coding mode, hybrid redundancy, dataset-specific redundancy, H-unit-aware redundancy, replica/fragment placement across failure domains.

---

## 60 ids-consistency-profile — Consistency profile selection
**Purpose:** Read/write behavior per profile.
**Contents:** Local acknowledged, quorum durable, regionally durable, cross-region durable, eventually replicated, read-only degraded, causally consistent profiles, read/write behavior definitions, failure behavior.

---

## 61 ids-routing-table — Routing table management
**Purpose:** Routing table construction and maintenance.
**Contents:** Routing table entries, next-hop resolution, path selection, locality metrics, replica freshness tracking, loop prevention, route expiration/revalidation, routing convergence.

---

## 62 ids-replication-stream — Replication stream protocol
**Purpose:** Ordered/versioned change propagation streams.
**Contents:** Stream creation, stream versioning, resumable delivery, acknowledgement tracking, lag observability, stream interruption handling, stream recovery.

---

## 63 ids-sync-checkpoint — Synchronization checkpoints
**Purpose:** Verified sync points for resumable replication.
**Contents:** Checkpoint creation, checkpoint verification, resume from checkpoint, reconciliation reference, checkpoint versioning, checkpoint garbage collection.

---

## 64 ids-reconciliation — Replica reconciliation
**Purpose:** Divergent replica convergence.
**Contents:** Version comparison, operation history comparison, conflict and authority rules, merge semantics, CRDT integration, conflict detection, conflict resolution, silent overwrite prevention.

---

## 65 ids-mvcc — Multiversion concurrency control
**Purpose:** Snapshot isolation for reads/writes.
**Contents:** Version management, snapshot reads, write visibility, garbage collection of old versions, transaction isolation levels, phantom read prevention, serialization anomaly prevention.

---

## 66 ids-vector-clock — Vector clocks for causality
**Purpose:** Causality tracking and conflict detection.
**Contents:** Vector clock implementation, clock comparison, causality detection, clock pruning, hybrid logical clock integration, version vector tracking.

---

## 67 ids-hlc — Hybrid logical clock
**Purpose:** Hybrid logical clock for ordering.
**Contents:** HLC implementation, physical+logical time combination, clock skew handling, ordering guarantees, fallback to IDS standard time.

---

## 68 ids-total-order-broadcast — Total order broadcast
**Purpose:** All nodes deliver messages in same order.
**Contents:** Uniform total order, atomic broadcast, consensus-based total order, FIFO-total order, state machine replication integration.

---

## 69 ids-causal-broadcast — Causal broadcast
**Purpose:** Causally related messages delivered in order.
**Contents:** Causality preservation, concurrent message delivery flexibility, happens-before tracking, causal delivery guarantees.

---

## 70 ids-atomic-broadcast — Atomic broadcast
**Purpose:** Uniform agreement on message delivery.
**Contents:** Uniform agreement, uniform integrity, validity, termination, reliable broadcast foundation, total order layer.

---

## 71 ids-flexible-paxos — Flexible Paxos consensus
**Purpose:** Flexible quorum intersection for consensus.
**Contents:** Phase-1/phase-2 quorum separation, non-intersecting quorums within views, intersection across views, leader election quorum, replication quorum, availability optimization.

---

## 72 ids-bft-consensus — BFT consensus protocols
**Purpose:** Byzantine fault-tolerant consensus.
**Contents:** PBFT, HotStuff, LibraBFT, Zyzzyva, SBFT, message complexity, view change, view synchronization, safety under Byzantine faults, liveness under partial synchrony.

---

## 73 ids-authenticated-broadcast — Authenticated Byzantine broadcast
**Purpose:** Dolev-Strong authenticated broadcast.
**Contents:** Message authentication, Byzantine broadcast with signatures, Dolev-Strong protocol, authenticated reliable broadcast, Byzantine fault tolerance with authentication.

---

## 74 ids-chandy-lamport — Distributed snapshot algorithm
**Purpose:** Chandy-Lamport global state recording.
**Contents:** Marker message algorithm, consistent global state, stable property detection, checkpoint/restore foundation, Flink-style barrier snapshotting integration.

---

## 75 ids-swim — SWIM failure detection
**Purpose:** Scalable Weak Membership failure detection.
**Contents:** SWIM protocol, gossip-based suspicion, ping/ping-ack, suspicion dissemination, failure confirmation, scalability to thousands of nodes.

---

## 76 ids-phi-accrual — Phi accrual failure detector
**Purpose:** Phi accrual suspicion with adaptive thresholds.
**Contents:** Phi value calculation, inter-arrival time distribution, suspicion threshold, adaptive accuracy, GC pause handling, false positive/negative tuning.

---

## 77 ids-quorum-intersection — Quorum intersection enforcement
**Purpose:** Guarantee read/write quorum intersection.
**Contents:** Read quorum ∩ write quorum ≠ ∅, quorum system validation, intersection proof, fault tolerance calculation, quorum size optimization.

---

## 78 ids-circuit-breaker — Circuit breaker pattern
**Purpose:** Failure isolation via circuit breakers.
**Contents:** Closed/open/half-open states, failure threshold, success threshold, timeout, backoff strategy, per-peer circuit breakers, global circuit breakers, handshake circuit breakers.

---

## 79 ids-bulkhead — Bulkhead isolation
**Purpose:** Resource isolation to prevent cascade failure.
**Contents:** Thread pool isolation, connection pool isolation, memory isolation, CPU isolation, failure domain isolation, blast radius limitation.

---

## 80 ids-backpressure — Backpressure and load shedding
**Purpose:** Flow control under load.
**Contents:** Credit-based flow control, blocked signal emission, credit reservation, credit return, queue management, load shedding, adaptive rate limiting, congestion detection.

---

## 81 ids-graceful-degradation — Graceful degradation hierarchy
**Purpose:** Ordered fallback states between health and failure.
**Contents:** Degradation hierarchy states, fallback behavior, recovery path, capability reduction, availability tradeoff, operator notification of degraded state.

---

## 82 ids-stonith — STONITH fencing
**Purpose:** Shoot The Other Node In The Head — split-brain prevention.
**Contents:** Node-level fencing, resource-level fencing, power controller integration, IPMI channel, fencing delay configuration, priority fencing delay, STONITH enabled enforcement.

---

## 83 ids-fencing-epoch — Fencing epoch management
**Purpose:** Epoch-based authority to prevent stale owners.
**Contents:** Fencing token increment, fencing token checking, epoch-based leadership, stale owner prevention, recovered region split-brain prevention, authority transfer verification.

---

## 84 ids-reed-solomon — Reed-Solomon erasure codes
**Purpose:** MDS erasure coding for storage efficiency.
**Contents:** RS (n,k) code construction, systematic encoding, decoding, any k of n reconstruction, storage overhead optimization, arithmetic constraints, finite field operations.

---

## 85 ids-regenerating-codes — Regenerating codes
**Purpose:** Bandwidth-efficient node repair.
**Contents:** Regenerating code construction, repair bandwidth minimization, helper node access minimization, functional vs exact repair, regenerating code parameters.

---

## 86 ids-locally-recoverable-codes — Locally recoverable codes
**Purpose:** Locality-aware erasure coding.
**Contents:** LRC construction, locality groups, single/double erasure recovery within locality, code with locality parameters, repair locality optimization.

---

## 87 ids-ppr-reconstruction — Partial Parallel Repair
**Purpose:** Distributed erasure-coded storage repair.
**Contents:** PPR algorithm, partial parallel reconstruction, reduction tree aggregation, m-PPR scheduling, concurrent failure handling, network pressure reduction.

---

## 88 ids-hot-tier — Hot storage tier
**Purpose:** High-performance storage for frequently accessed data.
**Contents:** Hot tier placement, hot tier promotion, hot tier access patterns, hot tier latency objectives, hot tier cost, hot tier capacity management.

---

## 89 ids-warm-tier — Warm storage tier
**Purpose:** Warm storage for moderately accessed data.
**Contents:** Warm tier placement, warm tier promotion/demotion, warm tier cost, warm tier capacity management, warm tier access patterns.

---

## 90 ids-cold-tier — Cold storage tier
**Purpose:** Cold storage for archival data.
**Contents:** Cold tier placement, cold tier promotion/demotion, cold tier cost, cold tier capacity management, cold tier access patterns, retrieval latency.

---

## 91 ids-tenant-isolation — Tenant isolation
**Purpose:** Multi-tenant resource isolation.
**Contents:** Per-tenant quotas, per-tenant isolation, tenant-level admission control, tenant-level policy enforcement, tenant-level resource monitoring.

---

## 92 ids-resource-quota — Resource quotas
**Purpose:** Per-entity resource limits.
**Contents:** Storage quotas, bandwidth quotas, compute quotas, concurrency quotas, recovery bandwidth quotas, quota enforcement, quota violation handling.

---

## 93 ids-workload-isolation — Workload isolation
**Purpose:** Prevent workload interference.
**Contents:** Workload classification, workload scheduling, workload priority, workload bandwidth limits, workload admission control, workload monitoring.

---

## 94 ids-data-use-policy — Data-use and ownership policies
**Purpose:** Policy-driven data access and use.
**Contents:** Data ownership policies, read/write/replicate/process/export/delete permissions, permitted regions, permitted operators, permitted purposes, derived data restrictions, data sovereignty policies.

---

## 95 ids-ownership-policy — Ownership policy enforcement
**Purpose:** Data ownership across regions.
**Contents:** Dataset ownership, regional ownership, partition ownership, ownership transfer, ownership verification, custody graph integration.

---

## 96 ids-legal-hold — Legal hold enforcement
**Purpose:** Prevent deletion during legal holds.
**Contents:** Legal hold creation, legal hold scope, legal hold enforcement across replicas/fragments/derived copies, legal hold release, retention vs legal hold precedence.

---

## 97 ids-retention-enforcement — Retention enforcement
**Purpose:** Enforce data retention periods.
**Contents:** Retention period tracking, retention expiry, retention across replicas, retention verification, retention vs deletion precedence, retention audit.

---

## 98 ids-immutable-record — Immutable record protection
**Purpose:** Immutable data protection.
**Contents:** Immutable record creation, immutability enforcement, tamper detection, immutable record across replicas, immutable record expiry, immutable record audit.

---

## 99 ids-compaction — Storage compaction
**Purpose:** Compact fragmented storage and reclaim space.
**Contents:** Fragment compaction, tombstone reclamation, obsolete version removal, space amplification reduction, compaction scheduling, compaction verification.

---

## 100 ids-reclamation — Storage reclamation
**Purpose:** Reclaim eligible storage space.
**Contents:** Eligible data expiration, space reclamation, snapshot/retention compatibility, reclamation verification, space reclamation audit.

---

## Dependency rule
All crates 2–100 depend on `ids-000-aa` for types and sealed trait contracts.
`ids-crypto` is the only crate that does NOT depend on `ids-000-aa` (avoids circular dependency).
One file at a time: create/modify → `git add -A` → `git commit` → `git push origin main` → verify `git log --oneline -1` and `git status` clean → next file.
