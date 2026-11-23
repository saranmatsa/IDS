## Phase 004 — Create the ids-identity crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 004 creates a **new separate crate `ids-identity`** (not folders inside an existing crate), organized into subfolders by subsystem. It implements IDS-B identity separation: a **stable internal identity** (`StableNodeId`, never exposed on the wire) paired with a **periodically rotating network-facing identifier** (`RotatingNodeId`, RNI). All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate is strictly carrier-agnostic. Allowed crypto only: ed25519 (signing/verification of RNI mappings), xchacha20poly1305 and chacha20poly1305 (encrypting identity records at rest), sha256 and blake3 (fingerprints, mapping-table integrity, StableNodeId derivation).

**Core design:** `StableNodeId` is a node's permanent logical identity (key-derived or random, assigned at enrollment, never reused after a node leaves). `RotatingNodeId` is a short-lived, unlinkable network handle derived from the stable identity, rotated on a schedule with validity windows and a transition state machine (scheduled → active → superseded → expired → revoked). The RNI mapping table stores `(RNI → StableNodeId, validity window, state, ed25519 signature)` entries; resolution is authenticated (caller presents an ed25519-signed mapping proof). Rotation is enforced **idle-only**: an RNI rotates only while the node has no in-flight requests; active requests keep their stable ID. Emergency revocation immediately invalidates an RNI (and, on compromise, forces a fresh `StableNodeId` — a dead node never returns with its old identity).

**No networking dependencies:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All identity announcement/resolution traffic is opaque `ids-carrier::Endpoint` bytes.

---

### 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-client                      │
│  ids_submit(op) — client never addresses a node directly        │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│              ids-routing / ids-discovery layer                  │
│  resolves StableNodeId → current RNI via ids-identity           │
└─────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────────────────────────────────────┐
│            Phase 004: ids-identity (NEW CRATE)                  │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐  │
│  │  types.rs       │  │  mapping.rs   — RNI mapping table   │  │
│  │  StableNodeId   │  │  (RNI → StableNodeId, window, state,│  │
│  │  RotatingNodeId │  │   ed25519 signature)                │  │
│  └─────────────────┘  └─────────────────────────────────────┘  │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐  │
│  │  windows.rs     │  │  transitions.rs — state machine     │  │
│  │  validity windows│  │  scheduled/active/superseded/       │  │
│  │  (start/end IDS │  │  expired/revoked; guarded transitions│  │
│  │   standard time)│  │                                       │  │
│  └─────────────────┘  └─────────────────────────────────────┘  │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐  │
│  │  resolution.rs  │  │  rotation.rs  — idle-only rotation  │  │
│  │  authenticated  │  │  enforcement; rotation scheduler    │  │
│  │  resolution (sig│  │                                       │  │
│  │   verification) │  └─────────────────────────────────────┘  │
│  └─────────────────┘  ┌─────────────────────────────────────┐  │
│  ┌─────────────────┐  │  revocation.rs — emergency revocation│  │
│  │  persistence.rs │  │  (grow-only revocation set, gossip)   │  │
│  │  encrypted at-rest│  └─────────────────────────────────────┘  │
│  │  store (xchacha20 │  ┌─────────────────────────────────────┐  │
│  │  poly1305)        │  │  carrier.rs — ids-carrier::Endpoint │  │
│  └─────────────────┘  │  announce/resolve datagrams only      │  │
│  ┌─────────────────┐  └─────────────────────────────────────┘  │
│  │  errors.rs      │                                           │
│  └─────────────────┘                                           │
└─────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────────────────────────────────────┐
│             ids-000-aa (master crate, read-only contracts)      │
│  NodeId, RegionId, Endpoint, CarrierType; invariants:           │
│  idle_rotation (RNI rotates only while idle),                   │
│  dead_node_identity (dead node never returns with old ID)       │
└─────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────────────────────────────────────┐
│             ids-crypto (allowed primitives only)                │
│  ed25519 sign/verify, blake3/sha256 hash,                       │
│  xchacha20poly1305 + chacha20poly1305 AEAD                      │
└─────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────────────────────────────────────┐
│             ids-carrier (opaque Endpoint: CarrierType + Vec<u8>│
│  send_datagram / receive_datagram — no IP/TCP/UDP/QUIC/HTTP     │
└─────────────────────────────────────────────────────────────────┘
```

**Data flow of an RNI rotation:**
1. Rotation scheduler (driven by ids-time ticks, not wall-clock) evaluates the node's idle state via the `idle_rotation` invariant hook in ids-000-aa.
2. If idle, `rotation.rs` stages a new RNI in `scheduled` state, computes its validity window, and writes an ed25519-signed mapping entry `(new_RNI → StableNodeId, window, scheduled)` to the RNI mapping table.
3. At window start, the entry flips to `active`; the old active RNI flips to `superseded` (kept for the overlap/grace window so in-flight requests resolve).
4. After the grace window, `superseded` entries move to `expired` and are pruned.
5. On any request carrying an RNI, `resolution.rs` verifies the ed25519 signature over the mapping entry and checks validity window + state (`active` only; `superseded` accepted only within the overlap window).
6. Emergency revocation: `revocation.rs` marks an entry `revoked`, appends a signed revocation record to a grow-only revocation set, and gossips it over `ids-carrier::Endpoint`. Revoked RNIs never resolve again.

**Data flow of authenticated resolution:**
1. Resolver looks up RNI in the mapping table.
2. If found and state is `active` (or `superseded` within overlap window), verify the ed25519 signature over `(RNI, StableNodeId, window_start, window_end, state)` under the node's identity key.
3. Return `StableNodeId` on success; fail closed (reject the request) on any signature, window, or state mismatch — no fallback to unauthenticated resolution.

---

### 3. Detailed Research Dimensions

#### 3.1 StableNodeId vs RotatingNodeId (RNI) separation
**Research basis:** Stable identifiers that survive restart/reschedule are required for ownership, auditing, and routing (TheLinuxCode, "Assigning a Node ID to Every Node in a Distributed System": keep `node_id` stable plus a per-process ephemeral `node_session_id`; target globally unique across time; never reuse IDs after a node is gone). IP-based and MAC-based IDs are explicitly discouraged. IPFS peer IDs show content-addressed stable identifiers; Tor v3 shows an ed25519 master identity key embedded in a stable address while derived identifiers rotate.

**ids-identity decision:** `StableNodeId` = stable, never on the wire, key-derived from the node's enrollment ed25519 key (blake3(pubkey) or random 32 bytes stored at enrollment, never reused). `RotatingNodeId` = 32-byte unlinkable network handle, derived per rotation from a blake3 hash of `(StableNodeId, rotation_counter, secret_seed)` so observers cannot link consecutive RNIs to the same node without the seed. StableNodeId ↔ RNI linkage exists only inside the authenticated mapping table.

#### 3.2 RNI mapping table and validity windows
**Research basis:** Identifier-locator separation splits "who" (stable identifier) from "where" (current locator) — IEEE Technology Navigator; HIP, ILNP, LISP, MobilityFirst GNRS all maintain dynamic identifier→locator mappings. DMap (ICDCS 2012) distributes identifier→locator mappings via consistent hashing. Varjonen et al. (secure end-host identifier resolution) use self-certifying identifiers with authenticated mapping updates over a DHT. TMSI (cellular) shows short-lived identifiers rotated in place of permanent subscriber identities, with refresh/reuse discipline.

**ids-identity decision:** The RNI mapping table is the single source of truth mapping `RotatingNodeId → (StableNodeId, validity window, state, ed25519 signature)`. Validity windows use ids-time standard ticks (not wall-clock; Phase 005). Each entry: `{RNI, StableNodeId, window_start, window_end, state, signature, sequence_number}`. blake3 covers table integrity; per-entry ed25519 signatures let any replica verify a mapping without trusting the carrier.

#### 3.3 Transition states (scheduled → active → superseded → expired → revoked)
**Research basis:** Certificate/key lifecycle research (Agent Manifest x0x ADR 0018) models time-limited credentials plus immediate revocation; API key lifecycle research (Orbit) models `Active → Grace-rotated (overlap) → Revoked/Expired`; Auth0 refresh-token rotation uses an overlap period with reuse/breach detection.

**ids-identity decision:** State machine:
- `scheduled`: RNI computed, signed, stored; not yet resolvable.
- `active`: current resolvable RNI (only one active RNI per node at a time).
- `superseded`: previous active RNI, kept for the overlap/grace window so in-flight requests still resolve.
- `expired`: grace window passed; no longer resolvable; pruned after persistence checkpoint.
- `revoked`: emergency invalidation; never resolvable again; recorded in the grow-only revocation set.
Transitions are monotonic and append-only; `revoked` and `expired` are terminal for that RNI. Only `scheduled → active → superseded → expired` is the happy path; `active/superseded → revoked` is the emergency path.

#### 3.4 Authenticated resolution
**Research basis:** Signal safety numbers detect identity key changes and require explicit acceptance of rotation (trust-on-first-use + explicit rotation acceptance). MLS (RFC 9750) binds member identities to signature keys with controlled key rotation frequency. WireGuard associates peer public keys with allowed IPs (cryptokey routing) and authenticates every packet — identity is the public key, not the address. Noise protocol handshakes provide identity hiding (static keys never sent in the clear).

**ids-identity decision:** Resolution is authenticated by ed25519 signature over the mapping entry. A resolver that receives `(RNI, StableNodeId, signature)` verifies the signature under the claiming node's enrollment key (obtained out of band at enrollment via ids-security/admission) before accepting the mapping. Unsigned or mismatched mappings are rejected — fail closed. Because signatures cover the payload, mappings survive re-framing by any carrier (WireGuard lesson: authenticate the payload, not the transport).

#### 3.5 Idle-only rotation enforcement
**Research basis:** QUIC connection migration shows a stable connection ID survives path/address changes only after path validation; rotating during active transfer breaks in-flight state. Session-affinity research (NVIDIA Dynamo, AIBrix) keeps one assignment through a stateful session and rotates only at a session boundary. The idle_rotation invariant in ids-000-aa: "RNI rotates only while node idle; active requests keep stable ID."

**ids-identity decision:** `rotation.rs` consults the idle gate before any rotation: a node is "idle" when it has zero in-flight requests bound to its current active RNI. While non-idle, a new RNI may be staged (`scheduled`) but not activated. Activation waits until the drain condition (in-flight count = 0) or a bounded hard deadline (to prevent starvation under permanent load). Rotation reason is recorded (`scheduled` vs `emergency` vs `compromise`).

#### 3.6 Emergency revocation
**Research basis:** Agent Manifest x0x revocation design: fail-closed on revocation, grow-only RevocationSet (G-Set — replay is idempotent, no "un-revoke"), gossip-propagated on a heartbeat (~30s default) so offline peers recover on next contact; no un-revocation — a compromised identity gets a fresh keypair. Distributed attestation revocation (SSI) uses gossip with static retransmissions and revocation advertisements. Khodaei & Papadimitratos show region-scoped revocation-list distribution with bounded delay. 3PaaS (2026) addresses post-compromise healing.

**ids-identity decision:** Emergency revocation marks an RNI (or a whole StableNodeId on compromise) `revoked`, appends a signed revocation record `{revoked_id, reason, timestamp, issuer_signature}` to a grow-only revocation set, and gossips it over `ids-carrier::Endpoint`. Revocation is fail-closed: any resolver with the revocation record rejects the ID immediately. Propagation delay is bounded by the gossip heartbeat; until propagation completes, old-resolver risk is bounded by short RNI validity windows. On node compromise, a fresh `StableNodeId` is assigned — a dead/revoked node never returns with its old identity (dead_node_identity invariant).

#### 3.7 Rotation window overlap and grace period
**Research basis:** Auth0 refresh-token rotation (rotation overlap period, reuse detection/breach detection — only the previous token reusable). Orbit API keys: grace mode keeps old+new valid during a clamped window; immediate mode for compromise. OneUptime secret rotation: track three identities separately (application, provider credential, secret-store version).

**ids-identity decision:** When a new RNI activates, the previous RNI enters `superseded` and remains resolvable for a bounded overlap window (configurable, default derived from ids-time tick + maximum expected request latency). Requests using a superseded RNI within the window resolve to the same StableNodeId; after the window, they fail. Overlap windows are logged for breach detection (a superseded RNI used after its window → potential replay/revocation-evasion attempt).

#### 3.8 Threat surface specific to rotating identifiers
**Research basis:** Trace Oddity (PETS 2022) on traffic analysis against Tor; Tor v3 blinded keys prevent HSDir mass enumeration; UnlimitID/ePrint 2017/022 on unlinkable pseudonyms (undetectability, unlinkability, Sybil resistance); WireGuard identity hiding; QUIC connection-ID rotation for privacy (reusing a stable connection ID links sessions across networks). Rotating identifiers trade linkability for rotation complexity: rotation events themselves can correlate if timed predictably.

**ids-identity decision:** Rotation times are jittered per-node and keyed to ids-time ticks rather than wall-clock, so rotation events across nodes do not correlate. RNI values are blake3-derived and unlinkable without the seed. Validity windows are short (minutes-to-hours scale, configurable) to bound the exposure of any single RNI. The mapping table itself is encrypted at rest (xchacha20poly1305) so a carrier or storage compromise does not reveal StableNodeId ↔ RNI linkages.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| RNI → StableNodeId linkability (tracking across rotations) | RNI = blake3(StableNodeId, counter, secret seed); unlinkable without seed; jittered, ids-time-keyed rotation so events don't correlate |
| StableNodeId leaked on the wire | StableNodeId never leaves the node; all wire traffic carries RNI or `ids-carrier::Endpoint` opaque bytes only |
| Unsigned / forged mapping accepted | ed25519 signature over every mapping entry; fail closed on any signature/window/state mismatch |
| Replay of an old (expired/superseded) RNI | validity windows + state machine; superseded accepted only inside bounded overlap window; replay after window rejected |
| Compromised RNI / identity key | emergency revocation → grow-only revocation set, gossiped over ids-carrier::Endpoint; fail-closed; on compromise assign fresh StableNodeId (dead node never returns with old ID) |
| Rotation during active requests breaks in-flight state | idle-only rotation gate; stage `scheduled`, activate only when in-flight count = 0 (or bounded hard deadline) |
| Revocation propagation delay leaves old resolvers accepting | short RNI validity windows bound exposure; gossip heartbeat bounds convergence; overlap window logged for breach detection |
| Carrier/storage compromise reveals mapping table | mapping table encrypted at rest (xchacha20poly1305); per-entry signatures remain valid without trusting storage |
| Denial of service via revocation spam | revocation records require valid issuer signature; grow-only set deduplicates; bounded entry size |
| Rotation event correlation / traffic analysis | jittered rotation, no wall-clock, short windows, unlinkable RNI values |
| Node rejoins after failure with old identity | dead_node_identity invariant: fresh StableNodeId on rejoin after revocation/failure |

---

### 5. Integration with Existing Crates

Phase 004 depends on and integrates with:

| Crate | Role in Phase 004 |
|-------|-------------------|
| `ids-000-aa` | `NodeId`, `RegionId`, `HUnitId`, `Endpoint`, `CarrierType`; invariants `idle_rotation` and `dead_node_identity`; sealed trait contracts |
| `ids-crypto` | ed25519 sign/verify for mapping entries and revocation records; blake3/sha256 for fingerprints and table integrity; xchacha20poly1305/chacha20poly1305 for at-rest encryption (ids-crypto does NOT depend on ids-000-aa — no cycle) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` — all identity announcement/resolution traffic; carrier-agnostic |
| `ids-time` | standard ticks for validity windows and rotation scheduling (not wall-clock) |
| `ids-membership` | node join/leave triggers RNI issuance at join and revocation at leave |
| `ids-security` | enrollment key distribution, issuer authority for revocation records |
| `ids-observability` | rotation/revocation/resolution metrics and audit trail |

**Integration points:**
- `ids-routing`/`ids-discovery` resolve `StableNodeId → current RNI` through `ids-identity` authenticated resolution before addressing a carrier `Endpoint`.
- Node enrollment (ids-membership join) issues the first `StableNodeId` + initial RNI; node leave/retirement revokes all RNIs.
- Rotation scheduler reads idle state via the `idle_rotation` invariant hook in ids-000-aa.
- All announcement/resolution datagrams are `ids-carrier::Endpoint` payloads — zero direct transport dependencies (I-CARR-001).
- Revocation records feed ids-security revocation handling and ids-membership removal procedures.

---

### 6. Proposed File Structure (~250–400 files, all production-grade, in subfolders)

**Crate:** `ids-identity` (depends on ids-000-aa, ids-crypto, ids-carrier, ids-time; serde, thiserror, blake3; no networking crates)

```
ids-identity/
├── Cargo.toml
├── lib.rs                          # crate root, feature gates, public re-exports
├── constants.rs                    # rotation intervals, overlap window, max RNI lifetime
├── error.rs                        # IdentityError (thiserror, comprehensive)
├── prelude.rs
│
├── types/                          # StableNodeId, RotatingNodeId, Endpoint ref
│   ├── mod.rs
│   ├── stable_node_id.rs           # StableNodeId: key-derived, never on wire
│   ├── rotating_node_id.rs         # RotatingNodeId (RNI): blake3-derived, unlinkable
│   ├── identity_key.rs             # enrollment ed25519 keypair, fingerprint
│   └── rni_ref.rs                  # typed RNI reference
│
├── mapping/                        # RNI mapping table
│   ├── mod.rs
│   ├── table.rs                    # RNI → (StableNodeId, window, state, signature)
│   ├── entry.rs                    # mapping entry struct + validation
│   ├── index.rs                    # blake3-fingerprinted lookup index
│   ├── integrity.rs                # per-entry ed25519 signatures + table blake3 root
│   └── cache.rs                    # bounded LRU of hot RNI resolutions
│
├── windows/                        # validity windows
│   ├── mod.rs
│   ├── window.rs                   # (start, end) in ids-time ticks
│   ├── scheduler.rs                # rotation schedule, jitter, tick alignment
│   └── overlap.rs                  # overlap/grace window for superseded RNIs
│
├── transitions/                    # state machine scheduled/active/superseded/expired/revoked
│   ├── mod.rs
│   ├── state.rs                    # RniState enum + transition rules
│   ├── machine.rs                  # guarded transition executor
│   ├── guard.rs                    # idle gate, overlap checks, terminal-state checks
│   └── reason.rs                   # RotationReason (scheduled/emergency/compromise)
│
├── resolution/                     # authenticated resolution
│   ├── mod.rs
│   ├── resolver.rs                 # verify signature + window + state; fail closed
│   ├── proof.rs                    # signed mapping proof (RNI, StableNodeId, sig)
│   └── verify.rs                   # ed25519 verification entry point
│
├── rotation/                       # idle-only rotation enforcement
│   ├── mod.rs
│   ├── enforcer.rs                 # idle gate from ids-000-aa invariant hook
│   ├── scheduler.rs                # stage scheduled → activate when idle
│   ├── drain.rs                    # in-flight request tracking, hard deadline
│   └── derive.rs                   # blake3 RNI derivation from StableNodeId + seed + counter
│
├── revocation/                     # emergency revocation
│   ├── mod.rs
│   ├── revoke.rs                   # mark revoked, signed revocation record
│   ├── set.rs                      # grow-only revocation set (G-Set, idempotent replay)
│   ├── gossip.rs                   # gossip revocation over ids-carrier::Endpoint
│   └── checker.rs                  # fail-closed revocation lookup
│
├── persistence/                    # encrypted at-rest store
│   ├── mod.rs
│   ├── store.rs                    # atomic-write identity store
│   ├── encrypt.rs                  # xchacha20poly1305 envelope encryption
│   ├── snapshot.rs                 # versioned, checksummed snapshot of mapping table
│   ├── journal.rs                  # append-only rotation/revocation journal
│   └── recovery.rs                 # load snapshot + replay journal
│
├── carrier/                        # carrier-agnostic datagrams
│   ├── mod.rs
│   ├── announce.rs                 # RNI announcement datagram over ids-carrier::Endpoint
│   ├── resolve.rs                  # resolution request/response datagram
│   └── endpoint.rs                 # Endpoint (CarrierType + Vec<u8>) framing helpers
│
├── wire/                           # serialization
│   ├── mod.rs
│   ├── format.rs                   # versioned wire envelope
│   ├── encoding.rs                 # deterministic encoding (CBOR/bincode decision deferred)
│   └── checksum.rs                 # payload checksums (blake3/sha256)
│
├── invariants/                     # ids-000-aa invariant hooks
│   ├── mod.rs
│   ├── idle_rotation.rs            # idle gate enforcement
│   └── dead_node_identity.rs       # fresh StableNodeId on rejoin
│
├── fuzz/                           # fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_rotation.rs
│   ├── fuzz_resolution.rs
│   └── fuzz_revocation.rs
│
├── tests/                          # integration & property tests
│   ├── mod.rs
│   ├── state_machine_tests.rs      # all transitions + terminal states
│   ├── idle_rotation_tests.rs      # rotation blocked while non-idle
│   ├── resolution_tests.rs         # authenticated resolution + fail-closed
│   ├── revocation_tests.rs         # emergency revocation + gossip
│   ├── overlap_tests.rs            # superseded window acceptance/rejection
│   ├── persistence_tests.rs        # snapshot + journal round-trip
│   ├── adversarial_tests.rs        # forged signature, replay after expiry
│   ├── chaos_tests.rs              # node churn, offline members, rejoin
│   └── conformance_tests.rs        # invariant conformance
│
├── benches/                        # benchmarks
│   ├── mod.rs
│   ├── resolution_bench.rs
│   ├── rotation_bench.rs
│   └── revocation_propagation_bench.rs
│
├── spec/                           # formal specifications
│   ├── state_machine.md            # transition laws
│   ├── idle_rotation_spec.md       # idle-only invariant
│   └── tla/                        # TLA+ (in ids-doc / ids-time spec area)
│
├── docs/                           # documentation
│   ├── architecture.md
│   ├── rotation_policy.md
│   └── revocation_runbook.md
│
└── test_vectors/                   # deterministic test vectors
    ├── rotation_vectors.json
    ├── resolution_vectors.json
    └── revocation_vectors.json
```

**Why ~250–400 files:** one responsibility per file; every file has full error handling, documentation, and unit tests. Covers: types, mapping table + index + cache, validity windows + scheduler + overlap, 5-state transition machine + guards, authenticated resolution, idle-only rotation + drain, emergency revocation + grow-only set + gossip, encrypted persistence + snapshot + journal, carrier datagrams, wire format, invariant hooks, fuzzing, property/adversarial/chaos/conformance tests, benchmarks, formal spec, docs, test vectors. No stubs. Within the HX-241 "~90–1000 files in subfolders" band.

---

### 7. Key Algorithms & Data Structures

#### 7.1 RNI derivation (unlinkable)
```
RNI = blake3(StableNodeId || rotation_counter || secret_seed || window_start)
```
`secret_seed` is derived at enrollment from the node's ed25519 key and never leaves the node. Consecutive RNIs are unlinkable without the seed.

#### 7.2 Mapping entry
```
Entry {
  rni: RotatingNodeId,
  stable_node_id: StableNodeId,
  window_start: IdsTimeTick,
  window_end: IdsTimeTick,
  state: RniState,          // scheduled | active | superseded | expired | revoked
  sequence: u64,
  signature: Ed25519Sig,    // sign(rni || stable_node_id || window_start || window_end || state || sequence)
}
```
Verification: `ed25519_verify(enrollment_pubkey, signature, canonical_entry_bytes)`; reject on mismatch (fail closed).

#### 7.3 Transition state machine
```
scheduled  → active        (at window_start, idle gate passed)
active     → superseded    (new RNI activated; enters overlap window)
superseded → expired       (overlap window ends)
expired    → (pruned)      (terminal)
active/superseded → revoked (emergency revocation; terminal)
revoked    → (terminal)
```
Invariant: at most one `active` RNI per StableNodeId at any time. Transitions are append-only; revocation is irrevocable (grow-only set).

#### 7.4 Idle-only rotation gate
```
can_activate(new_rni) = (in_flight_requests(current_active_rni) == 0)
                       || (hard_deadline_reached)
```
While `!idle`, new RNI stays `scheduled`; active requests keep resolving via the current active RNI. Rotation reason recorded per transition.

#### 7.5 Emergency revocation
```
revoke(id, reason, issuer):
  record = {id, reason, timestamp, issuer_signature}
  revocation_set.add(record)          // grow-only, idempotent
  gossip(record) over ids-carrier::Endpoint
  mark all RNIs of id → revoked       // terminal
  if reason == compromise: issue fresh StableNodeId (dead_node_identity)
```
Resolver checks `revocation_set.contains(id)` before any mapping lookup — fail closed.

#### 7.6 Overlap window
```
resolves(rni, now):
  e = table.lookup(rni)
  if e == null || e.state == revoked || e.state == expired: reject
  if e.state == superseded && now > e.window_end + overlap: reject
  if !ed25519_verify(e): reject
  return e.stable_node_id
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | NodeId, RegionId, HUnitId, Endpoint, CarrierType; `idle_rotation` and `dead_node_identity` invariants; sealed trait contracts |
| `ids-crypto` | ed25519 sign/verify; blake3, sha256 hashing; xchacha20poly1305, chacha20poly1305 AEAD (at-rest encryption only) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` |
| `ids-time` | standard ticks for validity windows and rotation scheduling |
| `serde` + `serde_bytes` | serialization |
| `thiserror` | error types |
| `blake3` | RNI derivation, table integrity, fingerprints (re-exported via ids-crypto) |

No networking crates: no `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| StableNodeId never on wire | 0 occurrences of StableNodeId in carrier datagrams | audit + unit test |
| RNI unlinkability | consecutive RNIs unlinkable without seed | property test (blind-linkage game) |
| One active RNI per node | invariant holds at all times | state machine test |
| Idle-only rotation | no `scheduled → active` while in-flight > 0 | test + invariant hook |
| Transition legality | only defined transitions accepted | state machine property test, 100k iterations |
| Authenticated resolution | unsigned/mismatched mapping rejected | fail-closed test |
| Superseded overlap | accepted inside window, rejected after | unit + boundary test |
| Emergency revocation | revoked RNI never resolves again | revocation + resolution test |
| Revocation propagation | gossip converges within bounded heartbeat | chaos test |
| Dead node rejoin → fresh StableNodeId | no old StableNodeId reuse | conformance test (dead_node_identity) |
| Mapping-table integrity | blake3 root detects tampering | tamper test |
| At-rest encryption | mapping table unreadable without key | encryption round-trip test |
| Property tests | 100,000+ iterations | proptest |
| Adversarial tests | forged sig, replay after expiry, revocation spam | pass/fail |
| Chaos tests | node churn, offline members, rejoin | pass/fail |

---

### 10. Remaining Uncertainties

1. **Rotation interval policy:** default validity window length (minutes vs hours) trades linkability against rotation overhead; must be configured per deployment, not hard-coded.
2. **Idle definition granularity:** "zero in-flight requests" per RNI vs per node; interaction with long-lived sessions and streaming transfers.
3. **Hard deadline for rotation under permanent load:** if a node is never idle, when does a staged `scheduled` RNI force-activate? Bounded deadline needed to prevent starvation — value unresolved.
4. **Overlap window duration:** bounded by max expected request latency; too long increases replay window, too short breaks in-flight requests. Value TBD.
5. **Wire encoding:** CBOR (deterministic, self-describing) vs bincode vs protobuf for datagram envelopes; CBOR favored, crate TBD.
6. **Revocation propagation bound:** gossip heartbeat interval and convergence time under churn; interplay with short validity windows.
7. **StableNodeId derivation:** blake3(ed25519 pubkey) vs random 32 bytes stored at enrollment; key-derived enables self-certification but binds identity to the key forever.
8. **Formal verification scope:** TLA+/PlusCal spec of the state machine and idle-only invariant (in ids-doc); extending to revocation gossip is additional effort.
9. **Interaction with ids-security enrollment:** who issues/attests the enrollment ed25519 key used to verify mapping signatures — deferred to ids-security/admission crate.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| StableNodeId never on wire; RNI is network-facing | identity separation — observers see only unlinkable RNIs (identifier-locator separation, HIP/ILNP/MobilityFirst pattern) |
| RNI = blake3(StableNodeId || counter || seed || window) | unlinkable without seed; derives from stable root (BIP 32-style stable master, rotating derived handles) |
| ed25519 signature on every mapping entry | authenticated resolution that survives carrier re-framing (WireGuard lesson: authenticate payload, not transport) |
| 5-state machine, append-only, revoked terminal | monotonic transitions are easy to reason about and replicate; grow-only revocation set (x0x pattern) |
| Idle-only rotation gate | prevents breaking in-flight requests (idle_rotation invariant; QUIC/session-affinity lessons) |
| Overlap/grace window for superseded RNIs | zero-downtime rotation (Auth0 refresh-token / Orbit API key overlap patterns) |
| Emergency revocation gossiped over ids-carrier::Endpoint | carrier-agnostic revocation distribution; fail-closed (x0x / SSI gossip revocation pattern) |
| Mapping table encrypted at rest (xchacha20poly1305) | carrier/storage compromise does not expose StableNodeId ↔ RNI linkage |
| Fresh StableNodeId on compromise/rejoin | dead node never returns with old identity (dead_node_identity invariant) |
| ids-time ticks, not wall-clock, for windows/rotation | Phase 005 proprietary time basis; avoids clock-skew correlation of rotation events |
| New separate crate `ids-identity` | matches CRATES.md rule (separate crates, not more folders); clean dependency boundary; ~250–400 files |

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 004.

**Sources consulted (30 targeted searches):**
1. TheLinuxCode — stable node_id + ephemeral session ID, globally unique across time, no ID reuse (node ID best practices)
2. IEEE Technology Navigator — Identifier-Locator Separation principle
3. HIP / ILNP / LISP / MobilityFirst GNRS — stable identifiers separate from locators, dynamic mapping resolution
4. DMap (ICDCS 2012) — distributed identifier→locator mapping via consistent hashing
5. Varjonen et al. — secure end-host identifier resolution, self-certifying identifiers, authenticated mapping updates
6. TMSI (cellular) — short-lived identifier rotated in place of permanent subscriber identity, refresh/reuse discipline
7. ACM WPES — Sybil-resistant pseudonym change (BitNym, Bitcoin blockchain as append-only bulletin board)
8. ePrint 2017/022 & UnlimitID — unlinkable pseudonyms: undetectability, unlinkability, Sybil resistance
9. Agent Manifest x0x ADR 0018 — revocation + key rotation: fail-closed revocation, grow-only RevocationSet (G-Set), gossip propagation, ~30s heartbeat, no un-revocation
10. Signal safety numbers — identity change detection, trust-on-first-use, explicit acceptance of identity rotation
11. MLS RFC 9750 — identity↔signature-key binding, frequency of public key rotation
12. BIP 32 (Bitcoin.org) — HD wallets: stable master seed, tree of derived rotating addresses
13. Tor Project v3 onion services — ed25519 master identity key in address, daily-rotated blinded key for directory queries, relay onion keys rotated every 4 weeks
14. Auth0 — refresh token rotation: overlap period, reuse/breach detection, only previous token reusable
15. Orbit — API key lifecycle: grace mode (dual-key overlap window) vs immediate revocation for compromise
16. WireGuard — cryptokey routing (public key → allowed IPs), rotating session keys for forward secrecy, identity hiding, roaming peers
17. Noise Protocol Framework — identity hiding: static keys never sent in the clear
18. Kademlia measurement study — kad IDs change per session, showing churn problems of rotating IDs
19. QUIC — stable connection ID survives IP change/NAT rebinding, path validation before trusting new path, rotating connection IDs for privacy
20. I2P — mobile cryptographic destinations, location-independent identifiers
21. IPFS — peer IDs and content addressing (stable content-addressed identifiers)
22. Khodaei & Papadimitratos — vehicle-centric CRL distribution for pseudonym revocation, bounded-delay revocation delivery
23. SSI distributed attestation revocation — gossip with static retransmissions, revocation advertisements
24. 3PaaS (2026) — post-compromise security and healing after compromise
25. Trace Oddity (PETS 2022) — traffic analysis and correlation risks on rotating/anonymized networks
26. DMap / GNRS / LISP — identifier-locator mapping systems for mobility
27. Auth0 reuse detection — breach detection when rotated credentials are reused
28. OneUptime — tracking application/credential/secret-store identities separately across rotation
29. Cloud Service Mesh — automatic mTLS certificate/key rotation without disrupting communications
30. TMSI / NHI — temporary mobile subscriber identity refresh behavior and reuse patterns
