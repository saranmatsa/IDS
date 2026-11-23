## Phase 016 — Create the ids-routing crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Note on numbering: prompts.txt entry 16 is ids-routing ("Create the ids-routing crate: Data-aware request resolution, not network packet routing."). plan-016.md covers the ids-routing crate.

---

### 1. Executive Summary

Phase 016 creates `ids-routing`, the IDS data-aware request routing crate. Purpose per prompts.txt entry 16 and CRATES.md §16: **data-aware request resolution, not network packet routing**. Contents: Request resolver, locality-aware routing (geography, health, load, freshness, capacity, ownership, consistency), replica-aware routing, near-to-far routing, adaptive routing, routing tables, path selection, loop prevention.

The crate is **carrier-agnostic**: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-routing` consumes hashing/signing type surface from `ids-000-aa`, no direct `ids-crypto` runtime dep where it would create a cycle — aligned to the rule that `ids-crypto` does not depend on `ids-000-aa`). `ids-000-aa` is the master crate with all canonical types and sealed trait contracts; `ids-routing` depends on `ids-000-aa`.

Routing here is **application-layer request resolution**: given a request keyed by object/entity id plus routing hints, select one or more candidate replicas/endpoints and order the attempt sequence. It is explicitly *not* IP/packet forwarding, BGP/OSPF, or any network-layer concern. Routing decisions are driven by data locality (which replica holds/shards the key), health, load, freshness, capacity, ownership, and consistency requirements — signals that live above the carrier.

Research basis: 30 parallel searches returned locality-aware request distribution (LARD, ACM SIGPLAN), content-based distribution, replica-aware/sticky routing (ClickHouse Cloud), near-to-far and prefix-affinity routing (Ray Serve / vLLM / NVIDIA Dynamo KV-aware routing), topology-aware routing (Kubernetes), capacity-aware load balancing (Google global LB), consistency-aware read routing (FLAIR NSDI'20, MongoDB read preference / nearest), gossip-based routing table dissemination and finite-time convergent gossiping, loop-free distance vector with sequence numbers (DSDV/AODV), application-layer routing requirements (Pamela Zave), DHT/Kademlia application-level lookup (libp2p Kad-DHT, XOR distance, k-buckets, iterative FIND_NODE), TiKV replica selection (leader/follower/mixed strategies, store health/liveness, label matching for data locality), request coalescing / single-flight (deduplicate identical in-flight requests), Merkle Search Trees for efficient state-based sync in open networks, vector clocks / Lamport clocks for causal ordering, blake3 content-addressed identifiers (iroh blobs), ed25519-signed routing metadata, xchacha20poly1305/chacha20poly1305 AEAD encrypted routing envelopes, failover and health-check routing, adaptive path selection (DARD), and Merkle-tree routing-table synchronization.

A skeleton `ids-routing` crate does NOT yet exist at `C:/idfs/ids-routing/`. This plan specifies creation from scratch; no source modifications are made in this phase.

---

### 2. Architecture Overview

`ids-routing` is the request-resolution layer for all IDS inbound requests. It receives a request (key + optional routing hints + consistency requirement) from `ids-client`/`ids-gateway` and returns an ordered list of candidate endpoints (each an `ids-carrier::Endpoint`) to try, plus a routing decision record. It owns: the request resolver, routing tables (locality, replica, health, load, freshness, capacity, ownership, consistency indexes), near-to-far candidate ordering, adaptive path selection, loop prevention (hop count + visited set + sequence numbers), replica selection (leader/follower/mixed/prefer-leader/learner), request coalescing (single-flight per key), and routing-table gossip/dissemination.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CLIENT / ids-client (ids_submit)                 │
│  submit(request with key, routing hints, consistency requirement)   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
┌──────────────────────────────▼──────────────────────────────────────┐
│                  Phase 016: ids-routing (THIS CRATE)                │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐   │
│  │  request         │ │  locality-aware  │ │  replica-aware   │   │
│  │  resolver        │ │  routing table   │ │  selection       │   │
│  │  (key -> key id) │ │  (geo/health/    │ │  (leader/follower/ │   │
│  │                  │ │   load/freshness/│ │   mixed/learner)   │   │
│  │                  │ │   capacity/own.) │ │                  │   │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘   │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐   │
│  │  near-to-far     │ │  adaptive path   │ │  loop prevention │   │
│  │  ordering        │ │  selection       │ │  (hop count/seq) │   │
│  │                  │ │  (cost model)    │ │                  │   │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘   │
│  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐   │
│  │  request         │ │  routing table   │ │  routing table   │   │
│  │  coalescing      │ │  gossip          │ │  persistence     │   │
│  │  (single-flight) │ │  (epidemic sync) │ │  (snapshot+journal)│ │
│  └──────────────────┘ └──────────────────┘ └──────────────────┘   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
┌──────────────────────────────▼──────────────────────────────────────┐
│              REPLICAS / TARGET ENDPOINTS (via ids-carrier)            │
│  ids-read · ids-write · ids-replication · ids-sync · ids-commit-state│
└─────────────────────────────────────────────────────────────────────┘

CARRIER BOUNDARY (no std::net / tokio::net): ids-carrier::Endpoint only
```

**Design rules (non-negotiable):**
- Routing is data-aware, application-layer: selects endpoint(s) by key/ownership/locality, not by IP or packet header.
- All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no `std::net`, `tokio::net`, libp2p, IP/TCP/UDP/QUIC/HTTP/DNS or any carrier import.
- Routing table entries keyed by blake3/sha256 content hash of the request key (content-addressed routing keys).
- Locality signals: geography (zone/region), health (reachable/healthy/unhealthy), load (queue depth / in-flight), freshness (last update / staleness bound), capacity (headroom vs quota), ownership (which replica owns the shard/key), consistency (read preference: leader / follower / nearest / any with staleness bound).
- Near-to-far: order candidates by a composite cost score (locality + load + health + freshness + capacity), try nearest first, fail over on error.
- Adaptive path selection: cost weights adjust from observed latency/success/failure telemetry (feedback loop).
- Loop prevention: per-request hop-count TTL + visited-endpoint set + routing sequence numbers; discard/return error on loop detection.
- Replica selection strategies: leader-only, follower-prefer, mixed, prefer-leader, learner (TiKV-pattern), with store health/liveness gating.
- Request coalescing: identical in-flight requests for the same key coalesce into one leader + waiters (single-flight), protecting downstream under thundering herd.
- Routing-table dissemination: gossip-based epidemic sync (finite-time convergent gossiping) so every node builds a convergent local view without central coordination; Merkle digest anti-entropy optional for table reconciliation.
- Crypto surface: `Checksum([u8;32])` (blake3/sha256), `Signature` (ed25519) from `ids-000-aa`; optional AEAD envelope (xchacha20poly1305 / chacha20poly1305) for encrypted routing metadata; no direct `ids-crypto` runtime dependency.

---

### 3. Detailed Research Dimensions

#### 3.1 Data-Aware Request Resolution vs Network Packet Routing

The foundational distinction (Pamela Zave, "Requirements for Routing in the Application Layer"): network-layer routing decides which node receives a packet based on addresses in headers; application-layer routing is a coordination mechanism that decides which server/application handles a message based on message content and application state. ids-routing is the latter. It resolves a request key to candidate endpoints using data locality and runtime signals — the carrier merely delivers opaque `Vec<u8>` payloads between them. Implication: no IP/TCP/UDP/QUIC/HTTP/DNS assumptions; routing decisions survive re-framing by any carrier; the resolver is pure over `(RequestKey, RoutingHints, ConsistencyReq, LocalView) -> OrderedCandidateList`.

#### 3.2 Locality-Aware Routing — Geography, Health, Load, Freshness, Capacity, Ownership

**Content-based / locality-aware request distribution** (LARD, ACM SIGPLAN Notices): a front-end directs requests to back-ends using requested content plus back-end load, improving cache locality and partitioning storage. ids-routing adopts the same principle keyed on blake3/sha256(request key).

Signals maintained per endpoint (in the locality routing table):
- **Geography**: zone/region of the endpoint; prefer same-zone/region (Kubernetes Topology Aware Routing pattern; LARS latency-aware route servers).
- **Health**: healthy / degraded / unhealthy / unknown, from health checks and carrier-level liveness; unhealthy endpoints excluded until recovered.
- **Load**: current in-flight count / queue depth / recent latency; lower load preferred (least-loaded / power-of-two-choices).
- **Freshness**: staleness of the endpoint's replica relative to the key's latest write; consistency-aware reads filter by max staleness (FLAIR consistency-aware routing; MongoDB maxStalenessSeconds).
- **Capacity**: headroom vs configured quota; near-saturated endpoints deprioritized (capacity-aware load balancing, Google global LB).
- **Ownership**: which endpoint owns the shard/range containing the key (shard ownership, Uber Shard Manager pattern); ownership is the primary filter — only owners (or allowed followers) are candidates.

Candidate score = weighted sum of normalized signals; weights adaptive (Section 3.6). The resolver returns candidates sorted ascending by score (near-to-far).

#### 3.3 Replica-Aware Routing — Leader / Follower / Mixed / Learner

TiKV replica selection (client-go): each region has multiple replicas with one leader. Read types: leader-only, follower-prefer, mixed (any leader/follower), prefer-leader, learner. Selection considers: leader status, store health/slowness, label matching for data locality, previous attempt history, learner status. Writes always go to the leader; reads may route to followers to balance load (read preference primary/secondary/nearest, MongoDB).

ids-routing maps this: `ReplicaReadStrategy { LeaderOnly, FollowerPrefer, Mixed, PreferLeader, Learner }`. Consistency requirement picks the strategy:
- Strong/linearizable read → leader only.
- Stale-acceptable read → follower / mixed / nearest with staleness bound.
- Session/read-after-write → sticky to the replica that served the prior write (replica-aware routing / sticky sessions, ClickHouse Cloud; best-effort, remapped on rescale).

Store health/liveness gating: track reachable/unreachable/unknown per endpoint; exclude unreachable until recovered; request forwarding (leader-unreachable → follower proxies to leader) as optional fallback.

#### 3.4 Near-to-Far Routing and Adaptive Path Selection

**Near-to-far**: candidates ordered by composite locality cost; try nearest first; on failure/circuit-breaker, fall back to next. Ray Serve PrefixCacheAffinityRouter and NVIDIA Dynamo KV-aware routing combine cache/locality overlap with active load: a cache-hot but overloaded worker can lose to a colder worker with lower projected total cost (cache locality + remaining prefill/decode work). ids-routing's cost model: `cost = w_locality·geo + w_load·queue + w_health·penalty + w_freshness·staleness + w_capacity·saturation`, weights tuned adaptively from telemetry.

**Adaptive path selection** (DARD, ICFEM adaptive routing): move traffic from overloaded paths to underloaded paths without central coordination; converges to a Nash equilibrium. ids-routing adapts: each node updates local weights from observed success/failure/latency; gossip disseminates weight deltas; routing converges under stable load. Reinforcement-learning / SLA-based path selection (arxiv adaptive path selection) noted as future enhancement; baseline is deterministic weighted cost + feedback.

#### 3.5 Routing Tables, Path Selection, and Dissemination

Routing tables: per-node bounded view of endpoints + signals (DHT principle: limited links, bounded state; Pastry neighborhood set / leaf set; Kademlia k-buckets organized by XOR proximity of blake3(key) — usable as a *logical* proximity index for locality grouping without any network addressing). Tables updated via **gossip-based dissemination**: epidemic broadcast of table deltas; finite-time convergent gossiping (each node tracks a small neighbor set, exchanges state periodically, converges globally without central coordinator). Merkle Search Trees (Auvolat & Taïani, SRDS 2019) provide efficient state-based reconciliation of routing tables over unreliable channels (causal broadcast not required) — aligned with carrier-agnostic unknown-membership model.

Path selection: given ordered candidates, choose first healthy eligible candidate; mark attempted; on error advance. Path selection is deterministic given (request, local view) so all nodes that see the same view make the same choice — supports convergence and idempotent retries.

#### 3.6 Loop Prevention

Loop-free distance-vector principles (DSDV: per-destination sequence numbers; bad news lower sequence; AODV sequence-number duplicate detection) and exact-hop-count (decrement TTL, discard at zero/on re-visit). ids-routing loop prevention:
- Per-request `hop_count` TTL (u8), decremented at each hop; discard at 0.
- `visited` endpoint set (blake3(endpoint id)); reject/return error if candidate already visited.
- Per-route sequence numbers: routing-table entries carry a monotonic sequence; prefer higher sequence on conflict (DSDV rule), preventing stale loops.
- Deterministic ordering + visited set guarantees no infinite forwarding; bounded retries configurable.

#### 3.7 Request Coalescing (Single-Flight)

Identical in-flight requests for the same key coalesce: one leader performs the resolution + dispatch, waiters reuse the result (Atlas request coalescing / Go singleflight / Varnish proxy_cache_lock / bunny.net). Coalescing key = blake3(request key + consistency req). Per-instance (per-endpoint) coalescing; fleet-wide dedup requires distributed lock (out of scope, noted). Reduces downstream load during thundering herd / cache-miss storms.

#### 3.8 Cryptographic Surface

Allowed only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- blake3/sha256: content-addressed routing keys, endpoint ids, checksums.
- ed25519: sign routing-table deltas and routing decisions for authentication (authenticated gossip, no transport trust).
- xchacha20poly1305 / chacha20poly1305: optional AEAD envelope for encrypted routing metadata envelopes.
- `ids-routing` consumes type surface from `ids-000-aa`; no direct `ids-crypto` runtime dep (avoid cycles).

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Stale routing table causes mis-routed requests | Gossip convergence + sequence numbers + per-request TTL + read-your-writes sticky fallback; error + retry with refreshed view |
|| Routing loop (misconfiguration / partition) | Hop-count TTL, visited-endpoint set, sequence numbers; deterministic fail-close |
|| Unhealthy/overloaded endpoint selected | Health gating, load in cost score, circuit-breaker on repeated failure, fail over to next candidate |
|| Thundering herd on hot key | Request coalescing (single-flight) per key; bounded waiters, waiter timeout |
|| Inconsistent reads from stale replica | Consistency requirement filters candidates by freshness/staleness bound; leader-only for strong reads |
|| Routing-table poisoning / fake advertisements | ed25519-signed table deltas; verify origin before applying (decentralized identity, no trusted third party) |
|| Carrier re-framing / untrusted relay | Routing decisions are content-addressed, carrier-independent; payload opaque; no carrier header trust |
|| Key space / endpoint id exhaustion | blake3 256-bit ids; bounded k-bucket-style tables; eviction policy (LRU/least-recently-used) |
|| Adaptive weight oscillation | Damping, min/max weight bounds, change-rate limits; converge-only-on-stable-observations |

**Security risks:** forged routing ads (mitigated by ed25519 signatures), replay of old table deltas (sequence numbers + TTL), routing-table DoS (bounded table size, per-peer quota), cache-poisoning via false health reports (signed health attestations), timing dependence (never use wall-clock for validity; HLC/Lamport only).

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 016 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Canonical types: `RequestKey`, `EndpointId`, `NodeId`, `ShardId`, `ZoneId`, `RoutingHints`, `ConsistencyReq`, `Checksum`, `Signature`; sealed trait contracts |
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); sole transport; all traffic via `Endpoint` |
|| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 — consumed only via `ids-000-aa` type surface (no runtime dep where cycle-prone) |
|| `ids-read` | Consumer — routes read requests to replica endpoints by strategy |
|| `ids-write` | Consumer — routes write requests to leader/owner endpoint |
|| `ids-replication` | Consumer — routes replication traffic by replica/ownership |
|| `ids-sync` | Consumer — routes sync/anti-entropy by table view |
|| `ids-commit-state` | Consumer — routes commit-state queries |
|| `ids-gateway` / `ids-client` | Entry point — submits requests, receives ordered candidate list |
|| `ids-durability-policy` | Consistency/durability requirements feed routing filters |

**Integration points:**
- `ids-client`/`ids-gateway` call `ids_routing::resolve(request) -> OrderedCandidates`; dispatch via `ids-carrier::Endpoint`.
- `ids-read`/`ids-write`/`ids-replication` consume candidate lists for replica-aware dispatch.
- Routing-table gossip rides `ids-carrier::Endpoint` (opaque bytes), zero carrier imports.
- Crypto only via `ids-000-aa` types (`Checksum`, `Signature`); `ids-crypto` not a direct runtime dep of `ids-routing` to preserve the no-cycle rule (`ids-crypto` does not depend on `ids-000-aa`).
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — carrier-agnostic by design (I-CARR-001 equivalent).

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-routing` (new crate, depends on `ids-000-aa`; `ids-carrier` via `Endpoint` type only; serde, thiserror, bytes, blake3; proptest/bolero for testing)

```
ids-routing/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (TTL, table size, weight defaults)
├── error.rs                        # RoutingError (thiserror, comprehensive)
│
├── resolver/                       # Request resolver (core entry point)
│   ├── mod.rs
│   ├── resolver.rs                 # Resolve(key, hints, consistency) -> OrderedCandidates
│   ├── request_key.rs              # RequestKey, blake3/sha256 content-addressed key
│   ├── routing_hints.rs            # RoutingHints (geo, consistency, staleness bound, strategy)
│   ├── candidate.rs                # Candidate endpoint + score + metadata
│   ├── ordered_candidates.rs       # Ordered attempt list + routing decision record
│   ├── coalescing.rs               # Request coalescing / single-flight per key
│   ├── coalesce_map.rs             # In-flight request map (leader + waiters)
│   └── resolver_proptests.rs       # Property tests: determinism, idempotent retries
│
├── locality/                       # Locality-aware routing table & signals
│   ├── mod.rs
│   ├── locality_table.rs           # Endpoint -> signals (geo/health/load/freshness/capacity/ownership)
│   ├── geography.rs                # Zone/region, same-zone preference
│   ├── health.rs                   # Health state machine (healthy/degraded/unhealthy/unknown)
│   ├── load.rs                     # Load signals (queue depth, in-flight, latency)
│   ├── freshness.rs                # Staleness tracking vs latest write
│   ├── capacity.rs                 # Capacity headroom vs quota, saturation
│   ├── ownership.rs                # Shard/key ownership (owner endpoint filter)
│   ├── signal.rs                   # Normalized signal type + weighting
│   ├── score.rs                    # Composite candidate score
│   ├── locality_proptests.rs       # Property tests: score monotonicity, owner filter
│   └── locality_fuzz.rs            # Fuzzing for signal boundary conditions
│
├── replica/                        # Replica-aware routing
│   ├── mod.rs
│   ├── replica_selector.rs         # Leader/follower/mixed/prefer-leader/learner selection
│   ├── strategy.rs                 # ReplicaReadStrategy enum + consistency mapping
│   ├── sticky.rs                   # Session affinity / sticky routing (best-effort)
│   ├── store_health.rs             # Store health/liveness (reachable/unreachable/unknown)
│   ├── forwarding.rs               # Leader-unreachable follower proxy option
│   ├── label_matching.rs           # Data-locality label matching
│   ├── attempt_history.rs          # Per-candidate previous attempt history
│   └── replica_proptests.rs        # Property tests: strategy correctness
│
├── near_to_far/                    # Near-to-far ordering
│   ├── mod.rs
│   ├── orderer.rs                  # Order candidates ascending by composite cost
│   ├── cost_model.rs               # Configurable cost weights + normalization
│   ├── adaptive.rs                 # Adaptive weight tuning from telemetry (feedback loop)
│   ├── telemetry.rs                # Observed latency/success/failure per endpoint
│   ├── damping.rs                  # Weight damping, min/max bounds, change-rate limits
│   └── near_to_far_proptests.rs    # Property tests: ordering stability
│
├── adaptive/                       # Adaptive path selection
│   ├── mod.rs
│   ├── path_selector.rs            # Select path from candidates; move traffic off overloaded paths
│   ├── equilibrium.rs              # Convergence toward balanced load (Nash-style, DARD-pattern)
│   ├── feedback.rs                 # Feedback loop from dispatch outcomes
│   └── adaptive_proptests.rs
│
├── loop_prevention/                # Loop prevention
│   ├── mod.rs
│   ├── loop_guard.rs               # Hop-count TTL + visited-endpoint set
│   ├── sequence.rs                 # Per-route sequence numbers (DSDV-style)
│   ├── ttl.rs                      # Hop-count TTL (u8), discard at zero
│   ├── visited.rs                  # Visited endpoint set (blake3 endpoint ids)
│   └── loop_proptests.rs           # Property tests: no infinite forwarding, bounded retries
│
├── routing_table/                  # Routing table storage & gossip
│   ├── mod.rs
│   ├── table.rs                    # Bounded local routing-table view (k-bucket-style by blake3 proximity)
│   ├── entry.rs                    # Table entry: endpoint, signals, sequence, TTL
│   ├── gossip.rs                   # Epidemic gossip dissemination of table deltas
│   ├── epidemic.rs                 # Finite-time convergent gossiping (neighbor set + periodic exchange)
│   ├── merkle.rs                   # Merkle digest anti-entropy for table reconciliation (optional)
│   ├── digest.rs                   # Table digest comparison + divergence location
│   ├── sync.rs                     # Table sync: push/pull deltas over ids-carrier::Endpoint
│   ├── retention.rs                # Retention bounds, eviction (LRU/least-recently-used)
│   ├── view.rs                     # Convergent local view (all nodes seeing same state agree)
│   └── table_proptests.rs          # Property tests: convergence under loss/dup/reorder
│
├── persistence/                    # Routing table persistence
│   ├── mod.rs
│   ├── snapshot.rs                 # Full table snapshot (versioned, checksummed)
│   ├── journal.rs                  # Append-only delta journal (WAL-style)
│   ├── recovery.rs                 # Load snapshot + replay journal
│   └── compaction.rs               # Snapshot + journal compaction
│
├── wire/                           # Serialization (carrier-agnostic bytes)
│   ├── mod.rs
│   ├── format.rs                   # Versioned wire envelope for routing messages
│   ├── cbor.rs                     # Deterministic CBOR encoding
│   ├── checksum.rs                 # Payload checksums (blake3/sha256)
│   └── signature.rs                # ed25519 sign/verify of routing deltas
│
├── envelope/                       # Optional encrypted routing metadata envelopes
│   ├── mod.rs
│   ├── aead.rs                     # xchacha20poly1305 / chacha20poly1305 AEAD envelope
│   ├── nonce.rs                    # Nonce generation for AEAD
│   └── envelope_proptests.rs
│
├── failover/                       # Failover & circuit breaking
│   ├── mod.rs
│   ├── failover.rs                 # Fail over to next candidate on error
│   ├── circuit_breaker.rs          # Per-endpoint circuit breaker (open/half-open/closed)
│   ├── retry.rs                    # Bounded retry policy with backoff
│   └── failover_proptests.rs
│
├── clocks/                         # Logical clocks (causality, never wall-clock validity)
│   ├── mod.rs
│   ├── lamport.rs                  # Lamport clock
│   ├── hlc.rs                      # Hybrid logical clock (bounded skew, not validity authority)
│   └── stamp.rs                    # Totally-ordered (counter, replica) stamp
│
├── types/                          # Composed routing types
│   ├── mod.rs
│   ├── routing_decision.rs         # RoutingDecision record (selected + attempted + reason)
│   ├── endpoint_view.rs            # Per-endpoint aggregate view
│   ├── table_id.rs                 # TableId, RoutingRound
│   └── types_proptests.rs
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_resolver.rs
│   ├── fuzz_table.rs
│   ├── fuzz_gossip.rs
│   └── fuzz_envelope.rs
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── convergence_tests.rs        # Table convergence under loss/dup/reorder
│   ├── property_tests.rs           # Proptest/bolero semilattice/score properties
│   ├── resolver_tests.rs           # Resolver unit + integration
│   ├── replica_tests.rs            # Replica selection scenarios
│   ├── loop_tests.rs               # Loop prevention (TTL, visited, seq)
│   ├── failover_tests.rs           # Failover + circuit breaker
│   ├── coalescing_tests.rs         # Single-flight dedup
│   ├── adversarial_tests.rs        # Poisoned ads, stale tables, partitions
│   └── chaos_tests.rs              # Endpoint churn, offline members
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── resolve_bench.rs            # Resolution latency
│   ├── gossip_bench.rs             # Gossip dissemination bandwidth
│   ├── scoring_bench.rs            # Score computation cost
│   └── merkle_bench.rs             # Merkle digest sync
│
├── spec/                           # Formal specifications
│   ├── routing_semilattice.md      # Table-merge semilattice laws
│   ├── convergence_spec.md         # Convergent view statement
│   └── tla/                        # TLA+/PlusCal (placeholder; in separate spec crate)
│
├── docs/                           # Documentation
│   ├── architecture.md             # Routing architecture
│   ├── integration_guide.md        # Integrating with ids-read/ids-write/ids-replication
│   └── routing_strategy_guide.md   # When to use which replica strategy
│
└── test_vectors/                   # Deterministic test vectors
    ├── resolve_vectors.json
    ├── gossip_vectors.json
    └── scoring_vectors.json
```

**Why ~90–1000 files:** each module has a single responsibility; resolver, locality signals, replica selection, near-to-far/adaptive ordering, loop prevention, routing table + gossip + Merkle, persistence, wire format, envelopes, failover, clocks, types, fuzzing, property/adversarial/chaos tests, benchmarks, formal spec, docs, test vectors. The count scales with production-grade error handling, documentation, and unit tests per module. No stubs.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Request Resolution (core)
```
resolve(key, hints, consistency, local_view) -> OrderedCandidates
  1. key_id = blake3(key)   // content-addressed routing key
  2. owner = ownership_filter(key_id, local_view)  // endpoints owning the shard
  3. candidates = owner ∪ allowed_followers (by consistency strategy)
  4. filter: health == healthy; staleness <= consistency.max_staleness; capacity headroom
  5. score(c) = w_geo·geo(c) + w_load·queue(c) + w_health·penalty(c) + w_fresh·staleness(c) + w_cap·saturation(c)
  6. order = sort(candidates, ascending score)   // near-to-far
  7. apply loop guard (hop TTL, visited set, sequence numbers)
  8. coalescing: if in-flight for key_id, join as waiter; else become leader
  9. return OrderedCandidates { order, routing_decision, hop_budget }
```

#### 7.2 Composite Score & Adaptive Weights
```
score(c) = Σ w_i · norm_i(c)   // each norm in [0,1], lower is better
weights adapt from telemetry: observed latency/success/failure per endpoint
damping: Δw bounded per interval; min/max clamp; change-rate limit
convergence: under stable load, weights settle; routing choice deterministic given (request, view)
```

#### 7.3 Replica Selection (TiKV-pattern)
```
strategy LeaderOnly   -> candidates = {leader} (writes, strong reads)
strategy FollowerPrefer -> candidates = followers sorted by score, leader last
strategy Mixed        -> candidates = all replicas sorted by score
strategy PreferLeader -> candidates = leader first, then followers by score
strategy Learner      -> candidates = learner replicas
gate: store health != unreachable; staleness <= maxStaleness for non-leader reads
sticky: best-effort route to replica that served prior write (remapped on rescale)
```

#### 7.4 Loop Prevention
```
hop_count-- at each hop; if hop_count == 0 -> error LoopDetected
if endpoint in visited_set -> error LoopDetected
else visited_set.add(endpoint)
route entry sequence: accept update only if seq > stored seq (DSDV rule)
bounded retries: max_attempts configurable; return Unroutable when exhausted
```

#### 7.5 Gossip-Based Routing-Table Dissemination
```
each node tracks small neighbor set; periodically exchange table deltas
view ← merge(local_view, received_view)   // semilattice merge (union + higher-seq wins)
finite-time convergence: under reliable eventual delivery, all views equal
Merkle digest anti-entropy: exchange root hashes; descend only differing branches; O(d·log n) divergence location
```

#### 7.6 Request Coalescing (single-flight)
```
coalesce_key = blake3(request_key + consistency_req)
if coalesce_map.contains(key):
    register waiter on existing leader's result channel
else:
    become leader; perform resolution; broadcast result to all waiters; remove entry
waiter timeout configurable; on leader failure, waiters re-resolve independently
```

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | Canonical types (RequestKey, EndpointId, NodeId, ShardId, ZoneId, Checksum, Signature) |
|| `ids-carrier` | `Endpoint` type surface only (CarrierType + Vec<u8>) |
|| `ids-crypto` | Consumed via ids-000-aa type surface (ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3); no direct runtime dep where cycle-prone |
|| `serde` + `serde_bytes` | Serialization |
|| `blake3` | Content-addressed routing keys, checksums (already in workspace) |
|| `sha2` (sha256) | Hashing alternative |
|| `thiserror` | Error types |
|| `bytes` | Buffer handling |
|| `proptest` / `bolero` | Property-based testing |
|| `criterion` | Benchmarks |
|| `tokio` | Async runtime |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — `ids-routing` is transport-agnostic over `ids-carrier::Endpoint` bytes.

---

### 9. Measurable Acceptance Criteria

|| Criterion | Target | Measurement ||
||-----------|----------|-------------|
|| Resolution determinism | Same (key, hints, consistency, view) → same ordered candidates | unit test, 100k iterations |
|| Owner filter | Owner endpoint always in candidate set for writes | unit test |
|| Replica strategy | Strong read → leader only; stale read allows followers | scenario test |
|| Near-to-far ordering | Candidates sorted ascending by composite score | unit test |
|| Loop prevention | Hop TTL + visited set + seq prevent infinite forwarding | adversarial test (topology loop) |
|| Request coalescing | N identical in-flight requests → 1 resolution | concurrency test |
|| Health gating | Unhealthy endpoint excluded until recovered | scenario test |
|| Staleness bound | Non-leader reads filtered by maxStaleness | unit test |
|| Gossip convergence | All nodes converge to same table view under loss/dup/reorder | chaos test |
|| Merkle digest | Equal roots ⇒ equal table; O(d·log n) divergence | bench |
|| Resolution latency | p99 within configurable budget (e.g., ≤1 ms) | bench |
|| Failover | Failed candidate → next candidate, bounded retries | scenario test |
|| Circuit breaker | Repeated failures open breaker; half-open recovery | scenario test |
|| Property tests | 100,000+ iterations | proptest/bolero |
|| Adversarial tests | Poisoned ads rejected (ed25519 sig), stale tables | pass/fail |
|| Carrier agnosticism | Compiles with zero std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS imports | build check |
|| Crypto surface | Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | dependency audit |

---

### 10. Remaining Uncertainties

1. **Cost-weight calibration:** Initial weights for geo/load/health/freshness/capacity and adaptive tuning rules are workload-dependent; unquantified without production telemetry.
2. **Sticky-session semantics:** Best-effort replica affinity (ClickHouse pattern) remaps on rescale — read-after-write guarantees need explicit consistency binding, not implicit affinity.
3. **Table size vs churn:** Bounded routing-table eviction (LRU) under high churn may evict still-relevant entries; eviction policy and retention bounds need deployment tuning.
4. **Merkle anti-entropy adoption:** Optional; adds complexity. Decide during implementation based on table size and gossip bandwidth.
5. **Encrypted envelope adoption:** xchacha20poly1305/chacha20poly1305 AEAD for routing metadata — optional, deferred unless threat model requires encrypted routing metadata.
6. **Formal verification scope:** TLA+/PlusCal spec for routing-table convergence and loop freedom is additional effort beyond core classes.
7. **Cross-region consistency routing:** Routing across regions with consistency requirements (read-your-writes, session consistency) needs explicit session-sticky binding design.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Application-layer data-aware routing (not packet routing) | Prompts.txt purpose; Zave requirements; content-addressed keys, carrier-independent |
|| `ids-carrier::Endpoint` sole transport | Carrier-agnostic (I-CARR-001); no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS |
|| blake3/sha256 content-addressed routing keys | Fixed-size, collision-resistant, content-based (LARD principle; iroh blobs pattern) |
|| Composite score + near-to-far ordering | Locality + load + health + freshness + capacity; cache-hot overloaded worker can lose to colder worker (Dynamo/KV-aware) |
|| TiKV-style replica strategies (leader/follower/mixed/prefer-leader/learner) | Production-verified (TiKV, MongoDB read preference); consistency maps cleanly to strategy |
|| Gossip-based table dissemination | Convergent without central coordinator; matches unknown-membership carrier-agnostic model |
|| Merkle Search Trees optional anti-entropy | Efficient state-based reconciliation over unreliable channels (no causal broadcast needed) |
|| Loop prevention: hop TTL + visited set + sequence numbers | Bounded retries, deterministic fail-close, DSDV sequence rule prevents stale loops |
|| Request coalescing (single-flight) | Protects downstream under thundering herd; per-instance coalescing |
|| Logical clocks only (Lamport/HLC), never wall-clock validity | Causality without clock-drift hazards |
|| Crypto via ids-000-aa types only | Preserve no-cycle rule (ids-crypto does not depend on ids-000-aa) |
|| New separate crate ids-routing | Per prompts.txt directive "use separate crates instead of more folders"; clean dependency boundary |

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 016.

**Sources consulted (30 targeted searches):**
- Locality-aware request distribution (LARD), ACM SIGPLAN Notices — content-based request distribution, back-end load + content
- ClickHouse Cloud replica-aware routing — sticky sessions / session affinity, best-effort replica mapping
- Ray Serve PrefixCacheAffinityRouter / Anyscale docs — prefix-aware routing, load-vs-locality tradeoff, power-of-two fallback
- NVIDIA Dynamo KV-aware routing — KV cache + active load cost model, cache-hot-overloaded can lose to colder worker
- CacheRoute (arxiv:2608.19677) — planned prefix-affinity routing, KV hit rate vs load balancing
- Kubernetes Topology Aware Routing — keep traffic within zone/region
- Google global load balancing — capacity-aware load distribution
- FLAIR (NSDI'20) — consistency-aware read routing to followers without breaking linearizability
- MongoDB read preference / server selection — primary/secondary/nearest, maxStalenessSeconds, latency window
- TiKV client-go replica selection — leader/follower/mixed/prefer-leader/learner, store health/liveness, label matching
- Uber Shard Manager — shard ownership decisions separated from application logic
- Gossip-based Networking for Internet-Scale Distributed Systems — epidemic dissemination, local convergence → global convergence
- Finite-time Convergent Gossiping (arxiv:1206.0992) — convergence metrics, pairwise exchange
- Analytical Model of Information Dissemination for Gossip (arxiv:0810.1571) — push/pull replication model
- Loop-Free Distance Vector Algorithms (DSDV sequence numbers) — per-destination sequence numbers, bad-news lower sequence
- Sequence numbers do not guarantee loop freedom (ACM MSWIM) — caveats; sequence numbers need accompanying rules
- Exact Hop Count (IEEE 802.1aq) — decrement TTL, discard at zero/re-visit, loop-free
- Requirements for Routing in the Application Layer (Pamela Zave, AT&T) — application-layer routing as coordination mechanism
- Kademlia / libp2p Kad-DHT — k-buckets, XOR proximity, iterative FIND_NODE lookup (logical proximity index only)
- DARD (ICDCS 2012) — distributed adaptive routing, move traffic off overloaded paths, Nash equilibrium
- Adaptive path selection RL (arxiv:2508.13806) — in-network RL path selection (future enhancement)
- Approaches to CRDTs (ACM Computing Surveys) — strong eventual consistency, semilattice merge (for table merge semantics)
- Merkle Search Trees (SRDS 2019, Auvolat & Taïani) — efficient state-based CRDT sync over open networks
- iroh blobs — BLAKE3 content-addressed identifiers, incremental verification
- Request coalescing / single-flight (Atlas, Varnish, bunny.net, Go singleflight) — dedupe identical in-flight requests
- Vector clocks / Lamport clocks (distributed systems lecture notes; Verified Causal Broadcast) — causality without wall-clock
- ed25519 signed routing metadata / did:crdt — authenticated routing-table deltas
- xchacha20poly1305 / chacha20poly1305 AEAD — encrypted routing metadata envelopes (fortified mesh routing reference)
- Failover / health-check routing (HashiCorp WA, AWS Well-Architected, Azure failover) — detect failure, redirect to healthy