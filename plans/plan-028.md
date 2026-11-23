## Phase 028 — Create the ids-byzantine crate (BFT consensus and authenticated broadcast for malicious nodes): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 028 creates `ids-byzantine`, the IDS crate for Byzantine fault tolerance (BFT) consensus and authenticated broadcast under a malicious-node threat model. It implements PBFT (three-phase pre-prepare/prepare/commit, 3f+1 replicas, view changes), HotStuff (view-by-view recipe, optimistic responsiveness, linear view change via quorum certificates), Flexible BFT (flexible quorums, optimistic responsiveness without synchrony for protocol execution), Byzantine broadcast via Dolev-Strong (signature chains, t+1 rounds, tolerates any t < n with PKI), authenticated broadcast, Sybil resistance, eclipse resistance, malicious routing/discovery behavior, replay-attack defense, traffic-manipulation resistance, resource-exhaustion and DoS resistance, and Byzantine node isolation/quarantine.

Crate: `ids-byzantine` (CRATES.md entry 28). Purpose: BFT consensus and authenticated broadcast for malicious nodes. Contents per prompts.txt entry 28: PBFT, HotStuff, Flexible BFT, Byzantine broadcast (Dolev-Strong), authenticated broadcast, Sybil resistance, eclipse resistance, malicious routing/discovery behavior, replay attacks, traffic manipulation, resource exhaustion, DoS resistance, Byzantine node isolation/quarantine.

Verbatim prompt (C:/idfs/prompts.txt lines 415-429): "Phase 028 — Create the ids-byzantine crate: BFT consensus and authenticated broadcast for malicious nodes."

Research basis: 30 parallel searches returned PBFT (Castro & Liskov SOSP 1999; pre-prepare/prepare/commit; 3f+1; view change; f Byzantine), HotStuff (view-by-view recipe; optimistic responsiveness; linear view change; quorum certificates), Flexible BFT (Flexible Byzantine Quorums; synchronous protocol where only commit needs delay bound; clients assume different fault/timing models), Dolev-Strong authenticated broadcast (signature chains; t+1 rounds; t < n with PKI; O(n²t) words; f+1 round lower bound), authenticated broadcast / Bracha reliable broadcast (INIT/ECHO/READY; validity/agreement/integrity; f < n/3), Sybil resistance (identity binding; cost per identity; proof of work / VDF conditions), eclipse resistance (diverse peer selection; multiple interfaces; trusted seed nodes; peer scoring), malicious routing/discovery behavior (poisoned discovery tables; DNS crawler manipulation; connection-slot hijacking), replay attacks (sequence numbers; nonces; monotonic counters; timestamp windows; nullifiers), traffic manipulation (message delay/reorder/drop; leader-targeted DoS; view-change amplification), resource exhaustion (mempool explosions; inflation attacks; block synchronizer; pull-induction), DoS resistance (leader-targeted DoS within seconds on few Mbps; rate limiting; per-peer quotas; batch verification; verification cache), Byzantine node isolation/quarantine (detection vs. masking; PEACHes/DecentBFT; Byzantine Fault Isolation; expose suspected/exposed/trusted indications; exclusion as last resort).

A skeleton `ids-byzantine` crate does NOT yet exist at `C:/idfs/ids-byzantine/`. `ids-000-aa` already declares `trait Byzantine` (BFT consensus, authenticated broadcast) at `src/contracts/byzantine.rs`; `ids-byzantine` implements it. Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-byzantine` consumes type-level surface and primitive IDs only). `ids-000-aa` is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-byzantine` sits above `ids-000-aa` (types + sealed `trait Byzantine` contract + invariants + versioning + errors) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It is layered so each Byzantine primitive is an independent module that can be composed, all carrier-agnostic:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / SMR LAYER (ids-* crates)            │
│  state machine replication · ordered command application · commit   │
├─────────────────────────────────────────────────────────────────────┤
│              Phase 028: ids-byzantine (THIS CRATE)                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │   PBFT      │ │  HotStuff   │ │ Flexible    │ │ Dolev-Strong│   │
│  │ consensus   │ │ consensus   │ │ BFT         │ │ broadcast   │   │
│  │ (3f+1,      │ │ (QC, view-  │ │ (flexible   │ │ (signature  │   │
│  │  pre-prepare│ │  by-view,   │ │  quorums,   │ │  chains,    │   │
│  │  prepare,   │ │  linear VC) │ │  opt. resp.)│ │  t+1 rounds)│   │
│  │  commit)    │ └─────────────┴───────────────┴─────────────┘   │
│  └─────────────┘                                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ authenticated│ │ Sybil &     │ │ Replay &    │ │ DoS &       │   │
│  │ broadcast   │ │ eclipse     │ │ traffic     │ │ resource    │   │
│  │ (Bracha)    │ │ resistance  │ │ manipulation│ │ exhaustion  │   │
│  └─────────────┴───────────────┴───────────────┴─────────────┘   │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ isolation/quarantine · detection evidence · exposure · exclude │  │
│  └───────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (ids-carrier::Endpoint only)           │
│  send_datagram(Endpoint{CarrierType, Vec<u8>}, &[u8])               │
│  receive_datagram → (Endpoint, Vec<u8>)                             │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└─────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `ViewId`, `SeqNo`, `QuorumCert`, `Endpoint`, `CarrierType`, `FaultDomain`, `QuarantineState`, `ConsistencyProfile`) imported from `ids_000_aa::X`, never redefined.
- `ids-byzantine` implements the sealed `trait Byzantine` from `ids-000-aa/src/contracts/byzantine.rs`; downstream crates use it as a trait bound, cannot implement it.
- All message transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16) + opaque `Vec<u8>`). No IP addresses, ports, sockets.
- Crypto surface: ed25519 signatures (authentication / Dolev-Strong chains / PBFT message auth), sha256/blake3 (message digests, QC hashes, view-change proofs), xchacha20poly1305/chacha20poly1305 (optional payload encryption). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Fault model: up to f Byzantine among n = 3f+1 replicas for PBFT/HotStuff; Dolev-Strong authenticated broadcast tolerates any t < n with PKI.
- Ordering is dataset-scoped via `ids-ordering-domain` (crate 55); no global master (invariant `no_global_master` in `ids-000-aa`).
- Operation lifecycle reuses `ids-000-aa` `OpState` and `CommitState`; Byzantine faults tracked in `FaultDomain`.

---

### 3. Detailed Research Dimensions

#### 3.1 PBFT — Practical Byzantine Fault Tolerance (Castro & Liskov, SOSP 1999)

PBFT is a state-machine-replication protocol tolerating up to f Byzantine replicas among n = 3f+1, safe at all times and live during periods of synchrony. Normal operation is a three-phase protocol:
- **Pre-prepare**: primary assigns a sequence number n to a client request in view v, multicasts `PRE-PREPARE(v, n, m, σ_primary)` to backups.
- **Prepare**: backups validate (view matches, sequence number in range, signature valid, no conflicting pre-prepare accepted) and multicast `PREPARE(v, n, digest(m), σ_i)`. A replica is *prepared* when it has a pre-prepare + 2f matching prepares.
- **Commit**: on becoming prepared, a replica multicasts `COMMIT(v, n, digest(m), σ_i)`. A replica is *committed* when prepared + 2f+1 matching commits; it then executes and replies.
- **View change**: when the primary is suspected faulty, replicas multicast view-change messages; a new view becomes active when 2f+1 view-change messages for the new view are collected; the new primary must prove it knows all committed requests and re-propose any gaps.

Key invariants: `prepared(m,v,n,i)` and `prepared(m',v,n,j)` with m≠m′ cannot both hold for honest replicas (quorum intersection guarantees at least one honest replica in both 2f+1 sets). Client waits for f+1 matching replies from different replicas.

Sources: Castro & Liskov PBFT paper (css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf); PBFT lecture notes (people.eecs.berkeley.edu; web.eecs.umich.edu); PBFT review (cs.utah.edu pbft.pdf).

#### 3.2 HotStuff — Linear View Change, Optimistic Responsiveness

HotStuff is a leader-based BFT consensus for the partial-synchrony model. A stable leader drives consensus at actual network delay (responsiveness) with linear communication complexity. Core concepts:
- **View**: numbered epoch with one leader; leader = replica `v mod n`.
- **Quorum Certificate (QC)**: a quorum of n−f signed votes for a block/proposal in a view; the first phase guarantees proposal uniqueness via QC formation.
- **Three-phase core**: prepare → pre-commit → commit (two-phase in happy path via pipelining); a new leader picks the highest QC it knows of, simplifying leader replacement.
- **View change**: linear communication — the new leader collects QCs from replicas and includes a proof in its proposal; no all-to-all view-change broadcast like PBFT.
- **Pipelining / chaining**: proposals reference prior QCs, enabling back-to-back decisions and leader rotation every round without extra delay.
- **Optimistic responsiveness**: progress at network speed δ rather than waiting for max delay Δ when the leader is honest.

HotStuff-2 (arxiv eprint 2023/397) achieves optimal two-phase responsive BFT with O(n²) worst-case and optimistically linear communication. Jolteon/Ditto (arxiv 2106.10362) replace HotStuff's quadratic pacemaker with an asynchronous fallback for DDoS resilience. MonadBFT (arxiv 2502.20692) is fork-resistant, timeout-free in both happy and unhappy paths.

Sources: HotStuff paper (arxiv 1803.05069); Lessons from HotStuff (arxiv 2305.13556); HotStuff-2 (eprint 2023/397); Jolteon/Ditto (arxiv 2106.10362); MonadBFT (arxiv 2502.20692).

#### 3.3 Flexible BFT — Flexible Byzantine Quorums

Flexible BFT (arxiv 1904.10067) introduces two ideas:
- A synchronous BFT protocol in which only the *commit* step needs to know the network delay bound; replicas execute without synchrony assumptions, so clients can commit at their own pace.
- **Flexible Byzantine Quorums**: deconstructs quorum roles into quorum availability (q_r — replicas responding to leader) and commit quorum (q_c — clients commit with this); only cross-phase quorum intersection is required, enabling small commit quorums and large election quorums (like Flexible Paxos).

Flexible BFT guarantees any two clients with correct (possibly different) assumptions commit the same value; if a client's assumption is wrong it may commit inconsistent values that may later be reverted. Strengthened Fault Tolerance (arxiv 2501.03715) extends this: blocks gain resilience from 1/3 up to 2/3 Byzantine as they gather more confirmations, staying safe even after the optimistic period.

Sources: Flexible BFT (arxiv 1904.10067); Strengthened Fault Tolerance (arxiv 2501.03715); Flexible Paxos (arxiv 1608.06696).

#### 3.4 Byzantine Broadcast (Dolev-Strong) and Authenticated Broadcast

**Byzantine broadcast** properties (for designated sender P_s with input m, tolerating t malicious):
- **Termination**: every honest party outputs a message.
- **Agreement**: all honest parties output the same message.
- **Validity**: if the sender is honest, all honest parties output m.

Impossibility without setup: broadcast achievable iff t < n/3 (Pease-Shostak-Lamport); deterministic protocols need t+1 rounds lower bound. With PKI (authenticated channels / digital signatures), authenticated broadcast is achievable for any t < n.

**Dolev-Strong (1983)**: authenticated broadcast tolerating any t < n with PKI in t+1 rounds, O(n²t) words. Uses signature chains — a message (v, p1, σ1, ..., pi, σi) where each σ is a valid signature by p_{i+1} over the preceding chain. Honest parties accept only chains whose length matches the round number. Properties: termination (t+1 rounds), validity (honest leader signs one value), agreement (signature chains contain an honest signer by round t+1; chains are forwarded so all honest parties see the same values).

**Authenticated broadcast (Srikanth-Toueg / consistent broadcast)**: quadratic messages, two-message latency; sender distributes request, witnesses echo it; correct party c-delivers on (n+t+1)/2 matching echos. Echo broadcast with digital signatures achieves linear messages at higher latency.

**Bracha reliable broadcast** (asynchronous, authenticated channels, f < n/3): three phases — SEND (sender → all), ECHO (on first SEND, echo to all), READY (on (n+f+1)/2 matching ECHOs or f+1 READYs); deliver on 2f+1 READYs. Guarantees BRB-Validity, No Duplication, Integrity, Agreement.

Sources: Dolev-Strong (decentralizedthoughts.github.io/2019-12-22-dolev-strong; UMD lecture 26; cs.umd.edu lecture 26); authenticated broadcast (dcl.epfl.ch sdc_byzconsensus.pdf; disco.ethz.ch chapter 9); Bracha reliable broadcast (arxiv 2104.03673; decentralizedthoughts 2020-09-19); Byzantine broadcast definitions (arxiv 2002.11321; arxiv html 2002.11321).

#### 3.5 Sybil Resistance

Sybil attacks: creating network identities is essentially free, so an adversary floods peer discovery with adversarial identities to eclipse victims or partition the overlay. Defenses require raising the per-identity cost:
- **Identity binding**: bind identity to a verifiable credential (stake, proof of work, VDF output).
- **Freshness**: work must be redone each period (prevents amortized cheap identities).
- **Shared deadline**: all identities must complete work by a common deadline (bounds concurrent active identities by processor count).
- **Sequential, non-batchable work**: prevents parallelizing across many identities.
- A VDF on an identity-bound input meets all four conditions without payments (bilinear Sybil cost; CPU speed advantage 2–8× vs 10³–10⁷× for hash puzzles).
- Stake-backed peer discovery (AetherWeave, arxiv 2603.23793): nodes prove valid stake without revealing which deposit; gossip doesn't expose peer tables; honest overlay stays connected w.h.p. or a (1−δ) fraction of nodes in small components raise attack-detection flags.

IDS application: peer identity = ed25519 public key registered via ids-000-aa; per-peer cost enforced through ids-resource quotas and ids-admission rate limits; no IP-based Sybil scoring (carrier-agnostic — addressing is opaque).

Sources: Bilinear Sybil Cost (arxiv 2611.06268); AetherWeave (arxiv 2603.23793); Eclipse attacks overview (bitcoinops.org); AetherWeave peer discovery definitions.

#### 3.6 Eclipse Resistance

Eclipse attack: adversary monopolizes all of a target node's connections, isolating it from honest peers, feeding a distorted chain view, censoring transactions, or causing stale-fork building. Defenses:
- **Diverse peer selection**: connect across multiple networks/interfaces, subnets, ASNs; limit peers per prefix/ASN (Gossipsub peer score; Catalyst libp2p layer).
- **Trusted seed nodes** for bootstrapping; peer scoring and reputation.
- **Multiple redundant network paths** so loss of one path doesn't isolate.
- **Monitor peer count and churn**: sudden drops or concentration signal eclipse.
- For unreachable nodes (NAT/firewall, no inbound): they depend entirely on outbound peers; once all outbound peers are attacker-controlled, no honest node can rediscover the victim — isolation is self-reinforcing. Defense: diverse outbound peer sets, periodic refresh, avoid trusting a single relay.

IDS application: eclipse resistance at the carrier layer via `ids-carrier::Endpoint` diversity (multiple CarrierType values, multiple endpoints per peer); malicious routing/discovery behavior detected via ids-discovery anomaly scoring; quarantine via ids-byzantine isolation module. No IP/ASN concepts (carrier-agnostic); diversity is expressed as CarrierType/endpoint diversity.

Sources: Eclipse attacks on Ethereum (arxiv 2601.16560); Monero unreachable-node eclipse (arxiv 2609.10260); Bitcoin eclipse attacks (bitcoinops.org); NEAR P2P eclipse advisories; AetherWeave eclipse definition.

#### 3.7 Malicious Routing / Discovery Behavior

Malicious peers can poison discovery tables via unsolicited messages, manipulate DNS-based peerlists, hijack idle connection slots, or advertise false routes. In a carrier-agnostic system there are no DNS/IP routes, but endpoints can be misrepresented:
- **Endpoint spoofing**: a Byzantine peer claims to be another's CarrierType+address; mitigated by ed25519-signed endpoint claims verified against ids-000-aa identity registry.
- **Discovery pollution**: Byzantine peers flood discovery with fake endpoints; mitigated by per-endpoint rate limits (ids-admission), reputation scoring, and bootstrap-authenticated trust anchors.
- **Routing manipulation**: Byzantine intermediate carriers drop/delay/reorder frames; mitigated by authenticated broadcast quorums (agreement despite message loss/reorder) and replay/dedup guards.

IDS application: ids-byzantine assumes untrusted carriers; all authentication is endpoint-level (ed25519 over payload + nonce + sequence), so carrier manipulation is detectable but not safety-breaking.

Sources: Ethereum eclipse via DNS crawler poisoning + connection-slot hijacking (arxiv 2601.16560); AetherWeave malicious peer-table entries (arxiv 2603.23793); GLP networking API (multi-underlay authenticated delivery) (arxiv 2610.32583).

#### 3.8 Replay Attacks

Replay: adversary re-transmits a previously-valid signed message to cause re-execution or state confusion. Defenses:
- **Per-sender monotonic sequence numbers**: receiver keeps highest accepted sequence per sender; rejects ≤ last-seen (strict equality gate for consume-once semantics).
- **Nonces**: 32-byte random per message; single-use enforcement via replay cache; one-nonce-one-presentation.
- **Timestamp windows**: signed timestamp outside a configured window rejected; never trust unsynchronized wall clocks for security decisions (use as secondary check only).
- **Nullifiers / spent-token sets**: mark authorization tokens as spent; bounded ring buffer to limit state growth (EIP-8266 expiring nonces).
- **Hybrid**: monotonic counter as normative defense, nonce as binding (SwarmSpeak handshake).

IDS application: every Byzantine message carries `(sender_id, sequence, nonce)` covered by ed25519 signature; replay cache per sender bounded with TTL eviction; sequence-number gaps trigger retransmission requests via ids-carrier::Endpoint re-send.

Sources: Replay attacks (arxiv 1901.11218; crypto.stackexchange MAC replay); monotonic nonce proofs (StoyanDenev GitHub); IETF timestamp/nonce guidance (3GPP TVP; RFC 9692); SwarmSpeak handshake freshness counters (datatracker.ietf.org); EIP-8266 expiring nonces.

#### 3.9 Traffic Manipulation

Byzantine nodes can delay, reorder, drop, or selectively deliver messages to bias consensus (e.g., timestamp bias in ordering, censorship, front-running). Defenses:
- **Sequence-numbered, signed messages**: reordering/dropping detectable via gaps; delivery buffers hold out-of-order messages.
- **View-based leadership with rotation**: limits how long a malicious leader can manipulate ordering (censorship resistance via frequent leader rotation — Moonshot research).
- **Quorum-based agreement**: safety holds even if the network adversary delays messages between honest nodes, as long as quorum intersection holds (partial synchrony).
- **Ordered consensus (Pompë)**: factors ordering out of consensus so Byzantine leaders cannot fully control final transaction ordering.

IDS application: all consensus messages carry view + sequence + digest; traffic manipulation is masked by quorum rules; leader rotation bounds manipulation window; `ids-ordering-domain` scopes leadership per dataset.

Sources: Traffic manipulation / ordering bias (arxiv 2411.09981 SoK fair ordering; cs.cornell.edu Pompë); Moonshot leader rotation (arxiv 2401.01791); causal-order Byzantine reliable broadcast (arxiv 2609.17074).

#### 3.10 Resource Exhaustion & DoS Resistance

BFT consensus is vulnerable to resource-exhaustion attacks:
- **Leader-targeted DoS**: a few Mbps of attack traffic halts HotStuff-style consensus within seconds (Empirical Study of Consensus DoS, AsiaCCS 2024); cost independent of committee size because the adversary tracks each leader.
- **Mempool explosion / inflation attacks** (Lifefin, arxiv 2511.15936): uncommitted DAG vertices accumulate; adversary proposes vertices but never votes; correct nodes exhaust storage/memory maintaining the mempool; liveness stalls even after GST. Bounded working storage (ACS mechanism) is the escape.
- **Pull-induction attacks** (Beluga, arxiv 2511.15517): adversary withholds blocks so only a subset receives them, triggering redundant overlapping pulls that exhaust bandwidth.
- **Merge bombs / metadata bombs**: small deltas trigger disproportionate state growth or CPU (bounded CRDT research).
- **View-change amplification**: Byzantine primary triggers repeated view changes, each costly.

Defenses: rate limiting per peer; per-peer quotas (ids-admission); batch signature verification + verification cache; bounded buffers with watermark garbage collection; custody-signal-driven retransmission (no RTT-bound timers); block synchronizer with resource-aware exchange; bounded mempool storage; view-change cost amortization; exclusion of persistently misbehaving peers.

IDS application: resource-exhaustion defenses live at the ids-byzantine layer via per-endpoint message budgets, replay-cache bounds, delivery-buffer watermarks, and quarantine integration with ids-fault circuit breakers. All enforced without std::net/tokio::net — purely at the Endpoint datagram level.

Sources: Consensus DoS resilience (netsec.ethz.ch AsiaCCS 2024); Lifefin mempool explosions (arxiv 2511.15936); Beluga block synchronizer (arxiv 2511.15517); Bounded CRDTs (arxiv 2310.18220; arxiv 1603.01529); Velisarios Coq PBFT verification (garbage collection + view change).

#### 3.11 Byzantine Node Isolation / Quarantine

Detection vs. masking: BFT masks bounded faults; detection systems identify faulty nodes so correct nodes can cease communicating with them (isolation/containment). Properties: **completeness** (observed faulty behavior eventually yields evidence against ≥1 faulty node) and **accuracy** (no valid evidence against a correct node). Detection enables: cease communication, initiate recovery, alert operator. Deterrent effect: faulty node risks isolation/expulsion.

PEACHes / DecentBFT (USENIX / ACM): replicas vote about subjective observations of omissions to a trusted config manager; detection separated from effectuation; fine-grained Byzantine behavior compatible with existing BFT; rapid detection of performance attacks. Byzantine Fault Isolation (Farsite, IPTPS 06): prevents a faulty BFT group from corrupting the whole system via formal semantic weakening/strengthening.

Indication levels: `trusted` (no evidence), `suspected` (evidence of ignoring inputs), `exposed` (proof of deviation from spec). Exclusion is last resort, gated on bootstrap-authenticated identity (not unauthenticated peer handles) to avoid Sybil gaming of reputation.

IDS application: ids-byzantine exposes a quarantine module: detection evidence (signed, verifiable), suspicion states per peer, exclusion with causal validity preservation (prior accepted updates stay valid — content trust ≠ identity trust), integration with ids-fault circuit breakers. No std::net — quarantine acts by refusing to send datagrams via ids-carrier::Endpoint to quarantined endpoints.

Sources: Detection & elimination of Byzantine peers (doi 10.1145/3357223.3365442 PEACHes); PeerReview detection (mpi-sws hotdep); Byzantine Fault Isolation (iptps06.cs.ucsb.edu Douceur-BFI); Byzantine fault containment (courses.cs.vt.edu PBFT presentation).

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| f Byzantine among n=3f+1 replicas (PBFT/HotStuff) | Quorum intersection (2f+1); three-phase commit; view change on primary suspicion |
|| Any t < n Byzantine (Dolev-Strong authenticated broadcast) | PKI + ed25519 signature chains; t+1 rounds; t < n tolerance |
|| Replay of signed consensus messages | Per-sender monotonic sequence + 32-byte nonce replay cache with TTL eviction; timestamp window secondary |
|| Traffic manipulation (delay/reorder/drop) | Sequence + view + digest on every message; delivery buffer + gap detection; quorum agreement masks loss/reorder |
|| Sybil identities flooding discovery | ed25519-registered identity; per-peer cost via ids-resource/ids-admission; bootstrap-authenticated trust anchors |
|| Eclipse (all peers malicious) | CarrierType/endpoint diversity; trusted seed endpoints; peer scoring; monitor churn/drops |
|| Malicious routing/discovery | Signed endpoint claims; discovery pollution rate limits; reputation scoring |
|| Resource exhaustion / mempool explosion | Bounded buffers + watermark GC; per-endpoint message budgets; custody-signal-driven retransmission; no RTT timers |
|| DoS on leader | Leader rotation bounds exposure; rate limiting; batch verification + verification cache; per-peer quotas |
|| Byzantine equivocation (conflicting messages to different peers) | Uniform agreement + integrity; signed messages make equivocation publicly provable; evidence → quarantine |
|| Carrier-agnostic rule violation | Endpoint sole address type; dependency audit bans networking crates; source greps for prohibited symbols |
|| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; ids-crypto enforces |
|| Version skew / wire incompatibility | ProtocolVersion + EdgeCaseBits from ids-000-aa; version-tagged frames; mismatched peers reject pre-delivery |
|| Memory growth of delivery/replay buffers | Bounded watermark buffer; TTL-bounded replay cache; gap watermark advances on contiguous delivery |
|| Isolation as last resort vs. availability | Exclusion gated on bootstrap-authenticated identity; prior accepted updates stay valid (content/identity trust decoupled) |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 028 ||
||-------|-------|-------------------|
|| `ids-000-aa` (master) | Canonical types (`NodeId`, `ViewId`, `SeqNo`, `QuorumCert`, `Endpoint`, `CarrierType`, `FaultDomain`, `QuarantineState`, `OpState`, `CommitState`), sealed `trait Byzantine`, invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits` u128), unified `Error` |
|| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-byzantine` |
|| `ids-crypto` (Phase 003) | ed25519 signing/verification, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 AEAD; consumed only |
|| `ids-ordering-domain` (crate 55) | Dataset-scoped ordering domains; per-domain committed position; domain-scoped leadership (no global master) |
|| `ids-consistency` (crate 19) | Operation-state tracking; consistency profiles; committed-prefix tracking |
|| `ids-fault` (crate 27) | Circuit breakers / backpressure for Byzantine delivery; quarantine integration |
|| `ids-admission` | Per-endpoint rate limits and quotas for DoS resistance |
|| `ids-resource` (crate 32) | Resource tracking; per-peer message budgets; bounded memory |
|| `ids-discovery` | Endpoint discovery; anomaly scoring for malicious routing/discovery behavior |
|| `ids-broadcast` (crate 20) | Reliable/causal/total-order broadcast substrate; ids-byzantine builds authenticated broadcast on top |
|| `ids-time` (crate 5) | `Idstime` tick basis for view timeouts and sequence windows |

**Integration points:**
- `ids-byzantine` implements `ids_000_aa::contracts::Byzantine` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-byzantine` never touches sockets.
- Authenticated messages signed with ed25519 via `ids-crypto`; message integrity via sha256/blake3 digests.
- Depends on `ids-000-aa` only for types/contracts; Cargo workspace: `ids-byzantine = { path = "../ids-byzantine" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-byzantine` (to be created at `C:/idfs/ids-byzantine/`)

```
ids-byzantine/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-ordering-domain,
│                                       # ids-consistency, ids-fault, ids-admission, ids-resource,
│                                       # ids-discovery, ids-time, ids-broadcast; thiserror, serde;
│                                       # no networking crates; allowed crypto only
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # ViewId, SeqNo, QuorumCert, ByzantineMsg, QuarantineState,
│   │                                   # FaultDomain, ConsensusRound, Delivery
│   ├── errors.rs                       # ByzantineError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Byzantine (sealed trait)
│   │
│   ├── pbft/                           # PBFT consensus (~10 files)
│   │   ├── mod.rs
│   │   ├── pbft_state_machine.rs       # replica view/sequence/state machine
│   │   ├── pre_prepare.rs              # PRE-PREPARE message + validation
│   │   ├── prepare.rs                  # PREPARE message + prepared predicate
│   │   ├── commit.rs                   # COMMIT message + committed predicate
│   │   ├── view_change.rs              # view-change protocol + new-view proof
│   │   ├── quorum.rs                   # 2f+1 / n-f counting + quorum intersection
│   │   ├── primary_rotation.rs         # primary = view mod n
│   │   ├── pbft_messages.rs            # datagram types for PBFT
│   │   └── pbft_layer.rs               # PBFT composition over Endpoint
│   │
│   ├── hotstuff/                       # HotStuff consensus (~10 files)
│   │   ├── mod.rs
│   │   ├── quorum_certificate.rs       # QC formation + verification (ed25519)
│   │   ├── hotstuff_view.rs            # view-by-view recipe
│   │   ├── hotstuff_messages.rs        # propose / vote datagram types
│   │   ├── linear_view_change.rs       # linear view change (no all-to-all)
│   │   ├── pipelining.rs               # chained proposals / back-to-back decisions
│   │   ├── responsiveness.rs           # optimistic responsiveness model
│   │   ├── leader_rotation.rs          # leader = view mod n, rotate every round
│   │   ├── hotstuff_state.rs           # locked QC / high QC tracking
│   │   └── hotstuff_layer.rs           # HotStuff composition over Endpoint
│   │
│   ├── flexible_bft/                   # Flexible BFT (~8 files)
│   │   ├── mod.rs
│   │   ├── flexible_quorum.rs          # quorum availability q_r / commit quorum q_c
│   │   ├── quorum_intersection.rs      # cross-phase intersection Q1 ∩ Q2 ≠ ∅
│   │   ├── client_assumptions.rs       # per-client fault/timing assumptions
│   │   ├── strengthened_fault.rs       # resilience 1/3 → 2/3 via confirmations
│   │   ├── flexible_messages.rs        # datagram types
│   │   └── flexible_layer.rs           # Flexible BFT composition over Endpoint
│   │
│   ├── dolev_strong/                   # Dolev-Strong authenticated broadcast (~8 files)
│   │   ├── mod.rs
│   │   ├── signature_chain.rs          # chain of distinct signer signatures
│   │   ├── dolev_strong_rounds.rs      # t+1 round protocol
│   │   ├── pki_registry.rs             # PKI public-key registry (ed25519)
│   │   ├── dolev_messages.rs           # authenticated broadcast datagram types
│   │   ├── validity_agreement.rs       # validity + agreement + termination predicates
│   │   └── dolev_layer.rs              # Dolev-Strong composition over Endpoint
│   │
│   ├── authenticated_broadcast/        # Authenticated broadcast (~6 files)
│   │   ├── mod.rs
│   │   ├── bracha_broadcast.rs         # Bracha SEND/ECHO/READY (f < n/3)
│   │   ├── echo_broadcast.rs           # echo broadcast with signatures (linear msgs)
│   │   ├── consistent_broadcast.rs     # consistent broadcast (Srikanth-Toueg)
│   │   ├── auth_messages.rs            # ed25519-signed broadcast messages
│   │   └── auth_layer.rs               # authenticated broadcast composition
│   │
│   ├── sybil/                          # Sybil resistance (~5 files)
│   │   ├── mod.rs
│   │   ├── identity_binding.rs         # bind ed25519 key to per-identity cost
│   │   ├── cost_enforcement.rs         # per-identity cost via ids-resource/ids-admission
│   │   ├── freshness.rs                # per-period work freshness model
│   │   └── sybil_resistance.rs         # Sybil-resistance policy composition
│   │
│   ├── eclipse/                        # Eclipse resistance (~5 files)
│   │   ├── mod.rs
│   │   ├── endpoint_diversity.rs       # CarrierType/endpoint diversity policy
│   │   ├── seed_endpoints.rs           # trusted seed endpoints for bootstrapping
│   │   ├── peer_scoring.rs             # peer scoring / reputation for eclipse detection
│   │   └── eclipse_layer.rs            # eclipse-resistance composition
│   │
│   ├── replay/                         # Replay-attack defense (~5 files)
│   │   ├── mod.rs
│   │   ├── sequence_numbers.rs         # per-sender monotonic sequence numbers
│   │   ├── nonce_cache.rs              # 32-byte nonce replay cache with TTL eviction
│   │   ├── timestamp_window.rs         # signed timestamp validity window (secondary)
│   │   └── replay_layer.rs             # replay-defense composition
│   │
│   ├── traffic/                        # Traffic-manipulation resistance (~5 files)
│   │   ├── mod.rs
│   │   ├── message_gaps.rs             # gap detection via sequence numbers
│   │   ├── delivery_buffer.rs          # out-of-order holdback + gap watermark
│   │   ├── leader_rotation_bounds.rs   # bounds on leader manipulation window
│   │   └── traffic_layer.rs            # traffic-manipulation resistance composition
│   │
│   ├── dos/                            # DoS & resource exhaustion (~8 files)
│   │   ├── mod.rs
│   │   ├── message_budget.rs           # per-endpoint message budgets
│   │   ├── bounded_buffer.rs           # bounded buffers with watermark GC
│   │   ├── replay_cache_bounds.rs      # bounded replay cache
│   │   ├── batch_verification.rs       # batch ed25519 verification + verification cache
│   │   ├── custody_retransmit.rs       # custody-signal-driven retransmission (no RTT timers)
│   │   ├── view_change_cost.rs         # view-change cost amortization
│   │   └── dos_layer.rs                # DoS-resistance composition
│   │
│   ├── quarantine/                     # Byzantine node isolation/quarantine (~8 files)
│   │   ├── mod.rs
│   │   ├── detection_evidence.rs       # signed, verifiable detection evidence
│   │   ├── suspicion_states.rs         # trusted / suspected / exposed per peer
│   │   ├── completeness_accuracy.rs    # completeness + accuracy properties
│   │   ├── exclusion.rs                # exclusion as last resort (gated on bootstrap identity)
│   │   ├── causal_validity.rs          # prior accepted updates stay valid after revocation
│   │   ├── quarantine_messages.rs      # quarantine datagram types
│   │   └── quarantine_layer.rs         # quarantine composition over Endpoint
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper
│   ├── codec/                          # wire encoding of Byzantine messages (~4 files)
│   │   ├── mod.rs
│   │   ├── byzantine_frame.rs          # version-tagged frame header
│   │   ├── message_codec.rs            # serde/bincode encode/decode
│   │   └── digest.rs                   # sha256/blake3 message digests
│   │
│   ├── validation/                     # input validation rules (~3 files)
│   │   ├── mod.rs
│   │   ├── message_validation.rs       # schema + size/depth budgets before semantics
│   │   └── byzantine_predicates.rs     # deterministic validity predicates per message class
│   │
│   ├── state/                          # state machines (~3 files)
│   │   ├── mod.rs
│   │   ├── consensus_state_machine.rs
│   │   └── quarantine_state_machine.rs
│   │
│   ├── observability/                  # Byzantine event types (view-change, quarantine, equivocation)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── pbft_tests.rs                   # three-phase commit + view change
│   ├── hotstuff_tests.rs               # QC + linear view change
│   ├── flexible_bft_tests.rs           # flexible quorum intersection
│   ├── dolev_strong_tests.rs           # signature chains + t+1 rounds
│   ├── authenticated_broadcast_tests.rs# Bracha / echo / consistent broadcast
│   ├── sybil_tests.rs                  # Sybil resistance policy
│   ├── eclipse_tests.rs                # eclipse resistance policy
│   ├── replay_tests.rs                 # sequence + nonce replay defense
│   ├── traffic_tests.rs                # gap detection + delivery buffer
│   ├── dos_tests.rs                    # message budgets + bounded buffers
│   ├── quarantine_tests.rs             # detection + exclusion + causal validity
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~110–160 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

1. **PBFT three-phase consensus** — PRE-PREPARE (primary assigns seqno in view) → PREPARE (2f matching) → COMMIT (2f+1); prepared/committed predicates; quorum intersection guarantees safety; view change on primary suspicion with new-primary proof of committed requests.
2. **HotStuff quorum certificates** — QC = n−f signed votes for a block/view; view-by-view recipe; linear view change via new leader collecting QCs; pipelined chained proposals; optimistic responsiveness at network speed δ.
3. **Flexible BFT quorums** — quorum availability q_r (replicas responding to leader) and commit quorum q_c (clients commit); only cross-phase intersection Q1 ∩ Q2 ≠ ∅ required; per-client fault/timing assumptions; strengthened resilience 1/3→2/3 via confirmations.
4. **Dolev-Strong authenticated broadcast** — signature chains of distinct signers; t+1 rounds; tolerates any t < n with PKI; honest parties accept only round-matched chain lengths; validity/agreement/termination.
5. **Authenticated broadcast (Bracha / echo / consistent)** — SEND/ECHO/READY three-phase (f < n/3); echo broadcast with signatures (linear messages); consistent broadcast (Srikanth-Toueg, quadratic, two-message latency).
6. **Replay defense** — per-sender monotonic sequence numbers; 32-byte nonce replay cache with TTL-bounded eviction; signed timestamp window as secondary check; strict-equality nonce gate.
7. **Traffic-manipulation resistance** — every message carries (view, sequence, digest); delivery buffer + gap watermark; leader rotation bounds manipulation window; quorum agreement masks delay/reorder/drop.
8. **DoS / resource exhaustion resistance** — per-endpoint message budgets; bounded buffers with watermark GC; bounded replay cache; batch ed25519 verification + verification cache; custody-signal-driven retransmission (no RTT-bound timers); view-change cost amortization.
9. **Quarantine / isolation** — signed detection evidence; per-peer trusted/suspected/exposed states; completeness + accuracy properties; exclusion as last resort gated on bootstrap-authenticated identity; prior accepted updates stay valid (content/identity trust decoupled).
10. **Sybil resistance** — ed25519 identity binding; per-identity cost via ids-resource/ids-admission; freshness (per-period work); shared deadline; sequential non-batchable work model.
11. **Eclipse resistance** — CarrierType/endpoint diversity; trusted seed endpoints; peer scoring/reputation; churn/drop monitoring.
12. **Message digest** — sha256/blake3 digest over `(sender, view, seq_no, domain, payload)` for dedup and integrity; committed-prefix tracking reuses `ids-consistency`.

---

### 8. Dependencies

`ids-byzantine/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Byzantine`, invariants, versioning, errors
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 (consumed only)
- `ids-ordering-domain = { path = "../ids-ordering-domain" }` — dataset-scoped ordering domains
- `ids-consistency = { path = "../ids-consistency" }` — op-state tracking, consistency profiles
- `ids-fault = { path = "../ids-fault" }` — circuit breakers, backpressure
- `ids-admission = { path = "../ids-admission" }` — per-endpoint rate limits, quotas
- `ids-resource = { path = "../ids-resource" }` — resource tracking, per-peer budgets
- `ids-discovery = { path = "../ids-discovery" }` — endpoint discovery, anomaly scoring
- `ids-time = { path = "../ids-time" }` — `Idstime` tick basis
- `ids-broadcast = { path = "../ids-broadcast" }` — broadcast substrate (authenticated broadcast builds on it)
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — message serialization
- `blake3 = "1.5"` — content hashing (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling consensus crates are composed via adapters; PBFT/HotStuff/Flexible BFT are implemented in this crate per prompts.txt entry 28 contents.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | PBFT consensus specified | `src/pbft/` module; three-phase (pre-prepare/prepare/commit); 3f+1; view change |
|| 2 | HotStuff consensus specified | `src/hotstuff/` module; quorum certificates; linear view change; optimistic responsiveness |
|| 3 | Flexible BFT specified | `src/flexible_bft/` module; flexible quorums q_r/q_c; cross-phase intersection |
|| 4 | Dolev-Strong authenticated broadcast | `src/dolev_strong/` module; signature chains; t+1 rounds; t < n with PKI |
|| 5 | Authenticated broadcast (Bracha/echo/consistent) | `src/authenticated_broadcast/` module; SEND/ECHO/READY or echo/consistent variants |
|| 6 | Sybil resistance | `src/sybil/` module; identity binding; per-identity cost enforcement |
|| 7 | Eclipse resistance | `src/eclipse/` module; endpoint diversity; seed endpoints; peer scoring |
|| 8 | Replay-attack defense | `src/replay/` module; sequence numbers; nonce cache; timestamp window |
|| 9 | Traffic-manipulation resistance | `src/traffic/` module; message gaps; delivery buffer; leader rotation bounds |
|| 10 | DoS & resource exhaustion | `src/dos/` module; message budgets; bounded buffers; custody retransmit |
|| 11 | Byzantine node isolation/quarantine | `src/quarantine/` module; detection evidence; suspicion states; exclusion; causal validity |
|| 12 | Sealed `trait Byzantine` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Byzantine` |
|| 13 | Canonical types from ids-000-aa only | `types.rs` imports types from `ids_000_aa`; no redefinitions |
|| 14 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`; all traffic via `ids_carrier::Endpoint` |
|| 15 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced |
|| 16 | No code written / no source modifications | Plan document only; `ids-byzantine/` crate not created; no edits to existing crates |
|| 17 | Conformance test plan enumerated | `tests/` list covers PBFT, HotStuff, Flexible BFT, Dolev-Strong, authenticated broadcast, Sybil, eclipse, replay, traffic, DoS, quarantine, carrier-agnostic assertion, crypto allowlist assertion |
|| 18 | File structure within range | ~110–160 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
||-------------|--------|-----------------|
|| Consensus variant selection: PBFT vs HotStuff vs Flexible BFT as primary | Affects `src/pbft/`, `src/hotstuff/`, `src/flexible_bft/` emphasis and default fault tolerance | All three specified; default PBFT for crash+Byzantine, HotStuff for linear view change, Flexible BFT for mixed client assumptions; decide at implementation |
|| Authenticated broadcast variant: Bracha vs echo vs consistent | Message complexity (quadratic vs linear) vs latency trade-off | All three specified; Bracha as default (f < n/3), echo/consistent as alternatives |
|| Quarantine exclusion policy | Exclusion can fragment availability under partitions; reputation can be Sybil-gamed | Exclusion gated on bootstrap-authenticated identity; prior updates stay valid; policy-driven, optional |
|| Bounded buffer / replay cache sizes | Memory safety under loss/attack vs. legitimate burst tolerance | Bounded watermark buffer + TTL cache; exact caps tuned in implementation |
|| View-change cost under Byzantine primary | Repeated view changes can stall liveness | View-change cost amortization; leader rotation bounds exposure; decided at implementation |
|| Integration timing with ids-ordering-domain | Ordering domain must exist before Byzantine consensus | Phases ordered: ids-byzantine after ordering-domain crate exists |
|| Whether Dolev-Strong PKI registry is centralized or distributed | Affects deployment topology | PKI registry as pluggable interface; bootstrap-authenticated seeds as default |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|
|| Separate `ids-byzantine` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; enforces single-responsibility boundary |
|| PBFT + HotStuff + Flexible BFT + Dolev-Strong all specified | Matches prompts.txt entry 28 contents exactly; each independently testable |
|| All traffic via `ids-carrier::Endpoint` only | Carrier-agnostic absolute rule (CRATES.md, prompts.txt entry 28) |
|| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
|| n = 3f+1 for PBFT/HotStuff; t < n for Dolev-Strong with PKI | Classical BFT resilience bounds (PBFT paper; Dolev-Strong lower bound) |
|| Quarantine as last resort, gated on bootstrap-authenticated identity | Avoids Sybil-gamed reputation; completeness + accuracy properties (PeerReview/PEACHes research) |
|| No RTT-bound timers; custody-signal-driven retransmission | Works over arbitrary carrier delay; matches carrier-agnostic unknown-latency model |
|| Replay defense: sequence + nonce + timestamp window | Defense in depth; monotonic counter normative, nonce binding, timestamp secondary |
|| Uniform agreement/integrity variants | Strongest safety; matches `trait Byzantine` contract intent in ids-000-aa |
|| Dataset-scoped ordering domains, no global master | Invariant `no_global_master` in ids-000-aa; leadership per domain |
|| No code in this phase | Research only per prompts.txt entry 28 |

---

### 12. Sources

1. PBFT — Castro & Liskov, SOSP 1999 — css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf — 3f+1 replicas; pre-prepare/prepare/commit; view change; f Byzantine tolerance.
2. PBFT lecture notes — people.eecs.berkeley.edu / web.eecs.umich.edu — three-phase protocol; authenticated messages; prepared/committed predicates.
3. PBFT review — cs.utah.edu/~stutsman/cs6963/public/pbft.pdf — committed/committed-local definitions; commit-phase invariant.
4. HotStuff — arxiv.org/abs/1803.05069 — leader-based BFT; QC; linear view change; responsiveness; linear communication.
5. Lessons from HotStuff — arxiv.org/pdf/2305.13556 — view-by-view recipe; optimistic responsiveness; secure broadcast building block.
6. HotStuff-2 — eprint.iacr.org/2023/397 — optimal two-phase responsive BFT; O(n²) worst-case, optimistically linear communication.
7. Jolteon/Ditto — arxiv.org/abs/2106.10362 — asynchronous fallback for pacemaker; DDoS resilience.
8. MonadBFT — arxiv.org/abs/2502.20692 — fork-resistant; timeout-free happy and unhappy paths.
9. Flexible BFT — arxiv.org/abs/1904.10067 — flexible Byzantine quorums; q_r/q_c; cross-phase intersection; no synchrony for protocol execution.
10. Strengthened Fault Tolerance — arxiv.org/abs/2501.03715 — resilience 1/3 → 2/3 via confirmations; safe after optimistic period.
11. Flexible Paxos — arxiv.org/abs/1608.06696 — cross-phase quorum intersection only (Q1 ∩ Q2 ≠ ∅).
12. Dolev-Strong authenticated broadcast — decentralizedthoughts.github.io/2019-12-22-dolev-strong — signature chains; t+1 rounds; PKI; t < n tolerance; O(n²t) words.
13. Dolev-Strong lecture — cs.umd.edu/~jkatz/gradcrypto2/NOTES/lecture26.pdf — broadcast impossibility t ≥ n/3 without setup; t+1 round lower bound; authenticated broadcast with PKI.
14. Dolev-Strong blockchain notes — homes.cs.washington.edu/~jrl/cse422wi24/notes/blockchain_3.pdf — Byzantine broadcast consistency/validity.
15. Authenticated broadcast / Srikanth-Toueg — dcl.epfl.ch/site/_media/education/sdc_byzconsensus.pdf — consistent broadcast; quadratic messages; two-message latency.
16. Echo broadcast with signatures — disco.ethz.ch/courses/ss04/distcomp/lecture/chapter9.pdf — linear messages; higher latency; n > 3t.
17. Bracha reliable broadcast — arxiv.org/pdf/2104.03673 — SEND/ECHO/READY; f < n/3; BRB-Validity/No Duplication/Integrity/Agreement.
18. Byzantine broadcast definitions — arxiv.org/abs/2002.11321 — termination/agreement/validity; authenticated BB t < n; Dolev-Strong O((k+ks)n² + n³).
19. Sybil resistance / bilinear cost — arxiv.org/abs/2611.06268 — identity binding, freshness, shared deadline, sequential non-batchable work; VDF meets all four.
20. AetherWeave Sybil-resistant peer discovery — arxiv.org/abs/2603.23793 — stake-backed peer discovery; honest overlay connected w.h.p.; attack-detection flags.
21. Eclipse attacks — bitcoinops.org/en/topics/eclipse-attacks — isolation from honest peers; diverse peer selection; trusted seed nodes.
22. Ethereum eclipse attacks — arxiv.org/abs/2601.16560 — discovery-table poisoning; DNS crawler manipulation; connection-slot hijacking.
23. Monero unreachable-node eclipse — arxiv.org/abs/2609.10260 — outbound-peer dependency; self-reinforcing isolation.
24. NEAR P2P eclipse advisories — chainscorelabs.com — peer count/churn monitoring; multiple redundant network paths.
25. Replay attacks — arxiv.org/abs/1901.11218 — sequence-number freshness; cross-shard replay defense.
26. Monotonic nonce proof — github.com/StoyanDenev/decentralized-message-queue — strict-equality nonce gate; consume-once semantics.
27. IETF timestamp/nonce guidance — 3GPP TVP; RFC 9692 — sequence numbers vs timestamps for freshness.
28. SwarmSpeak handshake — datatracker.ietf.org/doc/draft-swarmspeak-handshake — monotonic counter normative; nonce binding; one-nonce-one-presentation.
29. Traffic manipulation / fair ordering — arxiv.org/abs/2411.09981 — observation falsification; scheduling; suppression; leader ordering leverage.
30. Pompē ordered consensus — cs.cornell.edu/lorenzo/papers/Zhang20Byzantine.pdf — ordering factored out of consensus; Byzantine leaders cannot fully control ordering.
31. Consensus DoS resilience — netsec.ethz.ch/publications/papers/Consensus_DDoS_AsiaCCS_24.pdf — leader-targeted DoS within seconds; few Mbps; cost independent of committee size.
32. Lifefin mempool explosions — arxiv.org/abs/2511.15936 — inflation attacks; bounded working storage via ACS mechanism.
33. Beluga block synchronizer — arxiv.org/abs/2511.15517 — pull-induction attacks; resource-aware incremental block retrieval.
34. Bounded CRDTs / merge bombs — arxiv.org/abs/2310.18220; arxiv.org/abs/1603.01529 — cost bounding; delta-CRDT anti-entropy.
35. Velisarios Coq PBFT — link.springer.com/10.1007/978-3-319-89884-1_22 — PBFT verification; garbage collection; view change.
36. PEACHes / DecentBFT — doi.org/10.1145/3357223.3365442 — detection vs. masking; completeness + accuracy; exclusion as last resort.
37. PeerReview detection — people.mpi-sws.org/~druschel/publications/peerreview-hotdep.pdf — detectably faulty/ignorant; trusted/suspected/exposed indications.
38. Byzantine Fault Isolation — iptps06.cs.ucsb.edu/papers/Douceur-BFI.pdf — prevents faulty BFT group from corrupting whole system; formal semantic weakening/strengthening.
39. IDS prompts.txt entry 28 (verbatim) — C:/idfs/prompts.txt — crate definition, contents, constraints.
40. CRATES.md entry 28 — C:/idfs/CRATES.md — ids-byzantine: BFT consensus and authenticated broadcast for malicious nodes.
41. ids-000-aa skeleton — C:/idfs/ids-000-aa/ — src/contracts/byzantine.rs (`trait Byzantine`); canonical types; invariants; versioning; errors.
42. HX-241 plan format — C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture, Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
43. Existing plan-020.md — C:/idfs/plans/plan-020.md — phase plan template followed by this document (ids-broadcast, same carrier-agnostic/crypto-allowlist rules).

---

**End of plan-028.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**
