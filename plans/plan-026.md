## Phase 026 — Create the ids-snapshot crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 026 creates **`ids-snapshot`**, a new separate crate in the IDS workspace (C:/idfs) providing **consistent global state for checkpointing and recovery**. The crate implements the Chandy-Lamport distributed snapshot algorithm (marker messages, consistent cuts, in-flight/channel message recording), stable property detection, global snapshots, savepoints, checkpoints, incremental checkpoints, and state restoration — all carrier-agnostic, riding only `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), with zero direct network dependencies.

**Scope of ids-snapshot (research-verified):**
- **Chandy–Lamport global snapshot algorithm** — any node can initiate; initiator records local state, sends marker messages on all outgoing logical channels, begins recording inbound messages; on first marker received a node records its local state, marks the incoming channel as empty, forwards markers on all outgoing channels, and records subsequent messages on that channel as in-flight channel state; on a second marker received on a channel it records that channel's captured messages as channel state. Produces a consistent cut (if event `e` is captured, all events that happened-before `e` are also captured). Termination when every node has recorded its state and a marker on every incoming channel. (Chandy & Lamport 1985, "Distributed Snapshots: Determining Global States of Distributed Systems"; MIT 6.852J; Princeton COS418; UIUC ECE428/CS425.)
- **Marker messages** — special control messages distinct from application messages, FIFO-ordered on logical channels, barrier between pre-snapshot and post-snapshot traffic; guarantee zero loss + zero duplication of recorded channel state.
- **Consistent global state / consistent cut** — a cut `C` is consistent iff for every pair of events `e, f`, if `f ∈ C` and `e → f` (happens-before) then `e ∈ C`. A global state is consistent iff it corresponds to a consistent cut. Vector clocks (Fidge/Mattern) provide the causality test: cut is consistent iff `∀i, V(i) = V_i(i)` (no one knows more about a node than that node knows about itself).
- **In-flight message recording** — messages sent before the sender's state is recorded but received after the receiver's state is recorded are captured as channel state; this is what makes the cut consistent (no orphan receive without its matching send).
- **Stable property detection** — a predicate `y` is stable if once true it remains true; record a consistent global state `S*` and evaluate `y(S*)`; correctness follows from `S*` reachable from start and end reachable from `S*`. Examples: computation terminated, deadlock detected, token lost. Unstable predicates cannot be detected by snapshots alone.
- **Global snapshot, savepoint, checkpoint** — snapshot = consistent global state captured at runtime; savepoint = a named, exportable consistent state image for stop-and-resume/fork/update; checkpoint = a durable point-in-time state used for rollback recovery.
- **Incremental checkpoint** — differential snapshots capturing only state changed since the last checkpoint/base checkpoint; tiered checkpointing keeps lightweight differential checkpoints locally/peer-side and migrates heavyweight base checkpoints to durable storage.
- **State restoration** — restore from latest checkpoint + replay journal/delta entries since the checkpoint (snapshot + WAL pattern); recovery = load snapshot + replay journal; fail closed on hash mismatch; never partial replay on corruption.

**Key engineering decisions:**
- **New separate crate `ids-snapshot`**, not folders inside existing crates (per IDS rule: separate crates instead of more folders in existing ones). Depends only on `ids-000-aa`, `ids-carrier` (endpoint/opaque bytes), `ids-crypto`-allowed primitives; carrier-agnostic over `Endpoint`.
- **Chandy–Lamport over logical channels** — snapshot state is captured over `ids-carrier::Endpoint` datagrams; marker messages are opaque `Vec<u8>` payloads distinguished by a message-type tag, sent via `carrier.send_datagram(endpoint, &[u8])`. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.
- **Consistent cut via marker barrier** — FIFO ordering of markers relative to app messages on each logical channel is the consistency mechanism; no wall-clock or global snapshot coordinator required.
- **Vector-clock causality tracking** — each node maintains a vector clock (one entry per known node) tagged on state and markers; consistent-cut validation via the Mattern/Fidge max-vector rule.
- **Persistence: checkpoint + delta journal** — periodic full checkpoints (versioned, checksummed, bincode/CBOR, optional compression) plus append-only delta journal; recovery = load latest checkpoint + replay journal since checkpoint. Tombstones/compaction gated on causal stability (all nodes acked past a checkpoint).
- **Allowed crypto only**: ed25519 (snapshot signing/integrity), xchacha20poly1305 + chacha20poly1305 (at-rest snapshot encryption), sha256 and blake3 (content addressing, checksums, Merkle roots).
- **ids-000-aa is the master crate** — `Snapshot` sealed trait, `NodeId`, `Endpoint`, `CarrierType`, `DatasetId`, `SnapshotError`, etc. live there; `ids-snapshot` implements downstream.

---

### 2. Architecture Overview

The ids-snapshot crate sits between the application/state layer and the carrier layer, below `ids-recovery`/`ids-replication` and above `ids-carrier`. It is **not** a network protocol — it is a local, deterministic consistent-state capture engine that produces bytes shipped over `ids-carrier::Endpoint`.

```
┌─────────────────────────────────────────────────────────────┐
│              APPLICATION / ids-state                        │
│  local mutable state to be snapshotted                      │
├─────────────────────────────────────────────────────────────┤
│             ids-recovery / ids-replication                  │
│  rollback recovery, checkpoint triggers, restore requests   │
├─────────────────────────────────────────────────────────────┤
│         Phase 026: ids-snapshot (NEW CRATE)                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  snapshot engine: Chandy-Lamport, marker messages     │  │
│  │  consistent cut, channel/in-flight recording          │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  vector clock + causality / consistent-cut validation │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  checkpoint / savepoint / incremental checkpoint      │  │
│  │  versioned, checksummed, compressed (blake3/sha256)   │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  state restoration: checkpoint + delta journal replay │  │
│  │  fail-closed on hash mismatch                         │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  stable property detection (termination, deadlock)    │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  wire: marker + snapshot chunks over ids-carrier::Endpoint│ │
│  └───────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│              ids-carrier (Endpoint opaque bytes)            │
│  CarrierType + Vec<u8>; send_datagram / receive_datagram    │
├─────────────────────────────────────────────────────────────┤
│             ids-000-aa (master, sealed traits)              │
│  NodeId, Endpoint, CarrierType, DatasetId, Snapshot trait   │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a coordinated snapshot:**
1. Initiator `i` records local state `S_i`, tags with vector clock `VC_i`, sends a marker on every outgoing logical `Endpoint`, begins recording inbound messages on all incoming channels.
2. On receiving a marker on channel `c` from `k`: if first marker seen — record local state `S_j`, mark channel `c` state as empty, forward markers on all outgoing channels, begin recording inbound messages on all other incoming channels. If marker already seen — record channel `c` state as all messages received since local state was recorded (the in-flight messages).
3. Termination: every node has recorded local state and a marker on every incoming channel. The collection of local states + channel states forms a consistent global snapshot.
4. The snapshot is serialized (versioned, BLAKE3/SHA-256 checksummed, optional XChaCha20-Poly1305/ChaCha20-Poly1305 encrypted), written as a checkpoint/savepoint; a delta journal records subsequent changes.
5. Recovery: load latest checkpoint, replay delta journal; fail closed on hash/checksum mismatch; never partial replay on corruption.

---

### 3. Detailed Research Dimensions

#### 3.1 Chandy–Lamport Global Snapshot Algorithm

The foundational algorithm (Chandy & Lamport, 1985). System model: `n` processes, two unidirectional FIFO channels per ordered pair, all messages arrive intact and unduplicated, no failures, any process may initiate.

**Marker-sending rule (process p):** For each outgoing channel `c`, p sends one marker along `c` after p records its own state and before p sends further messages along `c`. The marker is a special control message with no effect on the underlying computation.

**Marker-receiving rule (process q):** On receiving a marker along channel `c`:
- if q has not recorded its state: q records its state; q records state of `c` as the empty sequence.
- else: q records the state of `c` as the sequence of messages received along `c` after q's state was recorded and before q received the marker along `c`.

**Termination:** All processes have recorded their state; all processes have received a marker on all incoming channels. A central collector (or each node locally) assembles the global snapshot from local states + channel states.

**Properties:** Any run creates a consistent cut — if event `e` happens-before `f` (`e → f`) and `f` is in the cut, then `e` is also in the cut. Proof: by FIFO ordering, markers on each link precede regular application messages, so a receive recorded in the snapshot implies its matching send is also recorded. Works with multiple initiators; decentralized — any process may start a snapshot.

#### 3.2 Consistent Cuts and Global State

A cut `C` selects a prefix of each process's local event history. `C` is consistent iff it is closed under happens-before: `f ∈ C ∧ e → f ⟹ e ∈ C`. Equivalently, every received message in the cut has its matching send in the cut; no orphan receives.

Vector-clock consistency test (Mattern/Fidge): let `V_i` be node `i`'s vector clock at its cut point, and `V = componentwise_max(V_1..V_n)`. The cut is consistent iff `∀i, V(i) = V_i(i)` — no node knows more about any node than that node knows about itself at its cut point. This rules out a node receiving a message (learning more about the sender) before its cut point while the sender's cut point is earlier.

The recorded global state may never have actually occurred during the real execution, but it is reachable by reordering only events within the snapshot window — it is sequentially consistent with a real state.

#### 3.3 Marker Messages and In-Flight Message Recording

Markers are special messages distinct from application messages, carried as opaque `Vec<u8>` payloads over `ids-carrier::Endpoint` with a message-type tag. They act as a barrier: messages sent by `p` before `p` records its state on channel `c` are "before the marker"; messages sent after are "after". Because channels are FIFO, the marker cleanly separates the two classes at the receiver.

In-flight message recording: a message `m` sent by `p` to `q` where `send(m)` is before `p`'s recorded state but `recv(m)` is after `q`'s recorded state is captured as part of channel `c`'s state. Without recording channel state, the snapshot would contain a receive without its matching send (an inconsistent cut / orphan message). The channel state is either "empty" (first-marker case) or the sequence of messages received between local-state recording and marker receipt (second-marker case).

#### 3.4 Stable Property Detection

A predicate `y` over global states is **stable** if `y(S) ⟹ y(S')` for all `S'` reachable from `S` (once true, stays true). Examples: computation terminated, system deadlocked, all tokens disappeared, no in-flight messages.

Detection algorithm: record a consistent global state `S*` via Chandy–Lamport; compute `definite = y(S*)`. Correctness: `S*` is reachable from the start state `S_i`; the termination state `S_f` is reachable from `S*`; by stability, `y(S*) ⟹ y(S_f)` and `¬y(S*) ⟹ ¬y(S_i)`. Unstable predicates cannot be detected by snapshots — a sequence of snapshots may have gaps exactly where the unstable predicate holds.

IDS relevance: stable-property detection underpins termination/quiescence checks before taking checkpoints, and deadlock detection across the carrier-agnostic logical topology.

#### 3.5 Checkpointing, Savepoints, and Incremental Checkpoints

- **Checkpoint:** durable point-in-time consistent state used for rollback recovery. Coordinated checkpointing (Chandy–Lamport markers) guarantees a consistent global checkpoint; uncoordinated/independent checkpoints risk inconsistent cuts and orphan messages, requiring rollback to older consistent checkpoints (domino effect).
- **Savepoint:** a named, exportable consistent state image for stop-and-resume, fork, or upgrade (Apache Flink savepoint pattern: directory of binary state files + small metadata file of pointers; can resume from savepoint, dispose of savepoint).
- **Incremental checkpoint / differential checkpoint:** captures only state changed since the last base checkpoint; tiered designs keep lightweight differential checkpoints local/peer-side and migrate heavyweight base checkpoints to durable storage (TierCheck pattern). Reduces I/O and write amplification vs full-state snapshots.
- **Persistence pattern (recommended):** periodic full checkpoint + append-only delta journal (WAL); recovery = load latest checkpoint + replay journal entries after the checkpoint checkpoint version. Compaction after all nodes ack past a checkpoint version. Avoids full-history replay (observed production case: 55K ops / 276MB replayed 27+ minutes — snapshot+journal bounds recovery to seconds).

#### 3.6 State Restoration

Restoration loads the latest checkpoint to initialize state, then replays the delta journal since the checkpoint version to reach the current state. Correctness rules: no partial replay on corruption; fail closed on hash/checksum mismatch; keep multiple historical checkpoints; tombstones compacted only after all nodes acknowledge (causal stability) to avoid "zombie records" (resurrected deleted data). Offline/never-acking nodes block GC — requires membership management or a grace/quarantine window.

#### 3.7 Vector Clocks and Causality for Snapshot Coordination

Vector clocks (one entry per known node) tag each node's local state and each marker. They enable consistent-cut validation without a global clock. Causal delivery: a node applies a marker/snapshot only after its causal dependencies are satisfied; gaps are tracked and requested. Never use wall-clock for validity decisions; HLC/Lamport may be used as a secondary bounded-skew timestamp but never as authority.

#### 3.8 Snapshot Serialization and Versioning

Snapshot payloads must be versioned for schema evolution. A magic number + format version header precedes the serialized body; a format change bumps the major version; backward compatibility via `#[serde(default)]`, `#[serde(alias)]`, or explicit versioned structs with migration `From`/`TryFrom` impls. Checksums (BLAKE3/SHA-256) on every payload; fail closed on mismatch. Firecracker snapshot versioning pattern (magic_id + MAJOR.MINOR.PATCH format version, independent of binary version) is a reference design.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|------------|
| Inconsistent cut / orphan receive (receive without matching send) | Chandy–Lamport marker barrier over FIFO logical channels; consistent-cut validation via vector clocks |
| Non-FIFO or reordered marker/app messages break consistency | Logical FIFO channel guarantee required from ids-carrier layer; markers tagged with sequence per (sender, channel) |
| Lost or duplicated marker messages | Retransmit markers until acked; idempotent state recording tolerates duplication |
| Snapshot never terminates (node never records / channel never marked) | Per-node completion tracking; timeout-based retry; dead/quarantined nodes handled by ids-recovery |
| Corrupt snapshot/journal payload | BLAKE3/SHA-256 checksum on every chunk/payload; fail closed on mismatch; multiple historical checkpoints |
| Zombie records / resurrected deleted data after GC | Tombstone compaction only after causal stability (all nodes ack); gate GC on all-peer ack |
| Offline node blocks tombstone GC / compaction | Membership-aware causal stability; grace/quarantine window; deliberate eviction, never by timeout |
| Wall-clock skew corrupts causality ordering | Vector clocks / Lamport clocks only; never raw `SystemTime` for validity |
| Unauthorized snapshot/restore | ed25519 signatures on snapshot commitments; integrate with ids-security |
| Snapshot encryption at rest | XChaCha20-Poly1305 / ChaCha20-Poly1305 only (allowed crypto set) |
| Carrier-specific assumptions leaking in | All traffic via `ids-carrier::Endpoint`; zero `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |

---

### 5. Integration with Existing Crates

`ids-snapshot` depends on and integrates with:

| Crate | Role in Phase 026 |
|-------|-------------------|
| `ids-000-aa` | Master crate: `NodeId`, `Endpoint`, `CarrierType`, `DatasetId`, `SnapshotError`, sealed `Snapshot` trait, canonical types |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); all marker/snapshot traffic via `send_datagram`/`receive_datagram` |
| `ids-crypto` (allowed subset) | ed25519 signing, xchacha20poly1305/chacha20poly1305 encryption, sha256/blake3 hashing |
| `ids-recovery` | Consumer: checkpoints/savepoints feed multi-level node/H-unit/region recovery |
| `ids-replication` | Consumer: consistent snapshots for replica state synchronization |
| `ids-state` / application layer | Source of local mutable state to be snapshotted/restored |
| `ids-fault` | Integration: circuit breakers/bulkheads protect snapshot triggers |

**Integration points:**
- `ids-snapshot` produces marker + snapshot-chunk bytes; `ids-carrier` ships them over opaque `Endpoint`s; `ids-snapshot` never imports a carrier implementation.
- `ids-000-aa`'s sealed `Snapshot` trait is implemented by `ids-snapshot`'s `SnapshotEngine`; downstream crates call through the trait, zero downstream edits on contract change.
- `ids-recovery` consumes checkpoints/savepoints for rollback recovery; `ids-replication` consumes consistent snapshots for replica sync.
- Carrier-agnostic by design (I-CARR-001 equivalent): compiles and behaves identically over Loopback/Serial/Optical/Radio/Custom carriers.

---

### 6. Proposed File Structure (~400–600 files, all production-grade)

**Crate:** `ids-snapshot` (new crate at `C:/idfs/ids-snapshot`, depends on `ids-000-aa`, `ids-carrier`, allowed crypto primitives; serde, thiserror, bincode/serde_cbor, blake3, proptest for testing)

```
ids-snapshot/
├── Cargo.toml
├── lib.rs                          # crate root, feature gates, public re-exports
├── constants.rs                    # protocol constants (snapshot intervals, GC thresholds)
├── error.rs                        # SnapshotError (thiserror, comprehensive)
│
├── engine/                         # core snapshot engine
│   ├── mod.rs
│   ├── snapshot_engine.rs          # Chandy-Lamport orchestrator (initiate/record/terminate)
│   ├── initiator.rs                # snapshot initiation: record local state, send markers
│   ├── receiver.rs                 # marker receive handling (first/second marker rules)
│   ├── terminator.rs               # termination detection (all states + all channels recorded)
│   └── engine_types.rs             # SnapshotId, SnapshotState, InitiatorId
│
├── marker/                         # marker messages
│   ├── mod.rs
│   ├── message.rs                  # MarkerMessage enum (marker, snapshot-chunk, ack)
│   ├── sender.rs                   # marker sending rule (after state, before further app messages)
│   ├── receiver.rs                 # marker receiving rule per channel
│   ├── id.rs                       # MarkerId (origin NodeId + seq)
│   └── channel_tag.rs              # logical channel identification over Endpoint
│
├── state/                          # local state capture
│   ├── mod.rs
│   ├── capture.rs                  # local state snapshot + vector clock tagging
│   ├── serializer.rs               # state serialization (versioned bincode/CBOR)
│   ├── diff.rs                     # state diff for incremental snapshots
│   └── state_types.rs              # LocalState, StateVec
│
├── channel/                        # in-flight / channel message recording
│   ├── mod.rs
│   ├── recorder.rs                 # per-channel in-flight message recording
│   ├── channel_state.rs            # ChannelState: empty or captured message sequence
│   ├── inflight.rs                 # in-flight message tracking (sent-before/recorded, received-after)
│   └── channel_types.rs            # ChannelId, MessageRef
│
├── cut/                            # consistent cut management
│   ├── mod.rs
│   ├── consistent_cut.rs           # cut construction from local states + channel states
│   ├── validator.rs                # consistent-cut validation (happens-before closure)
│   └── cut_types.rs                # Cut, CutPoint
│
├── vector_clock/                   # causality tracking
│   ├── mod.rs
│   ├── clock.rs                    # VectorClock (NodeId-keyed counters)
│   ├── hlc.rs                      # optional hybrid logical clock (secondary, never authority)
│   └── causality.rs                # happens-before test, gap tracking
│
├── checkpoint/                     # checkpoint / savepoint
│   ├── mod.rs
│   ├── checkpoint.rs               # durable checkpoint creation/lifecycle
│   ├── savepoint.rs                # named exportable savepoint (metadata + pointers)
│   ├── incremental.rs              # differential/incremental checkpoint
│   ├── lifecycle.rs                # created/finalized/restored/expired states
│   └── checkpoint_types.rs         # CheckpointId, CheckpointHeader, CheckpointVersion
│
├── journal/                        # delta journal (WAL)
│   ├── mod.rs
│   ├── journal.rs                  # append-only delta journal
│   ├── entry.rs                    # journal entry (seq, delta bytes, checksum)
│   ├── replay.rs                   # journal replay for recovery
│   ├── compaction.rs               # journal + tombstone compaction
│   └── journal_types.rs            # JournalSeq, DeltaEntry
│
├── restore/                        # state restoration
│   ├── mod.rs
│   ├── restore.rs                  # load checkpoint + replay journal
│   ├── verifier.rs                 # hash/checksum verification, fail-closed
│   └── restore_types.rs            # RestoreResult, RestoreProof
│
├── property/                       # stable property detection
│   ├── mod.rs
│   ├── detector.rs                 # stable-predicate detection (termination, deadlock)
│   ├── predicate.rs                # predicate trait + stable-property registry
│   ├── termination.rs              # termination detection (quiescence + empty channels)
│   └── property_types.rs           # StableProperty, DetectionResult
│
├── storage/                        # persistence backend adapter
│   ├── mod.rs
│   ├── backend.rs                  # storage trait (sled/memory/file features)
│   ├── snapshot_store.rs           # persisted snapshot chunks
│   ├── checkpoint_store.rs         # persisted checkpoints/savepoints
│   └── storage_types.rs            # StorageRef, ChunkRef
│
├── wire/                           # serialization & wire format
│   ├── mod.rs
│   ├── format.rs                   # versioned wire envelope (magic + version + body)
│   ├── cbor.rs                     # CBOR encoding (deterministic)
│   ├── bincode.rs                  # bincode encoding
│   ├── checksum.rs                 # payload checksums (BLAKE3/SHA-256)
│   └── compression.rs              # optional compression (lz4/zstd)
│
├── crypto/                         # allowed-crypto wrappers
│   ├── mod.rs
│   ├── signing.rs                  # ed25519 snapshot commitment signing
│   ├── encryption.rs               # xchacha20poly1305 / chacha20poly1305 at-rest encryption
│   ├── hashing.rs                  # sha256 / blake3 content addressing + checksums
│   └── crypto_types.rs             # Signature, Nonce, KeyRef
│
├── carrier_adapter/                # ids-carrier integration
│   ├── mod.rs
│   ├── endpoint.rs                 # Endpoint helpers (CarrierType + Vec<u8>)
│   ├── datagram.rs                 # send/receive marker+snapshot chunks via Carrier
│   └── adapter_types.rs            # CarrierSession, DatagramEnvelope
│
├── gc/                             # garbage collection
│   ├── mod.rs
│   ├── stability.rs                # causal stability (per-peer frontiers, MVV min)
│   ├── frontier.rs                 # peer applied/durable frontier tracking
│   └── collector.rs                # incremental tombstone / journal collector
│
├── types/                          # composed public types
│   ├── mod.rs
│   ├── snapshot.rs                 # Snapshot (public, implements ids-000-aa Snapshot trait)
│   ├── snapshot_config.rs          # SnapshotConfig
│   ├── snapshot_chunk.rs           # SnapshotChunk
│   ├── snapshot_restore.rs         # SnapshotRestore
│   ├── snapshot_retention.rs       # SnapshotRetention
│   └── snapshot_stats.rs           # SnapshotStats
│
├── fuzz/                           # fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_marker.rs
│   ├── fuzz_cut.rs
│   └── fuzz_restore.rs
│
├── tests/                          # integration & property tests
│   ├── mod.rs
│   ├── chandy_lamport_tests.rs     # snapshot under loss/dup/reorder
│   ├── consistent_cut_tests.rs     # cut validation
│   ├── checkpoint_tests.rs         # checkpoint/savepoint round-trip
│   ├── incremental_tests.rs        # differential checkpoint
│   ├── restore_tests.rs            # checkpoint + journal recovery
│   ├── property_tests.rs           # stable property detection
│   ├── gc_tests.rs                 # tombstone GC safety
│   ├── adversarial_tests.rs        # offline peer, partition healing
│   └── chaos_tests.rs              # node churn, offline members
│
├── benches/                        # benchmarks
│   ├── mod.rs
│   ├── snapshot_bench.rs           # snapshot capture latency/size
│   ├── restore_bench.rs            # checkpoint + journal recovery time
│   └── incremental_bench.rs        # delta vs full-state size/time
│
├── spec/                           # formal specifications
│   ├── consistent_cut.md           # consistent cut definition & proof
│   ├── chandy_lamport_spec.md      # algorithm specification
│   └── tla/                        # TLA+/PlusCal (placeholder for formal spec)
│
├── docs/                           # documentation
│   ├── architecture.md             # ids-snapshot architecture
│   ├── integration_guide.md        # integrating with ids-carrier/ids-recovery/ids-replication
│   └── crates_guide.md             # when to use checkpoint vs savepoint vs incremental
│
└── test_vectors/                   # deterministic test vectors
    ├── cut_vectors.json
    ├── snapshot_vectors.json
    └── restore_vectors.json
```

**Why ~400–600 files:** each module has a single responsibility; every file contains production-grade code with full error handling, documentation, and unit tests. The count reflects: engine + marker + state + channel + cut + vector_clock + checkpoint + journal + restore + property + storage + wire + crypto + carrier_adapter + gc + types + fuzzing + property/adversarial/chaos tests + benchmarks + formal spec + docs + test vectors. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Chandy–Lamport Snapshot (coordinated, marker-based)

```
initiate(p):
    record local state S_p tagged with VC_p
    for each outgoing channel c: send Marker on c  (after state, before further app msgs)
    begin recording inbound messages on all incoming channels

on receive Marker on channel c from k at q:
    if q has not recorded state:
        record local state S_q tagged VC_q
        record channel c state = empty
        for each outgoing channel c': send Marker on c'
        begin recording inbound on all incoming channels except c
    else:
        record channel c state = messages received since q recorded state
        stop recording on c

terminate: all nodes recorded state + marker on all incoming channels
```

#### 7.2 Consistent Cut Validation (vector clocks)

```
V_i = node i's vector clock at its cut point
V = componentwise_max(V_1..V_n)
cut consistent ⟺ ∀i: V(i) == V_i(i)
```

Happens-before: `e → f` iff `VC(e) < VC(f)` (all components ≤, at least one strict).

#### 7.3 In-Flight Message Recording

```
channel_state(c) = empty                    if c was first-marker channel
                 = [m | recv(m) after state_recorded ∧ recv(m) before marker_recv]
```

#### 7.4 Checkpoint + Delta Journal Recovery

```
Checkpoint { version, state_bytes, vector_clock, checksum, created_at }
Journal: append-only (seq, delta_bytes, checksum)
recover: load latest checkpoint, replay journal entries with seq > checkpoint.version
compact: after all-peer ack past checkpoint version, prune journal + tombstones before it
```

#### 7.5 Stable Property Detection

```
record consistent global state S* via Chandy-Lamport
definite = y(S*)
correctness: S* reachable from start S_i; end S_f reachable from S*; y stable ⟹ y(S*) ⟹ y(S_f)
```

#### 7.6 Causal Stability / GC Gate

```
stable_frontier[i] = min over all known peers of frontier[i]
version v stable ⟺ ∀ replicas r: frontier[r] >= v   (MVV componentwise min)
tombstone t GC-able ⟺ t's remove op is causally stable
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | NodeId, Endpoint, CarrierType, DatasetId, sealed Snapshot trait, SnapshotError |
| `ids-carrier` | Opaque Endpoint datagram send/receive (CarrierType + Vec<u8>) |
| `serde` + `serde_bytes` | Serialization |
| `bincode` or `serde_cbor` | Snapshot/wire encoding |
| `blake3` | Hashing, content addressing, checksums |
| `sha2` | SHA-256 checksums |
| `xchacha20poly1305` / `chacha20poly1305` | At-rest snapshot encryption |
| `thiserror` | Error types |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-snapshot` is transport-agnostic over `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Consistent cut | every captured state closed under happens-before | property test, 100k iterations |
| Marker barrier | zero loss + zero duplication of channel state | unit + chaos test (loss, dup, reorder) |
| Chandy–Lamport termination | all nodes record state + all channels marked | integration test |
| Multi-initiator | concurrent snapshots converge to consistent cuts | chaos test |
| Checkpoint + journal recovery | state restored exactly | round-trip test |
| Incremental checkpoint | delta ≪ full state | bench |
| Restore time | checkpoint + journal ≪ full replay | bench (target: seconds, not minutes) |
| Tombstone GC safety | no resurrection after GC | adversarial test (offline peer replay) |
| Causal stability gate | GC only after all-peer ack | unit test |
| Fail-closed on corruption | hash mismatch rejects payload | unit test |
| Checksum integrity | every chunk/payload checksummed | unit test |
| Carrier agnosticism | zero std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS | compile + audit |
| Allowed crypto only | ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency audit |
| Property tests | 100,000+ iterations | proptest/bolero |
| Stable property detection | termination/deadlock detected | property test |

---

### 10. Remaining Uncertainties

1. **Logical FIFO guarantee from ids-carrier:** Chandy–Lamport assumes FIFO channels; ids-carrier must provide per-Endpoint FIFO ordering of markers relative to app messages, or ids-snapshot must add per-channel sequencing. Carrier contract to be confirmed.
2. **Snapshot termination with offline members:** a node that never records / never receives markers blocks termination and GC; needs membership management (deliberate eviction) or a grace/quarantine window — trade-off between liveness and metadata bloat.
3. **Serialization format:** CBOR (deterministic, self-describing) vs bincode vs versioned custom format; Firecracker pattern (magic + MAJOR.MINOR.PATCH, independent of binary version) favored; crate decision pending.
4. **Incremental vs full checkpoint trade-off:** differential checkpoints reduce size but complicate restore; optimal tiering (local differential + durable base) unmeasured.
5. **Vector clock size:** O(n) per state/marker grows with node count; bounded-clock or dotted-variant may be needed for large deployments.
6. **Authentication scope:** unauthenticated state capture is a trusted-domain optimization; Byzantine safety requires signed snapshot commitments (ed25519) — deferred to ids-security phase.
7. **Formal verification scope:** TLA+/PlusCal spec for consistent-cut + Chandy–Lamport convergence is additional effort beyond core classes.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-snapshot` | IDS rule: separate crates instead of more folders in existing ones; clean dependency boundary; integrates ids-carrier/ids-recovery/ids-replication |
| Chandy–Lamport marker-based coordinated snapshot | Produces consistent cuts; decentralized (any node initiates); no global coordinator; tolerates marker loss via retransmit + idempotence |
| Marker messages over ids-carrier::Endpoint (opaque Vec<u8>) | Carrier-agnostic; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |
| Vector clocks for consistent-cut validation | Causality tracking without a global clock; Mattern/Fidge consistency test |
| Checkpoint + delta journal persistence | Bounded recovery time; avoids full-history replay |
| Tombstone/journal compaction only at causal stability (all-peer ack) | Prevents zombie resurrection; offline members block → needs quarantine/membership |
| Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS crypto policy |
| ids-000-aa sealed Snapshot trait implemented downstream | Single control point; zero downstream edits on contract change |
| Stable property detection for termination/quiescence | Stable predicates detectable via consistent snapshot (Chandy & Lamport 1985) |

---

### 12. Sources

- Chandy & Lamport, "Distributed Snapshots: Determining Global States of Distributed Systems" (1985): https://lamport.azurewebsites.net/pubs/chandy.pdf ; https://cs.cmu.edu/~gibbons/15-712/papers/chandy85.pdf
- Wikipedia, Chandy–Lamport algorithm: https://en.wikipedia.org/wiki/Chandy%E2%80%93Lamport_algorithm
- MIT 6.852J Distributed Algorithms, stable property detection / consistent global snapshots: https://ocw.mit.edu/courses/6-852j-distributed-algorithms-fall-2009/
- Princeton COS418 distributed snapshots (Chandy–Lamport, consistent cuts, vector clocks): https://cs.princeton.edu/courses/archive/fall26/cos418/docs/precept_04_distributed_snapshots.pdf ; https://cs.princeton.edu/courses/archive/spring25/cos418/docs/L7-snapshots.pdf ; https://cs.princeton.edu/courses/archive/fall18/cos418/docs/L4-vc.pdf ; https://cs.princeton.edu/courses/archive/fall17/cos418/docs/L5-vc.pdf
- UIUC ECE428/CS425 consistent cuts and snapshots: https://courses.grainger.illinois.edu/ece428/sp2020/assets/slides/lect6.pdf ; https://courses.grainger.illinois.edu/cs425/fa2020/L14.FA20.pdf ; https://courses.grainger.illinois.edu/ECE428/sp2018/snapshots.pdf
- Cornell CS6410 Lamport / distributed snapshots: https://cs.cornell.edu/courses/cs6410/2013fa/slides/18-lamport.pdf
- KAUST CS240 snapshot lecture: https://sands.kaust.edu.sa/classes/CS240/F21/docs/L7-snapshot.pdf
- UMass 590CC distributed snapshots chapter: https://people.cs.umass.edu/~arun/590CC/lectures/Snapshots.pdf
- Waterloo CS755 distributed snapshots: https://cs.uwaterloo.ca/~mkarsten/cs755-F14/presentations/DistributedSnapshots.pdf
- Princeton CSE138 snapshot notes: https://github.com/zhudotexe/cse138-notes/blob/main/snapshot.rst
- Chirag Shetty, Chandy-Lamport snapshots: https://chiragshetty.com/distributed_systems/chandy_lamport.html
- Decomposition.al example run of Chandy–Lamport: https://decomposition.al/blog/2019/04/26/an-example-run-of-the-chandy-lamport-snapshot-algorithm
- Specification and Verification of Dynamic Properties (stable properties, snapshots): https://sciencedirect.com/science/article/pii/S0743731585710982
- Global predicate detection (stable vs unstable): https://doi.org/10.1017/cbo9780511805318.012
- UT Austin distributed properties chapter (stable predicates): https://users.ece.utexas.edu/~garg/dist/jbkv2/chapter10-properties.pdf
- Detection of weak unstable predicates: https://doi.org/10.1109/71.277788
- An optimal algorithm for distributed snapshots with causal message ordering: https://sciencedirect.com/science/article/pii/0020019094000557
- Apache Flink Fault Tolerance via State Snapshots: https://nightlies.apache.org/flink/flink-docs-release-1.11/learn-flink/fault_tolerance.html
- Apache Flink Savepoints: https://flink.apache.org/docs/latest/docs/ops/state/savepoints
- Kafka Streams architecture / exactly-once state stores & changelog: https://docs.confluent.io/platform/current/streams/architecture.html ; https://docs.confluent.io/platform/current/streams/upgrade-guide.html
- Event sourcing snapshots for aggregate rehydration: https://codeopinion.com/snapshots-in-event-sourcing-for-rehydrating-aggregates
- Checkpoint-rollback recovery, orphan messages, domino effect: https://arxiv.org/pdf/2103.15285v1
- CheckMate: Evaluating Checkpointing Protocols for Streaming Dataflows (coordinated vs uncoordinated): https://ar5iv.labs.arxiv.org/html/2403.13629
- Firecracker snapshot versioning (magic_id + MAJOR.MINOR.PATCH, serde/bincode): https://github.com/firecracker-microvm/firecracker/blob/7c0dc1e1d5b86c496e1c4b69ebfab156c9736a13/docs/snapshotting/versioning.md
- Vercode: versionable serialization of Rust data structures: https://github.com/microsoft/vercode
- serde-evolve / serde_versioned / serde-flow (Rust schema versioning): https://github.com/danieleades/serde-evolve ; https://github.com/taka-k0bayashi/serde_versioned ; https://github.com/versolid/serde-flow
- reconcile crate snapshot header (magic + version + bincode): https://docs.rs/reconcile/0.5.0/src/reconcile/snapshot.rs.html
- RocksDB checkpoint (rust-rocksdb): https://docs.rs/rust-rocksdb/latest/rust_rocksdb/checkpoint/struct.Checkpoint.html
- sled checkpoints (non-blocking fuzzy checkpoints): https://dbdb.io/db/sled
- redb savepoints: https://redb.org/post/2023/06/16/1-0-stable-release
- Akka persistence snapshots: https://github.com/akka/akka-core/tree/main/akka-docs/src/main/paradox/typed/persistence-snapshot.md
- ARIES / WAL crash recovery (CMU 15-445): https://15445.courses.cs.cmu.edu/fall2024/notes/21-recovery.pdf
- PostgreSQL WAL / PITR: https://postgresql.org/docs/current/wal-intro.html ; https://postgresql.org/docs/current/continuous-archiving.htm
- MIT 6.828 crash logging / write-ahead log: https://pdos.csail.mit.edu/6.828/2026/lec/l-crash.txt
- Snapshot: Fast Userspace Crash Consistency (undo log, msync): https://arxiv.org/html/2310.16300v1
- Termination detection (Dijkstra-Scholten, Safra, token ring): https://arxiv.org/html/2602.00272 ; https://cs.rochester.edu/~sree/courses/csc-258/spring-2018/slides/22-td.pdf ; https://blog.lbenicio.dev/post/2025/08/30/implementing-the-dijkstrascholten-algorithm-for-termination-detection-in-distributed-systems
- Efficient Algorithms for Distributed Snapshots and GVT (Mattern): https://sciencedirect.com/science/article/pii/S0743731583710750
- TierCheck: Tiered Checkpointing for LLM Training (differential checkpoints): https://arxiv.org/html/2605.17821
- RocksDB/sled embedded KV persistence references (IDS storage backend candidates)

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 026.
