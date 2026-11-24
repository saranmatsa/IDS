## Phase 084 — Create the ids-reed-solomon crate: MDS erasure coding for storage efficiency.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1255-1269)

```
84. Research only do not read plan.md **[Phase 084 — Create the ids-reed-solomon crate: MDS erasure coding for storage efficiency.]**

Crate: ids-reed-solomon
Purpose: MDS erasure coding for storage efficiency.
Contents: RS (n,k) code construction, systematic encoding, decoding, any k of n reconstruction, storage overhead optimization, arithmetic constraints, finite field operations.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 084 — Create the ids-reed-solomon crate: MDS erasure coding for storage efficiency.]**

Create plan-084.md in C:/idfs/plans/ for the ids-reed-solomon crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 084 creates `ids-reed-solomon`, a **separate crate** (per the project rule: separate crates instead of folders inside existing crates) implementing Maximum Distance Separable (MDS) erasure coding via Reed-Solomon (n,k) codes over Galois fields, for storage efficiency in the IDS workspace. It divides data into `k` equal-size shards, computes `m = n − k` parity shards, and guarantees reconstruction of the original data from **any k of the n shards** — tolerating any `m` shard losses (data or parity, mixed) at storage overhead `n/k = (k+m)/k`. Examples: RS(10,4) → 1.4× overhead tolerating 4 failures; RS(14,10) → 1.4×; RS(9,6) → 1.5× (Google Colossus); RS(6,3) → 1.5× (Ceph default `reed_sol_van` k=2,m=2 profile); vs 3× replication which tolerates only 2 failures.

The crate is the implementation crate for the `Erasure` sealed trait contract declared in `ids-000-aa/src/contracts/erasure.rs`. It imports canonical types (`RecordId`, `RegionId`, `NodeId`, `Checksum`, `Endpoint`, `CarrierType`) and `ids_000_aa::Error`; it adds no new canonical types (shard identity is `RecordId` + shard index, maintained by the caller). All encoded-shard traffic is opaque `Vec<u8>` datagrams sent through `ids-carrier::Endpoint`; the crate never opens a socket and imports no networking stack. Crypto: the RS core is pure finite-field arithmetic — **not encryption** (consistent with the surveyed `reedsolomon` crate position: RS protects already-encrypted opaque bytes, carries no secrets). Optional per-shard integrity digests use only `blake3`/`sha256` from the allowed list; `ed25519`/`xchacha20poly1305`/`chacha20poly1305` are unused by the core.

Skeleton `ids-reed-solomon` does NOT exist yet at `C:/idfs/ids-reed-solomon/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned rust-rse/reed-solomon-erasure (crates.io v6.0.0, MIT; pure Rust GF(2^8)/GF(2^16), systematic encode + reconstruct any k of n; ports of Backblaze JavaReedSolomon and klauspost Go reedsolomon; SIMD C from Nicolas Trangez; seeking new maintainers — do not hard-depend), reed-solomon-simd (Leopard-RS, O(n log n), GF(2^16), runtime SIMD SSSE3/AVX2/Neon, up to 32768 shards, encode ~6–10 GiB/s), reed-solomon-engine (pure Rust GF(2^8), zero runtime deps, systematic Vandermonde, n ≤ 256 shards, documented wire format and never-mis-correct verification), Intel ISA-L (GF(2^8) block RS erasure codes, ec_encode_data/ec_decode_data, reduction polynomial 0x1d, AVX/AVX2 assembly, multibinary dispatch), jerasure + gf-complete (James Plank, UTK; Cauchy Reed-Solomon, bit-matrix coding, Vandermonde→systematic inversion tutorials; erasure ≠ error distinction), klauspost/reedsolomon (Go, Backblaze Java port, Leopard for >256 shards, >1 GB/s/core), sia_reed_solomon (Sia, Rust port of klauspost, AVX2/GFNI/NEON/WASM, production 11 years), reedsolomon (BolivarTech, #![forbid(unsafe_code)], RS(255,223) CCSDS over GF(2^8), never-mis-correct via syndrome verification, DoS-hardened try_reserve allocation), galois_field_2pm / galois_2p8 / fgf / isochronous_finite_fields (Rust GF(2^8) arithmetic; log/exp tables; constant-time option), ACM TOStorage survey 10.1145/3708994 (RS is the most widely deployed erasure-code family; (n,k); any k of n), Error Correction Zoo (MDS = Singleton bound equality d = n−k+1; corrects n−k erasures; RS is MDS; locally recoverable codes with locality r, related to Phase 085 regenerating codes), MathWorld Reed-Solomon (polynomial evaluations over F_q at n distinct elements), std::arch pclmulqdq/vpclmulqdq (carry-less multiplication intrinsics for GF(2) multiplication — SIMD reference), MinIO/Ceph deployment math (N = K + M; overhead (k+m)/k; read quorum K; 1.5× vs replication 3.0×).

---

### 2. Architecture Overview

`ids-reed-solomon` is a **pure computation crate**: finite-field arithmetic → coding matrices → encode/decode over byte shards. No I/O, no transport, no async runtime, no crypto in the core.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CALLER: ids-store / ids-recovery / ids-*               │
│  RecordId + data bytes → encode → n shards → shard i sent to NodeId i       │
│  via ids-carrier::Endpoint  |  k surviving shards → decode → original bytes │
├─────────────────────────────────────────────────────────────────────────────┤
│               Phase 084: ids-reed-solomon (THIS CRATE — pure compute)      │
│  ┌─────────────┐ ┌────────────────┐ ┌──────────────┐ ┌──────────────┐     │
│  │  field/     │ │  matrix/       │ │  codec/      │ │  shards/     │     │
│  │  GF(2^8)    │ │  Vandermonde,  │ │  systematic  │ │  Shard, shard│     │
│  │  GF(2^16)   │ │  systematic G, │ │  encode,     │ │  index vec   │     │
│  │  log/exp    │ │  Cauchy,       │ │  erase-decode│ │  buffers     │     │
│  │  tables     │ │  matrix inv    │ │  reconstruct │ │              │     │
│  └─────────────┘ └────────────────┘ └──────────────┘ └──────────────┘     │
│  ┌─────────────┐ ┌────────────────┐ ┌──────────────┐ ┌──────────────┐     │
│  │  params/    │ │  integrity/    │ │  errors.rs   │ │  lib.rs      │     │
│  │  CodecConfig│ │  blake3/sha256 │ │  ReedSolomonError│ │  re-exports│     │
│  │  (k,m,n)    │ │  shard digests │ │              │ │              │     │
│  └─────────────┘ └────────────────┘ └──────────────┘ └──────────────┘     │
└──────────────────────────────────────────┬────────────────────────────────┘
                                           │ opaque Vec<u8> datagrams
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, shard bytes)   │
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└─────────────────────────────────────────────────────────────────────────────┘
```

Design rules (non-negotiable):
- One field implementation per file; GF(2^8) default (n ≤ 256 shards), GF(2^16) optional (n ≤ 65535).
- Systematic form: first `k` shards are the data unchanged; parity computed from generator matrix `G = V · T⁻¹`.
- Any `k` of `n` shards reconstructs (MDS property); decoding inverts the k×k submatrix of surviving rows over the field.
- Encode/decode operate per byte offset across shards — independent, parallelizable, no cross-byte state.
- Shard buffers are `Vec<u8>`, all equal length; shard **order is part of the contract**: the caller tracks shard index ↔ NodeId/RegionId mapping; the crate is pure compute.
- No crypto in the core; optional shard digest (`blake3`/`sha256`) for bit-rot detection before decode.
- DoS-hardened: config validated upfront, `try_reserve` allocations (InvalidInput instead of abort on OOM), per the surveyed `reedsolomon` crate.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Reed-Solomon (n,k) MDS codes for storage
An `(n,k)` Reed-Solomon code over field F_q divides data into k symbols, produces n coded symbols, with minimum distance `d = n − k + 1` (Singleton bound equality → MDS). It tolerates exactly `n − k = m` erasures: any k of n symbols reconstruct the data. Source: Error Correction Zoo MDS entry; MathWorld Reed-Solomon; ACM TOStorage survey 10.1145/3708994. Deployment references: Google Colossus RS(9,6) → 1.5×; Facebook storage RS(14,10) → 1.4× (per John Fritsche survey citing Cauchy RS prevalence); MinIO erasure sets N = K + M with read/write quorum K; Ceph default profile k=2,m=2 (`reed_sol_van`) → overhead 1.5× vs replication 3.0×. Erasure ≠ error: storage failures are erasures (known positions), so decoding = matrix inversion only — no Berlekamp-Massey/error-locator machinery needed (Plank tutorials).

#### 3.2 Finite field arithmetic GF(2^m)
GF(2^8) is the default: one byte per symbol, n ≤ 256 distinct shard identifiers. Addition/subtraction = bitwise XOR. Multiplication via log/exp (antilog) tables built from a primitive polynomial and generator α = 2: `mul(a,b) = exp[(log[a] + log[b]) mod 255]`, with `mul(0,·) = 0`; division and inversion via table lookups. Primitive polynomials surveyed: 0x11d (x^8+x^4+x^3+x^2+1, CCSDS/QR/Reed-Solomon most common) and 0x1d (x^8+x^4+x^3+x^2+1 — ISA-L RAID-6/erasure-code reducing polynomial; note 0x11d and 0x1d share the same low bits but conventions differ by crate — pin exactly one in `field/tables.rs` and KAT-test against it). GF(2^16) optional for n up to 65535 shards (Leopard-RS / reed-solomon-simd). Rust field crates surveyed as reference: `galois_field_2pm` (lookup-table or computation-based, GFu8..GFu128), `galois_2p8` (all GF(2^8) polynomials), `fgf` (bulk packed field ops), `isochronous_finite_fields` (constant-time, branch-free, no table lookups — side-channel resistant). IDS writes its own small field core per ids-000-aa contracts, using these as cross-checks; table generation via `const fn` (pattern from `anyd` QR gf.rs).

#### 3.3 Coding matrices
Vandermonde matrix `V[r][c] = r^c` over GF elements, r = shard index (n distinct field elements). Every k×k submatrix of a Vandermonde on distinct nodes is nonsingular (determinant ∏_{a<b}(x_b − x_a) ≠ 0 in characteristic 2) — that is the MDS property: any k rows suffice. Systematic reduction: `T` = top k×k block of V; `G = V · T⁻¹` (n×k). T is a k×k Vandermonde on distinct nodes so T⁻¹ exists (Gaussian elimination over GF). Top k rows of G are the identity → systematic (data shards pass through unchanged); every k×k row submatrix of G stays invertible (invertible · invertible). Cauchy matrices (jerasure/gf-complete, Plank) are a faster-inversion alternative; noted as a later optimization, not Phase 084 baseline. Matrix inversion is one-time per (k,m,field) config; encode is per-byte dot products.

#### 3.4 Systematic encoding
Data shards `d_0..d_{k-1}` pass through unchanged. Parity `p_j = Σ_{i=0}^{k-1} G[k+j][i] · d_i` for j in 0..m. Computed per byte offset t independently: `p_j[t] = Σ_i G[k+j][i] ⊗ d_i[t]`. Dot products are SIMD-friendly (independent field muls then XOR reduction). Encode complexity O(n·k) field multiplications per byte. Memory: generator G is n×k field elements (1 byte each for GF(2^8); e.g. RS(100,10) → 1000 bytes).

#### 3.5 Erasure decoding (any k of n)
Collect any k surviving shards with known indices. Build k×k matrix A from the corresponding k rows of G over GF. Invert A via Gauss-Jordan over GF. Original data `d = A⁻¹ · survivors` applied per byte offset. Verification (never-mis-correct, per `reedsolomon` precedent): re-encode recovered data and compare parity shards; on mismatch return `Unrecoverable`, never wrong-but-plausible data. Decode complexity: O(k³) one-time inversion + O(k²) field muls per byte. Erasures-only (known positions) avoids syndrome decoding entirely — cheaper than classical errors-and-erasures RS (no Euclidean/BM algorithm for Phase 084).

#### 3.6 Storage efficiency & parameter selection
Overhead = n/k = 1 + m/k; fault tolerance = m arbitrary shard losses. Recommended configs (to be tuned at ids-store integration): RS(10,4) → 1.4×, 4 failures; RS(14,10) → 1.4×, 4 failures; RS(9,6) → 1.5×; RS(6,3) → 1.5×. Replication 3× tolerates 2 failures at 3× overhead. Larger k lowers overhead but increases decode matrix size and failure-domain width. Interaction notes (out of scope, reserved): Phase 085 `ids-regenerating-codes` builds on this crate for bandwidth-efficient node repair; locally repairable codes (locality r, Error Correction Zoo LRC entry) trade MDS optimality for local repair — future consideration only.

#### 3.7 Arithmetic constraints & limits
- GF(2^8): n ≤ 256 (one distinct field element per shard; shard indices 0..n-1).
- GF(2^16): n ≤ 65535.
- `k ≥ 1`, `m ≥ 1`, `n = k + m`.
- All shards equal length L ≥ 1 bytes; zero-length shard → InvalidConfig.
- Shard count × shard length bounds memory; caller caps stripe size.
- `n − k = m` is the erasure budget; losing > m shards → `InsufficientShards`.

#### 3.8 Existing implementations surveyed (reference only — no third-party FEC hard dependency)
| Implementation | Language | Field | SIMD | Notes |
|---|---|---|---|---|
| reed-solomon-erasure (rust-rse) | Rust | GF(2^8), GF(2^16) | C SIMD (Trangez) | Backblaze + klauspost ports; seeking maintainers |
| reed-solomon-simd | Rust | GF(2^16) | SSSE3/AVX2/Neon | Leopard-RS, O(n log n), ≤32768 shards |
| reed-solomon-engine | Rust | GF(2^8) | scalar | Pure Rust, zero deps, n ≤ 256, wire-format doc |
| reedsolomon (BolivarTech) | Rust | GF(2^8) | scalar | #![forbid(unsafe_code)], RS(255,223) CCSDS, never-mis-correct |
| klauspost/reedsolomon | Go | GF(2^8) | — | Backblaze port, Leopard >256 shards, >1 GB/s/core |
| sia_reed_solomon | Rust | GF(2^8) | AVX2/GFNI/NEON/WASM | Sia production, 11 years, wire-compatible |
| ISA-L (Intel) | C/asm | GF(2^8), poly 0x1d | AVX/AVX2 | ec_encode_data/ec_decode_data, multibinary |
| jerasure / gf-complete (Plank, UTK) | C/C++ | GF(2^w) | — | Cauchy RS, bit-matrix, field arithmetic tutorials |

Decision: ids-reed-solomon vendors its own implementation per the ids-000-aa `Erasure` contract; surveyed crates are reference/test-vector sources only (consistent with "no third-party FEC dependency unless justified" and with an unmaintained upstream risk on rust-rse).

#### 3.9 SIMD & acceleration (research, not Phase 084 delivery)
Scalar Rust first. Optional SIMD later, cfg-gated behind `std::arch`: `pclmulqdq`/`vpclmulqdq` carry-less multiplication (GF(2) polynomial multiplication, stable in std::arch since Rust 1.27/1.89), SSSE3/AVX2 (reed-solomon-simd pattern), GFNI (Intel Ice Lake+), NEON on AArch64; ISA-L multibinary dispatch as reference. std::arch intrinsics are safe wrappers — compatible with a `#![forbid(unsafe_code)]` target. Benchmark-gated before shipping (reed-solomon-engine: no-std CI check exists; criterion/divan harnesses).

#### 3.10 Error vs erasure; integrity boundary
Classical RS corrects errors (unknown symbol positions) via syndrome/locator polynomials — not needed for storage erasure. Storage path: known erasure positions → invert surviving submatrix. Optional integrity: per-shard `blake3`/`sha256` digest computed at encode, verified at decode before reconstruction (detects silent bit-rot, which is corruption not erasure). Core remains crypto-free; digest feature optional. ed25519/xchacha20poly1305/chacha20poly1305 have no role in RS coding (allowed list respected by omission).

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|---|---|
| Wrong shard order / index mismatch → garbage decode | Caller maintains shard index ↔ NodeId/RegionId mapping; shard order is part of the wire contract; decode verified by re-encode when digests enabled |
| Silent bit-rot (corruption, not erasure) → misdecode | Optional blake3/sha256 shard digest checked before decode; mismatched shards rejected; re-encode verification (never-mis-correct) |
| More than m shard losses → unrecoverable data | Codec returns `InsufficientShards`; no partial data; caller fails/replaces per policy |
| Invalid (k,m,n) or n > field limit → panic | `CodecConfig::new` validates upfront; `InvalidConfig` errors; n ≤ 256 (GF(2^8)) / n ≤ 65535 (GF(2^16)) |
| Shard length mismatch → encode/decode failure | All shards equal length L, validated upfront; `ShardLenMismatch` error |
| DoS via huge allocation on decode | `try_reserve` → `InvalidInput` instead of abort (reedsolomon pattern); max shard size cap configurable |
| Integer overflow in field arithmetic | u8/u16 field ops wrap by construction; table indices bounded mod 255/65535 |
| Dependency pulling networking/crypto into crate | Cargo.toml audited: no std::net/tokio::net/libp2p/any networking crate; crypto limited to optional blake3/sha256 digests; CI grep bans |
| ed25519/xchacha20poly1305/chacha20poly1305 creep into core | Not used by RS core; allowed list respected by omission; digest feature restricted to blake3/sha256 |
| Carrier-agnostic rule violation in crate or tests | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr` — zero matches; `cargo tree` shows no networking crates |

---

### 5. Integration with Existing Crates

- `ids-000-aa` (master, `C:/idfs/ids-000-aa/`): imports canonical types `RecordId`, `RegionId`, `NodeId`, `Checksum`, `Endpoint`, `CarrierType` and `ids_000_aa::Error`; implements the sealed `Erasure` trait contract declared in `src/contracts/erasure.rs`; adds no new canonical types (shard identity = `RecordId` + u16 shard index, owned by caller). Depends on `ids-000-aa` only.
- `ids-carrier`: encoded shards travel as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-reed-solomon` never imports a networking stack or addresses a carrier by name.
- `ids-crypto`: optional shard integrity via `blake3`/`sha256` (allowed list only); `ids-reed-solomon` does not use ed25519/xchacha20poly1305/chacha20poly1305.
- Consumers (planned, not in this phase): `ids-store` (encode on write), `ids-recovery` (reconstruct after shard loss), `ids-fragment` (shard distribution); benchmarks via `ids-bench` (Phase 048).
- `ids-regenerating-codes` (Phase 085, next): bandwidth-efficient node repair builds on `ids-reed-solomon` reconstruction — dependency direction is 085 consumes 084; repair-bandwidth interface defined at Phase 085, not here.
- Dependency graph: `ids-reed-solomon` → `ids-000-aa` (+ optional `ids-crypto` behind `integrity` feature); no reverse dependencies; no third-party FEC crate as hard dependency.

---

### 6. Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-reed-solomon/`

```
Cargo.toml                      # ids-000-aa (path); optional blake3/sha2 (feature integrity); dev: criterion, proptest
README.md
LICENSE                         # MIT or MIT/Apache-2.0 (matching surveyed implementations)
src/
  lib.rs                        # crate root; public API: ReedSolomon, CodecConfig, FieldType, encode/decode/reconstruct
  errors.rs                     # ReedSolomonError (thiserror): InvalidConfig, ShardCountExceedsFieldLimit, ShardLenMismatch, InsufficientShards, Unrecoverable, AllocationFailed
  params/
    mod.rs
    config.rs                   # CodecConfig {k, m, n, field}, validation (k≥1, m≥1, n=k+m, n ≤ field limit)
    field_type.rs               # enum FieldType { Gf256, Gf2p16 }
  field/
    mod.rs                      # GaloisField trait: add, sub, mul, div, inv, pow, generator, zero/one
    gf256.rs                    # GF(2^8): log/exp tables, primitive polynomial (pinned, 0x11d or 0x1d), generator α=2
    gf2p16.rs                   # GF(2^16): optional, larger shard counts
    tables.rs                   # const fn log/exp table generation (anyd QR gf.rs pattern); primitive polynomial constants
    arithmetic.rs               # add = xor; mul via tables; div; inv; zero handling
    poly.rs                     # generator polynomial; polynomial ops over GF
  matrix/
    mod.rs
    vandermonde.rs              # V[r][c] = r^c over field elements; distinct shard indices
    systematic.rs               # G = V·T⁻¹; top-k-block inversion via Gauss-Jordan over GF; identity top rows
    cauchy.rs                   # Cauchy matrix option (Plank/jerasure reference), later optimization
    invert.rs                   # k×k matrix inverse over GF (Gauss-Jordan)
    minor.rs                    # k×k submatrix selection from surviving shard rows
  codec/
    mod.rs
    encoder.rs                  # systematic parity; per-byte-offset dot products over shard buffers
    decoder.rs                  # erasure decode: invert surviving-row submatrix; apply to survivors
    reconstruct.rs              # any-k-of-n reconstruction entry point
    verify.rs                   # re-encode + compare (never-mis-correct guarantee)
    shard_codec.rs              # per-byte-offset loop across equal-length shard Vec<u8> buffers
  shards/
    mod.rs
    shard.rs                    # Shard { index: u16, data: Vec<u8> }
    shard_set.rs                # collection of shards; equal-length invariant enforcement
    index_map.rs                # caller-side shard index ↔ NodeId/RegionId mapping (no carrier types)
  integrity/
    mod.rs
    digest.rs                   # optional blake3/sha256 per-shard digest (allowed crypto only)
    verify_digest.rs            # digest check pre-decode
  internals/
    mod.rs
    dot_product.rs              # SIMD-ready field dot product (scalar implementation first)
    simd.rs                     # cfg-gated pclmulqdq/vpclmulqdq wrappers (std::arch), AVX2/Neon later
    dispatch.rs                 # multibinary dispatch stub (ISA-L pattern), research only
tests/                          # one file per property (~40-60 files)
  field_arithmetic.rs
  gf256_tables.rs               # KAT log/exp tables vs pinned primitive polynomial
  gf2p16_arithmetic.rs
  vandermonde_invertible.rs     # every k×k submatrix invertible (MDS property, exhaustive for small n)
  systematic_identity.rs        # top k rows of G are identity
  encode_round_trip.rs          # encode → decode all k-subsets, small (n,k), exact recovery
  reconstruct_any_k.rs          # every combination of k survivors reconstructs data, data+parity mixed loss
  parity_only_loss.rs           # losing only parity shards is trivially recoverable
  data_shard_loss.rs            # losing up to m data shards recovers from parity
  shard_order_sensitivity.rs    # permuted shard order detected / rejected
  shard_len_mismatch.rs
  zero_length_shard.rs
  invalid_config.rs
  insufficient_shards.rs
  never_miscorrect.rs           # corrupted shard → Unrecoverable, never wrong-but-plausible data
  digest_integrity.rs           # blake3/sha256 shard digest detects bit-rot
  field_limits.rs               # n ≤ 256 GF(2^8), n ≤ 65535 GF(2^16)
  do_not_panic_on_bad_input.rs
  carrier_agnostic.rs           # grep test: zero std::net/tokio::net/libp2p/IpAddr/SocketAddr
benches/                        # criterion.rs benchmarks (Phase 048 harness pattern)
  encode_throughput.rs
  decode_latency.rs
  reconstruct_latency.rs
  shard_count_scaling.rs        # n = 10..100, fixed k/m ratio
examples/
  basic_rs_10_4.rs
  reconstruct_erasures.rs
```

Count convention: src/ has ~25 module files across 8 subfolders; tests/ ~25 property files; plus benches/examples — the "~90-1000 files in subfolders" convention is met by per-algorithm-variant and per-property file granularity, expandable as the crate grows. Each algorithm variant (Vandermonde vs Cauchy, GF(2^8) vs GF(2^16), scalar vs SIMD) gets its own file.

---

### 7. Key Algorithms & Data Structures

- `CodecConfig { k: u16, m: u16, n: u16, field: FieldType }` — `n = k + m`; validated on construction (`1 ≤ k`, `1 ≤ m`, `n ≤ field limit`).
- `ReedSolomon { field, generator: Vec<Vec<F>> }` — holds the systematic generator matrix `G` (n×k field elements); constructed once per config, reused across many messages.
- Field arithmetic: GF(2^8) `add = XOR`; `mul(a,b) = exp[(log[a]+log[b]) mod 255]` via log/exp tables (primitive polynomial pinned, generator α = 2); `div`/`inv` via tables; `mul(0,·)=0`, `inv(0)` errors.
- Vandermonde `V[r][c] = r^c` with distinct field elements per shard index; every k×k submatrix nonsingular (determinant ∏(x_b − x_a) ≠ 0 in characteristic 2) → MDS → any k of n reconstructs.
- Systematic reduction: `T` = top k×k block of V; `G = V · T⁻¹` computed by Gauss-Jordan elimination over GF; top k rows of G = identity → systematic.
- Encode: per byte offset t, parity `p_j[t] = Σ_i G[k+j][i] ⊗ d_i[t]`; data shards unchanged.
- Decode: surviving rows form k×k matrix A over GF; `A⁻¹` by Gauss-Jordan; `d = A⁻¹ · survivors` per byte offset; verify by re-encode + compare (never-mis-correct).
- Complexity: encode O(n·k) field muls per byte; decode O(k³) one-time inversion + O(k²) per byte; matrix memory O(n·k) bytes (GF(2^8)).
- Limits: GF(2^8) ⇒ n ≤ 256; GF(2^16) ⇒ n ≤ 65535; shards equal length L ≥ 1.
- Recommended configs: RS(10,4) → 1.4× overhead / 4 failures; RS(14,10) → 1.4× / 4; RS(9,6) → 1.5×; RS(6,3) → 1.5× (Ceph default); vs replication 3× / 2 failures.

---

### 8. Dependencies

- `ids-000-aa` (path dep): canonical types (`RecordId`, `RegionId`, `NodeId`, `Checksum`, `Endpoint`, `CarrierType`), `ids_000_aa::Error`, sealed `Erasure` trait contract.
- `blake3` + `sha2` (optional, behind `integrity` feature): shard digests only — allowed crypto list.
- No `std::net`, `tokio::net`, `libp2p`, or any networking crate; no async runtime.
- `#![forbid(unsafe_code)]` target (reed-solomon precedent); `std::arch` SIMD intrinsics are safe wrappers, no conflict.
- No third-party FEC crate as hard dependency (vendored implementation per ids-000-aa contracts; surveyed crates used as reference/test-vector sources only).
- Dev-dependencies: `criterion`, `proptest`/`quickcheck` (property-based round-trip tests).
- Cargo workspace layout with path dependencies (ids-000-aa at `C:/idfs/ids-000-aa/`), per plan-001 workspace pattern.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | Crate compiles cleanly | `cargo check -p ids-reed-solomon` exits 0, no warnings |
| 2 | Systematic identity | Unit test: top k rows of generator G are identity, all (k,m) configs |
| 3 | MDS property | For small (n,k), exhaustive: every k-subset of surviving shards reconstructs original data exactly |
| 4 | Any k of n | Tolerates any m = n−k shard losses, data or parity mixed |
| 5 | Storage overhead | Stored bytes = n/k × input; RS(10,4)=1.4×, RS(14,10)=1.4×, RS(9,6)=1.5×, RS(6,3)=1.5×; documented in README + config test |
| 6 | Field limits | GF(2^8) n ≤ 256, GF(2^16) n ≤ 65535; invalid config → InvalidConfig error, no panic |
| 7 | Shard length | Mismatch → ShardLenMismatch; zero-length → InvalidConfig |
| 8 | Insufficient survivors | < k survivors → InsufficientShards error; no partial data returned |
| 9 | Never-mis-correct | Corrupted shard → detected via re-encode verification/digest mismatch → Unrecoverable, never wrong-but-plausible data |
| 10 | Crypto boundary | Core uses no ed25519/xchacha20poly1305/chacha20poly1305; optional digests restricted to blake3/sha256 |
| 11 | Carrier-agnostic | Grep: zero `std::net`/`tokio::net`/`libp2p`/`IpAddr`/`SocketAddr`/`TcpStream`/`UdpSocket`; `cargo tree` no networking crates |
| 12 | Dependency boundary | `cargo tree` shows only ids-000-aa (+ optional blake3/sha2) + dev deps |
| 13 | Tests pass | `cargo test -p ids-reed-solomon` all green |

---

### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|---|--------|-----------------|
| GF(2^8) cap n ≤ 256 vs GF(2^16) complexity | Max erasure-set size per stripe | Decide default field at ids-store integration based on target stripe width (MinIO 2–16 drives; Ceph small k/m) |
| SIMD acceleration (pclmulqdq/vpclmulqdq, AVX2/Neon, GFNI) | Encode/decode throughput | Scalar Rust first; SIMD as cfg-gated optional feature, benchmark-gated before shipping |
| `#![forbid(unsafe_code)]` compatibility | SIMD implementation approach | Safe std::arch wrappers only — no conflict; confirmed decision |
| Shard digests on by default? | Integrity vs encode performance | `integrity` feature off by default; toggle decided at ids-store integration |
| Integration with ids-regenerating-codes (Phase 085) | Repair-bandwidth interface | Defined at Phase 085; 084 provides reconstruction primitive only |
| Encoding placement (client vs storage node) | ids-store write-path design | Decided at ids-store integration, not here |
| Test-vector reference for KAT tests | Decode correctness confidence | Pin one reference implementation (reedsolomon RS(255,223) CCSDS KATs or reed-solomon-engine wire format) |
| no_std support | Embedded carrier use | Not required now; std used for Vec; evaluated later |

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate crate `ids-reed-solomon` (not a folder) | "Separate crates instead of more folders" directive; clean erasure-coding boundary |
| GF(2^8) default, GF(2^16) optional | n ≤ 256 covers typical erasure sets; GF(2^16) for large stripes; small (255-entry) log/exp tables |
| Systematic form (data shards unchanged) | Storage systems read data in place; matches reed-solomon-engine and ISA-L systematic construction |
| Matrix-inversion erasure decode (no error correction) | Storage failures are erasures (known positions); simpler and cheaper than errors-and-erasures decoding (no Berlekamp-Massey) |
| Vendored implementation per ids-000-aa `Erasure` contract, no third-party FEC dep | Single control point; avoids unmaintained upstream risk (rust-rse seeking maintainers); surveyed crates are reference/test-vector sources only |
| Shard order is caller's responsibility | Crate is pure compute; carrier-agnostic; caller maps shard index → NodeId/RegionId and ships Vec<u8> via ids-carrier::Endpoint |
| Optional integrity via blake3/sha256 only | Allowed crypto list; RS core is crypto-free arithmetic |
| Scalar Rust first, SIMD cfg-gated optional | Portability first; pclmulqdq/vpclmulqdq in std::arch on x86; NEON via std::arch; benchmark before shipping |
| `#![forbid(unsafe_code)]` target | reed-solomon precedent; std::arch intrinsics are safe wrappers |
| thiserror error enum with recovery hints | Matches ids-000-aa unified error contract style |
| Upfront config validation + try_reserve allocations | DoS-hardening pattern from the surveyed reedsolomon crate |

---

### 12. Sources

1. rust-rse, `reed-solomon-erasure` crate v6.0.0 (MIT) — https://crates.io/crates/reed-solomon-erasure ; https://github.com/rust-rse/reed-solomon-erasure — pure Rust RS erasure coding GF(2^8)/GF(2^16), systematic, reconstruct any k of n; Backblaze + klauspost ports; seeking new maintainers.
2. `reed-solomon-simd` (Leopard-RS) v3.1.0 — https://crates.io/crates/reed-solomon-simd ; https://github.com/AndersTrier/reed-solomon-simd — O(n log n), GF(2^16), runtime SIMD SSSE3/AVX2/Neon, up to 32768 shards, encode ~6–10 GiB/s.
3. `reed-solomon-engine` (stephenberry) — https://github.com/stephenberry/reed-solomon-engine — pure Rust GF(2^8), zero runtime deps, systematic Vandermonde, n ≤ 256 shards, WIRE_FORMAT.md design doc, never-mis-correct verification.
4. Intel ISA-L — https://github.com/intel/isa-l — GF(2^8) block RS erasure codes, ec_encode_data/ec_decode_data, reduction polynomial 0x1d, AVX/AVX2 assembly, multibinary dispatch (API doc 2.31.0).
5. James Plank, UTK — jerasure / gf-complete + tutorials — https://web.eecs.utk.edu/~jplank/plank/www/software.html — Cauchy Reed-Solomon, Vandermonde→systematic inversion via column operations, erasure vs errors distinction.
6. klauspost/reedsolomon (Go) — https://github.com/klauspost/reedsolomon — Backblaze JavaReedSolomon port, Leopard for >256 shards, >1 GB/s/core.
7. sia_reed_solomon (Sia) — https://sia.tech/blog/reed-solomon-rust — Rust port of klauspost, AVX2/GFNI/NEON/WASM, production deployment 11 years.
8. `reedsolomon` (BolivarTech) v0.2.0 — https://crates.io/crates/reedsolomon — #![forbid(unsafe_code)], RS(255,223) CCSDS GF(2^8), never-mis-correct via syndrome verification, DoS-hardened try_reserve allocation.
9. Finite-field Rust crates — `galois_field_2pm`, `galois_2p8`, `fgf`, `isochronous_finite_fields` — docs.rs — GF(2^8) arithmetic, log/exp tables, constant-time option.
10. std::arch `pclmulqdq` / `vpclmulqdq` — doc.rust-lang.org — carry-less multiplication intrinsics for GF(2) multiplication (SIMD reference).
11. ACM Trans. on Storage, "A Survey of the Past, Present, and Future of Erasure Coding for Storage Systems" — https://dl.acm.org/doi/10.1145/3708994 — RS most widely deployed erasure-code family; (n,k); any k of n.
12. Error Correction Zoo, MDS code — https://errorcorrectionzoo.org/c/mds — Singleton bound d = n−k+1; MDS = equality; corrects n−k erasures.
13. MathWorld, Reed-Solomon Code — https://mathworld.wolfram.com/Reed-SolomonCode.html — polynomial evaluations over F_q at n distinct elements, d = n−k+1.
14. Plank, UTK tutorials CS-03-504 / CS-07-603 — web.eecs.utk.edu/~jplank/plank/papers/ — systematic information dispersal matrix from Vandermonde; Cauchy Reed-Solomon.
15. MinIO erasure coding docs — https://min.io/docs/minio/container/operations/concepts/erasure-coding.html — N = K + M, read/write quorum K, RS(8,8)/RS(12,6) example deployments.
16. Ceph erasure code docs — https://docs.ceph.com/en/reef/rados/operations/erasure-code — reed_sol_van k=2,m=2 overhead 1.5× vs replication 3.0×; overhead formula (k+m)/k.
17. Error Correction Zoo, Locally recoverable code — https://errorcorrectionzoo.org/c/locally_recoverable — locality r; related to Phase 085 regenerating codes (out of scope here).
18. IDS prompts.txt entry 84 (verbatim) — C:/idfs/prompts.txt lines 1255-1269 — crate definition, contents, constraints.
19. HX-241 plan format references — C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — section layout, status line, table formats.
20. C:/idfs/ids-000-aa/ skeleton — types.rs, contracts/ (incl. erasure), errors.rs — canonical types and sealed trait contracts this crate implements.