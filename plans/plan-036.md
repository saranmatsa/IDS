## Phase 036 — Create the ids-accounting crate: Research Findings & Implementation Blueprint

**Status:** Research complete (30 targeted searches executed). No code written, no files modified. This document is the blueprint for implementation.

---

### 1. Executive Summary

Phase 036 creates **`ids-accounting`**, a new separate crate for the IDS project that measures contributed resources (storage, bandwidth, compute, recovery effort) and manages quotas, reservations, commitments, negotiated resource allocations, infrastructure accounting, billing events, cost per byte, and energy per byte.

**Scope of `ids-accounting` (research-verified):**
- **Resource measurement:** sample-based measurement of storage (bytes × time × replica count), bandwidth (bytes transferred, per direction), compute (cycles/time, fuel, invocations), and recovery effort (rebuild/repair work attributed to a contributor).
- **Quotas:** per-tenant / per-contributor hard ceilings on each resource dimension; enforcement so `Σ grants ≤ limit` (escrow bounded-counter pattern) with no overshoot.
- **Reservations:** reserve/commit/release lifecycle — capacity reserved before work begins, committed to actual usage on success, released (fully or partially) on failure.
- **Commitments:** spend-based or usage-based commitments over a term with continuous (use-it-or-lose-it, hourly evaluation, waste) or discontinuous (full-period accumulation, true-up) fulfillment models.
- **Negotiated resource allocations:** contribution-versus-consumption accounting between peers, with mediation-resistant session accounting to prevent balance inflation without actual contribution (PeerMint pattern).
- **Infrastructure accounting:** billing events as immutable, append-only, idempotent usage records; metering pipeline (ingest → deduplicate → aggregate → rate by rate card → bill); cost per byte and energy per byte metrics.
- **Carrier-agnostic:** zero direct networking dependencies. No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).
- **Allowed crypto only:** ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — nothing else (no X25519, no AES-GCM, no RSA).
- **Foundation:** `ids-000-aa` is the master crate holding all canonical types and sealed trait contracts; `ids-accounting` builds on those types, never redefining them.

**Key engineering decisions:**
- New separate crate `ids-accounting` (not more folders inside existing crates), per the IDS project convention.
- Immutable append-only billing-event ledger with idempotency-key deduplication; balances derived, never stored as mutable columns.
- Atomic check-then-reserve for quotas to prevent TOCTOU overshoot; hierarchical (path) quotas where one write is charged to bucket + tenant + root all-or-none.
- Commitment fulfillment modeled as continuous (waste tracked per interval) or discontinuous (true-up at period end), matching the FOCUS commitment categorization.
- Exertion-aware measurement where hardware exposes it (pressure stall time, proportional set size), falling back to volume-based metering otherwise.
- Energy per byte derived from measured dynamic power ÷ bandwidth (pJ/bit-style), never assumed.
- All accounting traffic framed over `ids-carrier::Endpoint`; signatures over canonical payloads so messages survive re-framing by any carrier.

---

### 2. Architecture Overview

`ids-accounting` sits above the resource producers (storage, compute, recovery) and below billing/settlement consumers. It is not a network protocol — it is a local measurement + policy engine that emits bytes (events, quotes, proofs) to be shipped over `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────┐
│                    APPLICATION / TENANT LAYER                    │
│         resource consumers, contributors, commitment holders     │
├──────────────────────────────────────────────────────────────────┤
│                    ids-accounting (NEW CRATE)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐  │
│  │  Measurement │  │    Quota     │  │   Reservation /       │  │
│  │  probes      │  │  Engine      │  │   Commitment Engine   │  │
│  │  (storage,   │  │  (escrow     │  │   (reserve/commit/    │  │
│  │   bandwidth, │  │   bounded    │  │   release, CUD-style  │  │
│  │   compute,   │  │   counter)   │  │   commitments)        │  │
│  │   recovery)  │  └──────────────┘  └───────────────────────┘  │
│  └──────┬───────┘  ┌──────────────┐  ┌───────────────────────┐  │
│         │          │   Allocation │  │   Billing / Metering  │  │
│         │          │   (peer      │  │   Pipeline            │  │
│         │          │   contribution│ │   (ingest, dedup,     │  │
│         │          │   vs        │  │   aggregate, rate,    │  │
│         │          │   consumption)│ │   emit events)        │  │
│  ┌──────▼───────┐  └──────────────┘  └───────────┬───────────┘  │
│  │   Ledger     │  ┌──────────────┐  ┌──────────▼──────────┐  │
│  │   (append-   │  │   Attribution│  │   Export / Proof    │  │
│  │   only,      │  │   (credit,   │  │   (signed envelopes,│  │
│  │   idempotent)│  │   fair-share)│  │   merkle summaries)   │  │
│  └──────┬───────┘  └──────────────┘  └──────────┬──────────┘  │
│         └───────────────┬───────────────────────┘              │
│                         │                                        │
│              ┌──────────▼──────────┐                             │
│              │   ids-carrier::Endpoint│                            │
│              │  CarrierType + Vec<u8>│                            │
│              │ (all traffic only)   │                             │
│              └──────────────────────┘                             │
├──────────────────────────────────────────────────────────────────┤
│                     ids-000-aa (MASTER CRATE)                      │
│         canonical types + sealed trait contracts (ids-000-aa)     │
└──────────────────────────────────────────────────────────────────┘
```

**Data flow of a measured operation:**
1. A resource producer (storage write, compute task, recovery job) emits a raw sample: contributor, tenant, resource dimension, quantity, unit, timestamp, optional exertion signal.
2. The measurement probe validates the sample against the sealed type contracts from `ids-000-aa` and writes it to the append-only measurement log.
3. The quota engine checks the reservation atomically (check + reserve against granted budget per dimension); admits or rejects before any state effect.
4. On completion, the reservation is committed to actual usage (or released on failure); the usage is aggregated into the tenant's period totals.
5. Billing events are emitted (idempotency-keyed), rated by the rate card (cost per byte, energy per byte), and reconciled against commitments (waste or true-up).
6. All cross-node traffic (reports, proofs, quotes, commitments) is framed over `ids-carrier::Endpoint`; `ids-accounting` never opens a socket or imports a transport crate.

**No end-to-end path assumption:** accounting may run offline and sync later; events are stored locally and shipped when a carrier opportunity exists.

---

### 3. Detailed Research Dimensions

#### 3.1 Resource Measurement Dimensions (storage, bandwidth, compute, recovery effort)

Research on cloud metering (Azure Blob Storage meters: data storage per GB/month, operations per transaction, data transfer per GB; S3-style metering: bytes uploaded, stored, egressed, operations counted separately) shows infrastructure billing decomposes into independent meters, each with its own unit and aggregation:
- **Storage:** bytes × duration (GB-months), plus replica count multiplier for replicated storage (SpaceDB-meter pattern: storage = bytes × time × replica_count). Tier changes (hot/cold/archive) require the meter to know the tier per object.
- **Bandwidth:** bytes transferred, per direction (ingress/egress/transit), corroborated bilaterally — neither side can unilaterally inflate a transit bill (SpaceDB-meter: transit = min(server-claimed-served, consumer-acknowledged-received)).
- **Compute:** cycles/time, fuel units, invocations; priced per megafuel + per invocation in the reference design.
- **Recovery effort:** rebuild/repair work attributed to the contributor whose data triggered it; measured as device/IO effort per byte recovered.

Pressure Stall Information (PSI, Linux kernel) provides stall-time pressure metrics for CPU, memory, and IO — the difference between *utilization* (resource in use) and *pressure* (work delayed waiting). PSI `some`/`full` stall fractions over a time window are the exertion signal: charging for the work a request actually induced, not just bytes requested (USENIX HotCloud '11 exertion-based cloud storage billing argues access billing should reflect disk-time exertion, not byte/I/O count). Memory attribution uses proportional set size (PSS, /proc/<pid>/smaps_rollup) so shared pages are divided among sharers rather than double-counted as RSS (MemMux pattern).

**Decision for IDS:** measurement samples carry `(quantity_bytes_or_units, exertion_stall_ms, tier, replica_count)`; the rate card decides which factor prices which resource. Pressure/PSS used where available, volume-based metering as the universal fallback.

#### 3.2 Quota Model & Enforcement

Quota = upper bound on resource consumption for a principal over a period or in total. Research surfaces three complementary patterns:
- **Escrow bounded counter (`bcounter`, docs.rs):** a global quota lends slices (grants) to nodes; a node admits a write only against rights it holds locally (`net used + amount ≤ granted`); the invariant `Σ grants ≤ limit` guarantees the limit is never exceeded with zero write-path coordination. Hierarchical `BCounterMap` charges one write against several scopes (bucket, tenant, root) all-or-none. `Δ = 0` gives strict overshoot-free enforcement; `Δ > 0` trades bounded overshoot for fewer false denials.
- **Token bucket / fixed window / rolling window:** token bucket (rate + burst capacity) allows bursts while bounding average rate (Redis token-bucket pattern; AWS fairness article); fixed-window quotas reset per window (simple, burst-at-boundary); rolling-window quotas use a lookback (recalculated per request, never fully resets — Apigee). Sliding-window quotas with exact admission (YARN research) bound peak error in both directions by visiting rate changes at window edges.
- **budgetkernel:** deterministic multi-dimension budget kernel (tokens, ms, bytes, calls, memory, custom units) returning `Continue / Warn(dim) / Exhausted(dim)`; zero heap on the hot path; the caller owns measurement, persistence, and coordination.

**Decision for IDS:** quotas are escrow-style bounded counters per (principal, resource, scope) with `Σ grants ≤ limit` invariant, hierarchical all-or-none path charging, plus a token-bucket rate shaper for burst smoothing. Verdict: `admit / warn-approaching / deny-exhausted`.

#### 3.3 Reservations & Commitments

**Reservations (reserve/commit/release):** capacity reserved before work begins (`reserve(estimate)`), committed to the actual amount on success (`commit(actual)`, releasing `estimate − actual`), fully released on failure (Cbox reserve/commit/release lifecycle; Kong metered entitlements). Atomic "check + reserve" prevents TOCTOU races where two concurrent claims both take the last unit. Hard enforcement denies at the limit; soft enforcement allows with warning + recorded overage.

**Commitments:** a principal commits to spend or use a minimum level of resources for a term (Google Cloud committed use discounts: resource-based commitments, 1 or 3 years, auto-renewal). FOCUS specification categorizes commitments:
- **Spend-based** ("I will spend $1M") vs **usage-based** ("I will use 500 vCPU-hours") — determines whether Cost or Quantity measures fulfillment.
- **Continuous fulfillment** (resource reservations): use-it-or-lose-it, evaluated every interval (hourly); unused capacity = waste reported as it occurs.
- **Discontinuous fulfillment** (enterprise spend agreements): usage accumulates over the full period; end-of-term true-up settles the difference between committed and actual consumption.

**Decision for IDS:** commitments carry `(amount, unit, term_start, term_end, fulfillment_model: Continuous | Discontinuous)`; continuous commitments compute waste per interval; discontinuous commitments accrue and true-up at period end; commitments feed the quota engine as pre-paid grant pools.

#### 3.4 Negotiated Resource Allocations (contribution vs consumption)

PeerMint (decentralized, secure accounting for P2P applications): tracks contribution and consumption of resources per peer; aggregated accounting enforces fair sharing and underpins charging/payment. Session mediation peers keep session information about transactions between peers, minimizing collusion where peers inflate balances without contributing. Credit-based cycle-sharing systems treat credits as digitally signed entities conveying contribution information; distributed feedback lets nodes check credit history before exchanging.

Credit fairness (LendRecoup): agents contribute endowment to a shared pool; credits gained/lost one-to-one with resources lent/borrowed; allocation prioritizes agents with low cumulative allocations — credit-fair and Pareto efficient.

**Decision for IDS:** allocation = contribution credit balance vs consumption debit; negotiated allocations are signed agreements between contributors and consumers; mediation-resistant accounting requires corroboration (bilateral transit corroboration for bandwidth; storage proofs) before crediting a contribution.

#### 3.5 Billing Events & Metering Pipeline

Usage-based billing architecture (Kong, Stripe Edgee, Lago, Twilio): product action → event → ingestion → metering → rating → invoice. Core event fields: event ID, customer/tenant ID, event type, quantity, unit, timestamp, properties (dimensions), idempotency key, schema version.

Two guarantees: **at-least-once delivery** (retries, durability) and **exactly-once accounting** (dedupe by event id + idempotency key; DB uniqueness constraints; ON CONFLICT DO NOTHING). Dedup windows are time-bounded (OpenMeter: 32-day default window — keys need not be stored forever). Aggregation: sum/count/max/unique/percentile meters over time windows (hourly pre-aggregation before reporting to billing providers). Stripe usage-based billing used Flink for exactly-once stream processing with a 30-second tumbling fast path (alerts) and a 5-minute slow path (disk ledger, invoices, analytics).

Rate card: prices per dimension — storage per GiB-month, compute per megafuel + per invocation, transit per GiB; multiply-before-divide so sub-unit usage never rounds to zero; `estimate(usages)` pre-deploy cost check.

**Decision for IDS:** billing events are append-only, idempotency-keyed records; meters aggregate per (tenant, dimension, window); rate card rates cost per byte, energy per byte; exactly-once semantics via dedup at ingestion.

#### 3.6 Exertion-Based vs Volume-Based Accounting

USENIX HotCloud '11 (Wachs): current cloud storage billing charges for bytes or I/Os requested, but provider cost is exertion (disk time) which varies by workload locality and transfer size by orders of magnitude. Exertion-based billing (disk time per tenant) is fairer but harder to measure. PSI stall fractions and PSS memory attribution are the kernel-level primitives for this.

**Decision for IDS:** samples record both volume and exertion; the rate card selects per resource: volume for transit/storage, exertion (stall time) for contended compute/recovery where available.

#### 3.7 Energy per Byte

Energy metrics for data networks (IETF draft): Power Consumption per Data Rate (PCDR = W / Gbps), PUE, Network Equipment Energy Efficiency, Energy Proportionality Coefficient. HPC network energy studies isolate dynamic energy per bit (pJ/bit): e.g., sender NIC+PCIe ≈ 4.1 pJ/bit on Perlmutter; storage efficacy measured as MB/s per W (= MB/J). Storage power metrics: kWh/TB annual energy per storage capacity.

**Decision for IDS:** energy per byte = measured dynamic energy ÷ bytes served, component-level (NIC, disk, CPU) where sensors exist; otherwise a configured per-component joule-per-byte coefficient. Never assumed from wall-clock time.

#### 3.8 Immutable Ledger & Double-Entry Accounting

Immutable ledger patterns (Amazon QLDB journal-first, flow-ledger append-only with SHA-256 export hash, Modern Treasury double-entry): every posting is append-only, sequence-ordered, hash-chained to the previous posting; corrections are new compensating postings, never edits. Four invariants: gapless sequence ordering, zero-sum postings, non-destructive correction, independent verifiability. Usage-ledger pattern (reservation-aware AI ledger): Execution Truth / Rated Truth / Settlement Truth reconciliation; corrections flow into an append-only Adjustment Ledger; time-travel via `created_at`/`superseded_at` range conditions.

**Decision for IDS:** billing-event ledger is append-only with SHA-256/BLAKE3 hash chaining; balances are derived by aggregation, never stored as mutable columns; corrections append compensating events.

#### 3.9 Attribution & Fair Sharing

Fair-share (Flux): ratio of allocated vs consumed resources, decaying historical usage so short-term spikes don't permanently penalize. Credit fairness (LendRecoup): one-to-one credits with lent/borrowed resources; prioritize low cumulative allocation. PeerMint: contribution/consumption accounts with redundancy options (non-redundant = one peer holds an account; redundant = replicated over several peers).

**Decision for IDS:** attribution tracks contribution credit vs consumption debit per contributor; fair-share weighting influences future allocation priority; account redundancy is configurable (single holder vs replicated).

#### 3.10 Reconciliation & Audit

Two-ledger model (arxiv:2611.06989): settlement ledger (provider billing, SKU grain) joined to attribution ledger (invocation-grain usage); reconciliation in both directions finds unmetered residual and provider billing discrepancies. Metering reconciliation compares local totals vs pushed totals per (item, period) with idempotent usage records; a reconciler proves final state matches the settlement source of truth.

**Decision for IDS:** periodic reconciliation of metered usage vs committed amounts vs settled billing; waste and overage reported per interval; audit trail is the append-only ledger itself.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Inflated self-reported usage (peer claims contribution it never gave) | Bilateral corroboration (transit = min(served, received)); session-mediation accounting (PeerMint pattern); storage/repair proofs before crediting |
| Quota overshoot via concurrent reservations | Atomic check + reserve; escrow invariant `Σ grants ≤ limit`; hierarchical all-or-none path charging |
| Double counting from retried/duplicated events | Idempotency keys + dedup window; uniqueness constraints at insert; exactly-once accounting |
| Ledger tampering / balance drift | Append-only, hash-chained postings (SHA-256/BLAKE3); balances derived by aggregation; compensating corrections only |
| Quota exhaustion used as denial-of-service | Hard vs soft enforcement modes; overage recording; admit/warn/deny verdicts; fail-closed on policy error |
| Commitment default (committed use not consumed) | Continuous waste tracking per interval; discontinuous true-up at period end |
| Energy/byte or exertion figures fabricated | Measurement probes are the only authors of samples; signed sample envelopes; verifier re-checks against rate-card bounds |
| Carrier eavesdropping/tampering of accounting traffic | All traffic via `ids-carrier::Endpoint`; payloads signed with ed25519 over canonical encoding; signatures survive re-framing |
| Clock dependence for quota windows | Window boundaries keyed to logical/sequence time where possible; wall clock never the sole authority for validity |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 036 |
|-------|-------------------|
| `ids-000-aa` | Master crate: all canonical types and sealed trait contracts (`TenantId`, `ContributorId`, `ResourceId`, `Amount`, `Quantity`, `Timestamp`, sealed traits) reused verbatim |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — the only traffic boundary; all accounting reports, proofs, and commitments shipped as Endpoint payloads |
| `ids-crypto` (workspace) | Only allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-storage` | Storage measurement hook: bytes stored × time × replica count, tier, exertion |
| `ids-compute` | Compute measurement hook: cycles/fuel/invocations per task |
| `ids-recovery` | Recovery-effort measurement hook: rebuild/repair work attributed per contributor |
| (persistent ledger backend) | Append-only billing-event and measurement log storage (sled or equivalent, feature-gated) |

**Integration points:**
- Measurement probes subscribe to resource-producer events locally; no cross-crate network calls.
- Quota engine exposes `reserve(estimate) → Reservation`, `commit(reservation, actual)`, `release(reservation)`; commit/release append to the ledger.
- Billing pipeline emits `BillingEvent { event_id, tenant, meter, quantity, unit, timestamp, idempotency_key }`; events are framed over `ids-carrier::Endpoint` for export.
- Commitments are granted against quotas: a commitment pre-pays a grant pool; consumption draws from it; waste/true-up computed per fulfillment model.
- No dependency on `std::net`, `tokio::net`, libp2p, or any transport — carrier-agnostic per I-CARR-001-equivalent stance for IDS.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-accounting` (new crate; depends on `ids-000-aa`, `ids-carrier`, workspace crypto; `serde`, `thiserror`, `bytes`, `blake3`, `parking_lot`/`dashmap`, `time`, `proptest`, `criterion`)

```
ids-accounting/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol defaults (dedup window, quota deltas)
├── error.rs                        # AccountingError (thiserror, comprehensive)
├── features.rs                     # Feature flags (persistent, metrics, fuzzing)
│
├── measurement/                    # Resource sample collection
│   ├── mod.rs
│   ├── types.rs                    # Sample, ResourceDimension, Unit, Exertion
│   ├── storage.rs                  # Storage sample: bytes × time × replica_count × tier
│   ├── bandwidth.rs                # Bandwidth sample: bytes per direction
│   ├── compute.rs                  # Compute sample: fuel, invocations, cycles
│   ├── recovery.rs                 # Recovery-effort sample: rebuild/repair work
│   ├── probe.rs                    # Measurement probe: subscribes to producer events
│   ├── psi.rs                      # Pressure-stall-information exertion readings
│   ├── pss.rs                      # Proportional-set-size memory attribution
│   ├── energy.rs                   # Energy-per-byte derivation (joules/byte, pJ/bit)
│   ├── sample_envelope.rs          # Signed canonical sample envelope (ed25519/sha256/blake3)
│   └── measurement_types.rs        # SampleId, ProbeId structures
│
├── quota/                          # Quota model and enforcement
│   ├── mod.rs
│   ├── quota.rs                    # Quota: limit, granted, used per (principal, resource, scope)
│   ├── escrow.rs                   # Escrow bounded counter: Σ grants ≤ limit, local admission
│   ├── grant.rs                    # Grant slices lent to nodes/contributors
│   ├── token_bucket.rs             # Token bucket rate shaper (rate + burst)
│   ├── window.rs                   # Fixed / rolling / sliding window quota accounting
│   ├── hierarchical.rs             # Path quotas: all-or-none charging across scopes
│   ├── admission.rs                # Admit / warn-approaching / deny-exhausted verdicts
│   ├── quota_types.rs              # QuotaId, Verdict, BudgetConfig
│   └── quota_proptests.rs          # Property tests: no accepted write exceeds limit
│
├── reservation/                    # Reserve / commit / release lifecycle
│   ├── mod.rs
│   ├── reservation.rs              # Reservation state: reserved, committed, released
│   ├── lifecycle.rs                # reserve(estimate) → commit(actual) / release()
│   ├── atomic_reserve.rs           # Atomic check-and-reserve (TOCTOU-free)
│   ├── hard_enforcement.rs         # Deny at limit
│   ├── soft_enforcement.rs         # Allow with warning + overage record
│   └── reservation_types.rs        # ReservationId, OverdraftDecision
│
├── commitment/                     # Commitments and fulfillment
│   ├── mod.rs
│   ├── commitment.rs               # Commitment: amount, unit, term, fulfillment model
│   ├── spend_based.rs              # Spend-based commitment ("spend $1M")
│   ├── usage_based.rs              # Usage-based commitment ("use 500 vCPU-hours")
│   ├── continuous.rs               # Continuous fulfillment: hourly waste tracking
│   ├── discontinuous.rs            # Discontinuous fulfillment: period true-up
│   ├── fulfillment.rs              # Fulfillment evaluation against period usage
│   ├── waste.rs                    # Waste tracking (unused committed capacity)
│   ├── true_up.rs                  # End-of-period settlement
│   └── commitment_types.rs         # CommitmentId, FulfillmentModel
│
├── allocation/                     # Negotiated resource allocations
│   ├── mod.rs
│   ├── allocation.rs               # Contribution credit vs consumption debit
│   ├── credit.rs                   # Credit balance per contributor
│   ├── mediation.rs                # Session-mediation-resistant accounting (PeerMint)
│   ├── corroboration.rs            # Bilateral corroboration (transit min(served, received))
│   ├── fair_share.rs               # Fair-share weighting with decayed history
│   ├── redundancy.rs               # Account redundancy: single vs replicated holder
│   └── allocation_types.rs         # AllocationId, ContributionRecord
│
├── billing/                        # Billing events and metering pipeline
│   ├── mod.rs
│   ├── event.rs                    # BillingEvent (append-only, idempotency-keyed)
│   ├── ingestion.rs                # Event ingestion with deduplication
│   ├── dedup.rs                    # Idempotency-key dedup with time-bounded window
│   ├── meter.rs                    # Meters: sum/count/max/unique/percentile per window
│   ├── aggregation.rs              # Time-window aggregation (hourly rollups)
│   ├── rate_card.rs                # Rate card: cost per byte, energy per byte
│   ├── rating.rs                   # Apply rate card to aggregated usage
│   ├── invoice.rs                  # Invoice line items from rated usage
│   ├── estimate.rs                 # Pre-deploy cost estimate (multiply-before-divide)
│   └── billing_types.rs            # EventId, MeterId, RateCardId
│
├── ledger/                         # Immutable append-only accounting ledger
│   ├── mod.rs
│   ├── journal.rs                  # Append-only journal, gapless sequence ordering
│   ├── entry.rs                    # Ledger entry: debit/credit, zero-sum invariant
│   ├── hash_chain.rs               # SHA-256/BLAKE3 hash chaining to previous entry
│   ├── balance.rs                  # Derived balances (aggregation, never stored mutable)
│   ├── correction.rs               # Compensating correction entries (append-only)
│   ├── reconciliation.rs           # Two-ledger reconciliation: metered vs committed vs settled
│   ├── audit.rs                    # Audit trail: every billed unit traceable
│   └── ledger_types.rs             # EntryId, Posting, CorrectionId
│
├── attribution/                    # Contribution vs consumption attribution
│   ├── mod.rs
│   ├── attribution.rs              # Attribution engine
│   ├── contributor.rs              # Contributor account
│   ├── consumer.rs                 # Consumer account
│   ├── credit_balance.rs           # Credit vs debit balance
│   ├── history.rs                  # Decayed historical usage for fair-share
│   └── attribution_types.rs        # AttributionId
│
├── export/                         # Export and proofs over carrier
│   ├── mod.rs
│   ├── envelope.rs                 # Carrier-framed envelope over ids-carrier::Endpoint
│   ├── signer.rs                   # ed25519 signing of canonical payloads
│   ├── verifier.rs                 # Signature verification on receipt
│   ├── proof.rs                    # Merkle summaries of usage periods
│   ├── export_types.rs             # EnvelopeId, ProofId
│   └── export_proptests.rs         # Property tests: envelope integrity
│
├── crypto/                         # Allowed-primitive boundary (policy only)
│   ├── mod.rs
│   ├── primitives.rs               # ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305
│   ├── policy.rs                   # Rejects any non-listed algorithm at compile time
│   └── crypto_types.rs             # AlgorithmId
│
├── wire/                           # Serialization (no transport)
│   ├── mod.rs
│   ├── format.rs                   # Versioned canonical encoding (CBOR/bincode family)
│   ├── checksum.rs                 # Payload checksums (sha256/blake3)
│   └── wire_types.rs               # Envelope framing for Endpoint Vec<u8>
│
├── persistence/                    # Optional durable backend
│   ├── mod.rs
│   ├── store.rs                    # Persistent ledger/measurement store adapter
│   ├── snapshot.rs                 # Periodic state snapshots
│   └── gc.rs                       # Retention-bound garbage collection of dedup keys
│
├── config/                         # Configuration
│   ├── mod.rs
│   ├── quota_config.rs             # Quota defaults, delta overshoot allowance
│   ├── commitment_config.rs        # Commitment term defaults
│   ├── rate_card_config.rs         # Rate card defaults (cost/energy per byte)
│   ├── dedup_config.rs             # Dedup window configuration
│   └── export_config.rs            # Export cadence over Endpoint
│
├── metrics/                        # Metrics & tracing
│   ├── mod.rs
│   ├── measurement_metrics.rs      # Samples collected per dimension
│   ├── quota_metrics.rs            # Admit/warn/deny rates
│   ├── billing_metrics.rs          # Events ingested, deduplicated, rated
│   └── commitment_metrics.rs       # Waste, true-up delta
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_sample.rs
│   ├── fuzz_quota.rs
│   ├── fuzz_event.rs
│   └── fuzz_envelope.rs
│
├── tests/                          # Integration & adversarial tests
│   ├── mod.rs
│   ├── measurement_tests.rs        # Sample round-trip per dimension
│   ├── quota_tests.rs              # Escrow invariant, hierarchical charging
│   ├── reservation_tests.rs        # Reserve/commit/release lifecycle
│   ├── commitment_tests.rs         # Continuous/discontinuous fulfillment, true-up
│   ├── allocation_tests.rs         # Credit/debit, corroboration, fair-share
│   ├── billing_tests.rs            # Ingestion, dedup, aggregation, rating
│   ├── ledger_tests.rs             # Append-only, hash chain, correction
│   ├── export_tests.rs             # Envelope sign/verify over Endpoint bytes
│   ├── adversarial_tests.rs        # Duplicate events, overshoot attempts, tampered samples
│   ├── determinism_tests.rs        # Same inputs → identical ledger state
│   ├── property_tests.rs           # Proptest semilattice/budget invariants
│   └── replay_tests.rs             # Replay attack on billing events
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── measurement_bench.rs        # Sample ingestion cost
│   ├── quota_bench.rs              # Admission latency
│   ├── billing_bench.rs            # Event ingestion + dedup throughput
│   └── ledger_bench.rs             # Append + hash-chain cost
│
├── spec/                           # Normative specifications
│   ├── measurement_spec.md         # Resource measurement model
│   ├── quota_spec.md               # Quota & escrow invariants
│   ├── reservation_spec.md         # Reserve/commit/release lifecycle
│   ├── commitment_spec.md          # Commitment fulfillment models
│   ├── billing_spec.md             # Billing event & metering pipeline
│   ├── ledger_spec.md              # Immutable ledger invariants
│   └── threat_model.md             # Threat model document
│
├── docs/                           # Documentation
│   ├── architecture.md             # Accounting architecture & layering
│   ├── integration_guide.md        # Integrating measurement probes & exporters
│   ├── rate_card_guide.md          # Defining cost/energy per byte rates
│   ├── design_rationale.md         # Why escrow quotas, append-only ledger, etc.
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── sample_vectors.json
    ├── quota_vectors.json
    ├── event_vectors.json
    └── ledger_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — measurement probes per resource dimension, escrow quota engine, reservation lifecycle, commitment fulfillment models (continuous/discontinuous), allocation/corroboration, metering pipeline (ingest/dedup/aggregate/rate), immutable ledger, attribution, carrier export, crypto-policy boundary, persistence, tests, fuzz, benchmarks, specs, docs, test vectors. The count reflects genuine state-machine and storage complexity of resource accounting; no stubs, every file holds production-grade substance with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Measurement Sample
```
Sample {
    contributor: ContributorId,       // ids-000-aa canonical type
    tenant: TenantId,
    dimension: Dimension,             // Storage | Bandwidth | Compute | Recovery
    quantity: u128,                   // bytes or units
    exertion_ms: u64,                 // pressure stall time where available
    tier: Tier,                       // hot/warm/cold/archive (storage)
    replica_count: u32,               // for replicated storage
    timestamp: Timestamp,
    provenance: ProbeId,
}
```
Expiry/retention keyed to sample creation time, never to RTT (carrier-agnostic).

#### 7.2 Escrow Quota (bounded counter)
```
Quota { limit: u128, granted: u128, used: u128 }     // Σ grants ≤ limit globally
acquire(grant, amount) admitted ⟺ grant.used + amount ≤ grant.amount
commit(grant, amount): grant.used += amount
reclaim(grant, amount): grant.amount -= amount; Σ grants still ≤ limit
```
Hierarchical: one write charged to bucket + tenant + root all-or-none; if any scope is short, nothing is charged.

#### 7.3 Token Bucket Rate Shaper
```
bucket.tokens = min(capacity, tokens + now − last_refill × rate)
if tokens ≥ cost: tokens −= cost; ADMIT else: DENY (retry later)
```

#### 7.4 Reservation Lifecycle
```
reserve(estimate):  quota.admit(estimate) → Reservation { estimate }
commit(res, actual):  actual ≤ estimate; quota.charge(actual); release estimate − actual
release(res):  quota.release(res.estimate − res.charged)   // failure path
```

#### 7.5 Commitment Fulfillment
- Continuous: per interval, `waste = max(0, committed − used)`; use-it-or-lose-it.
- Discontinuous: `total_used` accumulates over term; `true_up = committed − total_used` at period end.

#### 7.6 Allocation / Corroboration
```
transit_billed = min(server_claimed_served_bytes, consumer_acknowledged_bytes)
credit_delta = contributed − consumed   // one-to-one with resource units
```

#### 7.7 Billing Event Pipeline
```
event = { event_id, tenant, meter, quantity, unit, ts, idempotency_key }
dedup: seen(event_id, idempotency_key) within window → drop duplicate
meter(tenant, dimension, window) += quantity
rated = rate_card.price(dimension, quantity)   // cost per byte, energy per byte
ledger.post(debit: tenant cost, credit: provider accrual)  // zero-sum
```

#### 7.8 Immutable Ledger
```
Entry { seq, prev_hash, postings[], hash = blake3(seq || prev_hash || postings) }
balance(tenant) = Σ credits − Σ debits over all entries   // derived, never stored
Correction = new compensating entry linked to original (never edits history)
```

#### 7.9 Carrier Export Envelope
```
Envelope { payload: Vec<u8>, sig: ed25519_signature over canonical(payload) }
shipped via ids-carrier::Endpoint { CarrierType, Vec<u8> = serialized envelope }
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types + sealed trait contracts (master crate) |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) — sole traffic boundary |
| `ids-crypto` (workspace) | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only |
| `ids-storage` / `ids-compute` / `ids-recovery` | Measurement hooks per resource dimension |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Hashing (already in workspace) |
| `parking_lot` / `dashmap` | Concurrent quota and ledger indexes |
| `time` | Clock abstraction (no RTT dependence) |
| `proptest` | Property-based testing |
| `criterion` | Benchmarks |

**Prohibited (explicitly excluded):** `std::net`, `tokio::net`, libp2p, and any crate implementing IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` only.

**Crypto policy:** only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — a compile-time policy rejects anything else (no X25519, AES-GCM, RSA).

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement Method |
|---|-----------|-------------------|
| AC-036.1 | Storage measurement | Sample records bytes × time × replica_count × tier; round-trip test per dimension |
| AC-036.2 | Bandwidth measurement | Bilateral corroboration: billed = min(served, received); unit test |
| AC-036.3 | Compute measurement | Fuel + invocations + cycles sample; unit test |
| AC-036.4 | Recovery-effort measurement | Rebuild/repair work attributed to triggering contributor; unit test |
| AC-036.5 | Escrow quota invariant | `Σ grants ≤ limit` never violated; property test, 100k iterations |
| AC-036.6 | Hierarchical all-or-none charging | Write charged to bucket+tenant+root or none; adversarial test (one scope short) |
| AC-036.7 | Reservation lifecycle | reserve/commit/release; commit(actual ≤ estimate), release on failure; state-machine test |
| AC-036.8 | Commitment fulfillment | Continuous waste per interval; discontinuous true-up at period end; unit tests both models |
| AC-036.9 | Billing event dedup | Duplicate event_id/idempotency_key counted once; exactly-once accounting test |
| AC-036.10 | Meter aggregation | Sum/count/max meters per (tenant, dimension, window); aggregation accuracy test |
| AC-036.11 | Rating | Cost per byte and energy per byte from rate card; multiply-before-divide no sub-unit round-to-zero; unit test |
| AC-036.12 | Immutable ledger | Append-only, hash chain verifiable, balances derived; tamper detection test |
| AC-036.13 | Attribution | Credit = contributed − consumed, one-to-one; fair-share weighting; unit test |
| AC-036.14 | Carrier agnosticism | Compiles/behaves identically with zero direct std::net/tokio-net/libp2p/TCP/UDP/QUIC/HTTP/DNS dependencies; all traffic via ids-carrier::Endpoint; dependency-audit test |
| AC-036.15 | Crypto policy | Only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 accepted; any other algorithm rejected at the policy boundary; unit test |
| AC-036.16 | Envelope sign/verify | ed25519 signature over canonical payload survives Endpoint re-framing; round-trip test |
| AC-036.17 | Exertion fallback | Pressure/PSS used where available, volume-based metering otherwise; config test |
| AC-036.18 | Performance | Quota admission ≤ configurable budget (e.g., ≤1 ms) on reference hardware; bench |
| AC-036.19 | Property tests | 100,000+ proptest iterations on budget/ledger invariants; pass/fail |
| AC-036.20 | Adversarial tests | Duplicate events, overshoot attempts, tampered samples all fail-safe; pass/fail |

---

### 10. Remaining Uncertainties

1. **Measurement source:** which resource producers emit samples and through what in-crate hook (push vs pull, sampling frequency) — defined per producer crate, not by this plan.
2. **Durable backend:** optional persistent ledger (sled vs pure in-memory); durability requirements for multi-period accounting are undefined.
3. **Dedup window length:** OpenMeter precedent uses 32 days; the IDS-specific window (billing period vs dispute window) is a product decision.
4. **Rate card governance:** who updates cost-per-byte / energy-per-byte rates, and how versioning affects already-issued invoices — unresolved.
5. **Commitment default handling:** what happens when a commitment is not consumed (waste write-off vs carry-over) needs policy, not research.
6. **Account redundancy:** single-holder vs replicated accounts (PeerMint non-redundant vs redundant) — trade-off between consistency and availability unquantified.
7. **Energy coefficients:** without per-component power sensors, joule-per-byte coefficients are configuration, not measurement; source of coefficients undefined.
8. **Reconciliation cadence:** how often metered usage is reconciled against committed/settled amounts — deployment-specific.
9. **Export cadence over Endpoint:** how frequently signed envelopes/proofs are shipped to peers — unmeasured.
10. **Formal verification scope:** which invariants (escrow Σ grants ≤ limit, ledger zero-sum) merit mechanized proof vs property testing.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-accounting` | Project convention: separate crates for new capabilities; keeps existing crates focused |
| Escrow bounded counter (`Σ grants ≤ limit`) | Strict overshoot-free quota with zero write-path coordination (bcounter / Balegas et al.) |
| Atomic reserve/commit/release | Prevents TOCTOU overshoot; release-on-failure avoids leaked capacity |
| Continuous vs discontinuous commitment models | Matches FOCUS commitment categorization (spend vs usage; hourly waste vs true-up) |
| Append-only hash-chained ledger; balances derived | Immutable audit trail; corrections never edit history (QLDB / flow-ledger pattern) |
| Idempotency keys + time-bounded dedup window | Exactly-once accounting over at-least-once delivery (OpenMeter / Stripe / Lago) |
| Cost per byte + energy per byte rate card | Directly ties billing to infrastructure cost; multiply-before-divide avoids sub-unit round-to-zero |
| Exertion (PSI/PSS) where available, volume fallback | Fairer access billing where hardware exposes it (HotCloud '11 exertion argument) |
| Bilateral corroboration for transit | Neither side can unilaterally inflate a bill (SpaceDB-meter pattern) |
| All traffic via ids-carrier::Endpoint; signed envelopes | Carrier-agnostic; signatures survive re-framing; no transport assumptions |
| Crypto restricted to ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 | IDS allowed-crypto policy; compile-time policy boundary rejects others |
| ids-000-aa canonical types only | Sealed trait contracts are the single source of truth; no redefinition |

---

### 12. Sources Consulted (30 targeted searches)

- PeerMint — Decentralized and Secure Accounting for Peer-to-Peer Applications (contribution/consumption accounting, session mediation): https://link.springer.com/article/10.1007/11422778_4
- `bcounter` — escrow bounded counter for distributed capacity quotas, hierarchical path quotas, `Σ grants ≤ limit`: https://docs.rs/crate/bcounter/latest
- Bounded counter escrow (Balegas et al.): https://arxiv.org/abs/1503.09052
- `budgetkernel` — deterministic multi-dimension budget accounting kernel: https://lib.rs/crates/budgetkernel
- SpaceDB Layer 6 metering & settlement (storage bytes×time×replicas, compute fuel+invocations, bilateral transit corroboration, rate card, budget): https://crates.io/crates/spacedb-meter
- PSI — Pressure Stall Information (CPU/memory/IO stall time, exertion vs utilization): https://docs.kernel.org/accounting/psi.html
- MemMux — memory attribution by proportional set size (PSS), Rust runtime governance: https://arxiv.org/html/2610.07257v2
- Credit Fairness: Online Fairness in Shared Resource Pools (LendRecoup, credit = lent/borrowed one-to-one): https://arxiv.org/html/2601.17944
- Fair-Share Calculation (Flux accounting, allocated vs consumed ratio, decayed history): https://flux-framework.readthedocs.io/projects/flux-accounting/en/latest/components/fair-share.html
- Resource Quotas (Kubernetes namespace aggregate resource constraints): https://kubernetes.io/docs/concepts/policy/resource-quotas
- Kueue two-phase admission (quota reservation then topology check): https://github.com/kubernetes-sigs/kueue/blob/main/site/content/en/docs/concepts/admission.md
- FOCUS Specification — Commitment Categorization (spend vs usage, continuous vs discontinuous fulfillment, waste, true-up): https://focus.finops.org/docs/specification/v1-4/sections/appendix/contract-commitments
- Google Cloud Committed Use Discounts (resource-based commitments, 1–3 year terms, auto-renewal): https://docs.cloud.google.com/compute/docs/instances/renew-commitments-automatically
- Usage Deduplication is Challenging (OpenMeter; idempotency keys, 32-day dedup window, at-least-once/exactly-once): https://openmeter.io/blog/usage-deduplication
- How we built it: Usage-based billing (Stripe; Flink exactly-once, fast/slow paths): https://stripe.com/blog/how-we-built-it-usage-based-billing
- Event Ingestion Architecture: Building Reliable Metering Pipelines (Lago; event_id/idempotency_key dedup, at-least-once vs exactly-once): https://getlago.com/blog/event-ingestion-architecture
- Metering Object Storage and S3 Billing (meters: sum uploaded bytes, GB-days stored, egress bytes, op count): https://dodopayments.com/blogs/metering-object-storage-s3-billing
- Plan and manage costs for Azure Blob Storage (meters: data storage GB/month, operations per transaction, data transfer per GB): https://learn.microsoft.com/en-us/azure/storage/common/storage-plan-manage-costs
- Charging for cloud storage (exertion-based metrics, disk time vs byte count): https://usenix.org/legacy/event/hotcloud11/tech/final_files/Wachs.pdf
- Energy Metrics For Data Networks (PCDR = W/Gbps, PUE, NEEE, EPC): https://ietf.org/archive/id/draft-bogdanovic-green-energy-metrics-00.html
- Collecting Infrastructure-Level Measures for Energy Analysis (MB/s per W = MB/J storage efficacy): https://arxiv.org/html/2611.22160
- A Unified Metric Architecture for AI Infrastructure (joules/op, kWh/kWh cross-layer taxonomy): https://arxiv.org/html/2511.21772
- Overview of Amazon QLDB (immutable, append-only journal, hash chaining): https://docs.aws.amazon.com/qldb/latest/developerguide/what-is.overview.html
- Metered Entitlements (Kong; usage periods, allowances, preserve overage, soft/hard limits): https://developer.konghq.com/metering-and-billing/entitlements
- Sliding-Window Quotas in Apache Hadoop YARN (exact admission, bounded error at window edges): https://arxiv.org/pdf/2606.06145v3
- Resource quota management for distributed workers (CPU/memory/GPU per worker): https://docs.rs/oximedia-distributed/latest/src/oximedia_distributed/resource_quota.rs.html
- multistore_metering — usage metering and quota enforcement middleware (pre-dispatch check, post-dispatch record): https://docs.rs/multistore-metering
- reserve/commit/release lifecycle with atomic enforcement (Cbox metering & enforcement): https://cbox.dk/packages/laravel-billing/docs/v0/core-concepts/metering
- The Two-Ledger Model (settlement ledger joined to attribution ledger; reconciliation both directions): https://arxiv.org/html/2611.06989v1
- Token bucket / rate limiting design (AWS fairness in multi-tenant systems, admission control): https://builder.aws.com/content/3Eupj3d2bo4fEvlzYbICMZNhQ3B/fairness-in-multi-tenant-systems
- Quota policy types (calendar, rollingwindow, flexi — Apigee/Google Cloud): https://docs.cloud.google.com/apigee/docs/api-platform/reference/policies/quota-policy

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 036.
