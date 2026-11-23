## Phase 041 — Create the ids-reclaim crate: Compact fragmented storage, reclaim space.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 610-624)

```
41. Research only do not read plan.md **[Phase 041 — Create the ids-reclaim crate: Compact fragmented storage, reclaim space.]**

Crate: ids-reclaim
Purpose: Compact fragmented storage, reclaim space.
Contents: Fragmented storage compaction, obsolete version removal, tombstone expiry, eligible data expiration, space amplification reduction, snapshot/retention compatibility, compaction scheduling, reclamation verification.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 041 — Create the ids-reclaim crate: Compact fragmented storage, reclaim space.]**

Create plan-041.md in C:/idfs/plans/ for the ids-reclaim crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 041 creates `ids-reclaim`, the IDS crate that compacts fragmented storage and reclaims eligible space. It owns fragmented-storage compaction, obsolete-version removal, tombstone expiry, eligible-data expiration, space-amplification reduction, snapshot/retention compatibility, compaction scheduling, and reclamation verification. `ids-reclaim` is a **guard-and-sweep layer**: it does not own storage; it evaluates eligibility, schedules compactions, and verifies reclamation, coordinating with `ids-immutable` (Phase 040 guard) so retained/held records are never reclaimed. It operates out-of-place (copy-live-forward, never overwrite-in-place), keeping old versions visible to concurrent snapshots until compaction commits atomically.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed from `ids-crypto`/`ids-000-aa`).

Research basis: 30 parallel searches returned RocksDB tiered/leveled/FIFO compaction and compaction scoring (level fill factor, tombstone threshold, TTL)[^1][^4]; LSM compaction design space and delete-aware file picking (tombstone count/age)[^5]; WiscKey key/value separation with separate value-log GC[^4]; Cassandra/ScyllaDB STCS/LCS/ICS/TWCS tombstone thresholds (`tombstone_threshold` default 0.2, `tombstone_compaction_interval` default 1 day)[^6][^7]; leanstore-style page compaction and B-tree[^8]; Pebble compaction kinds incl. delete-only/tombstone-density/rewrite[^9]; TigerBeetle incremental LSM compaction and move-table optimization[^10]; SQLite VACUUM full rebuild / auto_vacuum incremental[^11]; WiredTiger cooperative Btree+BlockManager compaction with checkpoint prerequisite[^12]; Apache Iceberg `expire_snapshots` + orphan-file removal[^13]; Delta Lake `OPTIMIZE` bin-packing + `VACUUM` tombstoned-file deletion[^14]; Sprite LFS segment cleaning (live-block copy-forward, cost/benefit segment selection)[^15]; log-structured storage GC write-amplification model (invalid vs valid block rewrites)[^16]; MVCC old-version garbage collection bottleneck[^2]; Delta Lake small-file compaction and checkpoint log compaction[^14]; space/reclamation metrics (logical vs physical bytes, reclaimed per pass)[^14]. This document is the research blueprint only — no code written, no files modified.

---

### 2. Architecture Overview

`ids-reclaim` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a **compaction-and-reclamation service**: it evaluates fragment/file eligibility, selects compaction victims, schedules and runs compactions out-of-place, and verifies reclaimed bytes. It defers to `ids-immutable` (Phase 040) for retention/hold vetoes — `ids-reclaim` never reclaims a record under an active legal hold or unexpired retention.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (mutation issuers)             │
│  ids-store, ids-fragment, ids-replication issue writes/deletes;             │
│  ids-immutable (Phase 040) gates retention/hold; ids-reclaim (THIS CRATE)   │
│  compacts eligible fragments and reclaims space                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                 Phase 041: ids-reclaim (THIS CRATE — sweep layer)           │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │ eligibility    │ │ victim         │ │ compaction     │ │ reclamation  │ │ │\n│  │ evaluator      │ │ selection      │ │ scheduler      │ │ verifier     │ │ │\n│  │ (tombstone     │ │ (cost/benefit, │ │ (out-of-place, │ │ (bytes       │ │ │\n│  │  expiry,       │ │  space-amp,    ││  copy-live-fwd,│ │  reclaimed,    │ │ │\n│  │  obsolete ver, │ │  snapshot-safe)│ │  atomic commit)  │ │  audit log)  │ │ │\n│  └────────────────┘ └────────────────┴────────────────┴──────────────┘ │ │\n│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │\n│  │ snapshot/      │ │ space-amp      │ │ compaction       │ │ carrier      │ │ │\n│  │ retention      │ │ reduction      │ │ strategy (leveled│ │ adapter      │ │ │\n│  │ compatibility  │ │ metrics        │ │ /tiered/universal│ │ (Endpoint)   │ │ │\n│  └────────────────┘ └────────────────┴────────────────┴──────────────┘ │ │\n└────────────────────────────────────────────────────────────────┬──────────┘
                                                                   │ opaque Vec<u8> datagrams
┌─────────────────────────────────────────────────────────────────▼──────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8]) │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a compaction cycle:**
1. **Eligibility evaluation:** each fragment/file is scored for tombstone density, obsolete-version ratio, expired TTL, and snapshot/retention safety. `ids-immutable` vetoes any record under retention/hold/compliance.
2. **Victim selection:** cost/benefit model picks candidates (highest reclaimable bytes per unit of rewrite I/O), respecting concurrent-snapshot pinning (versions visible to any open snapshot are retained).
3. **Compaction scheduling:** out-of-place copy-live-forward into new fragment files; old fragment files become obsolete only after the new set is durably committed and the manifest atomically swaps.
4. **Reclamation verification:** post-compaction byte delta (bytes_before − bytes_after) is recorded to an append-only, hash-chained reclamation audit log; obsolete physical files are deleted only after verification and after no reader/snapshot can reference them.
5. All compaction metadata travels as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-reclaim` never touches a carrier directly.

---

### 3. Detailed Research Dimensions

#### 3.1 Fragmented storage compaction (out-of-place copy-live-forward)
Fragmentation arises from out-of-place updates/deletes: newer versions shadow older ones, deletes leave tombstones, and small incremental writes create many small fragments. Compaction rewrites overlapping fragments into fewer, larger ones, dropping shadowed/obsolete versions and expired tombstones [^1][^15]. The copy-live-forward pattern (read live blocks, write to new segment, free old segment) mirrors Sprite LFS segment cleaning [^15] and is crash-safe because old files remain intact until the new set is atomically published [^2][^14].

#### 3.2 Obsolete version removal (MVCC garbage collection)
Out-of-place engines retain multiple versions per key for snapshot isolation; obsolete versions are reclaimed only when no open snapshot/reader can reference them [^2][^22]. Version-chain GC is a known HTAP bottleneck [^2]; the reclaimer must pin versions by oldest-active-snapshot sequence and drop versions strictly older than every retained snapshot's read point [^22]. `ids-reclaim` treats snapshot pins as hard retention: versions visible to any pinned snapshot are never removed, preserving snapshot/retention compatibility.

#### 3.3 Tombstone expiry
Deletes are recorded as tombstones rather than in-place removal [^6][^9][^16]. A tombstone is droppable only when (a) its delete is visible at all levels below (no older value can be resurrected), (b) it is older than the tombstone expiry / `gc_grace_seconds` window, and (c) every replica has compacted past it (MTRO/MCCO-style convergent rule [^12]). ScyllaDB uses a `tombstone_threshold` (default 0.2 tombstone density) plus `tombstone_compaction_interval` (default 1 day) to trigger single-SSTable tombstone compaction [^6][^7]; `ids-reclaim` adopts the same threshold + interval pattern.

#### 3.4 Eligible data expiration (TTL / retention sweep)
Records past their retention expiry (from `ids-immutable`) or TTL become eligible for disposition (retain/review/purge/archive) [^23][^24]. The eligibility evaluator consults `ids-immutable`'s policy engine; only records with an approved `purge` disposition enter compaction candidates. Expired-but-retained records keep a tombstone row (sequence range, Merkle root, signed checkpoint) forever — never a false gap [^24].

#### 3.5 Space amplification reduction
Space amplification = physical bytes / logical bytes [^1][^16]. Compaction reduces it by dropping obsolete versions and tombstones and coalescing small fragments. The cost/benefit score for a victim fragment = reclaimable_bytes / rewrite_io_bytes; candidates exceeding the threshold are compacted [^1][^5]. Temporary space overhead during compaction (old + new files coexist) is bounded by compacting one fragment set at a time [^1][^2].

#### 3.6 Snapshot / retention compatibility
Compaction must not break open snapshots: any version visible to a pinned snapshot (sequence ≤ snapshot read point) is retained even if obsolete [^2][^22]. `ids-immutable` retention/hold/compliance levels are hard vetoes: held/compliant records are never reclaimed; retention-only records are reclaimed only after `retain_until_date` passes [^23]. This matches Iceberg `expire_snapshots` (data files deleted only when unreferenced by any retained snapshot) [^13].

#### 3.7 Compaction scheduling
Compactions run as background jobs with configurable priority, rate limits, and write-stall backpressure (RocksDB-style: slowdown/stop thresholds on pending compaction bytes) [^1][^9]. Scheduling strategies: time-based (periodic sweep), threshold-based (tombstone density / space-amp / fragment-count triggers), and event-based (post-reclaim queue drain). Compaction is never allowed to starve foreground writes; backpressure stalls writes when pending work exceeds a hard cap [^1][^9].

#### 3.8 Reclamation verification
Each compaction run records bytes_before, bytes_after, tombstones_dropped, versions_dropped, and fragments_merged into an append-only, hash-chained (SHA-256/BLAKE3) reclamation audit log with an ed25519-signed checkpoint [^24]. Verification recomputes the chain and confirms `reclaimed_bytes == bytes_before − bytes_after` and that no live/retained version was removed. Physical deletion of obsolete fragment files happens only after the signed checkpoint is durable.

---

### 4. Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Reclaiming a retained/held/compliant record | Data durability / compliance | Hard veto via `ids-immutable` guard; hold/compliance unconditional; retention until `retain_until_date` |
| Resurrecting deleted data across replicas | Convergent deletion | Tombstone removal gated on MTRO/MCCO (all replicas compacted past) + `gc_grace_seconds` overlap |
| Removing a version visible to an open snapshot | Snapshot correctness | Oldest-snapshot pinning; versions ≤ any pinned read point are retained |
| Truncation / falsification of reclamation audit log | Audit integrity | SHA-256/BLAKE3 hash chain + ed25519-signed checkpoints; verification recomputes chain |
| Compaction crash leaving half-built fragments | Crash safety | Out-of-place copy-live-forward; atomic manifest swap; old files kept until new set durable |
| Unbounded compaction I/O stalling writes | Availability | Rate limits, backpressure (slowdown/stop thresholds), one fragment set at a time |
| Reclaim storm deleting too much too fast | Availability | Disposition gates (retain/review/purge/archive), grace days, per-cycle byte caps |
| Carrier re-framing / tampering of compaction metadata | Integrity of sweep decisions | Metadata signed over payload bytes; opaque `Endpoint` datagrams survive re-framing |

The carrier is never trusted: compaction decisions and reclamation proofs are signed over payload bytes, so they survive re-framing by any carrier.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-reclaim |
|-------|------|--------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `RecordId`, `FragmentId`, `SnapshotId`, `Checksum`, `IdsTime`, `CarrierType`, `OpState`, `CommitState` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive | All compaction/reclamation metadata travels as datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signatures, content hashes for reclamation audit chain and checkpoints |
| `ids-immutable` (Phase 040) | Retention/hold/compliance guard | Hard veto: ids-reclaim never reclaims retained/held/compliant records |
| `ids-store` | Primary record store (mutation issuer) | Produces fragments/versions/tombstones that ids-reclaim compacts |
| `ids-fragment` | Record fragments/shards | Compaction unit; id-reclaim selects and rewrites eligible fragments |
| `ids-replication` (Phase 017) | Replica synchronization | Replicates tombstones; convergent deletion via MTRO/MCCO across replicas |
| `ids-snapshot` (Phase 026) | Distributed snapshots (Chandy-Lamport) | Snapshot pins block version removal until snapshot releases |
| `ids-integrity` (Phase 030) | End-to-end checksums, Merkle trees, scrubbing | Reuses `Checksum`/Merkle primitives; compaction verifies integrity of rewritten fragments |
| `ids-backup` (Phase 042) | Recovery points and incremental backups | Backup copies pinned from reclamation until backup retention expires |

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-reclaim/` (a separate crate, not a folder inside an existing crate):

```
ids-reclaim/
├── Cargo.toml
├── src/
│   ├── lib.rs                      # crate root, public API surface
│   ├── errors.rs                   # ReclaimError, EligibilityError, CompactionError, VerificationError
│   ├── types.rs                    # FragmentId, EligibilityScore, CompactionJob, ReclamationRecord (re-exports ids-000-aa types)
│   ├── eligibility.rs              # eligibility evaluator (tombstone density, obsolete ratio, TTL, snapshot pin)
│   ├── victim_selection.rs         # cost/benefit victim picker (reclaimable bytes / rewrite I/O)
│   ├── compaction.rs               # compaction orchestrator (out-of-place copy-live-forward)
│   ├── scheduling.rs               # compaction scheduler (time/threshold/event triggers, rate limits, backpressure)
│   ├── copy_forward.rs             # out-of-place live-record copy into new fragment files
│   ├── obsolete_removal.rs         # obsolete version removal (MVCC snapshot-pinned)
│   ├── tombstone.rs                # tombstone lifecycle, expiry, MTRO/MCCO gated removal
│   ├── retention_compat.rs         # snapshot/retention compatibility checks (ids-immutable veto integration)
│   ├── space_amp.rs                # space-amplification metrics (physical/logical bytes)
│   ├── reclamation.rs              # reclamation verification + physical file deletion
│   ├── audit.rs                    # append-only hash-chained reclamation audit log
│   ├── checkpoint.rs               # periodic ed25519-signed reclamation checkpoint
│   ├── carrier_adapter.rs          # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
│   ├── contracts/                  # sealed trait contracts (~10 files)
│   │   ├── eligibility_contract.rs, victim_contract.rs, compaction_contract.rs,
│   │   │   tombstone_contract.rs, reclamation_contract.rs, audit_contract.rs,
│   │   │   checkpoint_contract.rs, scheduling_contract.rs, space_amp_contract.rs, mod.rs
│   ├── invariants/                 # compile-time/runtime invariants (~8 files)
│   │   ├── no_reclaim_held.rs, snapshot_pin_respected.rs, tombstone_gated_removal.rs,
│   │   │   atomic_manifest_swap.rs, chain_integrity.rs, carrier_agnostic.rs, crypto_allowlist.rs, mod.rs
│   ├── tests/                      # property/verification tests (~20 files)
│   ├── benches/                    # (~6 files)
│   └── docs/                       # (~12 files)
```
Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **Eligibility score:** `score = reclaimable_bytes / rewrite_io_bytes` per fragment; inputs = tombstone density, obsolete-version ratio, expired TTL, snapshot-pin status, ids-immutable veto. Victim picked when score ≥ threshold and no veto.
- **Out-of-place compaction:** read live (non-shadowed, non-expired) records from N input fragments → merge-sort → write to M new fragment files → atomic manifest swap → old fragments marked obsolete → physical deletion after verification + no reader pin.
- **Obsolete version removal:** version `v` droppable iff `v < oldest_pinned_snapshot_read_point` and a newer visible version exists for the same key; tombstone `t` droppable iff `t` expired + `t` ≤ min(MCCO across replicas) + `gc_grace_seconds` elapsed.
- **Tombstone expiry trigger:** `tombstone_threshold` (default 0.2 tombstone density) × `tombstone_compaction_interval` (default 1 day) — ScyllaDB/Cassandra pattern.
- **Space amplification:** `space_amp = physical_bytes / logical_bytes`; compaction target reduces it toward 1.0.
- **Hash chain + signed checkpoint:** each reclamation record SHA-256/BLAKE3-chained to previous digest; periodic ed25519-signed checkpoint anchors; verification recomputes chain and confirms reclaimed-byte delta.
- **Backpressure:** slowdown threshold on pending compaction bytes; stop threshold halts new compaction jobs until pending work drains.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`RecordId`, `FragmentId`, `SnapshotId`, `Checksum`, `IdsTime`, `CarrierType`, `OpState`, `CommitState`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-immutable` (Phase 040) — retention/hold/compliance veto surface (read-only).
- `serde` — serialization of compaction jobs, reclamation records, checkpoints.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-41.1 | Retained/held/compliant records never reclaimed | 100% of reclaim attempts on held/retention-active/compliance records rejected by ids-immutable veto |
| AC-41.2 | Tombstone removal only after expiry + grace + MTRO/MCCO | Zero premature tombstone removals under replica lag/partition (adversarial test) |
| AC-41.3 | Snapshot-pinned versions never removed | Versions ≤ any pinned snapshot read point retained across compaction runs |
| AC-41.4 | Reclamation verification detects byte-delta fraud | `reclaimed == before − after` verified; mismatches fail closed and are audit-logged |
| AC-41.5 | Audit chain integrity | 100% of single-record insert/delete/reorder in reclamation log detected; verification names offending entry |
| AC-41.6 | Space amplification reduced | Compaction run reduces `physical/logical` bytes; measured pre/post per fragment set |
| AC-41.7 | Out-of-place crash safety | Interrupted compaction leaves old fragments intact; recovery resumes or rolls back without data loss |
| AC-41.8 | Backpressure bounds | Writes stalled only when pending compaction bytes ≥ hard threshold; resume below slowdown threshold |
| AC-41.9 | Carrier-agnostic | Zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS; all traffic via `ids-carrier::Endpoint` |
| AC-41.10 | Allowed crypto only | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 appear in crypto surface; nothing else |

---

### 10. Remaining Uncertainties

1. **Compaction granularity:** whole-fragment vs sub-fragment range compaction — trade-off between rewrite I/O and reclaim precision; unquantified for IDS fragment sizes.
2. **Tombstone grace window tuning:** `gc_grace_seconds` default (ScyllaDB 10 days) balances resurrection safety vs metadata bloat; IDS deployment-specific tuning needed.
3. **Compaction vs snapshot tension:** long-running snapshots pin versions and block reclamation, risking space exhaustion; snapshot lease/timeout policy undefined.
4. **Physical file deletion timing:** delete obsolete files immediately after commit vs deferred (reclaim-only-when-space-pressure) — affects write amplification during space crises.
5. **Cross-replica MTRO stall:** an unavailable replica stalls tombstone removal (correct but delays reclamation); operational eviction policy for dead replicas is out of scope.
6. **Compaction scheduling defaults:** time-based vs threshold-based vs event-based triggers; rate-limit and backpressure thresholds are placeholders pending measurement.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-reclaim` (not a subfolder) | User directive: separate crates instead of more folders; keeps reclamation concerns isolated |
| Out-of-place copy-live-forward compaction | Crash-safe; old fragments intact until atomic manifest swap; matches Sprite LFS / RocksDB / Iceberg patterns |
| Tombstone + obsolete version removal gated by snapshot pins | Prevents snapshot corruption and data resurrection; matches Iceberg `expire_snapshots` unreferenced-file rule |
| ids-immutable hard veto (retention/hold/compliance) | Guarantees compliance (SEC 17a-4 WORM) and legal-hold precedence over reclamation |
| MTRO/MCCO convergent tombstone removal | All replicas must compact past a tombstone before removal — prevents cross-replica resurrection (Kafka pattern) |
| Threshold-based compaction trigger (tombstone density 0.2, 1-day interval) | ScyllaDB/Cassandra proven pattern; simple and predictable |
| Hash-chained audit log + ed25519-signed checkpoints | Tamper-evident reclamation proof; offline verifiable; matches Phase 040 audit design |
| Only allowed crypto; carrier-agnostic `Endpoint` boundary | Complies with IDS carrier-agnostic and crypto allowlist rules |

---

### 12. Sources

[^1]: RocksDB — Compaction taxonomy (Classic Leveled, Tiered, Tiered+Leveled, Leveled-N, FIFO); compaction scoring by level fill factor and tombstone threshold. https://github.com/facebook/rocksdb/wiki/Compaction
[^2]: Bottcher et al. — MVCC garbage collection as HTAP performance bottleneck; long-running transactions pin versions. https://users.cs.utah.edu/~pandey/courses/cs6530/fall23/papers/mvcc/p128-bottcher.pdf
[^4]: Lu et al. — WiscKey: key/value separation, separate value-log GC. https://research.cs.wisc.edu/wind/Publications/joint/tos17-lu.pdf
[^5]: Constructing and Analyzing the LSM Compaction Design Space; delete-aware file picking (tombstone count/age), read/space/write amplification tradeoffs. https://arxiv.org/html/2202.04522v2
[^6]: ScyllaDB Docs — Compaction strategies STCS/LCS/ICS/TWCS; `tombstone_threshold` default 0.2, `tombstone_compaction_interval` default 86400s. https://docs.scylladb.com/manual/stable/cql/compaction.md
[^7]: ScyllaDB KB — Efficient Tombstone Garbage Collection in ICS; tombstone density threshold, cross-tier compaction, purgeable tombstones. https://docs.scylladb.com/manual/stable/kb/garbage-collection-ics.html
[^9]: CockroachDB Pebble — compaction kinds incl. delete-only, tombstone-density, rewrite; tombstone compensation in table size. https://github.com/cockroachdb/pebble/blob/master/compaction.go
[^10]: TigerBeetle — incremental LSM compaction, move-table optimization, manifest log compaction. https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/internals/lsm.md
[^11]: SQLite — VACUUM full rebuild, auto_vacuum FULL/INCREMENTAL, incremental_vacuum. https://system.data.sqlite.org/home/doc/8e13c43294410407/Doc/Extra/Core/lang_vacuum.html
[^12]: WiredTiger — cooperative Btree+BlockManager compaction, checkpoint prerequisite, block relocation and truncation. https://source.wiredtiger.com/mongodb-6.0/arch-compact.html
[^13]: Apache Iceberg — Maintenance: `expireSnapshots`; data files deleted only when unreferenced by retained snapshot. https://iceberg.apache.org/docs/latest/maintenance
[^14]: Delta Lake — `OPTIMIZE` bin-packing, `VACUUM` tombstoned-file deletion, checkpoint log compaction, auto compaction. https://docs.delta.io/optimizations-oss
[^15]: Ousterhout et al. — Sprite LFS design: segment cleaning, copy-live-forward, cost/benefit segment selection. https://web.stanford.edu/~ouster/cgi-bin/papers/lfs.pdf
[^16]: Wang et al. — Log-structured storage GC write-amplification model (invalid vs valid block rewrites). https://usenix.org/system/files/fast22-wang.pdf
[^22]: Bingham et al. — Constructing and Analyzing the LSM Compaction Design Space (snapshot pinning, version retention). https://arxiv.org/html/2202.04522v2
[^23]: Open Archiver — Retention Policy Engine: max-duration-wins, legal hold short-circuits, disposition retain/review/purge/archive, grace days. (Cited in Phase 040 plan.)
[^24]: Prooflog / ATL — retention keeps tombstone row + signed checkpoint forever; expired-but-retained verifiable, not a gap. (Cited in Phase 040 plan.)

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 041.
