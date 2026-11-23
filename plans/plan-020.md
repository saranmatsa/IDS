## Phase 020 — Create the ids-broadcast crate (reliable/causal/total order/atomic broadcast): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 020 creates `ids-broadcast`, the IDS crate for message ordering guarantees across replicas. It implements the broadcast hierarchy: **reliable broadcast → FIFO broadcast → causal broadcast → total order broadcast (atomic broadcast) → FIFO-total order broadcast**, plus state machine replication (SMR) on top, and the four atomic broadcast properties **uniform agreement, uniform integrity, validity, termination**.

Crate: `ids-broadcast` (CRATES.md entry 20). Purpose: message ordering guarantees across replicas. Contents per prompts.txt entry 20: reliable broadcast, FIFO broadcast, causal broadcast, total order broadcast (atomic broadcast), FIFO-total order broadcast, state machine replication, uniform agreement, uniform integrity, validity, termination.

Verbatim prompt (C:/idfs/prompts.txt lines 295-309): "Phase 020 — Create the ids-broadcast crate: Message ordering guarantees across replicas."

Research basis: 30 parallel searches returned Bracha's reliable broadcast (1987, validity/agreement/integrity), FIFO broadcast (per-sender sequence ordering), causal broadcast (happens-before, vector clocks), total order = atomic broadcast (Hadzilacos & Toueg 1994: validity, uniform agreement, uniform integrity, uniform total order), Paxos made simple (Lamport), Zab (ZooKeeper total order broadcast), Raft (replicated log SMR), Viewstamped Replication (Oki & Liskov), PBFT (Castro & Liskov, 3f+1, pre-prepare/prepare/commit), Dolev-Strong authenticated broadcast (signature chains, t+1 rounds, PKI), HotStuff (view-by-view recipe, optimistic responsiveness), Flexible Paxos (cross-phase quorum intersection only), Totem (fault-tolerant totally ordered multicast), Lamport logical clocks, vector clocks (causality detection), hybrid logical clock (HLC), gossip/epidemic broadcast (HyParView, eventual reliability), SRM reliable multicast, atomic multicast vs atomic broadcast taxonomy, Chandra-Toueg failure detectors (consensus reducible to atomic broadcast), Fast Paxos (2-message-delay learning), EPaxos (leaderless dependency graph), causal atomic broadcast, delta-CRDT anti-entropy.

A skeleton `ids-broadcast` crate does NOT yet exist at `C:/idfs/ids-broadcast/`. `ids-000-aa` already declares `trait Broadcast` (reliable/causal/total order/atomic) at `src/contracts/broadcast.rs`; `ids-broadcast` implements it. Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024 bytes). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-broadcast` consumes type-level surface and primitive IDs only). `ids-000-aa` is the master crate with all canonical types and sealed trait contracts. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-broadcast` sits above `ids-000-aa` (types + sealed `trait Broadcast` contract + invariants + versioning + errors) and `ids-carrier` (opaque `Endpoint` datagram send/receive). It is layered so each ordering guarantee is an independent module that can be composed:

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / SMR LAYER (ids-* crates)            │
│  state machine replication · ordered command application · commit   │
├─────────────────────────────────────────────────────────────────────┤
│              Phase 020: ids-broadcast (THIS CRATE)                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ reliable    │ │ FIFO        │ │ causal      │ │ total order │   │
│  │ broadcast   │ │ broadcast   │ │ broadcast   │ │ (atomic)    │   │
│  │ (Bracha)    │ │ (per-sender │ │ (vector     │ │ (consensus │   │
│  │ INIT/ECHO/  │ │ seq + buffer│ │ clock + dep │ │ slot: Paxos/ │   │
│  │ READY)      │ │ reassembly) │ │ set + buf)  │ │ Raft/Zab)   │   │
│  └─────────────┘ └─────────────┴───────────────┴─────────────┘   │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ FIFO-total order · SMR executor · delivery buffer · gap repair │  │
│  │ Dolev-Strong authenticated broadcast (Byzantine, ed25519)    │  │
│  └───────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (ids-carrier::Endpoint only)           │
│  send_datagram(Endpoint{CarrierType, Vec<u8>≤1024}, &[u8])          │
│  receive_datagram → (Endpoint, Vec<u8>)                             │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
└─────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`NodeId`, `OrderingPos`, `OpRef`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`) imported from `ids_000_aa::X`, never redefined.
- `ids-broadcast` implements the sealed `trait Broadcast` from `ids-000-aa/src/contracts/broadcast.rs`; downstream crates use it as a trait bound, cannot implement it.
- All message transport is datagram-based via `ids-carrier::Endpoint` (`CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16) + opaque `Vec<u8>` capped at 1024 bytes). No IP addresses, ports, sockets.
- Crypto surface: ed25519 signatures (authentication/Dolev-Strong chains), sha256/blake3 (message digests, sequence commitments), xchacha20poly1305/chacha20poly1305 (optional payload encryption). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Ordering is dataset-scoped via `ids-ordering-domain` (crate 55); no global master (invariant `no_global_master` in `ids-000-aa`).
- Operation lifecycle reuses `ids-000-aa` `OpState` (`received → ordered → executing → committed → synchronized → verified → failed`) and `CommitState`.

---

### 3. Detailed Research Dimensions

#### 3.1 Broadcast Hierarchy and Guarantees

| Layer | Guarantee | Source |
|-------|-----------|--------|
| Reliable broadcast | Validity (correct sender → all correct deliver), Agreement (one correct delivers → all correct deliver), Integrity (no duplication, only broadcast messages delivered) | Bracha 1987; decentralizedthoughts.github.io/2020-09-19-living-with-asynchrony-brachas-reliable-broadcast |
| FIFO broadcast | Reliable + per-sender FIFO order (sender's messages delivered in broadcast order) | Purdue ECE 60872 DistributedPrimitives; ETH distsys ch.18 |
| Causal broadcast | Reliable + happens-before order (causally related messages delivered in order; concurrent unordered) | arXiv 2206.14767 (verified causal broadcast); Yale Aspnes Broadcast wiki |
| Total order broadcast (= atomic broadcast) | Reliable + uniform total order (all processes deliver same messages in same order) | Hadzilacos & Toueg 1994; Wikipedia Atomic broadcast; CMU atomic-broadcast.pdf |
| FIFO-total order | Total order + per-sender FIFO preserved within the total order | ACM total-order taxonomy 10.1145/1041680.1041682; Southampton causal+total order paper |

Key distinction (from research): total order alone does NOT imply FIFO or causal order; FIFO-total order and causal-total order are strict strengthenings. Atomic broadcast ≡ consensus in asynchronous crash-failure systems (Chandra-Toueg).

#### 3.2 Reliable Broadcast — Bracha Algorithm

The foundational primitive. A designated sender broadcasts value `v`; protocol uses three message classes: INIT (sender → all), ECHO (forwarded on first INIT), READY (sent on f+1 ECHOs or n-f READY conditions). Delivery when n-f READYs received for the same `v`. Tolerates up to f < n/3 Byzantine faults for agreement; crash-only variant needs no authentication. Properties:
- **Validity**: if the sender is non-faulty and broadcasts, all correct processes eventually deliver.
- **Agreement**: if any correct process delivers a message, all correct processes eventually deliver it.
- **Integrity**: every correct process delivers at most one message, and only a message that was broadcast.

For `ids-broadcast`: generic Bracha core parameterized by `n` and `f`, driven by `ids-carrier::Endpoint` datagrams; retransmission on timeout via endpoint re-send; duplicate suppression via message IDs.

#### 3.3 FIFO Broadcast

Built on reliable broadcast: each sender tags messages with a per-sender sequence number; receivers buffer out-of-order messages in a per-sender reassembly buffer and deliver in sequence order (`next[q]` cursor). No cross-sender ordering constraints. Research source: Purdue FIFO broadcast algorithm (msgBag + next[q] while-loop delivery).

#### 3.4 Causal Broadcast

Built on FIFO broadcast plus dependency tracking: each message carries a vector clock (or dependency set) of causally preceding broadcasts; a message is delivered only after all its dependencies have been delivered (delivery buffer + dependency-resolution loop). Implies single-source FIFO but not total order. Research: arXiv 2206.14767 (causal delivery = buffer until all causally preceding messages applied; vector clock metadata); Yale pinewiki (causal ⇒ single-source FIFO but not total). Vector clocks come from `ids-vector-clock` (crate 66); timestamps from `ids-hlc` (crate 67) optional.

#### 3.5 Total Order Broadcast / Atomic Broadcast

All correct processes deliver the same set of messages in the same total order. Uniform variants (apply also to faulty processes): Uniform Total Order, Uniform Agreement, Uniform Integrity. Plus Validity and Termination. Consensus equivalence: atomic broadcast is reducible to consensus and vice versa (Chandra-Toueg 1996); cannot be implemented in purely asynchronous systems with even one crash failure (FLP) — needs eventual synchrony or a failure detector (eventually strong ⋄P / ⋄S).

Implementation approaches for `ids-broadcast`:
- **Sequencer-based**: dataset-scoped leader assigns monotonically increasing sequence numbers (`OrderingPos`) from `ids-ordering-domain`; all replicas deliver in sequence order. Simplest, matches "no global master" since leadership is per ordering domain.
- **Consensus-slot-based**: one consensus instance per sequence slot (Multi-Paxos / Raft / Zab). Delegated to sibling crates: `ids-total-order-broadcast` (68), `ids-flexible-paxos` (71), `ids-bft-consensus` (72).
- **Leaderless**: EPaxos-style dependency graph (arXiv 2511.02743) — commands order collaboratively via predecessor agreement.

#### 3.6 Atomic Broadcast Properties (Uniform)

From Hadzilacos & Toueg / CMU atomic-broadcast.pdf:
- **Validity**: if a correct process broadcasts m, it eventually delivers m.
- **Uniform Agreement**: if any process (correct or faulty) delivers m, all correct processes eventually deliver m.
- **Uniform Integrity**: every process delivers m at most once, and only if m was previously broadcast by sender(m).
- **Uniform Total Order**: if p and q both deliver m and m′, then p delivers m before m′ iff q delivers m before m′.

Validity + Agreement + Integrity ⇒ reliable broadcast. Total order + agreement ⇒ atomic broadcast.

#### 3.7 State Machine Replication (SMR)

Broadcast is the ordering substrate for SMR: clients submit commands → total order broadcast assigns a position → every replica applies commands in that same order → deterministic state machine ⇒ identical state across replicas. Sources: Lamport "Paxos made simple" / replicated state machines; Raft (raft.github.io/raft.pdf); Zab (ZooKeeper ACM 1529974.1529978 — reliable delivery, total order, causal order); Viewstamped Replication (Oki & Liskov — viewstamps, totally ordered view IDs + timestamps). `ids-broadcast` defines the SMR integration surface (command log, deterministic apply hook); actual executor lives in `ids-write`/`ids-consistency` (crates 13/19).

#### 3.8 Ordering Mechanisms (metadata on broadcast messages)

- **Lamport logical clocks**: partial order, tie-broken by process ID (CMU 04-clocks.pdf; Yale LogicalClocks).
- **Vector clocks**: exact causality detection; concurrent when neither VC ≤ other (systemoverflow vector clocks guide). Consumed from `ids-vector-clock` (crate 66).
- **Hybrid Logical Clock (HLC)**: causality like logical clocks, stays close to NTP physical time (Kulkarni HLC paper, UChicago). Consumed from `ids-hlc` (crate 67).
- **IDS standard time** (`ids-time`, crate 5): proprietary tick basis (`Idstime`), not wall-clock — commit/ordering timestamps.

#### 3.9 Byzantine / Authenticated Broadcast

- **Dolev-Strong (1983)**: authenticated broadcast tolerating any t < n with PKI; signature chains; t+1 rounds; O(n²t) words; lower bound f+1 rounds for deterministic Byzantine broadcast (decentralizedthoughts.github.io/2019-12-22-dolev-strong; UMD lecture 26).
- **PBFT** (Castro & Liskov SOSP 1999): 3f+1 replicas, pre-prepare/prepare/commit three-phase, view changes, tolerates f Byzantine (cs.utah.edu pbft.pdf).
- **HotStuff**: view-by-view recipe, optimistic responsiveness, linear view change (arXiv 2305.13556).
`ids-broadcast` references `ids-authenticated-broadcast` (crate 73, Dolev-Strong) and `ids-bft-consensus` (crate 72); ed25519 (from `ids-crypto`) signs broadcast messages and signature chains.

#### 3.10 Quorum and Consensus Building Blocks

- **Flexible Paxos** (arXiv 1608.06696): only cross-phase quorum intersection (Q1 ∩ Q2 ≠ ∅) is required; within-phase quorums need not intersect. Enables small commit quorums, large election quorums. Sibling crate: `ids-flexible-paxos` (71).
- **Fast Paxos** (MSR TR 2005-112): learn in 2 message delays; collisions fall back to classic Paxos.
- **EPaxos** (arXiv 2511.02743): leaderless, dependency-graph ordering, command coordinator per command, failure recovery.
- **Chandra-Toueg failure detectors**: completeness (strong/weak) + accuracy (strong/weak, eventual); consensus and atomic broadcast are reducible to each other; ⋄S is the weakest failure detector for consensus (cs.princeton.edu cos518 unreliable.pdf). `ids-failure-detector` (crate 7) supplies suspicion; `ids-broadcast` consumes it for liveness.

#### 3.11 Gossip / Epidemic and Reliable Multicast

- **Gossip broadcast**: periodic random peer exchange; O(log N) rounds to reach all nodes w.h.p.; probabilistic reliability (ACM 1317379.1317383; HyParView IEEE 4272993 — fanout tuning for 99%+ reliability). Useful for anti-entropy and dissemination in `ids-broadcast` when carrier topology is sparse/multi-hop.
- **SRM reliable multicast** (IEEE 650139): eventual delivery without enforced order; ordering machinery layered on top.
- **Atomic multicast** (ACM taxonomy 10.1145/1041680.1041682; arXiv 1406.7540): broadcast generalized to destination groups; atomic broadcast = single-group atomic multicast; genuine atomic multicast involves only addressed processes. `ids-broadcast` covers all-replicas broadcast; partitioning/group variants deferred to later phases.

#### 3.12 Delivery Buffer, Gap Detection, and Retransmission

All broadcast flavors need: per-sender sequence space, delivery buffer (out-of-order holdback), gap detection (missing sequence numbers), duplicate suppression (delivered-set), and retransmission requests. Since there is no network stack, retransmission is expressed as re-sending a datagram via `ids-carrier::Endpoint` to the same endpoint — carrier decides how to deliver. This keeps `ids-broadcast` fully carrier-agnostic.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Reordering in carrier (datagrams may arrive out of order) | Per-sender sequence numbers + delivery buffer + gap detection before delivery |
| Duplicate datagrams | Delivered-set (message ID dedup); Uniform Integrity (at-most-once delivery) |
| Lost datagrams / carrier packet loss | Retransmission via endpoint re-send on timeout; Bracha ECHO/READY quorum masks loss |
| Byzantine sender sending conflicting values | Dolev-Strong signature chains / PBFT authentication via ed25519 from ids-crypto |
| Byzantine equivocation (sender sends m to some, m′ to others) | Uniform Agreement + Uniform Integrity; authenticated broadcast (crate 73) |
| Replica crash mid-broadcast | Uniform Agreement forces all correct replicas to deliver if any did; Termination under eventual synchrony |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; clippy/CI rule; test greps source for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Version skew / wire incompatibility between replicas | `ProtocolVersion` + `EdgeCaseBits` from ids-000-aa; broadcast message frames version-tagged; mismatched peers reject before delivery |
| Ordering stall if domain leader fails | Failure detector (`ids-failure-detector`) triggers view/epoch change; Flexible Paxos quorum reconfiguration |
| Memory growth of delivery buffers | Bounded buffer with gap watermark; backpressure via `ids-fault` circuit breakers (crate 27) |
| Causal delivery deadlock (missing dependency never arrives) | Timeout-based dependency resolution with retransmission requests; eventual delivery under carrier liveness |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 020 ||
||-------|-------|-------------------|
| `ids-000-aa` (master) | Canonical types (`NodeId`, `OrderingPos`, `OpRef`, `Endpoint`, `CarrierType`, `OpState`, `CommitState`, `ConsistencyProfile`, `FailureDomain`), sealed `trait Broadcast`, invariants (~40), versioning (`ProtocolVersion`, `EdgeCaseBits` u128), unified `Error` |
| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤1024); `send_datagram`/`receive_datagram`; sole traffic channel for `ids-broadcast` |
| `ids-crypto` (Phase 003) | ed25519 signing/verification, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 AEAD; consumed only |
| `ids-ordering-domain` (crate 55) | Dataset-scoped ordering domains, per-domain committed position, domain-scoped leadership (no global master) |
| `ids-consistency` (crate 19) | Operation-state tracking (received→ordered→executing→committed→synchronized→verified→failed), consistency profiles |
| `ids-write` (crate 13) | WAL and write lifecycle that consumes broadcast-delivered commands |
| `ids-vector-clock` (crate 66) | Causality metadata for causal broadcast |
| `ids-hlc` (crate 67) | Optional hybrid logical timestamps for ordering metadata |
| `ids-time` (crate 5) | `Idstime` commit/ordering tick basis |
| `ids-failure-detector` (crate 7) | Suspicion for leader liveness / view change |
| `ids-total-order-broadcast` (crate 68) | Consensus-based total order layer (Paxos/Raft/Zab slots) — `ids-broadcast` builds atomic broadcast on top |
| `ids-flexible-paxos` (crate 71) | Quorum-intersection-consensus slots |
| `ids-bft-consensus` (crate 72) | PBFT/HotStuff-style Byzantine consensus slots |
| `ids-authenticated-broadcast` (crate 73) | Dolev-Strong authenticated broadcast for Byzantine environments |
| `ids-fault` (crate 27) | Circuit breakers / backpressure for broadcast delivery |

**Integration points:**
- `ids-broadcast` implements `ids_000_aa::contracts::Broadcast` (sealed trait) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-broadcast` never touches sockets.
- Authenticated messages signed with ed25519 via `ids-crypto`; message integrity via sha256/blake3 digests.
- Depends on `ids-000-aa` only for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (per CRATES.md dependency rule) — `ids-broadcast` depends on both.
- Cargo workspace: `ids-broadcast = { path = "../ids-broadcast" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-broadcast` (to be created at `C:/idfs/ids-broadcast/`)

```
ids-broadcast/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto, ids-ordering-domain,
│                                       # ids-consistency, ids-vector-clock, ids-hlc, ids-time,
│                                       # ids-failure-detector; thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # BroadcastId, MessageId, BroadcastMsg, Delivery, SeqNo,
│   │                                   # ViewId, EpochId, DeliveryState, OrderingPosRef
│   ├── errors.rs                       # BroadcastError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Broadcast (sealed trait)
│   │
│   ├── properties/                     # atomic broadcast properties (~5 files)
│   │   ├── mod.rs
│   │   ├── uniform_agreement.rs
│   │   ├── uniform_integrity.rs
│   │   ├── validity.rs
│   │   └── termination.rs
│   │
│   ├── reliable/                       # Bracha reliable broadcast (~6 files)
│   │   ├── mod.rs
│   │   ├── bracha_core.rs              # INIT/ECHO/READY state machine
│   │   ├── bracha_messages.rs          # Init/Echo/Ready datagram types
│   │   ├── quorum.rs                   # f+1 echo / n-f ready counting
│   │   ├── delivery.rs                 # reliable delivery conditions
│   │   └── retransmit.rs               # endpoint-based retransmission on timeout
│   │
│   ├── fifo/                           # FIFO broadcast (~5 files)
│   │   ├── mod.rs
│   │   ├── per_sender_seq.rs           # per-sender sequence numbers
│   │   ├── reassembly_buffer.rs        # out-of-order holdback per sender
│   │   ├── next_cursor.rs              # next[q] delivery cursor
│   │   └── fifo_layer.rs               # reliable + FIFO composition
│   │
│   ├── causal/                         # Causal broadcast (~6 files)
│   │   ├── mod.rs
│   │   ├── dependency_set.rs           # message dependency metadata
│   │   ├── vector_clock_adapter.rs     # wrap ids-vector-clock (crate 66)
│   │   ├── causal_delivery_buffer.rs   # hold until dependencies satisfied
│   │   ├── happens_before.rs           # causality checks
│   │   └── causal_layer.rs             # FIFO + causal composition
│   │
│   ├── total_order/                    # Total order / atomic broadcast (~8 files)
│   │   ├── mod.rs
│   │   ├── sequencer.rs                # dataset-scoped sequencer assigns OrderingPos
│   │   ├── consensus_slot.rs           # one consensus instance per sequence slot
│   │   ├── total_order_delivery.rs     # deliver in sequence order
│   │   ├── atomic_broadcast.rs         # uniform agreement + integrity + total order
│   │   ├── view_change.rs              # leader/view rotation via failure detector
│   │   ├── paxos_slot_adapter.rs       # ids-flexible-paxos (crate 71) integration
│   │   └── raft_zab_adapter.rs         # ids-total-order-broadcast (crate 68) integration
│   │
│   ├── fifo_total/                     # FIFO-total order (~4 files)
│   │   ├── mod.rs
│   │   ├── per_sender_fifo.rs          # per-sender FIFO inside total order
│   │   ├── total_fifo_layer.rs         # total order + FIFO composition
│   │   └── tiebreak.rs                 # sender-ID tie-breaking for deterministic order
│   │
│   ├── smr/                            # State machine replication surface (~5 files)
│   │   ├── mod.rs
│   │   ├── command_log.rs              # totally ordered command log
│   │   ├── deterministic_apply.rs      # apply hook (executor lives in ids-write/ids-consistency)
│   │   ├── replica_state.rs            # replica applies same sequence → same state
│   │   └── smr_integration.rs          # wiring to ids-write WAL
│   │
│   ├── authenticated/                  # Byzantine authenticated broadcast (~5 files)
│   │   ├── mod.rs
│   │   ├── dolev_strong.rs             # signature chains, t+1 rounds (crate 73)
│   │   ├── pbft_slot.rs                # pre-prepare/prepare/commit (crate 72)
│   │   ├── hotstuff_view.rs            # view-by-view recipe (arXiv 2305.13556)
│   │   └── auth_messages.rs            # ed25519-signed broadcast messages
│   │
│   ├── gossip/                         # Epidemic dissemination & anti-entropy (~4 files)
│   │   ├── mod.rs
│   │   ├── fanout.rs                   # gossip fanout target selection
│   │   ├── epidemic_broadcast.rs       # probabilistic reliable broadcast (HyParView-style)
│   │   └── anti_entropy.rs             # digest exchange for convergence
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper
│   ├── delivery_buffer.rs              # generic out-of-order delivery buffer + gap detection
│   ├── dedup.rs                        # delivered-message ID set (uniform integrity)
│   ├── state/                          # state machines (~3 files)
│   │   ├── mod.rs
│   │   ├── broadcast_state_machine.rs
│   │   └── delivery_state_machine.rs
│   │
│   ├── codec/                          # wire encoding of broadcast messages (~4 files)
│   │   ├── mod.rs
│   │   ├── broadcast_frame.rs          # version-tagged frame header
│   │   ├── message_codec.rs            # serde/bincode encode/decode
│   │   └── digest.rs                   # sha256/blake3 message digests
│   │
│   ├── validation/                     # input validation rules
│   ├── observability/                  # broadcast event types (delivered, ordered, gap, retransmit)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── reliable_broadcast_tests.rs     # validity/agreement/integrity
│   ├── fifo_broadcast_tests.rs
│   ├── causal_broadcast_tests.rs
│   ├── total_order_broadcast_tests.rs
│   ├── atomic_broadcast_tests.rs       # uniform agreement/integrity/order
│   ├── fifo_total_broadcast_tests.rs
│   ├── smr_tests.rs
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~110–160 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

1. **Bracha reliable broadcast state machine** — INIT(sender, v, msg_id) → ECHO on first INIT → READY on f+1 matching ECHOs or n-f READYs; deliver on n-f READYs for same `v`; quorum counters per (msg_id, v); duplicate suppression by msg_id.
2. **Per-sender FIFO sequencing** — each sender increments a per-destination sequence number; receiver holds `next[q]` cursor and a per-sender `msgBag` buffer; deliver when `msgBag` contains `next[q]` from `q`, then increment (Purdue algorithm).
3. **Causal delivery with dependency sets** — message carries vector clock / dependency set; delivery buffer holds messages until all dependencies delivered; dependency-resolution loop after each delivery; implies FIFO but not total order.
4. **Dataset-scoped sequencer (total order)** — ordering domain leader (from `ids-ordering-domain`, crate 55) assigns next `OrderingPos`; all replicas deliver broadcast messages in `OrderingPos` order; leadership is per-domain, no global master (invariant `no_global_master`).
5. **Consensus slot per sequence position** — Multi-Paxos / Raft / Zab slot decides the message at each sequence number; uniform total order + uniform agreement; delegated to `ids-total-order-broadcast` (68), `ids-flexible-paxos` (71), `ids-bft-consensus` (72).
6. **Uniform atomic broadcast** = reliable broadcast + uniform total order + uniform agreement + uniform integrity + validity + termination (Hadzilacos & Toueg 1994).
7. **FIFO-total order** — global total order with per-sender FIFO as an intra-order constraint; tie-break by sender ID for deterministic total order among concurrent messages.
8. **SMR command log** — broadcast command → total order position → deterministic apply → identical replica state; executor hook lives in `ids-write`/`ids-consistency`.
9. **Dolev-Strong authenticated broadcast** — signature chains of distinct signers; t+1 rounds; t < n tolerated with PKI; ed25519 signatures from `ids-crypto`; message content signed, chain verified hop-by-hop.
10. **Delivery buffer + gap detection** — generic holdback buffer for all layers; gap = missing sequence from any sender/domain; retransmission request re-sent via `ids-carrier::Endpoint`; watermark advances on contiguous delivery.
11. **Gossip fanout dissemination** — on broadcast, send to `fanout` random/configured endpoints; first-time receivers re-gossip; O(log N) rounds to reach all nodes w.h.p.; used for anti-entropy and sparse carrier topologies.
12. **Message digest** — sha256/blake3 digest over `(sender, seq_no, domain, payload)` for dedup and integrity; committed-prefix tracking reuses `ids-consistency`.

---

### 8. Dependencies

`ids-broadcast/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Broadcast`, invariants, versioning, errors
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 (consumed only)
- `ids-ordering-domain = { path = "../ids-ordering-domain" }` — dataset-scoped ordering domains
- `ids-consistency = { path = "../ids-consistency" }` — op-state tracking, consistency profiles
- `ids-vector-clock = { path = "../ids-vector-clock" }` — causality metadata
- `ids-hlc = { path = "../ids-hlc" }` — optional hybrid logical timestamps
- `ids-time = { path = "../ids-time" }` — `Idstime` commit tick basis
- `ids-failure-detector = { path = "../ids-failure-detector" }` — suspicion for leader liveness
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — message serialization
- `blake3 = "1.5"` — content hashing (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling consensus crates (68, 71, 72, 73) are referenced via adapters but total-order delegation is a design option, not a hard dependency in this phase.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | Five broadcast flavors specified | `src/reliable/`, `src/fifo/`, `src/causal/`, `src/total_order/`, `src/fifo_total/` modules present; each documents its guarantee |
|| 2 | Atomic broadcast properties defined | `src/properties/` defines Validity, Uniform Agreement, Uniform Integrity, Uniform Total Order, Termination per Hadzilacos & Toueg 1994 |
|| 3 | SMR integration surface specified | `src/smr/` command log + deterministic apply hook; wiring notes to ids-write/ids-consistency |
|| 4 | Sealed `trait Broadcast` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Broadcast` |
|| 5 | Canonical types from ids-000-aa only | `types.rs` imports `NodeId`, `OrderingPos`, `OpRef`, `Endpoint`, `CarrierType`, `OpState`, `CommitState` from `ids_000_aa`; no redefinitions |
|| 6 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` in source; all traffic via `ids_carrier::Endpoint` |
|| 7 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM/etc. |
|| 8 | No code written / no source modifications | Plan document only; `ids-broadcast/` crate not created; no edits to existing crates |
|| 9 | Conformance test plan enumerated | `tests/` list covers reliability, FIFO, causal, total order, atomic, FIFO-total, SMR, carrier-agnostic assertion, crypto allowlist assertion |
|| 10 | File structure within range | ~90–160 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
||-------------|--------|-----------------|
| Total-order backend: sequencer vs consensus slots | Affects `src/total_order/` design and which sibling crate (68/71/72) is wired in | Decide during implementation; sequencer is simplest, consensus slots give fault tolerance |
| Uniform vs non-uniform variants | Stronger guarantees cost more messages | Uniform variants specified by default (matches CRATES.md `trait Broadcast` intent) |
| Failure model: crash-only vs Byzantine | Determines need for Dolev-Strong/PBFT layer | Crash-only for core; authenticated broadcast via crate 73 as optional layer |
| Delivery buffer size bounds | Memory safety under loss | Bounded watermark buffer; exact cap tuned in implementation |
| Retransmission vs carrier reliability | Protocol complexity | Assume unreliable datagrams; retransmit via endpoint re-send; optimize if carrier is reliable |
| Vector clock size growth (n entries per message) | Bandwidth cost | Pruning / HLC fallback from `ids-hlc`; evaluated per deployment |
| Whether `ids-broadcast` owns consensus or delegates | Crate boundaries | Delegate to ids-total-order-broadcast (68) + ids-flexible-paxos (71); this crate composes |
| Integration timing with ids-ordering-domain leadership | Ordering domain must exist before broadcast | Phases ordered: 020 after ordering domain crate exists |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|
| Separate `ids-broadcast` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; enforces single-responsibility boundary |
| Five-layer composition (reliable → FIFO → causal → total → FIFO-total) | Matches prompts.txt entry 20 contents exactly; each layer independently testable |
| Delegate total order to ids-total-order-broadcast (68) / Flexible Paxos (71) | Keeps this crate focused on broadcast semantics; consensus is a separate concern (CRATES.md entries 68, 71) |
| All traffic via `ids-carrier::Endpoint` only | Carrier-agnostic absolute rule (CRATES.md, prompts.txt entry 20) |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| Uniform variants of agreement/integrity/total order | Strongest safety; matches `trait Broadcast` contract in ids-000-aa |
| Dataset-scoped ordering domains, no global master | Invariant `no_global_master` in ids-000-aa; leadership per domain |
| Bracha reliable broadcast as foundation | Validity/Agreement/Integrity; f < n/3 Byzantine tolerance with authentication |
| Gossip/anti-entropy for dissemination | Resilient under carrier loss and sparse topology (HyParView research) |
| No code in this phase | Research only per prompts.txt entry 20 |

---

### 12. Sources

1. Bracha's Reliable Broadcast — decentralizedthoughts.github.io/2020-09-19-living-with-asynchrony-brachas-reliable-broadcast — validity/agreement/integrity, INIT/ECHO/READY, f < n/3.
2. Bracha 1987 (core.ac.uk PDF) and conference version 1984 (ecommons.cornell.edu 84-590) — original reliable broadcast paper.
3. Purdue ECE 60872 — DistributedPrimitives.pdf — reliable/FIFO/causal/atomic broadcast definitions and FIFO construction (msgBag + next[q]).
4. ETH Distributed Systems ch.18 — disco.ethz.ch/courses/distsys/lnotes2/chapter18.pdf — FIFO broadcast algorithm (echo rounds, n−f acceptance).
5. Verified Causal Broadcast with Liquid Haskell — arxiv.org/pdf/2206.14767v2 — causal delivery via buffering until all causally preceding messages applied; vector clock metadata.
6. Yale Aspnes — Broadcast (pinewiki) — single-source FIFO, total order, causal order relations; causal atomic broadcast.
7. Atomic broadcast (Wikipedia) — en.wikipedia.org/wiki/Atomic_broadcast — uniform total order, distinction from FIFO/causal.
8. Total order broadcast specification (CMU) — course.ece.cmu.edu/~ece845/sp11/docs/atomic-broadcast.pdf — Hadzilacos & Toueg 1994 properties: validity, uniform agreement, uniform integrity, uniform total order; causal order equivalence (FIFO + local order).
9. Causal + total order formal development (Southampton) — eprints.soton.ac.uk/264001/1/orderpaper2.pdf — causal order = FIFO + local order; total order conforming to causal.
10. Paxos Made Simple — cs.cmu.edu/~gibbons/15-712/papers/paxos-simple.pdf — Lamport consensus → state machine replication.
11. Zab: ZooKeeper atomic broadcast — doi.org/10.1145/1529974.1529978 — reliable delivery, total order, causal order requirements.
12. Raft — raft.github.io/raft.pdf — replicated log, leader-based total order, state machine replication.
13. Viewstamped Replication — cs.princeton.edu/courses/archive/fall11/cos518/papers/viewstamped.pdf — Oki & Liskov; viewstamps, totally ordered view IDs + timestamps.
14. PBFT — cs.utah.edu/~stutsman/cs6963/public/pbft.pdf — Castro & Liskov SOSP 99; 3f+1 replicas; pre-prepare/prepare/commit.
15. Dolev-Strong Authenticated Broadcast — decentralizedthoughts.github.io/2019-12-22-dolev-strong — signature chains, t+1 rounds, PKI, t < n tolerance.
16. Dolev-Strong lecture notes — homes.cs.washington.edu/~jrl/cse422wi24/notes/blockchain_3.pdf — Byzantine broadcast consistency/validity, f+1 round lower bound.
17. Lessons from HotStuff — arxiv.org/pdf/2305.13556 — view-by-view recipe, optimistic responsiveness, secure broadcast.
18. Flexible Paxos — arxiv.org/html/1608.06696 — cross-phase quorum intersection only (Q1 ∩ Q2 ≠ ∅).
19. Totem fault-tolerant multicast — dl.acm.org/doi/10.1145/227210.227226 — reliable totally ordered multicast over LANs/gateways.
20. Lamport logical clocks — cs.cmu.edu/~dga/15-712/S11/lectures/04-clocks.pdf — happens-before, logical clock assignment.
21. Vector clocks causality — systemoverflow.com/learn/distributed-primitives/vector-clocks/what-are-vector-clocks-and-how-do-they-encode-causality — VC comparison, concurrency detection.
22. Hybrid Logical Clock — klasses.cs.uchicago.edu/archive/2026/spring/23380-1/papers/kulkarni_hlc.pdf — HLC combines logical causality with physical time.
23. Gossip robustness — dl.acm.org/doi/10.1145/1317379.1317383 — epidemic broadcast, O(log N) rounds, eventual consistency with probability 1.
24. HyParView reliable gossip broadcast — ieeexplore.ieee.org/document/4272993 — fanout tuning for 99%+ reliability; gossip reliability definition.
25. SRM reliable multicast — ieeexplore.ieee.org/document/650139 — eventual delivery without enforced order; ordering layered on top.
26. Atomic broadcast/multicast taxonomy — dl.acm.org/doi/10.1145/1041680.1041682 — atomic broadcast ≡ single-group atomic multicast; uniform properties.
27. Atomic multicast extension (Multi-Ring Paxos) — arxiv.org/pdf/1406.7540v1 — atomic multicast = single group atomic broadcast; partitioned ordering.
28. Chandra-Toueg unreliable failure detectors — cs.princeton.edu/courses/archive/fall07/cos518/papers/unreliable.pdf — completeness/accuracy; consensus ≡ atomic broadcast reducibility.
29. Fast Paxos — microsoft.com/en-us/research/wp-content/uploads/2016/02/tr-2005-112.pdf — 2-message-delay learning, collision fallback.
30. EPaxos / Egalitarian Paxos — arxiv.org/html/2511.02743 — leaderless dependency-graph ordering; command coordinator.
31. Leaderless Concurrent Atomic Broadcast — arxiv.org/html/1608.05866v2 — validity/agreement/integrity/total order; set agreement.
32. Eliminating Servers in SMR — arxiv.org/pdf/2611.01363 — durable total order / shared log via consensus; SMR roles.
33. CRDT convergence — Shapiro et al. INRIA RR-7687 (2011); delta-CRDT arXiv:1603.01529 — join-semilattice, delta-mutators, anti-entropy (inform anti-entropy module).
34. IDS prompts.txt entry 20 (verbatim) — C:/idfs/prompts.txt — crate definition, contents, constraints.
35. CRATES.md entry 20 — C:/idfs/CRATES.md — ids-broadcast: reliable/causal/total order/atomic broadcast.
36. ids-000-aa skeleton — C:/idfs/ids-000-aa/ — types.rs, contracts/broadcast.rs (`trait Broadcast`), invariants, versioning, errors.
37. HX-241 plan format — C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture, Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
38. Existing plan-001.md — C:/idfs/plans/plan-001.md — phase plan template followed by this document.

---

**End of plan-020.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**