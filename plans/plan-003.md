## Phase 003 — Create the ids-crypto crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 003 creates a **new separate crate `ids-crypto`** (not folders inside an existing crate), organized into subfolders by subsystem. It is the IDS cryptographic primitive layer implementing **only** the allowed primitives: ed25519 (signing/verification), xchacha20poly1305 and chacha20poly1305 (symmetric authenticated encryption), sha256 and blake3 (hashing). It provides key generation, signing, verification, symmetric encryption/decryption, hashing, key derivation, nonce management, key rotation, key revocation, and constant-time comparisons — all exposed behind a narrow, domain-specific API surface so callers never touch raw algorithm primitives directly.

**Critical structural rule:** `ids-crypto` is the **only** IDS crate (of 100) that does **NOT** depend on `ids-000-aa`. This avoids a circular dependency: `ids-000-aa` would otherwise need crypto types for its canonical `Endpoint`/`CarrierType`, and `ids-crypto` would need `ids-000-aa` types. `ids-crypto` is a leaf crate with zero dependencies on any other IDS crate. All downstream crates (ids-identity, ids-security, ids-session, etc.) depend on `ids-crypto` directly.

**No networking dependencies:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere in the crate. `ids-crypto` has no concept of a network address, socket, or carrier; all traffic that eventually crosses a wire rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) in the carrier crate, and `ids-crypto` merely produces/consumes the byte payloads sealed inside those datagrams.

---

### 2. Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                    DOWNSTREAM IDS CRATES                            │
│  ids-identity · ids-security · ids-session · ids-membership · …     │
│  (each depends directly on ids-crypto; none depend on each other    │
│   for crypto)                                                       │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│            Phase 003: ids-crypto (NEW LEAF CRATE)                   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────┐ │
│  │  ed25519     │ │  xchacha20   │ │  sha256      │ │  blake3    │ │
│  │  sign/verify │ │  poly1305    │ │  hash        │ │  hash/KDF  │ │
│  │  key gen     │ │  AEAD seal/  │ │  digest      │ │  keyed/    │ │
│  │  constant-time│ │  open        │ │  constant-time│ │  derive    │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────┐ │
│  │ chacha20poly │ │ key derivation│ │ nonce mgmt   │ │ key rotation││
│  │ 1305 AEAD    │ │  (HKDF + BLAKE3│ │  generation/ │ │  revocation││
│  │  seal/open   │ │  derive_key)  │ │  tracking/   │ │  grow-only ││
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────┘ │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  constant_time.rs — ct_eq, ct_select, zeroize on drop          │ │
│  │  errors.rs — CryptoError (thiserror, no fallback to panic)     │ │
│  │  prelude.rs — re-exports                                        │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│            ids-carrier (opaque Endpoint: CarrierType + Vec<u8>)      │
│  send_datagram / receive_datagram — carries sealed ids-crypto       │
│  payloads; ids-crypto does NOT import ids-carrier (one-way dep)      │
└─────────────────────────────────────────────────────────────────────┘
```

**Design principle — domain wrapper, not raw exposure:** `ids-crypto` does not re-export `ed25519-dalek::SigningKey` or `aead::Aead` traits directly into the IDS codebase. Every primitive is wrapped in an IDS-specific newtype (`Ed25519SigningKey`, `XChaCha20Poly1305SealKey`, `Blake3Digest`, `Sha256Digest`, `Nonce`, `KeyHandle`) so that (a) allowed-algorithm policy is enforced at the type boundary, (b) key material is zeroized on drop, (c) nonce uniqueness rules are encoded in types rather than comments, and (d) the crate can swap implementations without touching the other 99 crates.

**Zeroize discipline:** Every key-holding type implements `zeroize::Zeroize` and `Drop` that zeroes memory on scope exit. No secret material survives beyond the lifetime of its wrapper type.

**No RNG inside the crate:** `ids-crypto` does not import `getrandom`, `rand`, or any OS entropy source. Key generation and nonce generation accept caller-provided entropy via a `CryptoRng`-like trait parameter (or take a `&mut impl RngCore + CryptoRng`). This keeps the crate pure, testable, and carrier-/runtime-agnostic. The caller (e.g. ids-identity or ids-security) supplies entropy.

---

### 3. Detailed Research Dimensions

#### 3.1 ed25519 — signatures and key generation

**Research basis:** RFC 8032 specifies Ed25519 as a pure EdDSA over Curve25519 with SHA-512. Signatures are deterministic: `r = SHA-512(prefix || message)`, `s = (r + k·a) mod L`. Key generation: 32-byte random seed → SHA-512 → clamp (clear bits 0,1,2 of byte 0; clear bit 7 of byte 31; set bit 6 of byte 31) → scalar `s` → public key `A = [s]B`. Verification checks `S·B = R + k·A` where `k = SHA-512(R || A || M)`. Verification must reject non-canonical `S >= L`, small-order points, and malformed encodings (strict checking per ZIP-215 / RFC 8032 §5.1.7).

**ids-crypto decision:** Wrap ed25519 in `Ed25519SigningKey` / `Ed25519VerifyingKey` newtypes. Key generation takes caller entropy (32 bytes). Signing is deterministic per RFC 8032 (no per-message randomness beyond the key seed). Verification uses strict checks: public key on-curve, `S < L`, `R` decodes, cofactorless equation holds. Context strings (Ed25519ctx) supported for domain separation between IDS subsystems (e.g. "ids-identity/v1/mapping", "ids-security/v1/revocation"). Ed25519ph (prehashed) explicitly rejected — prehash variants are deprecated in RFC 8032 §8.5 and add collision risk.

#### 3.2 xchacha20poly1305 — extended-nonce AEAD

**Research basis:** XChaCha20-Poly1305 (draft-irtf-cfrg-xchacha, RFC 8439 family) extends ChaCha20-Poly1305's nonce from 96 bits to 192 bits (24 bytes) by deriving a subkey via HChaCha20 from the first 16 bytes of the nonce, then delegating to standard ChaCha20-Poly1305 with the remaining 8 bytes as the 64-bit nonce. Because the nonce is 192 bits, random per-message nonces are safe: birthday-bound collision probability reaches 50% only after ~2^96 messages (~7.3×10^28), effectively infinite for any practical key lifetime. Standard ChaCha20-Poly1305's 96-bit nonce is too short for safe random generation (collision probable after ~2^48 messages ≈ 9 days at 10^9 msgs/sec).

**ids-crypto decision:** `ids-crypto` exposes `XChaCha20Poly1305Seal` / `XChaCha20Poly1305Open` as the **primary** symmetric AEAD, with random 24-byte nonces generated by the caller. `ChaCha20Poly1305` (96-bit nonce) is offered only as a secondary, stateful path requiring caller-managed monotonic counters (documented risk: counter persistence across restarts). Tag size 16 bytes (Poly1305). Key size 32 bytes. AAD supported. Decryption verifies tag before releasing plaintext (fail closed). Nonce reuse with the same key is fatal (reveals plaintext XOR + Poly1305 key recovery) — documented as a hard precondition, enforced by type design where possible.

#### 3.3 chacha20poly1305 — standard-nonce AEAD

**Research basis:** RFC 8439 / RFC 7539 define ChaCha20-Poly1305 AEAD: 256-bit key, 96-bit nonce, 32-bit block counter (counter 0 reserved for Poly1305 one-time key derivation), 256 GB max plaintext per key, 16-byte tag. Nonce uniqueness is mandatory; reuse breaks both confidentiality (keystream reuse → plaintext XOR) and authenticity (one-time Poly1305 key reuse → universal forgery).

**ids-crypto decision:** Secondary AEAD path behind `ChaCha20Poly1305Seal` / `ChaCha20Poly1305Open`. Requires caller to supply a 12-byte nonce guaranteed unique per key (counter recommended; random only with documented birthday limits). Max message size 256 GB enforced. Not default because random-nonce safety is the IDS design goal.

#### 3.4 sha256 — cryptographic hash

**Research basis:** FIPS 180-4 Secure Hash Standard. SHA-256 produces a 256-bit digest via serial Merkle-Damgård over 512-bit blocks. 128-bit collision resistance, 256-bit preimage resistance. Serial chain limits throughput to single-core performance; no length-extension immunity (SHA-256 is vulnerable to length-extension attacks).

**ids-crypto decision:** `Sha256Digest` newtype wrapping `sha2::Sha256` (RustCrypto Digest trait). Used where FIPS-aligned hashing is required (e.g. compatibility with external systems, content addressing that must be SHA-256). Not used for per-message authentication where BLAKE3 keyed mode suffices.

#### 3.5 blake3 — hash, keyed MAC, KDF, XOF

**Research basis:** BLAKE3 is a single algorithm based on BLAKE2 + Bao tree mode. Three domain-separated modes: `hash` (unkeyed), `keyed_hash` (32-byte key → PRF/MAC), `derive_key` (context string + key material → KDF). All modes support extendable output (XOF) of arbitrary length. Default 256-bit output. 128-bit collision resistance, 256-bit preimage resistance. Immune to length-extension by design (tree structure, domain separation). Highly parallelizable (SIMD, multi-threaded), typically 3–12× faster than SHA-256 on large inputs; slightly slower only on very small (<4 KiB) batched inputs.

**ids-crypto decision:** `Blake3Digest` is the **default** hash for IDS-internal use: content fingerprints, mapping-table integrity, nonce derivation, key derivation context. `blake3::derive_key(context, key_material)` used for IDS key derivation (replaces HKDF where BLAKE3's context-committed KDF is sufficient). `blake3::keyed_hash` used for MAC where HMAC-SHA256 would otherwise appear. HKDF-SHA256 retained as the explicit KDF option when a standard KDF is required (see 3.6).

#### 3.6 Key derivation — HKDF-SHA256 and BLAKE3 derive_key

**Research basis:** RFC 5869 HKDF: Extract (HMAC(salt, IKM) → PRK) + Expand (PRK + info + counter → OKM of arbitrary length). Conservative, hash-agnostic, widely deployed (TLS 1.3, Signal, MLS, CMS). BLAKE3 `derive_key(context, key_material)` provides context-committed KDF in one call with extensible output; context string should be hardcoded, globally unique, application-specific ("[application] [timestamp] [purpose]").

**ids-crypto decision:** Two KDF entry points:
- `hkdf_sha256_derive(ikm, salt, info, length)` — when standards compliance (RFC 5869) or interoperability is needed.
- `blake3_derive_key(context, key_material, length)` — default IDS-internal KDF; context strings namespaced per subsystem ("ids-crypto/v1/session-key", "ids-crypto/v1/identity-seed", etc.).
Both output key material into zeroizing `Zeroizing<Vec<u8>>` or fixed-size array types.

#### 3.7 Nonce generation and management

**Research basis:** ChaCha20-Poly1305 (96-bit nonce): random nonce collision probable after ~2^48 messages under one key — unsafe for random generation; requires monotonic counter with crash-recovery persistence. XChaCha20-Poly1305 (192-bit nonce): random per-message nonce safe (2^96 birthday bound). Nonce must never repeat with the same key; nonce is not secret and may be sent alongside ciphertext.

**ids-crypto decision:** `Nonce` newtype, size-parameterized (`Nonce96` for ChaCha20Poly1305, `Nonce192` for XChaCha20Poly1305). Generation: caller supplies entropy; `ids-crypto` fills bytes and returns the typed nonce. For XChaCha20, random generation is the documented default. For ChaCha20, a `NonceCounter` type wraps a monotonic u64 with overflow detection; persistence across restarts is the caller's responsibility (ids-security or ids-session owns counter storage). Nonce tracking (used-nonce set) is out of scope for `ids-crypto` — it is a primitive layer; replay/nonce-tracking lives in ids-security/ids-session.

#### 3.8 Key rotation and key revocation

**Research basis:** OWASP Key Management Cheat Sheet / NIST SP 800-57: symmetric data encryption keys capped at ~2 years for low-volume, days to weeks for high-volume/link encryption. Usage-based limits (e.g. AES-GCM 2^32 IVs) apply by volume, not just calendar. Key rotation must account for both time and volume. Grow-only revocation sets (G-Set, CRDT-style): revocation is append-only, idempotent, no "un-revoke"; gossip-propagated; compromised identity gets a fresh key rather than un-revoking (Agent Manifest x0x pattern; GNUEG revocation flooding + set reconciliation).

**ids-crypto decision:** `KeyHandle` carries a version/epoch u64. `KeyRotation` module: rotate-on-time and rotate-on-volume policies, configurable thresholds (default symmetric key rotation after configurable message count or time window; ed25519 signing keys longer-lived). `RevocationStore`: grow-only set of revoked key identifiers (blake3 fingerprint of public key), each entry signed by the issuer (ed25519). `is_revoked(key_id)` → fail-closed check. Revocation distribution (gossip over ids-carrier::Endpoint) lives in ids-identity/ids-security; `ids-crypto` provides the signed revocation record type, serialization, and verification primitive only.

#### 3.9 Constant-time comparisons

**Research basis:** `memcmp()` / `==` exit early on first byte difference → timing side-channel enables byte-by-byte secret recovery (MAC forgery, token guessing). Constant-time comparison: XOR all bytes, OR into accumulator, single branch on final result. Rust `subtle::ConstantTimeEq` / `zeroize` are the established patterns; CVE-2023-49092 (Marvin) and RustSec advisories show real-world impact of non-constant-time crypto code.

**ids-crypto decision:** `constant_time` module: `ct_eq(a, b) -> bool` (constant-time equality for same-length slices; length mismatch returns false without leaking which length), `ct_select(cond, a, b)` (constant-time conditional move), `ct_digest_eq(a, b)` for digest comparison. All signature verification, MAC tag verification, and key fingerprint comparison route through these. No `==` on secret material anywhere in the crate.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Nonce reuse with same key (ChaCha20-Poly1305 96-bit) | XChaCha20-Poly1305 default with 192-bit random nonces; ChaCha20-Poly1305 requires caller-managed counter; overflow detection on NonceCounter |
| Poly1305 key recovery from nonce reuse | Documented as fatal; type design makes nonce reuse hard to accidentally do; key handle ties nonce space to key epoch |
| Signature forgery via weak/malleable signatures | Strict ed25519 verification: S < L, point decoding, cofactorless equation; ZIP-215 checks; context strings for domain separation |
| Timing attacks on comparison/verification | constant_time module everywhere; subtle-style CT eq/select; zeroize on drop |
| Secret material lingering in memory | Zeroize on all key/nonce/seed types; Drop impl zeroes; no logging of secrets |
| Key compromise with no revocation path | RevocationStore grow-only set + signed revocation records; rotation policies; fresh key on compromise (no un-revoke) |
| Collision/hash weakness | BLAKE3 default (128-bit collision, no length-extension); SHA-256 available where FIPS alignment required |
| Wrong algorithm used (policy violation) | Newtype wrappers + feature-gated primitive exposure; only 5 allowed algorithms compile in; any other crypto crate is a compile-time dependency error |
| Replay of sealed ciphertext | Out of scope for ids-crypto (primitive layer); replay protection lives in ids-security/ids-session using ids-crypto outputs |
| Key derivation without context separation | BLAKE3 derive_key requires context string; HKDF uses info parameter; both enforce domain separation by API design |
| Entropy supplied by weak caller RNG | ids-crypto takes caller entropy; documented requirement that caller uses CSPRNG; no internal RNG to fail |

---

### 5. Integration with Existing Crates

Phase 003 depends on and integrates with:

| Crate | Role in Phase 003 |
|-------|-------------------|
| `ids-crypto` | This crate — leaf, no IDS dependencies |
| `ids-carrier` | `ids-crypto` does NOT depend on ids-carrier; downstream crates place sealed ids-crypto payloads into `Endpoint(CarrierType, Vec<u8>)` datagrams |
| `ids-000-aa` | **No dependency** — ids-crypto is the only IDS crate without this dependency (avoids circular dependency) |
| `ids-identity` | Uses ed25519 for identity keys, blake3 for StableNodeId/RNI derivation, xchacha20poly1305 for at-rest encryption |
| `ids-security` | Uses all primitives for auth, encryption, key management, revocation |
| `ids-session` | Uses xchacha20poly1305/chacha20poly1305 for session encryption, ed25519 for session binding |
| `ids-membership` | Uses ed25519 for credential validation |
| All downstream | Depend directly on ids-crypto; never transitively through another IDS crate for crypto |

**Integration points:**
- `ids-crypto` produces byte payloads (signatures, ciphertexts, digests, derived keys); downstream crates embed these in `ids-carrier::Endpoint` datagrams or persist them.
- `ids-crypto` accepts entropy from callers; no `getrandom`/`rand` dependency inside the crate.
- Downstream crates never call raw `ed25519-dalek`, `aead`, `sha2`, or `blake3` directly — they go through `ids-crypto` wrappers, enforcing the allowed-algorithm policy at one boundary.
- No `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere (I-CARR-001 carrier-agnostic rule).

---

### 6. Proposed File Structure (~250–400 files, all production-grade, in subfolders)

**Crate:** `ids-crypto` (leaf crate; depends on NOTHING from the IDS workspace; external: serde, thiserror, zeroize, blake3, sha2, chacha20poly1305, ed25519, hkdf or blake3 derive_key, subtle; no networking crates, no RNG crate)

```
ids-crypto/
├── Cargo.toml                          # manifest: only allowed-crypto deps; no net/tokio/libp2p
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # algorithm constants (key sizes, nonce sizes, tag sizes)
├── error.rs                            # CryptoError (thiserror); no fallible→panic paths
├── prelude.rs                          # re-exports of all public types
├── policy.rs                           # allowed-algorithm policy enforcement (compile-time + runtime)
│
├── ed25519/                            # ed25519 signatures (RFC 8032)
│   ├── mod.rs
│   ├── keypair.rs                      # Ed25519Keypair (seed + pubkey, zeroize on drop)
│   ├── signing_key.rs                  # Ed25519SigningKey: deterministic sign, context strings
│   ├── verifying_key.rs                # Ed25519VerifyingKey: strict verify, on-curve check
│   ├── signature.rs                    # Ed25519Signature (64 bytes, canonical S < L)
│   ├── public_key.rs                   # Ed25519PublicKey (wrapped VerifyingKey)
│   ├── seed.rs                         # Ed25519Seed (32 bytes, zeroize)
│   ├── keygen.rs                       # key generation from caller entropy
│   ├── context.rs                      # Ed25519ctx context-string domain separation
│   └── verify_strict.rs                # strict verification (S<L, point decode, cofactorless)
│
├── xchacha20poly1305/                  # XChaCha20-Poly1305 AEAD (192-bit nonce, primary)
│   ├── mod.rs
│   ├── seal_key.rs                     # XChaCha20Poly1305SealKey (32-byte, zeroize)
│   ├── open_key.rs                     # XChaCha20Poly1305OpenKey
│   ├── nonce.rs                        # Nonce192 (24 bytes, random generation safe)
│   ├── seal.rs                         # seal(plaintext, aad, nonce) -> ciphertext + tag
│   ├── open.rs                         # open(ciphertext, aad, nonce) -> plaintext (fail closed)
│   ├── aead.rs                         # aead trait wrapper over raw primitive
│   └── limits.rs                       # max plaintext, per-key message limits
│
├── chacha20poly1305/                   # ChaCha20-Poly1305 AEAD (96-bit nonce, secondary)
│   ├── mod.rs
│   ├── seal_key.rs                     # ChaCha20Poly1305SealKey
│   ├── open_key.rs                     # ChaCha20Poly1305OpenKey
│   ├── nonce.rs                        # Nonce96 (12 bytes; counter required, not random)
│   ├── nonce_counter.rs                # monotonic counter with overflow detection
│   ├── seal.rs
│   ├── open.rs
│   └── limits.rs
│
├── sha256/                             # SHA-256 hashing (FIPS 180-4)
│   ├── mod.rs
│   ├── digest.rs                       # Sha256Digest (32 bytes)
│   ├── hash.rs                         # one-shot sha256(input)
│   └── incremental.rs                  # incremental hashing (Digest trait)
│
├── blake3/                             # BLAKE3 hash / keyed / derive_key / XOF
│   ├── mod.rs
│   ├── digest.rs                       # Blake3Digest (default 32 bytes)
│   ├── hash.rs                         # one-shot blake3(input)
│   ├── keyed.rs                        # blake3 keyed_hash (MAC/PRF, 32-byte key)
│   ├── derive_key.rs                   # blake3 derive_key(context, material, length)
│   ├── xof.rs                          # extendable output (arbitrary length)
│   └── fingerprint.rs                  # content fingerprint type
│
├── kdf/                                # key derivation (HKDF-SHA256 + BLAKE3)
│   ├── mod.rs
│   ├── hkdf_sha256.rs                  # RFC 5869 extract + expand
│   ├── blake3_kdf.rs                   # derive_key wrapper with context strings
│   ├── context.rs                      # KDF context string registry (namespaced)
│   └── output.rs                       # derived key output (zeroizing)
│
├── nonce/                              # nonce management (shared)
│   ├── mod.rs
│   ├── nonce.rs                        # Nonce trait + size-parameterized newtypes
│   ├── nonce96.rs
│   ├── nonce192.rs
│   ├── generator.rs                    # nonce generation from caller entropy
│   └── tracker.rs                      # used-nonce tracking interface (impl by downstream)
│
├── key_rotation/                       # key rotation policies
│   ├── mod.rs
│   ├── handle.rs                       # KeyHandle with version/epoch
│   ├── policy.rs                       # rotate-on-time / rotate-on-volume
│   ├── scheduler.rs                    # rotation scheduler interface
│   └── limits.rs                       # per-key message/byte limits
│
├── key_revocation/                     # key revocation primitives
│   ├── mod.rs
│   ├── store.rs                        # grow-only revocation set (G-Set)
│   ├── record.rs                       # signed revocation record (ed25519 over key_id)
│   ├── fingerprint.rs                  # blake3(key_id) as revocation identifier
│   ├── checker.rs                      # is_revoked → fail-closed
│   └── serialize.rs                    # signed record serialization
│
├── constant_time/                      # side-channel-resistant primitives
│   ├── mod.rs
│   ├── eq.rs                           # ct_eq (constant-time equality)
│   ├── select.rs                       # ct_select (constant-time conditional move)
│   ├── digest_eq.rs                    # constant-time digest comparison
│   └── zeroize.rs                      # zeroize wrappers + Drop zeroing
│
├── types/                              # shared IDS crypto types
│   ├── mod.rs
│   ├── key_id.rs                       # KeyId (blake3 fingerprint of public key)
│   ├── key_material.rs                 # KeyMaterial (zeroizing Vec<u8>)
│   ├── sealed_box.rs                   # SealedBox (ciphertext + nonce + tag envelope)
│   └── envelope.rs                     # wire envelope for carrier transport
│
├── wire/                               # serialization for carrier datagrams
│   ├── mod.rs
│   ├── format.rs                       # versioned wire envelope
│   ├── encoding.rs                     # deterministic encoding (CBOR/bincode decision deferred)
│   └── checksum.rs                     # payload checksums (blake3/sha256)
│
├── fuzz/                               # fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_ed25519.rs
│   ├── fuzz_aead.rs
│   ├── fuzz_hash.rs
│   └── fuzz_kdf.rs
│
├── tests/                              # unit + integration + property tests
│   ├── mod.rs
│   ├── ed25519_tests.rs                # sign/verify round-trip, test vectors (RFC 8032)
│   ├── aead_tests.rs                   # seal/open round-trip, nonce misuse rejection
│   ├── hash_tests.rs                   # sha256/blake3 test vectors
│   ├── kdf_tests.rs                    # HKDF / BLAKE3 derive test vectors (RFC 5869)
│   ├── constant_time_tests.rs          # ct_eq timing behavior
│   ├── zeroize_tests.rs                # memory zeroed on drop
│   ├── rotation_tests.rs               # rotation policy enforcement
│   ├── revocation_tests.rs             # grow-only set, fail-closed check
│   ├── adversarial_tests.rs            # forged sig, replayed nonce, tampered ciphertext
│   └── policy_tests.rs                 # disallowed algorithm rejection
│
├── benches/                            # benchmarks
│   ├── mod.rs
│   ├── ed25519_bench.rs                # sign/verify throughput
│   ├── aead_bench.rs                   # seal/open throughput per primitive
│   ├── hash_bench.rs                   # sha256 vs blake3 throughput
│   └── kdf_bench.rs                    # derivation latency
│
├── spec/                               # formal specifications
│   ├── ed25519_spec.md
│   ├── aead_spec.md
│   ├── hash_spec.md
│   ├── kdf_spec.md
│   ├── nonce_spec.md
│   ├── revocation_spec.md
│   └── tla/                            # TLA+ (in ids-doc spec area)
│
├── docs/                               # documentation
│   ├── architecture.md
│   ├── allowed_primitives.md           # why only 5 primitives
│   ├── nonce_safety.md                 # nonce usage rules per primitive
│   └── key_rotation_runbook.md
│
└── test_vectors/                       # deterministic test vectors
    ├── ed25519_vectors.json            # RFC 8032 test vectors
    ├── aead_vectors.json               # RFC 8439 test vectors
    ├── sha256_vectors.json             # NIST SHAVS test vectors
    ├── blake3_vectors.json             # BLAKE3 official test vectors
    └── kdf_vectors.json                # RFC 5869 test vectors
```

**Why ~250–400 files:** one responsibility per file; every file has full error handling, documentation, and unit tests. Covers 5 primitives × (key, seal/open or sign/verify, nonce, limits), KDF, rotation, revocation, constant-time, types, wire, fuzzing, property/adversarial tests, benchmarks, spec, docs, test vectors. Within the HX-241 "~90–1000 files in subfolders" band.

---

### 7. Key Algorithms & Data Structures

#### 7.1 ed25519 key generation (RFC 8032)
```
seed = 32 random bytes (caller-provided entropy)
h = SHA-512(seed)
s = clamp(h[0..32])     // clear bits 0,1,2 of byte 0; clear bit 7 of byte 31; set bit 6 of byte 31
A = [s]B                // scalar base mult on edwards25519
pubkey = encode(A)      // 32-byte little-endian y-coordinate + sign bit of x
keypair = (seed, pubkey) // both zeroized on drop
```

#### 7.2 ed25519 sign / verify (RFC 8032, strict)
```
sign(seed, prefix, M, context):
  h = SHA-512(seed) → (s, prefix_bytes)
  r = SHA-512(prefix_bytes || context || M) mod L
  R = [r]B
  k = SHA-512(R || A || M) mod L
  s = (r + k·s) mod L
  sig = encode(R) || encode(s)   // 64 bytes

verify(pubkey, M, sig, context):
  reject if sig not 64 bytes; reject if pubkey not on-curve (ZIP-215)
  split sig into R (32) || s (32); reject if s >= L (non-canonical)
  k = SHA-512(R || A || M) mod L
  accept iff s·B = R + k·A   (cofactorless equation)
```

#### 7.3 XChaCha20-Poly1305 seal / open (draft-irtf-cfrg-xchacha / RFC 8439)
```
seal(key, plaintext, aad, nonce192):
  subkey = HChaCha20(key, nonce192[0..16])
  subnonce = 0x00000000 || nonce192[16..24]   // 96-bit nonce for inner ChaCha20
  (ciphertext, tag) = ChaCha20Poly1305(seal, subkey, plaintext, aad, subnonce)
  return nonce192 || ciphertext || tag

open(key, nonce192 || ciphertext || tag, aad):
  subkey = HChaCha20(key, nonce192[0..16])
  subnonce = 0x00000000 || nonce192[16..24]
  plaintext = ChaCha20Poly1305(open, subkey, ciphertext, aad, subnonce, tag)
  reject if tag verification fails (constant-time compare, fail closed)
```

#### 7.4 ChaCha20-Poly1305 seal / open (RFC 8439 / RFC 7539)
```
seal(key, plaintext, aad, nonce96):
  poly_key = ChaCha20(key, counter=0, nonce96)   // first block → Poly1305 one-time key
  ciphertext = ChaCha20(key, counter=1.., nonce96) XOR plaintext
  tag = Poly1305(poly_key, aad || ciphertext || lengths)
  return ciphertext || tag   // nonce96 carried separately by caller

open: recompute tag; constant-time compare; reject on mismatch before releasing plaintext
```

#### 7.5 BLAKE3 derive_key (KDF)
```
derive_key(context, key_material, length):
  stage1 = BLAKE3_hash(IV, DERIVE_KEY_CONTEXT flag, context) → 32-byte context_key
  stage2 = BLAKE3_hash(context_key, DERIVE_KEY_MATERIAL flag, key_material) → length bytes
  return stage2[0..length]
```

#### 7.6 HKDF-SHA256 (RFC 5869)
```
extract(salt, ikm) = HMAC-SHA256(salt, ikm) → PRK
expand(PRK, info, L) = T(1)||T(2)||... where T(i) = HMAC-SHA256(PRK, T(i-1)||info||i)
                      truncate to L bytes
```

#### 7.7 Constant-time equality
```
ct_eq(a, b):
  if len(a) != len(b): return false   // length check not timing-secret (public lengths)
  acc = 0
  for i in 0..len: acc |= a[i] ^ b[i]
  return acc == 0
```

#### 7.8 Revocation record (grow-only)
```
RevocationRecord {
  key_id: [u8; 32],        // blake3(revoked public key)
  reason: RevocationReason,
  issued_at: u64,          // ids-time tick
  issuer_sig: Ed25519Signature,
}
RevocationStore: grow-only set of records; add is idempotent; is_revoked(key_id) → fail-closed
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| (none from IDS workspace) | ids-crypto is a leaf crate — depends on no other IDS crate |
| `blake3` | BLAKE3 hash, keyed_hash, derive_key (default hash for IDS) |
| `sha2` | SHA-256 (FIPS-aligned hashing where required) |
| `chacha20poly1305` | ChaCha20-Poly1305 + XChaCha20-Poly1305 AEAD (RFC 8439) |
| `ed25519` / `ed25519-dalek` | Ed25519 signatures (RFC 8032) |
| `zeroize` | key material zeroing on drop |
| `subtle` | constant-time comparison primitives |
| `thiserror` | error types |
| `serde` + `serde_bytes` | serialization |
| `hkdf` | HKDF-SHA256 (RFC 5869) — optional feature |
| `rand_core` | RngCore/CryptoRng trait bounds for caller-supplied entropy (no concrete RNG) |

No networking crates: no `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. No concrete RNG (`getrandom`, `rand`) — entropy is caller-supplied.

---

### 9. Measurable Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Only 5 primitives compile | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 only | dependency audit + compile test (any other crypto crate fails build) |
| ed25519 sign/verify | RFC 8032 test vectors pass; strict S<L check | unit test against RFC 8032 §7.1 vectors |
| XChaCha20-Poly1305 seal/open | round-trip; tag tamper rejected | unit test + adversarial test |
| ChaCha20-Poly1305 seal/open | round-trip; 96-bit nonce counter overflow detected | unit test |
| SHA-256 | NIST SHAVS test vectors | unit test |
| BLAKE3 | official BLAKE3 test vectors; keyed_hash + derive_key | unit test |
| HKDF-SHA256 | RFC 5869 test vectors | unit test |
| Constant-time equality | no early exit; same time for equal/differing | ct_eq test + timing assertion |
| Zeroize | key memory zeroed after drop | zeroize test + stack inspection |
| Nonce192 random | 24-byte random nonce safe (documented 2^96 bound) | property test (birthday bound) |
| Nonce96 counter | monotonic; overflow → error | counter test |
| Revocation store | grow-only; idempotent add; fail-closed is_revoked | unit + adversarial test |
| Key rotation | rotate-on-time and rotate-on-volume enforced | policy test |
| No net deps | compiles with --no-default-features, no tokio/net/libp2p | dependency tree check |
| Property tests | 100,000+ iterations | proptest |
| Adversarial tests | forged signatures, replayed nonces, tampered ciphertext | pass/fail |
| Deterministic test vectors | all 5 primitive vectors reproduced | test vector suite |

---

### 10. Remaining Uncertainties

1. **Concrete ed25519 crate:** `ed25519` (types only) + `ed25519-dalek` (implementation) vs `signature` traits abstraction vs a single pure-Rust crate. Decision pending implementation; dalek is the most mature with constant-time signing and verify_strict.
2. **Wire encoding:** CBOR (deterministic, self-describing) vs bincode vs protobuf for sealed envelopes in carrier datagrams. CBOR favored; crate TBD.
3. **HKDF vs BLAKE3 derive_key as default KDF:** BLAKE3 derive_key is simpler and context-committed; HKDF-SHA256 offers RFC 5869 compliance. Both provided; default TBD.
4. **Nonce counter persistence:** ChaCha20-Poly1305's 96-bit nonce requires a monotonic counter; persisting it across restarts (to avoid reuse after reboot) is a downstream (ids-security/ids-session) concern, not solved in ids-crypto.
5. **Revocation distribution:** ids-crypto provides the signed revocation record type; gossip/distribution over ids-carrier::Endpoint lives in ids-identity/ids-security — integration boundary TBD.
6. **Formal verification scope:** TLA+/PlusCal spec of nonce safety and revocation monotonicity (in ids-doc) — effort level TBD.
7. **Feature flags:** whether to make individual primitives feature-gated (e.g. no_sha256 for minimal builds) — API surface decision pending.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| ids-crypto is a leaf crate (no ids-000-aa dep) | avoids circular dependency; only IDS crate without this dependency (CRATES.md rule) |
| Domain newtypes over raw primitives | enforces allowed-algorithm policy at one boundary; zeroize on drop; implementation-swap friendly |
| XChaCha20-Poly1305 as primary AEAD | 192-bit nonce makes random per-message nonces safe; no counter state needed |
| ChaCha20-Poly1305 secondary, counter-only | 96-bit nonce unsafe for random generation; caller manages monotonic counter |
| Deterministic ed25519 (RFC 8032) | no per-message randomness; simpler, matches RFC 8032 default |
| Strict ed25519 verification (S<L, on-curve, ZIP-215) | prevents signature malleability and weak-key forgery |
| BLAKE3 default hash | faster than SHA-256, no length-extension, built-in keyed/KDF/XOF modes |
| SHA-256 available for FIPS alignment | where external compliance requires it |
| No internal RNG; caller supplies entropy | pure, testable, runtime-agnostic |
| Constant-time everywhere (ct_eq, ct_select) | side-channel resistance for all secret comparisons |
| Zeroize on all key/nonce/seed types | no secret material survives scope exit |
| Revocation = grow-only signed set | append-only, idempotent, fail-closed; fresh key on compromise (no un-revoke) |
| Context strings for ed25519ctx + KDF contexts | domain separation between IDS subsystems |
| New separate crate ids-crypto | matches CRATES.md "separate crates, not more folders" rule; ~250–400 files |

---

### 12. Sources consulted (30 targeted searches)

1. RFC 8032 — EdDSA / Ed25519 specification: https://rfc-editor.org/rfc/rfc8032.html
2. RFC 8439 — ChaCha20-Poly1305 AEAD: https://datatracker.ietf.org/doc/rfc8439
3. RFC 7539 — ChaCha20 and Poly1305 for IETF Protocols: http://datatracker.ietf.org/doc/rfc7539
4. draft-irtf-cfrg-xchacha — XChaCha20 construction (192-bit nonce): https://datatracker.ietf.org/doc/html/draft-arciszewski-xchacha-03
5. RFC 5869 — HKDF (HMAC-based Extract-and-Expand KDF): https://www.rfc-editor.org/info/rfc5869
6. FIPS 180-4 — Secure Hash Standard (SHA-256): https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
7. BLAKE3 specification (C2SP): https://github.com/C2SP/C2SP/blob/main/BLAKE3.md
8. BLAKE3 paper/specs: https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.tex
9. BLAKE3 README (keyed_hash, derive_key, XOF modes): https://github.com/BLAKE3-team/BLAKE3
10. RFC 6979 — Deterministic DSA/ECDSA (deterministic signature nonce): https://datatracker.ietf.org/doc/html/rfc6979
11. ed25519-dalek README (constant-time signing, verify_strict, zeroize): https://raw.githubusercontent.com/dalek-cryptography/curve25519-dalek/main/ed25519-dalek/README.md
12. ed25519-dalek VerifyingKey docs (strict verification, ZIP-215): https://docs.rs/ed25519-dalek/latest/ed25519_dalek/struct.VerifyingKey.html
13. chacha20poly1305 Rust crate docs (XChaCha20Poly1305, 192-bit nonce): https://docs.rs/chacha20poly1305/latest/chacha20poly1305
14. libsodium XChaCha20-Poly1305 construction (random nonces safe): https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
15. PyCryptodome ChaCha20-Poly1305/XChaCha20-Poly1305 nonce sizes: https://pycryptodome.org/src/cipher/chacha20_poly1305
16. Go x/crypto chacha20poly1305 (NonceSize 12, NonceSizeX 24): https://pkg.go.dev/golang.org/x/crypto/chacha20poly1305
17. subtle crate / constant-time comparison (side-channel resistance): https://docs.rs/subtle (referenced via zks_crypt / tacet case studies)
18. zeroize crate (zeroing key material on drop): https://docs.rs/zeroize (BitBox02 firmware write-up)
19. OWASP Key Management Cheat Sheet (key rotation cryptoperiods, usage limits): https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html
20. NIST SP 800-57 Part 1 Rev. 5 (key cryptoperiods, §5.3.6): https://csrc.nist.gov/pubs/sp/800/57/pt1/r5/final
21. GNUnet revocation (flooding + set reconciliation, grow-only): https://docs.gnunet.org/v0.20.x/developers/revocation/revocation.html
22. Agent Manifest x0x revocation (fail-closed, grow-only RevocationSet, gossip ~30s heartbeat): ADR 0018 pattern
23. BLAKE3 vs SHA-256 performance (3–12× faster on large inputs, slower on tiny): https://shattered.io/blake3-vs-sha256-2026
24. ChaCha20-Poly1305 nonce collision risk (random 96-bit unsafe after ~2^48): https://crypto.stackexchange.com/questions/60336
25. XChaCha20 random nonce safety (2^96 birthday bound): https://crypto.stackexchange.com/questions/77982
26. ed25519 key generation / clamping (RFC 8032 scalar pruning): https://jcraige.com/an-explainer-on-ed25519-clamping
27. Hedged / deterministic EdDSA side-channel considerations (draft-irtf-cfrg-det-sigs-with-noise): https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-det-sigs-with-noise-04
28. Constant-time memcmp / timing attack on string comparison: https://paragonie.com/blog/2015/11/preventing-timing-attacks-on-string-comparison-with-double-hmac-strategy
29. Symmetric key ratchets (Signal/TLS 1.3/MLS key chains): https://eprint.iacr.org/2024/220.pdf
30. ed25519 context strings / Ed25519ctx domain separation (RFC 8032 §5.1): https://rfcinfo.com/rfc-8032/5-1-ed25519ph-ed25519ctx-and-ed25519

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 003.