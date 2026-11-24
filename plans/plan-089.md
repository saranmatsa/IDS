## Phase 089 — Create the ids-warm-tier crate (Warm storage for moderately accessed data): Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1330-1344)

```
89. Research only do not read plan.md **[Phase 089 — Create the ids-warm-tier crate: Warm storage for moderately accessed data.]**

Crate: ids-warm-tier
Purpose: Warm storage for moderately accessed data.
Contents: Warm tier placement, warm tier promotion/demotion, warm tier cost, warm tier capacity management, warm tier access patterns.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 089 — Create the ids-warm-tier crate: Warm storage for moderately accessed data.]**

Create plan-089.md in C:/idfs/plans/ for the ids-warm-tier crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 089 creates `ids-warm-tier`, the IDS crate for warm storage — the tier that holds moderately accessed data: less frequently accessed than hot data, but still actively used and requiring immediate availability, sitting between the hot tier (frequently accessed, highest performance) and the cold tier (infrequently accessed, lowest cost). It is a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), organized into subfolders by warm-tier concern: placement, promotion/demotion, cost modeling, capacity management, and access-pattern tracking.

**Core design:** `ids-warm-tier` owns warm-tier data lifecycle — admission into the warm tier, temperature scoring of records, promotion back to hot and demotion down to cold based on access frequency and recency, warm-tier capacity watermarks (low/high), cost-per-byte accounting for warm storage, and access-pattern histogram tracking. Every decision it returns is a policy decision; it never opens a socket or drives any transport directly. All cross-node warm-tier traffic flows as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` address ≤ 1024 bytes). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed via `ids-crypto` primitive IDs from `ids-000-aa` (ids-crypto does not depend on ids-000-aa to avoid circular dependency; ids-warm-tier depends on both).

**Scope boundary:** `ids-warm-tier` decides placement, promotion/demotion, capacity, and cost; it does not store bytes itself. Warm-tier storage nodes are served by other crates (ids-store, ids-erasure, ids-replication); `ids-warm-tier` tells them *where* and *when* data belongs. Thresholds are configurable budgets (watermark ratios, temperature scores, cost-per-byte caps), not hard-coded limits, so a capacity-constrained platform degrades gracefully rather than failing.

Skeleton `ids-warm-tier` does NOT yet exist at `C:/idfs/ids-warm-tier/`. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

**Research basis:** 30 parallel searches returned f4: Facebook's Warm BLOB Storage System (USENIX OSDI'14, Reed-Solomon(10,4) warm storage, effective replication factor 3.6 → 2.8/2.1); Elasticsearch data tiers (hot/warm/cold/frozen, ILM migration after weeks); HDFS storage policies (Hot/Warm/Cold with replica placement); Ceph cache tiering (cache pool + backing pool, writeback/read-only cache modes); S3 Intelligent-Tiering (automatic access-pattern-based moves, monitoring + automation charge); Azure blob access tiers (hot/cool/cold); Google Cloud Storage classes (Standard/Nearline/Coldline/Archive); Dropbox Magic Pocket warm/cold tiers (warm tier 1+1 replication, async cold migration); ELECT erasure-coding tiering for LSM-trees (hotness-aware redundancy transition, storage saving target α); Alluxio tiered storage (memory/SSD/HDD tiers, LRU/LFU/Greedy evictors, TTL, annotation policies); GlusterFS tiering volume watermark-hi/watermark-low with promote/demote frequency thresholds; IBM Easy Tier warm/cold demotion and warm promotion (extent-level heat values, 80% capacity threshold); HSM hierarchical storage management (migration policy, LRU/STP/STEP algorithms, file grooming); DITIS tiered storage simulator (tier placement, migrate up/down, deletion policies); Mole tier-specific hotness profiling (reaccess-interval histograms, targeted promotion, staging demotion); HotRAP hot-record retention and promotion for LSM tiered storage; CloudJump III eviction-centric engine-aware tiering; storgatech PLC SSD warm-tier SLOs (read p99 < 20ms, TTL 30–90 days).

---

### 2. Architecture Overview

`ids-warm-tier` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is a policy/decision layer: given a record's access history, current tier, tier capacity, and workload SLO, it decides (a) whether the record belongs in the warm tier, (b) whether to promote it to hot or demote it to cold, (c) how much warm-tier capacity to reserve, (d) what warm-tier cost-per-byte applies, and (e) what placement topology to use. It never sends bytes itself; it returns a `PlacementDecision` / `PromotionDecision` / `DemotionDecision` that the caller frames into opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (tier users: ids-store, ids-erasure, …)     │
│  read/write with temperature tag, admission to warm tier, migration requests             │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                    Phase 089: ids-warm-tier (THIS CRATE — policy)                        │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌──┐   │
│  │ access-pattern  │ │ temperature     │ │ placement       │ │ promotion/      │ │cost│ │
│  │ tracking        │ │ scoring         │ │ (warm tier)     │ │ demotion policy │ │model│ │
│  │ (frequency/     │ │ (hot/warm/cold) │ │ admission,      │ │ hysteresis,     │ │per │ │
│  │  recency)       │ │ per record      │ │ topology hints  │ │ thrashing prev. │ │byte│ │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘ └──┘   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐                            │
│  │ capacity        │ │ crypto surface  │ │ audit/logging   │                            │
│  │ management      │ │ (allowlist only)│ │ (signed+chained)│                            │
│  │ (watermarks,    │ │                 │ │                 │                            │
│  │  reservations)  │ │                 │ │                 │                            │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘                            │
└──────────────────────────────────────────┬──────────────────────────────────────────────┘
                                           │ opaque Vec<u8> datagrams
                                           ▼
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                                │
│  send_datagram(Endpoint{CarrierType, addr: Vec<u8> ≤ 1024 B}, payload: &[u8])            │
│  receive_datagram → (Endpoint, Vec<u8>)                                                  │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS /                │
│  Wi-Fi / Bluetooth / cellular / VPN / mesh                                               │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of a warm-tier admission decision:** client submits `Admit{record_id, size, access_history, current_tier}` → `ids-warm-tier` scores temperature (frequency × recency weight), checks warm-tier capacity watermark and reservation, returns `AdmitDecision{admit_to_warm: bool, topology_hint, reservation_bytes, cost_per_byte}` → caller places the record via `ids-store`/`ids-erasure` and sends migration as opaque datagram(s) to `ids-carrier::Endpoint` targets.

**Data flow of a promotion/demotion cycle:** `temperature_scanner` sweeps warm-tier records on a tick (monotonic `ids-time`, never wall-clock for ordering); records whose temperature falls below the demote threshold for N consecutive ticks become demotion candidates → `DemoteDecision{target_tier: Cold, bytes, migration_bandwidth_cap}`; records whose temperature rises above the promote threshold become promotion candidates → `PromoteDecision{target_tier: Hot, bytes}`. Hysteresis (separate promote/demote thresholds + minimum dwell time) prevents thrashing.

**Temperature semantics (research-verified):**
- **Hot data**: frequently accessed, highest performance tier (SSD/NVMe), small fraction of total data (the "20% most important" rule observed in tiered-storage literature).
- **Warm data**: accessed less frequently than hot but still actively used — the moderately accessed tier this crate owns; balanced performance/cost (HDD or cost-optimized SSD), typically holds recent-weeks data (Elasticsearch warm pattern) or data accessed monthly or less (Google Nearline pattern).
- **Cold data**: infrequently accessed, lowest cost, higher retrieval latency (object storage / archive), minimum retention durations.

**Crypto used:** ed25519 for signing placement/promotion/demotion decisions and audit checkpoints; sha256/blake3 for temperature hashing, access-pattern digest, audit-chain links; xchacha20poly1305/chacha20poly1305 to encrypt warm-tier state at rest (temperature tables, reservation records) when confidentiality required.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline with the source list in Section 12.

#### 3.1 Warm tier definition and the three-tier model (hot / warm / cold)

**Source:** Elasticsearch data tiers documentation (elastic.co/docs/manage-data/lifecycle/data-tiers); Aerospike tiered storage guide; Purestorage tiered data storage overview; Cloud storage tier optimization through storage object classification (Springer, Computing 2024); storgatech PLC SSD tiered storage strategy.

The three-tier model orders storage by access frequency: hot (frequent, fast, expensive), warm (moderate, balanced, mid-cost), cold (infrequent, slow, cheap). Warm tier sits between hot and cold: "data that is important to retain and use occasionally, but not frequently enough to justify the cost of premium storage" (Aerospike). Elasticsearch warm tier holds data from recent weeks, queried less frequently than hot, updates allowed but infrequent. Azure cool/cold tiers are for data accessed less frequently but still immediately available. `ids-warm-tier` owns the warm tier: admission, temperature scoring, promotion/demotion, capacity, cost.

#### 3.2 Data temperature classification (access frequency + recency)

**Source:** Data Temperature Informed Streaming for Optimising Large-Scale Multi-Tiered Storage (Big Data Mining and Analytics, 2024); Mole: Tier-Specific Hotness Profiling Driven Memory Tiering (arXiv:2610.12164); Automating Distributed Tiered Storage Management in Cluster Computing (PVLDB 2019); Harnessing Page Access Frequency Distribution for Efficient Memory Tiering (ACM TOCS).

Data temperature combines access frequency and recency: existing solutions classify hot/cold primarily on age and usage frequency, but research shows age alone assumes new data has priority and usage is purely reactive — additional variables (reaccess interval, unaccessed duration, user-defined weights) improve placement. Mole builds hotness histograms from reaccess intervals (not just access counts) and derives capacity-based tier thresholds. `ids-warm-tier` scores temperature as a weighted combination of access frequency, recency (last access age), and record size, with configurable per-class weights.

#### 3.3 Warm tier placement and admission policy

**Source:** HDFS Archival Storage / storage policies (Hot/Warm/Cold/All_SSD/One_SSD); Apache Ozone storage policies (Hot→SSD, Warm→DISK, Cold→ARCHIVE with fallback); DITIS tiered storage simulator (tier placement policies); Alluxio tiered storage block allocation.

Placement decides the initial tier for new data and the target tier for migrated data. HDFS warm policy stores some replicas on DISK and some on ARCHIVE (partially hot, partially cold). Ozone warm data goes to DISK (not SSD, not ARCHIVE), with fallback to warm/cold if hot is full. Alluxio allocates new blocks to the top tier by default, spilling to the next tier when full. `ids-warm-tier` admission policy: new writes may be admitted directly to warm (bypassing hot) based on size/class; placement considers tier capacity watermarks, replica topology, and cost-per-byte budget.

#### 3.4 Promotion and demotion policy (hysteresis, thrashing prevention)

**Source:** IBM Easy Tier automatic processing (warm demotion, cold demotion, warm promotion); GlusterFS tiering volume (watermark-hi/watermark-low, promote/demote frequency, access-count threshold); Mole staging demotion and targeted promotion; f4 warm BLOB storage (asynchronous background migration as data becomes cold).

Promotion moves warm→hot when temperature exceeds the promote threshold; demotion moves warm→cold when temperature falls below the demote threshold. Hysteresis (separate thresholds + minimum dwell time / cooldown) prevents thrashing — the "three movements up, three movements back down" wasted-movement problem documented in data-temperature research. Easy Tier: warm demotion triggered when a higher tier exceeds bandwidth capacity; warm promotion when a lower tier exceeds IOPS capacity; 80% capacity threshold on tier assignment. GlusterFS: watermark-low/hi govern promote/demote likelihood; promote/demote frequency and read/write access-count threshold mark files HOT/COLD. `ids-warm-tier` implements promote threshold > demote threshold with a minimum dwell time and per-record migration budget.

#### 3.5 Warm tier capacity management (watermarks, reservations, spill)

**Source:** IBM Easy Tier (80% tier capacity threshold, pending assignments above threshold); Alluxio tiered storage management (eviction when tiers full, watermark-based eviction deprecated in favor of annotation policies); HSM (migration policy controls tier transfers to keep response time short); Ceph cache tiering (flush objects from cache to backing tier).

Capacity management keeps warm tier within budget: low watermark (stop demotions, allow promotions), high watermark (stop promotions, trigger demotions), spill to cold when warm exceeds high watermark. Reservations: warm tier reserves bytes for expected promotions so hot-tier spill doesn't overflow warm. When warm tier is full, admission of new warm data is rejected or redirected to cold with a `CapacityExceeded` decision. `ids-warm-tier` tracks `tierStorageUsed` per tier and exposes `isFull`/`availableBytes`.

#### 3.6 Warm tier access-pattern tracking (histograms, scoring)

**Source:** Mole tier-specific hotness profiling (reaccess-interval histogram, tier thresholds from capacity boundaries); Automating Distributed Tiered Storage Management (tracking and predicting file access patterns with incremental learning); Harnessing Page Access Frequency Distribution (page access frequency distribution for tiering); DITIS (caching + tiering policies driven by access patterns).

Access-pattern tracking records per-record access frequency, recency, and access pattern (sequential vs random, read vs write ratio) in bounded histograms. Mole constructs hotness histograms from reaccess intervals and unaccessed durations, deriving tier thresholds when bucket counts reach tier capacity boundaries. `ids-warm-tier` maintains a per-record `AccessProfile` (access count, last-access tick, first-access tick, access-size histogram, sequentiality score) and recomputes temperature on each access and on each scanner tick.

#### 3.7 Warm tier cost modeling (cost per byte, cost per access)

**Source:** storgatech PLC SSD tiered storage strategy (SLOs: warm read p99 < 20ms, TTL 30–90 days); Amazon S3 Intelligent-Tiering (monitoring + automation charge per object); Google Cloud Storage cost guidance (Standard vs Nearline vs Coldline pricing, retrieval fees, early deletion); Cloud storage tier optimization through storage object classification (performance/cost trade-off per tier).

Cost model: warm tier cost per byte (storage) + cost per access (read/write operations) + migration cost (bytes moved × migration bandwidth cost). S3 Intelligent-Tiering charges a small monthly object monitoring and automation fee; Google Nearline/Coldline charge per-GB access and early-deletion fees. `ids-warm-tier` computes warm-tier cost per byte per record class and compares against hot/cold alternatives to recommend the cost-minimizing tier for each record, subject to SLO constraints (latency, availability).

#### 3.8 Warm tier durability and replication (replica vs erasure coding)

**Source:** f4: Facebook's Warm BLOB Storage System (OSDI'14, Reed-Solomon(10,4), effective replication factor 3.6 → 2.8/2.1); Dropbox Magic Pocket warm tier (1+1 replication scheme, 2× internal region replication); ELECT erasure-coding tiering for LSM (hybrid redundancy, storage saving target α); HDFS erasure coding (EC overhead ~50% vs 200% for 3× replication); Ceph erasure-coded pools (4+2 overhead 1.5× vs replication 3.0).

Warm data tolerates higher latency than hot, so warm tier can use erasure coding for storage efficiency where hot tier uses replication. f4 reduced warm BLOB effective replication factor from 3.6 to 2.1 using RS(10,4) across racks plus XOR wide-area. Magic Pocket warm tier uses 1+1 replication. ELECT selectively converts data from replication to erasure coding based on hotness and a configurable storage-saving target α. `ids-warm-tier` exposes redundancy policy per warm record (replication factor or erasure-code K/M) and durability scope (local/region), enforced jointly with `ids-consistency-profile`.

#### 3.9 Warm tier migration mechanics (bandwidth caps, background migration)

**Source:** f4 (asynchronous background migration as data becomes cold); Alluxio (tier management tasks back off under user I/O load, align/promote tasks respectful to user I/O); ELECT (redundancy transitioning and data offloading incur performance overhead, balanced by storage-saving target α); IBM Easy Tier (moves measured amount of extents at regular intervals without negatively impacting performance).

Migration moves bytes between tiers in the background, bounded by a migration bandwidth cap so foreground traffic is not starved. f4 migrates warm→cold asynchronously in the background. Alluxio tier management tasks (align/promote) back off whenever a worker or disk is under load. ELECT balances storage savings vs access performance via α. `ids-warm-tier` returns `MigrationPlan{bytes, bandwidth_cap_bytes_per_s, throttle_under_load: bool}` and the executor (ids-store/ids-replication) performs the actual datagram movement via `ids-carrier::Endpoint`.

#### 3.10 Warm tier consistency with cold/hot tiers (read-your-writes across tiers)

**Source:** ids-consistency-profile (Phase 060) profiles; Elasticsearch warm tier (updates still allowed, likely infrequent); Cloud Jump III eviction-centric engine-aware tiering (logical ownership decoupled from physical placement).

A record may move tiers while being read/written. Consistency across tier moves: logical ownership (which record belongs to which logical key) is decoupled from physical placement (which tier holds the bytes). Reads route to the current tier; writes go to the admission tier. During migration, reads are served from the source tier until the copy is acknowledged at the target, then ownership flips. `ids-warm-tier` coordinates with `ids-consistency-profile` so a record's profile quorum/durability requirements hold across tier transitions (e.g., warm tier demotion must not drop below the profile's durability requirement).

#### 3.11 Warm tier access SLOs (latency, availability targets)

**Source:** storgatech PLC SSD warm tier (read p99 < 20ms, write p99 < 50ms, 99.5% availability, accessed 0.05%–0.5% of objects/day); Elasticsearch warm tier (nodes don't need to be as fast as hot, replicas for resiliency); Google Cloud Storage Nearline (once per month or less) vs Standard (regular access).

Warm tier SLOs are looser than hot but tighter than cold: read p99 in the tens of ms, availability 99–99.5%, access frequency monthly-or-less to weekly. `ids-warm-tier` carries per-record SLO targets (max read latency, min availability, max cost per byte) and uses them in admission/promotion/demotion decisions — a record whose access pattern violates its warm SLO is promoted to hot; one that cools below the warm floor is demoted to cold.

#### 3.12 Warm tier and lifecycle management (TTL, retention, grooming)

**Source:** HSM hierarchical storage management (migration policy, file grooming = deletion from higher level after move to lower); Alluxio TTL (time-to-live with FREE/DELETE actions, periodic expiration check); Google Cloud Storage lifecycle (object transitions between storage classes, minimum storage durations Nearline 30 days / Coldline 90 days / Archive 365 days); Azure lifecycle management (hot 30 days → cool 90 days → archive 1 year).

Lifecycle: records have a warm-tier TTL or retention; after expiry they are groomed (deleted from warm after being moved to cold) or transitioned per a lifecycle policy. Minimum storage durations prevent premature demotion (e.g., 30-day Nearline minimum). `ids-warm-tier` exposes `warm_ttl` and `retention_policy` per record class, enforced by the capacity/scanner loop.

#### 3.13 Carrier-agnostic transport boundary (ids-carrier::Endpoint)

**Source:** IDS carrier-agnostic rule (prompts.txt entries 1, 89); HX-241 carrier/socket abstraction (Carrier/Socket/Endpoint traits with no networking-stack references); ids-000-aa `trait Carrier` sealed contract.

All warm-tier migration, replication, and placement traffic rides `ids-carrier::Endpoint` = `CarrierType` + `Vec<u8>` (≤ 1024 bytes). `CarrierType` enum: Loopback, Serial, Optical, Radio, Custom(u16). No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. `ids-warm-tier` never touches the carrier directly; it returns decisions that other crates frame into datagrams.

#### 3.14 Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305)

**Source:** IDS allowed-crypto policy (prompts.txt); RFC 8032 Ed25519; RFC 7539/8439 ChaCha20-Poly1305; libsodium XChaCha20-Poly1305 (192-bit nonce); NIST FIPS 180-4 SHA-256; BLAKE3 spec.

`ids-warm-tier` uses: ed25519 to sign placement/promotion/demotion/audit decisions; sha256 for temperature/access-pattern digest and audit-chain links; blake3 for fast content hashing of warm-state checkpoints; xchacha20poly1305/chacha20poly1305 to encrypt warm-tier state at rest (temperature tables, reservations) with random/counter nonces. No other crypto. `ids-crypto` exposes primitive IDs; `ids-warm-tier` references them via `ids-000-aa` only.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
|--------|-----------|-------------------------------|--------------------------|
| Thrashing (repeated promote/demote of same record) | Warm tier stability, migration bandwidth | Hysteresis: separate promote/demote temperature thresholds + minimum dwell time; per-record migration cooldown |
| Warm tier capacity overflow | Warm tier availability | High-watermark demotions, low-watermark promotion gating, spill-to-cold, `CapacityExceeded` admission decision |
| Stale temperature score (record heats up but stays cold) | Read latency, SLO violation | Periodic temperature scanner tick (monotonic ids-time), access-frequency decay, promote threshold re-evaluation |
| Migration storms (too many records moving at once) | Foreground traffic starvation | Migration bandwidth cap, per-tick migration byte budget, backoff under load (Alluxio pattern) |
| Durability drop below profile requirement on demotion | Data durability | `ids-consistency-profile` coordination: demotion only if target tier meets the record's profile durability scope |
| Replay of placement/promotion/demotion decisions | Decision integrity | ed25519-signed audit checkpoints, nonce-bound decisions, bounded replay cache per decision type |
| Temperature table tampering | Placement correctness | sha256/blake3 hash chain of temperature snapshots + periodic ed25519-signed checkpoints |
| Key extraction (warm-tier signing key) | Decision authenticity | Signing keys never leave node; encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on shutdown |
| Audit log tampering | Accountability | sha256/blake3 hash chain + periodic ed25519-signed checkpoints; offline verifiable |
| Carrier duplicating/delaying datagrams | Replay/ordering | Idempotent placement decisions tolerate duplicates; carrier-agnostic boundary keeps no network stack in trust base |
| Disallowed crypto algorithm | Crypto policy compliance | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305; CI allowlist test |

The carrier is never trusted: placement decisions and audit entries are signed over bytes, surviving any carrier re-framing; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-warm-tier ||
|-------|------|--------------------------|--------------------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`, `PlacementDecision` — consumed, never redefined; sealed trait contracts mirrored |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All warm-tier traffic flows to/from `Endpoint`; crate returns decisions, never sends directly |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signing decisions/audit; encrypting warm-tier state at rest; hashing temperature tables and access digests |
| `ids-store` | Primary record store (warm-tier consumer) | Receives admission/placement decisions; stores records on the tier `ids-warm-tier` assigns |
| `ids-erasure` | Erasure coding for warm-tier storage efficiency | Warm-tier redundancy (K/M) per `ids-warm-tier` redundancy policy; RS-style coding like f4 |
| `ids-replication` | Replica synchronization / migration | Performs warm↔hot↔cold data movement as opaque `Endpoint` datagrams per `MigrationPlan` |
| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | Demotion must not violate a record's profile durability/quorum requirement; profile tags records |
| `ids-cost` / `ids-accounting` | Cost tracking | Warm-tier cost per byte per record class; feeds cost model |
| `ids-time` | Monotonic time source | All temperature/scanner ticks keyed to `IdsTime`, never wall-clock |

**Integration points:** `ids-warm-tier` drives public contracts declared in `ids-000-aa`; it does not reach into crate internals. All cross-node traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`. No dependency on ids-discovery/ids-session/ids-cli beyond what the contracts expose.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-warm-tier/` (a separate crate, not a folder inside an existing crate):

```
ids-warm-tier/
├── Cargo.toml                          # Crate manifest; depends on ids-000-aa, ids-carrier, ids-crypto, ids-time
├── lib.rs                              # Crate root, public API surface
├── constants.rs                        # Warm tier defaults: watermarks, temperature weights, TTL, migration caps
├── error.rs                            # WarmTierError (thiserror): admission, capacity, migration, temperature errors
├── features.rs                         # Feature flags: crypto, audit, nightly
│
├── access_pattern/                     # Warm tier access-pattern tracking
│   ├── mod.rs
│   ├── tracker.rs                      # Records access frequency, recency, per-record AccessProfile
│   ├── histogram.rs                    # Bounded access histogram (frequency/size/sequentiality)
│   ├── scorer.rs                       # Temperature score = f(frequency, recency, size, weights)
│   ├── decay.rs                        # Access-frequency decay over idle ticks
│   └── types.rs                        # AccessProfile, AccessRecord, TemperatureScore structures
│
├── temperature/                        # Temperature classification (hot/warm/cold)
│   ├── mod.rs
│   ├── classifier.rs                   # Classifies record as hot/warm/cold per score thresholds
│   ├── zones.rs                        # Temperature zone definitions (HotZone, WarmZone, ColdZone)
│   └── types.rs                        # TemperatureZone, ZoneTransition structures
│
├── placement/                          # Warm tier placement and admission
│   ├── mod.rs
│   ├── admission.rs                    # AdmitDecision: admit to warm / redirect to cold / reject
│   ├── topology.rs                     # Topology hints for warm tier placement (rack/region awareness)
│   ├── reservation.rs                  # Warm tier capacity reservation for expected promotions
│   └── types.rs                        # PlacementDecision, AdmissionResult structures
│
├── promotion_demotion/                 # Promotion/demotion policy with hysteresis
│   ├── mod.rs
│   ├── policy.rs                       # Promote/demote thresholds, minimum dwell time, cooldown
│   ├── promoter.rs                     # Warm→hot promotion candidate selection
│   ├── demoter.rs                      # Warm→cold demotion candidate selection
│   ├── hysteresis.rs                   # Hysteresis: separate thresholds + dwell time
│   └── types.rs                        # PromotionDecision, DemotionDecision structures
│
├── capacity/                           # Warm tier capacity management (watermarks)
│   ├── mod.rs
│   ├── watermarks.rs                   # Low/high watermark logic, isFull, availableBytes
│   ├── spill.rs                        # Spill-to-cold when warm exceeds high watermark
│   ├── allocator.rs                    # Warm tier byte allocation tracking (tierStorageUsed)
│   └── types.rs                        # CapacityState, WatermarkConfig structures
│
├── cost/                               # Warm tier cost modeling
│   ├── mod.rs
│   ├── cost_model.rs                   # Cost per byte + cost per access + migration cost
│   ├── pricing.rs                      # Warm tier pricing vs hot/cold alternatives
│   ├── budget.rs                       # Cost-per-byte budget enforcement
│   └── types.rs                        # CostModelResult, CostBudget structures
│
├── migration/                          # Migration planning (bandwidth caps)
│   ├── mod.rs
│   ├── planner.rs                      # MigrationPlan: bytes, bandwidth cap, throttle
│   ├── scheduler.rs                    # Per-tick migration byte budget, backoff under load
│   └── types.rs                        # MigrationPlan, MigrationStatus structures
│
├── lifecycle/                          # Warm tier lifecycle (TTL, retention, grooming)
│   ├── mod.rs
│   ├── ttl.rs                          # Warm tier TTL with FREE/DELETE actions
│   ├── retention.rs                    # Retention policy per record class, minimum storage duration
│   ├── groom.rs                        # Grooming: delete from warm after move to cold
│   └── types.rs                        # TtlPolicy, RetentionPolicy structures
│
├── redundancy/                         # Warm tier durability (replica vs erasure)
│   ├── mod.rs
│   ├── policy.rs                       # Redundancy policy: replication factor or erasure K/M
│   ├── durability_scope.rs             # Local/region durability scope per profile
│   └── types.rs                        # RedundancyPolicy, DurabilityScope structures
│
├── carrier_adapter.rs                  # Opaque Vec<u8> datagram framing to ids-carrier::Endpoint (decision carrier only)
│
├── contracts/                          # Sealed trait contracts (~10 files)
│   ├── mod.rs
│   ├── placement_contract.rs           # trait Placement
│   ├── temperature_contract.rs         # trait Temperature
│   ├── capacity_contract.rs            # trait Capacity
│   ├── promotion_contract.rs           # trait Promotion
│   ├── demotion_contract.rs            # trait Demotion
│   ├── cost_contract.rs                # trait Cost
│   ├── migration_contract.rs           # trait Migration
│   ├── lifecycle_contract.rs           # trait Lifecycle
│   ├── carrier_contract.rs             # trait WarmCarrier (decision framing)
│   └── types.rs                        # Contract-level type re-exports
│
├── invariants/                         # Compile-time/runtime invariants (~8 files)
│   ├── mod.rs
│   ├── temperature_monotonic.rs        # Temperature score monotonicity on scanner tick
│   ├── hysteresis_sound.rs             # Promote threshold > demote threshold
│   ├── capacity_watermark.rs           # Low < high watermark ordering
│   ├── carrier_agnostic.rs             # No std::net/tokio::net in crate
│   ├── crypto_allowlist.rs             # Only allowed crypto algorithms
│   ├── profile_durability.rs           # Demotion respects profile durability scope
│   ├── audit_chain_sound.rs            # Audit chain hash + signature validity
│   └── types.rs                        # Invariant-level type re-exports
│
├── audit/                              # Signed, hash-chained audit log (~6 files)
│   ├── mod.rs
│   ├── audit_entry.rs                  # Placement/promotion/demotion decision entry
│   ├── audit_chain.rs                  # sha256/blake3 hash chain
│   ├── checkpoint.rs                   # Periodic ed25519-signed checkpoint
│   ├── signer.rs                       # ed25519 signing of decisions/checkpoints
│   ├── verifier.rs                     # Offline verification of audit chain
│   └── types.rs                        # AuditEntry, Checkpoint structures
│
├── tests/                              # Property/verification tests (~28 files)
├── benches/                            # (~8 files)
├── docs/                               # (~14 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `src/audit/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### 7. Key Algorithms & Data Structures

- **AccessProfile / temperature score**: `AccessProfile { access_count: u64, last_access_tick: IdsTime, first_access_tick: IdsTime, size_bytes: u64, sequentiality: f64 }`; `temperature_score = w_freq·log(1+access_count) + w_recency·decay(now − last_access_tick) + w_size·size_class`, weights configurable per record class.
- **Classifier**: `classify(score) → HotZone | WarmZone | ColdZone` using hot_threshold ≥ warm_threshold; warm zone = score ∈ [warm_threshold, hot_threshold).
- **Admission decision**: `admit(record, warm_available) → AdmitToWarm | RedirectToCold | Reject(CapacityExceeded)`; considers warm watermark, cost budget, redundancy feasibility.
- **Promotion/demotion with hysteresis**: `promote_if(score ≥ hot_threshold && dwell ≥ min_dwell)`; `demote_if(score ≤ warm_threshold − margin && dwell ≥ min_dwell)`; margin prevents thrashing at the boundary.
- **Watermark capacity**: `low_watermark ≤ tierStorageUsed ≤ high_watermark`; above high → stop promotions + trigger demotions; below low → allow promotions; reservation reserves bytes for expected promotions.
- **Migration plan**: `MigrationPlan { bytes: u64, bandwidth_cap: u64/s, throttle_under_load: bool }`; per-tick byte budget; backoff when foreground I/O saturated (Alluxio tier-management pattern).
- **Cost model**: `cost_per_byte = storage_price_per_gb/month + access_price_per_op × access_freq + migration_price_per_gb × migration_rate`; compares warm vs hot vs cold for cost-minimizing tier subject to SLO.
- **Redundancy policy**: replication factor N (warm: 1+1 like Magic Pocket) or erasure K/M (warm: RS(10,4) like f4, overhead 1.4× vs 3× replication); durability scope local/region.
- **Audit chain**: each placement/promotion/demotion decision = SHA-256/BLAKE3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### 8. Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `PlacementDecision`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait — decision transport only; crate never sends directly.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-time` — monotonic time source for temperature scanner ticks and dwell-time measurement.
- `serde` — serialization of warm-tier envelopes, temperature profiles, audit entries.
- `thiserror` — error types.

No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
|---|-----------|-----------|-------------|
| AC-089.1 | Access-pattern tracking: per-record frequency, recency, size recorded | `AccessProfile` populated on every access; scanner tick recomputes temperature |
| AC-089.2 | Temperature classification: hot/warm/cold zones with configurable thresholds | Classifier returns correct zone per score; unit tests cover boundary values |
| AC-089.3 | Warm tier admission: admit/redirect/reject decisions | `AdmitDecision` correct under below/above/at capacity watermarks |
| AC-089.4 | Promotion/demotion with hysteresis: no thrashing at boundary | Promote threshold > demote threshold + min dwell time; thrash test passes |
| AC-089.5 | Capacity watermarks: low < high; spill to cold above high | `isFull`/`availableBytes` correct; spill decision triggered above high watermark |
| AC-089.6 | Cost model: cost per byte per record class vs hot/cold | Cost model output matches pricing formula; budget enforcement rejects over-budget placements |
| AC-089.7 | Migration plan: bandwidth cap + per-tick budget | `MigrationPlan.bandwidth_cap` enforced; per-tick migration bytes ≤ budget |
| AC-089.8 | Lifecycle: TTL with FREE/DELETE; retention minimum duration | TTL expiry triggers action; minimum storage duration blocks premature demotion |
| AC-089.9 | Redundancy: replication factor or erasure K/M per record | Redundancy policy output matches configured replication/erasure scheme |
| AC-089.10 | All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh | Dependency audit test passes |
| AC-089.11 | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | Allowlist unit test passes |
| AC-089.12 | Every placement/promotion/demotion decision in append-only, hash-chained, ed25519-signed audit log | Audit chain verifies offline; checkpoints signed |

---

### 10. Remaining Uncertainties

- **Concrete temperature thresholds:** hot/warm/cold score thresholds are deployment-tuned; to be validated at integration with `ids-store`/`ids-erasure`.
- **Weights in temperature score:** frequency vs recency vs size weights are workload-dependent; unquantified defaults needed.
- **Watermark ratios:** low/high warm-tier watermark values (e.g., 70%/85%) are capacity-planning decisions; Easy Tier uses 80% as a reference point.
- **Migration bandwidth cap:** default bytes-per-second cap trades warm-tier rebalancing speed vs foreground latency; unquantified.
- **Minimum dwell time:** cooldown between promote/demote cycles prevents thrashing; value (in `IdsTime` ticks) unquantified.
- **Redundancy default for warm tier:** replication (1+1, Magic Pocket) vs erasure coding (RS(10,4), f4) vs hybrid (ELECT α); selection is a deployment decision.
- **Interaction with `ids-consistency-profile` (Phase 060):** demotion must not drop below the record's profile durability scope; exact guard handshake defined here, validated at integration.
- **Dwell time vs access burstiness:** a record with bursty access may oscillate near the boundary; decay function shape (linear vs exponential) unquantified.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
|----------|-----------|-------------|
| Separate crate `ids-warm-tier` (not a folder) | Follows "separate crates instead of more folders" directive; clear policy boundary |
| Temperature score = frequency + recency + size, weighted | Data temperature research (Big Data Mining & Analytics 2024; Mole) shows age+frequency alone insufficient; weights configurable |
| Hysteresis: separate promote/demote thresholds + min dwell time | Prevents thrashing documented in data-temperature research (wasted movement up/down tiers) |
| Low/high watermarks + spill to cold | IBM Easy Tier / Alluxio / HSM capacity management pattern; keeps warm tier within budget |
| Strongest durability on warm tier: replication or erasure (RS(10,4)) | f4 reduced warm effective replication factor 3.6→2.1 with RS(10,4); Magic Pocket warm uses 1+1 replication |
| Migration bounded by bandwidth cap + per-tick budget | f4 asynchronous background migration; Alluxio tier tasks back off under user I/O load |
| Audit: sha256/blake3 hash chain + ed25519-signed checkpoints | Offline-verifiable decision log; tamper evidence |
| Logical clocks only (IdsTime ticks); no wall-clock for temperature/dwell | Clock manipulation cannot break temperature or dwell-time correctness |
| Carrier-agnostic boundary via `ids-carrier::Endpoint`; crate returns decisions, never sends bytes | Preserves IDS architecture; no network stack in trust base |
| Only allowed crypto; `ids-crypto` exposes primitive IDs via `ids-000-aa` | Complies with allowed-crypto rule |

---

### Sources

[^1]: f4: Facebook's Warm BLOB Storage System, USENIX OSDI'14 — warm BLOBs, Reed-Solomon(10,4), effective replication factor 3.6 → 2.8/2.1. https://www.usenix.org/system/files/conference/osdi14/osdi14-paper-muralidhar.pdf
[^2]: Elasticsearch data tiers: hot, warm, cold, frozen — warm tier holds recent-weeks data, queried less frequently, updates infrequent. https://www.elastic.co/docs/manage-data/lifecycle/data-tiers
[^3]: Apache Hadoop 3.5 — Archival Storage, storage policies Hot/Warm/Cold; warm = some replicas on DISK, some on ARCHIVE. https://hadoop.apache.org/docs/current/hadoop-project-dist/hadoop-hdfs/ArchivalStorage.html
[^4]: Ceph cache tiering — cache pool + backing pool, writeback/read-only cache modes, flush from cache to backing tier. https://docs.ceph.com/en/quincy/dev/cache-pool
[^5]: Amazon S3 Intelligent-Tiering — automatic access-pattern-based moves, monitoring + automation charge per object. https://aws.amazon.com/s3/storage-classes/intelligent-tiering/
[^6]: Azure Storage access tiers — hot/cool/cold; cool/cold for less frequently accessed but immediately available data. https://docs.microsoft.com/en-us/azure/storage/blobs/access-tiers-overview
[^7]: Google Cloud Storage classes — Standard/Nearline/Coldline/Archive; Nearline for monthly-or-less access, 30-day minimum; Coldline 90-day minimum. https://developers.google.com/storage/docs/storage-classes
[^8]: Dropbox Magic Pocket cold/warm storage — warm tier 1+1 replication, async cold migration, warm 2× internal region replication. https://dropbox.tech/infrastructure/how-we-optimized-magic-pocket-for-cold-storage
[^9]: ELECT erasure-coding tiering for LSM-trees — hotness-aware redundancy transition, storage saving target α, replication→erasure conversion. https://usenix.org/system/files/fast24-ren.pdf
[^10]: HDFS erasure coding — EC overhead ~50% vs 200% for 3× replication; suitable for warm/cold low-I/O datasets. https://docs.cloudera.com/HDPDocuments/HDP3/HDP-3.1.0/data-storage/content/increasing_storage_capacity_with_hdfs_erasure_coding.html
[^11]: Ceph erasure-coded pools — 4+2 overhead 1.5× vs replication 3.0; cache tier before EC pool. https://github.com/ceph/ceph/blob/master/doc/rados/operations/erasure-code.rst
[^12]: Alluxio tiered storage — memory/SSD/HDD tiers, LRU/LFU/Greedy evictors, TTL, annotation policies, tier management tasks back off under load. https://docs.alluxio.io/os/user/2.7.4/en/core-services/Caching.html
[^13]: GlusterFS tiering volume — watermark-hi/watermark-low, promote/demote frequency, read/write access-count threshold. https://docs.redhat.com/en/documentation/red_hat_gluster_storage/3.4/html/administration_guide/chap-managing_data_tiering-configuring_tiered_volume
[^14]: IBM Easy Tier — warm demotion, cold demotion, warm promotion, extent-level heat values, 80% tier capacity threshold. https://ibm.com/docs/en/ds8000/10.0.0?topic=features-easy-tier
[^15]: Hierarchical storage management — migration policy (LRU/STP/STEP), file grooming, data temperature determines storage hierarchy. https://en.wikipedia.org/wiki/Hierarchical_storage_management
[^16]: Data Temperature Informed Streaming (Big Data Mining and Analytics, 2024) — age+frequency limitation, additional variables for smarter placement, wasted movement up/down tiers. https://www.sciopen.com/article/10.26599/BDMA.2023.9020039
[^17]: Mole: Tier-Specific Hotness Profiling (arXiv:2610.12164) — reaccess-interval histograms, targeted promotion, staging demotion, capacity-based tier thresholds. https://arxiv.org/html/2610.12164
[^18]: Automating Distributed Tiered Storage Management (PVLDB 2019) — tracking and predicting file access patterns, tier placement/migrate up/down/deletion policies. https://arxiv.org/html/1907.02394
[^19]: HotRAP: Hot Record Retention and Promotion for LSM-trees with Tiered Storage (arXiv:2402.02070) — promoting frequently accessed records from slow to fast storage. https://arxiv.org/html/2402.02070v3
[^20]: CloudJump III: Optimizing Cloud Databases for Tiered Storage (SIGMOD 2026) — eviction-centric engine-aware tiering, logical ownership decoupled from physical placement. https://alibabacloud.com/blog/cloudjump-iii-optimizing-cloud-databases-for-tiered-storage-sigmod-2026_603708
[^21]: storgatech PLC SSD tiered storage strategy — warm tier SLOs (read p99 < 20ms, TTL 30–90 days, 0.05%–0.5% access/day). https://storagetech.cloud/designing-tiered-storage-strategies-with-plc-ssds-in-mind
[^22]: DITIS: Distributed Tiered Storage Simulator — tier placement, migrate up/down, deletion policies across Hot/Warm/Cold. https://link.springer.com/article/10.1007/s42979-025-04275-9
[^23]: Aerospike tiered storage guide — warm data = important but not frequently accessed, balance of capacity/cost/accessibility. https://aerospike.com/blog/tiered-storage-guide
[^24]: Purestorage tiered data storage — Tier 2 warm data on SSD+HDD balance, accessed occasionally but readily available. https://purestorage.com/knowledge/what-is-tiered-data-storage.html
[^25]: Apache Ozone storage policies — Hot→SSD, Warm→DISK, Cold→ARCHIVE with fallback. https://ozone.apache.org/docs/core-concepts/replication/write-pipelines
[^26]: ed25519-dalek docs.rs — ed25519 key gen/sign/verify, RFC 8032. https://docs.rs/ed25519-dalek
[^27]: chacha20poly1305 docs.rs — ChaCha20Poly1305/XChaCha20Poly1305 AEAD, RFC 8439. https://docs.rs/chacha20poly1305
[^28]: sha2 RustCrypto — SHA-256 implementation. https://docs.rs/sha2
[^29]: BLAKE3-team BLAKE3 README — BLAKE3-256, SIMD, keyed hash, KDF, XOF. https://github.com/BLAKE3-team/BLAKE3/blob/master/README.md
[^30]: IDS project documents at C:/idfs: prompts.txt entry 89 (lines 1330-1344), ids-000-aa skeleton at C:/idfs/ids-000-aa/, IDS plan format references plan-001.md/plan-048.md/plan-060.md at C:/idfs/plans/

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, uncertainties, and engineering decisions for Phase 089 (ids-warm-tier).
