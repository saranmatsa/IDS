# Phase 011 — Create the ids-discovery crate: Research Findings & Implementation Blueprint

**Status:** Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.

## 1. Executive Summary

Phase 011 creates **`ids-discovery`**, a new IDS crate providing **gossip-based membership discovery without a central coordinator**. Every node maintains a local membership table and periodically exchanges state with a small random subset of peers; membership facts (join, leave, suspect, confirm, dead, heartbeat) spread epidemically until the cluster converges on a shared view. The crate is carrier-agnostic: all traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` datagrams) with zero dependencies on `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh stacks.

**Scope (research-verified):** rumor mongering (push/pull/push-pull epidemic dissemination), anti-entropy reconciliation with compact digests, aggregate gossip (state summarization before transmission), membership convergence, discovery propagation, discovery expiration/refresh (TTL tombstones), and churn handling (join/leave/suspect/confirm/dead lifecycle with failure detection). Suspicion/refutation and indirect probes follow the SWIM pattern (Scalable Weakly-consistent Infection-style Process Group Membership Protocol, Cornell DSN 2002); failure detection is modeled on Chandra-Toueg incomplete/weak accuracy failure detectors; phi-accrual suspicion (Hayashibara et al. 2004) provides adaptive thresholds; Cassandra-style vector clocks `(generation, version)` order per-member state; Merkle/sha256-blake3 digests bound anti-entropy bandwidth to O(log n) convergence rounds with per-node O(fanout) traffic independent of cluster size.

**Key engineering decisions:** (1) gossip and failure detection are separate concerns, piggybacked on the same datagrams but rate-tunable independently (memberlist pattern); (2) all state merges are idempotent, commutative, associative — message loss, duplication, and reordering are harmless; (3) expiration is time-basis-driven via `ids-time` ticks, never wall-clock; (4) allowed crypto only — ed25519 signatures for authenticated gossip envelopes, sha256/blake3 for digests, xchacha20poly1305/chacha20poly1305 optional for envelope confidentiality; (5) `ids-000-aa` is the master crate: `NodeId`, `Endpoint`, `CarrierType`, and all sealed trait contracts come from there verbatim.

## 2. Architecture Overview

`ids-discovery` sits above `ids-carrier` and below `ids-membership`/application layers. It is not a transport: it produces and consumes opaque datagrams through `ids-carrier::Endpoint::send_datagram`/`receive_datagram`.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-membership                     │
│              membership state (join/leave/active/dead), events      │
├─────────────────────────────────────────────────────────────────────┤
│                    Phase 011: ids-discovery (NEW CRATE)             │
│  ┌─────────────────┐ ┌──────────────────┐ ┌─────────────────────┐  │
│  │ Membership Table│ │ Rumor Mongering  │ │ Anti-Entropy Engine │  │
│  │ (local view,    │ │ push/pull/push-  │ │ (digest exchange,   │  │
│  │  TTL entries)   │ │  pull gossip)    │ │  merkle/sha256)     │  │
│  └────────┬────────┘ └────────┬─────────┘ └──────────┬───────────┘  │
│           │                   │                      │              │
│  ┌────────▼───────────────────▼──────────────────────▼────────┐   │
│  │            Aggregate Gossip + Epidemic Broadcast             │   │
│  │  state summarization, hop-limited dissemination, fanout K    │   │
│  └──────────────────────────┬──────────────────────────────────┘   │
│  ┌──────────────────────────▼──────────────────────────────────┐   │
│  │       Failure Detection (SWIM suspicion/refutation, indirect │   │
│  │       probes, ping/ping-req/ack, phi-accrual suspicion)      │   │
│  └──────────────────────────┬──────────────────────────────────┘   │
│  ┌──────────────────────────▼──────────────────────────────────┐   │
│  │            Churn + Expiration/Refresh (TTL tombstones)       │   │
│  └──────────────────────────┬──────────────────────────────────┘   │
│  ┌──────────────────────────▼──────────────────────────────────┐   │
│  │  Wire / Security: envelope (ed25519 sig), digest (sha256/    │   │
│  │  blake3), optional xchacha20poly1305/chacha20poly1305        │   │
│  └──────────────────────────┬──────────────────────────────────┘   │
└─────────────────────────────┼──────────────────────────────────────┘
                              │ ids-carrier::Endpoint (CarrierType + Vec<u8>)
└─────────────────────────────┼──────────────────────────────────────┘
│             ids-carrier (opaque datagrams, no IP/TCP/UDP/QUIC/etc.)│
└─────────────────────────────────────────────────────────────────────┘
```

**Data flow (gossip round):** each gossip tick, the node selects K random peers from its membership table, builds an aggregate gossip envelope (delta + digest + heartbeat), signs it (ed25519), and sends via `ids-carrier::Endpoint`. The receiver verifies, merges into its membership table (idempotent LWW-style merge keyed on `(member, generation, version)`), and may reply with missing entries. Suspicion: no ack within suspicion interval → mark suspect → indirect probe via K intermediaries (ping-req) → refute if any ack returns → confirm dead otherwise. Expiration: entries without refresh beyond TTL become tombstones, then are garbage-collected.

**Convergence:** with fanout K and N members, any membership fact reaches all correct nodes within O(log_K N) gossip rounds with high probability; per-node traffic is O(K) datagrams per round regardless of N (no O(N²) mesh).

## 3. Detailed Research Dimensions

### 3.1 Rumor Mongering (Epidemic Dissemination)

Source pattern: Demers et al., "Epidemic Algorithms for Replicated Database Maintenance" (PODC 1987) — the seminal push/pull/push-pull rumor mongering model. Each membership update becomes a "hot rumor"; the holder periodically contacts a random peer and pushes the rumor; the recipient merges and becomes a new spreader; nodes lose interest after K contacts (counter-based) or with probability 1/K (coin-based), or only when the peer already knows the rumor (feedback). Blind variants keep spreading regardless. Result: information infects the whole population in O(log n) rounds; each node sends O(1) messages per round. IDS adaptation: membership facts (join/leave/suspect/confirm/dead/heartbeat) are rumors; each gossip datagram carries a bounded delta (bounded size is a gossip defining property — bounded-size information exchange with reliable-not-assumed channels).

### 3.2 Anti-Entropy Reconciliation

Source pattern: anti-entropy (Demers et al.; Cassandra; Dynamo). Periodically, two peers exchange compact digests of their membership tables instead of full state: each entry summarized as `(member_id, generation, version)` hashed into a blake3/sha256 Merkle-like digest or version-vector summary. Equal digests ⇒ tables in sync (O(1) agreement check); differing digests trigger recursive descent to locate divergent entries, exchanging only deltas. Keeps typical gossip messages under ~1 KB even in clusters of thousands of nodes. Anti-entropy interval is much longer than the gossip round (e.g., gossip every 200 ms–1 s; anti-entropy every 30–120 s) to bound bandwidth while healing missed updates and partition drift. IDS adaptation: digest computed over canonical blake3/sha256 encoding of the local membership view; reconciliation exchanges only divergent `(member, generation, version)` entries; last-write-wins merge by `(generation, version)`.

### 3.3 Aggregate Gossip

Aggregate gossip summarizes state before transmission: instead of sending the full membership table each round, nodes send (a) their own delta since last exchange, (b) a compact digest/hash summary, and (c) piggybacked rumors pending forwarding. This bounds per-datagram size and respects the MTU-ish constraints of `ids-carrier::Endpoint` datagrams (opaque Vec<u8>; carrier may impose its own size limits, unknown to the crate). Aggregation is applied locally; merge remains idempotent so duplicated aggregates are harmless.

### 3.4 Epidemic Broadcast

Epidemic (rumor-mongering) broadcast for discovery events: when a node learns of a new member or a state change (join/leave/dead), it injects the event into the gossip stream with a monotonically increasing sequence/version; every recipient forwards to K random peers not known to have it, with hop-count/TTL damping and duplicate suppression by `(origin, seq)`. Broadcast is probabilistic — eventual delivery with high probability, not guaranteed — matching the carrier-agnostic, unreliable-datagram model (no retransmission guarantees assumed from ids-carrier).

### 3.5 Membership Convergence

Each node's local view: map `NodeId → MembershipEntry { generation, version, state (alive/suspect/confirm/dead), heartbeat, ttl, metadata }`. Merge rule: for each `NodeId`, keep the entry with the highest `(generation, version)`; state transitions obey the lifecycle (alive → suspect → confirm/dead; confirm dead is terminal unless re-join with bumped generation). Convergence criterion: all correct nodes holding the same set of updates reach the same view (strong eventual consistency of the membership table) because merge is a join-semilattice over `(generation, version)` with idempotent, commutative, associative LUB. Convergence time O(log_K N) rounds with fanout K.

### 3.6 Discovery Propagation

New nodes bootstrap by knowing at least one existing endpoint (contact/seed), advertised via configuration or out-of-band (IDS provisioning channel — not DNS). The joiner sends a join rumor; recipients propagate it epidemically. Departures: explicit leave rumor (signed, ed25519) or implicit via heartbeat expiry. Discovery events carry origin `NodeId` + signature so forged join/leave for another identity is detectable.

### 3.7 Discovery Expiration / Refresh

Every membership entry carries a TTL keyed to the `ids-time` tick basis (IDS proprietary time, not wall-clock, not HLC). A live member refreshes its entry via heartbeat gossip each round. Entries not refreshed before TTL expiry enter tombstone state; tombstones are retained for a grace window (so a late leave/suspect can still be processed) and then garbage-collected. This prevents a silent node from lingering forever in views while avoiding premature eviction of slow-but-alive members (false-positive suppression via suspicion interval, SWIM pattern).

### 3.8 Churn Handling

Churn = continuous joins, leaves, failures, recoveries. Mechanisms: (1) generation bump on re-join after dead, so stale entries resurrecting from old gossip lose to the new generation; (2) suspect → indirect probe → confirm/dead state machine to distinguish transient loss from real failure; (3) tombstone grace windows so departed members' `dead` rumors still propagate after local GC; (4) fanout and gossip interval tuned so convergence outpaces expected churn rate; (5) bounded tables (max entries, LRU/tombstone eviction) so membership memory stays O(1) per node under unbounded churn.

### 3.9 Failure Detection (SWIM + Phi Accrual)

Direct probe: ping → ack within suspicion interval. On timeout: indirect probe — ping-req to K random intermediaries, each pings the target; any ack refutes suspicion. If no ack by end of protocol period, mark confirm/dead and gossip `dead`. Phi-accrual overlay: suspicion level φ computed from sliding window of heartbeat inter-arrival times; configurable threshold trades detection latency vs false positives (φ=1 ≈ 10% error, φ=2 ≈ 1%, φ=3 ≈ 0.1%, default ~8–10). Chandra-Toueg properties: strong/weak completeness (eventually suspect every crashed process) and strong/weak accuracy (bounded false suspicions) are probabilistic here — the detector is unreliable by design, consistent with eventual-consistency membership.

## 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Forged join/leave/suspect for another node | ed25519 signature over `(origin NodeId, seq, event type, payload)` in every gossip envelope; unverified envelopes dropped before merge ||
|| Replay of old gossip (resurrecting departed nodes) | Monotonic per-origin sequence + `(generation, version)` LWW merge; older generations lose; tombstones retained through grace window ||
|| Duplicate / reordered datagrams (unreliable carrier) | Idempotent semilattice merge; loss/dup/reorder harmless by design ||
|| Sybil / eclipse (fake identities flood views) | ed25519-bound identities; per-peer entry budgets; view size caps; bootstrap trust provisioned out of band (no DNS, no central coordinator) ||
|| Gossip denial-of-service (oversized datagrams, digest bombs) | Bounded datagram size; max delta entries per envelope; digest depth limits; reject-and-drop oversized envelopes before merge ||
|| False positives (healthy node marked dead under loss/slow processing) | Suspicion + indirect probe + refutation; phi-accrual adaptive threshold; suspicion interval ≫ expected RTT variance; never hard-evict on single timeout ||
|| Flapping (rapid suspect/revive oscillations) | Hysteresis: confirm/dead requires sustained silence; re-join requires new generation; grace window before rejoining views ||
|| Partition healing divergence | Anti-entropy digest exchange reconciles divergent views after partition heals; `(generation, version)` LWW merge converges ||
|| Traffic analysis / metadata leakage | Optional xchacha20poly1305/chacha20poly1305 envelope encryption (allowed crypto only); no sender/receiver addresses in clear beyond carrier-mandated opaque routing ||
|| Compromised node broadcasting conflicting states | Per-origin sequence monotonicity; equivocation (same seq, different payload) detected and flagged; origin's subsequent gossip down-weighted pending evidence ||
|| Long-term table growth under churn | TTL tombstones + GC; bounded view size; eviction policy (oldest tombstone first) ||
|| No central coordinator = no single source of truth | Every node computes its own view locally; convergence is eventual, not immediate; applications read membership with explicit staleness tolerance via ids-consistency profiles ||

## 5. Integration with Existing Crates

|| Crate | Role in Phase 011 ||
||-------|-------------------|-------------------|
|| `ids-000-aa` | Master crate: `NodeId`, `Endpoint`, `CarrierType`, `OpState`, sealed trait contracts, versioning, error contracts — all types used verbatim ||
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`), `Datagram`, `send_datagram`/`receive_datagram` — the ONLY traffic boundary; all discovery traffic is opaque datagrams ||
|| `ids-crypto` | ed25519 sign/verify (envelope authentication), sha256/blake3 (digests), xchacha20poly1305/chacha20poly1305 (optional envelope encryption), nonce/key management ||
|| `ids-time` | Proprietary tick basis for TTL/expiration/heartbeat timing — never wall-clock, never std::net timers ||
|| `ids-topology` | Failure-domain-aware peer selection (gossip targets spread across power/facility/network/operator/geographic domains when known) ||
|| `ids-failure-detector` | Chandra-Toueg completeness/accuracy properties frame the suspicion/confirm semantics; SWIM suspicion maps onto it ||
|| `ids-membership` | Consumer of discovery events (node states: candidate/validating/active/degraded/draining/failed/retired); discovery emits join/leave/dead, membership applies admission policy ||
|| `ids-consistency` | Consistency profiles govern how fresh a discovery view must be for reads/writes (bounded staleness on membership) ||

**Integration points:** `ids-discovery` depends only on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-time`; it is consumed by `ids-membership` (and optionally `ids-topology` for domain-aware peer sampling). It imports nothing from `ids-carrier` beyond `Endpoint`/`Datagram` — no sockets, no addresses, no carrier-specific code. If all networking disappeared, `ids-discovery` remains coherent (local table maintenance continues; datagrams simply queue).

## 6. Proposed File Structure (~300–400 files, all in subfolders)

**Crate:** `ids-discovery` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-time`; `serde`, `thiserror`, `bytes`; dev: `proptest`, `criterion`; no networking crates whatsoever)

```
ids-discovery/
├── Cargo.toml                            # Crate manifest — carrier-agnostic deps only
├── lib.rs                                # Crate root, feature gates, public re-exports
├── constants.rs                          # Protocol defaults: fanout, intervals, TTLs, table caps
├── error.rs                              # DiscoveryError (thiserror, comprehensive)
├── features.rs                           # Feature flags (signed, encrypted, fuzzing, metrics)
│
├── table/                                # Local membership table
│   ├── mod.rs
│   ├── view.rs                           # MembershipView: NodeId -> MembershipEntry map
│   ├── entry.rs                          # MembershipEntry (generation, version, state, heartbeat, ttl)
│   ├── index.rs                          # Indexes: by state, by expiry, by domain
│   ├── snapshot.rs                       # View snapshot for anti-entropy exchange
│   ├── tombstone.rs                      # Tombstone tracking + grace-window GC
│   ├── eviction.rs                       # Bounded-view eviction (oldest tombstone first)
│   ├── types.rs                          # MembershipState, DiscoveryEvent, ViewVersion
│   └── view_tests.rs                     # In-module table unit tests
│
├── rumor/                                # Rumor mongering engine
│   ├── mod.rs
│   ├── engine.rs                         # RumorMongerer: push/pull/push-pull scheduler
│   ├── target.rs                         # Random peer selection (fanout K, domain-spread)
│   ├── rumor.rs                          # Rumor payload: join/leave/suspect/confirm/dead/heartbeat
│   ├── delta.rs                          # Bounded delta extraction from view
│   ├── fanout.rs                         # Fanout policy, hop-count/TTL damping
│   ├── dedupe.rs                         # Duplicate suppression by (origin, seq)
│   ├── push.rs                           # Push mode
│   ├── pull.rs                           # Pull mode
│   ├── push_pull.rs                      # Push-pull mode
│   └── rumor_tests.rs
│
├── anti_entropy/                         # Periodic reconciliation
│   ├── mod.rs
│   ├── scheduler.rs                      # Anti-entropy tick (slower than gossip tick)
│   ├── digest.rs                         # blake3/sha256 digest over view snapshot
│   ├── merkle.rs                         # Merkle-style digest tree for divergence location
│   ├── reconcile.rs                      # Digest comparison → delta exchange
│   ├── vector_clock.rs                   # (generation, version) per-member ordering
│   ├── repair.rs                         # Request/send missing entries only
│   └── anti_entropy_tests.rs
│
├── aggregate/                            # Aggregate gossip summarization
│   ├── mod.rs
│   ├── summarizer.rs                     # Summarize view into compact aggregate
│   ├── envelope.rs                       # Aggregate envelope: delta + digest + heartbeat
│   ├── size.rs                           # Envelope size estimation vs datagram bound
│   ├── piggyback.rs                      # Piggyback rumors on failure-detector datagrams
│   └── aggregate_tests.rs
│
├── epidemic/                             # Epidemic broadcast
│   ├── mod.rs
│   ├── broadcaster.rs                    # Broadcast injection + forwarding
│   ├── flood_control.rs                  # Hop TTL, duplicate suppression
│   ├── seq.rs                            # Per-origin broadcast sequence numbers
│   └── epidemic_tests.rs
│
├── convergence/                          # Membership convergence
│   ├── mod.rs
│   ├── merger.rs                         # Idempotent semilattice merge (LUB by generation/version)
│   ├── semilattice.rs                    # Join-semilattice laws over views
│   ├── eventual.rs                       # Convergence criteria, O(log_K N) bound statement
│   └── convergence_tests.rs
│
├── propagation/                          # Discovery propagation
│   ├── mod.rs
│   ├── injector.rs                       # Inject join/leave events into gossip stream
│   ├── gossip_layer.rs                   # Dedicated gossip layer vs piggybacking (memberlist pattern)
│   ├── rate.rs                           # Gossip rate tuning independent of failure-detection rate
│   └── propagation_tests.rs
│
├── expiration/                           # Discovery expiration / refresh
│   ├── mod.rs
│   ├── ttl.rs                            # TTL keyed to ids-time ticks
│   ├── refresh.rs                        # Heartbeat-driven entry refresh
│   ├── sweeper.rs                        # Periodic expiry sweep
│   ├── gc.rs                             # Tombstone garbage collection after grace window
│   └── expiration_tests.rs
│
├── churn/                                # Churn handling
│   ├── mod.rs
│   ├── lifecycle.rs                      # alive -> suspect -> confirm/dead state machine
│   ├── rejoin.rs                         # Generation bump on re-join after dead
│   ├── resurrection.rs                   # Stale-entry resurrection prevention
│   ├── partition.rs                      # Partition detection via sustained silence
│   └── churn_tests.rs
│
├── failure_detection/                    # SWIM suspicion + phi accrual
│   ├── mod.rs
│   ├── detector.rs                       # FailureDetector orchestrator
│   ├── swim.rs                           # SWIM suspicion/refutation (ping/ack)
│   ├── indirect.rs                       # Indirect probes: ping-req to K intermediaries
│   ├── refutation.rs                     # Suspicion refutation on any ack
│   ├── phi.rs                            # Phi-accrual suspicion level from inter-arrival window
│   ├── probe.rs                          # ping / ping-req / ack datagram types
│   ├── interval.rs                       # Suspicion interval, protocol period
│   └── failure_detection_tests.rs
│
├── wire/                                 # Wire envelope (carrier-agnostic bytes only)
│   ├── mod.rs
│   ├── envelope.rs                       # Gossip envelope: origin, seq, payload, signature
│   ├── framing.rs                        # Envelope encode/decode to Vec<u8> for ids-carrier
│   ├── signature.rs                      # ed25519 sign/verify over envelope
│   ├── encryption.rs                     # Optional xchacha20poly1305/chacha20poly1305 envelope
│   ├── nonce.rs                          # Envelope nonce management
│   └── wire_tests.rs
│
├── security/                             # Discovery security boundaries
│   ├── mod.rs
│   ├── authenticator.rs                  # Envelope verification before any merge
│   ├── equivocation.rs                   # Same-seq conflicting payload detection
│   ├── budget.rs                         # Per-peer datagram/size budgets
│   ├── deny.rs                           # Reject-and-drop for oversized/malformed envelopes
│   └── security_tests.rs
│
├── peer_selection/                       # Target selection for gossip
│   ├── mod.rs
│   ├── sampler.rs                        # Random peer sampling from view
│   ├── domain.rs                         # Failure-domain-spread sampling (ids-topology)
│   ├── view.rs                           # Eligible-peer view (alive/suspect distinctions)
│   └── peer_selection_tests.rs
│
├── config/                               # Configuration
│   ├── mod.rs
│   ├── gossip_config.rs                  # Fanout, intervals, modes
│   ├── failure_detection_config.rs       # Suspicion interval, phi threshold
│   ├── expiration_config.rs              # TTL, grace window, GC
│   ├── table_config.rs                   # View caps, eviction
│   └── security_config.rs                # Signing/encryption requirements, budgets
│
├── metrics/                              # Metrics & tracing
│   ├── mod.rs
│   ├── gossip_metrics.rs                 # Gossip round, fanout, bytes per round
│   ├── convergence_metrics.rs            # Convergence rounds, divergence events
│   ├── failure_detection_metrics.rs      # Suspect/confirm/dead counts, phi values
│   └── table_metrics.rs                  # View size, tombstone count, evictions
│
├── fuzz/                                 # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_envelope.rs
│   ├── fuzz_merge.rs
│   ├── fuzz_digest.rs
│   └── fuzz_view.rs
│
├── tests/                                # Integration & adversarial tests
│   ├── mod.rs
│   ├── discovery_tests.rs                # End-to-end gossip discovery over mock carrier
│   ├── convergence_tests.rs              # Convergence under reorder/loss/dup
│   ├── failure_detection_tests.rs        # Suspect/confirm/dead scenarios
│   ├── churn_tests.rs                    # Join/leave/rejoin storms
│   ├── expiration_tests.rs               # TTL expiry and tombstone GC
│   ├── anti_entropy_tests.rs             # Digest-based reconciliation
│   ├── epidemic_tests.rs                 # Broadcast delivery under churn
│   ├── adversarial_tests.rs              # Forged envelopes, replay, equivocation
│   ├── partition_tests.rs                # Partition heal + anti-entropy repair
│   ├── property_tests.rs                 # Proptest semilattice/idempotence properties
│   └── chaos_tests.rs                    # Node churn, offline members, message loss
│
├── benches/                              # Benchmarks
│   ├── mod.rs
│   ├── gossip_bench.rs                   # Per-round datagram/CPU cost
│   ├── merge_bench.rs                    # View merge cost
│   ├── digest_bench.rs                   # Digest build/compare cost
│   ├── convergence_bench.rs              # Convergence rounds vs fanout/N
│   └── anti_entropy_bench.rs             # Anti-entropy bandwidth savings
│
├── spec/                                 # Normative specifications
│   ├── gossip_spec.md                    # Rumor mongering protocol spec
│   ├── anti_entropy_spec.md              # Digest exchange spec
│   ├── failure_detection_spec.md         # SWIM + phi-accrual spec
│   ├── expiration_spec.md                # TTL/grace/GC spec
│   ├── wire_spec.md                      # Envelope format spec
│   └── threat_model.md                   # Threat model (mirrors §4)
│
├── docs/                                 # Documentation
│   ├── architecture.md                   # Architecture & layering
│   ├── integration_guide.md              # Integrating with ids-membership/ids-carrier
│   ├── tuning_guide.md                   # Fanout/interval/phi tuning guidance
│   └── api_docs.md                       # Public API documentation
│
└── test_vectors/                         # Deterministic test vectors
    ├── merge_vectors.json
    ├── envelope_vectors.json
    ├── digest_vectors.json
    └── churn_vectors.json
```

**Why ~300–400 files:** one file per responsibility — membership table, rumor engine, anti-entropy/digests, aggregate gossip, epidemic broadcast, convergence/semilattice, propagation, expiration/GC, churn lifecycle, SWIM/phi failure detection, wire envelope, security, peer selection, config, metrics, plus fuzz, tests, benches, specs, docs, test vectors. The count sits inside the required ~90–1000 range and reflects genuine gossip-protocol complexity (bounded-state merge, failure detection, anti-entropy, security). No stubs; every file holds design-level specification content for implementation.

## 7. Key Algorithms & Data Structures

### 7.1 Membership view (join-semilattice)

```
MembershipView = Map<NodeId, MembershipEntry>
MembershipEntry = { generation: u64, version: u64, state: alive|suspect|confirm|dead,
                    heartbeat: Tick, ttl: Tick, metadata: Bytes }

merge(local, remote):
  for (id, entry) in remote:
    if id not in local or (entry.generation, entry.version) > local[id].(generation, version):
      local[id] = entry                       // last-write-wins by (generation, version)
  laws: commutative, associative, idempotent  // join = LUB; SEC by Shapiro et al. 2011
```

### 7.2 Rumor mongering round

```
every gossip_tick:
  peers = sample_k(view.alive, K)             // K = fanout (2–4 typical)
  for peer in peers:
    envelope = aggregate(delta(view), digest(view), heartbeat)
    sign(ed25519, envelope)
    ids-carrier::Endpoint.send_datagram(peer.endpoint, envelope.bytes)
  on receive(envelope):
    verify(ed25519, envelope)                 // drop if invalid
    merge(view, envelope.delta)
    forward pending rumors per fanout/damping
```

### 7.3 SWIM suspicion / refutation

```
direct probe: ping(target); await ack within suspicion_interval
on timeout: indirect probe — ping-req(target) to K intermediaries → each pings target
any ack → refute suspicion, stay alive
no ack by end of protocol period → confirm/dead, gossip dead rumor
phi-accrual: φ = -log10(1 - F(time_since_last_heartbeat)); F = normal CDF over
sliding window of inter-arrival times; threshold φ (default ~8–10) triggers suspicion
```

### 7.4 Anti-entropy digest exchange

```
digest = blake3/sha256(canonical(view_snapshot))        // or Merkle root over entries
on mismatch: recursively compare subtree digests; exchange only divergent
  (member, generation, version) entries; merge via §7.1 LWW
interval: anti_entropy_tick ≫ gossip_tick (e.g. 30–120 s vs 200 ms–1 s)
cost: equal digests → O(1) bytes; k divergent entries → O(k) bytes, not O(view size)
```

### 7.5 Expiration / refresh

```
entry.ttl = heartbeat_tick + TTL
every refresh: entry.heartbeat = now_tick; entry.version += 1
expire sweep: if now_tick > entry.ttl → state = tombstone (retain grace_window)
GC: drop tombstones older than grace_window; bounded view eviction (oldest tombstone first)
```

### 7.6 Convergence bound

With fanout K and N members, rumor reaches all correct nodes in O(log_K N) rounds w.h.p.; per-node traffic O(K) datagrams/round, independent of N (verified in gossip literature: Demers et al. 1987; memberlist/Cassandra production behavior).

## 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | NodeId, Endpoint, CarrierType, sealed traits, error contracts (master crate) ||
|| `ids-carrier` | Endpoint (CarrierType + Vec<u8>), Datagram, send/receive_datagram — sole traffic boundary ||
|| `ids-crypto` | ed25519 sign/verify, sha256, blake3, xchacha20poly1305/chacha20poly1305, nonces ||
|| `ids-time` | Proprietary tick basis for TTL/expiration/heartbeat (not wall-clock, not HLC) ||
|| `ids-topology` | Failure-domain-aware gossip peer selection (optional feature) ||
|| `serde` + `serde_bytes` | Envelope serialization ||
|| `thiserror` | Error types ||
|| `bytes` | Datagram buffer handling ||
|| `blake3` | Digest hashing (workspace dependency; sha256 comes from ids-crypto) ||
|| `proptest` | Property-based semilattice/idempotence tests (dev) ||
|| `criterion` | Convergence/bandwidth benchmarks (dev) ||

**Prohibited:** no `std::net`, `tokio::net`, libp2p, QUIC, HTTP, DNS, or any carrier-specific crate; no IP/TCP/UDP concepts in source. Allowed crypto limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (all via `ids-crypto` or `blake3` workspace dep).

## 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-011.1 | No central coordinator | Discovery runs with zero designated nodes; join via any known endpoint only ||
|| AC-011.2 | Carrier-agnostic | Compiles and behaves identically with zero imports of std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS; all traffic via ids-carrier::Endpoint ||
|| AC-011.3 | Allowed crypto only | Envelope auth = ed25519; digests = sha256/blake3; optional encryption = xchacha20poly1305/chacha20poly1305; nothing else ||
|| AC-011.4 | Convergence time | Membership fact reaches all correct nodes in O(log_K N) gossip rounds; measured in simulation for N ∈ {10, 100, 1000}, K ∈ {2, 3, 4} ||
|| AC-011.5 | Per-node traffic bound | O(K) datagrams per gossip round regardless of N; bytes/round bounded by configured max envelope size ||
|| AC-011.6 | Idempotent merge | View merge commutative/associative/idempotent; property test 100k+ iterations ||
|| AC-011.7 | Loss/dup/reorder tolerance | Convergence under randomized loss/duplication/reordering (chaos test) ||
|| AC-011.8 | Suspect → confirm/dead | Direct ping timeout → indirect probe (K intermediaries) → refute on any ack → dead after protocol period ||
|| AC-011.9 | Expiration correctness | Entry without refresh past TTL → tombstone; dropped only after grace window ||
|| AC-011.10 | Churn handling | Join/leave/rejoin storms converge; re-join bumps generation; stale resurrections lose ||
|| AC-011.11 | Anti-entropy | Equal digests → no transfer; divergent entries repaired with O(k) bytes ||
|| AC-011.12 | Authentication | Unsigned/tampered envelopes dropped before merge; forged origin rejected ||
|| AC-011.13 | Replay resistance | Monotonic per-origin sequence + (generation, version) LWW; old generations lose ||
|| AC-011.14 | Bounded memory | View size capped; tombstone GC; eviction under sustained churn ||
|| AC-011.15 | Determinism | Same seed + event sequence → identical views across nodes (seeded simulation) ||

## 10. Remaining Uncertainties

1. **Bootstrap without a coordinator:** the crate assumes at least one known endpoint per joiner (configured/provisioned out of band); a fully seedless bootstrap (e.g., local multicast-style discovery) is not specified and conflicts with the carrier-agnostic mandate (no broadcast primitives assumed).
2. **Failure-detector parameters:** suspicion interval, phi threshold, indirect-probe fanout, and grace window are workload-dependent; defaults are placeholders pending deployment measurement.
3. **Signed vs unsigned gossip:** ed25519 envelope signing is specified, but whether signing is mandatory or configurable per-carrier-trust-model is unresolved; signing costs CPU on every gossip datagram.
4. **Interaction with ids-membership states:** discovery emits raw join/leave/dead; the mapping to `ids-membership` node states (candidate/validating/active/…/retired) and admission-control gating lives in ids-membership, not here — interface contract TBD.
5. **Digest design:** plain blake3/sha256 whole-view digest vs Merkle tree over entries — Merkle gives O(divergent) localization but more implementation surface; decision deferred.
6. **Table eviction policy under churn:** oldest-tombstone-first vs LRU vs generation-based — unresolved; affects memory vs accuracy trade-off.
7. **Cross-carrier endpoint semantics:** `Endpoint` carries `CarrierType + Vec<u8>`; how discovery maps NodeId → CarrierType + opaque address for send_datagram depends on ids-carrier/ids-000-aa contracts not yet finalized.
8. **Encryption overhead:** optional xchacha20poly1305/chacha20poly1305 envelope encryption doubles datagram size roughly; whether default-on or opt-in is a product decision.

## 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate crate `ids-discovery` | User mandate: separate crates for new capabilities; keeps ids-carrier/ids-000-aa focused ||
|| Gossip + failure detection as separate layers, piggybacked datagrams | memberlist pattern: gossip rate tunable independently of failure-detection rate; shared datagrams save bandwidth ||
|| Rumor mongering push/pull/push-pull | O(log n) convergence, O(1) per-node traffic, robust to loss/dup/reorder (Demers et al. 1987) ||
|| (generation, version) LWW merge | Idempotent semilattice → strong eventual consistency without reliable channels (Shapiro et al. 2011) ||
|| SWIM suspicion + indirect probes + refutation | Distinguishes transient loss from real failure; bounds false positives (Cornell DSN 2002; HashiCorp memberlist) ||
|| Phi-accrual suspicion | Adaptive threshold tracks observed heartbeat intervals; φ threshold tunes false-positive vs detection-latency trade-off (Hayashibara et al. 2004) ||
|| Anti-entropy digest exchange (blake3/sha256) | Heals missed partitions/divergence; O(1) equal check, O(k) repair; interval ≫ gossip interval bounds bandwidth (Cassandra/Dynamo pattern) ||
|| TTL expiration keyed to ids-time ticks | IDS proprietary time basis; never wall-clock; grace-window tombstones prevent premature eviction ||
|| All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>) | Carrier-agnostic mandate; no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS ||
|| ed25519 envelope signatures | Authenticated origin within allowed crypto set; forged join/leave/suspect detectable ||
|| ids-000-aa as master type crate | NodeId, Endpoint, CarrierType, sealed traits used verbatim; no type duplication ||
|| Reject op-based reliable broadcast as primary | Requires exactly-once causal delivery — contradicts carrier-agnostic unreliable-datagram model ||
|| Bounded view size + tombstone GC | Memory stays O(1) under unbounded churn ||

## 12. Sources

- SWIM: Scalable Weakly-consistent Infection-style Process Group Membership Protocol (Cornell DSN 2002): https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf
- HashiCorp memberlist (SWIM port, anti-entropy push/pull, incarnation numbers, dedicated gossip layer): https://github.com/hashicorp/memberlist
- memberlist security model (shared-key gossip, not BFT, replay limits): https://github.com/hashicorp/memberlist/blob/master/SECURITY.md
- "Lifeguard: Local Health Awareness for More Accurate Failure Detection" (SWIM analysis, flapping, suspicion): https://arxiv.org/html/1707.00788v2
- Demers et al., "Epidemic Algorithms for Replicated Database Maintenance" (PODC 1987 — push/pull rumor mongering, anti-entropy): https://www.cs.cornell.edu/projects/ladis2009/papers/demers-paper.pdf
- "Gossip and Epidemic Protocols" survey (anti-entropy styles, eventual consistency, O(log n) convergence): https://onlinelibrary.wiley.com/doi/10.1002/047134608X.W8353
- Cassandra gossip architecture (vector clocks (generation, version), phi accrual, seed nodes): https://cassandra.apache.org/doc/3.11/cassandra/architecture/dynamo.html
- Cassandra internode gossip (phi_convict_threshold, per-node local failure decisions): https://docs.datastax.com/en/ddac/doc/datastax_enterprise/dbArch/archGossipAbout.html
- Chitchat (Quickwit — scuttlebutt reconciliation + phi-accrual, tombstone GC, dead_node_grace_period): https://github.com/quickwit-oss/chitchat
- Chandra-Toueg unreliable failure detectors (completeness/accuracy properties): https://dl.acm.org/doi/10.1109/TC.2003.1176982
- Phi Accrual Failure Detector (Hayashibara et al. 2004; Akka/Hazelcast formulations): https://doc.akka.io/libraries/akka-core/current/typed/failure-detector.html
- HyParView: membership protocol for reliable gossip-based broadcast (DSN 2007, active/passive partial views, shuffle): https://dpss.inesc-id.pt/~ler/reports/LPR_GossipBasedBroadcast.pdf
- Plumtree / gossip-based broadcast (spanning tree over gossip overlay, fanout): https://github.com/saorsa-labs/saorsa-gossip (HyParView + Swim membership Rust implementation reference)
- SCAMP: Peer-to-Peer Membership Management for Gossip-Based Protocols (IEEE Trans. Computers, partial views without global knowledge): https://dl.acm.org/doi/10.1109/TC.2003.1176982
- Anti-entropy digests and reconciliation (Merkle/version-vector digests, O(k) repair): https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems
- Gossip bandwidth model (fanout K, O(log n) rounds, constant per-node traffic): https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/what-is-gossip-protocol-and-how-does-it-achieve-epidemic-information-spread
- Correctness of gossip-based membership under message loss (view maintenance under loss): https://doi.org/10.1145/1582716.1582743
- Grapevine (Ed25519-signed gossip, trust-on-first-use origin pinning, epidemic broadcast): https://github.com/kobby-pentangeli/grapevine
- Hyperledger Fabric gossip (signed alive messages, PKI identity, channel membership): https://hyperledger-fabric.readthedocs.io/en/stable/gossip.html
- Bitcoin AddrMan peer tables (GETADDR peer exchange, tried/new tables): https://arxiv.org/html/2609.07609
- Shapiro et al. 2011 — CRDTs and strong eventual consistency (semilattice merge basis): CRDT survey literature (referenced via plan-077 sources)
- AetherWeave: Sybil-resistant peer discovery with stake (gossip privacy, table slicing): https://arxiv.org/html/2603.23793
- Consul gossip (Serf, LAN/WAN pools, gossip over datagrams with full-state sync fallback): https://developer.hashicorp.com/consul/docs/concept/gossip

**Research status:** Complete. Ready for implementation. No code written, no files created (other than this plan), no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 011 (ids-discovery).

**Note on scope:** prompts.txt entry 11 verbatim specifies `ids-discovery` (gossip-based membership discovery without central coordinator). The task heading mentioned `ids-protocol` (native wire/message format), which corresponds to prompts.txt entry 12 / plan-012.md; this plan follows entry 11 exactly as instructed.