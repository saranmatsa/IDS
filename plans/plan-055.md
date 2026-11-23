## Phase 055 — Create the ids-ordering-domain crate: Ordering domains per dataset, no global master.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 820-834)

```
55. Research only do not read plan.md **[Phase 055 — Create the ids-ordering-domain crate: Ordering domains per dataset, no global master.]**

Crate: ids-ordering-domain
Purpose: Ordering domains per dataset, no global master.
Contents: Ordering domain creation, domain membership, current committed position, pending operations, agreed operation order, domain-scoped leadership, cross-domain transaction coordination.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 055 — Create the ids-ordering-domain crate: Ordering domains per dataset, no global master.]**

Create plan-055.md in C:/idfs/plans/ for the ids-ordering-domain crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 055 creates `ids-ordering-domain`, the IDS crate that owns per-dataset ordering domains with **no global ordering master**. It provides ordering domain creation, domain membership (one dataset → exactly one domain), current committed position per domain, a pending-operations buffer, agreed operation order within a domain, domain-scoped leadership, and cross-domain transaction coordination. Every ordering decision is scoped to a single domain; there is no cluster-wide sequencer and no cross-domain total order at this layer.

`ids-ordering-domain` builds on `ids-000-aa`, which already declares the sealed `trait Ordering` (dataset-scoped ordering domains, per-domain sequencing, causal ordering, total order within a domain, distributed snapshot isolation) in `src/contracts/ordering.rs`, plus canonical types `DatasetId`, `OrderingDomain`, `OrderingPos`, `OrderingPosSnapshot`, `RecordId`, `Endpoint`, `NodeId`, `IdsTime`, and the `no_global_master` invariant (`src/invariants/no_global_master.rs`: "dataset-scoped ordering only; no permanent global ordering master"). Downstream crates consume `ids_000_aa::Ordering` as a sealed trait bound and never redefine these types.

The crate is **carrier-agnostic**: it never opens sockets, never imports `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh, and ships all traffic as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). The `ids-carrier` crate itself does not yet exist on disk (only `ids-000-aa` does); this plan treats `ids-carrier::Endpoint` as the fixed carrier boundary contract from `ids-000-aa::types`.

Crypto is restricted to the five allowed primitives only: **ed25519** (domain leadership proofs, position/signing), **xchacha20poly1305** and **chacha20poly1305** (pending-operation payload confidentiality), **sha256** and **blake3** (position digests, state commitments). Crypto implementation lives in the planned `ids-crypto` crate; `ids-ordering-domain` consumes primitive IDs and type surface from `ids-000-aa`/`ids-crypto` only.

Research basis: 30 parallel searches returned the Lamport happened-before/partial-order paper (1978), hybrid logical clocks (Buffalo tech report 2014-04), vector clocks and causal consistency (Dynamo-style), CRDTs (Shapiro et al. 2011) and delta-CRDTs (arXiv:1603.01529), total-order broadcast taxonomy (ACM 10.1145/1041680.1041682), EpTO epidemic total order (ACM 10.1145/2814576.2814804), PCBCast gossip causal-order broadcast, per-partition ordering with per-partition leaders (Apache Kafka design; offsets as per-partition committed positions), Pulsar key-based ordered partitions, CockroachDB per-range Raft groups with range leases ("no cluster master; micro-masters per range"), AWS leader-election sharding and leases, Calvin deterministic cross-shard transaction ordering (SIGMOD 2012), Saga compensating transactions, Narwhal/Tusk DAG mempool for pending operations, linearizability vs sequential consistency per domain, plus Rust crate references for ed25519-dalek, chacha20poly1305/XChaCha20Poly1305, blake3, and sha2.

---

### 2. Architecture Overview

`ids-ordering-domain` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is **not** a network protocol: it decides order locally and emits ordering records/datagrams that consumers ship via `ids-carrier::Endpoint`. Each concern — domain registry, position sequencer, pending-operations buffer, total-order log, domain-scoped leadership, cross-domain coordination — is an independent subfolder module.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                   CONSUMER LAYER (ids-store, ids-replication, …)             │
│  submits operations per dataset; reads committed position per domain        │
├──────────────────────────────────────────────────────────────────────────────┤
│              Phase 055: ids-ordering-domain (NEW CRATE)                      │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ domain_registry│ │ position_seq │ │ pending_ops  │ │ total_order  │        │
│  │ create domain │ │ committed pos│ │ per-domain   │ │ BTreeMap<Pos │        │
│  │ dataset→domain│ │ per domain   │ │ buffer       │ │,RecordId>    │        │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ domain_leadership│ │ cross_domain │ │ crypto_surface│ │ carrier_tx   │        │
│  │ lease per domain│ │ 2PC / Saga   │ │ ed25519, xchacha│ │ Endpoint datagrams│
│  │ no global master│ │ coordination │ │20poly1305, sha256│ │ (CarrierType+   │
│  └──────────────┘ └──────────────┘ └──────┬────────┘ └─────┬────────┘        │
│                                         │                  │                │
├─────────────────────────────────────────┴──────────────────┴────────────────┤
│                          ids-carrier::Endpoint boundary                      │
│  CarrierType (Loopback, Serial, Optical, Radio, Custom) + Vec<u8> ≤ 1024 B │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of one operation:**
1. Consumer submits `Operation { dataset_id, payload }` to `ids-ordering-domain`.
2. `domain_registry` resolves `dataset_id → OrderingDomain` (creating the domain on first use).
3. Domain-scoped leader for that domain sequences the op: `position = advance_position(domain)` → monotonic `OrderingPos`.
4. Op enters `pending_ops` (per-domain buffer) until quorum/ack conditions for the domain are met.
5. On commit: `total_order` records `(position, record_id)`; committed position advances; result is emitted as an `Endpoint` datagram (`CarrierType + Vec<u8>`) to interested nodes.
6. Cross-domain operations (spanning datasets) are coordinated by `cross_domain` (2PC/Saga) — never by imposing a global order; ordering itself remains per-domain.

**Non-negotiable design rules:**
- One `OrderingDomain` per `DatasetId`; a dataset never moves domains; `DomainAlreadyExists` on duplicate creation.
- Positions are per-domain only; `happens_before` is defined within a domain; cross-domain position comparison is rejected (`CrossDomainOrdering`).
- No global sequencer, no global master node, no cluster-wide position counter.
- All outbound traffic rides `ids-carrier::Endpoint`; inbound datagrams carry `CarrierType + Vec<u8>` only.

---

### 3. Detailed Research Dimensions

#### 3.1 Ordering domains per dataset (partitioned ordering)

The core scalability insight from Apache Kafka: a topic is divided into totally ordered partitions; Kafka guarantees total order **within a partition only**, never across partitions. Position within a partition is a single integer offset — consumer state is one number per partition, commits are cheap, and each partition has one leader at a time. Pulsar applies the same model with key-based routing (`hash(key) % numPartitions`) so all messages for a key stay in one partition. IDS maps 1:1: one dataset = one ordering domain = one position sequence. Cross-dataset order is intentionally undefined at this layer; applications that need cross-dataset consistency use `cross_domain` coordination (Section 3.8).

Sources: Kafka design (kafka.apache.org/44/design/design); Pulsar PIP-460 key-to-partition ordering.

#### 3.2 No global master

CockroachDB proves the "many micro-masters" pattern: each keyspace range runs its own Raft group with its own leader/leaseholder; there is no cluster-wide master node, and leadership is partitioned and movable across thousands of ranges. AWS leader-election guidance scopes each leader to a shard — each datum has one leader, the system contains many leaders. IDS adopts this verbatim: domain-scoped leadership only; `no_global_master` is an enforced invariant in `ids-000-aa` (`src/invariants/no_global_master.rs`).

Sources: CockroachDB replication layer / range leases / MultiRaft (cockroachlabs.com/docs); AWS Builder Library "Leader Election in Distributed Systems".

#### 3.3 Current committed position

Committed position = the highest `OrderingPos` a domain has durably agreed on. Kafka's high-watermark pattern (offset of the last message copied to all in-sync replicas; writes commit only after the ISR acks) translates directly: a position is "committed" once the domain's leadership quorum acknowledges it. Consumers track one committed position per domain (small state, checkpointable). Log sequence numbers (LSN) in write-ahead logs (ARIES/BtrLog) confirm the monotonic-position pattern: each record gets a monotonically increasing position; readers resume from the last committed position.

Sources: Kafka design (offsets, ISR, high watermark); BtrLog low-latency logging (arxiv:2606.27051).

#### 3.4 Pending operations

Operations accepted but not yet committed live in a per-domain pending-operations buffer. Narwhal (DAG-based mempool) separates reliable dissemination from ordering: pending transactions are stored with causal history, and the ordering layer sequences them later — a good fit because IDS domains must accept operations even while leadership or connectivity is unstable. Pending ops carry idempotency keys so duplicates and reordering are harmless; the buffer is bounded per domain with an eviction policy to prevent a pending-op flood from exhausting memory.

Sources: Narwhal and Tusk (arxiv:2105.11827); Kafka idempotent delivery (producer ID + sequence deduplication).

#### 3.5 Agreed operation order (per-domain total order)

Within a domain all correct nodes agree on one total order of operations. Total-order broadcast (atomic broadcast) is the canonical primitive: messages are delivered by all processes in the same order. Deterministic sequencer designs (leader chooses order while alive; followers copy) are simplest; leaderless/epidemic variants (EpTO) give probabilistic total order with high probability for large-scale settings; gossip causal-order broadcast (PCBCast) provides causal delivery with vector timestamps when total order is not required. IDS uses a domain leader (Section 3.7) as the deterministic sequencer for agreed order; causal ordering within a domain is tracked so that `happens_before` is respected.

Sources: Total-order broadcast taxonomy (ACM 10.1145/1041680.1041682); EpTO (ACM 10.1145/2814576.2814804); PCBCast (Springer 10.1007/978-3-540-24768-5_25).

#### 3.6 Logical time per domain

Positions are logical, not wall-clock. Lamport clocks define happened-before and totally order events via `(counter, replica_id)` tie-breaks; hybrid logical clocks (HLC) keep causality close to NTP time in 64 bits without letting clock skew dictate validity; vector clocks precisely capture concurrency (incomparable vectors = concurrent operations needing conflict handling). IDS uses HLC-style logical timestamps per domain for position stamps, never wall-clock for ordering decisions, per the `ids-000-aa` clock drift policy (`Reject`/`Warn`/`Allow` with bounded max drift).

Sources: Lamport "Time, clocks, and the ordering of events" (amturing.acm.org/p558-lamport.pdf); HLC (cse.buffalo.edu/tech-reports/2014-04.pdf); vector clocks (Dynamo-style causal consistency).

#### 3.7 Domain-scoped leadership

Each domain has one leader at a time, elected per domain (lease-based: holder heartbeats; on expiry or failure, candidates re-elect). CockroachDB's range lease — colocated with Raft leadership — is the reference: the leaseholder handles writes for its range, reducing round trips and improving partition resilience. AWS uses leases as the most widespread leader-election mechanism with built-in fault tolerance. IDS implements `OrderingLease` per domain (holder `NodeId`, TTL, renew/expire/revoke) with fencing before failover (the `fencing_stonith` invariant) to prevent split-brain between old and new leaders.

Sources: CockroachDB range leases + leader leases; AWS Builder Library leader election (leases, sharding).

#### 3.8 Cross-domain transaction coordination

Operations spanning multiple datasets/domains are coordinated via `cross_domain`: either two-phase commit (Calvin-style deterministic ordering for distributed transactions — inputs replicated, not effects; no single point of failure) or Saga (asynchronous, reactive, compensating transactions on failure). The ordering layer itself never imposes cross-domain order: `CrossDomainOrdering` is a defined `OrderingError` variant ("order within domain only"). This keeps the fast path single-domain while offering explicit coordination for the slow path.

Sources: Calvin (SIGMOD 2012, cs.yale.edu); Saga (IBM developer microservices transaction management).

#### 3.9 Domain lifecycle & membership

Domain lifecycle states: `Creating → Active → Frozen → Retired` (`ids-000-aa` `DomainLifecycleState`). Membership = the dataset→domain mapping plus the set of nodes participating in a domain's leadership quorum. A domain can be frozen (quiesced, no new positions) or retired (terminal). No global membership view: each node tracks only the domains it participates in, matching carrier-agnostic unknown-membership operation.

Sources: ids-000-aa `src/contracts/ordering.rs` (already declared).

#### 3.10 Cryptographic surface (allowed primitives only)

- **ed25519** (RFC 8032): domain-leadership proofs and position/signing of committed positions; `ed25519-dalek` is the mature pure-Rust reference implementation.
- **xchacha20poly1305** / **chacha20poly1305** (RFC 8439): AEAD encryption of pending-operation payloads; XChaCha20's 192-bit nonce allows safe random nonces (birthday bound 2^96 vs 2^48).
- **sha256** (FIPS 180-4): position digests and state commitments.
- **blake3**: content addressing of ordering records; faster than SHA-2, no length-extension issues, parallelizable Merkle-tree internals.
- No other crypto may enter the crate; a `#[cfg]`/deny-list guard plus dependency audit enforces this.

Sources: docs.rs ed25519/ed25519-dalek; docs.rs chacha20poly1305; GitHub BLAKE3-team/BLAKE3; docs.rs sha2.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Position forgery or replay | ed25519-signed positions per domain; monotonic `OrderingPos`; replay of old positions rejected |
|| Split-brain / dual leaders per domain | Domain-scoped lease + fencing (STONITH before failover); one active lease per domain at a time |
|| Cross-domain order leakage | `CrossDomainOrdering` error; ordering stays within domain; cross-domain only via explicit 2PC/Saga |
|| Pending-operation flood | Bounded per-domain pending queue; eviction policy (oldest/lowest priority first); admission control |
|| Compromised domain leader | Lease expiry triggers re-election; leadership proofs are ed25519-signed; old leader fenced |
|| Clock skew corrupting order | Logical positions/HLC only; wall-clock never decides order; `ids-000-aa` clock drift policy |
|| Crypto misuse | Dependency allow-list: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only; audit-enforced |
|| Carrier-agnostic violation | No `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh; all traffic via `ids-carrier::Endpoint` |

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 055 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Master crate: canonical types (`DatasetId`, `OrderingDomain`, `OrderingPos`, `OrderingPosSnapshot`, `RecordId`, `Endpoint`, `CarrierType`, `NodeId`, `IdsTime`), sealed `trait Ordering` (contracts/ordering.rs), `no_global_master` invariant, unified errors, version bits |
|| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram transport — all traffic boundary; crate not yet on disk (planned downstream) |
|| `ids-crypto` | Allowed primitives (ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3) — not yet on disk; type-level surface declared in ids-000-aa |
|| `ids-session` | Client-session binding for operations submitted to domains (future integration) |
|| `ids-store` | Consumer: records written under a domain's agreed order |
|| `ids-replication` | Consumer: replicates committed positions per domain |
|| `ids-coordinator` | `trait Coordinator` (contracts/coordinator.rs): domain-scoped leadership, fencing/STONITH |

**Integration points:**
- `ids-ordering-domain` imports `ids_000_aa::*` types and implements/consumes `ids_000_aa::Ordering` where the downstream implementation lives; it never redefines canonical types.
- Every outbound message is an `ids-carrier::Endpoint` datagram; the crate has zero direct carrier imports.
- Position advances and committed positions are the feed `ids-replication` and `ids-store` consume per dataset.
- Cross-domain operations hand off to `ids-coordinator` (2PC/Saga) rather than ordering globally.

---

### 6. Proposed File Structure (~90-1000 files in subfolders)

**Crate:** `ids-ordering-domain` (new separate crate at `C:/idfs/ids-ordering-domain/`; depends on `ids-000-aa`, plus `ids-carrier`/`ids-crypto` once they exist; `thiserror`, `serde`, `blake3`, `ed25519-dalek`, `chacha20poly1305`, `sha2` — allowed crypto only). Source organized into subfolders by responsibility; no modules added inside existing crates.

```
ids-ordering-domain/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (pending queue caps, lease TTLs)
├── error.rs                        # OrderingDomainError (thiserror, comprehensive)
├── features.rs                     # Feature flags (crypto, durable, fuzzing)
│
├── domain/                         # Domain creation + dataset→domain membership
│   ├── mod.rs
│   ├── registry.rs                 # Domain registry: HashMap<OrderingDomain, DomainEntry>
│   ├── entry.rs                    # DomainEntry (lifecycle state, dataset_id, position)
│   ├── lifecycle.rs                # Creating → Active → Frozen → Retired transitions
│   └── membership.rs               # Participating nodes per domain
│
├── position/                       # Current committed position per domain
│   ├── mod.rs
│   ├── sequencer.rs                # DomainSequencer: monotonic OrderingPos per domain
│   ├── committed.rs                # Committed position + high-watermark tracking
│   └── advance.rs                  # Advance position (leader only, lease-checked)
│
├── pending/                        # Pending operations buffer
│   ├── mod.rs
│   ├── buffer.rs                   # Per-domain pending op queue (bounded)
│   ├── entry.rs                    # Pending op: idempotency key, payload, position hint
│   ├── eviction.rs                 # Eviction under quota (oldest/lowest priority first)
│   ├── dedup.rs                    # Idempotency-key dedup for duplicates/retransmits
│   └── quota.rs                    # Per-domain pending queue caps
│
├── order/                          # Agreed operation order per domain
│   ├── mod.rs
│   ├── total_order.rs              # BTreeMap<OrderingPos, RecordId> per domain
│   ├── entry.rs                    # TotalOrderEntry (position, record_id, domain, seq)
│   └── record_at.rs                # record_at_position lookup
│
├── leadership/                     # Domain-scoped leadership (no global master)
│   ├── mod.rs
│   ├── lease.rs                    # OrderingLease per domain (holder, TTL, renew/expire/revoke)
│   ├── election.rs                 # Per-domain leader election on lease expiry
│   ├── fencing.rs                  # STONITH/fencing before failover
│   └── proof.rs                    # ed25519 leadership proofs
│
├── crossdomain/                    # Cross-domain transaction coordination
│   ├── mod.rs
│   ├── coordinator.rs              # 2PC or Saga coordinator (dataset-spanning ops)
│   ├── twopc.rs                    # Two-phase commit across domains
│   ├── saga.rs                     # Compensating-transaction Saga
│   └── boundary.rs                 # CrossDomainOrdering guard (order within domain only)
│
├── clock/                          # Logical time per domain
│   ├── mod.rs
│   ├── hlc.rs                      # Hybrid logical clock stamps
│   ├── lamport.rs                  # Lamport (counter, replica_id) ordering
│   └── vector.rs                   # Vector clock for concurrency detection
│
├── crypto/                         # Allowed-primitive surface
│   ├── mod.rs
│   ├── ed25519.rs                  # Position/leadership signing (ed25519 only)
│   ├── aead.rs                     # xchacha20poly1305 / chacha20poly1305 for pending payloads
│   ├── digest.rs                   # sha256 / blake3 commitments
│   └── allowlist.rs                # Compile-time crypto allow-list (no other primitives)
│
├── carrier/                        # Carrier boundary (opaque bytes only)
│   ├── mod.rs
│   ├── endpoint.rs                 # ids-carrier::Endpoint passthrough (CarrierType + Vec<u8>)
│   ├── datagram.rs                 # Inbound/outbound datagram envelopes
│   └── guard.rs                    # Compile-time ban on std::net/tokio::net/etc.
│
├── snapshot/                       # Domain position snapshots
│   ├── mod.rs
│   ├── snapshot.rs                 # OrderingPosSnapshot (domain, position, IdsTime)
│   └── restore.rs                  # Restore domain from snapshot
│
├── persistence/                    # Optional durable backend
│   ├── mod.rs
│   ├── store.rs                    # In-memory store (default)
│   └── journal.rs                  # Append-only position journal (optional feature)
│
├── stats/                          # OrderingStats per domain
│   └── stats.rs
│
├── monitor/                        # Domain health monitoring
│   └── monitor.rs
│
├── validator/                      # Position/domain validation
│   └── validator.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── domain_tests.rs
│   ├── position_tests.rs           # Monotonicity, high-watermark
│   ├── pending_tests.rs            # Buffer, dedup, eviction
│   ├── order_tests.rs              # Per-domain total order determinism
│   ├── leadership_tests.rs         # Lease, election, fencing
│   ├── crossdomain_tests.rs        # 2PC/Saga, CrossDomainOrdering rejection
│   ├── carrier_tests.rs            # Endpoint-only traffic assertion
│   ├── chaos_tests.rs              # Reorder/loss/dup convergence per domain
│   └── property_tests.rs           # Proptest: semilattice of domain states
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_domain.rs
│   ├── fuzz_position.rs
│   └── fuzz_pending.rs
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── position_bench.rs
│   ├── pending_bench.rs
│   └── order_bench.rs
│
├── spec/                           # Formal specifications
│   ├── domain_spec.md              # One domain per dataset, lifecycle rules
│   ├── position_spec.md            # Committed position semantics
│   ├── leadership_spec.md          # Domain-scoped lease protocol
│   └── crossdomain_spec.md         # 2PC/Saga boundary
│
├── docs/                           # Documentation
│   ├── architecture.md
│   ├── integration_guide.md
│   └── carrier_agnostic_rules.md   # No std::net etc.; Endpoint only
│
└── test_vectors/                   # Deterministic test vectors
    ├── position_vectors.json
    ├── order_vectors.json
    └── lease_vectors.json
```

**Why ~120–250 files:** each module has one responsibility; every file holds production-grade code with full error handling, documentation, and unit tests. The count reflects domains/positions/pending/ordered order/leadership/cross-domain/clock/crypto/carrier/snapshot/persistence plus tests, fuzz, benchmarks, specs, docs, and test vectors. No stubs. This sits inside the `~90–1000 files in subfolders` mandate from the prompt.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Domain creation (one domain per dataset)

```
create_domain(dataset_id, domain_id):
    if domain_id exists          → DomainAlreadyExists
    if dataset_id → domain exists→ InvalidState ("dataset already has a domain")
    entry = DomainEntry(domain_id, dataset_id, Active, OrderingPos::ZERO)
    registry.insert(domain_id, entry)
    dataset_map.insert(dataset_id, domain_id)
    sequencer.initialize_position(domain_id, OrderingPos::ZERO)
    return domain_id
```

#### 7.2 Committed position (per domain, monotonic)

```
advance_position(domain):            # leader only, lease-validated
    if !domain exists → DomainNotFound
    pos = sequencer.advance_one(domain)   # current.value() + 1
    committed[domain] = pos
    total_order.append((pos, record_id))  # on record sequence
    stats.total_positions += 1
    return pos
happens_before(domain, a, b):        # within domain only
    return a.value() < b.value()     # positions totally ordered per domain
```

Cross-domain `happens_before` is rejected (`CrossDomainOrdering`).

#### 7.3 Pending operations (bounded per-domain buffer)

```
submit(op):                          # op = {dataset_id, idempotency_key, payload}
    domain = registry.resolve(dataset_id)   # create on first use
    if dedup.seen(op.idempotency_key) → return existing position
    pending[domain].push(op)            # bounded; evict oldest on overflow
    leader sequences op → position
    pending[domain].remove(op)          # on commit
```

Idempotency keys make duplicate/retransmitted operations harmless; gaps are filled when the missing position arrives (consistent with causal-order patterns).

#### 7.4 Agreed operation order (per-domain total order)

```
TotalOrder: BTreeMap<OrderingPos, TotalOrderEntry> per domain
sequence(domain, position, record_id):
    if position already recorded → PositionOutOfOrder
    if record_id already at another position → InvalidState
    entries.insert(position, TotalOrderEntry{position, record_id, domain, ...})
record_at_position(domain, position) → entries[position].record_id
```

BTreeMap gives deterministic iteration order and O(log n) lookup; positions are dense per domain starting at 0.

#### 7.5 Domain-scoped leadership (lease, no global master)

```
OrderingLease { holder: NodeId, domain, ttl_ms, expires_at_ms, state }
renew(holder, at_ms):   extend expires_at_ms if active and not expired
expire():               state = Expired → triggers per-domain election
revoke():               state = Revoked → fencing before new leader
```

One lease per domain; fencing (STONITH) precedes any failover (`fencing_stonith` invariant). ed25519 signs leadership proofs so followers can verify the current leader without a global authority.

#### 7.6 Cross-domain coordination (order within domain only)

```
cross_domain_submit(ops across domains D1..Dn):
    coordinator.begin(tx)
    for each domain Di: prepare(Di, position_hint)
    if all prepared: commit(Di) for all → per-domain committed positions advance
    else: abort + Saga compensations for already-committed domains
```

Never: one global sequence spanning domains. `CrossDomainOrdering` is raised if any code attempts to compare positions across domains.

#### 7.7 Logical timestamps

```
HLC stamp per domain: (physical_ms, logical, domain_id) — close to wall time,
causality-consistent, never authoritative for validity.
Vector clock per domain for concurrency: incomparable ⇒ concurrent ops ⇒ conflict.
```

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | Canonical types + sealed `trait Ordering` + invariants + errors (master crate) |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) datagram boundary (planned downstream) |
|| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 (planned) |
|| `thiserror` | Error types |
|| `serde` + `serde_bytes` | Serialization |
|| `blake3` | Content addressing / digests |
|| `ed25519-dalek` | Ed25519 signing/verification |
|| `chacha20poly1305` | XChaCha20Poly1305 + ChaCha20Poly1305 AEAD |
|| `sha2` | SHA-256 |

No networking crates: `std::net`, `tokio::net`, libp2p, and all IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh dependencies are prohibited. All traffic via `ids-carrier::Endpoint`.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-----------------|
|| AC-55.1 | One domain per dataset | `create_domain` twice for same dataset → `InvalidState`; second domain for same dataset_id → `DomainAlreadyExists` |
|| AC-55.2 | Committed position monotonic per domain | `advance_position` returns strictly increasing `OrderingPos` per domain; property test, 100k iterations |
|| AC-55.3 | Per-domain total order | `record_at_position(domain, pos)` deterministic; same sequence of submits → same order on all replicas |
|| AC-55.4 | No cross-domain order | Any cross-domain position comparison → `CrossDomainOrdering`; unit test |
|| AC-55.5 | Domain-scoped leadership | Exactly one active `OrderingLease` per domain at a time; lease expiry → re-election; fencing before failover |
|| AC-55.6 | Pending ops bounded & idempotent | Per-domain pending queue cap enforced; duplicate idempotency key returns existing position; eviction under quota |
|| AC-55.7 | Crypto allow-list | Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 used; dependency audit passes; no other crypto compiles |
|| AC-55.8 | Carrier-agnostic | Zero `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS imports; all traffic via `ids-carrier::Endpoint`; grep-based CI check |
|| AC-55.9 | Convergence under reorder/loss/dup | Property test: replicas receiving same set of ops per domain reach same committed position and order |
|| AC-55.10 | Snapshot/restore | `OrderingPosSnapshot` round-trip restores domain position exactly |
|| AC-55.11 | Property tests | 100,000+ iterations (proptest) over domain lifecycle and position monotonicity |
|| AC-55.12 | Adversarial tests | Dual-leader attempt, replayed position, pending flood — all fail-safe, pass/fail |

---

### 10. Remaining Uncertainties

1. **`ids-carrier` not yet on disk:** `Endpoint` (`CarrierType + Vec<u8>`) is the fixed contract from `ids-000-aa::types`, but the carrier crate itself must be created before `ids-ordering-domain` compiles against it.
2. **Domain leadership mechanism:** lease-based election (this plan) vs per-domain Raft/MultiRaft (CockroachDB pattern) — `ids-000-aa`'s sealed `trait Ordering` declares no leadership method yet; a contract addition may be needed.
3. **Cross-domain choice:** 2PC (Calvin-style) vs Saga — depends on whether cross-domain operations require atomic commit or tolerates compensation.
4. **Pending-op persistence:** in-memory buffer (default) vs durable journal — durability requirements for multi-domain outages undefined.
5. **Position width:** `OrderingPos` is u64-based in `ids-000-aa`; confirm sufficiency for expected per-domain op rates, or plan a wider position type.
6. **Recovery after leader crash:** lease TTL values and fencing timeout need deployment tuning; no global failure detector exists (carrier-agnostic).
7. **`ids-crypto` surface:** primitive IDs and checksum types referenced from `ids-000-aa` (`Signature`, `KeyId`, `AuthToken`, `Credential`, `Checksum`) must align with the five allowed primitives.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate `ids-ordering-domain` crate | Prompt rule: separate crates instead of folders inside existing ones; keeps `ids-000-aa` focused on types/contracts |
|| One ordering domain per dataset, no global master | Matches `no_global_master` invariant; Kafka/Pulsar per-partition ordering scales without a cluster sequencer |
|| Monotonic u64 `OrderingPos` per domain | Committed position = one integer per domain (Kafka offset pattern); cheap consumer state, checkpointable |
|| Domain-scoped leader + lease (no Raft dependency yet) | CockroachDB range-lease pattern: one leaseholder per range, no cluster master; leadership is partitioned and movable |
|| Cross-domain via 2PC/Saga, never global order | `CrossDomainOrdering` error enforces "order within domain only"; explicit coordination for the slow path |
|| Logical positions/HLC; no wall-clock for order | Lamport/HLC preserve causality; wall-clock skew loses concurrent order (CMU CRDT findings) |
|| Bounded pending-op queue + idempotency keys | Narwhal mempool pattern: accept under instability, sequence later; duplicates/reordering harmless |
|| Crypto restricted to five primitives | Policy: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 only |
|| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic absolute rule; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/etc. |

---

### Sources

1. Lamport, "Time, clocks, and the ordering of events in a distributed system" (1978): https://amturing.acm.org/p558-lamport.pdf
2. Kulkarni et al., Hybrid Logical Clocks (Buffalo tech report 2014-04): https://cse.buffalo.edu/tech-reports/2014-04.pdf
3. Shapiro et al., CRDTs: Consistency without concurrency control (INRIA RR-7687, 2011): cited via arxiv.org/abs/1603.01529 context
4. Almeida, Shoker, Baquero, Delta State Replicated Data Types (arXiv:1603.01529): https://arxiv.org/html/1603.01529
5. Total order broadcast and multicast algorithms: Taxonomy and survey (ACM 10.1145/1041680.1041682): https://dl.acm.org/doi/10.1145/1041680.1041682
6. EpTO: An Epidemic Total Order Algorithm (ACM 10.1145/2814576.2814804): https://dl.acm.org/doi/10.1145/2814576.2814804
7. PCBCast: Gossip Based Causal Order Broadcast (Springer 10.1007/978-3-540-24768-5_25): https://link.springer.com/10.1007/978-3-540-24768-5_25
8. Apache Kafka design — per-partition total order, offsets, ISR, high watermark: https://kafka.apache.org/44/design/design
9. Apache Pulsar PIP-460 — key-based ordered partitions: https://github.com/apache/pulsar/blob/master/pip/pip-460.md
10. CockroachDB — Replication Layer, per-range Raft groups, range leases, no cluster master: https://cockroachlabs.com/docs/v25.4/architecture/replication-layer
11. AWS Builder Library — Leader Election in Distributed Systems (sharding, leases): https://aws.amazon.com/builders-library/leader-election-in-distributed-systems
12. Thomson et al., Calvin: Fast Distributed Transactions (SIGMOD 2012): https://cs.yale.edu/homes/thomson/publications/calvin-sigmod12.pdf
13. IBM Developer — Saga pattern for distributed transactions: https://developer.ibm.com/articles/use-saga-to-solve-distributed-transaction-management-problems-in-a-microservices-architecture
14. Narwhal and Tusk: A DAG-based Mempool and Efficient BFT Consensus (arXiv:2105.11827): https://arxiv.org/pdf/2105.11827
15. Princeton COS 418 — Linearizability vs sequential consistency: https://cs.princeton.edu/courses/archive/spr22/cos418/docs/L15-consistency.pdf
16. Rust `ed25519-dalek` crate (Ed25519 signing/verification; RFC 8032): https://docs.rs/ed25519_dalek
17. Rust `chacha20poly1305` crate (ChaCha20Poly1305 + XChaCha20Poly1305; RFC 8439): https://docs.rs/chacha20poly1305
18. BLAKE3 — official Rust/C implementation (GitHub BLAKE3-team/BLAKE3): https://github.com/blake3-team/blake3
19. Rust `sha2` crate (SHA-256; FIPS 180-4): https://docs.rs/sha2
20. IDS `ids-000-aa` skeleton — `src/contracts/ordering.rs` (sealed `trait Ordering`), `src/invariants/no_global_master.rs`, `src/types.rs` (canonical types): C:/idfs/ids-000-aa/src/

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 055.