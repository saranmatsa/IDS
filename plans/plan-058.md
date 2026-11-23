## Phase 058 — Create the ids-durability-policy crate: Per-dataset durability requirements.

**Research complete (30 targeted searches executed). No code written, no source modifications, only the plan document. Research only — no code, no source modifications.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 865-879)

```
58. Research only do not read plan.md **[Phase 058 — Create the ids-durability-policy crate: Per-dataset durability requirements.]**

Crate: ids-durability-policy
Purpose: Per-dataset durability requirements.
Contents: Local persistence, quorum persistence, regional redundancy threshold, cross-region durability, acknowledgement semantics, commit vs synchronization distinction.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 058 — Create the ids-durability-policy crate: Per-dataset durability requirements.]**

Create plan-058.md in C:/idfs/plans/ for the ids-durability-policy crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 058 creates `ids-durability-policy`, a new standalone IDS crate (separate crate, not folders inside an existing one) that owns per-dataset durability requirements: local persistence, quorum persistence, regional redundancy threshold, cross-region durability, acknowledgement semantics, and the commit-vs-synchronization distinction. It is carrier-agnostic: it never opens sockets, never references `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh, and ships every durability-relevant message as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-000-aa`/`ids-crypto` primitive IDs and type surface. `ids-000-aa` remains the master crate holding every canonical type and sealed trait contract (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, sealed traits, error types); `ids-durability-policy` references them and never redefines them.

The crate evaluates, per dataset id, a durability requirement object: a local persistence tier (memory-only / journaled / synced), a quorum persistence threshold (how many replicas must durably persist before acknowledgement), a regional redundancy threshold (how many failure domains/regions must hold durable copies), a cross-region durability mode, and an acknowledgement level (none / leader / quorum / all). It separates the commit boundary (logical acceptance/ordering of a write) from the synchronization boundary (durable stable-storage barrier before ack) — matching the established finding that durability guarantees comprise two independent dimensions: acknowledgement guarantees and synchronization guarantees (Oracle Berkeley DB durability model). It returns a durable verdict (satisfied / unsatisfied / undetermined) that callers gate acknowledgement on.

Research basis: 30 parallel searches returned the `durability` crate (local persistence primitives: directories, atomic writes, WALs, record logs, checkpoints, CRC validation, sync helpers; `FsDirectory` for fsync/parent-directory sync; `SyncWalWriter::append_durable`; no per-write fsync by default, `flush_and_sync()` as the barrier), `spacedb-durability` (Reed-Solomon k-of-n erasure-coded snapshots, content-addressed manifest with blake3 hashes, anti-affinity placement across failure domains, health/repair/reclaim), MongoDB write concern (`w` count + `j` journal flag + `wtimeout`, `w:majority` default, `writeConcernMajorityJournalDefault`), Couchbase durable writes (`Majority` / `MajorityAndPersistToActive` / `PersistToMajority` durability levels), Apache Iggy topic durability (`replicated` = quorum commit + local application without stable-storage barrier; `persisted` = quorum commit backed by recoverable stable-storage copies at quorum, then local application; quorum = min(ceil(n/2),3) with intersecting view-change quorum), PostgreSQL `synchronous_commit` (remote_apply/on/remote_write/local/off) and `wal_sync_method` (fdatasync/fsync/fsync_writethrough/open_sync/open_datasync), fsync vs fdatasync semantics and the new-file directory-entry durability trap (fsync parent dir after file creation), RabbitMQ publisher confirms / quorum queues (ack after quorum of replicas confirmed; persistent messages to durable queues), Kafka `acks` (0/1/all) with `min.insync.replicas`, Cloud Storage (11 nines annual durability, erasure coding, regional = 2+ AZs confirmed before write success, cross-region dual/multi-region async replication), Azure Storage redundancy (LRS/ZRS/GRS/GZRS durability tiers, ZRS across 3+ AZs), Berkeley DB Replication Manager ack policies (NONE/ONE_PEER/QUORUM/ALL/ALL_PEERS; consistency vs durability distinction), Cassandra consistency levels (ALL/QUORUM/LOCAL_ONE/ONE/TWO/THREE), quipu-log SyncPolicy (Always/EveryN/OsManaged fsync policy + group commit + exposure window), durable-ioring (epoch-based durability points, high-water mark, no_std), iqdb-persist and persistent-queue FsyncPolicy (Always/Periodic/Never; Sync/Group/None), Giza erasure coding across global data centers (USENIX ATC '17), SQLite WAL/atomic commit (fsync at commit points, synchronous=FULL vs NORMAL), RustCrypto chacha20poly1305/XChaCha20Poly1305 (RFC 8439), and Rust API Guidelines sealed-trait pattern (C-SEALED, `private::Sealed` supertrait).

---

### 2. Architecture Overview

`ids-durability-policy` sits between the local store and the acknowledgement layer. It is not a transport or a network protocol — it is a deterministic, local policy engine that, given a dataset id and a proposed write outcome, computes whether the durability requirement for that dataset is satisfied and what acknowledgement level to emit. Every outbound durability signal is an opaque `Vec<u8>` handed to `ids-carrier::Endpoint`; the crate never addresses a peer by network identity.

```
┌─────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-store                   │
│  per-dataset durability requirement (policy per dataset id) │
├─────────────────────────────────────────────────────────────┤
│              Phase 058: ids-durability-policy (NEW)         │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────────┐   │
│  │ local       │ │ quorum       │ │ regional +       │   │
│  │ persistence │ │ persistence  │ │ cross-region     │   │
│  │ tier engine │ │ threshold    │ │ redundancy       │   │
│  └─────────────┘ └──────────────┘ └──────────────────┘   │
│  ┌──────────────────┐ ┌──────────────────────────────┐   │
│  │ ack semantics    │ │ commit vs synchronization    │   │
│  │ (none/leader/    │ │ boundary separator           │   │
│  │  quorum/all)     │ │ (commit ≠ durable sync)      │   │
│  └──────────────────┘ └──────────────────────────────┘   │
│  ┌────────────────────────────────────────────────────┐   │
│  │ durable verdict: Satisfied / Unsatisfied / Undetermined│ │
│  └────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│            ids-carrier::Endpoint (CarrierType + Vec<u8>)    │
│  all outbound durability signals as opaque byte datagrams   │
├─────────────────────────────────────────────────────────────┤
│             ids-000-aa (master crate, canonical types)      │
│  NodeId, RegionId, IdsTime, CarrierType, RecordId, Checksum,│
│  sealed trait contracts, error types                        │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a durability check:**
1. Local store prepares a write for dataset `D` and asks `ids-durability-policy`: "may I acknowledge, and at what level?"
2. The crate loads `DurabilityPolicy[D]` (local tier, quorum threshold `q` of `n`, regional threshold `r` of regions, cross-region mode, ack level).
3. It evaluates the local persistence barrier against the tier (e.g., fsync/fdatasync completed per `SyncPolicy`), the quorum against durable-replica reports, and the regional threshold against region-durable reports.
4. It separates commit (write accepted and ordered locally) from synchronization (stable-storage barrier reached); ack is emitted only when both the commit and the configured synchronization barrier are satisfied.
5. Result: `DurabilityVerdict` + `AckLevel`, serialized and handed to `ids-carrier::Endpoint` as opaque `Vec<u8>` bytes; the carrier decides how (if at all) to deliver them.

**Carrier-agnostic absolute rule:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere in the crate. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

**Crypto rule:** only ed25519 (signatures), xchacha20poly1305 (AEAD), sha256, blake3, chacha20poly1305 — consumed from `ids-000-aa`/`ids-crypto` primitive IDs; no other cryptographic primitives.

---

### 3. Detailed Research Dimensions

#### 3.1 Local persistence

Local persistence is the foundation layer: a dataset's durability requirement is only as strong as the weakest local durable copy. Research-verified building blocks:

- **Persistence tiers:** `MemoryOnly` (page-cache only, lost on power loss — acceptable for cache/replica tiers that can be rebuilt), `Journaled` (write-ahead log appended, ack after in-memory apply, vulnerable to correlated power loss), `Synced` (fsync/fdatasync barrier reached before ack — survives single-node power loss), `SyncedAndReplicated` (synced plus quorum/regional copies). This mirrors Couchbase durable-write levels (in-memory / majority-in-memory / majority-persisted) and Apache Iggy `replicated` vs `persisted` topic durability.
- **Sync policy:** `Always` (fsync after every append — safest, slowest, exposure window ~0), `EveryN(n)` / group commit (fsync every n appends — bounded exposure window of n−1 entries; ~12× throughput cost vs unmanaged but predictable), `OsManaged` (flush only, rely on OS writeback — fastest, unbounded exposure window, unsafe for power loss). Source: quipu-log SyncPolicy; `durability` crate `flush_and_sync()` barrier; iqdb-persist `FsyncPolicy::{Always, Periodic, Never}`; persistent-queue `Sync/Group/None`.
- **fsync vs fdatasync:** `fsync` flushes data + metadata needed to retrieve it (size, block mapping, timestamps); `fdatasync` flushes data plus only metadata required for a subsequent read (may skip mtime). Difference is workload-dependent; on journaled filesystems fdatasync skips a metadata journal transaction. Source: Linux kernel internals fsync/fdatasync.
- **New-file durability trap:** creating a file, writing, and fsyncing the file is insufficient — the directory entry linking the name to the inode must also be durable; fsync the parent directory after file creation (PostgreSQL `fsync_fname`/`SyncDir`, LevelDB/RocksDB practice). `ids-durability-policy` must require parent-directory sync for new dataset files.
- **CRC/checksum:** durable metadata and payload covered by checksum (blake3 preferred per allowed-crypto rule; sha256 as alternative). `durability` crate covers durable metadata + payload with CRC; `spacedb-durability` manifest carries every shard's content hash so corrupted/substituted shards are detected at reconstruction.
- **WAL + checkpoint:** append-only WAL for crash recovery, periodic atomic checkpoints, checkpoint publish + WAL truncation (`durability` crate `recover`/`publish`; SQLite WAL checkpoint; durable-ioring durability points + high-water mark that never reports a point durable while an earlier failure is unresolved).
- **Power-loss honesty:** a write returning success means only "in page cache" unless a sync barrier was crossed; acknowledged-but-unsynced writes are lost on power loss (PostgreSQL `synchronous_commit=off` semantics; Iggy `replicated` policy caveat). The crate must expose the exposure window per tier rather than overpromise.

#### 3.2 Quorum persistence

Quorum persistence decides how many replicas must durably persist a write before it is acknowledged, tolerating replica failure without data loss.

- **Quorum arithmetic:** with `n` replicas, write quorum `w` and read quorum `r` must satisfy `w + r > n` for read-your-writes consistency and `2w > n` (intersecting write quorums) to prevent divergent committed states. Apache Iggy: replication quorum `min(ceil(n/2), 3)`, view-change quorum `n − replication_quorum + 1`; quorums intersect. Berkeley DB `DB_REPMGR_ACKS_QUORUM` survives master change / network partition. Majority write quorum tolerates `⌊(n−1)/2⌋` failures.
- **Ack policies:** `NONE` (fire-and-forget, highest throughput, no durability), `LEADER` / `ONE` (primary only — risk if leader fails before replication), `QUORUM` (majority of replicas — survives single failure with n=3), `ALL` / `all ISR` (highest durability, lowest availability, blocks on any replica down — Kafka `acks=all` + `min.insync.replicas=2` tolerates one broker loss with RF=3). Cassandra consistency levels: `ALL/QUORUM/LOCAL_ONE/ONE/TWO/THREE`.
- **Per-dataset quorum:** `DurabilityPolicy[D].quorum = { n, w }` stored per dataset; quorum recomputed when replica membership changes; stale/unelectable replicas excluded from quorum counts (Berkeley DB electable-peer rule).
- **Failure-independence caveat:** replication count alone does not bound loss under correlated failure (shared power loss, OOM cascade, crash-on-every-replica); replicas must be isolated across failure domains (Iggy failure-independence note). `ids-durability-policy` expresses this as a required assumption, not a guarantee.

#### 3.3 Regional redundancy threshold

Regional redundancy extends quorum across geographic failure domains (regions / availability zones).

- **Threshold model:** per-dataset `regional_threshold = { required_regions r, total_regions R }`; a write is regionally durable only after `r` of `R` regions hold a durable (synced) copy. Cloud Storage regional buckets confirm writes only after data is redundantly stored across ≥2 AZs; dual-region/multi-region buckets add cross-region redundancy (11 nines durability target). Azure ZRS = synchronous across 3+ AZs in one region (12 nines); GRS/GZRS = geo-redundant across regions (16 nines).
- **Anti-affinity:** copies must be spread across distinct failure domains (spacedb-durability `allocate` spreads shards across failure domains; requires at least `num_shards` targets). A region threshold of `r` with all copies in one AZ is meaningless — the crate enforces failure-domain diversity as a precondition.
- **Durability targets:** 11 nines (99.999999999% annual) is the common cloud target for object storage; expressed per dataset as a configurable durability SLO, not a hard-coded constant.

#### 3.4 Cross-region durability

Cross-region durability protects against whole-region outages.

- **Async vs sync cross-region:** synchronous cross-region commit maximizes durability but adds inter-region latency to every write; asynchronous cross-region replication (Cloud Storage default replication: 99% within 1 hour target, 100% within 12 hours) trades a bounded replication lag for write latency. Giza (Azure Storage, USENIX ATC '17) applies erasure coding across global data centers to lower geo-redundancy storage cost vs three-way replication, with Local Reconstruction Coding within a DC.
- **RPO/RPO exposure:** async cross-region implies a recovery-point window (unreplicated writes lost in a region outage); the crate must expose the cross-region replication lag bound per dataset rather than conceal it.
- **Erasure coding across regions:** k-of-n Reed-Solomon shards spread across regions tolerate loss of up to `n−k` regions (spacedb-durability; Ceph default erasure profile k=2,m=2 tolerates 2 OSD losses with 2× storage overhead vs 3× for replication). RustFS automatic parity derives parity per erasure-set width (EC:0–EC:4).
- **Cross-region is not local persistence:** a regionally durable write still requires each region's local synced copy; cross-region redundancy composes on top of local persistence and quorum, never replaces them.

#### 3.5 Acknowledgement semantics

Acknowledgement semantics define what a "success" response promises — the central durability claim.

- **Ack levels:** `Unacknowledged` (success after send, no durability), `LeaderAck` (leader applied in memory — leader failure before replication may lose it), `QuorumAck` (quorum of replicas applied — survives single failure with n=3), `AllAck` (all replicas / all ISR — highest durability, blocks on any down replica). Kafka `acks=0/1/all`; Couchbase `durabilityLevel`; MongoDB `w` + `j`.
- **Ack basis (in-memory vs durable):** an ack may mean "applied in memory" or "persisted to stable storage". MongoDB `j:true` forces on-disk journal before ack; `writeConcernMajorityJournalDefault` decides whether `w:majority` implies journaling. Iggy `replicated` (quorum commit + local application, no stable-storage barrier) vs `persisted` (quorum commit backed by recoverable stable-storage copies at quorum, then local application) — exactly the replicated-vs-persisted distinction. `ids-durability-policy` makes the ack basis explicit per dataset: `AckBasis::{Memory, Journal, StableStorage}`.
- **Ack transfer of ownership:** once acknowledged, the receiver assumes full responsibility for the message; the sender may release it (RabbitMQ reliability guide: acknowledgement = receipt + transfer of ownership). The crate must not let a caller retract an ack after it is emitted.
- **wtimeout / deadline:** quorum or all-ack can block indefinitely; a bounded deadline (`wtimeout`) converts indefinite blocking into `Unsatisfied`/`Undetermined` verdicts (MongoDB write concern timeout; Iggy behavior when insufficient sites are connected — returns failure immediately rather than waiting).

#### 3.6 Commit vs synchronization distinction

Commit and synchronization are separable durability boundaries; conflating them is the source of most durability bugs.

- **Commit** = logical acceptance and ordering of a write (the write is assigned a position/sequence and replicas agree on it). **Synchronization** = the durable stable-storage barrier (fsync/fdatasync/WAL-flush) that makes the committed write survive power loss. Oracle Berkeley DB: "Durability guarantees include two types of information: acknowledgment guarantees and synchronization guarantees." PostgreSQL `synchronous_commit` controls how much WAL processing completes before success; `wal_sync_method` controls the actual disk flush — independent knobs.
- **Ordering guarantee:** commit establishes order (WAL-before-data: changes to data files are written only after WAL records describing them are flushed — PostgreSQL WAL principle; SQLite write-ahead logging). Synchronization without ordering is insufficient for recoverable state.
- **Exposure window:** between commit and synchronization lies the durability window (dirty page cache, bounded by kernel writeback or `EveryN` group commit). A system that acks at commit but syncs later promises "survives process crash/clean shutdown, not power loss" — honest only if documented (MerceMay fsync durability boundary).
- **ids-durability-policy design:** per-dataset `DurabilityPolicy` carries two independently configurable boundaries: `commit_required` (quorum ack at commit) and `sync_required` (stable-storage barrier before ack). The verdict is `Satisfied` only when both are met; otherwise `Unsatisfied` or `Undetermined` (e.g., committed but not yet synced → `Undetermined` until the barrier clears or a timeout elapses).

---

### 4. Threat Model & Mitigations

| Threat | Impact | Mitigation |
|--------|--------|------------|
| Correlated failure (shared power loss, rack/region outage) destroys all synced copies | Permanent data loss despite "quorum" | Regional redundancy threshold + failure-domain anti-affinity; quorum computed over independent failure domains; document exposure window honestly |
| Ack emitted before stable-storage barrier | Acknowledged writes lost on crash | Separate commit vs sync; `SyncRequired` tier forces fsync/fdatasync + parent-directory sync before ack; `AckBasis::StableStorage` default for durable datasets |
| Silent data corruption (bit rot, torn writes) | Corrupt state treated as durable | Content hashing (blake3/sha256) over durable metadata + payload; checksum verification on read/replay; manifest with per-shard content hashes |
| New file created but directory entry not durable | File "exists" after crash but name not resolvable | fsync parent directory after creating any new dataset file |
| Quorum computed over dead/unelectable replicas | False durable verdict | Exclude non-electable replicas from quorum counts; require failure-domain diversity |
| Indefinite quorum blocking | Write availability loss | Bounded `wtimeout`; `Undetermined` verdict on timeout; caller retry/backoff policy |
| Crypto downgrade / non-allowed primitive | Violates allowed-crypto constraint | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 from ids-000-aa/ids-crypto; lint to reject std crypto and any other primitive |
| Carrier assumption (TCP/IP etc.) | Violates carrier-agnostic constraint | All traffic via `ids-carrier::Endpoint`; compile-fail (no_std-friendly, no `std::net`/`tokio::net`) |

---

### 5. Integration with Existing Crates

- **ids-000-aa (master crate):** source of all canonical types and sealed trait contracts consumed by this crate — `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, sealed traits, error types. `ids-durability-policy` references these and defines no new canonical identifier types. Sealed-trait pattern (Rust API Guidelines C-SEALED) used for any trait this crate defines that must remain implementable only within the crate.
- **ids-carrier:** all outbound durability signals flow as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no direct transport dependency.
- **ids-crypto:** allowed primitives (ed25519 signatures, xchacha20poly1305/chacha20poly1305 AEAD, sha256/blake3 hashing) consumed by primitive ID; `ids-durability-policy` does not implement crypto itself.
- **ids-store / local store:** the durability policy engine is queried by the local store before acknowledging a per-dataset write; it is a pure policy function (dataset id + write outcome → verdict + ack level) with no store implementation dependency.
- No dependency on `std::net`, `tokio::net`, libp2p, or any networking crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

A separate crate `ids-durability-policy` at `C:/idfs/ids-durability-policy/`, organized into subfolders by durability dimension (not a monolithic file). Explicitly enumerated tree below totals ~125 files; property-based, chaos, and conformance test generation scales the tree toward 1,000 files.

```
ids-durability-policy/
├── Cargo.toml
├── README.md
├── LICENSE
├── CONTRACTS.md
├── SECURITY.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── types.rs               # DurabilityPolicy, PersistenceTier, AckLevel, AckBasis, DurabilityVerdict, CommitSyncBoundary
│   ├── policy.rs              # per-dataset requirement evaluation
│   ├── persistence/           # local persistence engine (~20 files)
│   │   ├── mod.rs, durable_dir.rs, wal.rs, checkpoint.rs, atomic_write.rs
│   │   ├── sync_policy.rs, crc.rs, manifest.rs, recovery.rs, exposure.rs
│   ├── quorum/                # quorum persistence (~15 files)
│   │   ├── mod.rs, quorum_config.rs, arithmetic.rs, replica_tracker.rs
│   │   ├── ack_policy.rs, failure_domain.rs, quorum_report.rs
│   ├── regional/              # regional redundancy threshold (~12 files)
│   │   ├── mod.rs, region_threshold.rs, anti_affinity.rs, region_report.rs
│   ├── cross_region/          # cross-region durability (~15 files)
│   │   ├── mod.rs, cross_region_mode.rs, erasure_coding.rs, rs_k_of_n.rs
│   │   ├── replication_lag.rs, geo_redundancy.rs, rpo.rs
│   ├── ack/                   # acknowledgement semantics (~12 files)
│   │   ├── mod.rs, ack_level.rs, ack_basis.rs, verdict.rs, timeout.rs
│   ├── commit_sync/           # commit vs synchronization distinction (~10 files)
│   │   ├── mod.rs, commit_boundary.rs, sync_barrier.rs, ordering.rs
│   ├── crypto/                # allowed-primitive wrappers (~10 files)
│   │   ├── mod.rs, ed25519.rs, xchacha20poly1305.rs, sha256.rs, blake3.rs, chacha20poly1305.rs, primitive_ids.rs
│   └── tests/                 # unit / property / chaos / conformance (~30+ files)
│       ├── unit/, property/, chaos/, conformance/
├── benches/                   # durability-throughput / exposure-window benchmarks
├── tests/
└── .cargo/config.toml
```

Carrier-agnostic constraint enforced structurally: the `src/` tree has no transport modules and no `std::net`/`tokio::net` references; every outbound message is typed as `Vec<u8>` addressed to `ids-carrier::Endpoint`.

---

### 7. Key Algorithms & Data Structures

**`DurabilityPolicy[dataset_id]`** (per dataset): `{ local_tier: PersistenceTier, quorum: {n, w}, regional: {r, R}, cross_region: CrossRegionMode, ack_level: AckLevel, ack_basis: AckBasis, sync_required: bool, wtimeout: Duration }`. Evaluated per write.

**Persistence tier evaluation:** `durable(local_tier)` → `MemoryOnly` (no barrier) | `Journaled` (WAL appended) | `Synced` (fsync/fdatasync + parent-dir sync crossed) | `SyncedAndReplicated` (synced + quorum). Exposure window reported per tier.

**Quorum arithmetic:** write quorum `w` and read quorum `r` valid iff `w + r > n` and `2w > n`; tolerates `⌊(n−1)/2⌋` replica failures; quorum reports exclude non-electable/dead replicas.

**Regional check:** `regionally_durable` iff `synced_regions ≥ r` across `R` regions with failure-domain anti-affinity satisfied (no two required copies in the same AZ/failure domain).

**Commit vs sync separator:** `Commit` (write accepted + ordered) and `Sync` (stable-storage barrier reached) computed independently; `verdict = Satisfied` iff commit ∧ sync (per policy); `Unsatisfied` iff commit ∧ ¬sync and timeout elapsed; `Undetermined` otherwise.

**Content-addressed durability manifest:** blake3/sha256 hash over durable metadata + payload per shard/copy; corrupted or substituted copy detected at verification (spacedb-durability manifest pattern).

**Allowed crypto:** ed25519 (durable-record signatures), xchacha20poly1305/chacha20poly1305 (AEAD for durable payloads at rest), sha256/blake3 (content hashing, manifest integrity). No other primitives.

**Sealed traits:** any extension trait (e.g., `DurabilityEvaluator`) sealed via `private::Sealed` supertrait so only this crate implements it (Rust API Guidelines C-SEALED).

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (NodeId, RegionId, IdsTime, CarrierType, RecordId, Checksum) + sealed trait contracts + error types |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — sole traffic channel |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |
| `tokio` | Async runtime (test harness only; no `tokio::net` in crate code) |

**No networking crates:** `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS are forbidden — all traffic via `ids-carrier::Endpoint`. **Allowed crypto only:** ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Local persistence tier | `Synced` survives single-node power loss; `MemoryOnly` does not | crash/power-loss fault-injection test per tier |
| fsync/fdatasync barrier | fsync + parent-directory sync before ack for new dataset files | unit test + fsync-failure injection (writer poisoned on fsync failure) |
| Quorum arithmetic | `w + r > n` and `2w > n` enforced; tolerates ⌊(n−1)/2⌋ failures | property test over n/w/r combinations |
| Quorum ack verdict | `Satisfied` only when `w` replicas durably persisted | replica-tracker unit test with dead/unelectable replica exclusion |
| Regional threshold | `r` of `R` regions synced, failure-domain anti-affinity satisfied | region-report unit test; reject same-AZ placement |
| Cross-region | async replication lag bound exposed per dataset | RPO benchmark (lag ≤ configured bound) |
| Commit vs sync | commit and sync boundaries independently configurable; verdict `Satisfied` iff both met | boundary-separator unit test; `Undetermined` when committed-not-synced |
| Acknowledgement levels | Unacknowledged / Leader / Quorum / All with `AckBasis::{Memory, Journal, StableStorage}` | unit test per level; `wtimeout` converts block to `Unsatisfied` |
| Content integrity | blake3/sha256 over durable metadata + payload; corruption detected | manifest verification unit test (tampered shard rejected) |
| Crypto constraint | only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | dependency/cargo audit + primitive-ID allowlist test |
| Carrier-agnostic | compiles with zero `std::net`/`tokio::net`/libp2p references; all traffic via `ids-carrier::Endpoint` | build lint + dependency scan |
| Property tests | 100,000+ iterations | proptest/bolero |
| Chaos tests | partition, offline member, replica churn | pass/fail |

---

### 10. Remaining Uncertainties

1. **Quorum size vs. availability trade-off per dataset:** `w = quorum` maximizes durability but blocks on minority failure; `w = n` (all) maximizes durability but zero fault tolerance for availability. Per-dataset policy must be product-configured, not research-determined.
2. **Cross-region replication lag bound:** acceptable RPO per dataset is workload-dependent; unquantified until deployment targets exist.
3. **fsync failure handling:** a failed fsync poisons the writer (durability crate); recovery path (restart from WAL vs. fail the dataset) needs an operator decision.
4. **Wire format for durability signals:** CBOR vs bincode vs protobuf for the opaque `Vec<u8>` handed to `ids-carrier::Endpoint`; CBOR favored for compactness/schema evolution but crate TBD.
5. **Tombstone / WAL reclamation:** durable WAL growth unbounded without checkpoint-compaction; retention policy (size/age/ack-based) deferred.
6. **Formal verification scope:** TLA+/PlusCal spec proving quorum + commit/sync separation correctness is additional effort beyond this plan.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-durability-policy` | Follows "separate crates instead of more folders in existing ones"; clean dependency boundary |
| Commit vs. sync as separate boundaries | Oracle Berkeley DB model: durability = acknowledgment guarantees + synchronization guarantees; prevents overclaiming |
| `Synced` tier default for durable datasets | fsync/fdatasync barrier before ack survives power loss (durability crate, quipu-log SyncPolicy `Always`/`EveryN`) |
| Parent-directory fsync after file creation | New-file durability trap (kernel internals; PostgreSQL/LevelDB practice) |
| blake3 preferred, sha256 allowed | content hashing/manifest integrity (spacedb-durability pattern); within allowed-crypto set |
| ed25519 signatures for durable records | non-repudiation of durability verdicts; within allowed-crypto set |
| Quorum `w + r > n`, `2w > n` | intersecting write quorums prevent divergent committed state (Apache Iggy quorum rules, Cassandra consistency) |
| Failure-domain anti-affinity for regional threshold | region copies in one AZ defeat regional redundancy (spacedb-durability placement; Cloud Storage/Azure AZ semantics) |
| Bounded `wtimeout`; `Undetermined` verdict | prevents indefinite quorum block (MongoDB wtimeout; Iggy immediate-failure when insufficient sites) |
| All traffic via `ids-carrier::Endpoint` | carrier-agnostic mandate; no `std::net`/`tokio::net`/libp2p |
| Research only — no code, no source modifications | per task constraint; this document is the implementation blueprint |

---

### Sources

- `durability` crate — local persistence primitives (directories, atomic writes, WALs, checkpoints, CRC, sync helpers): https://crates.io/crates/durability ; https://docs.rs/durability/latest/durability/
- `spacedb-durability` — Reed-Solomon k-of-n erasure-coded snapshots, content-addressed manifest, blake3, anti-affinity placement: https://docs.rs/spacedb-durability/latest/spacedb_durability ; https://crates.io/crates/spacedb-durability
- MongoDB write concern (`w`, `j`, `wtimeout`, majority, `writeConcernMajorityJournalDefault`): https://www.mongodb.com/docs/manual/reference/write-concern
- Couchbase durable writes (`Majority` / `MajorityAndPersistToActive` / `PersistToMajority`): https://docs.couchbase.com/c-sdk/current/concept-docs/durability-replication-failure-considerations.html
- Apache Iggy cluster durability (`replicated` vs `persisted`, quorum rules): https://iggy.apache.org/docs/clustering/durability ; https://iggy.apache.org/docs/server/durability/
- PostgreSQL `synchronous_commit` and `wal_sync_method`: https://www.postgresql.org/docs/current/runtime-config-wal.html
- fsync vs fdatasync, new-file directory durability trap: https://kernel-internals.org/io/fsync-fdatasync
- RabbitMQ publisher confirms / quorum queues (ack after quorum, per-message persistence): https://www.rabbitmq.com/docs/confirms ; https://www.rabbitmq.com/docs/quorum-queues ; https://www.rabbitmq.com/docs/reliability
- Kafka `acks` (0/1/all) with `min.insync.replicas`: https://www.conduktor.io/kafka/kafka-producer-acks-deep-dive
- Cloud Storage availability & durability (11 nines, erasure coding, ≥2 AZ confirmation, cross-region replication): https://docs.cloud.google.com/storage/docs/availability-durability
- Azure Storage redundancy (LRS/ZRS/GRS/GZRS tiers, AZ/region outage matrix): https://learn.microsoft.com/en-us/azure/storage/common/storage-redundancy
- Berkeley DB Replication Manager ack policies (NONE/ONE_PEER/QUORUM/ALL; ack vs synchronization guarantees): http://libdb.org/reference/guides/programmer_reference/rep_mgr_ack.html ; http://docs.oracle.com/cd/E26161_02/html/GettingStartedGuide/durability.html
- Cassandra consistency levels (ALL/QUORUM/LOCAL_ONE/ONE/TWO/THREE): http://docs.datastax.com/en/dse/6.9/architecture/database-internals/configure-consistency.html
- quipu-log SyncPolicy (Always/EveryN/OsManaged, group commit, exposure window): https://quipu-log.github.io/quipu-log-textbook/11-durability-policy.html
- durable-ioring (epoch-based durability points, high-water mark, no_std): https://docs.rs/crate/durable-ioring/latest
- iqdb-persist `FsyncPolicy` (Always/Periodic/Never): https://docs.rs/iqdb-persist/latest/iqdb_persist/enum.FsyncPolicy.html
- persistent-queue (Sync/Group/None durability policies): https://crates.io/crates/persistent-queue
- Giza: erasure coding objects across global data centers (USENIX ATC '17): http://users.ece.cmu.edu/~ganger/papers/usenix2000.pdf
- SQLite atomic commit / WAL (fsync at commit points): https://sqlite.org/atomiccommit.html ; https://sqlite.org/wal.html
- RustCrypto ChaCha20Poly1305 / XChaCha20Poly1305 (RFC 8439): https://docs.rs/chacha20poly1305/latest/chacha20poly1305
- Rust API Guidelines — sealed traits (C-SEALED, `private::Sealed`): https://rust-lang.github.io/api-guidelines/future-proofing.html
- IDS Dataspace Protocol (DCAT, ODRL, contract negotiation, transfer process): https://dobachi.github.io/memo-blog/2023/08/31/IDS-Dataspace-Protocol/
- IDS Policies (access/contract/usage policies, prohibitions, obligations): https://kb.internationaldataspaces.org/external/rulebook/105_Policies/
- AWS Fault Isolation Boundaries (control plane vs data plane): https://docs.aws.amazon.com/pdfs/whitepapers/latest/aws-fault-isolation-boundaries/aws-fault-isolation-boundaries.pdf

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. This plan document defines all architecture, dimensions, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 058 (ids-durability-policy).
