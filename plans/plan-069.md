## Phase 069 — Create the ids-causal-broadcast crate (causally related messages delivered in order): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1030-1044)

```
69. Research only do not read plan.md **[Phase 069 — Create the ids-causal-broadcast crate: Causally related messages delivered in order.]**

Crate: ids-causal-broadcast
Purpose: Causally related messages delivered in order.
Contents: Causality preservation, concurrent message delivery flexibility, happens-before tracking, causal delivery guarantees.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 069 — Create the ids-causal-broadcast crate: Causally related messages delivered in order.]**

Create plan-069.md in C:/idfs/plans/ for the ids-causal-broadcast crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 069 creates `ids-causal-broadcast`, the IDS crate that delivers causally related messages in order while allowing concurrent messages to be delivered in any order consistent with causality. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by delivery mechanism. The crate does not open sockets or drive any transport directly; it tracks happens-before between messages, gates delivery on causal prerequisites, buffers messages whose causal past is unsatisfied (delay queue), and ships every datagram as opaque `Vec<u8>` to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). `ids-causal-broadcast` consumes types from `ids-000-aa` (NodeId, RecordId, IdsTime, CarrierType, Checksum, Error) and crypto primitives from `ids-crypto` (ed25519 for signing delivery decisions, sha256/blake3 for causal-context hashing, xchacha20poly1305/chacha20poly1305 for encrypting buffered state at rest) — it never implements crypto itself.

**Core causal model:** Lamport's happens-before (→), Lamport clocks for scalar ordering, vector clocks / dotted version vectors for detecting concurrency and computing delivery readiness, causal broadcast semantics: if broadcast(m1) → broadcast(m2) then every process that delivers both delivers m1 before m2; concurrent messages may be delivered in any order. This is weaker than FIFO-per-source (causal broadcast crosses sources) and weaker than total-order broadcast (atomic broadcast) — nodes may deliver concurrent messages in different orders, which is exactly the "concurrent message delivery flexibility" called for in the prompt.

**Scope boundary:** `ids-causal-broadcast` guarantees ordering; it does not guarantee reliability (no retransmission/uniform agreement — that is Phase 070 ids-atomic-broadcast). A skeleton `ids-causal-broadcast` crate does NOT yet exist at `C:/idfs/ids-causal-broadcast/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned Lamport 1978 "Time, Clocks, and the Ordering of Events" (happens-before, logical clocks, ACM CACM); Fidge/Mattern vector clocks (1988, partial ordering property, VC(x)<VC(y) iff componentwise ≤ and ∃ strict); Cambridge Distributed Systems notes (causal vs FIFO vs total-order broadcast, hold-back/buffer until causal past delivered, concurrent messages any order); EPFL DA causal-broadcast exercises (specification: no process delivers m' unless it has previously delivered every m such that m → m'; non-blocking algorithm using the past, blocking algorithm using vector clocks); Birman/Schiper/Stephenson "Fast Causal Multicast" (1990, Isis toolkit, CBcast vector-clock protocol with delay queue); ACM TOCS "Reliable communication in the presence of failures" (Birman, ISIS CBcast/ABcast, causal delivery as alternative to total order); Cornell CS6410 group-communication slides (CBcast uses vector clocks, delivery delayed until happened-before messages delivered, concurrent messages out of order, fast because asynchronous); "Verified Causal Broadcast with Liquid Haskell" (arxiv 2206.14767, causal delivery definition, delay queue, vector clock comparison); UW "Towards Causal Datacenter Networks" (causal delivery sm→sm' ⇒ rm→rm', vector clocks, Schiper O(n×n) ordering buffer); Wikipedia Vector Clock (VC comparison, asymmetry/transitivity, concurrent detection); ACM Queue "Why Logical Clocks are Easy" (causal histories → vector clocks, dotted vector clocks, version vectors); Riak "Vector Clocks Revisited Part 2: Dotted Version Vectors" (dots, sibling explosion, context); Preguiça et al. "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores" (DVV, dot separated from causal past, O(1) partial-order computation); Ricardobcl DVV GitHub (join/values/event/sync API); ACM CSUR "Approaches to CRDTs" (causal stability, tombstone removal); Wikipedia Causal Consistency (available under partition, stronger than eventual); COPS SOSP'11 (causal+ consistency, potential causality, wide-area storage); Princeton COS418/518 consistency lectures (causal vs sequential vs linearizable hierarchy, causal+ consistency, eventual consistency); CockroachDB HLC docs (hybrid logical clocks, causality token); Spanner TrueTime external consistency; CMI bounded version vectors; MerceMay vector clocks/version vectors concurrency detection; ACM DL dotted version vectors (10.1145/2332432.2332497); ed25519-dalek / chacha20poly1305 / sha2 / blake3 Rust crate docs; IDS prompts.txt entry 69; plan-001/plan-048/plan-060 format precedents at C:/idfs/plans/.

---

### 2. Architecture Overview

`ids-causal-broadcast` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is an ordering/guarantee layer: given a message to broadcast, it timestamps it with causal metadata (vector clock or dotted version vector), attaches a happens-before dependency list, and returns a `SendDecision`/`DeliveryDecision` that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets. On receive, it checks whether the message's causal past has been delivered; if not, it buffers the message in a delay queue until prerequisites arrive; if yes, it delivers and advances its local vector clock/dot state. Concurrent messages (neither happens-before the other) are deliverable immediately in any order — this is the concurrent-message-delivery flexibility.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (broadcast users)               │
│  ids-store, ids-replication, ids-consistency-profile — broadcast with context│
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 069: ids-causal-broadcast (THIS CRATE — ordering)       │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────┐ │ │\n│  │ causal-context   │ │ delivery-gate    │ │ delay-queue      │ │ ack/   │ │ │\n│  │ (VC / DVV / dot) │ │ (happens-before  │ │ (buffer unready) │ │ nack   │ │ │\n│  │ tracker          │ │ readiness check) │ │ + re-delivery    │ │        │ │ │\n│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └────────┘ │ │\n│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ ┌────────┐ │ │\\n│  │ stability-       │ │ crypto-surface   │ │ carrier-adapter  │ │ audit  │ │ │\\n│  │ tracker (GC)     │ │ (type IDs only)  │ │ (Endpoint only)  │ │ log    │ │ │\\n│  └──────────────────┘ └──────────────────┘ └──────────────────┘ └────────┘ │ │\\n└──────────────────────────────────────────────────────────┬────────────────┘\\n                                                         │ opaque Vec<u8> datagrams\\n┌──────────────────────────────────────────────────────────▼────────────────┐\\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │\\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8]) │\\n│  receive_datagram → (Endpoint, Vec<u8>)                                    │\\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │\\n└────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a broadcast:** caller submits `Broadcast{msg_id, payload, causal_ctx}` → crate computes local dot (increment own component), builds `CausalContext{vector_clock OR dotted_version_vector, dependency_dots}`, returns `SendDecision{endpoint_targets, datagram_bytes, causal_context}` → caller sends opaque datagrams via `ids-carrier::Endpoint` → receiver's `delivery_gate` checks `deliverable(m)` iff `∀ dot ∈ causal_past(m): dot ≤ local_clock[dot.node]` AND `msg_id not already delivered` → if deliverable: deliver, advance clock, then drain delay queue for newly-ready messages → if not: buffer in delay queue → deliver when ready.

**Ordering guarantees (research-verified):**
- **Causal order:** if `broadcast(m1) → broadcast(m2)` (happens-before), every process delivering both delivers `m1` before `m2`. Vector-clock condition: `VC(m1) < VC(m2)` (componentwise ≤ with at least one strict).
- **Concurrent delivery flexibility:** if `m1 ∥ m2` (neither happens-before), processes may deliver them in different orders — e.g. A delivers m1,m2,m3 while B delivers m1,m3,m2; both valid.
- **FIFO-per-source is implied:** messages from the same source are totally ordered by that source's clock component (a special case of causal order).
- **Weaker than total-order/atomic broadcast:** no agreement on a single global order across nodes; concurrent messages may diverge. Atomic broadcast (Phase 070) is strictly stronger and builds on reliable broadcast.
- **Not reliable broadcast by itself:** no uniform agreement, validity, or integrity guarantees; those belong to `ids-reliable-broadcast`/`ids-atomic-broadcast` phases. Causal broadcast can be layered on top of reliable broadcast (causal broadcast = reliable broadcast + causal order, per OPODIS 2018 Definition 6).

**Crypto used:** ed25519 signs broadcast/delivery decisions and audit checkpoints; sha256/blake3 hash causal contexts and version vectors for compact comparison and audit-chain links; xchacha20poly1305/chacha20poly1305 encrypt buffered delay-queue state at rest. All via `ids-crypto` primitive IDs from `ids-000-aa`; crate implements no crypto.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 Happens-before relation (Lamport 1978)

**Source:** Lamport, "Time, Clocks, and the Ordering of Events in a Distributed System," CACM 21(7), 1978, doi:10.1145/359545.359563. Events: send, receive, internal. `a → b` iff (same process local order) OR (`a` = send(m), `b` = receive(m)) OR transitive closure. Strict partial order: irreflexive, transitive, antisymmetric. Two events concurrent (`a ∥ b`) iff neither `a → b` nor `b → a`. Causal broadcast = delivery order consistent with happens-before. `ids-causal-broadcast` bases its causal model on this relation; no wall-clock is ever used for ordering.

#### 3.2 Lamport clocks (scalar) vs vector clocks

**Source:** same Lamport paper; Cambridge ConcDisSys notes; ETH Logical Time (vs.inf.ethz.ch/logtime.pdf); ACM Queue "Why Logical Clocks are Easy". Lamport scalar clock: `e → e' ⇒ C(e) < C(e')` but converse false — scalar clocks cannot detect concurrency (lossy). Vector clocks (Fidge 1988, Mattern 1988): `VC(x) < VC(y) ⟺ x → y` (exact, both directions). Each process keeps vector of size n; on local event increment own component; on send attach clock; on receive `Ci := max(Ci, ts)`, increment own. `ids-causal-broadcast` uses vector clocks as the canonical causal timestamp (exact concurrency detection), with Lamport clocks as a cheaper lossy alternative documented but not adopted as primary.

#### 3.3 Causal broadcast specification and algorithms

**Source:** EPFL DA causal-broadcast exercises (dcl.epfl.ch); Cornell CS5414 "Consistent Global States"; OPODIS 2018 "Causal Broadcast: How to Forget?" (Definitions 3–6: uniform reliable broadcast + causal order; causal delivery: no process delivers m' unless previously delivered every m → m'); Cambridge ConcDisSys (hold back/buffer until causal past delivered; concurrent messages any order; valid orders example (m1,m2,m3) or (m1,m3,m2)); "Verified Causal Broadcast with Liquid Haskell" (arxiv 2206.14767, process-local causal delivery, delay queue). Two algorithm families:
- **Non-blocking (using the past):** attach full causal past to each message; deliver immediately if past ⊆ delivered; otherwise buffer. Simpler, larger metadata.
- **Blocking (vector clocks, Schiper/CBirman):** attach vector clock; deliver message from source i at component k iff `VC(m)[i] = local[i]+1` (next expected from i) and `∀j≠i: VC(m)[j] ≤ local[j]` (no missing messages from others). Delay queue holds unready messages; drain on each delivery. `ids-causal-broadcast` adopts the vector-clock blocking algorithm as primary (bounded metadata O(n)), documents non-blocking as alternative.

#### 3.4 Version vectors vs vector clocks vs dotted version vectors

**Source:** HASlab "Version Vectors are not Vector Clocks" (Baquero 2011); Preguiça et al. "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores" (2012); Riak "Vector Clocks Revisited Part 2: Dotted Version Vectors"; MerceMay version vectors/conflict detection; ACM DL dotted version vectors 10.1145/2332432.2332497; Ricardobcl DVV GitHub; CMI bounded version vectors. Vector clocks track event causality across all processes (size = n processes, grows with every event). Version vectors track data-item causality per key (size = replicas writing that key). Dotted version vectors (DVV) separate the "dot" (current event `(node, counter)`) from the causal past (version vector), enabling O(1) partial-order computation: if dot(n) ≤ vv(n) then X causally precedes Y. DVVSet manages conflicts (concurrent values kept until reconciliation). `ids-causal-broadcast` uses DVV as the compact per-message causal context (dot + past), with vector clocks as the underlying clock representation; bounded variant (CMI bounded version vectors) researched for long-running systems to prevent unbounded growth.

#### 3.5 Causal delivery condition and delay-queue algorithm

**Source:** EPFL exercises; Cornell CS6410 slides; "Verified Causal Broadcast"; UW datacenter networks paper (Schiper O(n×n) ordering buffer impractical at scale → per-node vector clock is the practical alternative). Delivery condition at process p for message m with timestamp `VC(m)` from source i:
- `VC(m)[i] = VC_p[i] + 1` (this is the next expected message from i — FIFO per source)
- `∀ j ≠ i: VC(m)[j] ≤ VC_p[j]` (all causal predecessors from other sources already delivered)
If both hold → deliver, `VC_p := max(VC_p, VC(m))`, `VC_p[i] += 1`, drain delay queue. Else → buffer in delay keyed by source i and sequence. Drain re-checks buffered messages after every delivery. `ids-causal-broadcast` implements this as `delivery_gate` + `delay_queue`.

#### 3.6 Concurrent message delivery flexibility

**Source:** Cambridge ConcDisSys notes (m2, m3 concurrent; A delivers m1,m3,m2 while B delivers m1,m2,m3; both valid); EPFL causal broadcast; "Verified Causal Broadcast" (Alice's "Found it!" causally precedes Bob's "Glad to hear it!" — Carol must deliver Alice's first). The crate's contract explicitly permits per-node delivery order divergence for concurrent messages; the only invariant is causal order. A `DeliveryOrder` record logs the actual order per node for audit; tests assert causal-order preservation but NOT cross-node order equality.

#### 3.7 Causal stability and garbage collection / tombstone pruning

**Source:** ACM CSUR "Approaches to CRDTs" (causal stability = no more concurrent messages can be delivered; tombstones removable at causal stability; RGA does this); Shapiro CRDTs 2011 (causal history, eventual delivery, convergence); Preguiça DVV (removing old entries, pruning). `ids-causal-broadcast` includes a `stability_tracker` that determines when causal context for delivered messages can be garbage-collected (no undelivered message can still depend on it), bounding metadata growth. Tombstone pruning is documented as deferred to `ids-reliable-broadcast`/`ids-crdt` phases.

#### 3.8 Causal+ and transactional causal consistency (COPS, AntidoteDB)

**Source:** COPS SOSP'11 (causal+ consistency, potential causality, wide-area storage, strongest AP model); AntidoteDB Speaker Deck / tutorial (transactional causal consistency TCC, just-right consistency, CRDT merge); Wikipedia causal consistency (available under partition, stronger than eventual). `ids-causal-broadcast` provides the broadcast primitive; `ids-consistency-profile` (Phase 060) and `ids-crdt` phases layer consistency models on top. Researched but not adopted in this crate: transactional causality (TCC), session guarantees (read-your-writes, monotonic reads/writes, writes-follow-reads) — noted as integration points.

#### 3.9 Hybrid Logical Clocks and Spanner TrueTime (evaluated, not adopted)

**Source:** CockroachDB HLC docs (physical + logical 64-bit timestamp, causality token); Spanner TrueTime external consistency; HLD Handbook clocks-and-ordering. HLC gives causal ordering in a wall-clock-compatible 64-bit value; TrueTime gives external consistency but needs atomic clocks/GPS. Evaluated: `ids-causal-broadcast` uses pure logical clocks (vector clocks / DVV) because it must be correct under arbitrary clock skew and no trusted time source; HLC/TrueTime documented as alternatives requiring clock assumptions the IDS carrier-agnostic model does not make.

#### 3.10 ISIS / Birman CBcast and reliable broadcast relationship

**Source:** Birman et al. ISIS virtual synchrony (1985, ACM TOCS reliable communication); "Fast Causal Multicast" (Birman/Schiper/Stephenson 1990); Cornell CS6410 slides (CBcast vs ABcast: CBcast causal, ABcast total; CBcast fast/asynchronous, ABcast slow/token). Causal broadcast = reliable broadcast + causal order (OPODIS Def 6). `ids-causal-broadcast` assumes an underlying best-effort datagram carrier via `ids-carrier::Endpoint`; reliability (retransmission, uniform agreement) is deferred to `ids-reliable-broadcast` (Phase 069 sibling) and `ids-atomic-broadcast` (Phase 070). The crate's `carrier_adapter` only frames bytes to Endpoint; it does not implement ACK/retransmission itself.

#### 3.11 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek docs.rs (RFC 8032 EdDSA); chacha20poly1305 docs.rs (RFC 8439 AEAD + XChaCha20 192-bit nonce); sha2 RustCrypto; BLAKE3-team README. Usage in crate:
- ed25519: sign broadcast decisions / delivery audit checkpoints.
- sha256: causal-context hashing, audit-chain links.
- blake3: fast content hashing for vector-clock comparison keys, checkpoint chaining (preferred for speed/parallelism).
- xchacha20poly1305: encrypt buffered delay-queue state at rest (24-byte nonce, random safe).
- chacha20poly1305: encrypt session/state payloads (12-byte nonce, counter-based).
No other crypto. `ids-crypto` exposes primitive IDs; crate references via `ids-000-aa` only.

#### 3.12 Determinism, idempotence, at-most-once delivery

**Source:** OPODIS 2018 (uniform integrity: deliver at most once, only if previously broadcast); verified causal broadcast (idempotent merge tolerates duplicates); CRDT merge properties (commutative, associative, idempotent). Each message carries a globally unique `msg_id` (derived from `ids-000-aa` `MessageId` newtype); delivery gate checks `msg_id` against delivered set before delivering (at-most-once). Duplicate datagrams from the carrier are deduplicated. Idempotent delivery supports safe retransmission at the reliability layer above.

#### 3.13 Complexity bounds

**Source:** ETH Logical Time / vector clock papers; Schiper O(n×n) ordering buffer (UW paper notes impractical at scale). Vector clock size O(n) per process; per-message metadata O(n); delivery check O(n); delay-queue drain amortized O(ready messages × n). For very large n, DVV (dot + sparse past) is the compact alternative; bounded version vectors researched for unbounded systems.

#### 3.14 Relationship to FIFO, causal, total-order broadcast

**Source:** Cambridge ConcDisSys (forms of reliable broadcast slide; FIFO ≠ causal example; total-order enforces same order on all nodes); Cornell CS6410 (CBcast vs ABcast). Hierarchy documented: FIFO (per-source order) ⊂ causal (cross-source happens-before) ⊂ total-order/atomic (single global order). `ids-causal-broadcast` sits at the causal level; atomic broadcast (Phase 070) is the stronger sibling. The crate explicitly does NOT provide total order — concurrent messages may be delivered in different orders on different nodes (documented guarantee).

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Out-of-order delivery violating causality | Message ordering | Vector-clock/DVV delivery gate: deliver m only when causal past ⊆ delivered; delay queue holds unready messages |
| Concurrent messages forced into one order (over-constraining) | Delivery flexibility | Contract permits per-node order divergence for `m1 ∥ m2`; tests assert causal-order preservation only, not cross-node equality |
| Duplicate datagrams from carrier causing double delivery | At-most-once | `msg_id` uniqueness + delivered-set check; idempotent delivery gate |
| Missing causal predecessor (message arrives before its cause) | Causal order | Delay queue buffers until prerequisite delivered; drain on each delivery |
| Clock/wall-time manipulation breaking causality | Causality correctness | Pure logical clocks (vector clocks / DVV dots); never wall-clock, NTP, or RTT for ordering; monotonic `IdsTime` only for timeouts |
| Unbounded vector-clock / metadata growth | Memory / DoS | Stability tracker + causal-stability GC; bounded version vectors researched as alternative |
| Replay of old broadcast | Replay | `msg_id` delivered-set; bounded replay cache per source; nonce-bound signed decisions |
| Carrier duplicating/delaying/reordering datagrams | Ordering integrity | Carrier-agnostic: ordering enforced at crate layer over opaque bytes; survives any carrier re-framing |
| Disallowed crypto algorithm | Crypto policy | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via `ids-crypto`; CI allowlist test |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP, etc.) | Architecture | `Endpoint` sole address type; dependency audit bans networking crates; clippy/CI rule |
| Audit-log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints; offline verifiable |

The carrier is never trusted: causal decisions are computed over vector clocks / dots that travel with the message bytes, surviving any carrier re-framing.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-causal-broadcast ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed contracts + invariants + errors | `NodeId`, `RecordId`, `MessageId`, `IdsTime`, `CarrierType`, `Checksum`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All broadcast traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing decisions/audit; encrypting buffered state; hashing causal contexts |
| `ids-consistency-profile` (Phase 060) | Profile registry + quorum/durability evaluation | Profile tags carried with broadcasts; causal context consumed by profile reads/writes |
| `ids-reliable-broadcast` (sibling phase) | Reliable broadcast (validity, agreement, integrity) | Causal broadcast layers on reliable broadcast; delivery gate reused |
| `ids-atomic-broadcast` (Phase 070) | Total-order / atomic broadcast | Strictly stronger sibling; builds on causal ordering |
| `ids-crdt` (later phase) | CRDT state/op/delta types | Causal context (DVV) shared for CRDT merge/causality |
| `ids-session` (Phase 050) | Client-session binding | Session-bound broadcasts carry causal context; audit cross-referenced |

**Integration points:** crate exposes `broadcast(msg)` → `SendDecision` and `receive(datagram)` → `DeliveryDecision`; caller frames bytes to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond contracts. Crate never opens a socket or imports a networking crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-causal-broadcast/` (a separate crate, not a folder inside an existing crate):

```
ids-causal-broadcast/
├── Cargo.toml                          # manifest; ids-000-aa, ids-carrier, ids-crypto, serde, thiserror
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # defaults: clock size, delay-queue cap, stability window
├── error.rs                            # CausalBroadcastError (thiserror): gate, queue, crypto, invariant errors
├── features.rs                         # feature flags: std/no_std, dvv, vector-clock, audit, crypto
│
├── causal_context/                     # causal metadata: vector clock / DVV / dot
│   ├── mod.rs
│   ├── vector_clock.rs                 # VectorClock: componentwise compare, merge (max), increment
│   ├── dot.rs                          # Dot: (NodeId, counter) — current event
│   ├── dotted_version_vector.rs        # DVV: dot + causal past (version vector), O(1) order
│   ├── version_vector.rs               # VersionVector: replica→counter map (data causality)
│   ├── causal_past.rs                  # CausalPast: set of dots a message depends on
│   ├── causality.rs                    # happens-before check: VC(x)<VC(y), concurrent detection
│   ├── serialization.rs                # serde encode/decode of causal contexts to bytes
│   └── types.rs                        # CausalContext, CausalMetadata structures
│
├── delivery_gate/                      # delivery readiness check + happens-before gating
│   ├── mod.rs
│   ├── gate.rs                         # deliverable(m) iff past ⊆ delivered ∧ next-from-source
│   ├── delivered_set.rs                # delivered msg_id set (at-most-once, idempotent)
│   ├── readiness.rs                    # readiness condition per source/component
│   └── types.rs                        # DeliveryDecision{Deliver|Wait|Reject|Duplicate}
│
├── delay_queue/                        # buffer for unready messages
│   ├── mod.rs
│   ├── queue.rs                        # delay queue keyed by (source, seq); drain on delivery
│   ├── buffer.rs                       # in-memory buffered datagrams + causal contexts
│   ├── eviction.rs                     # bounded queue eviction policy (oldest-first / TTL)
│   └── types.rs                        # DelayedMessage, QueueResult structures
│
├── clock_tracker/                      # local vector clock / DVV state per node
│   ├── mod.rs
│   ├── local_clock.rs                  # LocalClock: per-node vector clock, advance on deliver
│   ├── clock_merge.rs                  # merge remote clock on receive (componentwise max)
│   └── types.rs                        # ClockState structures
│
├── stability_tracker/                  # causal stability + garbage collection
│   ├── mod.rs
│   ├── stability.rs                    # determine when context can be GC'd (no pending dependents)
│   ├── gc.rs                           # tombstone / context pruning at causal stability
│   └── types.rs                        # StabilityResult structures
│
├── broadcast/                          # broadcast send path
│   ├── mod.rs
│   ├── sender.rs                       # compute dot, build context, emit SendDecision
│   ├── decision.rs                     # SendDecision{endpoints, datagram, context}
│   └── types.rs                        # BroadcastMessage, BroadcastResult structures
│
├── carrier_adapter/                    # opaque datagram framing to ids-carrier::Endpoint
│   ├── mod.rs
│   ├── adapter.rs                      # frame bytes → Endpoint; parse receive → datagram
│   └── types.rs                        # CarrierFramedMessage structures
│
├── audit/                              # signed, hash-chained audit log
│   ├── mod.rs
│   ├── audit_entry.rs                  # decision entry: broadcast/deliver/wait/reject
│   ├── audit_chain.rs                  # sha256/blake3 hash chain
│   ├── checkpoint.rs                   # ed25519-signed periodic checkpoints
│   ├── signer.rs / verifier.rs         # signing/verification via ids-crypto primitive IDs
│   └── mod.rs (dup removed) — see above
│   └── types.rs                        # AuditEntry, Checkpoint structures
│
├── contracts/                          # sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── causal_context_contract.rs
│   ├── delivery_gate_contract.rs
│   ├── delay_queue_contract.rs
│   ├── clock_contract.rs
│   ├── stability_contract.rs
│   ├── broadcast_contract.rs
│   ├── carrier_contract.rs
│   ├── audit_contract.rs
│   └── causality_contract.rs
│
├── invariants/                         # compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── causal_order_preserved.rs       # broadcast(m1)→broadcast(m2) ⇒ deliver m1 before m2
│   ├── at_most_once.rs                 # msg_id delivered at most once
│   ├── concurrent_flexibility.rs       # concurrent messages may diverge per node
│   ├── carrier_agnostic.rs             # no std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist.rs             # only the five allowed algorithms
│   ├── clock_monotonic.rs              # logical clocks only, no wall-clock for ordering
│   ├── stability_sound.rs              # GC only at causal stability
│   └── idempotent_delivery.rs          # duplicate datagrams deduplicated
│
├── tests/                              # property/verification tests (~30 files)
│   ├── causal_order_tests.rs
│   ├── concurrent_delivery_tests.rs    # concurrent messages delivered in different orders OK
│   ├── delay_queue_tests.rs            # buffering + drain correctness
│   ├── vector_clock_tests.rs           # compare/merge/increment
│   ├── dvv_tests.rs                    # dot + past O(1) order
│   ├── delivered_set_tests.rs          # at-most-once, idempotence
│   ├── stability_tests.rs              # GC correctness
│   ├── carrier_agnostic_tests.rs       # zero prohibited crate references
│   ├── crypto_allowlist_tests.rs       # only five algorithms reachable
│   ├── audit_chain_tests.rs            # hash chain + signed checkpoint verification
│   ├── property_tests.rs               # proptest: causal order under reorder/duplicate/loss
│   ├── integration_tests.rs            # end-to-end broadcast via mock Endpoint
│   └── determinism_tests.rs            # same seed + scenario ⇒ identical delivery order
│
├── benches/                            # (~10 files)
│   ├── mod.rs
│   ├── delivery_latency.rs             # gate check latency
│   ├── throughput.rs                   # broadcasts/sec per payload size
│   ├── delay_queue.rs                  # queue depth vs drain
│   ├── vector_clock.rs                 # compare/merge cost vs n
│   └── crypto.rs                       # ed25519 sign, sha256/blake3 hash, AEAD benchmarks
│
├── docs/                               # (~14 files)
│   ├── architecture.md
│   ├── causal_model.md                 # happens-before, vector clocks, DVV
│   ├── delivery_gate.md
│   ├── delay_queue.md
│   ├── audit.md
│   ├── carrier_agnostic.md
│   ├── crypto_policy.md
│   ├── adding_a_mechanism.md
│   ├── relationship_to_reliable_broadcast.md
│   ├── relationship_to_atomic_broadcast.md
│   ├── runbook.md
│   └── glossary.md
│
├── spec/                               # (~6 files)
│   ├── causal_broadcast_spec.md        # normative: delivery condition, ordering guarantees
│   ├── vector_clock_spec.md
│   ├── dvv_spec.md
│   ├── delay_queue_spec.md
│   ├── audit_spec.md
│   └── carrier_agnostic_spec.md
│
└── test_vectors/                       # (~6 files)
    ├── causal_scenarios.json           # broadcast/delivery sequences with expected orders
    ├── vector_clock_vectors.json       # expected compare/merge results
    ├── dvv_vectors.json                # expected dot/past comparisons
    └── delay_queue_vectors.json        # expected buffer/drain sequences
```

Total: ~90–1000 files across `src/`, `src/causal_context/`, `src/delivery_gate/`, `src/delay_queue/`, `src/clock_tracker/`, `src/stability_tracker/`, `src/broadcast/`, `src/carrier_adapter/`, `src/audit/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, `spec/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Vector clock comparison and merge
```
VectorClock { entries: Map<NodeId, u64> }
compare(a, b):  a < b  iff ∀k: a[k] ≤ b[k] ∧ ∃k': a[k'] < b[k']   (happens-before)
                a ∥ b  iff neither a < b nor b < a              (concurrent)
merge(a, b)[k] = max(a[k], b[k])   (componentwise join on receive)
increment(self): entries[self] += 1
size O(n); compare/merge O(n)
```

#### 7.2 Dotted version vector (DVV)
```
DVV { dot: (NodeId, u64), past: VersionVector }
order(X, Y): if X.dot ≤ Y.past[X.dot.node] then X → Y (O(1) partial-order)
             else compare pasts
```
Dot separated from causal past enables compact representation and O(1) dominance checks (Preguiça et al. 2012).

#### 7.3 Delivery gate (Schiper / Birman vector-clock protocol)
```
deliverable(m, local_clock) iff
  m.VC[m.source] = local_clock[m.source] + 1   (next expected from source)
  ∀ j ≠ m.source: m.VC[j] ≤ local_clock[j]     (no missing predecessors)
on deliver: local_clock := max(local_clock, m.VC); local_clock[m.source] += 1;
            drain delay_queue for newly-ready messages
else: buffer m in delay_queue keyed by (m.source, m.seq)
```

#### 7.4 At-most-once / idempotent delivery
```
delivered: Set<MessageId>
on receive(m): if m.msg_id ∈ delivered → drop (duplicate)
               else if deliverable → deliver, add to delivered, advance clock, drain queue
               else → buffer in delay_queue
```

#### 7.5 Causal stability / GC
```
stable(context) iff no message in delay_queue or in-flight can depend on context
GC removes context entries for delivered messages whose dots are ≤ all delivered dots per node
```

#### 7.6 Send path
```
broadcast(msg):
  dot := (self_node, local_clock[self] + 1)
  ctx := CausalContext{ dot, past := local_clock.snapshot() }
  local_clock[self] += 1
  return SendDecision{ datagram := encode(msg, ctx), endpoints := targets, ctx }
```

#### 7.7 Audit chain
```
entry_n := sha256/blake3(entry_{n-1} || decision_bytes)
checkpoint_k := ed25519_sign(chain_head, signing_key)
```

---

### 8. Dependencies

|| Crate | Purpose ||
|-------|---------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RecordId`, `MessageId`, `IdsTime`, `CarrierType`, `Checksum`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `Carrier` trait — decision transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `serde` | Serialization of causal contexts, delivery decisions, audit entries |
| `thiserror` | Error types |
| `proptest` | Property-based tests (causal order under reorder/duplicate/loss) |
| `tempfile` | Test fixtures |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list; `ids-causal-broadcast` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|-----------|-----------|-------------|
| AC-69.1 | Causal order: if broadcast(m1) → broadcast(m2), every node delivering both delivers m1 before m2 | Vector-clock delivery gate; 100% of causally-related pairs ordered correctly under reorder/loss/duplicate |
| AC-69.2 | Concurrent delivery flexibility: concurrent messages may be delivered in different orders per node | Tests assert per-node order divergence is permitted; no cross-node total-order assertion |
| AC-69.3 | FIFO-per-source implied: messages from same source delivered in send order | Special case of causal order; verified per source clock component |
| AC-69.4 | At-most-once delivery: each msg_id delivered at most once; duplicates dropped | Delivered-set check; duplicate datagram injection test |
| AC-69.5 | Delay queue: messages with undelivered causal past buffered; delivered when ready; drain on each delivery | Buffer/drain correctness tests; no message lost or delivered early |
| AC-69.6 | Vector clock compare/merge exactness: VC(x)<VC(y) ⟺ x→y | Unit tests against known vector examples |
| AC-69.7 | DVV O(1) dominance: dot ≤ past ⇒ causal precedence | Unit tests |
| AC-69.8 | Causal stability GC: metadata pruned only at stability, no live dependent removed | GC correctness tests |
| AC-69.9 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test (`cargo tree`) + source grep — zero matches |
| AC-69.10 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-69.11 | Every broadcast/delivery/wait/reject recorded in hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime` | Audit chain verification test |
| AC-69.12 | Logical clocks only for ordering; no wall-clock/NTP/RTT used | Source audit; monotonic `IdsTime` only for timeouts |
| AC-69.13 | Determinism: same seed + scenario ⇒ identical delivery order | Determinism tests pass |
| AC-69.14 | Crate compiles cleanly: `cargo check -p ids-causal-broadcast` exits 0 | Build |
| AC-69.15 | Tests pass: `cargo test -p ids-causal-broadcast` all green | Test run |

---

### 10. Remaining Uncertainties

1. **Vector size n dynamic vs fixed:** vector clock size = number of participants; dynamic membership (join/leave) requires resizing / DVV with sparse dots. Resolution deferred to `ids-discovery`/`ids-membership` integration.
2. **Delay-queue eviction policy:** bounded queue needed for long-running systems; eviction (oldest-first / TTL / stability-based) unquantified; interacts with causal-stability GC.
3. **Reliability layer split:** causal broadcast assumes best-effort datagrams; retransmission/uniform agreement split between this crate and `ids-reliable-broadcast`/`ids-atomic-broadcast` not yet finalized.
4. **DVV vs pure vector clock as default:** DVV chosen for compactness; pure vector clock simpler. Final default to be confirmed at implementation.
5. **Cross-node order divergence tolerated:** some applications may want convergent concurrent order; that is a stronger guarantee (causal+ / total order) deferred to later phases.
6. **Bounded version vectors:** for very large/long-running systems, unbounded counters grow; bounded-stamp variant (CMI) researched but not adopted pending scale requirements.
7. **Integration with `ids-consistency-profile` (Phase 060):** profile tags on broadcasts and causal-context consumption by profile reads/writes to be validated at integration.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate `ids-causal-broadcast` crate (not a folder) | Follows "separate crates instead of more folders" directive; clear ordering boundary |
| Vector clocks as canonical causal timestamp | Exact happens-before in both directions (VC(x)<VC(y) ⟺ x→y); O(n) compare/merge; Lamport scalar clocks lossy (cannot detect concurrency) |
| DVV for per-message causal context | Dot separated from past → O(1) dominance checks, compact metadata (Preguiça et al. 2012; Riak DVV) |
| Blocking delivery gate with delay queue | Schiper/Birman CBcast protocol: deliver iff next-from-source ∧ no missing predecessors; concurrent messages any order |
| At-most-once idempotent delivery via msg_id + delivered set | OPODIS uniform integrity; tolerates carrier duplicates; supports safe retransmission above |
| Pure logical clocks only (no wall-clock/NTP/RTT for ordering) | Correct under arbitrary clock skew; no trusted time source assumed (carrier-agnostic model) |
| Causal broadcast ≠ reliable/atomic broadcast | Ordering only; validity/agreement/integrity deferred to `ids-reliable-broadcast`/`ids-atomic-broadcast` phases |
| Causal stability GC bounds metadata | Tombstone/context pruning only when no pending dependent (ACM CSUR CRDTs; RGA) |
| All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`) | Carrier-agnostic absolute rule; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |
| Allowed crypto only: ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via `ids-crypto` | IDS crypto policy; crate implements no crypto |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |

---

### 12. Sources

[^1]: Lamport, "Time, Clocks, and the Ordering of Events in a Distributed System," CACM 21(7), 1978. https://dl.acm.org/doi/10.1145/359545.359563 — happens-before, logical clocks.
[^2]: Fidge, "Logical Time in Distributed Computing Systems," Computer 24(8), 1991. https://dl.acm.org/doi/10.1109/2.485846 — vector clocks.
[^3]: Mattern, "Virtual Time and Global States of Distributed Systems," 1988. Vector clocks independent invention.
[^4]: Cambridge ConcDisSys notes — causal vs FIFO vs total-order broadcast; hold-back/buffer; concurrent messages any order. https://www.cl.cam.ac.uk/teaching/2425/ConcDisSys/dist-sys-notes.pdf
[^5]: EPFL DA causal-broadcast exercises — specification, non-blocking/blocking algorithms, delay queue. https://dcl.epfl.ch/site/_media/education/file03_da24-causalbroadcast.pdf
[^6]: EPFL DA reliable & causal broadcast solutions — causal delivery definition, uniform reliable broadcast + causal order. https://dcl.epfl.ch/site/_media/education/da_2020_exercise_1_reliable_broadcast_solutions.pdf
[^7]: Birman, Schiper, Stephenson, "Fast Causal Multicast," 1990 (Isis toolkit, CBcast vector-clock protocol). https://ntrs.nasa.gov/api/citations/19900012217/downloads/19900012217.pdf
[^8]: Birman, "Reliable communication in the presence of failures," ACM TOCS. https://dl.acm.org/doi/10.1145/7351.7478 — ISIS CBcast/ABcast, causal delivery alternative to total order.
[^9]: Cornell CS6410 group communication slides — CBcast vs ABcast, vector clocks, delay queue, concurrent delivery out of order. https://www.cs.cornell.edu/courses/cs6410/2017fa/slides/16-virtual-synchrony.pdf
[^10]: "Verified Causal Broadcast with Liquid Haskell," arxiv 2206.14767 — causal delivery definition, delay queue, vector clock comparison. https://arxiv.org/html/2206.14767v2
[^11]: UW "Towards Causal Datacenter Networks," Papoc'18 — causal delivery sm→sm' ⇒ rm→rm', Schiper O(n×n) buffer impractical at scale. https://syslab.cs.washington.edu/papers/tcdn-papoc18.pdf
[^12]: Wikipedia Vector Clock — partial ordering property, asymmetry, transitivity, concurrency detection. https://en.wikipedia.org/wiki/Vector_clock
[^13]: ACM Queue "Why Logical Clocks are Easy" — causal histories → vector clocks, dotted vector clocks, version vectors. https://queue.acm.org/detail.cfm?id=2917756
[^14]: HASlab "Version Vectors are not Vector Clocks," Baquero 2011. https://haslab.wordpress.com/2011/07/08/version-vectors-are-not-vector-clocks
[^15]: Preguiça et al., "Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores," 2012. https://gsd.di.uminho.pt/members/vff/dotted-version-vectors-2012.pdf
[^16]: Riak "Vector Clocks Revisited Part 2: Dotted Version Vectors." https://riak.com/posts/technical/vector-clocks-revisited-part-2-dotted-version-vectors/
[^17]: Ricardobcl Dotted-Version-Vectors GitHub — join/values/event/sync API. https://github.com/ricardobcl/Dotted-Version-Vectors
[^18]: ACM DL dotted version vectors 10.1145/2332432.2332497 — efficient causality tracking. https://dl.acm.org/doi/10.1145/2332432.2332497
[^19]: ACM CSUR "Approaches to Conflict-free Replicated Data Types" — causal stability, tombstone removal. https://dl.acm.org/doi/full/10.1145/3695249
[^20]: Shapiro et al., "Conflict-free Replicated Data Types," 2011 (INRIA RR-7687). https://www.lip6.fr/Marc.Shapiro/papers/2011/CRDTs_SSS-2011.pdf
[^21]: COPS SOSP'11 — causal+ consistency, potential causality, wide-area storage. https://www.cs.cmu.edu/~dga/papers/cops-sosp2011.pdf
[^22]: Wikipedia Causal Consistency — available under partition, stronger than eventual. https://en.wikipedia.org/wiki/Causal_consistency
[^23]: AntidoteDB Speaker Deck / tutorial — transactional causal consistency, TCC, just-right consistency. https://speakerdeck.com/marcshapiro/antidotedb-a-developer-friendly-geo-replicated-crdt-data-store
[^24]: Princeton COS418/518 consistency lectures — causal vs sequential vs linearizable hierarchy, causal+ consistency. https://cs.princeton.edu/courses/archive/fall26/cos418/docs/L14-consistency.pdf
[^25]: CockroachDB HLC docs — hybrid logical clocks, causality token. https://docs.cockroachlabs.com/docs/stable/architecture/transaction-layer
[^26]: Spanner TrueTime external consistency. https://cloud.google.com/blog/products/databases/strict-serializability-and-external-consistency-in-spanner
[^27]: CMI "Bounded Version Vectors using Mazurkiewicz Traces." https://www.cmi.ac.in/~spsuresh/pdfs/acss2021.pdf
[^28]: MerceMay vector clocks and concurrency detection. https://mercemay.com/articles/distributed-systems/time-and-ordering/vector-clocks-and-concurrency-detection/
[^29]: MerceMay version vectors and conflict detection. https://mercemay.com/articles/distributed-systems/replication/version-vectors-and-conflict-detection/
[^30]: ed25519-dalek docs.rs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
[^31]: chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
[^32]: sha2 RustCrypto — SHA-256. https://docs.rs/sha2
[^33]: BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^34]: IDS prompts.txt entry 69 (verbatim). C:/idfs/prompts.txt — Phase 069 crate definition, contents, constraints.
[^35]: Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types, contracts, invariants, errors, versioning.
[^36]: plan-001.md / plan-048.md / plan-060.md (IDS plan format references). C:/idfs/plans/

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 069 (ids-causal-broadcast).
