## Phase 056 — Create the ids-operation-ref crate: Nine-character alphanumeric operation IDs separate from ordering positions.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 835-849)

```
56. Research only do not read plan.md **[Phase 056 — Create the ids-operation-ref crate: Nine-character alphanumeric operation IDs separate from ordering positions.]**

Crate: ids-operation-ref
Purpose: Nine-character alphanumeric operation IDs separate from ordering positions.
Contents: 36^9 namespace, reference generation, uniqueness guarantees, collision detection, idempotency binding, retry recognition, commit-status lookup, synchronization tracking, recovery tracking, audit records.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 056 — Create the ids-operation-ref crate: Nine-character alphanumeric operation IDs separate from ordering positions.]**

Create plan-056.md in C:/idfs/plans/ for the ids-operation-ref crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 056 creates `ids-operation-ref`, the IDS crate that owns **operation references (OpRef)**: nine-character alphanumeric operation IDs that are deliberately **separate from ordering positions**. An OpRef is a flat, opaque reference to a single logical operation (a message send, a state mutation, a resource creation, a write). It carries no ordering information — ordering is a separate concern owned by dataset-scoped ordering domains (Phase 055 `ids-ordering-domain`) via the canonical `OrderingPos` type in `ids-000-aa`. `OpRef` and `OrderingPos` are distinct canonical types in `ids-000-aa::types.rs` and must never be conflated or packed into one another (contrast Stellar SEP-0035, which packs ledger sequence + transaction order + operation index into one 64-bit ID — that conflation is exactly what this crate avoids).

`ids-operation-ref` provides: a 36^9 ≈ 1.016×10^14 namespace (alphabet 0-9A-Z, case-insensitive, nine positions); reference generation with uniqueness guarantees and collision detection; idempotency binding (an OpRef bound to normalized operation arguments); retry recognition (a repeated OpRef returns the stored outcome); commit-status lookup (Pending → Committed → Failed/Rejected); synchronization tracking over carrier-agnostic endpoints; recovery tracking for interrupted operations; and append-only audit records.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-operation-ref` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-operation-ref` crate does NOT yet exist at `C:/idfs/ids-operation-ref/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Per CRATES.md and plan-013.md, `ids-operation-ref` (Phase 056) generates OpRef; `ids-write` binds/validates it. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned distributed-identifier design (random strings vs integers vs UUIDs vs KSUIDs vs URNs; opaque vs transparent; prefix disambiguation), unique-ID generation (DB counter, random+birthday, coordinator blocks, timestamp+node+counter composition), UUIDv4 collision probability (n²/2^123), UUIDv7 (RFC 9562), ULID (26-char Crockford Base32, sortable, monotonic option), short-ID Rust crates (block-id invertible integer→alphanumeric, terseid SHA256→base36→truncate with birthday-bound collision avoidance, nexus-id Base62Id/Base36Id), the IETF Idempotency-Key header pattern, Stripe and AWS idempotent-API designs (client token, ACID token+mutation recording, 24h pruning, mismatched-params rejection), Azure Idempotent Consumer (stable dedup key survives redelivery; never key on transport IDs), the "every retry needs the same operation ID" Intend/Attempt/Recognise/Resolve contract, ICP idempotency (dedup window + created_at_time validity), etcd revision as cluster-wide logical clock (total order, separate from any opaque reference), Stellar SEP-0035 (conflated op-ID-plus-ordering — the anti-pattern to avoid), CRDT op-based references (globally unique element identifiers, dots `(replica, counter)`, causal delivery), Automerge/JSON-CRDT operation IDs (Lamport `N x ReplicaID`, dedup by "already applied" check, buffer until dependencies satisfied), deterministic replay-safe operation IDs (blake2b over `{counter}` / `{parent_id}-{counter}`, monotonic counter, determinism invariant), YouTube-style 64/128-bit random IDs with no embedded metadata and cancelled IDs never reused, birthday-bound collision math (√N rule; p ≈ 1 − exp(−k(k−1)/(2·Nᴸ))), Crockford Base32 (excludes I/L/O/U), append-only audit logs and event sourcing as system of record, MongoDB resume tokens (oplog position, resume after crash), and deterministic idempotency keys derived from operation inputs (hash workflow+task+body, conditional writes, TTL expiration).

---

### 2. Architecture Overview

`ids-operation-ref` sits above `ids-000-aa` (canonical `OpRef`/`OrderingPos` types, sealed contracts, invariants, errors) and `ids-carrier` (opaque `Endpoint` datagram transport), with optional `ids-crypto` primitives (sha256/blake3 fingerprints, ed25519 signatures). Each concern — OpRef minting, uniqueness registration, collision detection, idempotency binding, retry recognition, commit-status lookup, synchronization tracking, recovery tracking, audit — is an independent module. The crate is a reference-generation and registry layer: it does not store application payloads itself; it mints OpRefs, registers them, binds them to operation arguments, and gates retry/commit handling before results flow back through `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (OpRef consumers)               │
│  ids-write, ids-store, ids-replication — mint/consume OpRefs per operation  │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 056: ids-operation-ref (THIS CRATE — OpRef registry)      │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ OpRef minting  │ │ Uniqueness     │ │ Collision      │ │ Idempotency  │ │ │
│  │ (sequential /  │ │ registration   │ │ detection      │ │ binding      │ │ │
│  │  random / hash-│ │ (OpRef → record│ │ (birthday-bound│ │ (OpRef ↔ arg │ │ │
│  │  based)        │ │  existence)    │  check + regen)  │  fingerprint)│ │ │
│  └────────────────┘ └────────────────┘ └────────────────┘ └──────────────┘ │ │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ Retry          │ │ Commit-status  │ │ Synchronization│ │ Recovery     │ │ │
│  │ recognition    │ │ lookup         │ │ tracking       │ │ tracking     │ │ │
│  │ (in-progress/  │ │ (Pending→      │ │ (which         │ │ (resume      │ │ │
│  │  committed/    │ │  Committed/    ││  endpoints saw │ │  tokens,     │ │ │
│  │  failed states)│ │  Failed/Rejected│ │  OpRef; via    │ │  checkpoints)│ │ │
│  └────────────────┘ └────────────────┘ │ Endpoint only) │ └──────────────┘ │ │
│  ┌────────────────┐ ┌────────────────┐ │                │                  │ │ │
│  │ Audit records  │ │ types/contracts│ └────────────────┘                  │ │ │
│  │ (append-only,  │ │ + invariants   │                                     │ │ │
│  │  chained)      │                  │                                     │ │ │
│  └────────────────┘ └────────────────┘                                     │ │ │
└──────────────────────────────────────────────────────────────┬───────────────┘
                                                                │ opaque Vec<u8> datagrams
┌─────────────────────────────────────────────────────────────▼──────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType ≤1024B addr, Vec<u8>}, payload: &[u8]) │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS   │
└────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of OpRef minting + idempotent execution:** client submits an operation → `ids-operation-ref` mints an OpRef (9-char, 0-9A-Z), checks uniqueness against the registration set (collision detection), binds the OpRef to a blake3/sha256 fingerprint of normalized operation arguments, records state `Pending`, returns the OpRef as an opaque datagram to `ids-carrier::Endpoint`. On execution completion, commit-status transitions to `Committed(outcome)` / `Failed` / `Rejected`. A retry carrying the same OpRef is recognized: `InProgress` → report pending; `Committed` → replay stored outcome; same OpRef with a different argument fingerprint → reject as conflict. Audit records append every lifecycle event.

---

### 3. Detailed Research Dimensions

#### 3.1 Nine-character alphanumeric ID space (36^9 namespace)

Nine positions over a 36-symbol alphabet (0-9, A-Z, case-insensitive) gives `36^9 = 101,559,956,668,416 ≈ 1.016×10^14` possible OpRefs — roughly 101.6 trillion. Per-character entropy is `log2(36) ≈ 5.17` bits, so a full 9-char OpRef holds ≈ 46.5 bits of space. This is large enough for modest operation volumes but small enough that **birthday-bound collisions are a real design constraint**, not a theoretical nicety.

Birthard-bound math (p ≈ 1 − exp(−k(k−1)/(2·N)) with N = 36^9):
- 50% collision probability at `k ≈ √N ≈ 10.1M` OpRefs.
- 1% collision probability at `k ≈ √(2·N·0.01) ≈ 1.43M` OpRefs.
- Exhaustion (100% collision) only after all N values are issued.

So random 9-char generation is safe only up to roughly the low millions of operations unless collision detection + regeneration is applied. For higher volumes the crate must either (a) use sequential/counter-based minting (collision-free by construction) or (b) cap random generation and fall back to sequential allocation once near the birthday bound. This is why **collision detection is a first-class content item** of the crate. Confusables (O/0, I/1, L/1) may be dropped from the alphabet (→ 32 symbols, Crockford-style) at a capacity cost (32^9 ≈ 3.6×10^13, ~2.8× smaller) for human-transcription friendliness; that is an unresolved trade-off (see Section 10).

References: distributed-identifier design and opaque-vs-transparent IDs [1]; unique-ID generation schemes (counter, random+birthday, coordinator blocks, timestamp+node+counter) [2]; short-ID Rust crates block-id (invertible integer→alphanumeric, successor to tiny_id, notes distributed state is hard so invertible mapping is preferred) [5] and terseid (SHA256→base36→truncate with birthday-problem collision avoidance and 4-tier fallback) [7]; ULID Crockford Base32 alphabet excluding I/L/O/U [8]; YouTube 64/128-bit random IDs with no embedded metadata [21]; birthday-bound engineering [23]; HardGuard25 birthday formula p = 1 − exp(−k(k−1)/(2·Nᴸ)) [24].

#### 3.2 OpRef separate from OrderingPos

`ids-000-aa` defines both `OpRef` (the nine-character alphanumeric operation reference) and `OrderingPos` (the monotonic logical position within a dataset-scoped ordering domain, Phase 055). They are distinct canonical types and must remain so:

- **OpRef** is an opaque, flat name for *which* operation occurred — no structure, no order information, no timestamp, no replica component. It is the identity clients use to retry, audit, and reference an operation.
- **OrderingPos** is a monotonic position used only for *where* the operation sits in an ordering domain's commit sequence. It is not a client-facing reference and must never be derived from or embedded in an OpRef.

This separation mirrors findings that conflating an identifier with its ordering position creates fragility: Stellar SEP-0035 packs ledger sequence + transaction order + operation index into one 64-bit "operation ID" [17], which couples the reference to a specific total order and makes re-ordering, re-issuance, and cross-domain reference awkward. etcd's revision [16] is similarly a cluster-wide logical clock (total order) used for MVCC, not an opaque client reference. UUIDv7/ULID embed timestamps for sortability [3][8] — also a coupling this crate rejects. An OpRef must be issuable before any ordering position is known, and two operations with adjacent OpRefs must not imply adjacent ordering positions.

References: opaque-vs-transparent and prefix disambiguation [1]; Stellar SEP-0035 conflated op-ID-plus-ordering [17]; etcd revision as logical clock [16]; UUIDv7 RFC 9562 / ULID sortable timestamps [3][8]; CRDOT globally unique element identifiers separate from positions [18]; Automerge operation IDs separate from object key/position [19].

#### 3.3 Reference generation strategies

Three generation strategies were evaluated:

1. **Sequential counter → base-36, zero-padded to 9 chars.** Collision-free by construction (monotonic counter, one issuer or sharded counter range). Simple and capacity-efficient, but reveals issuance rate and approximate order — acceptable internally but potentially undesirable for public-facing OpRefs. block-id's core lesson applies here: tight sequential state is hard to synchronize across nodes, so an invertible integer→alphanumeric mapping lets any sequential source (e.g., a per-domain counter in ids-ordering-domain) produce short opaque strings [5].

2. **Random 9-char + collision check + regenerate.** Opaque and order-revealing in neither direction, but bounded by the birthday limit (~10.1M for 50% collision). On a generated collision, regenerate with a fresh seed and re-check; track recent collisions to avoid livelock. terseid's 4-tier fallback (nonce escalation → length extension → long fallback → desperate fallback) is the reference pattern for collision recovery [7].

3. **Deterministic hash-based OpRef:** `blake3(normalized operation args || domain separator)` truncated/encoded to 9 chars base-36. Replay-safe: the same logical operation always mints the same OpRef, which makes dedup intrinsic (durable_lambda_core's OperationIdGenerator uses blake2b over `{counter}` or `{parent_id}-{counter}` with a monotonic counter and a determinism invariant for replay engines [20]). Collision risk is the same birthday bound, but collisions now indicate two *different* operations hashing to the same OpRef — detectable and rejectable via the uniqueness registry.

**Recommendation for Phase 056:** support all three behind a `GenerationStrategy` trait, with sequential counter as default (collision-free, fits 36^9) and random/deterministic as feature-gated alternatives. The default avoids collision complexity entirely for the common case; random is offered for callers that need opacity.

References: block-id [5]; nexus-id Base62Id 11-char / Base36Id 13-char [6]; terseid [7]; ULID [8]; durable_lambda_core deterministic OpId [20]; UUIDv4 collision n²/2^123 and monotonic mode [3].

#### 3.4 Uniqueness guarantees

Every minted OpRef is registered in an in-memory (optionally durable) uniqueness set keyed by OpRef before the operation is accepted. The uniqueness guarantee is: **no two accepted operations share an OpRef**. Issuance performs an existence check (`contains(OpRef)`) then registers atomically; a pre-existing OpRef triggers regeneration (random strategy) or counter advance (sequential strategy). Capacity is bounded by N = 36^9; near-exhaustion (configurable threshold, e.g. 80%) must raise a reusable event so callers rotate to a new domain/namespace rather than silently colliding.

Kubernetes' distinction is instructive: a `UID` is opaque, system-generated, unique across time and space; a `Name` is unique only within a scope at a time and enables idempotent creation [4]. OpRef plays the UID role (opaque, global, stable); it is deliberately *not* scoped or human-meaningful.

References: Kubernetes UID vs Name and idempotent creation [4]; block-id distributed-state difficulty [5]; CRDT dots `(replica, counter)` for uniqueness without central coordination [18].

#### 3.5 Collision detection

Collision detection runs on every OpRef issue:
1. Generate candidate OpRef (per strategy).
2. Check `registry.contains(candidate)`.
3. If present → collision: regenerate (random) / advance counter (sequential) / reject (deterministic, two distinct ops hashed alike).
4. If absent → register atomically, return OpRef.

The registry must be the single source of truth; no cache may bypass it. Collision rate and regeneration count are observable metrics. The birthday bound (Section 3.1) sets expectations: at 1M random OpRefs, collision probability ≈ 1%; at 5M ≈ 22%; regeneration handles these gracefully until the bound is approached, at which point the crate must switch strategies or exhaust the namespace.

References: birthday-bound [23][24]; terseid collision-avoidance fallback [7]; short-UID StackOverflow birthday analysis [25].

#### 3.6 Idempotency binding

Idempotency binding ties an OpRef to the *arguments* of the operation it names, so a retry is recognized as "the same logical operation" rather than "a new operation that happens to have the same ID."

Binding mechanics:
- Caller normalizes and validates operation arguments, then computes a fingerprint: `blake3` or `sha256` over canonical encoding of (operation type, target, normalized args).
- `ids-operation-ref` records `OpRef → {fingerprint, commit-status, outcome, created_at, attempts}`.
- On a repeat request with the same OpRef:
  - Same fingerprint → recognized retry: return stored outcome (or report InProgress).
  - Different fingerprint → **conflict**: reject with "same OpRef, different arguments" (never silently execute different work under one ID).
- The binding must be recorded atomically with the operation's first effect (ACID: token + mutation recorded together, per AWS idempotent-API guidance [11]).

This is the Stripe Idempotency-Key pattern: same key + same request → same result; same key + different payload → 422 conflict; keys pruned after a retention window (Stripe ≥24h) [10]. Azure Idempotent Consumer adds: the dedup key must be stable across redelivery and must not be a transport-level ID regenerated on retry [3]. ICP idempotency adds a validity window (`created_at_time`) so a dedup window's expiry cannot let a stale retry succeed [14].

References: IETF Idempotency-Key header (fingerprint option, 400/422) [9]; Stripe idempotency [10]; AWS idempotent APIs (ClientToken, ACID recording) [11]; On Idempotency Keys [12]; Azure Idempotent Consumer [3]; ICP idempotency window [14]; "every retry needs the same operation ID" Intend/Attempt/Recognise/Resolve [14b]; deterministic idempotency keys from operation inputs [28]; Microsoft agent idempotent tool actions [29].

#### 3.7 Retry recognition

Retry recognition answers "have we already seen this OpRef?" using the registry:
- `InProgress` → the first attempt is still executing; report pending to the caller, do not start a second execution (prevents duplicate side effects under concurrent retries — two workers must not both claim the same absent OpRef; conditional-write semantics apply [11]).
- `Committed(outcome)` → return the stored outcome; no re-execution.
- `Failed` / `Rejected` → terminal; caller may issue a *new* OpRef for a new attempt.
- Unknown OpRef → treat as new; mint/register per Section 3.3.

Retries must carry the *same* OpRef and *same* arguments across every retry path (transport retry, model-selected retry, task resumption after restart). Generating a fresh key per attempt defeats the contract [14b]. Retention of in-progress/committed records must cover the longest retry + queue + manual-recovery window; expired records are pruned per a configurable TTL (Stripe's 24h window; ICP's TX_WINDOW) [10][14].

References: Inference Institute Intend/Attempt/Recognise/Resolve [14b]; AWS idempotent APIs [11]; Azure Idempotent Consumer [3]; ICP dedup window [14]; deterministic key derivation [28]; Microsoft agent patterns [29].

#### 3.8 Commit-status lookup

Commit-status is a small terminal-state FSM per OpRef:

```
Pending --(execute success)--> Committed(outcome)
Pending --(execute failure)--> Failed
Pending --(reject/conflict)--> Rejected
Committed / Failed / Rejected  (terminal; transitions out are forbidden)
```

Lookup by OpRef returns the current status and, if Committed, the stored outcome. This is the commit-status lookup content item. Status transitions are durable (written before acknowledging completion) so a crash mid-execution leaves `InProgress` or `Pending` recoverable (Section 3.10). etcd's revision demonstrates how a monotonic commit position tracks state changes separately from any opaque reference — here, OrderingPos tracks commit order while OpRef tracks identity [16].

References: etcd revision/MVCC [16]; CRDT op dedup "already applied" check [19]; commit state machine concepts (Phase 057 ids-commit-state) [CRATES.md line 392].

#### 3.9 Synchronization tracking (carrier-agnostic)

Synchronization tracking records **which endpoints have observed which OpRefs**, so replication/reconciliation can ask "has endpoint E seen OpRef O?" without knowing how E is reached. Tracking is strictly over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`): each observed OpRef is logged against the sender/recipient Endpoint. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh is used or referenced; the crate has no concept of addresses beyond `CarrierType`. This preserves the carrier-agnostic stance enforced across CRATES.md and plan-002 (ids-carrier) [CRATES.md lines 9-11].

References: ids-carrier Endpoint (CarrierType + Vec<u8>) [CRATES.md lines 11, 62]; carrier-agnostic rule [CRATES.md lines 9-11].

#### 3.10 Recovery tracking

Recovery tracking handles interrupted operations: a crash between OpRef minting and commit leaves the operation's outcome unknown. On restart, `ids-operation-ref` reconciles via:
- Resume tokens / checkpoints recording per-OpRef progress (minted → bound → executing → committed).
- Monotonic `IdsTime` deadlines (from `ids-time`, following plan-050's pattern) for idle/absolute recovery windows.
- Pending/InProgress entries on restart are reconciled: retry from checkpoint or mark Failed, per the operation's resumability contract.
- MongoDB resume tokens (oplog position, resume after crash without missing events) are the reference model [27]; Resonate's Recovery Protocol (lease + heartbeat, fail-stop process model) is a related pattern [15].

Recovery does not require knowing the ordering position; it operates purely on OpRef identity, keeping recovery decoupled from ordering (Section 3.2).

References: MongoDB resume tokens [27]; Resonate Recovery Protocol [15]; ids-time IdsTime pattern (plan-050) [plan-050 line 30-36].

#### 3.11 Audit records

Every OpRef lifecycle event is appended to an immutable audit log: minted, bound (fingerprint), retry-recognized, committed (outcome), failed, rejected, recovered, pruned. Audit records are append-only (event sourcing: the append-only store is the system of record, improving auditability [26]), optionally chained/signed with ed25519 so later records cannot be rewritten undetected, and retained per a configurable policy (matching the idempotency retention window). Audit records are written through `ids-carrier::Endpoint` if shipped off-node, never via a direct transport.

References: Event Sourcing append-only store [26]; audit-log design (transactional outbox, idempotent consumers, protected archive) [22]; ed25519 is the only allowed signature primitive [prompt line 846].

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| OpRef collision (two ops share one ID) | Uniqueness registry + existence check before issue; collision detection + regeneration; near-exhaustion threshold event (80% of 36^9) to rotate namespace ||
|| Birthday-bound exhaustion of random generation | Cap random issuance; fall back to sequential counter; switch GenerationStrategy before approaching √N ≈ 10.1M ||
|| OpRef reused with different arguments (conflict) | Idempotency binding: same OpRef + different blake3/sha256 fingerprint → 422-style conflict reject; never silently execute different work ||
|| Duplicate side effect from concurrent retries | Atomic claim of OpRef before execution (conditional-write semantics); InProgress state blocks second execution ||
|| Stale retry after dedup window expiry | Configurable retention TTL (e.g., 24h Stripe-style / ICP TX_WINDOW); created_at_time validity bound ||
|| Lost outcome (crash mid-execution) | Durable commit-status FSM; Pending/InProgress reconciled on restart via resume tokens/checkpoints ||
|| Carrier assumptions leaking in | No std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/wireless deps; all traffic via ids-carrier::Endpoint only ||
|| Non-allowed crypto | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (via ids-crypto); sha256/blake3 for fingerprints, ed25519 optional for audit signing ||
|| Audit log tampering | Append-only, optionally ed25519-chained, immutable retention policy ||
|| Namespace exhaustion (all 36^9 issued) | Rotation event at threshold; new domain/namespace; never reuse cancelled OpRefs (YouTube pattern: cancelled IDs never recycled) ||

---

### 5. Integration with Existing Crates

Phase 056 depends on and integrates with:

|| Crate | Role in Phase 056 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Canonical types `OpRef`, `OrderingPos`, `NodeId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`; sealed trait contracts; invariants; error types (master crate — never redefined here) ||
|| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); sole traffic channel; send/receive datagrams only ||
|| `ids-crypto` | Allowed primitives: sha256/blake3 (argument fingerprints), ed25519 (optional audit signing); `ids-operation-ref` consumes primitive IDs only ||
|| `ids-time` | Monotonic `IdsTime` for deadlines/retention windows (pattern from plan-050 ids-session) ||
|| `ids-ordering-domain` (Phase 055) | Dataset-scoped ordering positions (`OrderingPos`) — kept strictly separate from `OpRef` ||
|| `ids-write` (Phase after 056) | Binds/validates generated OpRefs; `ids-operation-ref` generates; `ids-write` binds/validates (per CRATES.md/plan-013) ||

**Integration points:**
- `ids-operation-ref` **generates** OpRef; `ids-write` **binds/validates** OpRef to operations (allocator visibility split per plan-013.md).
- OpRefs and outcomes flow as opaque `Vec<u8>` datagrams over `ids-carrier::Endpoint`; the crate never opens a socket or imports a carrier-specific type.
- Commit-status results feed `ids-commit-state` (Phase 057) write state machine (accepted → ordered → executing → committed …).
- Recovery/checkpoint data may be persisted via the store contract from `ids-000-aa`, but no storage backend is mandated by this crate.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS or any wireless carrier — preserves I-CARR-001 carrier-agnostic stance.

---

### 6. Proposed File Structure (~110–140 files, all in subfolders)

**Crate:** `ids-operation-ref` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-time`; plus `serde`, `thiserror`, `blake3`, `sha2`, `proptest`/`bolero`, `criterion`)

```
ids-operation-ref/
├── Cargo.toml                          # Crate manifest (ids-000-aa, ids-carrier, ids-crypto, ids-time; no net crates)
├── lib.rs                              # Crate root, feature gates, public re-exports
├── constants.rs                        # Namespace constants (36^9, charset, length=9, retention TTLs)
├── error.rs                            # OpRefError (thiserror): collision, conflict, duplicate, unknown, exhaustion
├── features.rs                         # Feature flags (deterministic, random, durable-audit, fuzzing)
│
├── id/                                 # OpRef type and namespace
│   ├── mod.rs
│   ├── opref.rs                        # OpRef: 9-char [0-9A-Z], opaque, validated, Display/FromStr
│   ├── namespace.rs                    # 36^9 size, charset, case-folding, confusables option
│   ├── validation.rs                   # OpRef format validation (length, charset, position)
│   └── ordering_pos.rs                 # Reference to ids-000-aa::OrderingPos (kept separate; no packing)
│
├── generation/                         # OpRef minting strategies
│   ├── mod.rs
│   ├── strategy.rs                     # GenerationStrategy trait (mint -> Result<OpRef>)
│   ├── sequential.rs                   # Monotonic counter → base-36, zero-padded (collision-free)
│   ├── random.rs                       # Random 9-char + collision check + regenerate
│   ├── deterministic.rs                # blake3(args) → base-36 truncate (replay-safe)
│   └── counter.rs                      # Counter source abstraction (per-domain, shardable)
│
├── uniqueness/                         # Uniqueness guarantees & registration
│   ├── mod.rs
│   ├── registry.rs                     # OpRef registration set (OpRef → record), existence check
│   ├── existence_check.rs              # contains(OpRef) before issue
│   └── exhaustion.rs                   # Near-exhaustion threshold + namespace rotation event
│
├── collision/                          # Collision detection
│   ├── mod.rs
│   ├── detector.rs                     # Birthday-bound-aware collision detection on issue
│   ├── regenerator.rs                  # Regenerate-with-new-seed on collision
│   └── bounds.rs                       # Birthday-bound math (√N ≈ 10.1M @ 50%, 1.43M @ 1%)
│
├── idempotency/                        # Idempotency binding (OpRef ↔ arguments)
│   ├── mod.rs
│   ├── binding.rs                      # Bind OpRef to blake3/sha256 fingerprint of normalized args
│   ├── fingerprint.rs                  # Canonical encoding + hash of operation arguments
│   ├── conflict.rs                     # Same OpRef + different fingerprint → conflict reject
│   └── retention.rs                    # Retention TTL + pruning (24h-style window)
│
├── retry/                              # Retry recognition
│   ├── mod.rs
│   ├── recognizer.rs                   # Lookup OpRef → InProgress/Committed/Failed/Rejected
│   ├── in_progress.rs                  # In-progress guard (block duplicate execution)
│   └── replay.rs                       # Return stored outcome for Committed retries
│
├── commit_status/                      # Commit-status lookup
│   ├── mod.rs
│   ├── status.rs                       # CommitStatus FSM: Pending → Committed/Failed/Rejected
│   ├── lookup.rs                       # Lookup by OpRef → status + outcome
│   └── transition.rs                   # Durable status transitions (write before ack)
│
├── sync/                               # Synchronization tracking (carrier-agnostic)
│   ├── mod.rs
│   ├── tracker.rs                      # Which endpoints observed which OpRefs
│   ├── endpoint_map.rs                 # OpRef → set of ids-carrier::Endpoint
│   └── observation.rs                  # Log observation events over Endpoint only
│
├── recovery/                           # Recovery tracking for interrupted ops
│   ├── mod.rs
│   ├── resume_token.rs                 # Resume token / checkpoint per OpRef
│   ├── checkpoint.rs                   # Progress: minted→bound→executing→committed
│   ├── reconciler.rs                   # Restart reconciliation of Pending/InProgress
│   └── deadline.rs                     # IdsTime idle/absolute recovery windows
│
├── audit/                              # Append-only audit records
│   ├── mod.rs
│   ├── log.rs                          # Append-only audit log of OpRef lifecycle events
│   ├── record.rs                       # AuditRecord: event type, OpRef, timestamp, outcome
│   ├── chain.rs                        # Optional ed25519 chaining of records
│   └── retention.rs                    # Audit retention policy
│
├── crypto/                             # Allowed crypto usage surface
│   ├── mod.rs
│   ├── fingerprint.rs                  # sha256/blake3 fingerprint helpers
│   └── signing.rs                      # Optional ed25519 signing (audit only)
│
├── types/                              # Composed types
│   ├── mod.rs
│   ├── op_record.rs                    # OpRef → {fingerprint, status, outcome, attempts, created_at}
│   └── generation_strategy.rs          # Strategy selection config
│
├── wire/                               # Serialization over Endpoint Vec<u8>
│   ├── mod.rs
│   ├── encode.rs                       # OpRef/record → byte payload
│   └── decode.rs                       # Byte payload → OpRef/record
│
├── fuzz/                               # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_mint.rs
│   ├── fuzz_collision.rs
│   └── fuzz_idempotency.rs
│
├── tests/                              # Integration & property tests
│   ├── mod.rs
│   ├── mint_tests.rs                   # All three strategies
│   ├── uniqueness_tests.rs             # No duplicate OpRefs
│   ├── collision_tests.rs              # Detection + regeneration
│   ├── idempotency_tests.rs            # Bind/conflict/replay
│   ├── retry_tests.rs                  # Recognition + dedup
│   ├── commit_status_tests.rs          # FSM transitions
│   ├── sync_tests.rs                   # Endpoint observation tracking
│   ├── recovery_tests.rs               # Restart reconciliation
│   ├── audit_tests.rs                  # Append-only + chain
│   ├── carrier_agnostic_tests.rs       # Asserts no prohibited deps compile
│   └── property_tests.rs               # Proptest/bolero invariants
│
├── benches/                            # Benchmarks
│   ├── mod.rs
│   ├── mint_bench.rs
│   ├── registry_bench.rs
│   └── collision_bench.rs
│
├── spec/                               # Normative specifications
│   ├── opref_spec.md                   # OpRef format & namespace spec
│   ├── generation_spec.md              # Strategy specs
│   ├── idempotency_spec.md             # Binding & conflict spec
│   ├── commit_status_spec.md           # FSM spec
│   ├── audit_spec.md                   # Audit record spec
│   └── threat_model.md                 # Threat model document
│
├── docs/                               # Documentation
│   ├── architecture.md
│   ├── integration_guide.md            # Integrating with ids-write / ids-carrier
│   ├── opref_vs_orderingpos.md         # Why OpRef is separate from ordering position
│   └── api_docs.md
│
└── test_vectors/                       # Deterministic test vectors
    ├── mint_vectors.json
    ├── fingerprint_vectors.json
    └── commit_status_vectors.json
```

**Why ~110–140 files:** one file per responsibility — OpRef type/namespace, three generation strategies, uniqueness registry, collision detection, idempotency binding, retry recognition, commit-status FSM, synchronization tracking, recovery/checkpoint, audit, wire encoding, crypto surface, tests, fuzz, benchmarks, specs, docs, test vectors. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 OpRef type and namespace

```rust
// OpRef: nine-character opaque reference, alphabet 0-9A-Z (case-insensitive), length 9
// Namespace size: N = 36^9 = 101,559,956,668,416 (~1.016e14)
// Per-character entropy: log2(36) ≈ 5.17 bits; total ≈ 46.5 bits
struct OpRef([u8; 9]);   // validated at construction; never reveals ordering
```

`OpRef` is distinct from `OrderingPos` (ids-000-aa). `OpRef` is never constructed from an `OrderingPos`, never contains a timestamp/replica/counter field, and is never sortable-by-creation by design.

#### 7.2 Birthday-bound collision math

```
N = 36^9 = 101,559,956,668,416
P(collision after k) ≈ 1 − exp(−k·(k−1) / (2·N))
k @ 50% collision  ≈ √N  ≈ 10,077,696  (~10.1M)
k @ 1% collision   ≈ √(2·N·0.01) ≈ 1,425,000 (~1.43M)
```

Random generation is viable to ~low-millions; beyond that, sequential minting or namespace rotation is required.

#### 7.3 Minting algorithm (all strategies)

```
mint(strategy) -> OpRef:
  loop:
    candidate = strategy.generate()          // sequential / random / blake3-based
    if registry.contains(candidate):         // collision
       strategy.on_collision()               // regenerate / advance / reject
       continue
    registry.register(candidate)             // atomic claim
    return candidate
```

Sequential strategy: `counter → base36 → zero-pad to 9`; counter sharded per ordering domain (never packed into OpRef).
Deterministic strategy: `candidate = base36(truncate(blake3(normalized_args || domain_separator), 9))`.

#### 7.4 Idempotency binding

```
fingerprint = blake3(canonical_encode(operation_type, target, normalized_args))   // or sha256
record = { opref, fingerprint, status: Pending, outcome: None, attempts: 0, created_at }
on repeat(opref, fingerprint):
  if record.fingerprint == fingerprint:
     return record.status/outcome   // recognized retry; Committed → replay outcome
  else:
     reject(Conflict)               // same OpRef, different arguments — forbidden
```

Record is stored atomically with the operation's first effect (ACID token + mutation).

#### 7.5 Commit-status FSM

```
Pending --execute ok--> Committed(outcome)   [terminal]
Pending --execute fail--> Failed              [terminal]
Pending --reject/conflict--> Rejected         [terminal]
Committed / Failed / Rejected: no outgoing transitions
```

#### 7.6 Synchronization tracking

```
observe(opref, endpoint: ids-carrier::Endpoint):
  sync_map[opref].insert(endpoint)   // endpoint = (CarrierType, Vec<u8>) only
queried(opref) -> Set<Endpoint>
```

No address parsing, no IP/port extraction; `Endpoint` is opaque.

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | Canonical `OpRef`, `OrderingPos`, `NodeId`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`; sealed contracts; errors ||
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); sole traffic channel ||
|| `ids-crypto` | sha256/blake3 fingerprints; ed25519 optional audit signing (allowed set only) ||
|| `ids-time` | Monotonic `IdsTime` deadlines/retention windows ||
|| `ids-ordering-domain` | `OrderingPos` reference (kept separate from `OpRef`) — no dependency direction reversal ||
|| `serde` + `serde_bytes` | Serialization over Endpoint `Vec<u8>` ||
|| `thiserror` | Error types ||
|| `blake3`, `sha2` | Hashing for fingerprints (allowed primitives) ||
|| `proptest` / `bolero` | Property-based testing ||
|| `criterion` | Benchmarks ||

No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. `ids-operation-ref` does not depend on any transport crate beyond `ids-carrier`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-056.1 | OpRef format | Minted OpRef is exactly 9 chars, [0-9A-Z] case-insensitive; unit test validates charset/length ||
|| AC-056.2 | Namespace size | 36^9 = 101,559,956,668,416 documented; near-exhaustion event fires at configurable threshold (e.g., 80%) ||
|| AC-056.3 | Uniqueness | No two accepted operations share an OpRef; property test over 100k mints ||
|| AC-056.4 | Collision detection | Random-mint collision triggers regeneration; duplicate OpRef never returned twice ||
|| AC-056.5 | OpRef ≠ OrderingPos | No construction of OpRef from OrderingPos; no timestamp/replica/counter embedded; type-level separation ||
|| AC-056.6 | Idempotency binding | Same OpRef + same args → replay stored outcome; same OpRef + different args → conflict reject ||
|| AC-056.7 | Retry recognition | InProgress → report pending; Committed → return stored outcome; Failed/Rejected → terminal ||
|| AC-056.8 | Commit-status FSM | Pending → Committed/Failed/Rejected; terminal states immutable; unit + transition tests ||
|| AC-056.9 | Synchronization tracking | Endpoint observation recorded via ids-carrier::Endpoint only; query returns observed endpoint set ||
|| AC-056.10 | Recovery tracking | Restart reconciles Pending/InProgress via resume tokens/checkpoints; no lost outcomes ||
|| AC-056.11 | Audit records | Append-only immutable log; every lifecycle event recorded; optional ed25519 chain verifies ||
|| AC-056.12 | Carrier agnosticism | Compiles with zero std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS/wireless deps; all traffic via ids-carrier::Endpoint ||
|| AC-056.13 | Allowed crypto only | sha256/blake3 for fingerprints; ed25519 optional signing; no other crypto primitives ||
|| AC-056.14 | Property tests | 100,000+ proptest/bolero iterations on uniqueness/idempotency invariants ||
|| AC-056.15 | Adversarial tests | Collision flood, conflict retry, stale-retry-after-TPL all fail-safe ||

---

### 10. Remaining Uncertainties

1. **Alphabet size:** 36 symbols (0-9A-Z, case-insensitive, 36^9) vs. dropping confusables (Crockford-style 32 symbols, 32^9 ≈ 3.6×10^13) — trades human-transcription friendliness for ~2.8× capacity. Decision deferred.
2. **Default generation strategy:** sequential counter (collision-free, reveals rate) vs. random (opaque, birthday-bound limited) vs. deterministic hash (replay-safe). Recommend sequential default; final choice depends on caller opacity requirements.
3. **Coordination-free minting across nodes:** a single monotonic counter is collision-free but centralized; sharding counter ranges per ordering domain needs allocation logic from `ids-ordering-domain`; random-with-detection is coordination-free but capacity-limited. No decision yet.
4. **Durability of the uniqueness registry:** in-memory vs. persisted via the `ids-000-aa` store contract; cross-restart retry recognition requires durable registration. Backend undefined.
5. **Retention window:** idempotency/commit-status/audit record TTL (Stripe ≥24h; ICP TX_WINDOW) — workload-dependent, not yet quantified.
6. **Audit signing:** whether ed25519 chaining of audit records is mandatory or optional; key management lives outside this crate (ids-identity/ids-crypto).
7. **`ids-write` split:** confirmed that `ids-operation-ref` generates and `ids-write` binds/validates (plan-013/CRATES.md), but the exact bind/validate API surface is not yet specified.
8. **Outcome storage:** where Committed outcomes live for replay (in this crate, in ids-write, or in a store) is undefined.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| OpRef separate from OrderingPos | `ids-000-aa` defines both; coupling them (Stellar SEP-0035 pattern) binds identity to a total order — rejected ||
|| 36^9 nine-character alphanumeric namespace | Directly matches prompt/Purpose; ≈1.016×10^14 space, ~46.5 bits ||
|| Collision detection as first-class item | Birthday bound (~10.1M @ 50%) makes collisions probable at modest volume; detection + regeneration required ||
|| Sequential counter default generation | Collision-free, simple, capacity-efficient; invertible int→alphanumeric mapping (block-id pattern) avoids hard-to-sync distributed state ||
|| Idempotency binding via blake3/sha256 fingerprint | Binds OpRef to operation arguments; same-key-different-args rejected (Stripe/Azure pattern) ||
|| Commit-status FSM Pending→Committed/Failed/Rejected | Terminal-state lookup enables retry recognition without re-execution ||
|| Carrier-agnostic via ids-carrier::Endpoint only | No std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/wireless; preserves I-CARR-001 ||
|| Allowed crypto subset (ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3) | Per HX-241/IDS policy; sha256/blake3 for fingerprints, ed25519 optional audit signing, xchacha20/chacha20 available if payloads need encryption ||
|| ids-operation-ref generates; ids-write binds/validates | Allocator visibility split per CRATES.md and plan-013.md ||
|| Append-only, optionally ed25519-chained audit | Event sourcing as system of record; tamper-evident lifecycle history ||
|| Research only, no code | This document is the blueprint; no source modifications ||

---

### Sources

1. Distributed System IDs (opaque vs transparent, prefixes, random vs UUID vs KSUID): https://mattlayman.com/blog/distributed-system-ids
2. Unique ID Generator — counter, random+birthday, coordinator blocks, timestamp+node+counter: https://arpanpathak.github.io/cracking-the-systems-programming-interview/distributed-systems-from-the-ground-up/08-unique-id-generator.html
3. Design a Distributed ID Generator (UUIDv4 n²/2^123, monotonic mode, ticket server, UUIDv7 RFC 9562, ULID): https://hiringbar.dev/cases/id-generation
4. Kubernetes Object Names and IDs (UID vs Name, idempotent creation): https://kubernetes.io/docs/concepts/overview/working-with-objects/names
5. block-id Rust crate (invertible integer→alphanumeric, successor to tiny_id): https://docs.rs/block-id/latest/block_id/
6. nexus-id Rust crate (Base62Id 11-char, Base36Id 13-char, ULID 26-char Crockford): https://docs.rs/nexus-id/latest/nexus_id/
7. terseid Rust crate (SHA256→base36→truncate, birthday-problem collision avoidance): https://crates.io/crates/terseid
8. ULID spec (Crockford Base32 excludes I/L/O/U, sortable, monotonic option): https://github.com/ulid/spec
9. IETF Idempotency-Key HTTP Header Field (client key, fingerprint, 400/422): https://apisix.apache.org/blog/2024/04/04/fix-duplicate-api-requests/
10. Stripe — Designing robust and predictable APIs with idempotency: https://stripe.com/blog/idempotency
11. AWS — Making retries safe with idempotent APIs (ClientToken, ACID token+mutation): https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/
12. On Idempotency Keys (UUID vs sequence vs transaction-log-derived; atomic key+result store): https://morling.dev/blog/on-idempotency-keys
13. Azure — Idempotent Consumer Pattern (stable dedup key, never transport IDs): https://learn.microsoft.com/en-us/azure/architecture/patterns/idempotent-consumer
14. ICP — Safe Retries and Idempotency (dedup window, created_at_time validity): https://docs.internetcomputer.org/guides/canister-calls/idempotency
14b. Every agent retry needs the same operation ID (Intend/Attempt/Recognise/Resolve): https://inference.institute/research/every-agent-retry-needs-the-same-operation-id
15. Resonate — Recovery Protocol (lease/heartbeat, fail-stop recovery): https://docs.resonatehq.io/spec/execution-model/recovery-protocol
16. etcd API guarantees (revision as cluster-wide logical clock): https://etcd.io/docs/v3.8/learning/api_guarantees
17. Stellar SEP-0035 (64-bit op ID packs ledger+tx+op-index — conflation anti-pattern): https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0035.md
18. Approaches to Conflict-free Replicated Data Types (globally unique element IDs, dots `(replica,counter)`, causal delivery): https://arxiv.org/pdf/2310.18220
19. JSON CRDT / Automerge operation IDs (Lamport `N x ReplicaID`, dedup by already-applied): https://github.com/ipfs/notes/blob/master/CRDT/json-crdt.md
20. durable_lambda_core OperationIdGenerator (deterministic blake2b op IDs, monotonic counter, determinism invariant): https://docs.rs/durable-lambda-core/latest/durable_lambda_core/operation_id/struct.OperationIdGenerator.html
21. YouTube video ID format (64/128-bit random, base64, no metadata, cancelled never reused): https://webapps.stackexchange.com/a/54448
22. Audit Log System Design (append-only store, idempotent consumers, protected archive): https://codelit.io/blog/audit-log-system-design
23. The Birthday Paradox in Engineering (√N collision rule): https://www.javacodegeeks.com/2026/11/the-birthday-paradox-in-engineering-why-collisions-arrive-sooner-than-you-think.html
24. HardGuard25 (birthday bound p = 1 − exp(−k(k−1)/(2·Nᴸ)), confusables excluded): https://github.com/snapsynapse/hardguard25/blob/main/SPEC.md
25. Short UID generation birthday analysis (StackOverflow): https://stackoverflow.com/questions/6248666/how-to-generate-short-uid-like-ax4j9z-in-js/59913873
26. Event Sourcing Pattern (append-only store as system of record, auditability): https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing
27. MongoDB resume tokens (oplog position, resume after crash): https://github.com/mongodb/specifications/blob/master/source/change-streams/change-streams.md
28. AWS Durable Execution — Idempotency and retries (deterministic keys from operation inputs, conditional writes, TTL): https://docs.aws.amazon.com/durable-execution/patterns/best-practices/idempotency
29. Microsoft agent design patterns — idempotent tool actions (scope keys by tenant/tool/operation, retain records): https://github.com/microsoft/agentdesignpatterns/blob/main/docs/patterns/idempotent-tool-actions.md
30. CUID vs ULID vs UUID (nanoid 21-char 126-bit, ULID 26-char Crockford, CUID2 24-char SHA-3): https://theproductguy.in/blogs/cuid-vs-ulid-vs-uuid

---

**Research status:** Complete. `plan-056.md` is the blueprint for implementing `ids-operation-ref` (Phase 056): nine-character alphanumeric OpRefs (36^9 namespace) separate from ordering positions, with reference generation, uniqueness guarantees, collision detection, idempotency binding, retry recognition, commit-status lookup, synchronization tracking, recovery tracking, and audit records — carrier-agnostic via `ids-carrier::Endpoint` only, restricted crypto set, `ids-000-aa` as master crate. No code written, no files created, no repository changes made.

**Research only — no code written, no files created in the repository, no repository modified.** Plan written to `C:/idfs/plans/plan-056.md`.
