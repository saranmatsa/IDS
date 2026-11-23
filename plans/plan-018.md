## Phase 018 — Create the ids-sync crate: Replica reconciliation and convergence.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Verbatim prompt (C:/idfs/prompts.txt lines 265-279):
"Phase 018 — Create the ids-sync crate: Replica reconciliation and convergence."
Crate: ids-sync
Purpose: Replica reconciliation and convergence.
Contents: Incremental change streams, checkpoint management, missing update detection, retry procedures, version tracking, Merkle tree comparison, anti-entropy repair, read repair, replica reconciliation, integrity verification.

--| Separate crates instead of more folders in existing ones.

Create plan-018.md in C:/idfs/plans/ for the ids-sync crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.

---

### 1. Executive Summary

Phase 018 creates `ids-sync`, the IDS crate responsible for replica reconciliation and convergence: keeping every replica of a dataset convergent despite independent local writes, missed updates, out-of-order delivery, and long-lived partitions. It is a **separate crate** (CRATES.md entry 18) located at `C:/idfs/ids-sync/`, not a folder inside an existing crate.

The crate layers four mechanisms on top of each other, from low-latency common case to full-convergence backstop:

1. **Incremental change streams** — per-dataset append-only change feed keyed by version; peers exchange only entries newer than their last acknowledged checkpoint (push/pull via `ids-carrier::Endpoint` datagrams).
2. **Checkpoint management** — periodic signed state snapshots with version positions; a replica resumes sync from the latest common checkpoint instead of replaying the whole history.
3. **Missing update detection** — version-vector / dotted-version-vector comparison plus Merkle-tree digest exchange locates exactly which keys/ranges diverge in O(d · k) hash comparisons instead of O(n) full comparison.
4. **Anti-entropy repair + read repair** — scheduled Merkle-tree sweep reconciles cold/rarely-read data (the backstop that guarantees convergence regardless of access pattern); read repair heals divergent keys on the read path.

Convergence model: **strong eventual consistency (SEC)** — replicas that have received the same set of updates reach identical state regardless of delivery order, duplication, or delay (CALM theorem; Shapiro et al. 2011). Every update is inflationary in the join-semilattice sense; merge is commutative, associative, idempotent.

**Carrier-agnostic absolute rule:** no `std::net`, `SocketAddr`, `IpAddr`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh anywhere. `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, opaque bytes capped per carrier) is the sole address and traffic type. All sync traffic rides `ids-carrier::Endpoint` datagrams: `send_datagram(endpoint, &[u8])` / `receive_datagram() -> (Endpoint, Vec<u8>)`.

**Allowed crypto only:** ed25519 (signed checkpoints, signed change entries, authenticated reconciliation messages), xchacha20poly1305 / chacha20poly1305 (optional payload encryption for sync entries in transit), sha256 / blake3 (Merkle tree digests, content addressing, version hashes). All crypto consumed from `ids-crypto`; `ids-sync` declares usage boundaries only, never implements primitives.

`ids-000-aa` is the master crate with all canonical types and sealed trait contracts: `NodeId`, `DatasetId`, `RecordId`, `PartitionId`, `Version`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, sealed `trait Replication`, sealed `trait CRDT`, invariants (`version_vector`, `commit_prefix`, `replica_freshness`, `custody_graph`, `out_eq_in`), versioning (`ProtocolVersion`, `EdgeCaseBits` u128). `ids-sync` imports these verbatim and never redefines them.

**Research only** — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-sync` sits between `ids-replication` (ordered streams, checkpoints, retries — CRATES.md entry 17) and `ids-consistency` (consistency profiles, committed-prefix — CRATES.md entry 19), below the application/query layer and above `ids-carrier`.

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / QUERY LAYER (ids-*)                 │
│  reads, writes, consistency-profile requests, read-repair triggers   │
├─────────────────────────────────────────────────────────────────────┤
│                    Phase 018: ids-sync (THIS CRATE)                 │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────────┐   │
│  │ Change Stream   │ │ Checkpoint      │ │ Missing Update      │   │
│  │ (feed, versions)│ │ Management      │ │ Detection (VV/DVV)  │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────────┘   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────────┐   │
│  │ Merkle Tree     │ │ Anti-Entropy    │ │ Read Repair +       │   │
│  │ Comparison      │ │ Repair          │ │ Reconciliation      │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────────┘   │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Integrity Verification · Retry Procedures · Version Tracking   │  │
│  └───────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│              CARRIER BOUNDARY (ids-carrier::Endpoint only)          │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, &[u8])               │
│  receive_datagram() -> (Endpoint, Vec<u8>)                          │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└─────────────────────────────────────────────────────────────────────┘
```

**Data flow — incremental sync (common case):**
1. Local write lands via `ids-replication` stream; `ids-sync` appends a versioned change entry to the local change stream (append-only log keyed by `Version`/`OrderingPos`).
2. On a sync tick (or on demand), the local replica sends its current checkpoint position + version-vector summary to a peer over `ids-carrier::Endpoint`.
3. The peer compares version vectors (or Merkle roots) to compute the missing set: entries the peer lacks but the sender has, and vice versa.
4. Missing entries are shipped as compact change batches (only the delta, not full state); each entry is signed with ed25519 and integrity-verified with sha256/blake3 before apply.
5. Receiver applies entries idempotently (duplicate suppression by entry ID); advances its checkpoint position; sends an ack back over the carrier endpoint.

**Data flow — anti-entropy repair (backstop):**
1. Periodically (or on integrity-audit trigger), each replica builds a Merkle tree over its dataset partition keyed by `RecordId`; leaves hash `(key, version, value/tombstone)`, interior nodes hash `sha256(left || right)` or `blake3(left || right)`.
2. Replicas exchange root hashes (one hash summarizes the entire range). Equal roots ⇒ identical state, stop immediately.
3. On mismatch, descend only into divergent subtrees (prune matching branches). Locating k divergent keys costs O(d · k) hash exchanges instead of O(n).
4. At divergent leaves, exchange actual key-value entries; resolve by deterministic winner (LWW by version/timestamp, tombstone beats value, CRDT merge for concurrent writes) — symmetric so the outcome does not depend on which replica starts.
5. Re-hash repaired leaves up to the root; roots now equal ⇒ convergence verified.

**Data flow — read repair (opportunistic):**
1. A read contacts R replicas; coordinator compares versions/digests of the returned values.
2. On disagreement, the winner (by consistency profile: LWW, CRDT merge, or quorum overlap) is pushed back to stale replicas before returning to the client.
3. Heals hot keys on the access path; anti-entropy handles cold keys.

**No end-to-end path assumption:** sync messages are datagrams over `ids-carrier::Endpoint`; the carrier decides delivery. Retransmission is expressed as re-sending a datagram to the same endpoint — never a TCP retransmit, never RTT-bound timers. Timeouts are absolute deadlines keyed to entry creation/checkpoint time.

---

### 3. Detailed Research Dimensions

#### 3.1 Incremental change streams & checkpoint management

**Change stream model:** per-dataset append-only log of change entries `{ entry_id, version, op_type, record_id, value/tombstone, prev_version_hash, sig }`. Entry ID derived from BLAKE3(content-addressed) so identical changes have identical IDs and duplicates are trivially deduped. Each entry references its predecessor (hash chain / append-only log), giving a tamper-evident history without a central sequencer.

**Checkpoint management:** periodic signed snapshots `{ dataset_id, checkpoint_version, state_root (Merkle root), version_vector, sig }` persisted via `ids-replication` checkpoints. A sync session begins by exchanging checkpoint positions; replicas resume from the latest *common* checkpoint and only stream entries after it. Checkpoints are signed with ed25519 so an untrusted peer cannot serve a forged snapshot.

Research basis: write-ahead log + periodic checkpoint (append-only log before apply; recovery = load snapshot + replay tail — bounds replay work); CouchDB replication protocol changes feed (sequence ID, continuous replication); subduction batch/incremental sync (fingerprint-based reconciliation + per-peer monotonic counters); hypercore/hyperdb secure append-only logs (public-key identified feed, immutable entries, verify-on-download from untrusted peers).

#### 3.2 Missing update detection (version tracking)

**Version Vector (VV):** map `ReplicaId → counter`; detects causality (VV1 ≤ VV2 iff every entry ≤; concurrent when neither ≤ the other). Source: DOMAINS Layer 12 (`version_vector` invariant in ids-000-aa); Parker et al. 1983.

**Dotted Version Vector (DVV):** compact form — a version vector plus "dots" (replica, counter) for the most recent writes; reduces metadata size when few replicas write each key. Source: Preguiça et al. 2010/2012 "Dotted Version Vectors"; eribeiro VV-vs-DVV comparison.

**Missing set computation:** peer A sends its VV/DVV summary (O(replicas) size, not O(keys)); peer B computes which entries it lacks and which A lacks; only those are transferred. For very large datasets, Merkle-tree digest exchange (3.4) replaces VV when per-key metadata is too large.

Research basis: Dynamo/Dread versioning, Causal Bayes, did:crdt signed deltas (version-vector-based optimized OR-Set tombstone-free variant).

#### 3.3 Merkle tree comparison & anti-entropy repair

**Merkle tree over dataset partition:** leaves = hash buckets of `(key || version || value || tombstone)`; interior = `sha256(left || right)` or `blake3(left || right)`; fixed binary shape (bucketized key space) so two replicas' trees share shape and compare positionally. Leaf covers a fixed key-range (not one key), capping tree size: depth d covers 2^d buckets; O(d · k) diff cost for k divergent buckets.

**Diff algorithm:** send root hash. Equal → converged in O(1). Different → exchange child hashes, recurse only into mismatched subtrees, prune matching ones. At leaf level, exchange actual entries in divergent buckets, resolve deterministically, re-hash up.

**Repair semantics:** symmetric last-writer-wins by version/timestamp (tombstone beats value), or CRDT merge for concurrent writes — outcome independent of which replica initiates. Overstreaming trade-off: coarse buckets stream more keys than needed; fine buckets cost more tree metadata.

Research basis: Dynamo anti-entropy (Merkle trees, 2007), Cassandra `nodetool repair` (full/sub-range/incremental repair, depth-15 compact trees), Riak active anti-entropy, ScyllaDB, Git object model, IPFS Merkle-DAG, Certificate Transparency (RFC 6962 inclusion/consistency proofs), "Anti-Entropy, Digests, and Reconciliation" (digest exchange + Merkle localization), Merkle-CRDTs (arXiv 2004.00107 — Merkle-DAG as logical clock, sync without copying full DAG).

#### 3.4 Read repair & replica reconciliation

**Read repair:** coordinator reads from R replicas; digests compared; mismatches trigger write-back of winning version to stale replicas. Foreground (blocking, stronger) vs background (probabilistic, lower latency). Heals only keys that are read — complementary to anti-entropy backstop.

**Reconciliation / read repair vs anti-entropy division of labor:**
| Mechanism | When it runs | Covers |
|---|---|---|
| Read repair | On read path | Hot keys touched by clients |
| Hinted handoff (via ids-replication retries) | Transient peer unavailability | Writes missed during short outage |
| Anti-entropy Merkle sweep | Scheduled / on-demand | Cold data, long outages, bit rot |

Research basis: Cassandra read repair (blocking/none/probabilistic), Dynamo hinted handoff + Merkle anti-entropy, "Anti-Entropy & Read Repair" synthesis (semicolony.dev), CRDT-based reconciliation (merge eliminates winner-picking for CRDT-typed keys).

#### 3.5 Retry procedures & integrity verification

**Retry:** sync entries are idempotent (entry-ID dedup); failed delivery = re-send datagram to same `Endpoint`. Bounded retry with jittered backoff, retry-storm prevention (ids-fault circuit breakers), dead-letter queue for persistently-failing peers. No RTT-estimate timeouts — absolute deadlines keyed to entry creation time.

**Integrity verification:** every change entry carries ed25519 signature over (author, entry_id, prev_version_hash, payload); receiver verifies before apply (rejects forged/tampered entries). Merkle tree leaves include value hash so a peer serving corrupted data is detected at leaf comparison. Checkpoints signed with ed25519; chunk-level hash proofs (sha256/blake3) let untrusted peers ferry data without being trusted.

**Version tracking:** per-dataset `Version` + `OrderingPos` from ids-000-aa; `commit_prefix` invariant (committed-prefix tracking) ensures reads respect committed position prefix; `replica_freshness` invariant validates replica freshness for reads.

Research basis: RFC 8032 EdDSA deterministic signatures (no bad randomness), batch verification (ed25519-dalek `verify_batch`), hash-history mechanism (OceanStore — dependency graph of version hashes, false-coincidence reduction), "Ensuring Convergence and Invariants Without Coordination" (ECOOP 2025 — No-Op framework, causal stability, strong convergence without coordination).

#### 3.6 Gossip/epidemic dissemination & carrier-agnostic delivery

Sync may run over sparse/multi-hop carrier topology. Gossip fanout (periodic random peer exchange; O(log N) rounds to reach all w.h.p.) disseminates change announcements; actual entry transfer is pull-based on request. SRM-style reliable multicast (eventual delivery without enforced order) as the base; ordering machinery (ids-broadcast) layered on top when needed. All delivery re-expressed as `send_datagram(endpoint, &[u8])` retries — carrier-agnostic.

Research basis: Demers et al. gossip/epidemic protocols, HyParView fanout tuning, SRM reliable multicast, did:crdt causal delivery (weaker than total order, no consensus).

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|---|---|
| Forged / tampered change entries | ed25519 signature over (author, entry_id, prev_hash, payload); verify before any state effect |
| Replay of old signed entries | Per-peer monotonic sequence + nonce replay cache with TTL; tombstones carry deletion authority; causality check rejects stale dependency positions |
| Duplicate datagrams | Entry-ID dedup (idempotent apply); Uniform Integrity at-most-once |
| Lost datagrams / carrier loss | Idempotent re-send to same Endpoint; bounded retry with jittered backoff; retry-storm prevention |
| Byzantine sender conflicting values | Deterministic merge (LWW/CRDT) is order-independent; signed entries prevent forgery; equivocation detectable via signed prev_hash chain |
| Metadata bomb (huge change batch) | Per-peer batch size caps, max entry count, depth/size budgets before verification |
| State explosion / merge bomb | Expansion-ratio ceiling, delta-size caps, per-peer merge budget, cascade-breaker depth limit |
| Read repair clobbers fresher state | Deterministic symmetric winner (LWW by version, tombstone beats value, CRDT merge); never clobber newer |
| Checkpoint forgery | ed25519-signed checkpoints; verify before resume-from-checkpoint |
| Merkle tree stale after bulk inserts | Rebuild/incremental-update tree on every write; O(log N) rehash per leaf change |
| Clock-skew dependence | Never use wall-clock for validity; versions/positions are logical (OrderingPos, IdsTime) |
| Disallowed crypto | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; ids-crypto enforces |
| Carrier-agnostic rule violation | Endpoint sole address type; dependency audit bans networking crates; grep test for prohibited symbols |

Critical failure modes: forged updates (signatures), key compromise (bounded revocation + content trust — accepted updates stay valid), metadata bombs (budgets), replay (cache + nonces), amplification (merge guards), clock skew (never trust wall time for validity), state-drift silence (periodic state-root exchange + divergence audit triggers gap reconciliation from signed checkpoints).

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 018 |
|---|---|
| `ids-000-aa` (master) | Canonical types (`NodeId`, `DatasetId`, `RecordId`, `PartitionId`, `Version`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`), sealed `trait Replication`, sealed `trait CRDT`, invariants (`version_vector`, `commit_prefix`, `replica_freshness`, `custody_graph`, `out_eq_in`), versioning, unified `Error` |
| `ids-carrier` (entry 2) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); `send_datagram`/`receive_datagram`; sole traffic channel |
| `ids-crypto` (entry 3) | ed25519 sign/verify, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 AEAD; consumed only |
| `ids-replication` (entry 17) | Change streams, checkpoints, retries, ack tracking, idempotent application — ids-sync builds on this |
| `ids-consistency` (entry 19) | Consistency profiles, committed-prefix tracking, read-repair winner selection input |
| `ids-ordering-domain` | Dataset-scoped ordering domains, per-domain committed position |
| `ids-fault` | Circuit breakers / backpressure for sync delivery, retry-storm prevention |
| `ids-write` | WAL that consumes reconciled entries |
| `ids-identity` | Signer identities for signed entries/checkpoints |

**Integration points:**
- `ids-sync` implements `ids_000_aa::contracts::Replication` (streams, checkpoints, retries) and `trait CRDT` (delta/state merge) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; never touches sockets.
- Signed entries/checkpoints via ed25519 via `ids-crypto`; integrity via sha256/blake3.
- Depends on `ids-000-aa` for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (CRATES.md rule) — `ids-sync` depends on both.
- Cargo workspace: `ids-sync = { path = "../ids-sync" }` in dependents.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-sync` (to be created at `C:/idfs/ids-sync/`)

```
ids-sync/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-replication,
│                                       # ids-consistency, ids-ordering-domain, ids-fault;
│                                       # thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ChangeEntry, Checkpoint, SyncPosition, VersionSummary,
│   │                                   # MerkleLeaf, RepairPlan, SyncSession, EntryId
│   ├── errors.rs                       # SyncError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Replication + CRDT
│   │
│   ├── change_stream/                  # Incremental change feed (~9 files)
│   │   ├── mod.rs
│   │   ├── feed.rs                     # Append-only change feed per dataset
│   │   ├── entry.rs                    # ChangeEntry: id, version, op, record, sig, prev_hash
│   │   ├── entry_id.rs                 # BLAKE3 content-addressed entry id
│   │   ├── append_log.rs               # Hash-chained append-only log
│   │   ├── cursor.rs                   # Per-peer sync cursor (last acked version)
│   │   ├── batch.rs                    # Compact change batch for transfer
│   │   ├── dedup.rs                    # Entry-ID dedup (idempotent apply)
│   │   ├── stream_adapter.rs           # ids-replication stream binding
│   │   └── proptests.rs                # Property tests: dedup, append invariants
│   │
│   ├── checkpoint/                     # Checkpoint management (~9 files)
│   │   ├── mod.rs
│   │   ├── checkpoint.rs               # Signed snapshot: state_root, version_vector, sig
│   │   ├── checkpoint_manager.rs       # Periodic snapshot creation/rotation
│   │   ├── state_root.rs               # Merkle root over current dataset state
│   │   ├── common_checkpoint.rs        # Latest common checkpoint between two peers
│   │   ├── resume.rs                   # Resume sync from latest common checkpoint
│   │   ├── checkpoint_signer.rs        # ed25519 sign/verify of checkpoints
│   │   ├── persistence.rs              # Persist checkpoints via ids-storage/sled
│   │   └── proptests.rs
│   │
│   ├── version_tracking/               # Missing update detection (~9 files)
│   │   ├── mod.rs
│   │   ├── version_vector.rs           # VV: ReplicaId -> counter (causality)
│   │   ├── dotted_vv.rs                # DVV: compact version tracking
│   │   ├── version_summary.rs          # O(replicas) summary exchanged at sync start
│   │   ├── missing_set.rs              # Compute entries each side lacks
│   │   ├── causality_check.rs          # Reject stale dependency positions
│   │   ├── freshness.rs                # replica_freshness invariant validation
│   │   ├── commit_prefix.rs            # committed-prefix tracking input
│   │   ├── version_types.rs            # Version, OrderingPos re-export wrappers
│   │   └── proptests.rs
│   │
│   ├── merkle/                         # Merkle tree comparison (~9 files)
│   │   ├── mod.rs
│   │   ├── tree.rs                     # Fixed-shape binary Merkle tree over key buckets
│   │   ├── leaf.rs                     # Leaf = hash(key, version, value/tombstone)
│   │   ├── node.rs                     # Interior = sha256/blake3(left || right)
│   │   ├── root.rs                     # Root hash = whole-range summary
│   │   ├── diff.rs                     # Top-down tree diff: O(d · k) divergence localization
│   │   ├── proof.rs                    # Inclusion/exclusion proof (sha256/blake3)
│   │   ├── rebuild.rs                  # Incremental O(log N) rehash on leaf change
│   │   └── proptests.rs
│   │
│   ├── anti_entropy/                   # Scheduled repair backstop (~9 files)
│   │   ├── mod.rs
│   │   ├── scheduler.rs                # Periodic anti-entropy sweep trigger
│   │   ├── repair_session.rs           # Peer reconciliation session driver
│   │   ├── divergence_locator.rs       # Merkle diff → divergent bucket list
│   │   ├── entry_exchange.rs           # Exchange actual entries in divergent buckets
│   │   ├── resolver.rs                 # Deterministic winner: LWW/tombstone/CRDT merge
│   │   ├── rehash.rs                   # Re-hash repaired path to root
│   │   ├── throttle.rs                 # Throttle repair so it doesn't starve live traffic
│   │   └── proptests.rs
│   │
│   ├── read_repair/                    # Opportunistic on-read healing (~7 files)
│   │   ├── mod.rs
│   │   ├── digest_compare.rs           # Compare value digests across R replicas
│   │   ├── winner_select.rs            # Consistency-profile-aware winner (LWW/CRDT/quorum)
│   │   ├── write_back.rs               # Push winner to stale replicas
│   │   ├── foreground.rs               # Blocking read repair
│   │   ├── background.rs               # Probabilistic background read repair
│   │   └── proptests.rs
│   │
│   ├── reconciliation/                 # Replica reconciliation orchestration (~7 files)
│   │   ├── mod.rs
│   │   ├── reconciler.rs               # Orchestrates change stream + Merkle + read repair
│   │   ├── session.rs                  # Sync session state machine (init/summary/diff/transfer/apply/ack)
│   │   ├── integrity.rs                # Integrity verification gate before apply
│   │   ├── convergence_check.rs        # Final root-hash equality verification
│   │   ├── divergence_audit.rs         # State-root exchange + drift detection
│   │   ├── gap_reconciliation.rs       # Gap reconciliation from signed checkpoints
│   │   └── proptests.rs
│   │
│   ├── retry/                          # Retry procedures (~7 files)
│   │   ├── mod.rs
│   │   ├── retry_scheduler.rs          # Bounded retry with jittered backoff
│   │   ├── backoff.rs                  # Jittered exponential backoff
│   │   ├── dead_letter.rs              # Dead-letter queue for persistently failing peers
│   │   ├── retry_storm.rs              # Retry-storm prevention (ids-fault circuit breakers)
│   │   ├── absolute_deadline.rs        # Deadlines keyed to entry creation time (no RTT)
│   │   └── proptests.rs
│   │
│   ├── integrity/                      # Integrity verification (~7 files)
│   │   ├── mod.rs
│   │   ├── entry_verifier.rs           # ed25519 verify before apply
│   │   ├── checkpoint_verifier.rs      # Signed checkpoint verification
│   │   ├── chunk_proof.rs              # Per-chunk hash proof (sha256/blake3)
│   │   ├── value_hash.rs               # Leaf value hash for corruption detection
│   │   ├── batch_verifier.rs           # ed25519 batch verification (verify_batch)
│   │   ├── verification_cache.rs       # Cache of verified entries/proofs
│   │   └── proptests.rs
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper
│   ├── delivery_buffer.rs              # Out-of-order entry holdback + gap detection
│   ├── state/                          # State machines (~3 files)
│   │   ├── mod.rs
│   │   ├── sync_session_state_machine.rs
│   │   └── repair_state_machine.rs
│   ├── codec/                          # Wire encoding (~4 files)
│   │   ├── mod.rs
│   │   ├── sync_frame.rs               # Version-tagged frame header
│   │   ├── entry_codec.rs              # serde/bincode encode/decode of change entries
│   │   └── digest.rs                   # sha256/blake3 message digests
│   ├── config/                         # Configuration (~5 files)
│   │   ├── mod.rs
│   │   ├── sync_config.rs              # Sync tick, batch size, retry bounds
│   │   ├── merkle_config.rs            # Tree depth, bucket size
│   │   ├── anti_entropy_config.rs      # Sweep interval, throttle
│   │   └── retry_config.rs             # Backoff bounds, dead-letter limits
│   ├── metrics/                        # Metrics & tracing (~5 files)
│   │   ├── mod.rs
│   │   ├── sync_metrics.rs             # Sync session metrics
│   │   ├── repair_metrics.rs           # Anti-entropy repair metrics
│   │   ├── divergence_metrics.rs       # Divergence detection metrics
│   │   └── integrity_metrics.rs        # Verification metrics
│   ├── fuzz/                           # Fuzzing harnesses (~4 files)
│   │   ├── mod.rs
│   │   ├── fuzz_entry.rs
│   │   ├── fuzz_merkle.rs
│   │   ├── fuzz_repair.rs
│   │   └── fuzz_sync.rs
│   ├── tests/                          # Integration & adversarial tests (~12 files)
│   │   ├── mod.rs
│   │   ├── change_stream_tests.rs
│   │   ├── checkpoint_tests.rs
│   │   ├── version_tracking_tests.rs
│   │   ├── merkle_tests.rs
│   │   ├── anti_entropy_tests.rs
│   │   ├── read_repair_tests.rs
│   │   ├── reconciliation_tests.rs
│   │   ├── retry_tests.rs
│   │   ├── integrity_tests.rs
│   │   ├── adversarial_tests.rs        # Replay, tamper, flood, forged entries
│   │   ├── determinism_tests.rs        # Same seed/updates → identical converged state
│   │   └── carrier_tests.rs            # Endpoint-only delivery (no sockets)
│   ├── benches/                        # Benchmarks (~5 files)
│   │   ├── mod.rs
│   │   ├── merkle_bench.rs
│   │   ├── sync_bench.rs
│   │   ├── repair_bench.rs
│   │   └── integrity_bench.rs
│   ├── spec/                           # Normative specifications (~6 files)
│   │   ├── change_stream_spec.md
│   │   ├── checkpoint_spec.md
│   │   ├── merkle_diff_spec.md
│   │   ├── anti_entropy_spec.md
│   │   ├── read_repair_spec.md
│   │   └── threat_model.md
│   └── docs/                           # Documentation (~5 files)
│       ├── architecture.md
│       ├── integration_guide.md
│       ├── security_considerations.md
│       ├── design_rationale.md
│       └── api_docs.md
```

---

### 7. Key Algorithms & Data Structures

**ChangeEntry** = `{ entry_id: BLAKE3(content), version: Version, op: OpType, record_id: RecordId, value: Option<Vec<u8>>, tombstone: bool, prev_version_hash: [u8;32], author: NodeId, sig: Ed25519Signature }`

**Checkpoint** = `{ dataset_id: DatasetId, checkpoint_version: Version, state_root: [u8;32], version_vector: VersionVector, sig: Ed25519Signature }`

**Merkle diff** (top-down):
```
merkle_diff(node_a, node_b):
  if node_a.hash == node_b.hash: return ∅        # identical subtree, prune
  if node_a.is_leaf: return {node_a.range}        # divergent bucket
  return merkle_diff(a.left, b.left) ∪ merkle_diff(a.right, b.right)
```
Cost: O(d · k) hash exchanges for k divergent buckets out of 2^d leaves; O(1) when roots match.

**Anti-entropy repair** (symmetric):
```
repair(bucket_a, bucket_b):
  winner = LWW_by_version(bucket_a, bucket_b)   # tombstone beats value; CRDT merge for concurrent
  apply winner to losing side; re-hash leaf → root
```
Outcome independent of which replica initiates.

**Missing set via DVV:**
```
missing(A_dvv, B_dvv):
  for each (replica, counter) in A_dvv not dominated by B_dvv:
     emit entries with version > B's counter for replica
  symmetric for B_dvv vs A_dvv
```

**Read repair:**
```
read_repair(key, values[(replica, version, digest)]):
  if all digests equal: return
  winner = select_winner(values, consistency_profile)  # LWW / CRDT merge / quorum
  write_back(winner, stale_replicas)
  return winner to client
```

**Retry with absolute deadline:**
```
retry_send(endpoint, datagram, deadline):
  while now() < deadline:
     if send_datagram(endpoint, datagram).ok(): return
     sleep(jittered_backoff(attempt))
  dead_letter_queue.push(endpoint, datagram)
```

---

### 8. Dependencies

| Dependency | Purpose | Allowed? |
|---|---|---|
| `ids-000-aa` | Canonical types, sealed traits, invariants, versioning, errors | Yes (master) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` | Yes (entry 2) |
| `ids-crypto` | ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 | Yes (entry 3, only allowed crypto) |
| `ids-replication` | Change streams, checkpoints, retries, ack tracking | Yes (entry 17) |
| `ids-consistency` | Consistency profiles, committed-prefix | Yes (entry 19) |
| `ids-ordering-domain` | Dataset-scoped ordering domains | Yes |
| `ids-fault` | Circuit breakers, backpressure, retry-storm prevention | Yes |
| `ids-write` | WAL for reconciled entries | Yes |
| `ids-identity` | Signer identities | Yes |
| `thiserror`, `serde` | Error enums, serialization | Yes |

**Prohibited (zero tolerance):** `std::net`, `tokio::net`, `SocketAddr`, `IpAddr`, libp2p, QUIC, WebRTC, HTTP, DNS, TCP, UDP, IP, Wi-Fi, Bluetooth, cellular, VPN, conventional mesh, any crate that pulls in a networking stack. `ids-crypto` does NOT depend on `ids-000-aa` (CRATES.md rule) — `ids-sync` depends on both.

---

### 9. Measurable Acceptance Criteria

- [ ] `ids-sync` compiles as a standalone crate with no `std::net`, `tokio::net`, libp2p, or any networking dependency
- [ ] All traffic via `ids_carrier::Endpoint` (`CarrierType` + `Vec<u8>`); source grep confirms zero prohibited symbols
- [ ] Crypto surface limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (via `ids-crypto` only)
- [ ] Every canonical type imported from `ids_000_aa::X`, never redefined
- [ ] Implements `ids_000_aa::contracts::Replication` and `trait CRDT` (sealed traits from master)
- [ ] Change entries content-addressed by BLAKE3; duplicates deduped; idempotent apply
- [ ] Merkle diff locates k divergent buckets in O(d · k) hash exchanges (verified in tests)
- [ ] Equal Merkle roots ⇒ converged in O(1) (one hash exchange)
- [ ] Anti-entropy repair produces symmetric outcome independent of initiating replica
- [ ] Read repair never clobbers a fresher version (LWW/tombstone/CRDT winner rules)
- [ ] Checkpoints ed25519-signed; verified before resume-from-checkpoint
- [ ] Retry uses absolute deadlines keyed to entry creation time (no RTT-bound timers)
- [ ] Per-peer replay cache (nonce + monotonic sequence) with TTL-bounded eviction
- [ ] Batch size caps and expansion-ratio ceiling enforced (merge bomb protection)
- [ ] Divergence audit (state-root exchange) detects drift; triggers gap reconciliation from signed checkpoints
- [ ] File structure ≥90 files in subfolders under `ids-sync/src/` (per plan table)
- [ ] Research only: no `.rs` implementation files written, no repository modifications

---

### 10. Remaining Uncertainties

- Whether `ids-sync` should implement a sealed `trait Sync` in addition to `Replication`/`CRDT`, or whether `Replication` alone covers streams+checkpoints+retries (decided at implementation, contract surface in `ids-000-aa`).
- Exact Merkle tree depth / bucket size defaults (workload-dependent; configurable via `MerkleConfig`, depth 15–16 typical per Cassandra).
- Whether DVV or plain VV is the default summary; DVV needs contiguous-prefix property validation per replica.
- Whether read repair is foreground (blocking) or background (probabilistic) by default — consistency-profile-dependent.
- Checkpoint periodicity and retention bounds (tombstone GC only at causal stability).
- Interaction with `ids-consistency` profiles for read-repair winner selection under bounded staleness / read-your-writes.
- Whether anti-entropy throttle is credit-based or time-window-based (ids-fault integration).

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate crate `ids-sync`, not folders in existing crates | CRATES.md entry 18; "try to use separate crates instead of more folders" |
| Four-layer stack: change stream → checkpoint → VV/DVV missing detection → Merkle anti-entropy + read repair | Low-latency common case (incremental) + full-convergence backstop (anti-entropy); read repair heals hot keys opportunistically |
| SEC via join-semilattice merge (commutative, associative, idempotent) | CALM theorem; convergence without coordination; matches carrier-agnostic unknown-membership model |
| Content-addressed entry IDs (BLAKE3) + hash-chained append log | Trivial dedup, tamper-evident history, no central sequencer |
| Merkle trees bucketized by fixed key-range, sha256/blake3 interior | O(d · k) diff vs O(n) full compare; fixed shape lets two replicas compare positionally |
| Symmetric repair (LWW/tombstone/CRDT, outcome independent of initiator) | Never clobber fresher state; no two-way handshake needed |
| ed25519-signed entries + checkpoints; verify before apply | Untrusted peers/carriers can ferry data without being trusted; batch verification via `verify_batch` |
| Absolute deadlines keyed to entry creation time (no RTT) | Correct over arbitrary carrier delay; no TCP/RTT assumptions |
| All crypto via `ids-crypto` only; allowed set enforced | Carrier-agnostic + crypto policy compliance |

---

### 12. Sources

1. Shapiro et al. 2011 — CRDTs, strong eventual consistency, join-semilattice merge (CAI).
2. Almeida, Shoker, Baquero 2018 — δ-CRDTs (delta-state incremental exchange).
3. Hellerstein & Alvaro — CALM theorem (confluence without coordination for monotone/join-semilattice computations).
4. Amazon Dynamo (2007) — version vectors, Merkle-tree anti-entropy, hinted handoff, read repair, sloppy quorum.
5. Apache Cassandra — Merkle-tree anti-entropy repair (full/sub-range/incremental), depth-15 compact trees, read repair (blocking/none/probabilistic).
6. Riak — active anti-entropy, DT/ORSet, DT registers.
7. CouchDB Replication Protocol — changes feed, sequence ID, continuous replication.
8. inkandswitch subduction — batch + incremental sync, fingerprint-based reconciliation, per-peer monotonic counters, LooseCommit push.
9. hypercore/hyperdb — secure append-only logs, public-key identified immutable feeds, verify-on-download.
10. IPFS Merkle-DAG — content-addressed self-verified nodes, sync without copying full DAG.
11. Merkle-CRDTs (arXiv 2004.00107) — Merkle-DAG as logical clock, anti-entropy for Merkle-CRDTs.
12. Certificate Transparency RFC 6962 — Merkle inclusion/consistency proofs.
13. OceanStore hash histories (ICDCS 2003) — version-hash dependency graph, false-coincidence reduction.
14. Preguiça et al. 2010/2012 — Dotted Version Vectors (DVV) for compact causality tracking.
15. Parker et al. 1983 — Version Vectors.
16. "Anti-Entropy & Read Repair" synthesis — digest exchange + Merkle localization, three-mechanism division of labor.
17. RFC 8032 — Ed25519/EdDSA deterministic signatures.
18. RFC 8439 — ChaCha20-Poly1305 and XChaCha20-Poly1305 AEAD.
19. ed25519-dalek — `verify_batch` batch verification, `VerifyingKey::verify`.
20. RustCrypto `chacha20poly1305` — ChaCha20Poly1305 / XChaCha20Poly1305 AEAD.
21. BLAKE3 — tree-based hash, incremental Hasher, extended output.
22. RustCrypto `sha2` — SHA-256 Digest implementation.
23. did:crdt (arXiv 2606.16223) — signed CRDTs, causal delivery weaker than total order, no consensus.
24. "Ensuring Convergence and Invariants Without Coordination" (ECOOP 2025) — No-Op framework, causal stability, strong convergence.
25. Demers et al. — gossip/epidemic protocols; HyParView fanout tuning.
26. SRM reliable multicast — eventual delivery without enforced order.
27. Hadzilacos & Toueg — atomic broadcast properties (for reference, not implemented here).
28. Chandra-Toueg failure detectors — for retry/liveness integration with ids-fault.
29. "Anti-Entropy, Digests, and Reconciliation" — digest exchange localizes divergence.
30. scalecube-prism anti-entropy — fixed-shape Merkle tree, per-bucket digest comparison, re-advertise only owned records.

**End of plan-018.md — research blueprint only, no code, no source modifications.**