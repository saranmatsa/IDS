## Phase 023 — Create the ids-erasure crate (reconstruction-based redundancy with storage efficiency): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 023 creates `ids-erasure`, the IDS crate for reconstruction-based redundancy with storage efficiency. It implements erasure coding across the redundancy spectrum: Reed-Solomon (n,k) codes in systematic form, Maximum Distance Separable (MDS) codes, regenerating codes, locally recoverable codes (LRC), erasure-code reconstruction, decode-latency optimization, fragment placement across failure domains, and partial parallel repair (PPR). `ids-erasure` is an umbrella crate that composes sibling specialized crates — `ids-reed-solomon` (84), `ids-regenerating-codes` (85), `ids-locally-recoverable-codes` (86), `ids-ppr-reconstruction` (87) — rather than folding them into existing crates, matching the user directive "try to use separate crates instead of more folders in existing ones."

Crate: `ids-erasure` (CRATES.md entry 23). Purpose: Reconstruction-based redundancy with storage efficiency. Contents per prompts.txt entry 23: Reed-Solomon (n,k) codes, systematic form, maximum distance separable (MDS), regenerating codes, locally recoverable codes (LRC), erasure-code reconstruction, decode latency, fragment placement across failure domains, partial parallel repair (PPR).

Verbatim prompt (C:/idfs/prompts.txt lines 340-354): "Phase 023 — Create the ids-erasure crate: Reconstruction-based redundancy with storage efficiency." / "Crate: ids-erasure / Purpose: Reconstruction-based redundancy with storage efficiency. / Contents: Reed-Solomon (n,k) codes, systematic form, maximum distance separable (MDS), regenerating codes, locally recoverable codes (LRC), erasure-code reconstruction, decode latency, fragment placement across failure domains, partial parallel repair (PPR)."

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305. `ids-000-aa` is the master crate with all canonical types and sealed trait contracts (including the sealed `trait Erasure` contract in `src/contracts/erasure.rs`, already present). Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-erasure` sits above `ids-000-aa` (types + sealed `trait Erasure` contract + `ErasureThreshold` invariant + `RedundancyMode`) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It composes four sibling crates, each responsible for one code family:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / PLACEMENT LAYER (ids-* crates)      │
│  redundancy policy · fragment placement · repair coordination         │
├─────────────────────────────────────────────────────────────────────┤
│              Phase 023: ids-erasure (THIS UMBRELLA CRATE)           │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ reed-solomon│ │ regenerating│ │    LRC      │ │    PPR      │   │
│  │ (crate 84)  │ │ (crate 85)  │ │ (crate 86)  │ │ (crate 87)  │   │
│  │ RS (n,k)    │ │ MSR/MBR     │ │ locality r  │ │ reduction   │   │
│  │ systematic  │ │ repair bw   │ │ groups      │ │ tree schedule│   │
│  │ MDS         │ │ exact/func  │ │ global+local│ │ log(k+1)    │   │
│  └─────────────┴─────────────┴─────────────┴─────────────┘   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  reconstruction engine · decode latency · fragment place- │  │
│  │  ment across failure domains · integrity checksums (blake3│  │
│  │  / sha256) · decode-latency optimization                   │  │
│  └───────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (ids-carrier::Endpoint only)           │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, &[u8])               │
│  receive_datagram → (Endpoint, Vec<u8>)                             │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└─────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `DatasetId`, `FragmentId`, `FragmentPlacement`, `RedundancyMode`, `FailureDomain`, `Checksum`, `OpRef`, `OrderingPos`, `Epoch`, `Version`) imported from `ids_000_aa::X`, never redefined.
- `ids-erasure` implements the sealed `trait Erasure` from `ids-000-aa/src/contracts/erasure.rs`; downstream crates use it as a trait bound, cannot implement it.
- All fragment transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16) + opaque `Vec<u8>`). No IP addresses, ports, sockets.
- Crypto surface: ed25519 (fragment authentication), sha256/blake3 (fragment integrity, content addressing), xchacha20poly1305/chacha20poly1305 (optional fragment encryption at rest/in-transit). All ops in `ids-crypto`; this crate declares only usage boundaries.
- `ids-crypto` does NOT depend on `ids-000-aa` (dependency rule from CRATES.md); `ids-erasure` depends on both.
- `ids-erasure` composes `ids-reed-solomon`, `ids-regenerating-codes`, `ids-locally-recoverable-codes`, `ids-ppr-reconstruction` as path dependencies; code-family selection is per-dataset redundancy policy.

---

### 3. Detailed Research Dimensions

#### 3.1 Reed-Solomon (n,k) Codes — Foundation MDS Code

Reed-Solomon codes are the standard erasure-coding design choice for distributed storage. An (n,k) RS code divides data into k equal-sized chunks, encodes them into n coded chunks of equal size such that any k out of n coded chunks suffice to reconstruct the original data. RS codes are Maximum Distance Separable (MDS): they achieve optimal fault tolerance for minimum storage overhead — any n−k chunk failures can be tolerated without data loss. RS codes are defined over Galois Fields GF(2^w), most commonly GF(256) where each field element is one byte; systematic form places the original k data chunks uncoded among the n chunks (generator matrix top k×k block is identity), so normal reads avoid decoding entirely.

For `ids-erasure`: RS (n,k) encoder/decoder over GF(256) with Vandermonde or Cauchy generator matrices, systematic encoding (identity top block), decoding via Gaussian elimination over GF(256) selecting k surviving fragment rows and inverting the sub-matrix. Decoding requires at least k available fragments; fewer → `InsufficientFragments` error.

#### 3.2 Systematic Form — Data Preserved Uncached

Systematic codes keep the original k data chunks unchanged after encoding; they can be accessed without a decoding process by reading the systematic blocks directly. This preserves read performance in normal operations (the common case) and only incurs decode cost on degraded reads (missing/unavailable fragments). Systematic encoding also means parity chunks are linear combinations of data chunks with coding coefficients, each parity `P_j = sum(alpha_{i,j} * D_i)` over GF(2^w).

For `ids-erasure`: systematic encoding as default mode; `is_systematic` flag on `ErasureConfig`; reads prefer k data fragments, fall back to decode only when data fragments are unavailable; parity-only recovery path documented.

#### 3.3 Maximum Distance Separable (MDS) Codes — Optimal Redundancy-Reliability Tradeoff

MDS codes attain the Singleton bound: minimum distance d = n − k + 1, tolerating exactly n−k erasures. Among linear codes, MDS codes (Reed-Solomon principal example) provide optimal storage efficiency — expansion factor n/k, overhead (n−k)/k — for a given fault tolerance f = n−k. For example RS(10,4) tolerates 4 failures at 1.4× storage overhead vs 3× for triple replication. Facebook's f4 uses RS(10,4); HDFS-EC ships RS(6,3), RS(3,2), RS(2,1).

For `ids-erasure`: `RedundancyMode::ErasureCoded { k, n }` exposes `failure_tolerance()` = n−k and `expansion_factor()` = n/k; MDS property verified via `any k columns of generator matrix are linearly independent`; non-MDS options (LRC) documented as trade-off (extra storage for locality).

#### 3.4 Regenerating Codes — Bandwidth-Efficient Node Repair

Regenerating codes (Dimakis et al., IEEE Trans. IT 2010) address the core RS weakness: repairing a single failed node traditionally requires downloading k full chunks (the whole data) to regenerate one. Regenerating codes let a replacement node download functions (β symbols) from d helper nodes instead of full chunks (α symbols), drastically reducing repair bandwidth. The fundamental tradeoff between per-node storage α and repair bandwidth γ = d·β is characterized by a cut-set bound. Two extremal points: Minimum-Storage Regenerating (MSR) codes (α = M/k, MDS-optimal storage, reduced bandwidth) and Minimum-Bandwidth Regenerating (MBR) codes (minimum repair bandwidth at slightly higher storage). Exact repair (replica identical to failed node) vs functional repair (information-equivalent). Clay codes (MSR, FAST'18) achieve exact repair with up to 2.9× less repair traffic; Simple Regenerating Codes achieve exact repair via XOR-only packet operations at rates up to 2/3.

For `ids-erasure`: `ids-regenerating-codes` (crate 85) implements MSR/MBR parameterization; exact-repair default; repair bandwidth γ vs storage α tradeoff exposed via `RegeneratingParams`; `ids-crypto` not involved in regenerating math (pure GF arithmetic); helper-node selection minimizes distinct-node reads.

#### 3.5 Locally Recoverable Codes (LRC) — Locality-Aware Repair

LRCs (Papailiopoulos & Dimakis, ISIT 2012; deployed in Windows Azure Storage, Facebook HDFS-Xorbas) introduce repair locality r: each code symbol can be recovered by accessing at most r other symbols (vs trivial locality k in MDS codes). Small r reduces network bandwidth and disk I/O during single-node repair. Trade-off: non-trivial locality necessarily increases storage overhead beyond MDS (distance penalty d ≤ n − k − ceil(k/r) + 2 for optimal LRC). Availability t: multiple disjoint repair sets per symbol. LRC = local parities (within small groups) + global parities (MDS across groups). Facebook Xorbas (10,6,5) LRC: 10 data blocks → RS(10,4) global parities + local XOR parities; ~2× less repair disk I/O and network traffic vs RS-only, at 14% more storage.

For `ids-erasure`: `ids-locally-recoverable-codes` (crate 86) implements (n,k,r) LRC with locality groups; local repair path (contact r nodes) preferred over global RS decode (contact k nodes); global fallback when locality group is itself damaged; availability t = number of disjoint repair sets.

#### 3.6 Erasure-Code Reconstruction — Decode from k of n

Reconstruction recreates lost data from any k surviving fragments. RS decoding: (1) remove erased rows from generator matrix, (2) select k surviving rows to form k×k sub-matrix, (3) invert sub-matrix over GF(256), (4) multiply by surviving data/parity values to recover erased elements. Any square submatrix of a Vandermonde generator matrix is invertible, so decoding is always possible from any k surviving shards. Decoding is the latency-critical path — degraded reads and failed-node repair both wait on it. Integrity verification (blake3/sha256 per fragment) must precede decode to avoid reconstructing from corrupted fragments.

For `ids-erasure`: `reconstruct(dataset, available, missing_indices)` via GF(256) matrix inversion; decode-latency optimization (parallel fragment fetch, SIMD GF multiplication, precomputed inverse matrices per erasure pattern); `decode(dataset, fragments)` returns original data from ≥k fragments; checksum verification before use.

#### 3.7 Decode Latency — Reconstruction Performance

Decode latency dominates degraded-read and repair tail latency. RS (n,k) decode contacts k nodes and performs O(k²·fragment_size) GF operations; LRC reduce to O(r·fragment_size) for local repairs; regenerating codes reduce transferred bytes to d·β < k·α. Optimization levers: precompute inverse matrices per erasure pattern, parallel fragment retrieval across helpers, SIMD-accelerated GF(256) multiplication (table lookups), fragment-size alignment to cache lines, read-repair vs full-decode choice based on availability. Intel ISA-L achieves ~5 GB/s RS EC on server Xeon via SSE/ASM; pure-Rust crates (reed-solomon-erasure, tape-reed-solomon) fall back to portable Rust with runtime SIMD selection.

For `ids-erasure`: decode-latency benchmarks target (p50/p95/p99) per erasure pattern; inverse-matrix cache keyed by (n,k,erasure-set); parallel fragment fetch with per-helper timeouts; SIMD feature flag; no dependency on ISA-L C code in research phase (documented integration option).

#### 3.8 Fragment Placement Across Failure Domains — Correlated Failure Awareness

Placement determines effective fault tolerance. An (n,k) layout tolerates m = n−k fragment losses only if no single failure domain holds more than m fragments of a stripe. Placement across servers, racks, datacenters, regions: each fragment of a stripe on a distinct fault domain at the required level. f4 (Facebook) lays RS(10,4) blocks across different racks within one datacenter, XOR coding wide-area for datacenter resilience. Scality layouts: k+m splits, rule "no fault domain may hold more than m fragments of any stripe"; minimum cluster shape derived per failure level (server, rack, DC). Correlated failures (power, network segment, operator, jurisdiction) modeled via `FailureDomain` hierarchy from `ids-000-aa` (`topology_diversity`, `correlated_failure` invariants).

For `ids-erasure`: `place_fragments(dataset, fragments, nodes)` respects failure-domain constraints; `FragmentPlacement` records node + domain; placement validator rejects layouts where any domain holds > (n−k) fragments; `ids-topology` (crate 6) supplies domain map; `ids-placement` (crate 9) supplies capacity-aware node selection.

#### 3.9 Partial Parallel Repair (PPR) — Distributed Reconstruction Scheduling

PPR (EuroSys 2016, Purdue) divides reconstruction into small partial operations scheduled across multiple nodes already involved in reconstruction, then progressively combines partial results via a reduction tree. For a (k,m) RS code, conventional repair transfers k fragments sequentially; PPR completes network transfer in ⌈log₂(k+1)⌉ time vs k time, reducing network pressure and repair time significantly, compatible with existing erasure codes with no additional storage overhead. m-PPR scheduling handles concurrent failures; partial results aggregated through a reduction tree (each node combines two partial results per round).

For `ids-erasure`: `ids-ppr-reconstruction` (crate 87) implements PPR partial-operation scheduling, reduction-tree aggregation, m-PPR for concurrent failures; integrated into `repair()` path; PPR optional flag on `ErasureConfig`; repair-time benchmarks compare sequential vs PPR.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Correlated failure wipes > m fragments of a stripe (same domain holds too many fragments) | Placement validator enforces ≤ (n−k) fragments per domain; `FailureDomain` overlap check; multi-domain placement policy |
| Fragment corruption (bit rot) silently decodes into wrong data | blake3/sha256 checksum per fragment; verify before decode; `verify_fragment()` gate; corrupted fragments excluded from decode set |
| Insufficient fragments available (available < k) | `InsufficientFragments` error; `detect_loss()` reports reconstructability before attempting decode; auto-repair triggered when below threshold |
| Decode latency spike on degraded reads | Precomputed inverse matrices, parallel fragment fetch, SIMD GF ops, LRC local-repair fast path, PPR reduction tree |
| Repair bandwidth explosion (RS full-chunk download) | Regenerating codes (MSR/MBR) download β < α per helper; LRC contact only r nodes; PPR distributes load |
| Carrier-agnostic rule violation (std::net, tokio::net, IP/TCP/UDP/…) | `ids-carrier::Endpoint` sole address type; dependency audit bans networking crates; CI grep for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Version skew / wire incompatibility between replicas | `ProtocolVersion` + `EdgeCaseBits` from ids-000-aa; erasure frame format version-tagged |
| Storage overhead too high for cold data | Lenient configs (k=2,n=3) vs strict (k=8,n=16); redundancy policy selects per dataset; `expansion_factor()` exposed |
| Reconstruction from wrong/fragment-set ambiguity | Fragment index + `FragmentId` + `Version` + `Epoch` on every `ErasureFragment`; decode set validated against expected indices |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 023 ||
|-------|-------|-------------------|
| `ids-000-aa` (master) | Canonical types (`NodeId`, `DatasetId`, `FragmentId`, `FragmentPlacement`, `RedundancyMode`, `FailureDomain`, `Checksum`, `OpRef`, `OrderingPos`, `Epoch`, `Version`), sealed `trait Erasure`, `ErasureThreshold` invariant, versioning, unified `Error` |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); `send_datagram`/`receive_datagram`; sole traffic channel for fragment transport |
| `ids-crypto` (Phase 003) | ed25519 signing/verification, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 AEAD; consumed only |
| `ids-reed-solomon` (crate 84) | RS (n,k) GF(256) encoder/decoder, systematic form, MDS — code family implementation |
| `ids-regenerating-codes` (crate 85) | MSR/MBR regenerating codes, bandwidth-efficient repair |
| `ids-locally-recoverable-codes` (crate 86) | (n,k,r) LRC, locality groups, local repair |
| `ids-ppr-reconstruction` (crate 87) | Partial Parallel Repair, reduction-tree scheduling |
| `ids-topology` (crate 6) | Failure-domain hierarchy, topology map, correlated-failure model |
| `ids-placement` (crate 9) | Capacity-aware, failure-domain-aware node selection |
| `ids-consistency` (crate 19) | Operation-state tracking, consistency profiles for fragment writes |
| `ids-write` (crate 13) | WAL and write lifecycle consuming encoded fragments |

**Integration points:**
- `ids-erasure` implements `ids_000_aa::contracts::Erasure` (sealed trait) — declared in master, implemented here.
- All outbound fragment traffic: `ids_carrier::Endpoint` datagrams only; `ids-erasure` never touches sockets.
- Fragment integrity via sha256/blake3 digests; optional fragment encryption via xchacha20poly1305/chacha20poly1305 from `ids-crypto`.
- Depends on `ids-000-aa` for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (per CRATES.md dependency rule) — `ids-erasure` depends on both.
- Cargo workspace: `ids-erasure = { path = \"../ids-erasure\" }` in each dependent crate; sibling crates 84–87 as path dependencies.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-erasure` (to be created at `C:/idfs/ids-erasure/`)

```
ids-erasure/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-reed-solomon,
│                                       # ids-regenerating-codes, ids-locally-recoverable-codes,
│                                       # ids-ppr-reconstruction, ids-topology, ids-placement;
│                                       # thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ErasureScheme, CodeFamily, RedundancyPolicy, StripeId,
│   │                                   # ReconstructionMode, DecodeStrategy, RepairRole
│   ├── errors.rs                       # ErasureError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Erasure (sealed trait)
│   │
│   ├── codec/                          # wire encoding of erasure fragments (~5 files)
│   │   ├── mod.rs
│   │   ├── fragment_frame.rs           # version-tagged fragment wire frame
│   │   ├── fragment_codec.rs           # serde/bincode encode/decode
│   │   ├── stripe_header.rs            # stripe metadata (k, n, epoch, version)
│   │   └── checksum.rs                 # blake3/sha256 fragment checksums
│   │
│   ├── reed_solomon/                   # RS (n,k) code family — delegates to ids-reed-solomon (84) (~8 files)
│   │   ├── mod.rs
│   │   ├── rs_encoder.rs               # systematic RS encoding over GF(256)
│   │   ├── rs_decoder.rs               # RS decode via GF(256) matrix inversion
│   │   ├── gf256.rs                    # GF(256) arithmetic (log/exp tables, mul/div/inv)
│   │   ├── vandermonde.rs              # Vandermonde generator matrix
│   │   ├── cauchy.rs                   # Cauchy generator matrix
│   │   ├── matrix.rs                   # k×k sub-matrix selection + inversion
│   │   ├── inverse_cache.rs            # precomputed inverse matrices per erasure pattern
│   │   └── systematic.rs               # systematic-form encoding/read path
│   │
│   ├── regenerating/                   # Regenerating codes — delegates to ids-regenerating-codes (85) (~6 files)
│   │   ├── mod.rs
│   │   ├── msr.rs                      # Minimum-Storage Regenerating code
│   │   ├── mbr.rs                      # Minimum-Bandwidth Regenerating code
│   │   ├── repair_params.rs            # alpha (storage) vs gamma (bandwidth) tradeoff
│   │   ├── helper_selection.rs         # d helper nodes, beta symbols each
│   │   ├── exact_repair.rs             # exact vs functional repair mode
│   │   └── bandwidth.rs                # repair bandwidth computation
│   │
│   ├── lrc/                            # Locally Recoverable Codes — delegates to ids-locally-recoverable-codes (86) (~7 files)
│   │   ├── mod.rs
│   │   ├── locality_group.rs           # local parity group (r symbols)
│   │   ├── local_repair.rs             # repair via r nodes (fast path)
│   │   ├── global_repair.rs            # fallback to global MDS decode
│   │   ├── availability.rs             # t disjoint repair sets
│   │   ├── lrc_encoder.rs              # local + global parity construction
│   │   └── lrc_decoder.rs              # locality-aware decode
│   │
│   ├── ppr/                            # Partial Parallel Repair — delegates to ids-ppr-reconstruction (87) (~6 files)
│   │   ├── mod.rs
│   │   ├── ppr_scheduler.rs            # divide reconstruction into partial operations
│   │   ├── reduction_tree.rs           # progressive combination of partial results
│   │   ├── m_ppr.rs                    # concurrent failure scheduling
│   │   ├── partial_op.rs               # partial reconstruction operation
│   │   └── ppr_strategy.rs             # sequential vs PPR selection
│   │
│   ├── reconstruction/                 # Core reconstruction engine (~8 files)
│   │   ├── mod.rs
│   │   ├── reconstructor.rs            # orchestrate decode from k of n fragments
│   │   ├── decode_strategy.rs          # sequential / parallel / LRC-local / PPR selection
│   │   ├── fragment_fetcher.rs         # parallel fragment retrieval via Endpoint
│   │   ├── integrity_gate.rs           # checksum verify before decode
│   │   ├── latency_opt.rs              # SIMD GF ops, cache-line alignment, inverse cache
│   │   ├── degraded_read.rs            # read path: data fragments first, decode fallback
│   │   ├── repair_path.rs              # failed-node repair orchestration
│   │   └── reconstruction_stats.rs     # decode latency histograms (p50/p95/p99)
│   │
│   ├── placement/                      # Fragment placement across failure domains (~7 files)
│   │   ├── mod.rs
│   │   ├── placer.rs                   # place fragments respecting domain constraints
│   │   ├── domain_constraint.rs        # ≤ (n−k) fragments per failure domain
│   │   ├── topology_adapter.rs         # wrap ids-topology FailureDomain map
│   │   ├── capacity_adapter.rs         # wrap ids-placement node selection
│   │   ├── placement_validator.rs      # reject correlated-failure layouts
│   │   ├── replacer.rs                 # rebalance placement on topology change
│   │   └── placement_stats.rs          # per-domain fragment distribution
│   │
│   ├── integrity/                      # Fragment integrity & checksums (~5 files)
│   │   ├── mod.rs
│   │   ├── fragment_checksum.rs        # blake3/sha256 per-fragment digest
│   │   ├── verify_fragment.rs          # checksum gate before decode/use
│   │   ├── corrupted_tracker.rs        # track corrupted fragment indices
│   │   └── integrity_report.rs         # per-stripe integrity summary
│   │
│   ├── config/                         # Erasure configuration (~5 files)
│   │   ├── mod.rs
│   │   ├── erasure_policy.rs           # per-dataset redundancy policy (RS/LRC/regenerating)
│   │   ├── code_family.rs              # RS vs regenerating vs LRC selection
│   │   ├── ppr_config.rs               # PPR enable/disable, tree arity
│   │   ├── latency_config.rs           # decode-latency targets, SIMD toggle
│   │   └── preset_configs.rs           # default/strict/lenient (k=4,n=10 etc.)
│   │
│   ├── stats/                          # Erasure statistics (~4 files)
│   │   ├── mod.rs
│   │   ├── erasure_stats.rs            # encodes/decodes/reconstructs/repairs counters
│   │   ├── bandwidth_tracker.rs        # repair bandwidth bytes
│   │   └── latency_histogram.rs        # decode latency percentiles
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper for fragments
│   ├── dedup.rs                        # fragment ID dedup (at-most-once reconstruction)
│   ├── state/                          # state machines (~4 files)
│   │   ├── mod.rs
│   │   ├── erasure_state_machine.rs    # stripe lifecycle: encoded → protected → degraded → repairing → restored
│   │   └── fragment_state_machine.rs   # fragment lifecycle: healthy → suspected → lost → reconstructed
│   ├── validation/                     # input validation rules (k>0, n>=k, fragment sizes equal)
│   ├── observability/                  # erasure events (encoded, decoded, reconstructed, repaired, lost)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── reed_solomon_tests.rs           # RS encode/decode, any k of n
│   ├── systematic_tests.rs             # systematic read without decode
│   ├── mds_tests.rs                    # MDS property: any k columns independent
│   ├── regenerating_tests.rs           # MSR/MBR repair bandwidth
│   ├── lrc_tests.rs                    # local repair via r nodes
│   ├── ppr_tests.rs                    # PPR reduction-tree correctness
│   ├── reconstruction_tests.rs         # decode from k of n, failure patterns
│   ├── decode_latency_tests.rs         # p50/p95/p99 decode latency
│   ├── placement_tests.rs              # failure-domain placement constraints
│   ├── integrity_tests.rs              # checksum verification, corruption detection
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~110–170 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

1. **Reed-Solomon (n,k) systematic encoding** — data split into k equal fragments; generator matrix G (n×k) over GF(256) with top k×k = identity (systematic); parity rows from Vandermonde/Cauchy matrix pre-multiplied by inverse of top k×k sub-matrix so any k×k submatrix of G is invertible; output = [data, parity]. Any k of n fragments reconstruct original.

2. **GF(256) arithmetic** — log/exp tables (256 + 512 entries), primitive polynomial 0x11d; mul/div/pow/inv via table lookup; SIMD-accelerated option; constant-time not required (erasure coding is not secret-handling).

3. **RS decoding via matrix inversion** — select k surviving fragment rows; form k×k sub-matrix of G; Gaussian elimination over GF(256) to invert; multiply inverse by surviving values to recover erased fragments; O(k²·fragment_size).

4. **Regenerating code repair** — replacement node contacts d helpers, downloads β symbols each (not full α); γ = d·β < k·α repair bandwidth; MSR keeps α = M/k (MDS-optimal storage); exact repair reproduces lost fragment identically.

5. **LRC local repair** — lost symbol recovered from r locality-mates (r ≪ k); local parity = linear combination of group symbols; global MDS parities survive multi-group failures; availability t = disjoint repair sets.

6. **PPR reduction-tree repair** — reconstruction split into partial ops; ⌈log₂(k+1)⌉ communication rounds vs k sequential; reduction tree combines partial results pairwise; m-PPR for concurrent failures; zero extra storage.

7. **Fragment placement validator** — for each failure domain, count fragments of stripe; reject if any domain holds > (n−k) fragments; uses `ids-topology::FailureDomain` overlap check (power/facility/network/operator/geographic).

8. **Integrity gate** — blake3/sha256 checksum per fragment computed at encode; verified before decode and before serving; corrupted fragments excluded, counted in `ErasureLossReport.corrupted_indices`.

9. **Erasure state machine** — stripe lifecycle: `encoded → protected → degraded (available < n) → repairing → restored`; terminal `lost` if available < k (unreconstructable); transitions guarded by `can_transition_to`.

10. **Reconstruction strategy selection** — data-fragments-first (systematic read) → LRC local repair (r nodes) → PPR reduction tree (log rounds) → full RS decode (k nodes, matrix inversion) → report `InsufficientFragments` if < k available.

---

### 8. Dependencies

`ids-erasure/Cargo.toml` (proposed):
- `ids-000-aa = { path = \"../ids-000-aa\" }` — types, sealed `trait Erasure`, invariants, versioning, errors
- `ids-carrier = { path = \"../ids-carrier\" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = \"../ids-crypto\" }` — ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 (consumed only)
- `ids-reed-solomon = { path = \"../ids-reed-solomon\" }` — RS (n,k) GF(256) encoder/decoder (crate 84)
- `ids-regenerating-codes = { path = \"../ids-regenerating-codes\" }` — MSR/MBR repair (crate 85)
- `ids-locally-recoverable-codes = { path = \"../ids-locally-recoverable-codes\" }` — (n,k,r) LRC (crate 86)
- `ids-ppr-reconstruction = { path = \"../ids-ppr-reconstruction\" }` — PPR reduction-tree repair (crate 87)
- `ids-topology = { path = \"../ids-topology\" }` — FailureDomain map, correlated-failure model (crate 6)
- `ids-placement = { path = \"../ids-placement\" }` — capacity-aware node selection (crate 9)
- `ids-consistency = { path = \"../ids-consistency\" }` — op-state tracking, consistency profiles (crate 19)
- `thiserror = \"2.0\"` — error enum derive
- `serde = { version = \"1\", features = [\"derive\"] }` — fragment serialization
- `blake3 = \"1.5\"` — content hashing (workspace dependency)
- `serde_json = \"1\"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime; datagram-driven. `cargo tree` must show zero networking crates. Sibling crates 84–87 are path dependencies; erasure-code math (GF arithmetic) is pure Rust — no C/ASM (ISA-L documented as future integration option).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|---|-----------|-------------|
| 1 | Four code families specified | `src/reed_solomon/`, `src/regenerating/`, `src/lrc/`, `src/ppr/` modules present; each documents its guarantee |
| 2 | Sealed `trait Erasure` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Erasure` |
| 3 | RS (n,k) any-k-of-n reconstruction | Encode k data → n parity; decode succeeds from any k of n; fails with `InsufficientFragments` below k |
| 4 | Systematic form | k data fragments stored uncoded; normal reads serve from data fragments without decode |
| 5 | MDS property | tolerates exactly n−k fragment losses; expansion factor n/k exposed |
| 6 | Regenerating repair bandwidth | γ = d·β < k·α; MSR keeps MDS-optimal storage α = M/k |
| 7 | LRC local repair | single-symbol repair contacts ≤ r nodes (r ≪ k); global fallback on multi-group failure |
| 8 | PPR repair rounds | ⌈log₂(k+1)⌉ communication rounds vs k sequential; reduction-tree aggregation |
| 9 | Fragment placement | ≤ (n−k) fragments per failure domain; `place_fragments` validates domain constraints |
| 10 | Integrity verification | blake3/sha256 per fragment; verified before decode; corrupted fragments excluded |
| 11 | Decode latency benchmarks | p50/p95/p99 per erasure pattern (RS, LRC-local, PPR); inverse-matrix cache |
| 12 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` in source; all traffic via `ids_carrier::Endpoint` |
| 13 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM |
| 14 | No code written / no source modifications | Plan document only; `ids-erasure/` crate not created; no edits to existing crates |
| 15 | Conformance test plan enumerated | `tests/` list covers RS, systematic, MDS, regenerating, LRC, PPR, reconstruction, decode latency, placement, integrity, carrier-agnostic, crypto allowlist |
| 16 | File structure within range | ~110–170 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
|-------------|---------|----------------|
| GF field size: GF(256) vs GF(2^16) | n ≤ 256 for GF(256); larger n needs GF(2^16) | GF(256) default (n ≤ 256 sufficient for most layouts); GF(2^16) option for wide stripes |
| Code-family selection per dataset | Affects `ErasureConfig.mode` and which sibling crate is used | Redundancy policy (RS/LRC/regenerating) per dataset; decided at configure time |
| PPR integration maturity | PPR is a 2016 research technique; production adoption limited | PPR optional flag; sequential repair as default; PPR enabled via config |
| Regenerating code exact vs functional repair | Exact repair keeps code coefficients; functional allows coefficient change | Exact repair default (simpler system maintenance); functional documented option |
| LRC storage overhead trade-off | 14%+ overhead vs MDS for locality benefit | Per-dataset policy chooses MDS (max efficiency) vs LRC (faster repair) |
| SIMD GF multiplication | Pure-Rust GF(256) table lookup vs SIMD/ISA-L | Portable Rust default; `simd` feature flag for runtime SIMD selection; ISA-L C integration deferred |
| Fragment size alignment | Unequal fragment sizes complicate GF matrix ops | Equal-size fragments enforced; padding documented |
| Checksum algorithm choice | blake3 (fast) vs sha256 (FIPS) | Both allowed; blake3 default for fragment integrity; sha256 option for compliance |
| n upper bound for RS | GF(256) limits n ≤ 256 (including parity) | n ≤ 256 validated in config; larger n requires multi-stripe or GF(2^16) |
| Interaction with ids-recovery (crate 25) | Erasure reconstruction is a recovery mechanism | `ids-recovery` consumes `ids-erasure::reconstruct()`; integration point documented |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Umbrella `ids-erasure` crate composing siblings 84–87 | Matches user directive \"separate crates instead of more folders\"; each code family independently versioned and tested |
| RS (n,k) GF(256) systematic as default | MDS-optimal storage; systematic reads avoid decode in common case; industry-standard (HDFS-EC, f4, Xorbas) |
| LRC for locality-sensitive deployments | Local repair r ≪ k reduces repair bandwidth/I/O; 14% overhead accepted where repair speed matters |
| Regenerating codes (MSR/MBR) for bandwidth-constrained repair | γ < k·α; MSR keeps MDS storage optimality |
| PPR for distributed repair scheduling | ⌈log₂(k+1)⌉ rounds vs k; reduces network pressure; zero extra storage |
| Fragment placement ≤ (n−k) per failure domain | Preserves effective fault tolerance under correlated failures; uses `ids-topology::FailureDomain` |
| blake3 default checksum, sha256 option | blake3 fast for integrity; sha256 for FIPS compliance; both in allowed set |
| `ids-carrier::Endpoint` sole transport | Carrier-agnostic absolute rule; no networking stack |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| No code in this phase | Research only per prompts.txt entry 23 |

---

### 12. Sources

1. Optimal Systematic Distributed Storage Codes with Fast Encoding (MDS, systematic form, Reed-Solomon): https://arxiv.org/html/1509.01858v1
2. On the Effectiveness of Polynomial Realization of Reed-Solomon Codes for Storage Systems (RS over GF(2^w), systematic encoding): https://arxiv.org/html/1312.5155v1
3. A Survey on Network Codes for Distributed Storage ((n,k) MDS codes, any k of n): https://arxiv.org/html/1004.4438
4. Network Coding for Distributed Storage Systems (Dimakis et al., regenerating codes, IEEE Trans. IT 2010): http://itsoc.org/publications/papers/network-coding-for-distributed-storage-systems
5. A flexible class of regenerating codes for distributed storage (repair bandwidth, MSR/MBR): https://ieeexplore.ieee.org/document/5513353
6. Regenerating Codes: A System Perspective (ACM SIGOPS 2013, storage vs repair bandwidth tradeoff): https://arxiv.org/html/1204.5028v3
7. Simple Regenerating Codes: Network Coding for Cloud Storage (exact repair via XOR, rate up to 2/3): https://ieeexplore.ieee.org/document/6195703
8. Locally Repairable Codes (Papailiopoulos & Dimakis, ISIT 2012, locality r): https://arxiv.org/abs/1206.3804
9. Locally recoverable code (Wikipedia, [n,k,d,r] LRC, distance-locality tradeoff): https://en.wikipedia.org/wiki/Locally_recoverable_code
10. XORing Elephants: Novel Erasure Codes for Big Data (LRC in HDFS-Xorbas, Facebook RS(10,4), ~2× repair improvement): https://arxiv.org/html/1301.3791v1
11. Erasure Coding in Windows Azure Storage (LRC in production, Huang et al. USENIX ATC 2012): cited in arxiv:1712.08566
12. Partial-parallel-repair (PPR): a distributed technique for repairing erasure coded storage (EuroSys 2016, ⌈log₂(k+1)⌉ rounds): https://doi.org/10.1145/2901318.2901328
13. Erasurecoded repair with PPR (Purdue EuroSys 2016 camera-ready, reduction tree, network pressure): https://engineering.purdue.edu/dcsl/publications/papers/2016/final_erasurecodedrepair_eurosys16_cameraready.pdf
14. Optimizing Cauchy Reed-Solomon Codes for Fault-Tolerant Network Storage (Cauchy RS, IEEE): https://ieeexplore.ieee.org/document/1659489
15. Reed-Solomon erasure coding Rust crate (reed-solomon-erasure, GF(256), BackBlaze/Klaus Post port): https://docs.rs/reed-solomon-erasure
16. Clay codes Rust implementation (ClayCode, MSR regenerating, Ceph FAST'18): https://docs.rs/crate/clay-codes/latest
17. tape-reed-solomon Rust crate (fast RS, SIMD, shard encoding): https://lib.rs/crates/tape-reed-solomon
18. Reed-Solomon erasure engine (no_std, GF(2^8), systematic Vandermonde): https://docs.rs/reed-solomon-engine/latest/src/reed_solomon_engine/lib.rs.html
19. Intel ISA-L erasure code and recovery (RS EC ~5 GB/s, SSE/ASM): https://software.intel.com/content/www/us/en/develop/articles/intel-isa-l-erasure-code-and-recovery.html
20. f4 – Facebook's Warm Blob Storage System (RS(10,4), rack-level placement, XOR wide-area): https://research.facebook.com/file/278332680350779/f4-facebook-s-warm-blob-storage-system.pdf
21. Erasure coding layouts: Capacity and resilience (Scality, k+m, fault-domain placement rule): https://scality.com/blog/erasure-coding-layouts
22. Comparing cost and performance of replication and erasure coding (storage overhead n/k, overhead (n−k)/k): https://arxiv.org/html/1308.1887v2
23. Accelerating XOR-based Erasure Coding using Program Optimization Techniques (GF(2^8) RS, ISA-L polynomial): https://arxiv.org/html/2108.02692v1
24. Systematic Erasure Codes with Optimal Repair Bandwidth and Storage (systematic MDS, stripe model): https://dl.acm.org/doi/10.1145/3109479
25. A Survey of the Past, Present, and Future of Erasure Coding for Storage Systems (RS, LRC, regenerating overview): https://keyuncheng.github.io/files/publications/tos24ecsurvey.pdf
26. Codes for Distributed Storage (RGC vs LRC goals: repair bandwidth vs repair degree): https://arxiv.org/html/2010.01344v1
27. On Fault Tolerance, Locality, and Optimality in Locally Repairable Codes (LRC locality-distance optimality): https://dl.acm.org/doi/10.1145/3381832
28. Monotone Erasure Codes (threshold access structures, MDS underneath): https://arxiv.org/html/2605.22426
29. IDS prompts.txt entry 23 (verbatim): C:/idfs/prompts.txt lines 340–354
30. CRATES.md entries 23, 84–87 and dependency rule: C:/idfs/CRATES.md; ids-000-aa erasure contract: C:/idfs/ids-000-aa/src/contracts/erasure.rs; HX-241 plan format: C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md; idfs plan format: C:/idfs/plans/plan-001.md, plan-020.md

---

**End of plan-023.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**
