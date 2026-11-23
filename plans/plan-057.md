## Phase 057 — Create the ids-commit-state crate: Write state: accepted/ordered/executing/committed/replicating/synchronized/verified/failed.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation. Research only — no code, no source modifications.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 850-864)

```
57. Research only do not read plan.md **[Phase 057 — Create the ids-commit-state crate: Write state: accepted/ordered/executing/committed/replicating/synchronized/verif]**

Crate: ids-commit-state
Purpose: Write state: accepted/ordered/executing/committed/replicating/synchronized/verified/failed.
Contents: Commit state transitions, durability conditions, commit prefix tracking, gap detection, abort semantics, state persistence, state recovery.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 057 — Create the ids-commit-state crate: Write state: accepted/ordered/executing/committed/replicating/synchronized/verif]**

Create plan-057.md in C:/idfs/plans/ for the ids-commit-state crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 057 creates `ids-commit-state`, a new standalone IDS crate (separate crate, not folders inside an existing one) that owns the end-to-end write commit state machine: **accepted → ordered → executing → committed → replicating → synchronized → verified → failed**. The crate governs commit state transitions, durability conditions, commit prefix tracking, gap detection, abort semantics, state persistence, and state recovery.

**Relationship to existing types:** `ids-000-aa` (master crate) already defines two relevant enums:
- `OpState`: received → ordered → executing → committed → synchronized → verified → failed (IDS-A §42, CAPABILITIES §61), with `can_transition_to` enforcing a linear progression.
- `CommitState`: accepted → prepared → committed → aborted (a transaction-level commit protocol enum).

`ids-commit-state` builds on these canonical types without redefining them. Its primary state machine follows the `OpState` progression, extended with `replicating` between `committed` and `synchronized`, and `accepted` as an entry alias for received/accepted writes. `Committed` and `Aborted` are terminal states per the master-crate `CommitState` contract; `Verified` and `Failed` are terminal per `OpState`.

The crate is carrier-agnostic: it never opens sockets, never references `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh, and ships every commit-state signal as an opaque `Vec<u8>` datagram to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-000-aa`/`ids-crypto` primitive IDs and type surface. `ids-000-aa` remains the master crate holding every canonical type and sealed trait contract; `ids-commit-state` references them and never redefines them.

Research basis: 30 parallel searches returned Raft log matching + commit index (committed entries = prefix with no holes; `lastApplied`/`commitIndex` cursor), Two-Phase Commit (voting + commit/abort phases; stable-storage write-ahead log; blocking on coordinator failure), Three-Phase Commit (pre-commit buffer enabling non-blocking termination), Paxos/MultiPaxos quorum commit (majority persistence before commit; log compaction via snapshots + suffix replay), PBFT (pre-prepare/prepare/commit phases; total order across views), Zab (atomic broadcast; prefix preservation; leader completeness), chain replication (head→tail propagation; commit at tail), Kafka high watermark / LEO / ISR (committed = replicated to all ISR; min.insync.replicas), CRAQ (chain with dirty→clean version commit at tail), HotStuff/LibraBFT (3-chain commit rule; quorum certificates), FLEET (commitIndex hole-free prefix; gap detection via holes in log array), dual-write recovery impossibility (crash between two durable systems; acceptance-record fences), checkpoint-based replica recovery (snapshot + log suffix; garbage-collect covered prefix), Bayesian/Bayou anti-entropy (commit sequence numbers; prefix property; omitted sequence number for truncated log detection), deterministic commit rule over unstructured DAGs, fault-injection transition rules (failure→commit or abort depending on state), and commit barrier durability (fsync/fdatasync group commit; commit boundary = syscall return determines durable or not).

---

### 2. Architecture Overview

`ids-commit-state` sits between the local store / replication layer and the acknowledgement layer. It is a deterministic, local state machine: given a write's current state and an incoming event, it computes the next state, the durability conditions that must hold, and whether the write may advance. Every outbound commit-state signal is an opaque `Vec<u8>` handed to `ids-carrier::Endpoint`; the crate never addresses a peer by network identity.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-store                          │
│  per-write commit state (accepted → ordered → … → verified/failed)  │
├─────────────────────────────────────────────────────────────────────┤
│                 Phase 057: ids-commit-state (NEW CRATE)             │
│  ┌─────────────────┐ ┌──────────────┐ ┌─────────────────────┐   │
│  │ state transitions│ │ durability   │ │ commit prefix + gap │   │
│  │ (8-state FSM)    │ │ conditions   │ │ detection / repair  │   │
│  └─────────────────┘ └──────────────┘ └─────────────────────┘   │
│  ┌──────────────────┐ ┌─────────────────────────────────────┐   │
│  │ abort semantics  │ │ state persistence + recovery        │   │
│  │ (terminal Aborted│ │ (snapshot + WAL suffix replay)      │   │
│  │  and Failed)     │ └─────────────────────────────────────┘   │
│  └──────────────────┘                                           │
├─────────────────────────────────────────────────────────────────────┤
│            ids-carrier::Endpoint (CarrierType + Vec<u8>)            │
│  all outbound commit-state signals as opaque byte datagrams         │
├─────────────────────────────────────────────────────────────────────┤
│             ids-000-aa (master crate, canonical types)              │
│  NodeId, RegionId, IdsTime, CarrierType, RecordId, Checksum,        │
│  OpState, CommitState, sealed trait contracts, error types          │
└─────────────────────────────────────────────────────────────────────┘
```

**Data flow of a commit-state transition:**
1. Local store submits a write; `ids-commit-state` assigns state `Accepted` and a commit-prefix position.
2. On ordering evidence (quorum ack / ordering position), transition `Accepted → Ordered`.
3. On execution start, `Ordered → Executing`; on durable execution result, `Executing → Committed`.
4. `Committed → Replicating` once the write is accepted for replication to peers.
5. `Replicating → Synchronized` once a quorum/regional set holds a durable copy (durability barrier crossed).
6. `Synchronized → Verified` once integrity verification (checksum) passes.
7. Any transition may go `→ Failed` on unrecoverable error; `Failed` and `Verified` are terminal.
8. Result (`CommitStateTransition` + optional `CommitPrefixReport`) serialized and handed to `ids-carrier::Endpoint` as opaque `Vec<u8>` bytes; the carrier decides delivery.

**Carrier-agnostic absolute rule:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere in the crate. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

**Crypto rule:** only ed25519 (signatures), xchacha20poly1305/chacha20poly1305 (AEAD), sha256/blake3 (content hashing) — consumed from `ids-000-aa`/`ids-crypto` primitive IDs; no other cryptographic primitives.

---

### 3. Detailed Research Dimensions

#### 3.1 Commit state machine (8 states)

The FSM: `accepted → ordered → executing → committed → replicating → synchronized → verified → failed`, with `failed` reachable from any non-terminal state, and `verified`/`failed` terminal. This extends the master-crate `OpState` (received→ordered→executing→committed→synchronized→verified→failed) by inserting `accepted` as an entry state (received+accepted alias) and `replicating` between `committed` and `synchronized`.

- **Accepted:** write received and admitted; a commit-prefix position is assigned. No durability claim yet.
- **Ordered:** write has a total order position relative to other writes (ordering consensus / sequencing layer).
- **Executing:** write is being applied to local state; tentative effects isolated until commit.
- **Committed:** write is durably decided (quorum commit / commit index reached); effects are now irrevocable.
- **Replicating:** write is being propagated to peer replicas; not yet durably held by a quorum.
- **Synchronized:** durable stable-storage barrier crossed on a quorum/regional set; survives single-node power loss.
- **Verified:** integrity check (checksum) passed on the durable copy; terminal success state.
- **Failed:** unrecoverable error at any stage; terminal failure state.

#### 3.2 Durability conditions

Durability is not one condition but two separable boundaries (Oracle Berkeley DB model: acknowledgment guarantees + synchronization guarantees):
- **Commit boundary:** logical acceptance + ordering + quorum decision (the write is irrevocably decided).
- **Synchronization boundary:** durable stable-storage barrier (fsync/fdatasync/WAL-flush + parent-directory sync for new files) reached before ack.

PostgreSQL `synchronous_commit` (remote_apply/on/remote_write/local/off) and `wal_sync_method` (fdatasync/fsync/fsync_writethrough/open_sync/open_datasync) demonstrate these as independent knobs. `ids-commit-state` tracks both independently per write: `commit_reached` (bool) and `sync_reached` (bool); a write is considered durable only when both hold. Exposure window between commit and synchronization is reported honestly (ack at commit only = survives clean shutdown, not power loss).

fsync vs fdatasync: fsync flushes data + metadata (size, block mapping, timestamps); fdatasync flushes data plus only metadata needed for a subsequent read. New-file durability trap: fsync the parent directory after creating any new commit-state file (the directory entry linking name→inode must be durable too). CRC/checksum (blake3 preferred; sha256 alternative) over durable metadata + payload to detect silent corruption/torn writes.

#### 3.3 Commit prefix tracking

The commit prefix is the hole-free contiguous prefix of the commit log known to be committed and applied. Research foundations:
- **Raft log matching property:** if two logs contain an entry with the same index and term, they are identical through that index; `commitIndex` cursor ensures the prefix before it is hole-free; entries between `lastApplied` and `commitIndex` are applied sequentially. Source: Raft paper §5.3; etcd raft `commitIndex`/`lastApplied`.
- **FLEET:** commitIndex as hole-free prefix cursor; log entries may arrive out of order creating holes; committed entries maintain non-decreasing term values.
- **Kafka high watermark (HW):** offset up to which all ISR replicas have replicated; consumers read only up to HW; HW = min LEO across ISR, only advances when ISR size ≥ min.insync.replicas. Log End Offset (LEO) may exceed HW for unreplicated entries.
- **Prefix property (Bayou):** if replica R holds write W_i accepted by server X, R also holds all writes X accepted before W_i. Commit sequence numbers (CSN) totally order committed writes.
- **Omitted sequence number (OSN):** detects truncated stable writes missing from a peer (OSN_A > CSN_B ⇒ full database transfer needed).

`ids-commit-state` maintains `commit_prefix = { first_index, last_index, term }` per write set, with `gap_start`/`gap_end` for detected holes. A write can advance past `committed` only if its index lies within the hole-free prefix.

#### 3.4 Gap detection

Gaps (missing log entries / holes) arise from out-of-order delivery, replica lag, truncation, or crash. Gap detection mechanisms:
- **Index/term continuity check:** each entry carries `(index, term)`; a missing predecessor index ⇒ gap. Raft AppendEntries consistency check: leader includes prevIndex/prevTerm; follower rejects if mismatch.
- **CommitIndex watermark:** entries beyond `commitIndex` are uncommitted; a hole between `lastApplied` and `commitIndex` blocks sequential application until filled.
- **OSN vs CSN comparison:** anti-entropy detects when one side truncated stable writes the other hasn't seen.
- **ISR tracking:** replicas below the high-watermark lag; gap = HW − replica_LEO.

Gap repair: leader (or any peer holding the missing entries) re-sends the missing suffix; if the sender truncated entries the receiver needs (OSN > receiver CSN), fall back to full state transfer (snapshot + log suffix replay). `ids-commit-state` exposes `GapReport { missing_indices: Vec<u64>, severity: GapSeverity, repair_action: RepairAction }`.

#### 3.5 Abort semantics

Abort = all-or-nothing rollback; once a write aborts, all tentative effects are discarded and the state reverts.
- **2PC abort:** any participant votes No, or coordinator/participant timeout ⇒ global abort; participants undo via undo log, release locks, send ack; coordinator logs abort to stable storage. Failure transition rule: for every non-final state, if it contains a commit, assign failure→commit; else failure→abort.
- **3PC abort:** pre-commit buffer lets survivors safely abort when no participant is pre-committed; non-blocking termination.
- **Terminal abort states:** master-crate `CommitState::Aborted` and `OpState::Failed` are terminal; a failed write never retries implicitly — retry is an explicit new write with a new accept position.
- **Abort visibility:** aborted writes are never exposed to readers (cf. CRAQ: dirty versions not readable until tail commits/cleans).

`ids-commit-state` defines `AbortReason { VoteNo, Timeout, CoordinatorFailure, DurabilityBarrierFailed, ChecksumMismatch, GapUnrecoverable, InvalidTransition }` and an `AbortRecord { write_id, reason, aborted_at, rollback_scope }` persisted to stable storage before the `Failed`/`Aborted` transition is final.

#### 3.6 State persistence

Persistence model: **snapshot + WAL suffix** (checkpoint-based replica recovery):
- Periodic full snapshots of commit-state (serialized state machine + commit prefix) atomically replaced (write temp + rename + fsync + parent-dir fsync).
- Append-only WAL of commit-state transitions; each transition fsynced per `SyncPolicy::{Always, EveryN, OsManaged}` before acknowledged durable.
- Recovery: load latest valid snapshot (largest index, fall back if corrupt), replay WAL suffix from `index+1`.
- WAL truncation: garbage-collect prefix covered by a checkpoint after checkpoint is durable (VR Revisited: garbage-collect log prefix covered by checkpoint to avoid divergence).
- Snapshot file naming with length-prefix / strict-prefix rejection so a truncated prefix never decodes (arachne-kv pattern).

Persistence tiers (per write/dataset): `MemoryOnly`, `Journaled` (WAL appended), `Synced` (fsync/fdatasync crossed), `SyncedAndReplicated` (synced + quorum copies). Exposure window reported per tier.

#### 3.7 State recovery

Recovery paths:
- **Clean restart:** load snapshot + replay WAL suffix; rebuild commit prefix and in-flight write states; reject transitions that violate `can_transition_to`.
- **Crash mid-transition:** WAL records the in-doubt transition; on recovery, if a transition lacks its durability barrier record, treat as `Failed` (conservative) or replay to completion if the barrier record exists.
- **Replica rejoining:** peer reports its commit prefix (`commit_index`, `term`); leader detects divergence (prefix mismatch ⇒ truncate conflicting suffix and resend; Raft nextIndex backoff). If sender truncated needed entries (OSN > receiver CSN), full state transfer.
- **Verify on recovery:** recompute checksums over recovered state; mismatch ⇒ `Failed` state for affected writes, operator intervention.

Recovery is deterministic: same snapshot + same WAL suffix ⇒ same state. No reliance on wall-clock for validity (HLC timestamps only as bounded-skew hints, never authority).

---

### 4. Threat Model & Mitigations

| Threat | Impact | Mitigation |
|--------|--------|------------|
| Correlated failure (shared power loss, rack outage) destroys all synced copies | Permanent data loss despite "committed" | Regional redundancy + failure-domain anti-affinity; honest exposure-window reporting per tier |
| Ack before stable-storage barrier | Acknowledged writes lost on crash | Separate commit vs. sync boundaries; `Synced` tier forces fsync/fdatasync + parent-dir sync before `Synchronized` |
| Silent data corruption (bit rot, torn writes) | Corrupt state treated as durable/verified | blake3/sha256 checksum over durable metadata + payload; verification before `Verified`; mismatch → `Failed` |
| New commit-state file created but directory entry not durable | File "exists" after crash but name unresolvable | fsync parent directory after creating any new state file |
| Gap between commit prefix and applied state | Blocked or divergent application | Hole-free `commitIndex` cursor; gap detection via index/term continuity + OSN/CSN; repair by resend or full state transfer |
| Indefinite commit blocking (quorum unavailable) | Write availability loss | Bounded wait; `Undetermined`-like verdict via `Failed` on timeout; caller retry policy |
| Invalid state transition (state machine corruption) | Divergent replica states | `can_transition_to` guard enforced on every transition; terminal states never exit |
| Crypto downgrade / non-allowed primitive | Violates allowed-crypto constraint | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 from ids-000-aa/ids-crypto; lint to reject others |
| Carrier assumption (TCP/IP etc.) | Violates carrier-agnostic constraint | All traffic via `ids-carrier::Endpoint`; compile-fail friendly (no `std::net`/`tokio::net`) |

---

### 5. Integration with Existing Crates

- **ids-000-aa (master crate):** source of all canonical types and sealed trait contracts — `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpState`, `CommitState`, sealed traits, error types. `ids-commit-state` references `OpState`/`CommitState` and defines no new canonical identifier types. Sealed-trait pattern (Rust API Guidelines C-SEALED, `private::Sealed`) used for any extension trait this crate defines.
- **ids-carrier:** all outbound commit-state signals flow as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no direct transport dependency.
- **ids-crypto:** allowed primitives (ed25519 signatures, xchacha20poly1305/chacha20poly1305 AEAD, sha256/blake3 hashing) consumed by primitive ID; `ids-commit-state` does not implement crypto itself.
- **ids-store / local store:** the commit-state engine is queried by the local store before acknowledging a write; it is a pure transition function (current state + event → next state + durability conditions) with no store implementation dependency.
- No dependency on `std::net`, `tokio::net`, libp2p, or any networking crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

A separate crate `ids-commit-state` at `C:/idfs/ids-commit-state/`, organized into subfolders by state-machine dimension (not a monolithic file). The enumerated tree below totals ~130 files; property-based/chaos/conformance test generation scales toward 1,000 files.

```
ids-commit-state/
├── Cargo.toml
├── README.md
├── LICENSE
├── CONTRACTS.md
├── SECURITY.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── types.rs               # CommitStateTransition, CommitPrefix, GapReport, AbortRecord, SyncPolicy, PersistenceTier
│   ├── fsm/                   # 8-state commit state machine (~15 files)
│   │   ├── mod.rs, states.rs, transitions.rs, transition_rules.rs
│   │   ├── accepted.rs, ordered.rs, executing.rs, committed.rs
│   │   ├── replicating.rs, synchronized.rs, verified.rs, failed.rs
│   │   ├── terminal.rs, can_transition.rs, opstate_mapping.rs, commitstate_mapping.rs
│   ├── durability/            # durability conditions (~12 files)
│   │   ├── mod.rs, commit_boundary.rs, sync_barrier.rs, fsync_barrier.rs
│   │   ├── exposure_window.rs, sync_policy.rs, parent_dir_sync.rs, crc_check.rs
│   ├── prefix/                # commit prefix tracking (~12 files)
│   │   ├── mod.rs, commit_prefix.rs, commit_index.rs, term_tracker.rs
│   │   ├── hole_free_cursor.rs, last_applied.rs, prefix_report.rs
│   ├── gap/                   # gap detection (~12 files)
│   │   ├── mod.rs, gap_detector.rs, gap_report.rs, gap_repair.rs
│   │   ├── continuity_check.rs, os_n_csn.rs, isr_lag.rs, full_transfer.rs
│   ├── abort/                 # abort semantics (~10 files)
│   │   ├── mod.rs, abort_reason.rs, abort_record.rs, rollback_scope.rs
│   │   ├── terminal_states.rs, retry_policy.rs
│   ├── persistence/           # state persistence (~15 files)
│   │   ├── mod.rs, snapshot.rs, wal.rs, atomic_write.rs
│   │   ├── checkpoint.rs, recovery.rs, truncation.rs, snapshot_name.rs
│   │   ├── persistence_tier.rs, sync_writer.rs, durability_tracker.rs
│   ├── recovery/              # state recovery (~12 files)
│   │   ├── mod.rs, clean_restart.rs, crash_recovery.rs, replica_rejoin.rs
│   │   ├── divergence_repair.rs, verify_on_recovery.rs, deterministic_replay.rs
│   ├── carrier/               # carrier-agnostic output (~8 files)
│   │   ├── mod.rs, endpoint.rs, datagram.rs, commit_signal.rs
│   │   ├── opaque_bytes.rs, carrier_type.rs
│   ├── crypto/                # allowed-primitive wrappers (~10 files)
│   │   ├── mod.rs, ed25519.rs, xchacha20poly1305.rs, sha256.rs, blake3.rs, chacha20poly1305.rs, primitive_ids.rs
│   └── tests/                 # unit / property / chaos / conformance (~30+ files)
│       ├── unit/, property/, chaos/, conformance/
├── benches/                   # commit-throughput / exposure-window benchmarks
├── tests/
└── .cargo/config.toml
```

Carrier-agnostic constraint enforced structurally: the `src/` tree has no transport modules and no `std::net`/`tokio::net` references; every outbound message is typed as `Vec<u8>` addressed to `ids-carrier::Endpoint`.

---

### 7. Key Algorithms & Data Structures

**`CommitStateTransition { write_id, from, to, event, commit_prefix, durability_conditions, timestamp }`** — result of evaluating one transition.

**8-state FSM with `can_transition_to` guard:**
- `Accepted → {Ordered, Failed}`
- `Ordered → {Executing, Failed}`
- `Executing → {Committed, Failed}`
- `Committed → {Replicating, Failed}`
- `Replicating → {Synchronized, Failed}`
- `Synchronized → {Verified, Failed}`
- `Verified → {}` (terminal)
- `Failed → {}` (terminal)

**Durability conditions:** `{ commit_reached: bool, sync_reached: bool, barrier_crossed: Instant-or-None, exposure_window_ms: u64 }`; durable iff both bools true.

**Commit prefix:** `CommitPrefix { first_index: u64, last_index: u64, term: u64, hole_free: bool }`; advances only when the new index extends the hole-free prefix.

**Gap detection:** `GapReport { missing_indices: Vec<u64>, severity: GapSeverity, repair_action: Resend | FullStateTransfer }`; detected via `(index, term)` continuity mismatch or `OSN > receiver CSN`.

**Abort semantics:** `AbortRecord { write_id, reason, aborted_at, rollback_scope }` persisted before `Aborted`/`Failed` becomes final; terminal states never transition out.

**Persistence:** snapshot (atomic replace + fsync + parent-dir fsync) + append-only WAL (per-transition fsync per `SyncPolicy`); recovery = latest valid snapshot + WAL suffix replay; truncate prefix covered by durable checkpoint.

**Content integrity:** blake3/sha256 over durable metadata + payload per commit entry; verified before `Synchronized → Verified`.

**Allowed crypto:** ed25519 (durable-record signatures), xchacha20poly1305/chacha20poly1305 (AEAD for payloads at rest), sha256/blake3 (content hashing). No other primitives.

**Sealed traits:** any extension trait sealed via `private::Sealed` supertrait (Rust API Guidelines C-SEALED).

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (NodeId, RegionId, IdsTime, CarrierType, RecordId, Checksum, OpState, CommitState) + sealed trait contracts + error types |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — sole traffic channel |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |
| `tokio` | Async runtime (test harness only; no `tokio::net` in crate code) |

**No networking crates:** `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS forbidden — all traffic via `ids-carrier::Endpoint`. **Allowed crypto only:** ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| 8-state FSM | accepted → ordered → executing → committed → replicating → synchronized → verified → failed, with failed reachable from any non-terminal state | unit test enumerating all transitions |
| Terminal states | Verified and Failed never transition out; can_transition_to returns false | unit test on terminal states |
| Durability conditions | commit_reached ∧ sync_reached required for Synchronized; exposure window reported per tier | boundary-separator unit test; power-loss fault injection |
| fsync + parent-dir sync | fsync + parent-directory sync before ack for new state files | unit test + fsync-failure injection |
| Commit prefix | hole-free prefix cursor; commitIndex advances only with contiguous entries | property test over index/term sequences |
| Gap detection | missing index between lastApplied and commitIndex ⇒ GapReport; repair = resend or full transfer | unit test with synthetic holes; OSN/CSN truncation test |
| Abort semantics | Aborted/Failed terminal; AbortRecord persisted before final; rollback scope honored | unit test per AbortReason |
| State persistence | snapshot + WAL suffix replay reproduces identical state | deterministic recovery test (same snapshot+WAL ⇒ same state) |
| State recovery | crash mid-transition ⇒ conservative Failed or replay-to-completion | chaos test: kill at each transition point |
| Content integrity | blake3/sha256 over metadata+payload; tampered entry rejected before Verified | manifest verification unit test |
| Crypto constraint | only allowed primitives | dependency/cargo audit + primitive-ID allowlist test |
| Carrier-agnostic | compiles with zero std::net/tokio::net/libp2p refs; all traffic via ids-carrier::Endpoint | build lint + dependency scan |
| Property tests | 100,000+ iterations | proptest/bolero |
| Chaos tests | partition, offline member, replica churn, kill-at-transition | pass/fail |

---

### 10. Remaining Uncertainties

1. **State count reconciliation:** The prompt lists 8 states (accepted/ordered/executing/committed/replicating/synchronized/verified/failed) while `ids-000-aa` has `CommitState` (accepted/prepared/committed/aborted) and `OpState` (received/ordered/executing/committed/synchronized/verified/failed). The plan maps `accepted` to an entry alias of `received/accepted` and inserts `replicating` between `committed` and `synchronized`; the exact mapping table must be confirmed against IDS-A §42 / CAPABILITIES §61 during implementation.
2. **Commit prefix vs. OpState coupling:** whether commit-prefix tracking is per-write or per-write-set, and whether `replicating` implies quorum or single-peer replication, is product-configured, not research-determined.
3. **fsync failure handling:** a failed fsync poisons the writer (durability crate); recovery path (restart from WAL vs. fail the dataset) needs an operator decision.
4. **Wire format for commit-state signals:** CBOR vs bincode vs protobuf for the opaque `Vec<u8>` handed to `ids-carrier::Endpoint`; CBOR favored for compactness/schema evolution but crate TBD.
5. **WAL truncation policy:** retention (size/age/ack-based) deferred; unbounded WAL growth without checkpoint-compaction.
6. **Formal verification scope:** TLA+/PlusCal spec proving commit-state safety (no divergent committed states, terminal-state integrity) is additional effort beyond this plan.
7. **Abort retry semantics:** whether a `Failed` write may be retried as a new `Accepted` write with a fresh prefix position, or whether the original write_id is permanently dead — policy decision.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-commit-state` | Follows "separate crates instead of more folders in existing ones"; clean dependency boundary |
| Build on `ids-000-aa` `OpState`/`CommitState` | Master crate already holds canonical state enums; never redefine canonical types |
| 8-state FSM with `replicating` between `committed` and `synchronized` | Matches prompt verbatim; separates irrevocable commit from durable replication barrier |
| Commit vs. sync as separate boundaries | Oracle Berkeley DB model: durability = acknowledgment guarantees + synchronization guarantees; prevents overclaiming |
| Hole-free commit prefix (`commitIndex` cursor) | Raft log matching + FLEET; prevents applying across gaps |
| Gap detection via index/term continuity + OSN/CSN | Raft AppendEntries consistency + Bayou anti-entropy; detects truncation mismatches |
| Abort terminal + AbortRecord persisted first | 2PC stability (once commit/abort, remains forever); rollback scope recorded before final |
| Snapshot + WAL suffix persistence | Checkpoint-based replica recovery (VR Revisited); bounded recovery work |
| Parent-directory fsync after new state file | New-file durability trap (kernel internals; PostgreSQL/LevelDB practice) |
| blake3 preferred, sha256 allowed | Content hashing/manifest integrity (spacedb-durability pattern); within allowed-crypto set |
| ed25519 signatures for durable commit records | Non-repudiation of commit-state transitions; within allowed-crypto set |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic mandate; no `std::net`/`tokio::net`/libp2p |
| Research only — no code, no source modifications | Per task constraint; this document is the implementation blueprint |

---

### Sources

- Raft — In Search of an Understandable Consensus Algorithm (log matching, commitIndex, lastApplied): https://raft.github.io/raft.pdf ; https://raft.github.io/
- etcd raft (`commitIndex`/`lastApplied`, apply committed entries in order): https://github.com/etcd-io/raft/blob/main/raft.go
- Two-Phase Commit (voting, commit/abort, stable-storage WAL, blocking): https://en.wikipedia.org/wiki/Two-phase_commit_protocol ; https://cs186berkeley.net/notes/note16
- Three-Phase Commit (pre-commit buffer, non-blocking termination): https://en.wikipedia.org/wiki/Three-phase_commit_protocol ; https://pk.org/assets/files/rutgers-417-notes/08b-3pc-slides.pdf
- Paxos Commit (Lamport; 2F+1 coordinators, F+1 working): https://lamport.azurewebsites.net/video/consensus-on-transaction-commit.pdf
- MultiPaxos Made Complete (quorum commit, log compaction via snapshots): https://arxiv.org/html/2405.11183
- PBFT (pre-prepare/prepare/commit, total order, deterministic replicas): https://css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf
- Zab — High-performance broadcast for primary-backup systems (prefix preservation, leader completeness): https://ieeexplore.ieee.org/document/5958223
- Chain replication (head→tail, commit at tail): https://nil.csail.mit.edu/6.824/2020/notes/l-craq.txt ; https://timilearning.com/posts/mit-6.824/lecture-9-craq
- CRAQ (dirty→clean version commit at tail): https://cs.princeton.edu/courses/archive/fall15/cos518/studpres/craq.pdf
- HotStuff/LibraBFT SMR (3-chain commit rule, quorum certificates): https://developers.diem.com/papers/diem-consensus-state-machine-replication-in-the-diem-blockchain/2019-10-24.pdf
- FLEET (commitIndex hole-free prefix, out-of-order holes, non-decreasing terms): https://vldb.org/pvldb/vol18/p1522-fan.pdf
- Kafka high watermark / LEO / ISR / min.insync.replicas: https://developer.confluent.io/courses/architecture/data-replication ; https://docs.confluent.io/kafka/design/replication.html ; https://cwiki.apache.org/confluence/x/mpOzDw
- Machine-Checked Dual-Write Recovery from a Commit Log (acceptance-record fences, truncation dilemma): https://arxiv.org/html/2608.00501v5
- VR Revisited — Checkpoint-Based Replica Recovery (snapshot + log suffix, garbage-collect covered prefix): https://jack-vanlightly.com/analyses/2023/01/22/paper-vr-revisited-checkpoint-based-replica-recovery-part-6
- Bayou anti-entropy (commit sequence numbers, prefix property, omitted sequence number): https://people.cs.umass.edu/~arun/cs677/notes/Bayou.pdf ; https://pages.cs.wisc.edu/~thanhdo/qual-notes/ds/ds6-anti-entropy.txt
- A Deterministic Commitment Rule over Unstructured DAGs: https://arxiv.org/html/2610.41837
- Fault-injection transition rules (failure→commit or abort by state): https://courses.cs.vt.edu/cs5204/fall11-kafura/Presentations/CommitProtocols.pdf
- Commit barrier durability (Barrier + Durability; fsync commit boundary): https://arxiv.org/html/2609.17771 ; https://arxiv.org/html/2603.01384 (commit boundary definition)
- fsync vs fdatasync, new-file directory durability trap: https://kernel-internals.org/io/fsync-fdatasync
- Group commit self-clocks (flush batching trade-off): https://arxiv.org/html/2606.18187
- Execute-verify SMR (agree-then-execute vs execute-then-verify): https://usenix.org/sites/default/files/conference/protected-files/kapritsos_osdi12_slides.pdf
- Stateful supervised state machine (Issue/Verify journal, terminal faults): https://pkg.go.dev/github.com/open-ships/statemachine/supervised
- RustCrypto ChaCha20Poly1305 / XChaCha20Poly1305 (RFC 8439): https://docs.rs/chacha20poly1305/latest/chacha20poly1305
- Rust API Guidelines — sealed traits (C-SEALED, `private::Sealed`): https://rust-lang.github.io/api-guidelines/future-proofing.html
- arachne-kv snapshot (atomic replace, strict-prefix rejection): https://docs.rs/arachne-kv/latest/src/arachne_kv/storage/snapshot.rs.html
- IDS Dataspace Protocol / IDS Policies: https://dobachi.github.io/memo-blog/2023/08/31/IDS-Dataspace-Protocol/ ; https://kb.internationaldataspaces.org/external/rulebook/105_Policies/

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. This plan document defines all architecture, dimensions, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 057 (ids-commit-state).
