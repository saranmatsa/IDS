## Phase 074 — Create the ids-chandy-lamport crate: Consistent global state for checkpointing and recovery. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1105-1119)

```
74. Research only do not read plan.md **[Phase 074 — Create the ids-chandy-lamport crate: Chandy-Lamport global state recording.]**

Crate: ids-chandy-lamport
Purpose: Chandy-Lamport global state recording.
Contents: Marker message algorithm, consistent global state, stable property detection, checkpoint/restore foundation, Flink-style barrier snapshotting integration.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 074 — Create the ids-chandy-lamport crate: Chandy-Lamport global state recording.]**

Create plan-074.md in C:/idfs/plans/ for the ids-chandy-lamport crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 074 creates `ids-chandy-lamport`, a separate IDS crate (not a folder inside an existing crate, per the project rule) that records a consistent global state of a distributed computation using the Chandy-Lamport marker algorithm. The crate owns: marker message algorithm (initiate/propagate/terminate), local process state capture, per-channel in-flight message recording, consistent-cut construction and validation, stable property detection (termination, deadlock, quiescence), a checkpoint/restore foundation (save/restore of `GlobalSnapshot`), and a Flink-style barrier snapshotting integration (numbered checkpoints as barriers, aligned vs unaligned modes, exactly-once restore contract).

`ids-chandy-lamport` is carrier-agnostic: it never opens a socket and imports no networking crate. Markers, snapshots, and checkpoint traffic ride as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). The crate consumes canonical types (`NodeId`, `RegionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, `IdsTime`, `Error`) and sealed trait contracts from `ids-000-aa` (the master crate) and never redefines them. Allowed crypto only: ed25519 (snapshot envelope and audit-checkpoint signatures), sha256 and blake3 (channel-state digests, snapshot IDs, hash chains), xchacha20poly1305 and chacha20poly1305 (snapshot state encrypted at rest).

Research basis: 30 parallel targeted searches covering the original Chandy-Lamport 1985 paper (ACM TOCS 3(1), 63–75), Lamport's EWD 864a treatment, consistent cuts and happens-before closure, FIFO channel assumptions and non-FIFO variants (Mattern coloring, Lai-Yang), stable property detection and lattice-of-consistent-cuts theory, Apache Flink asynchronous barrier snapshotting / aligned vs unaligned checkpoints / savepoints / exactly-once semantics, checkpoint/restore patterns (etcd, Open MPI, async checkpointing for eventually consistent DBs), and Rust prior art for actor checkpoint/restore (theta, axactor, dstate). Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-chandy-lamport` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), `ids-crypto` (allowed primitives), and `ids-time` (monotonic `IdsTime`). It is a recording/snapshotting layer: it does not run the application, does not send application bytes itself, and does not decide routing — it records state, collates consistent cuts, and hands completed snapshots/checkpoints back to the caller as opaque byte envelopes to be sent via `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (processes under observation)       │
│  ids-store, ids-replication, ids-query — normal execution, opaque datagrams only │
├──────────────────────────────────────────────────────────────────────────────────┤
│               Phase 074: ids-chandy-lamport (THIS CRATE — recording layer)      │
│  ┌─────────────────┐ ┌──────────────────┐ ┌─────────────────┐ ┌──────────────┐ │ │
│  │ marker engine   │ │ local state      │ │ channel recorder│ │ snapshot   │ │ │
│  │ (initiate/prop. │ │ capture per node │ │ in-flight msgs  │ │ collator     │ │ │
│  │  /terminate)    │ │ (ids-000-aa types│ │ since state save│ │ GlobalSnapshot│ │ │
│  └─────────────────┘ └──────────────────┘ └─────────────────┘ │ + channel    │ │ │
│  ┌─────────────────┐ ┌──────────────────┐ │                   │ states       │ │ │
│  │ consistent cut  │ │ stable property  │ │                   └──────────────┘ │ │
│  │ validator       │ │ detector         │ │                   │                │ │ │
│  │ (happens-before │ │ (termination/    │ │                   │                │ │ │
│  │  closure)       │ │  deadlock)       │ │                   │                │ │ │
│  └─────────────────┘ └──────────────────┘ │                   │                │ │ │
│  ┌─────────────────┐ ┌──────────────────┐ │                   │                │ │ │
│  │ checkpoint/     │ │ barrier adapter  │ │                   │                │ │ │
│  │ restore (save,  │ │ (Flink ABS:      │ │                   │                │ │ │
│  │  reset+rewind)  │ │  numbered        │ │                   │                │ │ │
│  └─────────────────┘ │   barriers,      │ │                   │                │ │ │
│                      │   aligned/unal.) │ │                   │                │ │ │
│                      └──────────────────┘ │                   │                │ │ │
│  ┌─────────────────┐ ┌──────────────────┐ │                   │                │ │ │
│  │ audit log       │ │ crypto surface   │ │                   │                │ │ │
│  │ (hash chain +   │ │ ed25519/sha256/  │ │                   │                │ │ │
│  │  signed cp.)    │ │  blake3/xchacha/ │ │                   │                │ │ │
│  └─────────────────┘ └──────────────────┘ └───────────────────┴──────────────┘ │ │
└──────────────────────────────────────────────────────┬───────────────────────────┘
                                                       │ opaque Vec<u8> datagrams
                                                       ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│                  CARRIER BOUNDARY: ids-carrier::Endpoint only                    │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])   │
│  receive_datagram → (Endpoint, Vec<u8>)                                          │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS /        │
│  Wi-Fi / Bluetooth / cellular / VPN / mesh anywhere in the crate                 │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a Chandy-Lamport snapshot:** any process may initiate. The initiator records its local state, then sends a marker on each outbound channel before sending any further application message on that channel. On receiving a first marker: record own local state, mark the inbound channel empty, forward markers on all outbound channels, begin recording messages on all other inbound channels. On receiving a subsequent marker on channel `c`: record `c`'s state as the messages recorded since the local state was saved, stop recording on `c`. Termination: every process has received a marker on every inbound channel. The collator assembles `GlobalSnapshot = {local_state(p) for all p} ∪ {channel_state(c) for all c}`, which is a consistent cut (if a receive event is in the cut, the matching send event is also in the cut — guaranteed under FIFO channels).

**Data flow of a Flink-style barrier checkpoint:** a numbered checkpoint barrier is injected at sources and flows through the operator graph with the data. In aligned mode, an operator forwards input only after barriers from all inputs have arrived (barrier alignment); in unaligned mode (Flink 1.11+), in-flight data is buffered in state so checkpoints are not blocked by backpressure. On barrier completion, the operator snapshots state and acks; when all operators ack, the checkpoint is durable. Exactly-once restore requires replayable sources and idempotent/transactional sinks, with rewind to the barrier position.

**Consistency semantics:** the recorded global state may never have occurred at a single instant, but it is a reachable state — a consistent reordering of events exists that passes through it. This is sufficient for checkpoint/restore and stable property detection.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline; the full source list is in Section 12.

#### 3.1 Chandy-Lamport marker algorithm (original, 1985)

**Source:** Chandy & Lamport, "Distributed Snapshots: Determining Global States of Distributed Systems", ACM Transactions on Computer Systems 3(1), Feb 1985, pp. 63–75. https://lamport.org/pubs/chandy.pdf ; mirror https://cs.cmu.edu/~gibbons/15-712/papers/chandy85.pdf

Assumptions (base algorithm): no process or channel failures; all messages arrive intact, exactly once; channels are unidirectional and FIFO ordered; a communication path exists between any two processes; any process may initiate the snapshot; the algorithm does not interfere with normal execution; each process records only its own local state and the states of its incoming channels.

Rules:
- Marker sending rule (initiator and on first marker at a process): record local state, then send a marker on each outgoing channel before any further application message on that channel.
- Marker receiving rule (first marker at process p): record p's local state; mark the state of the channel the marker arrived on as empty; send markers on all outbound channels; start recording messages on all other inbound channels.
- Marker receiving rule (subsequent marker on channel c at p): record the state of c as all messages received on c since p recorded its local state and before this marker; stop recording on c.
- Termination: the snapshot is complete when every process has received a marker on every inbound channel.

Marker messages are control messages distinct from application messages and participate in the same FIFO ordering; a marker on a channel separates pre-snapshot ("white") messages from post-snapshot ("red") messages. A process can snapshot without stopping the application, and multiple processes may initiate snapshots concurrently without interfering.

#### 3.2 Consistent global state and consistent cuts

**Source:** Illinois ECE428, "Consistent Cuts / The Snapshot Algorithm" slides. https://courses.grainger.illinois.edu/ECE428/ECE428/sp2018/snapshots.pdf ; Princeton COS418 lecture L4 (vector clocks + marker idea). https://cs.princeton.edu/courses/archive/fall18/cos418/docs/L4-vc.pdf ; Cornell CS6410, Lamport ordering and consistent cuts. https://cs.cornell.edu/courses/cs6410/2011fa/lectures/19-lamport.pdf

A cut `C` of a distributed computation is consistent iff it is left-closed under happens-before: `e ∈ C ∧ f → e ⇒ f ∈ C`. Graphically, every channel arrow crossing the cut must cross forward (send before cut, receive after cut); a message received in the cut without its send in the cut is inconsistent (an effect without its cause). A global state is consistent iff it corresponds to a consistent cut.

Chandy-Lamport guarantees every run produces a consistent cut: if receive event `e_j` is in the cut then send event `e_i` (`e_i → e_j`) is also in the cut, by FIFO ordering (markers on each link precede later application messages). The recorded global state is "possible": there need not be a single instant at which it occurred, but some reachable reordering of the execution passes through it. This reachable-state semantics is exactly what checkpoint/restore needs: restore reinstalls a state the system could have reached.

#### 3.3 Channel state and in-flight messages

**Source:** Chandy-Lamport 1985 paper; IISc distributed-systems slides on Chandy-Lamport. https://cds.iisc.ac.in/wp-content/uploads/DS256.2018.L4.Global.Snapshot.pdf ; decomposition.al worked example. https://decomposition.al/blog/2019/04/26/an-example-run-of-the-chandy-lamport-snapshot-algorithm

The state of channel `c = (p, q)` recorded at process `q` is `M(c) = sent(c, t_p) − received(c, t_q)`: messages sent on `c` up to `p`'s snapshot time, minus messages received on `c` up to `q`'s snapshot time — equivalently, the messages recorded on `c` between `q`'s local-state save and `q`'s receipt of the marker on `c` (empty if the marker arrived before any post-save message). These are exactly the messages in flight across the cut. The crate records channel state incrementally per channel: a per-channel recording buffer started at local-state save and closed at marker receipt (first marker → empty; subsequent marker → buffer contents).

#### 3.4 Stable property detection

**Source:** Chandy-Lamport 1985 (stable properties: "computation has terminated", "the system is deadlocked", "all tokens in a token ring have disappeared"); Springer, "Efficient detection of a class of stable properties". https://link.springer.com/article/10.1007/bf02280830 ; JPdc 2006, "Efficient detection of a locally stable predicate in a distributed system". https://doi.org/10.1016/j.jpdc.2006.12.004

A stable property is a predicate that, once true, remains true forever. Detection: repeatedly take consistent snapshots and test the predicate over the collected local + channel states; once the property holds, every later snapshot also satisfies it, so repeated snapshots eventually detect it. Examples: computation termination, deadlock, quiescence, token disappearance. The crate exposes `StableProperty` evaluation (predicate over `GlobalSnapshot`) plus a re-snapshot scheduler; it does not decide which properties to track — that is configured by the consumer crate. Locally stable predicates can in principle be detected with weaker assumptions than full Chandy-Lamport (no control messages, possibly-inconsistent snapshots) — noted as a research alternative, not the default.

#### 3.5 Checkpoint/restore foundation

**Source:** Chandy-Lamport 1985 ("Global state detection can also be used for checkpointing"); etcd snapshot/restore docs. https://github.com/etcd-io/etcd/blob/v3.4.14/Documentation/op-guide/recovery.md ; Open MPI interconnect-agnostic checkpoint/restart (ACM DL 1551609.1551619). https://dl.acm.org/doi/10.1145/1551609.1551619 ; arxiv 2510.06404, "Asynchronous Checkpoint for Eventually Consistent Databases" (DTCS/MuFASA, overhead of traditional snapshots). https://arxiv.org/html/2510.06404v2

Checkpoint/restore built on consistent global state: save `GlobalSnapshot` (all local states + all channel states) plus a snapshot identifier to durable storage; restore reinstalls every process's state and every channel's in-flight messages, then resumes execution from that consistent cut. Because the cut is consistent, no restored receive lacks its send — restored execution is causally sound. The crate defines the save/restore contract (serialized snapshot envelope, checksum, restore validation) but not the storage backend (bytes only, shipped via `ids-carrier::Endpoint`; durable-store crate TBD — see Section 10).

#### 3.6 Flink-style barrier snapshotting integration

**Source:** Apache Flink docs, "Fault Tolerance via State Snapshots" (asynchronous barrier snapshotting). https://nightlies.apache.org/flink/flink-docs-release-1.11/learn-flink/fault_tolerance.html ; Flink docs, "Checkpointing" (aligned vs unaligned checkpoints, exactly-once vs at-least-once). https://nightlies.apache.org/flink/flink-docs-master/docs/dev/datastream/fault-tolerance/checkpointing ; Flink docs, "Savepoints". https://nightlies.apache.org/flink/flink-docs-release-1.5/ops/state/savepoints.html ; Flink blog, "From Aligned to Unaligned Checkpoints – Part 1". https://flink.apache.org/2020/10/15/from-aligned-to-unaligned-checkpoints-part-1-checkpoints-alignment-and-backpressure

Flink uses a Chandy-Lamport variant called asynchronous barrier snapshotting (ABS): the checkpoint coordinator injects numbered checkpoint barriers at sources; barriers flow through the operator graph with the data stream and mark the before/after parts of each stream for a checkpoint. On receiving a barrier: aligned mode buffers each input until barriers from all inputs arrive (barrier alignment), then snapshots state; unaligned mode stores in-flight data into the operator state so the barrier proceeds immediately, trading checkpoint state size for latency under backpressure (Flink 1.11+). Exactly-once guarantees require replayable sources and transactional/idempotent sinks (e.g., two-phase commit with Kafka: pre-commit at checkpoint start, commit at completion). Checkpoints are automatic recovery snapshots (can be incremental); savepoints are user-triggered for operational redeploy/upgrade/rescaling and are always complete.

The crate's barrier adapter mirrors ABS: `BarrierId` numbered barriers injected at entry points, per-operator barrier alignment state, aligned/unaligned modes selectable per checkpoint, `CheckpointRef` + durable envelope, and an exactly-once restore contract (rewind inputs to barrier position, replay from sources). The IDS stack has no streaming job graph yet, so the barrier adapter is specified as a pluggable layer with operator boundaries supplied by consumer crates — see Section 10.

#### 3.7 Algorithm variants and relaxations

**Source:** McGill snapshot-algorithm notes (no-control-message, non-FIFO algorithm). https://cs.mcgill.ca/~carl/distsnaply.pdf ; IEEE, "Fast and Message-Efficient Global Snapshot Algorithms for Large-Scale Distributed Systems" (Chandy-Lamport O(n²) control messages; survey of variants). https://ieeexplore.ieee.org/document/5401156 ; PODC '90 StableSnap self-stabilizing iterated snapshot. https://dl.acm.org/doi/10.1145/93385.93405 ; Retroscope HLC-based consistent cuts (cut consistent iff vector-clock condition K(i) = V_i(i) ∀i). https://ieeexplore.ieee.org/document/8693529 ; arxiv 2008.11837 crash-tolerant atomic snapshot via lattice agreement. https://arxiv.org/pdf/2008.11837v1

Variants researched (documented, not default): Mattern — non-FIFO channels with explicitly colored messages; Lai-Yang — non-FIFO channels with sent/received-set exchange (higher overhead); Spezialetti-Kearns — concurrent snapshot initiation optimizations; causal-order snapshot algorithms using existing causal-delivery metadata with no extra control messages; Retroscope-style hybrid-logical-clock consistent cuts for retrospective snapshots; crash-tolerant atomic snapshots via lattice agreement. The IDS crate defaults to Chandy-Lamport over assumed-FIFO carrier channels; variant selection is a deployment decision captured in Section 10.

#### 3.8 Lattice theory of consistent cuts (stable detection support)

**Source:** Garg et al., lattice-theoretic monitoring of distributed computations (consistent cuts form a distributive lattice; order ideals = consistent global states). https://users.ece.utexas.edu/~garg/dist/rv14-slides.pdf ; formal consistent-cut definition and predicate detection. https://cs.cornell.edu/courses/cs5414/2010fa/publications/BM93.pdf

The set of all consistent cuts of a computation forms a distributive lattice under set inclusion; each consistent global state is an order ideal of the happens-before poset. Predicate detection over this lattice underpins stable property detection (repeated snapshots sample cuts of the lattice). StableSnap-style self-stabilizing iteration (round numbers, prod messages, repeated snapshots from process 0) is the reference pattern for repeated snapshot detection. The crate exposes a `CutLattice` interface (cut validity, join/meet of consistent cuts) consumed by the stable property detector.

#### 3.9 Carrier-agnostic transport mapping

**Source:** IDS plan format references (plan-060.md, plan-048.md) — carrier-agnostic absolute rule from prompts.txt entry 1; Flink barrier flow as logical streams (no IP assumptions in the algorithm itself).

All snapshot traffic is mapped onto `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`): each logical channel between two nodes is an ordered datagram stream over an `Endpoint` pair; markers are a distinct datagram type framed inside the opaque payload; snapshots and checkpoints are serialized envelopes sent as datagrams. The crate depends on `ids-carrier` only for `Endpoint` and the `Carrier` trait (`send_datagram`/`receive_datagram`); it never constructs addresses, never touches `std::net`/`tokio::net`, and assumes nothing about the underlying carrier (loopback, serial, optical, radio, custom). FIFO per channel is assumed for the base algorithm (consistent-cut guarantee); non-FIFO carrier behavior falls back to the variant algorithms of Section 3.7.

#### 3.10 Allowed crypto usage

**Source:** RFC 8032 (Ed25519) / ed25519-dalek docs.rs; RFC 7539 / chacha20poly1305 docs.rs (ChaCha20-Poly1305 + XChaCha20-Poly1305); NIST FIPS 180-4 (SHA-256) / sha2 RustCrypto docs; BLAKE3-team README (BLAKE3-256, SIMD, keyed hash, KDF, XOF).

Allowed set only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305. In `ids-chandy-lamport`: ed25519 signs snapshot envelopes and audit checkpoints (proof that a recorded cut was taken at a given barrier); sha256 and blake3 digest channel states and snapshot IDs and chain audit entries; xchacha20poly1305 (random 192-bit nonces) and chacha20poly1305 (12-byte nonces, bounded use) encrypt snapshot state at rest when confidentiality is required. Crypto operations are implemented in `ids-crypto`; this crate consumes primitive IDs and envelope types from `ids-000-aa`/`ids-crypto` and performs no raw cryptography itself.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Marker lost, duplicated, or delayed by carrier | Snapshot consistency / termination | First-marker gate makes duplicate markers harmless; markers carry per-snapshot sequence ids; termination waits for markers on all inbound channels with bounded timeouts; carrier-agnostic boundary means no network stack in trust base |
| Non-FIFO carrier channel | Consistent-cut guarantee | Base algorithm assumes FIFO per Endpoint channel (documented assumption); if violated, use colored-message (Mattern) or sent/received-set (Lai-Yang) variants from Section 3.7 |
| Recorded global state never actually occurred | Restore correctness | Documented reachable-state semantics: some execution reordering passes through the cut; restore is causally sound |
| Stable property not detected promptly (false negative) | Detection soundness | Repeated snapshots over the lattice of consistent cuts; once a stable property holds, all later cuts satisfy it |
| Restore resurrects stale or causally incomplete state | Recovery correctness | Consistent-cut closure (every receive's send is present); Flink-style barrier completion + rewind; exactly-once contract needs replayable sources and idempotent sinks (consumer crates) |
| Barrier alignment blocked by backpressure | Checkpoint latency | Unaligned mode: buffer in-flight data in operator state (Flink 1.11+ pattern) so checkpoints proceed under backpressure |
| Snapshot/checkpoint envelope tampering | Snapshot authenticity | ed25519-signed envelopes and audit checkpoints; sha256/blake3 hash chain over audit entries; offline verifiable |
| Snapshot state read at rest | Confidentiality | xchacha20poly1305 / chacha20poly1305 encryption of snapshot state at rest; keys never leave the node |
| Checkpoint/restore replay attacks | Replay integrity | Nonce-bound barrier ids; bounded replay cache; monotonic `IdsTime` (never wall-clock) for timestamps |
| Carrier re-framing or reordering datagrams | Ordering/duplication | Idempotent marker and message recording; duplicate messages tolerated by channel-state reconstruction |
| Snapshot initiation storm (many concurrent snapshots) | Resource exhaustion | Snapshot IDs namespace; concurrent snapshots allowed (per Chandy-Lamport) but rate-limitable via config |

The carrier is never trusted: snapshot decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-chandy-lamport ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` (exists at C:/idfs/ids-000-aa/) | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, `IdsTime`, `Error` consumed, never redefined; sealed `Snapshot`/`Recovery`/`Carrier` contracts mirrored |
| `ids-carrier` (planned, Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B), `Carrier` trait send/receive datagram | All marker, snapshot, checkpoint traffic flows via `Endpoint`; crate returns envelopes, never sends directly |
| `ids-crypto` (planned, Phase 003) | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing envelopes/checkpoints; hashing channel states and snapshot IDs; encrypting snapshot state at rest |
| `ids-time` (planned) | Monotonic `IdsTime` time source | Snapshot timestamps and termination timeouts; never wall-clock for causality |
| `ids-checkpoint` / `ids-restore` (planned consumers) | Durable persistence of `GlobalSnapshot` envelopes and restore reinstatement | Consume snapshot envelopes from this crate; restore resets processes/channels to the cut and rewinds inputs |
| `ids-store`, `ids-replication`, `ids-recovery` (future users) | Record store, replica synchronization, recovery | Users of stable property detection and checkpoint/restore foundation |
| `ids-query` / ids operator crates (future) | Operators behind barrier boundaries | Barrier adapter injection points for Flink-style ABS |

**Integration points:** `ids-carrier` implements `Carrier`; `ids-chandy-lamport` declares usage but no transport implementation. `ids-crypto` implements the allowed primitives; this crate references primitive IDs via `ids-000-aa` only. All cross-node traffic is opaque `Vec<u8>` to `ids-carrier::Endpoint`; no dependency on `std::net`, `tokio::net`, libp2p, or any carrier-specific crate. Cargo workspace: member crate at `C:/idfs/ids-chandy-lamport/`, depending on `ids-000-aa = { path = "../ids-000-aa" }`, `ids-carrier = { path = "../ids-carrier" }`, `ids-crypto = { path = "../ids-crypto" }` when those crates exist.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-chandy-lamport/` (a separate crate, not a folder inside an existing crate):

```
ids-chandy-lamport/
├── Cargo.toml                          # serde, thiserror; workspace member; ids-000-aa, ids-carrier, ids-crypto deps
├── src/
│   ├── lib.rs                          # crate root; public API surface; feature gates
│   ├── types.rs                        # SnapshotId, Marker, SnapshotConfig, ChannelState, GlobalSnapshot,\
│   │                                   SnapshotStatus, BarrierId, Barrier, CheckpointRef (re-export ids-000-aa types)
│   ├── errors.rs                       # SnapshotError (thiserror): marker, channel, cut, barrier, restore errors
│   ├── marker_engine.rs                # marker send/receive rules; first- and subsequent-marker handling
│   ├── channel_recorder.rs             # per-channel in-flight message recording buffers (open/close at markers)
│   ├── local_state.rs                  # process local state capture (application-state serialization hook)
│   ├── snapshot_coordinator.rs         # initiation, termination detection (marker on all inbound channels), collation
│   ├── consistent_cut.rs               # consistent-cut validation (happens-before closure); cut lattice interface
│   ├── stable_property.rs              # stable predicate evaluation (termination, deadlock, quiescence); re-snapshot scheduler
│   ├── checkpoint.rs                   # save/restore contract for GlobalSnapshot; envelope + checksum + restore validation
│   ├── barrier.rs                      # Flink-style numbered barriers; barrier lifecycle; aligned/unaligned mode selection
│   ├── barrier_alignment.rs            # per-operator barrier alignment state; unaligned in-flight buffering
│   ├── restore.rs                      # restore reinstatement: install states, reinstall channel messages, rewind inputs
│   ├── audit.rs                        # append-only audit log: hash chain + ed25519-signed checkpoints
│   ├── contracts/                      # sealed trait contracts (~12 files)
│   │   ├── mod.rs\n│   │   ├── marker_contract.rs      # trait MarkerEngine\n│   │   ├── snapshot_contract.rs    # trait SnapshotCollator\n│   │   ├── channel_contract.rs     # trait ChannelRecorder\n│   │   ├── state_contract.rs       # trait LocalStateCapture\n│   │   ├── cut_contract.rs         # trait ConsistentCut\n│   │   ├── stable_property_contract.rs # trait StableProperty\n│   │   ├── checkpoint_contract.rs  # trait CheckpointRestore\n│   │   ├── barrier_contract.rs     # trait BarrierAdapter\n│   │   ├── carrier_contract.rs     # trait Carrier usage (Endpoint datagrams)\n│   │   ├── audit_contract.rs       # trait AuditLog\n│   │   └── restore_contract.rs     # trait RestoreValidator
│   ├── invariants/                     # compile-time/runtime invariants (~8 files)
│   │   ├── mod.rs\n│   │   ├── fifo_channel_assumed.rs\n│   │   ├── marker_once_per_channel.rs\n│   │   ├── cut_consistent.rs\n│   │   ├── termination_detected.rs\n│   │   ├── carrier_agnostic.rs\n│   │   ├── crypto_allowlist.rs\n│   │   └── snapshot_signed.rs
│   ├── crypto_surface.rs               # allowed primitive IDs only; delegation to ids-crypto (ed25519/sha256/blake3/xchacha/chacha20)
│   └── prelude.rs                      # re-export glob for downstream crates
├── tests/                              # property/verification tests (~30 files)
│   ├── marker_tests.rs\n│   ├── channel_state_tests.rs\n│   ├── consistent_cut_tests.rs\n│   ├── stable_property_tests.rs\n│   ├── barrier_alignment_tests.rs\n│   ├── checkpoint_restore_tests.rs\n│   ├── exactly_once_restore_tests.rs\n│   ├── carrier_agnostic_tests.rs     # no prohibited crate references\n│   └── crypto_allowlist_tests.rs
├── benches/                            # (~10 files)
│   ├── marker_latency.rs\n│   ├── snapshot_collation.rs\n│   ├── barrier_alignment.rs\n│   └── checkpoint_save_restore.rs
└── docs/                               # (~14 files)
    ├── marker_algorithm.md\n│   ├── consistent_cut.md\n│   ├── stable_property_detection.md\n│   ├── barrier_snapshotting.md\n│   ├── checkpoint_restore.md\n│   └── integration.md
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **Marker protocol**: initiator records local state then sends a marker on each outbound channel before any further message on that channel; first-marker rule at a process (record state, channel empty, propagate markers, record other inbound channels); subsequent-marker rule (channel state = recorded messages since state save; stop recording).
- **ChannelState**: incremental per-channel buffer of messages received between local-state save and marker receipt; equals in-flight messages at the cut (`sent up to sender's save − received up to receiver's save`).
- **LocalState**: application state snapshot hook (serialized via serde; consumer defines what state is captured).
- **GlobalSnapshot**: `{local_state(p) ∀p} ∪ {channel_state(c) ∀c}`; a consistent cut under FIFO; reachable state (not necessarily an actually-occurred instant).
- **Consistent cut**: left-closed under happens-before (`e ∈ cut ∧ f → e ⇒ f ∈ cut`); validator checks every cross-cut channel arrow crosses forward.
- **Cut lattice**: order ideals of the happens-before poset; join/meet of consistent cuts for stable detection support.
- **Stable property**: predicate over `GlobalSnapshot` that stays true once true; examples termination, deadlock, quiescence; detected via repeated snapshots.
- **Barrier (Flink ABS)**: numbered `BarrierId` injected at entry points, flows with data; aligned mode waits for barriers from all inputs; unaligned mode buffers in-flight data in state; checkpoint completes when all operators ack or is discarded.
- **Checkpoint/restore**: save `GlobalSnapshot` + `BarrierId` as signed envelope; restore reinstalls all states and channel messages, rewinds input positions to the barrier, resumes; exactly-once needs replayable sources + idempotent/transactional sinks.
- **Audit**: each snapshot/barrier/checkpoint decision = sha256/blake3-chained entry + ed25519-signed checkpoint; offline verifiable.
- **Crypto**: ed25519 signatures on envelopes/checkpoints; sha256/blake3 digests and snapshot IDs; xchacha20poly1305/chacha20poly1305 at-rest encryption of snapshot state.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `RecordId`, `Checksum`, `Endpoint`, `CarrierType`, `IdsTime`, `Error`) and sealed trait contracts; master crate, consumed only.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait (`send_datagram`/`receive_datagram`); transport boundary only; crate never sends bytes itself.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of marker/snapshot/checkpoint envelopes and audit entries.
- `thiserror` — unified `SnapshotError` contract.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-crypto` does not depend on `ids-000-aa` to avoid a circular dependency (pattern established in plan-048).

---

### 9. Measurable Acceptance Criteria

1. Marker initiation: any node may initiate; initiator records local state, then sends a marker on each outbound channel before any further message on that channel.
2. First-marker rule: on first marker, record local state, mark inbound channel empty, propagate markers on all outbound channels, record all other inbound channels.
3. Subsequent-marker rule: channel state = messages recorded since local state save; recording stops on that channel.
4. Termination: snapshot complete iff every node has received a marker on every inbound channel.
5. Consistent cut: 100% of recorded cuts satisfy happens-before left-closure (if a receive is in the cut, the matching send is in the cut) — under the FIFO channel assumption.
6. Channel state correctness: recorded channel state equals in-flight messages at the cut (`sent(c, t_p) − received(c, t_q)`).
7. Stable property detection: termination/deadlock/quiescence predicates evaluated on recorded `GlobalSnapshot`; repeated snapshots eventually detect any stable property that becomes true.
8. Barrier snapshotting: numbered barriers flow end-to-end; aligned mode blocks until all-input barriers arrive; unaligned mode stores in-flight data; checkpoints complete or are discarded cleanly.
9. Checkpoint/restore: `GlobalSnapshot` saved as signed envelope via `ids-carrier::Endpoint` opaque datagrams; restore reinstates snapshot state and rewinds inputs to barrier position; exactly-once restore with replayable sources/idempotent sinks.
10. Carrier-agnostic: zero references to `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh; all traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B).
11. Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in the crate's crypto surface; snapshot envelopes and audit checkpoints ed25519-signed.
12. Audit: every snapshot/barrier/checkpoint decision recorded in an append-only, sha256/blake3 hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime`; offline verifiable.

---

### 10. Remaining Uncertainties

- **FIFO channel assumption over `ids-carrier`:** the base Chandy-Lamport guarantee requires FIFO per channel; IDS datagram channels over `Endpoint` may not guarantee FIFO — confirm or adopt colored-message (Mattern) / sent-received-set (Lai-Yang) variants (Section 3.7).
- **Marker framing:** whether markers travel as a distinct datagram type or are piggybacked on application traffic; affects `ids-carrier` framing contract (TBD with `ids-carrier` Phase 002).
- **Snapshot storage backend:** durable store for `GlobalSnapshot` envelopes is out of scope here (bytes only via `ids-carrier`); backend crate TBD at integration with `ids-checkpoint`/`ids-restore`.
- **Multi-initiator coexistence:** original algorithm permits any number of concurrent initiators; interaction with barrier checkpoints (single coordinator) needs definition.
- **Termination timeouts:** bounded wait values for marker completion under carrier delay — deployment-tuned, validated at integration.
- **Flink job graph mapping:** IDS has no streaming job graph/operator model yet; barrier adapter boundaries supplied by consumer crates — design decision pending.
- **Exactly-once sink contract:** transactional/idempotent sink requirements for restore to be defined by downstream crates (ids-store/ids-query).
- **Snapshot granularity:** full state vs incremental checkpoints (Flink incremental checkpoints) — sizing and delta strategy TBD.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-chandy-lamport` (not a folder inside an existing crate) | Follows "separate crates instead of more folders" directive; clear recording-layer boundary |
| Chandy-Lamport base algorithm (FIFO, no failures) as default | Original 1985 algorithm; simple, proven consistent-cut guarantee; well-documented assumptions |
| Non-FIFO variants (Mattern coloring, Lai-Yang) documented, not default | Carrier FIFO behavior unconfirmed; variants kept as research alternatives for integration |
| Flink-style barrier snapshotting as separate barrier submodule | Numbered barriers, aligned/unaligned modes, exactly-once restore; operator boundaries pluggable since IDS has no job graph yet |
| Recorded state = consistent cut / reachable state, not necessarily an actually-occurred instant | Matches Chandy-Lamport semantics; sufficient and sound for checkpoint/restore |
| Stable property detection via repeated snapshots + predicate evaluation over the cut lattice | Stable properties persist once true; repeated sampling eventually detects them |
| Marker as first-class control datagram via `ids-carrier::Endpoint` | Keeps snapshot traffic distinguishable; FIFO per Endpoint channel assumed |
| Crypto: ed25519 signatures (envelopes/checkpoints), sha256/blake3 digests, xchacha20poly1305/chacha20poly1305 at-rest encryption | Complies with allowed-crypto rule; crypto implemented in `ids-crypto`, type surface from `ids-000-aa` |
| `ids-000-aa` types only; no redefinition of canonical types | Single canonical definition per type; downstream crates import `ids_000_aa::X` |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence for snapshots |

---

### 12. Sources

[^1]: K. M. Chandy and L. Lamport, "Distributed Snapshots: Determining Global States of Distributed Systems", ACM Transactions on Computer Systems 3(1), 1985, pp. 63–75. https://lamport.org/pubs/chandy.pdf
[^2]: L. Lamport, EWD 864a, "The Distributed Snapshot of K. M. Chandy and L. Lamport". https://cs.utexas.edu/~EWD/transcriptions/EWD08xx/EWD864a.html
[^3]: Wikipedia, "Chandy–Lamport algorithm" (assumptions, marker rules, consistent cut). https://en.wikipedia.org/wiki/Chandy–Lamport_algorithm
[^4]: University of Illinois ECE 428, "Consistent Cuts / The Snapshot Algorithm" slides. https://courses.grainger.illinois.edu/ECE428/sp2018/snapshots.pdf
[^5]: Princeton COS 418, Lecture 4, vector clocks and distributed snapshot marker idea. https://cs.princeton.edu/courses/archive/fall18/cos418/docs/L4-vc.pdf
[^6]: Cornell CS 6410, Lamport: ordering and consistent cuts. https://cs.cornell.edu/courses/cs6410/2011fa/lectures/19-lamport.pdf
[^7]: McGill University, distributed snapshot algorithm notes (non-FIFO, no control messages). https://cs.mcgill.ca/~carl/distsnaply.pdf
[^8]: decomposition.al, "An example run of the Chandy-Lamport snapshot algorithm" (worked example, channel states). https://decomposition.al/blog/2019/04/26/an-example-run-of-the-chandy-lamport-snapshot-algorithm
[^9]: IISc, DS256.2018.L4, Global Snapshot (Chandy-Lamport) slides. https://cds.iisc.ac.in/wp-content/uploads/DS256.2018.L4.Global.Snapshot.pdf
[^10]: IEEE Xplore 5401156, "Fast and Message-Efficient Global Snapshot Algorithms for Large-Scale Distributed Systems" (Chandy-Lamport O(n²) control messages; variant survey). https://ieeexplore.ieee.org/document/5401156
[^11]: V. K. Garg et al., lattice-theoretic monitoring of distributed computations (consistent cuts form a distributive lattice). https://users.ece.utexas.edu/~garg/dist/rv14-slides.pdf
[^12]: Cornell CS 5414, BM93.pdf — consistent cuts, global state, happens-before formalism. https://cs.cornell.edu/courses/cs5414/2010fa/publications/BM93.pdf
[^13]: Journal of Parallel and Distributed Computing 2006, "Efficient detection of a locally stable predicate in a distributed system". https://doi.org/10.1016/j.jpdc.2006.12.004
[^14]: Springer, "Efficient detection of a class of stable properties" (stable property detection via snapshots). https://link.springer.com/article/10.1007/bf02280830
[^15]: ACM PODC '90, StableSnap self-stabilizing iterated Chandy-Lamport snapshot. https://dl.acm.org/doi/10.1145/93385.93405
[^16]: IEEE Xplore 8693529, Retroscope — HLC-based consistent cuts for retrospective snapshots. https://ieeexplore.ieee.org/document/8693529
[^17]: arXiv 2008.11837, "Amortized Constant Round Atomic Snapshot in Message Passing Systems" (crash-tolerant snapshot via lattice agreement). https://arxiv.org/pdf/2008.11837v1
[^18]: arXiv 2510.06404, "Asynchronous Checkpoint for Eventually Consistent Databases" (DTCS, MuFASA, snapshot overhead). https://arxiv.org/html/2510.06404v2
[^19]: Apache Flink, "Fault Tolerance via State Snapshots" (asynchronous barrier snapshotting). https://nightlies.apache.org/flink/flink-docs-release-1.11/learn-flink/fault_tolerance.html
[^20]: Apache Flink, "Checkpointing" (aligned vs unaligned checkpoints; exactly-once vs at-least-once). https://nightlies.apache.org/flink/flink-docs-master/docs/dev/datastream/fault-tolerance/checkpointing
[^21]: Apache Flink, "Savepoints". https://nightlies.apache.org/flink/flink-docs-release-1.5/ops/state/savepoints.html
[^22]: Flink blog, "From Aligned to Unaligned Checkpoints – Part 1: Checkpoints, Alignment, and Backpressure". https://flink.apache.org/2020/10/15/from-aligned-to-unaligned-checkpoints-part-1-checkpoints-alignment-and-backpressure
[^23]: etcd v3.4.14, operation guide: snapshot and restore. https://github.com/etcd-io/etcd/blob/v3.4.14/Documentation/op-guide/recovery.md
[^24]: ACM DL 1551609.1551619, "Interconnect agnostic checkpoint/restart in Open MPI". https://dl.acm.org/doi/10.1145/1551609.1551619
[^25]: docs.rs theta::persistence — Rust actor snapshot/restore prior art. https://docs.rs/theta/latest/theta/persistence
[^26]: docs.rs axactor::rt::Actor — `snapshot()`/`restore()` trait prior art. https://docs.rs/axactor/latest/axactor/rt/trait.Actor.html
[^27]: RFC 8032 (Ed25519) / ed25519-dalek docs.rs. https://docs.rs/ed25519-dalek
[^28]: RFC 7539 / chacha20poly1305 docs.rs (ChaCha20-Poly1305, XChaCha20-Poly1305). https://docs.rs/chacha20poly1305
[^29]: NIST FIPS 180-4 (SHA-256) / sha2 RustCrypto docs.rs. https://docs.rs/sha2
[^30]: BLAKE3-team, BLAKE3 README (BLAKE3-256, SIMD, keyed hash, KDF, XOF). https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^31]: plan-060.md (IDS plan format reference) — C:/idfs/plans/plan-060.md
[^32]: plan-048.md (IDS plan format reference) — C:/idfs/plans/plan-048.md

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 074.
