## Phase 037 — Create the ids-hotcold crate: Research Findings & Implementation Blueprint

**Status:** Research complete (30 targeted searches executed). No code written, no files modified. This document is the blueprint for implementation.

---

### 1. Executive Summary

Phase 037 creates **`ids-hotcold`**, a new separate crate for the IDS project that implements adaptive hot/cold/warm storage tiering: data movement among placement classes (tiers) driven by observed access patterns. The crate owns tier placement state (hot/warm/cold), access-pattern tracking (temperature estimation), promotion and demotion decisions, latency objectives, cost optimization, integration with lifecycle policies, logical identity preservation during tier movement, and destination verification of moved data. It is carrier-agnostic: zero direct networking dependencies. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — nothing else. `ids-000-aa` is the master crate holding all canonical types and sealed trait contracts; `ids-hotcold` builds on those types, never redefining them.

**Scope of `ids-hotcold` (research-verified):**
- **Three placement classes:** hot tier (low latency, high cost, frequently accessed), warm tier (moderate latency/cost, infrequently accessed), cold tier (highest latency, lowest cost, rarely accessed but retained).
- **Access pattern tracking:** per-object/data-unit temperature estimation from recency, frequency, and size of accesses; exponential moving average cooling; dual frequency + momentum counters for fast adaptation to hotness changes.
- **Promotion/demotion:** threshold-based and ML-informed upgrade/downgrade decisions with hysteresis to prevent thrashing (ping-pong migration); four decision points (when to start, which object, how to move, when to stop).
- **Latency objectives:** per-tier read/write latency targets; migration scheduled so movement overhead does not exceed the latency savings it buys.
- **Cost optimization:** storage cost × time × replica count, per-GB retrieval, per-request, and bandwidth costs minimized by aligning data with the cheapest tier that meets its latency SLO.
- **Lifecycle policy integration:** time-based and access-based transitions (e.g., cool after N idle days, cold after M more days, auto-promote back to hot on re-access), compatible with external lifecycle rules.
- **Logical identity preservation:** an object's `RecordId`/`DatasetId`/`PartitionId` identity is invariant across tier movement; movement is a placement change, not an identity change.
- **Destination verification:** post-move consistency validation in three dimensions (object count, content checksum, metadata) before source copy is released; fail-closed on mismatch.
- **Carrier-agnostic:** all data movement framed over `ids-carrier::Endpoint`; signatures over canonical payloads survive re-framing by any carrier.
- **Allowed crypto only:** ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.

**Key engineering decisions:**
- New separate crate `ids-hotcold` (not more folders inside existing crates), per the IDS project convention.
- Temperature = EMA of access frequency + momentum (fast/slow dual counters, per HybridTier/FreqTier research), cooled periodically to forget stale hotness.
- Hysteresis between promotion and demotion thresholds to prevent ping-pong migration under boundary workloads.
- Move only when expected latency savings exceed migration cost (cost-benefit gate from Lustre LCMP quantitative analysis).
- Identity invariant: `RecordId` unchanged across tiers; tier is a placement attribute stored separately.
- Destination verification: SHA-256/BLAKE3 content checksum + object-count consistency; fail closed, keep source until destination verified.
- Move planned as copy-then-verify-then-delete-source, with resume support via persisted progress (seed/vector style).
- All movement traffic framed over `ids-carrier::Endpoint`; ed25519 signatures over canonical movement manifests.

---

### 2. Architecture Overview

`ids-hotcold` sits above the storage producers (hot/warm/cold tier backends) and below the lifecycle/scheduling consumers. It is not a network protocol — it is a local placement + movement engine that emits bytes (movement plans, manifests, proofs) to be shipped over `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / DATA LAYER                          │
│         objects, records, datasets, partitions (ids-000-aa types)    │
├──────────────────────────────────────────────────────────────────────┤
│                    Phase 037: ids-hotcold (NEW CRATE)                │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────────────┐   │
│  │  Placement     │  │  Temperature   │  │  Promotion/Demotion  │   │
│  │  State (hot/   │  │  Tracking      │  │  Engine              │   │
│  │   warm/cold)   │  │  (EMA + momentum│ │  (hysteresis,        │   │
│  │  per RecordId  │  │   dual counter)│  │   cost-benefit gate) │   │
│  └───────┬────────┘  └──────┬─────────┘  └──────────┬───────────┘   │
│          │                 │                        │                │
│  ┌───────▼─────────────────▼────────────────────────▼─────────┐   │
│  │  Movement Planner / Executor                              │   │
│  │  (copy→verify→delete-source, resume, progress vector)     │   │
│  └───────┬───────────────────────────────────────────┬────────┘   │
│          │                                           │            │
│  ┌───────▼──────────────────────┐  ┌─────────────────▼────────┐  │
│  │  Destination Verification    │  │  Lifecycle Policy        │  │
│  │  (count, sha256/blake3, meta)│  │  Integration               │  │
│  └───────┬──────────────────────┘  └─────────────────┬────────┘  │
│          │                                           │            │
│  ┌───────▼───────────────────────────────────────────▼────────┐  │
│  │  Wire: signed movement manifests over ids-carrier::Endpoint │  │
│  │  (ed25519 signature; CarrierType + Vec<u8>)               │  │
│  └─────────────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────┤
│                  ids-carrier (Endpoint, Datagram)                    │
│           CarrierType + Vec<u8>, opaque datagrams                    │
├──────────────────────────────────────────────────────────────────────┤
│             ids-000-aa (canonical types, sealed contracts)           │
│         NodeId, RegionId, RecordId, DatasetId, PartitionId,          │
│         Endpoint, CarrierType, PlacementDecision, …                  │
└──────────────────────────────────────────────────────────────────────┘
```

**Data flow — access tracking:** On every read/write of a data unit, the access is recorded (recency timestamp + frequency increment + size). Periodically, temperature counters are cooled (EMA decay). Each data unit's tier is derived from temperature vs. hot/warm/cold thresholds.

**Data flow — promotion/demotion:** The engine evaluates each data unit against tier thresholds with hysteresis. A promotion (cold→warm→hot) or demotion (hot→warm→cold) is scheduled only when the cost-benefit gate (expected latency savings > migration cost) passes and tier capacity permits. The movement planner issues a copy operation over `ids-carrier::Endpoint`; after copy, destination verification runs; on success the source copy is released and placement state updated.

**Logical identity preservation:** The `RecordId` (and parent `DatasetId`/`PartitionId`) is carried unchanged in the movement manifest; tier is an attribute, not part of identity. Consumers address data by `RecordId` regardless of tier.

---

### 3. Detailed Research Dimensions

#### 3.1 Placement Classes — Hot / Warm / Cold

**Hot tier:** Holds the most-recent, most-frequently-accessed data. Optimized for low-latency read/write; fastest media, highest cost per byte, typically replicated for resiliency. New writes land here by default. (Elasticsearch hot tier model: entry point, most-recent/most-frequently-searched, SSDs, replicas.)

**Warm tier:** Holds data accessed less frequently than hot data but still queried; updates allowed but infrequent. Moderate hardware, lower cost than hot. In Elasticsearch, warm tier holds recent-weeks time-series data; searchable snapshots can cut disk ~50% vs regular indices.

**Cold tier:** Holds infrequently accessed, rarely updated data retained for compliance/analytics. Lowest cost, highest latency. Fully mounted searchable indices (no replicas) reduce space ~50%. Azure smart tier: hot→cool after 30 idle days, cool→cold after 60 more; re-access promotes back to hot immediately.

**Granularity:** Tiering can operate at object/file, block, page, or segment granularity. Object/file granularity (S3, Azure Blob) is simplest and matches `RecordId`-addressed IDS data units; page/segment granularity (ReStore, MOST) is finer and suited to DBMS/KV workloads but higher metadata overhead.

#### 3.2 Access Pattern Tracking & Temperature Estimation

Temperature reflects popularity/heat over a time granularity. Research-verified inputs:
- **Recency:** time of last access (all file systems track last-access time).
- **Frequency:** number of accesses within a window; maintained as last-k access times plus count.
- **Size:** object size affects move cost and cost-benefit.
- **Cooling:** periodic decay so stale hotness is forgotten (EMA with cooling period C; FreqTier/Memtis cool counters by dividing by 2 each period).
- **Dual metric (frequency + momentum):** HybridTier/FreqTier maintain two EMA counters per object — frequency (minutes-to-hours, long-term hotness) and momentum (seconds, short-term intensity). Promote on high frequency OR high momentum; demote only on low frequency AND low momentum; low-frequency/high-momentum gets a second chance. This adapts quickly to hotness changes while preserving long-term distribution.
- **Metadata overhead:** Exact per-object counters in a hash table give O(1) lookup but scale linearly with object count; for millions/billions of objects, approximate structures (counting Bloom filters) or sampling are needed. Lustre LCMP found in-memory LRU/heap inapplicable at billion-file scale — full tier scans required otherwise.

#### 3.3 Promotion / Demotion Algorithms

Four decision points (Herodotou et al., PVLDB 2019): (1) when to start downgrade/upgrade, (2) which object to move, (3) how to move it, (4) when to stop.

Policy menu (research-verified):
- **LRU:** downgrade least-recently-used; upgrade on access (on-single-access).
- **LFU:** downgrade least-frequently-used.
- **LRFU:** combined recency+frequency weight `W = 1 + H·W·decay + H(1−Δt)` (half-life H).
- **EXD (Exponential Decay):** weight `W = 1 + W·e^(−α·Δt)`, α tuned; downgrade lowest weight.
- **XGB / ML:** gradient-boosted-tree classifier predicts near-future access; upgrade/downgrade by probability threshold.
- **RL (ReStore):** MDP per tier; state = tier temperature + pressure; action = maintain or promote; reward = negative response time; adapts to workload drift and SSD asymmetry.

**Hysteresis:** Separate promotion and demotion thresholds (e.g., promote at hotness ≥ T_high, demote at ≤ T_low, T_high > T_low) prevent ping-pong migration when an object hovers near a threshold. Lustre LCMP principle: "Only evict data when necessary; evict as little as possible."

**Cost-benefit gate:** A move is worthwhile only if `Access[cold]/Perf[upper] + Access[hot]/Perf[lower] − Overhead` yields net saved time (Lustre quantitative analysis). Migration overhead (bandwidth, write amplification, device wear) must be less than the latency savings.

#### 3.4 Latency Objectives

Each tier carries a latency SLO (hot: sub-millisecond to low-ms; warm: tens of ms; cold: seconds to minutes, e.g., archive retrieval). Migration is throttled so it does not saturate the tier serving live traffic. MOST-style dynamic sizing: under light load, keep more data on the performance device; under intense load, spill to capacity device to balance load. Cost model accounts for read/write asymmetry (SSD writes slower than reads) and device concurrency.

#### 3.5 Cost Optimization

Cost components: storage cost ($/GB/month), transaction cost (per read/write request), retrieval cost (per GB fetched from cold/archive), bandwidth cost, and energy/byte. Goal: minimize total cost subject to per-object latency SLO. Commercial evidence:
- Azure Smart Tier: automatically moves objects hot→cool (30 idle days)→cold (60 more days); >50% of managed capacity shifted to cooler tiers based on actual access patterns; no tier-transition or retrieval fees, only a monitoring fee.
- S3 Intelligent-Tiering: monitors each object, moves among Frequent Access / Infrequent Access (30 days) / Archive Instant Access (90 days); no retrieval fees; 128 KB minimum billable size; objects <128 KB not auto-tiered.
- S3 Glacier Instant Retrieval: millisecond access, up to 68% cheaper than S3 Standard-IA for quarterly access, 90-day minimum duration.
- Evidence-based lifecycle: once access age is known, fixed lifecycle rules beat generic intelligent tiering; set transition age conservatively to avoid retrieval-fee risk from brief re-access spikes.

#### 3.6 Lifecycle Policy Integration

Lifecycle rules express time-based and access-based transitions:
- `tierToCool` after `daysAfterLastAccessTimeGreaterThan` N
- `tierToCold` after `daysAfterModificationGreaterThan` M
- Auto tier back to hot on re-access (limited frequency to avoid thrash/fees)
- Filter by blob type, prefix/path; one action per rule scope.

ids-hotcold exposes a policy engine that consumes lifecycle rules and emission of movement intents, integrated with ids-lifecycle (if present) or standalone. Conditions assessed once per policy run; movements queued in a subsequent run.

#### 3.7 Logical Identity Preservation During Tier Movement

An object's `RecordId` is invariant across tier changes. Tier is a placement attribute stored separately from identity and content. Movement manifest carries `RecordId`, source tier, destination tier, size, content checksum, and expected placement attributes. Consumers continue to address data by `RecordId`; the placement layer resolves the current tier at read time. This matches Azure NetApp Files cool access: tiered blocks remain online/transparent to the application.

#### 3.8 Destination Verification

Move is not complete until destination is verified. Three dimensions (Alibaba Cloud Data Online Migration; AWS DataSync; Google Storage Transfer Service):
1. **Object count consistency:** source file set == destination file set, no missing objects.
2. **Content consistency:** per-object checksum match (SHA-256/BLAKE3; MD5 only for single-part uploads; multipart ETag is not a content hash).
3. **Metadata consistency:** supported metadata entries consistent between source and destination.

Validation logic: full key listing from each side compared as sets → keys-only-source, keys-only-destination, size mismatches. Fail closed: source copy retained until destination verified; on mismatch, re-transfer affected objects. Sampling strategy (1–5%) acceptable only for TB/PB-scale where full validation is prohibitive; core business data fully validated.

#### 3.9 Movement Execution & Resilience

Copy-then-verify-then-delete-source ordering prevents data loss. Progress persisted as a resumable seed/vector (perfect hash vector over segment fingerprints) so interrupted moves resume without re-copying completed segments. Concurrent moves bounded by bandwidth budget; migration traffic shaped to avoid saturating live-request paths. Idempotent movement manifests so redelivery over `ids-carrier::Endpoint` is safe.

#### 3.10 Convergence & Safety Properties

- **Identity invariance:** `RecordId` unchanged across any number of tier moves.
- **Placement convergence:** repeated evaluation converges to a stable tier assignment (no oscillation) under fixed workload and thresholds.
- **No data loss:** source released only after destination checksum-verified.
- **No split-brain placement:** exactly one tier is authoritative for a `RecordId` at any time; movement transitions atomically in placement state.
- **Carrier agnosticism:** behavior identical regardless of underlying carrier; all bytes opaque `Vec<u8>`.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Ping-pong migration (object oscillates near tier threshold) | Hysteresis: separate promotion/demotion thresholds; cooldown period after a move ||
|| Move costs more than latency saved | Cost-benefit gate: evaluate expected savings vs. migration bandwidth/write cost before scheduling ||
|| Data loss during move | Copy→verify→delete-source; source retained until destination checksum-verified ||
|| Corrupted destination after move | SHA-256/BLAKE3 content checksum verification; fail closed, re-transfer on mismatch ||
|| Forged/tampered movement manifest | ed25519 signature over canonical manifest; unsigned manifests dropped ||
|| Replay of old movement manifest | `(RecordId, tier_seq, origin)` monotonically increasing; stale/duplicate rejected ||
|| Metadata not preserved across move | Metadata consistency dimension in destination verification; copy full attribute set ||
|| Object count mismatch (omitted objects) | Full key-listing set comparison; count consistency checked first, decisive ||
|| Metadata overhead at scale (millions of objects) | Approximate counters / sampling / tier-granular batching; full scan avoided via hierarchical temperature index ||
|| Migration traffic saturates live path | Bandwidth budget + throttle moves; schedule during low-load windows; per-move concurrency cap ||
|| Carrier-agnostic violation (direct socket use) | All traffic framed over `ids-carrier::Endpoint`; compile-time dependency audit ||
|| Disallowed crypto | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; no X25519/AES-GCM/RSA ||

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 037 ||
||-------|-------------------||
|| `ids-000-aa` | `NodeId`, `RegionId`, `RecordId`, `DatasetId`, `PartitionId`, `Endpoint`, `CarrierType`, `PlacementDecision`, `IdsTime`, sealed trait contracts, versioning ||
|| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) is the sole transport for movement manifests and verification proofs ||
|| `ids-crypto` | ed25519 signing/verification, sha256/blake3 hashing (consumed via ids-000-aa re-exports; no direct dependency per crate rule) ||
|| `ids-time` | `IdsTime` for access recency timestamps and cooling windows; monotonic tick source ||
|| `ids-write` | Producer: write accesses feed temperature tracking; new objects default to hot tier ||
|| `ids-read` | Consumer: resolves `RecordId` → current tier; read accesses feed temperature tracking ||
|| `ids-lifecycle` | Consumer/producer: lifecycle rules drive tier transitions; hotcold emits movement intents ||
|| `ids-storage` | Tier backend abstraction (hot/warm/cold storage backends) ||
|| `ids-placement` | Placement attribute storage (tier per `RecordId`); sealed contracts for placement decisions ||
|| `ids-identity` | `NodeId` origin binding for signed movement manifests ||

**Integration points:**
- `ids-hotcold` exposes `PlacementClass`, `TemperatureTracker`, `PromotionDemotionEngine`, `MovementPlanner`, `DestinationVerifier` types and traits.
- `ids-write`/`ids-read` record accesses into the temperature tracker on each operation.
- Lifecycle rules (time-based or access-based) enqueue promotion/demotion intents; the engine evaluates them with hysteresis and the cost-benefit gate.
- Movement manifests ride `ids-carrier::Endpoint` datagrams — no direct carrier dependency beyond the `Endpoint` type from `ids-000-aa`.
- No dependency on libp2p/TCP/IP/UDP/QUIC/HTTP/DNS — carrier-agnostic by construction.
- `ids-crypto` is the only crate that does NOT depend on `ids-000-aa`; `ids-hotcold` depends on `ids-000-aa` and consumes crypto primitives through it.

---

### 6. Proposed File Structure (~200–400 files, all production-grade)

**Crate:** `ids-hotcold` (new crate; depends on `ids-000-aa`, `ids-carrier` types; consumes `ids-crypto` primitives via re-exports; `ids-time` for recency; `serde`, `thiserror`, `blake3`, `sha2`, `bytes`, `proptest`/`bolero` for testing)

```
ids-hotcold/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (tier thresholds, cooling period, hysteresis margins)
├── error.rs                        # IdsHotColdError (thiserror, comprehensive)
├── features.rs                     # Feature flags (movement, verification, testing, fuzzing)
│
├── types/                          # Core value types
│   ├── mod.rs
│   ├── placement_class.rs          # PlacementClass enum: Hot / Warm / Cold (+transient)
│   ├── placement_attr.rs           # Placement attributes per RecordId (tier, size, seq)
│   ├── record_ref.rs               # RecordId/DatasetId/PartitionId reference + tier binding
│   ├── movement_id.rs              # MovementId (opaque, unique per move)
│   ├── tier_seq.rs                 # Monotonic tier-sequence per RecordId (anti-replay)
│   └── latency_slo.rs              # Per-tier latency SLO (read/write targets)
│
├── temperature/                    # Access pattern tracking & temperature estimation
│   ├── mod.rs
│   ├── tracker.rs                  # TemperatureTracker — per-object access recording
│   ├── access_sample.rs            # AccessSample (recency, frequency, size, op type)
│   ├── ema.rs                      # Exponential moving average with cooling
│   ├── dual_counter.rs             # Frequency + momentum EMA counters (HybridTier/FreqTier style)
│   ├── cooling.rs                  # Periodic cooling policy (period C, decay factor)
│   ├── histogram.rs                # Hot/warm/cold histogram over objects
│   ├── working_set.rs              # Working-set estimation for tier capacity planning
│   └── approx.rs                   # Approximate counters (counting Bloom) for scale
│
├── policy/                         # Promotion/demotion decision engine
│   ├── mod.rs
│   ├── engine.rs                   # PromotionDemotionEngine — evaluates thresholds
│   ├── thresholds.rs               # Tier thresholds (promote/demote with hysteresis)
│   ├── gate.rs                     # Cost-benefit gate (savings vs. migration cost)
│   ├── selector.rs                 # Select objects to move (coldest to evict, hottest to fetch)
│   ├── lifecycle.rs                # Lifecycle rule ingestion (time/access based)
│   ├── intent.rs                   # MovementIntent (queued promotion/demotion)
│   ├── scheduler.rs                # Move scheduler (bandwidth budget, throttling)
│   └── cooldown.rs                 # Per-object move cooldown (anti-thrash)
│
├── movement/                       # Movement planning & execution
│   ├── mod.rs
│   ├── planner.rs                  # MovementPlanner — copy→verify→delete-source
│   ├── manifest.rs                 # MovementManifest (signed, canonical)
│   ├── progress.rs                 # Resumable progress (segment fingerprint vector)
│   ├── executor.rs                 # MoveExecutor — drives copy/verify/release
│   ├── segment.rs                  # Data segment unit for chunked moves
│   ├── resume.rs                   # Resume interrupted moves from persisted progress
│   └── throttle.rs                 # Bandwidth/throttle shaping for move traffic
│
├── verification/                   # Destination verification
│   ├── mod.rs
│   ├── verifier.rs                 # DestinationVerifier — 3-dimension validation
│   ├── count_check.rs              # Object count consistency (key-listing set compare)
│   ├── content_check.rs            # Content consistency (sha256/blake3 per object)
│   ├── metadata_check.rs           # Metadata consistency dimension
│   ├── result.rs                   # VerificationResult (pass/fail/per-object detail)
│   └── sampling.rs                 # Sampling strategy for TB/PB-scale (1–5%)
│
├── wire/                           # Serialization & framing
│   ├── mod.rs
│   ├── format.rs                   # Versioned wire envelope (CBOR/bincode)
│   ├── cbor.rs                     # Deterministic CBOR encoding
│   ├── checksum.rs                 # Payload checksums (sha256/blake3)
│   └── framing.rs                  # CarrierType + Vec<u8> framing
│
├── crypto/                         # Allowed-crypto boundary (consumes via ids-000-aa)
│   ├── mod.rs
│   ├── ed25519.rs                  # ed25519 sign/verify for movement manifests
│   ├── hash.rs                     # sha256/blake3 content hashing
│   └── nonce.rs                    # Nonce/freshness for movement manifests
│
├── placement_state/                # Authoritative tier state store
│   ├── mod.rs
│   ├── store.rs                    # PlacementStateStore (tier per RecordId)
│   ├── transition.rs               # Atomic tier transition (one authoritative tier)
│   ├── journal.rs                  # Append-only placement journal (move history)
│   └── snapshot.rs                 # Periodic placement-state snapshot + recovery
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── placement_tests.rs          # PlacementClass invariants, identity invariance
│   ├── temperature_tests.rs        # EMA cooling, dual counter behavior
│   ├── policy_tests.rs             # Thresholds, hysteresis, cost-benefit gate
│   ├── movement_tests.rs           # Copy→verify→delete-source ordering
│   ├── verification_tests.rs       # 3-dimension validation, fail-closed
│   ├── wire_tests.rs               # Wire format round-trip
│   ├── adversarial_tests.rs        # Forged manifests, replay, count mismatch
│   ├── chaos_tests.rs              # Interrupted move resume, carrier failure
│   └── property_tests.rs           # Proptest/bolero placement convergence
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_temperature.rs
│   ├── fuzz_policy.rs
│   ├── fuzz_movement.rs
│   └── fuzz_verification.rs
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── temperature_bench.rs        # Access recording + cooling throughput
│   ├── movement_bench.rs           # Move latency + bandwidth overhead
│   ├── verification_bench.rs       # Checksum verification throughput
│   └── gate_bench.rs               # Cost-benefit gate evaluation
│
├── spec/                           # Formal specifications
│   ├── placement_spec.md           # Placement class & transition laws
│   ├── temperature_spec.md         # Temperature estimation model
│   ├── movement_spec.md            # Copy→verify→delete-source safety
│   └── tla/                        # TLA+/PlusCal (in ids-doc crate)
│
├── docs/                           # Documentation
│   ├── architecture.md             # ids-hotcold architecture
│   ├── integration_guide.md        # Integrating with ids-write/ids-read/ids-lifecycle
│   ├── tier_model.md               # Hot/warm/cold tier model rationale
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── temperature_vectors.json
    ├── movement_vectors.json
    └── verification_vectors.json
```

**Why ~200–400 files:** one file per responsibility — placement classes, attributes, temperature tracking (tracker/EMA/dual-counter/cooling/histogram/working-set/approx), policy engine (thresholds/gate/selector/lifecycle/intent/scheduler/cooldown), movement (planner/manifest/progress/executor/segment/resume/throttle), verification (count/content/metadata/result/sampling), wire format, crypto boundary, placement state store, tests, fuzz, benchmarks, specs, docs, test vectors. No stubs; every file holds production-grade code with error handling, documentation, and tests. (~200–400 lands within the ~90–1000 file envelope requested for this phase.)

---

### 7. Key Algorithms & Data Structures

#### 7.1 Placement Class & Transition

```
PlacementClass ::= Hot | Warm | Cold | Transient(moving_to)
PlacementAttr { record_id: RecordId, tier: PlacementClass, size: u64, tier_seq: u64 }

// Invariant: exactly one PlacementAttr per RecordId; tier_seq increments on every transition
// Identity invariance: RecordId unchanged across transitions; tier is an attribute, not identity
```

#### 7.2 Temperature Estimation (dual EMA counter)

```
on_access(obj, size, now):
    freq[obj] = EMA(freq[obj], +1, cooling C_freq)     // minutes-to-hours
    mom[obj]  = EMA(mom[obj],  +1, cooling C_mom)      // seconds, fast
    last_access[obj] = now

temperature(obj) = combine(freq[obj], mom[obj])        // promote on high freq OR high mom

cool_all():  // every cooling period C
    for obj: freq[obj] /= 2; mom[obj] /= 2
```

Promote if temperature ≥ T_high; demote if ≤ T_low and not in cooldown; T_high > T_low (hysteresis).

#### 7.3 Cost-Benefit Gate

```
worthwhile(obj, from, to):
    saved = Access[obj] × (Latency[from] − Latency[to])
    cost  = Size[obj] × (MoveBandwidthCost + WriteAmplification[to])
    return saved > cost × SafetyMargin   // SafetyMargin > 1 to avoid marginal moves
```

(Lustre LCMP quantitative model: net saved = Access_cold/Perf_upper + Access_hot/Perf_lower − Overhead.)

#### 7.4 Movement Execution (copy→verify→delete-source)

```
move(obj, from, to):
    manifest = sign(RecordId, from, to, size, sha256(obj), tier_seq+1)
    send(manifest) over ids-carrier::Endpoint
    progress = resume_state(obj)  // segments already copied
    for seg in obj.segments not in progress:
        copy(seg) → to; persist progress(seg)
    if verify_destination(obj, to):   // 3-dimension check
        release_source(obj)           // delete from `from`
        set_placement(obj, to, tier_seq+1)
    else:
        keep_source(obj); fail_closed(obj)  // re-transfer on next cycle
```

#### 7.5 Destination Verification (3 dimensions)

```
verify(obj, source, dest):
    count_ok  = list(source) == list(dest)     // object-count consistency
    content_ok = for each obj: sha256/blake3(src) == sha256/blake3(dest)
    meta_ok   = metadata(src) == metadata(dest) // supported entries
    return count_ok && content_ok && meta_ok
```

Fail closed: any dimension false ⇒ keep source, schedule re-transfer.

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------||
|| `ids-000-aa` | NodeId, RegionId, RecordId, DatasetId, PartitionId, Endpoint, CarrierType, PlacementDecision, sealed contracts ||
|| `ids-carrier` | Endpoint type (CarrierType + Vec<u8>) — sole transport ||
|| `ids-crypto` | ed25519, sha256, blake3 primitives (consumed via ids-000-aa re-exports) ||
|| `ids-time` | IdsTime for access recency & cooling windows ||
|| `ids-write` | Producer: write accesses feed temperature tracking ||
|| `ids-read` | Consumer: resolves RecordId → current tier ||
|| `ids-lifecycle` | Lifecycle rules drive tier transitions ||
|| `ids-storage` | Tier backend abstraction (hot/warm/cold) ||
|| `ids-placement` | Placement attribute storage (tier per RecordId) ||
|| `serde` + `serde_bytes` | Serialization ||
|| `thiserror` | Error types ||
|| `blake3` | Content hashing (workspace dependency) ||
|| `sha2` | SHA-256 hashing ||
|| `bytes` | Buffer handling ||
|| `proptest` / `bolero` | Property-based testing ||
|| `criterion` | Benchmarks ||

No networking crates (libp2p/TCP/IP/UDP/QUIC/HTTP/DNS) — `ids-hotcold` is transport-agnostic over `ids-carrier::Endpoint`. No `std::net` or `tokio::net`. Allowed crypto only: ed25519 (manifest signing), xchacha20poly1305/chacha20poly1305 (permitted but not required here), sha256/blake3 (content hashing).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement Method ||
||---|-----------|-------------------||
|| AC-037.1 | Placement class invariants | Unit test: exactly one PlacementAttr per RecordId; tier_seq monotonic ||
|| AC-037.2 | Identity invariance | Unit test: RecordId unchanged across hot↔warm↔cold transitions ||
|| AC-037.3 | Temperature EMA | Unit test: access updates frequency+momentum; cooling decays both ||
|| AC-037.4 | Dual-counter promote/demote | Property test: promote on high freq OR high mom; demote only on low freq AND low mom ||
|| AC-037.5 | Hysteresis | Unit test: T_high > T_low prevents oscillation at boundary ||
|| AC-037.6 | Cost-benefit gate | Unit test: move scheduled iff expected savings > migration cost × margin ||
|| AC-037.7 | Move ordering | Adversarial test: source released only after destination verified; fail-closed on mismatch ||
|| AC-037.8 | Destination verification | Unit test: 3-dimension check (count, sha256/blake3 content, metadata) ||
|| AC-037.9 | Replay resistance | Unit test: duplicate/stale (RecordId, tier_seq) manifests rejected ||
|| AC-037.10 | Signature verification | Zero manifests accepted without valid ed25519 signature ||
|| AC-037.11 | Resume after interruption | Test: interrupted move resumes from persisted segment progress ||
|| AC-037.12 | Placement convergence | Property test: fixed workload + thresholds → stable tier assignment, no oscillation ||
|| AC-037.13 | Lifecycle integration | Unit test: time/access-based rules enqueue correct movement intents ||
|| AC-037.14 | Carrier agnosticism | Compiles/behaves identically with zero std::net/tokio.net/libp2p/TCP/UDP/QUIC/HTTP/DNS ||
|| AC-037.15 | Allowed crypto only | Dependency audit: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 ||
|| AC-037.16 | Property tests | 100,000+ iterations (proptest/bolero) on temperature/policy/placement ||
|| AC-037.17 | Adversarial tests | Forged manifests, replay, count mismatch, metadata loss — all fail-safe ||

---

### 10. Remaining Uncertainties

1. **Tier granularity:** object/`RecordId` granularity vs. page/segment granularity — object granularity matches IDS addressing but page-level may be needed for DBMS/KV workloads; decision deferred to implementation.
2. **Temperature defaults:** hysteresis margins, cooling periods, and cost-benefit safety margin are workload-dependent; defaults are placeholders pending deployment measurement.
3. **Wire format:** CBOR (deterministic, self-describing) vs bincode vs protobuf for movement manifests; CBOR favored for compactness and schema evolution.
4. **Approximate counters at scale:** exact per-object EMA counters scale linearly with object count; counting Bloom filter or hierarchical indexing threshold unresolved.
5. **Move concurrency & bandwidth shaping:** number of concurrent moves and per-move bandwidth caps need benchmarking against tier media characteristics.
6. **Placement-state persistence:** append-only journal + snapshot cadence and crash-consistency (WAL-like or periodic snapshot) for the placement store unresolved.
7. **Lifecycle rule conflicts:** overlapping time-based and access-based rules targeting the same object need precedence semantics.
8. **Formal verification scope:** TLA+/PlusCal spec for placement invariants and move safety; extending to temperature convergence is additional effort.
9. **Cross-region movement:** tier movement across regions (not just media) adds latency/failure-domain considerations; scope (single-node vs. distributed) TBD.
10. **Verification sampling threshold:** at what dataset size full validation becomes prohibitive and sampling (1–5%) is acceptable; core-data policy TBD.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------||
|| New separate crate `ids-hotcold` | Per IDS convention: separate crates, not more folders in existing ones ||
|| Three classes hot/warm/cold | Matches Elasticsearch/Azure/S3 tier nomenclature and research (hot frequent, warm infrequent, cold retained) ||
|| Dual EMA frequency+momentum temperature | Adapts quickly to hotness changes while preserving long-term distribution (HybridTier/FreqTier) ||
|| Cooling via periodic decay | Forgets stale hotness; prevents permanently-hot misclassification ||
|| Hysteresis thresholds (T_high/T_low) | Prevents ping-pong migration at boundary workloads ||
|| Cost-benefit gate before moving | Migration only when latency savings exceed move cost (Lustre LCMP model) ||
|| Copy→verify→delete-source ordering | Prevents data loss; source retained until destination verified ||
|| 3-dimension destination verification | Count + content (sha256/blake3) + metadata; matches cloud migration validation practice ||
|| `RecordId` identity invariant | Tier is a placement attribute, not identity; consumers address by RecordId regardless of tier ||
|| Signed movement manifests over ids-carrier::Endpoint | ed25519 prevents forged moves; carrier-agnostic transport ||
|| Allowed crypto only: ed25519/sha256/blake3 (+permitted xchacha/chacha unused) | Within IDS crypto policy; no X25519/AES-GCM/RSA ||
|| Depends on ids-000-aa; crypto via re-exports | Follows crate dependency rule (ids-crypto is sole exception) ||

---

### 12. Sources Consulted (30 targeted searches)

1. Elasticsearch data tiers: hot, warm, cold, frozen — https://www.elastic.co/docs/manage-data/lifecycle/data-tiers (tier roles, hardware profiles, searchable snapshots)
2. Herodotou & Kakoulli, "Automating Distributed Tiered Storage Management in Cluster Computing," PVLDB 13(1), 2019 — http://www.vldb.org/pvldb/vol13/p43-herodotou.pdf (four decision points, ML access-pattern prediction, upgrade/downgrade policies)
3. ACM TOMS, "Harnessing Page Access Frequency Distribution for Efficient Memory Tiering" (Memtis), 2026 — https://dl.acm.org/doi/full/10.1145/3777549 (hot-set classification, warm-set migration reduction, threshold adaptation)
4. Azure Blog, "Optimize object storage costs automatically with smart tier," 2026 — https://azure.microsoft.com/en-us/blog/optimize-object-storage-costs-automatically-with-smart-tier-now-generally-available (hot→cool 30d, cool→cold 60d, re-access promotes to hot)
5. Apache Doris, "Tiered storage for hot and cold data," 2023 — https://doris.apache.org/blog/Tiered-Storage-for-Hot-and-Cold-Data-What-Why-and-How (cold-data caching, block-granularity, LRU block cache)
6. ScienceDirect, "Adaptive Intelligent Tiering for modern storage systems" (AIT), 2023 — https://www.sciencedirect.com/science/article/abs/pii/S0166531623000020 (two-stage migration policy, DL+RL control)
7. Springer, "Adaptive Parameter Tuning for Constructing Storage Tiers," 2022 — https://link.springer.com/article/10.1007/s44227-022-00004-3 (tiering thresholds Th/Tl, selective migration, capacity pressure demotion)
8. USENIX FAST'26, "Getting the MOST out of your Storage Hierarchy with Mirror-Optimized Storage Tiering" — https://www.usenix.org/system/files/fast26-tu.pdf (mirrored vs tiered class, dynamic sizing, hot segment promotion)
9. ACM Computing Surveys, "Machine Learning-Based Caching and Tiering in Modern Data Storage Systems: A Survey," 2026 — https://dl.acm.org/doi/full/10.1145/3803857 (tiering taxonomy, placement/migration policies, ML/RL/DL tradeoffs)
10. AMC: adaptive multi-level cache algorithm in hybrid storage systems, 2015 — http://arc.zju.edu.cn/_upload/article/files/eb/1a/1e2226a24259b4db9cb1d56866ca/3873da32-d5b5-4f0a-9ac0-b9574c811e62.pdf (selective promote/demote, read+write, SSD lifetime)
11. ReStore-in-Action: Adaptive Multi-Tier Storage Management via Intelligent Data Migration Policy, VLDB 2026 — https://www.vldb.org/pvldb/vol19/p4702-zhang.pdf (RL migration MDP, tier temperature + pressure state, SSD asymmetry)
12. Automating Distributed Tiered Storage Management (arXiv), 2019 — https://arxiv.org/html/1907.02394v2 (replication upgrade/downgrade definitions, LRU/LFU/LRFU/LIFE/EXD/XGB policies, OctopusFS)
13. Lustre LCMP cache replacement policies, 2024 — https://www.eofs.eu/wp-content/uploads/2024/02/15_li_xi-cache_replacement_policies.pdf (quantitative cost-benefit of replacement, coldest/hottest selection, "evict as little as possible")
14. MDPI, "Employing Streaming Machine Learning for Modeling Workload Patterns in Multi-Tiered Data Storage Systems," 2025 — https://www.mdpi.com/1999-5903/17/4/170 (temporal/spatial access patterns, file hotness prediction)
15. Big Data Mining and Analytics, "Data Temperature Informed Streaming," 2024 — https://www.sciopen.com/article/10.26599/BDMA.2023.9020039 (data temperature metadata, hybrid algorithm, movement-limiting rules)
16. HybridTier: Adaptive Lightweight CXL-Memory Tiering, 2023 — https://arxiv.org/html/2312.04789v2 (frequency + momentum dual EMA, promote/demote matrix)
17. FreqTier: Lightweight Adaptive Tiering for CXL Memory Systems, 2025 — https://nvmw.ucsd.edu/nvmw2025-program/nvmw2025-final55.pdf (frequency+momentum, counting Bloom filter metadata overhead)
18. Efficient Big-Data Access: Taxonomy and Survey, 2020 — https://www.cse.iitk.ac.in/users/amitangshu/TBD.pdf (data heat prediction, cross-tier migration triggers)
19. Alibaba Cloud, "Validate data consistency after migration" — https://www.alibabacloud.com/help/en/data-online-migration/user-guide/how-to-verify-data-consistency-after-migration (3 dimensions: count, content, metadata)
20. AWS DataSync, "Configuring how AWS DataSync verifies data integrity" — https://docs.aws.amazon.com/datasync/latest/userguide/configure-data-verification-options.html (checksum verification, verify transferred vs all data)
21. Google Cloud, "Data integrity | Storage Transfer Service" — https://docs.cloud.google.com/storage-transfer/docs/data-integrity (end-to-end checksum, on-the-fly checksumming, after-transfer checks)
22. Scality, "Object storage integrity: How to verify migrated data," 2026 — https://www.scality.com/blog/object-storage-integrity-verification (key-listing set comparison, ETag limits, metadata copy semantics)
23. Azure NetApp Files, "Storage with cool access" — https://learn.microsoft.com/en-us/azure/azure-netapp-files/cool-access-introduction (warm initial temperature, coolness period, transparent retrieval, metadata never cooled)
24. AWS ElastiCache, "Data tiering in ElastiCache" — https://docs.aws.amazon.com/AmazonElastiCache/latest/dg/data-tiering.html (LRU memory↔SSD tiering, keys-in-memory, 300µs SSD latency)
25. AWS S3, "Performance characteristics of Intelligent-Tiering storage class" — https://docs.aws.amazon.com/fsx/latest/LustreGuide/intelligent-tiering-file-systems.html (auto tiering, SSD read cache, tens-of-ms cold latency)
26. Azure Blob, "Best practices for using blob access tiers" — https://learn.microsoft.com/en-us/azure/storage/blobs/access-tiers-best-practices (hot/cool/cold/archive cost model, read-percentage optimization)
27. Azure Blob, "Lifecycle management policies that transition blobs between tiers" — https://learn.microsoft.com/en-us/azure/storage/blobs/lifecycle-management-policy-access-tiers (tierToCool/tierToArchive, last-accessed-time conditions)
28. Microsoft Fabric OneLake, "Creating a lifecycle management policy," 2026 — https://learn.microsoft.com/en-us/fabric/onelake/onelake-lifecycle-management (tierToCool/tierToCold, daysAfterLastAccessTime conditions)
29. AWS Storage Blog, "Reducing petabyte-scale S3 storage costs with evidence-based lifecycle rules," 2026 — https://aws.amazon.com/blogs/storage/reducing-petabyte-scale-s3-storage-costs-with-evidence-based-lifecycle-rules (Intelligent-Tiering, Glacier Instant Retrieval, transition age evidence)
30. MDPI, "Adaptive Data Prefetching for File Storage Systems Using Online Machine Learning," 2026 — https://www.mdpi.com/2504-2289/10/1/28 (prefetching decisions, online ML training, real traces)

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 037 (ids-hotcold).
