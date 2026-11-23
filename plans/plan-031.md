## Phase 031 — Create the ids-security crate: Security across complete infrastructure.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---\n

### 1. Executive Summary

Phase 031 creates a **new separate crate `ids-security`** (not folders inside an existing crate), organized into subfolders by subsystem. It is the IDS security layer implementing mutual authentication (mutual), dataset/operation/identity-level authorization, data encryption in transit + at rest, key management (generation, rotation, revocation, distribution, recovery), regional security boundaries, auditability, and tamper-evident audit records. The crate is strictly carrier-agnostic: all traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

`ids-security` depends on `ids-000-aa` (master crate, canonical types + sealed trait contracts) and `ids-crypto` (leaf crate, allowed primitives). It implements the sealed `Security` trait contract defined in `ids-000-aa/src/contracts/security.rs`. There is no global security master — each dataset/region manages its own security context. The design is research-only: this plan specifies architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure; zero code and zero source modifications result from this phase.

---\n

### 2. Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DOWNSTREAM IDS CONSUMERS                                 │
│  ids-session · ids-membership · ids-byzantine · ids-saga · ids-integrity · │
│  ids-resource · ids-lifecycle · ids-compute · ids-federation · ids-client · │
│  (each depends on ids-security via ids-000-aa sealed trait bounds)          │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              Phase 031: ids-security (NEW SEPARATE CRATE)                   │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │  authentication│ │ authorization  │ │ encryption     │ │ key mgmt     │ │\n│  │  mutual auth   │ │ dataset/op/    │ │ xchacha20poly  │ │ generate/    │ │\n│  │  challenge-resp│ │ identity-level │ │ chacha20poly   │ │ rotate/      │ │\n│  │  AuthManager   │ │ AuthzGrant     │ │ 1305 AEAD      │ │ revoke/dist  │ │\n│  └────────────────┘ └────────────────┘ └────────────────┘ └──────────────┘ │\n│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │ nonce mgmt     │ │ replay         │ │ channel binding│ │ regional     │ │\n│  │ NonceManager   │ │ protection     │ │ ChannelBinding │ │ boundary     │ │\n│  │                │ │ ReplayWindow   │ │ verify ct eq   │ │ RegionalBoundary│ │\n│  └────────────────┘ └────────────────┘ └────────────────┘ └──────────────┘ │\n│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │ ephemeral key  │ │ address limit  │ │ zero nonce     │ │ audit        │ │\n│  │ EphemeralKeyVal│ │ AddressLimitVal│ │ ZeroNonceVal   │ │ AuditManager │ │\n│  │                │ │                │ │                │ │ tamper-evident │ │\n│  └────────────────┘ └────────────────┘ └────────────────┘ └──────────────┘ │\n│  ┌────────────────────────────────────────────────────────────────────────┐ │\n│  │ errors.rs  — SecurityError (thiserror, categories, recoverable flags)  │ │\n│  │ types.rs   — AuthSession, AuthzGrant, EncryptedPayload, KeyInfo        │ │\n│  │ prelude.rs — re-exports                                                │ │\n│  └────────────────────────────────────────────────────────────────────────┘ │\n└─────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│            ids-000-aa (master: canonical types + sealed Security trait)     │
│            ids-crypto (leaf: ed25519, xchacha20poly1305, chacha20poly1305, │\n│            sha256, blake3; zeroize, constant-time, caller-supplied entropy)│\n└─────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│            ids-carrier (opaque Endpoint: CarrierType + Vec<u8>)             │\n│  All traffic rides Endpoint datagrams; ids-security does NOT import ids-car-│\n│  rrier (one-way dependency, carrier-agnostic by construction)               │\n└─────────────────────────────────────────────────────────────────────────────┘
```

**Design principles:**
- **Domain wrapper, not raw exposure:** every primitive is wrapped in IDS-specific newtypes (`Ed25519SigningKey`, `XChaCha20Poly1305SealKey`, `Blake3Digest`, `Sha256Digest`, `Nonce`, `KeyHandle`) so allowed-algorithm policy is enforced at the type boundary, key material is zeroized on drop, and the crate can swap implementations without touching the other 99 crates.
- **Zeroize discipline:** every key-holding type implements `zeroize::Zeroize` and `Drop` that zeroes memory on scope exit. No secret material survives beyond the lifetime of its wrapper type.
- **No RNG inside the crate:** key generation and nonce generation accept caller-provided entropy via a `CryptoRng`-like trait parameter. The caller (e.g. ids-security) supplies entropy.
- **Fail-closed semantics:** revoked keys, missing tokens, failed authorization, zero nonces, all-zeros ephemeral keys, oversized endpoint addresses all reject by default. Recoverable vs. unrecoverable errors are categorized.
- **No global security master:** each dataset/region manages its own security context through its own key material and policy.

---\n

### 3. Detailed Research Dimensions

#### 3.1 Mutual Authentication (ed25519 challenge-response)

Authentication is mutual: both parties prove possession of their ed25519 private keys. The verifier issues a fresh nonce (32 bytes, caller-supplied entropy via `getrandom`-adjacent trait), the claimant signs `(nonce || domain_separator || identity)` with its ed25519 key, and the verifier checks the signature against the enrolled public key. Domain separation strings (e.g. `ids-security/v1/auth`) bind signatures to this subsystem to prevent cross-protocol replay. `AuthManager` tracks sessions by `AuthToken` (blake3 hash of credential), enforces TTL, rejects empty and all-zeros credentials, and rotates out the previous token for an identity on re-authentication. Reference pattern: ed25519 challenge-response as used in `aegis-auth`, `identikey_auth`, `Ironseal`, and `krot-proto` — all Rust implementations signing a nonce-bound challenge payload and verifying with `ed25519-dalek::verify_strict`.

#### 3.2 Authorization (dataset/operation/identity-level permissions)

Authorization is token-based and dataset-scoped. An `AuthzGrant` binds `(AuthToken, DatasetId)` to a `Permission` derived from `OpType` (Read, Write, Delete, Replicate, Recover, Admin), with optional TTL. Admin grants subsume all permissions on a dataset. The evaluation is `(token, dataset, op) → allow/deny` with fail-closed default. `AuthzManager` tracks grant counts, denial rates, expiry, and supports revocation of individual grants. Design reference: Cedar policy language (default deny, explicit permit/forbid, ABAC attributes) and casbin-rs `{subject, object, action}` enforcement — ids-security adopts a simpler grant-table model appropriate to its sealed-trait boundary, with Cedar-style default-deny semantics and optional future ABAC extension.

#### 3.3 Data Encryption in Transit + At Rest (xchacha20poly1305, chacha20poly1305)

Encryption uses only the two allowed AEADs: `XChaCha20Poly1305` (192-bit/24-byte nonce, random nonce safe, no counter persistence needed) and `ChaCha20Poly1305` (96-bit/12-byte nonce, requires monotonic counter persistence to avoid reuse). `EncryptionManager` registers keys by `KeyId`, validates key size (32 bytes) and rejects all-zeros key material, and produces `EncryptedPayload { ciphertext, nonce, algorithm, key_id, encrypted_at_ms }`. For XChaCha20, random nonce generation is the documented default (birthday bound 2^96 messages per key). For ChaCha20, nonce counter persistence (monotonic u64 with overflow detection) is a downstream caller responsibility owned by ids-security/ids-session. Both algorithms are implemented in `chacha20poly1305` (RustCrypto, RFC 8439).

#### 3.4 Key Management (generation, rotation, revocation, distribution, recovery)

`KeyManager` owns the key lifecycle: `KeyState { Pending, Active, Rotating, Revoked, Expired }`, `KeyInfo` (key_id, state, timestamps, usage_count, max_usage, algorithm), `create_key`, `rotate_key` (derives new KeyId via blake3 of old key_id), `revoke_key`, `expire_key`, `increment_key_usage` with usage-limit expiry. `KeyRotationManager` holds `RotationSchedule` entries (next rotation at ms, interval, max rotations) and supports idle-time rotation execution with due/missed tracking. `KeyRevocationManager` maintains a grow-only revocation map of `RevocationEntry { key_id, revoked_at_ms, reason, revoked_by }` with fail-closed `is_revoked` checks. Key distribution and recovery are out-of-band concerns (enrollment key distribution via ids-security/admission; recovery keys sealed under dataset-level master keys).

#### 3.5 Regional Security Boundaries

`RegionalBoundary` checks `(source: RegionId, target: RegionId)` against a configured allowlist of region pairs, rejecting cross-region traffic not on the list. There is no global master — each region manages its own boundary policy. This is a configuration-level control enforced at the security layer before any encryption/authentication operation crosses regions.

#### 3.6 Nonce Management, Replay Protection, Zero Nonce Rejection

`NonceManager` generates nonces as `blake3_hash_32(context)`, rejects zero nonces, tracks used nonces with eviction at capacity. `ReplayProtection` wraps a `ReplayWindow` (HashSet of `[u8; 9]` op refs) with window eviction at capacity, plus `NonceManager` for `[u8; 32]` nonces. `check_op_ref` returns `Err(ReplayDetected)` on duplicate. All nonce operations are fail-closed: unknown nonce → replay detected; zero nonce → `ZeroNonceRejected`.

#### 3.7 Channel Binding (constant-time comparison)

`ChannelBinding` stores per-node bindings and verifies with `constant_time_eq` (XOR-accumulate, no early exit) to prevent timing attacks on binding comparison. Reference: `subtle` crate's `ct_eq`, and the existing `constant_time_eq` already present in `ids-000-aa/src/contracts/security.rs`.

#### 3.8 Ephemeral Key Validation, Address Limits, Zero Nonce Validation

`EphemeralKeyValidator` rejects all-zeros 32-byte public keys, weak high-bit-only keys, and duplicate validations. `AddressLimitValidator` enforces a 1024-byte maximum endpoint address size. `ZeroNonceValidator` rejects any `[u8; 32]` that is all zeros. These are lightweight preconditions that run before any cryptographic operation on untrusted input.

#### 3.9 Tamper-Evident Audit Records

`AuditManager` maintains an append-only hash chain: each `AuditEvent { event_id, node_id, event_type, detail, timestamp_ms, sequence_number, previous_hash, event_hash }` commits to the previous event's hash via `blake3_hash_32`. `verify_chain` walks the chain and fails on any broken link. Reference: `airframe_audit` (SHA-256 hash chain, RFC 6962-style), `pamoja-audit`, `Quipu-Log`, and `settled` (Merkle-tree inclusion/consistency proofs). ids-security uses a simpler hash chain (per the sealed trait contract in ids-000-aa) with optional future Merkle-tree upgrade. Audit entries are signed-context aware: the caller's `node_id` and `event_type` are bound into the hash.

#### 3.10 Security Stats & Monitoring

`SecurityStats` aggregates `auth_count`, `authz_count`, `encrypt_count`, `decrypt_count`, failure rates, denial rates, replay rates, mismatch rates, and zero-nonce rejection counts. These feed observability and alerting without exposing secrets.

---\n

### 4. Threat Model

| Threat | Mitigation |
|--------|-----------|
| Credential theft / impersonation | Mutual ed25519 challenge-response; nonce-bound signatures; short TTL auth tokens; fail-closed on invalid credential |
| Replay of auth tokens / operations | `ReplayWindow` + `NonceManager` tracking; per-op `[u8; 9]` refs; fail-closed on duplicate |
| Replay of sealed ciphertext | Nonce uniqueness enforced (XChaCha random 192-bit; ChaCha monotonic counter); replay protection at ids-security/ids-session layer |
| Key compromise | Key rotation (time- and volume-based), revocation grow-only set, fail-closed `is_revoked`, key derivation via blake3 |
| Authorization bypass | Default-deny grant table; Admin subsumes all; token+dataset+op triple checked; denial counting |
| Tampering of audit records | Hash-chained audit entries; `verify_chain` detects any modification/reordering/deletion |
| Timing attacks on comparisons | `constant_time_eq` for channel binding and all secret comparisons |
| All-zeros / weak ephemeral keys | `EphemeralKeyValidator` rejects all-zeros and high-bit-only weak keys |
| Oversized endpoint addresses | `AddressLimitValidator` 1024-byte cap, fail-closed |
| Zero nonce reuse | `reject_zero_nonce` everywhere nonces are consumed |
| Cross-region data leakage | `RegionalBoundary` allowlist, enforced before any cross-region operation |
| Insider key misuse | Audit logging of every auth/authz/encrypt/decrypt/rotate/revoke event; per-node attribution |
| Entropy exhaustion | Caller-supplied entropy via trait parameter; no OS RNG inside the crate; testable with deterministic entropy |

Critical failure modes: revoked-key acceptance (mitigated by fail-closed `is_revoked` check before every operation), nonce reuse under ChaCha20 (mitigated by monotonic counter persistence requirement documented for callers), audit chain truncation (mitigated by bounded ring buffer with max_events and tamper detection counting).

---\n

### 5. Integration with Existing Crates

| Crate | Role in Phase 031 |
|-------|-------------------|
| `ids-000-aa` | Master crate: canonical types (`NodeId`, `AuthToken`, `Credential`, `DatasetId`, `KeyId`, `OpType`, `RegionId`, `Signature`, `Endpoint`, `SecurityError`) and the sealed `Security` trait contract that ids-security implements |
| `ids-crypto` | Leaf crate (no ids-000-aa dependency): ed25519 sign/verify, xchacha20poly1305 + chacha20poly1305 AEAD, sha256, blake3 hashing, zeroize, constant-time compare; caller-supplied entropy |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); all traffic rides Endpoint datagrams; ids-security does NOT import ids-carrier |
| `ids-identity` | Provides `StableNodeId`/`RotatingNodeId` identities consumed by ids-security authentication; enrollment key distribution |
| `ids-session` | Consumer: session binding, nonce counter persistence for ChaCha20, replay protection coordination |
| `ids-membership` | Consumer: dataset/operation authorization grants, revocation coordination |
| `ids-byzantine` | Consumer: authenticated broadcast signatures, Byzantine node isolation |
| `ids-integrity` | Consumer: checksum/integrity verification integrated with security audit |

**Integration points:**
- `ids-security` implements the sealed `Security` trait from `ids-000-aa/src/contracts/security.rs` (methods: authenticate, authorize, encrypt, decrypt, rotate_key, revoke_key, audit_event, generate_nonce, replay_check, verify_channel_binding, validate_ephemeral_key, validate_endpoint_address_size, reject_zero_nonce, check_regional_boundary, stats, security_name, auth_count, authz_count, encrypt_count, decrypt_count).
- All `SecurityError` variants from the contract are implemented with `category()` and `is_recoverable()`/`suggested_action()`.
- No dependency on any other `ids-*` crate except through `ids-000-aa` sealed trait contracts (matching the workspace dependency rule).
- No networking dependencies: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

---\n

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-security` (new separate crate; depends on `ids-000-aa`, `ids-crypto`; dev deps: proptest, criterion)

```
ids-security/
├── Cargo.toml                                    # Crate manifest
├── .cargo/
│   └── config.toml                               # Build configuration
├── PLAN.md                                       # This document (plan-031.md)
├── CONTRACTS.md                                  # Sealed trait contract reference
└── src/
    ├── lib.rs                                    # Crate root, feature gates, public re-exports
    ├── constants.rs                              # Security constants (TTL defaults, limits, sizes)
    ├── error.rs                                  # SecurityError (thiserror, categories, recoverable)
    ├── types.rs                                  # AuthSession, AuthzGrant, EncryptedPayload, KeyInfo
    ├── prelude.rs                                # Re-exports
    │
    ├── authentication/                           # Mutual authentication subsystem
    │   ├── mod.rs
    │   ├── auth_manager.rs                       # AuthManager: sessions, tokens, TTL
    │   ├── auth_session.rs                       # AuthSession state machine
    │   ├── challenge.rs                          # Challenge generation/verification
    │   ├── credential.rs                         # Credential validation
    │   ├── token.rs                              # AuthToken derivation (blake3)
    │   ├── auth_types.rs                         # AuthState, session structures
    │   ├── auth_proptests.rs                     # Property tests
    │   └── auth_fuzz.rs                          # Fuzzing harness
    │
    ├── authorization/                            # Dataset/op/identity-level authz
    │   ├── mod.rs
    │   ├── authz_manager.rs                      # AuthzManager: grant table
    │   ├── authz_grant.rs                        # AuthzGrant structure
    │   ├── permission.rs                         # Permission enum (Read/Write/Delete/Replicate/Recover/Admin)
    │   ├── grant_evaluator.rs                    # (token, dataset, op) → allow/deny
    │   ├── grant_revocation.rs                   # Per-grant revocation
    │   ├── authz_types.rs                        # GrantId, AuthzDecision
    │   ├── authz_proptests.rs
    │   └── authz_fuzz.rs
    │
    ├── encryption/                               # Data encryption in transit + at rest
    │   ├── mod.rs
    │   ├── encryption_manager.rs                 # EncryptionManager: register/seal/open
    │   ├── encrypted_payload.rs                  # EncryptedPayload structure
    │   ├── algorithm.rs                          # EncryptionAlgorithm (XChaCha20Poly1305, Chacha20Poly1305)
    │   ├── key_handle.rs                         # KeyHandle newtype with zeroize
    │   ├── nonce_counter.rs                      # NonceCounter for ChaCha20 (monotonic u64 + overflow)
    │   ├── encryption_types.rs
    │   ├── encryption_proptests.rs
    │   └── encryption_fuzz.rs
    │
    ├── key_management/                           # Key lifecycle
    │   ├── mod.rs
    │   ├── key_manager.rs                        # KeyManager: create/rotate/revoke/expire
    │   ├── key_info.rs                           # KeyInfo: state, timestamps, usage
    │   ├── key_state.rs                          # KeyState enum
    │   ├── key_rotation_manager.rs               # RotationSchedule, idle rotation
    │   ├── key_revocation_manager.rs             # Grow-only revocation set
    │   ├── revocation_entry.rs                   # RevocationEntry structure
    │   ├── key_distribution.rs                   # Out-of-band distribution hooks
    │   ├── key_recovery.rs                       # Recovery key sealing hooks
    │   ├── key_types.rs
    │   ├── key_proptests.rs
    │   └── key_fuzz.rs
    │
    ├── nonce_management/                         # Nonce generation & replay protection
    │   ├── mod.rs
    │   ├── nonce_manager.rs                      # NonceManager: blake3-based, zero rejection
    │   ├── replay_protection.rs                  # ReplayProtection: window + nonce tracking
    │   ├── replay_window.rs                      # ReplayWindow: HashSet of [u8; 9] op refs
    │   ├── nonce_entry.rs                        # NonceEntry structure
    │   ├── nonce_types.rs
    │   ├── nonce_proptests.rs
    │   └── nonce_fuzz.rs
    │
    ├── channel_binding/                          # Constant-time channel binding
    │   ├── mod.rs
    │   ├── channel_binding.rs                    # ChannelBinding: per-node bindings, ct_eq
    │   ├── binding_types.rs
    │   ├── binding_proptests.rs
    │   └── binding_fuzz.rs
    │
    ├── ephemeral_key_validation/                 # Ephemeral public key validation
    │   ├── mod.rs
    │   ├── ephemeral_key_validator.rs            # Reject all-zeros, weak, duplicate
    │   ├── validator_types.rs
    │   └── validator_proptests.rs
    │
    ├── address_limit_validation/                 # Endpoint address size limit
    │   ├── mod.rs
    │   ├── address_limit_validator.rs            # 1024-byte max, fail-closed
    │   └── address_limit_proptests.rs
    │
    ├── zero_nonce_validation/                    # Zero nonce rejection
    │   ├── mod.rs
    │   ├── zero_nonce_validator.rs               # Reject all-zeros [u8; 32]
    │   └── zero_nonce_proptests.rs
    │
    ├── regional_boundary/                        # Regional security boundaries
    │   ├── mod.rs
    │   ├── regional_boundary.rs                  # RegionId pair allowlist
    │   ├── boundary_types.rs
    │   └── boundary_proptests.rs
    │
    ├── audit/                                    # Tamper-evident audit records
    │   ├── mod.rs
    │   ├── audit_manager.rs                      # AuditManager: hash-chained events
    │   ├── audit_event.rs                        # AuditEvent structure
    │   ├── audit_chain.rs                        # Chain verification
    │   ├── audit_types.rs
    │   ├── audit_proptests.rs
    │   └── audit_fuzz.rs
    │
    ├── security_stats/                           # Security metrics
    │   ├── mod.rs
    │   ├── security_stats.rs                     # Stats aggregation
    │   ├── security_monitor.rs                   # Monitoring hooks
    │   └── stats_proptests.rs
    │
    ├── security_validator/                       # Composite security validation
    │   ├── mod.rs
    │   ├── security_validator.rs                 # Orchestrates all validators
    │   └── validator_proptests.rs
    │
    ├── fuzz/                                     # Top-level fuzzing
    │   ├── mod.rs
    │   ├── fuzz_authentication.rs
    │   ├── fuzz_authorization.rs
    │   ├── fuzz_encryption.rs
    │   ├── fuzz_key_management.rs
    │   ├── fuzz_nonce.rs
    │   ├── fuzz_audit.rs
    │   └── fuzz_full.rs
    │
    ├── tests/                                    # Integration tests
    │   ├── mod.rs
    │   ├── integration_tests.rs                  # Cross-subsystem flows
    │   ├── mutual_auth_tests.rs                  # Challenge-response end-to-end
    │   ├── authorization_tests.rs                # Grant/deny scenarios
    │   ├── encryption_tests.rs                   # Seal/open round-trip
    │   ├── key_lifecycle_tests.rs                # Rotate/revoke/expire
    │   ├── replay_tests.rs                       # Replay attack scenarios
    │   ├── audit_tests.rs                        # Chain tamper detection
    │   ├── adversarial_tests.rs                  # All-zeros, zero nonce, oversized address
    │   ├── property_tests.rs                     # Proptest suites
    │   └── chaos_tests.rs                        # Token expiry, key rotation mid-flow
    │
    ├── benches/                                  # Benchmarks
    │   ├── mod.rs
    │   ├── auth_bench.rs                         # Authentication latency
    │   ├── authz_bench.rs                        # Authorization decision latency
    │   ├── encrypt_bench.rs                      # Seal/open throughput
    │   ├── key_bench.rs                          # Key rotation/revocation
    │   └── audit_bench.rs                        # Chain append/verify
    │
    ├── spec/                                     # Formal specifications
    │   ├── security_contract.md                  # Sealed trait contract restatement
    │   ├── auth_protocol.md                      # Mutual auth protocol spec
    │   ├── key_lifecycle.md                      # Key state machine spec
    │   ├── audit_chain.md                        # Hash chain spec
    │   └── threat_model.md                       # Threat model document
    │
    ├── docs/                                     # Documentation
    │   ├── architecture.md                       # Security architecture
    │   ├── integration_guide.md                  # Integrating with ids-security
    │   ├── carrier_agnostic.md                   # Carrier-agnostic design rationale
    │   ├── crypto_policy.md                      # Allowed-crypto policy
    │   └── api_docs.md                           # Public API documentation
    │
    └── test_vectors/                             # Deterministic test vectors
        ├── auth_vectors.json
        ├── encryption_vectors.json
        ├── key_rotation_vectors.json
        └── audit_chain_vectors.json
```

**Why ~90–1000 files:** one file per responsibility across 16 subsystems (authentication, authorization, encryption, key management, nonce management, channel binding, ephemeral key validation, address limit validation, zero nonce validation, regional boundary, audit, security stats, security validator, fuzz, tests, benches) plus spec, docs, and test vectors. Every file holds production-grade code with error handling, documentation, and tests. No stubs. The file count target is a ceiling; implementation produces as many files as subsystems warrant, all in subfolders.

---\n

### 7. Key Algorithms & Data Structures

#### 7.1 Mutual Authentication (ed25519 challenge-response)

```
challenge = random_32_bytes(caller_entropy)
payload   = domain_separator || nonce || claimant_identity
signature = ed25519_sign(claimant_private_key, payload)
valid     = ed25519_verify(claimant_public_key, payload, signature)  // verify_strict
token     = blake3_hash_32(credential)                               // AuthToken
session   = AuthSession{identity, token, state, ttl}
```

#### 7.2 Authorization (grant table)

```
AuthzGrant = {token, dataset_id, permission, granted_at_ms, expires_at_ms, grant_count}
authorize(token, dataset, op, at_ms):
  grant = grants[(token, dataset)]
  allow iff grant.valid(at_ms) && (grant.permission == Admin || grant.permission == op)
  default: deny (AuthorizationDenied)
```

#### 7.3 Encryption (xchacha20poly1305 / chacha20poly1305)

```
encrypt(data, key_id):
  key = keys[key_id]  // 32 bytes, validated non-zero
  nonce = random_24_bytes()        // XChaCha20
       or counter_12_bytes()        // ChaCha20 (caller persists counter)
  ciphertext = AEAD_seal(key, nonce, data)
  return EncryptedPayload{ciphertext, nonce, algorithm, key_id, at_ms}

decrypt(payload, key_id):
  key = keys[key_id]
  plaintext = AEAD_open(key, payload.nonce, payload.ciphertext)
```

#### 7.4 Key lifecycle state machine

```
Pending → Active (create_key) → Rotating (rotate_key, derives new KeyId via blake3)
Active → Revoked (revoke_key) → terminal
Active → Expired (expire_key or usage_limit) → terminal
Rotating → Active (new key activated)
is_revoked(key_id) → fail-closed check against grow-only revocation set
```

#### 7.5 Audit hash chain

```
AuditEvent = {event_id, node_id, event_type, detail, timestamp_ms, sequence_number,
              previous_hash, event_hash}
event_hash = blake3_hash_32(node_id || event_type || detail || timestamp_ms || sequence_number)
event_id   = blake3_hash_16(event_hash)
verify_chain: for each event, event.previous_hash == prior event_hash; else tamper detected
```

#### 7.6 Constant-time comparison

```
constant_time_eq(a, b):
  if len(a) != len(b): return false
  result = 0
  for (x, y) in zip(a, b): result |= x ^ y
  return result == 0
```

#### 7.7 Replay protection

```
check_op_ref(op_ref: [u8; 9]):
  if op_refs.contains(op_ref): return Err(ReplayDetected)
  evict half window if at capacity
  op_refs.insert(op_ref); return Ok(true)
```

---\n

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types + sealed `Security` trait contract |
| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3; zeroize; constant-time; caller-supplied entropy |
| `zeroize` | Secure memory wiping of key material on drop |
| `thiserror` | Error types with categories |
| `serde` + `serde_bytes` | Serialization |
| `proptest` | Property-based testing |
| `criterion` | Benchmarks |
| `getrandom` | Caller-side entropy (dev/test only; not imported by ids-security itself) |

Allowed crypto only: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3. No other cryptographic algorithms. No networking crates (libp2p, TCP/IP, QUIC, HTTP, DNS, etc.) — all traffic via `ids-carrier::Endpoint`.

---\n

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-031.1 | Mutual authentication: valid ed25519 signature over nonce-bound challenge required; zero accepted authentications without verification | Unit test + audit-log assertion |
| AC-031.2 | Authorization default-deny: `(token, dataset, op)` without a valid grant is denied; Admin subsumes all | Unit + adversarial tests |
| AC-031.3 | Encryption: seal/open round-trip is byte-identical; wrong key or tampered ciphertext fails | Unit test |
| AC-031.4 | Key rotation: `rotate_key` returns new KeyId; old key revoked; new key encrypts/decrypts | Unit test |
| AC-031.5 | Key revocation: `is_revoked` fail-closed; revoked key cannot encrypt/decrypt | Unit test |
| AC-031.6 | Replay resistance: duplicate op ref rejected; replay cache bounded with eviction | Test: replay after insertion and beyond capacity |
| AC-031.7 | Zero nonce rejection: all-zeros `[u8; 32]` rejected everywhere | Unit test |
| AC-031.8 | Ephemeral key validation: all-zeros and high-bit-only weak keys rejected | Unit test |
| AC-031.9 | Address limit: endpoint address > 1024 bytes rejected | Unit test |
| AC-031.10 | Channel binding: constant-time comparison; mismatch detected | Unit test |
| AC-031.11 | Audit chain: any modification/reordering/deletion breaks `verify_chain` | Tamper test |
| AC-031.12 | Carrier agnosticism: compiles with zero direct dependencies on TCP/IP, UDP, QUIC, HTTP, DNS, libp2p; all traffic via `ids-carrier::Endpoint` | Dependency audit + compile check |
| AC-031.13 | Allowed crypto only: no algorithm outside {ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3} | Dependency audit |
| AC-031.14 | Zeroize: key material wiped on Drop | Test: memory zeroed after scope exit |
| AC-031.15 | Error categories + recoverable flags correct for all `SecurityError` variants | Unit test |
| AC-031.16 | Property tests: 100,000+ iterations across auth/authz/encryption/nonce/audit | proptest |
| AC-031.17 | Adversarial tests: all-credentials, zero nonces, oversized addresses, replay, tampered audit | pass/fail |
| AC-031.18 | Performance: authentication + authorization decision within configurable budget (e.g. ≤1 ms on reference hardware) | Benchmark |

---\n

### 10. Remaining Uncertainties

1. **Nonce counter persistence for ChaCha20Pol1305:** The monotonic u64 counter must persist across restarts to avoid nonce reuse; storage ownership (ids-security vs. ids-session) and persistence mechanism are undefined.
2. **Key distribution and recovery:** Out-of-band enrollment key distribution and recovery key sealing are referenced but not designed; deferred to integration with ids-identity/admission.
3. **Audit storage backend:** In-memory ring buffer (per sealed trait contract) vs. durable append-only store; durability requirements for multi-day audit retention undefined.
4. **Merkle-tree upgrade for audit:** Hash chain is the baseline; Merkle inclusion/consistency proofs (RFC 6962 style) are a future enhancement, not in this phase.
5. **Regional boundary configuration source:** Static config vs. exchanged out-of-band vs. learned; all must be supported behind one interface, source undecided.
6. **Authentication session storage:** `RefCell<HashMap>` baseline is single-threaded; `Arc<DashMap>` or `parking_lot` for multi-threaded deployment is a deployment decision.
7. **ChaCha20 vs. XChaCha20 default:** XChaCha20 (random 192-bit nonce) is safer by default; ChaCha20 (counter nonce) requires counter persistence. Default algorithm selection for `EncryptionManager` is undecided.
8. **Formal verification scope:** No mechanized proof of the security contract planned for this phase; property tests are the verification mechanism.

---\n

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-security`, not subfolder | User requires separate crates for new capabilities; keeps `ids-000-aa` focused on types/contracts |
| Implements sealed `Security` trait from `ids-000-aa` | Single canonical contract; downstream crates program to the trait, not the crate |
| Depends on `ids-crypto` (leaf, no ids-000-aa) | Avoids circular dependency; `ids-crypto` is the only crate not depending on `ids-000-aa` |
| Allowed crypto only: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | Matches ids-crypto allowed-primitive policy; no algorithm agility beyond the set |
| Carrier-agnostic via `ids-carrier::Endpoint` only | No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh |
| Caller-supplied entropy (no RNG inside crate) | Pure, testable, runtime-agnostic; caller supplies entropy |
| Zeroize on all key-holding types | Key material wiped on Drop; no secret survives wrapper lifetime |
| Default-deny authorization | Explicit permit required; fail-closed on missing grant |
| Hash-chained audit (not Merkle yet) | Matches sealed trait contract in ids-000-aa; Merkle upgrade deferred |
| XChaCha20 random nonce default; ChaCha20 counter nonce caller-persisted | XChaCha20 avoids nonce-reuse risk; ChaCha20 counter persistence is downstream concern |
| Fail-closed everywhere: revoked keys, zero nonces, all-zeros ephemeral keys, oversized addresses | Security defaults to rejection, not acceptance |
| No global security master | Each dataset/region manages its own security context |

---\n

### 12. Sources

- ed25519-dalek — Ed25519 key generation, signing, verification: https://docs.rs/ed25519-dalek/latest/ed25519_dalek
- chacha20poly1305 — ChaCha20Poly1305 and XChaCha20Poly1305 AEAD (RFC 8439): https://docs.rs/chacha20poly1305/latest/chacha20poly1305
- blake3 — BLAKE3 cryptographic hash: https://docs.rs/blake3/latest/blake3
- sha2 — SHA-256 implementation: https://docs.rs/sha2/latest/sha2
- zeroize — Secure memory zeroing: https://docs.rs/crate/zeroize/latest
- getrandom — OS entropy source: https://github.com/rust-random/getrandom
- subtle — Constant-time comparison (ct_eq): https://docs.rs/subtle/latest/subtle
- aegis-auth — Ed25519 challenge-response authentication pattern: https://lib.rs/crates/aegis-auth
- identikey_auth — Challenge/response auth with hardware-enclave keys: https://docs.rs/identikey-auth/latest/identikey_auth
- krot-proto — Domain-separated ed25519 challenge signing: https://docs.rs/krot-proto/latest/src/krot_proto/auth.rs.html
- Cedar — Authorization policy language (default deny, ABAC/RBAC): https://arxiv.org/html/2403.04651
- casbin-rs — `{subject, object, action}` access control enforcement: https://github.com/apache/casbin-rs
- airframe_audit — Append-only SHA-256 hash-chain audit log: https://airframe_audit (lib.rs)
- pamoja-audit — Signed, hash-chained tamper-evident audit log: https://lib.rs/crates/pamoja-audit
- Quipu-Log — RFC 6962 Merkle tree audit logging: https://github.com/quipu-log/Quipu-Log
- settled — Merkle tree inclusion/consistency proofs, Ed25519 signed tree heads: https://github.com/richardadalton/settled
- spake2 — PAKE for mutual authentication over weak passwords: https://docs.rs/spake2
- commonware-p2p authenticated — ChaCha20-Poly1305 stream encryption, no TLS: https://docs.rs/commonware-p2p/0.0.17/commonware_p2p/authenticated
- XChaCha20Poly1305 nonce safety — 192-bit nonce random generation: https://docs.rs/ic-cipher/latest/ic_cipher/chacha/struct.XChaCha20Poly1305.html
- ChaCha20-Poly1305 nonce reuse warning: https://docs.rs/chacha20poly1305/0.10.1/src/chacha20poly1305/lib.rs.html
- base16ct / subtle-encoding — Constant-time hex encoding: https://docs.rs/base16ct/latest/base16ct
- rust-crate-audits / cargo-vet — Cryptography auditing criteria: https://github.com/google/rust-crate-audits

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 031.
