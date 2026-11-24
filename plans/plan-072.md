## Phase 072 — Create the ids-bft-consensus crate: Byzantine fault-tolerant consensus.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 072 creates `ids-bft-consensus`, the IDS crate for Byzantine fault-tolerant consensus. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by protocol family. `ids-bft-consensus` covers PBFT, HotStuff, LibraBFT/DiemBFT, Zyzzyva, SBFT, message complexity, view change, view synchronization, safety under Byzantine faults, and liveness under partial synchrony.

**Core design:** each protocol family is an isolated submodule exposing a `ConsensusEngine` trait contract declared in `ids-000-aa`. Engines run on quorums of `n >= 3f + 1` replicas tolerating up to `f` Byzantine replicas, organize progress into views with a designated leader/primary per view, use quorum certificates (threshold-signed aggregates of `2f+1` votes) as safety proofs, and route every message as an opaque `Vec<u8>` datagram to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B). The crate never opens sockets, never touches `std::net`/`tokio::net`/libp2p/TCP/UDP/QUIC/HTTP/DNS, and never implements a carrier; it returns decisions and framed datagrams for the caller to send via `ids-carrier::Endpoint`.

Allowed crypto only: ed25519 (signing votes/QCs), xchacha20poly1305 and chacha20poly1305 (AEAD state at rest), sha256 (block/header hashing), blake3 (content addressing, QC binding). Crypto operations live in `ids-crypto`; `ids-bft-consensus` consumes primitive IDs and type surface (`Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum`) from `ids-000-aa`/`ids-crypto` only.

**Scope boundary:** this crate defines the consensus *policy* and *message flow* — safety predicates (quorum intersection, prepare/commit locking), liveness conditions (partial synchrony / GST, optimistic responsiveness), view-change and view-synchronization procedures, and safety under Byzantine faults. It does not execute client commands, store state, replicate data, or transport bytes itself. Those concerns live in downstream crates (`ids-store`, `ids-replication`, `ids-carrier`, `ids-crypto`).

A skeleton `ids-bft-consensus` crate does NOT yet exist at `C:/idfs/ids-bft-consensus/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract referenced here. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned PBFT (Castro & Liskov 1999/2002; three-phase pre-prepare/prepare/commit; view change; O(n²) normal case, O(n³) view change, O(n⁴) cascading failures; 3f+1 replicas), HotStuff (Yin et al. 2018/2019; linear view change O(n), optimistic responsiveness, three-phase prepare/pre-commit/commit, quorum certificates, threshold signatures, chained/pipelined variant), HotStuff-2 (two-phase responsive BFT), LibraBFT/DiemBFT (HotStuff-derived; round synchronization, nil-block votes, two-chain commit rule, quadratic view-sync fallback), Zyzzyva (Kotla et al. 2007; speculative execution, output commit at client, 3f+1 replicas, agreement/view-change/checkpoint sub-protocols), SBFT (dual-mode; optimistic linear PBFT via collector + threshold signatures, quadratic fallback; n = 3f+2c+1; c slow/crashed replica tolerance), message complexity (PBFT O(n²) normal/O(n³) view change/O(n⁴) cascade; HotStuff O(n) view change/O(n²) worst case; linear PBFT via collector reduces all-to-all to linear), view synchronization (Cogsworth; optimistically linear view sync, quadratic baseline), safety under Byzantine faults (quorum intersection ≥ f+1 correct replicas in Byzantine quorums of 2f+1; prepare/commit locking; conflicting proposals impossible), liveness under partial synchrony (DLS 1988; GST model; safety always, liveness after GST; FLP impossibility for deterministic asynchronous consensus), and FLP impossibility (Fischer, Lynch, Paterson 1985). Sources cited in Section 13.

---

### 2. Architecture Overview

`ids-bft-consensus` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a protocol-policy layer: given a consensus request tagged with a protocol family and a replica set, it decides (a) the current view/round, (b) the leader/primary for the view, (c) whether a proposal/vote is valid under safety predicates, (d) whether a quorum certificate has formed, (e) whether a block/commit is decided, (f) when to trigger view change/view synchronization, and (g) what failure behavior applies. It never sends bytes itself; it returns `ConsensusDecision` and framed `Vec<u8>` datagrams for the carrier boundary.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (consensus users)              │
│  ids-store, ids-replication — submit commands tagged with protocol family   │
├─────────────────────────────────────────────────────────────────────────────┤
│               Phase 072: ids-bft-consensus (THIS CRATE — policy)           │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │\\
│  │ protocol        │ │ quorum          │ │ view change /   │ │ safety   │ │\\
│  │ registry        │ │ certificates    │ │ view sync         │ │ predicates │ │\\
│  │ (PBFT/HotStuff/ │ │ (QC, TC, votes) │ │ (timeout, new-view)│ │ (quorum   │ │\\
│  │  LibraBFT/Zyzzyva│ │                 │ │                 │ │  intersect│ │\\
│  │  /SBFT engines) │ │                 │ │                 │ │  /lock)   │ │\\
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │\\
│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │\\
│  │ liveness        │ │ message         │ │                 │                │ │\\
│  │ (partial sync/  │ │ complexity      │ │                 │                │ │\\
│  │  GST/responsiveness)│ │ analysis        │ │                 │                │ │\\
│  └─────────────────┘ └─────────────────┘ │                 │                │ │\\
│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │\\
│  │ decision types  │ │ crypto surface  │ │                 │                │ │\\
│  │ (propose/vote/  │ │ (ed25519/QC sig)│ │                 │                │ │\\
│  │  commit/decide) │ │                 │ │                 │                │ │\\
│  └─────────────────┘ └─────────────────┘ │                 │                │ │\\n└─────────────────────────────────────────┬──────────────────┴──────────────┘\\n                                          │ opaque Vec<u8> datagrams\\n┌─────────────────────────────────────────▼──────────────────┐\\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only  │\\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>},  │\\n│  payload: &[u8])                                           │\\n│  receive_datagram → (Endpoint, Vec<u8>)                    │\\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC │\\n│  / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN / mesh  │\\n└────────────────────────────────────────────────────────────┘
```

**Data flow of a PBFT normal-case decision:** client submits `Request{client_id, op, seq}` → primary (view v) assigns sequence number and multicasts `PRE-PREPARE{v, n, d, m}` → backups validate and multicast `PREPARE{v, n, d, i}` → on `2f+1` matching prepares (prepared), replica multicasts `COMMIT{v, n, d, i}` → on `2f+1` commits (committed-local), replica applies and replies to client → client accepts after `f+1` identical replies. All messages are framed as opaque `Vec<u8>` to `ids-carrier::Endpoint` targets.

**Data flow of a HotStuff chained decision:** leader of view v proposes block extending highest QC known → replicas vote via `safeNode` predicate (extend locked QC or higher view) → leader aggregates `2f+1` votes into `prepareQC` → next leader relays `prepareQC` as pre-commit proof → replicas lock → `commitQC` formed → `decide`. Three consecutive certified blocks with consecutive views commit the first (3-chain commit rule). View change = normal protocol path; new leader collects `2f+1` new-view messages with QCs, picks highest QC, linear O(n) communication.

**Protocol-family semantics (research-verified):**

- **PBFT**: three-phase (pre-prepare/prepare/commit), primary = v mod n, n = 3f+1, f Byzantine tolerated. Safety: quorum intersection of Byzantine quorums (2f+1) contains ≥ f+1 correct → ≥ 1 correct replica overlaps any two quorums, preventing conflicting prepares/commits at same (view, seq). Liveness: partial synchrony; view change elects new primary on timeout. Message complexity: O(n²) normal case, O(n³) view change, O(n⁴) cascading view changes without threshold signatures.
- **HotStuff**: leader-based, partially synchronous, n = 3f+1. Three-phase (prepare/pre-commit/commit) unified into single view-change path. Quorum certificates via threshold signatures → O(n) per phase, O(n) view change, optimistic responsiveness (leader proceeds on first n−f responses after GST). SafeNode predicate allows voting for new proposal if it extends locked QC or has higher view. Chained/pipelined variant commits one command per view, 3-chain commit rule.
- **LibraBFT / DiemBFT**: HotStuff-derived, round-based, leader rotates every block. Round synchronization bounds commit latency under synchrony; nil-block vote allows commits despite faulty leaders; two-chain commit rule (DiemBFT V4); quadratic view-synchronization fallback (DiemBFT production) helps slow parties catch up; linear happy path.
- **Zyzzyva**: speculative BFT, n = 3f+1, primary proposes order, replicas execute speculatively and reply immediately; clients detect divergence via history info and act only on stable replies; three sub-protocols: agreement, view change, checkpoint. Optimistic two-phase (servers avoid all-to-all commit); slower three-phase on fault. Liveness only during synchrony periods.
- **SBFT**: dual-mode protocol. Optimistic mode: linear PBFT via collector replica + threshold signatures, O(cn) normal-case communication (c slow/crashed replicas tolerated, c ≤ f/8 heuristic → O(fn)); fallback mode: PBFT-like quadratic. n = 3f+2c+1 (not optimal resilience for c > 0).

**Crypto used:** ed25519 for signing votes, QCs, view-change messages, and checkpoints; sha256 for block/header/Proposal hashing and QC binding; blake3 for content addressing and fast QC/digest hashing; xchacha20poly1305/chacha20poly1305 for encrypting consensus state at rest (locked QC, view history, view-change certificates) with bounded nonces. No other crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 PBFT (Practical Byzantine Fault Tolerance)

**Source:** Castro & Liskov, "Practical Byzantine Fault Tolerance" (OSDI 1999 / revisited 2002); MIT 6.824 course notes; NUS CS6234 slides; Utah CS6963 notes.

PBFT is the first practical BFT state-machine-replication protocol tolerating Byzantine faults in asynchronous environments, safe all the time and live during synchrony. It assumes n = 3f + 1 replicas tolerating f Byzantine replicas. Normal operation is a three-phase protocol:

- **Pre-prepare**: client sends request to primary; primary assigns sequence number n in view v, broadcasts `PRE-PREPARE{v, n, d, m}` where d = digest(m). Backups accept iff valid, same view, sequence not already used for a different digest.
- **Prepare**: on accepting pre-prepare, replica multicasts `PREPARE{v, n, d, i}`. A replica is *prepared* when it has pre-prepared plus 2f+1 matching PREPARE messages (including its own).
- **Commit**: on becoming prepared, replica multicasts `COMMIT{v, n, d, i}`. A replica is *committed-local* when prepared plus 2f+1 matching COMMITs; it then applies and replies. `committed` holds when prepared is true at f+1 non-faulty replicas; the view-change invariant guarantees cross-view consistency.

Safety argument: two different messages cannot have the same (view, sequence) because any two Byzantine quorums of 2f+1 intersect in ≥ f+1 replicas, of which ≥ 1 is correct, and a correct replica never signs conflicting messages for the same (view, seq). Liveness: guaranteed during partial synchrony via view change on primary failure/timeout.

Message complexity: O(n²) in normal case (all-to-all in prepare/commit), O(n³) in view change (each of 2f+1 replicas sends view-change to new primary; new primary broadcasts new-view proof of 2f+1 view-changes to all), O(n⁴) in cascading view-change failures without threshold signatures.

`ids-bft-consensus` PBFT engine implements pre-prepare/prepare/commit phases, prepared/committed-local predicates, view-change with new-view proof, checkpoint (state truncation), and client reply collection (f+1 identical replies).

#### 3.2 HotStuff (BFT Consensus in the Lens of Blockchain)

**Source:** Yin, Malkhi, Reiter, Gueta, Abraham, "HotStuff: BFT Consensus in the Lens of Blockchain" (arXiv:1803.05069, PODC 2019); "Lessons from HotStuff" (arXiv:2305.13556); eprint.iacr.org/2022/551 (HotStuff rotating-leader details); arxiv.org/html/2501.03695 (responsiveness survey); arxiv.org/pdf/2010.11454 (Fast-HotStuff/chained HotStuff).

HotStuff is a leader-based BFT replication protocol for the partial synchrony model. After GST, a correct leader drives consensus at actual network delay (optimistic responsiveness) with linear communication in replica count. Key properties: Linear View Change (O(n) authenticators after GST; O(n²) worst case with cascading leader failures) and Optimistic Responsiveness (wait only for first n−f responses). HotStuff is the first partially synchronous BFT SMR protocol with both properties.

Core: views numbered monotonically; each view has unique leader known to all (leader = view mod n in rotating mode). Each replica stores a tree of pending blocks; each node has parent link. Commit requires three phases — prepare, pre-commit, commit — each collecting votes from quorum n−f = 2f+1 into a Quorum Certificate (QC), a threshold signature over `(type, view, block)`. The `safeNode` predicate lets a replica vote for a new proposal if it extends its locked QC or comes from a higher view, enabling seamless leader replacement.

Chained HotStuff: view changes on every prepare phase; each proposal's prepare phase doubles as the previous view's pre-commit/commit, so the pipeline commits one command per view with one round per view in steady state; 3-chain commit rule (block committed when it is the first of three adjacent certified blocks with consecutive view numbers).

HotStuff-2 (eprint.iacr.org/2023/397): two-phase responsive variant achieving O(n²) worst-case, optimistically linear communication, optimistic responsiveness, and two-phase commit within a view.

`ids-bft-consensus` HotStuff engine implements prepare/pre-commit/commit phases, QC formation via threshold signatures, safeNode voting rule, chained/pipelined variant, view change as unified protocol path, and 3-chain commit rule.

#### 3.3 LibraBFT / DiemBFT

**Source:** developers.diem.com "State Machine Replication in the Libra Blockchain" (2019-06-28) and "Diem Consensus" (2020-04-09); arxiv.org/html/2103.03181 (DiemBFT linear + async fallback); arxiv.org/pdf/2010.11454 (DiemBFT adoption of pipelined HotStuff).

LibraBFT is the Diem (Libra) blockchain's BFT consensus, HotStuff-derived. Rounds with rotating leader (leader = round mod n). Each round: leader proposes block extending highest QC; validators vote by sending threshold-signature shares to next leader; next leader forms QC and embeds in next proposal. Commit via 3-chain (three consecutive rounds) originally; DiemBFT V4 switched to a two-chain commit rule. Round synchronization bounds commit latency under synchrony; nil-block vote lets commits proceed despite faulty leaders. Quadratic view-synchronization fallback (production DiemBFT) helps slow parties catch up under partial synchrony failure; async fallback exists for worst-case adversary (no liveness guarantee under full asynchrony — consistent with FLP).

`ids-bft-consensus` LibraBFT engine implements round-based proposal/vote/QC chain, round synchronization, nil-block votes, two-chain/3-chain commit rules (configurable), and quadratic view-sync fallback.

#### 3.4 Zyzzyva (Speculative Byzantine Fault Tolerance)

**Source:** Kotla, Alvisi, Dahlin, Clement, Wong, "Zyzzyva: Speculative Byzantine Fault Tolerance" (SOSP 2007; CACM 2008; arxiv/doi 10.1145/1400214.1400236); Cornell CS5414 notes; UMich slides.

Zyzzyva reduces BFT replication cost via speculation. n = 3f + 1 replicas; primary proposes order; replicas *speculatively execute* requests without expensive agreement and reply immediately to clients. Correct replicas may temporarily diverge; clients use reply + history information to detect inconsistency and act only on stable replies guaranteed to eventually be committed. Three sub-protocols: (1) agreement (orders requests within views, primary-led), (2) view change (elects new primary on fault/slowness), (3) checkpoint (bounds state, reduces view-change cost).

Optimistic case: two phases (primary → replicas speculative execution/reply; client stabilizes via f+1 matching replies), avoiding all-to-all commit. Faulty/slow primary forces slower three-phase path. Liveness guaranteed only during synchrony intervals (FLP-consistent: no deterministic asynchronous liveness with even one fault).

`ids-bft-consensus` Zyzzyva engine implements speculative agreement, client-driven stabilization, view change, checkpoint, and the fast/slow path selection.

#### 3.5 SBFT (Scalable Byzantine Fault Tolerance)

**Source:** ieeexplore.ieee.org/document/8809541 (SBFT: A Scalable and Decentralized Trust Infrastructure); arxiv.org/pdf/2002.03613 (LinearIBFT/SBFT comparison); arxiv.org/html/2204.03181 (comprehensive BFT review incl. SBFT); ACM Computing Surveys review (SBFT linear-PBFT via threshold signatures).

SBFT is a dual-mode BFT consensus protocol. Optimistic ("fast") mode: linear PBFT — instead of all-to-all, each replica sends to a designated *collector*, which aggregates into one broadcast; threshold signatures collapse the collector's outgoing message to constant size. Normal-case communication O(cn) where c = number of tolerated slow/crashed replicas (heuristic c ≤ f/8 → O(fn)); n = 3f + 2c + 1 (sub-optimal resilience when c > 0). Fallback ("slow") mode: PBFT-like quadratic when collector fails or faults exceed c. A single slow/failed collector forces switch to fallback; SBFT parameterizes c to balance.

`ids-bft-consensus` SBFT engine implements collector-based linear PBFT fast path, threshold-signature aggregation, fallback to quadratic PBFT, and c-parameterized fault tolerance.

#### 3.6 Message Complexity Analysis

**Source:** PBFT (Castro-Liskov); HotStuff paper (complexity table vs PBFT/BFT-SMaRt/SBFT); "Bottlenecks in Blockchain Consensus" (arxiv 2103.04234); "Half a Century of BFT Consensus" (arxiv 2407.19863); ACM Computing Surveys BFT review.

| Protocol | Normal case | View change | Worst case (cascade) |
|---|---|---|---|
| PBFT | O(n²) | O(n³) | O(n⁴) |
| HotStuff | O(n) per phase | O(n) | O(n²) cascading leaders |
| LibraBFT/DiemBFT | O(n) | O(n²) view-sync fallback | O(n²) |
| Zyzzyva | O(n²) (optimistic avoids all-to-all commit) | O(n³) | O(n⁴) |
| SBFT (fast) | O(cn), c ≤ f/8 → O(fn) | O(n²) fallback | O(n²) |
| Linear PBFT (collector) | O(n) | O(n²) | — |

Dolev-Reischuk 1985 quadratic lower bound on Byzantine broadcast communication (reducible to consensus). Cogsworth: optimistically linear view synchronization (nodes send to leader, leader relays one aggregate broadcast), constant latency under benign failures; quadratic baseline otherwise. HotStuff's QC via threshold signatures reduces per-phase authenticators from O(n²) to O(n); without threshold signatures, PBFT view change is O(n³) authenticators and O(n⁴) over O(n) cascading view changes.

`ids-bft-consensus` provides message-complexity analysis modules per engine and a complexity budget tracker so operators can select engine by deployment size and fault model.

#### 3.7 View Change and View Synchronization

**Source:** PBFT view-change section (new-view proof, view-change messages, checkpoint-based state transfer); HotStuff (view change = normal path, leader collects 2f+1 new-view messages with QCs, picks highest QC); LibraBFT/DiemBFT (round synchronization, timeouts, TC formation); Cogsworth (arxiv 1909.05204; optimistically linear view sync); Fast-HotStuff (AggQC from n−f η/new-view messages; threshold vs aggregated signatures).

View change is triggered when the leader/primary is suspected faulty or a view times out. Safety-critical: the new leader must learn the highest QC/prepared state across replicas to avoid forking or reordering committed operations.

- PBFT: replicas multicast signed view-change messages including highest prepared certificate; new primary waits 2f+1, broadcasts new-view message with proof set; replicas validate and switch. Bug-prone; O(n³) messages.
- HotStuff: unified path — new-view message carries highest QC; leader extends it; O(n) authenticators. SafeNode predicate preserves safety across views.
- LibraBFT/DiemBFT: round timeout → replica multicasts timeout share with highest QC; 2f+1 timeouts form Timeout Certificate (TC); TC + QCs advance round; quadratic view-sync fallback for slow catch-up.
- Cogsworth: replicas send sync messages to desired leader; leader aggregates into single broadcast (linear, optimistic); fallback quadratic.

`ids-bft-consensus` implements view-change state machine per engine: trigger conditions, view-change message format, new-view/TC aggregation, highest-QC selection, state transfer hook (delegated to ids-replication/ids-store), and view-synchronization strategy (linear optimistic vs quadratic fallback, configurable).

#### 3.8 Safety Under Byzantine Faults

**Source:** Princeton COS418/518 (Byzantine quorums, quorum certificates); MIT 6.824 linearizability/safety notes; PBFT prepared/committed predicates; "Bottlenecks in Blockchain Consensus" (quorum overlap ≥ f+1 correct); ACM Computing Surveys (quorum intersection ensures consistency).

Safety = all non-faulty replicas decide the same sequence of operations (agreement) and only valid operations are decided (validity). Mechanisms:

- **Quorum intersection**: Byzantine quorum = 2f+1 of n=3f+1; any two quorums overlap in ≥ f+1 replicas, ≥ 1 of which is correct. A correct replica signs at most one value per (view, seq), so two conflicting values cannot both reach quorum in the same view.
- **Prepare/commit locking**: PBFT prepared/committed-local predicates ensure a request executing at (view, seq) cannot be replaced by a different request at the same (view, seq) at any correct replica; commit-phase ordering extends across views.
- **QC extension rule (HotStuff)**: replicas vote for new proposal only if it extends their locked QC or has a higher view — a higher-view QC proves the previous value lost quorum, so unlocking is safe.
- **Two/three-chain commit**: commit only when 2 (or 3) consecutive certified blocks exist — forks cannot both satisfy the chain rule because QCs from different views intersect in a correct replica that would not sign conflicting chain extensions.
- **Client stabilization (Zyzzyva)**: client waits for f+1 matching replies + history; divergence detectable and correctable via view change.
- **Equivocation detection**: signed messages make double-signing provable; slashing/audit hook delegated to ids-fault/ids-audit.

`ids-bft-consensus` encodes safety predicates as pure functions over signed messages/QCs: `is_valid_vote`, `quorum_reached`, `conflicts_with_locked_qc`, `chain_satisfied`, `equivocation_detected`. All return decisions, never execute.

#### 3.9 Liveness Under Partial Synchrony

**Source:** Dwork, Lynch, Stockmeyer "Consensus in the Presence of Partial Synchrony" (JACM 1988; GST model); FLP impossibility (Fischer, Lynch, Paterson 1985; asynchronous deterministic consensus impossible with one fault); HotStuff optimistic responsiveness; Lessons from HotStuff (Aguilera-Toueg Ω(nΔ) worst-case latency bound).

Partial synchrony: message delay bound Δ and processor speed bound exist but may be unknown until an unknown Global Stabilization Time (GST). Safety holds always (even during asynchrony); liveness (termination) holds only after GST. This is how BFT protocols circumvent FLP: they sacrifice liveness during asynchrony, not safety.

- **FLP**: no deterministic protocol guarantees agreement, validity, and termination in a fully asynchronous system with even one crash fault. Randomization or failure detectors/oracles can circumvent, but IDS adopts the deterministic partial-synchrony model (safety always, liveness after GST).
- **GST model**: protocol uses timeouts with exponential backoff; before GST, timeouts may fire spuriously (safe: view changes without progress); after GST, timeouts bound view duration and a correct leader eventually drives a decision.
- **Optimistic responsiveness** (HotStuff): after GST, a correct leader waits only for first n−f responses (not for timeout Δ), so decision latency tracks actual delay δ, not worst-case Δ.
- **Worst-case latency**: Ω(nΔ) mandated by Aguilera-Toueg bound; cascading leader failures can cost O(f·n) rounds (HotStuff) or O(n²) communication.

`ids-bft-consensus` liveness model: view timers with exponential backoff, GST-agnostic operation (no wall-clock trust; timers local), leader rotation to avoid stuck views, nil-block/TC votes to progress despite faulty leaders (LibraBFT), and explicit documentation that liveness is conditional on partial synchrony — never guaranteed under asynchrony.

#### 3.10 Allowed Crypto Only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** RFC 8032 (Ed25519/EdDSA); RFC 7539/RFC 8439 (ChaCha20-Poly1305, XChaCha20-Poly1305); NIST FIPS 180-4 (SHA-256); BLAKE3 specification (BLAKE3-team); ed25519-dalek / chacha20poly1305 / sha2 / blake3 Rust crate docs.

- **ed25519**: signing consensus votes, QCs (threshold/aggregate shares), view-change/new-view messages, checkpoints; audit-chain signatures on decisions. 64-byte signatures, 32-byte public keys.
- **sha256**: block/header/proposal hashing, QC binding (`qc.digest = sha256(block)`), Merkle/digest construction for state digests, checkpoint chaining.
- **blake3**: content addressing of consensus messages, fast QC/digest hashing (SIMD, parallel), keyed hash/KDF/XOF where needed; preferred over sha256 for high-throughput digest work.
- **xchacha20poly1305**: AEAD encryption of consensus state at rest (locked QC, view history, view-change certificates); 24-byte nonce safe for random generation; bounded-use replay windows.
- **chacha20poly1305**: AEAD encryption of session/state payloads; 12-byte nonce (counter-based, bounded use).

No other algorithms. `ids-bft-consensus` declares no crypto implementation; it references primitive IDs from `ids-000-aa` and calls `ids-crypto` for operations. A compile-time allowlist (`crypto_allowlist.rs`) asserts only the five primitives are reachable.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Conflicting proposals at same (view, seq) | Safety/agreement | Quorum intersection (2f+1 quorums overlap ≥ f+1, ≥1 correct); correct replica signs one value per (view,seq) |
| Equivocation (double-signing) by Byzantine replica | Safety | Signed messages make equivocation provable; audit hook to ids-fault; slashing decision delegated |
| Forking across views (different values committed in different views) | Safety | Prepare/commit locking; HotStuff safeNode unlock rule; chain commit rule requires consecutive certified blocks |
| Primary/leader denial-of-service | Liveness | View change/view synchronization on timeout; leader rotation (view mod n); nil-block votes (LibraBFT) |
| Slow/crashed replicas stalling quorum | Liveness | Quorum = n−f = 2f+1 (progress with f faults); SBFT c-parameterized fast path; view change after timeout |
| Asynchronous network (no Δ bound) | Liveness | FLP-consistent: safety preserved, liveness explicitly conditional on partial synchrony (after GST); documented, not promised |
| Clock manipulation for timeouts | Liveness/safety | Local timers only, exponential backoff; no wall-clock trust; IdsTime monotonic for timestamps only |
| Replay of old votes/QCs/view-change msgs | Safety | View numbers bind messages; stale-view messages rejected; bounded replay cache per view |
| Carrier duplicating/delaying/reordering datagrams | Safety/liveness | Idempotent message handling; sequence/view binding; carrier-agnostic: opaque Endpoint, no network in trust base |
| Unauthorized crypto algorithm | Policy | Compile-time allowlist: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only; CI dependency audit |
| Key extraction (consensus signing key) | Decision authenticity | Keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |

The carrier is never trusted: consensus decisions and votes are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-bft-consensus |
|-------|------|-------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Signature`, `KeyId`, `AuthToken`, `Credential`, `Error` — consumed, never redefined; sealed `ConsensusEngine` contract mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All consensus traffic flows to/from `Endpoint`; crate returns decisions/datagrams, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing votes/QCs/view-change msgs; hashing blocks/QCs; encrypting state at rest |
| `ids-store` | Primary record store (consensus user) | Submits commands for ordering; applies decided blocks; consensus engine never touches storage directly |
| `ids-replication` | Replica synchronization / state transfer | View-change state transfer hook; checkpoint state exchange via `ids-carrier::Endpoint` |
| `ids-fault` | Fault detection / equivocation handling | Equivocation evidence routing; slashing/audit integration |
| `ids-session` (Phase 050) | Client-session binding | Client requests carry session context; replies matched to pending requests |
| `ids-identity` (Phase 004) | Stable/rotating node IDs | Validator public keys and `NodeId` binding for vote attribution |

**Integration points:** `ids-bft-consensus` drives public contracts declared in `ids-000-aa`; it does not reach into crate internals. All cross-replica traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond what contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-bft-consensus/` (a separate crate, not a folder inside an existing crate):

```
ids-bft-consensus/
├── Cargo.toml                          # ed25519-dalek, chacha20poly1305, sha2, blake3 (via ids-crypto ids), serde, thiserror; workspace member
├── lib.rs                              # crate root, feature gates, public re-exports
├── constants.rs                        # n=3f+1, quorum=2f+1, view timer defaults, MAX_VIEW_CHANGE_CASCADE
├── error.rs                            # ConsensusError (thiserror): safety, liveness, quorum, view, crypto errors
├── features.rs                         # Feature flags: pbft, hotstuff, libra-bft, zyzzyva, sbft, threshold-sig, async-fallback
├── types.rs                            # ViewId, SeqNo, BlockId, QcId, ConsensusDecision, Vote, QuorumCertificate, TimeoutCertificate (re-exports ids-000-aa types)
│
├── engines/                            # Protocol-family engines (~10 files)
│   ├── mod.rs
│   ├── engine_contract.rs              # sealed ConsensusEngine trait contract
│   ├── pbft.rs                         # PBFT engine: pre-prepare/prepare/commit/view-change/checkpoint
│   ├── hotstuff.rs                     # HotStuff engine: prepare/pre-commit/commit/QC/safeNode/3-chain
│   ├── libra_bft.rs                    # LibraBFT/DiemBFT engine: rounds, nil-block, round sync, two-chain
│   ├── zyzzyva.rs                      # Zyzzyva engine: speculative agreement, client stabilization, checkpoint
│   ├── sbft.rs                         # SBFT engine: collector fast path, threshold sig, fallback
│   └── engine_selector.rs              # Selects engine by deployment size/fault model
│
├── quorum/                             # Quorum certificates & voting (~8 files)
│   ├── mod.rs
│   ├── qc.rs                           # QuorumCertificate: threshold/aggregate signature over (type, view, block)
│   ├── tc.rs                           # TimeoutCertificate (LibraBFT/DiemBFT view sync)
│   ├── vote.rs                         # Vote message: signed partial signature share
│   ├── quorum_calc.rs                  # n=3f+1, quorum=2f+1, overlap=f+1 correct minimum
│   ├── threshold_sig.rs                # Threshold-signature interface (ids-crypto primitive ID)
│   ├── aggregate_sig.rs                # Aggregate-signature interface (distinct messages)
│   └── quorum_intersection.rs          # Safety proof: any two quorums overlap ≥ 1 correct replica
│
├── view/                               # View management & view change (~8 files)
│   ├── mod.rs
│   ├── view_state.rs                   # Current view, leader = view mod n, locked QC
│   ├── leader_rotation.rs              # view mod n mapping; rotation schedule
│   ├── view_change.rs                  # Trigger, view-change message, new-view proof
│   ├── view_sync.rs                    # View synchronization: linear optimistic (Cogsworth) vs quadratic fallback
│   ├── timeout_mgr.rs                  # View timers with exponential backoff (GST-agnostic)
│   ├── new_view.rs                     # New-view message aggregation from 2f+1 replicas
│   └── state_transfer.rs               # State transfer hook (delegated to ids-replication/ids-store)
│
├── safety/                             # Safety predicates (~6 files)
│   ├── mod.rs
│   ├── prepared_committed.rs           # PBFT prepared/committed-local predicates
│   ├── safe_node.rs                    # HotStuff safeNode voting rule
│   ├── chain_commit.rs                 # 2-chain / 3-chain commit rule
│   ├── quorum_overlap.rs               # Quorum intersection ≥ f+1 correct
│   └── equivocation.rs                 # Double-signing detection from signed messages
│
├── liveness/                           # Liveness under partial synchrony (~6 files)
│   ├── mod.rs
│   ├── partial_sync.rs                 # GST model: safety always, liveness after GST
│   ├── responsiveness.rs               # Optimistic responsiveness: wait n−f, not Δ
│   ├── timer_backoff.rs                # Exponential backoff view timers
│   ├── leader_rotation_liveness.rs     # Rotating leaders avoid stuck views
│   ├── nil_block.rs                    # Nil-block vote: progress despite faulty leader
│   └── flp_boundary.rs                 # FLP boundary documentation: no async liveness guarantee
│
├── complexity/                         # Message-complexity analysis (~6 files)
│   ├── mod.rs
│   ├── pbft_complexity.rs              # O(n²) normal, O(n³) view change, O(n⁴) cascade
│   ├── hotstuff_complexity.rs          # O(n) per phase/view, O(n²) worst case
│   ├── libra_bft_complexity.rs         # O(n) happy path, O(n²) view-sync fallback
│   ├── zyzzyva_complexity.rs           # O(n²) optimistic, avoids all-to-all commit
│   ├── sbft_complexity.rs              # O(cn) fast path, O(n²) fallback
│   └── complexity_budget.rs            # Per-engine complexity budget tracking
│
├── crypto/                             # Crypto surface & allowlist (~6 files)
│   ├── mod.rs
│   ├── ed25519.rs                      # Vote/QC/view-change signing (ed25519)
│   ├── hashing.rs                      # sha256/blake3 block & QC binding
│   ├── aead.rs                         # xchacha20poly1305/chacha20poly1305 state-at-rest encryption
│   ├── allowlist.rs                    # Compile-time assert: only five allowed primitives
│   └── nonce_mgr.rs                    # Bounded nonce management for AEAD
│
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── consensus_engine.rs             # trait ConsensusEngine: propose/vote/commit/decide/view_change
│   ├── quorum_contract.rs              # trait Quorum: qc formation/verification
│   ├── view_contract.rs                # trait View: view change/sync
│   ├── safety_contract.rs              # trait Safety: prepared/committed/chain predicates
│   ├── liveness_contract.rs            # trait Liveness: partial-synchrony boundary
│   ├── carrier_contract.rs             # Opaque Vec<u8> datagram framing to ids-carrier::Endpoint
│   └── crypto_contract.rs              # trait CryptoSurface: allowed primitives only
│
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── quorum_size.rs                  # n >= 3f + 1; quorum == 2f + 1
│   ├── view_monotonic.rs               # View numbers strictly increasing
│   ├── qc_uniqueness.rs                # One QC per (view, block)
│   ├── chain_linear.rs                 # Chain commit linearization
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # Only five allowed crypto primitives
│   ├── gst_boundary.rs                 # Liveness conditional on partial synchrony
│   └── mod.rs
│
├── decisions/                          # Decision types (~6 files)
│   ├── mod.rs
│   ├── propose_decision.rs
│   ├── vote_decision.rs
│   ├── commit_decision.rs
│   ├── decide_decision.rs
│   ├── view_change_decision.rs
│   └── decision_log.rs                 # Append-only decision log (hash-chained + ed25519 signed)
│
├── tests/                              # Property/verification tests (~30 files)
├── benches/                            # (~10 files)
├── docs/                               # (~14 files)
├── spec/                               # (~6 files)
└── test_vectors/                       # (~4 files)
```

Total: ~90–1000 files across `src/`, `src/engines/`, `src/quorum/`, `src/view/`, `src/safety/`, `src/liveness/`, `src/complexity/`, `src/crypto/`, `src/contracts/`, `src/invariants/`, `src/decisions/`, `tests/`, `benches/`, `docs/`, `spec/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **ViewId / SeqNo / BlockId / QcId**: `ViewId` = u64 monotonic view number; `SeqNo` = u64 sequence number within view; `BlockId` = blake3-256 of block contents; `QcId` = sha256 of `(type, view, block)` bound to threshold signature.
- **QuorumCertificate**: `QC{ qc_type, view, block_id, signature, signer_bitmap }` — threshold/aggregate signature over `(type, view, block)` from quorum of 2f+1 replicas. Valid iff `tverify(type, view, block, sig)` true.
- **TimeoutCertificate**: `TC{ view, highest_qc, signature }` — formed from 2f+1 timeout shares (LibraBFT/DiemBFT view sync).
- **Vote**: `Vote{ replica_id, view, block_id, phase, partial_sig }` — replica sends partial signature share to leader; leader aggregates into QC.
- **n = 3f + 1, quorum = 2f + 1, overlap ≥ f + 1 correct**: `admit_quorum(n, f)` requires `n >= 3f + 1`; `quorum_size(n) = n - f = 2f + 1`; `min_overlap(q1, q2, n) = 2*(2f+1) - (3f+1) = f + 1`, of which ≥ 1 is correct.
- **PBFT phases**: `PRE-PREPARE → PREPARE → COMMIT`; `prepared(v, n, i)` iff pre-prepared + 2f+1 matching PREPARE; `committed-local(v, n)` iff prepared + 2f+1 matching COMMIT; apply + reply after committed-local; client accepts after f+1 identical replies.
- **HotStuff safeNode**: replica votes for proposal P in view v iff `P.view > locked_view` or `P extends locked_QC`; leader picks highest QC from new-view messages.
- **Chain commit**: 2-chain — block committed if it and its parent are certified (two adjacent certified blocks); 3-chain — block committed if first of three adjacent certified blocks with consecutive views (configurable per engine).
- **View change**: trigger on view timeout; replicas multicast signed view-change + highest QC; new leader waits 2f+1, selects highest QC, broadcasts new-view proof; replicas validate and switch (HotStuff unified path: O(n); PBFT: O(n³)).
- **View synchronization**: linear optimistic — replicas send sync to leader, leader relays one aggregate broadcast (Cogsworth); quadratic fallback otherwise.
- **Speculative execution (Zyzzyva)**: replicas execute optimistically, reply to client; client stabilizes on f+1 matching replies + history; view change on instability.
- **Collector linear PBFT (SBFT)**: replicas send votes to collector; collector aggregates into one broadcast; threshold signature collapses to constant size; O(cn) fast path, O(n²) fallback.
- **Audit log**: append-only decision log, sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable.

---

### 8. Dependencies

`ids-bft-consensus/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Signature`, `KeyId`, `AuthToken`, `Credential`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of consensus envelopes, QCs, votes, audit entries.
- `thiserror` — error types.

Feature-gated engine selection: `pbft`, `hotstuff`, `libra-bft`, `zyzzyva`, `sbft`, `threshold-sig`, `async-fallback`.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-bft-consensus` imports no crypto directly — all via `ids-crypto`. No async runtime (no `tokio`).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-72.1 | PBFT engine: pre-prepare/prepare/commit phases, prepared/committed-local predicates, client accepts after f+1 identical replies | Unit tests on safety predicates with n=3f+1 |
| AC-72.2 | HotStuff engine: prepare/pre-commit/commit phases, QC via threshold signatures, safeNode rule, 3-chain commit | Unit tests on chain commit and view-change unification |
| AC-72.3 | LibraBFT engine: round-based proposal/vote/QC chain, nil-block vote, round synchronization | Unit tests on round advance and TC formation |
| AC-72.4 | Zyzzyva engine: speculative agreement, client stabilization (f+1 matching replies), checkpoint sub-protocol | Unit tests on fast/slow path selection |
| AC-72.5 | SBFT engine: collector fast path O(cn), threshold-signature aggregation, O(n²) fallback | Complexity tests on message counts |
| AC-72.6 | Quorum: n >= 3f + 1 enforced; quorum == 2f + 1; any two quorums overlap ≥ f+1, ≥ 1 correct | Quorum intersection test (100% overlap correctness) |
| AC-72.7 | View change: new leader selects highest QC; safeNode prevents conflicting proposals across views | Cross-view safety test (zero conflicting commits) |
| AC-72.8 | View synchronization: linear optimistic (Cogsworth-style) vs quadratic fallback, configurable | Message-count test (linear vs quadratic) |
| AC-72.9 | Liveness: safety always; liveness only after GST (partial synchrony); FLP boundary documented | Liveness model test; no async termination guarantee claimed |
| AC-72.10 | Message complexity: PBFT O(n²)/O(n³)/O(n⁴); HotStuff O(n) per phase/view; SBFT O(cn) fast path | Complexity budget tests per engine |
| AC-72.11 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep — zero matches |
| AC-72.12 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-72.13 | Every consensus decision recorded in append-only, hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime` | Audit log verification test (offline verifiable) |
| AC-72.14 | Crate compiles cleanly | `cargo check -p ids-bft-consensus` exits 0 with no warnings |
| AC-72.15 | Tests pass | `cargo test -p ids-bft-consensus` passes all unit/property tests |

---

### 10. Remaining Uncertainties

1. **Engine defaults per deployment:** concrete `(n, f)` and engine selection (PBFT vs HotStuff vs LibraBFT vs Zyzzyva vs SBFT) are deployment-tuned; to be validated at integration with `ids-store`/`ids-replication`.
2. **Threshold vs aggregate signatures:** threshold signatures (compact QC, unknown signer set) vs aggregate signatures (distinct messages, known signers, expensive verify) — HotStuff uses threshold; Fast-HotStuff/DiemBFT view-sync uses aggregate; selection per engine/path pending.
3. **Two-chain vs three-chain commit:** LibraBFT original uses 3-chain; DiemBFT V4 uses 2-chain; configurable in plan, default TBD at integration.
4. **View timer backoff parameters:** initial timeout and exponential backoff base affect liveness recovery after GST; workload/latency-dependent.
5. **State transfer during view change:** checkpoint/state exchange delegated to `ids-replication`/`ids-store`; exact hook signature TBD at integration.
6. **Nil-block vote adoption:** allows commits despite faulty leaders (LibraBFT); adds complexity; adoption decision pending.
7. **SBFT collector trust:** single slow/failed collector forces fallback; c-parameter (≤ f/8 heuristic) resilience tradeoff unquantified for IDS topology.
8. **Audit log performance:** hash-chained + signed checkpoints on every decision may add latency; batching strategy TBD.
9. **Async fallback:** DiemBFT async view-sync helps under adversary but gives no liveness guarantee (FLP); whether to include `async-fallback` feature TBD.
10. **QC size encoding:** threshold signature (constant size) vs appended 2f+1 partial signatures (linear size); affects wire frame size vs `Endpoint` 1024 B cap.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate `ids-bft-consensus` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear protocol boundary |
| Isolated engine submodules (PBFT/HotStuff/LibraBFT/Zyzzyva/SBFT) | Each family has distinct safety/liveness/complexity tradeoffs; feature-gated selection |
| n = 3f + 1, quorum = 2f + 1, overlap ≥ f+1 correct | Optimal Byzantine resilience; standard across PBFT/HotStuff/Zyzzyva |
| Quorum certificates via threshold signatures | Linear O(n) communication per phase/view (HotStuff innovation); reduces view-change cost |
| HotStuff safeNode unlock rule | Enables seamless leader replacement without synchronous delays; safety across views |
| View change = unified protocol path (HotStuff-style) | Avoids PBFT's bug-prone O(n³) view change; O(n) authenticators |
| Optimistic responsiveness (wait n−f, not Δ) | Decision latency tracks actual network delay after GST |
| Liveness conditional on partial synchrony (after GST); FLP boundary documented | Deterministic async consensus impossible (FLP); safety always, liveness after GST |
| Client stabilization (Zyzzyva-style f+1 matching replies) for speculative paths | Moves output commit to client; avoids server all-to-all commit in optimistic case |
| All traffic via `ids-carrier::Endpoint`; crate returns decisions/datagrams, never sends | Carrier-agnostic absolute rule; no network stack in trust base |
| Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS crypto policy; consumed via `ids-crypto` primitive IDs |
| Hash-chained + ed25519-signed audit log | Offline-verifiable decision log; tamper evidence |
| Local timers + exponential backoff; no wall-clock trust | GST-agnostic; clock manipulation cannot break safety/liveness assumptions |

---

### 12. Sources

1. Castro, Liskov — "Practical Byzantine Fault Tolerance" (OSDI 1999). https://css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf — three-phase pre-prepare/prepare/commit, view change, 3f+1 replicas, prepared/committed predicates.
2. Yin, Malkhi, Reiter, Gueta, Abraham — "HotStuff: BFT Consensus in the Lens of Blockchain" (arXiv:1803.05069, PODC 2019). https://arxiv.org/html/1803.05069v4 — linear view change, optimistic responsiveness, QC, safeNode, three-phase core, chained variant.
3. Malkhi, Yin et al. — "Lessons from HotStuff" (arXiv:2305.13556). https://arxiv.org/pdf/2305.13556 — HotStuff-2 two-phase responsive BFT, desirable properties F-1..F-4, responsiveness survey.
4. Diem/Libra consortium — "State Machine Replication in the Libra Blockchain" (2019-06-28). https://developers.diem.com/papers/diem-consensus-state-machine-replication-in-the-diem-blockchain/2019-06-28.pdf — LibraBFT rounds, rotating leader, QC chain, round synchronization, nil-block vote.
5. Diem consortium — "Diem Consensus" (2020-04-09). https://developers.diem.com/papers/diem-consensus-state-machine-replication-in-the-diem-blockchain/2020-04-09.pdf — LibraBFT enhancements, two-chain commit rule (V4), view-synchronization fallback.
6. Kotla, Alvisi, Dahlin, Clement, Wong — "Zyzzyva: Speculative Byzantine Fault Tolerance" (SOSP 2007). https://doi.org/10.1145/1400214.1400236 — speculative execution, client stabilization, agreement/view-change/checkpoint sub-protocols.
7. SBFT — "SBFT: A Scalable and Decentralized Trust Infrastructure" (IEEE). https://ieeexplore.ieee.org/document/8809541 — collector-based linear PBFT, threshold signatures, O(cn) fast path, O(n²) fallback, n = 3f+2c+1.
8. "Bottlenecks in Blockchain Consensus Protocols" (arXiv:2103.04234). https://arxiv.org/html/2103.04234v4 — PBFT quadratic/O(n³) view change/O(n⁴) cascade; HotStuff linear view change; IBFT/LinearIBFT comparison.
9. Cogsworth — "Be Prepared When Network Goes Bad: An Asynchronous View-Change Protocol" (arXiv:1909.05204). https://arxiv.org/pdf/1909.05204 — optimistically linear view synchronization, quadratic baseline, Dolev-Reischuk lower bound.
10. Dwork, Lynch, Stockmeyer — "Consensus in the Presence of Partial Synchrony" (JACM 1988). https://groups.csail.mit.edu/tds/papers/Lynch/jacm88.pdf — GST model, safety always/liveness after synchrony, partial synchrony definition.
11. Fischer, Lynch, Paterson — "Impossibility of Distributed Consensus with One Faulty Process" (FLP 1985). https://groups.csail.mit.edu/tds/papers/Lynch/jacm85.pdf — deterministic asynchronous consensus impossible with one fault.
12. "Half a Century of Distributed Byzantine Fault-Tolerant Consensus" (arXiv:2407.19863). https://arxiv.org/html/2407.19863v3 — protocol evolution, FLP, synchrony models, randomized consensus.
13. ACM Computing Surveys — "Reaching Consensus in the Byzantine Empire: A Comprehensive Review of BFT Consensus Algorithms". https://dl.acm.org/doi/10.1145/3636553 — protocol comparison table (HotStuff/LibraBFT O(‖M‖n), SBFT linear-PBFT), PBFT O(n²)/O(n³).
14. Princeton COS418/518 — strong consistency, Byzantine quorums, quorum certificates. https://cs.princeton.edu/courses/archive/fall16/cos418/docs/L13-strong-cap.pdf
15. MIT 6.824 — linearizability, PBFT notes. https://pdos.csail.mit.edu/6.824/notes/l-linearizability.txt
16. Fast-HotStuff — "Fast-HotStuff: A Fast and Robust BFT Protocol for Blockchains" (arXiv:2010.11454). https://arxiv.org/html/2010.11454v10 — aggregated vs threshold signatures, AggQC, two-chain commit.
17. RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA), ed25519. https://www.rfc-editor.org/rfc/rfc8032
18. RFC 7539 / RFC 8439 — ChaCha20-Poly1305 / XChaCha20-Poly1305 AEAD. https://www.rfc-editor.org/rfc/rfc7539
19. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
20. BLAKE3 — BLAKE3-team specification. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
21. ed25519-dalek docs.rs — ed25519 key gen/sign/verify. https://docs.rs/ed25519-dalek
22. chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD. https://docs.rs/chacha20poly1305
23. sha2 RustCrypto — SHA-256. https://docs.rs/sha2
24. IDS prompts.txt entry 72 (verbatim). C:/idfs/prompts.txt — Phase 072 crate definition, contents, constraints.
25. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml.
26. HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
27. IDS plan references: C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — IDS plan format precedents.
28. IDS carrier-agnostic rule: no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh; all traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>). C:/idfs/prompts.txt entries 1, 72.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, protocol families, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 072 (ids-bft-consensus).
