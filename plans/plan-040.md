## Phase 040 — Create the ids-immutable crate: Protect selected data against modification/deletion.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 595-609)

```
40. Research only do not read plan.md **[Phase 040 — Create the ids-immutable crate: Protect selected data against modification/deletion.]**

Crate: ids-immutable
Purpose: Protect selected data against modification/deletion.
Contents: Immutable record protection, retention period enforcement, legal hold enforcement, enforcement across replicas/fragments/derived copies, tamper detection, retention expiry.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 040 — Create the ids-immutable crate: Protect selected data against modification/deletion.]**

Create plan-040.md in C:/idfs/plans/ for the ids-immutable crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 040 creates `ids-immutable`, the IDS crate that protects selected records against modification and deletion. It owns immutable record protection, retention-period enforcement, legal-hold enforcement, enforcement across replicas/fragments/derived copies, tamper detection, and retention expiry. The design follows the WORM (write-once, read-many) model with two independent, composable controls: (a) a retention period with a retain-until-date after which a record may be modified/deleted, and (b) a legal hold with no expiration that blocks deletion/modification until explicitly released. A legal hold always takes precedence over retention. Every protection decision is recorded in an append-only, hash-chained audit trail with signed checkpoints so any modification, deletion, truncation, or reordering is detectable.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-immutable` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-immutable` crate does NOT yet exist at `C:/idfs/ids-immutable/`. `ids-000-aa` already declares canonical types `RecordId` (`src/types.rs:262`), `SnapshotId` (`src/types.rs:1329`), `FragmentId` (`src/types.rs:1889`), `RecoveryProof` (`src/types.rs:1905`), `Checksum` (`src/types.rs:1975`), `AnomalyType` ∈ {ByteMismatch, ReplicaDrift, ChecksumFailure, StateCorruption} (`src/types.rs:1951`), `NodeId` (`src/types.rs:21`), `RegionId` (`src/types.rs:68`), `IdsTime`, and `CarrierType` (`src/types.rs:394`) — all referenced by this crate but never redefined. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned AWS S3 Object Lock (WORM, fixed/variable retention periods, legal hold, governance vs compliance mode, SEC 17a-4/FINRA 4511/CFTC 1.31 assessed by Cohasset); Azure Immutable Blob Storage (time-based retention + legal hold, indefinite until cleared); Google Cloud Bucket Lock / Object Retention Lock (bucket- and object-level retention, irreversible lock, max 100 years); SEC Rule 17a-4 amendments (WORM non-rewriteable/non-erasable vs time-stamped audit-trail alternative: all modifications/deletions, date-time, actor identity, recreation of original); FINRA Rule 4511 and CFTC Reg 1.31 financial immutability; ICO UK GDPR storage limitation (Art. 5(1)(e)) and right to erasure (Art. 17, non-absolute, exceptions incl. legal obligation and legal claims; retention schedules; backups held "beyond use"); EDPB 2025 right-to-erasure sweep (difficulties defining/implementing retention periods, backup deletion); Trillian / Certificate Transparency (append-only logs, binary Merkle tree, Signed Tree Heads, inclusion + consistency proofs, RFC 6962); Facebook LogDevice (append-only, trimmable, record-oriented log with LSN); InfluxDB / CockroachDB / Spanner / Doris row-level TTL (periodic background retention enforcement, shard-group deletion, records invisible to queries before physical reclaim); ScyllaDB/Cassandra tombstones with `gc_grace_seconds` and repair-based GC to prevent cross-replica data resurrection; Kafka log-compaction tombstones with `delete.retention.ms` and MTRO/MCCO (a tombstone is removed only after every replica has compacted past it); Elasticsearch ILM rollover/delete retention for append-only data streams; IBM Logging for Compliance (cryptographically immutable log, distributed-ledger checkpoints, rotation after retention, anti-truncation/elongation); ATL Protocol (append-only log with cryptographic proofs, Bitcoin anchoring, Super-Tree consistency_to_origin proofs, right-to-erasure via payload deletion); EP-EVIDENCE-RECORD / RFC 4998 Evidence Record Syntax (crypto-agile renewal chain re-time-attested before algorithm aging, RFC 3161 timestamps); Prooflog (retention deletes ciphertext but keeps a tombstone row — sequence range, Merkle root, signed checkpoint — forever; legal hold overrides expiry; signed key-rotation handoff; old public keys retained forever); Open Archiver retention engine (policy-as-code, max-duration-wins, legal hold short-circuits first, disposition retain/review/purge/archive, grace days, immutable audit log); Microsoft 365 retention vs eDiscovery holds (hold always takes precedence over retention); Authenticated Append-Only Skip Lists (tamper-evident append-only, one-way digests, inclusion proofs, anti-invention/anti-equivocation); Agent Flight Recorder / AuditWeave / kxco-pq-audit (SHA-256 hash-chained tamper-evident audit trails, Merkle batching, anchoring); RGIP triple-hash chain (SHA-256/BLAKE3, cross-chain hash — note only SHA-256/BLAKE3 are allowed here); Proof-of-Retention (auditable retention proof-of-possession, auditor verifies without revealing data); IETF draft-hopley-x402-retention-chain (SHA-256 receipt chain, JCS canonicalization, self-verifiable, no network required); Prolly trees / haematite / immure / kin-blobs (content-addressed immutable storage, deduplication, GC via keep_last/keep_for/keep_versions).

---

### 2. Architecture Overview

`ids-immutable` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — record protection, retention, legal hold, replica/fragment/derived-copy enforcement, tamper detection, expiry — is an independent module. `ids-immutable` is a guard layer: it does not store records itself; it vetoes or stamps mutating operations issued by `ids-store`, `ids-fragment`, `ids-replication`, and `ids-reclaim` (Phase 041) before they take effect.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (mutation issuers)              │
│  ids-store, ids-fragment, ids-replication, ids-reclaim — issue writes/deletes│
├──────────────────────────────────────────────────────────────────────────────┤
│                 Phase 040: ids-immutable (THIS CRATE — guard layer)          │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ record         │ │ retention      │ │ legal hold     │ │ replica/     │ │ │
│  │ protection     │ │ period engine  │ │ enforcement    │ │ fragment/    │ │ │
│  │ (ProtectionLevel│ │ (retain-until- │ │ (indefinite,   │ │ derived-copy │ │ │
│  │  None/Retention│ │  date, only    │ │  until released)│ │ enforcement  │ │ │
│  │  /Hold/Compliance)│ │  extendable)│ │                │ │ (tombstones) │ │ │
│  └────────────────┘ └────────────────┴────────────────┴──────────────┘ │ │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ tamper         │ │ expiry/dispos- │ │ policy engine  │ │ audit chain  │ │ │
│  │ detection      │ │ ition (retain/ │ │ (max-duration-   │ │ (hash chain, │ │ │
│  │ (chain/Merkle) │ │  review/purge/ │ │  wins, hold-first│ │ signed ckpt) │ │ │
│  │                │ │  archive, grace│ │  short-circuit)  │ │              │ │ │
│  └────────────────┘ └────────────────┴────────────────┴──────────────┘ │ │
└────────────────────────────────────────────────────────────────┬────────────┘
                                                                 │ opaque Vec<u8> datagrams
┌────────────────────────────────────────────────────────────────▼────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])  │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a protected mutation:**
1. A mutation issuer (`ids-store` write/delete, `ids-reclaim` compaction, `ids-replication` replica update) submits a `RecordId` + intended operation to `ids-immutable`'s guard entrypoint.
2. The protection module resolves the record's `ProtectionLevel`: `None` → allow; `Retention` → check `retain_until_date` against `IdsTime` (monotonic); `Hold` → reject; `Compliance` → reject with no override path.
3. The policy engine evaluates all matching retention/hold rules (max-duration-wins; legal hold checked first and short-circuits every disposition) and emits a disposition (`retain` / `review` / `purge` / `archive`) recorded to the append-only audit log.
4. If allowed, the mutation proceeds and is stamped into the tamper-detection hash chain/Merkle log with a `Checksum` (BLAKE3/SHA-256) and optional ed25519 signature; a signed checkpoint (Signed Tree Head) anchors periodically.
5. Deletion of an expired/retained record does not erase history: it writes a tombstone (sequence range, Merkle root, signed checkpoint) that survives forever, so verifiers can distinguish "expired-but-retained" from an unexplained gap. Tombstones are removable only after every replica has acknowledged compaction past them (MTRO/MCCO-style).
6. All cross-node protection metadata travels as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-immutable` never touches a carrier directly.

**Protection levels:**
- `None` — no constraint; normal mutable record.
- `Retention` — record is immutable until `retain_until_date`; after expiry it may be modified/deleted per disposition (unless also held).
- `Hold` — legal hold; immutable and undeletable until explicitly released; independent of retention, no expiration.
- `Compliance` — WORM: immutable and undeletable for the retention period with no override (not even root); mirrors SEC 17a-4 compliance mode.

---

### 3. Detailed Research Dimensions

#### 3.1 Immutable record protection model (WORM, versioning, append-only)

Records are protected at the `RecordId` granularity by wrapping the record in an immutable envelope: content-addressed by `Checksum` (BLAKE3 or SHA-256 over canonical bytes), versioned by monotonic sequence, and linked into a per-stream hash chain so each version binds the previous version's digest. A protected record can be read and replicated but never overwritten in place; a "modification" of a retained record is rejected by the guard, and a new version is only creatable once retention has expired or been lawfully superseded under the audit-trail alternative (SEC 17a-4: the system must permit recreation of the original if modified/deleted, with a complete time-stamped audit trail naming actor, date-time, and all modifications)[^4][^5]. This matches the WORM requirement of non-rewriteable, non-erasable storage [^1][^2] and the audit-trail alternative's time-stamped, actor-attributed change log. Content-addressing (SHA-256/BLAKE3 keyed by content hash) gives integrity for free: any alteration breaks the digest, and identical content is stored once [^25][^24]. The envelope carries a `ProtectionLevel` and `retain_until_date`, both monotonic (retention may only be extended, never shortened — mirroring the irreversibility of a locked bucket retention policy)[^3].

#### 3.2 Retention period enforcement (fixed/variable retention, retain-until-date, policy engine)

Each protected record or record class carries a retention period with an absolute retain-until-date or a duration from a reference event (created/published/last-accessed). The enforcement engine runs periodically (background sweep, e.g. every N minutes like InfluxDB's retention service[^10]) and evaluates every record against its category policy. When multiple policies match a record, max-duration-wins applies: the longest retention period is honored, so any policy requiring longer retention prevails [^18]. A grace period buffers after the disposition date to prevent same-day churn [^18]. Retention can be extended (retain-until-date moved later) but never reduced once locked, consistent with Google Cloud's locked bucket policy (irreversible; cannot delete bucket while objects remain under retention)[^3]. Rows/records become ineligible for modification or deletion while within the retention period; after expiry they become eligible for disposition but are not physically removed until a compaction/reclaim pass (Phase 041 `ids-reclaim`) runs and the guard signs off [^10][^13]. This mirrors row-level TTL (CockroachDB/Spanner/Doris): expiration is checked at query time; physical reclamation is a separate background step with no fixed deletion deadline [^2][^10].

#### 3.3 Legal hold enforcement (indefinite, overrides retention, explicit release)

A legal hold is independent of retention: it has no expiration and remains in force until explicitly released [^1][^2]. When a record is under an active hold, all disposition (delete/modify) is suspended regardless of how far past its retention date it is — the hold is evaluated first and short-circuits every other rule [^18]. Hold and retention can coexist: a record under both a retention period and a hold is not deleted when retention expires; it is retained until the hold is lifted [^19][^3]. Holds are targeted to specific records, record classes, or custodians (e-discovery/litigation model) rather than applied organization-wide blindly [^19]. Release of a hold is a controlled, audited action recorded in the append-only audit log so the transition from "preserved" to "eligible for disposition" is traceable [^18][^19]. This implements the Microsoft 365/Box principle that preservation for a legal matter always takes precedence over routine retention [^19].

#### 3.4 Tamper detection (hash chains, Merkle trees, append-only verification, anchoring)

Every protected mutation is appended to an append-only, hash-chained audit ledger: each entry SHA-256- or BLAKE3-chains to the previous entry's digest over its canonical fields, so any modification, insertion, deletion, or reordering invalidates all subsequent hashes and is detected on verification (recomputation of the chain)[^21][^20][^24]. Periodic Merkle batching produces a Signed Tree Head (STH) — a Merkle root over the ledger signed with ed25519 — enabling compact inclusion and consistency proofs that the ledger is append-only between any two STHs (Certificate Transparency / Trillian model)[^8]. STHs are anchored (checkpointed) at intervals so an off-ledger verifier can confirm the ledger was not rewritten; anchoring itself is chained so removing an anchor is detectable [^21][^14]. IBM's Logging for Compliance demonstrates a provably immutable log rotated after retention, with signed checkpoints committed to a distributed ledger to eliminate truncation/elongation attacks[^14]. Authenticated Append-Only Skip Lists give a malicious-maintainer-resistant variant: one-way digests plus succinct inclusion proofs, with proofs that data cannot be invented or equivocated after a digest is published[^20]. Verification is offline: given the disclosed payload and Merkle proof, any verifier checks independently without trusting the logger [^21].

#### 3.5 Enforcement across replicas/fragments/derived copies (tombstones, convergent deletion)

Protection must be uniform across every replica, fragment, and derived copy of a record. A delete of an expired record does not erase the record everywhere immediately; it writes a tombstone (sequence range + Merkle root + signed checkpoint) that is replicated like any other entry, so offline/lagging replicas learn of the deletion when they reconnect [^17][^12]. Tombstone removal (garbage collection) is gated on causal stability: a tombstone is eligible for physical removal only after it is expired and its timestamp range does not overlap any live data on any replica (ScyllaDB/Cassandra `gc_grace_seconds`, default 10 days; repair-based GC sets the expiry threshold per token range so no replica can resurrect the data)[^11]. Kafka's MTRO/MCCO pattern is the precise convergent rule: per-replica maximum-cleanly-compacted offset (MCCO) only moves forward; the tombstone-removal offset (MTRO) is the minimum MCCO across the replica set, including unavailable replicas, so a tombstone below MTRO is safe to remove because every replica has compacted past it [^12]. Derived copies (fragment shards, replicas, caches) inherit the protection level of their source; a derived copy may not outlive its source's retention/hold, and compaction of a fragment must consult the guard before dropping any retained/held record. This keeps enforcement convergent under any delivery order, duplication, or delay — matching the carrier-agnostic, unknown-membership model [^12][^11].

#### 3.6 Retention expiry and disposition (retain / review / purge / archive, expired-but-retained)

When retention expires, the guard emits a disposition: `retain` (keep), `review` (human/approver gate before any destructive action), `purge` (permanent deletion via tombstone), or `archive` (move to cold/immutable storage)[^18]. The enforcer never deletes directly: it records the disposition in the immutable audit log and routes purges through a delayed queue or human review so a misconfigured rule causes a flagged review, not silent data loss[^18]. Expired data is not immediately erased; the ciphertext/blob may be removed but the tombstone row (sequence range, Merkle root, signed checkpoint) survives forever, and verifiers report the range as "expired-but-retained" — verifiable by the retained signed checkpoint — rather than as a false gap[^17]. ATL's approach of deleting the payload while retaining the hash (right-to-erasure compatible: the hash remains as a mathematical constant but no longer links to personal data)[^15] is relevant where GDPR erasure conflicts with an immutable audit trail. Elasticsearch ILM shows rollover + time-based delete phases for append-only indices[^13].

#### 3.7 Compliance frameworks (SEC 17a-4, FINRA 4511, CFTC 1.31, GDPR, legal hold precedence)

- SEC Rule 17a-4: electronic records preserved exclusively in non-rewriteable, non-erasable (WORM) format, or via the audit-trail alternative (complete time-stamped audit trail of all modifications/deletions with actor identity, permitting recreation of the original)[^4][^5]. FINRA Rule 4511 and CFTC Regulation 1.31 impose equivalent WORM/audit-trail obligations on financial records [^1][^2]. `ids-immutable` supports both pathways: `Compliance` protection = WORM; `Retention` + append-only signed audit chain = audit-trail alternative.
- GDPR: storage limitation (Art. 5(1)(e)) — keep no longer than necessary, justified per purpose; right to erasure (Art. 17) — non-absolute, with exceptions for legal obligation, public interest, archiving/research/statistics, and legal claims[^6][^7]. Tension: retention (keep) vs erasure (delete). Resolution: a legal hold or statutory retention obligation is a lawful basis to refuse erasure; once retention expires and no hold applies, purge via tombstone; backups held "beyond use" until replaced on schedule[^6][^7]. EDPB's 2025 sweep found organizations struggle to define and implement retention periods and to delete data in backups — the plan addresses both with a policy engine and expired-but-retained tombstones[^7].
- Legal hold always takes precedence over retention (Microsoft 365/Box principle)[^19].

#### 3.8 Cryptographic commitment & anchoring (allowed primitives only)

Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 are used:
- ed25519: signs audit entries and Signed Tree Heads/checkpoints (origin authentication, non-repudiation).
- sha256 / blake3: record content hashes (`Checksum`), hash-chain links, Merkle tree nodes, receipt-chain references (JCS-canonicalized, self-verifiable without network)[^24].
- xchacha20poly1305 / chacha20poly1305: encrypt record payloads at rest when confidentiality is required; retention deletes ciphertext while the hash/tombstone remains (ATL right-to-erasure pattern)[^15].
- Crypto-agility / renewal: because records may be retained for years, the audit chain supports re-time-attestation of prior entries under a fresh chain before an algorithm weakens (EP-EVIDENCE-RECORD / RFC 4998 Evidence Record Syntax renewal chain; RFC 3161 timestamps for offline existence proof)[^16]. SHA-256/BLAKE3 only — no SHA-3/Post-quantum primitives, per the allowed-crypto rule.

---

### Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Unauthorized modification or deletion of a protected record | Record integrity/immutability | Guard vetoes all writes/deletes against `ProtectionLevel`; `Compliance`/`Hold` reject unconditionally; every decision signed and chained |
| Insider / root-key override of a WORM record | Compliance (SEC 17a-4 compliance mode) | `Compliance` level has no override path, not even root; retention only extendable, never shortenable once locked |
| Replica divergence / data resurrection across replicas | Convergent deletion | Tombstones replicated like entries; removal gated on all-peer compaction (MTRO/MCCO) + `gc_grace_seconds` overlap checks |
| Truncation / elongation of the audit ledger | Audit trail integrity | Hash chain + Merkle STHs + signed checkpoints; any gap/reorder/edit breaks verification and names the entry |
| Rollback to a pre-protection state | Protection guarantee | Checkpoints are chained; reverting to an older state breaks the chain and fails verification against the latest anchored root |
| Signing-key compromise | Authenticity of proofs | Signed key-rotation handoff (old key seals the chain, new key continues); old public keys retained forever for verification |
| Clock manipulation to force early expiry | Retention correctness | `retain_until_date` evaluated against monotonic `IdsTime`, not wall clock; retention only extensible |
| Retention sweep / hold storm causing denial of service or mass deletion | Availability | Dispositions queued through review/purge gates; holds short-circuit and block mass purge; bounded per-peer metadata budgets |

The carrier is never trusted: protection metadata is signed over payload bytes, so it survives re-framing by any carrier, and the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### Integration with Existing Crates

| Crate | Role | Relevance to ids-immutable |
|-------|------|---------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `NodeId`, `RegionId`, `IdsTime`, `CarrierType` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All protection metadata travels as datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signatures, content hashes, encryption for `ids-immutable` |
| `ids-store` | Primary record store (mutation issuer) | Routes all writes/deletes through the `ids-immutable` guard before applying |
| `ids-fragment` | Record fragments/shards | Fragments inherit source protection level; compaction consults the guard |
| `ids-replication` | Replica synchronization | Replicates tombstones and protection metadata; convergent deletion via MTRO/MCCO |
| `ids-reclaim` (Phase 041) | Fragment compaction & space reclamation | Cannot reclaim retained/held records; expiry sweep output feeds reclamation eligibility |
| `ids-integrity` (Phase 030) | End-to-end checksums, Merkle trees, scrubbing | Reuses `Checksum`/Merkle primitives; `ids-immutable` audit chain builds on same content-addressed primitives |

---

### Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-immutable/` (a separate crate, not a folder inside an existing crate):
```
ids-immutable/
  Cargo.toml
  src/
    lib.rs                      # crate root, public API surface
    errors.rs                   # ProtectionError, HoldError, RetentionError, GuardError
    types.rs                    # ProtectionLevel, RetentionPolicy, LegalHold, Disposition, Tombstone, GuardDecision (re-exports ids-000-aa types)
    guard.rs                    # mutation veto/stamp entrypoint
    protection.rs               # ProtectionLevel state machine, retain-until-date, monotonic checks
    retention.rs                # retention-period engine, retain-until-date, extension-only
    hold.rs                     # legal hold: place/release, indefinite, overrides retention
    policy.rs                   # policy evaluator (max-duration-wins, hold-first short-circuit)
    expiry.rs                   # disposition: retain/review/purge/archive, grace days
    tombstone.rs                # tombstone lifecycle, expiry, removal gating
    replica.rs                  # cross-replica/fragment/derived-copy enforcement
    chain.rs                    # append-only hash chain, entry sealing
    merkle.rs                   # Merkle tree, Signed Tree Head, inclusion/consistency proofs
    anchor.rs                   # periodic signed checkpoint (ed25519) anchoring
    audit.rs                    # append-only audit log, verification, expired-but-retained reporting
    carrier_adapter.rs          # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
    contracts/                  # sealed trait contracts mirroring ids-000-aa style (~10 files)
      protection_contract.rs, retention_contract.rs, hold_contract.rs, chain_contract.rs,
      tombstone_contract.rs, replica_contract.rs, anchor_contract.rs, audit_contract.rs,
      guard_contract.rs, mod.rs
    invariants/                 # compile-time/runtime invariants (~8 files)
      no_override_compliance.rs, hold_overrides_retention.rs, monotonic_retain_until.rs,
      tombstone_removal_gated.rs, chain_integrity.rs, carrier_agnostic.rs, crypto_allowlist.rs, mod.rs
  tests/                        # property/verification tests (~20 files)
  benches/                      # (~6 files)
  docs/                         # (~12 files)
```
Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### Key Algorithms & Data Structures

- **ProtectionLevel** ∈ {None, Retention, Hold, Compliance}: resolved per `RecordId`; `Hold`/`Compliance` reject mutation unconditionally; `Retention` checks `retain_until_date` vs monotonic `IdsTime`; `None` permits.
- **Retain-until-date**: monotonic; set at protection time; extendable only (never shortened); once locked (compliance mode) immutable for the period.
- **Legal hold**: boolean + hold reference + placed/released timestamps + releasing actor; indefinite; checked first, short-circuits every disposition.
- **Policy engine**: for each record, collect all matching retention/hold rules; apply max-duration-wins; legal hold short-circuits to `RETAIN`; emit disposition {retain, review, purge, archive} with grace days; record to audit log.
- **Hash chain**: each audit entry = SHA-256/BLAKE3(entry || prev_digest || seq); verification replays the chain; any edit/gap/reorder breaks and names the entry.
- **Merkle tree + STH**: periodic Merkle root over the ledger, ed25519-signed; inclusion/consistency proofs prove append-only between any two STHs.
- **Tombstone**: {sequence range, Merkle root, signed checkpoint, deleted_at, retention/hold reason}; survives forever; verifiers report "expired-but-retained".
- **Tombstone removal**: eligible only after expired AND no overlap with live data on any replica (gc_grace threshold) AND removal offset ≤ min(MCCO across replica set) — MTRO/MCCO rule.
- **Signed checkpoint**: ed25519 signature over Merkle root + seq range + prev checkpoint; chained so removing an anchor is detectable; old public keys retained for rotation verification.

---

### Dependencies

- `ids-000-aa` — canonical types (`RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of protected envelopes, audit entries, checkpoints.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### Measurable Acceptance Criteria

1. While a record's retention period is active, 100% of write/modify/delete attempts against it are rejected by the guard; after expiry, disposition governs.
2. An active legal hold blocks 100% of modify/delete attempts regardless of retention state; release is the only clearing path and is audited.
3. `Compliance` (WORM) level rejects modification/deletion with no override path (not even root) for the full retention period.
4. Tamper detection catches 100% of single-record modifications, insertions, deletions, and reorderings in the audit ledger; verification names the offending entry.
5. A tombstone is physically removed only after (a) expiry, (b) no overlap with live data on any replica, and (c) removal offset ≤ min(MCCO across the replica set) — zero premature removals under any replica lag/partition.
6. Retention can only be extended, never shortened, once set; a locked retention policy is irreversible.
7. Every protection decision (place hold, set/extend retention, dispose, release hold) is recorded in the append-only, hash-chained, signed audit log with actor + `IdsTime`.
8. All protection metadata is carried as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS in the crate.
9. Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 appear in the crate's crypto surface.
10. Expired-but-retained records are reported by verifiers via retained signed checkpoints (not false gaps); payload deletion leaves a verifiable tombstone.

---

### Remaining Uncertainties

- Clock source for `retain_until_date`: monotonic `IdsTime` vs loosely-synced wall time; monotonic avoids manipulation but complicates cross-node absolute deadlines without a network clock.
- Interaction of key compromise/revocation with an active legal hold or compliance record: rotation preserves verifiability, but a compromised signing key's past checkpoints remain valid (content trust ≠ identity trust) — revocation policy is out of scope for this crate.
- Cross-replica tombstone convergence under long partitions: MTRO stalls with an unavailable replica (correct but may delay reclamation); acceptable trade-off, needs operational tuning of `gc_grace_seconds`.
- Performance of per-record hash chain at high write throughput; Merkle batching interval and anchor frequency are tunable parameters to be validated.
- Interaction with Phase 041 `ids-reclaim`: compaction must not violate retention/hold; the exact guard-reclaim handshake (eligibility proof) is defined here but validated at integration.

---

### Engineering Decisions Summary

- Separate crate `ids-immutable` (not a folder inside an existing crate), organized into subfolders/files by subsystem — one file per concern.
- Two independent controls (retention + legal hold); hold always wins; compliance/WORM has no override.
- Retention is extendable-only and irreversible once locked (mirrors locked bucket policy).
- Deletion never erases history: tombstones (sequence range, Merkle root, signed checkpoint) survive forever; "expired-but-retained" is verifiable, not a gap.
- Convergent deletion across replicas via tombstone + MTRO/MCCO + `gc_grace_seconds` overlap checks.
- Tamper detection via SHA-256/BLAKE3 hash chain + Merkle STHs + ed25519-signed checkpoints, verifiable offline.
- Only allowed crypto; carrier-agnostic boundary via `ids-carrier::Endpoint`; no network stack.

---

### Sources

[^1]: AWS, "S3 Object Lock" — WORM, fixed/variable retention periods, legal hold (indefinite until released), governance vs compliance mode, SEC 17a-4 / FINRA 4511 / CFTC 1.31 assessed by Cohasset.
[^2]: Microsoft, "Immutable Storage for Blob Data Overview" — time-based retention + legal hold, WORM, SEC 17a-4(f) / CFTC 1.31(d) / FINRA.
[^3]: Google Cloud, "Bucket Lock" / "Object Retention Lock" — bucket- and object-level retention, irreversible lock, max retention 100 years, project lien.
[^4]: SEC, "Rule 17a-4 amendments" (2022) — WORM (non-rewriteable/non-erasable) vs audit-trail alternative: time-stamped audit trail of all modifications/deletions, actor identity, recreation of original.
[^5]: FINRA / CFTC — Rule 4511 / Regulation 1.31 financial electronic recordkeeping immutability requirements.
[^6]: ICO (UK GDPR), "Storage limitation" (Art. 5(1)(e)) and "Right to erasure" (Art. 17) — retention schedules, backups "beyond use", non-absolute erasure with exceptions.
[^7]: EDPB, "2025 right-to-erasure coordinated report" — difficulties defining/implementing retention periods and deleting data in back-ups.
[^8]: Google Trillian / Certificate Transparency — append-only logs, binary Merkle tree, Signed Tree Heads, inclusion + consistency proofs (RFC 6962).
[^9]: Facebook LogDevice — distributed append-only, trimmable, record-oriented log with LSN.
[^10]: InfluxDB / CockroachDB / Spanner / Apache Doris — row-level TTL, periodic background retention enforcement, query-time invisibility before physical reclaim.
[^11]: ScyllaDB / Cassandra — tombstones, `gc_grace_seconds`, repair-based GC, overlap checks preventing cross-replica data resurrection.
[^12]: Apache Kafka / Redpanda — log-compaction tombstones, `delete.retention.ms`, MTRO/MCCO (tombstone removed only after every replica compacted past it).
[^13]: Elasticsearch ILM — rollover and time-based delete phases for append-only data streams.
[^14]: IBM Research, "Logging for Compliance" — cryptographically immutable log, distributed-ledger checkpoints, rotation after retention, anti-truncation/elongation.
[^15]: ATL Protocol — append-only log with cryptographic proofs, Bitcoin anchoring, Super-Tree consistency_to_origin, right-to-erasure via payload deletion.
[^16]: EP-EVIDENCE-RECORD / RFC 4998 Evidence Record Syntax — crypto-agile renewal chain re-time-attested before algorithm aging, RFC 3161 timestamps.
[^17]: Prooflog architecture — retention keeps tombstone row + signed checkpoint forever; legal hold overrides expiry; signed key-rotation handoff; old public keys retained.
[^18]: Open Archiver, "Retention Policy Engine" — policy-as-code, max-duration-wins, legal hold short-circuits first, disposition retain/review/purge/archive, grace days, immutable audit log.
[^19]: Microsoft 365 / Box — retention vs eDiscovery/litigation hold; hold takes precedence over retention.
[^20]: "Authenticated Append-Only Skip Lists" — tamper-evident append-only, one-way digests, inclusion proofs, anti-invention/anti-equivocation.
[^21]: Agent Flight Recorder / AuditWeave / kxco-pq-audit — SHA-256 hash-chained tamper-evident audit trails, Merkle batching, anchoring.
[^22]: RGIP — triple-hash chain (SHA-256/BLAKE3, cross-chain hash); only SHA-256/BLAKE3 used here per the allowed-crypto rule.
[^23]: "Proof-of-Retention" (arxiv:2609.26654) — auditable retention proof-of-possession; auditor verifies retention without revealing data.
[^24]: IETF draft-hopley-x402-retention-chain — SHA-256 receipt chain, JCS canonicalization, self-verifiable without network.
[^25]: Prolly trees / haematite / immure / kin-blobs — content-addressed immutable storage, deduplication, GC via keep_last/keep_for/keep_versions.
