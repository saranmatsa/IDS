## Phase 083 — Create the ids-fencing-epoch crate: Epoch-based authority to prevent stale owners.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1240-1254)

```
83. Research only do not read plan.md **[Phase 083 — Create the ids-fencing-epoch crate: Epoch-based authority to prevent stale owners.]**

Crate: ids-fencing-epoch
Purpose: Epoch-based authority to prevent stale owners.
Contents: Fencing token increment, fencing token checking, epoch-based leadership, stale owner prevention, recovered region split-brain prevention, authority transfer verification.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 083 — Create the ids-fencing-epoch crate: Epoch-based authority to prevent stale owners.]**

Create plan-083.md in C:/idfs/plans/ for the ids-fencing-epoch crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 083 creates `ids-fencing-epoch`, a separate IDS crate (not a folder inside an existing crate, per the project rule of separate crates for new capabilities) that owns epoch-based authority for preventing stale owners. The crate issues monotonically increasing fencing tokens (u64 `FencingToken`) on every ownership/authority grant, maintains per-resource high-water marks of accepted tokens, and rejects any operation carrying a token below the current high-water mark. It defines epoch-based leadership (a leadership term is an epoch; a new leader bumps the epoch before acting), stale-owner prevention (any write/read carrying a stale token is rejected at the protected resource, not by the owner itself), recovered-region split-brain prevention (a region returning from partition must present a current signed epoch certificate before serving reads or writes; an epoch below the region's last-known epoch fences the region), and authority-transfer verification (each transfer carries an old→new epoch transition signed with ed25519 and verified against the high-water mark before the new owner is admitted).

`ids-fencing-epoch` never sends bytes itself: it returns verdicts (`Admit`, `RejectStale`, `RejectSplitBrain`, `DeferTransfer`) and signed epoch certificates that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B). Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. Allowed crypto only: ed25519 (epoch certificate + authority-transfer signatures), sha256/blake3 (epoch hash chains, high-water-mark binding), xchacha20poly1305/chacha20poly1305 (epoch state encrypted at rest).

A skeleton `ids-fencing-epoch` crate does NOT yet exist at `C:/idfs/ids-fencing-epoch/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `FencingToken` (u64), `Epoch` (u64), plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Martin Kleppmann's fencing-token pattern ("How to do distributed locking", Chubby sequencers, ZooKeeper zxid/version as fencing tokens), pg-fenced-lease (epoch column incremented on takeover, fence check inside the same transaction as the write), disco (etcd cluster revision as fencing token, high-water-mark validation rejects `token < mark`), SlateDB `FenceableTransactionalObject` (u64 epoch, init bumps epoch, update/refresh fail with `Fenced` when stored epoch > local epoch), NamiDB single-writer epoch fencing via manifest CAS without Raft/ZooKeeper, Raft term as fencing token (higher term steps a server down; lower-term messages rejected), Kafka KIP-903 (stale broker epoch not allowed to join ISR), Cassandra LWT compare-and-set via Paxos ballot, Viewstamped Replication view-id total ordering, SBD/STONITH storage fencing for HA clusters, epoch-based authority revocation (epoch increment invalidates all outstanding capabilities), celld epoch-prefixed storage paths with self-fencing leases, bounded staleness (K versions / T time), and epoch-based distributed memory reclamation.

---

### 2. Architecture Overview

`ids-fencing-epoch` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is an authority/fencing layer: given a resource handle, an owner claim, and a fencing token, it decides whether the owner is still current (token ≥ high-water mark), whether a leadership transition is valid (new epoch > current epoch), whether a recovered region may rejoin (epoch certificate valid and current), and whether an authority transfer is authentic (ed25519 signature over old→new epoch, nonce-bound, replay-cache checked). It returns decisions; the caller transmits them as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (fencing clients)               │
│  ids-store, ids-replication, ids-region — tagged operations + owner claims   │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 083: ids-fencing-epoch (THIS CRATE — fencing policy)    │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌─────────┐ │
│  │ fencing token    │ │ epoch-based      │ │ stale owner      │ │ region  │ │
│  │ increment        │ │ leadership       │ │ prevention         │ │ split-  │ │
│  │ (bump on grant)  │ │ (term = epoch)   │ │ (token < mark →    │ │ brain   │ │
│  └──────────────────┘ └──────────────────┘ │  reject)           │ │ prevent │ │
│  ┌──────────────────┐ └──────────────────┘ └──────────────────┘ │ │       │ │
│  │ high-water mark  │ │ authority transfer│ │ epoch certificate│ │         │ │
│  │ (per-resource)   │ │ verification      │ │ (signed, current)│ │         │ │
│  └──────────────────┘ (ed25519, nonce-bound)└──────────────────┘ │         │ │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │         │ │
│  │ epoch hash chain │ │ epoch state at   │ │ verdicts: Admit/ │ │         │ │
│  │ (sha256/blake3)  │ │ rest (xchacha20/ │ │ Reject/Defer     │ │         │ │
│  └──────────────────┘ │ chacha20poly1305)│ └──────────────────┘ │         │ │
└─────────────────────────┬──────────────────┴──────────────────┬─┴──────────┘
                          │ opaque Vec<u8> datagrams              │
                          ▼                                       │
┌─────────────────────────────────────────────────────────────────┘
│              CARRIER BOUNDARY: ids-carrier::Endpoint only        │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B},   │
│  payload: &[u8])     receive_datagram → (Endpoint, Vec<u8>)     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC /    │
│  HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN / mesh         │
└─────────────────────────────────────────────────────────────────┘
```

**Data flow of a fenced write:** client submits `Write{resource_id, payload, owner, fencing_token}` → `ids-fencing-epoch` checks `fencing_token ≥ high_water_mark[resource_id]` → returns `Verdict::Admit` (and bumps high-water mark to the token) or `Verdict::RejectStale{current_mark}` → caller sends the write as opaque datagram(s) to `ids-carrier::Endpoint` targets only when admitted; rejected writes never leave the client.

**Data flow of a leadership transition:** new leader candidate submits `LeaderClaim{region_id, proposed_epoch, proof}` → crate checks `proposed_epoch > current_epoch[region_id]` → on success returns `EpochCertificate{region_id, epoch, signed_by: NodeId, signature: ed25519}` → caller broadcasts the certificate via `ids-carrier::Endpoint`; followers store the new epoch and reject any message from an older epoch.

**Data flow of region recovery after partition:** recovering region submits `RegionRejoin{region_id, last_known_epoch, certificate}` → crate validates the certificate chain and checks `certificate.epoch ≥ region_watermark[region_id]` → `Admit` lets the region serve reads/writes; below-watermark epochs return `RejectSplitBrain` (the region remains fenced until it synchronizes to a current epoch).

**Data flow of authority transfer:** `Transfer{resource_id, from, to, old_epoch, new_epoch, signature}` → crate verifies ed25519 signature over the transfer, checks `new_epoch = old_epoch + 1`, checks `old_epoch ≥ high_water_mark[resource_id]`, records the transfer in the hash-chained audit log, returns `TransferVerified` → caller frames the result to `ids-carrier::Endpoint`.

**Crypto used:** ed25519 signs epoch certificates and authority transfers; sha256/blake3 chain epoch entries and bind high-water marks; xchacha20poly1305/chacha20poly1305 encrypt persisted epoch state at rest (bounded-replay windows, causal histories). No other crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 14.

#### 3.1 Fencing tokens: monotonically increasing ownership generations (Kleppmann / Chubby / ZooKeeper)

**Source:** Martin Kleppmann, "How to do distributed locking" — a fencing token is a number that increases every time a client acquires the lock; the storage service rejects writes whose token has gone backwards. ZooKeeper's `zxid` (or znode version) and Chubby sequencers can serve as fencing tokens. Amazon DynamoDB lock clients and Google Chubby sequence numbers are production precedents. The `zxid = (epoch << 32) | txnCount` structure shows how an epoch field can be packed into a monotonic identifier.

`ids-fencing-epoch` adopts this exact pattern: every authority grant mints a strictly increasing u64 `FencingToken`; every protected operation carries its token; the protected resource persists the highest token accepted per resource and rejects `token < highest_seen`. The check lives at the resource/fencing layer, never at the owner — a paused or partitioned owner that wakes up and writes with an old token is fenced regardless of what it believes.

#### 3.2 Lease-backed epoch increments and atomic fence checks (pg-fenced-lease pattern)

**Source:** pg-fenced-lease (Rust crate) — a lease row holds `name`, `holder`, `epoch`, `expires_at`; acquisition increments `epoch` by 1 (the new epoch is the fencing token); renewal touches only `expires_at` and matches on `holder AND epoch` (a mismatch means someone else took over); the fence check runs inside the same transaction as the write (`FOR KEY SHARE` on the lease row), so a stale leader's write can never commit.

`ids-fencing-epoch` mirrors this: `fencing_token_increment()` bumps the epoch atomically on ownership transfer and never on renewal; `fencing_token_check()` validates holder + epoch + expiry in a single atomic decision; renewal never advances the fencing token (only the liveness deadline), so a renewed lease cannot be confused with a new ownership generation.

#### 3.3 High-water mark validation (disco / etcd cluster revision)

**Source:** disco — fencing token = etcd cluster revision at acquisition; the resource tracks the highest token ever seen: `token ≥ mark` → accepted and mark advances; `token < mark` → rejected as stale owner. ZooKeeper zxid and etcd revision are globally ordered within the cluster, so either works as a fencing token.

`ids-fencing-epoch` implements a per-resource `HighWaterMark` (u64, initialized to 0 / `FencingToken::ZERO`). The decision is `Admit` iff `token >= mark`; on admit, `mark = token + 1` semantics (strictly increasing ownership generations) or `mark = token` with `>` comparison — both forms are specified; the crate exposes both `ge` (accept-equal, idempotent retries) and `gt` (strict, single-use) modes so callers choose per-resource semantics.

#### 3.4 Epoch fencing for transactional objects (SlateDB / NamiDB)

**Source:** SlateDB `FenceableTransactionalObject` — wraps a transactional object with a monotonic u64 epoch; `init` sets the epoch to the next value; every `update`/`refresh` first checks `stored_epoch > local_epoch` and fails with `Fenced` if so; the epoch is never reset. NamiDB single-writer per namespace uses manifest CAS: a writer picks `epoch = current_manifest.epoch + 1` and commits via CAS; losing the CAS race means discovering a newer epoch and rejecting local operations — single-writer enforced without Raft, ZooKeeper, or file locks.

`ids-fencing-epoch` adopts the same structure: an `EpochFence` type holds `current_epoch: Epoch`; `claim_epoch(next: Epoch)` succeeds iff `next > current_epoch` (strict monotonicity) and persists the advance; any operation presenting `epoch <= current_epoch` is `Fenced`. This gives stale-owner prevention without a consensus dependency — the crate itself is the fencing authority for in-process and cross-carrier resources.

#### 3.5 Leader epoch / Raft term as fencing token

**Source:** Raft — time divided into terms (epochs); every message carries the sender's term; a server seeing a higher term immediately steps down; messages from a lower term are rejected. etcd's read-index confirms leadership via a majority heartbeat round before serving reads, so a partitioned leader cannot serve stale reads. Kafka KIP-903: a replica with a stale broker epoch is not allowed to join the ISR; the broker epoch changes on every restart, so the controller always knows the latest epoch.

`ids-fencing-epoch` maps `Epoch` directly to a Raft-style term: `Epoch::zero()` initial; `next_epoch() = current + 1`; leadership claims carry `proposed_epoch`; any peer observing `proposed_epoch > current_epoch` adopts it and rejects older-epoch messages; followers attached to a leader reject client reads/writes tagged with an older epoch. Read-path safety (read-index style) is exposed as `read_lease_valid(region_id, epoch)` — a read may proceed only if the region's epoch was confirmed by quorum within a bounded window.

#### 3.6 Split-brain prevention: quorums, fencing, epochs

**Source:** systemoverflow "Preventing Split Brain: Quorums, Fencing, and Epochs" — majority quorums ensure at most one partition forms a majority, but quorums alone are insufficient: a paused leader resuming after lease expiry must be fenced via monotonically increasing epochs persisted and validated on every state-changing operation. SBD/STONITH (storage-based death fencing) provides node self-fencing for HA clusters: an unresponsive node stops resetting its watchdog and the cluster fences it.

`ids-fencing-epoch` implements recovered-region split-brain prevention: a region that rejoins after a partition must present an `EpochCertificate` (ed25519-signed, covering `region_id`, `epoch`, `prev_hash`, `nonce`) with `epoch >= region_watermark`; below-watermark epochs trigger `RejectSplitBrain` and the region stays fenced until it syncs to a current epoch certificate. Self-fencing is exposed as `self_fence(region_id)` — a region that misses its lease deadline fences itself rather than waiting to be told it is superseded (celld self-fencing lease pattern).

#### 3.7 Authority transfer verification

**Source:** epoch-based authority revocation — incrementing an account/authority epoch invalidates all outstanding capabilities at once (`capability.epoch == current_epoch`); BLS/distributed authority systems verify monotonic sequence and revocation epochs. Viewstamped Replication: view changes require `f+1` replicas to know a request before execution, and the new view starts from a quorum; view-ids are totally ordered.

`ids-fencing-epoch` verifies transfers: `Transfer{resource_id, from, to, old_epoch, new_epoch, signature, nonce}` where `signature = Sign(ed25519, resource_id || from || to || old_epoch || new_epoch || nonce)`. Verification checks: signature valid under `from`'s key (from `ids-000-aa` `NodeId`-bound key registry), `new_epoch == old_epoch + 1`, `old_epoch >= high_water_mark[resource_id]`, nonce not in replay cache, then advances `high_water_mark[resource_id] = new_epoch` and records the transfer in the hash-chained audit log. Only after `TransferVerified` may `to` act as owner.

#### 3.8 Stale-owner prevention semantics

**Source:** kleppmann + stackbook fencing-tokens pattern — the check must be atomic with the write (conditional update); fencing prevents an older generation from overwriting a newer accepted generation, but does not undo work committed before the new epoch arrived. If that window is unacceptable, the destination needs current-ownership validation inside the same transaction as the side effect.

`ids-fencing-epoch` documents this boundary precisely: `fencing_token_check()` rejects stale tokens, but the crate's contract states that admission is a necessary, not sufficient, condition — the protected resource must apply the check inside the same atomic transition as the state change (ids-store conditional write, ids-replication append, etc.). The crate exposes `fence_atomic(resource_id, token) -> Verdict` returning both verdict and the new high-water mark so callers can combine check-and-apply in one step.

#### 3.9 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek / RFC 8032 (EdDSA over Curve25519); chacha20poly1305 docs.rs (RFC 8439 AEAD + XChaCha20-Poly1305 192-bit nonce); SHA-256 (NIST FIPS 180-4); BLAKE3 (BLAKE3-team README, 256-bit output, SIMD, keyed hash, KDF, XOF).

`ids-fencing-epoch` uses:
- ed25519: signing epoch certificates and authority-transfer proofs (proof of fencing authority).
- sha256: epoch hash-chain links, high-water-mark binding, resource-id hashing for audit entries.
- blake3: fast epoch-certificate hashing, checkpoint chaining, per-region digest summaries (preferred where parallelism/speed matters).
- xchacha20poly1305: encrypting persisted epoch state at rest with 24-byte random nonces (safe under random generation).
- chacha20poly1305: encrypting session/transfer payloads with 12-byte counter nonces (bounded use).

No other crypto. `ids-crypto` is the sole crypto surface; `ids-fencing-epoch` references primitive IDs via `ids-000-aa` only.

#### 3.9 (b) Carrier-agnostic transport

**Source:** IDS carrier-agnostic rule (prompts.txt entries 1, 83): `Endpoint = CarrierType + Vec<u8>` (max 1024 bytes); `CarrierType` = `Loopback, Serial, Optical, Radio, Custom(u16)`; no IP/TCP/UDP/QUIC/HTTP/DNS or any networking stack.

`ids-fencing-epoch` never addresses a network by name. It returns `Verdict`, `EpochCertificate`, and `TransferProof` values; the caller serializes them and sends via `ids-carrier::Endpoint`. All cross-node fencing traffic is opaque `Vec<u8>` datagrams. The crate depends on `ids-carrier` only for the `Endpoint` type (re-exported from `ids-000-aa`) — or directly on `ids-000-aa` to avoid pulling carrier implementations into a pure fencing policy crate.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Stale owner writes after lease expiry / GC pause | Data integrity | Monotonic fencing token per ownership grant; resource rejects `token < high_water_mark` |
| Stale leader serves reads after partition | Read correctness | Epoch-based leadership; read-path `read_lease_valid` requires quorum-confirmed epoch |
| Split-brain: two regions both active after partition heals | State divergence | Recovered-region split-brain prevention: rejoin requires current signed `EpochCertificate`; below-watermark → `RejectSplitBrain` |
| Lost fencing token / counter rollback after failover | Authority integrity | Epoch persisted with each advance; never reset; high-water mark survives restarts (pattern: pg-fenced-lease `epoch` column, SlateDB never-reset epoch) |
| Replay of an old epoch certificate or transfer | Replay attacks | Nonce-bound ed25519 signatures; replay cache per resource; hash-chained audit log detects truncation |
| Authority transfer to wrong recipient | Unauthorized ownership | ed25519 signature over `resource_id || from || to || old_epoch || new_epoch || nonce`; `old_epoch >= high_water_mark` pre-condition |
| Clock manipulation for epoch ordering | Causality correctness | Logical u64 epochs only; never wall-clock for authority decisions; monotonic `IdsTime` only for lease deadlines |
| Carrier delays/duplicates/reorders fencing datagrams | Replay/ordering | Idempotent high-water-mark comparison tolerates duplicates; verdicts carry epoch so stale certificates are self-identifying |
| Profile/epoch downgrade attack | Authority integrity | Epoch is immutable per resource/region; downgrade requires signed governance transfer, audited |
| Key extraction (epoch signing key) | Certificate forgery | Signing keys never leave the node; epoch state encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |

The carrier is never trusted: fencing verdicts and certificates are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-fencing-epoch |
|-------|------|-------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `FencingToken` (u64), `Epoch` (u64), `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All fencing traffic flows to/from `Endpoint`; crate returns verdicts, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing epoch certificates/authority transfers; encrypting epoch state at rest; hashing epoch chains |
| `ids-store` | Primary record store (fencing client) | Tags writes with fencing token; consults `ids-fencing-epoch` before admitting mutations |
| `ids-replication` | Replica synchronization | Rejects log entries tagged below the leader's epoch; fences stale leaders |
| `ids-region` | Region membership / recovery | Region rejoin gated on `EpochCertificate`; split-brain prevention on partition heal |
| `ids-fault` | Fault detection / STONITH integration | `self_fence()` hook for regions/nodes that miss lease deadlines |
| `ids-session` (Phase 050) | Client-session binding | Session-bound operations carry the session's fencing epoch; transfer audit cross-referenced |

**Integration points:** `ids-fencing-epoch` evaluates authority and returns verdicts/certificates; callers (`ids-store`, `ids-replication`, `ids-region`) enforce the verdict at the point of state change and frame certificates into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No crate-internal reach; no dependency on `ids-discovery`/`ids-session` beyond what contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-fencing-epoch/` (a separate crate, not a folder inside an existing crate):

```
ids-fencing-epoch/
├── Cargo.toml                          # Crate manifest; ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # Crate root, public API surface, feature gates
├── constants.rs                        # Fencing defaults: initial epoch, high-water-mark init, replay-cache TTL
├── error.rs                            # FenceError (thiserror): StaleToken, SplitBrain, InvalidTransfer, Crypto, Carrier errors
├── types.rs                            # FencingToken, Epoch, Verdict, EpochCertificate, TransferProof (re-exports ids-000-aa types)
├── fencing_token.rs                    # Fencing token increment, high-water mark, token check (ge/gt modes)
├── epoch.rs                            # Epoch-based leadership: claim_epoch, step_down, epoch comparison
├── authority.rs                        # Authority ownership model, owner registry, grant/revoke
├── stale_owner.rs                      # Stale-owner prevention: token < mark → reject, admission semantics
├── region_split_brain.rs               # Recovered-region split-brain prevention, region watermarks
├── authority_transfer.rs               # Authority transfer verification (ed25519, nonce, replay cache)
├── epoch_certificate.rs                # Signed epoch certificates (ed25519), cert validation chain
├── high_water_mark.rs                  # Per-resource high-water mark persistence and advance
├── epoch_hash_chain.rs                 # sha256/blake3 epoch hash chain for audit binding
├── epoch_state_enc.rs                  # Epoch state encryption at rest (xchacha20poly1305/chacha20poly1305)
├── replay_cache.rs                     # Nonce replay cache for transfers/certificates
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── fencing_contract.rs             # trait Fencing: increment/check/advance
│   ├── epoch_contract.rs               # trait EpochAuthority: claim/step_down/compare
│   ├── stale_owner_contract.rs         # trait StaleOwnerGuard: check/admit/reject
│   ├── region_contract.rs              # trait RegionFence: rejoin/watermark/self_fence
│   ├── transfer_contract.rs            # trait TransferVerifier: verify/admit/reject
│   ├── certificate_contract.rs         # trait EpochCert: sign/validate/chain
│   ├── high_water_contract.rs          # trait HighWaterMark: read/advance/reset
│   ├── carrier_contract.rs             # opaque Vec<u8> framing to ids-carrier::Endpoint
│   ├── audit_contract.rs               # trait AuditChain: append/verify/truncate-detect
│   └── mod.rs (duplicate guard — see mod.rs only)  # consolidated module declarations
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── monotonic_epoch.rs              # epoch only increases, never resets
│   ├── token_ge_mark.rs                # admit iff token >= high_water_mark
│   ├── transfer_chain.rs               # new_epoch == old_epoch + 1
│   ├── cert_current.rs                 # rejoin cert epoch >= region watermark
│   ├── carrier_agnostic.rs             # no std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305
│   ├── replay_resistance.rs            # nonce single-use
│   └── audit_chain_sound.rs            # hash chain detects truncation
├── audit/                              # Signed, hash-chained epoch audit log (~6 files)
│   ├── mod.rs
│   ├── audit_entry.rs                  # FencingAuditEntry (grant, reject, transfer, self-fence)
│   ├── audit_chain.rs                  # sha256/blake3 hash chain + entry linking
│   ├── checkpoint.rs                   # ed25519-signed periodic checkpoints
│   ├── signer.rs                       # ed25519 signing of certificates/checkpoints
│   ├── verifier.rs                     # ed25519 signature verification
│   └── replay_cache.rs                 # (shared with src) nonce cache
├── tests/                              # Property/verification tests (~24 files)
│   ├── mod.rs
│   ├── fencing_token_tests.rs
│   ├── high_water_mark_tests.rs
│   ├── epoch_leadership_tests.rs
│   ├── stale_owner_tests.rs
│   ├── region_split_brain_tests.rs
│   ├── authority_transfer_tests.rs
│   ├── certificate_tests.rs
│   ├── encryption_tests.rs             # xchacha20/chacha20 round-trip
│   ├── hash_chain_tests.rs
│   ├── replay_cache_tests.rs
│   ├── atomic_check_tests.rs           # check-and-apply atomicity
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net usage
│   ├── crypto_allowlist_tests.rs       # only allowed algorithms reachable
│   ├── invariant_tests.rs
│   ├── edge_case_tests.rs              # token == mark, epoch wrap (saturating), zero epoch
│   ├── determinism_tests.rs            # same inputs ⇒ same verdicts
│   ├── fuzz_fencing.rs                 # proptest: token sequences, reordering, duplicates
│   └── fuzz_transfer.rs                # proptest: adversarial transfer forgery
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── fencing_token.rs                # increment throughput
│   ├── token_check.rs                  # check latency
│   ├── transfer_verify.rs              # ed25519 verify throughput
│   ├── certificate.rs                  # cert sign/validate latency
│   ├── region_rejoin.rs                # split-brain decision latency
│   ├── hash_chain.rs                   # chain append throughput
│   ├── replay_cache.rs                 # lookup throughput
│   └── carrier_agnostic.rs             # boundary overhead only (Vec<u8> framing)
├── spec/                               # Normative specifications (~4 files)
│   ├── fencing_spec.md                 # Token increment/check semantics
│   ├── epoch_leadership_spec.md        # Epoch-based leadership protocol
│   ├── split_brain_spec.md             # Recovered-region split-brain prevention
│   ├── transfer_spec.md                # Authority transfer verification
├── docs/                               # (~10 files)
│   ├── architecture.md
│   ├── stale_owner_guide.md            # How protected resources enforce fencing
│   ├── region_recovery.md              # Recovered region rejoin procedure
│   ├── authority_transfer.md           # Transfer procedure and key rotation
│   ├── crypto_policy.md                # Allowed-crypto allowlist for this crate
│   ├── carrier_boundary.md             # Carrier-agnostic boundary contract
│   ├── audit_log.md                    # Audit chain format and verification
│   ├── edge_cases.md                   # Token equality, epoch wrap, zero state
│   ├── integration_examples.md         # ids-store / ids-replication / ids-region call patterns
│   └── runbook.md                      # Operating the fencing layer
└── README.md
```

Total: ~110–140 files (within the ~90–1000 target range; room for growth as invariants/contracts expand). All files are `.rs` source + `Cargo.toml` + tests/specs/docs; no binary targets.

---

### 7. Key Algorithms & Data Structures

**1. Fencing token increment (monotonic):** `increment(current: FencingToken) -> FencingToken = current + 1` with saturating guard at `u64::MAX` (epoch wrap → `EpochWrap` error; operator intervention required). Issued exactly once per ownership grant; never on renewal.

**2. High-water mark:** per-resource `HighWaterMark { mark: FencingToken }`, initialized to `FencingToken::ZERO`. `check(token) -> Verdict`: `token >= mark` → `Admit` (mark advanced to `token` in strict mode or left for idempotent mode); `token < mark` → `RejectStale{ current: mark }`. Check and advance are a single atomic decision.

**3. Epoch-based leadership:** `claim_epoch(region_id, proposed_epoch)` succeeds iff `proposed_epoch > current_epoch[region_id]`; on success, `current_epoch = proposed_epoch` and an `EpochCertificate` is minted. Any message tagged with `epoch < current_epoch` is rejected; peers observing a higher epoch step down immediately (Raft-term rule).

**4. Stale-owner prevention:** `stale_owner_guard(resource_id, token, owner)`: reject if `token < high_water_mark[resource_id]` OR `owner != recorded_owner[resource_id]` at that token. The protected resource applies the check inside the same atomic transition as the state change (conditional write), so admission ⟹ current ownership at commit time.

**5. Recovered-region split-brain prevention:** `region_rejoin(region_id, last_known_epoch, certificate)`: validate `ed25519` signature over `region_id || epoch || prev_hash || nonce`; require `certificate.epoch >= region_watermark[region_id]`; on success advance `region_watermark = certificate.epoch`; below-watermark → `RejectSplitBrain` and the region stays fenced until synchronized. `self_fence(region_id)` lets a region that misses its lease deadline fence itself.

**6. Authority transfer verification:** `verify_transfer(Transfer{resource_id, from, to, old_epoch, new_epoch, signature, nonce})`: checks ed25519 signature under `from`'s key; `new_epoch == old_epoch + 1`; `old_epoch >= high_water_mark[resource_id]`; `nonce` unseen; then `high_water_mark = new_epoch`, `recorded_owner = to`, append to hash-chained audit log, return `TransferVerified`.

**7. Epoch hash chain:** each audit entry `e_i` includes `prev = H(e_{i-1})` (sha256 or blake3), establishing total order and making truncation detectable; periodic ed25519-signed checkpoints anchor the chain.

**8. Epoch state encryption at rest:** persisted epoch/watermark state encrypted with xchacha20poly1305 (random 24-byte nonces) or chacha20poly1305 (counter nonces, bounded use).

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `FencingToken`, `Epoch`) and sealed trait contracts, `Error` |
| `ids-carrier` | `Endpoint` type (`CarrierType` + `Vec<u8>`) — re-exported via `ids-000-aa`; decision transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` | Serialization of fencing envelopes, certificates, audit entries |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list; `ids-fencing-epoch` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-083.1 | Fencing token increment is strictly monotonic: every grant returns `token = previous + 1` | Unit test over 100k sequential grants; no duplicates, no regressions |
| AC-083.2 | `check(token)` admits iff `token >= high_water_mark`; rejects `token < mark` with `RejectStale{current}` | Property test (proptest) over random token sequences; 100% rejection of stale tokens |
| AC-083.3 | Epoch-based leadership: `claim_epoch(proposed)` succeeds iff `proposed > current`; peers step down on higher epoch | Deterministic test over region epoch transitions; lower-epoch messages rejected 100% |
| AC-083.4 | Stale owner prevention: write with token below mark is rejected; admitted write advances mark | End-to-end verdict test; stale writes never admitted |
| AC-083.5 | Recovered-region split-brain prevention: rejoin requires `EpochCertificate.epoch >= region_watermark`; below-watermark → `RejectSplitBrain` | Region rejoin tests with valid/stale certificates; 100% correct admission/rejection |
| AC-083.6 | Authority transfer verification: ed25519 signature over transfer fields; `new_epoch == old_epoch + 1`; `old_epoch >= mark`; nonce single-use | Transfer tests with forged/old/replayed transfers; 100% rejection of invalid transfers |
| AC-083.7 | Epoch hash chain: truncation detectable; ed25519 checkpoints verify offline | Chain append + truncation test; checkpoint verify test |
| AC-083.8 | Epoch state encryption at rest round-trips via xchacha20poly1305 and chacha20poly1305 | Encrypt/decrypt tests at payload sizes 64 B … 1 MiB |
| AC-083.9 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh in the crate | Dependency audit test (`cargo tree`) + source grep — zero matches |
| AC-083.10 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 reachable in the crate's crypto surface | Allowlist unit test passes |
| AC-083.11 | Every grant/reject/transfer/self-fence recorded in append-only, hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime` | Audit log append + verify tests |
| AC-083.12 | Crate compiles cleanly: `cargo check -p ids-fencing-epoch` exits 0 with no warnings | Compilation check |
| AC-083.13 | All tests pass: `cargo test -p ids-fencing-epoch` | Test run |

---

### 10. Remaining Uncertainties

1. **High-water-mark advance semantics:** `mark = token` (strict, single-use) vs `mark = token + 1` (admit-equal for idempotent retries) — per-resource configuration to be decided at integration with `ids-store`/`ids-replication`.
2. **Persistence of high-water marks:** in-memory vs durable (manifest CAS per NamiDB pattern) — durability vs performance tradeoff unquantified; the crate exposes the interface, storage backend chosen per deployment.
3. **Epoch wrap at u64::MAX:** saturating guard with operator intervention — expected never to trigger in practice; handling (fail-closed vs epoch reset with re-certification) undecided.
4. **Replay-cache size and eviction:** bounded cache for transfer/certificate nonces; eviction policy (LRU, time-window) workload-dependent.
5. **Read-lease validity window:** `read_lease_valid` bounded window (IdsTime ticks) — length trades staleness vs availability; SLO definition pending.
6. **Interaction with `ids-fault` / STONITH:** `self_fence()` hook integration with node fencing agents to be validated at integration.
7. **Cross-region epoch propagation latency:** how quickly a new epoch certificate reaches all regions; staleness window during propagation unmeasured.
8. **Key rotation for epoch signing keys:** ed25519 key rotation procedure and certificate validation across key changes undecided.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-fencing-epoch` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear fencing-authority boundary |
| u64 `FencingToken`/`Epoch` from `ids-000-aa` | Single canonical definition; zero-cost newtypes; consistent with IDS type system |
| Strict monotonicity, epoch never reset | Matches pg-fenced-lease / SlateDB / NamiDB epoch-fencing precedent; prevents counter rollback after failover |
| Check lives at the resource, verdict returned by crate | Kleppmann fencing pattern: enforcement must be at the protected resource; crate never sends bytes |
| `token >= mark` admit mode (with strict `>` variant) | Balances idempotent retries (admit-equal) vs strict single-use ownership; caller chooses per resource |
| Recovered-region split-brain via signed `EpochCertificate` | Region must prove currency before serving reads/writes; prevents split-brain on partition heal |
| Authority transfer = ed25519-signed old→new epoch + nonce | Unforgeable transfer proof; replay cache prevents reuse; old-epoch pre-condition prevents hijack |
| Hash-chained audit log + ed25519 checkpoints | Offline-verifiable decision log; tamper evidence (sha256/blake3 chain) |
| Logical epochs only; no wall-clock for authority | Clock manipulation cannot break fencing order; `IdsTime` only for lease deadlines |
| Carrier-agnostic boundary via `ids-carrier::Endpoint` | Preserves IDS architecture; no network stack in trust base |
| Allowed crypto only; `ids-crypto` is sole crypto surface | Complies with IDS allowed-crypto rule |

---

### 12. Sources

1. Martin Kleppmann, "How to do distributed locking" — fencing token pattern, Chubby sequencers, ZooKeeper zxid as fencing token, storage service rejects backwards tokens. https://martin.kleppmann.com/2016/02/08/how-to-do-distributed-locking.html
2. pg-fenced-lease (Rust) — epoch-based lease, `epoch` incremented on takeover as fencing token, fence check inside the write transaction, renewal matches holder+epoch. https://docs.rs/pg-fenced-lease
3. disco (ahrtr/disco) — lease-based locks with fencing tokens from etcd cluster revision; high-water-mark validation (`token ≥ mark` accept, `token < mark` reject). https://github.com/ahrtr/disco
4. SlateDB `slatedb-txn-obj` `FenceableTransactionalObject` — monotonic u64 epoch fencing, `init` bumps epoch, `Fenced` on stale local epoch, never reset. https://docs.rs/slatedb-txn-obj
5. NamiDB storage-engine RFC — single-writer epoch fencing via manifest CAS (`epoch = current_manifest.epoch + 1`), no Raft/ZooKeeper/file lock. https://github.com/namidb/namidb/blob/main/docs/rfc/001-storage-engine.md
6. etcd Raft — term as epoch: higher term steps server down, lower-term messages rejected; read-index confirms leadership via quorum. https://github.com/etcd-io/raft/blob/main/doc.go
7. ZooKeeper leader election — `zxid = (epoch << 32) \| txnCount`; higher zxid wins; fencing prevents waking leader from writing. https://medium.com/@naveen.saini.eee16/the-tale-of-split-brain-34b42fe6a7e3
8. Apache Cassandra lightweight transactions — compare-and-set via Paxos ballot; generation-based fencing of writers. https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/dml/dmlLtwtTransactions.html
9. Apache Kafka KIP-903 — replicas with stale broker epoch not allowed to join ISR; broker epoch changes on restart. https://wiki.apache.org/confluence/pages/240880955/KIP-903
10. Viewstamped Replication (revisited) — totally ordered view-ids; view change requires quorum (`f+1` replicas know each request). https://cs.princeton.edu/courses/archive/fall11/cos518/papers/viewstamped.pdf
11. systemoverflow — "Preventing Split Brain: Quorums, Fencing, and Epochs"; quorums insufficient alone, epochs persisted and validated on every state-changing operation. https://systemoverflow.com/learn/distributed-primitives/leader-election/preventing-split-brain-quorums-fencing-and-epochs
12. SUSE SBD/STONITH documentation — storage-based death fencing for HA clusters; node self-fencing on unresponsiveness. https://documentation.suse.com/sle-ha/15-SP7/html/SLE-HA-all/cha-ha-storage-protect.html
13. dev.to — "Revocation Under Partial Failure": epoch-based invalidation revokes all capabilities with `capability.epoch == current_epoch`; incrementing epoch invalidates descendants. https://dev.to/doomhammerhell/revocation-under-partial-failure-killing-financial-authority-in-a-distributed-system-2gcn
14. mick delaney — celld object storage coordination: epoch-prefixed storage paths isolate stale owners; self-fencing leases. https://mickdelaney.com/posts/celld-2-object-storage-coordination
15. Stackbook — "Fencing Tokens" pattern: check atomic with write; token comparison prevents older generation overwriting newer accepted generation. https://stackbook.io/patterns/fencing-tokens
16. bsm/redislock (Go) — `FenceKey` mints strictly increasing fencing token per acquisition; resource rejects older tokens. https://github.com/bsm/redislock
17. Microsoft Azure Cosmos DB — bounded staleness: K versions or T time staleness bound. https://learn.microsoft.com/en-us/azure/cosmos-db/consistency-levels
18. arXiv 2002.03068 — epoch-based memory reclamation adapted to distributed memory (EpochManager/LocalEpochManager). https://arxiv.org/pdf/2002.03068v3
19. RFC 8032 — EdDSA/ed25519 signature algorithm. https://www.rfc-editor.org/rfc/rfc8032
20. RFC 7539 / libsodium — ChaCha20-Poly1305 AEAD (96-bit nonce) and XChaCha20-Poly1305 (192-bit nonce). https://www.rfc-editor.org/rfc/rfc7539 ; https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
21. NIST FIPS 180-4 — SHA-256 Secure Hash Standard. https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
22. BLAKE3 — BLAKE3-team specification (256-bit output, SIMD, keyed hash, KDF, XOF). https://github.com/BLAKE3-team/BLAKE3
23. IDS prompts.txt entry 83 (verbatim) — C:/idfs/prompts.txt lines 1240-1254.
24. Existing IDS plans — C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md (HX-241 plan format reference).
25. Existing ids-000-aa skeleton — C:/idfs/ids-000-aa/ (types.rs, contracts/, invariants/, errors.rs, versioning.rs).

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 083 (ids-fencing-epoch).
