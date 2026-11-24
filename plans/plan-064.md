## Phase 064 — Create the ids-reconciliation crate: Divergent replica convergence. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 955-969)

```
64. Research only do not read plan.md **[Phase 064 — Create the ids-reconciliation crate: Divergent replica convergence.]**

Crate: ids-reconciliation
Purpose: Divergent replica convergence.
Contents: Version comparison, operation history comparison, conflict and authority rules, merge semantics, CRDT integration, conflict detection, conflict resolution, silent overwrite prevention.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 064 — Create the ids-reconciliation crate: Divergent replica convergence.]**

Create plan-064.md in C:/idfs/plans/ for the ids-reconciliation crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 064 creates `ids-reconciliation`, the IDS crate responsible for reconciling divergent replicas. Its job is to detect divergence between replicas, compare versions and operation histories, apply conflict and authority rules, resolve conflicts through CRDT merge semantics, and prevent silent overwrites — all carrier-agnostic. `ids-reconciliation` does not open sockets or drive any transport directly; it takes two (or more) replica states as opaque inputs, produces a `MergeDecision` (accept / reject / merge / tombstone / quarantine), and the caller ships it as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

Contents per prompts.txt entry 64: version comparison, operation history comparison, conflict and authority rules, merge semantics, CRDT integration, conflict detection, conflict resolution, silent overwrite prevention.

**Research basis:** 30 parallel searches returned the Shapiro et al. 2011 CRDT paper (INRIA RR-7687 / Hal comprehensive study) proving strong eventual consistency via join-semilattice merge (commutative, associative, idempotent); the delta-CRDT paper (Almeida/Shoker/Baquero, arXiv:1603.01529) for incremental delta-state anti-entropy; state-based vs operation-based CRDT comparison (CvRDT vs CmRDT); version vectors and dotted version vectors for causal ordering and conflict detection (Baquero/Preguiça DVV paper, Riak KV docs); Merkle tree anti-entropy (Dynamo/Cassandra/DSE docs) for digest-based replica comparison; anti-entropy digest exchange (systemoverflow/scalecube-prism) for healing divergent state after partitions; LWW register semantics and its lost-update / silent-overwrite hazard (semantic conflict model paper); OR-set (observed-remove / add-wins) semantics; PN-counter and MV-register semantics; CRDT convergence theorem (ACID: associative, commutative, idempotent, distributed); hinted handoff / sloppy quorum (Dynamo paper); read repair (ScyllaDB/Cassandra); read repair and sibling explosion (Riak DVV vs vector clocks); fencing tokens / write fence (Kleppmann stale-write analysis, ETag/If-Match, leader epoch); tombstone garbage collection via causal stability (USENIX OSDI'26 Treedoc); IBLT / invertible bloom filters for set reconciliation (MIT, rateless Bloom filters arxiv 2510.27614); causal consistency session guarantees (read your writes, monotonic reads, monotonic writes, writes follow reads); OP/ROSEMBLE-style replication protocol (EPFL infoscience); MELD anti-entropy digest under lossy routing (arxiv 2608.16357).

A skeleton `ids-reconciliation` crate does NOT yet exist at `C:/idfs/ids-reconciliation/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `OpState`, `CommitState`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-reconciliation` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a pure reconciliation layer: given replica states (each tagged with version, operation history, causality metadata), it produces a `ReconcileResult` — the decision about what to keep, what to merge, what to tombstone, what to quarantine, and whether any silent overwrite was blocked. It never sends bytes itself; the caller frames decisions into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                   REPLICA OWNERS (ids-store, ids-replication)                │
│  hold local replica state; initiate reconciliation after partition heal      │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 064: ids-reconciliation (THIS CRATE — pure logic)       │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │ │\\
│  │ version         │ │ op-history      │ │ conflict        │ │ authority│ │ │\\
│  │ comparison      │ │ comparison      │ │ detection       │ │ rules    │ │ │ │\\
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │ │\\
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │ │\\
│  │ merge semantics │ │ CRDT integration│ │ conflict        │ │ silent   │ │ │ │\\
│  │ (CvRDT/CmRDT)   │ │ adapters        │ │ resolution      │ │ overwrite│ │ │ │\\
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │ │\\
│  ┌─────────────────┐ ┌─────────────────┐ │                 │ │tombstone │ │ │ │\\
│  │ quarantine      │ │ merkle/digest   │ │                 │ │ / GC     │ │ │ │\\
│  │ buffer          │ │ comparison      │ │                 │ │          │ │ │ │\\
│  └─────────────────┘ └─────────────────┘ │                 │ │          │ │ │ │\\
└─────────────────────────────────────────┬──────────────────┴──────────────┘\\                                          │ opaque Vec<u8> datagrams (MergeDecision payload)│                                          │                                          ▼│┌──────────────────────────────────────────────────────────────────────────────┐│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   ││  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])   ││  receive_datagram → (Endpoint, Vec<u8>)                                    ││  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    ││  / Wi-Fi / Bluetooth / cellular / VPN / mesh                               │└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one reconciliation cycle:**
1. Replica A detects partition heal or receives a peer's digest (merkle/version-vector) indicating divergence.
2. `version_comparison` compares protocol versions and replica state versions; rejects incompatible versions before merge.
3. `op_history_comparison` compares operation histories (OpRef, OrderingPos, dots) to find missing/concurrent operations.
4. `conflict_detection` flags concurrent writes to the same record (incomparable VV/DVV), lost-update candidates, and silent-overwrite risks.
5. `authority_rules` decide which replica's write wins (fencing token, epoch, LWW tie-break, or authority-by-region) — never a silent overwrite.
6. `merge_semantics` applies CRDT merge (join-semilattice LUB) or delta-CRDT delta merge; falls back to quarantine for non-commutative conflicts.
7. `tombstone_GC` marks removed entries with tombstones, garbage-collects only at causal stability.
8. `ReconcileResult` (accept/merge/reject/tombstone/quarantine) is returned; caller ships it via `ids-carrier::Endpoint`.

**Crypto used:** ed25519 for signing reconcile decisions and audit checkpoints; sha256/blake3 for version hashing, digest comparison, audit-chain links; xchacha20poly1305/chacha20poly1305 to encrypt reconcile state at rest when confidentiality required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 Strong eventual consistency and CRDT convergence theorem

**Source:** Shapiro et al. 2011, "Conflict-free Replicated Data Types" (INRIA RR-7687 / comprehensive study); arxiv 1806.10254 CRDT overview; arxiv 2504.05398 CRDT emulation/simulation; arxiv 2505.19373 CRDT compliance definition; arxiv 2504.01141 coordination-free consistency (ACID = associative, commutative, idempotent, distributed is necessary and sufficient for SEC).

A CRDT is strongly eventually consistent iff: (i) eventual delivery — every update reaches every replica; (ii) strong convergence — replicas that have received the same set of updates (in any order, possibly with duplicates) are in the same state. The merge operator `⊔` must be commutative (`a⊔b = b⊔a`), associative (`(a⊔b)⊔c = a⊔(b⊔c)`), and idempotent (`a⊔a = a`). `ids-reconciliation` enforces these three laws as compile-time/runtime invariants on every CRDT merge adapter it integrates. State-based CRDTs (CvRDT) ship full state and merge via join-semilattice LUB; operation-based CRDTs (CmRDT) ship individual operations requiring causal delivery and commutativity. `ids-reconciliation` supports both modes but defaults to state-based (more robust under lossy/unordered carrier).

#### 3.2 State-based vs operation-based CRDT (CvRDT vs CmRDT)

**Source:** arxiv 1603.01529 (delta-CRDT); arxiv 1710.04469 (pure op-based CRDTs); arxiv 2504.05398; CMU 17-350 lecture 10.

- CvRDT: replica state space is a join-semilattice; local updates are inflationary (`s ≤ update(s)`); merge = join. No special delivery precondition; robust under unordered/lossy delivery. Preferred default for `ids-reconciliation` because the carrier is opaque and unordered by design.
- CmRDT: each operation individually broadcast; requires reliable causal delivery (exactly-once, causal order) and commutative concurrent operations. Smaller messages but stronger transport assumptions. Supported as an optional mode; `ids-reconciliation` documents that CmRDT mode requires the carrier layer to provide causal delivery (which ids-carrier::Endpoint does not guarantee by itself).
- δ-CRDT (arXiv:1603.01529): delta-mutators return a small delta-state joined with both local and remote states; anti-entropy re-sends undelivered deltas. Best-of-both-worlds; `ids-reconciliation` plans a `delta_merge` path for large states.

#### 3.3 Version comparison

**Source:** plan-001/plan-048/plan-060 (ids-000-aa `ProtocolVersion`, `EdgeCaseBits`, `OrderingPos`, `OpRef`); CRDT literature on version vectors.

`ids-reconciliation` compares:
- `ProtocolVersion` (semver MAJOR.MINOR.PATCH) from `ids-000-aa` — rejects replicas speaking incompatible wire versions before any state merge.
- `EdgeCaseBits` (u128 bitfield) — one bit per protocol invariant; mismatched invariant bits → quarantine rather than merge.
- Per-record version vectors / dotted version vectors — `VV_1 < VV_2` (dominates), `VV_1 == VV_2` (equal), `VV_1 ∥ VV_2` (concurrent/incomparable → conflict).
- `OrderingPos` — total order position; `OpRef` — 9-byte operation reference (36^9 namespace).
Comparison outputs: `Ordered` (one dominates), `Equal`, `Concurrent` (conflict), `Incompatible` (version skew → reject).

#### 3.4 Operation history comparison

**Source:** OpSets (arxiv 1805.04263); PO-Log pure op-based CRDTs (arxiv 1710.04469); dot-based causality (eribeiro vv-dvv blog; Riak KV causal context).

Each operation carries `(OpRef, OrderingPos, dot=(NodeId, seq), causal_past: VV)`. `ids-reconciliation` compares two replicas' operation histories:
- Missing ops: present on one side, absent on the other → must be exchanged (delta or full-state catch-up).
- Concurrent ops: same record, incomparable dots → conflict (kept as siblings or resolved by authority rule).
- Duplicate ops: same OpRef/OrderingPos → idempotent, applied once.
- Out-of-order delivery: idempotent + commutative merge tolerates any order.
Output: `OpHistoryDiff { missing: Vec<OpRef>, concurrent: Vec<(OpRef, OpRef)>, duplicates: Vec<OpRef> }`.

#### 3.5 Conflict and authority rules

**Source:** semantic conflict model arxiv 2602.19231 (lost updates, silent overwrite); LWW register (arxiv 1806.10254; CMU slides); fencing token / write fence (thecolony.ai; morling.dev S3 leader election; etcfs Kleppmann analysis; redlease; insightful-data-lab fencing token).

Conflict types:
- Concurrent write-write on same record (incomparable VV/DVV) → siblings or authority resolution.
- Lost update: write A based on stale state overwrites write B → flagged by DVV comparison; prevented by fencing.
- Silent overwrite: a later-arriving write clobbers newer state without detection → blocked by version comparison + fencing token check.

Authority rules (decide winner when conflict must be resolved):
- Fencing token / epoch: writer carries a strictly increasing token (lease epoch, fencing token, or `OrderingPos`); store rejects writes with token below its high-water mark. Never a silent overwrite — rejected writes return `Conflict`.
- LWW tie-break: highest `(timestamp, NodeId)` wins; documents the lost-write risk explicitly.
- Authority by region: designated region is authoritative for a record; other regions' concurrent writes are tombstoned or quarantined.
- Application-supplied resolver: hook for domain-specific merge (e.g., three-way merge).

`ids-reconciliation` makes the authority rule explicit and configurable per record/region; default = fencing-token + LWW-with-conflict-flag (never silently discard).

#### 3.6 Merge semantics

**Source:** Shapiro 2011 join-semilattice; delta-CRDT arxiv 1603.01529; CRDT convergence theorem (ACID); OR-set CMU slides; LWW-Element-Set github junjizhi/roshi; MV-register arxiv 1710.04469 / crdt-kit docs.

Merge strategies per data kind:
- Register: LWW (timestamp + NodeId tie-break) — keeps one value, flags conflict if concurrent.
- MV-register: keeps all concurrent values as siblings; application resolves.
- G-counter / PN-counter: component-wise max (GCounter) or inc/dect pair (PNCounter) — always converges.
- Set: OR-set (add-wins, observed-remove, dot-based tombstones) — concurrent add beats concurrent remove.
- Map: OR-map whose values are themselves CRDTs; per-key merge recursion.
- Custom: application-supplied join function (must satisfy commutativity/associativity/idempotence; verified by property tests).

`ids-reconciliation` exposes `MergeSemantics` enum and a `MergeEngine` trait (sealed) with adapters per CRDT kind. Merge result: `Merged(state, conflicts_resolved: Vec<Conflict>, tombstones: Vec<Tombstone>)`.

#### 3.7 CRDT integration

**Source:** delta-CRDT arxiv 1603.01529 anti-entropy; join decompositions for CRDT sync after partition (dl.acm 10.1145/2957319.2957374); Merkle-CRDTs arxiv 2004.00107; OR-set/mv-register implementations.

`ids-reconciliation` does not implement every CRDT itself; it integrates CRDT adapters:
- Each adapter wraps an opaque CRDT state (`Vec<u8>` or typed), exposes `merge(local, remote) -> MergeResult`, `is_conflicted()`, `causal_context() -> DVV`.
- Adapters registered in `CRDTRegistry` keyed by `RecordId` type.
- Delta path: `delta_mutate(op) -> DeltaState` (small), joined locally and gossipped; anti-entropy re-sends undelivered deltas.
- Integration with `ids-crdt` (later phase) via the sealed `CRDT` contract in `ids-000-aa`; `ids-reconciliation` depends on the contract, not the implementation.

#### 3.8 Conflict detection

**Source:** version vectors (Wikipedia; eribeiro vv-dvv); dotted version vectors (Riak KV; gsd.di.uminho.pt DVV paper); MV-register sibling detection (crdt-kit docs); OP protocol (EPFL infoscience).

Detection inputs: two replica states + their DVVs. Output: `ConflictSet`.
- `VV_1 ∥ VV_2` (incomparable) → concurrent updates → conflict.
- Dot not in other's causal past → the writing replica hasn't seen the other's state → potential lost update.
- Same OpRef applied twice → duplicate (idempotent, not a conflict).
- Version skew (ProtocolVersion or EdgeCaseBits mismatch) → incompatibility, not a mergeable conflict → quarantine.
Detection is deterministic and order-independent: comparing A vs B yields the same ConflictSet as B vs A.

#### 3.9 Conflict resolution

**Source:** LWW register (silent overwrite hazard documented in semantic conflict model); OR-set add-wins; MV-register deferred resolution; fencing token (Kleppmann; etcfs; redlease).

Resolution strategies:
- `AuthorityWins` — designated authority (fencing token / epoch / region) wins; loser is tombstoned, not silently dropped.
- `LWW` — highest `(timestamp, NodeId)` wins; conflict flag set so the application can audit.
- `KeepAll` (MV-register style) — all concurrent values kept as siblings; application resolves later.
- `Quarantine` — conflict held in quarantine buffer; no state change until manual or policy resolution.
- `Reject` — incoming write rejected with `Conflict` error; caller retries with newer state.

`ids-reconciliation` never performs a silent overwrite: every resolution that discards a write is recorded (tombstone or quarantine) and audited (ed25519-signed checkpoint).

#### 3.10 Silent overwrite prevention

**Source:** semantic conflict model arxiv 2602.19231 (lost update = silently overwritten); fencing token articles (thecolony.ai write fence; morling.dev; etcfs Kleppmann; insightful-data-lab); Riak DVV vs vector clocks (sibling explosion reduction).

Silent overwrite = a write arriving later clobbers newer state because the store lacks causality metadata. Prevention mechanisms in `ids-reconciliation`:
- Every write carries its causal context (DVV) and a fencing token (epoch/`OrderingPos`).
- Before applying, `ids-reconciliation` checks: incoming causal context ≥ stored causal context? If not, and the incoming write is not an ancestor, it is a stale/silent-overwrite candidate → rejected or quarantined.
- Fencing token check: token must be ≥ store's high-water mark for that record; older token → `StaleFence` rejection.
- LWW with conflict flag: if timestamps tie or are concurrent, both values kept (siblings) unless an explicit authority rule selects one — never silently discard.
- Every discarded/tombstoned write is recorded in the audit chain (sha256/blake3 hash chain + ed25519-signed checkpoint) so silent overwrites are detectable offline.

#### 3.11 Digest-based replica comparison (Merkle / anti-entropy)

**Source:** systemoverflow anti-entropy digests; scalecube-prism anti-entropy merkle; DSE anti-entropy repair docs; arxiv 2608.16357 MELD digest; dl.acm join decompositions CRDT sync after partition.

Instead of shipping full state, replicas exchange compact digests:
- Merkle tree over hashed keyspace (bucketized by FNV-1a-64 hash into fixed-shape complete binary tree; same shape regardless of which keys each replica holds). Root-hash equality ⇒ in sync; mismatch → descend only differing branches; exchange O(k·log n) hashes for k divergent keys.
- Version-vector digest: per-record DVV summary; compare entry-wise; mismatched entries trigger op-history comparison.
- IBLT / rateless Bloom filter (arxiv 2510.27614; MIT set reconciliation) for small-difference set reconciliation: insert one set, remove the other, retrieve difference keys with high probability.
`ids-reconciliation` defines the `Digest` trait (sealed) with `MerkleDigest`, `VVDigest`, `IBLTDigest` implementations; digest comparison is the entry point to reconciliation (cheap common case: roots match → no work).

#### 3.12 Gossip / anti-entropy delivery

**Source:** dl.acm gossip robustness (Demers et al. 1987); systemoverflow anti-entropy post-partition healing; arxiv 2608.16357 MELD anti-entropy under lossy routing.

- Push/pull gossip rapidly disseminates recent changes; anti-entropy (periodic digest exchange) is the backstop that heals divergence missed by gossip or during partitions.
- After partition heal: gossip spreads joins/suspicions in seconds; anti-entropy runs later, exchanges digests, detects mismatched entries, reconciles only the drift.
- `ids-reconciliation` is transport-agnostic: digests and merge decisions are shipped as opaque `Vec<u8>` to `ids-carrier::Endpoint`; retry/re-delivery and anti-entropy scheduling are owned by `ids-replication`/`ids-gossip` (later phases), not this crate.
- Convergence guarantee: if every delta/mutation eventually reaches every replica (directly or transitively) and merge is a join, all replicas converge (δ-CRDT Proposition 1).

#### 3.13 Tombstone garbage collection

**Source:** USENIX OSDI'26 Treedoc (tombstone GC via causal stability); CMU slides OR-Set tombstone problem; arxiv 1608.03960 JSON CRDT GC requirement.

- Removes are recorded as tombstones (never silently delete) so concurrent adds can be correctly arbitrated (add-wins).
- Tombstone is garbage-collectable only at causal stability: an operation/tombstone `o` is stable when `VV_o ≤ VV_u` for every replica `u` (everyone has seen it).
- `ids-reconciliation` exposes `TombstoneGC` with `is_stable(tombstone, global_VV_min) -> bool`; actual GC triggered by `ids-replication`/`ids-gc` (later phases).
- Unbounded tombstone growth is a documented risk; bounded by causal-stability window + optional compaction.

#### 3.14 Causal consistency session guarantees

**Source:** Wikipedia causal consistency; arxiv 2211.03501 tunable causal consistency; MongoDB causal consistency docs.

Causal consistency = read your writes + monotonic reads + monotonic writes + writes follow reads. `ids-reconciliation` supports these via DVV gating:
- A read returns only after its observed causal context is satisfied at the serving replica (read your writes).
- Successive reads observe monotonically non-decreasing write sets (monotonic reads).
- Writes on the same session observed in session order (monotonic writes / writes follow reads).
Available under partition (causal consistency is the strongest partition-available model); weaker than linearizability — documented.

#### 3.15 Allowed crypto only

**Source:** ed25519-dalek docs.rs (RFC 8032); chacha20poly1305 docs.rs (RFC 8439, XChaCha20); sha2 RustCrypto (FIPS 180-4 SHA-256); BLAKE3-team README.

`ids-reconciliation` uses:
- ed25519: signing reconcile decisions, authority checkpoints, audit-chain links.
- sha256: version hashing, digest leaf hashing, audit-chain links.
- blake3: fast content hashing for Merkle digests, causal-context hashing, checkpoint chaining (preferred for speed/parallelism).
- xchacha20poly1305: encrypt reconcile state at rest (24-byte nonce, random safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based).
No other crypto; `ids-crypto` exposes primitive IDs; `ids-reconciliation` references via `ids-000-aa` only.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation |
|--------|---------------|------------|
| Silent overwrite (stale write clobbers newer state) | Data correctness | DVV + fencing-token pre-check; reject/quarantine stale writes; never discard without tombstone/audit |
| Lost update (concurrent write discarded) | Data correctness | OR-set add-wins; MV-register keeps siblings; LWW with conflict flag; authority rule explicit |
| Sibling explosion (unbounded concurrent values) | Memory / read cost | DVV de-duplication (vs plain vector clocks); tombstone GC at causal stability; compaction |
| Version skew / incompatible merge | Convergence safety | ProtocolVersion + EdgeCaseBits pre-check; quarantine on mismatch |
| Replay of merge decisions | Decision integrity | ed25519-signed audit checkpoints; OpRef idempotency; bounded replay cache |
| Clock manipulation for causality | Causality correctness | Logical clocks (DVV dots/counters + monotonic IdsTime); never wall-clock for ordering |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent merge (idempotence law); OpRef deduplication; carrier-agnostic boundary |
| Partition-induced divergence | Convergence | Anti-entropy digest backstop; δ-CRDT re-send of undelivered deltas; CRDT join convergence |
| Key extraction (signing key) | Decision authenticity | Keys encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
| Unbounded state growth (tombstones, history) | Memory | Causal-stability tombstone GC; digest-based comparison (no full-state ship) |
| Prohibited crate in dependency tree | Architecture | CI dependency audit; `cargo tree` asserts no std::net/tokio::net/libp2p/QUIC/HTTP/DNS |
| Disallowed crypto algorithm | Crypto policy | ids-crypto is sole crypto surface; allowlist unit test |

The carrier is never trusted: reconcile decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-reconciliation |
|-------|------|---------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `Checksum`, `OpState`, `CommitState`, `Error` — consumed, never redefined; sealed contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All reconcile traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing decisions/audit; encrypting reconcile state at rest; hashing versions and digests |
| `ids-store` | Primary record store (replica owner) | Initiates reconciliation on partition heal; applies `ReconcileResult` to local state |
| `ids-replication` | Replica synchronization / anti-entropy | Drives digest exchange, delta re-send, tombstone GC scheduling; uses reconcile decisions |
| `ids-crdt` | CRDT implementations (later phase) | Adapters registered in `CRDTRegistry`; `ids-reconciliation` depends on the sealed `CRDT` contract only |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Profile tags reads/writes; reconcile respects profile (e.g., quorum decisions before merge) |
| `ids-immutable` (Phase 040) | Immutable record protection | Reconcile must not permit writes to held/immutable records; guard checked before merge admission |
| `ids-session` (Phase 050) | Client-session binding | Session-bound operations carry causal context; reconcile audit cross-referenced with session audit |

Integration points: `ids-reconciliation` exposes a pure `reconcile(local, remote) -> ReconcileResult` API; callers frame the result into opaque datagrams to `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-reconciliation/` (a separate crate, not a folder inside an existing crate):

```
ids-reconciliation/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, serde, thiserror, blake3
├── lib.rs                              # crate root, public API: reconcile(), ReconcileResult, MergeDecision
├── constants.rs                        # limits: MAX_DIGEST_BUCKETS, TOMBSTONE_GC_WINDOW, FENCE_HIGH_WATER
├── error.rs                            # ReconcileError (thiserror): IncompatibleVersion, Conflict, StaleFence, SilentOverwriteBlocked
├── types.rs                            # VersionCmp, OpHistoryDiff, ConflictSet, ReconcileResult, MergeDecision (re-exports ids-000-aa types)
├── version_comparison.rs               # ProtocolVersion + EdgeCaseBits comparison; incompatible → reject
├── op_history_comparison.rs            # compare OpRef/OrderingPos/dot histories; missing/concurrent/duplicate
├── conflict_detection.rs               # VV/DVV incomparability → ConflictSet; lost-update candidates
├── authority_rules.rs                  # fencing token / epoch / LWW tie-break / region authority → resolution strategy
├── merge_semantics.rs                  # MergeEngine trait (sealed); LWW / OR-set / PN-counter / MV-register / map adapters
├── crdt_integration.rs                 # CRDTRegistry; delta_mutate/delta_merge; sealed CRDT contract adapter
├── conflict_resolution.rs              # AuthorityWins / LWW / KeepAll / Quarantine / Reject strategies
├── silent_overwrite_prevention.rs      # pre-apply fence + causal-context check; tombstone/audit every discard
├── digest_comparison.rs                # Digest trait (sealed); MerkleDigest / VVDigest / IBLTDigest
├── merkle_digest.rs                    # Merkle tree over hashed keyspace; root compare, descend differing branches
├── vv_digest.rs                        # version-vector digest comparison
├── iblt_digest.rs                      # invertible bloom filter set reconciliation (optional)
├── tombstone.rs                        # Tombstone entry; causal-stability check (VV_o ≤ VV_min)
├── tombstone_gc.rs                     # is_stable(); GC window; compaction hook
├── audit.rs                            # append-only hash-chained audit log (sha256/blake3) + ed25519 checkpoints
├── signer.rs                           # ed25519 signing of reconcile decisions / checkpoints
├── verifier.rs                         # offline verification of audit chain
├── reconcile_engine.rs                 # orchestrates: version → op-history → conflict → authority → merge → tombstone
├── contracts/                          # sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── merge_engine.rs                 # trait MergeEngine: merge(local, remote) -> MergeResult
│   ├── crdt_adapter.rs                 # trait CrdtAdapter: opaque state, merge, causal_context, is_conflicted
│   ├── digest.rs                       # trait Digest: digest(state) -> DigestBytes; diff(a,b) -> Divergence
│   ├── authority.rs                    # trait Authority: resolve(conflict) -> Resolution
│   ├── fence.rs                        # trait Fence: check(token) -> Accept|Reject|Stale
│   ├── tombstone_gc.rs                 # trait TombstoneGc: is_stable(tombstone) -> bool
│   ├── reconcile_contract.rs           # trait Reconcile: reconcile(local, remote) -> ReconcileResult
│   ├── carrier_adapter.rs              # opaque Vec<u8> framing to ids-carrier::Endpoint (decision carrier only)
│   └── mod.rs
├── invariants/                         # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── merge_commutative.rs
│   ├── merge_associative.rs
│   ├── merge_idempotent.rs
│   ├── no_silent_overwrite.rs
│   ├── fence_monotonic.rs
│   ├── causal_stability.rs
│   ├── carrier_agnostic.rs
│   ├── crypto_allowlist.rs
│   └── version_gate.rs
├── tests/                              # property/verification tests (~30 files)
├── benches/                            # (~8 files)
├── docs/                               # (~14 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **Version comparison**: `compare_versions(a, b) -> VersionCmp{Ordered, Equal, Concurrent, Incompatible}` using `ProtocolVersion`, `EdgeCaseBits`; `Incompatible` → quarantine before merge.
- **Op-history comparison**: `compare_histories(local, remote) -> OpHistoryDiff{missing, concurrent, duplicates}`; dot `(NodeId, seq)` + causal-past VV; idempotent on duplicates.
- **Conflict detection**: `VV_1 ∥ VV_2` (incomparable) → conflict; dot not in other's causal past → lost-update candidate; same OpRef twice → duplicate (not conflict).
- **Authority resolution**: fencing token high-water mark check → `Accept|Reject|Stale`; LWW tie-break `(timestamp, NodeId)`; region authority; application resolver hook. Every discard → tombstone or quarantine, never silent.
- **Merge semantics**: join-semilattice LUB `merge(a,b)` commutative/associative/idempotent; adapters: LWW-register, OR-set (add-wins), PN-counter, MV-register, OR-map; `MergeResult{state, conflicts_resolved, tombstones}`.
- **CRDT integration**: `CRDTRegistry` keyed by record type; `delta_mutate(op) -> DeltaState` (small) joined locally and remotely; δ-CRDT convergence: if every delta reaches every replica, states equalize.
- **Digest comparison**: Merkle tree over FNV-1a-64-bucketed keyspace; root equality ⇒ in sync; mismatch → descend only differing branches, O(k·log n) for k divergent keys; VVDigest and IBLTDigest variants.
- **Tombstone GC**: tombstone `o` stable iff `VV_o ≤ VV_min` (entry-wise min across all replicas); GC only at stability; bounded by GC window.
- **Audit chain**: each reconcile decision = sha256/blake3-chained entry + ed25519-signed checkpoint; offline verifiable.
- **Reconcile engine**: pipeline `version_gate → op_history_diff → conflict_detection → authority → merge → tombstone_gc → ReconcileResult`.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `Checksum`, `OpState`, `CommitState`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of reconcile envelopes, digests, audit entries.
- `thiserror` — error types.
- `blake3` — content hashing for Merkle digests (type-level surface; crypto ops in ids-crypto).

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No async runtime.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-64.1 | Version comparison rejects incompatible `ProtocolVersion`/`EdgeCaseBits` before merge | 100% of mismatched-version pairs → Quarantine, no state change |
| AC-64.2 | Op-history diff identifies missing, concurrent, duplicate ops deterministically | Same input pair → identical `OpHistoryDiff` regardless of comparison order |
| AC-64.3 | Conflict detection flags concurrent write-write (incomparable VV/DVV) | 100% of concurrent pairs → `ConflictSet` non-empty |
| AC-64.4 | Authority rules never silently overwrite | Every discarded write → tombstone or quarantine + audit entry; zero silent discards |
| AC-64.5 | Fencing token rejects stale writes (token < high-water mark) | 100% of stale-token writes → `StaleFence` rejection |
| AC-64.6 | Merge semantics satisfy commutative, associative, idempotent | Property tests pass for all registered adapters |
| AC-64.7 | CRDT integration: delta reaches every replica ⇒ states equalize | δ-CRDT convergence property test passes |
| AC-64.8 | Digest comparison: equal roots ⇒ no reconciliation needed | Root-match → zero data transfer; mismatch → O(k·log n) divergence localized |
| AC-64.9 | Tombstone GC only at causal stability | `is_stable` false until `VV_o ≤ VV_min`; GC test asserts no premature collection |
| AC-64.10 | Audit chain: every reconcile decision hash-chained + ed25519-signed | Offline verifier validates 100% of entries |
| AC-64.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
| AC-64.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-64.13 | Crate compiles cleanly | `cargo check -p ids-reconciliation` exits 0 |
| AC-64.14 | Tests pass | `cargo test -p ids-reconciliation` passes all property/unit/integration tests |
| AC-64.15 | Dependency audit: no prohibited crates | `cargo tree` shows only allowed dependencies |

---

### 10. Remaining Uncertainties

1. **Concrete authority rule defaults per record/region:** fencing token vs LWW vs region authority — deployment-tuned; validated at integration with `ids-store`/`ids-replication`.
2. **Digest choice per workload:** Merkle tree depth (bucket granularity) vs VVDigest vs IBLT; trade-off between detection precision and digest size unquantified.
3. **Anti-entropy interval:** gossip vs anti-entropy scheduling owned by `ids-replication`/`ids-gossip`; interval affects convergence speed vs bandwidth.
4. **Tombstone GC window size:** causal-stability window affects memory growth vs GC frequency; workload-dependent.
5. **MV-register application resolver:** interface for deferred conflict resolution not yet defined; hook reserved in `conflict_resolution.rs`.
6. **Interaction with `ids-consistency-profile` (Phase 060):** profile-based quorum gating before merge; exact handshake defined here, validated at integration.
7. **Interaction with `ids-immutable` (Phase 040):** reconcile must respect holds/retention; guard handshake defined here, validated at integration.
8. **Interaction with `ids-session` (Phase 050):** session-bound causal context cross-referencing to be validated at integration.
9. **CmRDT mode transport assumption:** operation-based CRDTs require causal/exactly-once delivery; `ids-carrier::Endpoint` doesn't guarantee it — mode gating decision pending.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-reconciliation` (not a folder) | Follows "separate crates instead of more folders" directive; clear convergence boundary |
| State-based (CvRDT) default, δ-CRDT optional | Robust under unordered/lossy opaque carrier; delta path optimizes large states |
| Merge = join-semilattice LUB (commutative, associative, idempotent) | Shapiro 2011 SEC theorem; order-independent convergence without coordination |
| DVV for causal tracking (not plain VV) | Riak DVV: de-duplicates updates, reduces sibling explosion, avoids lost writes/false conflicts |
| Authority rule explicit + configurable (fencing token / LWW / region) | Never silently overwrite; every discard tombstoned/quarantined + audited |
| Silent overwrite prevention via fence + causal pre-check | Blocks stale writes clobbering newer state; audit trail makes discarded writes detectable |
| Digest-based comparison entry point (Merkle/VV/IBLT) | Anti-entropy backstop: cheap common case (roots match → no work), localized repair on mismatch |
| Tombstone GC at causal stability only | OR-set tombstone problem solved; bounded by GC window + compaction |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence for discarded/tombstoned writes |
| Carrier-agnostic boundary via `ids-carrier::Endpoint`; crate returns decisions, never sends | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; `ids-crypto` exposes primitive IDs via `ids-000-aa` | Complies with allowed-crypto rule |

---

### 12. Sources

[^1]: Shapiro, Preguiça, Baquero, Zawirski — "Conflict-free Replicated Data Types," INRIA RR-7687 (2011). https://hal.inria.fr/inria-00555588 — CRDT convergence, join-semilattice, CvRDT/CmRDT, strong eventual consistency.
[^2]: Almeida, Shoker, Baquero — "Delta State Replicated Data Types," arXiv:1603.01529 (2016). https://arxiv.org/html/1603.01529 — δ-CRDT, delta-mutators, anti-entropy, convergence proposition.
[^3]: Baquero, Preguiça — "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores" (2012). https://gsd.di.uminho.pt/members/vff/dotted-version-vectors-2012.pdf — DVV definition, causal history, downset invariant.
[^4]: Riak KV — "Causal Context" / "Vector Clocks Revisited Part 2: Dotted Version Vectors." https://docs.riak.com/riak/kv/2.2.2/learn/concepts/causal_context/ — DVVs vs vector clocks, sibling explosion reduction.
[^5]: eribeiro.github.io — "Connecting the Dots: Version Vectors vs. Dotted Version Vectors." https://eribeiro.github.io/blog/vv-dvv/ — VV/DVV dominance, equality, concurrency; false conflicts vs genuine conflicts.
[^6]: Shapiro et al. — "Conflict-Free Replicated Data Types" (LNCS tutorial). https://link.springer.com/article/10.1007/978-3-642-24550-3_29 — SEC definition, semilattice merge laws, CmRDT commutativity theorem.
[^7]: arxiv 1806.10254 — "Conflict-free Replicated Data Types: An Overview." https://arxiv.org/html/1806.10254v1 — LWW vs MV-register semantics, concurrency semantics (add-wins/remove-wins/LWW).
[^8]: arxiv 2504.05398 — "CRDT Emulation, Simulation, and Representation Independence." https://arxiv.org/pdf/2504.05398 — state-based vs op-based CRDTs, strong convergence.
[^9]: arxiv 2504.01141 — "A Preliminary Model of Coordination-free Consistency." https://arxiv.org/pdf/2504.01141 — ACID (associative, commutative, idempotent, distributed) necessary and sufficient for SEC.
[^10]: arxiv 1710.04469 — "Pure Operation-Based Replicated Data Types." https://ar5iv.labs.arxiv.org/html/1710.04469 — PO-Log, op-based CRDTs, causal delivery precondition.
[^11]: arxiv 2505.19373 — "CRDTs for Neural Network Model Merging" (CRDT compliance definition). https://arxiv.org/html/2605.19373v1 — merge commutativity/associativity/idempotency laws.
[^12]: systemoverflow — "Anti-Entropy, Digests, and Reconciliation: Healing Divergent State in Gossip Systems." https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems — digest exchange, Merkle anti-entropy, post-partition healing.
[^13]: scalecube-prism — "anti-entropy-merkle.md." https://github.com/scalecube/scalecube-prism/blob/master/prism-docs/anti-entropy-merkle.md — fixed-shape Merkle tree, bucket diff, proportional repair.
[^14]: DataStax DSE — "Anti-entropy repair." https://docs.datastax.com/en/dse/5.1/docs/architecture/database-architecture/anti-entropy-repair.html — Merkle tree anti-entropy, depth-15 tree, branch-level comparison.
[^15]: arxiv 2608.16357 — "MELD: A Protocol for Merging Knowledge Across Distributed Agentic Memories." https://arxiv.org/pdf/2608.16357 — anti-entropy digest under lossy routing, per-claim digest, symmetric difference pull.
[^16]: dl.acm 10.1145/2957319.2957374 — "Join Decompositions for Efficient Synchronization of CRDTs after a Network Partition." https://dl.acm.org/doi/10.1145/2957319.2957374 — δ-CRDT delta re-send after partition, digest-driven sync.
[^17]: arxiv 2004.00107 — "Merkle-CRDTs: Merkle-DAGs meet CRDTs." https://arxiv.org/html/2004.00107v2 — Merkle tree + CRDT reconciliation.
[^18]: arxiv 2510.27614 — "Rateless Bloom Filters: Set Reconciliation for Divergent Replicas." https://arxiv.org/html/2510.27614 — rateless Bloom / IBLT set reconciliation, variable-sized elements.
[^19]: MIT (Harvard) — "Set Reconciliation and File Synchronization Using Invertible Bloom Lookup Tables." http://nrs.harvard.edu/urn-3:HUL.InstRepos:14398536 — IBLT set difference retrieval.
[^20]: arxiv 1608.03960 — "A Conflict-Free Replicated JSON Datatype." https://arxiv.org/html/1608.03960v2 — MV-register, tombstone GC requirement, conflict-free merge.
[^21]: USENIX OSDI'26 — Cottone et al. (Treedoc tombstone GC via causal stability). https://usenix.org/system/files/osdi26-cottone.pdf — tombstone GC, causal stability `VV_o ≤ VV_min`.
[^22]: arxiv 1805.04263 — "OpSets: Sequential Specifications for Replicated Datatypes." https://arxiv.org/html/1805.04263v2 — OpSets framework for replicated datatypes.
[^23]: arxiv 2602.19231 — "Semantic Conflict Model for Collaborative Data Structures." https://arxiv.org/html/2602.19231v1 — lost updates, silent overwrite hazard, visibility predicate.
[^24]: thecolony.ai — "Last-write-wins is how agents erase each other: multi-writer steps need a write fence." https://thecolony.ai/post/a642013b-0e09-4981-96c0-f34408518376 — write fence (ETag/If-Match, CAS version, fencing token).
[^25]: morling.dev — "Leader Election With S3 Conditional Writes." https://morling.dev/blog/leader-election-with-s3-conditional-writes — leader epoch as fencing token (Kleppmann).
[^26]: etcfs-docs — "Kleppmann's Stale-Write Hazard Analysis." https://etcfs.github.io/etcfs-docs/architecture/storage/kleppmann-stale-write-analysis — fencing token rejects stale writers at storage layer.
[^27]: insightful-data-lab — "Fencing Token." https://insightful-data-lab.com/2024/10/18/fencing-token — fencing token identifies ordered writer generation, prevents stale worker overwrite.
[^28]: Wikipedia — "Causal consistency." https://en.wikipedia.org/wiki/Causal_consistency — available under partition, four session guarantees (ryw, mr, mw, wfr).
[^29]: arxiv 2211.03501 — "Tunable Causal Consistency: Specification and Implementation." https://arxiv.org/html/2211.03501v1 — four session guarantees formalized.
[^30]: MongoDB Docs — "Causal Consistency and Read and Write Concerns." https://mongodb.com/docs/manual/core/causal-consistency-read-write-concerns — majority read/write concern = four causal guarantees.
[^31]: EPFL infoscience — replication protocol (eventual consistency without consensus). https://infoscience.epfl.ch/server/api/core/bitstreams/ce9f02f8-0bf3-4c55-81ae-ce356129267f/content — continuous updates, gap filling, unconscious consistency.
[^32]: dl.acm — "How robust are gossip-based communication protocols?" (Demers et al. 1987). https://dl.acm.org/doi/10.1145/1317379.1317383 — gossip robustness, epidemic dissemination.
[^33]: ed25519-dalek docs.rs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
[^34]: chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
[^35]: sha2 RustCrypto — SHA-256 implementation. https://docs.rs/sha2
[^36]: BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^37]: CMU 17-350 — "Lecture 10: Eventual Consistency and CRDTs." https://cs.cmu.edu/~aldrich/courses/17-350/slides/10-crdts.pdf — SEC guarantees, merge laws, OR-set, LWW register, PN-counter.
[^38]: arxiv 1707.01747 — "Verifying Strong Eventual Consistency in Distributed Systems." https://arxiv.org/pdf/1707.01747 — machine-checked CRDT correctness (RGA, ORSet, counter).
[^39]: IDS prompts.txt entry 64 (verbatim). C:/idfs/prompts.txt — Phase 064 crate definition, contents, constraints.
[^40]: Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml.
[^41]: HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
[^42]: plan-001.md / plan-048.md / plan-060.md (IDS plan format references). C:/idfs/plans/ — same format structure applied to ids-000-aa, ids-bench, ids-consistency-profile.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 064 (ids-reconciliation).

**Output file:** C:/idfs/plans/plan-064.md (this document).
