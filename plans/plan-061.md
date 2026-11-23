## Phase 061 — Create the ids-routing-table crate: Routing table construction and maintenance.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 910-924)

```
61. Research only do not read plan.md **[Phase 061 — Create the ids-routing-table crate: Routing table construction and maintenance.]**

Crate: ids-routing-table
Purpose: Routing table construction and maintenance.
Contents: Routing table entries, next-hop resolution, path selection, locality metrics, replica freshness tracking, loop prevention, route expiration/revalidation, routing convergence.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 061 — Create the ids-routing-table crate: Routing table construction and maintenance.]**

Create plan-061.md in C:/idfs/plans/ for the ids-routing-table crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 061 creates `ids-routing-table`, the IDS crate responsible for routing table construction and maintenance. It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by routing concern: route entries, next-hop resolution, path selection, locality metrics, replica freshness tracking, loop prevention, route expiration/revalidation, and routing convergence. `ids-routing-table` does not open sockets or drive any transport directly; it constructs, indexes, and maintains routing tables as pure data structures and policy functions, and returns every forwarding decision as opaque `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes). The crate never touches `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh.

Routing table contents (per prompts.txt entry 61):

| Concern | Responsibility |
|---|---|
| Routing table entries | Destination → next-hop/Endpoint entries with cost, metric, source, age, and state |
| Next-hop resolution | Given a destination, resolve the next-hop Endpoint and outgoing path |
| Path selection | Choose among candidate next-hops using cost, locality, freshness, ECMP groups |
| Locality metrics | Prefer local/region/rack replicas over remote (proximity-aware selection) |
| Replica freshness tracking | Track last-known-good/version per replica; prefer fresher replicas |
| Loop prevention | Split horizon / poison reverse / path-vector loop detection on table construction |
| Route expiration/revalidation | TTL per entry; stale entries revalidated or garbage-collected |
| Routing convergence | Detect topology change, recompute affected paths, converge to loop-free state |

A skeleton `ids-routing-table` crate does NOT yet exist at `C:/idfs/ids-routing-table/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `RecordId`, `Checksum`, `IdsTime`, `OpState`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned distributed construction of near-optimal routing schemes (Elkin & Neiman 2016, CONGEST-model routing tables with stretch guarantees), Kademlia DHT routing tables with XOR metric and k-buckets (libp2p docs), Chord DHT finger tables and successor lists (MIT TON Chord paper), distance-vector distributed Bellman-Ford (Stanford CS168 textbook) with route advertisement, next-hop + cost forwarding entries, split horizon and poison reverse loop prevention (CS168, INET RIP tutorial, RFC 2453 RIPv2), route expiration timers and garbage collection (INET RIP timeout/garbage-collection), routing convergence vs count-to-infinity (link state converges fast; distance vector suffers bad-news-travels-slowly), locality-aware routing for large-scale networks (Berkeley OceanStore locality paper), replica freshness / view divergence of freshness in replicated databases (SSRN freshness survey; Skybridge bounded staleness USENIX OSDI'25), BGP best-path selection algorithm (Cisco: weight, local preference, local origin, AS path, origin, MED, eBGP over iBGP, IGP metric, router ID), path-vector protocol loop detection via AS path (Wikipedia path vector; APNIC BGP basics; UCSD interdomain routing lecture), longest prefix match (DPDK LPM6 library; multi-bit trie), ECMP equal-cost multi-path load sharing (Palo Alto, Cumulus, Cisco, Huawei), BGP route flap damping (Juniper), loop-free alternate fast reroute (Cisco ASR 9000 LFA FRR), consistent hashing and rendezvous/highest random weight hashing (Ably blog; Wikipedia rendezvous hashing), on-demand MANET routing AODV/DSR route discovery and maintenance (INET MANET showcase), anycast closest-replica selection (OASIS anycast paper), reverse path forwarding loop prevention (Cisco community), gossip/sloppy management of structured P2P routing tables (HOTAC'08 sloppy management paper), distance vector vs link state vs path vector comparison (Princeton COS461; Auckland CS314), link-state SPF/Dijkstra routing table generation (Springer OSPF chapter; Pearson Routing Protocols Companion Guide; Nokia OSPF guide), RFC 9815 BGP-LS SPF routing (RIB/LSDB/SPF calculation), BGP route summarization and hierarchical aggregation (flylib summarizing outbound routing), routing table partitioning/sharding for scalable lookups (IEEE routing table partitioning SPAL; Tarantool vshard routing table bucket→replica set mapping), dead route detection and unreachable route garbage collection, carrier-agnostic opaque datagram addressing (IDS carrier-agnostic socket abstraction research), Rust Kademlia routing table implementation (mainline routing_table.rs k-buckets, MAX_BUCKET_SIZE_K=20), segment routing / source routing (RFC 8402 segment routing architecture; RFC 9256 SR policy).

---

### 2. Architecture Overview

`ids-routing-table` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a table-construction-and-maintenance layer: given a set of known nodes/endpoints, link metrics, locality hints, and replica versions, it builds routing tables, resolves next-hops, selects paths, tracks freshness, prevents loops, expires stale entries, and converges after topology changes. It never sends bytes itself; it returns `RouteDecision` / `NextHop` / `Path` structures that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` targets.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (routing table users)            │
│  ids-store, ids-replication, ids-query — route lookups via table queries      │
├──────────────────────────────────────────────────────────────────────────────┤
│               Phase 061: ids-routing-table (THIS CRATE — table maintenance)   │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────┐ │
│  │ route entries│ │ next-hop     │ │ path selection│ │ locality     │ │loop│ │
│  │ (dst→next-hop│ │ resolution   │ │ (cost/freshness│ │ metrics      │ │prev│ │
│  │  + cost/TTL) │ │              │ │  /ECMP)      │ │ (proximity)  │ │ent │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────┐ │
│  │ freshness    │ │ expiration   │ │ revalidation │ │ convergence  │ │    │ │
│  │ tracking     │ │ (TTL/GC)     │ │ (stale→valid)│ │ (topology Δ) │ │    │ │
│  │ (version/age)│ │              │ │              │ │              │ │    │ │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └────┘ │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                          │
│  │ contracts    │ │ invariants   │ │ decisions    │                          │
│  │ (routing)    │ │ (~12 rules)  │ │ (RouteDecision│                          │
│  │              │ │              │ │  types)      │                          │
│  └──────────────┘ └──────────────┘ └──────────────┘                          │
└──────────────────────────────────────────────────────────────────────────────┘
                                          │ opaque Vec<u8> datagrams
                                          ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                     │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])│
│  receive_datagram → (Endpoint, Vec<u8>)                                       │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS      │
│  / Wi-Fi / Bluetooth / cellular / VPN / mesh                                  │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a route lookup:** caller submits `RouteLookup{destination: RecordId, hints: LocalityHints}` → `ids-routing-table` resolves the table entry, applies path selection (cost, locality, freshness, ECMP), runs loop-prevention checks, returns `RouteDecision{next_hop: Endpoint, path: Vec<NodeId>, cost, freshness_ok, ecmp_group}` → caller sends the datagram to `ids-carrier::Endpoint` via `ids-carrier::Carrier::send_datagram`.

**Data flow of table maintenance:** topology change (node join/leave, link metric change, replica version bump) → `TableUpdate{kind: Add|Remove|Metric|Freshness|Expire}` → affected entries invalidated → path selection re-run for affected destinations → convergence check (loop-free, all known destinations reachable or marked unreachable) → `Converged` event emitted; stale entries past TTL garbage-collected.

**Crypto used:** sha256/blake3 for route-entry fingerprinting, table-version hashing, and loop-detection path hashing; ed25519 for signing route advertisements and table-version checkpoints; xchacha20poly1305/chacha20poly1305 to encrypt routing state at rest when confidentiality is required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Routing table entries: destination → next-hop + cost + TTL + state

Each entry maps a destination identifier (a `RecordId`/`NodeId`-scoped route key from `ids-000-aa`) to a next-hop `Endpoint`, a cost/metric, the originating node, a TTL, and an entry state (`Fresh`, `Stale`, `Expired`, `Unreachable`, `Poisoned`). Entry format follows the distance-vector forwarding-table model from Stanford CS168 (destination → (next hop, cost)) extended with TTL and freshness fields. Entries are immutable once published; updates create a new table version (monotonically increasing `TableVersion` u64). Source: CS168 distance-vector textbook; INET RIP tutorial.

#### 3.2 Next-hop resolution

Given a destination, `resolve_next_hop(destination)` returns the best `Endpoint` per current table version, or `Unreachable` if no entry exists or all candidates are expired/poisoned. Resolution applies longest-prefix-style matching when destination keys support prefix decomposition (trie-based lookup, per DPDK LPM6 multi-bit trie concepts adapted to opaque byte-key prefixes), falling back to exact-key lookup. Resolution never contacts the network; it is a pure function over the local table. Source: DPDK LPM6 library; CS168 forwarding table.

#### 3.3 Path selection

When multiple candidate next-hops exist for a destination, `select_path` ranks them by: (a) lowest cost/metric, (b) highest locality score (local > same region > same rack > remote), (c) freshest replica version, (d) ECMP tie-break via stable hash of (destination, flow) so all packets of a flow use the same next-hop. The result may be a single next-hop or an ECMP group of equal-cost next-hops. BGP best-path selection (Cisco attribute order: weight → local preference → local origin → AS path → origin → MED → eBGP over iBGP → IGP metric → router ID) is the reference model for multi-attribute path ranking, adapted to IDS attributes (cost, locality, freshness, admin weight). Source: Cisco BGP best-path algorithm; Palo Alto/Cumulus ECMP load-sharing; DPDK LPM6.

#### 3.4 Locality metrics

Locality is an ordinal score computed from `ids-000-aa` canonical types: `RegionId`, node rack/failure-domain hints, and endpoint `CarrierType`. Local (same node/rack) scores highest; same-region next; cross-region lowest. Locality biases path selection toward nearby replicas to limit wide-area traffic (per Berkeley OceanStore locality-awareness: exploit local resources over remote whenever possible, limiting the impact of local operations on WAN performance). Locality scores are recomputed when topology or carrier capabilities change. Source: OceanStore locality-aware mechanisms paper.

#### 3.5 Replica freshness tracking

Each routing table entry can carry a replica freshness marker: `(version: u64, last_seen: IdsTime, source: NodeId)`. Reads prefer the freshest known replica; writes route to replicas that have acknowledged recent versions. Freshness degrades with time since last update and with version lag behind the known head. This mirrors bounded-staleness freshness tracking (Meta TAO/Skybridge: staleness checking vs write HLC; view divergence of freshness in replicated databases). Staleness thresholds are configurable per route class; exceeding them triggers revalidation or route downgrade. Source: Skybridge USENIX OSDI'25; SSRN freshness survey.

#### 3.6 Loop prevention

Loop prevention combines three mechanisms, mirroring distance-vector and path-vector practice:

- **Split horizon:** never advertise a route back to the neighbor from which it was learned (CS168 Rule 6A; RFC 2453 split horizon).
- **Poison reverse:** advertise routes learned from a neighbor back to that neighbor marked unreachable/infinite cost (CS168 Rule 6B; RFC 2453 split horizon with poisoned reverse).
- **Path-vector loop detection:** each route advertisement carries the path of `NodeId`s traversed; a node discards any advertisement containing its own identifier (BGP AS-path loop detection; Wikipedia path vector; UCSD path-vector lecture).

At table-construction time, `ids-routing-table` rejects any entry whose next-hop path contains a cycle (path includes the local node or repeats a node). Source: CS168 distance-vector; RFC 2453; Wikipedia path-vector protocol.

#### 3.7 Route expiration / revalidation / garbage collection

Every entry carries a TTL. When no refresh advertisement arrives before TTL expiry, the entry transitions `Fresh → Stale → Expired`. Expired entries are garbage-collected after a hold-down period (RIP garbage-collection timer model: timeout then GC timer; INET RIP timers tutorial). Revalidation: a stale entry may be probed (one datagram to the recorded next-hop requesting a freshness echo) before deletion; if no response within the bounded wait (keyed to `ids-time`, never wall-clock), the entry is removed and dependents notified. Source: INET RIP timeout/garbage collection; CS168 distance-vector (entry expiry → poison advertisement).

#### 3.8 Routing convergence

After a topology change (node/endpoint add/remove, link metric change, freshness invalidation), affected entries are invalidated and path selection re-runs. Convergence is reached when: all known destinations have a route or are marked `Unreachable`, no entry's next-hop path contains a loop, and no `Stale` entries remain pending revalidation. Link-state (SPF/Dijkstra) converges faster than distance-vector (count-to-infinity: bad news travels slowly); `ids-routing-table` prefers link-state-style full-table recompute on topology change for small/medium tables and incremental distance-vector-style updates for large tables. Source: CS168 link state vs distance vector; Springer OSPF SPF chapter; CS144 routing lecture; Princeton COS461.

#### 3.9 DHT-style routing tables (k-buckets / finger tables)

For key-space routing (routing by hashed `RecordId`/`NodeId` rather than explicit topology), the crate supports a DHT mode: Kademlia-style k-buckets indexed by XOR distance buckets (libp2p DHT; XOR distance metric, identity/symmetry/triangle inequality properties; k-buckets with replacement caches) and Chord-style finger tables (successor(n + 2^(i-1)) entries, O(log N) nodes per table, O(log N) lookup hops). DHT mode is optional and carrier-agnostic: node identifiers are `NodeId` (32-byte), distances computed over byte arrays, all contact via `ids-carrier::Endpoint`. Source: libp2p DHT docs; MIT TON Chord paper; Chord SIGCOMM'01 paper.

#### 3.10 On-demand route discovery and maintenance

For sparse or dynamic topologies where proactive table construction is wasteful, on-demand (reactive) routing discovers a path only when needed: route request (RREQ) flood bounded by hop count/TTL, route reply (RREP) unicast back along the reverse path, route error (RERR) on link break. This is the AODV/DSR reactive model (INET MANET showcase). Route maintenance: local repair via loop-free alternates (LFA fast reroute: pre-compute backup next-hops satisfying the loop-free alternate condition) before initiating full re-discovery. Source: INET MANET routing protocols; Cisco ASR 9000 LFA FRR.

#### 3.11 Anycast / closest-replica selection

Anycast routing selects the "closest" replica among a set of candidates advertising the same destination. Closest is defined by the locality metric (3.4) plus cost; selection can be coarse (bucket client→location, return closest replica, per OASIS anycast) or fine (per-flow hash to an ECMP group of replicas). All traffic still via `ids-carrier::Endpoint`; anycast is a logical selection, not an IP anycast. Source: OASIS anycast paper; ClickHouse replica-aware routing (sticky routing to same replica for read-after-write consistency).

#### 3.12 Gossip/sloppy table maintenance

For very large or churn-prone deployments, proactive repair of every table entry is cumbersome and can cause liveness bugs. Sloppy management (HOTAC'08) uses continuous lazy background algorithms for routing-table maintenance while relying on the deterministic routing algorithm for actual message forwarding: periodic gossip anti-entropy exchanges table digests (version vectors or Merkle hashes) and reconciles only divergent entries. This bounds maintenance traffic and tolerates temporary inconsistencies. Source: HOTAC'08 sloppy management of structured P2P services; anti-entropy digest exchange.

---

### 4. Threat Model & Mitigations

| Threat | Asset at risk | Mitigation (this crate) |
|---|---|---|
| Routing loop (A→B→C→A) | Packet delivery / liveness | Split horizon + poison reverse + path-vector cycle rejection at table construction |
| Stale route after topology change | Delivery correctness | TTL expiration → Stale → Expired → GC; revalidation probe before deletion |
| Count-to-infinity / slow convergence | Convergence time | Link-state full recompute on topology change; triggered updates; hold-down timers |
| Black-hole route (advertise unreachable destination) | Delivery correctness | Path-vector loop detection; freshness gating; signed route advertisements (ed25519) |
| Replay of old route advertisement | Table integrity | Monotonic `TableVersion`; nonce/bounded replay cache per advertisement; sha256/blake3 entry fingerprinting |
| Route flap (repeated add/remove) | Table stability | BGP-style flap damping: suppress repeatedly-flapping routes (penalty decay, reuse threshold) |
| Locality downgrade (force remote path) | Performance / latency | Locality scores derived from canonical `ids-000-aa` types, not carrier-supplied claims; signed locality assertions |
| Freshness spoofing (fake recent version) | Read correctness | ed25519-signed freshness markers; version-vector comparison; reject markers below known head |
| Carrier duplicating/delaying datagrams | Ordering/duplicates | Idempotent table updates; duplicate suppression by `TableVersion`; carrier-agnostic boundary |
| Prohibited networking crate | Architecture compliance | Dependency audit: no `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS; `Endpoint` sole address type |
| Disallowed crypto | Crypto policy | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 via `ids-crypto` primitive IDs |

The carrier is never trusted: routing decisions and advertisements are signed/hashed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

| Crate | Role | Relevance to ids-routing-table |
|---|---|---|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `RecordId`, `Checksum`, `IdsTime`, `OpState`, `Error` consumed, never redefined; sealed `Routing` contract mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All routing traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing route advertisements/table checkpoints; hashing entries and paths; encrypting routing state at rest |
| `ids-time` | Monotonic `IdsTime` source for TTL, revalidation timeouts, convergence deadlines | All timing keyed to `IdsTime` ticks, never wall-clock |
| `ids-store` | Primary record store (routing table user) | Routes reads/writes to replicas via table lookups; consumes `RouteDecision` |
| `ids-replication` | Replica synchronization / anti-entropy | Consumes next-hop/ECMP group for gossip and hinted replay transport |
| `ids-query` | Query planning (routing table user) | Resolves query target nodes via path selection and locality metrics |
| `ids-fragment` | Record fragments/shards | Fragment reads/writes routed by the parent record's table entry |
| `ids-membership` (future) | Node/region membership changes | Membership events (join/leave) feed `TableUpdate` into the routing table |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Profile tag influences path selection (e.g., cross-region durable prefers cross-region quorum next-hops) |

Integration points: `ids-routing-table` drives public contracts declared in `ids-000-aa`; it does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on `ids-discovery`/`ids-session`/`ids-cli` beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-routing-table/` (a separate crate, not a folder inside an existing crate):

```
ids-routing-table/
├── Cargo.toml                          # serde, thiserror, blake3, ids-000-aa, ids-carrier, ids-crypto, ids-time
├── lib.rs                              # crate root, public API surface, feature gates
├── constants.rs                        # Table defaults: TTL, hold-down, k-bucket size K, ECMP max paths
├── error.rs                            # RouteError (thiserror): lookup, resolution, selection, expiration, loop
├── types.rs                            # RouteEntry, RouteDecision, NextHop, Path, TableVersion, LocalityScore (re-exports ids-000-aa types)
├── table.rs                            # RoutingTable core: insert/remove/lookup/versioned table
├── table_version.rs                    # Monotonic TableVersion, entry fingerprinting (sha256/blake3)
├── entry.rs                            # RouteEntry: dst→next-hop+cost+TTL+state+freshness
├── next_hop.rs                         # Next-hop resolution: resolve_next_hop(destination) → Endpoint | Unreachable
├── path_selection.rs                   # Path/cost ranking, locality, freshness, ECMP group selection
├── locality.rs                         # Locality metrics from RegionId/failure-domain/CarrierType
├── freshness.rs                        # Replica freshness tracking: (version, last_seen, source) per entry
├── loop_prevention.rs                  # Split horizon, poison reverse, path-vector cycle rejection
├── expiration.rs                       # TTL expiry: Fresh→Stale→Expired, revalidation probe, garbage collection
├── convergence.rs                      # Convergence detection: loop-free, reachable, no stale pending
├── dht/                                # Optional DHT mode: k-buckets + finger tables
│   ├── mod.rs
│   ├── k_buckets.rs                    # Kademlia k-buckets by XOR distance, replacement caches
│   ├── finger_table.rs                 # Chord finger table successor(n + 2^(i-1))
│   ├── xor_distance.rs                 # XOR distance over NodeId byte arrays
│   └── dht_lookup.rs                   # O(log N) lookup via k-buckets/finger table
├── on_demand/                          # Optional reactive route discovery
│   ├── mod.rs
│   ├── route_request.rs                # RREQ flood bounded by hop count/TTL
│   ├── route_reply.rs                  # RREP unicast along reverse path
│   ├── route_error.rs                  # RERR on link break
│   └── lfa.rs                          # Loop-free alternate fast reroute backup next-hops
├── anycast/                            # Closest-replica selection
│   ├── mod.rs
│   ├── replica_set.rs                  # Candidate replica set for a destination
│   └── closest.rs                      # Closest replica by locality + cost
├── gossip/                             # Sloppy table maintenance via digest exchange
│   ├── mod.rs
│   ├── digest.rs                       # Table digest (version vector / Merkle hash)
│   └── anti_entropy.rs                 # Periodic gossip reconciliation of divergent entries
├── flap_damping.rs                     # BGP-style route flap damping (penalty decay, suppress/reuse)
├── contracts/                          # Sealed trait contracts (~8 files)
│   ├── mod.rs
│   ├── routing_contract.rs             # trait Routing (sealed): lookup/resolve/select
│   ├── table_contract.rs               # trait TableMaintenance
│   ├── path_contract.rs                # trait PathSelection
│   ├── locality_contract.rs            # trait LocalityMetrics
│   ├── freshness_contract.rs           # trait ReplicaFreshness
│   ├── loop_contract.rs                # trait LoopPrevention
│   ├── expiration_contract.rs          # trait ExpirationGC
│   └── convergence_contract.rs         # trait RoutingConvergence
├── invariants/                         # Compile-time/runtime invariants (~10 files)
│   ├── mod.rs
│   ├── no_routing_loops.rs
│   ├── ttl_monotonic.rs
│   ├── locality_score_bounded.rs
│   ├── freshness_monotonic.rs
│   ├── carrier_agnostic.rs
│   ├── crypto_allowlist.rs
│   ├── endpoint_address_capped.rs
│   ├── table_version_monotonic.rs
│   ├── ecmp_group_sized.rs
│   └── convergence_sound.rs
├── decisions/                          # Decision types (~6 files)
│   ├── mod.rs
│   ├── route_decision.rs               # RouteDecision: next_hop/path/cost/freshness_ok/ecmp
│   ├── next_hop_decision.rs
│   ├── path_decision.rs
│   ├── ecmp_group.rs                   # ECMP group of equal-cost next-hops + flow hash
│   ├── unreachable.rs                  # Unreachable destination marker
│   └── table_update.rs                 # TableUpdate: Add|Remove|Metric|Freshness|Expire
├── audit/                              # Signed, hash-chained route audit log (~6 files)
│   ├── mod.rs
│   ├── audit_entry.rs
│   ├── audit_chain.rs
│   ├── checkpoint.rs                   # ed25519-signed table-version checkpoints
│   ├── signer.rs
│   └── verifier.rs
├── tests/                              # Property/verification tests (~36 files)
│   ├── mod.rs
│   ├── entry_tests.rs
│   ├── table_tests.rs
│   ├── next_hop_tests.rs
│   ├── path_selection_tests.rs
│   ├── locality_tests.rs
│   ├── freshness_tests.rs
│   ├── loop_prevention_tests.rs
│   ├── expiration_tests.rs
│   ├── convergence_tests.rs
│   ├── dht_tests.rs                    # k-buckets/finger-table lookup correctness
│   ├── on_demand_tests.rs              # RREQ/RREP/RERR determinism
│   ├── anycast_tests.rs
│   ├── gossip_tests.rs
│   ├── flap_damping_tests.rs
│   ├── audit_tests.rs
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/IP/TCP/UDP/QUIC/HTTP/DNS
│   ├── crypto_allowlist_tests.rs
│   ├── dependency_audit_test.rs        # cargo tree: no prohibited crates
│   └── property_tests.rs               # proptest properties for table updates
├── benches/                            # (~10 files)
│   ├── mod.rs
│   ├── lookup.rs                       # Route lookup latency
│   ├── table_construction.rs           # Table build/update throughput
│   ├── convergence.rs                  # Convergence time after topology change
│   └── crypto.rs                       # Route signing/hashing benchmarks
├── spec/                               # Normative specifications (~6 files)
│   ├── routing_spec.md
│   ├── loop_prevention_spec.md
│   ├── expiration_spec.md
│   ├── convergence_spec.md
│   ├── carrier_agnostic_spec.md
│   └── crypto_policy.md
├── docs/                               # Documentation (~10 files)
│   ├── architecture.md
│   ├── dht_mode.md
│   ├── on_demand_mode.md
│   ├── adding_a_route_source.md
│   └── runbook.md
├── baselines/                          # Stored baseline results for regression comparison
│   └── lookup_baseline.json
└── test_vectors/                       # Deterministic inputs and expected outputs
    ├── topology_vectors.json
    ├── route_vectors.json
    └── loop_vectors.json
```

Total: ~90–1000 files across `src/`, `src/dht/`, `src/on_demand/`, `src/anycast/`, `src/gossip/`, `src/contracts/`, `src/invariants/`, `src/decisions/`, `src/audit/`, `tests/`, `benches/`, `spec/`, `docs/`, `baselines/`, `test_vectors/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

1. **Routing table** — versioned map `Table { version: TableVersion, entries: BTreeMap<RouteKey, RouteEntry> }`; updates create a new version; entries immutable. RouteKey derived from destination `RecordId`/`NodeId` (opaque bytes, prefix-decomposable for trie lookup).
2. **Route entry** — `RouteEntry { dst, next_hop: Endpoint, path: Vec<NodeId>, cost: u64, locality: LocalityScore, freshness: FreshnessMarker, ttl: TTL, state: EntryState, origin: NodeId }`.
3. **Next-hop resolution** — `resolve_next_hop(dst)`: exact-key lookup, else longest-prefix-match over prefix-decomposable keys (multi-bit trie, per DPDK LPM6); returns `Endpoint` or `Unreachable`; pure function over local table.
4. **Path selection** — rank candidates by (cost asc, locality desc, freshness desc, admin weight desc); ECMP tie-break via stable hash of `(dst, flow_id)` → equal-cost next-hop group; all packets of a flow hash to the same next-hop (per-flow ECMP, per Cumulus/Palo Alto).
5. **Locality score** — ordinal from `RegionId`/rack/failure-domain + `CarrierType`: local > same region > same rack > remote; recomputed on topology/carrier change.
6. **Freshness marker** — `(version: u64, last_seen: IdsTime, source: NodeId)`; reads prefer freshest; staleness threshold configurable per route class; exceeding triggers revalidation.
7. **Loop prevention** — split horizon (don't advertise back to source neighbor), poison reverse (advertise back with infinite/unreachable), path-vector cycle rejection (discard advertisement whose path contains local node or repeats a node).
8. **Expiration / GC** — TTL per entry: `Fresh → Stale → Expired`; revalidation probe (bounded `ids-time` wait) before deletion; expired entries garbage-collected after hold-down (RIP timeout/GC model).
9. **Convergence** — after topology change, invalidate affected entries, re-run path selection; converged when all known destinations routed or `Unreachable`, no loops, no pending `Stale` revalidations.
10. **DHT k-buckets** — Kademlia buckets indexed by XOR distance over `NodeId` bytes; max bucket size K (default 20, per mainline routing_table.rs); replacement caches for failed nodes.
11. **DHT finger table** — Chord finger[i] = successor(node + 2^(i-1)); O(log N) entries, O(log N) lookup hops.
12. **On-demand discovery** — RREQ flood bounded by hop count/TTL, RREP unicast along reverse path, RERR on link break; LFA pre-computed backup next-hops for local repair before full re-discovery.
13. **Anycast selection** — closest replica from candidate set by locality + cost; optional per-flow sticky routing to same replica for read-after-write consistency.
14. **Gossip anti-entropy** — periodic digest exchange (version vector / Merkle hash) of table digests; reconcile only divergent entries (sloppy management).
15. **Route flap damping** — penalty on each flap, exponential decay, suppress when penalty exceeds threshold, reuse when below reuse threshold (BGP flap damping).
16. **Audit chain** — sha256/blake3 hash-chained route-advertisement entries + periodic ed25519-signed table-version checkpoints; offline verifiable.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `Endpoint`, `CarrierType`, `RecordId`, `Checksum`, `IdsTime`, `OpState`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-time` — monotonic `IdsTime` for TTL/revalidation/convergence deadlines.
- `serde` — serialization of route entries, decisions, audit entries.
- `thiserror` — error types.
- `blake3` — content hashing for entry fingerprints and table digests.
- `proptest` — property-based tests.
- `tempfile` — test fixtures.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list. `ids-routing-table` imports no crypto directly — all via `ids-crypto`.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-61.1 | Routing table entries: dst → next-hop + cost + TTL + state, immutable per version | `src/entry.rs`, `src/table.rs`; entry immutability enforced by versioned table |
| AC-61.2 | Next-hop resolution: returns `Endpoint` or `Unreachable`; pure function over local table | Unit tests: exact-key and prefix-match resolution, no network contact |
| AC-61.3 | Path selection: ranks by cost, locality, freshness; ECMP group with stable per-flow hash | Selection tests; ECMP same-flow → same next-hop |
| AC-61.4 | Locality metrics: local > same region > same rack > remote, derived from `ids-000-aa` types | Locality score tests with canonical types |
| AC-61.5 | Replica freshness tracking: `(version, last_seen, source)` per entry; reads prefer freshest | Freshness tests; staleness threshold triggers revalidation |
| AC-61.6 | Loop prevention: split horizon, poison reverse, path-vector cycle rejection | Loop tests: advertisements containing local node rejected; no cycles in table |
| AC-61.7 | Route expiration: `Fresh → Stale → Expired`; revalidation probe before deletion; GC after hold-down | Expiration/GC tests keyed to `IdsTime`, never wall-clock |
| AC-61.8 | Routing convergence: after topology change, all known destinations routed or `Unreachable`, loop-free, no pending stale | Convergence tests after simulated node/leave/metric change |
| AC-61.9 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test + source grep test passes |
| AC-61.10 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Crypto allowlist unit test passes |
| AC-61.11 | Every route advertisement/table checkpoint recorded in append-only hash-chained, ed25519-signed audit log | Audit log tests; offline verification |
| AC-61.12 | Crate compiles cleanly | `cargo check -p ids-routing-table` exits 0 with no warnings |
| AC-61.13 | Conformance tests pass | `cargo test -p ids-routing-table` passes all entry/table/next-hop/path/freshness/loop/expiration/convergence/DHT/on-demand/anycast/gossip/audit tests |

---

### 10. Remaining Uncertainties

- **DHT mode inclusion:** whether k-buckets/finger-table DHT mode is built in Phase 061 or deferred to a later phase; adds complexity but enables key-space routing without explicit topology.
- **On-demand mode inclusion:** whether reactive RREQ/RREP/RERR discovery is built in Phase 061 or deferred; useful for sparse/churn-prone topologies.
- **Concrete TTL / hold-down / flap-damping defaults:** deployment-tuned; to be validated at integration with `ids-store`/`ids-replication`.
- **ECMP max paths and flow-hash function:** max equal-cost paths and hash choice (e.g., blake3-based) affect load distribution; tuning pending.
- **Prefix decomposition of route keys:** whether `RecordId`/`NodeId` keys support longest-prefix-match trie lookup or exact-key only; affects `next_hop.rs` design.
- **Interaction with `ids-membership`:** membership join/leave events feeding `TableUpdate` — exact handshake defined here, validated at integration.
- **Interaction with `ids-consistency-profile` (Phase 060):** profile tag influencing path selection (e.g., cross-region durable prefers cross-region quorum next-hops) — validated at integration.
- **Gossip interval and digest size:** anti-entropy period and Merkle/vector digest tuning affect convergence speed vs bandwidth; workload-dependent.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| Separate crate `ids-routing-table` (not a folder) | Follows "separate crates instead of more folders" directive; clear table-maintenance boundary |
| Versioned immutable table (new `TableVersion` per update) | Enables convergence detection, replay protection, and audit over table history |
| Pure next-hop resolution (no network contact in crate) | Keeps crate a table/policy layer; carrier handles all I/O via `Endpoint` |
| Path selection: cost → locality → freshness → ECMP stable hash | Simple, deterministic, locality-aware; ECMP preserves flow stickiness |
| Loop prevention: split horizon + poison reverse + path-vector cycle rejection | Combines distance-vector (RFC 2453/CS168) and path-vector (BGP) loop defenses |
| Expiration: TTL → Fresh/Stale/Expired → revalidation probe → GC hold-down | RIP timer model adapted to `ids-time` (monotonic, never wall-clock) |
| Convergence: link-state full recompute on topology change (small/medium), incremental DV-style for large | Link state converges faster; avoids count-to-infinity |
| DHT mode optional (k-buckets + finger tables) | Key-space routing without explicit topology; XOR distance over `NodeId` bytes |
| On-demand mode optional (RREQ/RREP/RERR + LFA) | Reactive discovery for sparse/churn-prone topologies; LFA local repair before re-discovery |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable route advertisement log; tamper evidence |
| All traffic via `ids-carrier::Endpoint`; crate returns decisions, never sends bytes | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; via `ids-crypto` primitive IDs from `ids-000-aa` | Complies with allowed-crypto rule |
| All timing keyed to `IdsTime`, never wall-clock | Correct under arbitrary delays; no clock manipulation |

---

### 12. Sources

1. Elkin & Neiman, "On Efficient Distributed Construction of Near-Optimal Routing Schemes," arXiv:1602.02293 / INRIA, 2016 — distributed routing-table construction, CONGEST model, stretch guarantees, table size O(n^(1/k)). https://arxiv.org/html/1602.02293v2
2. libp2p DHT / Kademlia — XOR distance metric, k-buckets, routing table maintenance. https://docs.libp2p.io/concepts/fundamentals/dht
3. Stoica et al., "Chord: A Scalable Peer-to-peer Lookup Protocol," SIGCOMM 2001 / MIT TON — finger table successor(n + 2^(i-1)), O(log N) nodes/lookups, consistent hashing. https://pdos.csail.mit.edu/papers/ton:chord/paper-ton.pdf
4. Stanford CS168 Textbook, Distance-Vector Protocols — distributed Bellman-Ford, forwarding table (dst → next-hop + cost), split horizon, poison reverse. https://textbook.cs168.io/routing/distance-vector.html
5. RFC 2453, RIP Version 2 — split horizon, split horizon with poisoned reverse, triggered updates, route expiry/garbage collection. https://www.rfc-editor.org/info/rfc2453/
6. INET Framework RIP tutorial — timeout/garbage-collection timers, route aging and removal. https://inet.omnetpp.org/inet/docs/tutorials/rip/doc/step4.html
7. Princeton COS461 / Auckland CS314 — distance vector vs link state vs path vector comparison; count-to-infinity; SPF/Dijkstra. https://www.cs.princeton.edu/courses/archive/spr11/cos461/docs/lec14-distvector.pdf
8. Cisco BGP Best-Path Algorithm — weight, local preference, local origin, AS path, origin, MED, eBGP over iBGP, IGP metric, router ID. https://www.cisco.com/c/en/us/support/docs/ip/border-gateway-protocol-bgp/13753-25.html
9. Wikipedia, Path-vector routing protocol — AS-path loop detection, path-vector vs distance vector vs link state. https://en.wikipedia.org/wiki/Path_vector_protocol
10. APNIC BGP Basics — path vector, AS path loop avoidance, BGP attributes. https://training.apnic.net/wp-content/uploads/sites/2/2016/11/eROU03_BGP_Basics.pdf
11. UCSD CSE123 Lecture 16, Interdomain Routing — path-vector loop detection, BGP attributes, incremental updates/withdrawals. https://cseweb.ucsd.edu/classes/sp15/cse123-a/lectures/123sp15-lec16.pdf
12. DPDK LPM6 Library — longest prefix match, multi-bit trie for 128-bit keys. https://doc.dpdk.org/guides/prog_guide/lpm6_lib.html
13. Palo Alto ECMP Load-Balancing Algorithms — IP hash/modulo, session stickiness, equal-cost paths. https://docs.paloaltonetworks.com/ngfw/networking/ecmp/ecmp-load-balancing-algorithms
14. Cumulus Linux ECMP — equal-cost multi-path load sharing, per-flow hashing. https://docs.nvidia.com/networking-ethernet-software/cumulus-linux-59/Layer-3/Routing/Equal-Cost-Multipath-Load-Sharing
15. Juniper BGP Route Flap Damping — penalty decay, suppress/reuse thresholds. https://www.juniper.net/documentation/us/en/software/junos/routing-policy/topics/concept/policy-using-routing-policies-to-damp-bgp-route-flapping.html
16. Cisco ASR 9000 LFA Fast Reroute — loop-free alternate backup next-hops. https://www.cisco.com/c/en/us/td/docs/routers/asr9000/software/26xx/routing/configuration/guide/b-routing-cg-asr9000-26xx/ipv4-ipv6-loop-free-alternate-fast-reroute.html
17. Ably, Consistent Hashing Explained — hash ring, node add/remove minimal disruption. https://ably.com/blog/implementing-efficient-consistent-hashing
18. Wikipedia, Rendezvous Hashing — highest random weight, distributed agreement on replica selection. https://en.wikipedia.org/wiki/Rendezvous_hashing
19. INET MANET Showcase — reactive/on-demand routing (AODV/DSR), route discovery and maintenance. https://inet.omnetpp.org/docs/showcases/routing/manet/doc/
20. OASIS: Anycast for Any Service, NSDI'06 — closest-replica selection via bucket→location mapping. https://www.usenix.org/legacy/events/nsdi06/tech/full_papers/freedman/freedman.pdf
21. Cisco Community, Unicast RPF vs Reverse Path Forwarding — source-address verification loop prevention. https://community.cisco.com/t5/switching/what-is-the-difference-between-unicast-rpf-and-reverse-path/td-p/2685782
22. HOTAC'08, Sloppy Management of Structured P2P Services — lazy background routing-table maintenance, gossip anti-entropy. https://www.distributed-systems.net/my-data/papers/2008.hotac.pdf
23. Springer, Dynamic Routing / OSPF — link-state database, SPF/Dijkstra routing table generation. https://link.springer.com/chapter/10.1007/978-981-19-3029-4_6
24. Pearson Routing Protocols Companion Guide — link-state LSP flooding, SPF tree, routing table construction. https://www.pearson.de/media/muster/ext/9780133476293.pdf
25. Nokia OSPF Guide — link-state advertisements, topological database, SPF shortest path. https://documentation.nokia.com/sr/23-10-2/books/unicast-routing-protocols/ospf-unicast-routing-protocols.html
26. RFC 9815, BGP-LS SPF Routing — RIB/LSDB/SPF calculation, candidate list. https://www.rfc-editor.org/rfc/rfc9815.pdf
27. Flylib, Summarizing Outbound Routing Information / BGP — route summarization and hierarchical aggregation. https://flylib.com/books/en/2.286.1/summarizing_outbound_routing_information.html
28. IEEE, Routing Table Partitioning for Speedy Packet Lookups — SPAL table partitioning, trie-based lookup. https://store.computer.org/csdl/journal/td/2006/05/l0481/13rRUILc8eP
29. Tarantool vshard Architecture — routing table bucket→replica set mapping, failover/rebalancing. https://www.tarantool.io/en/doc/latest/platform/sharding/vshard_architecture
30. mainline routing_table.rs — Rust Kademlia routing table, k-buckets, MAX_BUCKET_SIZE_K=20. https://docs.rs/mainline/8.0.1/src/mainline/common/routing_table.rs.html
31. Skybridge: Bounded Staleness for Distributed Caches, USENIX OSDI'25 — staleness checking vs write HLC, freshness tracking. https://www.usenix.org/system/files/osdi25-lyerly.pdf
32. SSRN, View Divergence of Freshness in Replicated Database Systems — freshness measurement, distributed vs centralized update propagation. https://papers.ssrn.com/sol3/Delivery.cfm?abstractid=1898447
33. Berkeley OceanStore, Locality Aware Mechanisms for Large-Scale Networks — locality awareness, exploit local over remote. https://oceanstore.cs.berkeley.edu/publications/papers/pdf/fudico-locality.pdf
34. ClickHouse Replica-aware Routing — sticky routing to same replica for read-after-write consistency. https://clickhouse.com/docs/products/cloud/features/infrastructure/replica-aware-routing
35. IDS prompts.txt entry 61 (verbatim). C:/idfs/prompts.txt — Phase 061 crate definition, contents, constraints.
36. CRATES.md entry 61. C:/idfs/CRATES.md — "61 ids-routing-table — Routing table management".
37. Existing ids-000-aa skeleton. C:/idfs/ids-000-aa/ — canonical types and sealed trait contracts consumed by this crate.
38. HX-241 reference plan format. C:/idfs/plans/plan-001.md, plan-048.md, plan-060.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines all architecture, routing concerns, algorithms, data structures, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 061 (ids-routing-table).
