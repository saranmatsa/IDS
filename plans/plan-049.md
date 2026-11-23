## Phase 049 — Create the ids-doc crate: Spec, TLA+, PlusCal formal models.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 730-744)

```
49. Research only do not read plan.md **[Phase 049 — Create the ids-doc crate: Spec, TLA+, PlusCal formal models.]**

Crate: ids-doc
Purpose: Spec, TLA+, PlusCal formal models.
Contents: IDS Architecture Specification, TLA+ specs (session, stream, wire), PlusCal models, formal invariants, protocol conformance specification, determinism proofs, conformance suite.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 049 — Create the ids-doc crate: Spec, TLA+, PlusCal formal models.]**

Create plan-049.md in C:/idfs/plans/ for the ids-doc crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 049 creates `ids-doc`, the IDS crate that owns the **formal specification layer** of the IDS: the IDS Architecture Specification (natural-language + formal), TLA+ specifications for the session, stream, and wire layers, PlusCal algorithm models, formal invariants, a protocol conformance specification, determinism proofs, and a conformance suite generator. `ids-doc` is a specification-and-verification crate: it ships `.tla`/PlusCal sources and markdown specifications, checked with the TLA+ toolchain (TLC model checker, TLAPS proof system, Apalache symbolic model checker), not Rust implementation logic. It references canonical types and sealed trait contracts from `ids-000-aa` (the master crate) and models all traffic exclusively as opaque `Vec<u8>` datagrams over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

The crate is organized in three horizontal layers matching the IDS data path:

1. **Session layer** — TLA+ spec + PlusCal model of the session state machine: session creation, expiration, session key management, state transitions, client affinity/stickiness, session token management, replay protection (Phase 050 `ids-session` implements this contract).
2. **Stream layer** — TLA+ spec + PlusCal model of stream reassembly, ordering, deduplication, exactly-once delivery, and overlap handling (IDS `ids-stream` implements this contract).
3. **Wire layer** — TLA+ spec + PlusCal model of wire framing, message types, and dispatch over `ids-carrier::Endpoint` only.

Each layer has: a TLA+ module (`Init ∧ [][Next]_vars ∧ Liveness` pattern), an optional PlusCal algorithm model (translated to TLA+, checked by TLC), shared formal invariants, refinement mappings to an abstract IDS spec, determinism proofs, and conformance-suite coverage. Tooling: TLA+ syntactic analyzer, TLC model checker, TLAPS for proofs, Apalache for symbolic checking (state-explosion mitigation), VS Code TLA+ extension or TLA+ Toolbox as authoring environment.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh in `ids-doc` or any spec it references. All traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — any cryptographic primitive mentioned in a spec must come from this list.

A skeleton `ids-doc` crate does NOT yet exist at `C:/idfs/ids-doc/`. `ids-000-aa` declares canonical types (`NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Error`) and sealed trait contracts — all referenced by `ids-doc`, never redefined. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-doc` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is the formal contract layer: every IDS crate that owns a state machine (session, stream, wire) must have a corresponding TLA+/PlusCal spec in `ids-doc`, and its implementation must conform to that spec (verified by the conformance suite). `ids-doc` produces no runtime behavior itself — it produces specifications, models, invariants, proofs, and a conformance-suite generator.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    IDS IMPLEMENTATION LAYER (ids-* crates)                   │
│  ids-session (Phase 050), ids-stream, ids-wire — implement the formal specs  │
├──────────────────────────────────────────────────────────────────────────────┤
│                 Phase 049: ids-doc (FORMAL SPECIFICATION LAYER)              │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │ IDS Arch.    │ │ TLA+ specs     │ │ PlusCal models │ │ Conformance  │ │\n│  │ Specification│ │ (session/      │ │ (session/      │ │ suite gen.   │ │\n│  │ (markdown)   │ │  stream/wire)  │ │  stream/wire)  │ │ (BFS + oracle│ │\n│  └────────────────┘ └────────────────┴────────────────┴── + refusals)  │ │\n│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │\n│  │ Formal         │ │ Protocol       │ │ Determinism    │ │ Tooling      │ │\n│  │ Invariants     │ │ Conformance    │ │ proofs         │ │ (TLC/TLAPS/  │ │\n│  │ (safety/liveness│ │ (refinement    │ │ (inductive     │ │  Apalache)   │ │\n│  │  invariants)   │ │  mappings)     │ │  invariants)   │ │              │ │\n│  └────────────────┘ └────────────────┴────────────────┴──────────────┘ │ │\n└──────────────────────────────────────────────────────────────┬─────────────┘\n                                                                 │ opaque Vec<u8> datagrams\n┌────────────────────────────────────────────────────────────────▼────────────┐\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │\n│  send_datagram(Endpoint{CarrierType ≤ 1024 B addr, Vec<u8>}, payload)      │\n│  receive_datagram → (Endpoint, Vec<u8>)                                    │\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │\n└─────────────────────────────────────────────────────────────────────────────┘
```

**Spec-to-implementation contract per layer:**
1. The TLA+ module defines `Init` (initial-state predicate), `Next` (next-state relation over unprimed/primed variables), and `Spec == Init /\ [][Next]_vars /\ Liveness`.
2. The PlusCal model (where used) encodes the same algorithm imperatively (labels = atomic steps, `await` for guards, `either` for nondeterminism) and is translated to TLA+ (translator inserts `BEGIN TRANSLATION`/`END TRANSLATION`).
3. TLC checks safety invariants and temporal properties over bounded constants; Apalache symbolically checks larger instances; TLAPS proves determinism/refinement theorems.
4. A refinement mapping (`INSTANCE Abstract WITH ...`) proves the concrete layer spec implements the abstract IDS spec (`Concrete!Spec => Abstract!Spec`).
5. The conformance suite is generated mechanically from the model: breadth-first tree over the reachable state space, one test per root-to-leaf path, refusal checks at every state, counterexample traces replayable against the live implementation.

---

### 3. Detailed Research Dimensions

#### 3.1 TLA+ language and toolchain (spec authoring + model checking)

TLA+ (Temporal Logic of Actions, Leslie Lamport) is a formal specification language based on untyped ZF set theory, first-order logic, and temporal logic of actions. A system is specified as allowed behaviors — sequences of states — via `Init` (initial-state predicate) and `Next` (next-state relation: unprimed variables = pre-state, primed = post-state). A typical spec has the form `Init /\ [][Next]_vars /\ Liveness`, where stuttering steps (unchanged variables) are allowed so specs are stutter-invariant and composable. Specifications are organized in modules that `EXTEND` other modules (e.g. `Integers`, `Sequences`, `TLC`, `FiniteSets`).

Toolchain:
- **TLA+ syntactic analyzer** — parses modules; the VS Code TLA+ extension (TLA+ Foundation) or TLA+ Toolbox runs "Parse module". Requires a JVM (Java ≥ 11) for the underlying `tla2tools.jar`.
- **TLC model checker** — exhaustively explores all reachable states of a finite instantiation (bounded constants), checks invariants (safety) and temporal properties (liveness), produces counterexample traces on violation. Modes: model checking (exhaustive) and simulation (random, incomplete). Supports symmetry sets (permutes model values, reduces state space ~n!, safety-only — incompatible with liveness checks), constraint bindings, and worker parallelism.
- **TLAPS (TLA+ Proof System)** — interactive proof assistant for theorems asserted in modules (determinism, refinement mappings, inductive invariants), with automatic backends (SMT, provers, propositional temporal logic decision procedure).
- **Apalache** — symbolic model checker for TLA+, complements TLC when state explosion bounds TLC; checks bounded executions without enumerating all states.
- Authoring: VS Code TLA+ extension (Ctrl+T translates PlusCal; runs TLC; visualizes results) or TLA+ Toolbox (GUI, `Translate PlusCal Algorithm` Ctrl+T, `TLC Model Checker` run).

PlusCal (2009, Lamport) is an algorithm language embedded as a comment in a TLA+ module (`(* --algorithm name { ... } *)`), with C-like (c-syntax) and Pascal-like (p-syntax) variants. It translates to human-readable TLA+ (`BEGIN TRANSLATION`/`END TRANSLATION` blocks with checksums). Features: simple imperative constructs, powerful TLA+ expressions, nondeterminism (`either`/`or`), labels marking atomic steps, `await` guards, `with`/`choose` for nondeterministic choice, `print`, `assert`, macros (compile-time inline, no labels/loops), procedures (runtime call stack, labels allowed, `call ... ;` then a label or `return`, recursion allowed, requires `Sequences`), processes (each process has its own `pc`), fair/fair+ processes (weak/strong fairness constraints on actions), label modifiers (`+`, `-`). The translation semantics is formal (a TLA+ spec of the translator itself, executable by TLC).

Sources: Lamport "Specifying Systems" book (lamport.org/tla/book.html); "Specifying and Verifying Systems With TLA+" (Lamport/Matthews, Init/Next/Liveness form); PlusCal paper (Microsoft Research); PlusCal Tutorial sessions (lamport.org/tla/tutorial); Learn TLA+ (learntla.com); tlaplus/Examples repo; vscode-tlaplus docs (Java requirement, install steps); PlusCal translator docs (Inria/TLA+ Toolbox); Distributed PlusCal paper (channels, send/receive primitives); p-manual (p-syntax/c-syntax, Euclid example).

#### 3.2 Session TLA+ specification (session state machine)

Scope: session creation, expiration, session key management, session state machine, client affinity/stickiness, session token management, session replay protection. Modeled as a TLA+ state machine: variables include `sessions` (map SessionId → SessionRecord), `state[session]` ∈ {Created, Active, Expired, Revoked}, `tokens`, `last_activity[session]`, `replay_cache` (nonce + per-peer sequence), `keys[session]` (ed25519-derived session keys from ids-000-aa canonical types). Actions: `CreateSession`, `BindToken`, `Renew`, `Expire`, `Revoke`, `AffinityAssign`, `ReplayCheck`. Fairness: weak fairness on expiry progress so sessions do not stall indefinitely under `fair process` semantics where sessions are modeled as processes. Invariants: at most one `Active` state per SessionId; a token binds to exactly one session; an expired session never transitions back to Active; replay cache rejects (peer, seq) duplicates within TTL. PlusCal model: one `fair process (s ∈ Sessions)` per session with labels for each action, atomic steps between labels, `await` on preconditions (e.g. `await state[self] = "Active"`).

Refs: Learn TLA+ state machines (either/await state machines); tiny-tlaplus-examples StateMachine.tla (fair algorithm, define block, Init/Next/Spec, WF_vars fairness); PlusCal tutorial session 2 (TLC models, constants); Distributed PlusCal (message send/receive channels for session messages over Endpoint).

#### 3.3 Stream TLA+ specification (stream reassembly / ordering)

Scope: stream reassembly, in-order delivery, deduplication, exactly-once semantics, overlap/conflict handling (relevant to NIDS-style stream reassembly where reassembly policy must match the receiver's; overlapping chunks modeled explicitly), carrier-agnostic delivery (loss, duplication, reordering tolerated — no TCP assumptions). Variables: `chunks` (map ChunkId → bytes), `reassembled[stream]` (sequence of bytes), `next_expected[stream]`, `dup_seen`, `gaps[stream]`, `delivery[stream]` (delivered offsets). Actions: `ReceiveChunk`, `Reassemble`, `DeliverInOrder`, `DropDuplicate`, `GapTimeout`. Invariants: delivered bytes form a prefix of the reassembled stream (no gaps in delivery); no byte delivered twice (exactly-once); chunks with overlapping ranges are resolved by a deterministic tie-break rule (e.g., earliest arrival wins, recorded in the spec); delivery order matches stream order on every replica regardless of arrival order. PlusCal model: processes per stream with labels `Receive`, `Reassemble`, `Deliver`; `await` on contiguity (`next_expected` present) before delivery. Determinism: given the same multiset of chunks, every replica reaches the same reassembled state (convergence under reorder/dup/loss — CRDT-style argument adapted to stream reassembly).

Refs: LogPlayer TLA+/PlusCal spec (exactly-once delivery under shard/LogPlayer failures); "Overlapping data in network protocols: bridging OS and NIDS reassembly gap" (overlap types via interval algebra, NIDS reassembly consistency); stream reassembly DPI literature (TCP sequence reassembly, drop repeats); CCF Raft TLA+ specs (TLC nightly + CLI runs).

#### 3.4 Wire TLA+ specification (wire framing + dispatch over Endpoint)

Scope: wire format, framing, message types, dispatch — modeled strictly over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), opaque datagrams, no network stack concepts. Variables: `outbound[Endpoint]` (queue of Vec<u8> frames), `inbound[Endpoint]` (received frames), `wire_state` (framing state machine: awaiting header / reading payload / complete), `msg_types` registry, `framing_errors`. Actions: `SendFrame(endpoint, bytes)`, `ReceiveFrame(endpoint, bytes)`, `ParseHeader`, `ParsePayload`, `Dispatch`, `DropMalformed`. Invariants: every dispatched message passes schema validation before any semantic effect (bomb guards: max frame size, max nesting depth before parse); the carrier is untrusted — messages are authenticated over payload bytes so they survive re-framing; no spec variable references IP/TCP/UDP/ports/DNS; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom} (ids-000-aa canonical type) is the only carrier dimension. PlusCal model: a `process WireDispatcher` with labels per framing step and `await` guards on buffer completeness.

Refs: Practical TLA+ (wire transfer feature example, PlusCal processes); AgentConform / AgentRFC (protocol specs as TLA+ invariants with Protocol/Environment/Adversary action separation — wire layer modeled adversarially: environment = untrusted carrier); M2D2 formal data model for IDS alert correlation (IDS formal modeling precedent).

#### 3.5 PlusCal models (algorithm-level)

PlusCal is used where an imperative algorithm presentation aids review: session handshake, stream reassembly loop, wire dispatch loop. Each `.tla` module contains one or more `--algorithm` blocks inside comments; the translator (`pcal.trans` / VS Code Ctrl+T / Toolbox File → Translate PlusCal Algorithm) inserts the TLA+ translation below. Models use macros for recurring inline patterns (`send(to, msg)` appends to a network queue; `rcv(p, var)` blocks on a FIFO queue — these map directly to `ids-carrier::Endpoint` datagram send/receive), procedures for subroutines with local variables and `call ... ;` + label/return discipline, and `fair process` / `fair+` where liveness (progress) matters. Each PlusCal module carries its own TLC config (constants, invariants, properties). Translation integrity is an acceptance criterion: the inserted translation must round-trip (re-translation produces identical output).

Refs: PlusCal cheat sheet (pluscal.help; macros vs procedures table; call/return semantics); Learn TLA+ procedures and macros (stack representation, `Sequences` requirement); PlusCal translator docs (BEGIN/END TRANSLATION checksums, CTRL+T workflow); Distributed PlusCal (send/receive channels over networks — mapped to Endpoint datagrams); p-manual (c-syntax/p-syntax; Euclid; macro expansion rules).

#### 3.6 Formal invariants (safety + liveness)

Invariants are state predicates true in all reachable states; TLC checks `[]Inv` for each invariant entry. TLA+ has no special "invariant" construct — invariants are ordinary predicates checked by TLC as temporal properties — so `ids-doc` distinguishes: (a) type invariants (every variable is in its allowed domain — checked first, on every state), (b) safety invariants (bad things never happen: no double delivery, no expired session active, no replayed nonce accepted, no frame parsed beyond budget), (c) liveness properties (good things eventually: every session eventually expires or is revoked; every in-order complete stream eventually delivers; every sent frame is eventually dispatched or dropped) expressed with `<>` / `~>` / `[]<>` operators and checked as TLC temporal properties (separate config per invariant, since bundling stops TLC at the first violation). Stutter-invariance: all specs are stutter-invariant (no next-time operator); fairness constraints (`WF_vars`, `SF_vars`) on `fair process` actions rule out infinite stuttering where progress is required. Symmetry sets are used only for safety checks (never liveness) to tame state explosion.

Refs: Learn TLA+ temporal properties ([] / <> / ~> / stutter-invariance); Hillel Wayne weak & strong fairness; Lamport stutter-invariant temporal properties paper; AgentConform (per-invariant TLC configs, counterexample classification); "Specifying and Verifying Systems With TLA+" (safety = Inv, liveness = fairness conditions).

#### 3.7 Protocol conformance specification

Each concrete layer spec (session/stream/wire PlusCal/TLA+) is related to its abstract IDS spec by a refinement mapping: `Concrete!Spec => Abstract!Spec` expressed as `THEOREM Spec => HL!Spec` where `HL == INSTANCE AbstractSpec WITH ...` substitutes concrete variables/expressions for the abstract module's constants and variables (Abadi & Lamport refinement mapping existence; Lamport Specifying Systems Ch. 10; TLA+ Video Course Lecture 10; "Proving Safety" refinement-mapping note). The conformance specification additionally defines: the testable state space (bounded constants), the oracle (state after each action compared against the spec's allowed transitions), refusal checks (at every reachable state, every action the spec forbids must be refused by the implementation), and test generation: breadth-first tree over the reachable state space, one executable test per root-to-leaf path, each unused transition gets a short refusal test, prefix-subsumed tests dropped. This mirrors HoldSpec (machine-checked TLA+ model → conformance suite, 100% seeded-defect detection) and AgentConform (two-phase: model-check invariants, replay counterexample traces against live implementations).

Refs: "Proving Safety" refinement mapping note (Spec => HL!Spec); TLA+ Video Course lecture 10 (AB2 implements AB under refinement mapping); Hillel Wayne "Specification Refinement" (INSTANCE WITH mapping); HoldSpec (arxiv 2610.14419 — TLA+ state machine → conformance suite, BFS test tree, refusal checks, 100% seeded-defect detection); AgentRFC/AgentConform (TLA+ invariants as conformance spec; two-phase checker; counterexample replay against SDKs); Remix framework (conformance checking detects model↔implementation deviations); TraceFix (TLC counterexample → repair loop, replays against implementation).

#### 3.8 Determinism proofs

Determinism property: given the same initial state and the same sequence of inputs, every replica of the layer reaches the same state trajectory (state equivalence under any delivery order/duplication/delay — matching the carrier-agnostic unknown-membership model). Proven as TLAPS theorems: (a) the next-state relation is a deterministic function of (state, input) modulo commutative independent actions; (b) commutative actions commute (confluence); (c) fairness ensures progress. Inductive invariants (inference tools such as endive can discover candidate inductive invariants; TLAPS verifies them) establish that all reachable states satisfy determinism-relevant conditions (e.g., no two distinct states are observationally equivalent-but-divergent). TLC checks bounded instances exhaustively as a complement (not a substitute) to the proof. Determinism is proved separately for session, stream, and wire modules, then compositionally for the stack (refinement preserves determinism under the refinement mapping).

Refs: endive (inductive invariant inference for TLA+ protocols, verified with TLAPS, TwoPhase benchmark); MIT 6.826 concurrency lecture (invariant proofs by induction over atomic actions); PharOS determinacy proof in TLA+/TLAPS (determinacy = local states independent of scheduling order — directly applicable); "Specifying and Verifying Systems With TLA+" (assertional reasoning style); TLA+ Hyperbook (determinism/stuttering).

#### 3.9 TLA+ model checking & state explosion management

TLC state space grows with constants (sessions N, streams M, chunk sequences K). Mitigations researched: (1) symmetry sets over model values (reduces state space ~n!, safety checks only; never used with liveness; getchas: expensive on large sets, hidden asymmetry, no CHOOSE over symmetric sets); (2) constraint bindings (`TLCGet("level") < N`, `FastInit`/`FastNext` subsets); (3) Apalache symbolic checking for deeper bounds; (4) separate TLC config per invariant (per-invariant checking avoids early-stop masking); (5) simulation mode for rapid falsification before exhaustive runs. Acceptance: each spec must TLC-check within a bounded time budget at the configured constants; symmetry sets declared in config + `SYMMETRY` directive.

Refs: Learn TLA+ optimizing model checking (symmetry sets); "An introduction to symmetry in TLA+" (gotchas, liveness incompatibility); TLA+ By Example symmetry sets (57254 → 1647 states); Compositional Model Checking of Consensus Protocols (IPA abstraction for state explosion); CCF raft TLA+ (nightly TLC, CLI `tlc.sh`).

#### 3.10 IDS Architecture Specification (natural-language + formal)

The markdown specification in `spec/` defines: the three layers (session, stream, wire) and their responsibilities; the carrier boundary (`ids-carrier::Endpoint` only; opaque `Vec<u8>`; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}); the allowed-crypto allowlist (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) and a rule that any spec mentioning crypto names only these primitives; the conformance method (refinement + generated suite); determinism requirement; type surface from `ids-000-aa` (never redefined: `NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Error`). The architecture spec is the human-readable companion to the TLA+/PlusCal modules; every claim in it must have a corresponding invariant, model, or proof obligation in `tla/`/`pluscal/`/`proofs/`.

---

### Threat Model

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|
| TLC state explosion hides real violations | Specification confidence | Symmetry sets (safety only), constraint bounds, per-invariant TLC configs, Apalache symbolic fallback; time budget per spec |
| Spec–implementation divergence | Conformance integrity | Conformance suite generated mechanically from the model (BFS test tree + refusal oracle); counterexample traces replayed against live implementations (HoldSpec/AgentConform pattern) |
| Nondeterminism masks bugs | Determinism guarantee | Explicit `either`/`with` nondeterminism in models; determinism proofs (TLAPS + inductive invariants); convergence under reorder/dup/loss |
| Infinite stutter breaks liveness | Liveness guarantees | Liveness checked separately from safety as temporal properties; `WF`/`SF` fairness on `fair process` actions; symmetry never enabled for liveness |
| Cross-layer inconsistency (session/stream/wire) | Specification coherence | Shared invariant module; refinement mappings between each concrete layer and the abstract IDS spec; common types from ids-000-aa |
| Conformance gaps (untested transitions) | Test completeness | Refusal checks at every reachable state; BFS covers all reachable states; prefix-subsumed tests dropped; unused transitions get short refusal tests |
| Carrier assumptions leak into specs | Carrier-agnostic integrity | Spec variables model only `Endpoint` datagrams; invariant: no IP/TCP/UDP/port/DNS symbols in any module; carrier treated as untrusted environment (AgentConform adversary pattern) |
| Crypto primitive misuse in specs | Allowed-crypto rule | Allowlist invariant: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 named; reviewed per module |
| Refinement mapping unsound | Conformance soundness | Mapping proven as TLAPS theorem (`Spec => HL!Spec`); substitution documented in `WITH` clause; machine-checked |

---

### Integration with Existing Crates

|| Crate | Role | Relevance to ids-doc ||
|-------|-------|---------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Error` — referenced by every spec, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All spec traffic modeled as datagrams to `Endpoint`; carrier boundary invariant enforced in specs |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Crypto references in specs limited to this set (type surface and primitive IDs only) |
| `ids-session` (Phase 050) | Client-session binding and state | Session TLA+/PlusCal spec is the formal contract this crate must conform to |
| `ids-stream` | Stream reassembly and delivery | Stream TLA+/PlusCal spec is the formal contract |
| `ids-wire` | Wire framing/dispatch over Endpoint | Wire TLA+/PlusCal spec is the formal contract |
| `ids-integrity` | End-to-end checksums, Merkle trees, scrubbing | Reused as reference for checksum/conformance vocabulary (SHA-256/BLAKE3 only) |

---

### Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-doc/` (a separate crate, not a folder inside an existing crate; one file per concern; spec files are TLA+/PlusCal/markdown — no implementation logic):

```
ids-doc/
  Cargo.toml                    # crate manifest: ids-000-aa, ids-carrier, ids-crypto (type surface only)
  README.md                     # crate purpose: formal specification layer; how to run TLC/TLAPS/Apalache
  src/
    lib.rs                      # crate root: re-exports spec metadata; no runtime logic
    spec/                       # IDS Architecture Specification (markdown)
      architecture_spec.md      # three layers, carrier boundary, allowed-crypto allowlist, conformance method
      types_surface.md          # ids-000-aa types referenced (never redefined)
      carrier_boundary_spec.md  # Endpoint-only traffic invariant
      crypto_allowlist_spec.md  # ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 only
      conformance_method.md     # refinement mapping + generated suite method
      determinism_requirement.md
    tla/                        # TLA+ specifications (.tla modules)
      root.tla                  # root module, EXTENDS layer modules, defines abstract IDS Spec
      session/                  # session TLA+ specs
        session_spec.tla        # Init/Next/Spec for session state machine
        session_state_machine.tla
        session_lifecycle.tla   # creation/expiration/revocation actions
        session_keys.tla        # session key management model
        session_replay.tla      # replay cache model (nonce + per-peer seq)
        session_cfg.tla         # TLC config: constants, invariants, properties
      stream/                   # stream TLA+ specs
        stream_spec.tla
        stream_reassembly.tla   # chunk reassembly, in-order delivery
        stream_dedup.tla        # duplicate suppression, exactly-once
        stream_overlap.tla      # overlapping chunk resolution (deterministic tie-break)
        stream_exactly_once.tla # exactly-once delivery property
        stream_cfg.tla          # TLC config
      wire/                     # wire TLA+ specs
        wire_spec.tla
        wire_framing.tla        # framing state machine (header/payload/complete)
        wire_messages.tla       # message types + schema validation before semantics
        wire_endpoint.tla       # ids-carrier::Endpoint datagram model
        wire_cfg.tla            # TLC config
      invariants/               # shared formal invariants across layers
        type_invariant.tla
        safety_invariants.tla   # no double delivery, no expired-active session, no replay accept
        liveness_properties.tla # <> / ~> progress properties
        carrier_agnostic_invariant.tla  # no network-stack symbols
        crypto_allowlist_invariant.tla
      refinement/               # refinement mappings (concrete layer => abstract IDS spec)
        abstract_ids_spec.tla
        session_refinement.tla  # THEOREM SessionSpec => Abstract!Spec
        stream_refinement.tla
        wire_refinement.tla
    pluscal/                    # PlusCal algorithm sources (.tla with embedded algorithms)
      session/
        session_handshake.tla   # --algorithm sessionHandshake {...}
        session_expiration.tla
        session_affinity.tla
      stream/
        stream_reassembly_loop.tla
        stream_dedup_loop.tla
      wire/
        wire_dispatch_loop.tla
      macros/                   # PlusCal macros (inline, no labels/loops)
        send_macro.tla          # send(to, msg) -> Endpoint datagram append
        recv_macro.tla          # rcv(p, var) -> block on Endpoint queue
        validate_macro.tla      # schema validation before semantic effect
      procedures/               # PlusCal procedures (call/return, local vars, requires Sequences)
        parse_frame.tla
        dispatch_msg.tla
    conformance/                # conformance suite specification + generator design
      conformance_spec.md
      suite_generator.md        # BFS over reachable state space (HoldSpec pattern)
      bfs_test_tree.md          # one test per root-to-leaf path; refusal per unused transition
      refusal_checks.md         # refusal oracle at every state
      conformance_runner.md     # executes suite black-box against ids-* implementations
      oracle.md                 # state comparator + refusal checker
      test_vectors.md           # deterministic spec test vectors
    proofs/                     # determinism + refinement proofs (TLAPS)
      determinism_proof.tla     # top-level determinism theorem
      session_determinism.tla
      stream_determinism.tla
      wire_determinism.tla
      inductive_invariants.tla  # candidate inductive invariants (endive-discovered, TLAPS-verified)
      tlaps_proofs/             # TLAPS proof scripts per theorem
    tools/                      # spec tooling documentation (not implementation)
      tla_analyzer.md           # syntactic analyzer usage (parse module)
      pluscal_translator_runner.md  # pcal.trans / Ctrl-T workflow, translation integrity
      tlc_runner.md             # TLC model config + run procedure, symmetry sets
      apalache_runner.md        # symbolic checking fallback
      symmetry_config.md        # symmetry sets + gotchas (safety only, no liveness)
  test_vectors/                 # deterministic TLA+/PlusCal test vectors (~20 files)
  tests/                        # spec-integration tests (~15 files)
    tla_parsing_tests.rs        # every .tla parses under the analyzer
    translation_roundtrip_tests.rs  # PlusCal re-translation produces identical output
    tlc_check_tests.rs          # each layer TLC-checks within time budget
    conformance_generation_tests.rs
  benches/                      # (~6 files)
    tlc_state_space_bench.rs
    translation_bench.rs
  docs/
    authoring_guide.md          # how to add a layer spec
    tlc_operations.md
    tlaps_operations.md
```

Total: ~90–1000 files across `src/spec/`, `src/tla/**/`, `src/pluscal/**/`, `src/conformance/`, `src/proofs/`, `src/tools/`, `test_vectors/`, `tests/`, `benches/`, `docs/` — each concern a separate file, consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. No stubs: every TLA+/PlusCal module has invariants/properties and a TLC config.

---

### Key Algorithms & Data Structures

- **TLA+ layer spec pattern (all three layers):**
```
---- MODULE session_spec ----
EXTENDS Integers, Sequences, TLC, FiniteSets
CONSTANTS Sessions, MaxTokens, ReplayTTL
VARIABLES state, tokens, last_activity, replay_cache, keys
TypeInvariant == /\ state \in [Sessions -> {"Created","Active","Expired","Revoked"}]
                /\ tokens \subseteq SessionId  \* etc.
Init == /\ state = [s \in Sessions |-> "Created"] /\ tokens = {} /\ ...
Next == \/ CreateSession \/ BindToken \/ Renew \/ Expire \/ Revoke \/ ReplayCheck
Spec == Init /\ [][Next]_vars /\ Liveness
```
Session Liveness example: `/\ \A s \in Sessions : <> (state[s] = "Expired" \/ state[s] = "Revoked")` (weak fairness on expiry actions).
- **PlusCal model pattern:** `--algorithm session { variables ...; process (s \in Sessions) { start: await ...; create: ...; goto start; } }` — translated to TLA+ with `pc` program counter; labels mark atomic steps; `fair process` adds `WF` constraints.
- **Refinement mapping:** `HL == INSTANCE abstract_ids_spec WITH sessions <- state, ...; THEOREM SessionSpec => HL!Spec`.
- **Conformance suite generation:** BFS over reachable states from `Init` via `Next`; emit one executable test per root-to-leaf path; for each state, refusal tests for every action `Next` forbids; drop prefix-subsumed tests; oracle compares post-state to spec-allowed transitions and runs refusal checks.
- **Determinism proof (TLAPS):** `Determinism == \A s1, s2, inp : ...` — given equal inputs, equal resulting states; proved via inductive invariants + confluence of independent actions; compositional across layers via refinement mapping preservation.
- **Replay cache model:** `replay_cache[peer] \in [Seq -> Bool]`; `Accept(peer, seq) == ~replay_cache[peer][seq]` then cache updated; bounded by TTL (stale entries evicted — modeled as nondeterministic eviction within TTL bound).

---

### Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `RecordId`, `FragmentId`, `SnapshotId`, `RecoveryProof`, `Checksum`, `AnomalyType`, `IdsTime`, `CarrierType`, `Error`) and sealed trait contracts; type surface only, consumed never redefined.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait; the only traffic boundary modeled in specs.
- `ids-crypto` — allowed primitives referenced in specs: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (type surface / primitive IDs only).
- External verification tools (not Rust dependencies): TLA+ Toolbox / VS Code TLA+ extension (+ JVM ≥ 11), TLC model checker, TLAPS proof system, Apalache symbolic model checker, `pcal.trans` PlusCal translator.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-doc` adds zero runtime dependencies beyond the type surface.

---

### Measurable Acceptance Criteria

|| Criterion | Target | Measurement ||
|-----------|----------|-------------|---------------|
| IDS Architecture Specification | Covers session, stream, wire layers; carrier-agnostic; allowed-crypto allowlist stated | Document review; every claim traced to a TLA+/PlusCal module or proof |
| TLA+ session spec | Parses under syntactic analyzer; TLC-checks within time budget at bounded constants | Parse module exit 0; TLC run output "no error" |
| TLA+ stream spec | Parses; TLC-checks reassembly/exactly-once/invariants | TLC run |
| TLA+ wire spec | Parses; Endpoint-only invariant holds in all reachable states | TLC invariant check |
| PlusCal translation | Every `--algorithm` block translates cleanly; re-translation is byte-identical (round-trip) | `pcal.trans` / Ctrl-T; diff of translation blocks |
| Formal invariants | Type invariant + all safety invariants hold in all reachable states (per layer) | TLC invariant results |
| Liveness properties | Each layer's `<>`/`~>` properties pass (checked separately from safety, no symmetry) | TLC temporal property results |
| Determinism proofs | Session/stream/wire determinism theorems proved in TLAPS; inductive invariants verified | TLAPS proof obligations discharged |
| Refinement mappings | `LayerSpec => Abstract!Spec` theorems proved for all three layers | TLAPS `Spec => HL!Spec` theorems |
| Conformance suite | Generated mechanically from model: BFS test tree + refusal oracle; 100% of seeded protocol defects in scope detected | Generated suite run against model and a mutant implementation |
| Black-box conformance | Suite executable against ids-* implementations; oracle checks after every action; counterexample traces replayable | Conformance runner output |
| Carrier agnosticism | Zero `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS symbols in any spec or model | Grep audit across `src/tla/`, `src/pluscal/` |
| Allowed crypto only | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 named in specs | Grep audit across spec/model files |
| Model checking budget | Each spec TLC-checks within configured constant bounds; state explosion managed via symmetry (safety) / Apalache | TLC runtime + state count benchmarks |

---

### Remaining Uncertainties

1. **Layer granularity:** Whether session/stream/wire suffice or a fourth "control-plane" TLA+ spec is needed (e.g., topology or membership) — decided when Phase 050 `ids-session` contract is written.
2. **PlusCal vs raw TLA+ split:** PlusCal is preferable for algorithm presentation (session handshake, stream loop) but cannot express everything raw TLA+ can (parameterized modules, advanced recursion limits); the boundary between PlusCal models and raw TLA+ specs is per-module, not yet fixed.
3. **TLC constants bounds:** Choosing bounded constants (Sessions, streams, chunk sequences) that falsify bugs without making model checking intractable — tuned per layer during implementation; symmetry-set gotchas (never with liveness) apply.
4. **TLAPS proof effort:** Determinism and refinement theorems require interactive proof work; the proportion provable automatically vs. requiring manual TLAPS steps is unknown until theorems are stated.
5. **Conformance runner implementation:** The suite generator produces executable tests, but the black-box runner hook into ids-* implementations (how an implementation is driven and observed) is defined here only at the design level; validated at integration with Phase 050+ crates.
6. **Apalache adoption:** Whether to adopt Apalache as a symbolic fallback for deeper bounds than TLC allows — toolchain maturity vs. benefit unmeasured.
7. **Tooling environment:** JVM requirement for TLA+ tools; authoring via VS Code extension vs. TLA+ Toolbox — team preference undecided; both documented in `src/tools/`.

---

### Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| TLA+ for layer specs (`Init /\ [][Next]_vars /\ Liveness`) | Standard, tool-supported (TLC/TLAPS/Apalache); stutter-invariant and composable; industry-proven for distributed protocols |
| PlusCal for algorithm models | Imperative pseudocode feel eases review of session/stream/wire algorithms; translates to checkable TLA+; labels = atomic steps match carrier-agnostic datagram steps |
| Three layers: session / stream / wire | Mirrors the IDS data path; each has an owning implementation crate (ids-session Phase 050, ids-stream, ids-wire) with a formal contract |
| Refinement mapping per layer (`Spec => HL!Spec`) | Proves concrete layer implements the abstract IDS spec; properties flow downward automatically (Hillel Wayne refinement argument) |
| Determinism via TLAPS + inductive invariants | Determinism is a hyperproperty over execution pairs; TLC bounds it only finitely; TLAPS gives the general proof; endive-style invariant discovery as candidate source |
| Conformance suite generated mechanically (BFS + refusal oracle) | HoldSpec evidence: generated suite detects 100% of seeded defects vs 56–69% for random/hand-written suites; counterexamples replayable against implementations |
| Symmetry sets for safety only | ~n! state-space reduction; incompatible with liveness; never applied where liveness is checked |
| Carrier boundary = `ids-carrier::Endpoint` only | Preserves carrier-agnostic architecture; specs treat carrier as untrusted environment; messages authenticated over payload survive re-framing |
| Allowed-crypto allowlist as spec invariant | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 may appear in any spec; grep-enforced |
| ids-000-aa types never redefined | Single source of truth; specs reference canonical types and sealed trait contracts |

---

### Sources

[^1]: Lamport, "Specifying Systems: The TLA+ Language and Tools for Hardware and Software Engineers" — Init/Next/Liveness spec form, stutter-invariance, refinement mappings (Ch. 10), TLC: https://lamport.org/tla/book.html
[^2]: Lamport & Matthews, "Specifying and Verifying Systems With TLA+" — `Init ∧ □Next ∧ Liveness`, actions as pre/post-state predicates: https://lamport.org/pubs/spec-and-verifying.pdf
[^3]: Lamport, "The PlusCal Algorithm Language" — algorithm language transpiling to TLA+, checked by TLC, macros/procedures/processes/fairness: https://microsoft.com/en-us/research/wp-content/uploads/2016/12/The-PlusCal-Algorithm-Language.pdf
[^4]: TLA+ PlusCal Tutorial (Sessions 2, 7, 9) — Toolbox workflow, Ctrl-T translation, TLC models, fairness, termination/liveness: https://lamport.org/tla/tutorial/
[^5]: Learn TLA+ — state machines (either/await), temporal properties ([] / <> / ~> / stutter), PlusCal, procedures & macros: https://learntla.com/
[^6]: TLA+ Wikipedia — modules, state/behavior/step, invariants, TLA+2, TLAPS: https://en.wikipedia.org/wiki/TLA+
[^7]: tlaplus/Examples — collection of TLA+/PlusCal specifications with TLC/Apalache/TLAPS verification info: https://github.com/tlaplus/Examples
[^8]: tiny-tlaplus-examples StateMachine.tla — fair algorithm, define block, Init/Next/Spec translation, WF_vars fairness: https://github.com/lostbearlabs/tiny-tlaplus-examples
[^9]: PlusCal Translator docs (TLA+ Toolbox / Inria) — BEGIN/END TRANSLATION checksums, CTRL+T workflow, translation errors: https://tla.msr-inria.inria.fr/tlatoolbox/doc/pluscal/pluscal.html
[^10]: pluscal.help — PlusCal cheat sheet: macros vs procedures, call/return, fairness modifiers, TLC errors: https://pluscal.help/
[^11]: Learn TLA+, "Procedures and Macros" — procedure stack representation, `Sequences` requirement, macro restrictions: https://old.learntla.com/concurrency/procedures-and-macros
[^12]: PlusCal p-manual — p-syntax/c-syntax, Euclid's algorithm, macro expansion rules: http://research.microsoft.com/users/lamport/tla/p-manual.pdf
[^13]: Distributed PlusCal (Extending PlusCal for Modeling Distributed Algorithms) — send/receive channels over networks, mapped here to ids-carrier::Endpoint datagrams: https://inria.hal.science/hal-04293883/document
[^14]: Learn TLA+, "Optimizing Model Checking" — symmetry sets (~n! reduction), constraint bindings: https://learntla.com/topics/optimization.html
[^15]: "An introduction to symmetry in TLA+" — symmetry gotchas, never with liveness, hidden asymmetry: https://jack-vanlightly.com/blog/2024/12/5/an-introduction-to-symmetry-in-tla
[^16]: TLA+ By Example, Symmetry Sets — 57254 → 1647 states via symmetry: https://learning.tlapl.us/blocking-queue/symmetry
[^17]: "Compositional Model Checking of Consensus Protocols Specified in TLA+" — state explosion, compositional checking, TLC exhaustive exploration: https://arxiv.org/html/2202.11385v1
[^18]: CCF documentation, "TLA+ Specifications" — Raft consensus specs, nightly TLC, CLI `tlc.sh`, VS Code plugin: https://microsoft.github.io/CCF/main/architecture/raft_tla.html
[^19]: vscode-tlaplus docs — Java ≥ 11 requirement, install, Getting Started: https://github.com/tlaplus/vscode-tlaplus/wiki
[^20]: endive — inductive invariant inference for TLA+ protocols, verified with TLAPS (TwoPhase benchmark): https://github.com/will62794/endive
[^21]: Merz et al., PharOS determinacy proof in TLA+/TLAPS — determinacy = local states independent of scheduling order: https://members.loria.fr/Stephan.Merz/papers/abz2016-pharos.pdf
[^22]: MIT 6.826 concurrency lecture — invariant proofs by induction over atomic actions, mutex example: https://6826.csail.mit.edu/2020/lec/l14.pdf
[^23]: LogPlayer — TLA+/PlusCal spec of exactly-once delivery under shard failures: https://arxiv.org/pdf/1911.11286v1
[^24]: "Overlapping data in network protocols: bridging OS and NIDS reassembly gap" — NIDS stream reassembly, overlap types (interval algebra), reassembly consistency: https://arxiv.org/html/2504.21618v1
[^25]: HoldSpec / "A Machine-Checked Model and Conformance Suite for the Payment Authorization Hold Lifecycle" — TLA+ state machine → generated conformance suite (BFS test tree, refusal checks), 100% seeded-defect detection: https://arxiv.org/html/2610.14419
[^26]: AgentRFC / AgentConform — TLA+ invariants as protocol conformance spec, two-phase checker, counterexample replay against live implementations, per-invariant TLC configs: https://arxiv.org/html/2603.23801v1
[^27]: Remix — multi-grained specifications, conformance checking detects model↔implementation deviations: https://arxiv.org/html/2409.14301
[^28]: TraceFix — PlusCal → TLA+ → TLC counterexample-driven repair loop, traces replayed against implementation: https://arxiv.org/html/2605.07935v1
[^29]: M2D2 — formal data model for IDS alert correlation (IDS formal-modeling precedent): https://link.springer.com/chapter/10.1007/3-540-36084-0_7
[^30]: "Specifying and Verifying Systems With TLA+" (lamport.azurewebsites mirror) — safety = invariants, liveness = fairness conditions: https://lamport.azurewebsites.net/tla/proving-safety.pdf

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, modules, invariants, proofs, conformance method, dependencies, acceptance criteria, and file structure for Phase 049.
