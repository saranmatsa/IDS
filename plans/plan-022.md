## Phase 022 — Create the ids-crdt crate: Optimistic replication with deterministic convergence. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 022 creates **ids-crdt**, a new standalone crate in the IDS repository providing conflict-free replicated data types (CRDTs) for optimistic replication with deterministic convergence. CRDTs are the natural substrate for IDS's carrier-agnostic, masterless replication model: any replica can accept local updates at any time, and replicas that have received the same set of updates reach identical state regardless of delivery order, duplication, or delay — strong eventual consistency (SEC) (Shapiro et al., 2011; strong eventual consistency).

**Scope of CRDT classes (research-verified):**
- **G-Counter** — grow-only counter; per-replica increment counts; merge = pointwise max; value = sum of all entries. Only replica `r` writes entry `r`, and it only grows, so the max of entry `r` is the most recent count from `r` anyone has seen. The addition happens once, at read time, never in the merge.
- **PN-Counter** — positive/negative counter = pair of G-Counters (increments, decrements); value = pos − neg; merge joins each half independently. Both halves only grow, so decrements are never silently undone.
- **G-Set** — grow-only set; add-only; merge = set union; anonymous (no replica IDs needed). Trivially convergent because add is an idempotent inflation.
- **2P-Set** — two-phase set = pair of G-Sets (added, removed/tombstones); element present iff in added \ removed; once removed, never re-addable (remove-wins / remove-bans). Unbounded tombstone growth is the known shortcoming.
- **LWW-Register** — last-writer-wins register = (value, stamp); merge keeps the higher stamp; stamps must be unique, totally ordered, and consistent with causality (Lamport clock / hybrid logical clock + replica-id tie-breaker). Raw wall-clock time is unsafe: clock drift, NTP jumps, and VM pauses lose concurrent writes.
- **OR-Set** — observed-remove / add-wins set; each add creates a fresh unique tag (dot = replica, seq); remove tombstones only the tags this replica has observed; element is present iff it has at least one live (untombstoned) tag. A remove can only kill adds it has seen; a concurrent add on another replica creates a tag the remover could not have seen, so the add wins — add-wins semantics.
- **Operation-based vs state-based tradeoffs** — state-based (CvRDT) ships full state and merges via a join-semilattice; operation-based (CmRDT) ships operations and requires reliable exactly-once causal delivery. For IDS, state-based is the safer primary mechanism because it tolerates lost, reordered, stale, and duplicate messages over an unknown carrier.
- **Merge laws** — commutativity, associativity, idempotency of merge; local updates inflationary; merge computes the least upper bound. Together these are the sufficient condition for strong eventual consistency.

**Key engineering decisions:**
- **State-based (CvRDT) + δ-CRDT hybrid as primary:** full-state merge for simplicity and weak-network tolerance; δ-mutators ship small incremental states instead of full state (Almeida, Shoker, Baquero, 2018). State-based needs no ordering guarantees from the carrier — essential for IDS's carrier-agnostic, unknown-membership model where all traffic rides `ids-carrier::Endpoint` opaque bytes. Op-based CRDTs are rejected as the primary mechanism because they require reliable exactly-once causal broadcast, which IDS does not provide and which contradicts its carrier-agnostic design.
- **Join-semilattice merge:** every CRDT state forms a join-semilattice; merge = least upper bound; merge is commutative, associative, idempotent; local updates are inflationary. This is the sufficient condition for strong eventual consistency (SEC).
- **Persistence via snapshots + delta journal (WAL):** periodic full snapshots (versioned, checksummed) plus an append-only delta journal; recovery = load latest snapshot + replay journal. Tombstones compacted only at causal stability (all peers have acked past a version).
- **New crate `ids-crdt`**, carrier-agnostic, depends only on `ids-000-aa` (canonical types + sealed trait contracts), `ids-carrier` (`Endpoint` = `CarrierType` + `Vec<u8>`), and allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305). Zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies.

---

### 2. Architecture Overview

The CRDT layer sits between the application/IDS metadata layer and the sync layer. It is **not** a network protocol — it is a local, deterministic merge engine that produces bytes to be shipped over whatever carrier IDS is using, exclusively through `ids-carrier::Endpoint`.

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPLICATION / IDS LAYER                      │
│  CRDT metadata, content-addressed objects, local replica state  │
├─────────────────────────────────────────────────────────────────┤
│                    ids-sync (sync engine)                       │
│  anti-entropy, gossip, delta exchange, acks over ids-carrier    │
├─────────────────────────────────────────────────────────────────┤
│             Phase 022: ids-crdt (NEW CRATE)                     │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  CRDT classes: GCounter, PNCounter, GSet, TwoPSet,     │    │
│  │  LWWRegister, ORSet                                    │    │
│  │  δ-mutators, join-semilattice merge, ACI laws          │    │
│  └────────────────────────────────────────────────────────┘    │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  persistence: snapshot + delta journal (WAL)           │    │
│  │  tombstone GC (causal stability)                       │    │
│  └────────────────────────────────────────────────────────┘    │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  wire format: versioned CBOR, deterministic encoding   │    │
│  │  Merkle digest anti-entropy, state-vector exchange     │    │
│  └────────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────────┤
│              ids-storage (key-value, snapshots, compression)   │
├─────────────────────────────────────────────────────────────────┤
│           ids-000-aa  ·  ids-carrier  ·  ids-crypto            │
│  NodeId, ReplicaId, Endpoint · CarrierType + Vec<u8> ·          │
│  ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 │
└─────────────────────────────────────────────────────────────────┘
```

**Data flow of a local update + merge:**
1. Local client invokes a mutator (e.g. `counter.increment(replica_id)` or `orset.add(element)`).
2. The δ-mutator produces a small delta-state `δ = mδ(X)` and the local state advances: `X′ = X ⊔ δ`.
3. The delta is appended to the local delta journal (WAL) and queued for shipping.
4. Periodically (or on demand), ids-sync ships deltas (or full state on join) to peers; each peer merges `X = X ⊔ δ`. Idempotence means duplicates and reordering are harmless.
5. Anti-entropy: peers exchange Merkle digests/state vectors; divergent keys are repaired by shipping only the missing deltas.
6. Snapshots are taken periodically; once all peers have acked past a snapshot, tombstones and journal entries before it are compacted.

---

### 3. Detailed Research Dimensions

#### 3.1 CRDT Classes — Counters (G-Counter, PN-Counter)

**G-Counter (grow-only counter):** state is a map `ReplicaId → u64`; only the owning replica increments its own entry; merge = pointwise max; `value = sum of all entries`. Convergence proof: join = pointwise max over natural numbers, which is commutative, associative, idempotent; updates are inflations. Verified in Coq (G-Counter merge lemmas); production implementations include Hazelcast PN counter and the `delta-stream` Rust crate. (Shapiro et al. 2011; CMU 17-350 CRDT lecture.)

**PN-Counter (positive-negative counter):** composition of two G-Counters: `PNCounter = GCounter × GCounter` (increments, decrements); `value = pos − neg`; `merge = join each half independently`. Both halves only grow, so decrements are never silently undone — a single G-Counter cannot decrement because `max` would resurrect the old value. A pair of lattices joined componentwise is a lattice: the first example of CRDT composition. Maps naturally to `ids-000-aa::ReplicaId`-keyed `BTreeMap<u64>`. Merge cost O(replicas seen). No tombstones — no GC needed.

#### 3.2 CRDT Classes — Sets (G-Set, 2P-Set, OR-Set)

**G-Set:** state = set; add-only; merge = set union. Anonymous CRDT — no replica identifiers needed. Useful as a building block (accumulating search results, capability sets). Add is idempotent: adding the same element multiple times equals adding it once.

**2P-Set (two-phase set):** state = pair of G-Sets `(A added, R removed/tombstones)`; element present iff `e ∈ A \ R`; `add(e) → A ∪ {e}`; `remove(e) → R ∪ {e}` (guarded: only if `e ∈ A`); merge = union of both halves. Remove-wins / remove-bans semantics: once removed, an element can never be re-added. Shortcoming: unbounded tombstone growth; removed elements are permanently banned. Implemented in `crdt_kit`, `lattice_sets`; verified in Coq (Two-Part Set). Riak DT sets use similar add/remove-set pairs behind the scenes.

**OR-Set (observed-remove / add-wins set):** state = map `element → set of tags` plus tombstone set of removed tags. Each `add(e)` creates a globally unique tag (dot = replica, seq); `remove(e)` tombstones only the tags this replica has observed for `e`; element present iff it has at least one live (untombstoned) tag. Merge = union of tag sets, union of tombstones. This preserves sequential set semantics while allowing concurrent add/remove: a concurrent add creates a tag the remover could not have seen, so the add wins. Optimized OR-Set drops tombstones in favor of a version vector of every tag ever seen (Riak DT / Akka Pekko DData ORSet; `riak_dt_orswot`-inspired, no tombstones, version-vector based; in production Erlang, QuickCheck tested). Tombstone GC: a tombstone is only needed while it might suppress a not-yet-propagated tag; once no live element carries the tag and the removal is causally stable, the tombstone is safe to discard (`compact_tombstones` local GC; `compact_tombstones_all` only when all peers converged).

#### 3.3 CRDT Classes — Registers (LWW-Register)

**LWW-Register:** state = `(value, stamp)`; `assign(v)` stamps with a new, larger stamp; merge keeps the value with the higher stamp (tie broken by replica id). Stamps must be unique, totally ordered, and consistent with causality — a wall clock alone is insufficient (drift, NTP jumps, VM pauses lose concurrent writes). Use a Lamport clock / hybrid logical clock: `(counter, replica_id)` compared lexicographically, where the counter is raised above every stamp seen on merge. Semantics: last write wins; concurrent writes — one is silently lost. Acceptable when the choice among concurrent values does not matter (user profiles, config). Multi-writer needs read-modify-write care (use a counter CRDT for counts instead). Implemented in `crdt_kit::LWWRegister` (HybridTimestamp) and Pekko `LWWRegister` (vector-address tie-break; `reverseClock` for first-write-wins).

#### 3.4 Operation-based vs State-based Tradeoffs

**State-based (CvRDT):** replicas gossip entire state; merge reconciles via join-semilattice. Needs no guarantees from the communication channel: messages may be dropped, duplicated, and reordered. Downside: shipping full state is costly as state grows. Convergence guaranteed if every replica eventually receives every state (eventual transitive delivery of the latest state).

**Operation-based (CmRDT):** replicas gossip operations, not state. Execution splits into `prepare` (local, side-effect free, produces a message) and `effect` (applied at every replica on delivery). Requires reliable causal broadcast: exactly-once delivery, and concurrent operations must commute. If delivered without causal order, all operations must commute; if operations may be delivered more than once, all operations must be idempotent. Most op-based designs require exactly-once and causal delivery. Expressive power is greater, but the middleware assumptions conflict with IDS's carrier-agnostic model.

**IDS decision:** state-based as the primary mechanism; op-based considered only as a future layered option where a causal-delivery substrate exists. δ-CRDTs (delta-state CRDTs) bridge the gap: small incremental states shipped over unreliable channels, with idempotent join tolerating loss/duplication/reordering — the best of both worlds for IDS.

#### 3.5 Merge Laws and Convergence Proofs

**Strong Eventual Consistency (SEC, Shapiro et al. 2011):** safety = strong convergence (any two replicas that received the same set of updates are in the same state); liveness = eventual delivery (every update eventually reaches every replica). SEC = eventual consistency + immediate convergence.

**Sufficient condition for state-based (CvRDT) convergence:** (i) states form a join-semilattice (partial order + least-upper-bound join `⊔` that is commutative, associative, idempotent); (ii) local updates are inflations (`s ≤ update(s)`); (iii) merge computes the join (LUB). Then replicas that have seen the same updates meet at the same state — the LUB of all states involved. Three propositions: semilattice ordering; merge = LUB; monotonicity. Combined with eventual delivery → safety + liveness = SEC.

**Op-based (CmRDT) convergence:** operations delivered via reliable causal broadcast; concurrent effector operations must commute; exactly-once delivery (or idempotent ops). If delivered without causal order, all ops must commute.

**State ↔ op equivalence:** always possible to emulate one with the other (Shapiro et al. 2011): state-based as op-based by broadcasting `merge(st)`; op-based as state-based by lattice `(P(Op × Ts), ⊆, ∪)` (set of timestamped ops).

**Verification:** mechanized proofs in Isabelle ("Verifying Strong Eventual Consistency in δ-CRDTs"), Coq (G-Counter merge lemmas; Add-Wins Set 103 spec/228 proof, Remove-Wins Set 99/386, LWW Register 54/136, MV-Register 35/195, Grow-Only Set 22/57, PN-Counter 88/108). Type-checking CRDT convergence (2023) guarantees commutativity/associativity/idempotence via a type system.

**Randomized/convergence testing:** go-crdt proves convergence by randomized sessions with late/reordered/duplicated delivery and by enumerating every permutation of small histories; Ditto tests CRDTs in Rust from theory to practice (property-based convergence tests, adversarial interleavings).

#### 3.6 δ-CRDT: Efficient Merge over Unreliable Channels

**Problem:** state-based CRDTs ship the entire state on every sync — cost grows with state size.

**δ-CRDT (Delta State CRDT, Almeida/Shoker/Baquero 2018):** δ-mutators return a small delta-state `δ = mδ(X)` such that `m(X) = X ⊔ δ`; deltas are joined into local and remote states; shipped individually or coalesced into delta-groups. Convergence guaranteed if every delta reaches every replica; idempotent join tolerates loss, duplication, and reordering over unreliable channels — unlike op-based CRDTs, which demand exactly-once delivery. Anti-entropy ensures causal consistency via delta-intervals + causal delta-merging condition. Preserves commutativity/associativity/idempotence.

**Join decomposition:** decompose state into join-irreducible elements to ship minimal deltas; avoids redundant delta propagation on multi-path network graphs.

**Merkle anti-entropy:** exchange root hashes; equal roots ⇒ equal state (O(1) agreement check); descend only differing branches; locate k changed leaves in O(d·k); bandwidth O(d·log n). Merkle Search Trees preserve key order (important for sequences/maps) vs plain Merkle trees. Scuttlebutt = vector-clock anti-entropy (clock grows linearly with node count — bad for churn/large networks).

**ConflictSync (2025):** digest-driven sync for state-based CRDTs; reduce sync to set reconciliation of irredundant join decompositions; Bloom filters + rateless IBLTs; up to 18× less data transfer vs full-state sync; Bloom prefiltering cuts overhead up to 50% at low similarity.

#### 3.7 Persistence (snapshots, journal, compaction)

Two durable storage patterns:
1. **Log of changes (WAL/journal):** after every change, append the delta bytes; load = replay all deltas. Simple but recovery time grows with history (one production case: 55K ops / 276MB replayed 27+ minutes on restart).
2. **Whole-document snapshot:** `save()` → full serialized state; `load()` → restore. Fast recovery but full-state rewrite on every save (write amplification).
3. **Hybrid (recommended):** periodic full snapshot + append-only delta journal anchored by state vector; recovery = load latest snapshot + replay journal since snapshot. Compaction threshold (size, op count, age, tombstone ratio).

**Correctness rules (non-negotiable):** no partial replay on corruption; no out-of-order journal persistence; fail closed on hash mismatch; tombstones compacted only after **all** replicas acknowledge (causal stability) or deleted records resurrect ("zombie records"); offline/never-acking members block GC — requires membership management or a grace/quarantine window.

#### 3.8 Sequence CRDTs (out of scope for v1, noted for completeness)

Sequence CRDTs (RGA, Logoot, LSEQ, Treedoc, YATA/Yjs, ESBT) solve ordered lists / collaborative text. Each element gets a unique, immutable position identifier defining its logical position on every replica; new elements are placed relative to existing elements (not by index, since positions shift under concurrent edits); concurrent inserts merge deterministically; SEC. Tombstones accumulate and need GC (retained until causally stable, then re-parented before removal to avoid corrupting order). ESBT (2025) bounds identifier growth and is formally proven unique, totally ordered, convergent, SEC — best current answer to unbounded identifier growth. Not in the Phase 022 contents list; flagged as a future extension.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Concurrent writes silently lost (LWW-Register) | Document per-field semantics; offer multi-value register where no loss is tolerable; use counter CRDT for counts |
|| Tombstone / metadata growth unbounded | GC at causal stability; snapshot compaction; tombstone ratio/count thresholds; incremental GC |
|| Wall-clock skew corrupts LWW ordering | Hybrid logical clock / Lamport `(counter, replica_id)` stamps; never raw `SystemTime` |
|| Late/duplicate/reordered deltas break convergence | Idempotent, commutative, associative join; δ-CRDT tolerates unreliable channels |
|| Offline replica blocks tombstone GC | Membership-aware causal stability; grace/quarantine window; evict dead nodes deliberately, never by timeout |
|| Resurrected deleted data (zombie records) | Never compact before all peers ack; causal-stability gate; tombstone kept while any op pending for its key |
|| Corrupt snapshot/journal | Checksums (SHA-256) on every payload; fail closed on mismatch; keep multiple historical snapshots |
|| Identifier exhaustion (sequence CRDTs) | ESBT bounded identifiers; periodic renaming/compaction; Logoot/LSEQ as bounded alternatives |
|| Byzantine/malicious metadata | Out of scope for Phase 022 (deferred to synchronization security phase); authenticate deltas at trust boundary |

---

### 5. Integration with Existing Crates

Phase 022 depends on and integrates with:

|| Crate | Role in Phase 022 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Master crate: canonical types and sealed trait contracts (`NodeId`, `ReplicaId`, `Endpoint` surface, invariants) |
|| `ids-carrier` | `Endpoint` = `CarrierType` + `Vec<u8>`; all CRDT traffic bytes ride this opaque endpoint — zero carrier assumptions |
|| `ids-crypto` | Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (signatures, commitments, content addressing, authenticated encryption) |
|| `ids-storage` | Key-value persistence (sled/memory), snapshots, compression |
|| ids-sync | Sync engine — consumes CRDT deltas for gossip/anti-entropy, drives acks back for causal-stability GC |

**Integration points:**
- ids-sync becomes the delta/anti-entropy transport for ids-crdt: ids-crdt produces `δ` bytes; ids-sync gossips them over `ids-carrier::Endpoint` and reports acks back for causal-stability GC.
- ids-storage persists CRDT snapshots + delta journal; ids-crdt defines the snapshot/journal formats (versioned, checksummed, SHA-256).
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — ids-crdt works over `ids-carrier::Endpoint` opaque bytes, preserving the carrier-agnostic architecture.
- No `std::net` / `tokio::net` anywhere in the crate; enforced by `ids-000-aa`'s prohibited-dependency list, build.rs lint, and cargo-deny CI.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-crdt` (new crate, depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; serde, thiserror, bytes, blake3, hkdf; proptest/bolero for testing)

```
ids-crdt/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (snapshot intervals, GC thresholds)
├── error.rs                        # CrdtError (thiserror, comprehensive)
│
├── lattice/                        # Join-semilattice primitives
│   ├── mod.rs
│   ├── join.rs                     # Join trait (commutative, associative, idempotent)
│   ├── semilattice.rs              # Semilattice state trait + LUB
│   └── inflation.rs                # Inflationary update guard
│
├── replica/                        # Replica identity & logical clocks
│   ├── mod.rs
│   ├── id.rs                       # ReplicaId, NodeId mapping (from ids-000-aa)
│   ├── clock.rs                    # Lamport / hybrid logical clock
│   └── stamp.rs                    # Totally-ordered (counter, replica) stamp
│
├── counters/                       # G-Counter, PN-Counter
│   ├── mod.rs
│   ├── g_counter.rs                # Grow-only counter (BTreeMap<ReplicaId,u64>)
│   ├── pn_counter.rs               # PN-Counter = GCounter × GCounter
│   └── delta.rs                    # δ-mutators for counters
│
├── sets/                           # G-Set, 2P-Set, OR-Set
│   ├── mod.rs
│   ├── g_set.rs                    # Grow-only set (merge = union)
│   ├── two_p_set.rs                # Two-phase set (added/removed G-Sets)
│   ├── or_set.rs                   # OR-Set (tags, tombstones, add-wins)
│   ├── or_set_optimized.rs         # OR-Set without tombstones (version vector)
│   └── delta.rs                    # δ-mutators for sets
│
├── registers/                      # LWW-Register
│   ├── mod.rs
│   ├── lww_register.rs             # Last-writer-wins register (HLC stamp)
│   └── delta.rs
│
├── sequence/                       # Sequence CRDTs (future extension, stubbed)
│   ├── mod.rs
│   ├── id.rs                       # Element position identifier (dot, fraction)
│   ├── rga.rs                      # RGA linked-list sequence + tombstones
│   ├── esbt.rs                     # ESBT bounded identifiers (upgrade path)
│   ├── tombstone.rs                # Tombstone tracking + re-parenting
│   └── gc.rs                       # Sequence tombstone GC (causal stability)
│
├── delta/                          # δ-CRDT machinery
│   ├── mod.rs
│   ├── mutator.rs                  # DeltaMutator trait (mδ(X) -> delta)
│   ├── buffer.rs                   # Outbound delta buffer + coalescing
│   ├── group.rs                    # Delta-group (join of several deltas)
│   └── decomposition.rs            # Join decomposition into irreducibles
│
├── sync/                           # Anti-entropy & digest exchange
│   ├── mod.rs
│   ├── state_vector.rs             # Version-vector-style state vector
│   ├── merkle.rs                   # Merkle tree digest (Merkle Search Tree option)
│   ├── digest.rs                   # Digest comparison + divergence location
│   └── anti_entropy.rs             # Anti-entropy scheduler (gossip + pull)
│
├── persistence/                    # Snapshot + journal
│   ├── mod.rs
│   ├── snapshot.rs                 # Full-state snapshot (versioned, checksummed)
│   ├── journal.rs                  # Append-only delta journal (WAL)
│   ├── compaction.rs               # Snapshot + journal compaction
│   ├── recovery.rs                 # Load snapshot + replay journal
│   └── storage.rs                  # ids-storage backend adapter (sled/memory)
│
├── gc/                             # Tombstone garbage collection
│   ├── mod.rs
│   ├── stability.rs                # Causal stability (per-peer frontiers, MVV min)
│   ├── frontier.rs                 # Peer applied/durable frontier tracking
│   └── collector.rs                # Incremental tombstone collector
│
├── wire/                           # Serialization
│   ├── mod.rs
│   ├── format.rs                   # Versioned wire envelope (CBOR/bincode)
│   ├── cbor.rs                     # CBOR encoding (deterministic)
│   └── checksum.rs                 # Payload checksums (SHA-256)
│
├── types/                          # Composed CRDT types (maps, JSON-like)
│   ├── mod.rs
│   ├── lww_map.rs                  # LWW-Map (per-key LWW-Register)
│   ├── or_map.rs                   # OR-Map (per-key OR-Set)
│   └── json_crdt.rs                # Composed JSON-like CRDT
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_merge.rs
│   ├── fuzz_sequence.rs
│   └── fuzz_persistence.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── convergence_tests.rs        # Convergence under reorder/loss/dup
│   ├── property_tests.rs           # Proptest/bolero semilattice properties
│   ├── persistence_tests.rs        # Snapshot/journal round-trip
│   ├── gc_tests.rs                 # Tombstone GC safety
│   ├── adversarial_tests.rs        # Byzantine metadata, partition healing
│   └── chaos_tests.rs              # Node churn, offline members
│
└── benches/                        # Benchmarks
    ├── mod.rs
    ├── merge_bench.rs              # Merge latency/size
    ├── delta_bench.rs              # Delta vs full-state bandwidth
    └── gc_bench.rs                 # GC pause/throughput
```

Target: ~120–200 production-grade source files, within the ~90–1000 file allowance for subfolders (tests, fuzz, benches, docs, and generated artifacts bring the total toward the upper end of the range).

---

### 7. Key Algorithms & Data Structures

| Component | Algorithm / data structure | Convergence basis |
|-----------|---------------------------|-------------------|
| G-Counter | `Map<ReplicaId, u64>`; per-replica max on merge; value = sum | Pointwise max over ℕ is a join-semilattice |
| PN-Counter | Pair of G-Counters (pos, neg); value = pos − neg | Componentwise lattice join |
| G-Set | Set; merge = set union | Union is a join-semilattice; add is idempotent inflation |
| 2P-Set | Pair of G-Sets (added, removed); membership = added \ removed | Remove-wins; tombstones grow unboundedly |
| OR-Set | `Map<Element, Set<Tag>>` + tombstone set; tags = dots `(replica, seq)` | Add-wins: a remove tombstones only observed tags; concurrent adds survive |
| LWW-Register | `(value, stamp)`; stamp = `(counter, replica_id)` HLC; merge keeps higher stamp | Total order on stamps; tie-broken by replica id |
| δ-mutator | `mδ(X) → δ` with `m(X) = X ⊔ δ` | Delta join idempotent; tolerates loss/dup/reorder |
| Anti-entropy | Merkle digest exchange + state-vector comparison | O(1) agreement check on equal roots; repair only divergent branches |
| Persistence | Versioned snapshot + append-only delta journal (WAL) | Fail-closed on hash mismatch; replay from latest snapshot |
| Tombstone GC | Causal stability = min over all peer frontiers | Compact only after all peers ack past a version |

---

### 8. Dependencies

**Permitted (per IDS policy):**
- `ids-000-aa` — canonical types, sealed trait contracts, prohibited-dependency enforcement
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`); all traffic bytes
- `ids-crypto` — ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only
- `serde`, `thiserror`, `bytes`, `blake3`, `hkdf` — serialization/error handling/crypto primitives (allowed set)
- `proptest` / `bolero` — property-based testing (test-only)

**Prohibited (enforced by `ids-000-aa` build.rs lint + cargo-deny CI):**
`std::net`, `tokio::net`, libp2p, mio, socket2, cid, multihash, ipld-core, reqwest, hyper, url, rustls, webpki — and, per IDS phase policy: IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, mesh. No concrete networking stack may appear in the dependency graph of `ids-crdt` or its transitive dependencies.

---

### 9. Measurable Acceptance Criteria

- **Merge laws:** property tests verify merge is commutative, associative, and idempotent for every CRDT class (1000+ random cases per class).
- **Convergence:** randomized sessions with late, duplicated, and reordered delta delivery converge to identical state on all replicas; every permutation of small histories enumerated and checked.
- **Per-class semantics:** G-Counter value = sum of per-replica counts; PN-Counter = pos − neg; G-Set/2P-Set/OR-Set match published add/remove semantics (add-wins for OR-Set: concurrent add beats remove).
- **δ-CRDT correctness:** `m(X) = X ⊔ δ` for every δ-mutator; delta-group join equals sum of individual delta joins.
- **Persistence:** snapshot + journal round-trip survives corruption injection; fail-closed on SHA-256 mismatch; recovery = latest snapshot + journal replay.
- **GC safety:** tombstones compacted only after all-peer ack (causal stability); no zombie records; offline members block GC until grace/quarantine window expires.
- **Carrier-agnostic:** crate compiles with zero networking dependencies; `cargo-deny` and build.rs lint pass on prohibited-dependency list.
- **Allowed crypto only:** any cryptographic primitive used is limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 10. Remaining Uncertainties

- **Op-based adoption:** whether a future op-based layer (requiring exactly-once causal delivery) is ever justified for IDS, given the carrier-agnostic mandate. Currently rejected as primary.
- **Tombstone GC with offline members:** causal-stability GC blocks on never-acking peers; membership management or quarantine window policy still to be decided.
- **LWW clock bounds:** hybrid logical clocks bound skew loosely, but liveness under extreme clock drift needs workload tuning.
- **Sequence CRDTs:** RGA vs ESBT tradeoff, tombstone GC complexity, identifier exhaustion — deferred to a later phase; not in Phase 022 contents.
- **Composed types (LWW-Map, OR-Map, JSON-like CRDT):** semantics of nested merges and conflict resolution need per-use-case decisions.
- **Byzantine metadata:** authenticated deltas and equivocation handling deferred to the synchronization security phase.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| State-based (CvRDT) + δ-CRDT hybrid as primary | Tolerates lost/dup/reordered messages over unknown carrier; no ordering guarantees needed from `ids-carrier::Endpoint` |
| Op-based rejected as primary | Requires reliable exactly-once causal broadcast — unavailable and contrary to carrier-agnostic design |
| Join-semilattice merge (ACI + inflationary updates) | Sufficient condition for strong eventual consistency (Shapiro et al. 2011) |
| HLC / Lamport stamps for LWW-Register | Raw wall clock loses concurrent writes under drift; `(counter, replica_id)` is total, unique, causal |
| OR-Set add-wins with tombstones (+ optimized tombstone-free variant) | Preserves sequential set semantics under concurrent add/remove; version-vector variant trades tombstones for per-replica state |
| PN-Counter = two G-Counters | Decrements never silently resurrected; composition of lattices is a lattice |
| Hybrid persistence (snapshot + delta journal) | Bounded recovery time; write amplification controlled by compaction thresholds |
| Tombstone GC only at causal stability | Prevents zombie records; offline members handled via grace/quarantine window |
| ids-crdt as separate crate (not folders in existing crates) | Follows IDS separate-crates pattern; clean dependency boundary on `ids-000-aa` / `ids-carrier` / `ids-crypto` |

---

### 12. Sources

- Shapiro, Preguiça, Baquero, Zawirski, "Conflict-free Replicated Data Types," 2011 (INRIA RR-7687 / "A comprehensive study of Convergent and Commutative Replicated Data Types") — state-based and op-based approaches, G-Counter, PN-Counter, G-Set, 2P-Set, LWW-Register, OR-Set, semilattice convergence condition, SEC.
- Almeida, Shoker, Baquero, "Delta State Replicated Data Types," 2018 (arxiv:1603.01529) — δ-CRDT framework, delta-mutators, anti-entropy, convergence over unreliable channels.
- "Approaches to Conflict-free Replicated Data Types," 2023 (arxiv:2310.18220) — operation-based vs state-based tradeoffs, causal consistency, optimistic replication, deterministic convergence.
- "Conflict-free Replicated Data Types: An Overview," 2018 (arxiv:1806.10254) — CRDT definition, unique identifier lifecycle, operation generation/effector, delivery guarantees.
- CMU 17-350 Lecture 10/11: Eventual Consistency and CRDTs — G-Counter, PN-Counter, 2P-Set, OR-Set mechanics; strong eventual consistency; anti-entropy; convergence testing.
- arxiv:1608.03960 — JSON CRDT (Automerge foundation): automatic convergence, no lost updates, permutation equivalence.
- arxiv:2607.28101 — ESBT: bounded identifier sequence CRDT; 50–75% lower identifier memory vs Logoot/LSEQ; proven SEC.
- docs.rs `crdt_kit`, `delta-stream` (PN-Counter, GCounter Rust implementations), `merkle-search-tree` (Merkle Search Tree anti-entropy).
- `crdt-graph` (op-based 2P2P-Graph, prepare/downstream phases), `yrs`/`y-crdt` (Yjs Rust port, delta-state sequence), Automerge (JSON CRDT, production).
- Hazelcast PN Counter documentation; Pekko DData `LWWRegister`/`ORSet`; Riak DT (production OR-Set, tombstone-free version-vector variant).
- mattweidner.com CRDT Survey (Parts 1–3): unique IDs, dot IDs, op-based semantics (prepare/effect), causal order delivery, algorithmic techniques.
- Kleppmann, "Making CRDTs Byzantine fault tolerant," PaPoC '22 — convergence not guaranteed under Byzantine participants without deterministic validation.
- Verification: Isabelle "Verifying Strong Eventual Consistency in δ-CRDTs"; Coq CRDT proofs (LWW Register 54/136, PN-Counter 88/108, etc.); type-checking CRDT convergence (2023).
- go-crdt randomized convergence testing; Ditto Rust property-based convergence tests.
