## Phase 042 — Create the ids-backup crate: Recovery points and incremental backup histories.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 625-639)

```
42. Research only do not read plan.md **[Phase 042 — Create the ids-backup crate: Recovery points and incremental backup histories.]**

Crate: ids-backup
Purpose: Recovery points and incremental backup histories.
Contents: Snapshot metadata, captured versions, consistent reads, cloning, backup copies tracked separately from serving replicas, incremental backup history, point-in-time recovery, restore verification.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 042 — Create the ids-backup crate: Recovery points and incremental backup histories.]**

Create plan-042.md in C:/idfs/plans/ for the ids-backup crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 042 creates `ids-backup`, the IDS crate that owns recovery points and incremental backup histories. It provides point-in-time recovery: every backup is a complete, consistent, point-in-time snapshot (never a partial file copy), with a recovery-point history (full backup anchor + incremental deltas) that can be replayed to any point covered by the history. Restore verification confirms each recovery point is byte-identical and restorable before it is trusted. Backup copies are tracked separately from serving replicas — a backup copy is an independent, retention-protected copy that a compromised production node cannot alter or delete.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-backup` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-backup` crate does NOT yet exist at `C:/idfs/ids-backup/`. `ids-000-aa` already declares canonical types `RecordId` (`src/types.rs:262`), `SnapshotId` (`src/types.rs:1329`), `FragmentId` (`src/types.rs:1889`), `RecoveryProof` (`src/types.rs:1905`), `Checksum` (`src/types.rs:1975`), `RecoveryAction::RestoreRedundancy` (`src/types.rs:2455`), `OrderingPosSnapshot` (`src/types.rs:2355`), `IdsTime` (`src/types.rs:1069`), `CarrierType` (`src/types.rs:394`) — all referenced by this crate but never redefined. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned PostgreSQL continuous archiving / PITR (base backup + archived WAL replayed to any target time or named restore point; timeline IDs prevent WAL overwrite across recoveries)[^1][^2]; WAL archiving with off-site archive before segment recycling, `archive_timeout` bounding RPO on quiet systems[^2][^3]; Veeam / NAKIVO backup-chain models — forward incremental with periodic synthetic/active fulls as "chain breakers," forever-forward-incremental with synthetic-full merge, reverse-incremental (deprecated, incompatible with immutability)[^4][^5][^6]; synthetic full built from prior full + incrementals without reading the source again[^5]; chain fragility — one missing/corrupt incremental breaks the whole chain, requiring a new full backup[^29]; crash-consistent vs application-consistent snapshots (VSS writers flush pending I/O; crash-consistent = on-disk state as of power loss)[^18]; changed block tracking (CBT / Hyper-V Resilient Change Tracking) — bitmaps record changed blocks since a reference point so incrementals read only changed blocks; chain reset if tracking state is lost[^22][^23]; content-defined chunking (FastCDC / Rabin / Gear rolling hash; boundaries content-derived, so insertions shift only local boundaries; ~10–20% more dedup than fixed-size)[^20]; restic / Proxmox Backup / kit-msgvault repository models — content-addressed blobs, immutable append-only packs (~32 MB, sealed once, never modified), snapshot manifest written last (crash-safe), index rebuild from pack footers, prune+GC with grace period for concurrent writers[^24][^25][^26]; continuous data protection (CDP) — journal-based near-real-time change capture for near-zero RPO vs scheduled snapshots[^7]; RPO/RTO targets — RPO drives backup frequency, RTO drives recovery architecture; interval + run time ≤ RPO[^8]; immutable/air-gapped backup copies — logically air-gapped vaults, WORM, compliance vs governance mode, separate administrative boundary so compromised production identities cannot delete backups[^9][^10][^11]; restore verification / restore drills — automated periodic restore to isolated sandbox + checksum + data-usability assertions (AWS Well-Architected REL09-BP04, AWS Backup restore testing)[^13]; GFS grandfather-father-son tiered retention (weekly/monthly/yearly full backups as retention anchors)[^21]; backup checksums / pg_verifybackup — manifest-first verification: signature over canonical manifest proves digest authenticity, then per-artifact re-hash proves stored bytes still reproduce trusted digests; fail-closed on missing/invalid signature[^12][^13]; fsfreeze / LVM snapshot — filesystem freeze flushes dirty pages and journals for a consistent on-disk image before capture[^19]; differential vs incremental — incremental since last backup (smaller, slower restore chain) vs differential since last full (larger, faster restore)[^22].

---

### 2. Architecture Overview

`ids-backup` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a backup/recovery subsystem: it does not serve live traffic. It coordinates snapshot capture, incremental history, point-in-time replay, restore verification, and independent backup-copy tracking. Backup copies are tracked separately from serving replicas — a backup copy's lifecycle (retention, immutability, deletion) is governed by `ids-backup` alone, never by the replica that produced the source data.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (backup consumers)              │
│  ids-store, ids-snapshot, ids-replication — request backup/recovery ops      │
├──────────────────────────────────────────────────────────────────────────────┤
│                 Phase 042: ids-backup (THIS CRATE)                           │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌────────────┐ │
│  │ recovery point  │ │ incremental     │ │ restore         │ │ backup copy│ │
│  │ catalog         │ │ history / chain │ │ verification    │ │ tracking   │ │
│  │ (snapshot meta, │ │ (full anchor +  │ │ (checksum, full   │ │ (independent│ │
│  │  captured ver.) │ │  deltas, PITR)  │ │  restore, proof)│ │  lifecycle)│ │ │
│  └─────────────────┘ └─────────────────┴─────────────────┴────────────┘ │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌────────────┐ │
│  │ consistent      │ │ cloning /       │ │ point-in-time   │ │ catalog    │ │
│  │ capture (freeze │ │ delta compute   │ │ replay (full +  │ │ signing    │ │
│  │  + snapshot)    │ │  (CBT/diff)     │ │  deltas to T)   │ │ (ed25519)  │ │ │
│  └─────────────────┘ └─────────────────┴─────────────────┴────────────┘ │
└──────────────────────────────────────────────────────────────────┬──────────┘
                                                                   │ opaque Vec<u8> datagrams
┌──────────────────────────────────────────────────────────────────▼──────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])  │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a backup capture:**
1. A backup request (full or incremental) is submitted to `ids-backup`'s capture entrypoint with a target dataset/domain and a consistency mode (crash-consistent via on-disk freeze, or application-consistent via the source's own quiesce hook).
2. The consistent-capture module coordinates a point-in-time image: freeze writes (`fsfreeze`-style), take the snapshot, record the `SnapshotId` + `IdsTime`, thaw writes — all outside the carrier; the freeze window is bounded by a watchdog deadline.
3. The snapshot metadata (`RecoveryPoint`: id, parent chain, created_at, dataset, consistency level, byte size, checksum, delta-base reference) is written to the recovery-point catalog.
4. The incremental module computes the delta vs. the chosen base (full anchor for the first incremental; previous recovery point for chain incrementals): changed blocks via CBT-style dirty-tracking or content-defined-chunk diff; stores only changed content-addressed blobs, deduplicated by `Checksum` (BLAKE3/SHA-256).
5. Incremental backup history is appended (append-only chain: each recovery point references its parent, forming full-anchor → incremental → incremental …; periodic full backups break/anchor the chain).
6. Restore verification: on demand or schedule, materialize a recovery point to an isolated staging area, re-derive every referenced blob's `Checksum`, compare against the catalog, run integrity proof (`RecoveryProof`), and sign the verification result. Fail closed on any mismatch.
7. Backup copies are tracked independently: a backup copy is a retention-governed, immutability-protected replica of a recovery point stored separately from serving replicas; its retention/hold is managed by `ids-backup`, and a compromised production node cannot alter it.
8. All cross-node backup metadata travels as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-backup` never touches a carrier directly.

**Recovery point model:**
- A recovery point is a complete point-in-time image referenced by a single `SnapshotId`.
- `captured_versions`: each recovery point records the versions of every captured object at that instant (per-object `Checksum` + version), enabling consistent reads of "the state as of T."
- Cloning: a recovery point can be cloned (materialized to a new dataset/domain) without mutating the source; clone inherits read-only protection.

**Point-in-time recovery:** given a target `IdsTime` or named restore point, select the latest full anchor ≤ T, then replay all incrementals in chain order up to T; the result is the dataset state at T. Restore verification runs before the result is handed to a consumer.

---

### 3. Detailed Research Dimensions

#### 3.1 Recovery points & snapshot metadata (consistent capture, point-in-time image)

A recovery point is the unit of backup: a complete, consistent image of a dataset at one instant, identified by `SnapshotId` (`[u8; 16]`) and stamped with `IdsTime` (monotonic, from `ids-000-aa` — never wall clock for correctness decisions). Snapshot metadata records: snapshot id, parent snapshot id (chain link), created_at, dataset/domain id, consistency level (crash-consistent / application-consistent), byte size, root `Checksum`, delta-base reference (full anchor or previous recovery point), capture duration, bytes added, and the set of captured object versions (`captured_versions`: object id → `Checksum` at capture time).

Consistency tiers mirror the filesystem-consistency research[^19][^18]:
- **Crash-consistent:** capture the on-disk image as it exists at the instant; equivalent to "power loss at this moment." Journaled filesystems replay cleanly on mount. Zero cooperation needed from the source.
- **Filesystem-consistent:** freeze the filesystem (`fsfreeze`/`FIFREEZE` — flush dirty pages, commit journal, block new writes), take the image, thaw. Bounded freeze window guarded by a watchdog deadline (arm thaw deadline before freezing).
- **Application-consistent:** the source's own quiesce protocol flushes application in-memory/pending state to disk before the filesystem freeze (e.g. a DB `CHECKPOINT`); only the application knows its own invariants hold. This is the top tier and the one `ids-backup` targets for production datasets where available.

A snapshot is not a backup by itself unless the resulting copy has failure independence, retention protection, access separation, monitoring, and a tested restore path[^10]. `ids-backup` adds all of those around the snapshot.

#### 3.2 Incremental backup history (full anchor + delta chain)

Incremental backups copy only data changed since the previous backup of any type (full or incremental)[^22]. The chain is: one full backup anchor + N incrementals, each incremental referencing its immediate predecessor as delta base. To restore point T: materialize the full anchor, then replay incrementals in chain order up to T.

Two delta-compute strategies:
- **CBT-style dirty tracking** (Changed Block Tracking / Hyper-V RCT): a change tracker records which blocks were written since the last snapshot reference point; the incremental reads only those blocks[^22][^23]. The tracker state is tied to a reference point/snapshot generation; if tracking state is lost (reset/crash), the chain is invalid and a new full backup is required — matching the Veeam "broken backup chain" rule[^29].
- **Content-defined chunk diff** (FastCDC / Rabin/Gear rolling hash): split the dataset into content-defined chunks, hash each with BLAKE3/SHA-256, and compare the chunk set against the previous recovery point's chunk index; only new chunks are stored[^20][^24]. Insertions/deletions shift only local chunk boundaries, so most chunks are reused across versions (~10–20% better dedup than fixed-size chunking)[^20].

Full backups are taken periodically as chain anchors ("synthetic full" built from the prior full + incrementals without re-reading the source, or "active full" re-reading everything)[^5][^6]. A synthetic full breaks the old chain and starts a new anchor, which bounds replay length and limits blast radius from one corrupt incremental. Chain fragility is the central risk: one missing/corrupt incremental makes every incremental after it unusable[^29]; `ids-backup` mitigates with periodic synthetic/active fulls (chain breakers) and per-recovery-point checksums.

Incremental vs differential (research distinction, for the policy engine): differential copies everything since the last full (larger each run, faster restore — only full + last differential); incremental copies since the last backup (smaller each run, slower restore — full + all incrementals)[^22]. `ids-backup` defaults to incremental-with-periodic-full-anchors (the Veeam/NAKIVO dominant model)[^5][^6].

#### 3.3 Point-in-time recovery (PITR) & replay to arbitrary T

PITR combines a base backup with a continuous change stream replayed to any target[^1][^2][^3]. In `ids-backup`, the "base backup" is the latest full anchor ≤ target T, and the "change stream" is the ordered incremental history. Recovery target can be an `IdsTime`, a named restore point (created proactively before risky operations — Oracle "guaranteed restore point" / PostgreSQL `pg_create_restore_point` pattern[^27][^28]), or a chain position (recovery-point index).

Recovery semantics:
- `recovery_target_inclusive` style: stop before or at the target transaction; for safety, default to stopping before a named bad event[^28].
- Pause-and-verify: recover to a staging area, verify, then promote — never recover in place over production without verification[^28].
- Timeline handling: after a recovery completes and a new branch is written, a new timeline id prevents the new WAL/change stream from overwriting the old one (PostgreSQL timeline model)[^1] — `ids-backup` records a timeline/branch id per recovery branch so divergent histories never collide.

RPO/RTO: RPO (max data loss, looks backward) drives backup frequency; RTO (max downtime, looks forward) drives restore architecture[^8][^27]. The backup interval must be shorter than the RPO with margin for failed jobs (interval + run time ≤ RPO)[^8]. CDP (journal-based near-real-time capture) achieves near-zero RPO but higher cost; scheduled snapshots are cheaper with coarser RPO[^7][^8]. `ids-backup` supports both: scheduled incremental snapshots for the default tier, and CDP-style continuous journal capture for near-zero-RPO datasets.

#### 3.4 Backup copies tracked separately from serving replicas

A backup copy is an independent copy of a recovery point whose lifecycle is governed by `ids-backup`, not by the serving replica that produced it[^9][^10]. This is the ransomware-resilience pattern: two independent copies (primary operational + secondary isolated), separate administrative boundaries, immutability (WORM/compliance mode) so a compromised production identity cannot delete or shorten retention[^9][^11]. The backup copy is stored at a separate endpoint (reached via `ids-carrier::Endpoint`), retention-locked, and access-separated from production administration.

Key distinction: replication/snapshots/versioning/PITR alone are not backups unless the copy is failure-independent, retention-protected, access-separated, monitored, and has a tested restore path[^10]. `ids-backup` enforces this by treating each backup copy as a first-class tracked object with its own retention policy, immutability flag, and restore-test record.

#### 3.5 Consistent reads & captured versions

Each recovery point records `captured_versions`: for every object in the dataset, the object id and its `Checksum` at capture time. A consistent read against recovery point R returns exactly the object versions recorded in R — a read-only, point-in-time view. This is MVCC-style snapshot isolation applied to backups: the reader sees a stable picture as of the snapshot instant, unaffected by concurrent writes[^17][^16]. `captured_versions` also enables restore verification: every referenced blob's checksum is re-derived and compared.

#### 3.6 Cloning

Cloning materializes a recovery point to a new dataset/domain as a read-only copy. The clone is a full instantiation of the point-in-time image (all captured versions materialized), independent of the source. It inherits read-only protection and can be used for restore drills, forensics, or branching without mutating the backup. Clone cost is proportional to the data materialized; incremental clones (copy-on-write references to unchanged chunks) are an optimization.

#### 3.7 Restore verification

Restore verification proves a backup is usable, not merely present[^12][^13]. Layers:
1. **Checksum/manifest gate:** Ed25519 signature over the canonical recovery-point manifest proves the digest list is authentic; fail-closed on missing/invalid signature[^12][^13].
2. **Per-artifact re-hash:** every referenced blob is read and its `Checksum` re-derived; mismatch → reject[^12][^13][^30].
3. **Full decompression / structural check:** archives must be complete (end markers present), not truncated[^12].
4. **Staging restore drill:** materialize the recovery point to an isolated environment and run data-usability assertions (row counts, recent timestamps, integrity check)[^13][^28].
5. **Signed verification receipt:** ed25519-signed receipt listing every restored object with counts (names/counts only, never row data), hash-chained to the previous receipt[^12].

Verification runs on a schedule (daily/weekly for critical recovery points, within retention windows) and after every backup copy creation[^13]. A backup that hasn't been restored is unverified.

#### 3.8 Retention, immutability & GFS tiered retention

Recovery points carry retention (retain-until-date, extendable-only) and optional legal hold (indefinite, overrides retention, explicit release)[^1][^9][^21]. GFS (grandfather-father-son) tiered retention keeps weekly/monthly/yearly full backups as long-term anchors, breaking chains and bounding replay length[^21]. Immutable/air-gapped backup copies use WORM (compliance mode: no override, not even root; governance mode: override with elevated privileges)[^9][^11]. Retention expiry leads to disposition (retain/review/purge/archive) via the `ids-reclaim`-compatible expiry sweep, never silent deletion[^21].

#### 3.9 Cryptographic commitment & allowed primitives only

- ed25519: signs recovery-point manifests, verification receipts, backup-copy commitments (origin authentication, non-repudiation).
- sha256 / blake3: object `Checksum`s, content-addressed blob IDs, chunk indexes, manifest digests, receipt chains.
- xchacha20poly1305 / chacha20poly1305: encrypt backup payloads at rest when confidentiality required (per-chunk AEAD with counter-bound nonces to prevent reorder/truncation)[^14][^15].
No SHA-3, no post-quantum primitives, per the allowed-crypto rule. Manifests are deterministically canonicalized (sorted keys, compact separators, UTF-8 — RFC 8785 subset) before signing so verification is byte-stable across implementations[^12][^30].

---

### Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Corrupt or truncated backup mistaken for valid | Recoverability | Manifest signature gate first, then per-blob checksum re-derive; full restore drill with data-usability assertions; fail closed on any mismatch |
| Missing/corrupt incremental breaks chain | Chain integrity | Periodic synthetic/active fulls as chain anchors; per-recovery-point checksums; refuse restore if chain links missing |
| Unauthorized modification/deletion of a backup copy | Backup integrity | Backup copy tracked separately from serving replica; retention + immutability (WORM/compliance mode) governed by ids-backup; separate administrative boundary |
| Ransomware / compromised production deletes backups | Recoverability | Isolated/air-gapped backup copy; immutability lock; separate credentials; no production path to backup deletion |
| Recovery to wrong point / bad transaction included | PITR correctness | Named restore points created proactively; stop-before-target default; pause-and-verify before promote; timeline ids prevent branch collision |
| Replay of stale recovery point | Freshness | SnapshotId is content-derived over manifest; IdsTime monotonic; timeline/branch ids; signed verification receipts hash-chained |
| Clock manipulation to skew recovery-point ordering | Ordering correctness | IdsTime monotonic (not wall clock); SnapshotId content-derived; chain order verified on restore |
| Backup copy reachable from production network | Isolation | Separate endpoint via ids-carrier::Endpoint; separate admin credentials; logically/physically isolated copy |
| Signing-key compromise | Authenticity of proofs | Signed key-rotation handoff; old public keys retained forever for verifying old receipts/manifests |
| Silent data corruption (bit rot) | Backup integrity | Periodic full verification re-reads every referenced blob and re-derives its checksum |

The carrier is never trusted: backup metadata is signed over payload bytes, so it survives re-framing by any carrier, and the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-backup |
|-------|------|-------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `SnapshotId`, `RecordId`, `FragmentId`, `RecoveryProof`, `Checksum`, `RecoveryAction::RestoreRedundancy`, `OrderingPosSnapshot`, `IdsTime`, `CarrierType`, `NodeId`, `RegionId` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All backup/metadata traffic as datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signatures, content hashes, encryption for `ids-backup` |
| `ids-store` | Primary record store (backup source) | Provides consistent reads, `captured_versions`, freeze/quiesce hooks for capture |
| `ids-snapshot` (Phase 026) | Distributed snapshots (Chandy-Lamport) | Reuses snapshot coordination primitives; `ids-backup` recovery points are longer-lived, retention-governed snapshots |
| `ids-replication` | Replica synchronization | Backup copies go to separate endpoints; replication-independent tracking |
| `ids-reclaim` (Phase 041) | Fragment compaction & space reclamation | Cannot reclaim retained/held recovery points; expiry-sweep output feeds reclamation eligibility |
| `ids-immutable` (Phase 040) | Immutable record protection / WORM / legal hold | Backup copies reuse retention/hold enforcement; compliance-mode WORM for immutable backup copies |
| `ids-integrity` (Phase 030) | End-to-end checksums, Merkle trees, scrubbing | Reuses `Checksum`/Merkle primitives for per-blob verification and catalog integrity |

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-backup/` (a separate crate, not a folder inside an existing crate):

```
ids-backup/
  Cargo.toml
  src/
    lib.rs                      # crate root, public API surface
    errors.rs                   # BackupError, ChainError, RestoreError, VerifyError
    types.rs                    # RecoveryPoint, Incremental, BackupCopy, ChainPosition, TimelineId, CloneRequest (re-exports ids-000-aa types)
    capture.rs                  # backup-capture entrypoint (full/incremental), consistency modes
    consistent_capture.rs       # freeze/quiesce/thaw coordination, watchdog deadline
    recovery_point.rs           # recovery point lifecycle, catalog, captured_versions
    incremental.rs              # delta compute (CBT / content-defined chunk diff), dedup by Checksum
    chain.rs                    # full anchor + incremental chain, parent links, chain validation
    pitr.rs                     # point-in-time recovery: target T, replay incrementals, timeline/branch ids
    restore.rs                  # restore materialization to staging, read-only consistent view
    verify.rs                   # restore verification: manifest gate, per-blob re-hash, drill, signed receipt
    backup_copy.rs              # backup-copy tracking: separate lifecycle, retention, immutability, isolated endpoint
    retention.rs                # recovery-point retention, retain-until-date, extendable-only
    hold.rs                     # legal hold on backup copies: indefinite, overrides retention
    gfs.rs                      # grandfather-father-son tiered retention anchors
    catalog.rs                  # recovery-point catalog (append-only, content-addressed index)
    carrier_adapter.rs          # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
    contracts/                  # sealed trait contracts (~10 files)
      capture_contract.rs, chain_contract.rs, pitr_contract.rs, verify_contract.rs,
      backup_copy_contract.rs, retention_contract.rs, hold_contract.rs, catalog_contract.rs,
      carrier_contract.rs, mod.rs
    invariants/                 # compile-time/runtime invariants (~10 files)
      chain_integrity.rs, backup_copy_independent.rs, monotonic_ids_time.rs,
      retention_extendable_only.rs, hold_overrides_retention.rs, carrier_agnostic.rs,
      crypto_allowlist.rs, pitr_target_bounded.rs, verify_fail_closed.rs, mod.rs
  tests/                        # property/verification tests (~20 files)
  benches/                      # (~8 files)
  docs/                         # (~12 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, one file per concern — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **RecoveryPoint**: `{SnapshotId, parent_id, created_at (IdsTime), dataset_id, consistency_level, byte_size, root Checksum, delta_base (full anchor or prior RecoveryPoint), captured_versions: Map<ObjectId, Checksum>, duration, bytes_added, timeline_id}`. SnapshotId is content-derived (SHA-256 over canonical manifest with id blanked) so identical content at the same second yields the same ID; any change changes it.
- **Chain (full anchor + incrementals)**: `Chain = FullAnchor × Incremental*`; each Incremental references its parent; chain validity = every link present and checksummed; a missing link breaks the chain (require new full anchor).
- **Delta compute**: CBT dirty-block map since last reference point OR content-defined-chunk diff vs. previous recovery point's chunk index; store only new chunks, dedup by `Checksum` (BLAKE3/SHA-256).
- **Point-in-time replay**: `recover(T) = materialize(full_anchor ≤ T) then replay incrementals in chain order up to T`; stop-before-target default; timeline id per recovery branch prevents branch collision.
- **Consistent capture**: freeze (watchdog-guarded deadline) → snapshot image → record `captured_versions` → thaw; consistency level recorded per recovery point.
- **Restore verification**: Ed25519 signature over canonical manifest → per-blob `Checksum` re-derive → structural/decompression check → staging restore drill with data-usability assertions → ed25519-signed receipt hash-chained to prior receipt.
- **Backup copy**: independent lifecycle; retention + hold governed by `ids-backup`; stored at separate `ids-carrier::Endpoint`; immutability via WORM/compliance mode.
- **Retention/hold**: retain-until-date monotonic (extendable only); legal hold indefinite, checked first, short-circuits every disposition, explicit release only; GFS weekly/monthly/yearly anchors.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`SnapshotId`, `RecordId`, `FragmentId`, `RecoveryProof`, `Checksum`, `RecoveryAction`, `OrderingPosSnapshot`, `IdsTime`, `CarrierType`, `NodeId`, `RegionId`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of recovery points, manifests, verification receipts.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### 9. Measurable Acceptance Criteria

1. Every recovery point is a complete, consistent point-in-time image (not a partial file copy); verified by full restore drill byte-identity against source at capture instant.
2. Incremental backup history forms a valid chain: full anchor + ordered incrementals, every link checksummed; restore to any chain position T succeeds iff all links ≤ T present.
3. Point-in-time recovery to a named restore point or `IdsTime` reproduces the dataset state at T; default stop-before-target; pause-and-verify before promote.
4. Restore verification catches 100% of single-blob corruption/truncation; manifest signature gate fails closed on missing/invalid Ed25519 signature.
5. Backup copy tracked independently of serving replica: a compromised production node cannot alter/delete a backup copy; retention + WORM/compliance-mode immutability enforced.
6. Active legal hold blocks 100% of backup-copy modify/delete attempts regardless of retention state; release is the only clearing path and is audited.
7. Retention extendable-only, never shortened; locked retention irreversible for the period.
8. All backup/copy/disposition/verification events recorded in append-only catalog with actor + `IdsTime`; catalog entries content-addressed by `Checksum`.
9. All backup metadata carried as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS in the crate.
10. Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 appear in the crate's crypto surface.
11. Periodic synthetic/active fulls bound replay length; chain broken by one missing incremental → restore refuses, requires new full anchor.
12. Recovery-point catalog verifiable offline: re-derive every referenced blob's checksum and compare against catalog.

---

### 10. Remaining Uncertainties

- Consistency mode default: crash-consistent (zero cooperation) vs application-consistent (requires source quiesce hook) for each dataset; application-consistent is stronger but not always available.
- CBT dirty-tracking vs content-defined-chunk diff: CBT needs source-side tracking state (chain breaks if lost); CDC is source-agnostic but higher CPU; choose per dataset or hybrid.
- SnapshotId content-derived vs ULID/time-based: content-derived resists forgery but recompute cost on load; time-based gives lexicographic=chronological order (kit-msgvault pattern).
- Synthetic full frequency: bounds replay length but costs CPU/storage; tunable parameter to be validated at integration.
- Interaction with Phase 041 `ids-reclaim`: expiry sweep must not reclaim retained/held recovery points; exact guard-reclaim handshake (eligibility proof) defined here, validated at integration.
- Backup-copy isolation level: logical (separate endpoint/credentials) vs physical (separate site/account); operational decision, not a research conclusion.
- CDP-style continuous journal capture for near-zero-RPO datasets: journal format, truncation, and interplay with incremental snapshots undefined; out of scope for MVP.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-backup` (not a folder inside an existing crate) | User directive: separate crates for new capabilities; keeps existing crates focused |
| Full anchor + incremental chain with periodic synthetic/active fulls | Bounds replay length, limits blast radius from one corrupt incremental (Veeam/NAKIVO dominant model)[^5][^6] |
| Recovery point = complete point-in-time image, content-addressed `SnapshotId` | Every backup restorable on its own; content-derived ID resists forgery/renaming |
| Incremental = changed data since last backup (CBT or CDC diff) | Minimizes backup size/bandwidth; CBT needs tracking state, CDC is source-agnostic |
| PITR = full anchor ≤ T + replay incrementals in chain order, stop-before-target | Standard PITR pattern (PostgreSQL/Oracle); safe default prevents bad-transaction inclusion |
| Restore verification: manifest signature first, then per-blob re-hash, then staging drill | Signature proves digest authenticity before trusting digests; drill proves usability[^12][^13] |
| Backup copy independent of serving replica, retention-governed by ids-backup | Ransomware resilience: compromised production cannot delete backups[^9][^10] |
| Retention extendable-only; legal hold indefinite, overrides retention; GFS anchors | Mirrors WORM/immutable-backup practice; hold precedence (Microsoft 365/Box principle)[^1][^9][^21] |
| IdsTime monotonic, never wall clock, for correctness decisions | Prevents clock-manipulation early expiry; absolute deadlines need network clock (out of scope) |
| Only allowed crypto; carrier-agnostic via `ids-carrier::Endpoint` | Non-negotiable HX-241 / IDS constraints |

---

### Sources

[^1]: PostgreSQL, "Continuous Archiving and Point-in-Time Recovery (PITR)" — base backup + archived WAL replayed to any target time/named restore point; timeline IDs prevent WAL overwrite across recoveries. https://postgresql.org/docs/current/continuous-archiving.html
[^2]: Percona, "PostgreSQL Backup Strategies for Enterprise-Grade Environments" — WAL archiving before segment recycling; pgBackRest incremental backups + WAL management. https://percona.com/blog/postgresql-backup-strategy-enterprise-grade-environment
[^3]: Crunchy Data, "How to Recover When PostgreSQL is Missing a WAL File" — WAL archiving + PITR to any moment since last full backup. https://crunchydata.com/blog/how-to-recover-when-postgresql-is-missing-a-wal-file
[^4]: Veeam, "How Synthetic Full Backup Works" — synthetic full built from prior full + incrementals without re-reading source. https://helpcenter.veeam.com/docs/vbr/userguide/synthetic_full_hiw.html
[^5]: Veeam, "Immutability Target with Backup Job and Backup Copy Job and the Backup Chain" — forward incremental with periodic fulls as chain breakers; reverse-incremental deprecated/incompatible with immutability; chain fragility. https://community.veeam.com/blogs-and-podcasts-57/immutability-target-with-backup-job-and-backup-copy-job-and-the-backup-chain-13499
[^6]: NAKIVO, "What Is Synthetic Full Backup and How Does It Work?" — synthetic full, forever-incremental, full-synthetic mode. https://nakivo.com/blog/what-is-synthetic-backup
[^7]: Cohesity / Veritas, "Continuous Data Protection (CDP)" — journal-based near-real-time capture, near-zero RPO vs scheduled snapshots. https://veritas.com/information-center/continuous-data-protection
[^8]: Veeam / AWS / Druva, "RTO and RPO" — RPO drives backup frequency, RTO drives recovery architecture; interval + run time ≤ RPO. https://veeam.com/blog/recovery-time-recovery-point-objectives.html
[^9]: Microsoft Azure, "Design a Ransomware-Resilient Backup Architecture" — two independent immutable backup copies, separate subscriptions/regions, immutability lock, isolated admin. https://learn.microsoft.com/en-us/azure/architecture/security/ransomware-resilient-backup-architecture
[^10]: AWS DTT, "ADR 014: Backups and Recovery" — replication/snapshots/PITR are not backups unless failure-independent, retention-protected, access-separated, monitored, tested restore path. https://adr.dtt.digital.wa.gov.au/operations/014-object-backup.html
[^11]: Acronis / Avepoint / Bareos, "Immutable Backup" — WORM, compliance vs governance mode, SEC 17a-4; immutability at storage layer. https://avepoint.com/blog/backup/immutable-backup
[^12]: backupvalidation.org, "Cryptographic Manifest Signing" — signature over canonical manifest proves digest authenticity; per-artifact re-hash proves stored bytes; fail-closed on missing/invalid signature; RFC 8785 canonicalization. https://backupvalidation.org/automated-backup-integrity-check-implementation/cryptographic-manifest-signing
[^13]: AWS Well-Architected / AWS Backup, "Perform periodic recovery of the data to verify backup integrity" — automated restore testing to isolated sandbox, checksum + data-usability assertions. https://docs.aws.amazon.com/wellarchitected/latest/reliability-pillar/rel_backing_up_data_periodic_recovery_testing_data.html
[^14]: pilinux/crypt, "envelope package" — per-item/sub-key XChaCha20-Poly1305 envelopes; streaming chunks with counter-bound nonces preventing reorder/truncation. https://pkg.go.dev/github.com/pilinux/crypt/envelope
[^15]: shieldfive/crypto, "format-v1.md" — XChaCha20-Poly1305 per-chunk AEAD with counter binding and final-chunk flag. https://github.com/shieldfive/crypto/blob/main/spec/format-v1.md
[^16]: Greenplum / PostgreSQL, "Distributed Snapshots / MVCC Snapshots" — snapshot isolation: reader sees stable committed state as of snapshot instant. https://greenplum.org/improving-backup-performance-and-reliability-with-distributed-snapshots
[^17]: SCAR / CMU 15-445, "Multi-Version Concurrency Control" — MVCC snapshot isolation; read-only transactions see consistent snapshot without locks. https://15445.courses.cs.cmu.edu/spring2024/notes/19-multiversioning.pdf
[^18]: Nakivo / Nutanix / Microsoft, "Crash-Consistent vs Application-Consistent Snapshots" — VSS writers flush pending I/O; crash-consistent = on-disk state as of power loss. https://nakivo.com/blog/crash-consistent-vs-app-consistent-backup
[^19]: PandaStack / Ubuntu manpages, "fsfreeze" — filesystem freeze flushes dirty pages and journals for consistent on-disk image; watchdog thaw deadline before freeze. https://manpages.ubuntu.com/manpages/jammy/man8/fsfreeze.8.html
[^20]: FastCDC / restic, "Content-Defined Chunking" — rolling-hash CDC boundaries shift only locally on insertions; ~10–20% better dedup than fixed-size. https://ieeexplore.ieee.org/document/9055082
[^21]: Commvault / Veeam / Backblaze, "Grandfather-Father-Son (GFS) Data Retention" — weekly/monthly/yearly full backups as tiered retention anchors. https://documentation.commvault.com/11.42/software/grandfather_father_son_gfs_data_retention.html
[^22]: AWS / Nakivo / Acronis, "Incremental vs Differential Backup" — incremental since last backup vs differential since last full; chain fragility (one missing incremental breaks restore). https://aws.amazon.com/compare/the-difference-between-incremental-differential-and-other-backups
[^23]: Veeam / Microsoft, "Changed Block Tracking / Resilient Change Tracking" — dirty-block bitmap since reference point; chain reset if tracking state lost. https://helpcenter.veeam.com/archive/backup/120/hyperv/changed_block_tracking.html
[^24]: restic Design / kit-msgvault FORMAT.md — content-addressed blobs, immutable append-only packs (~32 MB, sealed once), manifest written last (crash-safe), index rebuild from pack footers, prune+GC with grace period. https://restic.readthedocs.io/en/v0.4.0/Design ; https://github.com/kenn-io/kit/blob/main/backup/FORMAT.md
[^25]: Proxmox Backup Server Technical Overview — chunks referenced by snapshot indexes; dedup across snapshots; change detection reuses unchanged chunks without re-reading source. https://pbs.proxmox.com/docs/technical-overview.html
[^26]: Proxmox Backup Maintenance — prune removes snapshot metadata, GC with grace period removes unreferenced chunks; 24h+5min safety for relatime filesystems. https://pbs.proxmox.com/docs/maintenance.html
[^27]: Oracle, "Performing Flashback and Database PITR" — guaranteed restore points before risky operations; recover to SCN/time/restore point. https://docs.oracle.com/en/database/oracle/oracle-database/19/bradv/rman-performing-flashback-dbpitr.html
[^28]: onenoughtone, "Point-in-Time Recovery" — named restore points created proactively; recovery_target_inclusive stop-before-target; pause-and-verify before promote. https://onenoughtone.com/learn/point-in-time-recovery/4
[^29]: Veeam experts-exchange / community, "How to fix Broken backup chain" — one missing/corrupt incremental breaks the whole chain; new full backup required. https://experts-exchange.com/articles/31137/Veeam-How-to-fix-Broken-backup-chain.html
[^30]: IETF draft-durga-agenttrust-trace-verification / elizaOS, "canonical JSON walk fail-closed" — JCS canonicalization, Ed25519, fail-closed verification for integrity gates. https://datatracker.ietf.org/doc/html/draft-durga-agenttrust-trace-verification-00

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 042.
