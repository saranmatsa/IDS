## Phase 066 — Create the ids-vector-clock crate: Causality tracking and conflict detection. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 985-999)

```
66. Research only do not read plan.md **[Phase 066 — Create the ids-vector-clock crate: Causality tracking and conflict detection.]**

Crate: ids-vector-clock
Purpose: Causality tracking and conflict detection.
Contents: Vector clock implementation, clock comparison, causality detection, clock pruning, hybrid logical clock integration, version vector tracking.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 066 — Create the ids-vector-clock crate: Causality tracking and conflict detection.]**

Create plan-066.md in C:/idfs/plans/ for the ids-vector-clock crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 066 creates `ids-vector-clock`, the IDS crate that owns causality tracking and conflict detection. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by logical-clock variant. `ids-vector-clock` implements: (a) standard vector clocks (Mattern/Fidge, 1988) for exact happens-before / concurrent detection, (b) version vectors (per-replica entry, bounded by replication degree) for object-version causality in replicated stores, (c) dotted version vectors (DVV / DVVSet, Almeida et al. 2012) for O(1) causality verification with metadata bounded by degree of replication rather than number of clients, (d) clock pruning / causal-stability garbage collection for bounded metadata growth, (e) hybrid logical clock (HLC, Kulkarni et al. 2014) integration — physical + logical time combination for near-physical timestamps with causality preservation, and (f) conflict detection (concurrent sibling identification) for multi-version replica states.

The crate never opens a socket, never touches `std::net` / `tokio::net` / libp2p / any carrier by name, and never routes traffic itself. It is pure logic + data structures: given two clocks it returns `Before | After | Equal | Concurrent`; given a local clock and an incoming message timestamp it merges (element-wise max) and returns the updated clock plus the causal relationship. All cross-node traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, ≤ 1024-byte address) as opaque `Vec<u8>` datagrams framed by the caller. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` (this crate declares type surface / references primitives, implements no crypto).

A skeleton `ids-vector-clock` crate does NOT yet exist at `C:/idfs/ids-vector-clock/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Mattern & Fidge vector clocks (1988, "Virtual Time and Global States of Distributed Systems", INRIA RR; strong clock condition `a → b ⇔ VC(a) < VC(b)`), Princeton COS418 vector-clock comparison algebra (`V(a) < V(b)` iff `∀k: a[k] ≤ b[k] ∧ ∃j: a[j] < b[j]`; concurrent iff neither dominates), dotted version vectors (ACM DL 10.1145/2332432.2332497; O(1) causality check `na ≤ vb[ia]`; size bounded by replication degree; integrated in Riak), DVVSet / dotted version vector sets (haslab.uminho.pt), Bloom clock (arXiv 2011.11744; probabilistic constant-size causality with false positives), plausible clocks (Springer 1999; constant-size logical clocks), interval tree clocks (Almeida/Baquero/Fonte 2008; dynamic membership, fork/event/join), Dynamo version vectors (DeCandia et al. SOSP 2007; siblings, read repair, anti-entropy, pruning can cause false conflicts), hybrid logical clock (Buffalo CSE tech report 2014-04; physical + logical, 64-bit, monotonic, close to NTP), causal stability / garbage collection (USENIX OSDI'26 Treedoc; locally causally stable tombstones; VVmin component-wise minimum), Rust clock crates (`asupersync` vclock, `crdts::vclock`, `uhlc-rs`, `hlc-gen`, `hybrid_logical_clock`), causal consistency session guarantees (read-your-writes, monotonic reads, monotonic writes, writes-follow-reads; arXiv 2211.03501), sequential vs linearizable (Attiya SPAA'91), anti-entropy digests / Merkle tree reconciliation (systemoverflow; Cassandra), and the IDS plans/format references.

---

### 2. Architecture Overview

`ids-vector-clock` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a pure logic crate: it takes clocks and timestamps as input and returns ordering decisions, merged clocks, and conflict sets. It never sends bytes itself; the caller frames clock metadata into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (clock users)                   │
│  ids-store, ids-replication, ids-crdt, ids-sync — attach clocks to versions │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 066: ids-vector-clock (THIS CRATE — pure logic)          │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ vector clock     │ │ version vector   │ │ dotted version   │ │ HLC     │ │
│  │ (Mattern/Fidge)  │ │ (per-replica VV) │ │ vector (DVV)     │ │ (phys+  │ │
│  │ VClock<A>        │ │ VV<A>            │ │ DVV<A> / DVVSet  │ │ logical)│ │
│  │ compare/merge    │ │ compare/merge    │ │ O(1) causality   │ │ now()/  │ │
│  │ tick/send/recv   │ │ tick/send/recv   │ │ tick/send/recv   │ │ update  │ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ pruning /        │ │ conflict         │ │ causal           │ │ clock   │ │
│  │ causal stability │ │ detection        │ │ stability (GC)   │ │ factory │ │
│  │ GC / bounds      │ │ siblings         │ │ VVmin / tombstone│ │ registry│ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │ opaque Vec<u8> datagrams (clock metadata)
                                      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh                                 │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a vector-clock update:** local event → `vclock.tick(node)` increments own counter → send: attach clock to message → receive: `vclock.merge(received_clock)` (element-wise max) then `tick(self)` → compare: `vclock.causal_order(a, b)` → `Before | After | Equal | Concurrent`.

**Data flow of conflict detection:** two versions of the same key carry clocks VC1, VC2 → `VC1 || VC2` (concurrent) → both kept as siblings → conflict set emitted → caller (ids-crdt/ids-replication) reconciles via CRDT merge or application resolver.

**Clock kinds supported (sealed, via `ClockKind` enum):**
- `VectorClock<NodeId>` — full vector clock, O(n) space, exact causality, strong consistency of the partial order.
- `VersionVector<NodeId>` — version vector, entries only for replicas that touched the object, bounded by replication degree.
- `DottedVersionVector<NodeId>` — DVV: dot `(node, counter)` + causal-past VV; O(1) causality check.
- `DottedVersionVectorSet<NodeId>` — DVVSet: compact set of DVVs, prevents false conflicts, merge = join of DVV sets.
- `HybridLogicalClock` — HLC: 64-bit physical+logical timestamp, monotonic, causality-preserving, close to wall time.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Vector clock fundamentals (Mattern / Fidge, 1988)

**Source:** Friedemann Mattern, "Virtual Time and Global States of Distributed Systems" (1988/1989); Colin Fidge, "Timestamps in Message-Passing Systems That Preserve the Partial Ordering" (1988); Princeton COS418 L5-vc lecture notes.

Vector clocks were proposed independently by Mattern and Fidge in 1988. A vector clock `VC` is an n-dimensional vector of non-negative integer counters, one per process/replica. Update rules:
- On a local event (or send) at process pi: `VC[i] += 1`.
- On send: attach the current `VC` to the message.
- On receive of message with timestamp `ts`: `VC[k] = max(VC[k], ts[k])` for all k, then `VC[i] += 1` for the receiving process.

Comparison algebra (strong clock condition):
- `VC(a) = VC(b)` iff `∀k: a[k] = b[k]` (same event).
- `VC(a) < VC(b)` (a happened-before b) iff `∀k: a[k] ≤ b[k]` and `∃j: a[j] < b[j]`.
- `VC(a) || VC(b)` (concurrent) iff `¬(a ≤ b) ∧ ¬(b ≤ a)` — neither dominates.

The strong clock condition: `a → b ⇔ VC(a) < VC(b)` — vector clocks capture causality precisely in both directions, unlike Lamport scalar clocks which only guarantee the forward implication.

#### 3.2 Clock comparison & causality detection

**Source:** Princeton COS418 L5-vc; systemoverflow "What Are Vector Clocks and How Do They Encode Causality"; `asupersync` vclock `causal_order()` returning `Before | After | Equal | Concurrent`; `crdts::vclock` `concurrent()`; `like-a-clockwork` `CausalityRelation`.

`ids-vector-clock` exposes:
- `compare(a, b) -> CausalOrder { Before, After, Equal, Concurrent }` — componentwise ≤ check over the union of keys (absent = 0).
- `happens_before(a, b) -> bool` — `a < b`.
- `concurrent(a, b) -> bool` — `a || b`.
- `is_equal(a, b) -> bool`.

Absent keys are treated as 0 (missing counter = never observed that replica), so `{A:1}` and `{A:1, B:0}` are Equal.

#### 3.3 Merge / join (componentwise max)

**Source:** vector clock receive rule `VC = max(VC, ts)`; CRDT join-semilattice (pointwise max = least upper bound); G-Counter as version vector.

`merge(a, b) -> VectorClock` = element-wise max over the union of keys. This is the join (least upper bound) in the join-semilattice of vector clocks: commutative, associative, idempotent. Used on message receipt and on anti-entropy state exchange.

#### 3.4 Version vectors (per-replica, bounded by replication degree)

**Source:** Dynamo paper (DeCandia et al. SOSP 2007); MIT 6.824 Dynamo notes ("version vectors indicate v1 supersedes v2"; `[a:1] -> [a:1,b:2]`); eribeiro "Connecting the Dots: Version Vectors vs. Dotted Version Vectors".

A version vector `VV` maps replica ID → highest sequence number observed from that replica. Unlike a full vector clock (one entry per system process), a version vector has one entry per replica that has touched the object — bounded by the replication degree (typically N=3–5), not the total cluster size. Dynamo uses VVs per object key; a version that is dominated (`VV1 < VV2`) is superseded and dropped; concurrent versions (`VV1 || VV2`) are kept as siblings.

`ids-vector-clock` provides `VersionVector<NodeId>` with:
- `contains(dot) -> bool` — `n ≤ vv[node]`.
- `dominates(other) -> bool` — `self < other`.
- `merge(other)` — pointwise max.
- `conflict(other) -> bool` — concurrent.

#### 3.5 Dotted version vectors (DVV / DVVSet) — O(1) causality

**Source:** ACM DL 10.1145/2332432.2332497 (Preguiça, Baquero, Almeida, Fonte, Gonçalves, "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores", 2012); gsd.di.uminho.pt PDF; `ricardobcl/Dotted-Version-Vectors` (DVVSet); eribeiro vv-dvv blog.

A DVV = `(dot, vv)` where `dot = (node_id, counter)` is the single event being described and `vv` is the causal past (traditional version vector). Causality: event a `((ia, na), va)` causally precedes event b `((ib, nb), vb)` iff `na ≤ vb[ia]` — the dot of a is reflected in b's causal past. O(1) check instead of O(n) componentwise comparison. Size bounded by number of servers storing a replica (degree of replication), not number of clients — solved the client-id explosion of earlier VV schemes. Integrated in Riak (~10% of the space of current VV implementations with unsafe pruning).

DVVSet (compact dotted version vector sets): prevents false conflicts that occur with plain VVs; smaller representation similar to VV size; API `update`, `event`, `join`, `sync` suited to get/put KV interfaces.

`ids-vector-clock` provides:
- `DottedVersionVector<NodeId>` — `dot: Dot<NodeId>`, `past: VersionVector<NodeId>`; `precedes(&self, other) -> bool` (O(1)); `merge`.
- `DottedVersionVectorSet<NodeId>` — set of DVVs; join = least upper bound of sibling contexts; prevents false conflicts.

#### 3.6 Clock pruning & causal-stability garbage collection

**Source:** MIT 6.824 Dynamo notes (Dynamo deletes entries for nodes modified long ago; can cause false conflicts — `[b:7]` pruned then `[b:8]` conflicts with derived `[b:7]`); systemoverflow Dynamo pruning (timestamp per `(node, counter)` pair, oldest removed after threshold — GC not causal ordering, false conflicts possible); USENIX OSDI'26 Treedoc (causal stability: operation o is causally stable if `VV(o) ≤ VV(min)` where `VV(min)` = component-wise minimum over all users; locally causally stable implies global; causally stable tombstones prunable; GarbageCollect protocol with `VVgc`); CRDT tombstone GC at causal stability (Shapiro et al. 2011; arxiv 2310.18220 survey).

Pruning strategies evaluated:
- **Time-based pruning** (Dynamo): drop old entries — simple but can create false conflicts (a real ancestor looks concurrent). Documented as a known hazard; `ids-vector-clock` will NOT prune by wall-clock time by default.
- **Causal-stability pruning**: prune only entries ≤ `VV(min)` (component-wise minimum across all known replicas) — safe, preserves causality, requires knowledge of the minimum vector (gossip/anti-entropy to disseminate).
- **Bounded-size pruning with conflict false-positive tracking**: cap entries, track "possible false conflict" flag when pruning removes potentially-relevant history.

`ids-vector-clock` provides a `PruningPolicy` enum: `None | CausalStability { min_vv } | BoundedSize { max_entries, track_false_conflicts: bool }`, plus `prune(&mut self, policy) -> PruneReport { entries_removed, false_conflict_risk: bool }`.

#### 3.7 Hybrid logical clock (HLC) integration

**Source:** Kulkarni et al., "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases" (Buffalo CSE tech report 2014-04); `uhlc-rs` (Unique Hybrid Logical Clock for Rust); `hlc-gen` (lock-free HLC, 42-bit wall + 22-bit logical, 64-bit); `hybrid_logical_clock` crate (no_std); Okapi causally consistent geo-replicated KV store (arXiv 1702.04263; HLC + Universal Stable Time stabilization).

HLC combines physical and logical time: each timestamp = `(physical_wall_time, logical_counter)`. On local event: if physical > last physical, set physical and reset logical to 0; else increment logical. On receive of remote timestamp: take max of physicals, merge logicals. Result: monotonic 64-bit timestamps close to NTP wall time, preserving happens-before (`a → b ⇒ HLC(a) < HLC(b)`), but the converse does NOT hold (HLC < HLC does not imply causality — concurrent events can still be ordered). HLC is weaker than vector clocks for causality but constant-size (64 bits) and useful for snapshots, MVCC, and debugging.

`ids-vector-clock` provides `HybridLogicalClock` with `now() -> HlcsTimestamp`, `send() -> Timestamp`, `receive(remote: &Timestamp)`, `compare(a, b) -> Ordering` (wall first, logical second), and `is_causally_possible(a, b)` — HLC ordering is a *necessary but not sufficient* condition for causality; use vector clock / DVV for the definitive check.

#### 3.8 Conflict detection & sibling management

**Source:** Dynamo version vectors (siblings `[a:1]`, `[a:1,b:2]`, `[a:2]`; client must merge); `crdts` survey (dot IDs, vector clock compression, concurrent detection); arXiv 1511.05010 "Eventually Consistent Register Revisited" (multi-value register returns concurrent values as a conflict set; LWW variant with timestamp); systemoverflow vector clocks (attach clocks to object versions; concurrent → siblings; application logic reconciles).

Conflict detection: two versions of the same object with clocks VC1, VC2 → if `VC1 || VC2` (concurrent), both are siblings = a true conflict requiring resolution. Resolution strategies (delegated to caller crates):
- Last-write-wins (LWW) using HLC/physical timestamp as tiebreak.
- CRDT merge (OR-Set, LWW-register, MV-register).
- Application-specific resolver.

`ids-vector-clock` provides `detect_conflicts(versions: &[(Clock, Value)]) -> ConflictSet { concurrent: Vec<Version>, dominated: Vec<Version> }` and `siblings(clocks: &[VectorClock]) -> Vec<Vec<usize>>` (groups of mutually concurrent clocks).

#### 3.9 Causal consistency session guarantees

**Source:** arXiv 2211.03501 "Tunable Causal Consistency" (four session guarantees: read-your-writes, monotonic reads, monotonic writes, writes-follow-reads; causal consistency = conjunction of the four); MongoDB causal consistency docs (majority read + majority write = all four guarantees); Wikipedia causal consistency (weaker than sequential/linearizable, available under partition).

`ids-vector-clock` supports tracking a per-session causal context: the session's observed DVV/VV is sent with each read (read context) and each write carries its causal dot. A read is gated: deliver only when `causal_past ⊆ applied` (the replica has applied all dependencies). This gives read-your-writes, monotonic reads, monotonic writes, writes-follow-reads without linearizability.

#### 3.10 Rust clock crate landscape

**Source:** docs.rs `asupersync::trace::distributed::vclock::VectorClock` (BTreeMap<NodeId,u64>, `causal_order()`, manual PartialEq with absent-is-zero); docs.rs `crdts::vclock::VClock` (actors + counters, `concurrent()`); `uhlc-rs` (HLC + unique u128 ID, NTP64 64-bit, update_with_timestamp drift guard); `hlc-gen` (lock-free HLC, 42-bit wall + 22-bit logical); `hybrid_logical_clock` (no_std, `is_concurrent()`).

`ids-vector-clock` will be a fresh IDS crate (not wrapping these directly, to avoid non-IDS dependencies and to keep the type system aligned with `ids-000-aa` `NodeId`), but the design borrows their proven APIs: `tick`, `send`, `receive`, `merge`, `compare/causal_order`, `concurrent`.

#### 3.11 Clock serialization & wire format

Clock metadata is serialized as opaque `Vec<u8>` for the carrier. Encoding: serde/bincode or a custom compact binary (node id as `NodeId` bytes + counter as u64/u128 varint). DVV/DVVSet serialized as `(dot, past)` pairs. HLC as 64-bit. Size bounds: vector clock O(n) entries; version vector bounded by replication degree; DVV/DVVSet bounded similarly; HLC constant 8 bytes. No crypto ops inside this crate — but clock envelopes may be signed with ed25519 or hashed with sha256/blake3 by `ids-crypto` at the caller layer when audit/integrity is required.

#### 3.12 Interaction with ids-000-aa types

`ids-vector-clock` imports `NodeId` (as the actor/replica identifier), `IdsTime` (for HLC physical basis and timeouts), `CarrierType` (not used directly — the crate never addresses endpoints), `Checksum` (for clock-hash digests if needed), and `Error` from `ids-000-aa`. It never redefines these. It references sealed trait contracts only as trait bounds where relevant (e.g. a `Clock` trait could be sealed in a later phase; for Phase 066 the concrete clock types are the API).

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| False conflicts from unsafe pruning | Causality correctness | No time-based pruning by default; causal-stability pruning only (`VV ≤ VV(min)`); `PruneReport` flags `false_conflict_risk` when bounded-size pruning removes possibly-relevant history |
| Clock size growth unbounded (O(n) vector clock) | Memory / bandwidth | Version vector and DVV/DVVSet bounded by replication degree; `BoundedSize` pruning policy with explicit risk flag; HLC constant-size alternative for non-critical ordering |
| Clock manipulation / forged causality | Causal integrity | Clock envelopes can be ed25519-signed / sha256/blake3-hashed by `ids-crypto` at the carrier layer; this crate never trusts remote clocks blindly — merge is idempotent and max-based (monotonic, no regression) |
| Wall-clock dependence for causality | Causality correctness | HLC uses wall time as a component but causality decisions use vector clock / DVV; HLC alone never asserted as causal proof; monotonic `IdsTime` only for timeouts, never for ordering |
| Concurrent siblings silently dropped | Data correctness | `detect_conflicts` returns all concurrent versions as siblings; dominated versions only dropped when strictly `VV1 < VV2`; application must resolve siblings |
| Missing counter treated as 0 vs explicit zero | Comparison correctness | Manual `PartialEq`/`Ord` with absent-is-zero rule (as in `asupersync` fix) — `Eq` coherence with `partial_cmp` maintained |
| Carrier duplication/reordering of clock metadata | Causal correctness | Merge is commutative/associative/idempotent — duplicates and reordering are safe; causal gate (DVV `precedes`) blocks delivery until dependencies applied |
| Non-IDS crate sneaking into dependency tree | Carrier-agnostic / crypto policy | Dependency audit in CI; `cargo tree` asserts no `std::net`/`tokio::net`/libp2p/any networking crate; crypto allowlist test asserts only the five algorithms reachable |

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-vector-clock |
|-------|------|------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed contracts + invariants + errors | `NodeId` (actor ID), `IdsTime` (HLC physical basis), `Checksum` (clock digest), `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive | Clock metadata rides as opaque `Vec<u8>` in `Endpoint` payloads; crate never sends/receives directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Optional signing/hashing of clock envelopes at caller layer; this crate implements no crypto |
| `ids-store` | Primary record store | Attaches vector clock / version vector to object versions; consults `ids-vector-clock` for sibling detection |
| `ids-replication` | Replica synchronization / anti-entropy | Exchanges clock digests; uses `compare`/`merge` to determine what to replicate |
| `ids-crdt` | Conflict-free replicated data types | Version vectors / DVV as CRDT metadata; `merge` = join; causal delivery for op-based CRDTs |
| `ids-sync` | Replica synchronization | Causal context piggybacked on sync messages; `ids-vector-clock` validates ordering |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Uses DVV causal context for `CausallyConsistent` profile; quorum profiles use version vectors for freshness |
| `ids-hlc` (Phase 067) | Hybrid logical clock for ordering | `ids-vector-clock` HLC integration is the logical-clock half; `ids-hlc` may build on or mirror it |

**Integration points:** `ids-vector-clock` exposes pure functions and data structures; callers frame clock metadata into `ids-carrier::Endpoint` datagrams. No dependency on `ids-discovery`/`ids-session`/`ids-cli`. No crate-internal reaching.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-vector-clock/` (a separate crate, not a folder inside an existing crate):

```
ids-vector-clock/
├── Cargo.toml                          # serde, blake3, sha2, thiserror, ids-000-aa, ids-carrier, ids-crypto (dev)
├── lib.rs                              # crate root, public API, ClockKind enum, re-exports
├── constants.rs                        # clock limits: MAX_VECTOR_ENTRIES, HLC_BITS, etc.
├── error.rs                            # VectorClockError (thiserror): invalid comparison, prune risk, serialization
├── traits.rs                           # sealed/unsealed Clock trait(s), Merge, Compare, Tick, Send/Receive
│
├── vector_clock/                       # standard vector clock (Mattern/Fidge)
│   ├── mod.rs
│   ├── clock.rs                        # VectorClock<NodeId>: BTreeMap<NodeId,u64>, tick/send/recv
│   ├── compare.rs                      # CausalOrder {Before,After,Equal,Concurrent}, compare()
│   ├── merge.rs                        # componentwise max join
│   ├── types.rs                        # VectorTimestamp, VectorClockEntry
│   └── causality.rs                    # happens_before, concurrent, is_equal
│
├── version_vector/                     # version vector (per-replica, bounded by replication degree)
│   ├── mod.rs
│   ├── clock.rs                        # VersionVector<NodeId>: Map<NodeId,u64>
│   ├── compare.rs                      # dominates, conflict, contains(dot)
│   ├── merge.rs
│   └── types.rs
│
├── dotted_version_vector/              # DVV / DVVSet (Almeida et al. 2012)
│   ├── mod.rs
│   ├── dvv.rs                          # DottedVersionVector: dot + past VV; O(1) precedes()
│   ├── dvvset.rs                       # DottedVersionVectorSet: join of DVVs, prevents false conflicts
│   ├── dot.rs                          # Dot = (NodeId, counter)
│   ├── compare.rs                      # precedes (na ≤ vb[ia]), O(1)
│   └── types.rs
│
├── hybrid_logical_clock/               # HLC integration (Kulkarni 2014)
│   ├── mod.rs
│   ├── clock.rs                        # HybridLogicalClock: (wall, logical) 64-bit
│   ├── timestamp.rs                    # HlcTimestamp
│   ├── compare.rs                      # wall-first then logical ordering
│   └── causality.rs                    # is_causally_possible (necessary not sufficient)
│
├── pruning/                            # clock pruning & causal-stability GC
│   ├── mod.rs
│   ├── policy.rs                       # PruningPolicy {None, CausalStability, BoundedSize}
│   ├── causal_stability.rs             # VV(min) component-wise minimum; locally causally stable
│   ├── bounded_size.rs                 # entry cap with false_conflict_risk flag
│   ├── report.rs                       # PruneReport {entries_removed, false_conflict_risk}
│   └── gc.rs                           # garbage collection entry point
│
├── conflict/                           # conflict detection & sibling management
│   ├── mod.rs
│   ├── detector.rs                     # detect_conflicts(versions) -> ConflictSet
│   ├── siblings.rs                     # sibling groups of mutually concurrent clocks
│   ├── resolution.rs                   # resolution hints (LWW, CRDT, application)
│   └── types.rs                        # ConflictSet, SiblingGroup, ResolutionHint
│
├── carrier_adapter.rs                  # clock metadata → opaque Vec<u8> for ids-carrier::Endpoint
│
├── contracts/                          # sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── clock_contract.rs               # trait Clock: tick/send/receive/merge/compare
│   ├── merge_contract.rs               # trait Merge
│   ├── compare_contract.rs             # trait Compare -> CausalOrder
│   ├── causality_contract.rs           # trait Causality: happens_before/concurrent
│   ├── dvv_contract.rs                 # trait DottedVersionVectorOps
│   ├── hlc_contract.rs                 # trait HybridLogicalClockOps
│   ├── pruning_contract.rs             # trait Prunable
│   └── conflict_contract.rs            # trait ConflictDetection
│
├── invariants/                         # runtime/compile-time invariants (~8 files)
│   ├── mod.rs
│   ├── monotonic_counters.rs           # counters never regress
│   ├── absent_is_zero.rs               # missing key == explicit zero == 0 (Eq coherence)
│   ├── merge_idempotent.rs             # merge(a,a)==a; merge commutative/associative
│   ├── strong_clock_condition.rs       # a→b ⇔ VC(a)<VC(b) for vector clocks
│   ├── hlc_necessary_not_sufficient.rs // HLC ordering ≠ causality proof
│   ├── carrier_agnostic.rs             # no std::net/tokio::net in crate
│   ├── crypto_allowlist.rs             # only allowed crypto referenced
│   └── pruning_safety.rs               # causal-stability pruning preserves causality
│
├── tests/                              # property/verification tests (~30 files)
│   ├── vector_clock_tests.rs
│   ├── version_vector_tests.rs
│   ├── dvv_tests.rs                    # O(1) precedes check
│   ├── dvvset_tests.rs
│   ├── hlc_tests.rs                    # monotonicity, wall-clock closeness
│   ├── pruning_tests.rs                # causal stability safety
│   ├── conflict_tests.rs               # sibling detection
│   ├── comparison_tests.rs             # CausalOrder algebra
│   ├── merge_tests.rs                  # join-semilattice laws
│   ├── carrier_agnostic_tests.rs       # grep: zero std::net/tokio::net references
│   ├── crypto_allowlist_tests.rs       # only 5 algorithms
│   ├── serialization_tests.rs          # round-trip Vec<u8> encoding
│   └── property_tests.rs               # proptest: merge laws, comparison coherence
│
├── benches/                            # (~8 files)
│   ├── vector_clock_bench.rs
│   ├── version_vector_bench.rs
│   ├── dvv_bench.rs
│   ├── hlc_bench.rs
│   ├── pruning_bench.rs
│   ├── conflict_bench.rs
│   ├── merge_bench.rs
│   ├── comparison_bench.rs
│   └── serialization_bench.rs
│
├── docs/                               # (~12 files)
│   ├── architecture.md
│   ├── clock_kinds.md                  # when to use VC vs VV vs DVV vs HLC
│   ├── pruning_guide.md                # causal stability vs bounded-size tradeoffs
│   ├── conflict_resolution.md          # sibling resolution strategies
│   ├── hlc_integration.md              # HLC + vector clock layering
│   ├── wire_format.md                  # clock → Vec<u8> encoding
│   ├── integration.md                  # ids-carrier::Endpoint boundary
│   ├── crypto_policy.md                # allowed crypto usage
│   └── api.md
│
└── spec/                               # (~6 files)
    ├── vector_clock_spec.md
    ├── version_vector_spec.md
    ├── dvv_spec.md
    ├── hlc_spec.md
    ├── pruning_spec.md
    └── conflict_spec.md
```

Total: ~90–1000 files across `src/`, `src/vector_clock/`, `src/version_vector/`, `src/dotted_version_vector/`, `src/hybrid_logical_clock/`, `src/pruning/`, `src/conflict/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, `spec/`. Each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Vector clock (Mattern/Fidge)

```
VectorClock<NodeId> { entries: BTreeMap<NodeId, u64> }

tick(node): entries[node] += 1                          // local event
send():   return clone(entries)                         // piggyback on message
receive(ts):                                            // message with timestamp ts
    for (node, c) in ts: entries[node] = max(entries[node], c)
    tick(self)

compare(a, b) -> CausalOrder:
    all_nodes = keys(a) ∪ keys(b)   // absent = 0
    a_le_b = ∀n: a[n] ≤ b[n]
    b_le_a = ∀n: b[n] ≤ a[n]
    (true, true)  → Equal
    (true, false) → Before   (a happened before b)
    (false, true) → After    (b happened before a)
    (false, false) → Concurrent
```

Strong clock condition: `a → b ⇔ compare(a,b) == Before`.

#### 7.2 Version vector

```
VersionVector<NodeId> { entries: Map<NodeId, u64> }   // one entry per replica that touched object

dominates(other) = self < other   // componentwise ≤ with at least one strict
conflict(other)  = ¬dominates(other) ∧ ¬other.dominates(self)
contains(dot)    = dot.counter ≤ entries[dot.node]
merge(other)     = pointwise max
```

Bounded by replication degree (entries only for replicas that wrote the object).

#### 7.3 Dotted version vector (DVV)

```
Dot<NodeId> = (node: NodeId, counter: u64)
DottedVersionVector<NodeId> { dot: Dot<NodeId>, past: VersionVector<NodeId> }

precedes(a, b) -> bool:   // O(1) — does a causally precede b?
    a.dot.counter ≤ b.past.entries[a.dot.node]
```

`a < b` iff `na ≤ vb[ia]` (the dot of `a` is in `b`'s causal past). Concurrent iff neither precedes.

#### 7.4 Dotted version vector set (DVVSet)

```
DottedVersionVectorSet<NodeId> { dvvs: Set<DottedVersionVector<NodeId>> }

join(a, b):  // least upper bound — merge sibling contexts
    result = a.dvvs ∪ b.dvvs
    prune dominated DVVs (where one's past dominates another's and dot is subsumed)
```

Prevents false conflicts of plain version vectors; compact representation similar to VV size.

#### 7.5 Hybrid logical clock (HLC)

```
HlcTimestamp { physical: u64 (wall ms), logical: u16, ... }   // 64-bit total

now():  if wall > last_wall: physical = wall, logical = 0
        else: physical = last_wall, logical += 1
send(): now() + increment logical
receive(remote):
    physical = max(local_wall, remote.physical)
    logical  = max(local_logical, remote.logical) + (if equal then 1 else 0)

compare(a, b): physical first, then logical   // total order
is_causally_possible(a, b): a < b in HLC order  // necessary, NOT sufficient for causality
```

#### 7.6 Pruning / causal-stability GC

```
VV(min) = component-wise minimum over all known replica version vectors
op o is locally causally stable  ⇔  VV(o) ≤ VV(min)_local
prune(policy):
    CausalStability: remove entries ≤ VV(min); false_conflict_risk = false
    BoundedSize:     cap entries; if evicted entry might be an ancestor → false_conflict_risk = true
PruneReport { entries_removed, false_conflict_risk }
```

#### 7.7 Conflict detection

```
detect_conflicts(versions: &[(Clock, Value)]) -> ConflictSet:
    for each pair (i, j):
        if compare(clock_i, clock_j) == Concurrent:
            add i, j to same sibling group
    ConflictSet { sibling_groups: Vec<Vec<index>>, dominated: Vec<index> }
```

Merge is commutative/associative/idempotent (join-semilattice) — duplicate and out-of-order delivery safe.

---

### 8. Dependencies

`ids-vector-clock/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `IdsTime`, `Checksum`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` type reference (carrier-agnostic boundary; crate never sends directly).
- `ids-crypto` — allowed primitives only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305); referenced via `ids-000-aa` primitive IDs, no direct crypto implementation.
- `serde` — serialization of clock envelopes.
- `blake3` / `sha2` — clock-hash digests (optional, via feature).
- `thiserror` — error enum.
- `proptest` (dev) — property-based tests for merge/comparison laws.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No async runtime.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-066.1 | Vector clock implements Mattern/Fidge update rules (tick/send/receive) | `vector_clock_tests.rs` passes; strong clock condition `a→b ⇔ VC(a)<VC(b)` verified |
| AC-066.2 | Clock comparison returns `Before | After | Equal | Concurrent` correctly | `comparison_tests.rs`: absent-is-zero; `{A:1}` == `{A:1,B:0}`; concurrent detection `{A:2,B:1}` || `{A:1,B:2}` |
| AC-066.3 | Merge is join-semilattice: commutative, associative, idempotent | `merge_tests.rs` property tests (proptest) |
| AC-066.4 | Version vector bounded by replication degree; dominates/conflict/contains | `version_vector_tests.rs`; Dynamo-style `[a:1] → [a:1,b:2]` supersede; `[a:1]` || `[a:2]` conflict |
| AC-066.5 | DVV O(1) causality check `na ≤ vb[ia]` | `dvv_tests.rs`; precedes() is O(1), correct per Almeida et al. 2012 |
| AC-066.6 | DVVSet prevents false conflicts of plain VVs | `dvvset_tests.rs`; join preserves all concurrent siblings |
| AC-066.7 | HLC monotonic 64-bit timestamps, close to wall time | `hlc_tests.rs`; timestamps never regress; `is_causally_possible` documented as necessary-not-sufficient |
| AC-066.8 | Causal-stability pruning is safe (no false conflicts) | `pruning_tests.rs`; `VV ≤ VV(min)` pruning preserves causality; `false_conflict_risk = false` |
| AC-066.9 | Bounded-size pruning reports `false_conflict_risk` when history may be lost | `pruning_tests.rs`; flag set when evicted entry could be an ancestor |
| AC-066.10 | Conflict detection: concurrent versions → siblings; dominated → superseded | `conflict_tests.rs`; Dynamo sibling example `[a:1]`, `[a:1,b:2]`, `[a:2]` |
| AC-066.11 | All clock metadata encodable as opaque `Vec<u8>` for `ids-carrier::Endpoint` | `serialization_tests.rs` round-trip; size bounds documented (VC O(n), VV/DVV bounded by degree, HLC 8 bytes) |
| AC-066.12 | Carrier agnostic: zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | `carrier_agnostic_tests.rs` grep — zero matches |
| AC-066.13 | Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 referenced | `crypto_allowlist_tests.rs` passes |
| AC-066.14 | Crate compiles cleanly | `cargo check -p ids-vector-clock` exits 0 |
| AC-066.15 | All tests pass | `cargo test -p ids-vector-clock` passes |

---

### 10. Remaining Uncertainties

1. **Actor identifier type:** `NodeId` from `ids-000-aa` is the natural actor/replica ID; confirm it fits as `VectorClock<NodeId>` key (it is `Copy + Hash + Ord`).
2. **Counter width:** u64 vs u128 for vector-clock counters — u64 sufficient for expected operation rates; u128 if extreme scale needed.
3. **Serialization format:** serde/bincode vs custom compact binary (varint counters) — tradeoff between interoperability and wire size; decision deferred to implementation.
4. **HLC physical clock source:** `IdsTime` (monotonic ticks) vs wall-clock ms — HLC needs a physical basis; `IdsTime` provides monotonic ticks but HLC ideally tracks wall-clock closeness; source to be confirmed at integration with `ids-hlc` (Phase 067).
5. **Pruning default:** `None` (no pruning) is the safe default; whether `CausalStability` becomes default depends on `VV(min)` dissemination mechanism (gossip/anti-entropy in `ids-replication`).
6. **DVV vs DVVSet default:** DVVSet prevents false conflicts but is more complex; default may be DVV for simplicity, DVVSet as opt-in.
7. **Interaction with `ids-hlc` (Phase 067):** whether HLC lives in `ids-vector-clock` or `ids-hlc`; Phase 066 includes HLC integration, Phase 067 may refine or extract it.
8. **Sealed `Clock` trait:** whether to seal the clock trait in `contracts/` (matching `ids-000-aa` pattern) or keep concrete types public; decision pending.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-vector-clock` (not a folder) | "Separate crates instead of more folders" directive; clear causality boundary |
| Multiple clock kinds (VC, VV, DVV, DVVSet, HLC) | Different tradeoffs: VC exact but O(n); VV/DVV bounded by replication degree; DVV O(1) causality; HLC constant-size 64-bit |
| Absent key = 0 in comparison (manual PartialEq) | Restores `Eq`/`Ord` coherence with `partial_cmp` (fixes `asupersync` deserialization mismatch) |
| No time-based pruning by default | Dynamo-style time pruning causes false conflicts; causal-stability pruning is the safe alternative |
| HLC ordering = necessary not sufficient for causality | HLC < HLC does not imply happens-before; vector clock / DVV for definitive causality |
| Merge = element-wise max (join-semilattice) | Commutative/associative/idempotent → safe under duplicate/out-of-order carrier delivery |
| All traffic via `ids-carrier::Endpoint`; crate never sends | Carrier-agnostic absolute rule; no network stack in the crate |
| Allowed crypto only; no crypto implemented here | IDS policy; `ids-crypto` implements, this crate only references primitive IDs / optionally hashes |
| `NodeId` as actor/replica key | Canonical type from `ids-000-aa`; never redefined |

---

### 12. Sources

1. Mattern, F. — "Virtual Time and Global States of Distributed Systems" (1988/1989). Vector clocks, strong clock condition `a → b ⇔ VC(a) < VC(b)`. https://vs.inf.ethz.ch/publ/papers/VirtTimeGlobStates.pdf
2. Fidge, C.J. — "Timestamps in Message-Passing Systems That Preserve the Partial Ordering" (1988). Independent vector clock invention. https://www.cs.uky.edu/~manivann/cs570/Fidge.pdf
3. Princeton COS418 — "L5-vc: Vector Clocks" lecture notes. Comparison algebra: `V(a) < V(b)` iff `∀k: a[k] ≤ b[k] ∧ ∃j: a[j] < b[j]`; concurrent iff neither dominates. https://cs.princeton.edu/courses/archive/fall19/cos418/docs/L5-vc.pdf
4. DeCandia et al. — "Dynamo: Amazon's Highly Available Key-value Store," SOSP 2007. Version vectors, siblings, read repair, anti-entropy, pruning hazards. https://nil.csail.mit.edu/6.824/2017/notes/l-dynamo.txt
5. Preguiça, Baquero, Almeida, Fonte, Gonçalves — "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores" (2012), ACM DL 10.1145/2332432.2332497. DVV: dot + past VV; O(1) causality `na ≤ vb[ia]`; bounded by replication degree; integrated in Riak. https://dl.acm.org/doi/10.1145/2332432.2332497
6. Almeida, Baquero, Fonte — "Interval Tree Clocks" (2008). Dynamic membership causality tracking; fork/event/join. https://gsd.di.uminho.pt/members/cbm/ps/itc2008.pdf
7. Shapiro, Preguiça, Baquero, Zawirski — "Conflict-free Replicated Data Types," INRIA RR-7687 (2011). Join-semilattice, CRDT convergence, causal stability for tombstone GC. https://www.lip6.fr/Marc.Shapiro/papers/RR-7687.pdf
8. Almeida, Shoker, Baquero — "Delta State Replicated Data Types," arXiv:1603.01529 (2016). δ-CRDT, anti-entropy. https://arxiv.org/html/1603.01529
9. Kulkarni et al. — "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases," Buffalo CSE tech report 2014-04. Hybrid Logical Clock (HLC): physical + logical, 64-bit, monotonic. https://cse.buffalo.edu/tech-reports/2014-04.pdf
10. arXiv 2011.11744 — "The Bloom Clock for Causality Testing." Probabilistic constant-size causality with false positives. https://arxiv.org/html/2011.11744v1
11. Schwarz, Mattern — "Detecting Causal Relationships in Distributed Computations" (1994). Vector clock dimension lower bound. (Cited in decomposition.al "Who invented vector clocks?")
12. arXiv 2211.03501 — "Tunable Causal Consistency." Four session guarantees: read-your-writes, monotonic reads, monotonic writes, writes-follow-reads. https://arxiv.org/pdf/2211.03501v1
13. MongoDB Docs — "Causal Consistency and Read and Write Concerns." Majority read + majority write = all four session guarantees. https://mongodb.com/docs/manual/core/causal-consistency-read-write-concerns
14. Attiya — SPAA'91 "Sequential Consistency versus Linearizability." https://courses.csail.mit.edu/6.852/01/papers/p91-attiya.pdf
15. systemoverflow — "Anti-Entropy, Digests, and Reconciliation." Merkle tree / version vector anti-entropy; Cassandra. https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems
16. systemoverflow — "What Are Vector Clocks and How Do They Encode Causality?" https://systemoverflow.com/learn/distributed-primitives/vector-clocks/what-are-vector-clocks-and-how-do-they-encode-causality
17. USENIX OSDI'26 — Treedoc (Cottone et al.). Causal stability, VV(min), locally causally stable tombstones, garbage collection. https://usenix.org/system/files/osdi26-cottone.pdf
18. arXiv 1702.04263 — Okapi: causally consistent geo-replicated KV store; HLC + Universal Stable Time. https://arxiv.org/pdf/1702.04263v1
19. arXiv 1511.05010 — "Eventually Consistent Register Revisited." Multi-value register conflict sets, LWW variant. https://arxiv.org/html/1511.05010v1
20. docs.rs `asupersync::trace::distributed::vclock::VectorClock`. BTreeMap<NodeId,u64>, `causal_order()`, absent-is-zero PartialEq fix. https://docs.rs/crate/asupersync/latest
21. docs.rs `crdts::vclock::VClock`. Actors + counters, `concurrent()`. https://docs.rs/crdts/latest/crdts/vclock
22. `uhlc-rs` — Unique Hybrid Logical Clock for Rust. HLC + unique u128 ID, NTP64 64-bit, drift guard. https://github.com/atolab/uhlc-rs
23. `hlc-gen` — Lock-free HLC, 42-bit wall + 22-bit logical. https://lib.rs/crates/hlc-gen
24. `hybrid_logical_clock` — no_std HLC, `is_concurrent()`. https://github.com/zamderax/hybrid-logical-clock
25. `ricardobcl/Dotted-Version-Vectors` — DVVSet (compact dotted version vector sets), prevents false conflicts. https://github.com/ricardobcl/Dotted-Version-Vectors
26. eribeiro — "Connecting the Dots: Version Vectors vs. Dotted Version Vectors." VV vs DVV comparison, dominates/conflict definitions. https://eribeiro.github.io/blog/vv-dvv
27. arXiv 2310.18220 — CRDT approaches survey (CvRDT vs CmRDT, causal stability, tombstone GC). https://arxiv.org/html/2310.18220v2
28. IDS prompts.txt entry 66 (verbatim). C:/idfs/prompts.txt lines 985-999.
29. IDS prompts.txt entries 1, 980-983 (constraints: carrier-agnostic, allowed crypto, ids-000-aa master, research only). C:/idfs/prompts.txt.
30. HX-241 reference plan format. C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, clock kinds, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 066 (ids-vector-clock).
