## Phase 071 — Create the ids-flexible-paxos crate: Flexible quorum intersection for consensus. Research complete (27 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1060-1074)

```
71. Research only do not read plan.md **[Phase 071 — Create the ids-flexible-paxos crate: Flexible quorum intersection for consensus.]**

Crate: ids-flexible-paxos
Purpose: Flexible quorum intersection for consensus.
Contents: Phase-1/phase-2 quorum separation, non-intersecting quorums within views, intersection across views, leader election quorum, replication quorum, availability optimization.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 071 — Create the ids-flexible-paxos crate: Flexible quorum intersection for consensus.]**

Create plan-071.md in C:/idfs/plans/ for the ids-flexible-paxos crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 071 creates `ids-flexible-paxos`, the IDS crate implementing Flexible Paxos (FPaxos): consensus with flexible quorum intersection where only cross-phase (phase-1 leader election vs phase-2 replication) quorums are required to intersect, while quorums within the same phase may be disjoint. `ids-flexible-paxos` is a **separate crate** (not a folder inside an existing crate, per the project rule). It owns phase-1/phase-2 quorum separation, non-intersecting quorums within views, intersection across views, leader election quorum sizing, replication quorum sizing, availability optimization (small replication quorum / large election quorum trade-off), proposal-number/epoch management, promise/accept state per acceptor, quorum certificate assembly, and the cross-phase intersection safety invariant `Q1_size + Q2_size > N` for simple quorum systems.

The crate is **protocol logic only**: it never opens sockets, never touches a carrier directly, and returns opaque `Vec<u8>` datagram payloads plus `Endpoint` targets (`CarrierType` + `Vec<u8>` ≤ 1024 bytes) for the caller to ship via `ids-carrier::Endpoint`. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh anywhere. Allowed crypto only: ed25519 (signing quorum certificates and ballot claims), xchacha20poly1305/chacha20poly1305 (encrypting per-acceptor promise state at rest), sha256/blake3 (ballot hashing, certificate chaining, quorum digest).

Skeleton `ids-flexible-paxos` does NOT yet exist at `C:/idfs/ids-flexible-paxos/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, plus sealed trait contracts and error types. Research only — no code, no source modifications, only this plan document.

Research basis: 27 parallel searches returned the Flexible Paxos OPODIS 2016 paper (Howard, Malkhi, Spiegelman, arxiv 1608.06696) with the core cross-phase intersection theorem and TLA+ model checking; Relaxed Paxos (arxiv 2203.03058) epoch-specific quorum weakening; Fast Flexible Paxos (ACM 3427796.3427815); WPaxos WAN flexible consensus (arxiv 1703.08905) grid quorums where Q1+Q2 need not exceed N; Q1+Q2>N pigeonhole principle; availability trade-offs (Q2 as small as 1 with Q1=N; even-N improvement reducing Q2 by one); disjoint phase-2 quorums for steady-state throughput; leader election vs replication phase asymmetry; reconfiguration/joint quorum safety during quorum size changes; quorum availability bounds (FPaxos tolerates up to |Q2|−1 failures for replication, more until next election); ballot/proposal-number ordering and safety proof structure; grid and weighted quorum constructions; CockroachDB Parallel Commits application of Flexible Paxos at transaction scope; idempotency and crash recovery semantics.

---

### 2. Architecture Overview

`ids-flexible-paxos` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a **consensus quorum layer**: given an acceptor set, a configured phase-1 quorum (Q1) and phase-2 quorum (Q2), and a proposal/epoch number, it decides whether a quorum is valid, whether phase-1/phase-2 intersection holds, what promise/accept state transitions are legal, what certificate constitutes a decision, and what failure behavior applies. It never sends bytes itself; it returns `Phase1Outcome` / `Phase2Outcome` decisions and opaque `Vec<u8>` payloads addressed to `ids-carrier::Endpoint` targets for the caller to transmit.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    CONSENSUS DRIVER LAYER (ids-* users)                      │
│  ids-consensus, ids-replication, ids-membership — propose/decide with quorum │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 071: ids-flexible-paxos (THIS CRATE — quorum logic)     │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐  │
│  │ quorum config   │ │ phase-1 (Q1)    │ │ phase-2 (Q2)    │ │ proposal │  │
│  │ Q1_size, Q2_size│ │ election quorum │ │ replication quorum│ │ numbers  │  │
│  │ Q1+Q2 > N       │ │ leader election │ │ value acceptance  │ │ / epochs │  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘  │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐  │
│  │ promise/accept  │ │ quorum          │ │ cross-view        │ │ crypto   │  │
│  │ state per       │ │ certificates    │ │ intersection      │ │ surface  │  │
│  │ acceptor        │ │ (Q1/Q2 merge)   │ │ Q1 ∩ Q2 ≠ ∅     │ │ (type    │  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ │ level    │  │
│  ┌─────────────────┐ ┌─────────────────┐ │ across views      │ │ only)    │  │
│  │ availability    │ │ reconfiguration │ │                   │          │  │
│  │ optimization    │ │ / joint quorums │ │                   │          │  │
│  │ (small Q2,      │ │ transitions     │ │                   │          │  │
│  │  large Q1)      │ │                 │ │                   │          │  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
                                          │ opaque Vec<u8> datagrams
                                          ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a phase-1 (leader election) round:** driver submits `Phase1{ballot, acceptor_set, q1_config}` → crate validates quorum family, computes required Q1 size, returns `Phase1Decision{required_q1, target_endpoints: Vec<Endpoint>, prepare_payload: Vec<u8>}` → driver sends `Prepare(ballot)` as opaque datagrams to the targets via `ids-carrier::Endpoint` and collects `Promise` replies → driver feeds promises back → crate validates promise quorum (≥ Q1 distinct acceptors), returns highest accepted proposal/value or `None` → driver proceeds to phase 2 or aborts.

**Data flow of a phase-2 (replication) round:** driver submits `Phase2{ballot, value, acceptor_set, q2_config}` → crate validates Q2, returns `Phase2Decision{required_q2, target_endpoints, accept_payload: Vec<u8>}` → driver sends `Accept(ballot, value)` → collects `Accepted` replies → crate validates accepted quorum (≥ Q2 distinct acceptors) → returns `Decided{value, certificate}` if quorum reached.

**Cross-view intersection:** the crate tracks the quorum families used per view/epoch and enforces that every phase-1 quorum of view `v` intersects every phase-2 quorum of views `< v` (per Flexible Paxos safety theorem and Relaxed Paxos epoch refinement). Within a view, phase-1 quorums need not intersect each other; phase-2 quorums need not intersect each other.

**Crypto used:** ed25519 for signing ballot claims and quorum certificates; sha256/blake3 for ballot hashing, certificate entry hashing, and quorum digest chains; xchacha20poly1305/chacha20poly1305 to encrypt per-acceptor promise/accept state at rest. Crypto itself lives in `ids-crypto`; this crate declares type-level surface and primitive IDs only.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 27 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Flexible Paxos core insight: cross-phase intersection only

**Source:** Howard, Malkhi, Spiegelman, "Flexible Paxos: Quorum Intersection Revisited," OPODIS 2016 (arxiv 1608.06696); Dagstuhl LIPIcs OPODIS.2016.25; fpaxos.github.io.

Classical Paxos requires all quorums (regardless of phase or round) to intersect, typically via majorities. Flexible Paxos weakens this: only phase-1 quorums (Q1, leader election / prepare) and phase-2 quorums (Q2, replication / accept) must intersect. Formally, with Q1 the set of all valid phase-1 quorums and Q2 the set of all valid phase-2 quorums over acceptor set A:

- ∀Q1 ∈ Q1: Q1 ⊆ A
- ∀Q2 ∈ Q2: Q2 ⊆ A
- ∀Q1 ∈ Q1, ∀Q2 ∈ Q2: Q1 ∩ Q2 ≠ ∅

Within a phase, quorums may be disjoint; majorities are not necessary. FPaxos with majority quorums for both phases is equivalent to classical Paxos. This is the foundational result `ids-flexible-paxos` implements.

#### 3.2 Q1 + Q2 > N for simple quorum systems

**Source:** arxiv 1608.06696 (§3); blog.acolyer.org 2016-09-27; pamaron.com Flexible Paxos card; The-Agony-of-Consensus-Algorithms ch15.

For uniform (simple) quorum systems where every phase-1 quorum has size Q1 and every phase-2 quorum has size Q2, the intersection requirement reduces to the pigeonhole principle: `Q1 + Q2 > N`. Given Q2 and N, the minimum phase-1 quorum size is `Q1 = N − Q2 + 1`. Since phase 2 (replication) runs far more often than phase 1 (leader election) in Multi-Paxos, designers shrink Q2 (hot path) at the cost of growing Q1 (cold path). Example: N=5, Q1=4 (elect), Q2=2 (replicate) — any pair of acceptors intersects the election quorum of four, so a new leader cannot hide from prior writes. `ids-flexible-paxos` enforces `Q1_size + Q2_size > N` as a compile-time/runtime invariant for simple quorum configurations.

#### 3.3 Phase-1/phase-2 quorum separation

**Source:** arxiv 1608.06696; web.eecs.umich.edu flexiblepaxos.pdf slides; jack-vanlightly.com "Applying Flexible Paxos to Raft."

Phase 1 (Prepare/Promise): a proposer claims a ballot by contacting a Q1 quorum; acceptors promise not to accept lower-numbered ballots and report their highest accepted value. Phase 1 does not commit anything. Phase 2 (Propose/Accepted): the proposer asks a Q2 quorum to accept its value; acceptance by Q2 means the value may be decided. Because Phase 1 must discover any value that might have been decided, every Q1 must intersect every Q2. `ids-flexible-paxos` models these as separate quorum families with separate sizes, separate target endpoint sets, and separate message types (`Prepare`/`Promise` vs `Propose`/`Accepted`).

#### 3.4 Non-intersecting quorums within views

**Source:** arxiv 1608.06696; fsgeek.ca CPSC 416 lecture 14; The-Agony-of-Consensus-Algorithms ch15.

Phase-1 quorums across rounds need not intersect each other (phase 1 never decides). Phase-2 quorums across slots need not intersect each other (ballots provide ordering between decisions). This allows disjoint replication quorums in steady state — e.g., different acceptor subsets decide different slots concurrently, increasing throughput — as long as each phase-2 quorum intersects every phase-1 quorum. `ids-flexible-paxos` permits configurable disjoint quorum families per phase, validated against the cross-phase intersection invariant.

#### 3.5 Leader election quorum vs replication quorum

**Source:** arxiv 1608.06696 (§3 "FPaxos"); dl.acm.org 10.1145/3139645.3139656 (Revisiting the Paxos Foundations); blog.acolyer.org.

The leader election phase (Q1) runs rarely; the replication phase (Q2) runs on every slot. Flexible Paxos exploits the asymmetry: make Q2 small for low-latency, high-throughput replication; make Q1 large so elections are safe but infrequent. Availability consequence: FPaxos always tolerates up to |Q2| − 1 failures for continued replication; between |Q2| and N − |Q2| failures, replication continues until a new leader is required, at which point Q1 must be formable. `ids-flexible-paxos` exposes both quorum sizes as explicit config fields with documented availability implications.

#### 3.6 Availability optimization

**Source:** arxiv 1608.06696; arxiv 1703.08905 (WPaxos); jack-vanlightly.com (Flexible Paxos applied to Raft table: replication 1 ↔ election 5, replication 2 ↔ election 4, etc. for N=5).

Trade-off table (N=5): replication quorum 5 ↔ election quorum 3; replication 4 ↔ election 3; replication 3 ↔ election 3; replication 2 ↔ election 4; replication 1 ↔ election 5. Even-N improvement: with N even, Q2 can be N/2 instead of N/2+1 when Q1 stays majority — improved availability from reducing second-phase quorum size by one. Small disjoint phase-2 quorums speed up steady state. `ids-flexible-paxos` documents the full Q1/Q2 size spectrum per N and lets deployments pick a point on the availability/latency curve.

#### 3.7 Ballot/proposal numbers and epoch management

**Source:** arxiv 1608.06696 (Theorems 1-2 safety proof); arxiv 2203.03058 (Relaxed Paxos, revision B/C epoch-specific quorums); arxiv 1802.05969 (Paxos deconstructed).

Each attempt is tagged with a monotonically increasing ballot/proposal number (proposal number, acceptor id) totally ordered. Acceptors track the highest ballot promised and the highest ballot accepted. A higher-numbered phase-1 quorum must learn all values that lower-numbered phase-2 quorums may have decided. Relaxed Paxos refines: a phase-1 quorum for round i need only intersect phase-2 quorums of rounds 0..i−1 (cross-view intersection), and once a proposer learns a value was proposed in a previous round it need not intersect that round's quorums at all (revision C). `ids-flexible-paxos` models ballot numbers as `(epoch: u64, node_id: NodeId)` pairs and tracks per-acceptor `max_promised_ballot` and `max_accepted_ballot`.

#### 3.8 Promise/accept state per acceptor

**Source:** arxiv 1608.06696 safety proof (cases on message arrival order at an acceptor); fsgeek.ca lecture 14.

Per-acceptor state: `last_promised: Option<Ballot>`, `accepted: Option<(Ballot, Value)>`. On `Prepare(b)`: if `b ≥ last_promised`, update `last_promised = b`, reply `Promise(b, accepted)`. On `Propose(b, v)`: if `b ≥ last_promised`, update `accepted = Some((b, v))`, reply `Accepted(b)`. Illegal to accept a proposal below the last promised ballot. These transitions are enforced as state-machine invariants in the crate; state is opaque to the carrier (serialized payloads only).

#### 3.9 Quorum certificate assembly

**Source:** arxiv 1608.06696; fpaxos.github.io; CockroachDB Parallel Commits blog.

A phase-1 certificate = promises from ≥ Q1 distinct acceptors for ballot b, including the highest accepted (ballot, value) among them. A phase-2 certificate = acceptances from ≥ Q2 distinct acceptors for ballot b/value v; acceptance by Q2 constitutes a decision candidate. The proposer chooses the value associated with the highest reported ballot. Certificates are opaque `Vec<u8>` payloads, optionally ed25519-signed, sha256/blake3-hashed per entry. `ids-flexible-paxos` defines certificate structure and validation rules (distinct acceptor counting, ballot monotonicity, value selection by highest ballot).

#### 3.10 Cross-view intersection (intersection across views)

**Source:** arxiv 2203.03058 (Relaxed Paxos); arxiv 1608.06696 (§7 enhancement); dl.acm.org 10.1145/3139645.3139656.

Safety across views/epochs: the leader election quorum of view v must overlap with replication-phase quorums of all previous views (so a new leader learns all past decisions). Relaxed Paxos weakens further: intersection needed only with phase-2 quorums of rounds whose outcomes are still possibly relevant, based on what the proposer has learned. `ids-flexible-paxos` maintains a view/epoch history of used quorum families and validates cross-view intersection when a new view is opened.

#### 3.11 Reconfiguration and joint quorums

**Source:** The-Agony-of-Consensus-Algorithms ch15 (safe reconfiguration procedure); arxiv 1703.08905 (WPaxos config transitions Q1′∪Q1, Q2′∪Q2); arxiv 1608.06696 (§7 enhancements, Vertical Paxos analogy).

Changing quorum sizes at runtime requires old and new configurations' quorums to intersect during transition. Conservative approach: joint quorums (max of old/new Q1, max of old/new Q2) during a reconfiguration entry, then switch. `ids-flexible-paxos` defines the reconfiguration interface (old config → new config → joint config) and the invariant that must hold at each step: old Q1 ∩ old Q2, new Q1 ∩ new Q2, joint Q1 ∩ old Q2, new Q1 ∩ joint Q2, etc.

#### 3.12 Quorum availability bounds

**Source:** arxiv 1608.06696; arxiv 1703.08905 (WPaxos partial availability); arxiv 2609.39715 (adaptive Byzantine quorum comparison).

FPaxos tolerates up to |Q2| − 1 acceptor failures while continuing replication. If between |Q2| and N − |Q2| acceptors fail, replication continues until a leader change is needed (Q1 unformable). Larger Q1 reduces failure tolerance for elections but improves replication availability. `ids-flexible-paxos` computes and exposes `max_tolerated_failures_replication = Q2 − 1`, `max_tolerated_failures_election = Q1 − 1`, and the mixed-failure window.

#### 3.13 Grid and weighted quorum constructions

**Source:** arxiv 1703.08905 (WPaxos grid: rows/columns intersect even when |Q1|+|Q2| < N); The-Agony-of-Consensus-Algorithms ch15 (grid 3×3: Q1 = full column + one per other column = 5, Q2 = full row = 3, intersect at one cell); paulcavallaro.com blog; arxiv 2512.20485 (weighted object consensus); arxiv 2609.39715 (weighted WHEAT).

Beyond uniform quorums: grid quorums (rows vs columns guarantee geometric intersection without Q1+Q2 > N), hierarchical quorums (majority of groups, majority within group), weighted quorums (per-replica weights, thresholds W1, W2 with W1 + W2 > total weight). These topologies are useful when acceptors are heterogeneous (geo-distributed, datacenter-aware). `ids-flexible-paxos` defines the quorum-family abstraction so grid/weighted/hierarchical families can be plugged in; validation checks pairwise intersection of Q1 × Q2 families.

#### 3.14 Formal verification and model checking

**Source:** arxiv 1608.06696 (§5 Safety, TLA+ model check of single-valued FPaxos with disjoint quorums); arxiv 2203.03058; arxiv 2603.28788 (Legible Consensus, TLA+ verified quorum intersection).

The Flexible Paxos safety proof follows Lamport's Paxos proof structure but with relaxed assumptions: if value v decided with proposal p, then for any propose(p′, v′) with p′ > p, v = v′. Proof by contradiction via the single acceptor in Qp,2 ∩ Qp0,1. TLA+ model checked. `ids-flexible-paxos` specifies the same invariant (cross-phase intersection ⇒ no conflicting decisions) and lists model-checking as validation scope.

#### 3.15 Failure behavior per quorum configuration

**Source:** arxiv 1608.06696; dl.acm.org 10.1145/3139645.3139656; arxiv 1703.08905.

- If Q1 cannot be formed: no new leader; replication continues under current leader until it fails.
- If Q2 cannot be formed: no new decisions; reads/stale service possible per upper layers.
- Below-quorum phase-1: election blocked (choose consistency over availability for elections).
- Below-quorum phase-2: replication blocked for that ballot.
`ids-flexible-paxos` reports quorum formability (`can_form_q1`, `can_form_q2`) given the live acceptor set, so drivers can degrade gracefully.

#### 3.16 Carrier-agnostic transport boundary

**Source:** IDS prompts.txt entry 71; plan-001/plan-048/plan-060 format references; C:/idfs/ids-000-aa skeleton (CarrierType enum, Endpoint cap 1024 bytes).

`ids-flexible-paxos` never imports `std::net`, `tokio::net`, libp2p, or any networking crate. All outbound/inbound traffic is expressed as `(Endpoint, Vec<u8>)` pairs handed to the driver, which uses `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Message types (`Prepare`, `Promise`, `Propose`, `Accepted`) are serialized by the driver and passed as opaque bytes. The crate validates quorum logic only; delivery, retransmission, and ordering belong to the carrier layer.

#### 3.17 Allowed crypto surface

**Source:** IDS policy (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305); RFC 8032; RFC 7539; libsodium XChaCha20-Poly1305; NIST FIPS 180-4; BLAKE3 spec.

- ed25519: signing ballot claims and quorum certificates (proof of participation).
- sha256: ballot hashing, certificate entry hashing.
- blake3: quorum digest chains, fast checkpoint hashing.
- xchacha20poly1305: encrypt per-acceptor promise/accept state at rest (24-byte nonce, random nonces safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-bound).
No other crypto. `ids-crypto` implements; `ids-flexible-paxos` references primitive IDs via `ids-000-aa` only.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|---|---|---|
| Conflicting values decided (safety violation) | Consensus correctness | Cross-phase intersection invariant Q1 ∩ Q2 ≠ ∅ enforced; simple config requires Q1_size + Q2_size > N |
| Phase-1 quorums not intersecting past phase-2 quorums | Safety across views | View/epoch history tracked; new view's Q1 validated against all prior Q2 families |
| Disjoint phase-2 quorums causing divergent decisions | Decision consistency | Ballot numbers totally order attempts; value selected by highest reported ballot; only one value per ballot accepted |
| Stale/forgotten accepted value | Read-your-writes after leader change | Phase-1 quorum learns highest accepted value from intersecting acceptor; proposer adopts it |
| Lower-ballot accept after higher promise | Protocol state corruption | Acceptor state machine rejects Propose(b) when b < last_promised |
| Quorum reconfiguration split-brain | Safety during config change | Joint quorums during transition; old/new/joint cross-intersection invariants |
| Carrier duplicating/delaying datagrams | Replay/ordering | Ballot-numbered messages are idempotent; acceptor state is keyed by ballot; duplicates harmless |
| Clock manipulation for ballot order | Causality correctness | Logical ballot numbers only; never wall-clock; IdsTime monotonic only for timeouts |
| Key extraction (certificate signing key) | Certificate authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305) |
| Certificate tampering | Decision verifiability | ed25519 signatures over certificate bytes; sha256/blake3 entry hashing; offline verifiable |
| Q2 too small → long unavailability windows | Liveness | Documented max_tolerated_failures = Q2−1; driver chooses Q1/Q2 point on availability curve |
| Prohibited crate in dependency tree | Carrier-agnostic rule | CI dependency audit (cargo tree) bans std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS crates |
| Disallowed crypto algorithm | Crypto policy | Only the five allowed primitives; ids-crypto enforces; CI allowlist test |

The carrier is never trusted: quorum decisions and certificates are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque Endpoint, CarrierType + Vec<u8>) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-flexible-paxos |
|---|---|---|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All paxos traffic flows to/from `Endpoint`; crate returns decisions + `(Endpoint, Vec<u8>)` pairs, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing certificates/ballot claims; hashing ballots and digests; encrypting acceptor state at rest |
| `ids-consensus` (planned) | Consensus orchestration layer | Consumes flexible-paxos quorum decisions; drives Prepare/Promise/Propose/Accepted message exchange |
| `ids-replication` (planned) | Log replication | Phase-2 replication quorum decisions gate commit of log entries |
| `ids-membership` (planned) | Acceptor set / view membership | Supplies acceptor set and view/epoch changes; reconfiguration handshake |
| `ids-fault` (planned) | Failure detection | Feeds live acceptor set; `can_form_q1`/`can_form_q2` consulted on failure events |
| `ids-session` (planned) | Client-session binding | Client proposals carry ballot context; audit cross-referenced with session audit |

Integration points: `ids-flexible-paxos` is a pure logic crate with zero transport — it receives the live acceptor set and returns quorum decisions plus opaque datagrams for `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-flexible-paxos/` (a separate crate, not a folder inside an existing crate):

```
ids-flexible-paxos/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, serde, thiserror; workspace member
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # quorum defaults, max ballot, certificate limits
├── error.rs                            # PaxosError (thiserror): quorum, ballot, certificate, config errors
├── features.rs                         # Feature flags: model_check, signed_certs, encrypted_state
│
├── config/                             # Quorum configuration
│   ├── mod.rs
│   ├── quorum_config.rs                # Q1_size, Q2_size, N, invariant Q1+Q2 > N
│   ├── simple_quorum.rs                # Uniform threshold quorum family (any N-k subset)
│   ├── grid_quorum.rs                  # Row/column grid quorum families
│   ├── weighted_quorum.rs              # Per-acceptor weights, thresholds W1/W2
│   ├── hierarchical_quorum.rs          # Group-based quorum families
│   ├── quorum_family.rs                # Trait: enumerate valid quorums, check intersection
│   └── reconfiguration.rs              # Old→new→joint quorum config transitions
│
├── ballot/                             # Ballot / proposal numbers
│   ├── mod.rs
│   ├── ballot.rs                       # Ballot = (epoch: u64, node_id: NodeId), total order
│   ├── epoch.rs                        # View/epoch number, epoch history
│   └── sequence.rs                     # Slot/instance sequence numbers
│
├── state/                              # Per-acceptor protocol state
│   ├── mod.rs
│   ├── acceptor_state.rs               # last_promised, max_accepted (ballot, value)
│   ├── state_machine.rs                # Prepare/Promise/Propose/Accepted transitions
│   ├── promise_store.rs                # Per-ballot promise records
│   └── acceptance_store.rs             # Per-ballot acceptance records
│
├── phase1/                             # Leader election phase (Q1)
│   ├── mod.rs
│   ├── driver.rs                       # Phase-1 decision logic (prepare quorum)
│   ├── decision.rs                     # Phase1Decision: required_q1, targets, payload
│   ├── certificate.rs                  # Q1 promise certificate (Q1 distinct acceptors)
│   └── quorum_check.rs                 # Q1 formability, Q1 ∩ Q2 validation
│
├── phase2/                             # Replication phase (Q2)
│   ├── mod.rs
│   ├── driver.rs                       # Phase-2 decision logic (accept quorum)
│   ├── decision.rs                     # Phase2Decision: required_q2, targets, payload
│   ├── certificate.rs                  # Q2 acceptance certificate (Q2 distinct acceptors)
│   └── quorum_check.rs                 # Q2 formability, max tolerated failures
│
├── intersection/                       # Cross-phase / cross-view intersection
│   ├── mod.rs
│   ├── cross_phase.rs                  # Q1 × Q2 intersection invariant (Q1+Q2 > N)
│   ├── cross_view.rs                   # View v Q1 vs views < v Q2 intersection
│   ├── verifier.rs                     # Exhaustive pairwise intersection validation
│   └── disjoint.rs                     # Disjoint quorum families within a phase
│
├── certificate/                        # Quorum certificates
│   ├── mod.rs
│   ├── structs.rs                      # Certificate header, entries, signatures
│   ├── builder.rs                      # Certificate assembly from promises/acceptances
│   ├── validator.rs                    # Certificate validation (distinctness, ballots, hashes)
│   ├── chain.rs                        # sha256/blake3 certificate digest chain
│   └── signer.rs                       # ed25519 sign/verify of certificate bytes
│
├── availability/                       # Availability optimization
│   ├── mod.rs
│   ├── tradeoff.rs                     # Q1/Q2 size spectrum per N
│   ├── tolerance.rs                    # max_tolerated_failures_replication/election
│   └── spectrum.rs                     # Documented Q1/Q2 configuration table
│
├── messages/                           # Opaque message types (serialized by driver)
│   ├── mod.rs
│   ├── prepare.rs                      # Prepare(ballot)
│   ├── promise.rs                      # Promise(ballot, accepted)
│   ├── propose.rs                      # Propose(ballot, value)
│   ├── accepted.rs                     # Accepted(ballot)
│   └── codec.rs                        # serde/bincode encode/decode into Vec<u8>
│
├── carrier_adapter.rs                  # Opaque (Endpoint, Vec<u8>) datagram framing for ids-carrier
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs, quorum_contract.rs, ballot_contract.rs, phase1_contract.rs,
│   │   phase2_contract.rs, certificate_contract.rs, intersection_contract.rs,
│   │   state_contract.rs, availability_contract.rs, carrier_contract.rs
│
├── invariants/                         # Compile-time/runtime invariants (~10 files)
│   ├── mod.rs, cross_phase_intersection.rs, q1_plus_q2_gt_n.rs,
│   │   ballot_monotonicity.rs, promise_before_accept.rs, distinct_acceptors.rs,
│   │   cross_view_intersection.rs, carrier_agnostic.rs, crypto_allowlist.rs,
│   │   reconfiguration_soundness.rs
│
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── mod.rs, audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs
│
├── crypto/                             # Type-level crypto surface (~5 files)
│   ├── mod.rs, ed25519_surface.rs, sha256_surface.rs, blake3_surface.rs,
│   │   xchacha20_surface.rs, chacha20_surface.rs
│
├── tests/                              # Property/verification tests (~30 files)
│   ├── mod.rs, quorum_config_tests.rs, simple_quorum_tests.rs, grid_quorum_tests.rs,
│   │   weighted_quorum_tests.rs, intersection_tests.rs, cross_view_tests.rs,
│   │   ballot_tests.rs, acceptor_state_tests.rs, phase1_tests.rs, phase2_tests.rs,
│   │   certificate_tests.rs, chain_tests.rs, availability_tests.rs, tolerance_tests.rs,
│   │   reconfiguration_tests.rs, message_codec_tests.rs, invariant_tests.rs,
│   │   carrier_agnostic_tests.rs, crypto_allowlist_tests.rs, dependency_audit_test.rs,
│   │   model_check_tests.rs, determinism_tests.rs, property_tests.rs
│
├── benches/                            # (~6 files)
│   ├── mod.rs, quorum_intersection.rs, certificate_validation.rs,
│   │   ballot_ordering.rs, availability_spectrum.rs, config_spectrum.rs
│
├── spec/                               # Normative specifications (~6 files)
│   ├── quorum_spec.md, ballot_spec.md, phase_spec.md, certificate_spec.md,
│   │   intersection_spec.md, carrier_agnostic_spec.md
│
├── docs/                               # Documentation (~12 files)
│   ├── architecture.md, quorum_tradeoffs.md, reconfiguration.md,
│   │   availability.md, safety_proof.md, crypto_policy.md, adding_quorum_family.md,
│   │   integration_guide.md, runbook.md, changelog.md, faq.md, glossary.md
│
├── baselines/                          # (~3 files)
│   ├── quorum_intersection_baseline.json, certificate_baseline.json, availability_baseline.json
│
└── test_vectors/                       # (~4 files)
    ├── ballot_vectors.json, quorum_vectors.json, certificate_vectors.json, intersection_vectors.json
```

Total: ~90–1000 files across `src/`, `src/config/`, `src/ballot/`, `src/state/`, `src/phase1/`, `src/phase2/`, `src/intersection/`, `src/certificate/`, `src/availability/`, `src/messages/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `src/crypto/`, `tests/`, `benches/`, `spec/`, `docs/`, `baselines/`, `test_vectors/`. One concern per file; no stubs.

---

### 7. Key Algorithms & Data Structures

1. **Quorum config invariant** — `QuorumConfig { n: u64, q1_size: u64, q2_size: u64 }` with invariant `q1_size + q2_size > n` (simple quorums); `min_q1(n, q2) = n − q2 + 1`.

2. **Quorum family abstraction** — `trait QuorumFamily { valid_quorums(&self) -> Vec<BitSet<NodeId>>; intersects(&self, other: &dyn QuorumFamily) -> bool }`; implementations: `SimpleQuorum`, `GridQuorum`, `WeightedQuorum`, `HierarchicalQuorum`.

3. **Cross-phase intersection check** — `∀q1 ∈ Q1_family, ∀q2 ∈ Q2_family: q1 ∩ q2 ≠ ∅`; exhaustive pairwise validation for small families; sampling + structural proof for grid/weighted families.

4. **Ballot total order** — `Ballot { epoch: u64, node_id: NodeId }` ordered lexicographically; `max_promised`, `max_accepted` per acceptor; `Propose(b, v)` accepted only if `b ≥ last_promised`.

5. **Phase-1 decision** — collect `Promise` from ≥ Q1 distinct acceptors for ballot b; adopt value with highest accepted ballot among promises; return `Phase1Decision { adopted_value, certificate }`.

6. **Phase-2 decision** — collect `Accepted` from ≥ Q2 distinct acceptors for (ballot, value); quorum reached ⇒ value decided; return `Phase2Decision { decided: bool, certificate }`.

7. **Cross-view intersection** — maintain `ViewHistory { views: Vec<(epoch, Q1_family, Q2_family)> }`; when opening view v, validate `Q1_v ∩ Q2_u ≠ ∅` for all `u < v` (Relaxed Paxos refinement: only views whose outcomes may still matter).

8. **Quorum certificate** — header (ballot, phase, view), entries `(acceptor_id, signature or hash)`, aggregate sha256/blake3 chain; ed25519 signature over full certificate bytes.

9. **Availability spectrum** — for each N, table of `(Q1, Q2)` pairs satisfying Q1+Q2 > N with `max_tolerated_failures_replication = Q2 − 1`, `max_tolerated_failures_election = Q1 − 1`; example N=5: (5,3),(4,3),(3,3),(2,4),(1,5).

10. **Reconfiguration** — joint config `Q1_joint = max(Q1_old, Q1_new)`, `Q2_joint = max(Q2_old, Q2_new)`; validate old-old, new-new, old-joint, joint-new intersections at each transition step.

11. **Carrier datagram framing** — `carrier_adapter` serializes `Prepare`/`Promise`/`Propose`/`Accepted` into `Vec<u8>` and pairs with target `Endpoint`; driver transmits via `ids-carrier::Endpoint`; incoming datagrams are deserialized and routed to the phase state machine.

12. **Audit chain** — each quorum decision (admit/reject/decide) = sha256/blake3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

`ids-flexible-paxos/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (type-level surface + primitive IDs).
- `serde` + `serde_bytes` — serialization of quorum messages/certificates into `Vec<u8>`.
- `thiserror` — error types.
- `bitflags` / `bit-set` — acceptor bitsets for quorum distinctness counting.
- Dev-only: `proptest` (quorum intersection properties), `tempfile` (fixtures).

No `std::net`, `tokio::net`, libp2p, TCP/IP, UDP, QUIC, HTTP, DNS implementations. No crypto beyond the allowed list; `ids-flexible-paxos` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-71.1 | Quorum config: `Q1_size + Q2_size > N` enforced for simple quorums; `min_q1(n, q2) = n − q2 + 1` | Config construction rejects invalid pairs; unit tests over N=3..=100 |
| AC-71.2 | Cross-phase intersection: every Q1 × Q2 pair intersects; within-phase disjoint allowed | Pairwise verifier passes on configured families; disjoint Q1-Q1 and Q2-Q2 accepted |
| AC-71.3 | Phase-1/phase-2 separation: separate quorum families, sizes, message types | `phase1` and `phase2` modules independent; `Prepare`/`Promise` vs `Propose`/`Accepted` distinct |
| AC-71.4 | Leader election quorum vs replication quorum asymmetry configurable | Q1 and Q2 sizes set independently; N=5 example (Q1=4, Q2=2) validated |
| AC-71.5 | Availability: tolerates up to Q2−1 failures for replication; Q1−1 for election | `max_tolerated_failures_*` computed and tested per config |
| AC-71.6 | Ballot total order: `(epoch, node_id)` lexicographic; monotonic per acceptor | `Ballot::cmp` tests; acceptor rejects `b < last_promised` |
| AC-71.7 | Promise/accept state machine: legal transitions only; illegal accept rejected | State-machine unit tests (all transition cases from FPaxos safety proof) |
| AC-71.8 | Quorum certificate: Q1 distinct acceptors (phase 1) / Q2 distinct acceptors (phase 2) | Certificate builder/validator tests; duplicate acceptor rejected |
| AC-71.9 | Cross-view intersection: view v Q1 vs all prior views' Q2 | `ViewHistory` validation tests; Relaxed Paxos refinement cases |
| AC-71.10 | Reconfiguration: joint quorums satisfy old/new/joint cross-intersection | Reconfiguration transition tests with invariant checks at each step |
| AC-71.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep — zero matches |
| AC-71.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-71.13 | Every quorum decision in append-only hash-chained, ed25519-signed audit log | Audit chain tests; offline verification |
| AC-71.14 | Crate compiles cleanly: `cargo check -p ids-flexible-paxos` exits 0, no warnings | Build verified |
| AC-71.15 | Tests pass: `cargo test -p ids-flexible-paxos` all green | Test run verified |

---

### 10. Remaining Uncertainties

1. **Concrete Q1/Q2 defaults per deployment:** N=5 (Q1=4, Q2=2) is the illustrative point; production defaults depend on failure tolerance vs latency targets and must be validated at integration with `ids-consensus`/`ids-replication`.
2. **Which views' phase-2 quorums must intersect a new view's Q1:** Relaxed Paxos refinement (only still-relevant rounds) reduces intersection scope, but the exact relevance criterion is deployment-specific.
3. **Grid/weighted/hierarchical quorum family adoption:** simple threshold quorums are the default; richer families (grid, weighted) need topology-aware intersection proofs before use.
4. **Certificate size and signing overhead:** ed25519 signing every certificate may be expensive at high slot rates; optional signing vs hashing trade-off unquantified.
5. **Ballot number exhaustion:** `u64` epoch with per-node counter — wrap-around behavior after 2^64 epochs undocumented; practically irrelevant but needs a stated bound.
6. **Interaction with `ids-membership` view changes:** acceptor set changes mid-consensus require joint quorum transitions; exact handshake to be defined at integration.
7. **Cross-carrier compatibility:** opaque `Vec<u8>` payloads must decode identically across carriers; codec versioning to be aligned with `ids-000-aa` codec layer.
8. **Model-checking scope:** TLA+ verification of the multi-valued protocol with flexible quorums is validation-phase work, not part of this crate's implementation.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate `ids-flexible-paxos` crate (not a folder) | Project rule: separate crates for new capabilities; clear quorum-logic boundary |
| Cross-phase intersection only (Q1 ∩ Q2 ≠ ∅), within-phase disjoint allowed | Flexible Paxos core theorem (Howard/Malkhi/Spiegelman, OPODIS 2016); strictly generalizes Paxos |
| Simple quorums default: `Q1_size + Q2_size > N` | Pigeonhole principle; uniform special case; easy to configure and validate |
| Phase-1 = leader election (Q1), phase-2 = replication (Q2) | Matches Multi-Paxos asymmetry; replication frequent → small Q2; election rare → large Q1 |
| Ballot = `(epoch: u64, node_id: NodeId)`, totally ordered | Standard Paxos proposal-number ordering; enables cross-view safety reasoning |
| Per-acceptor state: `last_promised`, `max_accepted` | Directly from FPaxos safety proof; enforces promise-before-accept |
| Quorum certificates: Q1 distinct acceptors / Q2 distinct acceptors | Distinctness counting prevents ballot-stuffing via duplicate messages |
| Cross-view intersection via view/epoch history | Safety across leader changes; new leader learns all past decisions |
| Joint-quorum reconfiguration (max of old/new sizes) | Conservative, provably safe transition; generalizes Raft joint consensus to two quorum sizes |
| Availability spectrum documented per N | Lets deployments pick Q1/Q2 point on latency/failure-tolerance curve |
| All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; via `ids-crypto`, type-level surface only in this crate |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable quorum decision log |
| Logical ballots only; no wall-clock for ordering | Clock manipulation cannot break quorum safety |

---

### 12. Sources

1. Howard, Malkhi, Spiegelman — "Flexible Paxos: Quorum Intersection Revisited," OPODIS 2016. https://arxiv.org/html/1608.06696 — core paper: cross-phase intersection, Q1+Q2>N, TLA+ model check, availability trade-offs.
2. Howard, Malkhi, Spiegelman — "Flexible Paxos: Quorum Intersection Revisited" (Dagstuhl LIPIcs OPODIS.2016.25). https://drops.dagstuhl.de/storage/00lipics/lipics-vol070-opodis2016/LIPIcs.OPODIS.2016.25/LIPIcs.OPODIS.2016.25.pdf — formal quorum system definition and safety proof.
3. fpaxos.github.io — "Flexible Paxos | Quorum intersection revisited." https://fpaxos.github.io/ — project page: phase-1/phase-2 quorum overlap requirement.
4. The Morning Paper (blog.acolyer.org), 2016-09-27 — "Flexible Paxos: Quorum intersection revisited." https://blog.acolyer.org/2016/09/27/flexible-paxos-quorum-intersection-revisited/ — Q1+Q2>N explanation, even-N improvement.
5. The Agony of Consensus Algorithms, ch15 — "Flexible Paxos and Quorum Relaxation." https://github.com/cloudstreet-dev/The-Agony-of-Consensus-Algorithms/blob/main/src/ch15-flexible-paxos.md — Q1/Q2 size spectrum table (N=5), grid quorums, weighted quorums, reconfiguration procedure.
6. Relaxed Paxos: Quorum Intersection Revisited (Again), arxiv 2203.03058. https://arxiv.org/html/2203.03058 — epoch-specific quorum weakening (revisions B/C), cross-view intersection refinement.
7. Fast Flexible Paxos: Relaxing Quorum Intersection for Fast Paxos, ACM 10.1145/3427796.3427815. https://doi.org/10.1145/3427796.3427815 — extension of flexible quorums to Fast Paxos.
8. WPaxos: Ruling the Archipelago with Fast Consensus, arxiv 1703.08905. https://arxiv.org/html/1703.08905v3 — grid quorums (rows/columns intersect without Q1+Q2>N), WAN flexible consensus, config transitions.
9. CockroachDB — "Parallel Commits: An atomic commit protocol for globally distributed transactions." https://cockroachlabs.com/blog/parallel-commits — Flexible Paxos applied at transaction scope (Q1=1 coordinator, Q2=all participants).
10. Revisiting the Paxos Foundations (VMware Research), ACM 10.1145/3139645.3139656. https://dl.acm.org/doi/10.1145/3139645.3139656 — leader-election vs replication quorum overlap guarantee; counting quorums (N=10, Q2=3, Q1=8).
11. CPSC 416 Lecture 14: Flexible Paxos (UBC). https://fsgeek.ca/media/2026/cpsc416-w25t2/L14-2026-02-26-flexible-paxos.pdf — phase-1/phase-2 intersection intuition, safety argument.
12. jack-vanlightly.com — "Applying Flexible Paxos to Raft." https://jack-vanlightly.com/blog/2022/6/9/applying-flexible-paxos-to-raft — replication/election quorum table for N=5; availability trade-offs.
13. paulcavallaro.com — "Flexible Paxos only needs Q1 + Q2 > N." https://pamaron.com/card/flexible-paxos-q1-plus-q2 — election set vs accept set overlap; Q2=N/2 with Q1=N−Q2+1 for even N.
14. Distributed consensus revised – Part III (blog.acolyer.org, 2019-05-10). https://blog.acolyer.org/2019/05/10/distributed-consensus-revised-part-iii — revisions A/B/C, epoch-specific quorums, skip phase 1 for minimum epoch.
15. Topology-Aware Quorum Geometry for Asymmetric Networks, arxiv 2603.28788. https://arxiv.org/html/2603.28788 — crumbling-wall construction, cross-intersection verification, Phase 2 hot path vs Phase 1 cold path.
16. RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA), ed25519. https://www.rfc-editor.org/rfc/rfc8032
17. RFC 7539 — ChaCha20 and Poly1305 for IETF Protocols. https://www.rfc-editor.org/rfc/rfc7539
18. Libsodium — XChaCha20-Poly1305 construction. https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
19. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
20. BLAKE3 — BLAKE3-team specification. https://github.com/BLAKE3-team/BLAKE3
21. Lamport — "Paxos Made Simple." https://lamport.azurewebsites.net/pubs/paxos-simple.pdf — classical Paxos two-phase quorum basis.
22. FLP impossibility (Fischer, Lynch, Paterson). https://groups.csail.mit.edu/tds/papers/Lynch/jacm85.pdf — consensus needs partial synchrony for progress (context for liveness bounds).
23. Dwork, Lynch, Stockmeyer — "Consensus in the presence of partial synchrony," JACM 1988. https://dl.acm.org/doi/10.1145/3139645.3139656 (context) — partial synchrony assumption for Paxos progress.
24. IDS prompts.txt entry 71 (verbatim). C:/idfs/prompts.txt lines 1060-1074 — Phase 071 crate definition, contents, constraints.
25. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, CarrierType, Endpoint (1024-byte cap), sealed trait contracts, Cargo.toml.
26. IDS plan format references. C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary → Sources structure used for this plan.
27. HX-241 carrier-agnostic socket abstraction. https://raw.githubusercontent.com/hxlabz/hx241-protocol/main/hx-inbound/src/receive/socket.rs — Carrier/Socket/Endpoint traits with no networking-stack references.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, quorum families, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 071 (ids-flexible-paxos).
