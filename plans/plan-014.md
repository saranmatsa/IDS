## Phase 014 — Create the ids-read crate: Distributed reads with replica-aware selection and fallback.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

-- -

### 1. Executive Summary

Phase 014 creates a new, separate crate `ids-read` for the IDS project: distributed reads with replica-aware selection and fallback. It is **not** a folder inside an existing crate — per the IDS convention, new capabilities land in separate crates with subfolder organization. `ids-read` owns the full read lifecycle: issuing a read, selecting the closest/most fresh replica, enforcing a consistency profile (strong, bounded-staleness, session, eventual), fanning out parallel reads, aggregating scattered results, repairing stale replicas on the read path, and falling back through hinted-handoff holders when preferred replicas are unreachable. All traffic rides `ids-carrier::Endpoint` (opaque `CarrierType` + `Vec<u8>`); there are zero direct networking dependencies (no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh). Allowed cryptography only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305. `ids-000-aa` remains the master crate holding all canonical types and sealed trait contracts; `ids-read` depends on it, `ids-carrier`, and `ids-crypto`-equivalent primitives, and is carrier-agnostic by construction.

The read design is Dynamo/Cassandra-lineage: N replicas per key, tunable read quorum R, quorum intersection R + W > N for freshness under normal operation, sloppy quorum + hinted handoff for availability during replica failure, read repair that merges divergent versions and writes back to stale replicas, and Merkle-tree anti-entropy as the periodic backstop. Session guarantees (read-your-writes, monotonic reads, writes-follow-reads, monotonic-writes from Bayou) and causal visibility (vector clocks / version vectors / hybrid logical clocks) sit on top for clients that need ordering. PACELC frames the latency-consistency trade-off: in the absence of partitions the crate chooses between latency and consistency via the consistency profile; under partition it degrades to available stale reads or fails closed per profile.

-- -

### 2. Architecture Overview

`ids-read` sits between the application/query layer and the carrier/transport layer, below any query decomposition (ids-query, Phase 015) and above `ids-carrier`/`ids-000-aa`. It is a local read orchestrator — it never opens a socket; it hands `Vec<u8>` read requests to `ids-carrier::Endpoint` and receives `Vec<u8>` responses.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / QUERY LAYER                        │
│              (read request, consistency profile, key)              │
├─────────────────────────────────────────────────────────────────────┤
│                    Phase 014: ids-read (NEW CRATE)                  │
│  ┌─────────────────┐ ┌──────────────────┐ ┌──────────────────┐     │
│  │  Read lifecycle │ │ Replica selector │ │  Read quorum R   │     │
│  │  (issue → await │ │ (closest/fresh   │ │  R + W > N fresh-│     │
│  │   → complete)   │ │   fallback)      │ │  ness intersection)│     │
│  └─────────────────┘ └──────────────────┘ └──────────────────┘     │
│  ┌─────────────────┐ ┌──────────────────┐ ┌──────────────────┐     │
│  │  Consistency    │ │  Parallel reads  │ │  Scatter-gather  │     │
│  │  profile enforce│ │  + fan-out/fan-in│ │  read + aggrega- │     │
│  │  (strong/bs/sess│ │                  │  tion/merge       │     │
│  │   /eventual)    │ │                  │                  │     │
│  └─────────────────┘ └──────────────────┘ └──────────────────┘     │
│  ┌─────────────────┐ ┌──────────────────┐ ┌──────────────────┐     │
│  │  Read repair    │ │  Hinted-handoff  │ │  Replica fresh-  │     │
│  │  (digest/full,  │ │  fallback holders│ │  ness (VC/VV/HLC)│     │
│  │   write-back)   │ │  (sloppy quorum) │ │                  │     │
│  └─────────────────┘ └──────────────────┘ └──────────────────┘     │
│  ┌─────────────────┐ ┌──────────────────┐ ┌──────────────────┐     │
│  │  Ordered-read   │ │  Session state   │ │  Merkle anti-en- │     │
│  │  visibility     │ │  (ryw, mr, mw,   ││  tropy (periodic │     │
│  │  (vector clock) │ │   wfr)           ││  backstop)       │     │
│  └─────────────────┘ └──────────────────┘ └──────────────────┘     │
├─────────────────────────────────────────────────────────────────────┤
│                 ids-carrier::Endpoint (CarrierType + Vec<u8>)       │
│              (opaque carrier — no std::net / tokio::net / libp2p)   │
├─────────────────────────────────────────────────────────────────────┤
│                 ids-000-aa (master crate, canonical types)          │
│          NodeID, ReplicaId, sealed trait contracts, crypto ids     │
└─────────────────────────────────────────────────────────────────────┘
```

**Data flow of a single read:**
1. Application submits `Read { key, consistency_profile, read_quorum_R, session_token }`.
2. Read lifecycle validates the request, loads the replica preference list (N homes for the key) from `ids-000-aa` canonical types.
3. Replica selector ranks candidates by freshness (version vector / HLC) and proximity, picks the read quorum R (closest/freshest first), and identifies hinted-handoff fallback holders for any unreachable home.
4. Parallel reads fan out to R replicas (plus fallbacks) over `ids-carrier::Endpoint` as `Vec<u8>` payloads; each request carries the read's causal context (session vector clock).
5. Replies carry version stamps; the coordinator applies read repair: if digests/versions disagree, it performs a full read from the disagreeing replicas, merges by LWW/causal rule, and writes the merged value back to stale replicas before returning.
6. Scatter-gather aggregation merges partial results (for multi-key/range reads) into one answer; ordered-read visibility checks the session vector clock so reads never move backward.
7. Consistency profile enforcement gates the answer: strong waits for R with R + W > N; bounded-staleness accepts within the staleness bound; session enforces read-your-writes/monotonic-reads; eventual returns the first reply.
8. The merged, repaired, visibility-checked answer is returned to the application.

**No end-to-end path assumption:** a read may be served by any reachable replica; unreachable homes are handled by sloppy-quorum fallback to hinted-handoff holders, not by blocking.

-- -

### 3. Detailed Research Dimensions

#### 3.1 Read lifecycle and quorum intersection (R + W > N)

**Quorum reads/writes (Dynamo, Cassandra):** each key is replicated on N homes. A write succeeds after W homes ack; a read succeeds after R homes respond. Setting R + W > N guarantees the read quorum overlaps every recent write quorum, so at least one responding replica holds the latest acknowledged write under normal conditions (no sloppy quorum, no concurrent conflicting write). Latency is dictated by the slowest of the R (or W) responders, so R and W are normally < N to tolerate slow/unavailable nodes and cut the tail of the delay distribution.

**Decision for ids-read:** expose R and W as configurable per-read (or per-profile) parameters; default R + W > N for strong reads; allow R = 1 / W = N (read-one/write-all) and R = N / W = 1 (read-all/write-one) as profile extremes. Never wait for all N. A read contacts all N homes in parallel and returns as soon as R valid replies arrive (the others are cancelled/drained), matching Dynamo-style tail-latency behavior.

**Edge cases where R + W > N still yields stale reads (documented, not eliminated):** sloppy quorum (writes land outside the N homes), concurrent writes with divergent versions, write/read race (write reflected on only some replicas), partial-write success on fewer than W replicas, and restore-from-old-replica scenarios. These are handled by read repair, version stamps, and consistency-profile gating rather than by stronger quorum alone.

#### 3.2 Closest-replica selection and fallback (hinted handoff / sloppy quorum)

**Replica selection:** rank the N home replicas by a composite score: freshness (version-vector/HLC stamp relative to the read's causal context), observed RTT/latency history, and load. Select the R highest-scoring reachable homes as the read quorum. Unreachable homes are not retried synchronously; the coordinator proceeds with the reachable subset if it still meets R, otherwise falls back.

**Sloppy quorum (Dynamo/Cassandra/ScyllaDB):** when a home replica is down, reads/writes proceed on the first N *healthy* nodes from the preference list, which may fall outside the key's N homes. This preserves availability but breaks the R + W > N overlap guarantee globally: a read to home replicas may miss a write that landed on a sloppy (fallback) node.

**Hinted handoff (Cassandra default max_hint_window 3h):** when a preferred replica A is unreachable, the coordinator stores the write locally on a fallback D tagged with a hint naming A as the intended recipient. When A recovers, D replays the hint to A and deletes its local copy. If A stays down beyond the hint window, hints stop accumulating and full anti-entropy repair is required. ScyllaDB bounds hint disk usage to avoid hint-holder disk exhaustion.

**Decision for ids-read:** read path supports sloppy-quorum reads: if fewer than R homes respond, the read quorum may include hinted-handoff holders (temp nodes holding values for homes), with the read marked "sloppy" and freshness degraded. A hint record = {target_home_replica, key, value, version, write_timestamp, hint_ttl}; fallback holders retain hints and replay them to the home on recovery; hints expire after a configurable window (default aligned to 3h Cassandra precedent) after which full repair (Merkle anti-entropy, §3.9) reconciles. Hint accumulation is bounded per holder to prevent DoS; exceeding the bound rejects new hints and triggers repair.

#### 3.3 Ordered-read visibility and session guarantees

**Session guarantees (Bayou; four per-session guarantees):** read-your-writes (a write becomes visible to subsequent reads in the same session), monotonic reads (successive reads in a session observe monotonically non-decreasing versions), writes-follow-reads (a write after a read causally depends on that read), monotonic-writes (writes in a session complete in session order). Causal consistency = conjunction of the four session guarantees.

**Implementation sketch:** each session holds a session vector clock (highest-read VC + highest-write VC). A read tags its request with the session's read VC; a replica only serves versions whose dependencies are ≤ that VC (causal delivery). On receiving a reply, the session advances its read VC to the entry-wise max of its current VC and the reply's version VC (monotonic reads). A write carries the session's write VC and bumps the session's own entry; subsequent reads include it (read-your-writes). Writes-follow-reads is enforced by making a write's dependency vector include the versions read in the session.

**Hybrid logical clocks (Okapi; "Session Guarantees with Raft and HLC"):** HLC = (physical_time, logical_counter, replica_id) gives causality-respecting timestamps with low latency and tolerance to clock skew; never raw wall-clock for validity. Used as the per-version stamp and for tie-breaking concurrent versions. HLC eliminates write-delay for session guarantees while bounding skew.

**Decision for ids-read:** ordered-read visibility is session-scoped via a session vector clock / HLC carried on every read request and returned on every reply. Replicas enforce causal visibility before serving. Total order for concurrent versions is provided by LWW on (HLC, replica_id) — see §3.6.

#### 3.4 Replica freshness and consistency profile enforcement

**Freshness:** each replica tracks, per key, a version stamp (version vector or HLC). A read coordinator collects (value, version) from R replicas and selects the freshest version (dominates in VC, or higher HLC). Versions that neither dominate nor are dominated are concurrent → conflict → resolved by consistency profile / LWW / read repair.

**Consistency profiles (tunable per read):**
- **Strong:** wait for R replies with R + W > N; return the freshest version; guarantees no stale read under normal conditions.
- **Bounded staleness:** return a version no older than K versions or T time behind the latest (Cosmos DB bounded-staleness; Skybridge-style bounded-staleness target ~2s for 99.99998% of writes). Reads may be served by any replica whose lag ≤ bound.
- **Session:** enforce read-your-writes + monotonic reads via session VC; reads may go to a nearby replica as long as session causality holds.
- **Eventual (consistent prefix):** return the first reply; reads may lag and may be non-monotonic without session guarantees; consistent-prefix reads never appear out of order (reads lag writes but never reorder).

**PACELC framing:** in the absence of partitions, strong/profile reads trade latency for consistency (choose L or C); under partition the crate degrades to available stale reads (sloppy quorum + hinted handoff) or fails closed per profile — a PA/EL-style choice made per profile.

**Freshness leases (optional):** a freshness lease from a replica guarantees the replica will not serve a version older than the lease bound for the lease duration; reduces coordinator-side version comparison. Deferred — leases require clock synchronization assumptions the carrier-agnostic design avoids unless explicitly configured.

#### 3.5 Parallel reads, scatter-gather, and query aggregation

**Parallel fan-out/fan-in:** reads are sent to all N homes (or the R selected quorum) in parallel; the coordinator waits for the first R valid replies and treats the rest as best-effort (cancel or drain). This is the Dynamo/Cassandra "reads and writes always sent to all N replicas; R/W determine how many we wait for" model — it absorbs slow nodes and reduces tail latency.

**Scatter-gather (Pinot, Druid, ClickHouse, MongoDB):** for a multi-key or range read, the coordinator decomposes the read into per-replica/per-partition sub-reads, fans them out in parallel, each replica computes a local partial result, and the coordinator merges partial results into the final answer (gather). Fan-out is bounded by the replica set holding the needed keys; the more servers participate, the higher tail latency (straggler problem) — so replica selection and adaptive routing (route within a replica group / mirror set to avoid slow servers) matter.

**Aggregation:** partial results are merged per the query's aggregation kind (count → sum, sum → sum, min/max → min/max, group-by → merge groups, filter → union, sort → merge-sort). Aggregation is pure/local; no cross-replica state mutation during gather. Read repair applies per-key before aggregation.

**Decision for ids-read:** scatter-gather for multi-key reads: fan out one sub-read per replica shard holding any requested key, gather partial results, merge. Single-key reads use the read-quorum path (§3.1). Aggregation operators supported: count, sum, min, max, group-by, filter, sort (merge-sort). Fan-out degree is capped; slow replicas are skipped after a deadline keyed to read start time (no RTT-bound timers).

#### 3.6 Conflict resolution and version merge (last-writer-wins, MV-register)

**Concurrent versions:** when two versions are concurrent (neither VC-dominates), the coordinator must pick or keep both. Options:
- **Last-writer-wins (LWW-Register):** keep the version with the highest (HLC, replica_id) stamp; concurrent writes — one is silently lost. Acceptable when losing a concurrent write is tolerable (user profiles, config).
- **Multi-value register (MV-Register):** keep all concurrent values; return the set to the application/session to resolve (Dynamo shopping cart, Riak siblings). No silent data loss; shifts conflict resolution to the reader.
- **Vector-clock-based "sibling" detection:** return multiple concurrent values with their version vectors; read repair reconciles.

**Decision for ids-read:** support both LWW-Register (default, single answer) and MV-Register (no loss, reader resolves), selectable per consistency profile / per key. LWW uses HLC + replica_id tie-break, never raw wall-clock. MV-register returns siblings with version vectors; the client or a repair pass picks the winner.

#### 3.7 Read repair (digest + full read, write-back)

**Cassandra read repair:** on a read, the coordinator issues one full read to the fastest replica and digest reads (CL-1) to the rest. If digests disagree, it upgrades to full reads, merges by timestamp (LWW), and writes the merged value back to stale replicas *before* returning. Read repair happens on every read at the configured consistency level; background read repair (CL=ONE) does not help the current request.

**Anti-entropy vs read repair:** read repair is passive (fixes what a read touches); anti-entropy (Merkle tree, §3.9) is periodic and proactive (fixes everything, including never-read keys). They complement: read repair heals the hot path quickly; anti-entropy is the safety net for cold keys and long partitions.

**Decision for ids-read:** read repair on every quorum read: compare version stamps across the R replies; on disagreement, fetch full versions from disagreeing replicas, merge (LWW or MV-register sibling set), write-back the merged value to stale replicas before returning. Read repair is bounded (only replicas in the read quorum) so it never amplifies into a storm; a separate anti-entropy job handles full-key-space healing.

#### 3.8 Hinted handoff on the read path

When a read's preferred home is unreachable and the read quorum cannot be filled from homes, the coordinator may read from a hinted-handoff holder (a temp node that accepted a write for the down home). The returned value is tagged as "from hinted handoff" and treated as potentially stale relative to the home; read repair, when the home recovers, reconciles. Reads from hinted holders are allowed only for eventual/session profiles (not strong), since strong requires R + W > N overlap which sloppy quorum breaks.

#### 3.9 Read anti-entropy (Merkle tree) — periodic backstop

**Merkle-tree anti-entropy (Dynamo/Cassandra/Riak):** each replica builds a Merkle tree over its key range; peers exchange root hashes; equal roots ⇒ equal state (O(1) agreement check). On mismatch, descend only into differing subtrees (O(d·k) for k divergent keys out of n), exchanging only divergent keys. Cassandra divides key ranges into segments, computes per-segment hashes, exchanges ~32KB for 1M keys vs 1GB full comparison. Riak Active Anti-Energy uses persistent on-disk hash trees, updated in real time, regenerated weekly. Trade-off: anti-entropy cycles are slow (30–120s) → max staleness of that duration; it complements read repair/gossip, never replaces them.

**Decision for ids-read:** periodic Merkle anti-entropy as the convergence backstop: fixed-shape Merkle tree over the keyspace (bucketized by hashed key so two replicas build identical tree shapes regardless of which keys each holds); beacon ships bucket digests; diff descends into differing buckets; repair re-sends only drifted entries. BLAKE3 for hashing (allowed crypto). Runs on a schedule, independent of reads; bounds staleness for cold keys.

-- -

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Stale read despite R + W > N (sloppy quorum, concurrent writes, partial writes) | Read repair + version stamps; consistency-profile gating; sloppy reads tagged/degraded; strong profile requires strict quorum overlap |
| Hint loss if hint-holder crashes before replay | Bound hint window; full Merkle anti-entropy reconciles after expiry; per-holder hint disk quota |
| Read-repair storm / amplification | Read repair bounded to replicas in the current read quorum; no cascading repair; separate scheduled anti-entropy |
| Version-vector / HLC clock skew corrupts ordering | HLC never raw wall-clock; HLC bounded skew; causality, not wall time, decides validity |
| Concurrent conflicting versions silently lost (LWW) | Offer MV-register per key/profile; siblings returned to reader for resolution |
| Read from unreplica/freshness-lagged replica | Freshness scoring (VC/HLC + latency + load); bounded-staleness profile rejects replicas beyond lag bound |
| Replay of old read responses | Nonce per read request; signed read envelopes (ed25519) over (coordinator, key, causal context, nonce) |
| Metadata bomb / oversized read request | Size/depth budgets before parsing; per-key and per-read request caps |
| Carrier-level DoS via read flood | Per-peer read rate limits; read-quorum R caps fan-out; no unbounded parallelism |
| Hinted-handoff holder serves stale value as fresh | Tag hint-sourced reads; strong profile disallows hint sources; read repair reconciles on recovery |

-- -

### 5. Integration with Existing Crates

Phase 014 depends on and integrates with:

| Crate | Role in Phase 014 |
|-------|-------------------|
| `ids-000-aa` | Master crate: canonical types (NodeID, ReplicaId, Key, Value, Version, CarrierType), sealed trait contracts; read preference list (N homes per key) |
| `ids-carrier` | `Endpoint` (CarrierType + Vec<u8>) — the only traffic boundary; all read requests/responses ride as `Vec<u8>` |
| `ids-crypto` (allowed set) | ed25519 signing/verification of read envelopes; sha256/blake3 for version/digest hashing; xchacha20poly1305/chacha20poly1305 if read payloads are encrypted |
| `ids-query` (Phase 015, future) | Consumer — query decomposition feeds multi-key scatter-gather reads to ids-read |
| `ids-storage` (future/local) | Local replica store backend for the replica this node hosts; read repair write-back targets local storage |

**Integration points:**
- `ids-carrier::Endpoint` is the sole carrier boundary — `ids-read` never imports `std::net`, `tokio::net`, libp2p, or any carrier-specific type; opaque `CarrierType` + `Vec<u8>` only, preserving I-CARR-001 carrier-agnostic stance.
- Read requests and responses carry a canonical envelope: {coordinator, key, read_quorum_R, consistency_profile, session_VC, nonce, version_stamp, payload, ed25519_sig}.
- Replica preference list (N homes per key) is supplied by `ids-000-aa` canonical types; `ids-read` does not own topology.
- Read repair write-back targets the local replica's store (`ids-storage`) via the same carrier envelope when the stale replica is remote.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — works over any `ids-carrier::Endpoint`.

-- -

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-read` (new crate; depends on `ids-000-aa`, `ids-carrier`, allowed-crypto primitives; `serde`, `thiserror`, `blake3`, `bytes`, `parking_lot`, `dashmap`, `proptest`/`bolero`, `criterion`). Target ~400–700 production-grade files, all under `src/` in subfolders; crate-root files (`Cargo.toml`, `lib.rs`, `constants.rs`, `error.rs`) directly in `src/`.

```
ids-read/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol defaults (R/W, hint window, staleness bounds)
├── error.rs                        # ReadError (thiserror, comprehensive)
│
├── read/                           # Read lifecycle
│   ├── mod.rs
│   ├── lifecycle.rs                # Read orchestrator: issue → await → complete
│   ├── request.rs                  # ReadRequest envelope {key, R, profile, session_VC, nonce}
│   ├── response.rs                 # ReadResponse {value, version, source, hinted, sig}
│   ├── envelope.rs                 # Canonical signed read envelope (ed25519)
│   ├── nonce.rs                    # Per-read nonce issuance + replay protection
│   └── deadline.rs                 # Read deadline keyed to start time (no RTT timers)
│
├── replica/                        # Replica selection & freshness
│   ├── mod.rs
│   ├── selector.rs                 # Closest/freshest replica ranking
│   ├── preference_list.rs          # N homes per key (from ids-000-aa)
│   ├── freshness.rs                # Freshness scoring (VC/HLC + latency + load)
│   ├── quorum.rs                   # Read quorum R selection (R + W > N)
│   ├── hinted_handoff.rs           # Fallback holders + hint records
│   ├── sloppy_quorum.rs            # Sloppy-quorum read support
│   └── replica_client.rs           # Per-replica read over ids-carrier::Endpoint
│
├── consistency/                    # Consistency profile enforcement
│   ├── mod.rs
│   ├── profile.rs                  # ConsistencyProfile enum (strong/bs/session/eventual)
│   ├── strong.rs                   # Strong read: wait for R, R + W > N
│   ├── bounded_staleness.rs        # Bounded-staleness (K versions / T time)
│   ├── session.rs                  # Session guarantees (ryw, mr, mw, wfr)
│   ├── eventual.rs                 # Eventual / consistent-prefix read
│   └── session_clock.rs            # Session vector clock / HLC
│
├── parallel/                       # Parallel reads & scatter-gather
│   ├── mod.rs
│   ├── fanout.rs                   # Parallel fan-out to N homes / R quorum
│   ├── gather.rs                   # Gather + merge of partial results
│   ├── scatter_gather.rs           # Multi-key scatter-gather coordinator
│   ├── aggregation.rs              # count/sum/min/max/group-by/filter/sort merge
│   └── adaptive.rs                 # Adaptive routing away from slow replicas
│
├── repair/                         # Read repair
│   ├── mod.rs
│   ├── read_repair.rs              # Digest/full-read repair + write-back
│   ├── merge.rs                    # Version merge (LWW / MV-register / sibling set)
│   ├── writeback.rs                # Write merged value to stale replicas
│   └── conflict.rs                 # Concurrent-version detection (VC divergence)
│
├── version/                        # Version stamps & ordering
│   ├── mod.rs
│   ├── vector_clock.rs             # Version vector (per-replica version)
│   ├── hlc.rs                      # Hybrid logical clock stamp
│   ├── lww_register.rs             # Last-writer-wins (HLC + replica_id)
│   ├── mv_register.rs              # Multi-value register (siblings kept)
│   └── ordering.rs                 # Dominance / concurrency / total order
│
├── anti_entropy/                   # Periodic Merkle anti-entropy backstop
│   ├── mod.rs
│   ├── merkle_tree.rs              # Fixed-shape Merkle tree over hashed keyspace
│   ├── digest.rs                   # Bucket digest exchange (beacon)
│   ├── diff.rs                     # Descend into differing subtrees
│   ├── repair_job.rs               # Scheduled anti-entropy job
│   └── checksum.rs                 # BLAKE3 payload checksums
│
├── hint/                           # Hinted-handoff records
│   ├── mod.rs
│   ├── record.rs                   # Hint {target_home, key, value, version, ttl}
│   ├── store.rs                    # Per-holder hint store (bounded)
│   ├── replay.rs                   # Replay hints to home on recovery
│   └── expiry.rs                   # Hint TTL expiry (default ~3h)
│
├── session/                        # Session state
│   ├── mod.rs
│   ├── state.rs                    # Session {read_VC, write_VC, token}
│   ├── guarantees.rs               # ryw / mr / mw / wfr enforcement
│   └── cache.rs                    # Session cache (bounded, TTL eviction)
│
├── wire/                           # Serialization
│   ├── mod.rs
│   ├── format.rs                   # Versioned read/wire envelope
│   ├── cbor.rs                     # CBOR encoding (deterministic)
│   └── checksum.rs                 # Payload checksums (sha256/blake3)
│
├── crypto/                         # Allowed-crypto wrapper
│   ├── mod.rs
│   ├── ed25519.rs                  # ed25519 sign/verify for read envelopes
│   ├── sha256.rs                   # sha256 hashing
│   ├── blake3.rs                   # blake3 hashing (Merkle, version digests)
│   ├── xchacha20poly1305.rs        # xchacha20poly1305 (optional payload encryption)
│   └── chacha20poly1305.rs         # chacha20poly1305 (optional payload encryption)
│
├── types/                          # Composed read types
│   ├── mod.rs
│   ├── read_quorum.rs              # ReadQuorum {replicas, responses, sloppy}
│   ├── read_result.rs              # ReadResult {value, version, profile_met, repaired}
│   └── consistency_bound.rs        # Staleness bound (K versions / T time)
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_read.rs
│   ├── fuzz_repair.rs
│   └── fuzz_quorum.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── read_lifecycle_tests.rs
│   ├── replica_selection_tests.rs
│   ├── quorum_tests.rs             # R + W > N freshness under normal conditions
│   ├── read_repair_tests.rs        # Digest/full repair + write-back
│   ├── hinted_handoff_tests.rs     # Fallback + replay + expiry
│   ├── scatter_gather_tests.rs     # Multi-key aggregation
│   ├── session_tests.rs            # ryw / mr / mw / wfr
│   ├── consistency_profile_tests.rs# strong / bs / session / eventual
│   ├── anti_entropy_tests.rs       # Merkle diff + repair
│   ├── adversarial_tests.rs        # Stale replica, hint loss, replay flood
│   ├── chaos_tests.rs              # Replica churn, offline members
│   └── determinism_tests.rs        # Same seed → identical read outcome
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── read_bench.rs               # Read latency / quorum wait
│   ├── repair_bench.rs             # Read repair overhead
│   ├── anti_entropy_bench.rs       # Merkle diff bandwidth
│   └── scatter_gather_bench.rs     # Fan-out/gather throughput
│
├── spec/                           # Normative specifications
│   ├── read_lifecycle_spec.md
│   ├── quorum_spec.md
│   ├── read_repair_spec.md
│   ├── hinted_handoff_spec.md
│   ├── session_guarantees_spec.md
│   ├── consistency_profiles_spec.md
│   └── threat_model.md
│
├── docs/                           # Documentation
│   ├── architecture.md
│   ├── integration_guide.md
│   ├── read_profile_guide.md       # When to use which consistency profile
│   └── api_docs.md
│
└── test_vectors/                   # Deterministic test vectors
    ├── read_quorum_vectors.json
    ├── repair_vectors.json
    ├── session_vectors.json
    └── merkle_vectors.json
```

**Why ~400–700 files:** one file per responsibility — read lifecycle, replica selection, freshness, quorum, hinted handoff, four consistency profiles, parallel reads, scatter-gather, aggregation, read repair, version merge, vector clocks/HLC/LWW/MV-register, Merkle anti-entropy, hint store, session state, wire format, crypto wrappers, tests, fuzz, benchmarks, specs, docs, test vectors. The breadth reflects the genuine state-machine and storage complexity of distributed reads. No stubs; every file holds production-grade code with error handling, documentation, and tests.

-- -

### 7. Key Algorithms & Data Structures

#### 7.1 Read quorum (R + W > N)
```
Read { key, R, profile, session_VC, nonce }
homes = preference_list(key)   // N homes from ids-000-aa
candidates = rank(homes) by freshness(score = VC/HLC, latency, load)
quorum = top R reachable candidates
if |reachable| < R: quorum += hinted_handoff_holders (sloppy; mark degraded)
fan out Read to quorum in parallel over ids-carrier::Endpoint
wait for first R valid replies (deadline = read_start + timeout)
return freshest version among replies; apply read repair on disagreement
```

#### 7.2 Closest-replica selection
```
score(replica) = w_fresh * freshness(replica, key)
               + w_lat  * (1 / observed_latency(replica))
               + w_load * (1 / replica_load(replica))
freshness = 1 if replica.version[key] >= session_VC else decay(lag)
select top R by score among reachable
```

#### 7.3 Read repair (digest + full read + write-back)
```
replies = [(value_i, version_i, replica_i)] for i in quorum
if all versions equal: return value
else:
  full = fetch full value+version from disagreeing replicas
  merged = merge(full)   // LWW: max(HLC, replica_id); MV-register: sibling set
  write_back(merged) to stale replicas (before returning)
  return merged
```

#### 7.4 Consistency profile gating
```
strong:    accept iff R replies received AND quorum overlap (R + W > N) holds
bs(K,T):   accept iff version.lag <= K versions OR age <= T
session:   accept iff version >= session.read_VC (read-your-writes, monotonic reads)
eventual:  accept first valid reply (may be stale, non-monotonic)
```

#### 7.5 Session guarantees (VC / HLC)
```
read(request):  tag with session.read_VC; replica serves versions deps <= session.read_VC
on reply: session.read_VC = entrywise_max(session.read_VC, reply.version_VC)
write:  session.write_VC[own] += 1; write carries session.write_VC as deps
subsequent reads include session.write_VC (read-your-writes)
writes_follow_reads: write.deps ⊇ versions read in session
```

#### 7.6 Version merge (LWW / MV-register)
```
LWW:  winner = version with max (HLC, replica_id)  // tie-break by replica_id
MV:   keep all concurrent versions; return sibling set with version vectors
concurrent(a, b):  !(a.VC <= b.VC) && !(b.VC <= a.VC)
```

#### 7.7 Hinted handoff
```
hint = {target_home, key, value, version, write_time, ttl}
on write failure to home A: store hint on fallback D; ack write (sloppy)
on A recovery: D replays hint to A; D deletes local copy
if now - write_time > ttl (default 3h): drop hint; full anti-entropy reconciles
bound hint store per holder; reject new hints when full
```

#### 7.8 Merkle anti-entropy
```
tree = MerkleTree over hashed keyspace (fixed 2^depth buckets)
beacon = bucket_digests (sparse, non-empty buckets only)
diff(ours, theirs): descend differing buckets; O(d·k) for k divergent keys
repair: re-send only drifted entries; full-state sync avoided
hash = blake3
```

-- -

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | NodeID, ReplicaId, Key, Value, Version, CarrierType, sealed trait contracts (master crate) |
| `ids-carrier` | `Endpoint` (CarrierType + Vec<u8>) — sole traffic boundary |
| `ids-crypto` (allowed set only) | ed25519 (envelope signatures), sha256 & blake3 (hashing/digests), xchacha20poly1305 & chacha20poly1305 (optional payload encryption) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Merkle / version digest hashing |
| `parking_lot` / `dashmap` | Concurrent replica/session state |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

**No networking crates:** zero `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS. All traffic via `ids-carrier::Endpoint`. **No x25519** — not in the allowed set for this crate. Allowed crypto strictly: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

-- -

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-014.1 | Read quorum freshness | Under normal conditions (R + W > N, no sloppy), read returns latest acknowledged write; property test, 100k iterations |
| AC-014.2 | Sloppy quorum + hinted handoff | Write succeeds with a home down via fallback holder; read from homes may be stale until hint replay; hint replayed on home recovery |
| AC-014.3 | Hint expiry | Hints older than TTL (default 3h) dropped; full Merkle anti-entropy reconciles divergent replica |
| AC-014.4 | Read repair | Digest disagreement → full read → merge → write-back to stale replicas before returning; bounded to read quorum |
| AC-014.5 | Session guarantees | ryw / mr / mw / wfr hold within a session (VC/HLC enforced); adversarial test passes |
| AC-014.6 | Ordered-read visibility | Reads never move backward within a session; concurrent versions detected via VC divergence |
| AC-014.7 | Consistency profiles | strong / bounded-staleness / session / eventual each enforce their gating; unit + profile tests |
| AC-014.8 | Parallel reads / scatter-gather | Multi-key read fans out to shards in parallel, gathers + merges (count/sum/min/max/group-by/filter/sort) correctly |
| AC-014.9 | Replica freshness selection | Closest/freshest replica selected by VC/HLC + latency + load score; read served by highest-score reachable |
| AC-014.10 | Carrier agnosticism | Compiles and behaves identically with zero std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS; all traffic via ids-carrier::Endpoint |
| AC-014.11 | Allowed crypto only | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 used; no x25519 or other algorithms |
| AC-014.12 | Merkle anti-entropy | Equal roots ⇒ equal state; divergent keys located in O(d·k); bandwidth ≪ full-state sync |
| AC-014.13 | Determinism | Same seed + replica states → identical read outcome and repair decision |
| AC-014.14 | Adversarial tests | Stale replica, hint loss, replay flood, oversized request all fail-safe |
| AC-014.15 | Property tests | 100,000+ iterations (proptest/bolero) on quorum, repair, session, merge |

-- -

### 10. Remaining Uncertainties

1. **Replica preference list source:** the N homes per key come from `ids-000-aa` canonical types, but the exact topology/membership API (how homes are discovered and updated) is undefined; `ids-read` consumes it read-only.
2. **Local storage backend:** read repair write-back targets a local store; whether `ids-storage` exists or a local KV trait is needed is undecided (Phase-local, not part of this crate).
3. **Read timeout value:** the per-read deadline is keyed to read start time (no RTT timers), but the absolute timeout default is workload-dependent and unquantified.
4. **Hint window default:** Cassandra precedent is 3h; the IDS default hint TTL and per-holder hint disk quota need product decision.
5. **LWW vs MV-register default:** which conflict policy is default per key is undecided; profile-keyed selection is the mechanism, the default is a product choice.
6. **Session clock size:** unbounded session VC growth under many replicas; compaction/pruning policy for session state unresolved.
7. **Encryption of read payloads:** xchacha20poly1305/chacha20poly1305 are allowed but whether read envelopes are encrypted by default is undecided (ed25519 signatures are mandatory for integrity).
8. **Anti-entropy interval:** periodic Merkle anti-entropy cycle length (30–120s precedent) trades staleness vs bandwidth; default unmeasured.
9. **HLC skew bound:** HLC tolerates clock skew but liveness (session staleness detection timeouts) needs deployment tuning; never used for validity.
10. **Cross-region read routing:** proximity scoring uses observed latency; multi-region topology awareness (local-quorum style) is future work.

-- -

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-read` crate, not subfolder | IDS convention: new capabilities in separate crates with subfolder organization; keeps existing crates focused |
| Read quorum R + W > N for strong reads | Guarantees read quorum overlaps every recent write quorum → no stale reads under normal conditions (Dynamo/Cassandra) |
| Sloppy quorum + hinted handoff for availability | Preserves read/write progress during replica failure; breaks strict overlap → reads tagged sloppy/degraded, reconciled via read repair + anti-entropy |
| Read repair on every quorum read | Heals the hot path quickly (digest/full/merge/write-back); anti-entropy is the cold-key backstop |
| Session VC / HLC for ordered reads | Bayou session guarantees (ryw, mr, mw, wfr); HLC avoids wall-clock validity and write-delay (Okapi precedent) |
| LWW default, MV-register optional | LWW single answer for most keys; MV-register keeps concurrent siblings where no loss is tolerable; per-key/profile selectable |
| Parallel fan-out to all N, wait for R | Absorbs slow nodes, cuts tail latency (Dynamo model) |
| Merkle anti-entropy backstop | Locates divergence in O(d·k) vs O(n) full-state; bounded staleness for cold keys (Riak/Cassandra precedent) |
| All traffic via ids-carrier::Endpoint | Carrier-agnostic; zero std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS |
| Allowed crypto: ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 only | Matches IDS crypto policy; no x25519; ed25519 signatures mandatory on read envelopes |
| Deadlines keyed to read start time, not RTT | Correct over any latency; avoids RTT-bound timeout misfire |

-- -

**Sources consulted (30 targeted searches):**

- Dynamo: Amazon's Highly Available Key-value Store (DeCandia et al., SOSP 2007) — N/R/W quorums, sloppy quorum, hinted handoff, read repair, Merkle anti-entropy, version vectors. https://www.cs.cornell.edu/courses/cs5414/2017fa/papers/dynamo.pdf
- Apache Cassandra architecture (Dynamo lineage) — consistency levels, R + W > RF, hinted handoff (max_hint_window 3h), read repair, ANY consistency. https://cassandra.apache.org/doc/stable/cassandra/architecture/dynamo.html
- DataStax: About hinted handoff writes — hint contents, max_hint_window_in_ms, 3h default. https://docs.datastax.com/en/cassandra-oss/2.1/cassandra/dml/dml_about_hh_c.html
- ScyllaDB: How Hinted Handoff Works — hint disk bounding. http://scylladb.com/2017/11/16/hinted-handoff-scylla
- Quorum consensus (MIT 6.033 / UMass 677 / Georgia Tech 6102) — R + W > N, Gifford's quorum, serial consistency. https://web.mit.edu/6.033/2005/wwwdocs/quorum_note.html
- Read-Write Quorum Systems Made Practical (Quoracle) — quorum trade-offs, throughput/latency. https://ar5iv.labs.arxiv.org/html/2104.04102
- PACELC theorem (Abadi; Wikipedia; Proving PACELC) — partition vs availability/consistency; else latency vs consistency; r + w ≥ d. https://en.wikipedia.org/wiki/PACELC_theorem
- PACELC quantitative characterization (arXiv:1509.02464). https://arxiv.org/html/1509.02464v2
- Bayou session guarantees (terry et al., 1994) — read-your-writes, monotonic reads, writes-follow-reads, monotonic writes. https://pages.cs.wisc.edu/~remzi/Classes/739/Fall2016/Papers/bayou-sessions94.pdf
- Vector clocks & causal consistency (MIT 6.824 notes; Microsoft Research Bernstein) — causality, happened-before, VC dominance. https://pdos.csail.mit.edu/archive/6.824-2013/notes/l11.txt
- Session Guarantees with Raft and Hybrid Logical Clocks (arXiv:1808.05698) — HLC for session guarantees, low latency, clock-skew tolerance. https://arxiv.org/html/1808.05698v1
- Okapi: Causally Consistent Geo-Replication (arXiv:1702.04263) — hybrid logical/physical clocks, UST stabilization. https://arxiv.org/html/1702.04263v1
- Tunable Causal Consistency (arXiv:2211.03501) — causal consistency, session guarantees, vector clocks. https://arxiv.org/html/2211.03501v1
- Read freshness / consistency-latency-freshness trade-off (arXiv:1810.01698) — three-way trade-off read consistency / delay / freshness. https://arxiv.org/pdf/1810.01698v1
- Azure Cosmos DB consistency levels — strong, bounded staleness (K versions / T time), session, consistent prefix, eventual. https://learn.microsoft.com/en-us/training/modules/use-consistency-models-azure-cosmos-db-sql-api/2-understand
- Skybridge (USENIX OSDI '25) — bounded staleness for replicated caches. https://usenix.org/system/files/osdi25-lyerly.pdf
- Amazon DynamoDB read consistency — eventually consistent vs strongly consistent reads, ConsistentRead parameter. https://docs.aws.amazon.com/us_en/amazondynamodb/latest/developerguide/HowItWorks.ReadConsistency.md
- DynamoDB global tables — MREC / MRSC, last-writer-wins conflict resolution. https://aws.amazon.com/dynamodb/global-tables
- Partial quorum inconsistency probability (arXiv:2002.06098) — sloppy quorum staleness probability. https://arxiv.org/pdf/2002.06098v4
- Cassandra read repair / anti-entropy (DataStax DSE) — Merkle tree repair, digest comparison, write-back. https://docs.datastax.com/en/dse/6.9/architecture/database-architecture/repair-nodes-read-repair.html
- Riak Active Anti-Entropy — Merkle tree anti-entropy, on-disk hash trees, read repair vs active anti-entropy. https://docs.riak.com/riak/kv/latest/learn/concepts/active-anti-entropy
- Merkle anti-entropy (scalecube-prism; asupersync; MIT 6.5830) — fixed-shape Merkle tree, O(d·k) divergence, bucketized digests. https://github.com/scalecube/scalecube-prism/blob/master/prism-docs/anti-entropy-merkle.md
- Scatter-gather execution (Apache Pinot routing; Apache Druid; ClickHouse parallel replicas; MongoDB distributed queries) — fan-out/fan-in, replica groups, aggregation merge. https://docs.pinot.apache.org/operate-pinot/tuning/routing
- Deep Engineering: Designing Staleness SLOs for Dynamo-style KV stores — N/R/W tuning, staleness. https://deepengineering.net/p/deep-engineering-13-designing-staleness
- Total order broadcast (DDIA ch. 9) — atomic broadcast, linearizable reads, consensus equivalence. https://timilearning.com/posts/ddia/part-two/chapter-9-2

-- -

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 014 (ids-read).
