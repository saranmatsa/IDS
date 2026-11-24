## Phase 068 — Create the ids-total-order-broadcast crate (uniform agreement on message delivery). Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1015-1029)

```
68. Research only do not read plan.md **[Phase 068 — Create the ids-total-order-broadcast crate: All nodes deliver messages in same order.]**

Crate: ids-total-order-broadcast
Purpose: All nodes deliver messages in same order.
Contents: Uniform total order, atomic broadcast, consensus-based total order, FIFO-total order, state machine replication integration.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 068 — Create the ids-total-order-broadcast crate: All nodes deliver messages in same order.]**

Create plan-068.md in C:/idfs/plans/ for the ids-total-order-broadcast crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 068 creates `ids-total-order-broadcast`, the IDS crate that delivers uniform agreement on message delivery: every correct node delivers the same set of messages in the same order (total order), and if any node delivers a message then every correct node eventually delivers it (uniform agreement). It is a **separate crate** (not a folder inside an existing crate, per project rule). It is carrier-agnostic: all traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes); the crate never opens a socket, imports no networking crate, and never references `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

`ids-total-order-broadcast` owns: total-order assignment (sequence numbers), uniform agreement delivery, atomic broadcast primitives (broadcast/deliver), validity, uniform integrity (at-most-once, only if previously broadcast), FIFO-total order option, consensus-based ordering integration (sequencer / leader-based / quorum-based), state-machine replication hook (deliver to deterministic state machine in the same order at every node), duplicate suppression, crash-recovery with stable storage + checkpoint, epoch/view change on leader failure, and an audit chain (sha256/blake3 chain + ed25519-signed checkpoints). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — consumed via `ids-crypto` primitive IDs from `ids-000-aa`, never implemented here.

A skeleton `ids-total-order-broadcast` crate does NOT yet exist at `C:/idfs/ids-total-order-broadcast/`. `ids-000-aa` is the master crate holding canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `Error`) and sealed trait contracts; this crate imports them and never redefines them. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned the atomic broadcast / total order broadcast four-property definition (Validity, Uniform Agreement, Uniform Integrity, Uniform Total Order) from Hadzilacos & Toueg and ACM taxonomy survey; equivalence of atomic broadcast and consensus (FLP impossibility applies: deterministic consensus impossible in asynchronous systems with one crash); Zab (ZooKeeper atomic broadcast: leader + quorum + recovery); Viewstamped Replication (Oki/Liskov; viewstamp = ⟨view-number, op-number⟩; primary assigns op-number = total order; view-change on primary failure; tolerates f crashes with N ≥ 2f+1); Multi-Paxos / Raft log replication (leader assigns consecutive sequence numbers; commit on majority; state machine replication); PBFT 3-phase total order broadcast (3f+1 replicas, pre-prepare/prepare/commit); DAG-BFT / DAG-Rider (dissemination layer reliably broadcasts blocks in rounds; ordering layer derives total order from DAG leaders via perfect coin); Lamport logical clocks (partial order; total order by tie-breaking with process ID); Hybrid Logical Clock (total order with bounded physical-skew); quorum intersection R + W > N (read/write quorum overlap guarantees latest-read); logic-only Rust consensus crates (`raft-lite`, `raft-consensus`, `simple-raft-rs`, `hotstuff_rs`) that produce `Vec<u8>` protocol messages with no networking — ideal carrier-agnostic model; allowed crypto specs (RFC 8032 Ed25519, RFC 7539 ChaCha20-Poly1305, XChaCha20-Poly1305 192-bit nonce libsodium, NIST FIPS 180-4 SHA-256, BLAKE3 256-bit SIMD).

---

### 2. Architecture Overview

`ids-total-order-broadcast` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), `ids-crypto` (allowed primitives), and `ids-time` (monotonic ticks). It is an ordering/agreement layer: given a broadcast request it assigns a total-order sequence number, runs agreement with the other nodes, and delivers messages to the local state machine in that exact order. It never sends bytes itself; it returns protocol `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint` targets that the caller (or the carrier adapter) transmits.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    CALLER LAYER (ids-* broadcast users)                      │
│  ids-state-machine, ids-replication, ids-fragment — TO-broadcast(message)   │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 068: ids-total-order-broadcast (THIS CRATE — ordering)   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌────────────┐ │
│  │ sequencer/      │ │ agreement       │ │ delivery        │ │ state-machine│ │
│  │ leader-orderer  │ │ (quorum/consensus)│ │ buffer + dedup  │ │ hook       │ │
│  │ (assigns seqno) │ │ (uniform agree) │ │ (seqno-ordered) │ │ (deliver in │ │
│  └─────────────────┘ └─────────────────┘ │ same order)     │ │ order)     │ │
│  ┌─────────────────┐ ┌─────────────────┘ └─────────────────┘ └────────────┘ │
│  │ stable storage  │ │ epoch/view      │ │                 │                │ │
│  │ (checkpoint,    │ │ change on       │ │                 │                │ │
│  │  recovery log)  │ │ leader failure  │ │                 │                │ │
│  └─────────────────┘ └─────────────────┘ │                 │                │ │
│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │
│  │ audit chain     │ │ duplicate       │ │                 │                │ │
│  │ (sha256/blake3  │ │ suppression     │ │                 │                │ │
│  │  chain + ed25519│ │ (msg-id set)    │ │                 │                │ │
│  │  checkpoints)   │ │                 │ │                 │                │ │
│  └─────────────────┘ └─────────────────┘ │                 │                │ │
└─────────────────────────────────────────┬──────────────────┴──────────────┘
                                          │ opaque Vec<u8> datagrams
                                          ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                      │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh                                 │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one broadcast:** caller invokes `TO-broadcast(m)` → orderer assigns `seqno = next_sequence()` and stamps message `{msg_id, sender: NodeId, seqno, payload}` → message sent as opaque datagram to `ids-carrier::Endpoint` targets → agreement protocol collects acks/quorum → once ordered and agreed, `TO-deliver(m)` delivered to local state machine → stable-storage checkpoint + audit-chain entry (sha256/blake3 chain + ed25519 signature) → duplicate suppressed on retransmission by `msg_id` set. FIFO-total order variant preserves per-sender send order within the global total order.

**Uniform agreement properties (research-verified):**
- **Validity:** if a correct node broadcasts m, it eventually TO-delivers m.
- **Uniform Agreement:** if any node (correct or faulty) TO-delivers m, all correct nodes eventually TO-deliver m.
- **Uniform Integrity:** every node TO-delivers m at most once, and only if m was previously TO-broadcast by sender(m).
- **Uniform Total Order:** if nodes p and q both TO-deliver m and m′, then p delivers m before m′ iff q delivers m before m′.

**Equivalence to consensus:** TO broadcast solvable ⇒ consensus solvable (broadcast initial values; decide first delivered value). Therefore FLP impossibility applies: no deterministic TO broadcast guarantees termination in a fully asynchronous system with even one crash. Practical designs evade FLP via partial synchrony (timeouts), failure detectors, or randomization — reflected in this plan's epoch/view-change and leader-failover design.

---

### 3. Detailed Research Dimensions

#### 3.1 Atomic broadcast four properties (Hadzilacos & Toueg / ACM taxonomy)

Sources: ACM DL 10.1145/1041680.1041682 (Total order broadcast and multicast algorithms: Taxonomy and survey); Wikipedia Atomic broadcast; TUM lecture DS-6 atomic broadcast.

Formal primitives: `TO-broadcast(m)` / `TO-deliver(m)`. Every message uniquely identified with sender identity. Properties:
- **Validity:** correct broadcaster eventually TO-delivers its own message.
- **Uniform Agreement:** any delivery ⇒ all correct nodes eventually deliver.
- **Uniform Integrity:** at-most-once delivery; only previously broadcast messages.
- **Uniform Total Order:** all nodes agree on relative order of any two delivered messages.

`ids-total-order-broadcast` implements exactly these four properties; uniform agreement (vs non-uniform agreement) is the IDS requirement — a faulty node's delivery must still propagate to all correct nodes.

#### 3.2 Equivalence to consensus; FLP impossibility

Sources: Chandra & Toueg unreliably failure detectors paper (Consensus and Atomic Broadcast reducible to each other); TUM DS-6 lecture; arXiv 2607.24095 (Consensus In Asynchrony); FLP 1985.

Consensus ⇔ atomic broadcast reduction:
- Consensus from broadcast: every process broadcasts its input; decides the first value TO-delivered (all decide same by total order).
- Broadcast from consensus: run one consensus instance per log slot to assign sequence numbers.

FLP: no deterministic protocol solves consensus (hence atomic broadcast) with guaranteed termination in a fully asynchronous system with one crash. Practical escape hatches: partial synchrony + timeouts (Zab/Raft/Paxos), failure detectors (Chandra-Toueg ♦S, ♦P), or randomization (Ben-Or, DAG-BFT perfect coin). `ids-total-order-broadcast` adopts partial synchrony with epoch/view change and bounded retry; termination is "eventual under synchrony," not guaranteed under asynchrony — documented as an uncertainty.

#### 3.3 Sequencer / leader-based ordering (single sequencer assigns sequence numbers)

Sources: TUM DS-6 lecture (sequencer assigns seqno, sends (m, i) to all); JavaGroups TOTAL protocol design (sequencer-based total order); Weave / Multi-Ring Paxos / CORFU geo-replication papers (central sequencer assigns log slots).

Simplest TO broadcast: a designated sequencer `s` receives broadcasts, assigns monotonically increasing sequence numbers, and multicasts `(m, seqno)` to all. Recipients buffer and deliver in seqno order. Single point of ordering (low latency, simple), single point of failure → mitigated by sequencer failover via epoch/view change (Section 3.6). `ids-total-order-broadcast` supports pluggable orderer strategies: `SequencerOrderer` (single sequencer), `LeaderOrderer` (rotating leader, Zab/Raft style), `QuorumOrderer` (quorum-assigned sequence numbers).

#### 3.4 Consensus-based total order: Zab, Viewstamped Replication, Multi-Paxos/Raft, PBFT

Sources: Zab (IEEE 5958223; ZooKeeper primary-backup; recovery on primary crash; quorum support for primary); Viewstamped Replication (Oki/Liskov PODC 1988; viewstamp ⟨view, op-number⟩; primary assigns op-number; view-change on failure; N ≥ 2f+1); VR Revisited (Liskov/Cowling 2012); MIT 6.824 / Princeton COS418/518; Raft (leader log replication, commit on majority, state machine replication); Multi-Paxos (log = sequence of consensus instances); PBFT (Castro/Liskov SOSP 1999; 3-phase pre-prepare/prepare/commit; 3f+1 replicas; Byzantine).

- **Zab:** primary receives all client requests, propagates transactions as Zab broadcast; on primary crash, recovery agrees on common consistent state and elects new primary (quorum support). `ids-total-order-broadcast` Zab-style: leader assigns epoch + sequence, followers ack, commit on quorum, recovery reconciles logs before new leader.
- **VR:** view = leadership period; viewstamp = ⟨view-number, op-number⟩ totally ordered; primary assigns op-numbers; view change requires quorum of DoViewChange; new primary adopts most up-to-date log.
- **Multi-Paxos/Raft:** replicated log; each entry = (index, term, command); committed on majority; all state machines execute same entries in same order.
- **PBFT:** Byzantine variant; 3f+1; client → primary pre-prepare (seqno), prepare phase (consistent ordering within view), commit phase; view change on primary misbehavior.

IDS choice: crash-fault-tolerant (CFT) leader-based (Zab/Raft/VR family) as default; Byzantine option (PBFT family) as a feature-gated variant using ed25519 signatures. Both map to the same `ids-total-order-broadcast` broadcast/deliver interface.

#### 3.5 FIFO-total order vs causal total order vs total order

Sources: Yale Aspnes Broadcast wiki (single-source FIFO vs totally ordered vs causally ordered); EPFL DA total-order-broadcast slides (reliable vs causal vs total order; total order need not respect causality); Cornell CS5414 Dynamo notes (sloppy quorum, hinted handoff).

- **Single-source FIFO:** messages from one sender delivered in send order. Weakest.
- **Causal broadcast:** if m1 causally precedes m2, all deliver m1 before m2. Partial order; different nodes may deliver concurrent messages in different orders.
- **Total order broadcast:** all nodes deliver all messages in one identical order; no constraint on relation to send/causal order (can be layered on top: FIFO-total = total order that also respects per-sender FIFO; causal-total = total order extending causality).

`ids-total-order-broadcast` implements total order; FIFO-total order is a configuration flag (per-sender sequence preserved inside global order). Causal ordering is delegated to `ids-causal-broadcast` (Phase 069) layering on top.

#### 3.6 Leader election, epochs, view change, crash recovery

Sources: VR / VR Revisited (view change; StartViewChange/DoViewChange/StartView; new primary adopts most up-to-date log); Zab recovery (agree on consistent state before new primary); Raft leader election (term, vote, majority); DAG-BFT seamless reconfiguration (multi-configuration, well-spaced transitions).

Design: monotonically increasing `Epoch` (u64) + `ViewId`. Leader of epoch e is deterministic (e.g., `e % N`). On leader timeout, nodes increment epoch, broadcast view-change with their log state; new leader elected by quorum, adopts the log with the highest (epoch, last_seqno), reconciles missing entries, then resumes ordering. Stable storage: last delivered seqno, epoch, committed log prefix persisted before ack (crash recovery). Checkpoint + garbage collection: once a prefix is delivered at all nodes (quorum-acknowledged stable checkpoint), older log entries and buffered messages may be garbage collected.

#### 3.7 Logical clocks for ordering: Lamport timestamps, Hybrid Logical Clocks

Sources: Princeton COS418 Lamport timestamps (total order by (clock, pid)); UCSB/Rochester logical clocks (totally ordered multicast with queue + ack-from-all); Kulkarni HLC paper (cse.buffalo.edu tech report 2014-04); sergeiturukin HLC explainer; CS StackExchange Lamport vs vector clocks; IETF YANG-Push draft (HLC adoption).

- **Lamport timestamp:** per-node counter incremented on events; message tagged; receiver sets `max(local, received)+1`. Gives causal-consistent partial order; tie-break by node ID → total order. Compact (O(1) per message, independent of N).
- **Hybrid Logical Clock:** (physical, logical) pair; close to wall-clock (bounded skew epsilon) + causal ordering; O(1) state; used in CockroachDB/MongoDB.

`ids-total-order-broadcast` assigns sequence numbers via the leader/sequencer (the authoritative total order), and uses Lamport timestamps or HLC as tie-breakers / causal metadata inside messages (consumed from `ids-time`/`ids-000-aa` types). Wall-clock is never used for ordering decisions (monotonic `IdsTime` only).

#### 3.8 State machine replication integration

Sources: Schneider SMR (1990); arXiv 1904.04098 (SMR = same initial state + total-order broadcast + deterministic execution); arXiv 2203.05681 (SMR scalability, epochs/segments); arxiv 2407.01720 (linearizability vs SMR).

SMR contract: all replicas start identical; every replica receives the same client requests in the same total order; deterministic execution ⇒ identical state transitions ⇒ identical responses. `ids-total-order-broadcast` exposes `deliver_to_state_machine(seqno, message)` hook; the caller's state machine applies deliveries in seqno order. Determinism requirement is on the caller (ids-state-machine), not this crate.

#### 3.9 Duplicate suppression, integrity, at-most-once

Sources: atomic broadcast definitions (uniform integrity: at-most-once, only if broadcast); PBFT no-duplication (same seqno ⇒ same request); DAG-BFT integrity.

Mechanism: every broadcast carries `msg_id` (unique, e.g., blake3(sender || counter) or ed25519-signed nonce). Each node keeps a bounded `DeliveredMsgId` set (recent window, checkpoint-evicted). Redelivery of a known `msg_id` is suppressed; a message never broadcast by its claimed sender is rejected (integrity). Signed broadcasts (ed25519) give sender authentication; unsigned broadcasts rely on carrier-layer trust.

#### 3.10 Stable storage, checkpoint, garbage collection

Sources: Pilotfish checkpoint garbage collection (quorum of CheckpointedMessage ⇒ delete prior checkpoints/buffered messages); VR recovery (disk write before state change); Zab recovery (stable log).

Design: each node persists `(epoch, last_seqno, committed_prefix_hash)` to stable storage before acknowledging a commit. Periodic checkpoints (sha256/blake3 hash of state-machine state + seqno) are broadcast; once a checkpoint is quorum-stable, all log entries and buffered messages ≤ checkpoint seqno are garbage collected. Recovery on restart: load last checkpoint, replay committed log entries from stable storage or fetch missing from peers via `ids-carrier::Endpoint` datagrams.

#### 3.11 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

Sources: RFC 8032 (Ed25519/EdDSA); RFC 7539 (ChaCha20-Poly1305, 96-bit nonce); libsodium XChaCha20-Poly1305 (192-bit nonce, random nonces safe); NIST FIPS 180-4 (SHA-256); BLAKE3 spec (256-bit, SIMD, keyed hash, KDF, XOF).

Usage in `ids-total-order-broadcast`:
- **ed25519:** sign broadcast messages (sender authentication, integrity), sign audit checkpoints, sign view-change/epoch messages.
- **sha256:** message IDs (blake3 preferred for speed; sha256 where NIST-conformant hashing required), audit-chain links.
- **blake3:** message dedup IDs, content addressing of log entries, fast checkpoint hashing (preferred for speed/parallelism).
- **xchacha20poly1305:** encrypt broadcast payloads at rest/in-transit confidentiality (24-byte random nonce safe per message).
- **chacha20poly1305:** encrypt session/state payloads (12-byte counter nonce, bounded use).

No other crypto. `ids-crypto` is the sole crypto surface; this crate references primitive IDs from `ids-000-aa`.

#### 3.12 Quorum-based agreement and fault tolerance

Sources: quorum R + W > N (thecodeforge, medium double-pointer, arxiv 2002.06098, deepengineering staleness SLOs); PBFT quorum (2f+1 of 3f+1); VR quorum (majority, f+1).

Fault model: crash-stop / crash-recovery, f < N/2 (majority quorums intersect). Write/ack quorum: a sequence number is committed when a quorum (⌊N/2⌋+1) of nodes have stored it. Read/deliver quorum intersects write quorum ⇒ delivered order is consistent. Byzantine option: 3f+1, PBFT quorums (2f+1), ed25519 signatures to filter equivocation.

#### 3.13 Batching and pipelining for throughput

Sources: "Optimizing Paxos with batching and pipelining" (EPFL TCS); MultiPaxos Made Complete (arxiv 2405.11183, concurrent handlers); Weave geo-replication.

Batching: multiple client messages batched under one sequence number / one consensus instance; amortizes agreement cost. Pipelining: multiple outstanding consensus instances without waiting for commit; increases throughput under WAN latency. `ids-total_order_broadcast` exposes configurable batch size and pipeline depth; batch boundary is an audit event.

#### 3.14 Logic-only Rust consensus crates as carrier-agnostic reference

Sources: docs.rs raft-lite (logic-only Raft, messages are `Vec<u8>`, guarantees total-order delivery); docs.rs raft-consensus (logic-only, no networking, caller passes messages); simple-raft-rs (no_std, alloc, returns `Vec<u8>` unicast messages to retransmit); hotstuff_rs (BFT SMR in Rust, BlockTree).

These crates confirm the carrier-agnostic pattern: consensus logic is pure functions over `Vec<u8>` messages; the caller is responsible for sending/receiving over `ids-carrier::Endpoint`. `ids-total-order-broadcast` follows the same pattern — pure ordering logic over opaque datagrams, no networking imports.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Two nodes deliver different orders | Total order correctness | Single leader/sequencer assigns seqno; quorum commit; uniform total order property |
| Message delivered twice | Uniform integrity | `msg_id` dedup set; bounded replay cache; at-most-once |
| Delivered message never broadcast | Uniform integrity | Sender authentication (ed25519); reject unsigned/unverified |
| Correct node's broadcast never delivered | Validity / Uniform agreement | Retransmission until quorum ack; recovery fetches missing log entries |
| Leader crash stalls ordering | Liveness | Epoch/view change; new leader by quorum; reconciles log before resuming |
| Asynchronous network (FLP) | Termination guarantee | Partial synchrony + timeouts; termination eventual under synchrony (documented) |
| Replay of old broadcast | Replay attack | Epoch + seqno window; dedup set rejects stale seqnos |
| Leader equivocates (two seqnos for one slot) | Order safety | Quorum intersection guarantees at most one value per slot; signed proposals |
| Clock manipulation for order | Causality/total order | Leader-assigned seqno (not wall-clock); Lamport/HLC as metadata only; monotonic IdsTime |
| Carrier duplicates/delays/reorders datagrams | Ordering/recovery | Seqno-based rebuffering; dedup; idempotent delivery |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |
| Signing key extraction | Message authenticity | Keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Prohibited networking crate sneaks in | Carrier-agnostic rule | Dependency audit (CI `cargo tree`); crate imports no std::net/tokio::net/libp2p |
| Disallowed crypto algorithm | Crypto policy | Only five allowed primitives; CI allowlist test |

The carrier is never trusted: order, agreement, and audit entries are signed/sequenced over bytes, surviving any carrier re-framing, reordering, or duplication.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-total-order-broadcast ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All TO broadcast traffic flows to/from `Endpoint`; crate returns datagrams, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing messages/checkpoints; encrypting payloads; hashing message IDs and audit chain |
| `ids-time` | Monotonic `IdsTime` ticks | Timeouts for leader election; never wall-clock for ordering |
| `ids-state-machine` (later phase) | Deterministic state machine applying deliveries | `deliver_to_state_machine(seqno, message)` hook; applies in total order |
| `ids-replication` | Replica synchronization | May build on TO broadcast for log replication |
| `ids-causal-broadcast` (Phase 069) | Causal ordering layer | Can layer causal guarantees on top of total order |
| `ids-consistency-profile` (Phase 060) | Read/write quorum profiles | Quorum config (R/W/N) feeds TO broadcast commit quorum |

**Integration points:** `ids-total_order_broadcast` exposes `TO-broadcast(message) → Vec<(Endpoint, Vec<u8>)>` datagrams to send and `on_datagram(endpoint, payload) → Option<TO-deliver(message)>` to receive. It never touches a socket or a networking crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-total-order-broadcast` (new crate at `C:/idfs/ids-total-order-broadcast/`; separate crate, not a folder):

```
ids-total-order-broadcast/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-time, serde, thiserror, blake3
├── lib.rs                              # crate root, feature gates, public API (broadcast/deliver)
├── constants.rs                        # Protocol constants: MAX_BATCH, MAX_PIPELINE, DEDUP_WINDOW, CHECKPOINT_INTERVAL
├── error.rs                            # TOError (thiserror): ordering, agreement, integrity, recovery, crypto errors
├── features.rs                         # Feature flags: byzantine, batching, pipeline, stable-storage, audit
├── types.rs                            # TOMessage, SeqNo, Epoch, ViewId, MsgId, Delivery, BroadcastRequest (re-exports ids-000-aa types)
│
├── orderer/                            # Total-order assignment strategies
│   ├── mod.rs
│   ├── sequencer.rs                    # Single sequencer assigns monotonically increasing seqno
│   ├── leader.rs                       # Rotating leader (Zab/Raft/VR style) assigns seqno
│   ├── quorum.rs                       # Quorum-assigned sequence numbers
│   └── strategy.rs                     # Pluggable OrdererStrategy trait
│
├── agreement/                          # Uniform agreement protocol
│   ├── mod.rs
│   ├── commit.rs                       # Quorum commit on seqno; commit certificate
│   ├── quorum_calc.rs                  # Quorum size from N; intersection check (R+W>N)
│   ├── ack_collector.rs                # Collects acks per (epoch, seqno)
│   └── view_change.rs                  # Epoch/view change on leader timeout; log reconciliation
│
├── delivery/                           # Ordered delivery to state machine
│   ├── mod.rs
│   ├── buffer.rs                       # Out-of-order message buffer keyed by seqno
│   ├── sequencer.rs                    # Delivers contiguous seqno prefix to state machine
│   ├── dedup.rs                        # msg_id dedup set (bounded window)
│   ├── integrity.rs                    # Verify message was broadcast by claimed sender
│   └── fifo.rs                         # Per-sender FIFO-total order option
│
├── stable_storage/                     # Crash recovery: checkpoint + log
│   ├── mod.rs
│   ├── checkpoint.rs                   # sha256/blake3 hash of state at seqno
│   ├── log.rs                          # Persistent committed log prefix
│   ├── gc.rs                           # Garbage collect entries ≤ stable checkpoint
│   └── recovery.rs                     # Restore from checkpoint; fetch missing entries from peers
│
├── audit/                              # Signed hash-chained audit log
│   ├── mod.rs
│   ├── entry.rs                        # Audit entry: (seqno, msg_id, hash, prev_hash)
│   ├── chain.rs                        # sha256/blake3 chain
│   ├── checkpoint.rs                   # ed25519-signed checkpoint
│   ├── signer.rs                       # ed25519 signing
│   └── verifier.rs                     # Offline verification of chain + signatures
│
├── crypto_surface/                     # Allowed-crypto allowlist (type-level only)
│   ├── mod.rs
│   ├── ed25519.rs                      # Sign/verify broadcast + checkpoints
│   ├── xchacha20.rs                    # XChaCha20-Poly1305 encrypt/decrypt payload
│   ├── sha256.rs                       # SHA-256 hashing
│   ├── blake3.rs                       # BLAKE3 hashing (message IDs, checkpoint hash)
│   └── chacha20.rs                     # ChaCha20-Poly1305 encrypt/decrypt payload
│
├── contracts/                          # Sealed trait contracts (~12 files)
│   ├── mod.rs
│   ├── broadcast_contract.rs           # trait Broadcast (TO-broadcast / TO-deliver)
│   ├── orderer_contract.rs             # trait Orderer (assign seqno)
│   ├── agreement_contract.rs           # trait Agreement (quorum commit)
│   ├── delivery_contract.rs            # trait Delivery (ordered delivery)
│   ├── storage_contract.rs             # trait StableStorage (checkpoint/recover)
│   ├── audit_contract.rs               # trait Audit (append/verify)
│   ├── crypto_contract.rs              # trait CryptoSurface (allowed primitives only)
│   ├── carrier_contract.rs             # trait CarrierAdapter (datagrams to Endpoint)
│   ├── view_change_contract.rs         # trait ViewChange (epoch transition)
│   ├── fifo_contract.rs                # trait FIFOOrder (per-sender order)
│   ├── batching_contract.rs            # trait Batching (batch boundary)
│   └── mod.rs
│
├── invariants/                         # Protocol invariants (~10 files)
│   ├── mod.rs
│   ├── uniform_agreement.rs
│   ├── uniform_integrity.rs
│   ├── uniform_total_order.rs
│   ├── validity.rs
│   ├── at_most_once.rs
│   ├── quorum_intersection.rs
│   ├── carrier_agnostic.rs
│   ├── crypto_allowlist.rs
│   ├── checkpoint_sound.rs
│   └── mod.rs
│
├── tests/                              # Property/verification tests (~30 files)
├── benches/                            # (~10 files)
├── docs/                               # (~16 files)
└── spec/                               # Normative specifications (~6 files)
```

Total: ~90–1000 files across `src/`, `src/orderer/`, `src/agreement/`, `src/delivery/`, `src/stable_storage/`, `src/audit/`, `src/crypto_surface/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, `spec/` — one file per responsibility, consistent with the "separate crates instead of more folders" directive applied intra-crate.

---

### 7. Key Algorithms & Data Structures

1. **Total-order sequence assignment** — `Orderer::next_seqno(&self, msg_id) -> SeqNo`: monotonically increasing per epoch; leader/sequencer assigns; stored with message before broadcast.
2. **Uniform agreement (quorum commit)** — `commit(epoch, seqno)` when quorum (⌊N/2⌋+1) acks; commit certificate = set of ack signatures (ed25519 in Byzantine mode).
3. **Uniform total order** — all nodes deliver messages in ascending `(epoch, seqno)` order; out-of-order messages buffered until prefix is contiguous.
4. **FIFO-total order** — per-sender send sequence preserved inside the global total order (sender seqno + global seqno).
5. **Duplicate suppression** — `DedupSet<MsgId>` bounded sliding window; redelivery of known `msg_id` suppressed; checkpoint evicts old entries.
6. **Integrity check** — deliver only if `verify_signature(sender, msg_id, payload)` or unsigned-but-carrier-trusted; reject messages never broadcast by claimed sender.
7. **View change / leader failover** — on timeout: `epoch += 1`; broadcast view-change with `(epoch, last_seqno, log_hash)`; new leader (deterministic `epoch % N`) elected by quorum, adopts log with highest `(epoch, last_seqno)`, reconciles missing entries.
8. **Stable storage + checkpoint** — persist `(epoch, last_seqno, committed_hash)` before ack; periodic sha256/blake3 checkpoint; quorum-stable checkpoint ⇒ GC log entries ≤ checkpoint.
9. **Recovery** — load last checkpoint; replay committed log from stable storage; fetch missing entries via `ids-carrier::Endpoint` datagrams to peers.
10. **Audit chain** — entry = `(seqno, msg_id, blake3(payload), prev_hash)`; chain = sha256/blake3 hash chain; periodic ed25519-signed checkpoint; offline verifiable.
11. **Batching + pipelining** — batch boundary = one audit event; pipeline depth = outstanding uncommited instances; configurable.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — datagram transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-time` — monotonic ticks for leader-election timeouts.
- `serde` — serialization of broadcast envelopes, commit certificates, audit entries.
- `thiserror` — error types.
- `blake3` — message-ID and checkpoint hashing.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-total-order-broadcast` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-68.1 | Total order: all nodes deliver same messages in same order | 100% of delivered seqno sequences identical across nodes |
| AC-68.2 | Uniform agreement: any delivery ⇒ all correct nodes eventually deliver | Delivery propagation test; 100% delivery under no-partition |
| AC-68.3 | Uniform integrity: at-most-once, only if broadcast | Dedup test; zero duplicate deliveries; reject unsigned/unverified |
| AC-68.4 | Validity: correct broadcaster eventually self-delivers | Self-delivery test |
| AC-68.5 | FIFO-total order option: per-sender send order preserved | Per-sender seqno monotonic within global order |
| AC-68.6 | View change: leader failover elects new leader by quorum | Kill leader; new leader elected; delivery resumes |
| AC-68.7 | Crash recovery: restart from checkpoint + replay | Restore test; state matches pre-crash |
| AC-68.8 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/etc. |
| AC-68.9 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-68.10 | Audit chain: sha256/blake3 chain + ed25519-signed checkpoints, offline verifiable | Verifier test passes on generated chain |
| AC-68.11 | Quorum commit: commit on ⌊N/2⌋+1 acks; intersection guarantees consistency | Quorum calc test |
| AC-68.12 | Batching/pipelining configurable; batch boundary = audit event | Config test |
| AC-68.13 | Logic over `Vec<u8>` datagrams only; no networking imports | Dependency audit test (`cargo tree`) passes |
| AC-68.14 | Compiles cleanly: `cargo check -p ids-total-order-broadcast` exits 0 | Build test |

---

### 10. Remaining Uncertainties

1. **Orderer strategy default:** sequencer vs leader vs quorum — default to leader-based (Zab/Raft) for fault tolerance; sequencer for low-latency single-sequencer deployments; validated at integration.
2. **Byzantine mode:** PBFT-style 3f+1 with ed25519 is feature-gated; default is CFT (crash-fault-tolerant) majority quorums.
3. **Quorum size formula:** ⌊N/2⌋+1 majority vs configurable R/W-style quorum from `ids-consistency-profile` (Phase 060); to be aligned at integration.
4. **Checkpoint interval & GC policy:** checkpoint frequency and garbage-collection aggressiveness are workload-dependent; unquantified.
5. **Dedup window size:** bounded `DeliveredMsgId` window trades memory vs duplicate-detection range; workload-dependent.
6. **Termination under asynchrony:** FLP ⇒ no deterministic termination guarantee with one crash; "eventual under synchrony" is the best achievable — documented limitation.
7. **Batch size / pipeline depth defaults:** throughput vs latency tradeoff; unquantified pending benchmarks (ids-bench, Phase 048).
8. **Interaction with `ids-state-machine`:** deterministic-execution requirement on the caller; hook contract to be validated at integration.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-total-order-broadcast` (not a folder) | "Separate crates instead of more folders" directive; clear ordering boundary |
| Four atomic-broadcast properties (Validity, Uniform Agreement, Uniform Integrity, Uniform Total Order) | Hadzilacos & Toueg / ACM taxonomy; uniform = IDS requirement |
| Leader/sequencer assigns total-order seqno | Simplest correct TO broadcast (TUM DS-6; JavaGroups TOTAL); Zab/Raft/VR for fault tolerance |
| Quorum commit (⌊N/2⌋+1 acks) | Majority quorums intersect ⇒ consistent order; tolerates f < N/2 crashes |
| View change / epoch on leader failure | Zab recovery + VR view change; new leader adopts most up-to-date log |
| Stable storage before ack; checkpoint + GC | Crash recovery; bounded memory (Pilotfish GC pattern) |
| `msg_id` dedup set + signed messages | Uniform integrity; at-most-once; sender authentication |
| Lamport/HLC as metadata only; leader seqno is the order | Wall-clock never used for ordering (clock manipulation cannot break total order) |
| Allowed crypto only; `ids-crypto` surface via `ids-000-aa` | IDS crypto policy |
| Logic over `Vec<u8>` datagrams; no networking imports | Carrier-agnostic; matches logic-only Rust consensus crates (raft-lite, raft-consensus, simple-raft-rs, hotstuff_rs) |
| Audit: sha256/blake3 chain + ed25519-signed checkpoints | Offline-verifiable decision/delivery log; tamper evidence |

---

### 12. Sources

1. Wikipedia, "Atomic broadcast" — Validity, Uniform Agreement, Uniform Integrity, Uniform Total Order. https://en.wikipedia.org/wiki/Atomic_broadcast
2. ACM DL 10.1145/1041680.1041682 — "Total order broadcast and multicast algorithms: Taxonomy and survey" (Hadzilacos & Toueg properties). https://dl.acm.org/doi/10.1145/1041680.1041682
3. TUM DS-6 lecture, "Atomic broadcast" — sequencer assigns sequence number; FLP impossibility for atomic broadcast. https://tropars.github.io/downloads/lectures/DS/DS-6-atomic_broadcast.pdf
4. Chandra & Toueg, "Unreliable Failure Detectors for Reliable Systems" — Consensus and Atomic Broadcast reducible to each other; ♦S/♦P failure detectors. https://cs.princeton.edu/courses/archive/fall07/cos518/papers/unreliable.pdf
5. Fischer, Lynch, Paterson (FLP 1985) — consensus impossibility in asynchronous systems with one crash. https://arxiv.org/pdf/cs/0209014
6. Zab (IEEE 5958223) — "Zab: High-performance broadcast for primary-backup systems"; ZooKeeper atomic broadcast; leader + quorum + recovery. https://ieeexplore.ieee.org/document/5958223
7. Oki & Liskov, "Viewstamped Replication" (PODC 1988) — viewstamp ⟨view, op-number⟩; primary assigns op-number; view change; N ≥ 2f+1. https://cs.princeton.edu/courses/archive/fall11/cos518/papers/viewstamped.pdf
8. Liskov & Cowling, "Viewstamped Replication Revisited" — consensus extension of VR. https://courses.cs.washington.edu/courses/cse550/22au/papers/CSE550.ViewStampedReplication.pdf
9. MIT 6.824 / Princeton COS418/518 — Raft log replication; linearizability vs sequential consistency. https://pdos.csail.mit.edu/6.824/notes/l-linearizability.txt
10. Lamport, "Time, Clocks, and the Ordering of Events" — Lamport timestamps; total order by tie-break. https://cs.princeton.edu/courses/archive/fall18/cos418/docs/L3-time.pdf
11. Kulkarni et al., "Hybrid Logical Clocks" — (physical, logical) pair; causal order + bounded physical skew; CockroachDB/MongoDB usage. https://cse.buffalo.edu/tech-reports/2014-04.pdf
12. Castro & Liskov, "Practical Byzantine Fault Tolerance" (SOSP 1999) — PBFT 3-phase; 3f+1 replicas. https://cs.cmu.edu/~dga/15-440/S14/lectures/bft.pdf
13. arXiv 2412.14061 — "Fast Byzantine Total Order Broadcast"; 2Δ lower bound; Flutter fast path. https://arxiv.org/html/2412.14061
14. arXiv 2610.00896 — "Seamless Reconfiguration for DAG-BFT"; DAG dissemination + leader-based total order; multi-configuration reconfiguration. https://arxiv.org/html/2610.00896
15. Schneider, "Implementing Fault-Tolerant Services Using the State Machine Approach" (1990) — SMR: same initial state, same total order, deterministic execution. https://arxiv.org/html/2407.01720
16. arXiv 1904.04098 — SMR formalization; total-order broadcast = atomic broadcast = consensus. https://arxiv.org/pdf/1904.04098v3
17. Quorum R + W > N — read/write quorum intersection; strict vs partial quorums. https://arxiv.org/html/2002.06098v4 ; https://thecodeforge.io/system-design/quorum-consensus
18. JavaGroups TOTAL protocol — sequencer-based total order design. https://cs.cornell.edu/info/people/bba/projects/total/design.html
19. EPFL TCS — "Optimizing Paxos with batching and pipelining." http://infoscience.epfl.ch/record/189440/files/TCS.pdf
20. arXiv 2405.11183 — "MultiPaxos Made Complete"; concurrent replication handlers. https://arxiv.org/html/2405.11183v1
21. arXiv 2203.05681 — "State-Machine Replication Scalability Made Simple"; epochs/segments. https://arxiv.org/pdf/2203.05681
22. RFC 8032 — EdDSA / Ed25519 signatures. https://datatracker.ietf.org/doc/rfc8032
23. RFC 7539 — ChaCha20-Poly1305 AEAD, 96-bit nonce. https://datatracker.ietf.org/doc/rfc7539
24. libsodium — XChaCha20-Poly1305 construction, 192-bit nonce, random nonces safe. https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
25. NIST FIPS 180-4 — SHA-256 Secure Hash Standard. https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
26. BLAKE3 spec — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.tex
27. docs.rs raft-lite — logic-only Raft; messages are `Vec<u8>`; total-order delivery guarantee. https://docs.rs/crate/raft-lite/latest
28. docs.rs raft-consensus — logic-only consensus; no networking; caller passes messages. https://docs.rs/raft-consensus
29. simple-raft-rs — Rust Raft; no_std; returns `Vec<u8>` unicast messages. https://github.com/simple-raft-rs/raft-rs
30. hotstuff_rs — BFT SMR in Rust; HotStuff variant; BlockTree. https://github.com/parallelchain-io/hotstuff_rs
31. IDS prompts.txt entry 68 (lines 1015-1029). C:/idfs/prompts.txt
32. plan-001.md / plan-048.md / plan-060.md — IDS plan format references. C:/idfs/plans/

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, agreement properties, orderer strategies, view change, stable storage, audit, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 068 (ids-total-order-broadcast).
