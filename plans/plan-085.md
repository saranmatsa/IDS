## Phase 085 — Create the ids-regenerating-codes crate: Bandwidth-efficient node repair.

**Research complete (30 targeted searches executed). No code written, no source files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1270-1284)

```
85. Research only do not read plan.md **[Phase 085 — Create the ids-regenerating-codes crate: Bandwidth-efficient node repair.]**

Crate: ids-regenerating-codes
Purpose: Bandwidth-efficient node repair.
Contents: Regenerating code construction, repair bandwidth minimization, helper node access minimization, functional vs exact repair, regenerating code parameters.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 085 — Create the ids-regenerating-codes crate: Bandwidth-efficient node repair.]**

Create plan-085.md in C:/idfs/plans/ for the ids-regenerating-codes crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 085 creates `ids-regenerating-codes`, a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by code-construction and repair-concern. Its purpose is bandwidth-efficient node repair: instead of reconstructing a failed storage node by downloading the whole file (naive Reed-Solomon repair contacts k helpers and downloads ~k·α symbols ≈ the entire file B), a regenerating code repairs a failed node by contacting d ≥ k helper nodes and downloading only β ≤ α symbols from each, for a total repair bandwidth γ = dβ that is typically far smaller than B.

The crate specifies regenerating-code constructions, the repair-bandwidth minimization problem, helper-node access minimization, functional-versus-exact repair modes, and regenerating-code parameters {n, k, d, α, β, B}. It is a **research-and-specification crate**: it defines code parameters, the cut-set bound, the MSR/MBR operating points, repair plans, helper-selection policies, and the carrier/crypto boundary; it does not open sockets or drive any transport. Every inter-node repair message is an opaque `Vec<u8>` payload addressed to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address, capped at 1024 bytes). Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-regenerating-codes` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-regenerating-codes` crate does NOT yet exist at `C:/idfs/ids-regenerating-codes/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `Checksum`, `Error`, sealed `Carrier` contract, …). Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned the Dimakis et al. survey "A Survey on Network Codes for Distributed Storage" (arXiv:1004.4438, 2010) introducing regenerating codes and showing maintenance bandwidth reduced by orders of magnitude vs Reed-Solomon; the cut-set bound B ≤ Σᵢ₌₀^{k−1} min{α, (d−i)β}; the MSR point (α = B/k, β = α/(d−k+1), γ = dα/(d−k+1)) and MBR point (α = dβ = 2dB/(k(2d−k+1)), rate ≤ 1/2); the product-matrix construction of Rashmi/Shah/Kumar/Vijay Kumar (IEEE TIT 2011) giving optimal exact-regenerating MSR codes for d ≥ 2k−2 and MBR codes for all n,k,d; zigzag codes of Tamo & Barg (optimal-repair MDS array codes, d = n−1, small fields q=3, optimal access/update); the MISER code (Cadambe et al., interference alignment, d = n−1 ≥ 2k−1, exact repair of systematic nodes; impossibility of linear exact-repair MSR for d < 2k−3 without symbol extension); Clay codes (USENIX FAST'18, Vajha et al.: MSR codes moulded from MDS codes, ~2.9× less repair traffic and ~3× faster repair vs Reed-Solomon, with a Rust crate `clay-codes` on docs.rs/crates.io and a Ceph implementation); small-d MSR codes (d ∈ {k+1, k+2, k+3}, field size O(n)); generalized regenerating codes on graphs (nonuniform helper download); cooperative regenerating codes (joint repair of h failures, cut-set bound RB ≥ h(h+d−1)l/(h+d−k)); locally repairable codes vs regenerating codes (locality vs bandwidth tradeoff); secure regenerating codes (information-theoretic security against eavesdropping helpers); DoubleR repair layering atop HDFS (Facebook reports a median 180 TB/day of repair bandwidth in erasure-coded clusters; Azure 1.33× and Facebook 1.4× redundancy vs 3× triple replication); Rust erasure-coding crates `clay-codes`, `reed-solomon-simd`, `tape-reed-solomon`, `erase`, `rlnc` (RLNC over GF(2⁸)).

---

### 2. Architecture Overview

`ids-regenerating-codes` sits above `ids-000-aa` (canonical types + sealed trait contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a code-construction and repair-planning layer: given a storage layout (n nodes, α symbols each), it computes code parameters, builds repair plans (which d helpers to contact, what β-symbol repair fragment each helper sends), selects repair mode (functional or exact), and verifies repaired fragments. It never sends bytes itself; it returns `RepairPlan` / `Fragment` / `VerifyResult` objects that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   STORAGE / REPAIR LAYER (ids-* clients)                    │
│  ids-erasure, ids-repair, ids-recovery — store coded fragments, trigger     │
│  node repair, ingest RepairPlan, ship fragments as opaque datagrams         │
├─────────────────────────────────────────────────────────────────────────────┤
│               Phase 085: ids-regenerating-codes (THIS CRATE — spec/plans)  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──┐ │
│  │ parameters   │ │ cut-set      │ │ MSR / MBR    │ │ functional   │ │  │ │
│  │ {n,k,d,α,β,  │ │ bound        │ │ operating    │ │ repair mode  │ │  │ │
│  │  B}          │ │ tradeoff     │ │ points       │ │              │ │  │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──┐ │
│  │ exact        │ │ repair plan  │ │ helper       │ │ fragment &   │ │  │ │
│  │ repair mode  │ │ (d helpers,  │ │ selection    │ │ reconstruction│ │  │ │
│  │              │ │ β each)      │ │ policy       │ │ verification │ │  │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──┐ │
│  │ constructions│ │ crypto       │ │ audit        │ │ contracts +  │ │  │ │
│  │ (PM, zigzag, │ │ boundary     │ │ log          │ │ invariants   │ │  │ │
│  │  clay, …)    │ │ (allowlist)  │ │              │ │ (~20 files)  │ │  │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──┘ │
└──────────────────────────────────────────┬────────────────────────────────┘
                                           │ opaque Vec<u8> datagrams
                                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│             CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload)      │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS     │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh anywhere in this crate        │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one node repair:** failure of node f detected → `parameters` validates {n,k,d,α,β,B} against the cut-set bound → `repair_plan` selects d helper nodes from the n−1 survivors per the helper-selection policy (blind / stationary / dynamic) → each helper computes its β-symbol repair fragment from its stored α symbols using the repair matrix for the chosen construction (product-matrix / zigzag / clay / …) → fragments shipped as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets of the helpers, and the newcomer collects them → `reconstruct` rebuilds the α symbols of the failed node → `verify` checks sha256/blake3 fragment checksums and the ed25519-signed repair manifest → node rejoins the roster.

**Repair-mode semantics (research-verified):**
- **Functional repair**: the regenerated node needs only to be functionally equivalent to the failed node (any k nodes still reconstruct the file). Random linear network coding achieves the full cut-set storage–bandwidth tradeoff for all parameters; simplest to construct, but the newcomer's content differs from the failed node's.
- **Exact repair**: the newcomer reproduces the exact contents of the failed node (keeps the code systematic/MDS in the same coordinates). Harder: linear exact-repair MSR codes are impossible for d < 2k−3 without symbol extension (Cadambe et al.); explicit exact-repair MSR constructions exist for d ≥ 2k−2 (product-matrix) and for d = n−1 (zigzag, Hadamard-based, MISER).

**Crypto used (allowed set only):** ed25519 for signing repair manifests and repair decisions; sha256/blake3 for fragment checksums and code-parameter hashing; xchacha20poly1305/chacha20poly1305 to encrypt repair datagrams in flight when confidentiality is required (24-byte random nonce safe for xchacha20poly1305; 12-byte counter-based nonce for chacha20poly1305).

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline; full list in Section 12.

#### 3.1 Regenerating-code framework and parameters {n, k, d, α, β, B}

A regenerating code over finite field F_q maps a file of B symbols onto n α-tuples (one per node). Any k nodes suffice to reconstruct the file (MDS/erasure property). When node f fails, a newcomer connects to any d surviving helper nodes (k ≤ d ≤ n−1) and downloads β ≤ α symbols from each; the total dβ is the repair bandwidth γ. Node size α is the sub-packetization level. Sources: Dimakis et al. survey (arXiv:1004.4438); arxiv:1806.04437 (parameter set and tradeoff); CMU DSS report (cs.cmu.edu codes-DSS-report.pdf).

#### 3.2 Cut-set bound and storage–repair-bandwidth tradeoff

The network-coding cut-set bound gives B ≤ Σ_{i=0}^{k−1} min{α, (d−i)β}. A code is optimal if it meets this bound and reducing either α or β violates it. Given (k, d), multiple (α, β) pairs satisfy the bound, yielding the tradeoff curve; the two extremal points are MSR and MBR, interior points are space-sharable mixtures. Sources: arxiv:1806.04437; arxiv:1211.1932 (Codes with Local Regeneration); CMU DSS report.

#### 3.3 MSR point — Minimum Storage Regenerating codes

First minimize α (α = B/k, i.e., MDS storage efficiency), then minimize β: β_MSR = α/(d−k+1) = B/(k(d−k+1)), total repair bandwidth γ_MSR = dα/(d−k+1). MSR codes are vector MDS codes; rate can approach 1. For d = n−1 and k close to n, γ_MSR can be dramatically below the naive RS repair cost k·α ≈ B (e.g., (n,k,d) = (21,10,20) gives >5× communication reduction vs downloading from k nodes). Sources: arxiv:1806.04437; arxiv:1704.03696 (DoubleR field notes).

#### 3.4 MBR point — Minimum Bandwidth Regenerating codes

First minimize β (down to what must be stored, dβ = α), then α: β_MBR = 2B/(k(2d−k+1)), α_MBR = dβ = 2dB/(k(2d−k+1)). Minimum total bandwidth overall, but maximum storage overhead; rate ≤ 1/2, achieved at k = d = n−1. Sources: arxiv:1806.04437; CMU DSS report.

#### 3.5 Functional vs exact repair

Functional repair: regenerated node is functionally equivalent; full cut-set tradeoff achievable for all parameters via random linear network coding (Dimakis et al. 2010). Exact repair: newcomer reproduces failed node's exact contents. Tension: no linear exact-repair MSR code achieves the cut-set bound for d < 2k−3 without symbol extension (MISER paper, arxiv:1005.1634); explicit exact-repair MSR for d ≥ 2k−2 (product-matrix), d = n−1 (zigzag, Hadamard designs, MISER systematic nodes). Exact repair keeps code coordinates stable (important for ids-* placement/replication invariants); functional repair is simpler but changes node content. Sources: arxiv:1005.1634; product-matrix paper (cs.cmu.edu/~nihars/Product_matrix_codes.pdf); Hadamard-design repair-optimal codes (IEEE Explore 6416066 / 6483351).

#### 3.6 Product-matrix construction (Rashmi–Shah–Kumar–Vijay Kumar, IEEE TIT 2011)

Optimal exact-regenerating codes at both MSR and MBR points via product-matrix construction. MBR: all n,k,d. MSR: d ≥ 2k−2 (low rate k/n ≤ ~1/2). Small sub-packetization, practical; basis for the `clay-codes` Rust crate lineage via Ceph. Explicit code matrices over F_q; repair is linear (helper multiplies stored vector by repair matrix, sends β symbols). Sources: Product_matrix_codes.pdf (CMU); arxiv:1101.0133 context.

#### 3.7 Zigzag codes (Tamo & Barg, IEEE TIT 2013)

MDS array codes with optimal rebuilding ratio 1/2 for single erasure of a 2-erasure-correcting code; general (n,k,d=n−1) optimal repair; small finite field (q = 3 for n/k = 2); optimal access (helpers send stored symbols directly, no computation) and optimal update properties. Sub-packetization α = r^{k/r} grows with k; high rate (t−1)/t constructions via coupled layers (arxiv:1607.07335). Sources: arxiv:1005.1634 references; arxiv:1607.07335; arxiv:1101.0133 (rebuilding ratio).

#### 3.8 Clay codes (Vajha et al., USENIX FAST'18)

MSR codes "moulded" from MDS codes: same fault tolerance and storage overhead as Reed-Solomon, but repair downloads only a fraction of each helper's data. Measured ~2.9× repair-traffic reduction and ~3× repair-time reduction vs Reed-Solomon; encode time ~70% higher (PFT/PRT operations). Implemented in Ceph (PR #14300); Rust crate `clay-codes` on docs.rs/crates.io. Directly relevant: an ids-* implementation target and a bandwidth benchmark (AC-08.8 analog: ≥2× reduction vs naive RS). Sources: usenix.org fast18_slides_vajha.pdf; docs.rs/crate/clay-codes.

#### 3.9 Reed-Solomon naive repair vs regenerating codes

Naive RS repair of one failed node contacts k helpers and downloads k full chunks (effectively the whole file B = kα) — repair bandwidth ≈ B regardless of d. Regenerating codes download dβ ≪ B by transferring coded linear combinations of fragments. Measured production impact: Facebook erasure-coded data center generates a median 180 TB/day of repair bandwidth; limiting factor for foreground jobs — precisely the problem this crate addresses. Sources: arxiv:1704.03696 (DoubleR); CMU DSS report.

#### 3.10 Helper-node selection and access minimization

Blind repair: newcomer contacts all n−1 nodes. Stationary repair: each node has a fixed helper set of size d. Dynamic helper selection: choose the "good" helpers (low load, high bandwidth, proximity); replication is sometimes more efficient than regenerating codes when only one good helper exists (selection reduces I/O and bandwidth). Access minimization = minimizing symbols read at helpers (optimal-access codes send exactly β read symbols, no local computation). Sources: IEEE Explore 7028491 (helper selection); arxiv:1101.0133; arxiv:1211.1932.

#### 3.11 Sub-packetization α and field size

α is the number of symbols per node; optimal exact repair typically needs large α (lower bounds α ≥ s^{n/s}, s = d−k+1, for optimal-access MSR). Field size q must support the construction (Cauchy matrices: q ≥ α + n − k; product-matrix: moderate; zigzag: q = 3 possible). Tradeoff: small field ⇒ cheaper arithmetic but larger sub-packetization; large field ⇒ smaller α. Sources: arxiv:1710.05876 (sub-packetization lower bounds); arxiv:1804.00598 (small-d MSR, linear field size O(n)); arxiv:2610.15329 (Ye–Barg family, small sub-packetization).

#### 3.12 Cooperative regenerating codes (multiple simultaneous node failures)

Repairing h failed nodes jointly (cooperation among newcomers) reduces total repair bandwidth vs separate repair. Cut-set bound for cooperative repair: RB ≥ h(h+d−1)l/(h+d−k). Relevant when several IDS nodes fail in the same window (correlated failures covered by ids-000-aa `correlated_failure` invariant). Sources: arxiv:1207.6762 (Cooperative Regenerating Codes); arxiv:1806.04437 (MSCR/MBCR points).

#### 3.13 Locally repairable codes (LRC) vs regenerating codes

LRCs minimize the number of helpers contacted (repair locality r ≪ k) by adding locality redundancy; regenerating codes minimize bytes downloaded (β < α from many helpers). They are complementary: locally regenerating codes combine both (local MSR/MBR codes). ids-regenerating-codes focuses on bandwidth; locality belongs to ids-locally-recoverable-codes (Phase 086). Sources: IEEE Explore 9174781 / 7541379; arxiv:1211.1932.

#### 3.14 Secure regenerating codes

Information-theoretically secure regenerating codes prevent eavesdropping helpers from learning the file; secrecy capacity derived at MBR point; built on product-matrix codes with randomization. Maps directly to the allowed-crypto rule: repair datagrams can be authenticated (ed25519) and encrypted (xchacha20poly1305/chacha20poly1305). Sources: GLOBECOM 2011 (10.1109/glocom.2011.6133754); arxiv:1505.01986 (secure MSR).

#### 3.15 Production evidence and Rust implementation landscape

- DoubleR (arxiv:1704.03696): repair layering deployed atop HDFS; cross-rack bandwidth minimized; node recovery + degraded reads.
- Azure (1.33× redundancy) and Facebook (1.4×) vs 3× triple replication — erasure coding is in production; repair bandwidth is the dominant cost.
- Rust crates: `clay-codes` (docs.rs — MSR codes, ClayCode::new(4,2,5) API, minimum_to_repair/repair partial-fetch API), `reed-solomon-simd` (GF(2^16), SIMD), `tape-reed-solomon`, `erase` (Backblaze/Klaus Post ports), `rlnc` (RLNC over GF(2⁸), functional-repair-adjacent random linear coding).

#### 3.16 Interactions with IDS invariants

- `erasure_threshold`: regenerating codes are vector MDS; any n−k node failures tolerated.
- `out_eq_in`: repair bytes in == acknowledged repair bytes out (self-check at carrier boundary).
- `correlated_failure`: cooperative repair for simultaneous failures.
- `hybrid_redundancy`: replica + erasure; regenerating codes apply to the erasure-coded tier.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Corrupt repair fragment from a helper | Repaired-node integrity | sha256/blake3 fragment checksums; verify against parity/MDS constraints; quarantine helper, retry with another of the d helpers |
| Helper eavesdrops on repair traffic | Data confidentiality | xchacha20poly1305 / chacha20poly1305 encryption of repair datagrams (allowed set only, via ids-crypto) |
| Replay of stale repair fragments | Repaired-node integrity | Nonce-bound repair manifests; ed25519-signed repair manifest with freshness (IdsTime-based nonce, never wall-clock for causality) |
| Helper selection attack / unavailable helpers | Repair liveness | Select d from n−1 survivors, d ≥ k; redundant helpers beyond minimum; dynamic helper selection avoids slow/failed helpers |
| Carrier re-framing, duplication, delay of datagrams | Integrity/ordering | Carrier never trusted: signatures and checksums are over payload bytes, surviving any carrier transformation; idempotent fragment assembly tolerates duplicates |
| Crypto outside allowlist | Policy compliance | Compile-time allowlist test; only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 reachable; no other algorithm may be added without plan update |
| Sub-packetization blowup (large α) | Memory/compute at helpers | Parameter bounds documented; field-size vs α tradeoff exposed; small-d MSR / zigzag options for low α |
| Exact-repair infeasibility (d < 2k−3, no extension) | Correctness | Guard: exact-repair MSR requires d ≥ 2k−2 or symbol extension; otherwise fall back to functional repair or increase d |
| Denial of service via helper access | Node I/O | Optimal-access pattern: each helper reads exactly β symbols it transmits; access minimization policy; bounded per-repair bandwidth budget |

The carrier is never trusted: repair manifests and fragment checksums are signed/hashed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-regenerating-codes |
|-------|------|-------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `Checksum`, `IdsTime`, `Error` consumed, never redefined; sealed contracts mirrored; invariants (`erasure_threshold`, `out_eq_in`, `correlated_failure`, `hybrid_redundancy`) honored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All repair traffic flows to/from `Endpoint`; crate returns plans/fragments, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Repair manifest signing, fragment checksums, repair datagram encryption; primitive IDs referenced via `ids-000-aa` only |
| `ids-erasure` (Phase 040 family) | Reed-Solomon erasure coding baseline | Naive RS repair (download k full chunks) is the bandwidth baseline this crate improves on (≥2× reduction target); interoperable fragment format |
| `ids-repair` / `ids-recovery` | Node repair and recovery orchestration | Consumes `RepairPlan`s; triggers repair on node failure; integrates with `ids-fault` failure detection |
| `ids-storage` | Coded fragment storage | Stores the α symbols per node that regenerating codes manipulate; provides helper fragments for repair |
| `ids-fault` / `ids-backpressure` | Failure detection, bounded recovery bandwidth | Bounds repair traffic so foreground traffic is not starved (repair bandwidth cap) |
| `ids-locally-recoverable-codes` (Phase 086) | Locality-aware erasure coding | Complementary concern (helper count locality); separate crate per the separate-crates rule |

**Integration points:** `ids-regenerating-codes` exposes parameter sets, cut-set checks, repair plans, and fragment constructors through public types; callers frame fragments into opaque datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond the contracts they expose. No crate-internal reach: only public contracts from `ids-000-aa`.

---

### 6. Proposed File Structure (~150–220 files in subfolders, one new crate)

**Crate:** `ids-regenerating-codes` (new crate at `C:/idfs/ids-regenerating-codes/`; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; `serde`, `thiserror`; dev-only `proptest`, `tempfile`)

```
ids-regenerating-codes/
├── Cargo.toml                          # crate manifest; ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # Code defaults: d = n−1, mode = Exact, bandwidth budgets
├── error.rs                            # RegeneratingCodeError (thiserror): parameter, cut-set, repair, verify errors
├── features.rs                         # Feature flags: exact, functional, cooperative, bench, crypto-verify
│
├── parameters/                         # Regenerating-code parameters {n,k,d,α,β,B}
│   ├── mod.rs
│   ├── parameter_set.rs                # {n,k,d,alpha,beta,B, field_size} + validation
│   ├── msr_params.rs                   # MSR point: α=B/k, β=α/(d−k+1), γ=dα/(d−k+1)
│   ├── mbr_params.rs                   # MBR point: α=dβ=2dB/(k(2d−k+1))
│   ├── tradeoff_curve.rs               # Storage–bandwidth tradeoff curve, interior points
│   ├── subpacketization.rs             # α level, lower bounds (α ≥ s^{n/s}), field-size tradeoff
│   └── field_size.rs                   # Finite field F_q selection per construction
│
├── cutset/                             # Cut-set bound
│   ├── mod.rs
│   ├── bound.rs                        # B ≤ Σ_{i=0}^{k−1} min{α, (d−i)β}
│   ├── optimal_check.rs                # Optimal iff bound tight and reducing α or β breaks it
│   └── types.rs                        # CutSetResult structures
│
├── repair/                             # Repair planning and execution spec
│   ├── mod.rs
│   ├── repair_plan.rs                  # RepairPlan: failed node, d helpers, β per helper, matrices
│   ├── planner.rs                      # Builds a repair plan from parameters + helper roster
│   ├── repair_matrix.rs                # Repair matrices S_{i→j}; linear combinations helpers transmit
│   ├── fragment.rs                     # Node fragment (α symbols) and repair fragment (β symbols)
│   ├── reconstruct.rs                  # Reconstruct α symbols of failed node from d β-fragments
│   ├── verify.rs                       # sha256/blake3 fragment checksum verification; manifest check
│   └── types.rs                        # RepairPlan, RepairFragment, VerifyResult structures
│
├── helper_selection/                   # Which helpers to contact (access minimization)
│   ├── mod.rs
│   ├── policy.rs                       # Blind / Stationary / Dynamic helper-selection policies
│   ├── blind.rs                        # Contact all n−1 nodes
│   ├── stationary.rs                   # Fixed helper set D_i per node i, |D_i| = d
│   ├── dynamic.rs                      # Choose good helpers (load, bandwidth, proximity) at repair time
│   ├── access.rs                       # Access-optimal: helper reads exactly β symbols it transmits
│   └── types.rs                        # HelperSelectionResult structures
│
├── modes/                              # Functional vs exact repair
│   ├── mod.rs
│   ├── functional.rs                   # Functional repair: newcomer functionally equivalent
│   ├── exact.rs                        # Exact repair: reproduce failed node contents (d ≥ 2k−2 guard)
│   ├── cooperative.rs                  # Joint repair of h simultaneous failures (MSCR/MBCR)
│   └── types.rs                        # Mode selection, ModeError
│
├── constructions/                      # Code constructions (specified, not implemented here)
│   ├── mod.rs
│   ├── product_matrix.rs               # Rashmi–Shah–Kumar–Vijay Kumar (MSR d ≥ 2k−2, MBR all)
│   ├── zigzag.rs                       # Tamo–Barg optimal-repair MDS array codes (d = n−1)
│   ├── clay.rs                         # Clay codes: MSR moulded from MDS (FAST'18)
│   ├── miser.rs                        # MISER interference-alignment exact repair (d = n−1 ≥ 2k−1)
│   ├── hadamard.rs                     # Hadamard-design repair-optimal MDS codes, uncoded repair
│   ├── small_d_msr.rs                  # Small-d MSR (d ∈ {k+1, k+2, k+3}, field size O(n))
│   ├── graph_based.rs                  # Generalized regenerating codes on graphs (nonuniform download)
│   └── selection.rs                    # Construction selection guide per parameter regime
│
├── crypto/                             # Allowed-crypto boundary (type surface only)
│   ├── mod.rs
│   ├── repair_signing.rs               # ed25519-signed repair manifests
│   ├── fragment_checksum.rs            # sha256 / blake3 fragment checksums
│   ├── repair_encryption.rs            # xchacha20poly1305 / chacha20poly1305 repair datagrams
│   ├── nonce_mgr.rs                    # IdsTime-based nonces (never wall-clock)
│   ├── allowlist.rs                    # Compile-time crypto allowlist assertion
│   └── types.rs                        # RepairManifest, Checksum structures
│
├── audit/                              # Signed repair audit log
│   ├── mod.rs
│   ├── repair_entry.rs                 # Hash-chained repair decision entry
│   ├── repair_chain.rs                 # sha256/blake3 chain + ed25519 checkpoints
│   ├── checkpoint.rs                   # Periodic ed25519-signed checkpoints
│   ├── verifier.rs                     # Offline verifiable audit chain
│   └── types.rs                        # AuditEntry structures
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── regenerating_contract.rs        # trait RegeneratingCode (parameter set, cut-set)
│   ├── repair_contract.rs              # trait RepairPlanner (build RepairPlan)
│   ├── helper_contract.rs              # trait HelperSelection (choose d helpers)
│   ├── reconstruction_contract.rs      # trait FragmentReconstructor (rebuild α symbols)
│   ├── functional_contract.rs          # trait FunctionalRepair mode
│   ├── exact_contract.rs               # trait ExactRepair mode (guard d ≥ 2k−2)
│   ├── parameter_contract.rs           # trait ParameterValidator (ranges, cut-set)
│   ├── carrier_contract.rs             # trait RepairCarrier (opaque Vec<u8> to Endpoint)
│   └── crypto_contract.rs              # trait RepairCrypto (allowed primitives only)
│
├── invariants/                         # Code invariants (~10 files)
│   ├── mod.rs
│   ├── cutset_bound.rs                 # B ≤ Σ min{α,(d−i)β} enforced
│   ├── mds_property.rs                 # Any k nodes reconstruct; any n−k failures tolerated
│   ├── repair_bandwidth_bound.rs       # γ = dβ ≤ budget; ≤ RS naive baseline k·α
│   ├── helper_count.rs                 # k ≤ d ≤ n−1; β ≤ α
│   ├── functional_vs_exact.rs          # Mode consistency; exact-repair feasibility guard
│   ├── carrier_agnostic.rs             # No std::net / tokio::net / libp2p / IP / TCP / UDP /
│   │                                   # QUIC / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN / mesh
│   ├── crypto_allowlist.rs             # Only ed25519 / xchacha20poly1305 / sha256 / blake3 /
│   │                                   # chacha20poly1305
│   ├── parameter_range.rs              # n, k, d, α, β, B ranges and consistency
│   ├── bandwidth_monotonicity.rs       # γ decreases as d grows for MSR
│   └── out_eq_in.rs                    # Repair bytes-in == bytes-acked (with ids-000-aa)
│
├── tests/                              # Property/verification tests (~28 files)
│   ├── mod.rs
│   ├── parameter_tests.rs
│   ├── cutset_tests.rs
│   ├── msr_tests.rs
│   ├── mbr_tests.rs
│   ├── functional_tests.rs
│   ├── exact_tests.rs
│   ├── cooperative_tests.rs
│   ├── repair_plan_tests.rs
│   ├── helper_selection_tests.rs
│   ├── access_optimal_tests.rs
│   ├── construction_tests.rs
│   ├── crypto_allowlist_tests.rs
│   ├── carrier_agnostic_tests.rs       # zero matches for forbidden networking symbols
│   ├── audit_tests.rs
│   ├── bandwidth_vs_rs_tests.rs        # ≥2× repair-bandwidth reduction vs naive RS (spec values)
│   ├── determinism_tests.rs            # Same parameters ⇒ identical repair plan
│   ├── property_tests.rs               # proptest properties for parameter sets
│   └── integration_contract_tests.rs   # Contracts honored end to end
│
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── repair_bandwidth.rs             # γ = dβ vs RS baseline k·α per parameter set
│   ├── helper_access.rs                # Symbols read per helper (β vs α)
│   ├── cutset.rs                       # Cut-set bound evaluation cost
│   ├── plan_construction.rs            # RepairPlan build time
│   └── crypto_overhead.rs              # Signing/checksum overhead per repair
│
├── spec/                               # Normative specifications
│   ├── parameter_spec.md               # {n,k,d,α,β,B} semantics and ranges
│   ├── repair_spec.md                  # Repair plan, fragment, reconstruction contract
│   ├── mode_spec.md                    # Functional vs exact repair semantics
│   ├── carrier_spec.md                 # Carrier-agnostic boundary contract
│   └── crypto_spec.md                  # Allowed-crypto usage in repair
│
├── docs/                               # Documentation (~14 files)
│   ├── architecture.md                 # Code architecture and layering
│   ├── cutset_guide.md                 # Cut-set bound and MSR/MBR points
│   ├── construction_guide.md           # Product-matrix, zigzag, clay, MISER, Hadamard
│   ├── helper_selection.md             # Blind / stationary / dynamic policies
│   ├── functional_vs_exact.md          # Mode tradeoffs and feasibility guards
│   ├── cooperative_repair.md           # Simultaneous failure repair
│   ├── bandwidth_budget.md             # Repair bandwidth budgets vs foreground traffic
│   ├── runbook.md                      # How to configure parameters per deployment
│   ├── crypto_policy.md                # Allowed-crypto allowlist policy
│   ├── comparisons.md                  # vs Reed-Solomon, vs LRC, replication baseline
│   └── production_notes.md             # Facebook 180 TB/day, Azure/Facebook redundancy data
│
├── test_vectors/                       # Deterministic inputs and expected outputs
│   ├── parameter_vectors.json          # Valid {n,k,d,α,β,B} sets
│   ├── cutset_vectors.json             # Expected bound tightness per parameter set
│   ├── repair_vectors.json             # Expected β fragments per helper (toy fields)
│   └── bandwidth_vectors.json          # Expected γ vs RS baseline
│
├── .gitignore
└── README.md
```

**Why ~150–220 files:** one file per responsibility — parameter math, cut-set bound, MSR/MBR points, repair planning, fragment/reconstruction, helper-selection policies, functional/exact/cooperative modes, seven constructions, crypto boundary, audit chain, ~20 contracts/invariants, ~28 tests, 8 benches, 5 specs, 10 docs, 4 test-vector files. Reflects the genuine code-construction + repair-planning complexity of a bandwidth-efficient node-repair crate. No stubs; every file holds specification-grade definitions, documentation, and tests. Total within the ~90–1000 target range with room for additional constructions/invariants.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Regenerating-code parameter set
```
ParameterSet { n: u32, k: u32, d: u32, alpha: u64, beta: u64, B: u64, field_size: u32 }
constraints: k ≤ d ≤ n−1, β ≤ α, B = k·α at MSR
```

#### 7.2 Cut-set bound
```
B ≤ Σ_{i=0}^{k−1} min{α, (d−i)β}
optimal ⟺ equality holds and reducing α or β violates it
```

#### 7.3 MSR point
```
α_MSR = B / k                              (minimum storage = MDS)
β_MSR = α / (d − k + 1) = B / (k(d−k+1))
γ_MSR = d·β_MSR = dα / (d−k+1)           (repair bandwidth)
```
For d = n−1, γ_MSR < B whenever d > k; e.g., (21,10,20) → >5× reduction vs k-node download.

#### 7.4 MBR point
```
β_MBR = 2B / (k(2d − k + 1))
α_MBR = d·β_MBR                            (α = dβ, repair downloads exactly what is stored)
rate ≤ 1/2, achieved at k = d = n−1
```

#### 7.5 Repair plan
```
RepairPlan { failed: NodeId, helpers: [NodeId; d], beta_per_helper: u64,
             matrices: [RepairMatrix; d], mode: Functional | Exact }
```

#### 7.6 Repair and reconstruction
- Helper j computes `S_{j→f} · fragment_j` (β symbols) and ships as opaque `Vec<u8>` to the newcomer's `Endpoint`.
- Newcomer assembles d β-fragments and reconstructs α symbols of node f (exact: identical to failed node; functional: equivalent code symbol).
- Verification: sha256/blake3 checksum of each fragment; ed25519-signed repair manifest.

#### 7.7 Helper-selection policies
- Blind: D = all n−1 survivors.
- Static: fixed D_i, |D_i| = d, per failed node i.
- Dynamic: choose d "good" helpers at repair time (bandwidth, load, proximity); access-optimal: helper reads exactly β transmitted symbols.

#### 7.8 Bandwidth comparison
- Naive RS repair: γ_RS = k·α ≈ B (download k full chunks).
- Regenerating repair: γ = dβ ≤ dα/(d−k+1) (MSR).
- Target: ≥2× reduction vs RS baseline for representative parameters (e.g., n=14,k=10,d=13).

#### 7.9 Cooperative repair (h failures)
- Cut-set: RB ≥ h(h+d−1)·l/(h+d−k); joint newcomer exchange in round 2.

#### 7.10 Audit chain
- Each repair decision = hash-chained entry (sha256/blake3) + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `Checksum`, `IdsTime`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` transport; opaque `Vec<u8>` repair datagrams — crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` | Serialization of parameter sets, repair plans, manifests |
| `thiserror` | Error types |
| `proptest` | Property-based tests |
| `tempfile` | Test fixtures |

No `std::net`, `tokio::net`, libp2p, or any networking stack in or under `ids-regenerating-codes`. No crypto beyond the allowed list; all crypto via `ids-crypto`. No finite-field implementation crate is mandated in this plan (field arithmetic is specified per construction; a lightweight GF crate may be added at implementation with plan update).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-085.1 | Parameter set {n,k,d,α,β,B} with k ≤ d ≤ n−1, β ≤ α, B consistent | `parameters` validates all fields; invalid sets rejected |
| AC-085.2 | Cut-set bound enforced: B ≤ Σ min{α,(d−i)β}; optimal-flag correct | `cutset` module; unit tests on known parameter sets |
| AC-085.3 | MSR point: α = B/k, β = α/(d−k+1), γ = dα/(d−k+1) | `msr_params` produces correct values |
| AC-085.4 | MBR point: α = dβ = 2dB/(k(2d−k+1)) | `mbr_params` produces correct values |
| AC-085.5 | Repair bandwidth γ < B; target ≥2× vs naive RS (k·α) for representative params | `bandwidth_vs_rs_tests` with spec vectors (e.g., n=14,k=10,d=13) |
| AC-085.6 | Reconstruction from any k nodes (MDS property) | `reconstruct` spec; any k of n fragments recover B |
| AC-085.7 | Functional repair mode specified; exact mode with feasibility guard (d ≥ 2k−2 or symbol extension) | `modes` unit tests |
| AC-085.8 | Helper count d selectable in [k, n−1]; blind/stationary/dynamic policies | `helper_selection` tests |
| AC-085.9 | Access-optimal: helper reads exactly β symbols it transmits | `access_optimal_tests` |
| AC-085.10 | All traffic via `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | `carrier_agnostic_tests` (grep/source audit) passes |
| AC-085.11 | Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | `crypto_allowlist_tests` pass |
| AC-085.12 | Repair fragments verified with sha256/blake3 checksums; manifests ed25519-signed | `verify` + `audit_tests` pass |
| AC-085.13 | Determinism: same parameters ⇒ identical repair plan | `determinism_tests` pass |
| AC-085.14 | Construction specs documented: product-matrix, zigzag, clay, MISER, Hadamard, small-d MSR, graph-based | `spec/` and `docs/` present; `construction_guide.md` covers selection per regime |

---

### 10. Remaining Uncertainties

1. **Construction selection:** product-matrix vs zigzag vs clay for the first build — tradeoff is sub-packetization α, field size q, and repair exactness; decision deferred to implementation phase with `construction_selection.rs` recording the choice.
2. **Exact vs functional default:** exact repair preserves coordinates (important for placement/replication invariants) but needs d ≥ 2k−2; default mode TBD per deployment.
3. **Helper-selection policy:** blind (d = n−1) minimizes bandwidth but maximizes helper count; stationary/dynamic reduce I/O; policy and d-selection algorithm (static n−1 vs adaptive) unquantified for IDS topology.
4. **Field size vs sub-packetization:** optimal exact repair needs large α; small-field constructions (zigzag q=3, small-d MSR O(n)) trade α for simplicity; target α range TBD.
5. **Cooperative repair scope:** simultaneous failure repair (MSCR/MBCR) specified but integration with `ids-fault` failure windows TBD.
6. **Integration boundary with `ids-erasure`/`ids-repair`:** fragment format compatibility (how regenerating-code fragments sit inside erasure-coded stripes) to be validated at integration.
7. **Bandwidth budget:** repair traffic cap vs foreground traffic (ids-backpressure integration) — numeric budget is a product decision.
8. **Finite-field crate:** whether to vendor GF arithmetic or depend on a small field crate; out of scope for this plan.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-regenerating-codes`, not a folder | Follows "separate crates instead of more folders in existing ones"; clear repair-code boundary |
| MSR-first design (min storage α = B/k, then min bandwidth) | Minimum storage preserves erasure-code storage efficiency; MSR repair bandwidth dα/(d−k+1) < naive RS k·α |
| Parameter-driven {n,k,d,α,β,B} with cut-set enforcement | Single source of truth for regenerating-code parameters; matches prompts.txt "regenerating code parameters" content |
| Functional + exact modes, exact guarded by d ≥ 2k−2 | Matches research: exact-repair MSR infeasible below d = 2k−3 without extension (Cadambe et al.) |
| All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Carrier-agnostic absolute rule; no network stack in crate |
| Allowed crypto only: ed25519 / xchacha20poly1305 / sha256 / blake3 / chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares type surface and allowlist |
| Repair manifest = sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable repair audit; tamper evidence despite untrusted carrier |
| Access-optimal helpers (read β, transmit β) | Minimizes helper disk I/O, a practical repair bottleneck (per FAST'18 clay-code findings) |
| ids-000-aa types only; no redefinition of NodeId/Endpoint/Checksum | Single control point; `ids-000-aa` is the master crate |
| `out_eq_in` invariant on repair bytes | bytes-in == bytes-acked at carrier boundary; catches lost repair fragments |

---

### 12. Sources

[^1]: Dimakis, Ramchandran, Wu, Suh — "A Survey on Network Codes for Distributed Storage," arXiv:1004.4438 (2010). https://arxiv.org/abs/1004.4438 — regenerating codes introduced; maintenance bandwidth orders of magnitude below Reed-Solomon.
[^2]: Dimakis, Godfrey, Wu, Wainwright, Ramchandran — "Network coding for distributed storage systems," IEEE Trans. Information Theory (2010). https://arxiv.org/abs/cs/0612065 (related) — cut-set bound, storage–bandwidth tradeoff.
[^3]: Rashmi, Shah, Kumar, Vijay Kumar — "Optimal Exact-Regenerating Codes for Distributed Storage at the MSR and MBR Points via a Product-Matrix Construction," IEEE TIT 57(8), 2011. https://www.cs.cmu.edu/~nihars/publications/Product_matrix_codes.pdf — product-matrix MSR (d ≥ 2k−2) and MBR (all n,k,d).
[^4]: Tamo, Barg — "Zigzag Codes: MDS Array Codes with Optimal Rebuilding," IEEE TIT 59(3), 2013. https://arxiv.org/abs/1101.0133 — optimal-repair MDS array codes, d = n−1, small fields.
[^5]: Cadambe et al. — MISER code, interference-alignment exact-repair MSR (d = n−1 ≥ 2k−1). https://arxiv.org/abs/1005.1634 — impossibility of linear exact-repair MSR for d < 2k−3 without extension.
[^6]: Vajha et al. — "Clay Codes: Moulding MDS Codes to Yield an MSR Code," USENIX FAST'18. https://www.usenix.org/conference/fast18/presentation/vajha — ~2.9× repair-traffic reduction, ~3× repair time vs Reed-Solomon; Ceph implementation.
[^7]: `clay-codes` Rust crate (docs.rs/crates.io), based on FAST'18 Clay codes. https://docs.rs/crate/clay-codes — Rust MSR-code implementation reference (ClayCode::new, minimum_to_repair, repair API).
[^8]: arxiv:1806.04437 — Regenerating code parameter set, MSR/MBR formulas, cooperative repair points (MSCR/MBCR). https://arxiv.org/abs/1806.04437
[^9]: arxiv:1211.1932 — "Codes with Local Regeneration," local MSR/MBR codes with information locality. https://arxiv.org/abs/1211.1932
[^10]: arxiv:1804.00598 — Small-d MSR codes (d ∈ {k+1,k+2,k+3}), optimal access, linear field size O(n). https://arxiv.org/abs/1804.00598
[^11]: arxiv:2405.11714 — Generalized Regenerating Codes and Node Repair on Graphs; nonuniform helper download. https://arxiv.org/abs/2405.11714
[^12]: arxiv:1207.6762 — Cooperative Regenerating Codes; joint repair of multiple failures. https://arxiv.org/abs/1207.6762
[^13]: arxiv:1704.03696 — DoubleR repair layering on HDFS; Facebook 180 TB/day repair bandwidth; Azure 1.33× / Facebook 1.4× redundancy vs 3× replication. https://arxiv.org/abs/1704.03696
[^14]: IEEE Explore 7028491 — Helper node selection in regenerating codes (blind vs stationary vs dynamic). https://ieeexplore.ieee.org/document/7028491
[^15]: arxiv:1710.05876 — Lower bounds on MSR sub-packetization (α ≥ s^{n/s}); optimal-access MSR structure. https://arxiv.org/abs/1710.05876
[^16]: IEEE Explore 6416066 / 6483351 — Repair-optimal MDS codes through Hadamard designs; exact uncoded repair. https://ieeexplore.ieee.org/document/6416066
[^17]: arxiv:1607.07335 — Coupled-layer high-rate MSR code, low sub-packetization, small field size. https://arxiv.org/abs/1607.07335
[^18]: arxiv:1505.01986 — Secrecy capacity of minimum storage regenerating codes. https://arxiv.org/abs/1505.01986
[^19]: `reed-solomon-simd` (GitHub) — GF(2^16) Reed-Solomon erasure coding Rust crate (RS baseline for bandwidth comparison). https://github.com/AndersTrier/reed-solomon-simd
[^20]: `rlnc` Rust crate (docs.rs) — Random Linear Network Coding over GF(2^8), functional-repair-adjacent. https://docs.rs/crate/rlnc
[^21]: `tape-reed-solomon` / `erase` Rust crates (docs.rs/lib.rs) — Reed-Solomon erasure-coding references. https://lib.rs/crates/tape-reed-solomon
[^22]: IDS project documents at C:/idfs: prompts.txt entry 85 (lines 1270-1284); CRATES.md; IDS.md; existing `ids-000-aa` skeleton at C:/idfs/ids-000-aa/.
[^23]: HX-241 plan format references: C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.

---

**Research status:** Complete. Ready for implementation phase. No source code written, no files created beyond this plan document, no repository changes made. The blueprint above defines all architecture, parameters, constructions, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 085 (ids-regenerating-codes).
