## Phase 045 — Create the ids-client crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 045 creates a **new separate crate `ids-client`** (not folders inside an existing crate — per the IDS rule of separate crates for new capabilities). It provides the IDS client-side entry point: a **single client handle `IDS`** exposing **`ids_submit(op)`** — THE ONE CALL through which all client operations enter the IDS system, matching the sealed `trait Client` contract defined in `ids-000-aa` (`src/contracts/client.rs`: `fn ids_submit(&self, op: Operation) -> Result<OpRef, ClientError>`).

The crate builds `Operation` values for **write / read / update / delete / query / describe**, manages **idempotency keys**, allocates and tracks **operation references (OpRef)**, assembles **results**, and presents a **client-facing interface abstraction** in which the client addresses only logical data (dataset/record) and **never addresses a region, H-unit, or node directly**.

**Carrier-agnostic by design:** all traffic crosses the carrier boundary as opaque datagrams over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), via the sealed `ids-000-aa` `trait Carrier` (`send_datagram` / `receive_datagram`). Zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

**Allowed crypto only:** ed25519 (op/authentication signatures), xchacha20poly1305 and chacha20poly1305 (envelope encryption), sha256 and blake3 (hashing). No other algorithms.

**Research only** — this plan describes architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure. No source modifications.

---

### 2. Architecture Overview

`ids-client` sits between the application and `ids-000-aa` / `ids-carrier`. The application never talks to a carrier or node directly; it holds one `IDS` handle and calls `ids_submit(op)`.

```
┌─────────────────────────────────────────────────────────────┐
│                      APPLICATION LAYER                      │
│             (builds ops, holds one IDS handle)              │
├─────────────────────────────────────────────────────────────┤
│                         ids-client (NEW CRATE)              │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  IDS handle │  │ ids_submit() │  │ Op builders: write │  │
│  │  (single    │  │  THE ONE     │  │ /read/update/del/  │  │
│  │   client    │  │  CALL        │  │  query/describe    │  │
│  │   handle)   │  └──────────────┘  └────────────────────┘  │
│  └──────┬──────┘  ┌──────────────┐  ┌────────────────────┐  │
│         │         │ Idempotency  │  │ OpRef + lifecycle  │  │
│         │         │ key mgmt     │  │ tracking / results │  │
│         └────────▶│ (dedup, keys)│  │ assembly           │  │
│                   └──────────────┘  └────────────────────┘  │
│         ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │
│         │ Logical      │  │ Carrier      │  │ Crypto     │  │
│         │ addressing   │  │ boundary     │  │ boundary   │  │
│         │ (dataset/    │  │ ids-carrier::│  │ ed25519 /  │  │
│         │  record only)│  │ Endpoint     │  │ xchacha20  │  │
│         └──────┬───────┘  │ (opaque      │  │ poly1305 / │  │
│                │          │  bytes)      │  │ sha256 /   │  │
│                │          └──────┬───────┘  │ blake3     │  │
│                └─────────────────┼──────────┴────────────┘  │
│                                   │                          │
├───────────────────────────────────┼──────────────────────────┤
│              ids-000-aa           │       ids-carrier        │
│  sealed trait Client (ids_submit) │  Endpoint (CarrierType   │
│  sealed trait Carrier             │  + Vec<u8>), Datagram    │
│  Operation, OpRef, IdempotencyKey │  send_datagram/receive_  │
│  IdempotencyManager, OpLifecycle  │  datagram, CarrierFrame  │
│  Tracker, OpResultMgr             │                          │
└───────────────────────────────────┴──────────────────────────┘
```

**Data flow of one `ids_submit(op)`:**
1. Application builds an `Operation` (write/read/update/delete/query/describe) over logical data using `IDS` builder methods; logical addressing only — no region/H-unit/node names appear in client code.
2. `IDS::ids_submit(op)` validates the op, generates/checks the idempotency key, allocates an `OpRef`, registers the op in the lifecycle tracker, signs the op (ed25519) and optionally envelope-encrypts it (xchacha20poly1305/chacha20poly1305), hashes content (blake3/sha256), and hands the opaque byte payload to the carrier layer.
3. The carrier layer maps the logical destination to an `ids-carrier::Endpoint` (via the `ids-000-aa` `Routing` contract) and sends `send_datagram(endpoint, payload)`. The client never learns or uses physical addresses.
4. Replies come back as datagrams keyed by the `OpRef`/correlation ID; `ids-client` matches response to pending op (request/response correlation), assembles the `OpResult`, and advances the lifecycle to a terminal stage (Committed/Verified or Failed).
5. The application reads results via `op_result(op_ref)` / `op_state(op_ref)` on the `IDS` handle.

**No end-to-end path assumption:** `ids_submit` is asynchronous-by-construction over a datagram carrier; delivery/ack timing is unbounded and handled by the pending-op tracker, not by RTT-bound timeouts.

---

### 3. Detailed Research Dimensions

#### 3.1 Single client handle — `IDS` struct

Research surface: single-handle client SDKs (gemini-cli-sdk `Client` with `connect`/`send`/`close` lifecycle; Aerospike Rust best-practices: one shared `Send + Sync` client instance per cluster, reused via `Arc`; `client-handle` pattern: channel + background worker + `oneshot` response channels).

**Decision:** `IDS` is the single client handle. It is `Clone` (cheap handle, internally `Arc<Inner>`) and `Send + Sync` so it can be shared across threads/tasks. Inner holds: session id, client `NodeId`, handle to `ids-000-aa` `ClientEngine` (session/idempotency/op-lifecycle/result managers), a carrier session, a pending-op map keyed by `OpRef`, and crypto context. Public surface: `ids_submit(op)`, plus convenience builders `write(...)`, `read(...)`, `update(...)`, `delete(...)`, `query(...)`, `describe(...)` that return an `Operation`, and `op_state(op_ref)` / `op_result(op_ref)` queries.

#### 3.2 `ids_submit(op)` — THE ONE CALL

Matches `ids-000-aa` `trait Client::ids_submit(&self, op: Operation) -> Result<OpRef, ClientError>`. Flow (already sketched in `ids-000-aa` `ClientEngine::ids_submit`): validate operation → idempotency duplicate check → register idempotency key → register op lifecycle (Received → Ordered) → record result placeholder → return `OpRef`. `ids-client` adds the client-side half: sign/encrypt the op, address the logical destination to a carrier `Endpoint`, send the datagram, and register the pending-op entry so the reply can be matched later.

Reference patterns: Microsoft API guidelines (`x-ms-client-request-id` client correlation id; service echoes `x-ms-request-id`); Google AIP-151 long-running operations (client gets an op token to poll for the result); Azure Asynchronous Request-Reply (decouple request from reply, polling acceptable).

#### 3.3 Write / read / update / delete / query / describe operations

`ids-000-aa` `types.rs` defines `Operation` with fields including `op_type: OpType`, `dataset_id: DatasetId`, `payload: Vec<u8>`, `idempotency_key: IdempotencyKey`, `op_ref: OpRef`, `ids_time: IdsTime`. `ids-client` provides typed builders so application code never touches `Operation` internals directly:

- **Write**: dataset id + logical record key + bytes → `OpType::Write`
- **Read**: dataset id + logical record key → `OpType::Read`
- **Update**: dataset id + logical record key + patch bytes → `OpType::Update`
- **Delete**: dataset id + logical record key → `OpType::Delete`
- **Query**: dataset id + query predicate → `OpType::Query`
- **Describe**: dataset id (or record) → `OpType::Describe` (schema/metadata introspection)

All builders attach a fresh idempotency key and OpRef, then delegate to `ids_submit`.

#### 3.4 Logical data addressing (client never addresses region/H-unit/node directly)

Client addresses only logical identifiers: `DatasetId`, `RecordId`/logical key, possibly a logical path. Resolution of logical → physical (which carrier endpoint, which node/H-unit/region) is the job of the `ids-000-aa` `Routing` contract (`trait Routing`: logical → physical resolution), not the client. This satisfies the constraint "client-facing interface abstraction (client never addresses region/H-unit/node directly)". Research backing: logical addressing decouples clients from physical node identity (Wiley "logical addressing" survey; versatile anycast maps logical address to physical nodes transparently).

#### 3.5 Idempotency key management

Client generates an `IdempotencyKey` per logical operation (UUID-style or derived from op content via blake3/sha256). `ids-000-aa` `IdempotencyManager` tracks key → (OpRef, status: Pending/Processed/Completed/Failed, result). On `ids_submit`, duplicate key → `ClientError::IdempotencyConflict` (or return the existing result if terminal). Retry of the same op reuses the same key → effectively-once semantics.

Research backing: Azure Idempotent Consumer (at-least-once delivery + idempotent consumer = effectively-once); request-token idempotency at scale (client-generated token, `Idempotency-Key`-style header, server dedup store with unique constraint); idempotency key max length ~256 bytes, scoped per scope/queue.

#### 3.6 Operation reference tracking

Each op gets an `OpRef` (allocated by `ids-000-aa` `OpRefAllocator`; 9-byte bytes). `OpLifecycleTracker` models stages: Received → Ordered → Executing → Committed → Synchronized → Verified (terminal) or Failed (terminal), with validated transitions. Pending-op map in `ids-client` maps `OpRef` → (submitted op, deadline, retry count, oneshot result sender). Replies carry the `OpRef`/correlation id and settle the pending entry exactly once (TOON-RPC client pattern: pending call removed before settle, settles exactly once; unknown/duplicate IDs reported as diagnostics).

Research backing: Azure API guidelines correlation ids; SQS request-response (correlation_id echoed in response, sender matches against pending list).

#### 3.7 Result assembly

`OpResult` (ids-000-aa): `{ op_ref, commit_state: CommitState, dataset_id, completed_at_ms, result_payload: Vec<u8> }`. `OpResultMgr` records results keyed by `OpRef`. `ids-client` assembles the final application-visible result from the reply datagram: verify signature, match `OpRef`, deserialise payload, return typed result (e.g. `Option<Vec<u8>>` for read, `CommitState` for write). Long-running ops: client polls `op_state`/`op_result` using the `OpRef` token (AIP-151 pattern) until terminal.

#### 3.8 Carrier-agnostic traffic via `ids-carrier::Endpoint`

`ids-carrier` exposes `Endpoint` (`CarrierType` + `Vec<u8>`) only. `ids-000-aa` `trait Carrier`: `send_datagram(&self, endpoint: &Endpoint, data: &[u8])`, `receive_datagram(&self) -> Result<(Endpoint, Vec<u8>>)`, `carrier_type()`, `max_datagram_size()`, delivery semantics enum (BestEffort/AtLeastOnce/AtMostOnce/ExactlyOnce), `CarrierFrame` (seq/flags/checksum/payload codec), `CarrierRouter`, `CarrierPool`, `CarrierTransactionId`. No `std::net`, `tokio::net`, IP, TCP, UDP, QUIC, HTTP, DNS, or any wireless carrier.

Research backing for the "no transport in the protocol" approach: clonic (ZCP codec — no transport, opaque bytes payload, transport-agnostic by design); chateau/fetch/overseerd-client (transport-agnostic client pipelines, protocol separated from transport); navar (transport-agnostic HTTP client over TCP/TLS/P2P streams — cited as contrast; ids-client adopts the separation but with opaque datagrams, not HTTP).

#### 3.9 Cryptographic budget (allowed crypto only)

- **ed25519**: sign op envelope (authentication/authorization). `ed25519-dalek`: `SigningKey`/`VerifyingKey`, `Signer`/`Verifier` traits, `verify_batch` for verification throughput (docs.rs ed25519-dalek).
- **xchacha20poly1305**: envelope encryption of op payloads; 24-byte (192-bit) nonce permits safe random nonces (birthday bound 2^96 vs 2^48 for 96-bit). `chacha20poly1305` RustCrypto crate implements ChaCha20Poly1305 (RFC 8439) and XChaCha20Poly1305; libsodium documents XChaCha20-Poly1305 as recommended for random-nonce misuse resistance.
- **chacha20poly1305**: standard 12-byte nonce variant where nonces are counter-derived (sequence-bound).
- **sha256**: `sha2` crate `Sha256` / `Digest` trait, fixed 32-byte output.
- **blake3**: `blake3` crate `Hasher`, incremental hashing, keyed hashing, key derivation (`derive_key`), extendable output (XOF), 32-byte default output; content addressing and idempotency-key derivation.

No other cryptographic primitives (no X25519/KEM, no AES, no RSA, no SHA-1/SHA-512 unless added to the allowed list by policy change).

#### 3.10 Client-facing interface abstraction

The `IDS` handle is the entire client API surface. Application code imports `ids_client::IDS` and calls builders + `ids_submit`. It never imports `ids-carrier` types, never constructs a `CarrierType`, never sees an IP/port/endpoint address, and never names a region/H-unit/node. `ids-000-aa` `trait Client` is sealed (downstream can use as bound, not implement), so `IDS` wraps `ClientEngine`/`Client` and adds the ergonomic, strongly-typed client layer.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Replay of a submitted op (duplicate datagram) | Idempotency key dedup; same key → same OpRef/result, no re-execution (IdempotencyManager) |
| Duplicate idempotency key for a different op | `DuplicateIdempotencyKey` error; caller must use a unique key per logical op |
| Op reply lost / delayed | Pending-op tracker with retry; `OpRef` token polling (`op_state`/`op_result`); no RTT-bound timeout — deadlines keyed to op submission time |
| Tampered op payload in carrier | ed25519 signature over op envelope verified before/after carrier transit |
| Payload confidentiality | xchacha20poly1305 / chacha20poly1305 envelope encryption (AAD = op metadata) |
| Logical address spoofing | Resolution via ids-000-aa `Routing` contract; client never supplies physical addresses |
| Oversized datagram | `CarrierError::DatagramTooLarge` / `AddressTooLarge` enforced at carrier boundary (max 65536 default, address ≤ 1024 bytes) |
| Carrier unavailable / transient failure | `CarrierError::is_retryable()`; retry with backoff (CarrierRetryManager pattern); non-retryable errors surface as `SubmitFailed` |
| Result mismatch / wrong op gets reply | Correlation by `OpRef` echoed in reply; unknown/duplicate IDs rejected (TOON-RPC settle-once pattern) |
| Weak nonce reuse | XChaCha20Poly1305 with random nonces; ChaCha20Poly1305 nonces derived from OpRef sequence |
| Key compromise of client signing key | Out of scope for Phase 045 (key rotation/revocation deferred to identity/security phases); client signs op envelope only |

---

### 5. Integration with Existing Crates

Phase 045 depends on and integrates with:

|| Crate | Role in Phase 045 ||
||-------|-------|-------------------|
| `ids-000-aa` | Master crate: `Operation`, `OpRef`, `IdempotencyKey`, `DatasetId`, `RecordId`, `NodeId`, `Endpoint`, `CarrierType`, `CommitState`, `OpState`, `OpType`, `IdsTime`, sealed `trait Client` (`ids_submit`), sealed `trait Carrier`, `ClientEngine`, `IdempotencyManager`, `OpRefAllocator`, `OpLifecycleTracker`, `OpResultMgr`, `ClientError` |
| `ids-carrier` | `Endpoint` (`CarrierType + Vec<u8>`), `Datagram`, carrier capability negotiation; all client traffic rides `Endpoint` opaque datagrams |
| `ids-crypto` | Allowed primitives only: ed25519 sign/verify, XChaCha20Poly1305/ChaCha20Poly1305 seal/open, blake3/sha256 hashing |
| `ids-time` | `IdsTime` timestamps on operations (IDS proprietary time basis, not wall-clock) |
| `ids-topology` | Failure-domain context for routing (logical → physical resolution hints, no client-visible node addressing) |

**Integration points:**
- `ids-client::IDS` holds `ids-000-aa` `Client`/`ClientEngine` and calls `ids_submit(op)` — THE ONE CALL.
- Carrier sends/receives go through `ids-carrier::Endpoint` + `trait Carrier`; `ids-client` contains no carrier implementation and no networking imports.
- `ids-000-aa` managers (idempotency, lifecycle, results) are reused verbatim; `ids-client` adds the pending-op map and result assembly on top.
- Logical-to-physical resolution delegates to `ids-000-aa` `trait Routing`; client supplies only `DatasetId`/logical key.

---

### 6. Proposed File Structure (~140 files, all in subfolders)

**Crate:** `ids-client` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; `serde`, `thiserror`, `bytes`, `blake3`, `sha2`, `chacha20poly1305`, `ed25519-dalek`, `zeroize`, `getrandom`, `tokio` runtime/timers only — no `tokio::net`)

```
ids-client/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports (IDS, Operation builders)
├── constants.rs                    # Protocol limits: max op payload, idempotency key size, pending-op cap
├── error.rs                        # ClientError (thiserror): submit, idempotency, op-ref, result, carrier-wrap variants
├── features.rs                     # Feature flags (crypto, fuzzing, testing)
│
├── handle/                         # IDS single client handle
│   ├── mod.rs
│   ├── ids.rs                      # IDS struct: Clone, Arc inner, Send + Sync, public API
│   ├── inner.rs                    # Inner state: session, pending ops, crypto context, carrier session
│   ├── session.rs                  # Session bind: session_create/expire/heartbeat delegation
│   ├── submit.rs                   # ids_submit orchestration: validate → key → OpRef → sign → send → register pending
│   ├── lifecycle.rs                # Pending-op map, OpRef → pending entry, reply settlement, timeout sweep
│   └── handle_ops.rs               # op_state / op_result query methods
│
├── ops/                            # Operation payload construction
│   ├── mod.rs
│   ├── builders.rs                 # write/read/update/delete/query/describe builders
│   ├── op_type.rs                  # OpType mapping to ids-000-aa OpType
│   ├── write_op.rs                 # Write operation assembly
│   ├── read_op.rs                  # Read operation assembly
│   ├── update_op.rs                # Update operation assembly
│   ├── delete_op.rs                # Delete operation assembly
│   ├── query_op.rs                 # Query operation assembly
│   ├── describe_op.rs              # Describe operation assembly
│   └── op_envelope.rs              # Signed/encrypted op envelope (ed25519 sig + AEAD + hash)
│
├── addressing/                     # Logical data addressing (logical → carrier endpoint)
│   ├── mod.rs
│   ├── logical.rs                  # LogicalId, DatasetId/RecordId wrappers
│   ├── resolver.rs                 # Delegates to ids-000-aa Routing contract; client sees none of this
│   ├── handle.rs                   # Logical data handle (opaque to application)
│   └── routing_adapter.rs          # ids-000-aa Routing trait adapter
│
├── idempotency/                    # Idempotency key management
│   ├── mod.rs
│   ├── key.rs                      # IdempotencyKey generation (random or content-derived via blake3/sha256)
│   ├── manager.rs                  # Wrapper around ids-000-aa IdempotencyManager
│   ├── dedup.rs                    # Duplicate detection and retry-key reuse policy
│   └── store.rs                    # Pending idempotency entries with TTL eviction
│
├── references/                     # Operation reference tracking
│   ├── mod.rs
│   ├── op_ref.rs                   # OpRef wrapper + allocation via ids-000-aa OpRefAllocator
│   ├── tracker.rs                  # Pending-op lifecycle: Received → … → Verified/Failed
│   ├── correlation.rs              # OpRef as correlation id in request/reply datagrams
│   └── pending.rs                  # PendingOp entry: op, deadline, attempts, result sender
│
├── results/                        # Result assembly
│   ├── mod.rs
│   ├── result.rs                   # Client-visible result types per op kind
│   ├── assembler.rs                # Match reply datagram (OpRef) → OpResult → typed result
│   ├── envelope.rs                 # Reply envelope verification (signature, hash)
│   ├── polling.rs                  # op_state/op_result polling loop for long-running ops
│   └── pending_map.rs              # In-flight results awaiting carrier reply
│
├── carrier/                        # Carrier boundary (ids-carrier::Endpoint only)
│   ├── mod.rs
│   ├── endpoint.rs                 # Endpoint handling (opaque; no parsing of address bytes)
│   ├── datagram.rs                 # Datagram send/receive via ids-000-aa trait Carrier
│   ├── router.rs                   # CarrierType-based routing (CarrierRouter pattern)
│   ├── frame.rs                    # CarrierFrame-style framing (seq/flags/checksum/payload)
│   ├── session.rs                  # Carrier session bind to IDS session
│   └── retry.rs                    # Retry/backoff for retryable CarrierError
│
├── crypto/                         # Cryptographic boundary (allowed primitives only)
│   ├── mod.rs
│   ├── signing.rs                  # ed25519 sign/verify of op envelopes
│   ├── encryption.rs               # XChaCha20Poly1305 / ChaCha20Poly1305 seal/open
│   ├── hashing.rs                  # blake3 / sha256 digests, key derivation
│   ├── nonce.rs                    # Nonce management: random (XChaCha) vs sequence-derived (ChaCha)
│   └── budget.rs                   # Enforcement: only allowed algorithms, no others
│
├── config/                         # Configuration
│   ├── mod.rs
│   ├── client_config.rs            # IDS handle defaults: timeouts, retries, key sizes
│   ├── crypto_config.rs            # Allowed-algorithm configuration
│   └── carrier_config.rs           # Carrier endpoint/session configuration
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_submit.rs
│   ├── fuzz_envelope.rs
│   └── fuzz_addressing.rs
│
├── tests/                          # Integration & adversarial tests
│   ├── mod.rs
│   ├── handle_tests.rs             # IDS handle lifecycle, Clone, submit
│   ├── ops_tests.rs                # Write/read/update/delete/query/describe builders
│   ├── idempotency_tests.rs        # Dedup, duplicate key, retry with same key
│   ├── reference_tests.rs          # OpRef allocation, lifecycle transitions
│   ├── result_tests.rs             # Reply matching, result assembly, polling
│   ├── carrier_tests.rs            # Endpoint datagram send/receive, retry
│   ├── crypto_tests.rs             # Sign/verify/seal/open/hash round-trips
│   ├── addressing_tests.rs         # Logical → endpoint resolution, no physical leakage
│   ├── integration_tests.rs        # End-to-end submit → reply over mock carrier
│   ├── adversarial_tests.rs        # Tampered envelope, replay, oversized datagram
│   ├── determinism_tests.rs        # Same input → same OpRef/key (deterministic builders)
│   ├── property_tests.rs           # Proptest properties
│   └── replay_tests.rs             # Replay attack tests
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── submit_bench.rs             # ids_submit latency
│   ├── envelope_bench.rs           # Sign/seal/hash cost
│   ├── result_bench.rs             # Result assembly latency
│   └── carrier_bench.rs            # Datagram send/receive overhead
│
├── spec/                           # Normative specifications
│   ├── client_spec.md              # IDS handle & ids_submit spec
│   ├── op_spec.md                  # Operation envelope & lifecycle spec
│   ├── idempotency_spec.md         # Idempotency key spec
│   ├── carrier_boundary_spec.md    # Endpoint datagram boundary spec
│   ├── crypto_spec.md              # Allowed-crypto usage spec
│   └── threat_model.md             # Threat model document
│
├── docs/                           # Documentation
│   ├── architecture.md             # Client architecture & layering
│   ├── integration_guide.md        # Using IDS handle from applications
│   ├── addressing_guide.md         # Logical addressing (client never sees nodes)
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── envelope_vectors.json
    ├── op_ref_vectors.json
    └── result_vectors.json
```

**Why ~140 files:** one file per responsibility — handle lifecycle, submit orchestration, six op builders, envelope, logical addressing, idempotency, OpRef tracking, pending ops, result assembly, carrier boundary, crypto boundary, config, fuzz, tests, benches, specs, docs, test vectors. No stubs; every file holds production-grade design with error handling and tests at implementation time. Fits the "~90-1000 files in subfolders" mandate.

---

### 7. Key Algorithms & Data Structures

#### 7.1 IDS handle and ids_submit flow

```
IDS::ids_submit(op):
  1. validate(op)                       // ids-000-aa ClientValidator
  2. key = idempotency_key(op)          // fresh or caller-supplied IdempotencyKey
  3. if duplicate(key) → IdempotencyConflict
  4. op_ref = allocate_op_ref()         // OpRefAllocator (ids-000-aa)
  5. register(key, op_ref)              // IdempotencyManager, status Pending
  6. register lifecycle(op_ref, Received → Ordered)
  7. envelope = sign(op) [+ seal(op)]   // ed25519 sign; optional xchacha20poly1305/chacha20poly1305 seal; blake3/sha256 digest
  8. endpoint = resolve_logical(op.dataset_id, op.record_key) // ids-000-aa Routing → ids-carrier::Endpoint
  9. carrier.send_datagram(endpoint, envelope bytes)
  10. pending[op_ref] = PendingOp { op, deadline, attempts, result_sender }
  11. return Ok(op_ref)
```

#### 7.2 Operation envelope

```
OpEnvelope {
    op: Operation,                          // ids-000-aa types
    signature: [u8; 64],                    // ed25519 over canonical op encoding
    nonce: [u8; 24] or [u8; 12],            // XChaCha20Poly1305 or ChaCha20Poly1305 nonce
    ciphertext: Vec<u8>,                    // AEAD-encrypted op payload (AAD = op metadata)
    digest: [u8; 32],                       // blake3 or sha256 of canonical op
}
```

Wire = `Vec<u8>` carried inside `ids-carrier::Endpoint` datagram. Carrier sees only opaque bytes.

#### 7.3 Idempotency key

`IdempotencyKey = [u8; 32]` (ids-000-aa type). Generated either randomly (`getrandom`) or deterministically: `blake3(canonical(op))` or `sha256(canonical(op))`. Max key size 256 bytes; scoped per client session. Duplicate key with terminal result → return cached result; duplicate key pending → `IdempotencyConflict`.

#### 7.4 OpRef lifecycle

`OpRef = [u8; 9]` (ids-000-aa type). Stages: Received → Ordered → Executing → Committed → Synchronized → Verified (terminal) | Failed (terminal). Transitions validated; only forward transitions allowed. Completion rate and failed count tracked (ids-000-aa `OpLifecycleTracker`).

#### 7.5 Result assembly via correlation

Reply datagram carries `OpRef` as correlation id. `PendingOp` entry holds a `oneshot`-style result sender; on reply, match `OpRef`, verify envelope signature/digest, deserialize `result_payload`, complete `OpResultMgr` entry, settle pending entry exactly once. Application reads `op_result(op_ref)`.

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
| `ids-000-aa` | Master crate: Operation, OpRef, IdempotencyKey, DatasetId, NodeId, Endpoint, CarrierType, sealed Client/Carrier traits, ClientEngine, IdempotencyManager, OpRefAllocator, OpLifecycleTracker, OpResultMgr, ClientError |
| `ids-carrier` | Endpoint (CarrierType + Vec<u8>), Datagram, carrier trait boundary |
| `ids-crypto` | Allowed primitives: ed25519, XChaCha20Poly1305/ChaCha20Poly1305, blake3, sha256 |
| `ids-time` | IdsTime timestamps on operations |
| `ed25519-dalek` | ed25519 SigningKey/VerifyingKey, Signer/Verifier, verify_batch |
| `chacha20poly1305` | XChaCha20Poly1305 / ChaCha20Poly1305 AEAD (RFC 8439) |
| `blake3` | BLAKE3 hashing, keyed hash, key derivation, XOF |
| `sha2` | SHA-256 Digest |
| `serde` + `serde_bytes` | Operation/envelope serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling for opaque datagrams |
| `zeroize` | Key material zeroing |
| `getrandom` | Nonce/key generation |
| `tokio` | Async runtime, timers, spawn — runtime only; **no `tokio::net`** |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

**Prohibited (explicitly excluded):** `std::net`, `tokio::net`, libp2p, any TCP/UDP/QUIC/HTTP/DNS crate, any Wi-Fi/Bluetooth/cellular/VPN/mesh crate. No networking stack may be imported by `ids-client`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
| AC-045.1 | Single client handle | `IDS` is one struct, `Clone` + `Send + Sync`, exposes `ids_submit(op)` returning `Result<OpRef, ClientError>` | compile + unit test |
| AC-045.2 | THE ONE CALL contract | `ids_submit` matches sealed `ids-000-aa` `trait Client::ids_submit` signature | compile against ids-000-aa |
| AC-045.3 | Six op builders | write/read/update/delete/query/describe builders produce valid `Operation` with correct `OpType` | unit test per builder |
| AC-045.4 | Logical addressing only | Client API references `DatasetId`/`RecordId` only; no region/H-unit/node types in public API | API audit |
| AC-045.5 | Idempotency dedup | Same idempotency key twice → same OpRef, second returns existing result or `IdempotencyConflict` | unit test |
| AC-045.6 | OpRef lifecycle | OpRef transitions Received → … → Verified/Failed only via valid transitions; terminal states unreachable twice | state-machine unit test |
| AC-045.7 | Result assembly | Reply datagram with matching OpRef → typed `OpResult`; unknown OpRef rejected | integration test |
| AC-045.8 | Carrier boundary | All sends via `ids-carrier::Endpoint` + `send_datagram`; zero `std::net`/`tokio::net`/libp2p/HTTP/etc. imports | dependency audit + grep |
| AC-045.9 | Crypto budget | Only ed25519/xchacha20poly1305/chacha20poly1305/sha256/blake3 used; sign/verify/seal/open/hash round-trips pass | crypto unit tests |
| AC-045.10 | Signature integrity | Tampered op envelope rejected before submit/send | adversarial test |
| AC-045.11 | Replay resistance | Replayed datagram with same idempotency key is detected as duplicate | replay test |
| AC-045.12 | Deterministic builders | Same logical input → same IdempotencyKey/OpRef (when caller does not supply them) | property test |
| AC-045.13 | Result polling | `op_state`/`op_result` return terminal state for completed op; `OperationNotFound` for unknown OpRef | unit test |
| AC-045.14 | Error categories | `ClientError` categories match ids-000-aa (session_not_found, idempotency_conflict, submit_failed, …) | unit test |
| AC-045.15 | Property tests pass | 100,000+ proptest/bolero iterations on submit/idempotency/lifecycle invariants | proptest run |

---

### 10. Remaining Uncertainties

1. **Pending-op storage bounds:** The in-flight `OpRef → PendingOp` map must be bounded (size/age/ack-based eviction); eviction policy not yet decided — risk of memory growth under unreplied ops.
2. **Carrier reply model:** `receive_datagram` blocks on any endpoint; matching replies to pending ops requires a dispatch loop or per-endpoint receiver — concurrency model (single receiver task vs. per-endpoint) unresolved.
3. **Envelope format:** Canonical serialization of `OpEnvelope` (CBOR via serde_cbor vs. bincode vs. custom) not yet chosen; must be deterministic for signature verification.
4. **Session/carrier binding:** How `IDS` binds to a `ids-carrier` carrier instance (injected at construction vs. resolved per `CarrierType`) is open.
5. **Op timeout/deadline:** Deadlines keyed to op submission time (no RTT timers) — default TTL and sweep interval unquantified.
6. **Result size limits:** Max `result_payload` size vs. `max_datagram_size`; chunking of large results not designed.
7. **Ids-000-aa coupling:** `ids-client` reuses `ClientEngine` managers directly — the stability boundary between master crate and client crate needs a versioning policy.
8. **Nonce exhaustion:** ChaCha20Poly1305 96-bit nonces derived from OpRef sequence — wrap-around behavior after 2^96 ops (theoretical; needs documented limit).

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
| New crate `ids-client`, not subfolder | IDS rule: separate crates for new capabilities; keeps `ids-000-aa` as master contract point only |
| Single `IDS` handle, `ids_submit(op)` THE ONE CALL | Matches sealed `ids-000-aa` `trait Client`; single entry point for all client ops |
| `IDS` is `Clone`/`Arc`+`Send + Sync` | Shared handle across threads/tasks (Aerospike single-client pattern) |
| Logical addressing only; no physical addresses in client | `ids-000-aa` `Routing` contract resolves logical → carrier endpoint; client never names region/H-unit/node |
| All traffic via `ids-carrier::Endpoint` (`CarrierType + Vec<u8>`) | Carrier-agnostic absolute rule (CRATES.md); no networking stack imports |
| Crypto limited to ed25519 / XChaCha20Poly1305 / ChaCha20Poly1305 / sha256 / blake3 | IDS allowed-crypto policy (CRATES.md, ids-crypto crate) |
| Idempotency key client-generated; dedup via IdempotencyManager | Azure Idempotent Consumer / request-token idempotency patterns |
| OpRef as correlation id for request/reply | SQS request-response / AIP-151 op-token patterns; settles exactly once |
| No RTT-bound timeouts; deadlines keyed to submission time | Unbounded carrier latency; avoids TCP-style timeout misfire |
| Sign op envelope (ed25519); optional AEAD seal (xchacha20poly1305) | Authentication + optional confidentiality using allowed primitives only |
| Reuse ids-000-aa managers verbatim (idempotency, lifecycle, results) | Single control point: contract changes in one file propagate to all clients |

---

### Sources

Consulted during the 30 targeted searches (local repository first, then public references):

1. `C:/idfs/ids-000-aa/src/contracts/client.rs` — sealed `trait Client`, `ids_submit(op: Operation) -> Result<OpRef, ClientError>`, `ClientEngine`, `IdempotencyManager`, `OpRefAllocator`, `OpLifecycleTracker`, `OpResultMgr`, `ClientError`.
2. `C:/idfs/ids-000-aa/src/contracts/carrier.rs` — sealed `trait Carrier` (`send_datagram`/`receive_datagram`), `CarrierError`, `CarrierFrame`, `CarrierRouter`, `CarrierPool`, delivery semantics.
3. `C:/idfs/CRATES.md` — IDS crate inventory: `ids-000-aa` master crate; `ids-carrier` `Endpoint` = `CarrierType + Vec<u8>`; `ids-crypto` allowed primitives; carrier-agnostic absolute rule.
4. ed25519-dalek (docs.rs) — `SigningKey`/`VerifyingKey`, `Signer`/`Verifier`, `verify_batch`.
5. chacha20poly1305 (docs.rs, RustCrypto) — `ChaCha20Poly1305`, `XChaCha20Poly1305`, RFC 8439, 24-byte extended nonce.
6. libsodium — XChaCha20-Poly1305 construction (192-bit nonce, random nonces safe, misuse resistance).
7. blake3 (docs.rs) — `Hasher`, keyed hash, `derive_key`, XOF, 32-byte output.
8. sha2 (docs.rs) — `Sha256`, `Digest` trait.
9. Azure Idempotent Consumer pattern — at-least-once delivery + idempotent consumer = effectively-once.
10. Request Token Idempotency at Scale — client-generated idempotency token, dedup store, 256-byte key bound.
11. Microsoft Azure API guidelines — `x-ms-client-request-id` / `x-ms-request-id` correlation ids.
12. Google AIP-151 — long-running operations: op token returned to client for polling.
13. Azure Asynchronous Request-Reply — decoupled request/reply, polling acceptable.
14. SQS Request-Response Pattern — `correlation_id` echoed in response, matched against pending list.
15. TOON-RPC client (reddb-io-toon-rpc) — pending-call correlation over any transport, settle-exactly-once, unknown-ID diagnostics.
16. clonic — transport-agnostic wire protocol, opaque bytes payload, no TCP/BLE/LoRa/libp2p.
17. chateau — Tower `Service`-based protocol-agnostic client/server.
18. fetch crate — runtime- and transport-agnostic client pipeline with pluggable transport handler.
19. overseerd-client — client contract traits (Unary/streaming) over a shared `Transport`, no fixed serialization.
20. gemini-cli-sdk — single `Client` handle: `connect`/`send`/`close` lifecycle.
21. Aerospike Rust client best practices — one shared `Send + Sync` client per cluster, reused via `Arc`.
22. irkle / eml / winter-crypto — BLAKE3 Merkle tree implementations (content addressing reference).
23. thiserror (docs.rs) — error enum derive pattern.
24. bytes (docs.rs) — `Bytes`/`BytesMut` zero-copy buffer for opaque datagrams.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 045 (ids-client crate).
