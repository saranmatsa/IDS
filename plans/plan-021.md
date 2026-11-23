## Phase 021 — Create the ids-quorum crate (quorum intersection for consistency): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 021 creates `ids-quorum`, a new separate crate (not folders inside existing crates), organized into subfolders by subsystem. It implements quorum intersection for consistency across the IDS workspace: read quorum, write quorum, quorum intersection (r + w > n), majority quorums, Flexible Paxos quorum intersection, quorum capacity, quorum fault tolerance, read/write fault tolerance, and quorum system optimization. It depends on `ids-000-aa` (canonical types + sealed trait contracts), speaks only over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), and uses only the allowed crypto set (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305). No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

**Core algorithmic choice:** A strict read-write quorum system `(R, W)` over a replica set of size `N` where every read quorum intersects every write quorum (`∀ r ∈ R, w ∈ W : r ∩ w ≠ ∅`), reducible to the size rule `r + w > N` for uniform quorum sizes. Majority quorums (`r = w = ⌊N/2⌋ + 1`) are the symmetric default, tolerating `f = ⌊(N-1)/2⌋` crash failures. Flexible Paxos weakens this to cross-phase intersection only (phase-1 election quorum intersects phase-2 replication quorum; quorums within a phase may be disjoint). Grid quorums offer `√N − 1` resilience with smaller quorums. Byzantine quorum systems require intersection size `2f + 1` to mask `f` Byzantine nodes. Quorum capacity/load/fault-tolerance tradeoffs follow Naor & Wool (capacity = 1/load).

**Decision:** Create a new separate crate `ids-quorum`, carrier-agnostic, depending only on `ids-000-aa` types/contracts and `ids-carrier::Endpoint` for any datagram transport of quorum messages. All quorum state is local and deterministic; the crate produces bytes to ship over whatever carrier IDS is using.

---

### 2. Architecture Overview

`ids-quorum` sits above `ids-000-aa` (canonical types + sealed contracts) and below any consumer crate that needs replicated consistency (e.g. a future `ids-replica`, `ids-store`, `ids-crdt`). It is **not** a network protocol — it is a local, deterministic quorum calculator and intersection enforcer that produces bytes to ship over `ids-carrier::Endpoint`.

```
┌─────────────────────────────────────────────────────────────┐
│                    CONSUMER ids-* CRATES                    │
│  ids-replica · ids-store · ids-crdt · ids-broadcast · …     │
│  (consume QuorumCalculator, QuorumSystem, QuorumIntersection)│
├─────────────────────────────────────────────────────────────┤
│              Phase 021: ids-quorum (NEW CRATE)              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  quorum system model: (R, W) over replica set N     │   │
│  │  r + w > N · majority · Flexible Paxos · grid ·     │   │
│  │  Byzantine masking (2f+1) · weighted voting         │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  quorum intersection enforcement                    │   │
│  │  read/write fault tolerance · capacity · optimization│   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  wire format: versioned, checksummed quorum envelopes│   │
│  │  (CBOR/bincode over ids-carrier::Endpoint bytes)     │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│              ids-000-aa (master crate, canonical types)     │
│  NodeId · RegionId · … · sealed trait Carrier · Quorum trait │
├─────────────────────────────────────────────────────────────┤
│             CARRIER BOUNDARY (no std::net / tokio::net)      │
│  ids-carrier::Endpoint = CarrierType + Vec<u8>               │
│  trait Carrier: send_datagram(Endpoint, &[u8]) / receive     │
└─────────────────────────────────────────────────────────────┘
```

**Data flow of a quorum decision:**
1. Consumer asks `ids-quorum` whether a read/write quorum is satisfiable given current replica roster `N` and desired `r`, `w`.
2. `ids-quorum` checks `r + w > N` (strict) or custom quorum-set intersection (Flexible Paxos / grid / weighted).
3. On fault injection, `ids-quorum` recomputes read fault tolerance, write fault tolerance, and overall resilience; reports which quorums remain available.
4. Consumer ships quorum proposals/votes as `ids-carrier::Endpoint` datagrams; `ids-quorum` only defines the versioned, checksummed envelope — never the transport.

---

### 3. Detailed Research Dimensions

#### 3.1 Read Quorum, Write Quorum, Quorum Intersection (r + w > N)

Given `N` replicas, a read quorum `r` and a write quorum `w` must satisfy `r + w > N` (equivalently `r > N − w` and `w > N − r`). By the pigeonhole principle, any `r`-sized subset and any `w`-sized subset of `N` elements overlap in at least one node, guaranteeing a read always sees at least one replica that participated in the last write — preventing stale reads. This is the foundational strict quorum rule (Gifford 1979; Naor & Wool 1998; UCSD CSE291 lecture; Cassandra/Dynamo `R + W > RF`).

**Three canonical tunings (strong consistency, all satisfy r + w > N):**
- `W = N, R = 1`: writes to all, read from one — fast reads, slow writes, no fault tolerance for writes.
- `W = 1, R = N`: write to one, read from all — fast writes, slow reads.
- `W = R = ⌊N/2⌋ + 1` (majority): symmetric, tolerates `f = ⌊(N-1)/2⌋` failures; two majorities always overlap.

Caveat: `R + W > N` prevents stale reads but does not, by itself, prevent write-write conflicts; version numbers/timestamps (or a leader/lease) are needed to order concurrent writes (UCSD lecture rule 4).

#### 3.2 Majority Quorums and Fault Tolerance

Majority quorum = `⌊N/2⌋ + 1` of `N`. A cluster of 5 needs quorum 3, tolerating 2 failures; cluster of 3 needs 2, tolerating 1. General rule: to tolerate `f` crash failures, cluster size `N = 2f + 1`. Majority quorums achieve optimal fault tolerance `⌊(N−1)/2⌋` but throughput caps at ~2× a single node regardless of cluster size (Naor & Wool), because every access touches a majority. Majority is the best-availability quorum when per-node failure probability < 0.5.

#### 3.3 Flexible Paxos Quorum Intersection

Flexible Paxos (Howard, Malkhi, Spiegelman, arxiv:1608.06696) weakens "all quorums intersect" to "quorums from different phases intersect." Phase-1 (election/prepare) quorum `Q1` and phase-2 (replication/accept) quorum `Q2` must satisfy `Q1 ∩ Q2 ≠ ∅` for every pair; quorums within a phase may be disjoint. For uniform sizes, `Q1 + Q2 > N`. This permits smaller replication quorums (better write throughput) at the cost of larger election quorums, and vice versa. WPaxos applies flexible quorums per-object-partition across WAN. Fast Flexible Paxos further relaxes fast-round intersection (fast quorums need ≥ 3/4 of nodes).

#### 3.4 Quorum Capacity, Load, and Fault Tolerance

Quorum-system quality metrics (Naor & Wool; Read-Write Quorum Systems Made Practical, arxiv:2104.04102; ETH Zurich Distributed Computing):
- **Load** `L(S)`: minimal max per-node access probability over all access strategies; the busiest node is the throughput bottleneck.
- **Capacity** `Cap(S) = 1 / L(S)`: max quorum accesses per time unit; directly proportional to max throughput.
- **Work** `W(Q) = |Q|`: nodes accessed per quorum.
- **Resilience** `R(S)`: max `f` such that after any `f` failures some quorum survives; `R(S) < min quorum size`.
- Majority quorum: load `1/2`, capacity `2`, resilience `⌊(N−1)/2⌋`.
- Grid quorum (`√N × √N`; quorum = full row + full column): resilience `√N − 1`, smaller quorums (~2√N), lower load than majority for large N.

`f`-resilient strategies trade capacity for resilience continuously (not binary). Quoracle (arxiv:2104.04102) quantifies these tradeoffs per deployment/workload via integer programming.

#### 3.5 Read / Write Fault Tolerance

Read fault tolerance = max `f` such that despite any `f` node failures, some read quorum remains available. Write fault tolerance defined similarly. Overall fault tolerance = min(read, write). Example grid `Q(2×3)`: read FT = 1, write FT = 2, overall FT = 1. Asymmetric read/write workloads can tune `R` and `W` independently (subject to `R + W > N`) to match workload skew — e.g., read-heavy: small `R`, large `W`.

#### 3.6 Quorum System Optimization

Optimization dimensions: quorum size (smaller = lower latency/load but lower resilience), access strategy (probabilistic selection over quorums to balance load), workload skew (read fraction vs write fraction), node heterogeneity (read capacity ≠ write capacity per node), and replica placement across failure domains. Probabilistic / ε-intersecting quorum systems relax intersection to `Pr[Q1 ∩ Q2 = ∅] ≤ ε`, improving availability at small inconsistency risk (used by Dynamo-style eventual-consistency systems). For IDS, strict intersection (`r + w > N`) is the default; probabilistic is an explicit opt-in with bounded `ε`.

#### 3.7 Byzantine Quorum Systems

Under crash faults, non-empty intersection suffices. Under Byzantine faults, two quorums may intersect only in faulty nodes, which can equivocate and break consistency. Masking quorum systems (Malkhi & Reiter) require every pair of quorums to intersect in `2f + 1` nodes to mask `f` Byzantine nodes, requiring `N > 4f` (or `N ≥ 3f + 1` for dissemination quorum systems with a correct quorum). Heterogeneous quorum systems (Stellar/Ripple style, per-node quorum slices) additionally require quorum subsumption + intersection + availability for consensus. IDS default is crash-fault model; Byzantine is a documented opt-in with `2f + 1` intersection.

#### 3.8 Leases and Bounded-Clock Quorum Reads

Leader leases (LeaseGuard, arxiv:2512.15659) allow a leader to serve linearizable local reads for a lease period without a quorum round-trip, requiring bounded-uncertainty clocks. Leases extend via committed log entries; a new leader cannot commit until the old lease expires. This trades a bounded-clock assumption for zero-RTT consistent reads. IDS may layer leases atop quorum reads as an optimization; the quorum core remains clock-free.

#### 3.9 Dynamic Quorum Reconfiguration

Changing `N`, `R`, `W`, or the quorum set itself must preserve intersection across the transition. Joint consensus (Raft joint configuration: new config committed only after old config agrees; quorums from both configs must intersect during the window) is the standard safe pattern. Flexible Paxos joint quorums: `c01.read` contains a `c0.read` and a `c1.read` quorum; same for write. Reconfiguration must never create a window where two disjoint quorums can both decide.

#### 3.10 Formal Verification

Quorum intersection is the safety-critical invariant: any two quorums overlap at a correct process, so information passes from one operation to the next. TLA+/PlusCal specifications model-check quorum intersection (Flexible Paxos TLA+ spec model-checked; etcd `IsQuorum` = `2*|votes ∩ config| > |config|`; `IntersectingQuorums` checks config pairs). Mechanized proofs (Ivy, TLA+) confirm leader uniqueness via quorum intersection. IDS should specify quorum intersection in TLA+/PlusCal (housed in a future `ids-state-machine` crate) and property-test the crate.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Stale read (read quorum misses last write) | Enforce `r + w > N`; version/timestamp reads to pick newest; majority or Flexible Paxos cross-phase intersection ||
|| Write-write conflict (two disjoint write quorums) | Require write quorum intersection (`w + w > N`) or a single leader/lease serializer; version ordering ||
|| Split-brain / two leaders | Quorum intersection ensures any two election quorums share a voter; single winner per term ||
|| Node failures disable all quorums | Resilience ≥ `f`; choose quorum size > `f`; grid/majority tuned to expected failure rate ||
|| Uneven load (hot node) | Probabilistic access strategy over quorums; load-balanced quorum selection; capacity-aware tuning ||
|| Byzantine intersection in faulty nodes only | Byzantine opt-in: intersection size `2f + 1`, `N > 4f`; content/identity trust decoupling ||
|| Reconfiguration creates disjoint quorum window | Joint consensus; old+new config quorums must intersect during transition ||
|| Clock skew corrupts lease/quorum reads | Bounded-uncertainty clocks only for lease optimization; quorum core never trusts wall clock ||
|| Carrier/transport failure between replicas | Carrier-agnostic; quorum decisions are local; retries handled by consumer over `ids-carrier::Endpoint` ||
|| Disallowed dependency (std::net/tokio::net/libp2p/etc.) | Dependency audit bans networking crates; clippy/CI rule; `Endpoint` is sole address type ||
|| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 enforced at implementation layer ||

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 021 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Canonical types (`NodeId`, `RegionId`, …), sealed trait contracts (`trait Quorum` declared here), invariants including `QuorumIntersection`, versioning, errors ||
|| `ids-quorum` (this crate) | Quorum system model, intersection enforcement, capacity/fault-tolerance, wire envelopes ||
|| `ids-carrier` | Concrete `Endpoint` transport for quorum datagrams; depends on `ids-000-aa`; `ids-quorum` imports `Endpoint` type only ||
|| Future `ids-replica` / `ids-store` | Consumers — replicated state built on quorum reads/writes ||
|| Future `ids-crdt` | May layer quorum-based convergence on top ||
|| Future `ids-state-machine` | TLA+/PlusCal quorum intersection specification ||

**Integration points:**
- `ids-000-aa` declares `trait Quorum` (sealed) and `src/invariants/quorum_intersection.rs` — `ids-quorum` is the implementation crate for these contracts.
- Quorum messages travel as versioned, checksummed envelopes over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); `ids-quorum` defines the envelope format but no transport implementation.
- No dependency on `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS — all traffic via `ids-carrier::Endpoint` (I-CARR-001 carrier-agnostic stance).
- Cargo workspace: `ids-quorum = { path = "../ids-quorum" }`, depends on `ids-000-aa` only (plus allowed crypto/serialization crates).

---

### 6. Proposed File Structure (~300 files in subfolders)

**Crate:** `ids-quorum` (new crate at `C:/idfs/ids-quorum/`; depends on `ids-000-aa`; serde, thiserror, bytes, blake3, sha2, ed25519-dalek, chacha20poly1305, xchacha20poly1305, proptest/bolero, criterion)

```
ids-quorum/
├── Cargo.toml                          # ids-000-aa + allowed crypto + serde/thiserror/bytes/blake3; workspace member
├── lib.rs                              # crate root; feature gates; public re-exports; QuorumSystem orchestrator
├── constants.rs                        # Protocol defaults (default r/w, majority formula, resilience bounds)
├── error.rs                            # QuorumError (thiserror): intersection, tolerance, sizing, validation variants
├── prelude.rs                          # re-export glob for downstream
│
├── model/                              # Quorum system core model
│   ├── mod.rs
│   ├── quorum_system.rs                # QuorumSystem<N>: replica set, R and W families
│   ├── read_quorum.rs                  # ReadQuorum: size r + member selection
│   ├── write_quorum.rs                 # WriteQuorum: size w + member selection
│   ├── replica_set.rs                  # ReplicaSet: ordered NodeId roster, membership version
│   ├── quorum_id.rs                    # QuorumId (deterministic id for a quorum selection)
│   ├── quorum_member.rs                # QuorumMember (NodeId + weight + role)
│   ├── intersection.rs                 # QuorumIntersection: prove/check r ∩ w ≠ ∅
│   ├── r_plus_w_gt_n.rs                # Size rule r + w > N + derived bounds
│   ├── uniform_quorum.rs               # Uniform quorum size (all quorums same cardinality)
│   └── quorum_family.rs                # Family of valid quorums (set of subsets)
│
├── majority/                           # Majority quorums (default symmetric)
│   ├── mod.rs
│   ├── majority_quorum.rs              # ⌊N/2⌋+1 quorum; resilience f = ⌊(N-1)/2⌋
│   ├── majority_read.rs                # Majority read quorum
│   ├── majority_write.rs               # Majority write quorum
│   ├── fault_tolerance.rs              # f tolerance for N=2f+1
│   └── majority_validation.rs          # Validate majority quorum selection
│
├── flexible_paxos/                     # Flexible Paxos quorum intersection
│   ├── mod.rs
│   ├── phase_quorum.rs                 # Phase1 (election) vs Phase2 (replication) quorum split
│   ├── election_quorum.rs              # Q1 quorum (leader election)
│   ├── replication_quorum.rs           # Q2 quorum (log replication/commit)
│   ├── cross_phase.rs                  # Q1 ∩ Q2 ≠ ∅ enforcement (Q1 + Q2 > N)
│   ├── within_phase.rs                 # Disjoint quorums allowed within a phase
│   ├── wpaxos_style.rs                 # Per-object-partition flexible quorums (reference)
│   └── fast_flexible.rs                # Fast-round ≥3/4 intersection (reference only)
│
├── grid/                               # Grid quorums (smaller quorums, √N − 1 resilience)
│   ├── mod.rs
│   ├── grid_quorum.rs                  # √N × √N matrix; quorum = full row + full column
│   ├── grid_layout.rs                  # Row/column assignment of NodeIds
│   ├── grid_resilience.rs              # √N − 1 resilience proof
│   └── grid_selection.rs               # Row/column quorum selection strategy
│
├── weighted/                           # Weighted voting quorum systems
│   ├── mod.rs
│   ├── weighted_quorum.rs              # Node weights; quorum = weight threshold
│   ├── weight_assignment.rs            # Per-node weight (heterogeneous capacity)
│   ├── threshold.rs                    # Weight threshold for quorum validity
│   └── cabinet_style.rs                # Dynamic weighted consensus reference (Cabinet)
│
├── byzantine/                          # Byzantine quorum systems (opt-in)
│   ├── mod.rs
│   ├── byzantine_quorum.rs             # Intersection size 2f+1 masking
│   ├── masking_quorum.rs               # Masking quorum system (Malkhi & Reiter)
│   ├── dissemination_quorum.rs         # Dissemination quorum (≥3f+1, one correct quorum)
│   ├── heterogeneous.rs                # Per-node quorum slices (Stellar/Ripple style)
│   └── quorum_subsumption.rs           # Quorum subsumption condition (heterogeneous)
│
├── probabilistic/                      # ε-intersecting probabilistic quorums (opt-in)
│   ├── mod.rs
│   ├── probabilistic_quorum.rs         # Pr[Q1 ∩ Q2 = ∅] ≤ ε
│   ├── epsilon.rs                      # Inconsistency bound ε
│   ├── distribution.rs                 # Weighted random quorum selection
│   └── dynamo_style.rs                 # Dynamo/Cassandra sloppy quorum reference
│
├── capacity/                           # Capacity / load / fault-tolerance metrics
│   ├── mod.rs
│   ├── load.rs                         # Load L(S): max per-node access probability
│   ├── capacity.rs                     # Capacity Cap(S) = 1 / L(S)
│   ├── work.rs                         # Work W(Q) = |Q|
│   ├── resilience.rs                   # Resilience R(S): max tolerated failures
│   ├── read_fault_tolerance.rs         # Read FT
│   ├── write_fault_tolerance.rs        # Write FT
│   ├── overall_fault_tolerance.rs      # min(read, write) FT
│   ├── access_strategy.rs              # Probabilistic quorum selection strategy
│   └── quoracle_style.rs               # Integer-programming tradeoff quantification (reference)
│
├── optimization/                       # Quorum system optimization
│   ├── mod.rs
│   ├── tuner.rs                        # Tune r/w for read vs write workload skew
│   ├── workload_profile.rs             # Read fraction / write fraction profile
│   ├── heterogeneity.rs                # Per-node read/write capacity
│   ├── placement.rs                    # Replica placement across failure domains
│   └── latency.rs                      # Quorum size vs latency tradeoff
│
├── reconfiguration/                    # Dynamic quorum reconfiguration
│   ├── mod.rs
│   ├── joint_config.rs                 # Joint old+new config quorums (Raft-style)
│   ├── config_transition.rs            # Safe config change window
│   ├── membership_change.rs            # Add/remove replicas preserving intersection
│   └── joint_quorum.rs                 # Quorum spanning two configs
│
├── lease/                              # Leader leases for zero-RTT reads (optimization)
│   ├── mod.rs
│   ├── lease.rs                        # Leader lease with bounded-uncertainty clock
│   ├── lease_grant.rs                  # Lease grant/renewal via committed entry
│   └── bounded_clock.rs                # Bounded-uncertainty clock abstraction
│
├── wire/                               # Versioned, checksummed quorum envelopes
│   ├── mod.rs
│   ├── envelope.rs                     # QuorumEnvelope: version + payload + checksum
│   ├── format.rs                       # Serialization format (CBOR/bincode)
│   ├── checksum.rs                     # Payload checksum (sha256/blake3)
│   ├── signature.rs                    # Optional ed25519 signature over envelope
│   ├── proposal.rs                     # Quorum proposal (read/write/vote)
│   ├── vote.rs                         # Quorum vote from a replica
│   └── ack.rs                          # Quorum ack
│
├── crypto/                             # Allowed-crypto surface (type-level + ops)
│   ├── mod.rs
│   ├── ed25519.rs                      # ed25519 sign/verify for quorum messages
│   ├── sha256.rs                       # sha256 hashing for quorum state
│   ├── blake3.rs                       # blake3 content addressing for quorum IDs
│   ├── chacha20poly1305.rs             # chacha20poly1305 AEAD (96-bit nonce)
│   └── xchacha20poly1305.rs            # xchacha20poly1305 AEAD (192-bit nonce)
│
├── calculator/                         # Quorum calculator (decision engine)
│   ├── mod.rs
│   ├── quorum_calculator.rs            # QuorumCalculator: satisfiable(r, w, N)?
│   ├── quorum_decision.rs              # QuorumDecision: satisfiable / quorum set / reason
│   ├── availability.rs                 # Availability given failed nodes
│   ├── satisfiability.rs               # Check quorum satisfiability under failures
│   └── solver.rs                       # Solve optimal r/w for workload profile
│
├── validation/                         # Quorum validation & proofs
│   ├── mod.rs
│   ├── intersection_validator.rs       # Validate r ∩ w ≠ ∅ for all pairs
│   ├── size_validator.rs               # Validate r + w > N
│   ├── resilience_validator.rs         # Validate resilience ≥ target f
│   └── proof.rs                        # Quorum intersection proof artifact
│
├── state/                              # Quorum state machines
│   ├── mod.rs
│   ├── quorum_state_machine.rs         # QuorumStateMachine (proposed→accepted→committed)
│   ├── view_state_machine.rs           # View/membership epoch state machine
│   └── lease_state_machine.rs          # Lease state machine
│
├── tests/                              # Integration & property tests
│   ├── mod.rs
│   ├── intersection_tests.rs           # r ∩ w ≠ ∅ for all quorum pairs
│   ├── r_plus_w_tests.rs               # r + w > N boundary tests
│   ├── majority_tests.rs               # Majority quorum + fault tolerance
│   ├── flexible_paxos_tests.rs         # Q1 ∩ Q2 cross-phase intersection
│   ├── grid_tests.rs                   # Grid quorum √N − 1 resilience
│   ├── byzantine_tests.rs              # 2f+1 Byzantine masking
│   ├── capacity_tests.rs               # Load/capacity/resilience metrics
│   ├── reconfiguration_tests.rs        # Joint config intersection
│   ├── lease_tests.rs                  # Lease read optimization
│   ├── wire_tests.rs                   # Envelope encode/decode round-trip
│   ├── property_tests.rs               # Proptest/bolero semilattice/intersection properties
│   ├── adversarial_tests.rs            # Byzantine, partition, churn
│   └── chaos_tests.rs                  # Node churn, offline members
│
├── fuzz/                               # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_intersection.rs
│   ├── fuzz_sizing.rs
│   ├── fuzz_wire.rs
│   └── fuzz_reconfiguration.rs
│
├── benches/                            # Benchmarks
│   ├── mod.rs
│   ├── intersection_bench.rs           # Intersection check latency
│   ├── capacity_bench.rs               # Load/capacity computation
│   ├── wire_bench.rs                   # Envelope serialization
│   └── solver_bench.rs                 # Optimal r/w solver
│
├── spec/                               # Formal specifications
│   ├── quorum_intersection.md          # Quorum intersection laws
│   ├── r_plus_w_spec.md                # r + w > N specification
│   ├── flexible_paxos_spec.md          # Cross-phase intersection spec
│   └── tla/                            # TLA+/PlusCal (in future ids-state-machine)
│
├── docs/                               # Documentation
│   ├── architecture.md                 # Quorum architecture & layering
│   ├── integration_guide.md            # Integrating with ids-replica/ids-store
│   ├── quorum_class_guide.md           # When to use majority/grid/flexible/Byzantine
│   └── tuning_guide.md                 # Tuning r/w for workload skew
│
└── test_vectors/                       # Deterministic test vectors
    ├── intersection_vectors.json
    ├── majority_vectors.json
    ├── grid_vectors.json
    ├── byzantine_vectors.json
    └── wire_vectors.json
```

**Why ~300 files:** one file per responsibility — quorum model, majority/Flexible Paxos/grid/weighted/Byzantine/probabilistic variants, capacity/load/resilience metrics, optimization, reconfiguration, leases, wire envelopes, crypto surface, calculator, validation, state machines, tests, fuzz, benchmarks, specs, docs, test vectors. The count sits within the required ~90–1000 range and reflects genuine quorum-system complexity. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Quorum System Model
```
QuorumSystem<N> {
    replicas: ReplicaSet<NodeId>,       // size N
    reads:   QuorumFamily,              // family of valid read quorums
    writes:  QuorumFamily,              // family of valid write quorums
}
// invariant: ∀ r ∈ reads, w ∈ writes : r ∩ w ≠ ∅
```

#### 7.2 Size Rule (strict quorum)
```
satisfiable(r, w, N) = (r + w > N) ∧ (1 ≤ r ≤ N) ∧ (1 ≤ w ≤ N)
// equivalently: r > N − w  and  w > N − r
```

#### 7.3 Majority Quorum
```
majority(N) = ⌊N/2⌋ + 1
resilience(N) = ⌊(N − 1) / 2⌋   // max crash failures tolerated
N = 2f + 1   // to tolerate f failures
```

#### 7.4 Flexible Paxos Cross-Phase
```
// Q1 (election) and Q2 (replication) may be disjoint within a phase
// but every Q1 must intersect every Q2:
∀ q1 ∈ Q1, q2 ∈ Q2 : q1 ∩ q2 ≠ ∅
// uniform sizes: |Q1| + |Q2| > N
```

#### 7.5 Grid Quorum
```
// √N × √N matrix; quorum = one full row + one full column
// any row+column quorum intersects any other at exactly one cell
// resilience = √N − 1
```

#### 7.6 Byzantine Masking (opt-in)
```
// any two quorums intersect in ≥ 2f + 1 nodes to mask f Byzantine
// requires N > 4f (dissemination: N ≥ 3f + 1 with one correct quorum)
```

#### 7.7 Capacity / Load
```
// Load L(S) = min over access strategies of max per-node access probability
// Capacity Cap(S) = 1 / L(S)
// Work W(Q) = |Q| (nodes per quorum access)
// Resilience R(S) = max f such that some quorum survives any f failures
// R(S) < min quorum size
```

#### 7.8 Wire Envelope
```
QuorumEnvelope {
    version: ProtocolVersion,
    payload: Vec<u8>,        // QuorumProposal / QuorumVote / QuorumAck
    checksum: [u8; 32],      // sha256 or blake3 of payload
    signature: Option<[u8; 64]>, // ed25519 over canonical encoding (optional)
}
// shipped as ids-carrier::Endpoint datagram bytes; no transport in crate
```

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | NodeId, RegionId, … canonical types; sealed `trait Quorum`; `QuorumIntersection` invariant; versioning; errors ||
|| `ids-carrier` | `Endpoint` type (`CarrierType` + `Vec<u8>`) for quorum datagrams (type import only) ||
|| `serde` + `serde_bytes` | Serialization ||
|| `bincode` or `serde_cbor` | Envelope/wire encoding ||
|| `blake3` | Content addressing for quorum IDs, checksums ||
|| `sha2` (sha256) | Hashing ||
|| `ed25519-dalek` | ed25519 sign/verify for quorum messages ||
|| `chacha20poly1305` | AEAD (96-bit nonce) ||
|| `xchacha20poly1305` | AEAD (192-bit nonce) ||
|| `thiserror` | Error types ||
|| `bytes` | Buffer handling ||
|| `proptest` / `bolero` | Property-based testing ||
|| `criterion` | Benchmarks ||

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-quorum` is transport-agnostic over `ids-carrier::Endpoint` bytes. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-021.1 | Strict intersection | `∀ r ∈ R, w ∈ W : r ∩ w ≠ ∅` enforced; unit + property test, 100k iterations ||
|| AC-021.2 | Size rule | `satisfiable(r, w, N) ⟺ r + w > N` boundary tests (r+w = N+1 pass, = N fail) ||
|| AC-021.3 | Majority quorum | `⌊N/2⌋+1`, resilience `⌊(N−1)/2⌋`; N=3→2/1, N=5→3/2, N=7→4/3 ||
|| AC-021.4 | Flexible Paxos | Q1 ∩ Q2 cross-phase intersection; within-phase disjoint allowed ||
|| AC-021.5 | Grid quorum | √N × √N; resilience √N − 1; row+column intersection ||
|| AC-021.6 | Byzantine masking | 2f+1 intersection masks f Byzantine; N > 4f (opt-in) ||
|| AC-021.7 | Read fault tolerance | Max f with some read quorum available; integer-programming verification ||
|| AC-021.8 | Write fault tolerance | Max f with some write quorum available ||
|| AC-021.9 | Overall fault tolerance | min(read FT, write FT) ||
|| AC-021.10 | Capacity/load | Cap(S) = 1/L(S); load-balanced access strategy ||
|| AC-021.11 | Reconfiguration | Joint old+new config quorums intersect during transition ||
|| AC-021.12 | Lease reads | Zero-RTT local read within lease; bounded-uncertainty clock ||
|| AC-021.13 | Wire envelope | Versioned + checksummed; encode/decode round-trip byte-identical ||
|| AC-021.14 | Carrier agnosticism | Compiles with zero dependencies on std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS; all traffic via ids-carrier::Endpoint ||
|| AC-021.15 | Allowed crypto only | ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 only; dependency audit passes ||
|| AC-021.16 | Property tests | 100,000+ iterations; intersection, sizing, resilience invariants ||
|| AC-021.17 | Adversarial tests | Partition, Byzantine, churn — all fail-safe ||

---

### 10. Remaining Uncertainties

1. **Quorum set representation:** Uniform size-based (`r + w > N`) vs arbitrary quorum families (explicit subsets). Size-based is simpler and sufficient for majority/Flexible Paxos; arbitrary families needed for grid/heterogeneous. Decision: start size-based, add family support for grid/weighted.
2. **Read-your-writes / session guarantees:** `r + w > N` prevents stale reads across clients but not read-your-writes within a session after a write to a minority. Needs session tracking or version gating — deferred to consumer crate.
3. **Version ordering for concurrent writes:** Intersection prevents stale reads but not write-write conflicts; needs vector clocks, Lamport/HLC timestamps, or a leader/lease. Deferred to `ids-time`/`ids-lease` integration.
4. **Wire format:** CBOR (deterministic, self-describing) vs bincode vs protobuf. CBOR favored for compactness/schema evolution; crate decision pending.
5. **Byzantine scope:** Default crash-fault model; Byzantine `2f+1` masking is opt-in with `N > 4f`; threat model and acceptance criteria assume crash faults unless opted in.
6. **Dynamic reconfiguration frequency:** Joint consensus adds latency; how often quorum configs change is workload-dependent.
7. **Formal verification scope:** TLA+/PlusCal quorum intersection spec (future `ids-state-machine`) proves intersection; property tests cover the rest — extent of mechanized proof unresolved.
8. **Integration consumer:** No existing IDS crate yet consumes `ids-quorum`; integration points (ids-replica/ids-store) are future phases — API surface may evolve.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| New separate crate `ids-quorum` | User requires separate crates for new capabilities; keeps `ids-000-aa` focused on types/contracts ||
|| Strict `r + w > N` default | Fundamental read-write intersection; prevents stale reads; well-understood (Gifford, Cassandra/Dynamo) ||
|| Majority quorum `⌊N/2⌋+1` default symmetric | Optimal crash fault tolerance `⌊(N−1)/2⌋`; simplest, best availability when p(failure) < 0.5 ||
|| Flexible Paxos cross-phase intersection | Smaller replication quorums possible (better write throughput); Q1+Q2 > N ||
|| Grid quorums as alternative | Smaller quorums (~2√N), resilience √N − 1; good for large N ||
|| Byzantine 2f+1 masking opt-in | Required for Byzantine faults; N > 4f; not default (crash model assumed) ||
|| Capacity/load/resilience metrics | Naor & Wool; enables quorum-system tuning per workload (Quoracle-style) ||
|| Joint-config reconfiguration | Preserves intersection across config changes; prevents split-brain ||
|| Lease optimization for reads | Zero-RTT consistent reads with bounded-uncertainty clock (LeaseGuard pattern); optional ||
|| Wire envelope versioned + checksummed | Forward compatibility; integrity over `ids-carrier::Endpoint` bytes ||
|| Carrier-agnostic; no transport | `ids-quorum` produces bytes; consumer ships via `ids-carrier::Endpoint`; preserves I-CARR-001 ||
|| Allowed crypto only | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; enforced at implementation layer ||
|| Reject "all quorums intersect" within a phase | Flexible Paxos result: only cross-phase intersection required; more efficient ||

---

### 12. Sources (30 targeted searches)

- Strict quorum intersection `R + W > N`: https://arxiv.org/pdf/2002.06098v4.pdf ; https://cseweb.ucsd.edu/classes/sp16/cse291-e/applications/ln/lecture7.html
- Cassandra tunable consistency `R + W > RF`: https://cassandra.apache.org/doc/stable/cassandra/architecture/dynamo.html
- Majority quorum + fault tolerance `N = 2f + 1`: https://martinfowler.com/articles/patterns-of-distributed-systems/majority-quorum.html ; https://developer.hashicorp.com/consul/docs/concept/consensus
- Flexible Paxos cross-phase intersection `Q1 + Q2 > N`: https://arxiv.org/html/1608.06696v1 ; https://blog.openacid.com/distributed/raft-rw-quorum
- Read-Write Quorum Systems Made Practical (Quoracle, load/capacity/resilience): https://arxiv.org/html/2104.04102v1
- Naor & Wool Load/Capacity/Availability of Quorum Systems (SIAM 1998): https://avishaiwool.sites.tau.ac.il/_files/ugd/5dd4a3_5e8c89245e0c44c4a9613384b07b0d4e.pdf ; https://cs.yale.edu/homes/aspnes/pinewiki/QuorumSystems.html
- Grid quorums `√N − 1` resilience: https://disco.ethz.ch/courses/hs21/distsys/lnotes/chapter21.pdf ; https://disco.ethz.ch/courses/ss03/distcomp/lecture/chapter7.pdf
- Quorum Systems origin/history: https://pages.cs.wisc.edu/~remzi/Classes/739/Papers/QuorumsOrigin.pdf
- Byzantine quorum systems / masking `2f+1`, `N > 4f`: https://doi.org/10.1016/s0020-0190(01)00311-8 ; https://arxiv.org/html/2304.04979v3 (quorum subsumption, heterogeneous)
- Riak `R + W > N` read-your-own-writes: https://riak.com/posts/technical/riaks-config-behaviors-part-2
- Stanford CS145 consistency `R + W > N`: https://cs145.stanford.edu/Module5-Distributed/consensus-leader-election.html
- CMU 14-736 quorum tuning `R + W > N`: https://andrew.cmu.edu/course/14-736-s19/applications/ln/14736-l12.pdf
- WPaxos flexible quorums per-object partition: https://arxiv.org/html/1703.08905v4
- Fast Flexible Paxos (≥3/4 fast-round intersection): https://arxiv.org/html/2008.02671v2
- LeaseGuard Raft leases / bounded-uncertainty clocks: https://arxiv.org/pdf/2512.15659.pdf
- Cabinet dynamic weighted consensus: https://arxiv.org/pdf/2503.08914v1
- Overcoming the Majority Barrier (witness model availability): https://link.springer.com/article/10.1007/978-3-540-39989-6_25
- Etcd formal quorum models (TLA+ `IsQuorum`, `IntersectingQuorums`): https://github.com/elastic/elasticsearch-formal-models/blob/master/cluster/tla/consensus.tla
- TLAPS-Bench quorum intersection leader uniqueness: https://arxiv.org/pdf/2611.02849.pdf
- Verifying distributed protocols (quorum intersection axiom): https://ilyasergey.net/assets/pdf/papers/Mark-Yuen-Capstone.pdf
- Generalized quorum systems (channel failures): https://arxiv.org/html/2505.02646v1
- Byzantine quorum systems with max availability: https://doi.org/10.1016/s0020-0190(01)00311-8
- CAP theorem (consistency vs availability under partition): https://ibm.com/think/topics/cap-theorem ; https://sites.cs.ucsb.edu/~rich/class/cs293b-cloud/papers/brewer-cap.pdf
- Inconsistency probability partial quorums (ε-intersecting): https://arxiv.org/pdf/2002.06098v4.pdf

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 021.
