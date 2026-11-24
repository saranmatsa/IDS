## Phase 070 — Create the ids-atomic-broadcast crate: Uniform agreement on message delivery. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1045-1059)

```text
70. Research only do not read plan.md **[Phase 070 — Create the ids-atomic-broadcast crate: Uniform agreement on message delivery.]**

Crate: ids-atomic-broadcast
Purpose: Uniform agreement on message delivery.
Contents: Uniform agreement, uniform integrity, validity, termination, reliable broadcast foundation, total order layer.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 070 — Create the ids-atomic-broadcast crate: Uniform agreement on message delivery.]**

Create plan-070.md in C:/idfs/plans/ for the ids-atomic-broadcast crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 070 creates `ids-atomic-broadcast`, the IDS crate that implements the atomic broadcast (total order broadcast) abstraction: all correct processes deliver the same set of messages in the same order, with uniform agreement (even a faulty process's delivery implies delivery at all correct processes), uniform integrity (each message delivered at most once, only if broadcast), validity (a correct broadcaster's message is eventually delivered), and termination. The crate owns the reliable-broadcast foundation (Bracha double-echo), the total-order layer (leader-sequenced slots, quorum-based commit, view changes), crash recovery via durable WAL + stable storage, bounded-memory/backpressure handling for slow peers, and all profile-critical state transitions — all carrier-agnostic.

`ids-atomic-broadcast` never opens a socket or drives any transport directly. It consumes opaque `Vec<u8>` datagrams from `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address ≤ 1024 B), produces opaque datagrams for the carrier to send, and returns delivery decisions to the caller. Allowed crypto only: ed25519 (signing sequence/certificate messages), xchacha20poly1305 and chacha20poly1305 (AEAD for state at rest), sha256 and blake3 (message/content hashing, quorum certificate chains).

A skeleton `ids-atomic-broadcast` crate does NOT yet exist at `C:/idfs/ids-atomic-broadcast/`. `ids-000-aa` is the master crate holding every canonical type (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, plus sealed trait contracts `Carrier`, `Broadcast`, `Ordering`, `Quorum`, `Recovery`, …) and the unified `Error` contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned the atomic broadcast vs reliable broadcast distinction (ACM total-order-broadcast taxonomy 10.1145/1041680.1041682; Défago et al. validity/uniform agreement/uniform integrity/uniform total order); reliable broadcast foundation (Bracha double-echo, t<n/3 Byzantine signature-free); Zab (ZooKeeper atomic broadcast, primary-backup, quorum, crash recovery); Paxos (quorum intersection, leader terms, phase 1/2); Raft (leader/follower/candidate, AppendEntries, viewstamps); Viewstamped Replication (views, viewstamps = view+op-number, recovery via f+1 responses); FLP impossibility (no deterministic consensus in pure asynchrony → partial synchrony + Ω eventual leader, or randomized w.p. 1 Ben-Or); Chandra–Toueg (consensus ≡ atomic broadcast reducibility; failure detectors); PBFT (N=3f+1, three-phase primary-backup); HotStuff (pipelined secure broadcast); Acuerdo (n=2f+1 crash quorum f+1 over RDMA); bounded-memory relaxed/abortable broadcast (slow peer ⊥ gap marker, bounded unacked pool); Ed25519 quorum certificates (Chop Chop); WAL crash recovery (write-ahead log, commit marker).

---

### 2. Architecture Overview

`ids-atomic-broadcast` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is an ordering/agreement layer: given a message to broadcast, it runs reliable broadcast to ensure all correct processes receive it, then runs total-order consensus to assign each message a global slot number, then delivers messages in slot order to the application. It never sends bytes itself; it returns `SendPlan` / `DeliveryDecision` structs that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (broadcast users)               │
│  ids-store, ids-replication, ids-fragment — abcast(message) / adeliver()     │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 070: ids-atomic-broadcast (THIS CRATE — ordering)       │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │ │\\
│  │ reliable        │ │ total order     │ │ slot sequencer  │ │ recovery │ │ │\\
│  │ broadcast       │ │ layer (leader   │ │ (slot → message │ │ (WAL +   │ │ │\\
│  │ (Bracha echo)   │ │  sequenced      │ │   commit certs) │ │ stable   │ │ │\\
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │ │\\
│  ┌─────────────────┐ ┌─────────────────┐ │                 │              │ │ │\\
│  │ uniform         │ │ bounded memory  │ │                 │              │ │ │\\
│  │ agreement       │ │ / backpressure  │ │                 │              │ │ │\\
│  │ (uniform vs     │ │ (slow peer ⊥   │ │                 │              │ │ │\\
│  │  agreement)     │ │   gap marker)   │ │                 │              │ │ │\\
│  └─────────────────┘ └─────────────────┘ │                 │              │ │\\
│  ┌─────────────────┐ ┌─────────────────┐ │                 │              │ │ │\\
│  │ view change     │ │ crypto surface  │ │                 │              │ │ │\\
│  │ (viewstamps)    │ │ (ed25519/xchacha│ │                 │              │ │ │\\
│  └─────────────────┘ └─────────────────┘ │  20poly1305/    │              │ │ │\\
│  ┌─────────────────┐ │  sha256/blake3) │ │                 │              │ │ │\\
│  │ termination     │ └─────────────────┘ │                 │              │ │ │\\
│  │ (partial sync)  │                     │                 │              │ │ │\\
│  └─────────────────┘                     │                 │              │ │\\
└─────────────────────────────────────────┬──────────────────┴──────────────┘\\
                                          │ opaque Vec<u8> datagrams\\
┌─────────────────────────────────────────▼──────────────────┐\\
│              CARRIER BOUNDARY: ids-carrier::Endpoint only  │\\
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B│\\
│  , payload: &[u8])                                         │\\
│  receive_datagram → (Endpoint, Vec<u8>)                    │\\
│  No std::net / tokio::net / libp2p / IP / TCP / UDP /      │\\
│  QUIC / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN    │\\
└────────────────────────────────────────────────────────────┘
```

**Data flow of one broadcast:**
1. Application calls `abcast(message)` → `ids-atomic-broadcast` assigns a local origin, runs reliable broadcast (echo/ready phases over `Endpoint` datagrams).
2. Once a quorum echoes, the leader sequencer proposes the message for the next slot; followers vote; a quorum certificate (ed25519-signed) finalizes the slot.
3. The slot→message mapping is durably logged (WAL + fsync-equivalent stable write) before delivery.
4. Messages are `adeliver`ed in ascending slot order; gaps are buffered; slow peers receive a bounded `⊥` gap marker and catch up via view change / state transfer.
5. Every decision (echo, ready, propose, vote, commit, deliver, view change) is recorded in an append-only, hash-chained (sha256/blake3), ed25519-signed audit log keyed by `NodeId` + `IdsTime`.

**Uniform agreement vs agreement:** standard agreement says "if a correct process delivers m, all correct processes deliver m." Uniform agreement strengthens this: "if ANY process (correct or faulty) delivers m, all correct processes eventually deliver m." This matters because a faulty process may deliver and crash before telling anyone; correct processes must still converge on m. `ids-atomic-broadcast` implements uniform agreement via the ready-quorum rule: delivery only after a quorum of READY certificates, so any single delivery implies quorum knowledge, which propagates to all correct processes.

**Crypto used:** ed25519 for signing sequence proposals, votes, quorum certificates, view-change messages, and audit checkpoints; sha256/blake3 for message/content hashing, slot certificate chaining, digest-based deduplication; xchacha20poly1305/chacha20poly1305 to encrypt durable state (slot log, view state) at rest.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Atomic broadcast = total order broadcast: four properties
**Source:** ACM "Total order broadcast and multicast algorithms: Taxonomy and survey" (10.1145/1041680.1041682); Défago et al. atomic broadcast lecture notes.

The atomic broadcast primitive (`abcast` / `adeliver`) satisfies:
- **Validity:** If a correct process abcasts m, every correct process eventually adelivers m.
- **Uniform Agreement:** If any process (correct or faulty) adelivers m, every correct process eventually adelivers m.
- **Uniform Integrity:** Every process adelivers m at most once, and only if m was previously abcast by some process.
- **Uniform Total Order:** If p adelivers m before m′, then q adelivers m only after adelivering m′.

Validity and Uniform Agreement are liveness properties; Uniform Integrity and Uniform Total Order are safety properties. `ids-atomic-broadcast` implements all four, with the stronger "uniform" variants throughout (uniform agreement and uniform integrity apply even to faulty senders/receivers).

#### 3.2 Reliable broadcast foundation (Bracha double-echo)
**Source:** EPFL DC LAB reliable broadcast notes; Bracha [Bra87] reliable broadcast; arXiv 1510.06882 Byzantine reliable broadcast.

Reliable broadcast = validity + agreement + integrity (no ordering). Bracha's algorithm uses two rounds of all-to-all exchange:
1. Sender sends (SEND, m) to all.
2. On receiving (SEND, m), echo (ECHO, m) to all.
3. On quorum ECHOs, send READY to all.
4. On t+1 READYs, echo READY; on 2t+1 READYs, deliver.

This is signature-free and tolerates t<n/3 Byzantine faults. `ids-atomic-broadcast` builds its reliable-broadcast foundation on this echo/ready quorum pattern, adapted to `ids-carrier::Endpoint` datagrams with `NodeId`-addressed opaque payloads.

#### 3.3 Uniform agreement vs agreement (the "uniform" strengthening)
**Source:** tropars DS-6 atomic broadcast lecture notes; arxiv 2001.03244 uniform reliable broadcast.

Standard agreement: "if a correct process delivers m, all correct processes eventually deliver m." Uniform agreement: "if ANY process delivers m (even a faulty one that then crashes), all correct processes eventually deliver m." Uniform integrity similarly applies "at most once" to faulty processes too. `ids-atomic-broadcast` implements uniform variants: a delivery decision requires a quorum of READY/commit evidence, so a single delivery at any node implies quorum knowledge that survives that node's crash and reaches all correct nodes via view change and state transfer.

#### 3.4 Leader-based total order (Paxos / Raft / Zab / Viewstamped Replication)
**Source:** Zab IEEE 5958223; Paxos/Flexible Paxos; Raft CS Princeton L14; Viewstamped Replication Oki/Liskov (PODC 1988 + revisited).

Total order is achieved by assigning each reliable-broadcast-delivered message a globally unique slot number, decided by consensus per slot. Practical designs are leader-based:
- **Zab** (ZooKeeper Atomic Broadcast): primary receives client requests, broadcasts transactions via Zab; on primary crash, recovery protocol agrees on consistent state and elects a new primary; quorum support required to lead.
- **Paxos:** two phases (prepare/promise, accept/accepted), majority quorum intersection guarantees safety; Flexible Paxos relaxes to cross-phase intersection only.
- **Raft:** leader/follower/candidate states; AppendEntries log replication; random election timeouts; term-based voting; at most one winner per term.
- **Viewstamped Replication:** views (view, primary); viewstamps = ⟨view-number, op-number⟩; recovery via f+1 RECOVERYRESPONSEs including one from the primary of the latest view.

`ids-atomic-broadcast` adopts a Zab/Raft-hybrid model: a sequencer (leader) assigns slots, followers acknowledge via quorum, a view-change protocol replaces a failed sequencer, and viewstamps (view, slot) detect lost/delayed information.

#### 3.5 FLP impossibility and its circumvention
**Source:** FLP JACM 1985; cs.cornell CS6410 FLP slides; Ben-Or randomized consensus; Chandra–Toueg failure detectors.

FLP: no deterministic protocol solves consensus (and hence atomic broadcast) in a fully asynchronous message-passing system with even one crash fault. Circumventions:
- **Partial synchrony** (Dwork–Lynch–Stockmeyer): message delays bounded but bound unknown; eventual leader Ω suffices — Zab/Raft/Paxos operate here.
- **Randomization** (Ben-Or): termination with probability 1 under asynchrony, f<n/2 crash.

`ids-atomic-broadcast` assumes partial synchrony for termination (like Zab/Raft); safety (integrity, agreement, total order) holds under pure asynchrony. A randomized fallback (Ben-Or-style coin) is noted as an uncertainty for the fully asynchronous case.

#### 3.6 Quorum intersection and fault tolerance
**Source:** Flexible Paxos (quorum intersection Q1+Q2>N); Acuerdo n=2f+1 crash quorum f+1; PBFT N=3f+1; deepengineering staleness SLOs.

- Crash-tolerant atomic broadcast: N=2f+1, quorum = f+1 (majority). Any two quorums intersect → at least one correct process carries state across slots/views.
- Byzantine-tolerant atomic broadcast: N=3f+1, quorum = 2f+1 (PBFT-style) or threshold signatures.
- Flexible quorums: only cross-phase intersection required (phase-1 election quorum vs phase-2 commit quorum).

`ids-atomic-broadcast` targets the crash-tolerant default (N=2f+1, quorum f+1), with a Byzantine extension noted (N=3f+1, ed25519-signed certificates).

#### 3.7 View numbers, viewstamps, and view change
**Source:** Viewstamped Replication Revisited (Liskov/Cowling); Zab recovery; Raft terms.

The system progresses through numbered views (terms), each with one sequencer. A view change is triggered by sequencer timeout/failure; the new sequencer collects state evidence (logs, commit numbers) from a quorum, installs a new view, and resumes slot assignment. Viewstamps = ⟨view, slot⟩ totally order messages and detect gaps/duplicates across view changes.

#### 3.8 Crash recovery, WAL, stable storage
**Source:** MIT 6.828 crash lecture (write-ahead log); relational DB WAL/recovery; Txn logging ARIES-style LSN.

Before a slot is committed, the sequencer durably writes the slot entry (view, slot, message hash, certificate) to stable storage (WAL). On recovery, a node replays the WAL suffix, fetches missing prefixes from a survivor, and rejoins at the latest committed slot. The commit marker ("done" in log) is written before any delivery, so a crash never loses a committed slot.

#### 3.9 Bounded memory, backpressure, and the slow-peer problem
**Source:** "Relaxed Atomic Broadcast: State-Machine Replication Using Bounded Memory" (IEEE 0950-5849(95)01094-7); "abortable broadcast" arxiv 2410.22080.

Pure atomic broadcast with bounded memory is impossible if a slow peer can be indistinguishable from a crashed one (unacknowledged messages cannot be GC'd). Relaxed atomic broadcast: when a peer lags, others buffer up to a bound, then replace missing messages with a `⊥` (void) gap marker; the lagging peer catches up via state transfer. `ids-atomic-broadcast` adopts bounded in-flight slots per peer, backpressure propagated to the broadcaster, and `⊥` gap markers for permanently-slow peers.

#### 3.10 Crypto surface: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305
**Source:** ed25519-dalek docs (RFC 8032); chacha20poly1305 docs (RFC 8439, XChaCha20 192-bit nonce); sha2 RustCrypto; BLAKE3 README; Chop Chop (Ed25519 quorum certificates).

Allowed and used:
- ed25519: signing slot proposals, votes, quorum certificates, view-change messages, audit checkpoints (32 B keys, 64 B signatures).
- sha256: message/content hashing, slot certificate chain links, dedup digests.
- blake3: fast content hashing, anti-entropy digest exchange between replicas, KDF/XOF for key derivation where needed.
- xchacha20poly1305: encrypt durable slot log / view state at rest (24-byte random nonce safe).
- chacha20poly1305: encrypt session/state payloads (12-byte counter nonce, bounded use).

No other crypto. `ids-crypto` exposes primitive IDs; `ids-atomic-broadcast` references them via `ids-000-aa` only.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Spurious / forged message delivery | Uniform integrity | ed25519-signed broadcast evidence; only messages with valid quorum certificates are delivered |
| Duplicate delivery | Uniform integrity | Slot-based dedup: each (view, slot) delivered at most once; `⊥` markers never re-delivered |
| Delivery without broadcast (out-of-thin-air) | Uniform integrity | Validity requires quorum echo/ready evidence tracing back to an abcast invocation |
| Split order across nodes | Uniform total order | Single sequencer assigns slots per view; quorum commit certifies slot before delivery |
| Faulty node delivers then crashes | Uniform agreement | Quorum READY evidence means any single delivery implies quorum knowledge; view change propagates to all correct nodes |
| Sequencer crash / partition | Termination | View change (timeout → new sequencer) after quorum state transfer; partial synchrony assumption |
| Slow peer exhausts memory | Bounded memory / liveness | Bounded in-flight slots per peer; `⊥` gap marker; backpressure to broadcaster; state transfer on rejoin |
| Key extraction (sequencer signing key) | Decision authenticity | Keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
| Carrier duplicating/delaying datagrams | Replay/ordering | Viewstamp (view, slot) dedup; idempotent delivery; carrier-agnostic boundary (opaque `Endpoint`) |
| Downgrade to weaker agreement | Consistency integrity | Uniform agreement enforced in code; downgrade requires signed governance decision, audited |

The carrier is never trusted: ordering decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-atomic-broadcast |
|-------|------|-----------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `Error` — consumed, never redefined; sealed contracts (`Broadcast`, `Ordering`, `Quorum`, `Recovery`) mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All atomic-broadcast traffic flows to/from `Endpoint`; crate returns plans/decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing proposals/votes/certs; hashing messages/slots; encrypting durable state at rest |
| `ids-store` | Primary record store (broadcast user) | Tags state-machine commands for atomic broadcast; delivers in slot order |
| `ids-replication` | Replica synchronization | Uses atomic broadcast to propagate updates to all replicas in the same order |
| `ids-fragment` | Record fragments/shards | Fragment writes ordered via atomic broadcast per shard group |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Profile-tagged broadcasts consult atomic-broadcast admission before ordering |
| `ids-session` (Phase 050) | Client-session binding | Session-bound broadcasts carry profile tag; audit cross-referenced with session audit |

Integration points: `ids-atomic-broadcast` drives public contracts declared in `ids-000-aa`; it does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-atomic-broadcast/` (a separate crate, not a folder inside an existing crate):

```
ids-atomic-broadcast/
├── Cargo.toml                          # Crate manifest; ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # Crate root, public API surface (abcast/adeliver)
├── errors.rs                           # AbcastError (thiserror): quorum, view, slot, integrity, recovery errors
├── types.rs                            # SlotId, ViewId, MessageId, BroadcastMsg, SlotEntry, QuorumCert, ViewStamp (re-exports ids-000-aa types)
├── constants.rs                        # Quorum defaults (N=2f+1, f+1), slot window, bounded-memory limits
├── feature_gates.rs                    # Feature flags: byzantine, recovery, audit, bounded_memory
│
├── reliable_broadcast/                 # Reliable broadcast foundation (Bracha double-echo)
│   ├── mod.rs
│   ├── echo.rs                         # ECHO phase: receive SEND → echo to all
│   ├── ready.rs                        # READY phase: quorum ECHOs → READY → quorum READYs → deliver-eligible
│   ├── integrity.rs                    # No-dup / no-creation checks per message
│   └── types.rs                        # ReliableBroadcastMsg, EchoState, ReadyState
│
├── total_order/                        # Total order layer (leader-sequenced slots)
│   ├── mod.rs
│   ├── sequencer.rs                    # Leader assigns slots, proposes messages
│   ├── slot.rs                         # Slot state machine: proposed → voted → committed → delivered
│   ├── vote.rs                         # Follower votes for slot proposals
│   ├── quorum_cert.rs                  # Quorum certificate (ed25519-signed) per slot
│   └── types.rs                        # SlotProposal, SlotVote, CommitCertificate
│
├── view_change/                        # View / term management and sequencer replacement
│   ├── mod.rs
│   ├── view.rs                         # View state machine, view number arithmetic
│   ├── viewstamp.rs                    # ⟨view, slot⟩ total ordering + gap detection
│   ├── election.rs                     # View change trigger, quorum state transfer, new sequencer install
│   └── types.rs                        # ViewChangeMsg, ViewState
│
├── delivery/                           # Ordered delivery to application
│   ├── mod.rs
│   ├── deliverer.rs                    # Delivers messages in ascending slot order; gap buffering
│   ├── gap_buffer.rs                   # Buffers out-of-order slots; ⊥ gap markers for slow peers
│   └── types.rs                        # DeliveryDecision, DeliveredMessage
│
├── recovery/                           # Crash recovery via WAL + stable storage
│   ├── mod.rs
│   ├── wal.rs                          # Write-ahead log of slot entries (commit-before-deliver)
│   ├── stable_storage.rs               # Stable write interface (fsync-equivalent marker)
│   ├── replay.rs                       # WAL replay on recovery; fetch missing prefix from survivor
│   └── types.rs                        # WalEntry, RecoverState
│
├── bounded_memory/                     # Bounded memory / backpressure for slow peers
│   ├── mod.rs
│   ├── inflight.rs                     # Bounded in-flight slots per peer
│   ├── backpressure.rs                 # Propagate backpressure to broadcaster
│   ├── gap_marker.rs                   # ⊥ void marker for unrecoverable lag
│   └── types.rs                        # PeerBudget, MemoryBound
│
├── audit/                              # Signed, hash-chained audit log
│   ├── mod.rs
│   ├── audit_entry.rs                  # Per-decision entry (echo/ready/propose/vote/commit/deliver/view)
│   ├── audit_chain.rs                  # sha256/blake3 hash chain
│   ├── checkpoint.rs                   # ed25519-signed periodic checkpoints
│   ├── signer.rs                       # Signing key handling (zeroize on shutdown)
│   └── verifier.rs                     # Offline verification of chain + checkpoints
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── broadcast_contract.rs           # trait Broadcast (reliable + atomic)
│   ├── ordering_contract.rs            # trait Ordering (slot assignment)
│   ├── quorum_contract.rs              # trait Quorum (quorum math, intersection)
│   ├── recovery_contract.rs            # trait Recovery (WAL/replay)
│   ├── delivery_contract.rs            # trait Delivery (ordered adeliver)
│   ├── view_contract.rs                # trait View (view numbers, view change)
│   ├── integrity_contract.rs           # trait Integrity (dedup, no-creation)
│   ├── carrier_contract.rs             # trait CarrierAdapter (opaque datagram framing)
│   ├── crypto_contract.rs              # trait CryptoSurface (allowed primitives only)
│   └── bounded_memory_contract.rs      # trait BoundedMemory (peer budgets, gap markers)
│
├── invariants/                         # Compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── uniform_agreement.rs            # Any delivery ⇒ all correct eventually deliver
│   ├── uniform_integrity.rs            # At most once; only if broadcast
│   ├── validity.rs                     # Correct broadcaster ⇒ eventual delivery
│   ├── total_order.rs                  # Same slot order at all nodes
│   ├── quorum_intersection.rs          # Q1 ∩ Q2 ≠ ∅ for safety
│   ├── viewstamp_monotonic.rs          # ⟨view,slot⟩ never decreases
│   ├── commit_before_deliver.rs        # WAL commit precedes adeliver
│   ├── carrier_agnostic.rs             # No std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305
│   └── bounded_memory.rs               # Per-peer memory bounds enforced
│
├── crypto_surface/                     # Type-level crypto wrappers (no ops here)
│   ├── mod.rs
│   ├── ed25519_signature.rs            # Signature([u8; 64])
│   ├── xchacha20poly1305_seal.rs       # AEAD envelope type
│   ├── chacha20poly1305_seal.rs        # AEAD envelope type
│   ├── sha256_digest.rs                # Digest([u8; 32])
│   └── blake3_digest.rs                # Digest([u8; 32])
│
├── tests/                              # Property/verification tests (~36 files)
│   ├── mod.rs
│   ├── reliable_broadcast_tests.rs
│   ├── total_order_tests.rs
│   ├── uniform_agreement_tests.rs      # Faulty-deliverer crash scenario
│   ├── view_change_tests.rs
│   ├── recovery_tests.rs               # WAL replay determinism
│   ├── bounded_memory_tests.rs         # ⊥ gap marker, backpressure
│   ├── crypto_allowlist_tests.rs
│   ├── dependency_audit_test.rs        # No prohibited crates
│   └── carrier_agnostic_tests.rs       # No std::net/tokio::net
│
├── benches/                            # (~8 files)
│   ├── mod.rs
│   ├── broadcast_latency.rs
│   ├── slot_commit_latency.rs
│   ├── view_change_latency.rs
│   ├── recovery_replay.rs
│   └── throughput.rs
│
├── docs/                               # (~16 files)
│   ├── architecture.md
│   ├── protocol_spec.md                # Uniform agreement / integrity / validity / termination
│   ├── view_change_spec.md
│   ├── recovery_spec.md
│   ├── bounded_memory_spec.md
│   ├── crypto_policy.md
│   ├── runbook.md
│   └── adding_a_view.md
│
└── test_vectors/                       # Deterministic inputs and expected outputs
    ├── slot_vectors.json
    ├── view_change_vectors.json
    └── recovery_vectors.json
```

Total: ~90–1000 files across `src/`, `src/reliable_broadcast/`, `src/total_order/`, `src/view_change/`, `src/delivery/`, `src/recovery/`, `src/bounded_memory/`, `src/audit/`, `src/contracts/`, `src/invariants/`, `src/crypto_surface/`, `tests/`, `benches/`, `docs/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

1. **Reliable broadcast (Bracha double-echo)** — SEND→ECHO→READY; quorum ECHOs trigger READY; quorum READYs make a message deliver-eligible; integrity: at most once, only if broadcast.
2. **Uniform agreement via quorum READY evidence** — Any delivery implies a quorum of READY certificates; that quorum intersects every future quorum, so every correct process learns of the delivery through view change / state transfer even if the delivering node crashes.
3. **Slot sequencer (Zab/Raft hybrid)** — Leader assigns monotonically increasing slots within a view; followers vote; quorum commit certificate (ed25519-signed) finalizes the slot; slot→message is durably WAL-logged before delivery.
4. **Viewstamp ordering** — `ViewStamp = ⟨view: u64, slot: u64⟩` totally orders messages; comparison is lexicographic (view first, then slot); gaps detected by missing slots between consecutive delivered viewstamps.
5. **View change** — On sequencer timeout, new view number = max(view) + 1; new sequencer collects log/commit evidence from f+1 (quorum) responses, installs new view, re-proposes uncommitted slots; viewstamps prevent reusing old slots.
6. **Crash recovery (WAL)** — Write-ahead log entry = ⟨view, slot, message_hash, certificate, commit_marker⟩; commit marker written to stable storage before any adeliver; recovery replays WAL suffix, fetches missing prefix from a survivor.
7. **Bounded memory / ⊥ gap marker** — Per-peer bounded in-flight slot window; when a peer's lag exceeds the bound, intervening slots are replaced with `⊥` void markers; peer catches up via state transfer after rejoining.
8. **Audit chain** — Each decision = sha256/blake3-chained entry; periodic ed25519-signed checkpoints; offline verifiable chain from genesis checkpoint.

---

### 8. Dependencies

`ids-atomic-broadcast/Cargo.toml`:
- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of broadcast envelopes, quorum certificates, view-change messages, audit entries.
- `thiserror` — error types.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. No async runtime (the crate is a pure decision/planning layer; the caller drives the carrier).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-070.1 | Uniform agreement: if any process (correct or faulty) delivers m, all correct processes eventually deliver m | Faulty-deliverer crash scenario test: 100% convergence at correct nodes |
| AC-070.2 | Uniform integrity: each message delivered at most once; only if previously broadcast | Dedup test: zero duplicate deliveries; forged-message rejection test |
| AC-070.3 | Validity: correct broadcaster's message eventually delivered by all correct processes | 100% delivery under no-fault, partial-synchrony conditions |
| AC-070.4 | Uniform total order: all correct nodes deliver messages in the same slot order | Pairwise order comparison across nodes; 100% agreement |
| AC-070.5 | Reliable broadcast foundation: echo/ready quorum phases implemented | Unit tests for echo→ready→deliver transitions |
| AC-070.6 | Quorum: N=2f+1 default, quorum f+1; any two quorums intersect | Quorum math unit tests; intersection property verified |
| AC-070.7 | View change: failed sequencer replaced; uncommitted slots re-proposed under new view | View change test with leader crash; no lost committed slots |
| AC-070.8 | Crash recovery: WAL replay restores committed slots; fetch missing prefix from survivor | Recovery determinism test; committed slots preserved after simulated crash |
| AC-070.9 | Bounded memory: per-peer in-flight slot bound; ⊥ gap marker for slow peers | Memory-bound test; gap marker emission; state transfer on rejoin |
| AC-070.10 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep passes |
| AC-070.11 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-070.12 | Every decision recorded in append-only, hash-chained, ed25519-signed audit log keyed by `NodeId` + `IdsTime` | Audit chain verification test (offline verifiable) |
| AC-070.13 | Commit-before-deliver: stable-storage commit marker precedes any adeliver | Recovery test: no delivered slot lost after crash |

---

### 10. Remaining Uncertainties

1. **Crash-tolerant vs Byzantine default:** N=2f+1 (quorum f+1) is the crash-tolerant default; a Byzantine extension (N=3f+1, quorum 2f+1, threshold signatures) is a design decision pending integration with `ids-crypto` threshold support.
2. **Quorum defaults per view:** concrete N and f values (e.g. N=3, f=1, quorum 2) are deployment-tuned; validated at integration with `ids-store`/`ids-replication`.
3. **View-change timeout value:** timeout trades false leader elections (too short) vs availability stalls (too long); to be calibrated per deployment.
4. **Randomized fallback for pure asynchrony:** FLP forbids deterministic termination under pure asynchrony; a Ben-Or-style randomized termination (w.p. 1) is noted but not specified.
5. **State transfer mechanism:** how a rejoining peer fetches missing WAL prefixes from a survivor (chunk size, authentication) is integration-level detail.
6. **`⊥` gap marker semantics:** application-side handling of void markers (skip vs. error) is a product decision.
7. **Stable storage abstraction:** `ids-atomic-broadcast` declares a stable-write interface but does not implement disk/flash specifics; the actual durable store crate is a later phase.
8. **Interaction with `ids-consistency-profile` (Phase 060):** profile-tagged broadcast admission handshake defined here, validated at integration.
9. **Slot window size for bounded memory:** the per-peer in-flight bound is workload-dependent; unquantified.
10. **Audit checkpoint periodicity:** frequency of ed25519-signed checkpoints trades audit size vs verification cost; pending.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-atomic-broadcast` (not a folder) | Follows "separate crates instead of more folders" directive; clear agreement/ordering boundary |
| Uniform agreement + uniform integrity (not just agreement/integrity) | Stronger safety: faulty-node deliveries still converge at all correct nodes; required by prompt ("uniform agreement") |
| Leader-sequenced slots + quorum commit (Zab/Raft hybrid) | Practical total order: single sequencer assigns slots, quorum certificates finalize; proven in Zab/ZooKeeper, Raft, Viewstamped Replication |
| Viewstamps = ⟨view, slot⟩ for total order | Detects gaps/duplicates across view changes; matches Viewstamped Replication practice |
| FLP circumvention via partial synchrony (not pure asynchrony) | Deterministic termination requires partial synchrony + eventual leader; safety holds under asynchrony |
| Crash-tolerant default N=2f+1, quorum f+1 | Minimal replica count for crash fault tolerance; Byzantine extension (N=3f+1) noted as future |
| WAL commit-before-deliver | No committed slot lost on crash; standard write-ahead log recovery |
| Bounded memory + ⊥ gap marker + backpressure | Bounded memory is impossible for pure atomic broadcast with slow peers; relaxed broadcast with void markers is the practical compromise |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |
| Logical view/slot numbers only; no wall-clock for ordering | Clock manipulation cannot break total order; `IdsTime` monotonic ticks for timeouts only |
| Carrier-agnostic boundary via `ids-carrier::Endpoint`; crate returns plans/decisions, never sends bytes | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; `ids-crypto` exposes primitive IDs via `ids-000-aa` | Complies with allowed-crypto rule |

---

### 12. Sources

1. ACM, "Total order broadcast and multicast algorithms: Taxonomy and survey" — validity, uniform agreement, uniform integrity, uniform total order. https://dl.acm.org/doi/10.1145/1041680.1041682
2. Défago et al., atomic broadcast properties (validity, uniform agreement, uniform integrity, uniform order). (Atomic Broadcasts and Consensus lecture material)
3. Bracha [Bra87], reliable broadcast double-echo algorithm; signature-free, t<n/3 Byzantine. https://arxiv.org/pdf/1510.06882
4. EPFL DC LAB, Reliable Broadcast lecture notes (BEB, RB, uniform reliable broadcast). https://dcl.epfl.ch/site/_media/education/file02_da24-reliablebroadcast.pdf
5. tropars DS-6, Atomic Broadcast lecture notes (uniform agreement vs agreement; uniform total order). https://tropars.github.io/downloads/lectures/DS/DS-6-atomic_broadcast.pdf
6. arxiv 2001.03244, Uniform Reliable Broadcast (validity, integrity, termination including faulty receivers). http://arxiv.org/pdf/2001.03244v1
7. Jalote, Chapter 4 — Broadcast (reliable vs atomic vs causal; centralized sequencer rotation). https://engineering.purdue.edu/ee695b/public-web/handouts/References/Chapt%204%20-%20Broadcast%20-%20jalote.pdf
8. Zab: High-performance broadcast for primary-backup systems (ZooKeeper atomic broadcast; quorum; crash recovery). https://ieeexplore.ieee.org/document/5958223
9. Paxos / Flexible Paxos — quorum intersection Q1+Q2>N; majority quorums. https://arxiv.org/pdf/2203.03058; https://web.eecs.umich.edu/~manosk/assets/papers/flexible_paxos_opodis2016.pdf
10. Raft — leader/follower/candidate; AppendEntries; term-based voting; at most one winner per term. https://cs.princeton.edu/courses/archive/spring21/cos418/docs/L14-raft.pdf
11. Viewstamped Replication (Oki/Liskov, PODC 1988) and Revisited (Liskov/Cowling) — views, viewstamps ⟨view,op-number⟩, recovery via f+1 responses. https://cs.princeton.edu/courses/archive/fall11/cos518/papers/viewstamped.pdf; https://classes.cs.uchicago.edu/archive/2026/spring/23380-1/papers/oki_viewstampedreplication.pdf
12. FLP Impossibility (Fischer/Lynch/Paterson, JACM 1985) — no deterministic consensus in pure asynchrony with one crash. https://people.cs.umass.edu/~arun/cs677/notes/FLP.pdf
13. Ben-Or randomized consensus — termination with probability 1 under asynchrony, f<n/2. https://ecommons.cornell.edu/bitstreams/f0479319-5d87-4637-8bc6-0b45d139c290/download
14. Chandra–Toueg, unreliable failure detectors; consensus ≡ atomic broadcast reducibility. https://cs.utexas.edu/~lorenzo/corsi/cs380d/papers/p225-chandra.pdf
15. PBFT (Castro/Liskov) — N=3f+1 Byzantine atomic broadcast; three-phase primary-backup. https://css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf
16. HotStuff — pipelined secure broadcast; leader replacement; DAG-based BFT. https://arxiv.org/pdf/2305.13556
17. Acuerdo — fast atomic broadcast over RDMA; n=2f+1 crash quorum f+1. https://doi.org/10.1145/3545008.3545041
18. BullShark / DAG-Rider — DAG-based asynchronous Byzantine atomic broadcast; O(n) amortized. https://arxiv.org/html/2201.05677v3
19. Relaxed Atomic Broadcast: State-Machine Replication Using Bounded Memory — slow peer ⊥ gap marker; bounded memory impossibility. https://ieeexplore.ieee.org/document/5283543
20. Abortable broadcast — bounded memory under backpressure; malicious-peer DoS prevention. https://arxiv.org/pdf/2410.22080v1
21. Chop Chop — Byzantine atomic broadcast; Ed25519 quorum certificates; legitimacy proofs. https://arxiv.org/pdf/2304.07081v2
22. MIT 6.828 crash lecture — write-ahead log, commit marker, recovery replay. https://pdos.csail.mit.edu/6.828/2021/lec/l-crash.txt
23. ed25519-dalek docs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
24. chacha20poly1305 docs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
25. sha2 RustCrypto — SHA-256. https://docs.rs/sha2
26. BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
27. IDS project documents at C:/idfs: CRATES.md (crate inventory, ids-000-aa master, ids-carrier Endpoint, ids-crypto allowed set), IDS.md, prompts.txt entry 70 (lines 1045-1059), plans/plan-001.md, plan-048.md, plan-060.md (IDS plan format references)
28. HX-241 reference plan format — C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md
29. Existing ids-000-aa skeleton — C:/idfs/ids-000-aa/ (types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml)

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 070 (ids-atomic-broadcast).