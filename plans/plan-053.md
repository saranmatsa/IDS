## Phase 053 — Create the ids-placement-policy crate: Dataset-specific placement policies.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Note on numbering: prompts.txt entry 53 is ids-placement-policy ("Create the ids-placement-policy crate: Dataset-specific placement policies."). Entry 54 (ids-partition-map) is a separate phase.

---

## 1. Executive Summary

Phase 053 creates **a new separate crate `ids-placement-policy`** (not folders inside an existing crate), organized into subfolders by subsystem. `ids-placement-policy` implements dataset-specific placement policies for the IDS distributed system: it decides where full replicas and encoded fragments of a dataset are placed across failure domains, how many copies and fragments are required, what the reconstruction threshold is, which placement domains are mandatory, how regions must be distributed, what recovery must do after failure, how placement is verified, and what the minimum acceptable redundancy state is before writes may proceed. The crate is carrier-agnostic: it never opens sockets or speaks IP/TCP/UDP/QUIC/HTTP/DNS directly; all placement traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519 (placement proof signatures), xchacha20poly1305 / chacha20poly1305 (placement payload encryption), sha256 and blake3 (content addressing, placement commitment digests).

Key engineering decisions:
- **New crate `ids-placement-policy`**, separate from `ids-000-aa` and all other crates, per the IDS principle of separate crates over folders.
- **Dataset-specific policy model**: every dataset carries a `PlacementPolicy` specifying replica count, fragment count, reconstruction threshold, placement domains, regional distribution, recovery requirements, verification requirements, and minimum redundancy state.
- **Failure-domain isolation**: at most one replica/fragment per placement domain at each tier (host, rack, row, room, datacenter, region) so a single domain failure cannot reduce redundancy below the reconstruction threshold.
- **Replica + erasure-code hybrid**: full replicas for hot/low-latency access, encoded fragments (k+m) for cold/high-durability storage; reconstruction threshold k defines the minimum fragments needed to rebuild the dataset.
- **Carrier-agnostic**: zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all frames carried via `ids-carrier::Endpoint`.
- **Allowed crypto only**: ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3.
- **ids-000-aa** holds canonical types and sealed trait contracts (e.g. `trait PlacementPolicy`, `PlacementDomain`, `RedundancyState`, `ReconstructionThreshold`, `VerificationResult`); `ids-placement-policy` implements the contracts.

---

## 2. Architecture Overview

`ids-placement-policy` sits above `ids-carrier` and below the dataset orchestration layer, bridging the gap between "where should data live" (placement decision) and "where does data actually live" (topology view from ids-partition-map / ids-carrier). It never opens sockets; it produces placement decisions and proofs shipped as `Vec<u8>` over `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────┐
│                   DATASET ORCHESTRATION LAYER                        │
│  dataset lifecycle, replication scheduling, repair initiation        │
├──────────────────────────────────────────────────────────────────────┤
│               Phase 053: ids-placement-policy (NEW CRATE)            │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  placement policy model (replicas, fragments, threshold)     │   │
│  │  failure-domain isolation (host/rack/row/room/dc/region)     │   │
│  │  regional distribution requirements                          │   │
│  │  recovery requirements (rebuild after failure)               │   │
│  │  verification requirements (proof-of-placement)              │   │
│  │  minimum acceptable redundancy state                          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  persistence: policy journal, redundancy watermark,          │   │
│  │  placement commitment digests (durable)                      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  wire: signed placement decisions (ed25519),                 │   │
│  │  encrypted payloads (xchacha20poly1305/chacha20poly1305)     │   │
│  │  content hashing (sha256 / blake3)                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────────┤
│             ids-carrier (Endpoint = CarrierType + Vec<u8>)          │
│  opaque datagram transport; no IP/TCP/UDP/QUIC/HTTP/DNS/etc.         │
├──────────────────────────────────────────────────────────────────────┤
│             ids-000-aa (master crate, sealed contracts)             │
│  trait PlacementPolicy, PlacementDomain, RedundancyState, ...        │
└──────────────────────────────────────────────────────────────────────┘
```

**Data flow of a placement decision:**
1. **Policy evaluation**: dataset author (or orchestrator) submits a `PlacementPolicy` for a dataset. `ids-placement-policy` validates it against cluster topology (from ids-partition-map / ids-carrier) and the allowed redundancy budget.
2. **Domain selection**: selects placement domains at each tier such that no two replicas/fragments of the same stripe share a domain, respecting `required_placement_domains` and `regional_distribution`.
3. **Replica / fragment assignment**: assigns `num_full_replicas` full copies and `num_encoded_fragments` erasure-coded fragments (k data + m parity = reconstruction threshold k) to selected domains.
4. **Commitment**: signs the placement decision (ed25519), optionally encrypts payloads, hashes content (sha256/blake3), and records the durability commitment.
5. **Verification**: proof-of-placement audit checks that fragments/replicas actually reside where the policy says; verifies `minimum_redundancy_state` is met before acknowledging writes.
6. **Recovery**: on domain failure, recomputes placement to restore redundancy above the minimum state; initiates rebuild of lost fragments from surviving k-of-(k+m).

**Redundancy safety argument:** failure-domain isolation guarantees a single domain failure knocks out at most one replica/fragment; with reconstruction threshold k and at least k surviving fragments across independent domains, the dataset is always reconstructable; `minimum_redundancy_state` is the operational floor below which writes are rejected.

---

## 3. Detailed Research Dimensions

### 3.1 Placement policy model (dataset-specific)

A `PlacementPolicy` is a per-dataset configuration that answers: how many full replicas, how many encoded fragments, what reconstruction threshold, which placement domains are required, how data must be regionally distributed, what recovery must achieve, how placement is verified, and what the minimum acceptable redundancy state is. Each dataset may carry a different policy (hot datasets favor replicas; cold datasets favor erasure coding; regulated datasets favor regional confinement). Sources: HDFS rack-aware placement (NameNode chooses rack-distinct replicas), Ceph CRUSH rules (failure domain per pool), Swift storage policies (per-container policy with replicas or erasure coding), Cassandra NetworkTopologyStrategy (replicas per datacenter, rack-aware within DC).

### 3.2 Redundancy policy: replicas vs encoded fragments

Two redundancy primitives, used alone or combined:
- **Full replication**: `num_full_replicas` complete copies of the dataset. Tolerates `num_full_replicas - 1` simultaneous domain failures if copies are in distinct domains. Simple reads, high storage overhead (N×).
- **Erasure coding**: encode dataset into `k` data fragments + `m` parity fragments = `num_encoded_fragments` total; any `k` (reconstruction threshold) suffice to reconstruct. Tolerates `m` fragment losses. Storage overhead `(k+m)/k`. Sources: Scality erasure coding layouts (k+m tolerates m losses, overhead `(k+m)/k`), Facebook f4 (14+10 RS, 1.4× overhead vs 5× replication for same 4-node fault tolerance), Azure eAID (fixed recovery threshold F+1, variable fragments), Ceph erasure-coded pools (k data + m coding slices, one slice per failure domain).
- **Hybrid**: `r` full replicas for hot access + `(k+m)` encoded fragments for durability; placement algorithm selects domains for each independently.

### 3.3 Reconstruction threshold

The reconstruction threshold `k` is the minimum number of encoded fragments (or full replicas, treated as trivially self-sufficient fragments) required to reconstruct the dataset. With `num_encoded_fragments = k + m`, the system tolerates any `m` fragment losses. Lower `k` means easier reconstruction but higher storage; higher `k` means lower overhead but less fault tolerance per fragment count. Sources: eAID (recovery threshold F+1 fixed while fragment count varies), Azure LRC (local reconstruction groups reduce bandwidth), regenerating codes (Dimakis et al. minimize repair bandwidth).

### 3.4 Required placement domains

`required_placement_domains` enumerates the failure domains (host, rack, row, room, datacenter, region) across which replicas/fragments must be spread. Constraint: at most one replica/fragment of a given stripe per domain at each tier. Losing one domain costs at most one copy. If `required_placement_domains` = 3 at rack tier, need ≥3 racks; a single rack failure loses ≤1 replica/fragment. Sources: Ceph failure domains (host/rack/row/room/datacenter, at most one slice per domain), Virtuozzo failure domains (disk/host/rack/row/room with redundancy ≥ domain count), HDFS rack-aware placement (replicas on distinct racks), OpenStack Swift zones (region → zone → server → drive, highest failure-domain isolation first).

### 3.5 Regional distribution requirements

`regional_distribution` specifies how copies span geographic regions: e.g. "at least one full replica in each of regions A, B, C" or "no more than m fragments in any single region" or "home region for writes, read-only replicas elsewhere." Governs latency, data residency, and regional outage tolerance. Sources: DynamoDB Global Tables (replica tables per region, multi-active or active-passive write modes), Google Cloud Storage regional/dual-region/multi-region buckets (redundant across ≥2 AZs or regions), TiKV Placement Rules (PRIMARY_REGION for leaders, REGIONS for followers, per table/database/partition), OpenStack Swift regions/zones (data first spread across regions, then zones).

### 3.6 Recovery requirements

`recovery_requirements` define what happens after a domain failure: how quickly redundancy must be restored, which nodes participate in rebuild, bandwidth limits, whether repair is eager (immediate on failure) or lazy (threshold-based). Sources: eAID-KV (proactive vs reactive repair, threshold-based triggers), IAPRepair (proactive repair with in-network aggregation), regenerating codes / MSR/MBR (minimize repair bandwidth), distributed repair strategies (new node downloads from surviving nodes to regenerate lost fragment).

### 3.7 Verification requirements

`verification_requirements` define how placement is audited: proof-of-placement challenges, fragment integrity checks (sha256/blake3 per fragment), signed placement decisions (ed25519), audit frequency, quorum of auditors. A placement is valid only if verifiers can confirm fragments/replicas reside in the policy-assigned domains and are intact. Sources: FrostFS Proof-of-Placement (hits/misses against container placement policy), Vault decentralized storage (VRF-based placement group sampling, PDP audit of EC fragments), did:crdt (signed deltas, Ed25519, BLAKE3-256 content addressing).

### 3.8 Minimum acceptable redundancy state

`minimum_redundancy_state` is the operational floor: the lowest redundancy level (replicas + fragments surviving across domains) at which writes are still accepted. Below it, the dataset enters read-only or degraded mode until recovery restores redundancy. Expressible as "at least N full replicas and K encoded fragments across D distinct domains" or "reconstructability guaranteed (k surviving fragments)." Sources: Ceph min_size (minimum written replicas before ack), etcd quorum (majority must be alive), DistributedLog minRegionsForDurability (ack quorum spans ≥ min regions), Ondat failure modes (hard/soft: how many replicas may fail before read-only).

### 3.9 Placement algorithms

Candidate algorithms for selecting domains and assigning copies:
- **Rendezvous / highest-random-weight hashing**: each node scores itself for a dataset key; top N distinct-domain nodes win. Naturally failure-domain aware when scores exclude same-domain nodes.
- **Consistent hashing ring with virtual nodes + successor replication**: hash dataset to ring position, walk clockwise picking next R distinct physical nodes, skipping same failure domain.
- **CRUSH-style pseudo-random rule**: deterministic, decentralized placement; administrator writes rules (take root, choose bucket type, fail if same domain).
- **Topology-aware greedy**: sort domains by remaining capacity/weight, pick one per tier until replica/fragment count satisfied.
Sources: Ceph CRUSH (failure domains, weights, step rules), consistent hashing with vnodes (bounded load, declustering), Scality sprov (disk-server-rack-site aware replication), systemoverflow consistent hashing implementation patterns (successor replication with AZ diversity).

### 3.10 did:crdt / signed placement proofs relevance

did:crdt (arxiv 2606.16223): signed CRDT deltas with HLC timestamps, causal delivery, Ed25519 signatures over canonical content, BLAKE3-256 content addressing — a model for authenticated, carrier-agnostic placement commitments that `ids-placement-policy` can mirror for signed placement decisions. Decoupling Trust in Byzantine CRDTs (arxiv 2606.31759): decouples identity trust from content trust; post-compromise handling without breaking causality — relevant when a placement authority's key is compromised mid-policy.

---

## 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Correlated failure: two replicas/fragments in same failure domain lost together | Failure-domain isolation: at most one copy per domain at each tier; validate domain distinctness before placement |
| Reconstructability lost: fewer than k fragments survive | `minimum_redundancy_state` blocks writes below floor; recovery rebuilds from k surviving fragments; erasure coding tolerates m losses |
| Placement authority compromised / forged placement decision | ed25519-signed placement decisions; verify signatures before applying |
| Fragment tampering / silent corruption | per-fragment sha256/blake3 hashes; verification audits recompute and compare |
| Placement eavesdropping / traffic analysis | optional xchacha20poly1305 / chacha20poly1305 encryption of placement payloads |
| Replay of stale placement decision | nonce + per-decision commitment digest; grantor rejects duplicate/out-of-order decisions |
| Regional outage violates regional_distribution | regional distribution checked at placement time; writes blocked if regional spread cannot be satisfied |
| Recovery stalls below minimum redundancy | eager vs lazy repair policy; bandwidth-bounded rebuild; read-only mode below floor |
| Carrier-agnostic rule violation (std::net etc.) | `ids-carrier::Endpoint` sole envelope; source grep assertion in tests |
| Disallowed crypto in placement module | dependency audit bans non-allowed crates; only ed25519/xchacha20poly1305/chacha20poly1305/sha256/blake3 |

---

## 5. Integration with Existing Crates

`ids-placement-policy` depends on and integrates with:

| Crate | Role in Phase 053 |
|-------|-------------------|
| `ids-000-aa` (master) | Canonical types: `trait PlacementPolicy` (sealed contract), `PlacementDomain`, `RedundancyState`, `ReconstructionThreshold`, `VerificationResult`, `Endpoint`/`CarrierType` re-exported via `ids-carrier`, sealed error contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); all placement traffic rides opaque datagrams over `Endpoint`; no direct carrier dependency |
| `ids-partition-map` (Phase 054) | Topology view: node → domain mapping (host/rack/region) used for domain selection |
| `ids-crypto` | ed25519 signatures, xchacha20poly1305/chacha20poly1305 encryption, sha256/blake3 hashing (only allowed primitives) |
| `ids-storage` | Durable persistence of policy journal, redundancy watermark, placement commitment digests |
| `ids-membership` | Join/leave/lifecycle context for placement domain members |
| `ids-quorum` | Quorum intersection for placement decision safety (at most one active policy per dataset) |

**Integration points:**
- `ids-partition-map` supplies the failure-domain topology (node → host/rack/row/room/datacenter/region) that domain selection consults.
- `ids-storage` persists the placement policy journal, redundancy high-water mark, and commitment digests so a restarted placement authority cannot regress redundancy.
- All placement traffic is `ids-carrier::Endpoint` frames (`CarrierType` + `Vec<u8>`); `ids-placement-policy` never imports `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — carrier-agnostic by design (I-CARR-001).

---

## 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-placement-policy` (new crate, depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`, `ids-partition-map`; ed25519-dalek, xchacha20poly1305, chacha20poly1305, sha2, blake3, thiserror, serde, prost/cbor for wire encoding)

```
ids-placement-policy/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (default replica/fragment counts, thresholds)
├── error.rs                        # PlacementError (thiserror, comprehensive)
├── prelude.rs
│
├── policy/                         # Placement policy model
│   ├── mod.rs
│   ├── placement_policy.rs         # PlacementPolicy: replicas, fragments, threshold, domains, regions
│   ├── redundancy_policy.rs        # RedundancyPolicy: replica vs erasure-code mix
│   ├── replica_count.rs            # NumFullReplicas
│   ├── fragment_count.rs           # NumEncodedFragments
│   ├── reconstruction_threshold.rs # ReconstructionThreshold (k of k+m)
│   ├── placement_domains.rs        # RequiredPlacementDomains (host/rack/row/room/dc/region)
│   ├── regional_distribution.rs    # RegionalDistribution (per-region replica/fragment quotas)
│   ├── recovery_requirements.rs    # RecoveryRequirements (eager/lazy, bandwidth, deadline)
│   ├── verification_requirements.rs # VerificationRequirements (audit frequency, quorum)
│   └── min_redundancy_state.rs     # MinimumRedundancyState (operational floor)
│
├── domain/                         # Failure-domain isolation
│   ├── mod.rs
│   ├── domain_tier.rs              # DomainTier enum (Host, Rack, Row, Room, Datacenter, Region)
│   ├── domain_id.rs                # PlacementDomainId
│   ├── domain_topology.rs          # DomainTopology: node → domain mapping (from ids-partition-map)
│   ├── domain_selector.rs          # Selects distinct domains per tier
│   ├── domain_constraint.rs        # At-most-one-copy-per-domain constraint checker
│   └── domain_weight.rs            # Domain capacity weights for balanced placement
│
├── placement/                      # Placement algorithm
│   ├── mod.rs
│   ├── placer.rs                   # Core placement: assign replicas + fragments to domains
│   ├── rendezvous.rs               # Rendezvous/highest-random-weight hashing placer
│   ├── consistent_hash.rs          # Consistent hashing ring with vnodes + successor replication
│   ├── crush_rule.rs               # CRUSH-style pseudo-random placement rule
│   ├── greedy_topology.rs          # Topology-aware greedy domain selection
│   ├── replica_placer.rs           # Places full replicas across distinct domains
│   ├── fragment_placer.rs          # Places encoded fragments across distinct domains
│   └── hybrid_placer.rs            # Combines replica + fragment placement
│
├── erasure/                        # Erasure coding parameters
│   ├── mod.rs
│   ├── ec_params.rs                # (k, m) erasure code parameters
│   ├── fragment_id.rs              # FragmentId (per stripe, per fragment index)
│   ├── stripe_id.rs                # StripeId (per dataset stripe)
│   └── reconstruction.rs           # Reconstruction logic: k-of-(k+m) rebuild
│
├── topology/                       # Regional & cluster topology
│   ├── mod.rs
│   ├── region.rs                   # Region (geographic)
│   ├── regional_quota.rs           # Per-region replica/fragment quotas
│   ├── topology_view.rs            # Aggregated cluster topology view
│   └── region_spread.rs            # Validates regional distribution requirements
│
├── recovery/                       # Recovery after domain failure
│   ├── mod.rs
│   ├── recovery_plan.rs            # Plan to restore redundancy after failure
│   ├── rebuild_scheduler.rs        # Schedules fragment rebuilds from surviving k
│   ├── repair_policy.rs            # Eager vs lazy repair trigger
│   ├── bandwidth_limit.rs          # Repair bandwidth throttling
│   └── degraded_mode.rs            # Read-only mode below minimum redundancy
│
├── verification/                   # Proof-of-placement & audit
│   ├── mod.rs
│   ├── placement_proof.rs          # Signed placement commitment (ed25519)
│   ├── proof_of_placement.rs       # Audit: verify fragments reside in assigned domains
│   ├── fragment_integrity.rs       # Per-fragment sha256/blake3 integrity check
│   ├── audit_scheduler.rs          # Periodic placement audit
│   ├── audit_quorum.rs             # Quorum of auditors required for validity
│   └── commitment_digest.rs        # blake3/sha256 commitment over placement
│
├── state/                          # Redundancy state tracking
│   ├── mod.rs
│   ├── redundancy_state.rs         # RedundancyState: healthy/degraded/critical/read-only
│   ├── redundancy_watermark.rs     # Durable high-water mark (replicas + fragments)
│   ├── state_machine.rs            # State transitions on failure/recovery
│   └── write_gate.rs               # Blocks writes below minimum_redundancy_state
│
├── wire/                           # Carrier-agnostic wire format
│   ├── mod.rs
│   ├── frame.rs                    # PlacementFrame: opaque Vec<u8> payload for Endpoint
│   ├── policy_msg.rs               # Signed placement policy message (ed25519)
│   ├── proof_msg.rs                # Proof-of-placement message
│   └── codec.rs                    # Serialization (cbor/prost), versioned
│
├── crypto/                         # Allowed-crypto boundary
│   ├── mod.rs
│   ├── signing.rs                  # ed25519 sign/verify for placement decisions
│   ├── encryption.rs               # xchacha20poly1305 / chacha20poly1305 for payloads
│   ├── hashing.rs                  # sha256 / blake3 for content addressing, commitment digests
│   └── nonce.rs                    # Nonce generation for replay resistance
│
├── persist/                        # Durable persistence
│   ├── mod.rs
│   ├── policy_journal.rs           # Append-only placement policy journal
│   ├── watermark_store.rs          # Durable redundancy high-water mark
│   ├── commitment_store.rs         # Durable placement commitment digests
│   └── snapshot.rs                 # Periodic placement-state snapshots
│
├── carrier/                        # Carrier-agnostic boundary
│   ├── mod.rs
│   └── endpoint_binding.rs         # Endpoint usage only, no std::net
│
├── tests/                          # Property & convergence tests (not shipped)
│   ├── mod.rs
│   ├── domain_isolation.rs         # Verify at most one copy per domain
│   ├── reconstructability.rs       # k surviving fragments → reconstructable
│   ├── redundancy_floor.rs         # Writes blocked below minimum_redundancy_state
│   ├── regional_distribution.rs    # Regional quotas honored
│   ├── recovery.rs                 # Rebuild after domain failure restores redundancy
│   ├── verification.rs             # Proof-of-placement audit passes/fails correctly
│   ├── carrier_agnostic.rs         # Asserts no std::net/tokio::net usage
│   └── crypto_compliance.rs        # Asserts only allowed crypto types
│
├── benches/                        # Performance benchmarks
│   ├── placement_latency.rs
│   ├── verification_throughput.rs
│   └── redundancy_check_cost.rs
│
├── docs/
│   ├── PLACEMENT_POLICY.md
│   ├── FAILURE_DOMAIN_MODEL.md
│   └── PROOF_OF_PLACEMENT.md
│
└── examples/
    ├── replicated_dataset_example.rs
    └── erasure_coded_dataset_example.rs
```

Target scale: ~300–600 source files across subfolders, production-grade, matching the ~90–1000 file subfolder structure directive.

---

## 7. Key Algorithms & Data Structures

**PlacementPolicy** = `{ dataset_id, redundancy_policy, num_full_replicas, num_encoded_fragments, reconstruction_threshold_k, required_placement_domains, regional_distribution, recovery_requirements, verification_requirements, minimum_redundancy_state, signature: ed25519::Signature }`

**Replica placement**: assign `num_full_replicas` full copies to `num_full_replicas` distinct placement domains at the required tier (host/rack/...), one copy per domain.

**Fragment placement**: encode dataset into `k` data + `m` parity fragments; assign `k+m` fragments to `k+m` distinct domains; reconstruction requires any `k` surviving fragments.

**Failure-domain isolation**: for each tier T in required_placement_domains, the set of domains holding copies of a stripe must be pairwise distinct; a single domain failure removes at most one copy.

**Regional distribution check**: per-region replica/fragment quotas must be satisfied; if not, placement is rejected and writes are blocked.

**Minimum redundancy state**: `RedundancyState` = Healthy | Degraded | Critical | ReadOnly; writes admitted only when state == Healthy (above `minimum_redundancy_state`); below floor → ReadOnly until recovery restores.

**Reconstruction**: on fragment loss, read any `k` surviving fragments from distinct domains, run erasure-code decode, write replacement fragment to a new distinct domain.

**Proof-of-placement**: auditor challenges node; node returns fragment + proof-of-possession; verifier checks fragment hash (sha256/blake3) against placement commitment and domain membership against policy.

**Placement algorithms**: rendezvous hashing (top N distinct-domain nodes by score), consistent hashing ring with vnodes + successor replication skipping same-domain nodes, CRUSH-style rule (take root → choose bucket type → fail if same domain), topology-aware greedy (capacity-weighted domain selection).

---

## 8. Dependencies

**Crate dependencies:**
- `ids-000-aa` — canonical types, sealed `trait PlacementPolicy` contract, redundancy/state/error contracts
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`), opaque datagram transport
- `ids-crypto` — ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 (only allowed primitives)
- `ids-storage` — durable placement policy journal, redundancy watermark, commitment digests
- `ids-partition-map` — failure-domain topology (node → domain mapping)

**External crypto dependencies (allowed set only):**
- `ed25519-dalek` — ed25519 signatures for placement decisions and proofs
- `xchacha20poly1305` / `chacha20poly1305` — placement payload encryption
- `sha2` — sha256 content addressing and commitment digests
- `blake3` — blake3 hashing, fragment integrity

**Standard library only for everything else** — no `std::net`, no `tokio::net`, no networking crates.

**Dev dependencies:** proptest/bolero for property tests, criterion for benchmarks (test-only, not in production build).

---

## 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | PlacementPolicy model | `src/policy/placement_policy.rs` exposes all fields (replicas, fragments, threshold, domains, regions, recovery, verification, min redundancy); `cargo check -p ids-placement-policy` exits 0 |
| 2 | Failure-domain isolation | `src/domain/domain_constraint.rs` enforces at most one copy per domain per tier; unit test: 2 copies in same rack rejected |
| 3 | Reconstruction threshold | `src/erasure/reconstruction.rs`: any k of k+m fragments reconstructs dataset; test: k=10, m=4, lose 4 → reconstruct OK; lose 5 → fail |
| 4 | Minimum redundancy state | `src/state/write_gate.rs` blocks writes below `minimum_redundancy_state`; test: state=Critical → write rejected |
| 5 | Regional distribution | `src/topology/region_spread.rs` enforces per-region quotas; test: quota unmet → placement rejected |
| 6 | Recovery after failure | `src/recovery/rebuild_scheduler.rs` restores redundancy from k surviving fragments; test: simulate domain loss → rebuild completes → state=Healthy |
| 7 | Proof-of-placement | `src/verification/proof_of_placement.rs` verifies fragment domain + integrity; test: fragment in wrong domain → audit fails |
| 8 | Carrier agnosticism | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches; all traffic via `ids-carrier::Endpoint` |
| 9 | Crypto compliance | Source references only ed25519/xchacha20poly1305/chacha20poly1305/sha256/blake3; no other crypto crates |
| 10 | Crate compiles cleanly | `cargo check -p ids-placement-policy` exits 0 with no new warnings |
| 11 | Conformance tests pass | `cargo test -p ids-placement-policy` passes all domain/reconstruction/redundancy/regional/recovery/verification tests |

---

## 10. Remaining Uncertainties

1. **Erasure codec selection:** The crate defines `(k, m)` parameters and reconstruction logic but defers the concrete erasure-code implementation (Reed-Solomon, LRC, regenerating codes) to a separate dependency or phase; only the placement/threshold semantics are specified here.
2. **Topology source:** `ids-placement-policy` consumes node→domain mapping from `ids-partition-map` (Phase 054); if that crate's topology model differs, the domain selector interface may need adaptation.
3. **Repair bandwidth budget:** `recovery_requirements` includes bandwidth limits but the numeric budget is deployment-specific and deferred.
4. **Audit quorum size:** The number of auditors required to validate a placement proof is policy-configurable; no default is mandated.
5. **Hybrid replica/fragment trade-off:** The split between `num_full_replicas` and `(k+m)` encoded fragments is workload-dependent; no single default is specified.
6. **Offline placement allowance:** Precomputed placement for partitioned/disconnected datasets (bounded offline finality) is a possible extension, not in scope.
7. **Placement commitment persistence crash window:** If the store commits a placement decision but crashes before durably recording the redundancy watermark, a stale watermark could permit writes below the floor; write-ahead logging mitigates but adds latency.

---

## 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-placement-policy` (not folders in `ids-000-aa`) | IDS principle: separate crates over folders; `ids-000-aa` holds only sealed trait contracts |
| Per-dataset `PlacementPolicy` | Different datasets have different redundancy, regional, and recovery needs |
| Failure-domain isolation (at most one copy per domain per tier) | Single domain failure knocks out at most one replica/fragment; preserves reconstructability |
| Erasure coding (k data + m parity, reconstruct from any k) | Lower storage overhead than replication at equal fault tolerance (Ceph, f4, eAID) |
| `minimum_redundancy_state` write gate | Operational floor prevents writes when redundancy cannot survive another failure |
| Proof-of-placement with ed25519 signatures + sha256/blake3 integrity | Authenticated, verifiable placement without carrier trust |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic; no `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
| Crypto limited to ed25519 / xchacha20poly1305 / chacha20poly1305 / sha256 / blake3 | Per IDS allowed-crypto policy |
| Durable redundancy watermark + policy journal | Prevents redundancy regression after restart |
| Placement algorithms: rendezvous / consistent-hashing / CRUSH / greedy | Decentralized, deterministic, failure-domain-aware placement options |

---

## 12. Sources

- Ceph CRUSH failure domains — https://ceph.com/en/news/blog/2026/slicing-the-pie (k+m erasure-coded slices, one per failure domain; replicated pool size N)
- Ceph erasure code plugin / LRC — https://docs.ceph.com/en/reef/rados/operations/erasure-code-lrc (k=4 m=2, crush-locality=rack, crush-failure-domain=host)
- Ceph placement groups — https://docs.ceph.com/en/mimic/rados/operations/placement-groups (PG maps objects to OSDs; replica placement)
- Ceph Canonical reference architecture — https://developer.ibm.com/storage/2020/04/08/configuring-spectrum-scale-file-systems-for-reliability (failure domains, replicated pools size 3, min_size 2)
- HDFS Architecture / rack-aware replica placement — https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/HdfsDesign.html (replication factor, rack-aware placement, NameNode)
- Cassandra Data replication — https://docs.datastax.com/en/cassandra-oss/3.0/cassandra/architecture/archDataDistributeReplication.html (SimpleStrategy, NetworkTopologyStrategy, replicas per DC, rack-aware)
- Cassandra architecture / Dynamo — https://cassandra.apache.org/doc/stable/cassandra/architecture/dynamo.html (RF, replication strategies, rack-aware replica placement)
- OpenStack Object Storage zones — https://docs.openstack.org/liberty/config-reference/content/swift-zones.html (region → zone → server → drive, highest failure-domain isolation)
- Swift Architectural Overview — https://docs.openstack.org/swift/2026.2/overview_architecture.html (ring, zones, devices, partitions, replicas, storage policies)
- Swift The Rings — https://docs.openstack.org/swift/rocky/overview_ring.html (replica count, failure domains region/zone/server, independent replica assignment)
- DynamoDB Global Tables — https://docs.aws.amazon.com/pdfs/prescriptive-guidance/latest/dynamodb-global-tables/dynamodb-global-tables.pdf (multi-region replica tables, write modes, regional failure resilience)
- DynamoDB FAQs — https://aws.amazon.com/dynamodb/faqs (synchronous replication across 3 AZs per Region)
- Google Cloud Storage availability and durability — https://cloud.google.com/storage/docs/availability-durability (regional/dual-region/multi-region, ≥2 AZs)
- TiKV Placement Rules in SQL — https://docs.pingcap.com/tidbcloud/placement-rules-in-sql (PRIMARY_REGION, REGIONS, per table/partition placement)
- MongoDB Zones / zone sharding — http://docs.mongodb.org/master/core/tag-aware-sharding (zone-based shard placement)
- eAID: Elastic Asynchronous Information Dispersal — https://arxiv.org/html/2603.24761v2 ((k,m) erasure code, recovery threshold F+1, variable fragments, leader assigns fragments)
- Scality erasure coding layouts — https://scality.com/blog/erasure-coding-layouts (k+m tolerates m losses, overhead `(k+m)/k`, fault domain placement rule)
- Erasure coding in Windows Azure Storage — https://usenix.org/sites/default/files/conference/protected-files/huang_atc12_slides_0.pdf (replica/fragment placement across independent fault domains and upgrade domains)
- Monotone Erasure Codes — https://arxiv.org/html/2605.22426 (MDS codes, any k of n fragments reconstruct, trust-aware access structures)
- IAPRepair: proactive repair for erasure-coded storage — https://arxiv.org/html/2610.11114v1 (proactive vs reactive repair, migration scheduling)
- Erasure Coding in Windows Azure Storage (slides) — replica placement for reliability across fault domains
- Distributed data storage with opportunistic repair — https://ieeexplore.ieee.org/document/6848122 (repair bandwidth minimization, regenerating codes)
- Regenrating codes / Dimakis et al. — https://arxiv.org/pdf/0803.0632 (MSR/MBR, minimize repair bandwidth)
- Repair Pipelining for Erasure-Coded Storage — https://arxiv.org/html/1908.01527v3 (repair traffic reduction, n/k overhead)
- FDP: Data Placement Promise of Modern NVMe SSDs — https://usenix.org/system/files/fast26-cheng.pdf (placement groups, invalidation-rate grouping)
- Attributes consistent hashing — https://dl.acm.org/doi/10.1145/3243176.3243202 (hashing-based data placement, heterogeneous nodes)
- Implementation Patterns: Placement, Replication, Bounded Load — https://systemoverflow.com/learn/partitioning-sharding/consistent-hashing/implementation-patterns-placement-replication-and-bounded-load (ring placement, successor replication, AZ diversity, bounded load)
- Redis Cluster spec — https://redis.io/docs/latest/operate/oss_and_stack/reference/cluster-spec (16384 slots, replication, replica migration)
- Comparing cost and performance of replication and erasure coding — https://arxiv.org/html/1308.1887v2 (replication vs erasure coding trade-offs)
- Ondat Topology-Aware Placement — https://docs.ondat.io/docs/concepts/tap (failure domains, failure modes hard/soft, topology labels)
- DistributedLog global replicated log — https://apache.github.io/distributedlog/globalreplicatedlog/main.html (minRegionsForDurability, ack quorum spans regions)
- etcd FAQ — https://etcd.io/docs/v3.3/faq (quorum majority, fault tolerance)
- FrostFS Proof-of-Placement — https://pkg.go.dev/github.com/TrueCloudLab/frostfs-sdk-go/audit (hits/misses against container placement policy)
- Vault: Decentralized Storage Made Durable — https://arxiv.org/pdf/2310.08403 (VRF-based placement group sampling, PDP audit of EC fragments)
- did:crdt — https://arxiv.org/html/2606.16223 (signed CRDT deltas, HLC, Ed25519, BLAKE3-256 content addressing)
- Decoupling Trust in Byzantine CRDTs — https://arxiv.org/html/2606.31759 (content vs identity trust, post-compromise)
- BookKeeper BP-34 Cluster Metadata Checker — https://bookkeeper.apache.org/bps/BP-34-cluster-metadata-checker (placement policy validation, durability contract)
- Cross-border replication / storage boundaries — https://scality.com/blog/cross-border-replication (system-level placement policy vs bucket-level rules)
- AEC-DS: Adaptive Erasure Coding — https://arxiv.org/html/2607.17460 (adaptive redundancy from audit feedback, PDP-triggered migration)
- IDS prompts.txt entry 53 (verbatim) — C:/idfs/prompts.txt lines 790-804 — ids-placement-policy crate definition, contents, constraints
- HX-241 plan format reference — C:/hx-241 plans plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources
- Reference IDS plans — C:/idfs/plans/plan-024.md (ids-lease), C:/idfs/plans/plan-012.md (ids-protocol) — same format, carrier-agnostic + allowed-crypto constraints
- HX-241 Rulebook — C:/idfs/RULEBOOK.md (git workflow, LOC targets, structure-first priorities)

---

**Constraint compliance verified:**
- New separate crate `ids-placement-policy` (not folders in existing crates): yes.
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`): yes.
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305: yes.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts (`trait PlacementPolicy`, etc.): yes.
- Research only — no code, no source modifications, only the plan document: yes.
- Plan matches HX-241 plan format (Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure, Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources): yes.
- Output file: `C:/idfs/plans/plan-053.md`: yes.
