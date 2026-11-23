## Phase 060 — Create the ids-consistency-profile crate: Read/write behavior per profile. Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

---

### Verbatim prompt (C:/idfs/prompts.txt lines 895-909)

```
60. Research only do not read plan.md **[Phase 060 — Create the ids-consistency-profile crate: Read/write behavior per profile.]**

Crate: ids-consistency-profile
Purpose: Read/write behavior per profile.
Contents: Local acknowledged, quorum durable, regionally durable, cross-region durable, eventually replicated, read-only degraded, causally consistent profiles, read/write behavior definitions, failure behavior.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 060 — Create the ids-consistency-profile crate: Read/write behavior per profile.]**

Create plan-060.md in C:/idfs/plans/ for the ids-consistency-profile crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 060 creates `ids-consistency-profile`, the IDS crate that defines read/write behavior per consistency profile. It owns profile registration, read quorum (R), write quorum (W), durability profile assignment, consistency-model enforcement per profile, failure behavior per profile, read-only degraded operation, and causal consistency tracking — all carrier-agnostic. `ids-consistency-profile` does not open sockets or drive any transport directly; it evaluates whether a read or write satisfies a profile's quorum/durability requirements, issues acknowledgment decisions, tracks causal ordering via dotted version vectors, and ships every protection-relevant message as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

Profiles defined:

| Profile | Read behavior | Write behavior | Guarantee |
|---------|---------------|----------------|-----------|
| Local acknowledged | Read from local replica only (R=1) | Write acknowledged on local replica only (W=1) | Weakest; lowest latency, highest staleness risk |
| Quorum durable | Read from quorum (R where R+W>N) | Write to quorum (W where R+W>N) | Strong: reads see latest write |
| Regionally durable | Read quorum within region | Write quorum within region; async replicate cross-region | Strong within region, eventual cross-region |
| Cross-region durable | Read quorum across regions | Write quorum across regions (EACH_QUORUM-style) | Strong across regions; highest latency |
| Eventually replicated | Read any single replica (R=1) | Write any single replica + hinted handoff (W=1) | Eventual; anti-entropy converges |
| Read-only degraded | Read from any available replica | Writes rejected (read-only) | Availability during partitions |
| Causally consistent | Read with causal context (DVV/dotted VV) | Write with causal dependency vector | Causal order preserved; no linearizability |

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-consistency-profile` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-consistency-profile` crate does NOT yet exist at `C:/idfs/ids-consistency-profile/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned Dynamo quorum R+W>N mechanics (Cassandra docs, Stanford CS145, UMD, MIT 6.033, arXiv 2002.06098); causal consistency model (Wikipedia causal consistency); sequential vs linearizable consistency (Attiya SPAA'91, MIT 6.824, Princeton COS418/518); CAP theorem and PACELC (Abadi PACELC paper, Wikipedia PACELC); CRDT state-based/op-based and LWW/MV-register conflict resolution (arXiv 1805.06358, arXiv 2504.05398, arxiv.org/html/1603.01529 delta-CRDTs, Shapiro et al.); dotted version vectors / efficient causality tracking (ACM DL 10.1145/2332432.2332497); Dynamo-style sloppy quorum, hinted handoff, read repair, anti-entropy (Cassandra docs, Dynamo paper); anti-entropy digests and gossip reconciliation (systemoverflow anti-entropy digests); MongoDB read concern / write concern and causal consistency (MongoDB docs v5.0); CockroachDB follower reads, bounded/exact staleness (CockroachDB docs); Spanner TrueTime external consistency (Google Cloud docs); ed25519 Rust crate (ed25519-dalek docs.rs); xchacha20poly1305/chacha20poly1305 Rust crate (chacha20poly1305 docs.rs); sha256/blake3 Rust crates (sha2, blake3 docs.rs and BLAKE3-team README).

---

### 2. Architecture Overview

`ids-consistency-profile` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a policy/evaluation layer: given a read or write request tagged with a profile and a replica set, it decides (a) whether the operation may proceed, (b) how many replicas must acknowledge (W) / respond (R), (c) whether durability requirements are met, (d) what causal metadata to attach, and (e) what failure behavior applies. It never sends bytes itself; it returns an `AckDecision` / `ReadDecision` that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (profile users)                 │
│  ids-store, ids-replication, ids-fragment — read/write with profile tag      │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 060: ids-consistency-profile (THIS CRATE — policy)      │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──────────┐ │ │\n│  │ profile         │ │ quorum          │ │ durability      │ │ failure  │ │ │\n│  │ registry        │ │ evaluation (R+W│ │ evaluation      │ │ behavior │ │ │\n│  │ (7 profiles)    │ │  >N, locality)  │ │ (local/region/  │ │ (reject/ │ │ │\n│  └─────────────────┘ └─────────────────┘ │  cross-region)  │ │ degrade) │ │ │\n│  ┌─────────────────┐ ┌─────────────────┐ └─────────────────┘ └──────────┘ │ │\n│  │ causal          │ │ read-only       │ │                 │                │ │ │\n│  │ tracking (DVV)  │ │ degraded mode   │ │                 │                │ │ │\n│  └─────────────────┘ └─────────────────┘ │                 │                │ │\n│  ┌─────────────────┐ ┌─────────────────┐ │                 │                │ │ │\n│  │ ack/read        │ │ audit/logging   │ │                 │                │ │ │\n│  │ decision types  │ │ (signed+chained)│ │                 │                │ │ │\n│  └─────────────────┘ └─────────────────┘ │                 │                │ │\n└─────────────────────────────────────────┬──────────────────┴──────────────┘\n                                          │ opaque Vec<u8> datagrams\n┌─────────────────────────────────────────▼──────────────────┐\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only  │\n│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>},  │\n│  payload: &[u8])                                           │\n│  receive_datagram → (Endpoint, Vec<u8>)                    │\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP /      │\n│  QUIC / HTTP / DNS / Wi-Fi / Bluetooth / cellular / VPN    │\n└────────────────────────────────────────────────────────────┘
```

**Data flow of a profile-tagged write:** client submits `Write{record_id, payload, profile, causal_ctx}` → `ids-consistency-profile` evaluates the profile: computes required W, durability scope (local/region/cross-region), checks replica availability, returns `WriteAck{required_w, acked_replicas, durability_met, causal_dot, ack_or_reject}` → caller sends the write as opaque datagram(s) to `ids-carrier::Endpoint` targets and collects acks → profile re-evaluates ack count against W → returns `WriteResult{committed, durable, causal_context}`.

**Data flow of a profile-tagged read:** client submits `Read{record_id, profile, causal_ctx}` → profile evaluates required R, read scope, staleness tolerance → returns `ReadDecision{required_r, read_replicas, max_staleness, causal_gate}` → caller fetches from replicas via `ids-carrier::Endpoint` → profile merges responses, applies causal gate (reject if causal dependencies unmet), returns `ReadResult{value, version, causal_context, staleness}`.

**Profile semantics (research-verified):**

- **Local acknowledged** (W=1, R=1): write acked on first local replica; read from local replica. No quorum overlap — stale reads possible. Lowest latency, lowest durability.
- **Quorum durable** (W + R > N): strict quorum. Write waits for W acks; read gathers R responses and returns newest by version. Quorum intersection guarantees reads see the latest write (Dynamo/Cassandra model).
- **Regionally durable**: write quorum within the region (W_local), async geo-replication to other regions. Read quorum within region. Strong within region; eventual cross-region.
- **Cross-region durable** (EACH_QUORUM-style): write quorum must be reached in every region; read quorum across regions. Strong cross-region; highest latency (WAN).
- **Eventually replicated** (W=1, R=1, W+R≤N): write to one replica + hinted handoff / anti-entropy gossip. Read any replica. Converges eventually; anti-entropy digests (Merkle-tree or version-vector based) heal divergence.
- **Read-only degraded**: writes rejected; reads served from any available replica. Used during partitions to preserve availability (CAP: choose A over C).
- **Causally consistent**: writes attach a dotted version vector (DVV) causal dependency; reads gate on causal context — a read returns only after its causal past is satisfied (read-your-writes, monotonic reads, writes-follow-reads). No linearizability; available under partition.

**Crypto used:** ed25519 for signing ack/read decisions and audit checkpoints; sha256/blake3 for profile IDs, version-vector hashing, audit-chain links; xchacha20poly1305/chacha20poly1305 to encrypt profile state at rest when confidentiality required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 13.

#### 3.1 Quorum replication: R + W > N (Dynamo/Cassandra model)

**Source:** Apache Cassandra Dynamo architecture docs (cassandra.apache.org); Stanford CS145 Consistency in Replicas; UMD Distributed Systems quorum notes; MIT 6.033 Quorum Consensus note; arXiv 2002.06098 Consistency Analysis of Replication-Based Probabilistic Key-Value Stores; thecodeforge.io Quorum R+W>N; medium.com/double-pointer Replication R+W>N.

Given N replicas, a write succeeds when W replicas ack; a read succeeds when R replicas respond. If R + W > N, every read quorum intersects every write quorum in at least one node (minimum overlap = W + R − N), so a read always contacts at least one replica that holds the latest write — strong consistency. If R + W ≤ N, quorums may be disjoint → stale reads possible → eventual consistency. Tuning extremes: W = N, R = 1 (fast reads, writes blocked by any single failure); W = 1, R = N (fast writes, reads need all nodes). Dynamo recommends N=3, W=2, R=2. `ids-consistency-profile` implements the `QuorumDurable` profile with strict quorum (R + W > N) and the `EventuallyReplicated` profile with partial quorum (R + W ≤ N) plus anti-entropy.

#### 3.2 Local acknowledged profile (W=1, R=1)

**Source:** Dynamo-style quorum analysis; thecodeforge.io write quorum spectrum (W=1 = single acknowledgment, fastest writes, data on one node only, suitable for logs/metrics/regenerable data).

`LocalAcknowledged`: write acked by the first local replica only; read served from the local replica only. No cross-replica coordination. Lowest latency, no quorum overlap guarantee — stale reads and lost writes possible on replica failure. Intended for ephemeral/cache-tier data where latency dominates durability.

#### 3.3 Regionally durable profile

**Source:** AWS DynamoDB Global Tables (multi-Region strong vs eventual modes); Cassandra LOCAL_QUORUM / EACH_QUORUM consistency levels; PACELC theorem (Abadi).

Write quorum reached within a single region (W_local replicas in-region); async replication to other regions. Read quorum within region. Strong consistency within region; eventual consistency cross-region (replication lag bounded by anti-entropy interval). Fits PACELC: elsewhere (no partition) choose latency (local reads/writes) with eventual cross-region; during partition choose availability within region.

#### 3.4 Cross-region durable profile (EACH_QUORUM-style)

**Source:** Cassandra EACH_QUORUM consistency level (majority of replicas in each datacenter); AWS DynamoDB Global Tables multi-Region strong consistency (RPO zero); Spanner TrueTime external consistency.

Write must reach a quorum in every region before ack; read gathers from quorums across regions and returns newest. Strong consistency across regions; highest latency (WAN round trips); tolerates single-region failure without losing consistency. Most expensive profile; used for control-plane / configuration data.

#### 3.5 Eventually replicated profile

**Source:** Dynamo sloppy quorum + hinted handoff (cs.cornell.edu Dynamo paper, cs.cornell.edu/cs5414); Cassandra ANY consistency level + hinted replay; anti-entropy digests and gossip reconciliation (systemoverflow anti-entropy digests); CRDT delta-state (arXiv 1603.01529); eventually consistent register (arXiv 1511.05010).

Write to one replica (W=1); coordinator stores a hint and replays later; anti-entropy gossip periodically reconciles divergent replicas via digest exchange (version vectors or Merkle hashes) and transfers only missing/conflicting entries. Read any replica (R=1); may return stale until convergence. CRDT merge (commutative, associative, idempotent) ensures all replicas converge to the same state given the same update set. `ids-consistency-profile` defines the merge/audit interface; `ids-replication` performs the actual gossip transport via `ids-carrier::Endpoint`.

#### 3.6 Read-only degraded profile

**Source:** CAP theorem (blogs.bmc.com CAP theorem: during partition choose C or A); CockroachDB follower reads / stale reads (docs.cockroachlabs.com); PACELC (during partition P choose A or C).

During a partition or when write quorum cannot be reached, the crate switches affected records to `ReadOnlyDegraded`: writes are rejected with `WriteRejected` error; reads served from any available replica (possibly stale). Preserves availability (CAP A) at the cost of consistency. Rejoining the partition triggers anti-entropy reconciliation to converge.

#### 3.7 Causally consistent profile

**Source:** Wikipedia Causal Consistency (available under partition; asynchronous; stronger than eventual, weaker than sequential/linearizable); MongoDB Causal Consistency and Read/Write Concerns (majority read + majority write = all four guarantees: read-own-writes, monotonic reads, monotonic writes, writes-follow-reads); dotted version vectors / efficient causality tracking (ACM DL 10.1145/2332432.2332497); version vectors / vector clocks.

Writes attach a causal dependency vector (dotted version vector): `(node_id, counter)` dot plus version-vector causal past. A read carries its observed causal context; the profile gates read delivery until the causal past is satisfied (replica has applied all dependencies). Guarantees read-your-writes, monotonic reads, monotonic writes, writes-follow-reads without linearizability; available under partition. DVV enables compact causality tracking (compare dots instead of full VVs).

#### 3.8 Sequential vs linearizable consistency

**Source:** Attiya SPAA'91 (sequential vs linearizable); MIT 6.824 linearizability notes; Princeton COS418/518 strong consistency lectures; geeksforgeeks strong vs eventual consistency.

Linearizability = strong consistency: once a write completes, all later reads (by real-time order) return that write or a later one; operations appear instantaneous at a single point between invocation and completion. Sequential consistency: all nodes agree on a single order of operations, but real-time order need not be preserved. `ids-consistency-profile` does NOT implement linearizability (requires consensus/leader + quorum read, e.g. Raft read_index); its strongest profile is `QuorumDurable` (strong read-your-writes via quorum intersection) plus `CrossRegionDurable` (strong across regions). Causal consistency is weaker than both.

#### 3.9 CAP theorem and PACELC

**Source:** CAP theorem (blogs.bmc.com); PACELC design principle (Wikipedia PACELC; Abadi PACELC paper cs.umd.edu).

CAP: under network partition P, choose between Consistency C and Availability A — not both. PACELC extends: elsewhere (E, no partition) choose between Latency L and Consistency C. `ids-consistency-profile` encodes this explicitly: `QuorumDurable`/`CrossRegionDurable` choose C (sacrifice A under partition); `EventuallyReplicated`/`ReadOnlyDegraded` choose A (sacrifice C); `RegionallyDurable` chooses C within region, A cross-region; `LocalAcknowledged` chooses A (lowest latency, weakest C). PACELC "elsewhere" latency vs consistency maps to profile selection: low-latency profiles (`LocalAcknowledged`, `EventuallyReplicated`) vs high-latency strong profiles (`CrossRegionDurable`).

#### 3.10 Failure behavior per profile

**Source:** Dynamo/Riak sloppy quorum and hinted handoff; Cassandra consistency levels (ONE/TWO/THREE/QUORUM/ALL/LOCAL_QUORUM/EACH_QUORUM/ANY/LOCAL_ONE); quorum availability analysis (W + R > N failure tolerance: writes tolerate N−W failures, reads tolerate N−R failures); deepengineering.net staleness SLOs for Dynamo-style stores.

Each profile defines failure behavior:
- `LocalAcknowledged`: write fails if local replica down; read fails if local replica down. No redundancy.
- `QuorumDurable`: write succeeds iff ≥ W replicas available; read succeeds iff ≥ R replicas available. Tolerates N−W write failures / N−R read failures. Below quorum → operation rejected (choose C over A).
- `RegionallyDurable`: write succeeds iff ≥ W_local replicas available in region; cross-region lag tolerated. Region failure → fall back to `EventuallyReplicated` or `ReadOnlyDegraded` for that region's data.
- `CrossRegionDurable`: write succeeds iff quorum reached in every region. Any single region down → writes blocked (strong C).
- `EventuallyReplicated`: write succeeds on any single replica; hint stored for later. Always available for writes (W=1). Reads always available (R=1) but possibly stale.
- `ReadOnlyDegraded`: writes always rejected; reads always available from any replica.
- `CausallyConsistent`: write succeeds if causal deps can be recorded; read blocks (bounded wait) until causal past satisfied or times out → `CausalTimeout` failure.

Failure behavior is recorded in the append-only audit log (sha256/blake3 chain + ed25519-signed checkpoints).

#### 3.11 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** ed25519-dalek docs.rs (ed25519 key gen/sign/verify, RFC 8032 EdDSA over Curve25519); chacha20poly1305 docs.rs (ChaCha20Poly1305 RFC 8439 AEAD + XChaCha20Poly1305 extended 192-bit nonce); sha2 RustCrypto (SHA-256); blake3 BLAKE3-team README (256-bit output, SIMD, keyed hash, KDF, XOF).

`ids-consistency-profile` uses:
- ed25519: signing ack/read decisions and audit checkpoints (proof of profile evaluation).
- sha256: profile ID derivation, version-vector entry hashing, audit-chain links.
- blake3: fast content hashing for anti-entropy digests, causal-context hashing, checkpoint chaining (preferred for speed/parallelism).
- xchacha20poly1305: encrypt profile state at rest (bounded replay windows, causal histories) with 24-byte nonce safe for random generation.
- chacha20poly1305: encrypt session/state payloads with 12-byte nonce (counter-based, bounded use).

No other crypto. `ids-crypto` exposes primitive IDs; `ids-consistency-profile` references them via `ids-000-aa` only.

---

### Threat Model

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Stale read below quorum (R + W ≤ N) | Read correctness | `QuorumDurable`/`CrossRegionDurable` enforce R + W > N; `EventuallyReplicated` documents staleness bound via anti-entropy SLO |
| Lost write (W=1 replica fails) | Write durability | `LocalAcknowledged` documents risk; `QuorumDurable` requires W ≥ 2 for N ≥ 3; `CrossRegionDurable` requires quorum per region |
| Split-brain / divergent replicas | State convergence | Anti-entropy digest reconciliation (version vectors/Merkle) in `ids-replication`; CRDT merge is commutative/associative/idempotent |
| Causal dependency violation | Causal consistency | DVV gating: read blocks until causal past satisfied; `CausalTimeout` on violation |
| Partition-induced inconsistency | CAP tradeoff | `ReadOnlyDegraded` chooses availability; `QuorumDurable` chooses consistency (rejects writes below quorum) |
| Replay of ack/read decisions | Decision integrity | ed25519-signed audit checkpoints; nonce-bound decisions; bounded replay cache per profile |
| Clock manipulation for causal order | Causality correctness | Logical clocks (DVV dots/counters), never wall-clock; monotonic `IdsTime` only for timeouts |
| Carrier duplicating/delaying datagrams | Replay/ordering | Carrier-agnostic: replay protection at profile layer; idempotent merge tolerates duplicates |
| Profile downgrade attack | Consistency integrity | Profile is immutable per record/region; downgrade requires signed governance decision, audited |
| Key extraction (profile signing key) | Decision authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |

The carrier is never trusted: profile decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### Integration with Existing Crates

|| Crate | Role | Relevance to ids-consistency-profile ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All profile traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing decisions/audit; encrypting profile state at rest; hashing version vectors and digests |
| `ids-store` | Primary record store (profile user) | Tags reads/writes with a profile; consults `ids-consistency-profile` before acknowledging |
| `ids-replication` | Replica synchronization / anti-entropy | Performs gossip and hinted replay for `EventuallyReplicated`; uses profile's merge/audit interface |
| `ids-fragment` | Record fragments/shards | Fragment reads/writes governed by the parent record's profile |
| `ids-immutable` (Phase 040) | Immutable record protection | Profile must not permit writes to held/immutable records; guard checked before write admission |
| `ids-session` (Phase 050) | Client-session binding | Session-bound reads/writes carry profile tag; profile audit cross-referenced with session audit |

---

### Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-consistency-profile/` (a separate crate, not a folder inside an existing crate):

```
ids-consistency-profile/
  Cargo.toml
  src/
    lib.rs                      # crate root, public API surface
    errors.rs                   # ProfileError, QuorumError, DurabilityError, CausalError
    types.rs                    # ProfileId, Profile, QuorumConfig, DurabilityScope, AckDecision, ReadDecision (re-exports ids-000-aa types)
    profile_registry.rs         # profile registration, lookup, per-record profile assignment
    quorum_eval.rs              # R/W evaluation, R + W > N check, quorum overlap, ack counting
    durability_eval.rs          # local/region/cross-region durability checks
    failure_behavior.rs         # per-profile failure behavior (reject/degrade/wait)
    causal.rs                   # dotted version vector, causal context, causal gating
    read_only_degraded.rs       # read-only degraded mode: reject writes, serve reads
    anti_entropy.rs             # anti-entropy digest interface (version-vector / Merkle digest exchange)
    ack_manager.rs              # ack collection, quorum satisfaction, write result
    read_manager.rs             # read response merging, newest-by-version, staleness computation
    carrier_adapter.rs          # opaque Vec<u8> datagram framing to ids-carrier::Endpoint (decision carrier only)
    contracts/                  # sealed trait contracts (~10 files)
      profile_contract.rs, quorum_contract.rs, durability_contract.rs, causal_contract.rs,
      failure_contract.rs, ack_contract.rs, read_contract.rs, anti_entropy_contract.rs,
      carrier_contract.rs, mod.rs
    invariants/                 # compile-time/runtime invariants (~8 files)
      quorum_overlap.rs, monotonic_clock_only.rs, causal_gate_sound.rs,
      carrier_agnostic.rs, crypto_allowlist.rs, profile_immutable.rs,
      audit_chain_sound.rs, mod.rs
    audit/                      # signed, hash-chained audit log (~6 files)
      audit_entry.rs, audit_chain.rs, checkpoint.rs, signer.rs, verifier.rs, mod.rs
  tests/                        # property/verification tests (~28 files)
  benches/                      # (~8 files)
  docs/                         # (~14 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### Key Algorithms & Data Structures

- **ProfileId / Profile**: `ProfileId` = BLAKE3-256 of profile name + region set; `Profile` enum {LocalAcknowledged, QuorumDurable, RegionallyDurable, CrossRegionDurable, EventuallyReplicated, ReadOnlyDegraded, CausallyConsistent} with associated `QuorumConfig{R, W, N, scope}`.
- **Quorum evaluation**: `admit_write(N, W, available) -> bool` iff `available ≥ W`; `admit_read(N, R, available) -> bool` iff `available ≥ R`; `strong_consistent(R, W, N)` iff `R + W > N`; `min_overlap(R, W, N) = W + R − N`.
- **Durability evaluation**: `Local` → local replica only; `Region` → quorum within region; `CrossRegion` → quorum in every region (EACH_QUORUM).
- **Causal context (DVV)**: dotted version vector `((node_id, counter), version_vector)`; `a < b` iff `dot_a.counter ≤ vv_b[dot_a.node]`; concurrent iff neither precedes. Read gate: deliver read only when `causal_past ⊆ applied`.
- **Ack decision**: `WriteAck{required_w, acked: BitSet<NodeId>, durable: bool, causal_dot, decision: Ack|Reject|Degrade}`; `ReadDecision{required_r, responses: Vec<(Endpoint, Value, Version)>, causal_gate, decision: Deliver|Wait|Reject}`.
- **Failure behavior**: per-profile matrix — reject below quorum (QuorumDurable/CrossRegionDurable), degrade to read-only (ReadOnlyDegraded), always-available with hints (EventuallyReplicated), local-only (LocalAcknowledged), bounded causal wait (CausallyConsistent).
- **Anti-entropy digest**: compact state summary (version vector or Merkle tree hash) exchanged peer-to-peer; mismatched digests trigger reconciliation of missing/conflicting entries via `ids-carrier::Endpoint` datagrams.
- **Audit chain**: each profile decision (admit/reject/degrade/deliver/wait) = SHA-256/BLAKE3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of profile envelopes, quorum decisions, audit entries.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### Measurable Acceptance Criteria

1. Profile registration: all 7 profiles register uniquely; `ProfileId` collision-free (BLAKE3-256).
2. Quorum durable: `R + W > N` enforced; 100% of reads after a committed quorum write return the latest value (quorum overlap ≥ 1).
3. Local acknowledged: `W=1, R=1`; write acked on first local replica; read from local replica; stale reads possible (documented).
4. Regionally durable: write quorum within region; async cross-region replication; strong within region, eventual cross-region.
5. Cross-region durable: quorum reached in every region before write ack; single-region failure blocks writes (strong C).
6. Eventually replicated: `W=1, R=1`; anti-entropy converges all replicas to same state given same update set (CRDT merge properties: commutative, associative, idempotent).
7. Read-only degraded: writes rejected 100%; reads served from any available replica during partition.
8. Causally consistent: DVV gating; read-your-writes, monotonic reads, monotonic writes, writes-follow-reads all hold; no linearizability (documented).
9. Failure behavior: write succeeds iff ≥ W available; read succeeds iff ≥ R available; below quorum → rejected (QuorumDurable/CrossRegionDurable) or degraded (ReadOnlyDegraded).
10. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh in the crate.
11. Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in the crate's crypto surface.
12. Every profile decision recorded in append-only, hash-chained, ed25519-signed audit log with `NodeId` + `IdsTime`.

---

### Remaining Uncertainties

- **Quorum defaults per profile:** concrete `(N, R, W)` defaults for each profile (e.g. N=3, W=2, R=2 for QuorumDurable) are deployment-tuned; to be validated at integration with `ids-store`/`ids-replication`.
- **Anti-entropy interval and digest size:** gossip period and Merkle/vector digest tuning affect convergence speed vs bandwidth; workload-dependent.
- **Causal wait timeout:** bounded wait for causal past satisfaction; timeout value trades consistency vs availability under partition.
- **Cross-region latency budget:** `CrossRegionDurable` WAN round trips may exceed acceptable latency for some workloads; SLO definition pending.
- **Interaction with `ids-immutable` (Phase 040):** profile write admission must respect holds/retention; exact guard handshake defined here, validated at integration.
- **Interaction with `ids-session` (Phase 050):** session-bound profile tags and audit cross-referencing to be validated at integration.
- **Read repair vs anti-entropy:** whether read path performs read-repair (fetch newest, push to stale replicas) in addition to background anti-entropy; design decision pending.

---

### Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-consistency-profile` (not a folder inside an existing crate) | Follows "separate crates instead of more folders" directive; clear policy boundary |
| Profile enum with associated `QuorumConfig{R, W, N, scope}` | Single data structure captures all seven profiles; R/W/N tuning explicit |
| Strongest profile = QuorumDurable (R + W > N), not linearizability | Linearizability needs consensus/leader (Raft read_index) — out of scope; quorum intersection gives read-your-writes without consensus |
| CausallyConsistent via dotted version vectors (DVV) | Compact causality tracking; available under partition; read-your-writes + monotonic guarantees |
| EventuallyReplicated = partial quorum (W=1, R=1) + anti-entropy gossip + CRDT merge | Dynamo sloppy quorum/hinted handoff pattern; convergence without coordination |
| ReadOnlyDegraded rejects writes, serves reads | CAP: choose availability during partition |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |
| Logical clocks only (DVV dots/counters + monotonic IdsTime); no wall-clock for causality | Clock manipulation cannot break causal order |
| Carrier-agnostic boundary via `ids-carrier::Endpoint`; crate returns decisions, never sends bytes | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; `ids-crypto` exposes primitive IDs via `ids-000-aa` | Complies with allowed-crypto rule |

---

### Sources

[^1]: Apache Cassandra, "Dynamo" architecture / consistency levels (ONE/TWO/THREE/QUORUM/ALL/LOCAL_QUORUM/EACH_QUORUM/ANY/LOCAL_ONE; R + W > N). https://cassandra.apache.org/doc/stable/cassandra/architecture/dynamo.html
[^2]: Stanford CS145, "Consistency in Replicas" — quorum R + W > N, read/write quorum overlap. https://cs145.stanford.edu/Module5-Distributed/consensus-leader-election.html
[^3]: UMD, "A note on Quorum Consensus" — R + W > N, read/write quorum intersection. http://web.mit.edu/6.033/2005/wwwdocs/quorum_note.html
[^4]: MIT 6.033 Quorum Consensus — read quorum R, write quorum W, R + W > N ensures overlap. http://web.mit.edu/6.033/2005/wwwdocs/quorum_note.html
[^5]: arXiv 2002.06098, "Consistency Analysis of Replication-Based Probabilistic Key-Value Stores" — strict vs partial quorums, R + W > N vs W + R ≤ N. http://arxiv.org/pdf/2002.06098v1
[^6]: thecodeforge.io, "Quorum R+W>N" — quorum overlap, write/read quorum spectrum, minimum overlap = W + R − N. https://thecodeforge.io/system-design/quorum-consensus
[^7]: medium.com/double-pointer, "Replication for System Design Interview (11): R + W > N" — write/read availability, N−W / N−R failure tolerance. https://medium.com/double-pointer/replication-for-system-design-interview-12-r-w-n-19cb93ef7ca3
[^8]: deepengineering.net, "Designing Staleness SLOs for Dynamo-Style KV Stores" — N/R/W control surfaces, staleness windows. https://deepengineering.net/p/deep-engineering-13-designing-staleness
[^9]: cs.cornell.edu, Dynamo paper (cs5414) — sloppy quorum, hinted handoff, R/W quorum protocol. https://cs.cornell.edu/courses/cs5414/2017fa/papers/dynamo.pdf
[^10]: Cassandra docs, "Configuring data consistency" — EACH_QUORUM, LOCAL_QUORUM, multi-datacenter. https://docs.datastax.com/en/cassandra/2.0/cassandra/dml/dml_config_consistency_c.html
[^11]: Wikipedia, "Causal consistency" — available under partition, asynchronous, stronger than eventual. https://en.wikipedia.org/wiki/Causal_consistency
[^12]: MongoDB Docs, "Causal Consistency and Read and Write Concerns" — majority read + majority write = read-own-writes, monotonic reads/writes, writes-follow-reads. https://mongodb.com/docs/manual/core/causal-consistency-read-write-concerns
[^13]: MongoDB Docs, "Read Isolation, Consistency, and Recency" — stale secondary reads, causal consistency, snapshot/read-committed. https://mongodb.com/docs/manual/core/read-isolation-consistency-recency
[^14]: ACM DL 10.1145/2332432.2332497, "Efficient causality tracking with dotted version vectors" — DVV definition, causal precedence, compact comparison. https://dl.acm.org/doi/10.1145/2332432.2332497
[^15]: Attiya, SPAA'91 — sequential consistency vs linearizability. https://groups.csail.mit.edu/tds/papers/Attiya/SPAA91.pdf
[^16]: MIT 6.824, linearizability notes — strong consistency = linearizability, real-time order. https://pdos.csail.mit.edu/6.824/notes/l-linearizability.txt
[^17]: Princeton COS418 / COS518, strong consistency lectures — linearizability definition. https://cs.princeton.edu/courses/archive/fall16/cos418/docs/L13-strong-cap.pdf; https://cs.princeton.edu/courses/archive/spring19/cos518/docs/L3-strong-consistency.pdf
[^18]: blogs.bmc.com, "CAP theorem" — consistency vs availability under partition. https://blogs.bmc.com/cap-theorem
[^19]: Wikipedia, "PACELC design principle" — P → A/C; E → L/C extension of CAP. https://en.wikipedia.org/wiki/PACELC_theorem
[^20]: cs.umd.edu, Abadi PACELC paper — consistency/latency tradeoff, Dynamo/Cassandra/Riak. https://cs.umd.edu/~abadi/papers/abadi-pacelc.pdf
[^21]: arXiv 1805.06358, "Conflict-free Replicated Data Types" — CRDT properties, LWW/MV-register, strong convergence. https://arxiv.org/html/1805.06358
[^22]: arXiv 2504.05398, "CRDT Emulation, Simulation, and Representation Independence" — state-based vs op-based CRDTs, strong convergence. https://arxiv.org/html/2504.05398
[^23]: arXiv 1603.01529, "Delta State Replicated Data Types" — δ-CRDT, small delta-state merge, anti-entropy. https://arxiv.org/html/1603.01529
[^24]: arXiv 1511.05010, "Eventually Consistent Register Revisited" — MV-register, LWW, conflict resolution. https://arxiv.org/html/1511.05010v1
[^25]: systemoverflow, "Anti-Entropy, Digests, and Reconciliation" — gossip digest exchange, Merkle/version-vector reconciliation. https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems
[^26]: CockroachDB Docs, "Follower Reads" — stale/bounded/exact staleness reads, consistent views. https://docs.cockroachlabs.com/docs/stable/follower-reads
[^27]: Google Cloud, "Spanner: TrueTime and external consistency" — external consistency, MVCC timestamps. https://docs.cloud.google.com/spanner/docs/true-time-external-consistency
[^28]: AWS, "DynamoDB Global Tables" — multi-Region strong vs eventual consistency, RPO zero. https://aws.amazon.com/dynamodb/global-tables
[^29]: ed25519-dalek docs.rs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
[^30]: chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
[^31]: sha2 RustCrypto — SHA-256 implementation. https://docs.rs/sha2
[^32]: BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^33]: plan-050.md (IDS plan format reference) — C:/idfs/plans/plan-050.md
[^34]: plan-046.md (IDS plan format reference) — C:/idfs/plans/plan-046.md

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 060.
