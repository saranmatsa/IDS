## Phase 086 — Create the ids-locally-recoverable-codes crate: Locality-aware erasure coding. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1285-1299)

```
86. Research only do not read plan.md **[Phase 086 — Create the ids-locally-recoverable-codes crate: Locality-aware erasure coding.]**

Crate: ids-locally-recoverable-codes
Purpose: Locality-aware erasure coding.
Contents: LRC construction, locality groups, single/double erasure recovery within locality, code with locality parameters, repair locality optimization.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 086 — Create the ids-locally-recoverable-codes crate: Locality-aware erasure coding.]**

Create plan-086.md in C:/idfs/plans/ for the ids-locally-recoverable-codes crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 086 creates `ids-locally-recoverable-codes`, the IDS crate for locality-aware erasure coding. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by LRC domain. The crate implements Locally Repairable / Recoverable Codes (LRCs): erasure codes in which each coded symbol can be recovered from at most `r` other symbols (repair locality), dramatically reducing repair disk I/O and network bandwidth compared to Reed-Solomon codes when only a few symbols fail.

**Core design:** an LRC divides `k` data symbols into local groups (each of at most `r` symbols), computes one local parity per local group, and adds `g` global parities computed from all data symbols. A single failed symbol inside a local group is repaired by reading only the `r` surviving members of its group (plus the local parity), instead of all `k` data symbols required by Reed-Solomon. Global parities handle failures that span multiple local groups or exceed local redundancy. The crate supports `(r, δ)`-locality (tolerate δ−1 concurrent erasures within a local group), multiple disjoint repair sets (availability `t`), hierarchical locality, code puncturing/shortening for parameter tuning, and repair-locality optimization.

**Carrier-agnostic absolute rule:** no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-locally-recoverable-codes` consumes primitive IDs and checksum types from `ids-000-aa`/`ids-crypto` only).

**Scope boundary:** `ids-locally-recoverable-codes` defines LRC construction, locality-group management, erasure recovery algorithms, locality-parameter tuning, and repair-locality optimization. It does not implement a transport, carrier, or any networking stack; recovered/symbol bytes are shipped as opaque `Vec<u8>` to `ids-carrier::Endpoint` by the caller. Research only — no code, no source modifications, only this plan document.

A skeleton `ids-locally-recoverable-codes` crate does NOT yet exist at `C:/idfs/ids-locally-recoverable-codes/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, plus sealed trait contracts and error types.

**Research basis:** 30 parallel searches returned the Gopalan et al. Singleton-like bound `d ≤ n − k − ⌈k/r⌉ + 2`; the `(r, δ)`-locality generalization `d ≤ n − k + 1 − (⌈k/r⌉ − 1)(δ − 1)`; LRC definition (each symbol recoverable from ≤ r others, repair set); Azure-LRC `(k, l, g)` family deployed in Windows Azure Storage (MTTF 2.6×10¹² years vs 6.1×10¹¹ for RS(6,3), 1.67x storage overhead, ~50% lower repair reads); HDFS-Xorbas (Facebook) LRC over RS parities (~2× repair disk I/O and network reduction, 14% more storage); Pyramid codes (first LRC family); Optimal Cauchy LRC / Uniform Cauchy LRC (wide LRCs at Google); availability `t` (disjoint repair sets); LRC+PPR (Partial Parallel Repair further cuts per-link traffic); LRC fault tolerance `g + i` failures spanning `i` local groups; repair degree equal to local group size; puncturing/shortening transformations; hierarchical locality; maximally recoverable LRCs; finite-field constructions (cyclic, Reed-Solomon-based, BCH, Gabidulin).

---

### 2. Architecture Overview

`ids-locally-recoverable-codes` sits above `ids-000-aa` (canonical types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is an erasure-coding layer: given a data stripe and locality parameters, it constructs an LRC codeword, partitions symbols into locality groups, computes local and global parities, and, on erasure, recovers failed symbols locally (within group) or globally (across groups). It never sends bytes itself; it returns encoded fragments and recovery plans that the caller ships as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (LRC users)                      │
│  ids-erasure, ids-recovery, ids-ppr-reconstruction — encode/decode stripes   │
├──────────────────────────────────────────────────────────────────────────────┤
│             Phase 086: ids-locally-recoverable-codes (THIS CRATE)            │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐   │
│  │ lrc_construction│ │ locality_groups │ │ erasure_recovery │ │ locality│ │
│  │ (encode/decode) │ │ (partitioning)  │ │ (single/double)  │ │ params  │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐   │
│  │ repair_opt      │ │ r_delta_locality│ │ availability_t  │ │ puncturing│ │
│  │ (optimization)  │ │ (multi-erasure) │ │ (disjoint sets) │ │ shortening│ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐   │
│  │ hierarchical    │ │ maximally_recov │ │ finite_field    │ │ codecs    │ │
│  │ locality        │ │ (MR-LRC)        │ │ (RS/cyclic/BCH) │ │ (XOR/GF) │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘   │
└─────────────────────────────────────────────┬────────────────────────────────┘
                                              │ opaque Vec<u8> fragments + plans
                                              ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                     │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of an LRC encode:** caller supplies `k` data symbols + locality parameters `(r, l, g)` → `lrc_construction` partitions `k` data symbols into `l = ⌈k/r⌉` local groups of ≤ `r` symbols each → computes one local parity per group (XOR or GF-linear combination) → computes `g` global parities over all `k` data symbols → returns codeword of `n = k + l + g` symbols partitioned into locality groups.

**Data flow of a single-symbol recovery:** failed symbol `c_i` belongs to local group `R_j` → `erasure_recovery` reads the `r` surviving symbols in `R_j` (excluding `c_i`) + the local parity of `R_j` → reconstructs `c_i` via the local code (single-parity-check or MDS local code) → returns recovered symbol.

**Data flow of a double-erasure recovery within a locality:** two symbols fail in the same local group → requires `(r, δ=3)`-locality (local code distance ≥ 3) → recover both from the local group using the extended local parity structure, or fall back to global parities if local redundancy insufficient.

**LRC parameter semantics (research-verified):**
- `(n, k, r)` LRC: length `n`, dimension `k`, locality `r`. Each symbol recoverable from ≤ `r` others.
- Data blocks grouped into local groups of at most `r`; one local parity per group; `g` global parities over all data.
- Local group size ≤ `r + 1` (data + local parity). Single failure in a group needs `r` reads.
- `n = k + ⌈k/r⌉ + g` (Azure-LRC form). Fault tolerance: any `g + 1` failures; up to `g + i` failures spanning `i` local groups.
- Singleton-like bound: `d ≤ n − k − ⌈k/r⌉ + 2`. Distance-optimal LRC meets equality.
- Rate bound: `k/n ≤ r/(r+1)`.
- `(r, δ)`-locality: local code distance ≥ `δ`, tolerates `δ−1` erasures per group. Bound: `d ≤ n − k + 1 − (⌈k/r⌉ − 1)(δ − 1)`.
- Availability `t`: `t` disjoint repair sets per symbol; enables parallel/concurrent recovery.

**Crypto used:** sha256/blake3 for fragment checksums and locality-group IDs; ed25519 for signing encode/decode audit entries; xchacha20poly1305/chacha20poly1305 to encrypt symbol fragments at rest when confidentiality required. No other crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 LRC definition and basic parameters

**Source:** Error Correction Zoo (errorcorrectionzoo.org/c/locally_recoverable); Gopalan et al. (2012 ISIT); Tamo & Barg (IEEE TIT 2014); Papailiopoulos & Dimakis (2012 ISIT).

A code `C` of length `n` over alphabet `Q` is locally recoverable with locality `r` if for every coordinate `i ∈ [n]` there exists a subset `R_i ⊆ [n] \ {i}`, `|R_i| ≤ r`, and a function `φ_i` such that for every codeword `c ∈ C`, `c_i = φ_i({c_j : j ∈ R_i})`. `R_i` is called a repair set. An `(n, k, r)` LRC has length `n`, dimension `k`, locality `r`. The Singleton-like bound (Gopalan et al.): `d ≤ n − k − ⌈k/r⌉ + 2`. An LRC meeting this bound is distance-optimal. Rate bound: `k/n ≤ r/(r+1)`.

#### 3.2 LRC construction: local groups, local parity, global parity

**Source:** USENIX ATC'12 "Erasure Coding in Windows Azure Storage" (Huang et al.); USENIX ATC'18 "On Fault Tolerance, Locality, and Optimality in Locally Repairable Codes" (Kolosov et al.); VLDB'13 "XORing Elephants" (Sathiamoorthy et al.); ACM ToS 2025 survey "A Survey of the Past, Present, and Future of Erasure Coding for Storage Systems".

An `(k, l, g)` LRC divides `k` data fragments into `l` local groups of `⌈k/l⌉` (≤ `r`) fragments each. It encodes one local parity per local group and `g` global parities over all `k` data fragments. Total `n = k + l + g`. Any single fragment failure in a local group is decoded from the `k/l` fragments within its group (plus local parity). LRC achieves Maximally Recoverable property: tolerates up to `g + 1` arbitrary fragment failures, and `g + i` failures spanning `i` local groups.

- **Azure-LRC:** variation of Pyramid codes, used in Windows Azure Storage. `(12, 2, 2)` LRC chosen for 1.33x storage overhead target; MTTF 2.6×10¹² years; ~50% fewer blocks read per repair vs RS.
- **Xorbas / HDFS-RAID:** Facebook HDFS module replacing RS with LRC; 4 RS parities + local XOR parities; ~2× repair disk I/O and network reduction, 14% more storage.
- **Pyramid codes:** first LRC family, based on RS codes.

#### 3.3 Single erasure recovery within a locality

**Source:** USENIX ATC'18 (Kolosov et al.); ACM ToS 2025 survey; VLDB'13 Xorbas.

When one block fails in a local group of size `r + 1` (r data + 1 local parity), the missing block is recovered by reading the `r` surviving blocks in the group and computing the local parity equation. Repair degree = `r` (number of helper nodes), vs `k` for Reed-Solomon. For `(12, 2, 2)` Azure-LRC: repair reads 6 blocks vs 12 for RS(12,4). Degraded read similarly local.

#### 3.4 Double erasure recovery within a locality — (r, δ)-locality

**Source:** arXiv 1701.07340 "Locally Repairable Codes with Unequal Local Erasure Correction"; arXiv 1702.05741 "LRCs with Multiple (r_i, δ_i)-Localities"; arXiv 1507.02796 "LRCs with Functional Repair and Multiple Erasure Tolerance"; arXiv 2106.03302 "Rack-Aware Regenerating Codes".

An `(r, δ)`-LRC: every symbol has a local repair set of size `r + δ − 1` such that the code restricted to that set has distance ≥ `δ`, tolerating `δ − 1` concurrent erasures within the group. Singleton-like bound: `d ≤ n − k + 1 − (⌈k/r⌉ − 1)(δ − 1)`. For `δ = 2` this degenerates to the basic LRC bound. Double erasure within a locality requires `δ ≥ 3`. Sequential repair of multiple failures: order newcomers so each is repaired from live nodes plus previously repaired newcomers.

#### 3.5 Code with locality parameters (n, k, r, l, g, δ, t)

**Source:** ACM ToS 2023 "Practical Design Considerations for Wide Locally Recoverable Codes"; USENIX ATC'12 Azure; Error Correction Zoo; arXiv 1810.08510 "Alphabet-Dependent Bounds for Linear LRCs".

Parameter space:
- `n`: code length (total symbols), `k`: dimension (data symbols), `r`: locality.
- `l = ⌈k/r⌉`: number of local groups; `g`: number of global parities; `n = k + l + g`.
- `δ`: local distance (δ=2 → single-erasure local tolerance; δ≥3 → multi-erasure).
- `t`: availability (number of disjoint repair sets per symbol).
- Storage overhead: `n/k`. Fault tolerance `f = g + 1` (single-group) up to `g + i` spanning `i` groups.
- Rate: `k/n ≤ r/(r+1)`; distance-optimal when Singleton-like bound met with equality.

Wide-LRC examples (rate ≥ 0.85): 24-of-28 (n=28,k=24,r=2), 48-of-55 (n=55,k=48,r=3), 72-of-80 (n=80,k=72,r=4), 96-of-105 (n=105,k=96,r=5).

#### 3.6 Repair locality optimization

**Source:** USENIX ATC'18 (Kolosov et al.); USENIX FAST'19 OpenEC; SRDS'24 "Harmonizing Repair and Maintenance in LRC-Coded Storage"; arXiv 2106.03302; arXiv 2505.06819 "New Wide LRCs with Unified Locality".

Repair locality `r̄` = average number of blocks read to repair a block. Optimization targets:
- Minimize `r̄` by balancing local group sizes and placing local groups within fault domains / clusters (one local group per cluster → zero cross-cluster traffic for local repairs).
- Azure-LRC+1: treats global parities as a local repair group too, improving locality and avoiding cross-zone transfers.
- Uniform Cauchy LRC: lowest locality `⌈n/p⌉` by uniformly dividing `n` blocks into `p` groups.
- Trade-off: repair performance vs maintenance (upcoding/downcoding) performance cannot be simultaneously optimized; configurable data placement navigates the trade-off.
- LRC+PPR: Partial Parallel Repair applied to LRC reduces per-link chunks (e.g., 6 → 3 chunks for (12,2,2) LRC).

#### 3.7 Availability `t` and multiple disjoint repair sets

**Source:** Error Correction Zoo; arXiv 2004.06770 "Cyclic and convolutional codes with locality" (Barg, Chen); terpconnect.umd.edu "On codes with the locality property" (Barg newsletter); MDPI Mathematics 2026 "Optimal Binary LRCs with Locality and Availability from Latin Squares".

A symbol has `(r, t)`-availability if it has `t` disjoint repair sets `R_1(i), …, R_t(i)`, each `|R_j(i)| ≤ r`, pairwise disjoint. Enables `t` independent simultaneous recovery processes → high availability for hot data. Orthogonal partitions (from coprime-order subgroups of multiplicative/additive field groups) yield disjoint repair sets. MOLS-based binary constructions achieve distance-optimal LRCs with availability.

#### 3.8 Puncturing and shortening of LRCs

**Source:** arXiv 1909.10553 "Error Decoding of Locally Repairable and Partial MDS Codes"; arXiv 2105.11271 "Constructions of Optimal Binary LRCs via Intersection Subspaces"; Yonsei IWSDA'20 "concatenated binary LRC with locality 2 using puncturing".

- Shortening: decrease length and dimension simultaneously; `n' = n − a(r+1)`, `k' ≥ k − ar`, `d' ≥ d` (remove `a` full local groups).
- Puncturing: decrease length, keep dimension; removing a column from each distinct submatrix of the parity-check matrix yields `[n' = n − τ, k' = k − τ, d' = d]`.
- Used to tune LRC parameters to target `n`, `k`, `r` when exact divisibility `(r+1) | n` or `r | k` does not hold.

#### 3.9 Hierarchical locality and H-LRCs

**Source:** arXiv 2004.06770 (Barg, Chen); ISCDS monograph "Codes for Distributed Storage" (Section 13 hierarchical locality).

Hierarchical locality: layers of local codes with increasing block length and erasure-recovery capability. Permits graceful degradation in repair degree as the number of erasures increases — when one local code is overwhelmed, fall back to the next layer. Cyclic constructions characterize zeros supporting hierarchical locality; quasicyclic form enables tail-biting convolutional LRCs.

#### 3.10 Maximally recoverable LRCs (MR-LRCs)

**Source:** Dagstuhl LIPIcs ICALP'2019 "Constructions of Maximally Recoverable LRCs via Function Fields"; arXiv 2012.07804 "Improved MR-LRCs using Skew Polynomials"; ISCDS monograph Section 14.

An MR-LRC corrects all erasure patterns information-theoretically correctable given local recovery groups: `a` erasures per local group plus up to `h` additional erasures anywhere. Guarantees maximal fault tolerance under locality constraints. Field size is exponential in `h` or `a` in known constructions; minimizing field size is an open research area. MR-LRCs maximize mean time to data loss.

#### 3.11 Finite-field constructions

**Source:** arXiv 1502.01414 "Cyclic LRC Codes and their Subfield Subcodes"; arXiv 2007.14752 "New Constructions of Optimal Cyclic (r, δ)-LRCs from Their Zeros"; arXiv 1810.08510; arXiv 2112.02569 "Optimal Quaternary (r, δ)-LRCs"; Springer 2026 "Cyclic (r, δ) LRCs from constacyclic/negacyclic subcodes".

- Reed-Solomon-based: polynomial evaluation; locality via partition of evaluation points into subsets of size `r+1` where a polynomial is constant.
- Cyclic LRCs: zeros of generator polynomial contain cosets of `ν`-th roots of unity; cyclic shifts partition support into disjoint recovery sets of size `r+1`.
- Small-field / binary: cyclic codes, Reed-Muller, Simplex-based, tensor-product constructions.
- Quaternary `(r, δ)`-LRCs: complete classification via projective geometry over F₄.

#### 3.12 LRC vs Reed-Solomon and deployment evidence

**Source:** USENIX ATC'12 Azure; VLDB'13 Xorbas; USENIX ATC'18 Kolosov; DIMACS slides "Coding for Distributed Storage" (Dimakis); ACM ToS 2025 survey.

- RS(12,4): repair reads 12 chunks; Azure-LRC(12,2,2): reads 6 chunks (~50% reduction).
- Storage overhead: LRC 1.67x vs RS 1.5x for comparable params; Xorbas 14% more storage than RS.
- Repair disk I/O and network: ~2× reduction (Xorbas); 30–40% faster repairs in some cases.
- MTTF: Azure-LRC(6,2,2) 2.6×10¹² years vs RS(6,3) 6.1×10¹¹ vs 3-replication 3.5×10⁹.
- Deployed: Windows Azure Storage, Facebook HDFS-RAID/Xorbas, Ceph LRC plugin, OpenEC middleware.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Local group overwhelm (δ−1+ erasures in one group) | Data loss within group | `(r, δ)`-locality with δ≥3; fall back to global parities; hierarchical locality for graceful degradation |
| Global parity bottleneck | Repair of global parities needs all k data | Treat global parities as local group (Azure-LRC+1); locality-aware placement |
| Correlated failure across local groups | Multiple groups fail simultaneously | Fault tolerance `g + i` spanning `i` groups; rack/rack-aware placement via ids-topology |
| Locality-distance tradeoff (sacrificed d) | Reduced fault tolerance vs MDS | Distance-optimal construction target; MR-LRC option for maximal patterns |
| Parameter mismatch (r ∤ k, (r+1) ∤ n) | Invalid group partitioning | Puncturing/shortening transformations; parameter validation at construction |
| Symbol fragment corruption | Silent data corruption | sha256/blake3 checksums per fragment; verify before use in recovery |
| Replay of encoded fragments | Stale data | ed25519-signed encode/decode audit entries; nonce/version binding |
| Carrier duplicating/delaying fragment datagrams | Reordering/duplication | Carrier-agnostic; idempotent XOR/GF reconstruction tolerates duplicates |
| Prohibited crypto/networking | Policy violation | Allowed-crypto allowlist; dependency audit bans std::net/tokio::net/libp2p |
| Key extraction (fragment signing key) | Decision authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305) |

The carrier is never trusted: LRC fragments and recovery plans are signed/checksummed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-locally-recoverable-codes ||
|-------|------|--------------------------|----------------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All LRC fragment traffic flows to/from `Endpoint`; crate returns fragments/plans, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Fragment checksums, audit signing, optional encryption of fragments at rest |
| `ids-erasure` (Phase 085/related) | General erasure-coding interface | `ids-locally-recoverable-codes` is a locality-aware erasure-code implementation behind the erasure contract |
| `ids-recovery` (Phase 085+) | Erasure-coded storage repair | Drives LRC single/double-erasure recovery; consumes recovery plans |
| `ids-ppr-reconstruction` (Phase 087) | Distributed erasure-coded storage repair (PPR) | LRC+PPR: PPR applied on top of LRC further reduces per-link repair traffic |
| `ids-topology` | Fault-domain / placement awareness | Locality-group placement across fault domains/racks/regions; correlated-failure avoidance |
| `ids-fragment` | Record fragments/shards | LRC operates on symbol fragments; fragment-to-symbol mapping |

**Integration points:** `ids-locally-recoverable-codes` implements the `Erasure` sealed trait contract declared in `ids-000-aa` (encode/decode/repair). It never opens sockets; all fragment movement is opaque `Vec<u8>` to `ids-carrier::Endpoint`. It depends on `ids-000-aa`, `ids-carrier`, and `ids-crypto` only.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-locally-recoverable-codes` (new crate at `C:/idfs/ids-locally-recoverable-codes/`; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`):

```
ids-locally-recoverable-codes/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # LRC defaults: group size, g, δ, t, field size
├── error.rs                            # LrcError (thiserror): construction, recovery, parameter errors
├── features.rs                         # Feature flags: rs_field, cyclic, binary, mr_lrc, hierarchical
│
├── types.rs                            # LrcParams, LocalityGroup, SymbolId, Fragment, Codeword, RepairPlan (re-exports ids-000-aa types)
├── lrc_construction.rs                 # Encode/decode: partition data, compute local+global parities
├── locality_groups.rs                  # Locality group partitioning, group assignment, group metadata
├── erasure_recovery.rs                 # Single/double erasure recovery within locality, global fallback
├── locality_params.rs                  # (n,k,r,l,g,δ,t) parameter computation, validation, bounds
├── repair_optimization.rs              # Repair-locality optimization, group balancing, placement hints
├── r_delta_locality.rs                 # (r, δ)-locality: multi-erasure within group, δ≥2
├── availability.rs                     # Availability t: disjoint repair sets, parallel recovery
├── puncturing.rs                       # Puncturing/shortening transformations for parameter tuning
├── hierarchical.rs                     # Hierarchical locality: layered local codes, graceful degradation
├── maximally_recoverable.rs            # MR-LRC: maximal fault-tolerance patterns (feature-gated)
├── finite_field.rs                     # Finite-field arithmetic (RS/cyclic/BCH) and XOR codecs
├── codecs/                             # per-codec modules (~10 files)
│   ├── mod.rs
│   ├── xor_codec.rs                    # Binary XOR local parity (Azure-LRC style)
│   ├── rs_codec.rs                     # Reed-Solomon-based LRC over GF(2^m)
│   ├── cyclic_codec.rs                 # Cyclic LRC codec
│   ├── bch_codec.rs                    # BCH-based small-field LRC
│   ├── gabidulin_codec.rs              # Gabidulin outer code for unequal locality
│   ├── mr_codec.rs                     # MR-LRC codec (function-field/skew-polynomial)
│   └── ...
├── contracts/                          # sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── lrc_contract.rs                 # trait LrcCode (encode/decode/repair)
│   ├── locality_contract.rs            # trait LocalityGroup (partitioning)
│   ├── recovery_contract.rs            # trait ErasureRecovery (single/double)
│   ├── params_contract.rs              # trait LocalityParams (parameter bounds)
│   ├── repair_contract.rs              # trait RepairOptimization
│   ├── availability_contract.rs        # trait Availability (disjoint sets)
│   ├── codec_contract.rs               # trait Codec (field arithmetic)
│   └── checksum_contract.rs            # trait FragmentChecksum (sha256/blake3)
├── invariants/                         # compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── singleton_bound.rs              # d ≤ n − k − ⌈k/r⌉ + 2 check
│   ├── rate_bound.rs                   # k/n ≤ r/(r+1)
│   ├── group_size.rs                   # local group ≤ r + 1
│   ├── fault_tolerance.rs              # g + i failures spanning i groups
│   ├── carrier_agnostic.rs             # no std::net/tokio::net usage
│   ├── crypto_allowlist.rs             # only allowed crypto
│   ├── checksum_sound.rs               # fragment checksum verification
│   └── parameter_valid.rs              # (n,k,r,l,g,δ,t) consistency
├── audit/                              # signed, hash-chained audit log (~6 files)
│   ├── audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs, mod.rs
├── tests/                              # property/verification tests (~30 files)
├── benches/                            # (~10 files)
├── docs/                               # (~14 files)
├── spec/                               # normative specifications (~6 files)
└── test_vectors/                       # deterministic encode/decode/recovery vectors
```

Total: ~90–1000 files across `src/`, `src/codecs/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, `spec/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 LRC construction (encode)
```
LrcParams { n, k, r, l=⌈k/r⌉, g, δ, t, field }
partition(data[0..k], l groups of ≤ r) → groups[0..l-1]
for each group j: local_parity[j] = XOR or GF-combination of group j
for i in 0..g: global_parity[i] = GF-linear-combination of all data[0..k]
codeword = data + local_parities + global_parities   (n = k + l + g symbols)
```

#### 7.2 Single erasure recovery within locality
```
failed symbol c_i ∈ group j
read surviving r symbols in group j (excluding c_i) + local_parity[j]
c_i = local_parity[j] XOR/GF-sum(survivors)   (single-parity local code, δ=2)
repair degree = r  (vs k for Reed-Solomon)
```

#### 7.3 Double erasure recovery within locality — (r, δ≥3)
```
two failures c_i, c_j in group j
requires local code distance ≥ 3 (δ=3)
recover both from local group using extended local parity structure
if insufficient local redundancy → fall back to global parities (read k symbols)
```

#### 7.4 Singleton-like bound check
```
d ≤ n − k − ⌈k/r⌉ + 2   (basic LRC)
d ≤ n − k + 1 − (⌈k/r⌉ − 1)(δ − 1)   ((r, δ)-locality)
distance-optimal iff equality
```

#### 7.5 Rate bound
```
k/n ≤ r/(r+1)
storage overhead n/k ≥ (r+1)/r
```

#### 7.6 Fault tolerance
```
tolerates any g + 1 arbitrary failures
tolerates g + i failures spanning i local groups (1 ≤ i ≤ l)
failures beyond → unrecoverable (data loss)
```

#### 7.7 Availability t (disjoint repair sets)
```
t disjoint repair sets R_1(i)…R_t(i) per symbol, each ≤ r, pairwise disjoint
enables t independent simultaneous recoveries of symbol i
```

#### 7.8 Puncturing / shortening
```
shorten: remove a full local groups → n' = n − a(r+1), k' ≥ k − ar, d' ≥ d
puncture: remove τ columns from parity-check submatrices → n' = n − τ, k' = k − τ, d' = d
```

#### 7.9 Repair-locality optimization
```
repair_locality r̄ = average helpers per repair
balance local group sizes; place one local group per fault domain/cluster
Azure-LRC+1: global parities as local group → zero cross-cluster traffic for parity repair
```

#### 7.10 Fragment checksum
```
checksum = sha256(fragment) or blake3(fragment) per symbol
verify before including in recovery; checksum chain in audit log
```

---

### 8. Dependencies

|| Crate | Purpose ||
|-------|---------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, `Error`) and sealed trait contracts (`Erasure`, …) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — fragment transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` | Serialization of LRC envelopes, repair plans, audit entries |
| `thiserror` | Error types |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-locally-recoverable-codes` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-086.1 | LRC construction: encode `k` data symbols into `(n, k, r)` codeword with `l = ⌈k/r⌉` local groups and `g` global parities | `n = k + l + g`; each group ≤ `r + 1` symbols |
| AC-086.2 | Locality: every symbol recoverable from ≤ `r` other symbols | Repair set size ≤ `r` for all `n` symbols |
| AC-086.3 | Singleton-like bound: `d ≤ n − k − ⌈k/r⌉ + 2`; distance-optimal if equality | Bound check passes on all constructed codes |
| AC-086.4 | Rate bound: `k/n ≤ r/(r+1)` | Verified per parameter set |
| AC-086.5 | Single erasure recovery within locality: repair degree `r` vs `k` for RS | Reads `r` symbols (not `k`) for single failure in group |
| AC-086.6 | Double erasure recovery within locality: `(r, δ≥3)`-locality recovers 2 failures in one group | Recovers 2 concurrent erasures per group when δ≥3 |
| AC-086.7 | Fault tolerance: tolerates `g + 1` arbitrary failures; `g + i` spanning `i` groups | Failure-pattern testing up to tolerance boundary |
| AC-086.8 | Availability `t`: `t` disjoint repair sets per symbol | `t` pairwise disjoint sets, each ≤ `r` |
| AC-086.9 | Puncturing/shortening: parameter tuning preserves `d' ≥ d` | Shortened/punctured code meets distance bound |
| AC-086.10 | Repair-locality optimization: `r̄` minimized via group balancing + placement | `r̄` measured; group-per-cluster placement yields zero cross-cluster local repair |
| AC-086.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) passes; source grep zero matches |
| AC-086.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-086.13 | Every encode/decode/recovery recorded in append-only, hash-chained (sha256/blake3), ed25519-signed audit log with `NodeId` + symbol IDs | Audit log verification passes |
| AC-086.14 | Crate compiles cleanly | `cargo check -p ids-locally-recoverable-codes` exits 0 with no warnings |
| AC-086.15 | Tests pass | `cargo test -p ids-locally-recoverable-codes` passes all construction/recovery/invariant tests |

---

### 10. Remaining Uncertainties

1. **Concrete `(n, k, r, l, g)` defaults:** deployment-tuned per workload (e.g., Azure `(12,2,2)`, Xorbas `(16,10,5)`); to be validated at integration with `ids-erasure`/`ids-recovery`.
2. **Field size vs locality tradeoff:** optimal LRC constructions require specific field sizes (MR-LRCs exponential in `h`/`a`); small-field binary/cyclic options trade distance for field size — selection pending.
3. **`(r, δ)`-locality δ default:** δ=2 (single-erasure local) vs δ≥3 (multi-erasure) affects storage overhead and field size; workload-dependent.
4. **Availability `t` default:** number of disjoint repair sets; higher `t` improves concurrent-repair availability at storage cost — unquantified.
5. **Hierarchical locality layer count:** number of layers and per-layer block length trade graceful degradation against complexity — design decision pending.
6. **MR-LRC inclusion:** maximally recoverable LRCs give maximal fault tolerance but large field sizes; feature-gated, inclusion decision pending.
7. **LRC+PPR integration:** PPR applied on LRC (Phase 087) further reduces per-link traffic; exact integration interface with `ids-ppr-reconstruction` to be defined.
8. **Checksum algorithm choice per fragment:** sha256 vs blake3 trade security vs speed; per-fragment configuration pending.
9. **Placement locality vs fault-domain constraints:** one-local-group-per-cluster optimizes repair locality but may conflict with rack/region fault-tolerance placement — resolved via `ids-topology` integration.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-locally-recoverable-codes` (not a folder) | Follows "separate crates instead of more folders" directive; clear erasure-coding boundary |
| `(k, l, g)` Azure-LRC form: `n = k + l + g`, `l = ⌈k/r⌉` local groups, `g` global parities | Proven in production (Azure, HDFS-Xorbas); simple XOR local parities + RS global parities |
| Locality `r` = max helpers for single-symbol repair | Directly minimizes repair degree; `r << k` vs Reed-Solomon's `k` |
| Singleton-like bound `d ≤ n − k − ⌈k/r⌉ + 2` as distance ceiling | Gopalan et al. information-theoretic bound; distance-optimal target |
| `(r, δ)`-locality for double-erasure within locality | Generalizes single-erasure; δ≥3 tolerates δ−1 concurrent erasures per group |
| Availability `t` via disjoint repair sets | Enables parallel/concurrent recovery; hot-data availability |
| Puncturing/shortening for parameter tuning | Handles non-divisible `(r+1) ∤ n`, `r ∤ k` cases; preserves distance |
| Hierarchical locality for graceful degradation | Layers of local codes; fall back when one layer overwhelmed |
| XOR local parities (binary) + GF global parities | Azure-LRC/Xorbas pattern; XOR avoids GF multiplication cost locally |
| All traffic via `ids-carrier::Endpoint`; crate returns fragments/plans, never sends | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only; via `ids-crypto` primitive IDs | IDS crypto policy |
| sha256/blake3 fragment checksums + ed25519-signed audit log | Fragment integrity + tamper-evident encode/decode audit |

---

### 12. Sources

[^1]: Gopalan et al., "Locally Repairable Codes," 2012 IEEE ISIT (DOI 10.1109/ISIT.2012.6284027). Singleton-like bound `d ≤ n − k − ⌈k/r⌉ + 2`; LRC introduction; repair locality metric.
[^2]: Tamo & Barg, "A Family of Optimal Locally Recoverable Codes," IEEE Trans. Info. Theory 60(8):4661–4676, 2014 (DOI 10.1109/TIT.2014.2321280). Polynomial-evaluation LRCs reducing to Reed-Solomon at r=k; multiple recovering sets.
[^3]: Huang et al., "Erasure Coding in Windows Azure Storage," USENIX ATC'12. Azure-LRC `(k, l, g)`; MTTF 2.6×10¹² years; (12,2,2) at 1.33x overhead; 20 fault domains.
[^4]: Sathiamoorthy et al., "XORing Elephants: Novel Erasure Codes for Big Data," VLDB'13. HDFS-Xorbas; ~2× repair disk I/O and network reduction; 14% more storage than RS.
[^5]: Kolosov et al., "On Fault Tolerance, Locality, and Optimality in Locally Repairable Codes," USENIX ATC'18 / ACM ToS 2020. Comparison of Xorbas, Azure-LRC, Optimal-LRC; fault tolerance `g + i` spanning `i` groups; repair-degree metrics.
[^6]: Error Correction Zoo, "Locally recoverable code (LRC)." https://errorcorrectionzoo.org/c/locally_recoverable. LRC definition, repair set, Singleton-like bound, rate bound `k/n ≤ r/(r+1)`, Azure (18,14,7), Facebook f4.
[^7]: arXiv 1701.07340, "Locally Repairable Codes with Unequal Local Erasure Correction." `(r, δ)`-locality bound `d ≤ n − k + 1 − (⌈k/r⌉ − 1)(δ − 1)`; unequal locality profiles.
[^8]: arXiv 1702.05741, Chen, Xia, Hao, "LRCs with Multiple (r_i, δ_i)-Localities." Multiple localities; hot data; optimal ML-LRC constructions.
[^9]: arXiv 1507.02796, Song & Yuen, "LRCs with Functional Repair and Multiple Erasure Tolerance." `(n,k,r,t)` functional LRCs; sequential repair; rate bound `k/n ≤ r/(r+2)` for t=2.
[^10]: arXiv 2004.06770, Chen & Barg, "Cyclic and convolutional codes with locality." Hierarchical LRCs; cyclic zeros characterization; availability via bi-cyclic codes.
[^11]: arXiv 1810.08510, "Alphabet-Dependent Bounds for Linear Locally Repairable Codes." All-symbol locality; availability; cooperative repair; local repair on graphs.
[^12]: arXiv 2112.02569, "Optimal Quaternary (r, δ)-Locally Recoverable Codes." Complete classification over F₄ via projective geometry.
[^13]: LIPIcs ICALP'2019, Xing & Yuan, "Construction of Optimal Locally Recoverable Codes and Connection with Hypergraph." Optimal LRC length bounds; algebraic geometry constructions.
[^14]: arXiv 2012.07804, "Improved Maximally Recoverable LRCs using Skew Polynomials." MR-LRC field-size improvements; function-field constructions.
[^15]: arXiv 1909.10553, "Error Decoding of Locally Repairable and Partial MDS Codes." Puncturing/shortening transformations; `(r, ρ)`-locality; PMDS/MR-LRC definitions.
[^16]: arXiv 2105.11271, "Constructions of Optimal Binary LRCs via Intersection Subspaces." Binary cyclic LRCs; shortening; d≥6, locality 2^b.
[^17]: terpconnect.umd.edu/~abarg, "On codes with the locality property" (2016 newsletter). LRC(2) codes; disjoint recovery sets; orthogonal partitions via coprime subgroups.
[^18]: MDPI Mathematics 14(8):1321 (2026), "Optimal Binary LRCs with Locality and Availability from Latin Squares." MOLS-based binary LRCs; distance- and rate-optimal; nonuniform localities.
[^19]: MDPI Entropy 27(3):269 (2025), Zhang & Zeng, "Binary LRCs with Nonuniform Locality and Availability Using Combinatorial Designs." PBD/BIBD constructions; IS/AS availability; (r,t)-availability definitions.
[^20]: arXiv 2505.06819, "New Wide Locally Recoverable Codes with Unified Locality." OLRC/ULRC at Google; recovery locality `r̄`; one-local-group-per-cluster; XOR encoding.
[^21]: arXiv 2106.03302, "Rack-Aware Regenerating Codes with Multiple Erasure Tolerance." LRC fault tolerance; rack-aware placement; local repair sets.
[^22]: arXiv 1908.01527, "Repair Pipelining for Erasure-Coded Storage." HDFS-RAID/HDFS-3/QFS background; LRC+PPR; repair pipelining.
[^23]: USENIX FAST'19, "OpenEC: Unified and Configurable Erasure Coding Management." LRC vs RS vs MSR throughput; LRC(10,6) local groups of 3.
[^24]: SRDS'24, "Harmonizing Repair and Maintenance in LRC-Coded Storage." Repair vs maintenance trade-off; configurable placement; rack-based HDFS prototype.
[^25]: ACM ToS 2025, "A Survey of the Past, Present, and Future of Erasure Coding for Storage Systems." LRC taxonomy; Azure-LRC `(k,l,g)`; wide LRCs; repair-bandwidth trade-offs.
[^26]: DIMACS slides, Dimakis, "Coding for Distributed Storage" (2015). Deployed systems: Azure LRC, HDFS Xorbas, Facebook f4, Ceph LRC plugin.
[^27]: arXiv 2001.04711, "Partial MDS Codes with Local Regeneration." PMDS/SD codes; local regenerating MDS; maximally recoverable.
[^28]: arXiv 1607.04137, "Balanced Locally Repairable Codes." Storage overhead, repair bandwidth, MTTDL metrics; update complexity.
[^29]: Springer Cryptography & Communications (2026), "Cyclic (r, δ) LRCs from constacyclic/negacyclic subcodes." Cyclic LRC conditions from zeros.
[^30]: arXiv 1502.01414, "Cyclic LRC Codes and their Subfield Subcodes." Reed-Solomon-like cyclic LRC construction; `(r+1) | n`, `r | k`.
[^31]: IDS prompts.txt entry 86 (lines 1285-1299), C:/idfs/prompts.txt — Phase 086 crate definition, contents, constraints.
[^32]: Existing ids-000-aa skeleton, C:/idfs/ids-000-aa/ — canonical types, sealed contracts, invariants, errors referenced by this crate.
[^33]: HX-241 reference plan format — C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture, Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document.
