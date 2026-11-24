## Phase 073 — Create the ids-authenticated-broadcast crate: Dolev-Strong authenticated broadcast. Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1090-1104)

```
73. Research only do not read plan.md **[Phase 073 — Create the ids-authenticated-broadcast crate: Dolev-Strong authenticated broadcast.]**

Crate: ids-authenticated-broadcast
Purpose: Dolev-Strong authenticated broadcast.
Contents: Message authentication, Byzantine broadcast with signatures, Dolev-Strong protocol, authenticated reliable broadcast, Byzantine fault tolerance with authentication.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 073 — Create the ids-authenticated-broadcast crate: Dolev-Strong authenticated broadcast.]**

Create plan-073.md in C:/idfs/plans/ for the ids-authenticated-broadcast crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 073 creates `ids-authenticated-broadcast`, the IDS crate implementing Dolev-Strong authenticated Byzantine broadcast. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by protocol concern. The crate delivers Byzantine broadcast with signatures: a single designated sender broadcasts a value to N permissioned parties; all honest parties terminate, agree on the same value, and if the sender is honest output exactly the sender's value — tolerating up to t < n Byzantine parties in a synchronous model with PKI.

Dolev-Strong works via **signature chains**: round 0 the sender signs its input and multicasts it; in each subsequent round r (1..=t+1) a party extends any valid (r-1)-signature chain it newly accepted by appending its own signature and multicasts the chain; at the end of round t+1 each party outputs the unique value it accepted via a valid (t+1)-signature chain, or a default value ⊥ if it accepted zero or multiple values. Resilience t < n is optimal for deterministic authenticated broadcast; t+1 rounds is the matching lower bound. Every party sends at most two chains per round, total communication O(n²t) signatures (O(n³) for t = Θ(n)). Messages can carry up to f+1 ed25519 signatures each.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent through `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). The crate never opens a socket or addresses a transport by name; it returns `Outgoing{Datagram{endpoint, payload}}` decisions for the carrier layer to deliver.

Allowed crypto only: ed25519 (signature chains), sha256/blake3 (chain/message hashing, value commitment), xchacha20poly1305/chacha20poly1305 (optional encryption of broadcast state at rest). Crypto operations live in `ids-crypto`; this crate consumes primitive IDs and type surface (`Signature`, `KeyId`, `Checksum`) from `ids-000-aa` only.

A skeleton `ids-authenticated-broadcast` crate does NOT yet exist at `C:/idfs/ids-authenticated-broadcast/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-authenticated-broadcast` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a protocol engine: given a broadcast session (sender, value, fault bound t, round schedule), it drives signature-chain construction, validation, and decision, emitting opaque datagrams to `ids-carrier::Endpoint`. It never sends bytes itself.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (broadcast users)              │
│  ids-consensus, ids-replication, ids-view-change — invoke broadcast session │
├─────────────────────────────────────────────────────────────────────────────┤
│             Phase 073: ids-authenticated-broadcast (THIS CRATE — engine)    │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │
│  │ session         │ │ chain           │ │ round           │ │ decision │ │
│  │ manager         │ │ validation      │ │ scheduler       │ │ engine   │ │
│  │ (t, n, sender)  │ │ (sig-chain verif│ │ (lock-step ∆)   │ │ (⊥ or v) │ │
│  │ per-instance    │ │ ication)        │ │                 │ │          │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └──────────┘ │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │
│  │ state           │ │ crypto-surface  │ │ carrier-adapter │ │ audit   │ │
│  │ machines        │ │ (ed25519 only   │ │ (Outgoing only) │ │ log      │ │
│  │ per round/value │ │  for chains)    │ │                 │ │ (sha256/ │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ │ blake3)  │ │
└─────────────────────────┬───────────────────┴────────────────┴─────────────┘
                          │ opaque Vec<u8> datagrams
                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  Outgoing{endpoint: Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B},          │
│          payload: &[u8]}   — crate emits decisions, never sends directly     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Protocol data flow (one broadcast session):** 1) Application calls `broadcast(session_id, sender, value, t)`. 2) Session manager initializes per-round state, fault bound t, round schedule (t+1 rounds + decision). 3) Round 0: sender signs value via ed25519, emits `Chain{v, [σ_sender]}` to all N parties as opaque datagrams to `ids-carrier::Endpoint`. 4) Rounds 1..=t: each party validates received (r-1)-signature chains (distinct signers, first signer = sender, all signatures valid under PKI, chain length = r-1, not already signed by self), extends with own signature, emits r-signature chain to all parties. 5) Round t+1: parties validate t-signature chains, extend to t+1, record accepted values in extracted set, do NOT forward. 6) Decision: if extracted set = {v} output v, else output ⊥. All cross-party traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

**Broadcast properties (research-verified):** Termination — all honest parties decide by round t+1 (deterministic, lock-step). Validity — honest sender signs exactly one value; signature unforgeability means no other valid chain exists; all honest output the sender's value. Agreement — all honest parties output the same value (unique extracted value or ⊥). Resilience — tolerates any t < n Byzantine corruptions (optimal for authenticated broadcast); t+1 rounds is the matching lower bound for deterministic protocols.

**Crypto used:** ed25519 for signature chains (each hop signs the prior chain); sha256/blake3 for value commitment and chain digest; xchacha20poly1305/chacha20poly1305 to encrypt per-session state at rest (bounded replay windows). No other crypto. `ids-crypto` exposes primitive IDs; `ids-authenticated-broadcast` references them via `ids-000-aa` only.

---

### 3. Detailed Research Dimensions

Each dimension maps to targeted searches executed (sources cited inline; full list in Section 13).

#### 3.1 Dolev-Strong protocol: original specification and properties

**Source:** Dolev & Strong, "Authenticated Algorithms for Byzantine Agreement," SIAM J. Comput. 12(4):656–666, 1983 (DOI 10.1137/0212045). Also IBM Research Report RJ3416 (1982). The paper shows authenticated broadcast requires at least t+1 rounds; presents algorithms polynomial in t and n; best algorithm uses t+1 phases and O(nt) messages. With PKI and unforgeable signatures, broadcast is possible for any t < n.

**Source:** Katz graduate crypto lecture notes (UMD), Lecture 26. Formalizes (v, i)-authentic messages: `(v, p1, σ1, …, pi, σi)` where p1 = sender, all pi distinct, receiver j ∈ {p1…pi}, each σi valid under PKpi of `(v, p1, σ1, …, pi−1, σi−1)`. Round 1: sender signs input v, sends to all. Rounds 2..n−1: on receiving a (v, i−1)-authentic message, sign it and forward with own signature appended. Each player sends at most 2 messages per round (one per value).

**Source:** Tim Roughgarden, Foundations of Blockchains Lecture 2. Sender numbered 1 with private input v*. Time step 0: sender sends signed v* to all non-senders. Time steps 1..f+1: a non-sender convinced of v by a message m not previously convinced of adds its signature and forwards. Final output: if convinced of exactly one value v output v, else output ⊥. Tolerates f ≤ n corrupt nodes; f+1 rounds necessary for deterministic BA.

#### 3.2 Signature chains: definition, validity, and security properties

**Source:** decentralizedthoughts.github.io "Dolev-Strong Authenticated Broadcast." A k-signature chain on v: 1-chain = (v, sign(v, 1)); for k>1, k-chain = (m, sign(m, i)) where m is a (k−1)-chain not containing a signature from i. Valid chain received at round k: first signer = leader; all signers distinct; all signatures valid; chain length = k. Two key properties: (a) a (t+1)-chain contains at least one honest signer; (b) in round i an honest party accepts only length-i chains, so Byzantine-only chains of length ≤ t are rejected at round t+1.

**Source:** homes.cs.washington.edu CSE 422 notes. Notation ⟨b⟩S = message b with valid signatures from set S. Round 0: sender sends ⟨b⟩{1}. Rounds 1..f+1: on receiving ⟨b̃⟩{1,j1,…,jr−1} with r distinct signatures including sender, if b̃ ∉ extri add b̃ to extracted set and send ⟨b̃⟩{1,j1,…,jr−1,i} to everyone. End of round f+1: if |extri| = 1 output the bit, else output 0. Theorem: achieves BB for f ≤ n corrupt nodes; f+1 rounds is the lower bound.

#### 3.3 Validity, agreement, termination — proofs and intuition

**Source:** Roughgarden FOB21 Lecture 2, Theorem 11.2 (Agreement). If honest party receives k-chain at end of round k, it forwards to all honest parties in round k+1 (for all rounds except t+1). A (t+1)-chain contains at least one honest party h, who already sent the value to all honest parties. Hence every value received by one honest party is received by all. If leader sends >2 values, all honest receive at least two and agree on default ⊥.

**Source:** CISPA "Byzantine Consensus in the Partially Authenticated Setting." Recap: Dolev-Strong tolerates t < n, t+1 rounds, deterministic. In round k ≤ t+1 each party extends and forwards any valid (k−1)-chain received in round k−1. Validity from unforgeability (honest sender signs one value only). Consistency from the fact that for rounds k < t+1, extending and forwarding an accepted chain means all other parties either do the same next round or already accepted it. A (t+1)-chain received in round t+1 includes at least one honest signer who forwarded it in round t.

**Source:** arxiv 2611.06479 (Optimal Subcubic BA). Dolev-Strong is the best deterministic authenticated BB for t < n; O(κn³) communication (messages carry up to f+1 signatures); converting to BA drops resilience to f < n/2 (optimal for authenticated BA).

#### 3.4 Round structure, synchrony assumptions, and timing

**Source:** decentralizedthoughts.github.io. Requires (1) synchronous model (lock-step); (2) authenticated setting / PKI; signature unforgeability. In Dolev-Strong a party must accept a length-k chain only in round k — offline clients cannot verify timing, so protocol fails when t ≥ n/2 in client-server model with passive/offline clients. t+1 rounds necessary and sufficient for deterministic authenticated broadcast.

**Source:** arxiv 2411.01689 (SMR with Dolev-Strong). Dolev-Strong tolerates up to 99% adversary parties ("99% resilience") for always-on validators; can be extended to support always-on communicating clients following the same rules as validators.

**Source:** Springer "Early Stopping for Any Number of Corruptions." Dolev-Strong DSC runs t+1 rounds against strongly adaptive adversary; t+1 is tight for worst case but loose when actual corruptions f < t. Early-stopping protocols achieve min{f+2, t+1} rounds (Abraham & Dolev STOC'15). Weak early stopping: O(f) rounds when sender honest, O(t) when corrupt.

#### 3.5 Complexity: message, communication, and signature cost

**Source:** decentralizedthoughts.github.io. Every party sends at most two chains, each length ≤ t+1 (O(t) signatures). Total communication O(n²t) signatures (O(n³) for t = Θ(n)). Messages carry up to f+1 ed25519 signatures (64 bytes each) plus value.

**Source:** arxiv 2007.13175 (Optimal Communication Complexity of Authenticated BA). Dolev-Reischuk quadratic lower bound for authenticated BA. Dolev-Strong uses quadratic messages but cubic communication for f ≥ n/3 — the open gap for decades. After multi-signatures: B(k) = Θ((k + ks)n² + n³) where ks is signature size.

**Source:** arxiv 2202.09123 (Make Every Word Count). Dolev-Reischuk lower bound: Ω(nt) messages and Ω(nt) signatures even in failure-free authenticated runs. Dolev-Strong is the baseline authenticated BB for t < n.

#### 3.6 Dolev-Strong vs Bracha broadcast vs unauthenticated protocols

**Source:** ETH Zurich Distributed Computing course (disco.ethz.ch chapter 9). Consistent broadcast (c-broadcast): validity, consistency, integrity — no termination with faulty sender. Reliable broadcast (r-broadcast / Byzantine Generals): adds totality. Bracha broadcast (echo/ready phases, n > 3t) — no PKI, asynchronous, f < n/3. Dolev-Strong: PKI required, synchronous, f < n (tolerates dishonest majority), t+1 rounds, signature chains.

Comparison table (Piazza lec10-blockchain):
| | Dolev-Strong | Phase-King | Bracha broadcast |
| PKI | YES | NO | NO |
| Synchronous | YES | YES | NO |
| Faulty parties | f ≤ n−2 | f ≤ (n−1)/3 | f ≤ (n−1)/3 |
| Rounds | f+1 | 3(f+1) | constant |
| Guaranteed termination | YES | YES | NO |

#### 3.7 Partially authenticated setting and registered senders

**Source:** CISPA partially authenticated paper. Dolev-Strong showed broadcast possible if |S| ≥ t+1 (registered parties with keys). Byzantine Agreement possible if |S| ≥ t+1 and 2t < n. Only t+1 registered parties need sign/disseminate. Extends to unregistered parties forwarding chains to registered parties in two-phase super-rounds. Relevant for IDS where not all nodes may hold ed25519 keys.

#### 3.8 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek docs.rs (RFC 8032 EdDSA over Curve25519; 64-byte signatures; 32-byte public keys; deterministic nonces). chacha20poly1305 docs.rs (RFC 8439 AEAD; 96-bit nonce for ChaCha20-Poly1305; 192-bit nonce for XChaCha20-Poly1305 safe for random generation). sha2 RustCrypto (SHA-256, FIPS 180-4). BLAKE3-team README (256-bit output, SIMD, keyed hash, KDF, XOF).

`ids-authenticated-broadcast` uses:
- ed25519: signature chains — each hop signs the prior chain; verification of every signature in a chain under the signer's PK from the PKI.
- sha256: value commitment (commit to broadcast value before revealing), chain digest computation.
- blake3: fast content hashing for chain dedup, session state hashing, checkpoint chaining (preferred for speed/parallelism).
- xchacha20poly1305: encrypt per-session state at rest (bounded replay windows, pending chains) — 24-byte nonce, random nonces safe.
- chacha20poly1305: encrypt session payloads — 12-byte nonce, counter-based bounded use.

No other crypto. `ids-crypto` is the sole crypto surface; this crate references primitive IDs via `ids-000-aa` only.

#### 3.9 PKI, key distribution, and trusted setup assumptions

**Source:** UMD lecture notes + decentralizedthoughts. Dolev-Strong assumes PKI: every party knows every other party's public key before the protocol starts; permissioned (participants known prior to run). Signature unforgeability is the core cryptographic assumption (Dolev-Yao model: signatures as ideal objects, perfect correctness and unforgeability). Resilience t < n means dishonest majority tolerated — possible only with authentication.

**Source:** TCC paper "An Unstoppable Ideal Functionality for Signatures" — modular analysis of Dolev-Strong broadcast; UC-realized by standard EUF-CMA signature schemes that are consistent and linear-time. Generalizes to threshold setting.

#### 3.9 (b) Early stopping and adaptive variants

**Source:** Springer early-stopping paper. Weak early stopping achieves O(f) rounds when sender honest. Dolev-Strong baseline t+1 rounds regardless of actual faults. `ids-authenticated-broadcast` plans the baseline (fixed t+1 rounds); early stopping is a documented future optimization (remaining uncertainty).

#### 3.10 Rust implementations and reference code

**Source:** GitHub pysel/dolev-strong — Rust implementation of Dolev-Strong consensus for Byzantine Broadcast. Assumptions: permissioned, PKI, synchronous, known bound f on Byzantine nodes. Reference for structure (session, round, chain types). GitHub abhavk/dolev-strong — Erlang implementation (Tim Roughgarden FOB lectures). GitHub timyiu478/dolev-strong-protocol — Python SMR on top of Dolev-Strong. GitHub samarthaggarwal/Fault-Tolerant-Broadcast — compares Dolev-Strong message counts: honest nodes send (n−f)(n−1), faulty send f(n−1), total n(n−1). No production Rust crate found; plan specifies fresh implementation aligned to IDS contracts.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Signature forgery (break EUF-CMA) | Chain integrity, validity | ed25519 only; unforgeability assumption; keys never leave node; `ids-crypto` enforces |
| Replay of stale chains across rounds/sessions | Agreement, termination | Session ID bound into signed message; round-number embedded in chain validation (accept length-k only in round k) |
| Duplicate chain forwarding (amplification) | Communication cost | Dedup: sign/forward each value at most once per round; per-session seen-chain set |
| Byzantine sender sends multiple values | Validity violation | Decision rule: extracted set size ≠ 1 → output ⊥; unforgeability bounds chains to sender-signed values |
| Last-round reveal (value learned in round t+1 only) | Agreement | (t+1)-chain contains honest signer who already forwarded to all — all honest see it |
| Byzantine-only chain of length ≤ t injected late | Agreement | Round i accepts only length-i chains; adversary chain rejected at round t+1 |
| Offline / passive clients | Termination, validity | Dolev-Strong requires continuously online parties; IDS deployment must ensure liveness (documented limitation) |
| Carrier delays, reordering, duplication | Round integrity | Synchronous lock-step assumption; round boundaries by `ids-time` ticks, not wall-clock; idempotent chain validation tolerates duplicates |
| Chain message size (up to t+1 signatures) | Memory/bandwidth | Cap t (max fault bound) per session; chain = value + vector of (signer_id, signature) pairs; bounded |
| Key compromise | Authenticity | Signing keys encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown; key rotation out of scope |
| Audit log tampering | Accountability | sha256/blake3 hash chain + ed25519-signed checkpoints per session |
| Prohibited crypto/crate sneaking in | Policy compliance | Dependency audit CI; allowlist test asserts only ed25519/sha256/blake3/xchacha20poly1305/chacha20poly1305 |
| Carrier-agnostic rule violation | Architecture | `Endpoint` sole address type; crate emits `Outgoing` decisions only; CI greps source for banned symbols |

The carrier is never trusted: signature chains are self-authenticating and survive any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-authenticated-broadcast ||
|-------|-------|------------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Signature`, `KeyId`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All broadcast traffic flows to/from `Endpoint`; crate returns `Outgoing` decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signature chain sign/verify; value commitment; state encryption at rest |
| `ids-time` | Monotonic IDS standard time source | Round boundaries keyed to `IdsTime` ticks (never wall-clock for ordering) |
| `ids-consistency-profile` (Phase 060) | Profile registry (quorum/durability policy) | Broadcast sessions may be tagged with a consistency profile; profile gate checked before broadcast admission |
| `ids-consensus` (later phase) | Consensus engine | Dolev-Strong broadcast is a building block for leader broadcast in consensus rounds |
| `ids-replication` (later phase) | Replica synchronization | Reliable broadcast of state-machine commands via authenticated broadcast |

**Integration points:** crate exposes `BroadcastSession`, `Chain`, `Outgoing{Datagram}`, `Decision{v | ⊥}`; caller frames `Outgoing` payloads into `ids-carrier::Endpoint` datagrams and feeds received datagrams back via `receive_datagram`. No dependency on `ids-discovery`/`ids-session` beyond what contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-authenticated-broadcast/` (a separate crate, not a folder inside an existing crate):

```
ids-authenticated-broadcast/
├── Cargo.toml                          # ed25519-dalek?, chacha20poly1305?, sha2, blake3 (via ids-crypto); ids-000-aa, ids-carrier, ids-crypto, ids-time
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # Protocol defaults: max fault bound t, max chain length t+1, max signatures per message
├── error.rs                            # BroadcastError (thiserror): chain, session, round, crypto, carrier errors
├── features.rs                         # Feature flags: std/no_std, early-stopping (future), audit
│
├── session/                            # Broadcast session lifecycle
│   ├── mod.rs
│   ├── session.rs                      # BroadcastSession: t, n, sender, round schedule, state
│   ├── config.rs                       # SessionConfig: fault bound, value domain, default ⊥
│   ├── manager.rs                      # Session manager: create/lookup/close sessions
│   ├── types.rs                        # SessionId, SessionState, BroadcastRequest
│   └── lifecycle.rs                    # Session start/progress/termination transitions
│
├── chain/                              # Signature-chain core
│   ├── mod.rs
│   ├── chain.rs                        # Chain{value, signers: [(NodeId, Signature)], length}
│   ├── builder.rs                      # ChainBuilder: extend chain with own signature
│   ├── validator.rs                    # ChainValidator: distinct signers, first=sender, valid sigs, length==round
│   ├── dedup.rs                        # Seen-chain set per session/round
│   ├── types.rs                        # ChainId, ChainRef, SignatureEntry
│   └── commitment.rs                   # Value commitment (sha256/blake3) pre-reveal
│
├── round/                              # Round-driven execution
│   ├── mod.rs
│   ├── scheduler.rs                    # RoundScheduler: lock-step ∆, round boundaries by IdsTime
│   ├── round_state.rs                  # RoundState: accepted chains, sent flags per round
│   ├── round_0.rs                      # Round 0: sender signs input, multicasts 1-chain
│   ├── rounds_1_t.rs                   # Rounds 1..t: validate (r−1)-chain, extend, forward r-chain
│   ├── round_t1.rs                     # Round t+1: validate t-chain, extend to t+1, record, no forward
│   └── types.rs                        # RoundNumber, RoundEvent, RoundOutcome
│
├── decision/                           # Decision engine (⊥ or v)
│   ├── mod.rs
│   ├── engine.rs                       # DecisionEngine: extracted set, uniqueness check, output
│   ├── extracted_set.rs                # ExtractedSet: distinct accepted values per session
│   └── types.rs                        # Decision{Value | Bottom}, DecisionReason
│
├── carrier_adapter/                    # Opaque datagram framing to ids-carrier::Endpoint
│   ├── mod.rs
│   ├── adapter.rs                      # CarrierAdapter: Outgoing{Datagram{endpoint, payload}} emission
│   ├── datagram.rs                     # Datagram: session_id, round, chain bytes, framing
│   ├── framing.rs                      # Frame/deframe chain payloads as Vec<u8> (serde/bincode)
│   └── types.rs                        # Outgoing, Incoming
│
├── crypto_surface/                     # Allowed-crypto surface (ids-crypto wrapper)
│   ├── mod.rs
│   ├── ed25519_signer.rs               # Sign chains via ids-crypto ed25519
│   ├── ed25519_verifier.rs             # Verify chain signatures under PKI keys
│   ├── hashing.rs                      # sha256/blake3 value and chain hashing
│   ├── encryption.rs                   # xchacha20poly1305/chacha20poly1305 state-at-rest encryption
│   ├── allowlist.rs                    # Compile-time assert: only the five algorithms
│   └── types.rs                        # KeyId, Signature re-exports from ids-000-aa
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── broadcast_contract.rs           # trait Broadcast (sealed): broadcast/receive/decide
│   ├── chain_contract.rs               # trait ChainOps
│   ├── round_contract.rs               # trait RoundScheduler
│   ├── decision_contract.rs            # trait DecisionEngine
│   ├── carrier_contract.rs             # trait CarrierAdapter (Outgoing only)
│   ├── crypto_contract.rs              # trait CryptoSurface (allowed primitives)
│   ├── session_contract.rs             # trait SessionManager
│   ├── audit_contract.rs               # trait AuditLogger
│   ├── invariant_contract.rs           # trait InvariantChecker
│   └── mod.rs
│
├── invariants/                         # Protocol invariants (~8 files)
│   ├── mod.rs
│   ├── chain_length_matches_round.rs   # accept length-k only in round k
│   ├── distinct_signers.rs             # no duplicate signer in a chain
│   ├── first_signer_is_sender.rs       # chain origin = designated sender
│   ├── unforgeability_bound.rs         # Byzantine-only chains rejected at round t+1
│   ├── decision_uniqueness.rs          # |extracted| == 1 ⇒ output v else ⊥
│   ├── carrier_agnostic.rs             # no std::net/tokio::net symbols
│   ├── crypto_allowlist.rs             # only allowed primitives
│   ├── audit_chain_sound.rs            # sha256/blake3 chain + ed25519 checkpoints
│   └── mod.rs
│
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── mod.rs
│   ├── audit_entry.rs
│   ├── audit_chain.rs
│   ├── checkpoint.rs
│   ├── signer.rs
│   ├── verifier.rs
│   └── mod.rs
│
├── pk/                                 # PKI keyring (public keys of all parties)
│   ├── mod.rs
│   ├── keyring.rs                      # NodeId → ed25519 public key mapping
│   ├── types.rs                        # PublicKey, PkiEntry
│   └── mod.rs
│
├── tests/                              # Property/verification tests (~30 files)
│   ├── mod.rs
│   ├── session_tests.rs
│   ├── chain_validation_tests.rs
│   ├── round_scheduler_tests.rs
│   ├── decision_tests.rs               # validity/agreement/termination property tests
│   ├── carrier_agnostic_tests.rs       # grep-assert: no banned symbols
│   ├── crypto_allowlist_tests.rs
│   ├── resilience_tests.rs             # t < n scenarios, f Byzantine nodes
│   ├── early_stopping_tests.rs         # (future, feature-gated)
│   ├── dependency_audit_test.rs        # cargo tree: no prohibited crates
│   └── property_tests.rs               # proptest: chain validity, dedup
│
├── benches/                            # (~6 files)
│   ├── chain_validation.rs
│   ├── decision_engine.rs
│   └── broadcast_session.rs
│
├── spec/                               # Normative specifications (~6 files)
│   ├── protocol_spec.md                # Dolev-Strong rounds, chains, decision rule
│   ├── security_spec.md                # validity/agreement/termination arguments
│   ├── carrier_agnostic_spec.md        # Endpoint-only boundary contract
│   ├── crypto_policy.md                # allowed-crypto allowlist
│   └── resilience_spec.md              # t < n, t+1 rounds lower bound
│
├── docs/                               # Documentation (~10 files)
│   ├── architecture.md
│   ├── runbook.md                      # running a broadcast session
│   ├── integrating.md                  # wiring to ids-carrier/ids-crypto
│   └── limitations.md                  # synchrony, online parties, dishonest-majority limits
│
├── test_vectors/                       # Deterministic inputs and expected outputs
│   ├── chain_vectors.json              # valid/invalid chains per round
│   ├── scenario_vectors.json           # Byzantine sender, multiple values, ⊥
│   └── decision_vectors.json           # extracted set → output mapping
│
└── .gitignore
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `src/pk/`, `tests/`, `benches/`, `spec/`, `docs/`, `test_vectors/`. Each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Signature chain (core)

```
Chain { value: Value, signers: Vec<(NodeId, Signature)>, length: u64 }
  1-chain: (v, sign(v, sender))                              — round 0, sender only
  k-chain (k>1): (m, sign(m, i)) where m is (k−1)-chain without i's signature
  Valid(k, round r): first signer == sender ∧ all signers distinct ∧ all sigs valid ∧ length == k ∧ k == r
  Extend(chain, self): chain' = (chain, sign(chain, self_id))
```

Invariant: accept length-k chain only in round k; Byzantine-only chains of length ≤ t rejected at round t+1.

#### 7.2 Round execution

```
Round 0: sender signs input v, emits 1-chain to all N parties via ids-carrier::Endpoint
Rounds 1..=t: on receiving valid (r−1)-chain not yet signed by self: extend with own signature, emit r-chain to all parties
Round t+1: validate t-chains, extend to t+1, add value to extracted set, do NOT forward
```

Each party sends at most two chains per round; total O(n²t) signatures.

#### 7.3 Decision engine

```
extracted_set = { v : received a valid (t+1)-chain on v }
if |extracted_set| == 1 → output the unique v
else → output ⊥ (default)
```

Agreement argument: if an honest party accepts v in round r ≤ t, all honest accept v by round r+1; if first acceptance is in round t+1, chain has t+1 signatures including an honest signer who already forwarded — all honest receive it.

#### 7.4 Resilience and bounds

- Tolerates any t < n Byzantine parties (dishonest majority OK with PKI).
- t+1 rounds — matches the deterministic lower bound (Dolev & Strong 1983).
- Communication O(n²t) signatures; O(n³) for t = Θ(n).
- Message size: value + up to t+1 ed25519 signatures (64 B each) + signer IDs.

#### 7.5 Value commitment

Before round 0, sender may commit `commit = sha256(value)` or `blake3(value)`; chain carries the committed digest; value revealed with chain. Prevents value manipulation across rounds.

#### 7.6 Audit chain

Each session decision (output v or ⊥) = sha256/blake3-chained audit entry + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Signature`, `KeyId`, `Error`) and sealed trait contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly |
| `ids-crypto` | Allowed primitives only: ed25519 (chains), sha256/blake3 (hashing), xchacha20poly1305/chacha20poly1305 (state-at-rest encryption) |
| `ids-time` | Monotonic time source for round boundaries |
| `serde` + `bincode` | Chain/datagram serialization (wire format over opaque `Vec<u8>`) |
| `thiserror` | Error types |
| `proptest` | Property-based tests |
| `tempfile` | Test fixtures |

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list — all via `ids-crypto`. Signature-chain signing uses ed25519 only (the allowed primitive for signatures).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-073.1 | Dolev-Strong session lifecycle: create/start/progress/terminate per session | Session state machine transitions validated |
| AC-073.2 | Round 0: sender signs input via ed25519, emits 1-chain to all N parties via `ids-carrier::Endpoint` | Outgoing datagrams carry Chain{v, [σ_sender]} |
| AC-073.3 | Rounds 1..=t: validate (r−1)-chain (distinct signers, first=sender, valid sigs, length==r−1), extend, emit r-chain | ChainValidator unit tests |
| AC-073.4 | Round t+1: validate t-chain, extend to t+1, record value, do NOT forward | RoundState sent_flags |
| AC-073.5 | Decision: |extracted_set| == 1 → output v; else → output ⊥ | DecisionEngine tests |
| AC-073.6 | Validity: honest sender → all honest output sender's value | Property test across t < n scenarios |
| AC-073.7 | Agreement: all honest parties output same value (v or ⊥) | Property test |
| AC-073.8 | Termination: all honest decide by round t+1 (deterministic, lock-step) | RoundScheduler boundary test |
| AC-073.9 | Resilience: tolerates any t < n Byzantine parties (dishonest majority) | Resilience tests with f Byzantine nodes |
| AC-073.10 | Lower bound: fixed t+1 rounds (no early stopping in baseline) | RoundScheduler enforces exactly t+1 rounds |
| AC-073.11 | Complexity: each party sends ≤ 2 chains/round; chains ≤ t+1 signatures | Instrumented bench |
| AC-073.12 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test + source grep |
| AC-073.13 | Only ed25519/sha256/blake3/xchacha20poly1305/chacha20poly1305 in crypto surface | Crypto allowlist test passes |
| AC-073.14 | PKI: NodeId → ed25519 public key keyring; all signatures verified under registered keys | Keyring tests |
| AC-073.15 | Audit: sha256/blake3 hash chain + ed25519-signed checkpoints per session | Audit verifier test |
| AC-073.16 | Value commitment: sha256/blake3 digest bound into chain | Commitment tests |
| AC-073.17 | Determinism: same session config + inputs → same decision | Determinism tests |

---

### 10. Remaining Uncertainties

1. **Early stopping:** baseline runs fixed t+1 rounds; weak early stopping (O(f) when sender honest) is a documented future optimization — not in this plan.
2. **Concrete fault bound t per deployment:** t is session-configured; maximum supported t (chain length cap, message size with t+1 signatures) to be validated at integration.
3. **Multi-valued broadcast:** baseline supports a value domain; large values (bytestrings) handled via commitment + out-of-band payload — exact framing deferred to integration with `ids-carrier`.
4. **Session cleanup / garbage collection:** completed sessions and their chain state retention policy undefined; to be decided at integration.
5. **Interaction with `ids-consistency-profile` (Phase 060):** broadcast sessions tagged with a profile; profile gate before admission — handshake defined here, validated at integration.
6. **No_std support:** crate uses `std` (Vec, HashMap); `no_std` embedded-carrier support evaluated but not adopted for Phase 073.
7. **PKI bootstrap:** how ed25519 public keys are distributed to all parties before broadcast — out of scope; keyring consumed from `ids-000-aa`/`ids-identity`.
8. **Chain dedup memory:** seen-chain set per session/round — eviction policy unbounded; to be bounded at implementation.
9. **Value domain size:** Value type (fixed-size bytes vs variable) — to be aligned with `ids-000-aa` canonical types at implementation.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-authenticated-broadcast` (not a folder) | Follows "separate crates instead of more folders" directive; clear protocol boundary |
| Baseline = classic Dolev-Strong (fixed t+1 rounds, no early stopping) | Matches the 1983 specification; t+1 is the deterministic lower bound; simplest correct baseline |
| Signature chains via ed25519 only | ed25519 is the allowed signature primitive; unforgeability underpins validity/agreement |
| Round boundaries keyed to `ids-time` ticks (never wall-clock) | Synchronous lock-step requires bounded, monotonic time; immune to clock manipulation |
| Dishonest majority tolerated (t < n) | Dolev-Strong resilience with PKI — strongest authenticated-broadcast model |
| Decision ⊥ when |extracted| ≠ 1 | Catches Byzantine sender sending multiple values; preserves agreement |
| Each party sends ≤ 2 chains per round, ≤ t+1 signatures per chain | Matches published protocol; bounds per-party and total communication |
| Crate returns `Outgoing{Datagram}` decisions; never sends directly | Preserves IDS carrier-agnostic architecture; no network stack in trust base |
| All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 B) | Carrier-agnostic absolute rule |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |
| Allowed crypto only; via `ids-crypto`, primitive IDs from `ids-000-aa` | Complies with allowed-crypto rule |

---

### 12. Sources

1. Dolev, D., Strong, H.R. "Authenticated Algorithms for Byzantine Agreement." SIAM J. Comput. 12(4):656–666, 1983. DOI 10.1137/0212045. (Original Dolev-Strong protocol; t+1 rounds lower bound; O(nt) messages.)
2. Katz, J. Graduate Cryptography, Lecture 26 (UMD). (v, i)-authentic message notation; round-by-round protocol; PKI setup. http://cs.umd.edu/~jkatz/gradcrypto2/NOTES/lecture26.pdf
3. Roughgarden, T. Foundations of Blockchains, Lecture 2 — Dolev-Strong Protocol. f+1 rounds necessary; extracted-set decision rule; dishonest-majority tolerance. https://timroughgarden.github.io/fob21/l/l2.pdf
4. decentralizedthoughts.github.io — "Dolev-Strong Authenticated Broadcast." Signature chains; validity/agreement/termination; O(n²t) communication; online-client limitation. https://decentralizedthoughts.github.io/2019-12-22-dolev-strong/
5. homes.cs.washington.edu CSE 422 notes (blockchain_3.pdf). ⟨b⟩S notation; round 0..f+1 execution; theorem for f ≤ n corrupt nodes.
6. ETH Zurich Distributed Computing, Chapter 9 — Broadcast primitives; Dolev-Strong vs Bracha comparison table (PKI, synchrony, resilience, rounds). https://disco.ethz.ch/courses/ss04/distcomp/lecture/chapter9.pdf
7. CISPA — Byzantine Consensus in the Partially Authenticated Setting. Dolev-Strong recap; |S| ≥ t+1 registered parties; two-phase super-rounds for unregistered parties.
8. arxiv 2007.13175 — Optimal Communication Complexity of Authenticated BA. Dolev-Strong quadratic messages / cubic communication; Dolev-Reischuk lower bound; multi-signature variant B(k).
9. arxiv 2611.06479 — Optimal Subcubic BA with Plain PKI. Dolev-Strong as best deterministic authenticated BB for t < n; 99% resilience.
10. Springer — Early Stopping for Any Number of Corruptions. Weak early stopping min{f+2, t+1}; Dolev-Strong DSC baseline t+1 rounds.
11. arxiv 2202.09123 — Make Every Word Count. Dolev-Reischuk Ω(nt) messages and Ω(nt) signatures lower bound (authenticated, failure-free).
12. arxiv 2411.01689 — Dolev-Strong extended to SMR with 99% adversary resilience; always-on clients.
13. arXiv 2002.11321 — Improved Extension Protocols for BB/BA. Dolev-Strong best deterministic authenticated BB for t < n; B(k) = Θ((k+ks)n² + n³).
14. TCC — An Unstoppable Ideal Functionality for Signatures; Modular Analysis of Dolev-Strong Broadcast. EUF-CMA realization; threshold generalization.
15. ed25519-dalek docs.rs — RFC 8032 EdDSA over Curve25519; 64-byte signatures. https://docs.rs/ed25519-dalek
16. chacha20poly1305 docs.rs — RFC 8439 AEAD; 96-bit / 192-bit nonces. https://docs.rs/chacha20poly1305
17. sha2 RustCrypto — SHA-256 (FIPS 180-4). https://docs.rs/sha2
18. BLAKE3-team BLAKE3 README — 256-bit output, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3
19. GitHub pysel/dolev-strong — Rust implementation reference (permissioned, PKI, synchronous, known f). https://github.com/pysel/dolev-strong
20. GitHub abhavk/dolev-strong — Erlang implementation reference (Tim Roughgarden FOB lectures).
21. GitHub samarthaggarwal/Fault-Tolerant-Broadcast — Dolev-Strong message counts: honest (n−f)(n−1), total n(n−1).
22. IDS prompts.txt entry 73 (verbatim). C:/idfs/prompts.txt lines 1090-1104.
23. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — types.rs, lib.rs, contracts/, invariants/, errors.rs, versioning.rs, Cargo.toml.
24. IDS plan format references: C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure (~90-1000 files), Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
25. HX-241 carrier-agnostic socket abstraction — Carrier/Socket/Endpoint traits with no networking-stack references.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 073 (ids-authenticated-broadcast).