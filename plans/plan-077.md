## Phase 077 — Create the ids-quorum-intersection crate: Guarantee read/write quorum intersection. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 077 creates `ids-quorum-intersection`, the IDS crate that guarantees read/write quorum intersection — the fundamental safety property ensuring that every read quorum overlaps every write quorum by at least one replica, so a read always sees the latest acknowledged write. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by quorum domain. The crate owns: quorum system validation (R + W > N), intersection proof generation, fault tolerance calculation (write tolerance N−W, read tolerance N−R), quorum size optimization, minimum-overlap computation (W + R − N), strict vs partial quorum distinction, weighted quorum support, and Byzantine quorum intersection. `ids-quorum-intersection` never sends bytes itself; it evaluates quorum configurations, proves intersection, and returns decisions (`QuorumDecision`, `IntersectionProof`) that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-quorum-intersection` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-quorum-intersection` crate does NOT yet exist at `C:/idfs/ids-quorum-intersection/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`, sealed traits, invariants incl. `QuorumIntersection`). Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Gifford's weighted voting (SOSP 1979, r + w > v), Thomas majority consensus (TODS 1979), quorum consensus algorithm (Lynch-Merritt PODC 87), strict quorum definition (any two quorums intersect), minimum overlap formula W + R − N, pigeonhole principle proof of intersection, Dynamo R + W > N (N=3, W=2, R=2 default), Cassandra consistency levels (QUORUM, LOCAL_QUORUM, EACH_QUORUM), PBS (Probabilistically Bounded Staleness) for partial quorums, Flexible Paxos (phase-cross quorum intersection only), PBFT/BFT quorum intersection (n=3f+1, quorum 2f+1 → intersection ≥ f+1, at least one correct), quorum leases (fast reads without sacrificing writes), k-quorum systems (return one of last k values), read-write quorum systems formalism (Q = (R, W) where every r ∈ R intersects every w ∈ W), masking quorum systems (intersect in ≥ correct servers despite Byzantine), anti-entropy + Merkle trees for eventual convergence, quorum-based mutual exclusion, dynamic/adaptive quorum systems (Orca), heterogeneous quorum systems (Satrapy), weighted quorum systems (WHEAT, Swiper, Cabinet), read repair + hinted handoff for sloppy quorums, CAP/PACELC tradeoff, linearizability vs quorum intersection (W+R>N is not linearizability).

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1150-1164)

```
77. Research only do not read plan.md **[Phase 077 — Create the ids-quorum-intersection crate: Guarantee read/write quorum intersection.]**

Crate: ids-quorum-intersection
Purpose: Guarantee read/write quorum intersection.
Contents: Read quorum ∩ write quorum ≠ ∅, quorum system validation, intersection proof, fault tolerance calculation, quorum size optimization.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 077 — Create the ids-quorum-intersection crate: Guarantee read/write quorum intersection.]**

Create plan-077.md in C:/idfs/plans/ for the ids-quorum-intersection crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 2. Architecture Overview

`ids-quorum-intersection` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a policy/validation layer: given N replicas and desired read/write quorum sizes R and W, it (a) validates the quorum configuration (R + W > N for strict quorum), (b) generates an intersection proof (certificate that every read quorum intersects every write quorum), (c) computes fault tolerance (write tolerance = N − W, read tolerance = N − R), (d) optimizes quorum sizes for a target availability/latency tradeoff, and (e) supports weighted and Byzantine quorum variants. It never sends bytes itself; it returns `QuorumDecision`/`IntersectionProof` that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (quorum users)                  │
│  ids-store, ids-replication, ids-consistency-profile — quorum-configured ops │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 077: ids-quorum-intersection (THIS CRATE — validation)  │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────┐ │ │\n│  │ quorum config    │ │ R+W>N validation │ │ intersection     │ │ fault  │ │ │\\n│  │ (N, R, W, scope) │ │ strict vs partial│ │ proof generation │ │ tolerance│ │ │\\n│  └──────────────────┘ └──────────────────┘ │ (certificate)    │ │ calc       │ │ │\\n│  ┌──────────────────┐ ┌──────────────────┘ └──────────────────┘ └────────┘ │ ││\n│  │ quorum size      │ │ weighted quorum  │ │                  │                │ │ │\\n│  │ optimization     │ │ Byzantine quorum │ │                  │                │ │ │\\n│  └──────────────────┘ └──────────────────┘ │                  │                │ │ │\\n│  ┌──────────────────┐ ┌──────────────────┐ │                  │                │ │ │\\n│  │ decision types   │ │ audit/logging    │ │                  │                │ │ │\\n│  │ (QuorumDecision) │ │ (signed+chained) │ │                  │                │ │ │\\n│  └──────────────────┘ └──────────────────┘ │                  │                │ │ │\\n└──────────────────────────────┬─────────────┴──────────────────┴────────────┘\\n                              │ opaque Vec<u8> datagrams\\n┌──────────────────────────────▼──────────────────┐\\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only│\\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>},│\\n│  payload: &[u8])                                          │\\n│  receive_datagram → (Endpoint, Vec<u8>)                    │\\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP /     │\\n│  QUIC / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN   │\\n└────────────────────────────────────────────────────────────┘
```

**Data flow of a quorum-validated write:** client submits `Write{record_id, payload, quorum_config}` → `ids-quorum-intersection` validates `R + W > N` (or weighted/Byzantine equivalent), generates `IntersectionProof`, computes fault tolerance, returns `QuorumDecision{admit: bool, required_w: W, required_r: R, min_overlap: W+R−N, proof, fault_tolerance}` → caller sends the write to W replicas as opaque datagrams to `ids-carrier::Endpoint` targets and collects acks → quorum re-validates ack count against W → returns `WriteResult{committed, durable, intersection_verified}`.

**Quorum semantics (research-verified):**
- **Strict quorum** (R + W > N): every read quorum intersects every write quorum → strong read-your-writes; minimum overlap = W + R − N ≥ 1.
- **Partial quorum** (R + W ≤ N): read and write quorums may be disjoint → eventual consistency only; staleness possible.
- **Majority quorum** (W = R = ⌊N/2⌋ + 1): symmetric balanced case; tolerates ⌊N/2⌋ failures.
- **Write-all/read-one** (W = N, R = 1): fast reads, writes blocked by any single failure.
- **Write-one/read-all** (W = 1, R = N): fast writes, reads need all nodes.
- **Dynamo default** (N=3, W=2, R=2): 2+2>3, overlap=1, tolerates 1 failure each way.
- **Weighted quorum**: replicas carry votes; read weight + write weight > total weight → intersection; generalizes NWR.
- **Byzantine quorum** (n = 3f+1, quorum = 2f+1): any two quorums intersect in ≥ f+1 nodes, at least one correct → safety under Byzantine faults.

**Crypto used:** ed25519 for signing quorum decisions and intersection proofs; sha256/blake3 for quorum config IDs, version-vector hashing, proof-chain links; xchacha20poly1305/chacha20poly1305 to encrypt quorum state at rest when confidentiality required.

---

### 3. Detailed Research Dimensions

#### 3.1 Quorum intersection: R + W > N (strict quorum)

**Source:** Gifford "Weighted Voting for Replicated Data" (SOSP 1979); Thomas "A Majority Consensus Approach to Concurrency Control for Multiple Copy Databases" (ACM TODS 1979); Quorum Consensus (Lynch-Merritt, PODC 1987); Wikipedia Quorum (distributed computing); Stanford CS145; UMD/MIT 6.033 Quorum Consensus notes; arxiv 2002.06098.

Given N replicas, a write succeeds when W replicas ack; a read succeeds when R replicas respond. If R + W > N, then by the pigeonhole principle, any read quorum (size R) and any write quorum (size W) must overlap in at least W + R − N ≥ 1 nodes. That overlapping node carries the latest acknowledged write, so a read always contacts at least one replica with the current value — strong read-your-writes (freshness, not linearizability). If R + W ≤ N, quorums may be disjoint → stale reads possible → eventual consistency. `ids-quorum-intersection` implements strict-quorum validation with the `R + W > N` guard and minimum-overlap computation `W + R − N`.

#### 3.2 Pigeonhole principle proof of intersection

**Source:** Multiple system-design references (thecodeforge.io, onenoughtone.com, sdepedia.com, hld.handbook.academy).

Proof: N total slots; write fills W slots; remaining unfilled = N − W. A read set of R nodes can avoid the write set only if R ≤ N − W, i.e. R + W ≤ N. Contrapositive: if R + W > N, the read set cannot avoid the write set — at least one node is in both. Minimum overlap size = W + R − N. Examples: N=3, W=2, R=2 → overlap = 1; N=5, W=3, R=3 → overlap = 1; N=5, W=4, R=3 → overlap = 2. Edge case W + R = N exactly → quorums could be complementary with zero overlap, which is why strict inequality `>` is required, not `≥`.

#### 3.3 Fault tolerance calculation

**Source:** deepengineering.net staleness SLOs; hld.handbook.academy quorums/replication; ironclad.academy quorum/read-repair; BEE-19014 NWR consistency.

- Write availability: write succeeds iff ≥ W replicas available → tolerates N − W write failures.
- Read availability: read succeeds iff ≥ R replicas available → tolerates N − R read failures.
- W = N, R = 1: 0 write failures tolerated, N − 1 read failures tolerated.
- W = 1, R = N: N − 1 write failures tolerated, 0 read failures tolerated.
- W = R = ⌊N/2⌋ + 1: ⌊N/2⌋ failures tolerated each way (balanced).
- `ids-quorum-intersection` exposes `write_tolerance() = N − W` and `read_tolerance() = N − R`, and validates that configured tolerances meet deployment requirements.

#### 3.4 Quorum size optimization

**Source:** systemoverflow tuning quorum parameters; JavaAtoll quorum cheat sheet; hld.handbook.academy config table.

Optimization trades off latency, availability, and consistency:
- Smaller W → lower write latency, higher write availability, weaker durability (fewer replicas have the write).
- Smaller R → lower read latency, higher read availability, weaker freshness.
- Larger overlap (W + R − N) → stronger consistency but higher latency on both sides.
- Recommended defaults: N=3, W=2, R=2 (balanced, tolerates 1 failure); N=3, W=3, R=1 (read-heavy); N=3, W=1, R=3 (write-heavy telemetry); N=5, W=3, R=3 (mission-critical larger cluster).
- `ids-quorum-intersection` provides an optimizer that, given N and a target fault tolerance f, computes minimal W and R satisfying R + W > N and N − W ≥ f, N − R ≥ f.

#### 3.5 Strict vs partial quorum

**Source:** arxiv 2002.06098 (Consistency Analysis of Replication-Based Probabilistic KV Stores); arxiv 1804.00742 (Minimizing Content Staleness); PBS paper (arxiv 2104.04102 / CACM); Berkeley EECS-2012-4.

- Strict quorum (R + W > N): strong consistency, every read sees the latest write; higher latency, lower availability.
- Partial quorum (R + W ≤ N): eventual consistency, lower latency, higher availability, staleness unbounded without anti-entropy.
- Expanding partial quorums: write quorum grows post-ack via anti-entropy; eventual intersection.
- `ids-quorum-intersection` classifies configurations as strict/partial and enforces the crate's guarantee (strict by default; partial requires explicit opt-in with documented staleness SLO).

#### 3.6 Weighted quorum systems

**Source:** Gifford SOSP 1979 (fractional weights, v_r + v_w > v); arxiv 2307.15561 (Swiper); arxiv 2609.39715 (adaptive Byzantine quorum, WHEAT); arxiv 2512.20485 (Cabinet dynamic weights); arxiv 1403.5128 (wheel quorum protocol).

Each replica i carries weight w_i; total weight V = Σ w_i. Read requires weight ≥ v_r; write requires weight ≥ v_w; intersection guaranteed iff v_r + v_w > V. Weighted quorums generalize NWR and let operators tune per-replica influence (failure domains, capacity, responsiveness). `ids-quorum-intersection` supports weighted configs with `v_r + v_w > V` validation and weighted overlap computation.

#### 3.7 Byzantine quorum intersection

**Source:** Princeton COS 418 BFT lecture; arxiv 2609.39715 (adaptive Byzantine quorum); Satrapy (heterogeneous quorum systems, 2026); arxiv 2104.04102 masking quorums; Malkhi-Reiter Byzantine quorum survey.

With n = 3f + 1 replicas and quorum size 2f + 1, any two quorums intersect in ≥ f + 1 nodes, hence ≥ 1 correct node → safety under f Byzantine faults. Masking quorums: read quorum intersects last write quorum in ≥ correct servers, enabling the reader to identify correct up-to-date data. Heterogeneous quorum systems: each participant has its own quorum set; quorum intersection + weak availability + quorum subsumption are sufficient for consensus. `ids-quorum-intersection` supports Byzantine quorum validation with `n = 3f + 1`, quorum = `2f + 1`, intersection ≥ `f + 1`, correct-intersection ≥ 1.

#### 3.8 Flexible Paxos / phase-cross quorum intersection

**Source:** arxiv 1608.06696 (Flexible Paxos); arxiv 2203.03058; blog.acolyer.org.

Paxos traditionally requires all quorums (all phases/rounds) to intersect. Flexible Paxos shows only phase-1 and phase-2 quorums of a given round need intersect: |Q1| + |Q2| > N. Within a phase, disjoint quorums are safe. This trades larger leader-election quorums for smaller replication quorums. `ids-quorum-intersection` documents this as an advanced optimization; the crate enforces the simpler universal rule (all read/write quorums intersect) by default.

#### 3.9 Quorum leases

**Source:** cs.cmu.edu quorum leases (Paxos Quorum Leases: Fast Reads Without Sacrificing Writes).

Quorum leases let a majority of replicas perform strongly consistent local reads without contacting the leader, reducing read latency by orders of magnitude in WAN. Leases are time-bounded; a leader grants lease to a quorum; reads within lease are local and linearizable. Writes still need quorum. `ids-quorum-intersection` documents quorum leases as a read-path optimization; lease management lives in `ids-time`/`ids-leader` (later phases).

#### 3.10 k-quorum systems

**Source:** Cornell DISC 2005 / DISC 2006 (Byzantine and Multi-writer K-Quorums).

k-quorums guarantee a read returns one of the last k written values, even under adversarial scheduling. Write propagates tuples for preceding k−1 writes; reads collect quorum, pick latest valid tuple, write back to make it visible at more locations. Trade: higher availability than strict quorums at cost of bounded staleness (k values). `ids-quorum-intersection` documents k-quorum as a relaxed-consistency option; not the default guarantee.

#### 3.11 Read-write quorum systems (formalism)

**Source:** arxiv 2104.04102 (Read-Write Quorum Systems Made Practical); arxiv 2610.00702 (Ditto: generalized reconfigurable linearizable reads).

Formal definition: Q = (R, W) over node set X; R = set of read quorums (subsets of X), W = set of write quorums; every r ∈ R intersects every w ∈ W (r ∩ w ≠ ∅). Upward closure: supersets of quorums are quorums. Ditto: shrinking read quorums forces write quorums to grow and vice versa; local reads (read quorums of size 1) require write quorums containing at least one process from every read quorum. `ids-quorum-intersection` implements the formal relation `∀r∈R, ∀w∈W: r ∩ w ≠ ∅` as an exhaustive checker for small N and a sufficient-condition checker (R + W > N) for uniform quorums.

#### 3.12 CAP / PACELC and availability

**Source:** CAP theorem; PACELC (Abadi); Wikipedia PACELC; CockroachDB fundamental tradeoffs.

CAP: under partition P, choose Consistency C or Availability A. PACELC: elsewhere (no partition) choose Latency L or Consistency C. `ids-quorum-intersection` encodes this: strict quorum chooses C (rejects below quorum); partial quorum chooses A (progress with possible staleness); quorum leases choose L in the common case. Failure behavior matrix per configuration.

#### 3.13 Anti-entropy and eventual convergence (partial quorum backstop)

**Source:** HLD Handbook Merkle trees/anti-entropy; HotShard anti-entropy & read repair; jepemo read repair/anti-entropy; Dynamo paper (SOSP 2007) Merkle trees + hinted handoff; Cassandra read repair.

Partial quorums converge via anti-entropy (Merkle-tree digest exchange, O(K log N) comparisons for K differences) and read-repair (quorum read detects digest mismatch, returns newest, pushes back to stale replicas). Hinted handoff: writes to sloppy nodes are replayed when the intended target recovers. `ids-quorum-intersection` documents the backstop but delegates repair execution to `ids-replication`/`ids-anti-entropy` (later phases) via `ids-carrier::Endpoint`.

#### 3.14 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek (RFC 8032); chacha20poly1305 (RFC 8439 + XChaCha20); sha2 (FIPS 180-4); blake3 (BLAKE3-team).

- ed25519: signing quorum decisions, intersection proofs, audit checkpoints.
- sha256: quorum config IDs, version-vector entry hashing, proof-chain links.
- blake3: fast content hashing for quorum-state fingerprints, checkpoint chaining (parallel, SIMD).
- xchacha20poly1305: encrypt quorum state at rest (24-byte nonce, random nonces safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based).
No other crypto. `ids-crypto` exposes primitive IDs; `ids-quorum-intersection` references them via `ids-000-aa` only.

#### 3.15 Linearizability boundary

**Source:** Jepsen (Cassandra QUORUM write loss under partition); JavaAtoll quorum cheat sheet; Attiya SPAA'91; CockroachDB linearizability notes.

W + R > N guarantees the read set and write set intersect → never read a stale *committed* value. It does NOT guarantee linearizability: concurrent writes, partially failed writes, and read-modify-write races can still yield non-linear states. True linearizability needs consensus (Paxos/Raft) or ABD write-back. `ids-quorum-intersection` documents this boundary explicitly: it provides freshness (read-your-writes via intersection), not linearizability.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Stale read below quorum (R + W ≤ N) | Read correctness | `R + W > N` enforced for strict quorum; partial quorum requires explicit opt-in + documented staleness SLO |
| Lost write (W replicas fail) | Write durability | `write_tolerance() = N − W`; deployment must provision N such that N − W ≥ required failure tolerance |
| Split-brain / disjoint quorums | State consistency | Intersection proof certificate required before any quorum-configured operation is admitted |
| Byzantine intersection node | Correctness under faults | Byzantine quorum mode: n = 3f+1, quorum 2f+1 → intersection ≥ f+1, ≥1 correct |
| Clock manipulation for quorum state | Causality correctness | Logical version numbers only (Gifford pattern), never wall-clock; monotonic `IdsTime` for timeouts |
| Quorum config downgrade attack | Consistency integrity | Config is immutable per record/region; changes require signed governance decision, audited |
| Key extraction (quorum signing key) | Decision authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent intersection checks; replay protection via nonces and bounded replay cache |
| Partial quorum masquerading as strict | Safety violation | Compile-time/runtime flag `Strict` vs `Partial`; `Partial` cannot be silently promoted to `Strict` |

The carrier is never trusted: quorum decisions and proofs are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-quorum-intersection ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; `QuorumIntersection` invariant mirrored here |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All quorum traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing quorum decisions/proofs; hashing quorum configs and proof chains; encrypting quorum state at rest |
| `ids-store` | Primary record store (quorum user) | Tags writes/reads with quorum config; consults `ids-quorum-intersection` before admitting |
| `ids-replication` | Replica synchronization / anti-entropy | Runs repair backstop for partial quorums; uses this crate's quorum validation |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Profiles map to quorum configs (R, W, N); this crate validates and proofs them |
| `ids-time` | Monotonic time source | Quorum lease timeouts and `IdsTime` timestamps on decisions (never wall-clock for causality) |

**Integration points:** `ids-quorum-intersection` is a pure validation/proof layer. It imports canonical types and contracts from `ids-000-aa`, emits `QuorumDecision`/`IntersectionProof` values, and relies on callers (`ids-store`, `ids-replication`, `ids-consistency-profile`) to frame them into opaque datagrams to `ids-carrier::Endpoint`. It does not depend on `ids-discovery`, `ids-session`, or any carrier implementation.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-quorum-intersection/` (a separate crate, not a folder inside an existing crate):

```
ids-quorum-intersection/
├── Cargo.toml                          # Crate manifest; depends on ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # Crate root, public API surface, re-exports
├── constants.rs                        # Quorum defaults: N=3, W=2, R=2; Byzantine n=3f+1
├── error.rs                            # QuorumError (thiserror): validation, intersection, config errors
├── types.rs                            # QuorumConfig, QuorumDecision, IntersectionProof, QuorumMode (re-exports ids-000-aa types)
├── quorum_config.rs                    # QuorumConfig construction/validation (N, R, W, scope, weights)
├── strict_quorum.rs                    # R + W > N validation, strict/partial classification
├── intersection_proof.rs               # Proof generation: certificate that every read quorum intersects every write quorum
├── min_overlap.rs                      # Minimum overlap = W + R − N; overlap size computation
├── fault_tolerance.rs                  # write_tolerance() = N − W; read_tolerance() = N − R
├── quorum_optimizer.rs                 # Given N + target fault tolerance f, compute minimal W, R
├── weighted_quorum.rs                  # Weighted configs: v_r + v_w > V validation, weighted overlap
├── byzantine_quorum.rs                 # n = 3f + 1, quorum = 2f + 1, intersection ≥ f + 1, correct ≥ 1
├── k_quorum.rs                         # k-quorum relaxed consistency (one of last k values)
├── quorum_lease.rs                     # Quorum lease types (documentation + interface for ids-time/ids-leader)
├── linearizability_boundary.rs         # Documents W+R>N is freshness, not linearizability
├── relation_checker.rs                 # Exhaustive ∀r∈R, ∀w∈W: r∩w≠∅ checker (small N) + sufficient-condition checker
├── carrier_adapter.rs                  # Opaque Vec<u8> datagram framing to ids-carrier::Endpoint (decision carrier only)
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── quorum_contract.rs              # trait Quorum (validate/prove/tolerate)
│   ├── intersection_contract.rs        # trait Intersection (proof generation/verification)
│   ├── tolerance_contract.rs           # trait FaultTolerance (write/read tolerance)
│   ├── optimizer_contract.rs           # trait QuorumOptimizer (size optimization)
│   ├── weighted_contract.rs            # trait WeightedQuorum (weighted validation)
│   ├── byzantine_contract.rs           # trait ByzantineQuorum (Byzantine intersection)
│   ├── kquorum_contract.rs             # trait KQuorum (k-quorum relaxed consistency)
│   ├── lease_contract.rs               # trait QuorumLease (lease interface)
│   ├── relation_contract.rs            # trait QuorumRelation (intersection checking)
│   └── carrier_contract.rs             # trait CarrierAdapter (decision framing to Endpoint)
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── quorum_intersection.rs          # R + W > N invariant (mirrors ids-000-aa QuorumIntersection)
│   ├── monotonic_version_only.rs       # Logical versions, no wall-clock for causality
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305
│   ├── config_immutable.rs             # Quorum config immutability per record/region
│   ├── audit_chain_sound.rs            # Hash-chained + signed audit entries
│   ├── partial_opt_in.rs               # Partial quorum requires explicit opt-in
│   └── no_linearizability_claim.rs     # Crate does not claim linearizability
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── audit_entry.rs
│   ├── audit_chain.rs
│   ├── checkpoint.rs
│   ├── signer.rs
│   ├── verifier.rs
│   └── mod.rs
├── tests/                              # Property/verification tests (~30 files)
│   ├── mod.rs
│   ├── quorum_config_tests.rs
│   ├── strict_quorum_tests.rs
│   ├── intersection_proof_tests.rs
│   ├── min_overlap_tests.rs
│   ├── fault_tolerance_tests.rs
│   ├── optimizer_tests.rs
│   ├── weighted_quorum_tests.rs
│   ├── byzantine_quorum_tests.rs
│   ├── k_quorum_tests.rs
│   ├── relation_checker_tests.rs
│   ├── carrier_agnostic_tests.rs       # Asserts no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist_tests.rs       # Asserts only allowed crypto reachable
│   ├── linearizability_boundary_tests.rs # Asserts no linearizability claim
│   ├── determinism_tests.rs            # Same N/R/W → identical decisions
│   ├── invariant_tests.rs              # All invariants enforced
│   ├── property_tests.rs               # proptest properties for quorum math
│   └── integration_tests.rs            # Mock caller flow (decision → Endpoint framing)
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── quorum_validation.rs
│   ├── intersection_proof.rs
│   ├── min_overlap.rs
│   ├── fault_tolerance.rs
│   ├── optimizer.rs
│   ├── weighted_quorum.rs
│   ├── byzantine_quorum.rs
│   └── relation_checker.rs
├── spec/                               # Normative specifications
│   ├── quorum_spec.md                  # What quorum intersection means and how it is enforced
│   ├── proof_spec.md                   # Intersection proof format and verification
│   ├── tolerance_spec.md               # Fault tolerance calculation contract
│   ├── optimizer_spec.md               # Quorum size optimization contract
│   ├── carrier_agnostic_spec.md        # Carrier-agnostic boundary contract
│   └── crypto_policy.md                # Allowed-crypto allowlist policy
├── docs/                               # Documentation
│   ├── architecture.md                 # Quorum architecture and layering
│   ├── runbook.md                      # How to configure and validate quorums
│   ├── adding_a_quorum_mode.md         # Adding a new quorum mode (weighted/Byzantine/k-quorum)
│   ├── pigeonhole_proof.md             # Formal proof of R + W > N intersection
│   └── linearizability_boundary.md     # Freshness vs linearizability boundary
├── test_vectors/                       # Deterministic inputs and expected outputs
│   ├── quorum_vectors.json             # N/R/W configs with expected overlap/tolerance
│   ├── proof_vectors.json              # Expected intersection proofs
│   └── byzantine_vectors.json          # Byzantine quorum configs with f values
└── README.md
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `spec/`, `docs/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 QuorumConfig / QuorumMode
`QuorumConfig { n: u32, r: u32, w: u32, mode: QuorumMode, scope: QuorumScope, weights: Option<Vec<u32>> }` where `QuorumMode = Strict | Partial | Weighted | Byzantine | KQuorum { k }` and `QuorumScope = Local | Region | CrossRegion`. Re-exports `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum` from `ids-000-aa`.

#### 7.2 Strict quorum validation
```
admit(N, R, W) -> bool iff R + W > N
classify(N, R, W) -> Strict if R + W > N else Partial
validate(config) -> QuorumDecision { admit, required_r, required_w, min_overlap, proof, fault_tolerance }
```

#### 7.3 Intersection proof (certificate)
For uniform quorums (all subsets of size R/W): proof = certificate that `R + W > N` holds, signed (ed25519) and hash-chained (sha256/blake3). For arbitrary quorum sets Q = (R_set, W_set): exhaustive checker `∀r∈R_set, ∀w∈W_set: r ∩ w ≠ ∅` (exponential, feasible for small N); result `Some(true)`/`Some(false)`/`None` with safe fallback on `None`.

#### 7.4 Minimum overlap
```
min_overlap(R, W, N) = W + R − N   (when R + W > N; else 0 / no guarantee)
```
Examples: N=3, W=2, R=2 → 1; N=5, W=3, R=3 → 1; N=5, W=4, R=3 → 2.

#### 7.5 Fault tolerance
```
write_tolerance(N, W) = N − W
read_tolerance(N, R) = N − R
```
Balanced majority: W = R = ⌊N/2⌋ + 1 → tolerates ⌊N/2⌋ failures each way.

#### 7.6 Quorum size optimizer
Given N and target fault tolerance f: find minimal W, R such that `R + W > N`, `N − W ≥ f`, `N − R ≥ f`. Returns (W, R) minimizing max(W, R) (latency-balanced) or minimizing W (write-optimized) / R (read-optimized) per objective.

#### 7.7 Weighted quorum
Weights w_i, total V = Σ w_i; read threshold v_r, write threshold v_w; admit iff `v_r + v_w > V`; weighted overlap computed per config.

#### 7.8 Byzantine quorum
n = 3f + 1; quorum = 2f + 1; any two quorums intersect in ≥ f + 1 nodes; ≥ 1 correct node in intersection → safety under f Byzantine faults.

#### 7.9 Audit chain
Each quorum decision (admit/reject/reconfigure) = SHA-256/BLAKE3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts; `QuorumIntersection` invariant.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of quorum configs, decisions, proofs, audit entries.
- `thiserror` — error types.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-quorum-intersection` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|---|-----------|-------------|
| AC-077.1 | Strict quorum validation: `R + W > N` enforced; `R + W ≤ N` classified as Partial | Unit tests: N=3,W=2,R=2 → Strict; N=5,W=3,R=2 → Partial |
| AC-077.2 | Intersection proof generated for every admitted strict-quorum config | Proof certificate signed + hash-chained; verifier returns valid |
| AC-077.3 | Minimum overlap computed: `W + R − N` | N=3,W=2,R=2 → 1; N=5,W=4,R=3 → 2; N=5,W=3,R=2 → 0 |
| AC-077.4 | Fault tolerance: `write_tolerance() = N − W`, `read_tolerance() = N − R` | N=3,W=2,R=2 → write=1, read=1; N=5,W=3,R=3 → write=2, read=2 |
| AC-077.5 | Quorum optimizer: given N + target f, returns minimal W, R satisfying constraints | N=5, f=1 → W=3, R=3 (balanced) or W=4,R=2 / W=2,R=4 per objective |
| AC-077.6 | Weighted quorum: `v_r + v_w > V` enforced | Weighted configs validated; disjoint weighted quorums rejected |
| AC-077.7 | Byzantine quorum: n = 3f + 1, quorum = 2f + 1, intersection ≥ f + 1, correct ≥ 1 | f=1 → n=4, quorum=3, intersection≥2, correct≥1 |
| AC-077.8 | k-quorum: read returns one of last k written values | k-quorum semantics verified via property tests |
| AC-077.9 | Exhaustive relation checker: `∀r∈R, ∀w∈W: r∩w≠∅` for small N | Correct Some(true)/Some(false)/None with safe fallback |
| AC-077.10 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
| AC-077.11 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-077.12 | Every quorum decision in append-only, hash-chained, ed25519-signed audit log | `NodeId` + `IdsTime` on every entry |
| AC-077.13 | No linearizability claim | Boundary documented; tests assert freshness-only guarantee |
| AC-077.14 | Determinism: same N/R/W/mode → identical `QuorumDecision` | Determinism tests pass |
| AC-077.15 | Crate compiles cleanly | `cargo check -p ids-quorum-intersection` exits 0 |
| AC-077.16 | Conformance tests pass | `cargo test -p ids-quorum-intersection` passes all tests |

---

### 10. Remaining Uncertainties

1. **Concrete quorum defaults per deployment:** N/R/W defaults (e.g., N=3, W=2, R=2) are deployment-tuned; to be validated at integration with `ids-store`/`ids-consistency-profile`.
2. **Anti-entropy interval for partial quorum backstop:** convergence schedule for partial-quorum staleness healing; workload-dependent.
3. **Quorum lease integration:** lease grant/renewal flow with `ids-time`/`ids-leader` (later phases) not yet designed.
4. **Weighted quorum weight assignment:** how weights are derived from failure domains/capacity; policy pending.
5. **Byzantine fault model scope:** whether IDS assumes Byzantine faults at all; crash-fault-only would simplify to n=2f+1.
6. **k-quorum adoption:** whether k-quorum (bounded staleness) is offered as a profile or deferred.
7. **Relation checker scalability:** exhaustive checker is exponential; sufficient-condition (R+W>N) used for uniform quorums; arbitrary quorum-set checking for large N needs sampling/approximation.
8. **Audit chain performance:** signed checkpoints per decision may be hot-path expensive; batching interval TBD.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate crate `ids-quorum-intersection` (not a folder) | Follows "separate crates instead of more folders" directive; clear validation boundary |
| Strict quorum default (`R + W > N`), partial requires explicit opt-in | Guarantee is the crate's purpose; partial quorum weakens it intentionally |
| Minimum overlap = W + R − N; strict inequality `>` (not `≥`) | Pigeonhole principle; W + R = N allows complementary disjoint quorums |
| Intersection proof certificate (signed + hash-chained) | Auditability and offline verifiability of every quorum decision |
| Fault tolerance: write = N − W, read = N − R | Direct from quorum definition; exposes availability tradeoff |
| Quorum optimizer for minimal W/R given target fault tolerance | Automates config tuning; balances latency vs availability |
| Weighted quorum support (`v_r + v_w > V`) | Generalizes NWR; lets operators tune per-replica influence |
| Byzantine quorum (n = 3f+1, quorum = 2f+1, intersection ≥ f+1) | Standard BFT quorum intersection; ≥1 correct in intersection |
| Logical versions only, no wall-clock for causality; monotonic IdsTime for timeouts | Clock manipulation cannot break quorum correctness |
| Freshness guarantee only, not linearizability | W + R > N ≠ linearizability; consensus needed for linearizability (later phases) |
| Carrier-agnostic via `ids-carrier::Endpoint`; crate returns decisions, never sends | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; via `ids-crypto` primitive IDs through `ids-000-aa` | Complies with allowed-crypto rule |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |

---

### 12. Sources

[^1]: Gifford, "Weighted Voting for Replicated Data," ACM SOSP 1979 — r + w > v, fractional weights, read/write quorum votes. https://dl.acm.org/doi/pdf/10.1145/800215.806583
[^2]: Thomas, "A Majority Consensus Approach to Concurrency Control for Multiple Copy Databases," ACM TODS 4(2), 1979 — majority voting foundation.
[^3]: Goldman, "Quorum Consensus in Nested-Transaction Systems," PODC 1987 (Lynch-Merritt model) — Gifford's algorithm formalization. https://groups.csail.mit.edu/tds/papers/Lynch/podc87-goldman-reload.pdf
[^4]: Wikipedia, "Quorum (distributed computing)" — V_r + V_w > V, V_w > V/2, one-copy serializability. https://en.wikipedia.org/wiki/Quorum_(distributed_computing)
[^5]: Stanford CS145, "Consistency in Replicas" — R + W > N, read/write quorum overlap, majority rule. https://cs145.stanford.edu/Module5-Distributed/consensus-leader-election.html
[^6]: UMD / MIT 6.033, "A note on Quorum Consensus" — R + W > N intersection. http://web.mit.edu/6.033/2005/wwwdocs/quorum_note.html
[^7]: arxiv 2002.06098, "Consistency Analysis of Replication-Based Probabilistic Key-Value Stores" — strict vs partial quorums, W + R > N. https://arxiv.org/html/2002.06098v4
[^8]: arxiv 1804.00742, "Minimizing Content Staleness in Dynamo-Style Replicated Storage Systems" — strict quorum, pigeonhole principle, w + r > n. https://arxiv.org/html/1804.00742v1
[^9]: thecodeforge.io, "Quorum R+W>N" — quorum overlap, minimum overlap = W + R − N. https://thecodeforge.io/system-design/quorum-consensus
[^10]: onenoughtone.com, "Quorum Based Replication" — pigeonhole proof, overlap examples, edge case W + R = N. https://onenoughtone.com/learning-path/system-design-hld/learning/quorum-based-replication/1
[^11]: sdepedia.com, "Quorum consensus math" — W + R > N, pigeonhole proof, linearizability boundary note. https://sdepedia.com/hld/internals/consensus-quorum-math
[^12]: hld.handbook.academy, "Quorums and Replication: The Math of R + W > N" — configs table, Dynamo default N=3 W=2 R=2, staleness vs linearizability. https://hld.handbook.academy/curriculum/distributed-systems-theory/quorums-and-replication
[^13]: deepengineering.net, "Designing Staleness SLOs for Dynamo-Style KV Stores" — N/R/W control surfaces, staleness windows. https://deepengineering.net/p/deep-engineering-13-designing-staleness
[^14]: arxiv 2104.04102 / CACM, "Probabilistically Bounded Staleness for Practical Partial Quorums" — PBS, partial quorums, strict quorum comparison. https://cacm.acm.org/research/quantifying-eventual-consistency-with-pbs
[^15]: arxiv 1608.06696, "Flexible Paxos: Quorum intersection revisited" — phase-cross intersection only, |Q1| + |Q2| > N. https://arxiv.org/html/1608.06696v1
[^16]: blog.acolyer.org, "Flexible Paxos: Quorum intersection revisited" — summary of FPaxos insight. https://blog.acolyer.org/2016/09/27/flexible-paxos-quorum-intersection-revisited
[^17]: cs.cmu.edu, "Paxos Quorum Leases: Fast Reads Without Sacrificing Writes" — quorum leases, local reads, WAN latency. https://cs.cmu.edu/~imoraru/papers/qrl.pdf
[^18]: Cornell DISC 2005 / DISC 2006, "K-Quorums" — k-quorum relaxed consistency, one of last k values. https://cs.cornell.edu/lorenzo/papers/disc05.pdf ; https://cs.cornell.edu/lorenzo/papers/disc2006.pdf
[^19]: arxiv 2104.04102, "Read-Write Quorum Systems Made Practical" — Q = (R, W) formalism, ∀r∈R, ∀w∈W: r∩w≠∅. https://arxiv.org/pdf/2104.04102v1
[^20]: arxiv 2610.00702, "Ditto: Generalized Reconfigurable Linearizable Reads" — read-write quorum intersection, local reads force write quorum growth. https://arxiv.org/html/2610.00702
[^21]: arxiv 1403.5128, "A novel Quorum Protocol" — weighted voting, wheel quorum protocol. https://arxiv.org/html/1403.5128v1
[^22]: arxiv 2307.15561, "Swiper: a new paradigm for efficient weighted distributed protocols" — weighted voting, weighted quorum systems. https://arxiv.org/html/2307.15561
[^23]: arxiv 2609.39715, "Exploring Adaptive Byzantine Quorum Systems to Improve Latency in the WAN" — Byzantine quorum, 2f+1, intersection ≥ f+1, weighted WHEAT. https://arxiv.org/html/2609.39715
[^24]: arxiv 2512.20485, "WOC: Dual-Path Weighted Object Consensus Made Efficient" — dynamic weighted quorums, Cabinet. https://arxiv.org/pdf/2512.20485v1
[^25]: link.springer.com, "Satrapy: From abstract to practical consensus for heterogeneous quorum systems" — heterogeneous quorum, intersection + availability + subsumption. https://link.springer.com/article/10.1007/s00446-026-00507-0
[^26]: cs.princeton.edu, COS 418 BFT lecture — Byzantine quorum, n=3f+1, overlap ≥ f+1, ≥1 correct. https://cs.princeton.edu/courses/archive/fall17/cos418/docs/L14-bft.pdf
[^27]: arxiv 2104.04102 masking quorums — intersect in ≥ correct servers despite Byzantine. (see [^14])
[^28]: Malkhi-Reiter Byzantine quorum systems survey — masking/dissemination quorums. (referenced in [^23])
[^29]: HLD Handbook, "Merkle Trees and Anti-Entropy" — Merkle digest exchange, O(K log N) comparison. https://hld.handbook.academy/curriculum/distributed-systems-theory/merkle-trees-anti-entropy/
[^30]: HotShard, "Anti-Entropy & Read Repair" — read repair, anti-entropy, hinted handoff. https://hotshard.com/read-repair-ae
[^31]: jepemo, "Read Repair, Anti-Entropy, and Merkle Divergence Checks" — worked anti-entropy trace, blocking read repair. https://jepemo.github.io/lessons/consistency-and-replication/008.html
[^32]: DeCandia et al., Dynamo paper (SOSP 2007) — N=3, W=2, R=2, sloppy quorum, hinted handoff, Merkle anti-entropy. (Dynamo paper)
[^33]: JavaAtoll, "Quorum NWR Read/Write Consistency and Read Repair Cheat Sheet" — W+R>N is freshness not linearizability. https://javaatoll.com/en/cheats/quorum-nwr-read-write-consistency-read-repair
[^34]: alivedise.github.io, "BEE-19014: Quorum Systems and NWR Consistency" — NWR model, Dynamo defaults, sloppy quorum/hinted handoff. https://alivedise.github.io/backend-engineering-essentials/distributed-systems/quorum-systems-and-nwr-consistency
[^35]: ed25519-dalek docs.rs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
[^36]: chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
[^37]: sha2 RustCrypto — SHA-256 implementation. https://docs.rs/sha2
[^38]: BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^39]: Attiya, SPAA'91 — sequential consistency vs linearizability. https://groups.csail.mit.edu/tds/papers/Attiya/SPAA91.pdf
[^40]: MIT 6.824 linearizability notes — strong consistency = linearizability. https://pdos.csail.mit.edu/6.824/notes/l-linearizability.txt
[^41]: plan-001.md / plan-048.md / plan-060.md (IDS plan format references) — C:/idfs/plans/
[^42]: C:/idfs/prompts.txt entry 77 (lines 1150-1164) — verbatim Phase 077 prompt.

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 077 (ids-quorum-intersection).
