## Phase 063 — Create the ids-sync-checkpoint crate: Verified sync points for resumable replication. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 940-954)

```
63. Research only do not read plan.md **[Phase 063 — Create the ids-sync-checkpoint crate: Verified sync points for resumable replication.]**

Crate: ids-sync-checkpoint
Purpose: Verified sync points for resumable replication.
Contents: Checkpoint creation, checkpoint verification, resume from checkpoint, reconciliation reference, checkpoint versioning, checkpoint garbage collection.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 063 — Create the ids-sync-checkpoint crate: Verified sync points for resumable replication.]**

Create plan-063.md in C:/idfs/plans/ for the ids-sync-checkpoint crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 063 creates `ids-sync-checkpoint`, the IDS crate that owns verified sync points for resumable replication. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by checkpoint concern. `ids-sync-checkpoint` provides checkpoint creation, checkpoint verification, resume-from-checkpoint, reconciliation reference, checkpoint versioning, and checkpoint garbage collection — all carrier-agnostic.

**Core design:** a checkpoint is a verified, self-describing snapshot of replication state at a durable position. Each checkpoint carries: a checkpoint ID (BLAKE3-256 content hash), the replicated state root (BLAKE3 Merkle root), the last-applied operation position (`OpRef`/`OrderingPos`), a parent checkpoint link (for incremental/differential chains), an ed25519 signature over the checkpoint header, a sha256/blake3 hash chain entry, and an optional xchacha20poly1305/chacha20poly1305-encrypted payload section. Checkpoints are never sent by this crate; it produces and verifies checkpoint envelopes that callers ship as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

**Scope boundary:** `ids-sync-checkpoint` manages sync points; it does not open sockets, drive transports, or implement replication itself. It returns `CheckpointResult`, `ResumePosition`, and `VerificationResult` values that the replication layer consumes. Thresholds (checkpoint interval, retention count, GC age) are configurable budgets, not hard-coded limits.

**Research basis:** 30 parallel searches returned Chandy-Lamport distributed snapshots (consistent cuts, marker-based channel state, 1985 ACM TOCS); coordinated vs uncoordinated checkpointing and two-phase checkpoint protocol; PostgreSQL WAL checkpoints (checkpoint records, crash recovery starting from checkpoint, replication slots); Raft snapshotting (InstallSnapshot RPC, last-included index/term anchor, log compaction, snapshot transfer as separate operation); Apache Flink checkpointing (consistent snapshot of operator state + stream position, exactly-once, checkpoint interval trades overhead vs recovery time, asynchronous state writeback); Apache Spark Streaming checkpointing (metadata + data checkpointing to fault-tolerant storage); Apache Kafka consumer offset checkpointing (periodic offset commit, rewind to old offset); RocksDB incremental backup (share table files, verify backup); event sourcing snapshots (rehydrate aggregate from snapshot + post-snapshot events, snapshot frequency vs replay cost tradeoff); read-write quorum systems (R+W>N intersection, every read quorum intersects every write quorum); anti-entropy Merkle tree repair (Cassandra Merkle trees, digest exchange, reconcile divergent replicas); dotted version vectors (causal precedence, compact causality tracking); vector clocks (Lamport, Mattern/Fidge); LSM compaction and tombstone GC; state machine replication checkpoint-based recovery (PBFT signed checkpoints, 2f+1 quorum, state transfer from checkpoint + log suffix); checkpoint interval optimization (Young/Daly, utilization model, failure rate vs checkpoint cost); incremental/differential checkpointing (copy only changed blocks since base); two-phase commit checkpointing for distributed transactions; Byzantine fault-tolerant checkpoint verification (signed checkpoints, stable checkpoint from quorum digests); CRDT checkpoint anti-entropy convergence (state-based vs op-based, strong convergence, tombstone GC at causal stability); BLAKE3 (Merkle-tree parallel hashing, KDF, XOF, keyed hash); SHA-256 (FIPS 180-4); ed25519-dalek (RFC 8032 EdDSA sign/verify); xchacha20poly1305 (libsodium 192-bit nonce, random nonces safe); chacha20poly1305 (RFC 7539 AEAD, 96-bit nonce).

---

### 2. Architecture Overview

`ids-sync-checkpoint` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a checkpoint management layer: given a replication position and state digest, it creates a verified checkpoint envelope; given a stored checkpoint, it verifies integrity and authenticity; given a resume request, it computes the resume position and the post-checkpoint operation range. It never sends bytes itself; it returns checkpoint envelopes and resume decisions that callers frame into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    REPLICATION / SYNC LAYER (checkpoint users)               │
│  ids-replication, ids-sync, ids-recovery — create/consume sync checkpoints   │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 063: ids-sync-checkpoint (THIS CRATE — checkpoint mgmt) │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────┐ │ │
│  │ checkpoint       │ │ checkpoint       │ │ resume           │ │ GC     │ │ │
│  │ creation         │ │ verification     │ │ from checkpoint  │ │ policy │ │ │
│  │ (envelope build) │ │ (sig+hash verify)│ │ (position+range) │ │ (retain│ │ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ │ /age)  │ │ │
│  ┌──────────────────┐ ┌──────────────────┐ └──────────────────┘ └────────┘ │ │
│  │ versioning       │ │ reconciliation   │ │                    │         │ │ │
│  │ (chain+format)   │ │ reference        │ │                    │         │ │ │
│  └──────────────────┘ └──────────────────┘ │                    │         │ │
│  ┌──────────────────┐ ┌──────────────────┐ │                    │         │ │
│  │ crypto surface   │ │ envelope store   │ │                    │         │ │ │
│  │ (ed25519/sha256/ │ │ (index+metadata) │ │                    │         │ │ │
│  │  blake3/xchacha/ │ └──────────────────┘ │                    │         │ │ │
│  │  chacha)         │                      │                    │         │ │
│  └──────────────────┘                      │                    │         │ │
└───────────────────────────────────────────┬─────────────────────┴──────────┘
                                            │ opaque Vec<u8> datagrams
                                            ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload)      │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of checkpoint creation:** replication layer submits `CreateCheckpoint{state_root, last_position, causal_context}` → `ids-sync-checkpoint` builds `CheckpointEnvelope{header, signature, hash_chain_link, optional_encrypted_payload}` → returns envelope → caller ships as opaque datagram(s) to `ids-carrier::Endpoint` targets and collects acks → checkpoint marked stable when quorum of replicas confirm receipt.

**Data flow of resume from checkpoint:** replication layer submits `ResumeFrom{checkpoint_id}` → `ids-sync-checkpoint` verifies signature + hash chain + state root → returns `ResumePosition{checkpoint_id, last_position, post_checkpoint_op_range, causal_past}` → caller replays operations after the checkpoint position via `ids-carrier::Endpoint`.

**Checkpoint semantics (research-verified):**
- **Consistent cut / global snapshot:** Chandy-Lamport marker-based snapshot guarantees no effect is in the past while its cause is in the future; every message received in the snapshot's past was sent in the past (consistent cut).
- **Checkpoint anchor:** last-included index/term pair anchors the checkpoint into the log's coordinate system (Raft); snapshot + log suffix suffices for catch-up.
- **Quorum-certified checkpoint:** PBFT-style — signed checkpoints collected from 2f+1 replicas form a stable checkpoint; only then prune below the low watermark.
- **Exactly-once resume:** restore state from checkpoint, replay operations strictly after checkpoint position; duplicates suppressed via idempotent application.
- **Incremental/differential checkpoints:** checkpoint chain where each checkpoint references its parent; only changed state since parent is stored (differential backup pattern).
- **Garbage collection:** retain-by-count and retain-by-age policies; mark-and-sweep reachability from live roots (active checkpoints, pinned resume positions); GC cannot remove a checkpoint still referenced by an in-flight resume or the latest stable checkpoint.
- **Anti-entropy reconciliation:** Merkle tree digest exchange between replicas identifies divergent ranges; checkpoint versioning detects stale checkpoints; reconciliation reference guides repair.

**Crypto used:** ed25519 for signing checkpoint headers and audit checkpoints; sha256/blake3 for checkpoint IDs, state roots, hash-chain links, Merkle digests; xchacha20poly1305/chacha20poly1305 for encrypting checkpoint payload at rest when confidentiality required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Chandy-Lamport distributed snapshots (consistent cuts)

**Source:** Chandy & Lamport, "Distributed Snapshots: Determining Global States of Distributed Systems," ACM Transactions on Computer Systems, Vol 3, No 1 (1985); Princeton COS418 lecture notes on distributed snapshots; UMass 590CC lecture notes on global state recording.

The Chandy-Lamport algorithm captures a consistent global state of a distributed system using marker messages on FIFO channels. A process records its local state, then sends a marker on each outgoing channel before sending further messages. On receiving a marker, if the process hasn't snapshotted, it records its state and sends markers; otherwise it records channel state. The result is a consistent cut: for any event recorded, all events that happened-before it are also recorded. `ids-sync-checkpoint` adopts the consistent-cut principle: a sync checkpoint must represent a causally closed state — no operation whose effects are partially included. Marker-equivalent logic is expressed as barrier positions in the replication log (last-applied `OpRef`/`OrderingPos`).

#### 3.2 Coordinated vs uncoordinated checkpointing

**Source:** "Checkpointing Techniques in Distributed Systems: A Synopsis of Diverse Strategies Over the Last Decades" (SBC WTF journal); Springer "Implementing Rollback-Recovery Coordinated Checkpoints" (ISSADS 2005).

Coordinated checkpointing: all processes agree on a safe execution point via a two-phase checkpoint protocol (prepare phase: processes block and notify readiness; commit phase: initiator signals all to checkpoint). Ensures a consistent global checkpoint (recovery line) but causes latency spikes. Uncoordinated checkpointing: each process checkpoints independently; may produce inconsistent global states requiring rollback-dependency analysis. `ids-sync-checkpoint` supports both: coordinated checkpoints for strong consistency (quorum-signed, stable checkpoints) and uncoordinated local checkpoints for opportunistic local save points, with consistency validation at verification time.

#### 3.3 WAL-based checkpointing and crash recovery

**Source:** PostgreSQL 19 documentation: Write-Ahead Logging (WAL), WAL configuration, continuous archiving and point-in-time recovery.

WAL's central concept: data file changes are written only after WAL records describing them are flushed to permanent storage. Crash recovery always starts from the last checkpoint record; only WAL records after the checkpoint need replaying. A checkpoint record stores metadata about the current database state. `ids-sync-checkpoint` adopts the same structure: a checkpoint is a durable position marker; recovery replays operations strictly after the checkpoint position. Checkpoint interval trades off WAL/log growth against recovery replay time.

#### 3.4 Raft snapshotting and InstallSnapshot

**Source:** Ongaro & Ousterhout, "In Search of an Understandable Consensus Algorithm" (Raft paper); hashicorp/raft snapshot.go; MerceMay "Snapshot Install and the Cost of Compacting a Consensus Log"; openraft docs on snapshot replication.

Raft snapshots compact the log: the state machine writes current state to a snapshot, then committed log entries up to the snapshot boundary are discarded. A snapshot carries last-included index and term (the anchor). InstallSnapshot is a separate RPC, not a large AppendEntries; followers install chunks in order, then reset their state machine from the snapshot. A node behind the snapshot edge cannot catch up via log entries alone — it needs a snapshot transfer. `ids-sync-checkpoint` models sync points as Raft-style anchors: last-applied position + term/epoch, with snapshot transfer via opaque carrier datagrams.

#### 3.5 Apache Flink checkpointing (exactly-once state snapshots)

**Source:** Apache Flink docs: Stateful Stream Processing, Tuning Checkpoints and Large State; "An Overview of End-to-End Exactly-Once Processing in Apache Flink with Apache Kafka."

Flink checkpoints mark a specific point in each input stream plus operator state. A streaming dataflow resumes from a checkpoint with exactly-once semantics by restoring operator state and replaying records from the checkpoint position. Checkpoints are written asynchronously to persistent storage; the checkpoint interval trades fault-tolerance overhead against recovery time. Incremental checkpointing: operators start from the latest full snapshot and apply incremental updates. `ids-sync-checkpoint` adopts exactly-once resume semantics and the interval-overhead/recovery-time tradeoff model.

#### 3.6 Kafka consumer offset checkpointing

**Source:** Apache Kafka design document.

Kafka consumer position is a single integer offset per partition; state can be periodically checkpointed. A consumer can rewind to an old offset and re-consume — essential for bug recovery. Offset checkpoints are small (one number per partition) and cheap. `ids-sync-checkpoint` uses the offset-checkpoint pattern: a resume position is a compact coordinate (last applied `OpRef`/`OrderingPos` per replication stream) enabling cheap rewind and replay.

#### 3.7 RocksDB incremental backup / verify

**Source:** facebook/rocksdb wiki "How to backup RocksDB."

RocksDB backups are normally incremental (share table files): `CreateNewBackup` copies only new data; `VerifyBackup` validates backup integrity. `ids-sync-checkpoint` adopts incremental checkpoint creation (only state changed since parent checkpoint) and built-in verify (signature + hash chain + state root re-check).

#### 3.8 Event sourcing snapshots

**Source:** CodeOpinion "Snapshots in Event Sourcing for Rehydrating Aggregates"; EventSourcingDB "Snapshots and Performance"; Azure Architecture Center "Event Sourcing Pattern"; Axon Framework "Snapshots."

An entity's state is reconstructed by replaying its event stream; snapshots capture state at a stream position. Rehydration loads the latest snapshot, then replays only post-snapshot events. Snapshot frequency balances storage cost vs replay time. Snapshots are an optimization, not a replacement — full replay remains the fallback if a snapshot is unavailable or unusable. `ids-sync-checkpoint` treats checkpoints the same: resume from checkpoint + replay post-checkpoint ops; full replication from genesis is the fallback path.

#### 3.9 Read-write quorum systems (R+W>N)

**Source:** arXiv 2104.04102 "Read-Write Quorum Systems Made Practical"; Atlas of Engineering "Quorum, Read/Write Intersection and Split-Brain Prevention"; UMass CS677 lecture notes on quorum-based protocols; AWS Aurora quorum blog.

A read-write quorum system (R, W) over replica set X requires every read quorum to intersect every write quorum (r ∩ w ≠ ∅). R + W > N guarantees read/write intersection — a read always contacts at least one replica holding the latest write. Write/write intersection requires W > N/2. Availability: writes tolerate N−W failures, reads tolerate N−R. `ids-sync-checkpoint` uses quorum intersection for checkpoint stability: a checkpoint is stable when a quorum of replicas have signed it; resume requires quorum agreement on the checkpoint position.

#### 3.10 Anti-entropy Merkle tree repair

**Source:** Cassandra docs "Anti-entropy repair" (Merkle trees, depth-15 compact trees); Zenoh blog "Keeping storages aligned in Zenoh" (era-based hash trees, fingerprint comparison); Database Internals Ch. 12 (Merkle trees, read repair, hinted handoff); ACM "Efficient reconciliation and flow control for anti-entropy protocols."

Anti-entropy: background process compares replicas via digest exchange (Merkle trees or version vectors) and reconciles divergent ranges. Cassandra builds a Merkle tree per replica (leaf = row hash), compares trees top-down, streams only differing leaves. Zenoh slices time into eras with era-based hash trees; replicas publish fingerprints, compare, identify missing data. `ids-sync-checkpoint` uses Merkle roots as checkpoint state roots and supports anti-entropy reconciliation reference: given two checkpoints, the Merkle proof path identifies which sub-ranges diverge, enabling targeted repair rather than full resync.

#### 3.11 Dotted version vectors and causal tracking

**Source:** Riak "Vector Clocks Revisited Part 2: Dotted Version Vectors"; ACM DL 10.1145/2332432.2332497 "Efficient causality tracking with dotted version vectors"; UMass DS-2-time lecture notes on logical/vector clocks.

Version vectors track per-node causality but grow with node count. Dotted version vectors (DVV) compact this: a dot is (node_id, counter); a DVV is a set of dots plus a version vector covering the causal past. Causal precedence: dot_a < dot_b iff dot_a.counter ≤ vv_b[dot_a.node]; concurrent iff neither precedes. `ids-sync-checkpoint` stores a DVV as the checkpoint's causal context, enabling resume to verify that all causally-preceding operations are included before the checkpoint position.

#### 3.12 Idempotent replay and exactly-once semantics

**Source:** Apache Flink exactly-once overview; stripe/idempotency-key documentation; CheckMate "Evaluating Checkpointing Protocols for Streaming Dataflows."

Exactly-once processing: each event affects results exactly once even under failure. Flink uses two-phase commit (pre-commit = checkpoint barrier, commit = external state commit). Idempotency keys give exactly-once at the application layer. `ids-sync-checkpoint` guarantees exactly-once resume: state restored from checkpoint, then each post-checkpoint operation applied exactly once via idempotent application keys derived from `OpRef`.

#### 3.13 Checkpoint garbage collection

**Source:** SlateDB docs "Checkpoints" (manifest version pinning, GC keeps reachable manifests/SSTs, checkpoint expiration); OriginTrail SWM public snapshot GC (pressure-based, age + free-space watermarks); RetainZ "Reclaim-Time-Aware Placement for AI Checkpoints" (reclaim epochs); GC-CR decentralized garbage collector for checkpointing.

GC strategies: retain-by-count (keep N most recent), retain-by-age (keep checkpoints newer than T), pressure-based (evict when disk below watermark), reachability-based (mark-and-sweep from live roots: active checkpoints + pinned resume positions). SlateDB: a checkpoint pins a manifest version; GC cannot remove manifests/SSTs reachable from active checkpoints. `ids-sync-checkpoint` implements retain-by-count, retain-by-age, and reachability-based GC with a grace period; a checkpoint referenced by an in-flight resume or the latest stable checkpoint is never collected.

#### 3.14 Checkpoint interval optimization

**Source:** ScienceDirect "A utilization model for optimization of checkpoint intervals in distributed stream processing systems" (FGCS 2020); CPE-05-0002 "Performance and effectiveness trade-off for checkpointing"; ACM TECS "Optimal Checkpointing Strategy for Real-time Systems"; ACM TOPC "Checkpointing Workflows à la Young/Daly Is Not Good Enough."

Young/Daly formula: optimal checkpoint interval balances checkpoint overhead against expected re-execution after failure. Excessive checkpointing degrades performance; deficient checkpointing incurs expensive recovery. Utilization model incorporates checkpoint interval, failure rate, checkpoint cost, failure detection, restart cost, topology depth, message delay. `ids-sync-checkpoint` exposes a configurable interval with guidance: interval = sqrt(2·checkpoint_cost/failure_rate) as a starting point, tunable per deployment.

#### 3.15 Incremental / differential checkpointing

**Source:** Microsoft Learn "Differential Backups (SQL Server)"; SlateDB checkpoints (pin manifest version, cheap metadata-only checkpoint); RocksDB incremental backup (share table files).

A differential/incremental checkpoint is based on a previous full checkpoint and captures only changed state since the base. Much faster than full checkpoints; chains of differentials accumulate until a new full checkpoint is taken. `ids-sync-checkpoint` supports full checkpoints (complete state root) and differential checkpoints (parent link + changed-ranges Merkle proof), with chain length limits to bound resume replay.

#### 3.16 State machine replication checkpoint-based recovery

**Source:** Scixa "Checkpoints and Log Compaction in State-Machine Replication Lifecycle"; jack-vanlightly "VR Revisited - Checkpoint-Based Replica Recovery"; ETH "State Machine Replication Scalability Made Simple" (PBFT signed checkpoints, 2f+1 quorum, stable checkpoints, state transfer from checkpoint + log suffix).

SMR checkpoints: application state snapshots at op-number boundaries; a stale replica recovers by obtaining state from another replica (checkpoint + small log suffix), then switches back to log replication. PBFT: replicas broadcast signed checkpoints after each checkpoint period; 2f+1 matching checkpoints form a stable checkpoint; only then prune below the low watermark. `ids-sync-checkpoint` adopts quorum-signed stable checkpoints and state-transfer-from-checkpoint-plus-suffix recovery.

#### 3.17 Two-phase commit checkpointing for distributed transactions

**Source:** Oracle Database "Distributed Transactions Concepts" (two-phase commit: prepare phase, commit phase).

2PC: coordinator asks participants to prepare (promise to commit/rollback); if all prepare, coordinator sends commit; otherwise rollback. Checkpoints can be taken at prepare/commit boundaries for distributed transaction recovery. `ids-sync-checkpoint` provides the checkpoint primitive at transaction boundaries; the 2PC orchestration lives in higher-layer crates (ids-saga, ids-coordinator).

#### 3.18 Byzantine fault-tolerant checkpoint verification

**Source:** ACM DL 10.1145/3436728 "Byzantine Fault-tolerant State-machine Replication from a Systems Perspective"; ETH SMR scalability paper.

BFT checkpoints: replicas broadcast signed checkpoints; a stable checkpoint requires 2f+1 signatures from distinct replicas; a trailing replica can verify a checkpoint from a single correct source because the proof contains 2f+1 signatures. `ids-sync-checkpoint` supports BFT verification mode: checkpoint header signed by the creating replica, stable when quorum of signatures collected; verification checks signature validity and quorum count.

#### 3.19 CRDT checkpoint anti-entropy convergence

**Source:** ACM Computing Surveys "Approaches to Conflict-free Replicated Data Types" (state-based vs op-based); arXiv 1603.01529 delta-CRDTs.

CRDTs guarantee strong convergence: replicas receiving the same update set converge to the same state, regardless of delivery order. State-based CRDTs merge full state (join-semilattice least upper bound); op-based propagate operations with causal delivery. Checkpoints of CRDT state are mergeable; anti-entropy exchanges Merkle digests of CRDT state and reconciles divergences. `ids-sync-checkpoint` stores CRDT-opaque state roots and supports CRDT merge as the reconciliation reference.

#### 3.20 Vector clocks and Lamport timestamps

**Source:** UMass DS-2-time lecture notes; Wikipedia causal consistency; MIT 6.824 linearizability notes.

Lamport clocks satisfy e → e′ ⇒ TS(e) < TS(e′); vector clocks satisfy e → e′ ⇔ V(e) < V(e′) and detect concurrency. `ids-sync-checkpoint` uses logical timestamps only (never wall-clock) for checkpoint ordering; wall-clock is never used for causality or resume decisions.

#### 3.21 LSM compaction and tombstone GC

**Source:** VLDB 2021 "Constructing and Analyzing the LSM Compaction Design Space."

LSM trees compact sorted runs to reclaim space from overwrites and tombstones. Compaction strategies (leveling, tiering) trade write amplification vs space amplification. `ids-sync-checkpoint` models checkpoint GC analogously: old checkpoints are "compacted" (garbage collected) when their state is covered by newer checkpoints, bounded by retention policy.

#### 3.22 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** RFC 8032 (Ed25519); libsodium XChaCha20-Poly1305 construction (192-bit nonce, random nonces safe); RFC 7539 (ChaCha20-Poly1305 AEAD, 96-bit nonce); NIST FIPS 180-4 (SHA-256); BLAKE3-team README (Merkle-tree parallel hash, KDF, XOF, keyed hash); docs.rs sha2, docs.rs ed25519-dalek.

`ids-sync-checkpoint` uses: ed25519 for signing checkpoint headers (64-byte signatures over canonical envelope bytes); sha256 for hash-chain entries and checkpoint ID domain separation; blake3 for checkpoint IDs (content hash of state root + position + parent), Merkle roots, and KDF-derived sub-keys; xchacha20poly1305 for encrypting checkpoint payload at rest with random 192-bit nonces; chacha20poly1305 for encrypting small session/state payloads with 96-bit nonces. No other crypto.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|--------------------------|
| Torn / partial checkpoint write | Checkpoint integrity | Write to temp file, fsync, atomic rename; verify signature + hash chain before accepting |
| Checkpoint forgery / tampering | Decision authenticity | ed25519 signature over canonical envelope bytes; quorum of signatures for stable checkpoints |
| Replay of stale checkpoint | Resume correctness | Hash chain prevents replay of superseded checkpoints; resume validates checkpoint is ancestor of current position |
| Checkpoint chain fork (divergent parents) | Consistency | Single parent link per checkpoint; verification rejects checkpoints whose parent chain doesn't converge to genesis |
| Causal gap in checkpoint (missing dependency) | Causal correctness | DVV causal context stored; resume rejects checkpoint if causal past not satisfied |
| Checkpoint DoS via excessive creation | Storage exhaustion | Configurable max checkpoints; GC enforces retain-by-count/age/reachability |
| Encryption key extraction | Payload confidentiality | Keys never leave node; xchacha20poly1305/chacha20poly1305 at rest; zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
| Clock manipulation for checkpoint order | Ordering correctness | Logical positions (OpRef/OrderingPos) only; never wall-clock for checkpoint ordering |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent checkpoint application; idempotency keys from OpRef; carrier is opaque |
| Checkpoint version skew (incompatible format) | Interoperability | Checkpoint format version in header; verification rejects unknown/unsupported versions |

The carrier is never trusted: checkpoint integrity is cryptographic (signature + hash chain), surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-sync-checkpoint |
|-------|------|----------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `OpRef`, `OrderingPos`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All checkpoint traffic flows to/from `Endpoint`; crate returns envelopes, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing checkpoint headers; hash chains; Merkle roots; encrypting payload at rest |
| `ids-replication` | Replica synchronization (primary user) | Creates checkpoints at positions; consumes checkpoints for state transfer and catch-up |
| `ids-sync` | Resumable replication (primary user) | Resumes from checkpoint; replays post-checkpoint operations |
| `ids-recovery` | Crash recovery / WAL replay | Uses checkpoints as recovery start points; replays WAL after checkpoint position |
| `ids-crdt` | CRDT state (mergeable checkpoints) | CRDT state roots stored in checkpoints; CRDT merge as reconciliation reference |
| `ids-fault` | Fault detection / failure domains | Failure events trigger checkpoint-based recovery; fault layer consults checkpoints |
| `ids-membership` | Replica membership / region topology | Quorum calculations for checkpoint stability; region-scoped checkpoints |

Integration points: `ids-sync-checkpoint` produces `CheckpointEnvelope` and `ResumePosition` values; consumers frame them into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. The crate never reaches into dependent crate internals and never sends bytes itself.

---

### 6. Proposed File Structure (~90-1000 files in subfolders)

**Crate:** `ids-sync-checkpoint` (new crate at `C:/idfs/ids-sync-checkpoint/`; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`):

```
ids-sync-checkpoint/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, serde, thiserror, blake3, sha2
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # Checkpoint defaults: interval, retain count, max chain length
├── error.rs                            # CheckpointError (thiserror): creation, verify, resume, GC, version errors
├── features.rs                         # Feature flags: bft, encrypted, incremental, metrics
│
├── creation/                           # Checkpoint creation
│   ├── mod.rs
│   ├── builder.rs                      # Builds CheckpointEnvelope from state + position
│   ├── anchor.rs                       # Last-applied position anchor (OpRef/OrderingPos + term/epoch)
│   ├── causal_ctx.rs                   # DVV causal context capture at checkpoint time
│   ├── full.rs                         # Full checkpoint (complete state root)
│   └── incremental.rs                  # Differential checkpoint (parent link + changed ranges)
│
├── verification/                       # Checkpoint verification
│   ├── mod.rs
│   ├── verifier.rs                     # Signature + hash chain + state root verification
│   ├── signature_check.rs              # ed25519 signature validation over envelope bytes
│   ├── chain_check.rs                  # Hash chain integrity (sha256/blake3)
│   ├── quorum_check.rs                 # Quorum-signature check for stable checkpoints (BFT mode)
│   ├── version_check.rs                # Checkpoint format version compatibility
│   └── causal_check.rs                 # DVV causal past satisfied before resume
│
├── resume/                             # Resume from checkpoint
│   ├── mod.rs
│   ├── planner.rs                      # Computes resume position + post-checkpoint op range
│   ├── position.rs                     # ResumePosition: checkpoint_id, last_position, op range
│   ├── replay_guard.rs                 # Idempotent replay guard (OpRef-based dedup)
│   └── fallback.rs                     # Full-replay-from-genesis fallback path
│
├── reconciliation/                     # Reconciliation reference
│   ├── mod.rs
│   ├── merkle_root.rs                  # BLAKE3 Merkle root of checkpoint state
│   ├── merkle_proof.rs                 # Merkle proof path for divergent sub-ranges
│   ├── diff.rs                         # Differential diff between two checkpoints
│   └── anti_entropy_ref.rs             # Anti-entropy digest reference for repair guidance
│
├── versioning/                         # Checkpoint versioning
│   ├── mod.rs
│   ├── format_version.rs               # Checkpoint envelope format version
│   ├── chain.rs                        # Checkpoint chain (parent links, genesis anchor)
│   ├── chain_validator.rs              # Validates chain converges to genesis, no forks
│   └── migration.rs                    # Format migration rules between versions
│
├── gc/                                 # Checkpoint garbage collection
│   ├── mod.rs
│   ├── policy.rs                       # GC policy: retain-by-count, retain-by-age, reachability
│   ├── collector.rs                    # Mark-and-sweep from live roots (active checkpoints, pinned resumes)
│   ├── retainer.rs                     # Retain-by-count and retain-by-age enforcement
│   └── grace.rs                        # Grace period for in-flight resumes before collection
│
├── store/                              # Checkpoint envelope storage index
│   ├── mod.rs
│   ├── index.rs                        # Checkpoint ID → envelope location index
│   ├── metadata.rs                     # Checkpoint metadata (created at, size, position, parent)
│   └── layout.rs                       # On-disk layout: temp + atomic rename pattern
│
├── crypto/                             # Crypto surface (allowed set only)
│   ├── mod.rs
│   ├── signer.rs                       # ed25519 signing of checkpoint headers
│   ├── hasher.rs                       # sha256/blake3 hashing for IDs, chains, roots
│   ├── encryptor.rs                    # xchacha20poly1305/chacha20poly1305 payload encryption
│   └── allowlist.rs                    # Compile-time assert: only the five algorithms
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── creation_contract.rs            # trait CheckpointCreation
│   ├── verification_contract.rs        # trait CheckpointVerification
│   ├── resume_contract.rs              # trait CheckpointResume
│   ├── reconciliation_contract.rs      # trait ReconciliationReference
│   ├── versioning_contract.rs          # trait CheckpointVersioning
│   ├── gc_contract.rs                  # trait CheckpointGC
│   ├── store_contract.rs               # trait CheckpointStore
│   ├── crypto_contract.rs              # trait CheckpointCrypto
│   ├── carrier_contract.rs             # trait CheckpointCarrier (opaque Endpoint only)
│   └── audit_contract.rs               # trait CheckpointAudit (signed audit log)
│
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── consistent_cut.rs               # Checkpoint represents causally closed state
│   ├── chain_linear.rs                 # Checkpoint chain has no forks, converges to genesis
│   ├── quorum_stable.rs                # Stable checkpoint requires quorum signatures
│   ├── causal_past.rs                  # Resume requires causal past satisfied
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305
│   ├── atomic_write.rs                 # Temp+fsync+rename for checkpoint persistence
│   └── idempotent_resume.rs            # Replay guard prevents duplicate application
│
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── mod.rs
│   ├── audit_entry.rs                  # One checkpoint decision per entry (create/verify/resume/gc)
│   ├── audit_chain.rs                  # sha256/blake3 hash chain over entries
│   ├── checkpoint_audit.rs             # ed25519-signed checkpoint audit checkpoints
│   ├── signer.rs                       # Audit entry signing
│   ├── verifier.rs                     # Audit chain verification
│   └── replay_cache.rs                 # Bounded replay cache per checkpoint ID
│
├── tests/                              # Property/verification tests (~36 files)
│   ├── mod.rs
│   ├── creation_tests.rs
│   ├── verification_tests.rs
│   ├── resume_tests.rs
│   ├── reconciliation_tests.rs
│   ├── versioning_tests.rs
│   ├── gc_tests.rs
│   ├── chain_tests.rs
│   ├── causal_tests.rs
│   ├── idempotency_tests.rs
│   ├── crypto_allowlist_tests.rs
│   ├── carrier_agnostic_tests.rs       # Asserts no std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── atomic_write_tests.rs
│   ├── audit_chain_tests.rs
│   └── property_tests.rs               # proptest properties for envelopes/chains
│
├── benches/                            # (~10 files)
│   ├── mod.rs
│   ├── creation_bench.rs
│   ├── verification_bench.rs
│   ├── resume_bench.rs
│   ├── gc_bench.rs
│   └── chain_validation_bench.rs
│
├── docs/                               # (~16 files)
│   ├── architecture.md
│   ├── checkpoint_spec.md              # Checkpoint envelope format spec
│   ├── resume_spec.md                  # Resume-from-checkpoint procedure spec
│   ├── gc_spec.md                      # Garbage collection policy spec
│   ├── carrier_agnostic_spec.md        # Carrier-agnostic boundary contract
│   ├── crypto_policy.md                # Allowed-crypto allowlist policy
│   ├── adding_a_checkpoint_type.md     # Adding full/incremental checkpoint variants
│   └── runbook.md                      # Operating checkpoints: interval, retention, GC
│
└── test_vectors/                       # Deterministic inputs and expected outputs
    ├── envelope_vectors.json
    ├── chain_vectors.json
    ├── resume_vectors.json
    └── merkle_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — creation, verification, resume, reconciliation, versioning, GC, store, crypto, contracts, invariants, audit, tests, benches, docs, test vectors. Reflects the genuine checkpoint-management complexity (consistent cuts, hash chains, quorum signatures, Merkle proofs, GC reachability). No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

**1. Checkpoint envelope construction**
```
CheckpointEnvelope {
  header: CheckpointHeader { version, id, parent_id, position, term, state_root, causal_ctx, created_at },
  signature: ed25519 Signature over canonical(header) bytes,
  hash_chain_entry: sha256(header || prev_chain_hash),
  encrypted_payload: Optional<xchacha20poly1305/chacha20poly1305 ciphertext>
}
CheckpointID = blake3(state_root || position || parent_id || created_at)
```

**2. Consistent-cut checkpoint rule**
A checkpoint is valid iff its state is causally closed: for every operation included in the state, all operations that happened-before it (per DVV causal context) are also included. Equivalent to Chandy-Lamport consistent cut: no effect in the past while its cause is in the future.

**3. Quorum-stable checkpoint (BFT mode)**
A checkpoint becomes stable when `2f+1` distinct replicas have signed identical envelope headers. Verification checks signature validity and quorum count. Only stable checkpoints may serve as resume bases or GC roots.

**4. Resume position computation**
```
ResumePosition { checkpoint_id, last_position (OpRef/OrderingPos), post_checkpoint_op_range, causal_past (DVV) }
Resume is valid iff: checkpoint verified + causal_past ⊆ applied_ops + last_position < current_head
```

**5. Idempotent replay guard**
Each post-checkpoint operation carries an idempotency key derived from `OpRef`. Replay applies each operation at most once; duplicates are detected and skipped. Guarantees exactly-once resume semantics.

**6. Merkle-based reconciliation reference**
Given checkpoint A and checkpoint B with divergent state roots: compare Merkle roots; if equal, states identical; if different, descend the tree comparing child hashes to identify divergent leaf ranges; transfer only differing ranges via `ids-carrier::Endpoint` datagrams.

**7. Garbage collection (mark-and-sweep)**
1. Enumerate live roots: active checkpoints, pinned resume positions, latest stable checkpoint per region.
2. Traverse reachable checkpoints via parent links; mark reachable.
3. Sweep: delete unmarked checkpoints older than retain-by-age or beyond retain-by-count.
4. Grace period: checkpoints referenced by in-flight resumes are never collected.

**8. Atomic checkpoint persistence**
Write envelope to temp file → fsync → atomic rename to final location. Verification reads final location only; temp files never visible to readers. Prevents torn checkpoints.

**9. Checkpoint chain validation**
Walk parent links from a checkpoint to genesis; reject if: cycle detected, fork detected (two checkpoints claim same child), missing parent, or format version unsupported.

**10. Interval optimization (Young/Daly-derived)**
Suggested checkpoint interval: `sqrt(2 · checkpoint_cost / failure_rate)` as a deployment starting point; trades checkpoint overhead against expected re-execution after failure. Configurable per deployment.

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `OpRef`, `OrderingPos`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — envelope transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` + `serde_bytes` | Checkpoint envelope serialization |
| `thiserror` | Error types |
| `blake3` | Checkpoint IDs, Merkle roots, content hashing |
| `sha2` | SHA-256 hash-chain entries |
| `proptest` | Property-based tests |
| `tempfile` | Test fixtures |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list; `ids-sync-checkpoint` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-63.1 | Checkpoint creation produces a signed, hash-chained envelope with BLAKE3-256 ID | Envelope contains ed25519 signature, sha256/blake3 hash-chain entry, BLAKE3 ID |
| AC-63.2 | Checkpoint verification validates signature + hash chain + state root + causal context | Verification returns Verified/Rejected with reason; 100% detection of tampered envelopes |
| AC-63.3 | Resume from checkpoint returns correct ResumePosition + post-checkpoint op range | Resume position = checkpoint's last position; replay starts strictly after checkpoint |
| AC-63.4 | Exactly-once resume: no operation applied twice after resume | Idempotency-key dedup; property test proves at-most-once application |
| AC-63.5 | Consistent-cut invariant: checkpoint state is causally closed | DVV causal past ⊆ operations included in checkpoint state |
| AC-63.6 | Quorum-stable checkpoints (BFT mode): 2f+1 signatures required | Stable checkpoint rejected unless quorum signatures verified |
| AC-63.7 | Checkpoint chain linear: no forks, converges to genesis | Chain validator walks parent links; rejects cycles/forks/missing parents |
| AC-63.8 | Garbage collection: retain-by-count, retain-by-age, reachability enforced | GC collects only unmarked checkpoints beyond policy limits; live roots never collected |
| AC-63.9 | Atomic persistence: temp + fsync + rename; no torn checkpoints | Crash-during-write test: recovery never observes partial envelope |
| AC-63.10 | Merkle reconciliation: divergent sub-ranges identified via proof path | Merkle proof descent identifies changed leaves; only differing ranges transferred |
| AC-63.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep passes |
| AC-63.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-63.13 | Every checkpoint decision recorded in append-only, hash-chained, ed25519-signed audit log | Audit chain verifies end-to-end |
| AC-63.14 | Logical positions only (OpRef/OrderingPos); never wall-clock for checkpoint ordering or resume decisions | No `SystemTime`/`Instant` used for causality/ordering |
| AC-63.15 | Full-replay-from-genesis fallback works when checkpoint unavailable | Fallback path replays all operations from genesis |

---

### 10. Remaining Uncertainties

- **Checkpoint interval default:** the optimal interval depends on failure rate and checkpoint cost (Young/Daly model); concrete defaults to be tuned per deployment at integration with `ids-replication`/`ids-sync`.
- **Quorum size for stable checkpoints:** `2f+1` for BFT mode; non-BFT mode may use simple majority. Deployment-dependent.
- **Differential checkpoint chain length limit:** unbounded differential chains increase resume time; max chain length before forcing a full checkpoint is unquantified.
- **GC grace period duration:** how long to retain a checkpoint referenced by an in-flight resume before collection; tradeoff between storage and resume success.
- **Encryption of checkpoint payload:** optional (`encrypted` feature); key management and rotation for at-rest encryption not yet designed.
- **Interaction with `ids-crdt` (checkpoint of CRDT state):** CRDT merge as reconciliation reference; exact merge-audit interface to be validated at integration.
- **Cross-region checkpoint consistency:** region-scoped checkpoints and cross-region stable checkpoints need quorum-per-region semantics; unquantified latency impact.
- **Checkpoint format migration:** rules for migrating between envelope format versions undefined; migration path TBD.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-sync-checkpoint` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear checkpoint boundary |
| Checkpoint = consistent cut (Chandy-Lamport principle) | Guarantees causally closed state; no effect in past while cause in future |
| Anchor = last-applied position (OpRef/OrderingPos) + term/epoch | Raft-style anchor positions checkpoint in the log coordinate system |
| Quorum-signed stable checkpoints (2f+1 for BFT) | PBFT pattern; trailing replica can verify from single correct source |
| Exactly-once resume via idempotency keys (OpRef-derived) | Exactly-once processing semantics under failure; duplicates suppressed |
| Merkle root as state root + Merkle proof for reconciliation | Efficient divergence detection; transfer only differing ranges (Cassandra anti-entropy pattern) |
| DVV causal context stored per checkpoint | Compact causality tracking; resume validates causal past before applying |
| Atomic persistence (temp + fsync + rename) | Prevents torn checkpoints; crash-safe writes |
| GC = mark-and-sweep from live roots + retain-by-count/age | SlateDB/OriginTrail patterns; never collect checkpoints referenced by in-flight resumes |
| Logical positions only; never wall-clock for ordering | Clock manipulation cannot break checkpoint order or resume correctness |
| All traffic via `ids-carrier::Endpoint`; crate returns envelopes, never sends | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; `ids-crypto` exposes primitives via `ids-000-aa` | Complies with allowed-crypto rule |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |

---

### 12. Sources

1. Chandy & Lamport, "Distributed Snapshots: Determining Global States of Distributed Systems," ACM Transactions on Computer Systems, Vol 3, No 1 (1985). https://dl.acm.org/doi/10.1145/214451.214456 — consistent cuts, marker-based snapshot, global state recording.
2. Princeton COS418, "Distributed Snapshots" lecture notes. https://www.cs.princeton.edu/courses/archive/fall26/cos418/docs/L6-snapshots.pdf — Chandy-Lamport algorithm details, consistent cut interpretation.
3. UMass 590CC, "Global State and Snapshot Recording Algorithms." https://people.cs.umass.edu/~arun/590CC/lectures/Snapshots.pdf — consistent cut, marker sending/receiving rules.
4. "Checkpointing Techniques in Distributed Systems: A Synopsis" (SBC WTF). https://sol.sbc.org.br/index.php/wtf/article/download/24679/24500 — coordinated vs uncoordinated checkpointing, two-phase checkpoint protocol.
5. Springer, "Implementing Rollback-Recovery Coordinated Checkpoints" (ISSADS 2005). https://link.springer.com/chapter/10.1007/11533962_22 — coordinated non-blocking checkpoints, recovery line.
6. PostgreSQL 19 docs, "Write-Ahead Logging (WAL)." https://www.postgresql.org/docs/current/wal-intro.html — WAL, crash recovery from checkpoint, checkpoint records.
7. PostgreSQL 19 docs, "WAL Configuration." https://www.postgresql.org/docs/current/wal-configuration.html — checkpoint interval, crash recovery start point.
8. Ongaro & Ousterhout, "In Search of an Understandable Consensus Algorithm" (Raft). https://raft.github.io/raft.pdf — snapshotting, InstallSnapshot RPC, last-included index/term anchor.
9. hashicorp/raft snapshot.go. https://github.com/hashicorp/raft/blob/main/snapshot.go — snapshot compaction, last stable snapshot, log truncation.
10. MerceMay, "Snapshot Install and the Cost of Compacting a Consensus Log." https://mercemay.com/articles/distributed-systems/consensus/log-compaction-snapshot-install/ — snapshot boundary, install vs append, snapshot transfer.
11. openraft docs, snapshot replication. https://docs.rs/openraft/latest/openraft/docs/protocol/replication/snapshot_replication/index.html — snapshot-log conditions, install snapshot, purge logs.
12. Apache Flink docs, "Stateful Stream Processing." https://nightlies.apache.org/flink/flink-docs-stable/docs/concepts/stateful-stream-processing — checkpoint = stream position + operator state, exactly-once resume, interval vs recovery tradeoff.
13. Apache Flink, "An Overview of End-to-End Exactly-Once Processing." https://flink.apache.org/2018/02/28/an-overview-of-end-to-end-exactly-once-processing-in-apache-flink-with-apache-kafka-too — two-phase commit, checkpoint barriers, exactly-once semantics.
14. Apache Kafka design. https://kafka.apache.org/08/design/design — consumer offset checkpointing, rewind to old offset.
15. facebook/rocksdb wiki, "How to backup RocksDB." https://github.com/facebook/rocksdb/wiki/How-to-backup-RocksDB — incremental backup, share table files, verify backup.
16. CodeOpinion, "Snapshots in Event Sourcing for Rehydrating Aggregates." https://codeopinion.com/snapshots-in-event-sourcing-for-rehydrating-aggregates — snapshot + post-snapshot event replay, frequency vs cost tradeoff.
17. Azure Architecture Center, "Event Sourcing Pattern." https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing — snapshot as optimization, full replay as fallback.
18. Axon Framework, "Snapshots." https://docs.axoniq.io/axon-framework-reference/5.3/tuning/snapshotting — snapshot policy, snapshot lifecycle, fallback to full replay.
19. arXiv 2104.04102, "Read-Write Quorum Systems Made Practical." https://arxiv.org/html/2104.04102v1 — read/write quorum intersection, R+W>N.
20. Cassandra docs, "Anti-entropy repair." https://docs.datastax.com/en/cassandra-oss/3.x/cassandra/operations/opsRepairNodesManualRepair.html — Merkle trees, digest exchange, reconcile divergent replicas.
21. Zenoh blog, "Keeping storages aligned in Zenoh." https://zenoh.io/blog/2022-11-29-zenoh-alignment — anti-entropy, era-based hash trees, fingerprint comparison.
22. Riak, "Vector Clocks Revisited Part 2: Dotted Version Vectors." https://riak.com/posts/technical/vector-clocks-revisited-part-2-dotted-version-vectors/ — DVV causal precedence, compact causality tracking.
23. ACM DL 10.1145/2332432.2332497, "Efficient causality tracking with dotted version vectors." https://dl.acm.org/doi/10.1145/2332432.2332497 — DVV definition, causal comparison.
24. UMass DS-2-time lecture notes, "The notion of time." https://tropars.github.io/downloads/lectures/DS/DS-2-time.pdf — Lamport clocks, vector clocks, happened-before.
25. SlateDB docs, "Checkpoints." https://slatedb.io/docs/design/checkpoints/ — checkpoint pins manifest version, GC keeps reachable, checkpoint expiration.
26. OriginTrail, "SWM Public Snapshot Garbage Collection." https://docs.origintrail.io/operate/swm-public-snapshot-gc — pressure-based GC, age + free-space watermarks.
27. RetainZ, "Reclaim-Time-Aware Placement for AI Checkpoints." https://arxiv.org/pdf/2610.05554 — reclaim epochs, retention policies for checkpoints.
28. ScienceDirect, "A utilization model for optimization of checkpoint intervals." https://www.sciencedirect.com/science/article/abs/pii/S0167739X19320102 — checkpoint interval optimization, utilization model.
29. ACM TOPC, "Checkpointing Workflows à la Young/Daly Is Not Good Enough." https://dl.acm.org/doi/full/10.1145/3548607 — Young/Daly checkpoint interval tradeoff.
30. ACM DL 10.1145/3436728, "Byzantine Fault-tolerant State-machine Replication." https://dl.acm.org/doi/fullHtml/10.1145/3436728 — signed checkpoints, 2f+1 quorum, stable checkpoint, state transfer from single correct source.
31. ETH, "State Machine Replication Scalability Made Simple." https://www.research-collection.ethz.ch/server/api/core/bitstreams/ddcdffc2-dab6-4c08-8249-1fa08251972f/content — PBFT checkpoints, Merkle roots, stable checkpoints, garbage collection.
32. Scixa, "Checkpoints and Log Compaction in State-Machine Replication." https://scixa.com/article?slug=checkpoints-log-compaction-state-machine-replication — checkpoint semantics, beside-the-log vs in-the-protocol, truncation safety.
33. jack-vanlightly, "VR Revisited - Checkpoint-Based Replica Recovery." https://jack-vanlightly.com/analyses/2023/01/22/paper-vr-revisited-checkpoint-based-replica-recovery-part-6 — checkpoint + log suffix recovery, periodic checkpointing.
34. BLAKE3-team BLAKE3 README. https://github.com/BLAKE3-team/BLAKE3 — BLAKE3 hash, Merkle-tree parallel, KDF, XOF, keyed hash.
35. NIST FIPS 180-4, Secure Hash Standard (SHA-256). https://csrc.nist.gov/projects/hash-functions — SHA-256 specification.
36. docs.rs sha2 — SHA-256 Rust implementation. https://docs.rs/sha2/latest/sha2/struct.Sha256.html
37. docs.rs ed25519-dalek — ed25519 sign/verify. https://docs.rs/ed25519-dalek
38. libsodium, XChaCha20-Poly1305 construction. https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction — 192-bit nonce, random nonces safe.
39. RFC 7539, ChaCha20 and Poly1305 for IETF Protocols. https://www.rfc-editor.org/info/rfc7539 — 96-bit nonce AEAD.
40. RFC 8032, Edwards-Curve Digital Signature Algorithm (EdDSA). https://www.rfc-editor.org/rfc/rfc8032 — Ed25519 signature algorithm.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 063 (ids-sync-checkpoint).
