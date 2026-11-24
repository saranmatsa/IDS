## Phase 062 — Create the ids-replication-stream crate: Ordered/versioned change propagation streams.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 925-939)

```
62. Research only do not read plan.md **[Phase 062 — Create the ids-replication-stream crate: Ordered/versioned change propagation streams.]**

Crate: ids-replication-stream
Purpose: Ordered/versioned change propagation streams.
Contents: Stream creation, stream versioning, resumable delivery, acknowledgement tracking, lag observability, stream interruption handling, stream recovery.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 062 — Create the ids-replication-stream crate: Ordered/versioned change propagation streams.]**

Create plan-062.md in C:/idfs/plans/ for the ids-replication-stream crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 062 creates `ids-replication-stream`, a separate IDS crate (not a folder inside an existing crate, per the project rule of separate crates for new capabilities) that owns ordered, versioned change-propagation streams. It is responsible for stream creation, stream versioning, resumable delivery, acknowledgement tracking, lag observability, stream interruption handling, and stream recovery — all carrier-agnostic.

`ids-replication-stream` does not open sockets or drive any transport directly. It sequences changes into per-stream versioned logs, tracks per-consumer delivery positions (watermarks + resume tokens), manages acknowledgement state (in-flight / acked / nacked), detects and repairs gaps on interruption, observes lag per target, and recovers streams from checkpoints. Every byte that crosses a node boundary is framed as an opaque `Vec<u8>` datagram to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes); the crate itself never addresses IP/TCP/UDP/QUIC/HTTP/DNS or any carrier by name.

It builds on `ids-000-aa` canonical types already present for replication: `StreamId` ([u8; 32]), `CheckpointId` ([u8; 32]), `LagInfo`, `StreamState` (Pending/Active/Paused/Stopped/Failed/Completed), `ReplicationStream`, `ReplicationCheckpoint`, `RetryPolicy`, `StreamStateTracker`, and the sealed `Replication` trait (`start_stream`, `stop_stream`, `create_checkpoint`, `restore_checkpoint`, `schedule_retry`, `get_lag`, `stats`). `ids-replication-stream` consumes these types and contracts — it does not redefine them.

A skeleton `ids-replication-stream` crate does NOT yet exist at `C:/idfs/ids-replication-stream/`. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Kafka partitioned log + consumer-controlled offset + acknowledgement semantics (exactly-once via producer ID + sequence number), PostgreSQL logical replication slots (resumable LSN position, checkpoint-confirmed flush LSN, crash-safe persistence), MongoDB change streams (resume token, total ordering via global logical clock, driver auto-reconnect), Debezium exactly-once delivery (at-least-once baseline, deduplication requirement on failure/restart), delta-CRDT anti-entropy convergent replication (deltas tolerate duplicates/reordering; lost deltas retransmitted), vector clocks and dotted version vectors for partial/causal ordering, hybrid logical clocks for versioned stream ordering, stream delivery semantics & resumability (sequence numbers, acks, replay windows, snapshot resync beyond window), replication lag observability (consumer lag max, bytes-behind, alert thresholds), Flink coordinated checkpointing (exactly-once state restore from latest checkpoint + input offset), Raft log replication (leader→follower append, committed index), SCTP/RFC 9260 sequenced delivery within streams (stream sequence numbers, ordered vs unordered delivery modes), gRPC server-streaming resumption (continuation tokens, token journal, replay from cursor), XEP-0198 stream management (ack elements c/b, resume with previd + last-received seq), NATS JetStream delivery & acknowledgment (ack floor, in-flight, redeliver after Ack Wait), Durable Streams protocol (append-only offset-ordered byte streams, catch-up reads, live tailing, explicit EOF), Azure Event Sourcing (append-only immutable event stream, entity event version, replay/rehydration), Cassandra consistency levels (quorum R+W>N overlap guarantee).

---

### 2. Architecture Overview

`ids-replication-stream` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a stream-management layer: it sequences outgoing changes into versioned per-stream logs, tracks each consumer's delivery position, manages acks/nacks and in-flight state, repairs gaps after interruptions, observes lag, and recovers from checkpoints. It never sends bytes itself; it produces framed `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint` targets that the caller (or `ids-carrier`) transmits.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    ids-* LAYER (stream users: store, crdt, sync, …)          │
│  submit_change(dataset, payload, version) → StreamManager                    │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 062: ids-replication-stream (THIS CRATE)                 │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ stream           │ │ stream           │ │ acknowledgement  │ │ lag     │ │
│  │ creation         │ │ versioning       │ │ tracking (in-    │ │ observ- │ │
│  │ (StreamId,       │ │ (StreamVersion,  │ │ flight/acked/    │ │ ability │ │
│  │  targets, source)│ │  resume token)   │ │  nacked)         │ │         │ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ resumable        │ │ interruption     │ │ stream recovery  │ │ crypto  │ │
│  │ delivery         │ │ handling (gap    │ │ (checkpoint      │ │ surface │ │
│  │ (watermark +     │ │  detect/repair)  │ │  restore +       │ │ (type-   │ │
│  │  resume token)   │ │                  ││  replay)         │ │ level)   │ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
└─────────────────────────────────────────┬──────────────────────────────────┘
                                          │ opaque Vec<u8> datagrams
                                          ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a versioned change:** caller submits `Change{payload, dataset_id}` → `StreamManager` assigns monotonically increasing `StreamVersion` (u64) + `StreamId`, appends to the append-only versioned log for that stream, frames `StreamFrame{stream_id, version, payload, checksum}` as opaque `Vec<u8>` → dispatched to targets via `ids-carrier::Endpoint` → each target's consumer tracks `last_delivered_version` (watermark) + `resume_token` → target acks version → ack recorded in acknowledgement tracker → in-flight entry cleared → lag recomputed per target.

**Data flow of resumable delivery after interruption:** consumer reconnects presenting `resume_token` (= last acked version + stream id) → server replays from `resume_token + 1` up to stream head, or instructs full snapshot/resync if beyond retention window → gap-free ordered redelivery.

**Stream states (from ids-000-aa `StreamState`):** Pending → Active → Paused → (Stopped | Failed | Completed). `Paused` supports interruption handling; `Failed` triggers recovery path; `Completed` is terminal.

**Crypto used:** ed25519 for signing stream checkpoints and resume tokens; sha256/blake3 for frame checksums, stream-version hashing, checkpoint chaining; xchacha20poly1305/chacha20poly1305 to encrypt stream state payloads at rest when confidentiality required. No other crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Ordered change-propagation stream core

An ordered stream is an append-only, versioned sequence of changes consumed positionally. Every change receives a monotonically increasing `StreamVersion` (u64) at append time; consumers track `last_delivered_version` and resume from `last_delivered_version + 1`. Ordering is per-stream (total order within a stream), matching Kafka's per-partition commit offset, PostgreSQL LSN, and EventStoreDB commit position. Sources: Kafka design (partitioned log, consumer-controlled offset), PostgreSQL logical decoding (LSN position, checkpoint), Azure Event Sourcing (append-only event stream, per-entity version).

#### 3.2 Stream creation

`start_stream(dataset_id, source, targets)` allocates a `StreamId`, initializes an empty versioned log (head version = 0), creates per-target consumer state (watermark = 0, resume token = genesis), sets `StreamState::Pending`, then `activate()` moves to `Active`. Returns `ReplicationStream` (from ids-000-aa). Stream creation is idempotent per `StreamId`: re-creating an existing `StreamId` returns `StreamAlreadyExists`. The crate never redefines `StreamId`/`ReplicationStream` — both live in `ids-000-aa/src/contracts/replication.rs`.

#### 3.3 Stream versioning

Each stream carries a `StreamVersion` (u64) that advances by exactly 1 per appended change. Versions are the resume/cursor unit: a consumer presents `last_acked_version` to resume. Stream versions are independent of `ids-000-aa` `ProtocolVersion`/`EdgeCaseBits` (wire-contract gating) — stream versions are intra-stream logical positions, not protocol versions. Checkpointing snapshots the head version at a point in time (`ReplicationCheckpoint.position` of type `OrderingPos` from ids-000-aa). Version skew between replicas is detected by comparing head versions; divergence triggers gap repair or resync.

#### 3.4 Resumable delivery

Resumability requires (a) a durable resume token (stream id + last acked version + optional epoch), (b) server-side retention of the versioned log long enough to replay from the token, and (c) a policy for when the token is beyond the retention window (instruct full snapshot/resync). Sources: MongoDB change streams (resume token = `_id`; auto-reconnect with cached token; persist token across app restarts), PostgreSQL logical slots (persist slot independently of connection; resume from LSN; client records last LSN seen), XEP-0198 stream management (resume with `previd` + last received seq `b`), gRPC streaming lessons (continuation token; server journals every token; replay from `resume_from_token` with original indices), Durable Streams protocol (catch-up reads from offset; live tailing).

#### 3.5 Acknowledgement tracking

Each delivered change enters an in-flight state keyed by `(stream_id, version, target)` until the target acks. On ack, the watermark advances and the entry is cleared. Nack or timeout → redeliver (at-least-once baseline). Idempotent delivery is achieved by consumer-side dedup on `(stream_id, version)` — a duplicate delivery is a no-op if `version <= last_acked_version`. Sources: Kafka idempotent producer (PID + sequence; broker rejects ≤ highwater mark, accepts exactly last+1), NATS JetStream (ack floor; in-flight; redeliver after Ack Wait; ack exactly once), XEP-0198 (ack element `b` removes all stanzas ≤ value; unacked queue drained), Cloud Pub/Sub exactly-once (ack ID versioning; only latest ack ID valid), Debezium (at-least-once → duplicates possible on restart; dedup required).

#### 3.6 Lag observability

Per-target lag = `head_version - last_acked_version` (version lag) plus `bytes_behind` (unacked bytes) and `time_behind` (ms since last ack). `LagInfo` (from ids-000-aa) carries `target`, `lag_ms`, `bytes_behind`, `last_heartbeat_ms`, `recovered`. `is_stale(threshold_ms)` flags `lag_ms > threshold`. Sources: Kafka monitoring (`records-lag-max`, bytes-behind, alert on sustained growth not non-zero), PostgreSQL replication slot lag (`pg_wal_lsn_diff(current, restart_lsn)`), Cassandra under-replicated partitions.

#### 3.7 Stream interruption handling

Interruptions (carrier pause, target unreachable, consumer crash) move a stream to `Paused` or `Failed`. Gap detection: on resume, if `resume_token + 1 < head_version`, the missing range `[resume_token+1, head_version]` is repaired by replay. If the gap exceeds the retention/replay window, the server instructs a full snapshot/resync. Buffer limits are in bytes and version span (not item count) to bound memory; an attacker sending a far-future version must not exhaust memory. Sources: systemoverflow delivery semantics (reorder buffer, gap repair when age/bytes exceed policy; replay window 5–15 min then resync), gRPC streaming (continuation token resume), XEP-0198 (session discarded if not resumed in time; unacked stanzas handled as offline).

#### 3.8 Stream recovery

Recovery restores a stream from a `ReplicationCheckpoint` (from ids-000-aa: checkpoint_id, dataset_id, position/OrderingPos, node_id, created_at_ms, size_bytes, verified, restored). `restore_checkpoint` verifies the checkpoint (sha256/blake3 chain + ed25519 signature), resets the stream head to `position`, replays pending changes after the checkpoint, and returns `StreamState::Active`. Recovery is bounded: replay only changes after the checkpoint position; idempotent dedup prevents double-apply. Sources: Flink coordinated checkpointing (rollback to latest checkpoint + reset inputs to offset; exactly-once), PostgreSQL logical slot crash recovery (slot returns to earlier LSN on crash; clients idempotent), Kafka exactly-once (checkpoint + offset restore).

#### 3.9 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

`ids-replication-stream` uses: ed25519 to sign stream checkpoints and resume tokens (proof of stream position integrity); sha256 for frame checksums and checkpoint chain links; blake3 for fast content hashing of stream frames and causal-context hashing (parallel, SIMD); xchacha20poly1305 to encrypt stream state at rest (24-byte nonce, random nonces safe); chacha20poly1305 to encrypt session/payload datagrams (12-byte nonce, counter-based bounded use). Crypto operations live in `ids-crypto`; this crate declares only type-level surface and references primitive IDs via `ids-000-aa`. Sources: RFC 8032 Ed25519; RFC 7539 ChaCha20-Poly1305; libsodium XChaCha20-Poly1305; NIST FIPS 180-4 SHA-256; BLAKE3 spec.

#### 3.10 Causality and version vectors (optional causal stream mode)

For causally-ordered streams, changes carry a dotted version vector (DVV) causal dependency; a consumer gates delivery until its causal past is satisfied. This is weaker than linearizability but available under partition. Dots `(node_id, counter)` enable compact causality comparison. Sources: Princeton vector clocks (partial ordering), ACM DL dotted version vectors (10.1145/2332432.2332497), Bernstein write-fast-read-in-past causal consistency, CRDT delta-state (arXiv 1603.01529) — deltas tolerate duplicates/reordering and converge.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|-------------|-------------------------|
| Out-of-order / duplicated change delivery | Stream ordering correctness | Per-stream `StreamVersion` + consumer watermark; dedup on `version <= last_acked_version`; gap buffer with byte/span limits |
| Lost change on carrier/target failure | Delivery durability | In-flight tracking + ack/nack; redeliver on timeout (at-least-once); idempotent dedup upgrades to exactly-once semantics |
| Resume token beyond retention window | Resumability | Server instructs full snapshot/resync; documented boundary |
| Memory exhaustion from far-future versions | Availability | Buffer limits in bytes + version span; reject out-of-window versions |
| Stale/lagging target | Lag SLO | `LagInfo.is_stale(threshold)`; `LagTooHigh` error; backpressure signal to caller |
| Stream interruption losing in-flight changes | Delivery guarantee | In-flight queue persisted until ack; `Paused`/`Failed` states; replay from `resume_token + 1` on recovery |
| Checkpoint tampering / replay attack | Recovery integrity | sha256/blake3 hash chain + ed25519-signed checkpoints; verify before restore |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent version dedup tolerates duplicates; sequence gaps detected |
| Disallowed crypto algorithm | Crypto policy | Allowlist unit test; only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto |
| Carrier-agnostic rule violation (std::net/tokio::net/IP/etc.) | Architecture | `Endpoint` sole address type; dependency audit bans networking crates |

The carrier is never trusted: stream positions and checkpoints are signed/hashed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-replication-stream |
|-------|------|-----------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `StreamId`, `CheckpointId`, `LagInfo`, `StreamState`, `ReplicationStream`, `ReplicationCheckpoint`, `RetryPolicy`, `StreamStateTracker`, sealed `Replication` trait, `OrderingPos`, `Endpoint`, `CarrierType`, `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All stream traffic flows to/from `Endpoint`; crate returns frames/decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing checkpoints/resume tokens; hashing frames/checkpoints; encrypting stream state at rest |
| `ids-store` | Primary record store (stream consumer) | Subscribes to replication streams; applies versioned changes in order |
| `ids-replication` | Replica synchronization orchestrator | Owns `Replication` sealed trait impl; drives `ids-replication-stream` for per-dataset streams |
| `ids-sync-checkpoint` (Phase 063) | Verified sync points for resumable replication | Checkpoints produced here feed sync-checkpoint resume; shared `CheckpointId`/`ReplicationCheckpoint` types |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Stream writes tagged with profile; profile quorum/durability evaluated before ack |
| `ids-time` | Monotonic IDS standard time | Stream timestamps, heartbeat ms, lag_ms keyed to `IdsTime` ticks (never wall-clock for ordering) |

Integration points: `ids-replication-stream` exposes `StreamManager`, `VersionedLog`, `AckTracker`, `LagMonitor`, `RecoveryMgr`; it produces framed `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint` targets. It does not reach into crate internals or touch any network stack.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-replication-stream/` (a separate crate, not a folder inside an existing crate):

```
ids-replication-stream/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-time, thiserror, serde, blake3
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # Stream defaults: max replay window, ack timeout, buffer byte/span caps
├── error.rs                            # StreamError (thiserror): StreamNotFound, VersionGap, ResumeBeyondWindow, LagTooHigh, etc.
├── types.rs                            # StreamFrame, ResumeToken, StreamCursor, AckRecord, InFlightEntry (re-exports ids-000-aa ids)
├── stream_manager.rs                   # Stream lifecycle: create/activate/pause/resume/stop/fail/complete
├── versioned_log.rs                    # Append-only versioned log; head version; append/read by version
├── stream_versioning.rs                # StreamVersion (u64) allocation, version skew detection, head comparison
├── resumable_delivery.rs               # Resume token issuance/validation; replay from token; snapshot/resync policy
├── acknowledgement.rs                  # AckTracker: in-flight/acked/nacked state; watermark advance; dedup
├── ack_record.rs                       # per-(stream, version, target) ack state machine
├── in_flight.rs                        # In-flight queue with timeout; redelivery scheduler
├── lag_monitor.rs                      # LagInfo computation per target; is_stale; LagTooHigh emission
├── lag_types.rs                        # LagSample, LagThreshold, LagDirection
├── interruption.rs                     # Gap detection [resume_token+1, head]; gap repair; Paused/Failed handling
├── gap_repair.rs                       # Missing-range replay; replay-window bounds; resync instruction
├── recovery.rs                         # Restore from ReplicationCheckpoint; verify; replay after checkpoint
├── checkpoint_mgr.rs                   # Checkpoint creation/verification at head version; hash chain
├── resume_token.rs                     # ResumeToken {stream_id, last_acked_version, epoch} encode/decode/sign
├── frame.rs                            # StreamFrame {stream_id, version, payload, checksum} frame/deframe
├── frame_codec.rs                      # serde/bincode codec for StreamFrame; version-tagged
├── checksum.rs                         # sha256/blake3 frame checksum helpers (type surface)
├── crypto_surface.rs                   # ed25519 sign/verify for checkpoints/tokens; xchacha/chacha encrypt helpers
├── causal.rs                           # Optional dotted version vector for causal stream mode
├── dvv.rs                              # Dotted version vector: dot, causal precedence, merge
├── contracts/                          # sealed trait contracts (~12 files)
│   ├── mod.rs
│   ├── stream_contract.rs              # trait StreamMgr (create/activate/pause/…/stats)
│   ├── versioning_contract.rs          # trait StreamVersioning
│   ├── resumable_contract.rs           # trait ResumableDelivery (issue/validate resume token, replay)
│   ├── ack_contract.rs                 # trait AckTracker (ack/nack/in-flight/dedup)
│   ├── lag_contract.rs                 # trait LagMonitor (lag per target, is_stale)
│   ├── interruption_contract.rs        # trait InterruptionHandler (gap detect/repair)
│   ├── recovery_contract.rs            # trait StreamRecovery (restore checkpoint)
│   ├── checkpoint_contract.rs          # trait CheckpointMgr (create/verify)
│   ├── frame_contract.rs               # trait StreamFrameCodec (frame/deframe)
│   ├── crypto_contract.rs              # trait StreamCrypto (sign/verify/encrypt type surface)
│   ├── causal_contract.rs              # trait CausalStream (DVV gate)
│   ├── carrier_contract.rs             # trait StreamCarrierAdapter (opaque Vec<u8> to Endpoint)
├── invariants/                         # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── version_monotonic.rs            # versions strictly increase by 1
│   ├── watermark_bounded.rs            # watermark ≤ head
│   ├── ack_dedup_sound.rs              # duplicate ack is no-op
│   ├── gap_repair_complete.rs          # repair covers [token+1, head]
│   ├── carrier_agnostic.rs             # no std::net/tokio::net/IP/etc.
│   ├── crypto_allowlist.rs             # only allowed algorithms
│   ├── resume_token_sound.rs           # token validates against log
│   ├── checkpoint_verified.rs          # restore only verified checkpoints
│   ├── lag_threshold.rs                # lag reported consistently
│   ├── stream_state_transition.rs      # legal StreamState transitions
├── audit/                              # signed, hash-chained stream audit log (~6 files)
│   ├── audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs, mod.rs
├── tests/                              # property/verification tests (~30 files)
├── benches/                            # (~10 files)
├── docs/                               # (~14 files)
└── spec/                               # (~4 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, `spec/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

1. **Stream creation** — `start_stream(dataset_id, source, targets)` → allocate `StreamId`, empty versioned log (head=0), per-target consumer state (watermark=0, resume token genesis), `StreamState::Pending` → `activate()` → `Active`. Idempotent per `StreamId`; `StreamAlreadyExists` on duplicate.

2. **Versioned log append** — `append(change)` assigns `head_version + 1`, appends `StreamFrame{stream_id, version, payload, checksum}`, advances head. `read(version)` returns frame at position; `read_range(from, to)` replays `[from, to]`.

3. **Resume token** — `ResumeToken{stream_id, last_acked_version, epoch}`; issued on ack watermark advance; validated against log on resume; replay starts at `last_acked_version + 1`; beyond retention → full snapshot/resync.

4. **Acknowledgement tracking** — `AckRecord{stream_id, version, target, state: InFlight|Acked|Nacked, delivered_at_ms, acked_at_ms}`. Ack advances per-target watermark; clears in-flight. Nack/timeout → redeliver. Dedup: `version <= last_acked_version` ⇒ no-op.

5. **Lag computation** — `lag_versions = head_version - last_acked_version`; `bytes_behind` = unacked payload bytes; `time_behind` = now − last_acked_at_ms; `LagInfo.is_stale(threshold)` ⇒ `lag_ms > threshold`.

6. **Gap detection & repair** — on resume, missing = `[resume_token+1, head_version]`; if span ≤ replay window → replay gap; else → resync instruction. Buffer caps in bytes + version span bound memory.

7. **Stream recovery** — `restore_checkpoint(checkpoint_id)` verifies (hash chain + ed25519), sets head to `position`, replays changes after `position`, returns `Active`. Idempotent dedup prevents double-apply.

8. **Stream state machine** — `Pending → Active → Paused → (Stopped | Failed | Completed)`; `is_terminal()` for Stopped/Failed/Completed; `is_active()` for Active; interruption moves Active→Paused/Failed; recovery moves Failed→Active.

9. **Frame codec** — `StreamFrame::encode()`/`decode()` (stream_id u8[32], version u64, payload_len u32, payload Vec<u8>, checksum [u8;32]); version-tagged for forward compatibility.

10. **Dotted version vector (causal mode)** — `DVV{dot: (node_id, counter), vv: Vec<(node_id, counter)>}`; `a < b` iff `dot_a.counter ≤ vv_b[node_a]`; concurrent iff neither precedes; read gate delivers only when `causal_past ⊆ applied`.

---

### 8. Dependencies

`ids-replication-stream/Cargo.toml`:
- `ids-000-aa` — canonical types (`StreamId`, `CheckpointId`, `LagInfo`, `StreamState`, `ReplicationStream`, `ReplicationCheckpoint`, `RetryPolicy`, `StreamStateTracker`, `OrderingPos`, `Endpoint`, `CarrierType`, `Error`) and sealed `Replication` trait.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — frame transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-time` — monotonic `IdsTime` ticks for timestamps, heartbeats, lag_ms.
- `thiserror` — error enum derive.
- `serde` + `serde_bytes` — frame/envelope serialization.
- `blake3` — content hashing (type-level surface; ops in ids-crypto).

No `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS implementations in or under `ids-replication-stream`. No crypto beyond the allowed list; all crypto ops via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-62.1 | Stream creation: `start_stream` allocates unique `StreamId`, head version = 0, state Pending→Active | `StreamId` unique; `ReplicationStream.state == Active` after activate |
| AC-62.2 | Ordered delivery: changes delivered in strictly increasing `StreamVersion` per stream; no gaps in applied sequence | per-stream version sequence contiguous; dedup rejects `version <= last_acked` |
| AC-62.3 | Stream versioning: head advances by exactly 1 per append; version skew between replicas detectable | `head_version == prior + 1`; skew = head_A − head_B reported |
| AC-62.4 | Resumable delivery: resume token (`stream_id`, last acked version) replays from `token+1`; gap-free | replay covers `[token+1, head]`; beyond window → resync instruction |
| AC-62.5 | Acknowledgement tracking: ack clears in-flight, advances watermark; nack/timeout redelivers (at-least-once) | in-flight count → 0 after ack; redelivery on nack/timeout |
| AC-62.6 | Exactly-once via dedup: duplicate `version <= last_acked_version` is a no-op | duplicate deliveries produce no state change |
| AC-62.7 | Lag observability: per-target `LagInfo` (lag_ms, bytes_behind, last_heartbeat_ms); `is_stale(threshold)` flags lag | `LagInfo` matches `head − last_acked`; stale flag accurate |
| AC-62.8 | Stream interruption: Active→Paused/Failed on interruption; gap detect/repair on resume | Paused/Failed states; missing range replayed or resync instructed |
| AC-62.9 | Stream recovery: restore verified checkpoint, reset head to position, replay after, return Active | checkpoint verified (hash chain + ed25519); head == position; replay idempotent |
| AC-62.10 | Stream states: `Pending→Active→Paused→(Stopped|Failed|Completed)`; terminal states flagged | `is_terminal()`/`is_active()` correct per StreamState |
| AC-62.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B); zero refs to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | dependency audit test (`cargo tree`) passes; source grep zero matches |
| AC-62.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | allowlist unit test passes |
| AC-62.13 | Every checkpoint signed/hashed: sha256/blake3 chain + ed25519 signature; verify before restore | verify() returns false on tampered checkpoint |
| AC-62.14 | Crate compiles cleanly | `cargo check -p ids-replication-stream` exits 0 |
| AC-62.15 | Conformance tests pass | `cargo test -p ids-replication-stream` passes all stream/version/ack/lag/recovery tests |

---

### 10. Remaining Uncertainties

- **Replay window size:** bytes + version span bound for the gap-repair buffer; workload-dependent, unquantified.
- **Retention window vs resume token age:** how long the versioned log is retained before instructing full resync; deployment-tuned.
- **Ack timeout value:** in-flight redelivery timeout trades redelivery storm vs latency; to be validated at integration with `ids-carrier`/`ids-replication`.
- **Causal stream mode (DVV):** whether to ship the optional dotted-version-vector causal gating in Phase 062 or defer to a later phase; design decision pending.
- **Interaction with `ids-sync-checkpoint` (Phase 063):** shared `CheckpointId`/`ReplicationCheckpoint` types already exist in ids-000-aa; exact handshake (checkpoint → sync point) to be validated at integration.
- **Interaction with `ids-consistency-profile` (Phase 060):** stream writes tagged with profile; profile quorum/durability evaluated before ack; exact admission handshake pending.
- **Stream head persistence:** whether head version and consumer watermarks are checkpointed durably or held in-memory with periodic snapshots; durability/performance tradeoff unquantified.
- **Multi-target fan-out:** per-target watermark state scales with target count; bounded-state strategy for large target sets undecided.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-replication-stream` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear stream-management boundary |
| Per-stream `StreamVersion` (u64) as resume/cursor unit | Matches Kafka commit offset, PostgreSQL LSN, EventStoreDB commit position; simple, gap-free resume |
| At-least-once baseline + consumer-side dedup → exactly-once | Kafka idempotent producer / Pub/Sub ack-ID versioning pattern; dedup on `version <= last_acked` |
| Resume token = `stream_id + last_acked_version + epoch` | Mirrors MongoDB change-stream resume token and XEP-0198 resume (`previd` + `b`) |
| Lag = `head − last_acked` versions + bytes_behind + time_behind | Matches Kafka `records-lag-max` and PostgreSQL slot lag observability |
| Gap repair on replay window; resync beyond window | systemoverflow delivery semantics: replay window then full resync; bounds memory |
| Checkpoints: sha256/blake3 hash chain + ed25519 signature, verify before restore | Tamper-evident, offline-verifiable recovery |
| All traffic via `ids-carrier::Endpoint`; crate never sends directly | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only; ops in `ids-crypto`, type surface here | Complies with allowed-crypto rule |
| `ids-time` monotonic ticks for timestamps/lag; never wall-clock for ordering | Correct under arbitrary delays; no clock manipulation |
| Stream states reuse `ids-000-aa` `StreamState` | Single canonical definition; no redefinition |

---

### 12. Sources

1. Kafka design — partitioned log, consumer-controlled offset, acknowledgement semantics: https://kafka.apache.org/44/design/design
2. Kafka idempotent producer — PID + sequence, exactly-once, dedup ≤ highwater: https://cwiki.apache.org/confluence/display/KAFKA/Idempotent+Producer ; https://developer.confluent.io/patterns/event-processing/idempotent-writer
3. PostgreSQL logical decoding / replication slots — resumable LSN, checkpoint-confirmed flush LSN, crash-safe persistence: https://postgresql.org/docs/19/logicaldecoding-explanation.html ; https://postgresql.org/docs/current/logicaldecoding.html
4. MongoDB change streams — resume token, total ordering via global logical clock, driver auto-reconnect: https://resources.mongodb.com/mongodb-3-6/an-introduction-to-change-streams ; https://mongodb.com/docs/v8.0/changestreams
5. Debezium — towards exactly-once delivery (at-least-once baseline, dedup on failure/restart): https://debezium.io/blog/2023/06/22/towards-exactly-once-delivery
6. Delta CRDT / anti-entropy — deltas tolerate duplicates/reordering, convergent: https://arxiv.org/html/1603.01529
7. Vector clocks — partial ordering, vector clock update rules: https://cs.princeton.edu/courses/archive/fall22/cos418/docs/L6-vc.pdf
8. Dotted version vectors — efficient causality tracking: https://dl.acm.org/doi/10.1145/2332432.2332497
9. Write fast, read in the past — causal consistency: https://dl.acm.org/doi/10.1145/2814576.2814733 ; https://microsoft.com/en-us/research/wp-content/uploads/2016/02/sigtt611-bernstein.pdf
10. Stream delivery semantics & resumability — sequence numbers, acks, replay windows, snapshot resync: https://systemoverflow.com/learn/networking-protocols/websockets/delivery-semantics-and-resumability-sequence-numbers-acks-and-replay
11. Kafka monitoring — consumer lag max, bytes-behind, alert thresholds: https://coralogix.com/guides/kafka-monitoring
12. Flink checkpointing — exactly-once state restore from latest checkpoint + input offset: https://nightlies.apache.org/flink/flink-docs-release-1.9/internals/stream_checkpointing.html
13. Raft log replication — leader→follower append, committed index: https://web.stanford.edu/~ouster/cgi-bin/papers/raft-atc14
14. SCTP / RFC 9260 — sequenced delivery within streams, ordered vs unordered modes: https://datatracker.ietf.org/doc/html/rfc9260
15. gRPC streaming lessons — continuation tokens, token journal, replay from cursor: https://jonkuhn.com/2022/04/02/gRPC-Streaming-Lessons-Learned.html
16. XEP-0198 stream management — ack elements c/b, resume with previd + last received seq: https://xmpp.org/extensions/attic/xep-0198-0.5.html
17. NATS JetStream — delivery & acknowledgment, ack floor, in-flight, redeliver: https://docs.nats.io/learn/jetstream/delivery-and-acknowledgment
18. Cloud Pub/Sub exactly-once — ack ID versioning, only latest valid: https://cloud.google.com/blog/products/data-analytics/cloud-pub-sub-exactly-once-delivery-feature-is-now-ga
19. Durable Streams protocol — append-only offset-ordered byte streams, catch-up reads, live tailing: https://github.com/durable-streams/durable-streams/blob/main/PROTOCOL.md
20. Azure Event Sourcing — append-only immutable event stream, per-entity version, replay/rehydration: https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing
21. Cassandra consistency levels — quorum R+W>N overlap: https://cassandra.apache.org/doc/stable/cassandra/architecture/dynamo.html
22. RFC 8032 — Ed25519 EdDSA: https://www.rfc-editor.org/rfc/rfc8032
23. RFC 7539 — ChaCha20-Poly1305 AEAD: https://www.rfc-editor.org/rfc/rfc7539
24. Libsodium — XChaCha20-Poly1305 construction: https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
25. NIST FIPS 180-4 — SHA-256: https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
26. BLAKE3 — BLAKE3-team README: https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
27. IDS prompts.txt entry 62 (verbatim): C:/idfs/prompts.txt lines 925-939
28. Existing ids-000-aa replication contract: C:/idfs/ids-000-aa/src/contracts/replication.rs (StreamId, CheckpointId, LagInfo, StreamState, ReplicationStream, ReplicationCheckpoint, RetryPolicy, StreamStateTracker, sealed Replication trait)
29. HX-241 plan format: C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md
30. IDS CRATES.md — crate inventory, ids-carrier Endpoint, ids-crypto allowed set: C:/idfs/CRATES.md

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 062 (ids-replication-stream).
