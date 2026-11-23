## Phase 065 — Create the ids-mvcc crate: Snapshot isolation for reads/writes. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 970-984)

```
65. Research only do not read plan.md **[Phase 065 — Create the ids-mvcc crate: Snapshot isolation for reads/writes.]**

Crate: ids-mvcc
Purpose: Snapshot isolation for reads/writes.
Contents: Version management, snapshot reads, write visibility, garbage collection of old versions, transaction isolation levels, phantom read prevention, serialization anomaly prevention.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 065 — Create the ids-mvcc crate: Snapshot isolation for reads/writes.]**

Create plan-065.md in C:/idfs/plans/ for the ids-mvcc crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 065 creates `ids-mvcc`, the IDS crate that implements snapshot isolation for reads and writes as a separate crate (not a folder inside an existing crate, per the project rule). It is a concurrency-control layer: every write produces a new version tagged with a commit timestamp / transaction ID; reads are served from a stable snapshot taken at transaction start and never block writers; writers conflict only on write-write to the same key and abort the later committer (first-committer-wins). The crate owns version management (per-key version chains), snapshot reads, write visibility rules, garbage collection of old (expired) versions, transaction isolation levels (read committed / repeatable read / snapshot / serializable), phantom-read prevention, and serialization-anomaly prevention via Serializable Snapshot Isolation (SSI) monitoring of read-write dependencies ("dangerous structures" of two adjacent rw-conflict edges).

`ids-mvcc` never opens a socket or drives any transport directly. It evaluates visibility, conflict, and snapshot decisions locally and returns them as decisions; all bytes cross a node only as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). It depends on `ids-000-aa` (canonical types, sealed contracts, invariants, errors) and consumes only the allowed crypto set (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) via `ids-crypto`.

A skeleton `ids-mvcc` crate does NOT yet exist at `C:/idfs/ids-mvcc/`. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-mvcc` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a pure concurrency-control/policy crate: it decides what a transaction may read, whether a write may commit, which versions are expired, and whether an anomaly pattern is forming. It never sends bytes itself; callers (`ids-store`, `ids-write`, `ids-read`, `ids-replication`, …) frame decisions into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (txn drivers)                   │
│  ids-store, ids-write, ids-read, ids-replication — begin/read/write/commit   │
├──────────────────────────────────────────────────────────────────────────────┤
│                Phase 065: ids-mvcc (THIS CRATE — concurrency control)        │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ version mgr      │ │ snapshot mgr     │ │ visibility       │ │ gc      │ │
│  │ (version chains, │ │ (read view at    │ │ (xmin/xmax/xip,  │ │ (expired│ │
│  │  commit ts alloc)│ │  txn start)      │  own-writes first) │ │  old ver│ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ conflict mgr     │ │ isolation levels │ │ phantom          │ │ ssi     │ │
│  │ (ww first-committer││ RC/RR/SI/Serializable│ │ prevention (predicate│ │ │monitor│ │
│  │   wins / abort)  │ │   read views)    │ │   / index locks) │ │(rw edges)│ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └─────────┘ │
└──────────────────────────────────────────────┬──────────────────────────────┘
                                               │ opaque Vec<u8> datagrams
┌─────────────────────────────────────────────▼──────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload)     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh anywhere in this crate        │
└────────────────────────────────────────────────────────────────────────────┘
```

**Lifecycle of one snapshot-isolation transaction:**
1. `begin()`: allocate a unique transaction ID + snapshot timestamp from the monotonic commit-timestamp allocator; snapshot = (xmin = oldest active txn, xmax = next txn to be assigned, xip = active txn IDs at snapshot time).
2. Reads: walk the per-key version chain; pick the newest version whose creating txn committed before the snapshot and whose deleting txn (if any) is either not-yet-committed or post-snapshot. Own uncommitted writes are visible first (self-consistency).
3. Writes: buffered in the transaction's private workspace; no lock held for the transaction's lifetime; at `commit()`, allocate a commit timestamp and run first-committer-wins write-write conflict detection on every written key. If another transaction committed a newer version of any written key after this snapshot began → abort.
4. Commit success: new versions published atomically; become visible to transactions whose snapshot timestamp is > this commit timestamp.
5. GC: a version is reclaimable when no active snapshot can see it (created by aborted txn, or superseded and older than the oldest active snapshot). Tuple-level (background vacuuming) and transaction-level GC strategies; protected-timestamp (PTS) records can hold GC back for long-running snapshots (CockroachDB pattern).

**Isolation levels implemented:**
- Read Committed: new snapshot per statement; sees commits up to each statement start.
- Repeatable Read / Snapshot: one snapshot at transaction start (first read or explicit BEGIN); stable for the whole transaction.
- Serializable (SSI): snapshot isolation plus runtime rw-conflict dependency tracking ("dangerous structure" detection) to prevent write skew and serialization anomalies; aborted transactions are retryable.

**Crypto used:** ed25519 signs commit decisions and audit checkpoints; sha256/blake3 hash version-vector entries and version IDs; xchacha20poly1305/chacha20poly1305 encrypt version payloads at rest when confidentiality is required. Nothing else.

---

### 3. Detailed Research Dimensions

Each dimension maps to targeted searches executed in parallel. Sources are cited inline with the source list in Section 12.

#### 3.1 MVCC theory — Bernstein & Goodman (1983), Adya et al. (2000)

Foundational theory: each write produces a new version; correctness = equivalence to some serial execution. Snapshot isolation first described in Berenson et al. (1995); generalized isolation-level definitions by Adya, Liskov, O'Neil (ICDE 2000) define phenomena (dirty reads, fuzzy/non-repeatable reads, phantoms, write skew) in terms of read/write dependencies. Bernstein & Goodman's MVCC theory underpins the version-visibility rules used here: a version is visible iff its create-commit precedes the read time and its delete-commit (if any) does not.

#### 3.2 Snapshot reads and version visibility (PostgreSQL xmin/xmax/xip model)

A snapshot is not a physical copy; it is defined by numbers: xmin (oldest active txn; all older XIDs visible unconditionally), xmax (all XIDs >= xmax invisible), xip[] (in-progress txns between xmin and xmax). Tuple visibility: version created by xid < xmin → visible; xmin ≤ xid < xmax → visible iff that txn is not in xip (i.e., committed before snapshot); xid ≥ xmax → invisible. Own transaction's writes always visible. Read view captured at first read (repeatable read: stable for whole transaction; read committed: new per statement).

#### 3.3 Write visibility and first-committer-wins

A transaction's writes are buffered privately and published atomically at commit with a new commit timestamp larger than any existing start/commit timestamp. First-committer-wins: T1 commits only if no transaction T2 with a commit timestamp in T1's execution interval [start_ts, commit_ts] wrote a key that T1 also wrote. If such a T2 exists → T1 aborts (write-write conflict). Lost updates prevented (phenomenon P4). Reads never block writers and writers never block readers; conflicts detected at commit time, not with long-held locks.

#### 3.4 Transaction isolation levels and SQL phenomena

SQL-92 phenomena: dirty read (reads uncommitted), non-repeatable read (re-read differs due to concurrent commit), phantom read (re-executed query sees new/removed rows matching a predicate), serialization anomaly (committed group inconsistent with any serial order). Levels: Read Uncommitted (all allowed), Read Committed (no dirty), Repeatable Read (no dirty/non-repeatable; phantoms possible in standard, prevented in PG via SSI), Serializable (none). `ids-mvcc` implements RC, RR/Snapshot, and Serializable via SSI; document exactly which phenomena each prevents.

#### 3.5 Phantom read prevention

A phantom occurs when a transaction re-runs a predicate query and the matching set changed due to another committed transaction. Pure snapshot isolation prevents phantoms within a stable snapshot (snapshot taken at start), but the write-skew / serialization anomaly remains. Prevention options researched: predicate locks (Eswaran/Gray/Lorie/Traiger 1976 — the original predicate-lock notion), index range locks, and SI's weaker guarantee. `ids-mvcc` design: snapshot reads inherently see a stable predicate set for the transaction's lifetime (no phantom inside one snapshot); cross-snapshot phantoms addressed by SSI's rw-conflict tracking (a read that later conflicts with a concurrent write can be rolled back). Predicate/index-locking as an optional stronger mode, documented as such.

#### 3.6 Serialization anomaly prevention — Serializable Snapshot Isolation (SSI)

Cahill, Röhm, Fekete (SIGMOD 2008 / TODS 2011): SI permits anomalies (write skew, read-only anomaly). SSI keeps SI's benefits (no reader/writer blocking) while detecting anomaly cycles at runtime. Core insight: each SI anomaly corresponds to a cycle in the dependency graph containing a "dangerous structure" of two adjacent rw-conflict edges: Tin →(rw) Tpivot →(rw) Tout. SSI tracks only rw-conflicts between concurrent transactions (not wr/ww), rolls back a transaction when a dangerous structure is detected. Optimizations: only roll back if Tout commits before Tpivot/Tin; read-only transactions can only be in a cycle if Tout committed before Tin took its snapshot. PostgreSQL 9.1+ implementation (README-SSI in postgres source). `ids-mvcc` implements SSI monitoring as the serializable level: maintain rw-conflict edges, detect dangerous structures, abort one member (retryable serialization failure).

#### 3.7 Write skew anomaly

Two transactions read overlapping state, write disjoint sets, commit; result inconsistent with any serial order (classic: two doctors on-call, each reads count ≥ 2, each marks self off-call → zero on-call). SI allows it because write sets are disjoint (no ww conflict). Prevented only by SSI (track the read→write anti-dependency / rw-conflict edges) or by explicit locking/promotion. `ids-mvcc` SSI level prevents write skew; snapshot level documents it as an allowed anomaly (matching Jepsen's SI model).

#### 3.8 Garbage collection of old versions

MVCC creates new versions on every write; unbounded growth unless reclaimed. Version reclaimable when no active transaction can see it AND it was not created by an in-flight/aborted transaction. Two GC approaches (CMU 15-721/15-445, VLDB '12 main-memory MVCC survey):
- Tuple-level GC: background vacuuming threads scan tables/version chains for expired versions (works with any storage; "dirty page bitmap" optimization skips unchanged pages); cooperative cleaning — workers identify reclaimable versions while traversing version chains (only O2N append-only chains; risk of "dusty corners" if a chain is never traversed).
- Transaction-level GC: each transaction tracks its own old versions; when it completes, all its versions become reclaimable once no snapshot needs them.
Granularity: single-version vs timestamp-based (global minimum timestamp / high-water mark = oldest active snapshot start; versions committed before it are obsolete) vs interval-based (SAP HANA, HyPer — can excise middle of chains). Block compaction: reclaim index entries too. The "vicious cycle": long-running transactions block GC, version chains grow, reads slow down. Mitigation: protected timestamps (CockroachDB PTS holds GC for a timestamp), TiDB GC safe point with max wait cap so one stuck transaction cannot block forever.

#### 3.9 Version chain storage and read path

Per logical key, versions form a chain sorted by commit timestamp (append-only O2N or N2O). A read at snapshot timestamp RT walks the chain and picks the newest version with begin ≤ RT and (end = ∞ or end > RT / end's txn aborted or post-snapshot). If begin/end fields temporarily contain in-progress transaction IDs, resolve that txn's state without blocking (or register a commit dependency). Index management: index entries point to version heads; GC must unlink index entries for reclaimed versions; index-only scans need visibility info (PostgreSQL visibility map pattern) or heap fetch to check visibility.

#### 3.10 Commit timestamp allocation and ordering

Timestamps from a global monotonically increasing counter (atomic fetch-add). Transaction start ts = latest commit ts at begin; commit ts assigned at commit, strictly increasing. FoundationDB pattern: proxy contacts other proxies to get most recent commit version; concurrency-control processes process requests in strict commit-version order. In a single-node `ids-mvcc`, a single atomic counter suffices; distributed extension (later phases) can use HLC or per-shard counters. Never wall-clock for ordering (clock skew).

#### 3.11 Hybrid logical clocks (optional distributed timestamp ordering)

For future multi-node deployment, HLC (Hybrid Logical Clock) combines physical time with logical counter: bounded growth, real-time correlation, causal ordering. Research: hybridclocktxn snapshot isolation with HLC. `ids-mvcc` Phase 065 uses a single monotonic counter; HLC integration noted as a future uncertainty, not adopted now.

#### 3.12 Rust MVCC implementations (reference, not dependencies)

- `mvcc_cell` (gitlab.com/2e71828/mvcc_cell): STM-style MVCC cells; each txn sees a fixed snapshot at start; first committer wins; concurrent commits to accessed cells abort others.
- `txn_db` (docs.rs/txn-db): MVCC transaction layer over a VersionStore trait; versions tagged with commit timestamp; snapshot isolation; retryable conflicts.
- `kovan-mvcc` (github.com/vertexclique/kovan): Percolator-model MVCC using Kovan epoch-based memory reclamation.
- `mvcc-core` (docs.rs/mvcc-core): `db.begin_with::<Snapshot>()` example showing snapshot read unaffected by concurrent commits; serializable write-skew example.
These are research references only — `ids-mvcc` is a fresh crate and does not depend on them.

#### 3.13 Production MVCC systems (reference)

- RocksDB: LSM never modifies in-place; every write = new version; transactions layer on top (Artem Krylysov blog).
- CockroachDB: revision/timestamp-based MVCC KV layer; tombstones for deletes; read "as of" timestamp; GC TTL default 25h; protected timestamps hold GC.
- TiDB/TiKV: Percolator-inspired distributed transactions with snapshot isolation; GC safe point; `tidb_gc_life_time` default 10m, `tidb_gc_max_wait_time` default 86400s.
- FoundationDB: strict serializable by default via conflict ranges (read + write key ranges); snapshot reads omit read conflict ranges (relaxed isolation, no conflict on concurrent writes); snapshot() gives consistent point-in-time view.
- PostgreSQL: xmin/xmax/xip snapshots; SSI since 9.1; VACUUM for GC; XID wraparound (32-bit) mitigations.
These are design references only.

#### 3.14 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

`ids-mvcc` uses: ed25519 to sign commit decisions and GC/audit checkpoints; sha256 and blake3 to hash version IDs and version-chain entries (blake3 preferred for speed/parallelism); xchacha20poly1305 (24-byte nonce, random-safe) and chacha20poly1305 (12-byte nonce, counter-bounded) to encrypt version payloads at rest when confidentiality required. All via `ids-crypto`, referenced through `ids-000-aa` primitive IDs. Nothing else.

#### 3.15 Concurrency control protocol choice (optimistic / MVCC vs locking vs 2PL)

MVCC is a form of optimistic concurrency control: conflicts detected at commit, no read locks, no writer/reader blocking. Pessimistic 2PL holds locks and serializes but blocks readers/writers. `ids-mvcc` chooses optimistic MVCC per the IDS goal of high concurrency; write-write conflicts abort the later committer (retryable). Documented tradeoff: higher abort rates under heavy write contention, higher version-storage/GC cost.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Dirty read (reads uncommitted txn) | Read correctness | Visibility rules exclude uncommitted creators; own-writes visible only within same txn |
| Non-repeatable read | Repeatable-read guarantee | Stable snapshot at txn start (RR/SI); new snapshot per statement only at RC |
| Phantom read | Predicate stability | Snapshot at start freezes predicate set for txn lifetime; SSI covers cross-snapshot cases |
| Write skew / serialization anomaly | Data consistency | SSI rw-conflict "dangerous structure" detection → abort one txn (retryable) |
| Lost update (P4) | Write correctness | First-committer-wins write-write conflict detection at commit |
| Long-running txn blocks GC → version bloat | Storage/memory, read perf | Protected timestamps with max-wait cap; interval GC excising middle of chains; vacuums |
| Aborted-txn versions never reclaimed | Storage leak | Aborted txn's versions marked reclaimable immediately on abort |
| Timestamp counter overflow | Ordering correctness | u64 monotonic counter; saturating semantics documented; wrap handled like PG XID (not in scope) |
| Clock skew (distributed extension) | Snapshot correctness | Logical counters only in Phase 065; HLC noted as future, never wall-clock for ordering |
| GC reclaiming a version still needed by a snapshot | Read correctness / data loss | Reclaim only when version older than oldest active snapshot start; PTS records hold GC |
| Carrier duplication/delay/reordering | Replay/ordering | Decisions are deterministic functions of local state; idempotent version application tolerates duplicates |
| Prohibited crate/dependency | Architecture violation | Dependency audit; Cargo.toml contains only allowed crates; no networking stack |
| Disallowed crypto algorithm | Crypto policy | Allowlist enforced; only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via ids-crypto |
| Signing-key extraction | Decision authenticity | Signing keys never leave node; encrypted at rest; zeroized on shutdown |

The carrier is never trusted: visibility/commit/GC decisions are pure functions over local version state; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) keeps no network stack in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-mvcc ||
|-------|------|--------------------------|--------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `RecordId`, `Checksum`, `OpState`, `CommitState`, `Error` — consumed, never redefined; sealed contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) via `Carrier` trait | All traffic to/from `Endpoint`; crate returns decisions, never sends bytes itself |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Sign commit decisions/GC checkpoints; hash version IDs; encrypt payloads at rest |
| `ids-time` | Monotonic `IdsTime` source | Commit-timestamp and snapshot timestamps derived from monotonic ticks, never wall-clock |
| `ids-store`, `ids-write`, `ids-read` | Primary record store / txn drivers | Use ids-mvcc for begin/read/write/commit/visibility decisions |
| `ids-replication` | Replica synchronization | Replicates committed versions via `ids-carrier::Endpoint`; applies MVCC visibility on replay |
| `ids-consistency-profile` (Phase 060) | Profile (R/W quorum, durability) | Profile gates write admission; MVCC enforces isolation within the admitted write |

**Integration points:** `ids-mvcc` exposes a public API (`begin`, `read`, `write`, `commit`, `abort`, `snapshot_at`, `gc_safe_point`, `is_version_visible`) called by store/write/read crates. It does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond exposed contracts.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-mvcc/` (a separate crate, not a folder inside an existing crate):

```
ids-mvcc/
├── Cargo.toml                          # manifest: ids-000-aa, ids-carrier, ids-crypto, ids-time, serde, thiserror, blake3, sha2, chacha20poly1305, xchacha20poly1305, ed25519-dalek (crypto via ids-crypto re-export surface)
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # MVCC defaults: snapshot retention, GC intervals, chain-length limits
├── error.rs                            # MvccError (thiserror): visibility, conflict, GC, txn errors
├── types.rs                            # TxnId, CommitTs, SnapshotTs, VersionId, Snapshot, Version, TxnState (re-export ids-000-aa types)
│
├── txn/                                # transaction lifecycle (~10 files)
│   ├── mod.rs
│   ├── transaction.rs                  # Txn struct: read/write set, snapshot, state machine
│   ├── begin.rs                        # begin(): allocate txn id + snapshot ts
│   ├── commit.rs                       # commit(): first-committer-wins ww conflict, publish versions
│   ├── abort.rs                        # abort(): discard private workspace, mark versions reclaimable
│   ├── read_set.rs                     # read-set tracking (keys + versions read)
│   ├── write_set.rs                    # write-set buffer (private workspace)
│   └── state_machine.rs                # TxnState: Active → Committing → Committed/Aborted
│
├── version/                            # version management (~12 files)
│   ├── mod.rs
│   ├── version_chain.rs                # per-key chain of versions sorted by commit ts
│   ├── version.rs                      # single Version: value, begin_ts, end_ts, create_txn, delete_txn
│   ├── commit_ts_alloc.rs              # monotonic commit-timestamp allocator (u64 counter)
│   ├── snapshot_alloc.rs               # snapshot timestamp at txn start
│   ├── tombstone.rs                    # delete marker version
│   ├── version_id.rs                   # VersionId hashing (blake3/sha256)
│   └── private_workspace.rs            # per-txn buffered writes
│
├── snapshot/                           # snapshot reads (~8 files)
│   ├── mod.rs
│   ├── read_view.rs                    # snapshot: xmin/xmax/xip, visibility rule set
│   ├── visibility.rs                   # is_visible(version, snapshot): xmin/xmax/xip rules
│   ├── own_writes.rs                   # own-uncommitted-writes-first visibility
│   ├── statement_snapshot.rs           # read-committed: new snapshot per statement
│   └── txn_snapshot.rs                 # repeatable-read/snapshot: stable snapshot for txn
│
├── write_visibility/                   # write visibility & conflict (~8 files)
│   ├── mod.rs
│   ├── first_committer_wins.rs         # ww conflict: abort if newer commit on written key in interval
│   ├── conflict_detection.rs           # detect write-write conflicts at commit
│   ├── publish.rs                      # atomic publish of committed versions
│   └── lost_update.rs                  # P4 prevention
│
├── isolation/                          # isolation levels (~10 files)
│   ├── mod.rs
│   ├── level.rs                        # IsolationLevel enum: ReadCommitted / RepeatableRead / Snapshot / Serializable
│   ├── read_committed.rs
│   ├── repeatable_read.rs
│   ├── snapshot_level.rs
│   ├── serializable.rs                 # SSI: rw-conflict edges, dangerous structures
│   ├── rw_conflict.rs                  # read-write conflict edge tracking
│   ├── dangerous_structure.rs          # Tin→rw Tpivot→rw Tout cycle detection
│   ├── write_skew.rs                   # write-skew anomaly definition + prevention
│   ├── phantom.rs                      # phantom-read prevention (predicate stability + SSI)
│   └── serialization_anomaly.rs        # serialization anomaly prevention
│
├── gc/                                 # garbage collection (~12 files)
│   ├── mod.rs
│   ├── gc_manager.rs                   # GC orchestration: detect, unlink, reclaim
│   ├── tuple_level.rs                  # background vacuuming + cooperative cleaning
│   ├── transaction_level.rs            # per-txn old-version tracking
│   ├── safe_point.rs                   # oldest active snapshot start (high-water mark)
│   ├── protected_ts.rs                 # PTS records holding GC for long snapshots (CockroachDB pattern)
│   ├── expired_version.rs              # reclaimability predicate
│   ├── index_unlink.rs                 # remove index entries for reclaimed versions
│   ├── chain_compact.rs                # version-chain compaction / block compaction
│   ├── vicious_cycle.rs                # long-running-txn GC blocking mitigation
│   ├── ti_db_safe_point.rs             # TiDB-style safe point with max-wait cap
│   └── vacuum_scheduler.rs             # scheduled vacuum passes
│
├── carrier_adapter.rs                  # opaque Vec<u8> framing for decision transport to ids-carrier::Endpoint
│
├── contracts/                          # sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── mvcc_contract.rs
│   ├── snapshot_contract.rs
│   ├── visibility_contract.rs
│   ├── conflict_contract.rs
│   ├── isolation_contract.rs
│   ├── gc_contract.rs
│   ├── commit_contract.rs
│   ├── phantom_contract.rs
│   ├── ssi_contract.rs
│   └── carrier_contract.rs
│
├── invariants/                         # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── snapshot_stability.rs
│   ├── first_committer_wins_holds.rs
│   ├── gc_safety.rs                    # never reclaim a version visible to an active snapshot
│   ├── monotonic_ts.rs                 # commit ts strictly increasing
│   ├── carrier_agnostic.rs             # no std::net/tokio::net in crate
│   ├── crypto_allowlist.rs
│   ├── visibility_sound.rs
│   ├── mod.rs
│
├── audit/                              # signed, hash-chained commit/GC audit (~6 files)
│   ├── audit_entry.rs
│   ├── audit_chain.rs
│   ├── checkpoint.rs
│   ├── signer.rs                       # ed25519 signing of checkpoints
│   ├── verifier.rs
│   └── mod.rs
│
├── tests/                              # property/verification tests (~30 files)
│   ├── mod.rs
│   ├── visibility_tests.rs
│   ├── snapshot_tests.rs
│   ├── commit_conflict_tests.rs
│   ├── isolation_level_tests.rs
│   ├── write_skew_tests.rs
│   ├── phantom_tests.rs
│   ├── ssi_tests.rs
│   ├── gc_tests.rs
│   ├── protected_ts_tests.rs
│   ├── safe_point_tests.rs
│   ├── carrier_agnostic_test.rs        # asserts no std::net/tokio::net usage
│   ├── crypto_allowlist_test.rs
│   ├── determinism_tests.rs            # same ops + order → same visible versions
│   ├── property_tests.rs               # proptest on version chains
│   └── integration_tests.rs
│
├── benches/                            # (~10 files)
│   ├── read_latency.rs
│   ├── commit_conflict.rs
│   ├── gc_throughput.rs
│   └── chain_length.rs
│
├── docs/                               # (~14 files)
│   ├── architecture.md
│   ├── isolation_levels.md
│   ├── ssi.md
│   ├── gc.md
│   ├── carrier_agnostic.md
│   ├── crypto_policy.md
│   └── runbook.md
│
└── spec/                               # (~4 files)
    ├── visibility_spec.md
    ├── ssi_spec.md
    ├── gc_spec.md
    └── isolation_spec.md
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, `spec/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Snapshot and version visibility (xmin/xmax/xip)

```
Snapshot { xmin: TxnId, xmax: TxnId, xip: BitSet<TxnId> }
is_visible(version, snapshot):
  if version.create_txn == self_txn: return true        # own writes
  if version.create_txn < snapshot.xmin: return true    # committed before snapshot
  if version.create_txn >= snapshot.xmax: return false  # started after snapshot
  if version.create_txn in snapshot.xip: return false   # in-flight at snapshot
  return true                                           # committed before snapshot
  # deletion: version invisible if delete_txn committed ≤ snapshot
```

Applied per version while walking the per-key chain; newest visible version wins.

#### 7.2 First-committer-wins write-write conflict

```
commit(txn):
  for each key k in txn.write_set:
    latest = latest_committed_version(k)
    if latest.commit_ts in (txn.start_ts .. txn.commit_ts):
       abort(txn)   # another txn wrote k in our execution interval
  commit_ts = next_monotonic_ts()
  publish versions with commit_ts
```

Prevents lost updates (P4); abort is retryable.

#### 7.3 Serializable Snapshot Isolation — dangerous structures

Track rw-conflict edges: edge T1 → T2 if T1 read a version that T2 later overwrote (T1's read precedes T2's write in version order). A "dangerous structure" = two adjacent rw edges Tin → Tpivot → Tout. Abort one member (prefer Tout if it committed before Tpivot/Tin; read-only Tin only at risk if Tout committed before Tin's snapshot). Breaks cycles → serializability.

#### 7.4 Garbage collection — safe point

```
safe_point = min(start_ts of all active transactions)
version reclaimable iff version.commit_ts < safe_point AND no active snapshot references it
PTS(ts): hold GC above ts until record removed (protects long-running snapshots)
max_wait cap: single stuck txn cannot block GC past configured deadline (TiDB pattern)
```

Two strategies: tuple-level (background vacuuming + cooperative cleaning on O2N chains) and transaction-level (per-txn version lists reclaimed on commit/abort).

#### 7.5 Phantom prevention

Within one snapshot: predicate set frozen at snapshot start → no phantom. Cross-snapshot: SSI rw edges abort a transaction whose read set was invalidated by a concurrent commit. Predicate/index locking optional stronger mode (documented, not default).

#### 7.6 Version chain data structure

```
VersionChain { head: Option<Version>, versions: Vec<Version> }  # append-only, sorted by commit_ts
read_at(chain, rt): walk from head; first version with visible(rt) wins
write(chain, new_version): append; old version gets end_ts = new.commit_ts
```

---

### 8. Dependencies

`ids-mvcc/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `RecordId`, `Checksum`, `OpState`, `CommitState`, `Error`) and sealed trait contracts
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`); decision transport only; crate never sends directly
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305
- `ids-time` — monotonic time source for commit/snapshot timestamps
- `serde` + `thiserror` — serialization + error enum
- `blake3`, `sha2` — version ID / version-chain hashing (type-level; ops via ids-crypto where applicable)
- dev-only: `proptest`, `criterion`, `tempfile`

**No** `std::net`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, `dns`, or any networking crate. No async runtime. No crypto beyond the allowed list; `ids-mvcc` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-65.1 | Snapshot read: txn sees newest version committed before its start; never sees uncommitted or post-start commits | Visibility unit tests on version chains, 100% of cases |
| AC-65.2 | Write visibility: own uncommitted writes visible within same txn; invisible to others until commit | Read/write isolation tests |
| AC-65.3 | First-committer-wins: two txns writing same key, later committer aborts with retryable conflict | Commit-conflict test, deterministic |
| AC-65.4 | Lost update (P4) prevented: concurrent read-modify-write on same key serializes | Serialization test |
| AC-65.5 | Write skew prevented at Serializable level (SSI); allowed/documented at Snapshot level | SSI dangerous-structure test |
| AC-65.6 | Phantom read prevented within a snapshot; cross-snapshot via SSI | Predicate-stability + phantom test |
| AC-65.7 | Isolation levels RC / RR/Snapshot / Serializable behave per SQL phenomena table | Isolation-level matrix test |
| AC-65.8 | GC reclaims only versions older than oldest active snapshot start; never reclaims visible versions | GC safety invariant test |
| AC-65.9 | Protected timestamps hold GC until released; max-wait cap prevents indefinite block | PTS + safe-point test |
| AC-65.10 | Aborted transaction's versions reclaimable immediately | Abort + GC test |
| AC-65.11 | Commit timestamps strictly increasing (monotonic u64 counter) | Timestamp allocator test |
| AC-65.12 | All traffic via `ids-carrier::Endpoint`; zero `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep |
| AC-65.13 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Crypto allowlist unit test |
| AC-65.14 | Commit decisions and GC checkpoints ed25519-signed, hash-chained audit log | Audit verifier test |
| AC-65.15 | Determinism: same operation sequence → same visible versions and commit outcomes | Determinism test |
| AC-65.16 | Crate compiles cleanly: `cargo check -p ids-mvcc` exits 0 with no warnings | Build test |
| AC-65.17 | All tests pass: `cargo test -p ids-mvcc` | Test suite |

---

### 10. Remaining Uncertainties

1. **Commit-timestamp allocator distribution:** single atomic u64 counter for Phase 065; distributed extension (per-shard counters / HLC) deferred to a later phase; HLC integration noted but not adopted.
2. **GC strategy mix:** tuple-level background vacuuming vs transaction-level per-txn tracking; ratio and scheduling to be tuned at integration with `ids-store`.
3. **Version-chain length cap and compaction:** max chain length before forced compaction unquantified; long chains degrade read walk cost.
4. **Phantom prevention strength:** snapshot-level prevents phantoms within a transaction; whether to add optional predicate/index locking for stronger guarantees is a design decision pending.
5. **SSI abort rate under contention:** dangerous-structure detection may abort more than pure SI; acceptable false-positive rate unmeasured for IDS workloads.
6. **Interaction with `ids-consistency-profile` (Phase 060):** profile quorum/durability gates write admission; exact handshake (profile → MVCC admission) defined here, validated at integration.
7. **Protected-timestamp max-wait default:** CockroachDB-style PTS with TiDB max-wait cap; default value workload-dependent.
8. **Index visibility management:** whether reclaimed versions unlink index entries eagerly or lazily; index-only scan visibility map pattern (PostgreSQL) considered but not mandated.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-mvcc`, not a folder | Project rule: separate crates for new capabilities; clear concurrency-control boundary |
| Optimistic MVCC, first-committer-wins | Readers never block writers, writers never block readers; conflicts at commit time; retryable aborts |
| Snapshot at txn start (xmin/xmax/xip) | Proven PostgreSQL model; simple, well-understood visibility rules |
| Serializable via SSI (dangerous structures) | Prevents write skew + serialization anomalies while keeping SI performance; tracks rw edges only |
| Phantom prevented via snapshot stability + SSI | Predicate set frozen for txn lifetime; cross-snapshot handled by rw-conflict abort |
| GC safe point = oldest active snapshot start | Simple, correct reclaimability; protected timestamps + max-wait cap prevent indefinite GC block |
| Two GC strategies (tuple-level + transaction-level) | Covers all storage schemes; cooperative cleaning for O2N chains, vacuuming as fallback |
| Monotonic u64 commit timestamps, never wall-clock | Clock-skew-safe ordering; distributed HLC deferred |
| All traffic via `ids-carrier::Endpoint`; crate returns decisions, never sends | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only, via `ids-crypto` / `ids-000-aa` | IDS crypto policy; ed25519/signing, sha256/blake3/hashing, xchacha20poly1305/chacha20poly1305/encryption |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable commit/GC decision log |
| Fresh crate, no dependency on existing Rust MVCC crates | `mvcc_cell`, `txn_db`, `kovan-mvcc`, `mvcc-core` are research references only |

---

### 12. Sources

[^1]: Bernstein & Goodman, "Multiversion concurrency control—theory and algorithms," ACM TODS 8(4):465–483, 1983. https://dl.acm.org/doi/10.1145/319996.319998 — MVCC foundational theory.
[^2]: Berenson et al., "A critique of ANSI SQL isolation levels," SIGMOD 1995. https://dl.acm.org/doi/10.1145/1071610.1071615 — snapshot isolation definition, phenomena.
[^3]: Adya, Liskov, O'Neil, "Generalized isolation level definitions," ICDE 2000. Read-write dependency phenomena (dirty read, fuzzy read, phantom, write skew).
[^4]: PostgreSQL Docs 19, "Transaction Isolation" (13.2) and "Introduction to MVCC" (13.1). https://www.postgresql.org/docs/current/transaction-iso.html ; https://www.postgresql.org/docs/17/mvcc-intro.html — isolation levels, phenomena table, MVCC model.
[^5]: PostgreSQL source, `src/include/utils/snapshot.h` and `src/backend/storage/lmgr/README-SSI`. https://doxygen.postgresql.org/snapshot_8h_source.html ; https://github.com/postgres/postgres/blob/master/src/backend/storage/lmgr/README-SSI — xmin/xmax/xip snapshot struct; SSI dangerous structures.
[^6]: PostgreSQL Docs 19, "Routine Vacuuming" (24.1). https://www.postgresql.org/docs/current/routine-vacuuming.html — VACUUM reclaims dead row versions; XID wraparound.
[^7]: Cahill, Röhm, Fekete, "Serializable Isolation for Snapshot Databases," SIGMOD 2008 / TODS 34(4), 2011. https://dl.acm.org/doi/10.1145/1376616.1376690 ; https://dl.acm.org/doi/10.1145/1620585.1620587 — SSI algorithm, dangerous structures, rw-conflict tracking.
[^8]: Jepsen, "Snapshot Isolation" consistency model. https://jepsen.io/consistency/models/snapshot-isolation — SI definition, first-committer-wins, write skew, read-only anomaly, internal/external consistency properties.
[^9]: Eswaran, Gray, Lorie, Traiger, "The notions of consistency and predicate locks," CACM 19(11), 1976. Cited in [^7] references — predicate locks, phantom prevention basis.
[^10]: CMU 15-721 / 15-445 MVCC lecture notes (garbage collection, version storage, tuple-level vs transaction-level GC). https://15721.courses.cs.cmu.edu/spring2020/notes/05-mvcc3.pdf ; https://15445.courses.cs.cmu.edu/fall2025/notes/20-multiversioning.pdf — GC design decisions, background vacuuming, cooperative cleaning.
[^11]: "An Empirical Evaluation of In-Memory Multi-Version Concurrency Control," VLDB 10. https://www.vldb.org/pvldb/vol10/p781-Wu.pdf — GC three steps (detect, unlink, reclaim); vicious cycle of garbage.
[^12]: "Scalable garbage collection for in-memory MVCC systems," PVLDB. https://doi.org/10.14778/3364324.3364328 — high-water mark, interval-based GC, long-running txn blocking.
[^13]: CockroachDB, "Protected Timestamps: For a future with less garbage." https://cockroachlabs.com/blog/protected-timestamps-for-less-garbage — GC threshold, PTS records hold GC per replica.
[^14]: TiDB Development Guide, "MVCC garbage collection" and "How TiDB GC Safe Points Work." https://pingcap.github.io/tidb-dev-guide/understand-tidb/mvcc-garbage-collection.html ; https://jzleetcode.github.io/posts/design-how-tidb-gc-safe-points-work — GC safe point, `tidb_gc_life_time`, `tidb_gc_max_wait_time`.
[^15]: FoundationDB Developer Guide, conflict ranges and snapshot reads. https://apple.github.io/foundationdb/developer-guide.html ; https://apple.github.io/foundationdb/read-write-path.html — snapshot reads omit read conflict ranges; strict serializable default.
[^16]: CockroachDB blog, "Writing History: How we rebuilt bulk operations to preserve a history of changes." https://www.cockroachlabs.com/blog/writing-history-how-we-rebuilt-bulk-operations-to-preserve-a-history-of-changes/ — MVCC revision storage, tombstones, read-as-of-timestamp.
[^17]: TiKV / Percolator. https://tikv.org/deep-dive/distributed-transaction/percolator — distributed transactions with snapshot isolation, commit timestamps.
[^18]: SQLite, "Isolation In SQLite" and `sqlite3_snapshot` C interface. https://sqlite.org/isolation.html ; https://sqlite.org/c3ref/snapshot.html — WAL-mode snapshot isolation, point-in-time snapshots.
[^19]: `mvcc_cell` (gitlab.com/2e71828/mvcc_cell), `txn_db` (docs.rs/txn-db), `kovan-mvcc` (github.com/vertexclique/kovan), `mvcc-core` (docs.rs/mvcc-core) — Rust MVCC reference implementations (reference only, not dependencies).
[^20]: "Efficient Correction of Anomalies in Snapshot Isolation Transactions," TACO 2014 (Litz et al.). https://people.ucsc.edu/~hlitz/papers/write-skew.pdf — write-skew anomaly, SI anomalies, online prevention.
[^21]: FoundationDB Record Layer, "ISOLATION LEVEL SNAPSHOT." https://foundationdb.github.io/fdb-record-layer/reference/statement_options/ISOLATION_LEVEL_SNAPSHOT.html — snapshot reads vs serializable reads, conflict-range tradeoffs.
[^22]: hybridclocktxn, "Snapshot Isolation with Hybrid Clock." https://github.com/unmeshjoshi/hybridclocktxn/blob/main/SNAPSHOT_ISOLATION_WITH_HYBRID_CLOCK.md — HLC for distributed snapshot isolation (future reference).
[^23]: IDS prompts.txt entry 65 (verbatim). C:/idfs/prompts.txt lines 970-984 — Phase 065 crate definition, contents, constraints.
[^24]: Existing `ids-000-aa` skeleton. C:/idfs/ids-000-aa/ — canonical types, sealed contracts, invariants, errors consumed by this crate.
[^25]: HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md; IDS plans plan-001.md, plan-048.md, plan-060.md — plan document structure precedent.
[^26]: CMU 15-445 Project #4 Concurrency Control (BusTub MVOCC, version chains, undo logs). https://15445.courses.cs.cmu.edu/fall2023/project4 — practical MVCC implementation notes.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, version/visibility/GC algorithms, isolation levels, SSI anomaly prevention, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 065 (ids-mvcc).
