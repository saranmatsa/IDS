## Phase 012 — Create the ids-write crate (write engine and write-ahead log): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Note on numbering: prompts.txt entry 12 is ids-protocol; entry 13 is ids-write ("Create the ids-write crate: Write lifecycle: received → ordered → executing → committed → synchronized."). Per the user's directive, plan-012.md covers the ids-write crate (the write engine + WAL).

---

### 1. Executive Summary

Phase 012 creates `ids-write`, the IDS write engine and write-ahead log. It owns the durable write path: `received → ordered → executing → committed → synchronized`. Contents per CRATES.md §13 and prompts.txt entry 13: WAL, append-only log, durable write path, write fan-out (up to ~30 databases simultaneously), idempotent writes, operation references, ordering positions, durability conditions, commit acknowledgements, write-order management, concurrent-write resolution.

The crate is carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-write` only consumes hashing/signing type surface from `ids-000-aa`). `ids-000-aa` is the master crate holding every canonical type and sealed trait contract; `ids-write` depends on `ids-000-aa` only (plus `ids-crypto` types, no `ids-crypto` runtime dep since crypto is type-level here — aligned to the rule that `ids-crypto` does not depend on `ids-000-aa` to avoid cycles).

Research basis: 30 parallel searches returned ARIES WAL (Mohan et al., 1992) analysis/redo/undo, log sequence number (LSN) semantics and idempotent replay (`record_lsn > page_lsn`), group commit / batched fsync amortizing flush cost, segment rotation + checkpoint + truncation + garbage collection (Kudu LogGCOp, Prometheus TSDB checkpoint, Grafana Loki truncation), torn-tail recovery (truncate corrupt tail, never guess), CRC/checksummed frames (RocksDB WAL record format: CRC32C + length + type + payload), physical vs logical logging, write concern / durability acknowledgement levels (MongoDB w/j), high-watermark/commit-index separation (Martin Fowler distributed patterns; Raft commit index), idempotency keys for exactly-once semantics (RIFL, Stripe, Kafka dedup windows), multi-writer / fan-out replication (TiCDC, Cassandra CommitLog idempotent mutations, DynamoDB Global Tables operation-ID dedup), backpressure under WAL load (ClickHouse WAL backpressure cgroups, Loki disk-full throttle), low-latency cloud logging (BtrLog quorum replication, LSN windowing), and Rust WAL primitives (wal-db lock-free append, wal-engine fsync policies + group commit, nano-wal lock-free segment rotation).

A skeleton `ids-write` crate does NOT yet exist at `C:/idfs/ids-write/`. This plan specifies creation from scratch; no source modifications are made in this phase.

---

### 2. Architecture Overview

`ids-write` is the durability and ordering layer for all IDS writes. Every write crosses five lifecycle states defined in `ids-000-aa::OpState`: `received → ordered → executing → committed → synchronized → verified → failed`. `ids-write` owns the WAL (durable, append-only, checksummed, segmented), the ordering position allocator (monotonic `OrderingPos`), the op-reference (`OpRef`) binder for idempotency, the durability-condition evaluator, commit-ack tracking, write-order management, concurrent-write resolution, and fan-out to up to ~30 downstream databases/replicas.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CLIENT / ids-client (ids_submit)                 │
│  submit(op with OpRef idempotency key)                              │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
┌──────────────────────────────▼──────────────────────────────────────┐
│                   Phase 012: ids-write (THIS CRATE)                 │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐ │
│  │  receive    │ │  order       │ │  execute        │ │ commit  │ │
│  │  (OpRef     │ │  (OrderingPos│ │  (durable write │ │  ack    │ │
│  │   dedup)    │ │   allocator) │ │   path + WAL)   │ │  tracker│ │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘ │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐ │
│  │  synchronize│ │  concurrent  │ │  fan-out (~30   │ │  WAL    │ │
│  │  (replica   │ │  write       │ │   dbs)          │ │  engine │ │
│  │   apply)    │ │  resolution  │ │                 │ │         │ │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘ │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
┌──────────────────────────────▼──────────────────────────────────────┐
│              REPLICAS / DOWNSTREAM DBs (via ids-carrier)            │
│  ids-replication · ids-sync · ids-read · ids-commit-state · …       │
└─────────────────────────────────────────────────────────────────────┘

CARRIER BOUNDARY (no std::net / tokio::net): ids-carrier::Endpoint only
```

**Design rules (non-negotiable):**
- WAL is append-only, segmented, checksummed (blake3/sha256 per record), with torn-tail recovery on restart.
- Every record gets a monotonically increasing LSN/`OrderingPos`; recovery is idempotent (`record_lsn > applied_lsn` skips already-applied).
- Durability = fsync/equivalent before ack, with group commit (one flush per batch) to amortize cost.
- Idempotency via `OpRef` (9-byte alphanumeric, 36^9 namespace) + dedup window; duplicate OpRef = no-op, return prior result.
- Write fan-out to up to ~30 databases simultaneously; each fan-out target tracks its own commit ack.
- Durability conditions (local / quorum / regional / cross-region) evaluated per `ids-durability-policy`; commit vs synchronization distinguished.
- Concurrent writes to the same key resolved by write-order (last writer wins by ordering position) or domain-scoped conflict rule.
- No `std::net`, `tokio::net`, or any networking stack; `Endpoint` is the sole address type.
- Crypto surface: `Checksum([u8;32])` (blake3/sha256), `Signature` (ed25519) from `ids-000-aa`; no direct `ids-crypto` runtime dependency.

---

### 3. Detailed Research Dimensions

#### 3.1 Write-Ahead Log (WAL) — Append-Only, Segmented, Checksummed
Core principle: describe a change durably *before* applying it (ARIES WAL rule; log first, data second). Each WAL record: fixed header (LSN/u64, record type u8, length u32, flags u8, checksum u32/blake3 or sha256) + variable payload. Records grouped into fixed-size segments; segment rolls at size threshold; new segment file created per segment sequence number. CRC/checksum per record enables corruption detection; a checksum mismatch or truncated tail marks the logical end of the log (torn-tail recovery: truncate at last complete record, never guess). Sources: ARIES paper (Stanford cs345d), RocksDB WAL file format (CRC32C + size + type + payload, 32KB blocks), Prometheus TSDB WAL checkpoint, Kudu WAL segment GC, Grafana Loki WAL truncation/checkpoint, CMU 15-721 physical vs logical logging, BtrLog low-latency cloud logging.

Record types (logical logging, aligned to idempotent ops): `BEGIN_OP`, `WRITE_RECORD` (op payload + OpRef + ordering position), `COMMIT_OP`, `ABORT_OP`, `CHECKPOINT`, `TRUNCATE_MARKER`, `FANOUT_ACK`. Logical (not physical) records keep entries small and support idempotent replay; physical page-level logging is explicitly rejected (IDS has no buffer pool/page concept in this phase).

#### 3.2 Log Sequence Number / Ordering Position
Every WAL record receives a monotonically increasing `OrderingPos` (u64) — the LSN. Assigned at append time, before durability sync. `OrderingPos::next()`/`prev()` with saturating arithmetic (from `ids-000-aa`). LSN makes replay idempotent: on recovery, skip records whose LSN <= already-applied LSN (`record_lsn > applied_lsn`). LSNs coordinate checkpoints, fan-out ack progress, and the oldest retained position for truncation. Sources: ARIES, CMU 15-445 logging notes, wal-db (LSN = byte offset, atomic reservation).

#### 3.3 Durable Write Path & Commit Acknowledgement
Write path: (1) receive op + OpRef; (2) check idempotency (seen OpRef → return cached result, no-op); (3) allocate OrderingPos; (4) append WAL record(s); (5) fsync/flush per durability policy + fsync policy (Always / EveryN / Interval / Never — wal-engine); (6) apply to executing state; (7) write COMMIT_OP to WAL + fsync; (8) send commit ack via ids-carrier::Endpoint; (9) fan-out to replicas; (10) track synchronized state when replicas ack. A transaction is committed only when all its log records + COMMIT record are on stable storage (ARIES rule #2). Durability levels: local-persisted, quorum-persisted, regionally-durable, cross-region-durable (from `ids-durability-policy`, ids-consistency-profile). Commit vs synchronization distinguished: commit = durable locally; synchronized = durably replicated to required fan-out targets. Sources: MongoDB write concern (w/j), SQL Server transaction log architecture, CMU 15-445 recovery, Martin Fowler high-watermark/commit index.

#### 3.4 Group Commit & Batching
Concurrent commits coalesce into one `write()` + one `fdatasync()`/`fsync()` (group commit) to amortize the fixed flush cost. Release rule: flush after timer T, after K pending records, or adaptive. Ack is held until the group flush returns. Off-lock fsync lets new appends proceed during an in-flight flush. Sources: Group Commit Self-Clocks (arxiv 2606.27051), wal-engine `append_batch`, Happy Engineering WAL & group commit (off-lock fsync, commit tokens in ticket order), toyMQ batched-fsync ADR.

#### 3.5 Idempotent Writes & Operation References
`OpRef` = exactly 9 ASCII alphanumeric bytes (36^9 namespace, from `ids-000-aa`). Client supplies idempotency key; `ids-write` binds OpRef → result. Duplicate OpRef within the dedup window returns the prior result without re-execution. Dedup state persisted in WAL (commit idempotency key + outcome). Window sized from longest survivable outage; window expiry = duplicate-admission event, not silent cleanup (Machine-Checked Dual-Write Recovery). RIFL converts at-least-once to exactly-once via idempotent ops; DynamoDB Global Tables uses operation ID + origin ID + dedup window (~10 min). Idempotency guarantees exactly-once application even with retries/replays. Sources: RIFL (Stanford rifl.pdf), Stripe idempotency keys, Kafka idempotent-producer dedup, Machine-Checked Dual-Write Recovery (arxiv 2608.00501), scirs2-io exactly-once WAL.

#### 3.6 Segment Rotation, Checkpoint, Truncation, GC
WAL grows unbounded unless bounded. Segments roll at size threshold; a background `LogGCOp`-style task deletes segments no longer needed for durability (all replicas acked past them, checkpoint taken). Before truncation, a checkpoint (filtered WAL / snapshot of committed state) is taken named after the last retained segment; replay = load checkpoint + replay subsequent segments. Idempotent replay tolerates re-applying already-checkpointed ops. Loki: delete all but last segment at checkpoint interval, advance WAL segments first. Prometheus: checkpoint after head truncation, resume replay from segment after checkpoint. Kudu: fixed-size segments, LogGCOp background GC. RocksDB: variable-length records padded to block size, trailer skipped. Sources: Grafana Loki WAL design doc, Prometheus TSDB WAL blog, Kudu WAL GC docs, RocksDB WAL file format wiki.

#### 3.7 Torn-Tail & Corruption Recovery
On restart, scan WAL forward; a torn/partial final record (interrupted write) or CRC mismatch = logical end of log; truncate the tail at last complete record (never repair/guess). Damage in a sealed (non-tail) segment = hard corruption error, never silent data loss (wal-engine design). Every record checksummed independently so a batch crash can leave a valid prefix. fsync before ack ensures acked records survive power loss. Sources: wal-engine torn-tail recovery, raft-wal commit frame CRC, SQLite/LMDB/BoltDB assumptions (powersafe overwrites).

#### 3.8 Write Fan-Out (~30 Databases)
A write fans out to up to ~30 downstream databases/replicas simultaneously. Each target: independent `ids-carrier::Endpoint`; ordered/versioned stream; per-target ack cursor; lag observability; resumable delivery from cursor. Multi-leader/multi-writer fan-out: concurrent writers produce independent histories merged by ordering position; each event carries origin identity so forwarding preserves origin (no replication loops). Fan-out rate model: cross-region egress ≈ W·B·(M−1); balanced ingress ≈ W·B·(M−1)/M. Idempotent application at targets (OpRef dedup) tolerates at-least-once redelivery. Sources: TiCDC multi-writer CDC, Cassandra CommitLog (idempotent mutations, shared segments), system-design-patterns multi-leader replication commit/ship/apply.

#### 3.9 Write-Order Management & Concurrent-Write Resolution
Concurrent writes to the same key/partition are totally ordered by `OrderingPos` (assigned at append). Last writer wins by ordering position, or domain-scoped conflict rule from `ids-consistency` / `ids-crdt`. Optimistic concurrency: prepare against current version; commit advances version conditionally (OCC, Apache Iceberg pattern) — if another writer committed first, discard and retry. Write-order management preserves per-key serialization; cross-key ordering left to `ids-ordering-domain` (dataset-scoped, no global master). Sources: Iceberg optimistic concurrency control, Apache Cassandra commit log ordering, CMU 15-445 logging.

#### 3.10 Backpressure & Flow Control
When writes arrive faster than the WAL can fsync, apply backpressure: reject/throttle new writes once unflushed bytes exceed a ceiling (Service Fabric `MaxWriteQueueDepthInKB`, ClickHouse WAL backpressure cgroup caps, Loki disk-full threshold 90%, replay memory ceiling). WAL backpressure is distinct from carrier backpressure; `ids-write` exposes credit-based signals only through `ids-carrier::Endpoint` payloads (no network stack). Sources: ClickHouse WAL backpressure, Service Fabric throttling guidelines, Grafana Loki WAL disk-full throttle, OpenData backpressure on unflushed deltas.

#### 3.11 Crypto Surface & Integrity
Allowed crypto only: ed25519 (signatures), xchacha20poly1305/chacha20poly1305 (AEAD), sha256 and blake3 (hashing/checksums). WAL record integrity: blake3 or sha256 checksum per record (not CRC32C — CRC32 not in allowed list; blake3 preferred for speed). OpRef/signing surface from `ids-000-aa` (`Signature`, `KeyId`, `Checksum`). No `ids-crypto` runtime dependency (crypto crate does not depend on `ids-000-aa`; `ids-write` consumes type surface only). Sources: RFC 8032 Ed25519, RFC 7539 ChaCha20-Poly1305, libsodium XChaCha20-Poly1305, NIST FIPS 180-4 SHA-256, BLAKE3 spec.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|---|---|
| Unacked write lost on power loss | fsync before ack; group commit fence; durable LSN published only after flush |
| Torn/partial WAL record on crash | Per-record blake3/sha256 checksum; torn-tail truncation at last complete record |
| Silent corruption in sealed segment | Hard error on checksum mismatch in non-tail segment; never guess |
| Duplicate apply after replay/restart | Idempotent replay (`record_lsn > applied_lsn`); OpRef dedup window |
| OpRef collision / replay attack | 36^9 namespace; allocator uniqueness; collision variant in error contract |
| Write fan-out target lost / lagging | Per-target ack cursor; resumable delivery; bounded backoff retries; lag observability |
| Write storm exhausts WAL disk | Backpressure throttle at unflushed-byte ceiling; disk-full threshold reject |
| Carrier-agnostic rule violation (`std::net`, IP, etc.) | `ids-carrier::Endpoint` sole address; dependency audit bans networking crates |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| Ordering position overflow | `saturating_add`/`saturating_sub`; MAX constants |
| Commit vs synchronization confusion | Separate commit ack (local durable) from synchronized ack (replicas applied) |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 012 |
|---|---|
| `ids-000-aa` (master) | Canonical types: `OpRef`, `OrderingPos`, `Endpoint`, `OpState`, `CommitState`, `Checksum`, `Signature`, `KeyId`; sealed trait contracts; invariants; versioning; errors |
| `ids-carrier` (Phase 002) | `Endpoint` opaque transport for commit acks and fan-out datagrams; no `std::net` |
| `ids-crypto` (Phase 003) | Type surface only (`Checksum`, `Signature`); no runtime dependency (avoids cycle — `ids-crypto` does not depend on `ids-000-aa`) |
| `ids-commit-state` (Phase 057) | Consumes write lifecycle states; `ids-write` emits state transitions |
| `ids-durability-policy` (Phase 058) | Durability conditions evaluated on commit path |
| `ids-consistency-profile` (Phase 060) | Consistency profile gating on write acknowledgement |
| `ids-ordering-domain` (Phase 055) | Dataset-scoped ordering positions; no global master |
| `ids-operation-ref` (Phase 056) | OpRef generation/uniqueness (9-byte, 36^9) |
| `ids-replication` / `ids-replication-stream` (Phases 017/062) | Fan-out targets; ordered/versioned change streams, ack tracking |
| `ids-consistency` (Phase 019) | Operation-state tracking (received/ordered/executing/committed/synchronized/verified/failed) |
| All other `ids-*` crates | Depend on `ids-000-aa`; `ids-write` depends on `ids-000-aa` only |

Integration points:
- `ids-write` never touches the network directly; all fan-out/ack traffic via `ids-carrier::Endpoint`.
- `ids-commit-state`, `ids-durability-policy`, `ids-consistency-profile` define the semantics `ids-write` enforces; `ids-write` is the enforcement point on the durable write path.
- Cargo workspace member; `ids-write = { path = "../ids-write" }` dependents declare path dependency on `ids-000-aa`.
- No dependency on `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-write` (new at `C:/idfs/ids-write/`)

```
ids-write/
├── Cargo.toml                          # blake3, sha2, thiserror, serde; workspace member; depends on ids-000-aa
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── errors.rs                       # WriteError enum (thiserror) + recovery hints
│   ├── prelude.rs                      # re-export glob
│   │
│   ├── wal/                            # write-ahead log engine
│   │   ├── mod.rs
│   │   ├── wal_engine.rs               # append, sync, group commit, durable_lsn
│   │   ├── segment.rs                  # segment file, roll at size threshold
│   │   ├── record.rs                   # WAL record types (BEGIN_OP/WRITE_RECORD/COMMIT_OP/ABORT_OP/CHECKPOINT/TRUNCATE/FANOUT_ACK)
│   │   ├── record_header.rs            # LSN u64, type u8, length u32, flags u8, checksum u32
│   │   ├── checksum.rs                 # blake3/sha256 per-record checksum
│   │   ├── reader.rs                   # sequential read, resume at LSN
│   │   ├── recovery.rs                 # torn-tail truncation, idempotent replay
│   │   ├── checkpoint.rs               # checkpoint creation, resume from checkpoint
│   │   ├── truncation.rs               # segment GC, retention watermark
│   │   └── group_commit.rs             # batch flush, commit tokens, ticket-order publish
│   │
│   ├── write_path/                     # durable write path
│   │   ├── mod.rs
│   │   ├── write_engine.rs             # received→ordered→executing→committed→synchronized
│   │   ├── op_handler.rs               # receive op, OpRef idempotency check
│   │   ├── ordering.rs                 # OrderingPos allocator, next/prev, saturating
│   │   ├── commit_ack.rs               # commit acknowledgement tracker
│   │   ├── durability.rs               # durability condition evaluator (local/quorum/regional/cross-region)
│   │   ├── execute.rs                  # executing-state application
│   │   └── synchronize.rs              # synchronized-state via fan-out acks
│   │
│   ├── idempotency/                    # OpRef dedup
│   │   ├── mod.rs
│   │   ├── op_ref_binder.rs            # OpRef → result binding, dedup window
│   │   └── dedup_window.rs             # window sizing, expiry as duplicate-admission
│   │
│   ├── fanout/                         # write fan-out (~30 databases)
│   │   ├── mod.rs
│   │   ├── fanout_manager.rs           # fan-out to N targets via ids-carrier::Endpoint
│   │   ├── target_cursor.rs            # per-target ack cursor, lag
│   │   ├── resumable_delivery.rs       # resume from cursor, bounded backoff
│   │   └── lag_observability.rs        # replication lag metrics
│   │
│   ├── write_order/                    # ordering & concurrent-write resolution
│   │   ├── mod.rs
│   │   ├── write_order_manager.rs      # total order by OrderingPos per key/partition
│   │   ├── concurrent_resolution.rs    # last-writer-wins / domain-scoped conflict rule
│   │   └── occ_conflict.rs             # optimistic concurrency, version-conditioned commit
│   │
│   ├── backpressure/                   # flow control under WAL load
│   │   ├── mod.rs
│   │   ├── backpressure_controller.rs  # unflushed-byte ceiling, throttle/reject
│   │   └── credit_signal.rs            # credit-based signal via Endpoint payload
│   │
│   ├── state/                          # lifecycle state machines
│   │   ├── mod.rs
│   │   ├── op_state_machine.rs         # received→ordered→executing→committed→synchronized→verified→failed
│   │   └── commit_state_machine.rs     # accepted→prepared→committed→aborted
│   │
│   ├── carrier/                        # carrier-agnostic boundary
│   │   ├── mod.rs
│   │   └── endpoint_binding.rs         # Endpoint usage only, no std::net
│   │
│   └── crypto_surface/                 # type-level crypto consumption only
│       ├── mod.rs
│       ├── checksum_binding.rs         # blake3/sha256 Checksum
│       └── signature_binding.rs        # ed25519 Signature (type surface only)
│
├── tests/                              # conformance tests (no source modification)
│   ├── wal_tests.rs
│   ├── group_commit_tests.rs
│   ├── recovery_tests.rs               # torn-tail, idempotent replay
│   ├── idempotency_tests.rs            # OpRef dedup
│   ├── write_path_tests.rs             # lifecycle transitions
│   ├── fanout_tests.rs                 # fan-out ack tracking
│   ├── backpressure_tests.rs           # throttle at ceiling
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net usage
│   └── crypto_surface_tests.rs         # asserts only allowed crypto types
│
├── .gitignore
└── README.md
```

Total: ~90–150 files (within ~90–1000 target; room for growth). All `.rs` source + `Cargo.toml` + tests; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **WAL append** — reserve byte range atomically (LSN = start offset), write length-prefixed checksummed record, `durable_lsn()` tracks highest synced LSN. Source: wal-db lock-free append.
2. **Group commit** — concurrent `sync` calls coalesce into one `write()` + one `fdatasync()`; commit tokens resolve in enqueue/ticket order; ack held until group flush returns. Sources: wal-engine `append_batch`, Happy Engineering off-lock fsync, toyMQ batched-fsync.
3. **Torn-tail recovery** — scan forward; CRC mismatch or partial final record = logical EOF; truncate at last complete record; sealed-segment corruption = hard error. Source: wal-engine torn-tail recovery, raft-wal commit CRC.
4. **Idempotent replay** — `record_lsn > applied_lsn` skips already-applied; replay restarts after crash safely (CLR-style undo not needed for logical ops; abort records discard uncommitted ops). Sources: ARIES, CMU 15-445, Duke CompSci 516.
5. **Checkpoint + truncation** — checkpoint named after last retained segment; delete prior segments; resume replay from segment after checkpoint. Sources: Loki truncation, Prometheus TSDB checkpoint, Kudu LogGCOp.
6. **OpRef idempotency** — 9-byte alphanumeric; `OpRef → result` binding; duplicate = cached no-op; dedup window expiry = duplicate-admission event. Sources: RIFL, Stripe, Machine-Checked Dual-Write Recovery.
7. **Ordering position** — monotonic u64 `OrderingPos`; `next()`/`prev()` with saturating arithmetic; per-key total order; cross-key ordering deferred to `ids-ordering-domain`.
8. **Durability condition evaluation** — level 1–6 (local/quorum/regional/cross-region); `meets(required)` gates commit ack; commit vs synchronization distinguished.
9. **Fan-out manager** — per-target `Endpoint`, ack cursor, lag observability, resumable delivery, bounded backoff; idempotent apply at targets. Source: TiCDC, Cassandra CommitLog, multi-leader replication.
10. **Concurrent-write resolution** — last-writer-wins by `OrderingPos`; OCC version-conditioned commit (discard + retry on conflict). Sources: Iceberg OCC, CMU logging.
11. **Backpressure** — throttle/reject when unflushed bytes exceed ceiling; credit-based signal via `Endpoint` payload only. Sources: ClickHouse WAL backpressure, Service Fabric, Loki disk-full.
12. **Checksum per record** — blake3 (preferred speed) or sha256; CRC32C explicitly not used (not in allowed crypto list). Sources: BLAKE3, NIST FIPS 180-4.

---

### 8. Dependencies

`ids-write/Cargo.toml`:
- `ids-000-aa = { path = "../ids-000-aa" }` — canonical types, sealed contracts, invariants, errors
- `blake3 = "1.5"` — WAL record checksum (preferred)
- `sha2 = "0.10"` — SHA-256 WAL record checksum (alternative)
- `thiserror = "2.0"` — `WriteError` enum
- `serde = { version = "1", features = ["derive"] }` — WAL record serialization
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime. No networking stack. `ids-write` does NOT depend on `ids-crypto` at runtime (consumes `Checksum`/`Signature` type surface from `ids-000-aa` only, preserving the no-cycle rule).

Downstream dependents (all on `ids-000-aa`, plus `ids-write` where they consume the write path):
- `ids-commit-state`, `ids-durability-policy`, `ids-consistency-profile`, `ids-replication`, `ids-replication-stream`, `ids-sync`, `ids-read`, `ids-consistency` — consume `ids-write` outputs (commit acks, state transitions, WAL positions).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | WAL engine: append + sync + durable_lsn | `src/wal/wal_engine.rs` exposes `append`, `sync`, `durable_lsn()`; `cargo check -p ids-write` exits 0 |
| 2 | Segmented WAL with roll at size threshold | `src/wal/segment.rs` rolls segment at configurable size; new segment file per sequence |
| 3 | Per-record blake3/sha256 checksum | `src/wal/checksum.rs`; only blake3/sha256 used (no CRC32C) |
| 4 | Torn-tail recovery truncates at last complete record | Recovery test: corrupt final record → log ends at last complete record; no silent data loss |
| 5 | Idempotent replay (`record_lsn > applied_lsn`) | Replay test: re-applying already-applied records is a no-op |
| 6 | Group commit: one flush per batch | `src/wal/group_commit.rs` coalesces concurrent syncs into single `fdatasync` |
| 7 | OpRef idempotency dedup | Duplicate OpRef returns prior result, no re-execution; 9-byte validation |
| 8 | Write lifecycle state machine | `received → ordered → executing → committed → synchronized → verified → failed` transitions enforced |
| 9 | Commit vs synchronization distinguished | Commit ack = local durable; synchronized ack = fan-out targets applied |
| 10 | Write fan-out to N targets via Endpoint | Fan-out manager tracks per-target ack cursor + lag; all traffic via `ids-carrier::Endpoint` |
| 11 | Backpressure at unflushed-byte ceiling | Writes throttled/rejected when unflushed bytes exceed ceiling |
| 12 | Carrier-agnostic assertion | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches |
| 13 | Allowed-crypto-only assertion | Source references only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 type surface; no other crypto crates |
| 14 | Crate compiles cleanly | `cargo check -p ids-write` exits 0 with no new warnings |
| 15 | Conformance tests pass | `cargo test -p ids-write` passes all WAL/recovery/idempotency/write-path/fan-out/backpressure tests |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|---|---|---|
| fsync primitive on Windows (target host) | `fdatasync`/`fsync` semantics differ | Use `std::fs::File::sync_all` + platform-appropriate flush; document per-platform behavior |
| Whether WAL record checksum is blake3 or sha256 by default | Record size, speed | blake3 default (speed), sha256 option; configurable per dataset |
| Logical vs physical record choice | Recovery complexity, record size | Logical records only (IDS has no buffer pool/page concept in this phase) |
| Whether group commit release rule is timer/K-counted/adaptive | Commit latency vs throughput | Start timer + count; adaptive evaluated in bench phase (ids-bench) |
| Dedup window size for OpRef | Memory vs outage survival | Sized from longest intended outage; expiry = duplicate-admission event |
| Whether fan-out is synchronous (blocking) or asynchronous | Write latency | Asynchronous fan-out with per-target cursor; synchronous only for cross-region durability policy |
| Whether `ids-write` owns `OpRef` generation or `ids-operation-ref` does | Allocator visibility | `ids-operation-ref` (Phase 056) generates; `ids-write` binds/validates |
| Whether concurrent-write resolution is LWW-only or domain-scoped | Conflict semantics | LWW by OrderingPos default; domain-scoped rule from `ids-consistency`/CRDT phases |

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| `ids-write` crate for write engine + WAL (not folders in existing crate) | Matches user directive: separate crates, not more folders |
| WAL = append-only, segmented, checksummed (blake3/sha256) | Durable, crash-recoverable, corruption-detectable; checksum restricted to allowed crypto |
| Logical (not physical) WAL records | IDS has no buffer pool/page concept; logical records are smaller and idempotent-replay-friendly |
| `OrderingPos` (u64 LSN) assigned at append, before sync | Monotonic total order; enables idempotent replay (`record_lsn > applied_lsn`) |
| fsync before ack; group commit batching | Durability guarantee; amortizes fixed flush cost across concurrent commits |
| OpRef (9-byte, 36^9) idempotency + dedup window | Exactly-once application under retries/replays; window expiry = duplicate-admission, not cleanup |
| Commit ≠ synchronization (local durable vs replicas applied) | Matches `ids-durability-policy` and `ids-consistency-profile` semantics; commit vs synchronization distinction explicit in CRATES.md |
| Fan-out via `ids-carrier::Endpoint` only, per-target cursor | Carrier-agnostic absolute rule; resumable ordered delivery; lag observability |
| Backpressure at unflushed-byte ceiling | Prevents unbounded WAL memory/disk growth under write storms |
| No `ids-crypto` runtime dependency | Preserves rule that `ids-crypto` does not depend on `ids-000-aa` (no cycles); type surface only |
| `saturating_add`/`saturating_sub` on all position/tick arithmetic | No overflow panics in distributed ordering/commit |
| Cargo workspace member at `C:/idfs/ids-write/` | Matches existing workspace layout (`ids-000-aa` at `C:/idfs/ids-000-aa/`) |

---

### 12. Sources

1. Mohan et al. — ARIES: Algorithm for Recovery and Isolation Exploiting Semantics (Stanford cs345d, 1992). https://web.stanford.edu/class/cs345d-01/rl/aries.pdf — WAL analysis/redo/undo, repeating history, LSN.
2. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf — SHA-256 specification.
3. RFC 8032 — EdDSA / ed25519. https://www.rfc-editor.org/rfc/rfc8032 — Ed25519 signatures.
4. RFC 7539 — ChaCha20-Poly1305 AEAD. https://www.rfc-editor.org/rfc/rfc7539 — 96-bit nonce AEAD.
5. Libsodium — XChaCha20-Poly1305. https://doc.libsodium.org — 192-bit nonce, random nonces.
6. BLAKE3 — BLAKE3 specification. https://github.com/BLAKE3-team/BLAKE3 — fast content-addressed hashing.
7. RocksDB WAL file format. https://github.com/facebook/rocksdb/wiki/Write-Ahead-Log-File-Format — CRC32C/length/type/payload records, 32KB blocks (checksum concept adapted to blake3/sha256).
8. Apache Cassandra CommitLog. https://cassandra.apache.org/_/blog/Learn-How-CommitLog-Works-in-Apache-Cassandra.html — append-only segments, idempotent mutations, shared segments, concurrent append allocation.
9. Grafana Loki WAL design. https://grafana.com/docs/loki/next/community/design-documents/2020-09-write-ahead-log — truncation, checkpoints, replay, segment naming.
10. Prometheus TSDB WAL & checkpoint. https://ganeshvernekar.com/blog/prometheus-tsdb-wal-and-checkpoint — checkpoint after head truncation, resume from segment after checkpoint.
11. Kudu WAL garbage collection. https://docs.cloudera.com/runtime/7.3.2/kudu-background-operations/topics/kudu-write-ahead-log-garbage-collection.html — fixed-size segments, LogGCOp background GC.
12. MongoDB write concern. https://mongodb.com/docs/manual/reference/write-concern — w/j durability acknowledgement levels.
13. SQL Server transaction log architecture. https://learn.microsoft.com/en-us/sql/relational-databases/sql-server-transaction-log-architecture-and-management-guide — LSN, STEAL/NO-FORCE, commit record on stable storage.
14. CMU 15-445/15-721 logging & recovery notes. https://15445.courses.cs.cmu.edu/fall2019/notes/20-logging.pdf — physical vs logical logging, WAL rules, STEAL + NO-FORCE.
15. Martin Fowler — High-Water Mark / Write-Ahead Log. https://martinfowler.com/articles/patterns-of-distributed-systems/high-watermark.html — commit index / high-watermark separation.
16. RIFL — Reusable Infrastructure for Linearizability (Stanford). https://stanford.edu/~ouster/cgi-bin/papers/rifl.pdf — at-least-once → exactly-once via idempotent ops.
17. Stripe idempotency keys. https://stripe.com/blog/idempotency — idempotency keys for exactly-once semantics.
18. Machine-Checked Dual-Write Recovery (Isabelle/HOL). https://arxiv.org/html/2608.00501v4 — dedup window shelf life, expiry as duplicate-admission.
19. Kafka idempotent producer / Stripe 24h dedup window. Referenced in (18); dedup horizon sizing.
20. wal-db — lock-free Rust WAL primitive. https://github.com/jamesgober/wal-db — atomic LSN reservation, group commit, torn-tail recovery.
21. wal-engine — Rust WAL with fsync policies + group commit. https://github.com/anirudhpundhir/wal-engine — Always/EveryN/Interval/Never sync policies, torn-tail truncation.
22. Happy Engineering — WAL & group commit. https://happy.engineering/topics/how-it-works/wal-and-group-commit — off-lock fsync, commit tokens, ticket-order publish.
23. Group Commit Self-Clocks. https://arxiv.org/html/2606.27051v2 — release rule (timer/count/adaptive), flush amortization.
24. BtrLog — low-latency cloud logging. https://arxiv.org/html/2606.27051 — quorum replication, LSN windowing, single round-trip commits.
25. TiCDC multi-writer CDC. https://pingcap.com/blog/multi-writer-change-data-capture-cdc-architecture-ticdc-tidb — multi-writer ordering, merge by origin.
26. System-design-patterns — multi-leader replication commit/ship/apply. https://github.com/babushkai/system-design-patterns/blob/main/02-distributed-databases/02-multi-leader-replication.md — fan-out rate model, origin identity.
27. ClickHouse WAL backpressure. https://clickhouse.com/blog/wal-backpressure-clickhouse-managed-postgres — cgroup write caps by pending segment backlog.
28. Grafana Loki WAL disk-full throttle. https://grafana.com/docs/loki/latest/operations/storage/wal — 90% disk-usage write rejection.
29. Iceberg optimistic concurrency control. https://iceberg.apache.org/docs/1.6.0/reliability/#concurrent-write-operations — version-conditioned commit, discard + retry.
30. IDS prompts.txt entry 13 (verbatim) + CRATES.md §13. C:/idfs/prompts.txt, C:/idfs/CRATES.md — ids-write crate definition, contents, constraints.
31. HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
32. Cargo workspaces. https://doc.rust-lang.org/cargo/reference/workspaces.html — workspace members, path dependencies.

---

**End of plan-012.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**