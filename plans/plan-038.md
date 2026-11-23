## Phase 038 — Create the ids-mobility crate: Dataset/partition region changes with controlled handoff.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 565-579)

```
38. Research only do not read plan.md **[Phase 038 — Create the ids-mobility crate: Dataset/partition region changes with controlled handoff.]**

Crate: ids-mobility
Purpose: Dataset/partition region changes with controlled handoff.
Contents: Preferred/authoritative region changes, logical identity preservation, history preservation, access policy preservation, controlled handoff between old/new owners, cutover verification.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 038 — Create the ids-mobility crate: Dataset/partition region changes with controlled handoff.]**

Create plan-038.md in C:/idfs/plans/ for the ids-mobility crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 038 creates `ids-mobility`, a new separate IDS crate that owns dataset/partition region changes with controlled handoff. Its scope covers: preferred/authoritative region changes, logical identity preservation across a move, history preservation, access policy preservation across the region boundary, controlled handoff between old and new owners, and cutover verification. The crate is a guard/orchestration layer: it does not store dataset payloads itself; it drives the region-transition state machine, fences the old owner, validates the catch-up, and publishes the new authoritative region only when the new owner provably holds the history.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address; `CarrierType` ∈ {Loopback, Serial, Optical, Radio, Custom}). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-mobility` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-mobility` crate does NOT yet exist at `C:/idfs/ids-mobility/`. `ids-000-aa` already declares canonical types `NodeId` (`src/types.rs:21`), `RegionId` (`src/types.rs:68`), `PartitionId`, `RecordId` (`src/types.rs:262`), `Endpoint` (`src/types.rs:448`), `CarrierType` (`src/types.rs:394`), `AnomalyType`, `Checksum`, `SnapshotId`, `FragmentId`, `RecoveryProof` — all referenced by this crate but never redefined. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned (1) Shard Growth 20→200 — routing authority placement, consistent snapshot + mutation replay, controlled handoff/brief write barrier, per-range transfer checksums, old owners fenced but kept as recovery evidence through a safety window, durable migration state (not inferred from files on disk); (2) Live Cross-Region Data Migration four-phase protocol — fence operations by class rather than phase, content-stripped tombstones so a paginated copy cannot resurrect deleted data, locks enforced through routing metadata every request reads, single atomic routing update gated on end-to-end validation; (3) blue-green / shadow-table / dual-write migration — backfill historical data first, dual-write ongoing changes, verify with row counts + checksums + shadow reads, cut over reads before writes, keep source read-only as rollback target; (4) Remus live migration — ordered diversion via MVCC, snapshot copy then async update propagation then dual execution then ownership transfer, durability before ownership; (5) skaidb resharding — consistent-hash ring membership change, tombstones migrate with rows, reclaim only after current owner acks it holds a version at least as new (ack-gate prevents dropping the last copy); (6) Koala — ownership transfer decoupled from state location, lazy/per-key fetch-on-demand migration; (7) fencing tokens / monotonic epochs — every ownership grant returns a strictly increasing token, protected resource rejects lower tokens, split-brain prevention; (8) Cassandra tombstones with `gc_grace_seconds` — tombstone retained until all replicas have compacted past it, prevents zombie resurrection; (9) OR-Set CRDT add-wins semantics with tombstone GC at causal stability; (10) GDPR/data residency, authorization portability (identity separate from policy so access decisions survive a move); (11) anti-entropy Merkle-tree digest reconciliation for divergent-state repair after a region change; (12) cutover readiness checklists — row counts, checksums, shadow reads, business-rule validation, go/no-go sign-off gate.

---

### 2. Architecture Overview

`ids-mobility` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a region-change orchestration/guard layer: `ids-store`, `ids-fragment`, `ids-replication`, and `ids-reclaim` issue mutations against a dataset/partition; `ids-mobility` vetoes or stamps them while a region change is in flight, drives the handoff state machine, and publishes the new authoritative region only at a verified cutover point.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 APPLICATION / ids-* LAYER (mutation issuers)                │
│  ids-store, ids-fragment, ids-replication, ids-reclaim — issue writes/deletes│
├─────────────────────────────────────────────────────────────────────────────┤
│               Phase 038: ids-mobility (THIS CRATE — handoff orchestrator)   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ │
│  │ region-change   │ │ identity        │ │ access-policy   │ │ cutover   │ │
│  │ state machine   │ │ preservation    │ │ portability     │ │ verification│ │
│  │ (pending/       │ │ (stable logical │ │ (policy travels   │ │ (checksum,│ │
│  │  snapshot/      │ │  id survives    │ │  with the data; │ │  shadow    │ │
│  │  catch-up/      │ │  the move;      │ │  RBAC/ABAC      │ │  read,    │ │
│  │  fenced/        │ │  no re-issuance)│ │  re-evaluated   │ │  gate)    │ │
│  │  cut-over/      │ │                 │ │  at target)     │ │           │ │
│  │  rollback)      │ └─────────────────┘ └─────────────────┘ └───────────┘ │
│  └─────────────────┘                                                        │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ │
│  │ history         │ │ fencing/        │ │ anti-entropy    │ │ handoff   │ │
│  │ preservation    │ │ epoch tokens    │ │ repair (Merkle) │ │ audit     │ │
│  │ (tombstones,    │ │ (monotonic      │ │ (divergence     │ │ (append-  │ │
│  │  lineage,       │ │  epoch, old     │ │  detection +    │ │  only log)│ │
│  │  changelog)     │   owner fenced)   │   targeted sync)  │ │           │ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └───────────┘ │
├─────────────────────────────────────────────────────────────────────────────┤
│                  ids-000-aa · ids-carrier · ids-crypto                      │
│  RegionId, PartitionId, RecordId, NodeId, Endpoint, CarrierType            │
│  sealed trait contracts, invariants, error contracts                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a controlled region change:** (1) A `RegionChangeProposal` is submitted with source region, target region, dataset/partition id, and preferred/authoritative flag. (2) `ids-mobility` takes a revision-anchored consistent snapshot of the source dataset/partition and records the cut-over position `R`. (3) The target region is provisioned; a bulk copy streams the snapshot while the source remains the sole authoritative owner and keeps serving writes. (4) Ongoing mutations after `R` are captured (dual-write / CDC-style catch-up stream) and replayed at the target in order, tombstones included, so deletes are never resurrected. (5) When catch-up reaches `R`, a write fence is raised on the old owner (monotonic epoch bumped), the final delta is applied, and a content digest is computed on both sides. (6) Cutover verification: source and target digests compared at the same boundary, shadow reads pass, business-rule validation green. (7) Atomic routing update publishes the target as the new authoritative region at epoch `R+1`; old owner is fenced (its writes rejected), kept read-only as recovery evidence through a safety window. (8) After the observation period, the old owner's copy may be physically purged only if its current digest still matches the cut-over proof (no post-proof writes landed).

**Old-owner-authoritative contract:** until the flip, exactly one region answers as authoritative for the dataset/partition — the source. The target does not serve reads or accept writes off a stale snapshot. Ownership is a reward for provable possession, not a precondition for it.

---

### 3. Detailed Research Dimensions

#### 3.1 Preferred/authoritative region changes

A dataset/partition has a single authoritative region at any time (the owner that accepts writes and whose routing entry clients resolve to). A "preferred" region is a candidate target designated for an upcoming move; it becomes "authoritative" only at cutover. Research: routing authority stores a placement identifying the allowed write owner per range (Shard Growth 20→200). Migration workers track a consistent snapshot and how far mutation replay has progressed. Copies do not acquire ownership merely by existing on the target. The new placement is published only after the target is ready to own the history, and old routing caches receive redirects/rejections rather than permission to keep writing to the old owner.

Design implication for `ids-mobility`: a versioned `RegionAuthority` record (dataset/partition id → (authoritative region, epoch, preferred region, state)) is the single source of truth; every request carries the epoch so a region can refuse stale requests; cutover is an atomic compare-and-swap on this record gated on end-to-end validation.

#### 3.2 Logical identity preservation

The dataset/partition's logical identity must survive the region change: consumers referencing it by `DatasetId`/`PartitionId` must not need re-issuance or re-discovery after the move. Research: StableID (persistent entity identifiers across graph merges, identifier stability ≥95%); Source Known Identifiers (three-tier identity with BLAKE3 MAC, epoch selector); composite `(region, id)` keys so a row stays identifiable across regions; ULID-inspired composite identifiers (node + logical clock + namespace tag). Design implication: `DatasetId`/`PartitionId` are region-opaque `BLAKE3`-derived identifiers stored once in `ids-000-aa`; the region change updates only the `RegionAuthority` mapping, never the dataset id. Logical identity is decoupled from physical location.

#### 3.3 History preservation

History (the changelog of mutations, including deletes/tombstones) must be preserved across the move so that no edit or deletion is lost. Research: content-stripped tombstones carried during copy so a paginated copy cannot resurrect deleted data (four-phase protocol); tombstones retained until all replicas have compacted past them (Cassandra `gc_grace_seconds`); tombstones migrate with rows during resharding (skaidb); history as an append-only, hash-chained audit trail with signed checkpoints (plan-040 ids-immutable pattern); consistent cut / Chandy-Lamport distributed snapshot for capturing a global consistent state. Design implication: the migration copies the snapshot plus the full mutation log (tombstones included) from the cut-over position back to the beginning of history; the target's history is a superset of the source's at the fence.

#### 3.4 Access policy preservation

Access policies (RBAC/ABAC grants tied to the dataset/partition) must travel with the data and be re-evaluated at the target so no permission is silently dropped or elevated at the boundary. Research: NIST SP 800-207A zero-trust (policy based on service/application identity, not network location; environment-agnostic, follows the application); authorization portability (identity attributes, policy mapping, federation so RBAC/ABAC/entitlement decisions remain valid during migration); multi-region authorization as eventual-consistency replicated control plane with versioned policy snapshots; sticky policies remaining attached to data across systems (SPEAR-Q evidence quorums). Design implication: `ids-mobility` carries the access-policy grant set (subject/object/op/position) as part of the migration package; the target re-instantiates an equivalent policy snapshot versioned with the region epoch; policy changes during the move are replicated causally so the target never authorizes what the source would deny at the same causal position.

#### 3.5 Controlled handoff between old/new owners

The handoff is the core protocol: a sequence of phases, each refusing to advance until the previous is provably complete. Research: Remus live migration (snapshot copy → async propagation → sync mode change → dual execution → ownership transfer; durability before ownership); Zaris cut-over protocol (revision-anchored snapshot, write fence then final delta, content-digest proof of equality, target dominates source and all live replicas before flip); system-design resharding (close old gate, catch new up to exact final position F, validate contents match, activate new owner at new epoch, publish conditionally); fencing tokens (monotonic epoch, old owner rejected at the resource boundary). Design implication: `ids-mobility` implements a fenced handoff state machine — snapshot → catch-up → fence → final-delta → digest proof → atomic epoch cut-over → old-owner fence → safety window → optional physical purge. Generations are strictly increasing and never reused; a fenced old owner can still sign appends to its own closed epoch but those appends are post-close evidence, not state.

#### 3.6 Cutover verification

Cutover is gated on verification, not on the absence of errors. Research: row counts + per-range checksums (Shard Growth 20→200); shadow reads (read both source and target, serve source, log mismatches — caught 47 query incompatibilities in one production migration); checksum/invariant/boundary validation (getrightdata checklist); Merkle-tree anti-entropy to locate divergent keys (Cassandra/Dynamo anti-entropy); go/no-go sign-off gate with named owners and recorded results (Dynamics 365 cutover checklist). Design implication: `ids-mobility` exposes a `CutoverVerification` API producing (a) per-range checksum comparison source vs target at the same boundary, (b) shadow-read mismatch rate over a configurable traffic sample, (c) business-rule invariant results, (d) a signed cut-over proof (content digest + epoch) that the atomic routing update references. Cutover proceeds only when all gates pass; otherwise abort and keep the old owner authoritative.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Split-brain: both old and new region accept writes during handoff | Monotonic epoch fencing; old owner's writes rejected at the resource boundary once the new epoch is published; exactly one authoritative region at a time |
| Resurrected deleted data (zombie records) | Tombstones migrate with the copy; content-stripped tombstones during paginated copy; tombstone GC only after all replicas ack past the cut-over position |
| Stale read from a not-yet-caught-up target | Target does not serve reads until catch-up reaches the fence position; owner-first reads with prior-owner probe during the cooling-off window |
| Data loss from a write landing between snapshot and fence | Final delta replayed after the fence; cut-over proof digest must equal source+all-live-replicas digest before flip |
| Access policy drift / silent privilege change at boundary | Policy snapshot versioned with the region epoch; causal replication of policy changes; target never authorizes what source would deny at same position |
| Rollback losing post-cutover writes | Old owner kept read-only as recovery evidence through a safety window; reverse replication prepared; physical purge only if digest still matches the cut-over proof |
| Metadata bomb / oversized migration package | Size/depth budgets on the migration manifest; decompression bounds before parsing; per-peer quotas |
| Replay of an old handoff message | Monotonic generation + nonce replay cache with TTL; signed handoff messages |
| Long-running transactions blocked during handoff | Ordered diversion / MVCC dual execution so existing source transactions complete while new ones route to target |

---

### 5. Integration with Existing Crates

`ids-mobility` depends on and integrates with:

| Crate | Role in Phase 038 |
|-------|-------------------|
| `ids-000-aa` | `NodeId`, `RegionId`, `PartitionId`, `RecordId`, `Endpoint`, `CarrierType`, sealed trait contracts, invariants, error contracts — never redefined here |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — all mobility traffic is opaque datagrams; no direct carrier imports |
| `ids-crypto` | ed25519 signatures, xchacha20poly1305/chacha20poly1305 encryption, sha256/blake3 hashing — allowed primitives only |
| `ids-store` | Mutation issuer vetoed/stamped during handoff; source of the dataset/partition being moved |
| `ids-fragment` | Fragment-level copy and tombstone handling during partition moves |
| `ids-replication` | Catch-up stream source; anti-entropy repair consumer after region change |
| `ids-reclaim` (Phase 041) | Physical purge candidate after safety window, gated on digest-proof match |
| `ids-immutable` (Phase 040) | Retention/legal-hold enforcement must survive the region change; holds ported with the data |

**Integration points:**
- `ids-store`/`ids-fragment` consult `ids-mobility`'s `RegionAuthority` before accepting writes; a fenced region returns `STALE_ROUTE`/`RegionFenced`.
- Catch-up stream rides `ids-carrier::Endpoint` datagrams; signatures over canonical payloads survive re-framing by any carrier.
- `ids-replication` performs Merkle-digest anti-entropy between source and target during catch-up to locate divergent keys in O(d·log n).
- `ids-reclaim` only physically purges the old copy after the safety window and only if the current digest still equals the cut-over proof.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-mobility` (new separate crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; serde, thiserror, bytes, blake3)

```
ids-mobility/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol defaults (epoch, safety window, quotas)
├── error.rs                        # MobilityError (thiserror, comprehensive)
├── features.rs                     # Feature flags (durable, metrics, fuzzing)
│
├── authority/                      # Versioned region-authority record (source of truth)
│   ├── mod.rs
│   ├── region_authority.rs         # RegionAuthority: dataset/partition → (region, epoch, state)
│   ├── placement.rs                # Placement record per range/dataset
│   ├── epoch.rs                    # Monotonic, never-reused region epoch
│   ├── preferred_region.rs         # Preferred/authoritative region designation
│   ├── routing_cache.rs            # Stale-route redirect/rejection handling
│   ├── authority_types.rs          # AuthorityId, Epoch, AuthorityState structures
│   ├── authority_proptests.rs      # Property tests: epoch monotonicity, single-owner invariant
│   └── authority_fuzz.rs           # Fuzzing for authority state transitions
│
├── handoff/                        # Controlled handoff state machine
│   ├── mod.rs
│   ├── handoff_machine.rs          # State machine: pending/snapshot/catch-up/fenced/cut-over/rollback
│   ├── phases.rs                   # Individual handoff phases with pre/post conditions
│   ├── transition_guard.rs         # Refuses to advance until previous phase provably complete
│   ├── ticket.rs                   # Handoff ticket: dataset/partition + generation (strictly increasing)
│   ├── generation.rs               # Monotonic generation, never reused
│   ├── handoff_types.rs            # HandoffId, Phase, TransitionDecision structures
│   ├── handoff_proptests.rs        # Property tests: phase ordering safety
│   └── handoff_fuzz.rs             # Fuzzing for state-machine races
│
├── snapshot/                       # Revision-anchored consistent snapshot
│   ├── mod.rs
│   ├── snapshotter.rs              # Takes snapshot at cut-over position R
│   ├── consistent_cut.rs           # Chandy-Lamport-style consistent cut across replicas
│   ├── snapshot_id.rs              # SnapshotId derivation
│   ├── manifest.rs                 # Snapshot manifest: key ranges, counts, digests
│   ├── snapshot_types.rs           # Snapshot, SnapshotPosition structures
│   └── snapshot_proptests.rs       # Property tests: cut consistency
│
├── catchup/                        # Mutation catch-up stream after snapshot
│   ├── mod.rs
│   ├── catchup_stream.rs           # Replays post-snapshot mutations in order, tombstones included
│   ├── cdc_source.rs               # Change-data-capture source from source region log
│   ├── replay.rs                   # Ordered, idempotent replay at target
│   ├── lag_tracker.rs              # Catch-up lag vs fence position
│   ├── catchup_types.rs            # CatchUpPosition, DeltaEntry structures
│   └── catchup_proptests.rs        # Property tests: replay idempotence, ordering
│
├── fence/                          # Write fencing of the old owner
│   ├── mod.rs
│   ├── epoch_fence.rs              # Monotonic epoch bump; old owner rejected
│   ├── fence_gate.rs               # Ownership gate: open/closed per region per dataset
│   ├── stale_route.rs              # STALE_ROUTE error for fenced-region requests
│   ├── fence_types.rs              # FenceToken, GateState structures
│   └── fence_proptests.rs          # Property tests: fence rejects lower epochs
│
├── digest/                         # Content-digest cut-over proof
│   ├── mod.rs
│   ├── content_digest.rs           # Digest of dataset/partition state at a boundary
│   ├── proof.rs                    # CutOverProof: digest + epoch + source + target
│   ├── merkle.rs                   # Merkle tree for divergence location (O(d·log n))
│   ├── digest_compare.rs           # Source vs target digest comparison at same boundary
│   ├── digest_types.rs             # Digest, ProofStatus structures
│   └── digest_proptests.rs         # Property tests: digest equality ⇒ state equality
│
├── identity/                       # Logical identity preservation
│   ├── mod.rs
│   ├── stable_id.rs                # Region-opaque DatasetId/PartitionId (BLAKE3-derived)
│   ├── id_mapping.rs               # Logical-id → physical-region mapping (updated at cutover)
│   ├── identity_types.rs           # DatasetId, PartitionId, IdentityRecord structures
│   └── identity_proptests.rs       # Property tests: id stability across move
│
├── history/                        # History preservation (changelog + tombstones)
│   ├── mod.rs
│   ├── changelog.rs                # Append-only mutation log copied with the data
│   ├── tombstone.rs                # Tombstone carry-along; GC at causal stability
│   ├── lineage.rs                  # History lineage record across region moves
│   ├── history_types.rs            # ChangeLogEntry, TombstoneRecord structures
│   └── history_proptests.rs        # Property tests: no resurrection after GC
│
├── policy/                         # Access-policy portability
│   ├── mod.rs
│   ├── policy_snapshot.rs          # Versioned policy snapshot ported with the data
│   ├── grant_portability.rs        # RBAC/ABAC grant re-instantiation at target
│   ├── causal_policy.rs            # Causal replication of policy changes during the move
│   ├── policy_types.rs             # Grant, PolicyVersion, PolicyPosition structures
│   └── policy_proptests.rs         # Property tests: no silent privilege change
│
├── cutover/                        # Cutover verification & atomic routing update
│   ├── mod.rs
│   ├── cutover_verifier.rs         # Verification: checksums, shadow reads, business rules
│   ├── shadow_read.rs              # Shadow-read validation (read both, serve source, log diffs)
│   ├── checksum_compare.rs         # Per-range checksum comparison source vs target
│   ├── business_rules.rs           # Domain invariant validation
│   ├── go_no_go.rs                 # Sign-off gate with named owners, recorded results
│   ├── routing_update.rs           # Atomic compare-and-swap publishing new authoritative region
│   ├── cutover_types.rs            # CutoverDecision, VerificationResult structures
│   └── cutover_proptests.rs        # Property tests: cutover gated on verification
│
├── safety_window/                  # Old-owner recovery evidence retention
│   ├── mod.rs
│   ├── safety_window.rs            # Read-only retention period post-cutover
│   ├── reverse_replication.rs      # Prepared reverse sync for rollback
│   ├── purge_gate.rs               # Physical purge only if digest still matches proof
│   ├── safety_types.rs             # SafetyWindow, PurgeDecision structures
│   └── safety_proptests.rs         # Property tests: purge safety invariant
│
├── anti_entropy/                   # Post-move divergence repair
│   ├── mod.rs
│   ├── merkle_anti_entropy.rs      # Merkle digest exchange to locate divergent keys
│   ├── repair.rs                   # Targeted sync of divergent ranges
│   ├── anti_entropy_types.rs       # DigestRange, RepairPlan structures
│   └── anti_entropy_proptests.rs   # Property tests: convergence after repair
│
├── carrier/                        # Carrier-agnostic transport binding
│   ├── mod.rs
│   ├── endpoint_binding.rs         # ids-carrier::Endpoint datagram framing
│   ├── envelope.rs                 # Framing-independent signed payload carrier
│   └── carrier_types.rs            # MobilityEnvelope structures
│
├── config/                         # Configuration
│   ├── mod.rs
│   ├── handoff_config.rs           # Handoff timeouts, safety window, quotas
│   ├── catchup_config.rs           # Catch-up stream configuration
│   └── verification_config.rs      # Cutover verification thresholds
│
├── metrics/                        # Metrics & tracing
│   ├── mod.rs
│   ├── handoff_metrics.rs          # Handoff phase duration, success/failure
│   ├── catchup_metrics.rs          # Lag, replay rate
│   └── verification_metrics.rs     # Shadow-read mismatch rate, checksum results
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_handoff.rs
│   ├── fuzz_catchup.rs
│   ├── fuzz_cutover.rs
│   └── fuzz_fence.rs
│
├── tests/                          # Integration & adversarial tests
│   ├── mod.rs
│   ├── handoff_tests.rs            # Full state-machine execution
│   ├── snapshot_tests.rs           # Consistent cut round-trip
│   ├── catchup_tests.rs            # Replay ordering + idempotence
│   ├── fence_tests.rs              # Old-owner fencing enforcement
│   ├── digest_tests.rs             # Cut-over proof equality
│   ├── identity_tests.rs           # Logical id stability across move
│   ├── history_tests.rs            # Tombstone carry + no resurrection
│   ├── policy_tests.rs             # Policy portability
│   ├── cutover_tests.rs            # Verification gates, go/no-go
│   ├── safety_tests.rs             # Safety window + purge gate
│   ├── anti_entropy_tests.rs       # Divergence repair convergence
│   ├── adversarial_tests.rs        # Split-brain, replay, stale reads
│   ├── determinism_tests.rs        # Same seed → identical handoff outcome
│   ├── property_tests.rs           # Proptest properties
│   └── replay_tests.rs             # Handoff message replay attack tests
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── handoff_bench.rs            # Handoff latency
│   ├── catchup_bench.rs            # Catch-up throughput
│   ├── digest_bench.rs             # Merkle digest comparison
│   └── verification_bench.rs       # Cutover verification cost
│
├── spec/                           # Normative specifications
│   ├── handoff_spec.md             # Handoff state machine & phase contract
│   ├── cutover_spec.md             # Cutover verification & routing update spec
│   ├── fence_spec.md               # Epoch fencing spec
│   ├── policy_portability_spec.md  # Access-policy portability spec
│   └── threat_model.md             # Threat model document
│
├── docs/                           # Documentation
│   ├── architecture.md             # Architecture & layering
│   ├── integration_guide.md        # Integrating with ids-store/ids-fragment/ids-replication
│   ├── migration_runbook.md        # Operational runbook for region changes
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── handoff_vectors.json
    ├── cutover_proof_vectors.json
    ├── catchup_vectors.json
    └── safety_window_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — authority/epoch, handoff state machine, snapshot, catch-up, fencing, digest proof, identity, history/tombstones, policy portability, cutover verification, safety window, anti-entropy, carrier binding, tests, fuzz, benchmarks, specs, docs — reflects the genuine state-machine and storage complexity of controlled region handoff. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Region authority & monotonic epoch

```
RegionAuthority { dataset_id, partition_id, authoritative_region, epoch, state, preferred_region }
Epoch: u64, strictly increasing, never reused
state ∈ {Authoritative, Preferred, Fenced, Retired}
```

Every request carries the epoch; a region refuses requests with a stale epoch. Cutover is an atomic compare-and-swap on `RegionAuthority` gated on verification.

#### 7.2 Handoff state machine

```
pending → snapshot → catch-up → fenced → cut-over → (rollback | retired)
```

Each phase advances only when its preconditions are provably satisfied (durable migration state, not inferred from files on disk). A crashed controller resumes from durable migration state.

#### 7.3 Revision-anchored snapshot + catch-up

```
snapshot at cut-over position R (consistent cut, Chandy-Lamport)
bulk copy streams snapshot while source remains authoritative
catch-up replays mutations after R in order, tombstones included
fence first, then measure final position F (no write can slip past the measurement)
target must catch up to exactly F before activation
```

#### 7.4 Cut-over proof (content digest)

```
CutOverProof { source_digest, target_digest, epoch, source_region, target_region, position F }
flip only when source_digest == target_digest AND target dominates source and all live replicas
```

#### 7.5 Epoch fencing

```
fence(epoch η′): require η′ > current_epoch; advance atomically; reject all mutations carrying η < η′
old owner fenced but kept read-only as recovery evidence through the safety window
```

#### 7.6 Tombstone / history preservation

Tombstones migrate with the data; a tombstone is GC-able only after its removal is causally stable (all replicas ack past it), preventing zombie resurrection.

#### 7.7 Access-policy portability

```
PolicySnapshot { version, grants: [(subject, object, op, position)], region_epoch }
grants re-instantiated at target; policy changes replicated causally during the move
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | NodeId, RegionId, PartitionId, RecordId, Endpoint, CarrierType, sealed contracts, invariants, errors |
| `ids-carrier` | `Endpoint` datagram transport (CarrierType + Vec<u8>) |
| `ids-crypto` | ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 (allowed primitives only) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Hashing (workspace dependency) |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS) — all traffic via `ids-carrier::Endpoint`. No X25519, no AES-GCM, no RSA — only the five allowed primitives.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement Method |
|---|-----------|-------------------|
| AC-038.1 | Single authoritative region at any time | Exactly one `RegionAuthority` entry per dataset/partition; concurrent writers to two regions rejected |
| AC-038.2 | Logical identity preserved | `DatasetId`/`PartitionId` unchanged across the move; no re-issuance |
| AC-038.3 | History preserved | Tombstones + full changelog copied; no delete resurrected after cutover |
| AC-038.4 | Access policy preserved | Equivalent policy snapshot active at target; no privilege elevation/drop at boundary |
| AC-038.5 | Controlled handoff | State machine phases advance only when preconditions provably complete; crash recovery from durable state |
| AC-038.6 | Old-owner fenced | Writes to fenced region rejected with STALE_ROUTE; monotonic epoch enforced |
| AC-038.7 | Cut-over proof | Source and target content digests equal at position F before flip; target dominates all live replicas |
| AC-038.8 | Cutover verification | Checksums + shadow reads + business rules all green before routing update |
| AC-038.9 | Safety window | Old owner kept read-only post-cutover; physical purge only if digest still matches proof |
| AC-038.10 | Anti-entropy convergence | Merkle-digest repair converges source and target after region change |
| AC-038.11 | Carrier agnosticism | Compiles and behaves identically with zero direct deps on TCP/IP/UDP/QUIC/HTTP/DNS/libp2p; all traffic via `ids-carrier::Endpoint` |
| AC-038.12 | Determinism | Same seed + migration inputs → identical handoff outcome and cut-over proof |

---

### 10. Remaining Uncertainties

1. **Catch-up mechanism:** pull-based CDC stream vs push-based dual-write vs change-stream from `ids-replication` log — decision depends on what `ids-replication` exposes; all three must be supported behind one interface.
2. **Safety window duration:** how long the old owner is kept read-only as recovery evidence; workload- and compliance-dependent, unquantified.
3. **Physical purge criteria:** purge only if digest still matches the cut-over proof; the observation period and rollback decision point are product decisions.
4. **Policy portability granularity:** which access-policy fields are portable verbatim vs need re-mapping at the target; deferred to policy owners.
5. **Preferred vs authoritative timing:** how long a region may remain "preferred" before cutover or cancellation; no hard limit decided yet.
6. **Anti-entropy after move:** whether Merkle repair runs continuously or on-demand post-cutover; frequency and scope undecided.
7. **Integration with `ids-reclaim` (Phase 041):** physical purge API is provisional pending that crate's design.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-mobility` | IDS convention: separate crates for new capabilities, not more folders in existing ones |
| Versioned `RegionAuthority` as single source of truth | Routing authority placement identifies allowed write owner per range (Shard Growth 20→200) |
| Monotonic, never-reused epoch + fencing token | Prevents split-brain; stale owner rejected at resource boundary (fencing-token pattern) |
| Revision-anchored snapshot + ordered catch-up, tombstones included | Copy-then-replay catches mutations during the move; tombstones prevent zombie resurrection (Remus, Zaris, Cassandra) |
| Fence first, then measure final position | No write slips past the measurement; durability before ownership |
| Cut-over proof = content-digest equality (source + all live replicas) | Ownership is a reward for provable possession, not a precondition (Zaris cut-over protocol) |
| Atomic routing update gated on verification | Cutover proceeds only when checksums + shadow reads + business rules pass |
| Old owner kept read-only through safety window | Rollback target; physical purge gated on digest-proof match |
| Policy snapshot versioned with region epoch | Access-policy portability; identity separate from network location (NIST 800-207A, authorization portability) |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic; no TCP/IP/UDP/QUIC/HTTP/DNS/libp2p |
| Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 | HX-241/IDS allowed-crypto policy |

---

### Sources

- Shard Growth 20→200 (routing authority, controlled handoff, safety window): https://system-design-atlas.com/topics/shard-growth-20-200.html
- Live Cross-Region Data Migration four-phase protocol (fencing by class, tombstones, atomic cutover): https://doi.org/10.20944/preprints202610.0828.v1
- Blue-green deployment & rollback (Harness): https://harness.io/harness-devops-academy/what-is-a-blue-green-deployment
- Shadow table strategy for zero-downtime migration (InfoQ): https://infoq.com/articles/shadow-table-strategy-data-migration
- Zero-downtime data migration: dual-write, backfill, CDC, shadow reads, cutover: https://youngju.dev/blog/database/2026-06-16-zero-downtime-data-migration-dual-write-cdc.en
- Dual-write and backfill migration (sixteenpillars): https://sixteenpillars.com/dual-write-and-backfill-migrating-live-data-without-a-freeze
- Remus live migration for distributed databases (snapshot → async propagation → dual execution → ownership transfer): https://doi.org/10.1145/3514221.3526047
- skaidb resharding (tombstone migration, reclaim ack-gate): https://skaidb.org/docs/resharding.html
- Koala non-disruptive reconfiguration (ownership transfer decoupled from state location): https://arxiv.org/html/2610.28939
- Fencing tokens / monotonic epochs (split-brain prevention): https://systemoverflow.com/learn/distributed-primitives/distributed-locks/fencing-tokens-preventing-split-brain-operations
- Fencing in distributed systems (Twitter-style monotonic tokens): https://positioniseverything.net/fencing-in-distributed-systems-twitters-approach
- Cassandra tombstones & gc_grace_seconds (zombie prevention): https://cassandra.apache.org/doc/latest/cassandra/managing/operating/compaction/tombstones.html
- Anti-entropy Merkle trees (Cassandra/Dynamo divergence repair): https://systemoverflow.com/learn/distributed-primitives/gossip-protocol/anti-entropy-digests-and-reconciliation-healing-divergent-state-in-gossip-systems
- Consistent cut / Chandy-Lamport distributed snapshot: https://cs.brown.edu/courses/cs138/s16/lectures/10globalstate-notes.pdf
- NIST SP 800-207A zero-trust (policy by identity, not network location): https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-207A.pdf
- Authorization portability: https://nhimg.org/glossary/authorization-portability
- Multi-region authorization without inconsistent decisions: https://nhimg.org/faq/how-should-security-teams-implement-multi-region-authorization-without-creating
- StableID persistent entity identification: https://doi.org/10.3390/industries1010003
- Source Known Identifiers (three-tier identity): https://arxiv.org/html/2604.00151v1
- GDPR international transfers / data residency (ICO): https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/international-transfers/a-guide-to-international-transfers/
- Data migration testing cutover checklist (getrightdata): https://getrightdata.com/resources/blog/data-migration-testing-checklist
- Dynamics 365 go-live/cutover checklist (sign-off gate): https://learn.microsoft.com/en-us/dynamics365/guidance/implementation-guide/prepare-go-live-checklist
- OceanBase tree-2PC (partition transfer during transactions): https://arxiv.org/html/2603.00866
- OR-Set CRDT add-wins semantics & tombstone GC at causal stability: https://cs.cmu.edu/~aldrich/courses/17-350/slides/10-crdts.pdf
- Data sovereignty / residency controls (AWS Well-Architected): https://docs.aws.amazon.com/pdfs/wellarchitected/latest/digital-sovereignty-lens/digital-sovereignty-lens.pdf
- Handoff machines Rust crate (partition ownership transfer, fencing tokens): https://docs.rs/handoff-machines/latest/handoff_machines

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 038.
