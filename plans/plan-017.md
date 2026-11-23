## Phase 017 — Create the ids-replication crate (replication streams and checkpoints): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Verbatim prompt (C:/idfs/prompts.txt lines 250-264): "**Phase 017 — Create the ids-replication crate: Change propagation, ordered/versioned streams, resumable delivery.**"

Crate: `ids-replication`
Purpose: Change propagation, ordered/versioned streams, resumable delivery.
Contents per CRATES.md §17 and prompts.txt entry 17: Independent replication, full replication, replication streams, acknowledgement tracking, replication lag observability, synchronization checkpoints, incremental synchronization, synchronization retries, bounded backoff, idempotent application.

The crate is carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-replication` consumes type surface and primitive IDs from `ids-000-aa` only). `ids-000-aa` is the master crate holding every canonical type and sealed trait contract, including the sealed `trait Replication` (streams, checkpoints, retries) at `src/contracts/replication.rs`, which `ids-replication` implements. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned CDC ordered change propagation with LSN/txn-visibility watermarks (arxiv DBLog 2609.08160/2605.31475; YugabyteDB CDC docs; TiCDC; Wikipedia CDC); monotonically increasing log sequence numbers for versioned streams (VLDB logging papers; BtrLog arxiv 2606.27051; Apache DistributedLog; DDIA sequence ordering); checkpoint/resume with at-least-once + idempotent dedup (Airbyte resumability; Machine-Checked Dual-Write Recovery arxiv 2608.00501; JuiceFS; PowerSync; scroll cursor protocol); log-based commit/ship/apply replication (Cornell CS530; Oracle Streams IEEE; replicated-log pattern); sequenced stream delivery with bounded reassembly and gap detection (RFC 2960 SCTP; Wireshark reassembly; EIP resequencer); version vectors for causal ordering (CMU 17-350; arxiv 1511.05010; Wikipedia; COPS SOSP'11); full vs independent replication strategies (ScyllaDB; Redis; MariaDB; CMU 15-445); persistent ACK cursors and per-consumer ack state (Apache Pulsar PIP-81/PIP-299; NATS JetStream; Oxia FollowerCursor); replication lag observability (Google Cloud SQL; AWS RDS; Confluent Replicator; CockroachDB; MongoDB); at-least-once vs exactly-once semantics (Kafka idempotent producer + transactions; Confluent; Strimzi); idempotent consumer + dedup window (Azure pattern; Machine-Checked Dual-Write Recovery); incremental sync + missing-update detection (SurrealDB sync; Sui StateSync; DBLog incremental snapshots; Nango checkpoints); bounded exponential backoff with jitter (AWS Architecture Blog); snapshot + WAL checkpoint truncation + torn-tail recovery (Databricks; happy.engineering; SQL Server; TigerBeetle VSR); state machine replication via replicated log (Cornell DSN; MIT 6.824 Raft; Restate); Raft log replication + InstallSnapshot catch-up (Raft paper; CockroachDB; etcd raft); Kafka consumer offset commit/resume (Confluent; Redpanda); allowed-crypto primitives (RFC 8032 Ed25519; RFC 9591 FROST-Ed25519; libsodium XChaCha20-Poly1305; IETF draft-irtf-cfrg-xchacha-03; BLAKE3 spec; Borg BLAKE3 store hash; NIST FIPS 180-4 SHA-256; RFC 7539/8439 ChaCha20-Poly1305). Carrier-agnostic opaque endpoint: no off-the-shelf standard matches IDS `Endpoint` (HYPOTHESIS — IDS defines its own via `ids-carrier`); stream reassembly memory exhaustion is a documented DoS vector (quinn-proto RUSTSEC-2026-0185/0341; bRPC CVE-2026-80463) that IDS must bound by design.

A skeleton `ids-replication` crate does NOT yet exist at `C:/idfs/ids-replication/`. This plan specifies creation from scratch; no source modifications are made in this phase.

---

### 1. Executive Summary

Phase 017 creates `ids-replication`, the IDS crate for propagating committed changes between replicas over ordered, versioned, resumable streams. It owns the replication half of the write lifecycle: once `ids-write` has durably committed an op (Phase 012), `ids-replication` ships that op's change to independent and full-replication targets, tracks per-target acknowledgement cursors, exposes replication-lag observability, takes synchronization checkpoints, performs incremental synchronization (catch-up from cursor after restart/partition), retries synchronization with bounded backoff, and guarantees idempotent application at targets.

Every change stream is versioned: each record carries a monotonic `VersionPos` (u64) plus the canonical `OrderingPos` from `ids-000-aa`, so receivers can detect gaps, reorder, deduplicate, and resume from any checkpoint. Delivery is carrier-agnostic: stream frames are opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` targets; `ids-replication` never opens a socket. Application at targets is idempotent: an apply key of (dataset, OpRef, VersionPos) inside a dedup window makes redelivery harmless, matching the exactly-once research basis (at-least-once delivery + idempotent apply).

No `ids-replication` code exists yet; this plan is the complete blueprint for implementation. It depends on `ids-000-aa` (canonical types + sealed `trait Replication` contract), `ids-carrier` (opaque `Endpoint` datagram send/receive), and `ids-write` (change source, via `OrderingPos`/`OpRef`). It is consumed by `ids-sync` (Phase 018, replica reconciliation/convergence) and `ids-broadcast` (Phase 020, ordering guarantees).

---

### 2. Architecture Overview

`ids-replication` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-write` (durable commit log source of changes). It is layered so each concern — streams, acks, lag, checkpoints, incremental sync, retries, idempotent apply — is an independent module:

```
┌──────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-SM LAYER (ids-* crates)          │
│  state machine replication · ordered command application · commit    │
├──────────────────────────────────────────────────────────────────────┤
│                    Phase 012: ids-write (CHANGE SOURCE)              │
│  durable WAL → committed ops with OrderingPos + OpRef                │
└──────────────────────────────────┬───────────────────────────────────┘
                                   │ committed change (OrderingPos, OpRef)
┌─────────────────────────────────▼───────────────────────────────────┐
│                 Phase 017: ids-replication (THIS CRATE)             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────┐ │
│  │  streams     │ │  ack tracking│ │  lag         │ │ checkpoints│ │
│  │  (ordered,   │ │  (per-peer   │ │  observability│ │ (sync pts)│ │
│  │   versioned) │ │   cursor)    │ │              │ │            │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────┐ │
│  │  incremental │ │  retries +   │ │  idempotent  │ │  modes     │ │
│  │  sync        │ │  bounded     │ │  application │ │  independent│ │
│  │  (catch-up)  │ │  backoff     │ │  (dedup)     │ │  / full    │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────┘ │
└──────────────────────────────────┬───────────────────────────────────┘
                                   │ opaque Vec<u8> frames
┌─────────────────────────────────▼───────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only           │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, payload: &[u8])      │
│  receive_datagram → (Endpoint, Vec<u8>)                             │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└──────────────────────────────────────────────────────────────────────┘
```

**Data flow of one replicated change:**
1. `ids-write` commits an op: durable WAL record with `OrderingPos`, `OpRef`, dataset id, payload.
2. `ids-replication` streams module wraps it in a versioned frame: `(stream_id, VersionPos, OrderingPos, OpRef, flags, payload, blake3/sha256 checksum, optional ed25519 signature)`.
3. Frame sent as an opaque datagram to each replication target's `ids-carrier::Endpoint` (independent mode: per-dataset streams; full mode: one dataset-wide stream).
4. Target's stream receiver reassembles in-order, detects gaps (NACK/gap report), delivers to idempotent-apply module.
5. Idempotent-apply: apply key `(dataset, OpRef, VersionPos)`; duplicate within dedup window = no-op, return prior result.
6. Target advances its apply cursor and sends an ack back over `ids-carrier::Endpoint`.
7. Source's ack tracker advances the per-target ack cursor; lag meter computes `send_cursor − apply_cursor` per target.
8. Periodically or on catch-up, synchronization checkpoints are taken: `(checkpoint_id, VersionPos, blake3 root hash, peer set, timestamp)`; streams below a stable checkpoint are truncated.
9. On restart/partition heal, incremental sync resumes from the last ack cursor / checkpoint (missing-update detection via version-range comparison).

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `OrderingPos`, `OpRef`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `VersionPos`) imported from `ids_000_aa::X`, never redefined.
- `ids-replication` implements the sealed `trait Replication` (streams, checkpoints, retries) from `ids-000-aa/src/contracts/replication.rs`; downstream crates use it as a trait bound, cannot implement it.
- All transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum + opaque `Vec<u8>`). No IP addresses, ports, sockets, or networking stack.
- Crypto surface: ed25519 signatures (stream/checkpoint authentication), sha256/blake3 (frame/checkpoint checksums, hash chains), xchacha20poly1305/chacha20poly1305 (optional payload encryption). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Replication is dataset-scoped via `ids-ordering-domain`; invariant `no_global_master` in `ids-000-aa` (no permanent global ordering master).
- Operation lifecycle reuses `ids-000-aa` `OpState` and `CommitState`.

---

### 3. Detailed Research Dimensions

#### 3.1 Independent Replication vs Full Replication
**Independent replication:** each dataset (or partition) has its own versioned stream; targets subscribe to the datasets they hold; fan-out is N streams over N `ids-carrier::Endpoint` targets. Rate model: cross-carrier egress ≈ W·B·(M−1) for W writers, B bytes/write, M targets — bounded by per-target credit (see 3.10). Source: multi-leader replication commit/ship/apply (system-design-patterns); TiCDC multi-writer CDC.
**Full replication:** one dataset-wide versioned stream carries every committed op; every target receives and applies the full stream. Simpler ordering (single total order per dataset) but higher bandwidth per target. Used as the baseline mode; independent mode is the optimization.
**Engineering decision:** both modes implemented as stream configurations over the same stream primitive; mode selected per dataset at replication session start. Source research: ESTABLISHED (Q007, Q027 HYPOTHESIS for rate model — IDS-specific model to validate).

#### 3.2 Replication Streams — Ordered, Versioned
Each stream: `stream_id` (from `ids-000-aa` types) + monotonic `VersionPos` (u64, assigned at append, before send) + canonical `OrderingPos`. Frame: `(stream_id, VersionPos, OrderingPos, OpRef, flags, payload_len, payload, checksum [blake3/sha256], optional ed25519 signature)`. Receivers deliver in VersionPos order; out-of-order frames held in a bounded reassembly buffer (cap per stream — bounds memory, see Q029); gaps detected by VersionPos holes trigger NACK/gap report and resume-from-cursor. Logical logging (not physical): frames carry op payloads, not page images (IDS has no buffer pool/page concept). Source: BtrLog/VLDB monotonic LSN (Q002 ESTABLISHED); RFC 2960 sequenced delivery + bounded reassembly (Q005 ESTABLISHED).

#### 3.3 Acknowledgement Tracking — Per-Target Cursors
Each replication target tracks: `send_cursor` (highest VersionPos sent), `ack_cursor` (highest VersionPos acked by target, persisted), `apply_cursor` (highest applied). Acks are cumulative: acking VersionPos V implies all ≤ V applied (like Pulsar cumulative ack marks and Oxia FollowerCursor.AckOffset). Out-of-order acks folded into a persistable ack-watermark cursor. The ack cursor is the resume point: after restart, the stream resumes sending at `ack_cursor + 1`. Source: Pulsar PIP-81/PIP-299, NATS JetStream consumer ack state, Oxia FollowerCursor (Q008, Q010 ESTABLISHED).

#### 3.4 Replication Lag Observability
`lag(target) = send_cursor(target) − apply_cursor(target)` measured in VersionPos units and bytes; also time-based lag (last_send_time − last_apply_time). Exposed as metrics per target and per stream: lag count, lag bytes, max lag, lag over threshold alerts. Lag drives backpressure (3.10) and catch-up priority in incremental sync. Source: Google Cloud SQL replication lag, AWS RDS, Confluent Replicator monitoring, CockroachDB logical replication monitoring, MongoDB rs.printSecondaryReplicationInfo (Q009 ESTABLISHED).

#### 3.5 Synchronization Checkpoints
A synchronization checkpoint: `(checkpoint_id, VersionPos, root_hash [blake3 hash chain of applied stream prefix], peer_set, timestamp, dataset)`. Checkpoints are durable (persisted via the store contract). A checkpoint is *stable* when all peers in `peer_set` have acked past its Versionpos (causal stability). Streams below the latest stable checkpoint are truncated (segments deleted, log_start_offset advanced). Recovery: load latest checkpoint + replay stream frames after it. Source: snapshot + WAL checkpoint truncation (Q015 ESTABLISHED); Raft InstallSnapshot catch-up (Q017 ESTABLISHED); TigerBeetle VSR checkpoints.

#### 3.6 Incremental Synchronization — Missing Update Detection
After restart or partition heal, a target does not re-sync everything. It exchanges version ranges with the source: `missing = [source_version_range] \ [target_applied_range]` computed from version vectors / per-stream apply cursors. The source ships only missing frames (delta exchange), then resumes live streaming. Timestamp-based tracking (SurrealDB) is rejected as primary mechanism (sub-microsecond rounding gaps drop rows — Q013 airbyte issue); version-position ranges are the source of truth. Source: Sui StateSync checkpoint discovery, DBLog incremental snapshots, Nango incremental/full sync docs (Q013 ESTABLISHED).

#### 3.7 Synchronization Retries — Bounded Backoff
Retry policy: classify errors (transient: carrier unavailable, endpoint not initialized, timeout → retry; permanent: invalid endpoint, authentication failure → do not retry). Bounded attempts (max N). Exponential backoff with jitter: `delay = min(cap, base * 2^attempt + jitter)`, jitter decorrelates retries across replicas to avoid thundering herds/retry storms. Retry budget caps concurrent in-flight retries. Source: AWS Architecture Blog exponential backoff + jitter (Q014, Q026 ESTABLISHED).

#### 3.8 Idempotent Application
Apply key: `(dataset_id, OpRef, VersionPos)`. Target maintains a dedup window of recent apply keys; a frame whose key is in the window is applied as no-op and the prior result returned. Dedup window expiry = duplicate-admission event, never silent cleanup (Machine-Checked Dual-Write Recovery). Window sized from the longest survivable outage. Idempotency turns at-least-once carrier delivery into exactly-once effect. Source: Azure Idempotent Consumer pattern, Kafka idempotent producer, arxiv 2608.00501 (Q011, Q012 ESTABLISHED).

#### 3.9 Crypto Surface & Integrity
Allowed crypto only: ed25519 (stream frame / checkpoint signatures — transferable authentication evidence), xchacha20poly1305 / chacha20poly1305 (optional payload AEAD; XChaCha preferred for random nonces — 192-bit nonce bounds misuse risk), sha256 and blake3 (per-frame checksums, checkpoint root-hash hash chains). CRC32C explicitly not used (not in allowed list; blake3 preferred for speed). All crypto ops live in `ids-crypto`; `ids-replication` declares usage boundaries and primitive IDs only. Source: RFC 8032/9591, libsodium XChaCha docs, IETF draft-irtf-cfrg-xchacha-03, BLAKE3 spec, Borg BLAKE3-256, NIST FIPS 180-4 (Q020-Q024 ESTABLISHED).

#### 3.10 Backpressure & Flow Control
When targets apply slower than the source sends, the source applies backpressure: reject/throttle new stream frames once unacked bytes per target exceed a ceiling (credit-based; credit granted by target acks returned over `ids-carrier::Endpoint` payloads). Bounded reassembly buffer per stream caps memory; peers flooding past the window are refused (not recorded as received) — direct response to the documented stream-reassembly memory-exhaustion DoS (Q029). Source: Akka Streams boundedness, system-design-patterns backpressure, Flink backpressure (Q025 ESTABLISHED).

#### 3.11 Research Status by Dimension
| Dimension | Status | Basis |
|-----------|--------|-------|
| Independent vs full replication | ESTABLISHED | Q007; rate model HYPOTHESIS (Q027) |
| Versioned ordered streams | ESTABLISHED | Q002, Q005 |
| Ack tracking / cursors | ESTABLISHED | Q008, Q010 |
| Lag observability | ESTABLISHED | Q009 |
| Checkpoints | ESTABLISHED | Q013, Q015, Q017 |
| Incremental sync | ESTABLISHED | Q013 |
| Retries + bounded backoff | ESTABLISHED | Q014, Q026 |
| Idempotent application | ESTABLISHED | Q011, Q012 |
| Carrier-agnostic endpoint | HYPOTHESIS (IDS-defined) | Q019 |
| Allowed crypto (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) | ESTABLISHED | Q020-Q024 |
| Bounded reassembly memory | ESTABLISHED problem; scheme = design decision | Q029 |
| Log shipping catch-up | ESTABLISHED | Q030 |

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation |
|---|-----------|------------|
| Lost change on carrier failure (unacked frame) | Retry with bounded backoff from ack cursor; at-least-once delivery + idempotent apply |
| Duplicate redelivery after retry/restart | Idempotent apply key `(dataset, OpRef, VersionPos)` + dedup window |
| Out-of-order / duplicated frames | VersionPos ordering + bounded reassembly buffer + gap NACK |
| Stream reassembly memory exhaustion | Per-stream reassembly cap; refuse frames past window (never record) |
| Replication lag unbounded (slow target) | Per-target credit/backpressure; lag metrics + alerts; catch-up priority |
| Checkpoint taken while target behind → truncation loses data | Checkpoint stable only when all peers acked past it (causal stability) |
| Checkpoint / frame corruption | blake3/sha256 per-frame checksum; fail closed on mismatch; hash-chain checkpoints |
| Forged / tampered stream | ed25519 signatures on frames/checkpoints; nonce-random XChaCha20-Poly1305 optional encryption |
| Disallowed crypto / networking stack | Dependency audit bans std::net/tokio::net/libp2p etc.; only 5 allowed primitives |
| OpRef / VersionPos collision or replay | 36^9 OpRef namespace; monotonic VersionPos; collision variant in error contract |
| Position overflow | saturating arithmetic on VersionPos/OrderingPos |
| Retry storm across replicas | Jitter + bounded attempts + retry budget |
| Commit vs synchronization confusion | Commit = local durable (ids-write); synchronized = target applied (ack cursor) |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 017 |
|---|-------|-------------------|
| `ids-000-aa` (master) | Canonical types: `NodeId`, `OrderingPos`, `OpRef`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `VersionPos`; sealed `trait Replication` (streams, checkpoints, retries); invariants (`no_global_master`, `out_eq_in`); errors; versioning |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); `send_datagram`/`receive_datagram`; sole transport for all replication traffic |
| `ids-write` (Phase 012) | Change source: committed WAL records with `OrderingPos` + `OpRef` fed into replication streams |
| `ids-crypto` (Phase 003) | Crypto ops (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305); `ids-replication` consumes type surface/primitive IDs only, no runtime dependency beyond allowed set |
| `ids-ordering-domain` (Phase 055) | Dataset-scoped ordering domains; no global master |
| `ids-sync` (Phase 018) | Consumer: incremental change streams + checkpoint management for reconciliation/convergence |
| `ids-broadcast` (Phase 020) | Consumer: ordered broadcast layer builds on replication streams |
| `ids-consistency` (Phase 019) | Consistency profiles gating replication acknowledgement |
| All other `ids-*` crates | Depend on `ids-000-aa`; `ids-replication` depends on `ids-000-aa` + `ids-carrier` only |

Integration points:
- `ids-replication` never touches the network directly; all stream/ack/checkpoint traffic via `ids-carrier::Endpoint` datagrams.
- `ids-replication` implements the sealed `trait Replication` from `ids-000-aa`; downstream crates (`ids-sync`, `ids-broadcast`) depend on it as a trait bound.
- `ids-write` emits committed changes; `ids-replication` consumes them as the change source — no circular dependency.
- Cargo workspace member; `ids-replication = { path = "../ids-replication" }`; dependents declare path dependency.
- No dependency on `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-replication` (new at `C:/idfs/ids-replication/`)

```
ids-replication/
├── Cargo.toml                          # blake3, sha2, chacha20poly1305, ed25519, thiserror, serde; workspace member; depends on ids-000-aa, ids-carrier
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── errors.rs                       # ReplicationError enum (thiserror) + recovery hints
│   ├── prelude.rs                      # re-export glob
│   ├── constants.rs                    # protocol constants (dedup window, reassembly cap, backoff cap)
│   │
│   ├── streams/                        # ordered, versioned replication streams
│   │   ├── mod.rs
│   │   ├── stream.rs                   # ReplicationStream: send/recv versioned frames over Endpoint
│   │   ├── stream_session.rs           # per-peer stream state (send/recv cursors)
│   │   ├── version_pos.rs              # VersionPos monotonic u64 (wrapper over ids-000-aa OrderingPos)
│   │   ├── stream_id.rs                # StreamId type + dataset scoping
│   │   ├── frame.rs                    # stream frame: VersionPos/OrderingPos/OpRef/flags/payload/checksum/signature
│   │   ├── sequencing.rs               # VersionPos allocation; send-seq vs recv-seq
│   │   ├── reassembly.rs               # bounded out-of-order frame reassembly + gap detection
│   │   ├── backpressure.rs             # per-target credit/flow control over Endpoint payloads
│   │   └── stream_registry.rs          # active stream lifecycle per dataset
│   │
│   ├── ack/                            # acknowledgement tracking
│   │   ├── mod.rs
│   │   ├── ack_tracker.rs              # per-target ack state: send/ack/apply cursors
│   │   ├── ack_cursor.rs               # durable ack cursor = resume point
│   │   ├── ack_watermark.rs            # fold out-of-order acks into persistable cursor
│   │   ├── ack_policy.rs               # cumulative ack semantics
│   │   └── nack.rs                     # gap report / negative acknowledgement
│   │
│   ├── lag/                            # replication lag observability
│   │   ├── mod.rs
│   │   ├── lag_meter.rs                # lag = send_cursor − apply_cursor per target (count + bytes + time)
│   │   ├── lag_probe.rs                # lag sampling over carrier round trips
│   │   ├── lag_report.rs               # per-stream/per-target lag report
│   │   └── lag_alert.rs                # lag threshold alerts
│   │
│   ├── checkpoints/                    # synchronization checkpoints
│   │   ├── mod.rs
│   │   ├── checkpoint.rs               # checkpoint record: id/VersionPos/root_hash/peer_set/timestamp
│   │   ├── checkpoint_store.rs         # durable checkpoint persistence
│   │   ├── checkpoint_coordinator.rs   # initiate/ack checkpoints; stability = all peers acked past
│   │   ├── truncation.rs               # truncate streams below stable checkpoint
│   │   └── hash_chain.rs               # blake3/sha256 hash chain of applied stream prefix
│   │
│   ├── incremental/                    # incremental synchronization (catch-up)
│   │   ├── mod.rs
│   │   ├── sync_session.rs             # incremental sync session state machine
│   │   ├── missing_detection.rs        # version-range missing update detection
│   │   ├── catchup.rs                  # catch-up from cursor after restart/partition
│   │   ├── delta_rsync.rs              # delta exchange (hash-based diff of stream prefix)
│   │   └── log_shipping.rs             # segment-by-segment WAL-style catch-up
│   │
│   ├── retries/                        # synchronization retries + bounded backoff
│   │   ├── mod.rs
│   │   ├── retry_policy.rs             # transient vs permanent error classification
│   │   ├── backoff.rs                  # exponential backoff with jitter, bounded cap
│   │   └── retry_budget.rs             # cap on concurrent in-flight retries
│   │
│   ├── idempotent/                     # idempotent application
│   │   ├── mod.rs
│   │   ├── apply_guard.rs              # apply key (dataset, OpRef, VersionPos) guard
│   │   ├── dedup_window.rs             # dedup window sizing; expiry = duplicate-admission event
│   │   └── application.rs              # idempotent apply entry point
│   │
│   ├── modes/                          # independent vs full replication
│   │   ├── mod.rs
│   │   ├── replication_mode.rs         # Independent / Full enum
│   │   ├── independent.rs              # per-dataset streams
│   │   └── full.rs                     # dataset-wide single stream
│   │
│   ├── carrier/                        # carrier-agnostic boundary
│   │   ├── mod.rs
│   │   └── endpoint_send.rs            # Endpoint usage only; no std::net
│   │
│   ├── crypto_surface/                 # allowed crypto only (usage boundaries)
│   │   ├── mod.rs
│   │   ├── signing.rs                  # ed25519 stream/checkpoint signatures
│   │   ├── encryption.rs               # xchacha20poly1305 / chacha20poly1305 payload AEAD
│   │   └── hashing.rs                  # sha256 / blake3 frame checksums + hash chains
│   │
│   ├── state/                          # replication state machines
│   │   ├── mod.rs
│   │   ├── stream_state.rs             # stream lifecycle state machine (open/streaming/paused/closed/failed)
│   │   └── sync_state.rs               # sync session state machine
│   │
│   └── types/                          # composed types
│       ├── mod.rs
│       ├── peer_id.rs                  # peer identity over Endpoint
│       └── replication_config.rs       # per-session replication configuration
│
├── tests/                              # conformance tests (no source modification)
│   ├── stream_tests.rs                 # ordered/versioned delivery
│   ├── reassembly_tests.rs             # out-of-order + bounded buffer
│   ├── ack_tests.rs                    # per-target cursor + cumulative ack
│   ├── lag_tests.rs                    # lag meter + alerts
│   ├── checkpoint_tests.rs             # checkpoint/stability/truncation
│   ├── incremental_tests.rs            # missing detection + catch-up
│   ├── retry_tests.rs                  # bounded backoff + budget
│   ├── idempotent_tests.rs             # dedup window + no-op redelivery
│   ├── carrier_agnostic_tests.rs       # asserts zero std::net/tokio::net/libp2p usage
│   └── crypto_surface_tests.rs         # asserts only allowed crypto primitives
│
├── benches/
│   ├── stream_bench.rs
│   ├── ack_bench.rs
│   ├── lag_bench.rs
│   ├── checkpoint_bench.rs
│   └── backpressure_bench.rs
│
├── docs/
│   ├── architecture.md                 # replication architecture
│   ├── stream_protocol.md              # versioned frame format
│   ├── checkpoint_format.md            # checkpoint record format
│   └── integration_guide.md            # integrating with ids-sync / ids-broadcast
│
├── spec/
│   ├── replication_stream.md         # stream protocol spec
│   ├── checkpoint.md                   # checkpoint spec
│   └── ack_protocol.md                 # ack/cursor protocol spec
│
├── fuzz/
│   ├── mod.rs
│   ├── fuzz_stream.rs
│   ├── fuzz_frame.rs
│   └── fuzz_checkpoint.rs
│
├── test_vectors/
│   ├── stream_vectors.json
│   ├── checkpoint_vectors.json
│   └── ack_vectors.json
│
├── .gitignore
└── README.md
```

Total: ~90 files (within ~90–1000 target; room for growth). All `.rs` source + `Cargo.toml` + tests + specs; no binary targets.

---

### 7. Key Algorithms & Data Structures

1. **Versioned stream frame** — `(stream_id, VersionPos, OrderingPos, OpRef, flags, payload, checksum[blake3/sha256], optional ed25519 signature)`. VersionPos assigned at append, before send; enables gap detection and resume. Source: BtrLog/VLDB monotonic LSN (Q002).
2. **Ordered delivery with bounded reassembly** — receiver holds out-of-order frames in a per-stream bounded buffer; delivers contiguous prefix from `recv_cursor`; VersionPos holes trigger NACK/gap report; buffer cap prevents memory-exhaustion DoS. Source: RFC 2960 sequenced delivery (Q005); quinn-proto RUSTSEC-2026-0185 fix pattern (Q029).
3. **Per-target ack cursor** — cumulative ack: acking VersionPos V implies all ≤ V applied; ack cursor persisted as resume point; out-of-order acks folded into watermark. Source: Pulsar PIP-81 cumulative acks, Oxia FollowerCursor.AckOffset (Q008, Q010).
4. **Replication lag** — `lag(target) = send_cursor − apply_cursor` in VersionPos units + bytes + time; per-target metrics and alerts; feeds backpressure and catch-up priority. Source: Cloud SQL/AWS/CockroachDB/MongoDB lag monitoring (Q009).
5. **Synchronization checkpoint** — `(checkpoint_id, VersionPos, root_hash, peer_set, timestamp)`; stable when all peers ack past VersionPos; truncate streams below stable checkpoint; recovery = load checkpoint + replay after it. Source: snapshot+WAL truncation (Q015); Raft InstallSnapshot (Q017).
6. **Incremental sync / missing detection** — missing = source version range \ target applied range; ship only missing frames then resume live; version-position ranges (not timestamps) are source of truth. Source: Sui StateSync, DBLog incremental snapshots (Q013).
7. **Retry + bounded backoff** — classify transient/permanent; bounded attempts; `delay = min(cap, base*2^attempt + jitter)`; retry budget; jitter prevents thundering herds. Source: AWS Architecture Blog (Q014, Q026).
8. **Idempotent application** — apply key `(dataset, OpRef, VersionPos)`; dedup window; duplicate = no-op returning prior result; window expiry = duplicate-admission event. Source: Azure Idempotent Consumer, Kafka idempotent producer, arxiv 2608.00501 (Q011, Q012).
9. **Credit-based flow control** — per-target unacked-byte ceiling; target grants credit via acks over Endpoint payloads; throttle/reject above ceiling. Source: Akka Streams boundedness, backpressure pattern (Q025).
10. **Hash-chain checkpoint integrity** — blake3/sha256 hash chain over applied stream prefix; checkpoint root hash anchors chain; Ed25519 signature on checkpoint. Source: SHA-256 hash chains, FIPS 180-4 (Q023).

---

### 8. Dependencies

`ids-replication/Cargo.toml`:
- `ids-000-aa = { path = "../ids-000-aa" }` — canonical types, sealed `trait Replication` contract, invariants, errors
- `ids-carrier = { path = "../ids-carrier" }` — opaque `Endpoint` datagram send/receive
- `blake3 = "1.5"` — frame/checkpoint checksum (preferred), hash-chain root hash
- `sha2 = "0.10"` — SHA-256 frame/checkpoint checksum (alternative)
- `chacha20poly1305 = "0.11"` — XChaCha20-Poly1305 / ChaCha20-Poly1305 optional payload AEAD
- `ed25519 = "1.5"` / `ed25519-dalek = "2.1"` — stream/checkpoint signatures
- `thiserror = "2.0"` — `ReplicationError` enum
- `serde = { version = "1", features = ["derive"] }` — frame/checkpoint serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime requirement beyond what `ids-carrier` provides. No networking stack. `ids-replication` depends on `ids-000-aa` and `ids-carrier` only (plus allowed-crypto crates).

Downstream dependents: `ids-sync` (Phase 018), `ids-broadcast` (Phase 020), `ids-consistency` (Phase 019) — consume `ids-replication` streams/acks/checkpoints.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement |
|---|---|-----------|-------------|
| 1 | Versioned stream send/recv | `src/streams/stream.rs` exposes ordered send/recv over `ids-carrier::Endpoint`; `cargo check -p ids-replication` exits 0 |
| 2 | VersionPos monotonic assignment | Every frame gets monotonically increasing VersionPos before send; saturating arithmetic |
| 3 | Per-frame blake3/sha256 checksum | `src/crypto_surface/hashing.rs`; only blake3/sha256 (no CRC32C) |
| 4 | Bounded out-of-order reassembly | `src/streams/reassembly.rs` caps buffer per stream; gap floods refused, never recorded |
| 5 | Gap detection + NACK | VersionPos hole triggers NACK/gap report; resume from cursor |
| 6 | Per-target ack cursor (cumulative) | `src/ack/ack_cursor.rs`; acking V implies all ≤ V applied; cursor is resume point |
| 7 | Replication lag metrics | `src/lag/lag_meter.rs` computes send_cursor − apply_cursor per target; alerts on threshold |
| 8 | Synchronization checkpoint + stability | `src/checkpoints/checkpoint_coordinator.rs`; stable iff all peers acked past checkpoint VersionPos |
| 9 | Truncation below stable checkpoint | `src/checkpoints/truncation.rs` truncates streams below latest stable checkpoint |
| 10 | Incremental sync catch-up | `src/incremental/catchup.rs` resumes from last ack cursor/checkpoint; missing = version range diff |
| 11 | Retry + bounded backoff with jitter | `src/retries/backoff.rs`: `delay = min(cap, base*2^attempt + jitter)`; bounded attempts; retry budget |
| 12 | Idempotent application + dedup window | `src/idempotent/apply_guard.rs`; duplicate `(dataset, OpRef, VersionPos)` = no-op, prior result returned |
| 13 | Independent and full replication modes | `src/modes/independent.rs` (per-dataset streams), `src/modes/full.rs` (dataset-wide stream) |
| 14 | Carrier-agnostic assertion | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`, `QUIC`, `HTTP`, `DNS` — zero matches |
| 15 | Allowed-crypto-only assertion | Source references only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305; no other crypto crates |
| 16 | Crate compiles cleanly | `cargo check -p ids-replication` exits 0 with no new warnings |
| 17 | Conformance tests pass | `cargo test -p ids-replication` passes all stream/ack/lag/checkpoint/incremental/retry/idempotent tests |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path |
|---|-------------|--------|-----------------|
| Fan-out rate model for heterogeneous carriers | Bandwidth/credit sizing per target | Q027 HYPOTHESIS — validate with ids-sim benchmarks; credit ceiling is a tunable |
| Dedup window size | Memory vs outage survival | Sized from longest intended outage; expiry = duplicate-admission event |
| Checkpoint interval vs truncation aggressiveness | Recovery time vs storage | Start time-based + count-based; adaptive in bench phase |
| Whether `ids-crypto` runtime dep is allowed | Dependency direction (ids-crypto must not depend on ids-000-aa) | Confirm no-cycle rule; use primitive IDs + type surface only |
| Reassembly memory cap value | DoS resistance vs latency under reorder | Derive from bounded-buffer analysis; configurable per stream |
| Endpoint framing/MTU per carrier type | Frame size limits | CarrierType-specific max datagram size negotiated via ids-carrier capability |
| Whether VersionPos and OrderingPos are distinct or aliased | API simplicity vs semantics | VersionPos = stream-local monotonic; OrderingPos = canonical global position from ids-000-aa |
| Formal verification scope for stream state machine | Confidence in gap/liveness properties | TLA+ spec in ids-state-machine deferred; property tests first |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale |
|---|-----------|-----------|
| `ids-replication` as separate crate | Matches user directive: separate crates, not more folders |
| Versioned streams with VersionPos + OrderingPos | Monotonic versioning enables gap detection, dedup, resume (Q002 ESTABLISHED) |
| Logical (op payload) frames, not physical page images | IDS has no buffer pool/page concept; logical frames smaller and idempotent-apply-friendly |
| Cumulative per-target ack cursor as resume point | Proven pattern (Pulsar, NATS JetStream, Oxia); resume = cursor + 1 |
| Checkpoint stable iff all peers acked past it | Prevents truncation data loss; causal stability (Q013, Q015, Q017 ESTABLISHED) |
| Version ranges (not timestamps) for missing detection | Timestamps drop rows at rounding boundaries (Airbyte #84400 Q013) |
| Bounded reassembly + gap NACK | Counters memory-exhaustion DoS (Q029 documented problem) |
| Idempotent apply key `(dataset, OpRef, VersionPos)` + dedup window | Exactly-once effect over at-least-once carrier delivery (Q011, Q012 ESTABLISHED) |
| Bounded exponential backoff with jitter + retry budget | Prevents retry storms (Q014, Q026 ESTABLISHED) |
| All traffic via `ids-carrier::Endpoint` only | Carrier-agnostic absolute rule (CRATES.md §9-11) |
| Crypto: ed25519 signatures, sha256/blake3 hashing, XChaCha20-Poly1305/chacha20poly1305 AEAD | Only allowed primitives (CRATES.md §3) |
| `ids-crypto` type surface only, no runtime cycle | Preserves rule that `ids-crypto` does not depend on `ids-000-aa` |
| satura­ting arithmetic on all positions | No overflow panics in distributed versioning |
| Cargo workspace member at `C:/idfs/ids-replication/` | Matches existing layout (`ids-000-aa` at `C:/idfs/ids-000-aa/`) |

---

### 12. Sources

1. Mohan et al. — ARIES: Algorithm for Recovery and Isolation Exploiting Semantics (Stanford cs345d, 1992). WAL analysis/redo/undo, LSN. https://web.stanford.edu/class/cs345d-01/rl/aries.pdf
2. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
3. RFC 8032 — EdDSA / ed25519. https://www.rfc-editor.org/rfc/rfc8032
4. RFC 7539 / RFC 8439 — ChaCha20-Poly1305 AEAD. https://www.rfc-editor.org/rfc/rfc7539
5. Libsodium — XChaCha20-Poly1305 construction (192-bit nonce, random nonces). https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
6. IETF draft-irtf-cfrg-xchacha-03 — eXtended-nonce ChaCha. https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03
7. BLAKE3 specification. https://github.com/BLAKE3-team/BLAKE3
8. Borg backup — BLAKE3-256 store hash (content-addressed objects). https://raw.githubusercontent.com/borgbackup/borg/master/docs/internals/data-structures.rst
9. Apache DistributedLog — detail design, versioned log updates. https://apache.github.io/distributedlog/design/main.html
10. BtrLog — Low-Latency Logging for Cloud Database Systems (monotonic LSN). https://arxiv.org/pdf/2606.27051
11. VLDB — logging with monotonically increasing log sequence numbers (p3745-xu). https://vldb.org/pvldb/vol17/p3745-xu.pdf
12. DBLog — verified change-data-capture contract (arxiv 2609.08160); theoretical study (arxiv 2605.31475). https://arxiv.org/html/2609.08160v1
13. YugabyteDB — CDC architecture; LSN replication slots. https://docs.yugabyte.com/stable/architecture/docdb-replication/change-data-capture
14. TiCDC — multi-writer change data capture architecture. https://pingcap.com/blog/multi-writer-change-data-capture-cdc-architecture-ticdc-tidb
15. Cornell CS530 — replication approaches lecture (log-based recovery, hot standby, logical). https://cs.cornell.edu/courses/cs530/2004sp/lect/Lec15.pdf
16. Oracle Streams — high-performance asynchronous replication (IEEE). https://ieeexplore.ieee.org/document/4812537
17. RFC 2960 — SCTP sequenced delivery within streams. https://docbox.etsi.org/Reference/IETF/RFC/RFC2960.pdf
18. CMU 17-350 — CRDT lectures: version vectors, convergence testing. https://cs.cmu.edu/~aldrich/courses/17-350/slides/11-testing.pdf
19. arxiv 1511.05010 — Eventually Consistent Register Revisited (causality + convergence). https://arxiv.org/pdf/1511.05010v1
20. Wikipedia — Version vector mechanism. https://en.wikipedia.org/wiki/Version_vector
21. COPS — causal+ consistency for wide-area storage (SOSP'11). https://cs.princeton.edu/~mfreed/docs/cops-sosp11.pdf
22. Apache Pulsar PIP-81 — split individual/cumulative acks into ledger entries. https://github.com/apache/pulsar/wiki/PIP-81%3A-Split-the-individual-acknowledgments-into-multiple-entries
23. NATS JetStream — streams, consumers, delivered/acked state, exactly-once delivery. https://docs.nats.io/concepts/jetstream.md
24. Oxia — FollowerCursor (LastPushed/AckOffset per follower). https://pkg.go.dev/github.com/oxia-db/oxia/oxiad/dataserver/controller/lead
25. Google Cloud SQL — replication lag. https://docs.cloud.google.com/sql/docs/postgres/replication/replication-lag
26. AWS RDS — diagnose/resolve replica lag. https://aws.amazon.com/blogs/database/diagnose-and-resolve-replica-lag-in-amazon-rds-for-oracle-replicas-part-2
27. Confluent — Kafka exactly-once semantics; delivery guarantees; consumer offsets. https://confluent.io/blog/exactly-once-semantics-are-possible-heres-how-apache-kafka-does-it
28. Kafka design — at-most-once / at-least-once / exactly-once. https://kafka.apache.org/41/design/design
29. Azure Architecture Center — Idempotent Consumer pattern. https://learn.microsoft.com/en-us/azure/architecture/patterns/idempotent-consumer
30. Machine-Checked Dual-Write Recovery (Isabelle/HOL) — dedup window shelf life, expiry as duplicate-admission. https://arxiv.org/html/2608.00501v5
31. AWS Architecture Blog — Exponential Backoff and Jitter. https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter
32. Akka Streams — boundedness / backpressure. https://doc.akka.io/libraries/akka-core/current/stream/stream-flows-and-basics.html
33. Raft — In Search of an Understandable Consensus Algorithm (log replication, InstallSnapshot). https://raft.github.io/raft.pdf
34. etcd-io/raft — replicated state machine via replicated log. https://github.com/etcd-io/raft/blob/v3.7.0/doc.go
35. PostgreSQL — log-shipping standby servers / streaming replication. https://postgresql.org/docs/current/warm-standby.html
36. SQL Server — transaction log architecture (active tail, checkpoints). https://learn.microsoft.com/en-us/sql/relational-databases/sql-server-transaction-log-architecture-and-management-guide
37. happy.engineering — crash recovery: snapshot + WAL replay + torn-tail truncation. https://happy.engineering/topics/how-it-works/recovery
38. MIT 6.824 — Raft notes (SMR, partition handling, resume). http://nil.csail.mit.edu/6.824/2017/notes/l-raft.txt
39. quinn-proto RUSTSEC-2026-0185 — stream reassembly memory exhaustion DoS. https://rustsec.org/advisories/RUSTSEC-2026-0185.html
40. IDS prompts.txt entry 17 (verbatim) + CRATES.md §17. C:/idfs/prompts.txt lines 250-264; C:/idfs/CRATES.md lines 152-154
41. HX-241 / IDS reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md; C:/idfs/plans/plan-012.md, plan-020.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Decisions, Sources
42. Cargo workspaces. https://doc.rust-lang.org/cargo/reference/workspaces.html

---

**End of plan-017.md. Research complete — 30 targeted searches executed. No code written, no files created, no repository changes made beyond this plan document and its research findings files (FINDINGS-017.md, results-017.md).**
