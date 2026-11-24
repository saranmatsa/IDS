## Phase 075 — Create the ids-swim crate (Scalable Weak Membership failure detection): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1120-1134)

```
75. Research only do not read plan.md **[Phase 075 — Create the ids-swim crate: Scalable Weak Membership failure detection.]**

Crate: ids-swim
Purpose: Scalable Weak Membership failure detection.
Contents: SWIM protocol, gossip-based suspicion, ping/ping-ack, suspicion dissemination, failure confirmation, scalability to thousands of nodes.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 075 — Create the ids-swim crate: Scalable Weak Membership failure detection.]**

Create plan-075.md in C:/idfs/plans/ for the ids-swim crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 075 creates `ids-swim`, the IDS crate for scalable weak-membership failure detection. It implements the SWIM protocol (Das, Gupta, Motivala, Cornell, IEEE SRDS 2002/2004): a membership service built from two separated components — a **failure detector** (peer-to-peer periodic randomized probing via ping/ping-ack, with indirect probing through k other nodes when a direct ping fails) and a **dissemination component** (infection-style gossip that piggybacks membership updates — joins, voluntary leaves, failures — on the ping/ack datagrams). Suspicion is used before declaring failure (a suspected node is gossiped as suspicious; an `alive` multicast clears the suspicion label), which keeps the false-positive rate low while guaranteeing eventual detection of every crash. Expected detection time and per-member message load are independent of group size, which is what lets SWIM scale to thousands of nodes where all-to-all heartbeating fails quadratically.

`ids-swim` is a **separate crate** (not a folder inside an existing crate, per the project rule). It is a policy/state-machine crate: it maintains the local membership list (`NodeId` → state + incarnation + last-heard timestamp), runs probe rounds, decides suspicion/failure transitions, and emits membership-update envelopes. It never opens a socket or addresses IP/TCP/UDP/QUIC/HTTP/DNS; every outbound message is an opaque `Vec<u8>` datagram handed to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes), and every inbound message arrives from the carrier boundary. Allowed crypto only: ed25519 (signed failure/suspicion announcements and audit checkpoints), sha256 and blake3 (membership-list digests, anti-entropy reconciliation, audit-chain links), xchacha20poly1305 and chacha20poly1305 (membership state encrypted at rest where confidentiality is required). Crypto operations live in `ids-crypto`; `ids-swim` consumes only the type surface and primitive IDs from `ids-000-aa`/`ids-crypto`. `ids-000-aa` remains the master crate holding canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `IdsTime`, `Checksum`, `Error`, …) and sealed trait contracts; `ids-swim` imports them and never redefines them.

A skeleton `ids-swim` crate does NOT yet exist at `C:/idfs/ids-swim/`. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned the original SWIM paper (cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf — randomized probing, ping/ping-ack, indirect probes via k nodes, suspicion, infection-style dissemination, round-robin probe targets, detection time independent of group size), the IEEE SRDS record (ieeexplore.ieee.org/document/1028914), the Wikipedia SWIM Protocol summary (ping every T′, suspect→fail after T′, strong completeness, detection time T′/(1−e^{−qf})), Lifeguard (arxiv.org/html/1707.00788v2 — local health awareness, more accurate failure detection over gossip), HashiCorp Consul memberlist/serf gossip internals (hashicorp.com, github.com/hashicorp/serf), SWIM vs Phi accrual comparison (systemoverflow.com), Illinois CS425 SWIM lecture notes (courses.grainger.illinois.edu), dkmccandless "SWIM for Implementers" (probe-round pseudocode), Apple swift-cluster-membership SWIM (github.com/apple/swift-cluster-membership), Tarantool swim module (tarantool.io), brianstorti.com SWIM explainer, prakhar.me SWIM article, builddistributedsystem.com SWIM detector task, Mochi SWIM backend (mochi.readthedocs.io), Vanlightly TLA+ SWIM model, medium.com Rust SWIM build.

---

### 2. Architecture Overview

`ids-swim` sits above `ids-000-aa` (canonical types, sealed contracts, invariants, errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a failure-detection and membership-dissemination state machine: it runs periodic probe rounds, sends pings, handles ping-acks, escalates to indirect probes (ping-req via k other nodes), transitions members through ALIVE → SUSPECT → FAILED → LEFT, gossips membership changes (piggybacked on pings/acks), and returns signed audit entries. It never sends bytes itself — it produces datagram envelopes and `AckDecision`/`FailureVerdict` objects that the caller frames into `ids-carrier::Endpoint` datagrams.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                 MEMBERSHIP CONSUMERS (ids-*, application layer)               │
│  ids-store, ids-replication, ids-placement — read local membership list      │
├──────────────────────────────────────────────────────────────────────────────┤
│                    Phase 075: ids-swim (THIS CRATE — policy/state)           │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐   │
│  │ membership      │ │ probe scheduler │ │ ping engine     │ │ suspicion│   │
│  │ list (NodeId →  │ │ (random +       │ │ (ping, ping-ack,  │ │ state    │   │
│  │  state+incarn)  │ │  round-robin)   │ │  indirect probe)  │ │ machine  │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐   │
│  │ failure         │ │ dissemination   │ │ carrier adapter │ │ crypto   │   │
│  │ confirmation    │ │ (piggyback on   │ │ (opaque Vec<u8> │ │ surface  │   │
│  │ (k-node indirect││  pings/acks)      ││  to Endpoint)     ││ (allowed │   │
││  verify)          │ │                 │ │                   ││  set)    │   │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘   │
└──────────────────────────────────────────┬──────────────────────────────────┘
                                           │ opaque Vec<u8> datagram envelopes
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh anywhere in ids-swim            │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one probe round:**
1. `probe_scheduler` picks a target: random selection (SWIM default) or round-robin (bounds worst-case detection time); every protocol period T′ each node pings one other member of its list.
2. `ping_engine` builds a ping envelope: opaque `Vec<u8>` containing probe id, sender `NodeId`, incarnation, and piggybacked membership updates (joins/leaves/failures/suspicions pending dissemination). Handler sends it via `ids-carrier::Endpoint`.
3. Target replies `ping-ack` (opaque datagram back to sender's endpoint). On ack: target marked ALIVE, "last heard from" refreshed.
4. On no response: `indirect_probe` contacts k other list members (ping-req: "ping this suspect for me"); each replies ping-ack or confirms silence.
5. After T′ with no successful response (direct or indirect): `suspicion` state machine moves target ALIVE → SUSPECT; suspicion is gossiped to all members. If target responds before suspicion timeout, an `alive` update is gossiped and the suspicion label is cleared.
6. If still unresponsive after SUSPECT → FAILED transition: `failure_confirmation` marks target FAILED; the failure is disseminated infection-style to the whole group via subsequent pings/acks.
7. `crypto_surface`: ed25519 signs failure/suspicion announcements and periodic audit checkpoints; sha256/blake3 hash membership-list digests for anti-entropy reconciliation; xchacha20poly1305/chacha20poly1305 encrypt sensitive membership state at rest.

**SWIM semantics (research-verified):**
- Strong completeness: every crash is eventually detected by all live nodes.
- Expected detection time ≈ T′ / (1 − e^{−qf}) where qf is the fraction of non-faulty nodes — detection time and per-member message load do not grow with group size.
- Weak consistency: membership lists at different nodes need not be identical at the same instant; convergence via gossip.
- No central server: fully peer-to-peer, no single point of failure or bottleneck.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline; full list in Section 12.

#### 3.1 SWIM paper: randomized probing instead of all-to-all heartbeating
**Source:** SWIM paper, Das/Gupta/Motivala, Cornell (cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf); IEEE SRDS 2002/2004 (ieeexplore.ieee.org/document/1028914).

Traditional heartbeating does not scale: heartbeats to a central server create a hot spot; heartbeats to all members (multicast or gossip) impose message load quadratic in group size; ring heartbeating has unpredictable detection time under multiple failures. SWIM's design insight: fuse nothing — separate **failure detection** (efficient non-multicast randomized probing) from **membership-update dissemination** (used only when a change occurs). Every T′ time units, each node pings one random other node from its membership list. Mathematical analysis shows expected failure-detection time, false-positive rate, and per-member message load are all independent of group size — enabling thousands of nodes. `ids-swim` adopts this separation as its core architecture: probe rounds run continuously; dissemination runs only on membership changes, piggybacked on probe traffic.

#### 3.2 Ping / ping-ack exchange
**Source:** Wikipedia "SWIM Protocol"; SWIM paper; dkmccandless "SWIM for Implementers" (dkmccandless.github.io/2022/06/27/swim-for-implementers.html).

Node N1 picks target N2 and sends a ping. If N2 replies with ping-ack, N2 is healthy: N1 updates N2's "last heard from" timestamp to now. If no reply within T′, N1 escalates to indirect probing (Section 3.4). The ping envelope carries: probe id, sender NodeId, sender incarnation, target NodeId, and piggybacked membership deltas. `ids-swim` frames ping/ping-ack as opaque `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint` targets; the carrier is never consulted about addresses.

#### 3.3 Ping-ack datagram framing (carrier-agnostic)
All traffic is opaque `Vec<u8>` to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). `ids-swim` defines only the envelope payload layout (probe id, sender/target NodeId, incarnation, state, membership delta, optional ed25519 signature, optional AEAD nonce+tag). No `std::net`, `tokio::net`, `SocketAddr`, `IpAddr`, `TcpStream`, `UdpSocket`, or any transport concept appears in the crate. The carrier adapter converts `ids-swim` envelopes to/from `Endpoint` datagrams; routing and delivery are the carrier's responsibility.

#### 3.4 Indirect probing (ping-req via k other nodes)
**Source:** SWIM paper; Wikipedia; Lifeguard (arxiv.org/html/1707.00788v2); Mochi SWIM backend (mochi.readthedocs.io/en/latest/flock/09_backends_swim.html).

When N1's direct ping to N2 gets no response, N1 contacts k other members {N3…N3+k} and asks each to ping N2 on its behalf. If any indirect probe succeeds, N2 is alive (the initial failure was likely packet loss). If none succeed within T′, N1 suspects N2. Indirect probing amortizes the cost of lost packets: one suspected node triggers k probes but those k probes are normal pings to other members, so total load stays bounded and independent of group size. `ids-swim` makes k and the indirect-probe timeout configurable per deployment.

#### 3.5 Suspicion mechanism (suspect before declaring failed)
**Source:** SWIM paper ("suspect a process before declaring it as failed — allows the system to discover and rectify false failure detections"); Wikipedia extensions (suspicion + alive multicast); Lifeguard.

Unresponsive nodes are not immediately marked FAILED: they enter SUSPECT, and the suspicion is gossiped to all members (piggybacked on pings/acks). If the suspected node responds to a subsequent ping before the suspicion timeout, an `alive` announcement is gossiped and the suspicion label is cleared — false positives self-heal without coordinator intervention. Suspicion also limits the blast radius of a mistaken failure declaration: a suspected-but-alive node is treated with degraded trust rather than evicted immediately. `ids-swim` implements a three-valued suspicion outcome per probe: `clear` (responded), `sustain` (still silent), `escalate` (timeout → FAILED).

#### 3.6 Failure confirmation and dissemination
**Source:** SWIM paper; HashiCorp Consul memberlist/serf gossip internals (github.com/hashicorp/serf; hashicorp.com "Everybody Talks: Gossip, Serf, memberlist, Raft, and SWIM in Consul"); builddistributedsystem.com SWIM detector task.

Once N2 is FAILED, N1 multicasts (infection-style, piggybacked) the failure to the whole group; every member that learns of the failure re-gossips it, so the failure spreads epidemically until all live members know. Voluntary joins/leaves are disseminated the same way. Failure confirmation requires either direct evidence (N1's indirect probes all failed) or corroborating gossip from another member (another member reports N2 FAILED — N1 can adopt the report subject to incarnation checks, Section 3.9). `ids-swim` records each state transition (ALIVE→SUSPECT→FAILED→LEFT) in its local membership list with an incarnation number and timestamp.

#### 3.7 Infection-style dissemination and round-robin probe targets
**Source:** SWIM paper (infection-style/epidemic/gossip dissemination piggybacked on pings and acks; round-robin probe target selection bounds worst-case detection time without degrading average); Wikipedia extensions.

Instead of a separate multicast for every membership change, updates are piggybacked on ping/ping-ack datagrams — dissemination rides the probe traffic that already exists, adding near-zero marginal load. Optionally, probe targets are chosen round-robin across the list rather than at random, which gives a deterministic upper bound on detection time (every member is probed within a bounded number of rounds) while keeping the average detection time unchanged. `ids-swim` supports both target-selection policies as a configurable switch; random is the SWIM default, round-robin is offered for bounded-latency deployments.

#### 3.8 Scalability to thousands of nodes
**Source:** SWIM paper abstract ("Both the expected time to first detection of each process failure, and the expected message load per member, do not vary with group size"); Wikipedia detection-time formula T′/(1−e^{−qf}); Lifeguard; Illinois CS425 SWIM lecture notes (courses.grainger.illinois.edu/CS425/fa2020/L6.FA20.pdf).

All-to-all heartbeating: message load per member grows O(N) and detection time or false-positive rate grows with N. SWIM: each member sends one ping per period T′ to one target → O(1) load per member per period, O(N) total network load but O(1) per node; indirect probing adds at most k extra pings per suspicion, still O(1) per node per period. Expected detection time is independent of N (depends on T′ and fault fraction qf, not group size). `ids-swim` is designed for thousands of nodes: bounded per-node state (membership list of known members), O(1) messages per probe round per node, gossip convergence of membership changes in O(log N) rounds.

#### 3.9 Member states, incarnation numbers, and stale-entry suppression
**Source:** SWIM paper; Tarantool swim module (tarantool.io/docs/reference_lua_swim); Apple swift-cluster-membership (github.com/apple/swift-cluster-membership/Sources/SWIM/SWIMInstance.swift); Lifeguard member states (joined/failed/suspect/left).

Each list entry: `{NodeId, state ∈ {ALIVE, SUSPECT, FAILED, LEFT}, incarnation: u64, last_heard: IdsTime, address: Endpoint}`. Incarnation (generation counter) increments on every state change; gossip carries the incarnation so members ignore stale reports (a late SUSPECT with an older incarnation than a known ALIVE entry is discarded). LEFT is set when a node voluntarily departs (gossiped leave). Rejoining after FAILED/LEFT requires a higher incarnation and re-introduction via gossip; `ids-swim` validates incarnation monotonicity before applying any remote update — this is how the crate prevents replayed or forged membership changes from resurrecting dead nodes or killing live ones. Crypto binding: failure/suspicion announcements are ed25519-signed by the reporting node so recipients can verify the reporter (Section 3.14).

#### 3.10 Anti-entropy reconciliation and membership-list digests
**Source:** Lifeguard (arxiv.org/html/1707.00788v2 — gossip robustness); SWIM paper anti-entropy component (tarantool swim anti-entropy patch discussion); systemoverflow anti-entropy digests/gossip reconciliation.

Periodically (anti-entropy interval), members exchange compact digests of their membership lists (sha256/blake3 hashes of `(NodeId, state, incarnation)` entries) instead of full lists. Mismatched digests trigger reconciliation: members exchange the differing entries and converge. Anti-entropy heals divergence caused by lost datagrams, partitioned suspicion gossip, or clock skew in last-heard timestamps. Digest size is O(number of members) but compressed via hashing; reconciliation transfers only deltas. `ids-swim` defines the digest format and reconciliation policy; transport is via `ids-carrier::Endpoint` datagrams.

#### 3.11 Detection time, false positives, and configuration knobs
**Source:** Wikipedia SWIM detection-time formula T′/(1−e^{−qf}); SWIM paper experimental results (stable detection time, stable false-positive rate, low per-member load); Lifeguard false-positive reduction.

Configurable knobs exposed by `ids-swim` (all plan-level, no values hard-coded as product decisions): protocol period T′, indirect-probe fanout k, suspicion timeout, ping timeout, anti-entropy interval, target-selection policy (random/round-robin). Trade-offs: smaller T′ → faster detection, more load; larger k → fewer false positives, more indirect-probe load; longer suspicion timeout → fewer false positives, slower detection. `ids-swim` documents the trade-off surface; concrete defaults are set at integration with `ids-membership`/deployment.

#### 3.12 SWIM vs Phi accrual failure detector (related work — Phase 076)
**Source:** systemoverflow "SWIM and Phi Accrual: Two Approaches to Failure Detection Over Gossip"; arxiv.org/html/2603.03736 (failure detector taxonomy).

SWIM is a binary-ish detector (ALIVE/SUSPECT/FAILED) driven by probe timeouts and suspicion. Phi accrual (Phase 076, `ids-phi-accrual`) outputs a continuous suspicion level φ from inter-arrival time distributions with adaptive thresholds. They are complementary: `ids-swim` provides membership + failure detection; `ids-phi-accrual` provides adaptive suspicion thresholds that can feed `ids-swim`'s suspicion escalation decision. `ids-swim` does not implement phi accrual; its suspicion/timeout knobs are static per deployment until Phase 076 integration is defined. This plan records the integration point (uncertainty in Section 10).

#### 3.13 Reference implementations
**Source:** HashiCorp memberlist (github.com/hashicorp/memberlist), serf gossip internals (github.com/hashicorp/serf/docs/internals/gossip.html.markdown), Consul (hashicorp.com "Everybody Talks…"); Mochi SWIM backend (mochi.readthedocs.io); Tarantool swim module; Apple swift-cluster-membership SWIM; brianstorti.com and prakhar.me explainers; builddistributedsystem.com SWIM detector task; medium.com "Building a SWIM Gossip Membership Protocol from Scratch in Rust"; Vanlightly TLA+ formal model of SWIM.

These confirm the production shape of SWIM: ping/ping-ack, indirect probes, suspicion, piggybacked dissemination, incarnation lists, gossip-based convergence, and memberlist-style SWIM running over unreliable datagrams. `ids-swim` adopts the same datagram-centric shape, adapted to `ids-carrier::Endpoint` instead of UDP sockets. memberlist/serf's security documentation (github.com/hashicorp/memberlist/SECURITY.md) notes gossip membership is unauthenticated by default — `ids-swim` closes that gap with ed25519-signed announcements (IDS allowed crypto only).

#### 3.14 Allowed crypto surface only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)
**Source:** RFC 8032 Ed25519; RFC 7539/RFC 8439 ChaCha20-Poly1305; libsodium XChaCha20-Poly1305; NIST FIPS 180-4 SHA-256; BLAKE3 specification; ed25519-dalek/chacha20poly1305/sha2/blake3 Rust crate docs.

`ids-swim` crypto usage (all via `ids-crypto` primitive IDs from `ids-000-aa`; `ids-swim` implements no crypto itself):
- ed25519: sign failure/suspicion announcements and audit checkpoints (proves which node reported a failure; prevents forged failure reports).
- sha256: membership-list digest entries, audit-chain links.
- blake3: fast membership-list digest hashing for anti-entropy, preferred for speed/parallelism.
- xchacha20poly1305: encrypt membership state at rest (24-byte nonce, random nonces safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based bounded use).
No other algorithm may enter the crate; a compile-time allowlist test asserts this (Section 9).

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|---|---|---|
| Forged failure report (node claims another failed) | Membership correctness | ed25519-signed failure/suspicion announcements; recipients verify signer; unsigned reports dropped |
| Replayed suspicion/failure gossip resurrects/kills nodes | Membership integrity | Incarnation numbers per entry; remote update applied only if incarnation > stored; replay cache per sender |
| Suspicion spam (constant re-suspecting) | Control-plane load | Suspicion state machine (ALIVE→SUSPECT→FAILED→LEFT monotonic); suspicion gossip dedup; per-sender rate budget |
| Lost ping mistaken for failure | False positives | Indirect probing via k nodes; suspicion-before-failure; `alive` multicast clears suspicion |
| Partition splits membership views | Availability vs consistency | Weak consistency by design; lists converge after partition heals via gossip + anti-entropy digests |
| Carrier delays/duplicates/reorders datagrams | Ordering/replay | Idempotent state-machine transitions; incarnation checks tolerate duplicates; no carrier in trust base |
| Clock manipulation skews last-heard / suspicion | Causality correctness | Logical timestamps from `ids-time` (monotonic ticks); never wall-clock for suspicion decisions |
| Key extraction (announcement signing key) | Decision authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
| Prohibited crate sneaks into dependency tree | Carrier-agnostic rule | CI dependency audit (`cargo tree`): no std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS crate |

The carrier is never trusted: announcements and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) keeps the network stack out of the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-swim |
|---|---|---|
| `ids-000-aa` | Master crate: canonical types, sealed trait contracts, invariants, errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; sealed contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) send/receive via `Carrier` trait | All ids-swim traffic flows to/from `Endpoint`; crate returns envelopes, never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signed announcements, digests, encrypted state at rest |
| `ids-time` | Monotonic IDS standard time | Probe periods, suspicion timeouts, last-heard timestamps (never wall-clock) |
| `ids-identity` (Phase 004) | `StableNodeId`/`RotatingNodeId` on `NodeId` | Node identities in membership list; rotation handled via incarnation bump |
| `ids-phi-accrual` (Phase 076) | Phi accrual suspicion with adaptive thresholds | Optional input to ids-swim suspicion escalation (integration point TBD) |
| `ids-membership` (future) | Group membership service built on failure detection | Consumes ids-swim's failure detector output as its membership source |
| `ids-fault` / `ids-backpressure` | Failure handling, bounded recovery bandwidth | Failure verdicts from ids-swim feed fault handling |

**Integration points:** ids-swim evaluates suspicion/failure and returns `FailureVerdict`/`MembershipDelta`; the caller frames envelopes into `ids-carrier::Endpoint` datagrams. ids-swim never reaches into dependent-crate internals and never addresses a transport by name.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-swim` (new crate at `C:/idfs/ids-swim/`; separate crate, not a folder inside an existing crate):

```
ids-swim/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-time, serde, thiserror (+ dev deps)
├── lib.rs                              # crate root, public API, feature gates
├── constants.rs                        # Protocol defaults: T′, k, suspicion timeout, anti-entropy interval (configurable)
├── errors.rs                           # SwimError (thiserror): probe, suspicion, verdict, carrier, config errors
├── types.rs                            # ProbeId, MembershipEntry, FailureVerdict, MembershipDelta, ProbeResult (re-exports ids-000-aa types)
├── membership_list.rs                  # Local list: NodeId → entry (state, incarnation, last_heard, endpoint)
├── member_entry.rs                     # Entry: state transitions ALIVE→SUSPECT→FAILED→LEFT, incarnation checks
├── probe_scheduler.rs                  # Target selection: random (default) or round-robin; one probe per T′ per node
├── ping_engine.rs                      # Ping/ping-ack envelope build/parse; piggybacked membership deltas
├── indirect_probe.rs                   # ping-req via k other nodes; indirect verdict aggregation
├── suspicion.rs                        # Suspicion state machine; suspicion timeout; alive-clear gossip
├── failure_confirmation.rs             # FAILED verdict: direct + corroborating gossip; failure announcement
├── dissemination.rs                    # Infection-style gossip of joins/leaves/failures/suspicions
├── carrier_adapter.rs                  # Envelope ↔ ids-carrier::Endpoint (CarrierType + Vec<u8> ≤ 1024 B)
├── crypto_surface.rs                   # Allowed-crypto allowlist; ed25519 signs, sha256/blake3 digests, AEAD at rest
├── anti_entropy.rs                     # Membership-list digest exchange and reconciliation (sha256/blake3)
├── audit.rs                            # Append-only signed, hash-chained audit of state transitions (~6 files)
│   ├── audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs, mod.rs
├── contracts/                          # sealed trait contracts (~10 files)
│   ├── mod.rs, probe_contract.rs, suspicion_contract.rs, dissemination_contract.rs,
│   │   failure_contract.rs, carrier_contract.rs, crypto_contract.rs, membership_contract.rs,
│   │   verdict_contract.rs, audit_contract.rs
├── invariants/                         # runtime/compile-time invariants (~10 files)
│   ├── mod.rs, carrier_agnostic.rs, crypto_allowlist.rs, incarnation_monotonic.rs,
│   │   suspicion_monotonic.rs, detection_bound.rs, per_node_load.rs, weak_consistency.rs,
│   │   endpoint_size.rs, mod.rs
├── fuzz/                               # fuzzing harnesses (~5 files)
│   ├── mod.rs, fuzz_state_machine.rs, fuzz_digest.rs, fuzz_verdict.rs, fuzz_incarnation.rs
├── tests/                              # property/verification tests (~30 files)
│   ├── mod.rs, probe_tests.rs, ping_tests.rs, indirect_probe_tests.rs,
│   │   suspicion_tests.rs, failure_tests.rs, dissemination_tests.rs, incarnation_tests.rs,
│   │   digest_tests.rs, carrier_agnostic_tests.rs (zero std::net/tokio::net refs),
│   │   crypto_allowlist_tests.rs, determinism_tests.rs, property_tests.rs (proptest)
├── benches/                            # (~8 files)
│   ├── mod.rs, probe_round.rs, dissemination.rs, detection_time.rs, digest_exchange.rs
├── spec/                               # normative specifications (~8 files)
│   ├── protocol_spec.md, probe_spec.md, suspicion_spec.md, dissemination_spec.md,
│   │   carrier_agnostic_spec.md, crypto_policy_spec.md, determinism_spec.md, audit_spec.md
├── docs/                               # documentation (~12 files)
│   ├── architecture.md, runbook.md, adding_a_probe_policy.md, crypto_policy.md,
│   │   integration_with_phi_accrual.md, scalability_analysis.md, ...
└── test_vectors/                       # deterministic inputs/expected outputs (~6 files)
    ├── probe_vectors.json, suspicion_vectors.json, digest_vectors.json, ...
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `spec/`, `docs/`, `fuzz/`, `test_vectors/` — one concern per file, consistent with the "separate crates instead of more folders" directive applied intra-crate as separate files/modules. No stubs; every file holds planned production-grade structure with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

1. **Membership list** — `HashMap<NodeId, MemberEntry>` where `MemberEntry { state: ALIVE\|SUSPECT\|FAILED\|LEFT, incarnation: u64, last_heard: IdsTime, endpoint: Endpoint }`. One canonical definition; consumers read via `ids-swim` query API.
2. **Probe round (every T′)** — pick target (random from list, or round-robin for bounded worst-case); build ping envelope (probe id, sender NodeId + incarnation, target NodeId, piggybacked deltas); send via `ids-carrier::Endpoint`.
3. **Ping / ping-ack** — on ping-ack: `entry.state = ALIVE; entry.last_heard = now`. On silence: escalate to indirect probe (Section 4).
4. **Indirect probe (ping-req)** — on direct-ping failure, contact k other members asking each to ping the suspect; collect ping-acks or silence; any success ⇒ suspect alive; all silent after T′ ⇒ sustain suspicion.
5. **Suspicion state machine** — `ALIVE → SUSPECT` (after T′ silence, direct + indirect) → `FAILED` (suspicion timeout, still silent or corroborated by gossip) → `LEFT` (voluntary leave gossip). SUSPECT is gossiped; `alive` multicast clears suspicion before timeout.
6. **Failure confirmation** — FAILED requires direct evidence (k indirect probes silent) OR corroborating gossip from another member with `incarnation > stored`. Failure announcement gossiped infection-style until convergence.
7. **Dissemination** — membership deltas (join/leave/fail/suspect/alive) piggybacked on ping/ping-ack; no separate multicast channel. O(log N) epidemic convergence.
8. **Anti-entropy digest** — sha256/blake3 hash of sorted `(NodeId, state, incarnation)` tuples; periodic exchange; mismatches trigger delta reconciliation.
9. **Incarnation monotonicity** — remote update applied iff `remote.incarnation > local.incarnation` for the same NodeId; equal/lower incarnations discarded (anti-replay, anti-stale-resurrection).
10. **Detection-time bound** — expected detection ≈ T′ / (1 − e^{−qf}); per-node load = 1 ping + piggyback per T′ (+ k indirect pings per suspicion) — O(1) per node, independent of group size.
11. **Signed announcement** — ed25519 signature over `{reporter: NodeId, target: NodeId, verdict, incarnation, timestamp}`; verifiable offline.
12. **Audit chain** — sha256/blake3-chained entries of every state transition; ed25519-signed checkpoint every N entries.

---

### 8. Dependencies

| Crate | Purpose |
|---|---|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — the only transport boundary ids-swim touches |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-time` | Monotonic time source for probe periods, suspicion timeouts, last-heard |
| `serde` + `thiserror` | Serialization of envelopes/verdicts; error types |
| `proptest`, `tempfile` | Dev-dependencies for property tests |

**No** `std::net`, `tokio::net`, `libp2p`, TCP/IP, UDP, QUIC, HTTP, DNS, or any networking stack in or under `ids-swim`. No async runtime. `ids-swim` imports no crypto directly — all via `ids-crypto`. Crypto beyond the five allowed primitives is rejected by the `crypto_allowlist` invariant and a compile-time test.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-75.1 | SWIM failure detector implemented: ping/ping-ack, indirect probe via k nodes, suspicion before failure, infection-style dissemination | `src/` modules: probe_scheduler, ping_engine, indirect_probe, suspicion, failure_confirmation, dissemination |
| AC-75.2 | Strong completeness: every injected crash is eventually detected by all live nodes (simulated carrier test) | Determinism test: crashed node reaches FAILED on every live member's list |
| AC-75.3 | Detection time and per-member message load independent of group size (tested at 10, 100, 1,000 nodes) | Per-node ping count per T′ = 1 (+k indirect on suspicion); detection time stable vs N |
| AC-75.4 | Suspicion self-heals: silent node that replies before suspicion timeout is cleared via `alive` gossip | Suspicion test: SUSPECT → ALIVE transition on reply |
| AC-75.5 | Member states: ALIVE→SUSPECT→FAILED→LEFT monotonic with incarnation increment on each transition | State-machine unit tests |
| AC-75.6 | Incarnation monotonicity: stale/lower-incarnation remote updates rejected | Invariants: incarnation_monotonic test |
| AC-75.7 | Anti-entropy digest reconciliation converges divergent lists | Digest test: mismatch → delta exchange → identical lists |
| AC-75.8 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Source grep + dependency-audit test (`cargo tree`) |
| AC-75.9 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 reachable in crypto surface | `crypto_allowlist` invariant + compile-time test |
| AC-75.10 | Failure/suspicion announcements ed25519-signed and verifiable | Signer/verifier tests with test vectors |
| AC-75.11 | Audit chain: sha256/blake3 hash chain + ed25519-signed checkpoints, offline verifiable | Audit tests with test vectors |
| AC-75.12 | Crate compiles cleanly: `cargo check -p ids-swim` exit 0, no warnings | Build |
| AC-75.13 | Test suite passes: `cargo test -p ids-swim` all green | Test run |
| AC-75.14 | Random vs round-robin target selection both implemented; round-robin bounds worst-case probe interval | Target-selection tests |

---

### 10. Remaining Uncertainties

1. **Concrete knob values:** protocol period T′, indirect-probe fanout k, suspicion timeout, ping timeout, anti-entropy interval — deployment-tuned; no product-level defaults decided in this plan.
2. **Target-selection default:** random (SWIM default, simplest) vs round-robin (bounded worst-case detection); to be confirmed at `ids-membership` integration.
3. **Interaction with `ids-phi-accrual` (Phase 076):** whether phi suspicion feeds ids-swim's suspicion escalation, or runs alongside; integration point undefined.
4. **WAN extension:** SWIM paper discusses extensibility to WAN-wide scale; gossip period and failure-detection trade-offs over high-latency links unquantified.
5. **Leave handling:** voluntary LEFT gossip vs silent failure — graceful-leave protocol and leave confirmation details TBD.
6. **Membership list size at scale:** thousands of nodes × per-entry state is O(N) memory per node; sharding or region-scoped lists may be needed (deferred to later phases).
7. **Corroborating-gospel trust policy:** how many independent FAILED reports are required before adopting another member's verdict without direct evidence — unquantified.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate `ids-swim` crate (not a folder) | User directive: "separate crates instead of more folders in existing ones"; clear failure-detection boundary |
| Failure detection + dissemination separated | SWIM paper core insight; dissemination only on change, piggybacked on probe traffic |
| Ping/ping-ack + indirect probes via k nodes | SWIM paper mechanism; bounds false positives from packet loss |
| Suspicion before failure (SUSPECT state) + `alive` clear gossip | SWIM paper extension; false positives self-heal, no coordinator |
| Random probe targets (default) + optional round-robin | SWIM defaults; round-robin bounds worst-case detection time |
| Infection-style dissemination piggybacked on pings/acks | Zero marginal dissemination load; O(log N) convergence |
| Incarnation numbers + monotonicity checks | Anti-replay, anti-stale-resurrection, weak consistency convergence |
| All traffic via `ids-carrier::Endpoint` opaque datagrams | Carrier-agnostic absolute rule; no network stack in ids-swim |
| Crypto only via `ids-crypto` primitive IDs from `ids-000-aa` | Allowed set: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| ed25519-signed failure/suspicion announcements | memberlist/serf gossip is unauthenticated by default; IDs close the forgery gap |
| sha256/blake3 anti-entropy digests | Compact reconciliation of divergent membership lists |
| Monotonic `IdsTime` ticks only; no wall-clock for suspicion | Clock manipulation cannot break failure detection |
| Research only: plan document, no code, no source modifications | Per prompts.txt entry 75 |

---

### 12. Sources

1. Das, Gupta, Motivala — "SWIM: Scalable Weakly-consistent Infection-style Process Group Membership Protocol," Cornell University / IEEE SRDS. https://cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf — randomized probing, ping/ping-ack, indirect probes via k nodes, suspicion, infection-style dissemination, O(1) per-member load.
2. IEEE Xplore — SWIM record. https://ieeexplore.ieee.org/document/1028914
3. Wikipedia — "SWIM Protocol." https://en.wikipedia.org/wiki/SWIM_Protocol — ping every T′, suspect→fail, strong completeness, detection time T′/(1−e^{−qf}), suspicion + alive extensions, round-robin targets.
4. Das/Gupta et al. — "Lifeguard: Local Health Awareness for More Accurate Failure Detection." https://arxiv.org/html/1707.00788v2 — gossip robustness, suspicion accuracy over SWIM.
5. HashiCorp — "Everybody Talks: Gossip, Serf, memberlist, Raft, and SWIM in Consul." https://hashicorp.com/en/resources/everybody-talks-gossip-serf-memberlist-raft-swim-hashicorp-consul — production SWIM-based membership.
6. HashiCorp serf — gossip internals. https://github.com/hashicorp/serf/blob/master/docs/internals/gossip.html.markdown — ping/ping-ack, suspicion, memberlist shape.
7. HashiCorp memberlist — SECURITY.md (unauthenticated gossip by default). https://github.com/hashicorp/memberlist/blob/master/SECURITY.md
8. System Overflow — "SWIM and Phi Accrual: Two Approaches to Failure Detection Over Gossip." https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/swim-and-phi-accrual-two-approaches-to-failure-detection-over-gossip
9. Illinois CS425 — SWIM lecture notes. https://courses.grainger.illinois.edu/CS425/fa2020/L6.FA20.pdf ; https://courses.grainger.illinois.edu/cs425/fa2022/dsn02-SWIM.pdf
10. dkmccandless — "SWIM for Implementers." https://dkmccandless.github.io/2022/06/27/swim-for-implementers.html — probe-round pseudocode.
11. Apple — swift-cluster-membership SWIM. https://github.com/apple/swift-cluster-membership/blob/main/Sources/SWIM/SWIMInstance.swift — member states, incarnation handling.
12. Tarantool — swim module + anti-entropy component. https://tarantool.io/docs/tarantool/en/3_x/reference/reference_lua/swim
13. brianstorti.com — "SWIM: The scalable membership protocol." https://brianstorti.com/swim
14. prakhar.me — "The SWIM Membership Protocol." https://prakhar.me/articles/swim/
15. builddistributedsystem.com — "Detect Failures with SWIM." https://builddistributedsystem.com/tracks/gossiper/tasks/task-3-3-4-swim-detector
16. Mochi docs — "Backend: SWIM." https://mochi.readthedocs.io/en/latest/flock/09_backends_swim.html — indirect probing, ping-req.
17. Vanlightly — formal-methods TLA+ SWIM model. https://github.com/Vanlightly/formal-methods-playground/tree/master/tla/tlaplus-conf/swim
18. medium.com — "Building a SWIM Gossip Membership Protocol from Scratch in Rust." https://medium.com/@cb7chaitanya/building-a-swim-gossip-membership-protocol-from-scratch-in-rust-e82558d9fe99
19. arXiv 2603.03736 — failure detector taxonomy (SWIM vs accrual). https://arxiv.org/html/2603.03736
20. RFC 8032 — Ed25519 (edDSA). https://www.rfc-editor.org/rfc/rfc8032
21. RFC 7539 / RFC 8439 — ChaCha20-Poly1305 AEAD. https://www.rfc-editor.org/rfc/rfc7539
22. Libsodium — XChaCha20-Poly1305 construction. https://doc.libsodium.org/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
23. NIST FIPS 180-4 — SHA-256. https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
24. BLAKE3-team — BLAKE3 specification. https://github.com/BLAKE3-team/BLAKE3
25. ed25519-dalek / chacha20poly1305 / sha2 / blake3 crate docs (docs.rs). https://docs.rs/
26. IDS project documents at C:/idfs — CRATES.md, IDS.md, prompts.txt entry 75 (lines 1120-1134), existing ids-000-aa skeleton at C:/idfs/ids-000-aa/, plan format references C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md.
27. HX-241 reference plan format — C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 075 (ids-swim).