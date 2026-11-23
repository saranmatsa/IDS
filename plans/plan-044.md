## Phase 044 — Create the ids-observability crate: Metrics, tracing, audit trail.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 655-669)

```
44. Research only do not read plan.md **[Phase 044 — Create the ids-observability crate: System state exposure and observability.]**

Crate: ids-observability
Purpose: System state exposure and observability.
Contents: Metrics collection, distributed tracing, audit trail, telemetry, observability dashboard, control-plane vs data-plane separation observability, SLA/SLO tracking.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 044 — Create the ids-observability crate: System state exposure and observability.]**

Create plan-044.md in C:/idfs/plans/ for the ids-observability crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 044 creates **`ids-observability`**, the IDS crate that exposes system state for observability: metrics collection, distributed tracing, audit trail, telemetry export, control-plane vs data-plane separation observability, and SLA/SLO tracking. `ids-observability` is a **separate crate** (per the separate-crates directive, not more folders inside existing ones) that sits above `ids-000-aa` (canonical types + sealed trait contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It never owns a storage engine; it ingests telemetry produced by other crates, aggregates it locally, signs/audit-chains audit records, and emits opaque `Vec<u8>` telemetry envelopes to `ids-carrier::Endpoint` for off-node delivery.

Three pillars are dimensioned independently but share one ingest bus:

- **Metrics** — counters, gauges, histograms (HDR-style), and summaries; registry with bounded cardinality; lazy registration; cardinality limits with overflow rollup; downsampling/rollup retention tiers.
- **Distributed tracing** — W3C trace-context propagation (trace-id 128-bit, span-id 64-bit, sampled flag), span creation/events/links, head-based + tail-based sampling, batch export over opaque byte envelopes.
- **Audit trail** — append-only, hash-chained log with optional Ed25519 signing per entry and periodic signed checkpoints; inclusion/consistency proofs (RFC 6962/CT-style); WORM append semantics verified by immutable-store policy; retention per record class.

Control-plane vs data-plane separation is enforced by separate ingest channels: data-plane telemetry (per-operation metrics/traces/audit) is bounded and low-overhead; control-plane telemetry (config changes, topology, compaction, retention) is richer but lower frequency. SLA/SLO tracking consumes SLIs from metrics to compute error budgets and burn rates.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed from `ids-crypto`/`ids-000-aa`).

Research basis: 30 parallel searches returned the Rust `metrics` registry (counters/gauges/histograms, Receiver pattern)[^1]; `tracing` spans/events/`#[instrument]` + `tracing-subscriber` fmt JSON layer + EnvFilter[^2][^3]; OpenTelemetry metrics API (Meter/MeterProvider, instrument kinds Counter/Gauge/Histogram/Summary, synchronous+asynchronous, cardinality limits default 2000)[^4][^5]; OpenTelemetry OTLP exporter over gRPC/HTTP (binary/JSON), batch span processor defaults (queue 2048, batch 512)[^6][^7]; W3C Trace Context `traceparent` format `version-traceid-spanid-traceflags`, trace-id random ≥7 rightmost bytes, span-id random[^8][^9][^10]; HDR histogram (constant-time record, fixed memory, percentile queries)[^11]; Prometheus/OpenMetrics text exposition, `prometheus-client` Registry+Collector[^12]; append-only hash-chained audit log + Ed25519-signed checkpoints + inclusion/consistency proofs (CT/RFC 6962 pattern, Merkle tree O(log n) proofs)[^13][^14][^15]; SLO/error-budget/burn-rate multi-window alerting (long window / short window 1/12th; burn-rate constants 14.4/6/1 for 99.9%)[^16][^17]; control-plane/data-plane separation[^18]; telemetry cardinality limits and dynamic-identifier avoidance[^4]; downsampling/rollup retention[^19]. This document is the research blueprint only — no code written, no files modified.

---

### 2. Architecture Overview

`ids-observability` is an **ingest-and-export layer**: other crates push telemetry (metrics updates, trace spans/events, audit records) into its ingest bus; the crate aggregates, samples, downsamples, signs/audit-chains, and emits opaque `Vec<u8>` envelopes to `ids-carrier::Endpoint`. It is not a dashboard server — dashboards consume exported envelopes from any backend.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         APPLICATION / ids-* LAYER                            │
│  ids-store, ids-fragment, ids-reclaim, ids-client emit metrics/spans/audit  │
├─────────────────────────────────────────────────────────────────────────────┤
│                 Phase 044: ids-observability (THIS CRATE)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐      │
│  │ metrics      │ │ tracing      │ │ audit trail  │ │ SLA/SLO        │      │
│  │ registry     │ │ ingest       │ │ ingest       │ │ tracking       │      │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬────────┘      │
│         │                │                │                │                │
│  ┌──────▼────────────────▼────────────────▼────────────────▼───────┐       │
│  │            ingest bus → sampler → aggregator → exporter         │       │
│  │  downsampler/rollup (retention tiers)   hash-chain + sign       │       │
│  └──────────────────────────────┬──────────────────────────────────┘       │
│           opaque Vec<u8> telemetry envelopes                                │
└───────────────────────────────────────────────┬─────────────────────────────┘
                                                │ opaque Vec<u8> datagrams
┌───────────────────────────────────────────────▼─────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])   │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data flow:**
1. **Ingest:** crates call `metrics::increment_counter!` / `tracing::span!` / audit `append_record!`; calls land on per-pillar ingest channels.
2. **Aggregate/sample:** metrics are aggregated per flush interval; spans are sampled (head-based by default, tail-based optional); audit records are appended immediately (synchronous, WORM).
3. **Downsample/rollup:** metrics roll up to retention tiers (full-resolution hot, downsampled warm, aggregated cold).
4. **Audit-chain:** each audit record is hash-chained to its predecessor; optionally Ed25519-signed; periodic signed checkpoints (Merkle root over tree).
5. **Export:** telemetry envelopes (metrics payload, trace batch, audit checkpoint/proof) are serialized and handed to the carrier adapter as opaque `Vec<u8>` to `ids-carrier::Endpoint`. `ids-observability` never touches a carrier directly.

**Control-plane vs data-plane separation:** data-plane telemetry is tagged `plane=data` and flows through a bounded, drop-on-pressure channel (observability must never block the data path); control-plane telemetry is tagged `plane=control`, flows through an unbounded channel with richer attributes, and is never mixed into data-plane percentiles.

---

### 3. Detailed Research Dimensions

#### 3.1 Metrics collection (registry, instrument kinds, cardinality)

Metrics follow the OpenTelemetry instrument model: **Counter** (monotonic u64), **Gauge** (i64, up/down), **Histogram** (value distribution, HDR-style buckets, percentile queries), **Summary** (quantile approximations). A `Registry` holds all instruments by name+attributes; each unique attribute combination is a separate series — cardinality is bounded (default 2000 per metric stream) with overflow rollup to a single overflow data point marked `otel.metric.overflow=true`. Lazy registration: instruments are created on first use. Downsampling rolls full-resolution hot buckets into wider warm/cold aggregates after configurable retention.

Lazy registration + cardinality limits prevent metric-name explosion (dynamic identifiers such as request IDs must never be attributes — use route templates instead). Aggregation is synchronous per-record (constant-time for counter/gauge, fixed-cost for HDR histogram) to keep data-plane overhead bounded.

#### 3.2 Distributed tracing (W3C trace-context, spans, sampling)

Spans are `tracing`-style: a span is a period with fields/attributes and a parent; events are momentary occurrences within a span. Span context (`SpanContext`) carries trace-id (128-bit), span-id (64-bit), and trace-flags (sampled bit) and is propagated across process boundaries via W3C `traceparent`: `00-{32 hex trace-id}-{16 hex span-id}-{2 hex flags}`. Trace-id must be globally unique: generate with ≥7 random rightmost bytes and set the random-trace-id flag; span-id random. Spans may link to causally-related spans via their SpanContext. Sampling: head-based (decide at span start, cheap but may miss interesting traces) and tail-based (decide at span end, needs a buffer; captures error traces). Export batches finished spans as opaque byte envelopes over the carrier; batch span processor with bounded queue (default 2048) and max export batch (default 512).

#### 3.3 Audit trail (append-only, hash-chained, tamper-evident)

Audit records are append-only: insert-only, no update/delete. Each entry carries a monotonic sequence, timestamp, actor (principal id from `ids-000-aa` types), action, object, outcome, and a SHA-256/BLAKE3 hash chain linking `entry[i].hash = H(prev_hash || entry[i])`. Optional Ed25519 signature per entry or per checkpoint. Periodic signed checkpoints: Merkle root over the tree + tree size + timestamp, signed with Ed25519 (Signed Tree Head pattern, CT/RFC 6962). Inclusion proofs (O(log n)) prove an entry is in the log; consistency proofs prove the log at size N₁ is a prefix of the log at N₂ (append-only growth, no redaction/reorder). Offline proof bundles let a verifier check integrity without network. Retention is per record class (compliance-driven); legal hold suspends expiry. Sensitive data (PII, keys, credentials) must never be written to the audit log.

#### 3.4 Telemetry export & carrier-agnostic delivery

All telemetry leaves the node as opaque `Vec<u8>` envelopes to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Envelope = {pillar, seq, timestamp, payload, optional signature}. The carrier adapter serializes and hands bytes to `ids-carrier`; `ids-observability` never imports `std::net`, `tokio::net`, libp2p, or any transport. Export is asynchronous with a bounded batch queue and backpressure: if the queue is full, data-plane telemetry is dropped (observability never blocks the data plane), control-plane telemetry applies backpressure.

#### 3.5 Control-plane vs data-plane separation observability

Data-plane telemetry (per-operation metrics/trace/audit) is low-overhead, bounded, sampled aggressively, and drop-on-pressure. Control-plane telemetry (topology changes, config edits, compaction, retention, SLO evaluations) is richer, lower-frequency, and retained longer. Separate ingest channels prevent control-plane burst from skewing data-plane percentiles and from starving the export queue.

#### 3.6 SLA/SLO tracking

SLIs are derived from metrics: availability (success/total requests), latency (p95/p99 histogram quantiles), error rate. An SLO = SLI target over a compliance window (e.g., 99.9% availability over 30 days). Error budget = 1 − target. Burn rate = error rate / error budget per unit time. Multi-window burn-rate alerting: long window (e.g., 1h) + short window (1/12th, 5m); high-burn thresholds 14.4/6 and low-burn 1 with corresponding budget consumption 2%/5%/10%. Alerts fire only when both windows exceed threshold (reduces false positives; resets when short window drops below). Burn-rate constants derive from SLO target and period.

#### 3.7 Observability dashboard model

`ids-observability` produces telemetry; dashboards are external consumers of exported envelopes. The crate defines the data model (time-series points, span traces, audit entries, SLO snapshots) and a pull/push export surface, but renders no UI. Downsampled retention tiers keep storage bounded: hot (full resolution, short TTL), warm (downsampled, medium TTL), cold (aggregates, long TTL).

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Metric cardinality explosion (dynamic identifiers as attributes) | Hard cardinality limit per metric (default 2000) with overflow rollup; reject dynamic identifiers in labels at registration |
| Observability overhead blocking the data plane | Separate data-plane channel with drop-on-pressure; bounded queue; control-plane backpressure only |
| Audit log tampering (edit/delete/reorder/inject) | Append-only WORM store; SHA-256/BLAKE3 hash chain; optional Ed25519 per-entry; periodic signed checkpoints; inclusion/consistency proofs |
| Audit log containing sensitive data (PII/keys) | Schema validation rejects sensitive fields; documented rule; PII never in audit records |
| Replay of telemetry envelopes | Monotonic sequence per pillar + nonce; replay cache with TTL-bounded eviction |
| Delayed/export-failure telemetry loss | Bounded export queue with backpressure; drop data-plane on overflow (never block); control-plane retries |
| Unverified checkpoint/audit proof | Clients verify signatures and proofs client-side; full audit recalculates all hashes vs historical roots |
| Sampling hiding incidents | Tail-based sampling option captures error traces; head rate configurable; never sample audit records |
| Carrier misdelivery/envelope tampering | Optional Ed25519 signature over envelope payload; verification at ingest boundary of receiver |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 044 |
|-------|-------------------|
| `ids-000-aa` | Canonical types (NodeID, Principal, ObjectId, Tick), sealed trait contracts, invariants, error base; actor/principal for audit records |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); all telemetry traffic as opaque `Vec<u8>` datagrams |
| `ids-crypto` | Allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 |
| `ids-store` / `ids-fragment` / `ids-reclaim` | Producers of data-plane metrics, trace spans, audit records |
| `ids-client` | Consumer-facing; emits client-side telemetry via the shared ingest bus |

**Integration points:**
- `ids-000-aa` supplies principal/object identifiers used in audit records and trace attributes; sealed contracts guarantee stable types across crates.
- `ids-carrier::Endpoint` is the sole egress: `send_datagram(Endpoint, &[u8])`. No direct socket/transport imports.
- `ids-crypto` provides SHA-256/BLAKE3 for hash chains and Ed25519 for signing; xchacha20poly1305/chacha20poly1305 available if envelope encryption is ever required.
- Other IDS crates call `ids-observability` ingest APIs (metric counters, trace spans, audit appends); no crate depends on `ids-observability` for correctness — observability is additive instrumentation.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-observability` (new crate; depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`; plus `serde`, `thiserror`, `bytes`, `blake3`, `hdrhistogram`, `tracing`, `tracing-subscriber`, `metrics`, `metrics-exporter-prometheus`, `opentelemetry`, `zeroize`)

```
ids-observability/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol defaults (cardinality limit, retention, queue sizes)
├── error.rs                        # ObservabilityError (thiserror, comprehensive)
├── features.rs                     # Feature flags (metrics, tracing, audit, export, fuzzing)
│
├── ingest/                         # Unified ingest bus for all pillars
│   ├── mod.rs
│   ├── bus.rs                      # IngestBus: channels per pillar + plane
│   ├── channel.rs                  # Bounded + unbounded ingest channels, backpressure
│   ├── plane.rs                    # plane=control / plane=data tagging + separation
│   ├── backpressure.rs             # Drop-on-pressure (data-plane) vs backpressure (control-plane)
│   ├── ingest_types.rs             # IngestItem, Pillar, Plane, Envelope structures
│   ├── ingest_proptests.rs
│   └── ingest_fuzz.rs
│
├── metrics/                        # Metrics collection pillar
│   ├── mod.rs
│   ├── registry.rs                 # Metrics registry, instrument lifecycle, lazy registration
│   ├── counter.rs                  # Counter (monotonic u64)
│   ├── gauge.rs                    # Gauge (i64 up/down)
│   ├── histogram.rs                # Histogram (HDR-style buckets, percentiles)
│   ├── summary.rs                  # Summary (quantile approximations)
│   ├── instrument.rs               # Instrument trait + kinds (sync/async)
│   ├── cardinality.rs              # Cardinality limit + overflow rollup (default 2000)
│   ├── downsampler.rs              # Rollup hot→warm→cold retention tiers
│   ├── retention.rs                # Retention policy per tier (TTL-based expiry)
│   ├── snapshot.rs                 # Point-in-time metric snapshot for export
│   ├── metric_types.rs             # MetricId, AttributeSet, Value, Aggregation
│   ├── metric_proptests.rs
│   └── metric_fuzz.rs
│
├── tracing/                        # Distributed tracing pillar
│   ├── mod.rs
│   ├── span.rs                     # Span: fields, attributes, parent, SpanContext
│   ├── event.rs                    # Event within a span
│   ├── context.rs                  # SpanContext: trace-id (128-bit), span-id (64-bit), flags
│   ├── trace_id.rs                 # TraceId generation (≥7 random rightmost bytes)
│   ├── span_id.rs                  # SpanId (64-bit random)
│   ├── propagation.rs              # W3C traceparent encode/decode (00-traceid-spanid-flags)
│   ├── sampler.rs                  # Head-based + tail-based sampling decisions
│   ├── tail_sampler.rs             # Tail-based buffer + decision at span end
│   ├── batcher.rs                  # Finished-span batcher (queue 2048, batch 512)
│   ├── export_batch.rs             # Trace export batch serialization
│   ├── tracing_types.rs            # SpanId, TraceId, SpanContext, SamplingDecision
│   ├── tracing_proptests.rs
│   └── tracing_fuzz.rs
│
├── audit/                          # Audit trail pillar
│   ├── mod.rs
│   ├── log.rs                      # Append-only audit log (WORM, sequence + timestamp)
│   ├── entry.rs                    # AuditEntry: seq, actor, action, object, outcome, hash
│   ├── hash_chain.rs               # SHA-256/BLAKE3 hash chain: entry[i] = H(prev || entry[i])
│   ├── merkle.rs                   # Merkle tree over log entries; inclusion/consistency proofs (O(log n))
│   ├── checkpoint.rs               # Signed checkpoint: (tree_size, root_hash, timestamp, Ed25519 sig)
│   ├── signer.rs                   # Ed25519 signing of entries/checkpoints
│   ├── verifier.rs                 # Proof verification: inclusion, consistency, checkpoint signature
│   ├── proof_bundle.rs             # Offline proof bundle (entry + proof + signed checkpoint)
│   ├── retention.rs                # Per-class retention; legal hold suspends expiry
│   ├── worm_store.rs               # Append-only store interface (immutable-store policy)
│   ├── sensitive_guard.rs          # Reject PII/keys/credentials at audit ingest
│   ├── audit_types.rs              # AuditEntryId, Proof, Checkpoint, RetentionClass
│   ├── audit_proptests.rs
│   └── audit_fuzz.rs
│
├── telemetry/                      # Telemetry export pillar
│   ├── mod.rs
│   ├── envelope.rs                 # TelemetryEnvelope: {pillar, seq, timestamp, payload, signature}
│   ├── serializer.rs               # Envelope -> byte payload (deterministic encoding)
│   ├── deserializer.rs             # Byte payload -> Envelope
│   ├── exporter.rs                 # Export orchestrator: batch + send over carrier
│   ├── batch_queue.rs              # Bounded export queue, backpressure, drop-on-pressure
│   ├── carrier_adapter.rs          # Maps envelopes to ids-carrier::Endpoint datagrams
│   ├── export_types.rs             # ExportBatch, ExportResult, BackpressureDecision
│   ├── export_proptests.rs
│   └── export_fuzz.rs
│
├── slo/                            # SLA/SLO tracking pillar
│   ├── mod.rs
│   ├── sli.rs                      # Service Level Indicator from metrics (availability, latency, error rate)
│   ├── slo.rs                      # SLO: SLI + target + compliance window
│   ├── error_budget.rs             # Error budget = 1 − target; remaining budget tracking
│   ├── burn_rate.rs                # Burn rate computation; multi-window (long/short 1/12th)
│   ├── alert.rs                    # Burn-rate alert (fires when both windows exceed threshold)
│   ├── alert_rule.rs               # Alert rule definition (severity, window pair, threshold)
│   ├── slo_types.rs                # SLI, SLO, ErrorBudget, BurnRate, Alert
│   ├── slo_proptests.rs
│   └── slo_fuzz.rs
│
├── dashboard/                      # Dashboard data model (no UI rendered here)
│   ├── mod.rs
│   ├── timeseries.rs               # Time-series point model for metric export
│   ├── trace_view.rs               # Trace waterfall model
│   ├── audit_view.rs               # Audit entry + proof view model
│   ├── slo_view.rs                 # SLO snapshot view model
│   ├── export_format.rs            # Serialization format for external dashboards
│   └── dashboard_types.rs
│
├── carrier/                        # Carrier boundary (ids-carrier::Endpoint only)
│   ├── mod.rs
│   ├── adapter.rs                  # CarrierAdapter: send_datagram(Endpoint, &[u8])
│   ├── endpoint_binding.rs         # Endpoint construction from CarrierType + Vec<u8>
│   └── carrier_types.rs
│
├── config/                         # Configuration
│   ├── mod.rs
│   ├── metrics_config.rs           # Cardinality limit, retention tiers, downsampling
│   ├── tracing_config.rs           # Sampling rate, tail buffer size, batch size
│   ├── audit_config.rs             # Retention classes, signing interval, WORM policy
│   ├── export_config.rs            # Queue sizes, backpressure thresholds, batch sizes
│   ├── slo_config.rs               # SLO targets, burn-rate thresholds, alert severities
│   └── observability_config.rs     # Top-level config aggregation
│
├── fuzz/                           # Fuzzing harnesses (one per ingestion boundary)
│   ├── mod.rs
│   ├── fuzz_ingest.rs
│   ├── fuzz_metrics.rs
│   ├── fuzz_tracing.rs
│   ├── fuzz_audit.rs
│   ├── fuzz_export.rs
│   └── fuzz_slo.rs
│
├── tests/                          # Integration & adversarial tests
│   ├── mod.rs
│   ├── metrics_tests.rs            # Counter/gauge/histogram/summary semantics
│   ├── cardinality_tests.rs        # Cardinality limit + overflow rollup
│   ├── tracing_tests.rs            # Span/event lifecycle, W3C propagation round-trip
│   ├── sampling_tests.rs           # Head + tail sampling behavior
│   ├── audit_tests.rs              # Append-only WORM, hash chain integrity
│   ├── proof_tests.rs              # Inclusion/consistency proof verification
│   ├── checkpoint_tests.rs         # Signed checkpoint verification
│   ├── export_tests.rs             # Envelope serialization, carrier boundary
│   ├── backpressure_tests.rs       # Drop-on-pressure (data-plane) vs backpressure (control-plane)
│   ├── slo_tests.rs                # Error budget, burn rate, multi-window alerting
│   ├── integration_tests.rs        # Cross-pillar ingest → export over carrier
│   ├── adversarial_tests.rs        # Cardinality bomb, audit tamper detection, replay
│   ├── determinism_tests.rs        # Same seed → identical export
│   ├── property_tests.rs           # Proptest properties across pillars
│   └── replay_tests.rs             # Telemetry replay attack tests
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── metrics_bench.rs            # Counter/gauge/histogram record throughput
│   ├── tracing_bench.rs            # Span creation/event emission overhead
│   ├── audit_bench.rs              # Hash chain append + proof generation
│   ├── export_bench.rs             # Envelope serialization + send throughput
│   ├── slo_bench.rs                # Burn-rate computation benchmarks
│   └── cardinality_bench.rs        # Cardinality limit enforcement overhead
│
├── spec/                           # Normative specifications
│   ├── metrics_spec.md             # Metrics model & cardinality limits
│   ├── tracing_spec.md             # Span model + W3C trace-context propagation
│   ├── audit_spec.md               # Append-only hash chain + Merkle proofs
│   ├── telemetry_spec.md           # Envelope format & carrier export
│   ├── slo_spec.md                 # SLO/error budget/burn-rate model
│   └── threat_model.md             # Threat model document
│
├── docs/                           # Documentation
│   ├── architecture.md             # Architecture & layering
│   ├── integration_guide.md        # Integrating ids-observability with other IDS crates
│   ├── observability_guide.md      # When to use metrics vs tracing vs audit
│   ├── security_considerations.md  # Security considerations (PII, WORM, signing)
│   ├── design_rationale.md         # Design choices (separate crate, carrier-agnostic)
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── metric_vectors.json
    ├── trace_context_vectors.json
    ├── audit_chain_vectors.json
    ├── checkpoint_vectors.json
    └── envelope_vectors.json
```

**Why ~90–1000 files:** one file per responsibility — ingest bus, three pillars (metrics/tracing/audit) each with registry/model/chain/proof/signing/retention, telemetry export, SLO tracking, dashboard data model, carrier boundary, config, fuzz, tests, benches, specs, docs, test vectors. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Metrics registry & instrument kinds

```
InstrumentKind = Counter | Gauge | Histogram | Summary
MetricId = {name, attribute_set}
Registry: Map<MetricId, Instrument>
record(name, attributes, value): lazy-create instrument; aggregate constant-time
cardinality: if series_count(name) > 2000 → overflow rollup (otel.metric.overflow=true)
downsample: hot(full, TTL_h) → warm(downsampled, TTL_w) → cold(aggregated, TTL_c)
```

#### 7.2 W3C trace-context propagation

```
traceparent = "00" "-" {32 hex trace-id} "-" {16 hex span-id} "-" {2 hex trace-flags}
trace-id: 128-bit; ≥7 rightmost bytes random; random-trace-id flag if so
span-id: 64-bit random
SpanContext = {trace_id, span_id, trace_flags(sampled)}
Span = {context, parent: Option<SpanContext>, fields, links: Vec<SpanContext>, start, end}
propagate(span_context) -> traceparent string for carrier envelope headers
```

#### 7.3 Audit hash chain + Merkle proofs

```
entry[i].hash = SHA-256(prev_hash || entry[i].canonical_bytes)   // or BLAKE3
Merkle leaf = SHA-256(0x00 || entry); node = SHA-256(0x01 || left || right)  // domain separation
Inclusion proof: sibling hashes from leaf to root (O(log n))
Consistency proof: prove root(N₁) is prefix of root(N₂) (append-only)
Checkpoint = {tree_size, root_hash, timestamp, ed25519_signature}
```

#### 7.4 Telemetry envelope

```
TelemetryEnvelope = {pillar, seq: u64, timestamp: u64, payload: Vec<u8>, signature: Option<[u8;64]>}
carrier_datagram = serialize(envelope) → Vec<u8> → ids-carrier::Endpoint
```

#### 7.5 SLO / error budget / burn rate

```
SLI = good_events / total_events  (availability, latency-threshold, error-rate)
SLO = {sli, target, compliance_window}
error_budget = 1 − target
burn_rate = error_rate / error_budget_per_unit_time
multi-window alert: long_window (e.g. 1h) AND short_window (long/12, e.g. 5m) both > threshold
burn-rate constants for 99.9%/30d: 14.4 (1h/5m, 2% budget), 6 (6h/30m, 5%), 1 (3d/6h, 10%)
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | Canonical types (NodeID, Principal, ObjectId, Tick), sealed contracts, error base |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); sole egress boundary |
| `ids-crypto` | ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (allowed set only) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `bytes` | Buffer handling |
| `blake3` | Hashing (workspace dependency) |
| `hdrhistogram` | Histogram buckets, percentile queries |
| `metrics` | Metrics primitives (counter/gauge/histogram) |
| `tracing` / `tracing-subscriber` | Span/event model, JSON layer, EnvFilter |
| `opentelemetry` | Instrument model, semantic conventions reference |
| `zeroize` | Key material zeroing |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

No networking crates (libp2p/TCP/IP/QUIC/HTTP/DNS). No `tokio::net` / `std::net`. All traffic via `ids-carrier::Endpoint` opaque `Vec<u8>` datagrams. No crypto outside the allowed set.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-044.1 | Metrics semantics | Counter monotonic; gauge up/down; histogram percentiles via HDR; unit + property test, 100k iterations |
| AC-044.2 | Cardinality bound | Per-metric series capped (default 2000); overflow rollup marked; cardinality-bomb test passes |
| AC-044.3 | W3C trace-context round-trip | SpanContext → traceparent → SpanContext identical; trace-id ≥7 random rightmost bytes; span-id random |
| AC-044.4 | Span lifecycle | Create/enter/exit span; events within span; parent-child links; export batch correctness |
| AC-044.5 | Sampling | Head-based rate enforced; tail-based captures error traces; sampling never applies to audit records |
| AC-044.6 | Audit append-only WORM | Insert-only; update/delete rejected; 100% of single-record insert/delete/reorder detected; offender named |
| AC-044.7 | Hash chain integrity | 100% of tamper detected; verification names offending entry; offline proof bundle verifies without network |
| AC-044.8 | Merkle proofs | Inclusion proof O(log n); consistency proof proves append-only growth between two sizes |
| AC-044.9 | Signed checkpoint | Ed25519 signature over (tree_size, root_hash, timestamp); verifiable client-side |
| AC-044.10 | Sensitive-data rejection | PII/keys/credentials rejected at audit ingest; property test |
| AC-044.11 | Envelope carrier boundary | serialize → Vec<u8> → ids-carrier::Endpoint; zero references to std::net/tokio::net/TCP/UDP/QUIC/HTTP/DNS |
| AC-044.12 | Data-plane vs control-plane separation | Data-plane drop-on-pressure never blocks writes; control-plane backpressure only; percentiles not skewed |
| AC-044.13 | Export queue bounds | Bounded batch queue; overflow drops data-plane, backpressures control-plane; measured by bench |
| AC-044.14 | SLO/error budget | error_budget = 1 − target; burn-rate multi-window alert fires only when both windows exceed threshold; reset on recovery |
| AC-044.15 | Allowed crypto only | ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 appear; no other primitive in crypto surface |
| AC-044.16 | Performance bound | Metric record ≤ ~100ns p50 (counter/gauge); span emit bounded; hash-chain append constant; bench-verified |
| AC-044.17 | Determinism | Same seed + input → identical export envelopes and proofs |
| AC-044.18 | Adversarial tests pass | Cardinality bomb, audit tamper, replay, sensitive-data injection all fail-safe |

---

### 10. Remaining Uncertainties

1. **Registry choice:** `metrics` crate + custom registry vs OpenTelemetry Rust SDK meter — OTel SDK pulls OTLP exporter (gRPC/HTTP) which conflicts with carrier-agnostic byte-envelope export; likely adopt `metrics` primitives with `ids-observability`-owned export. Decision needed before implementation.
2. **Histogram implementation:** `hdrhistogram` (constant-time, fixed memory) vs custom exponential buckets — `hdrhistogram` favored for latency percentiles; crate availability/licensing to confirm.
3. **Audit store backend:** WORM append-only interface must be backed by an immutable store (sled/rocksdb with insert-only policy, or object-storage WORM) — backend selection and `ids-storage` integration TBD.
4. **Envelope encoding:** deterministic CBOR vs bincode vs custom binary — must be versioned and checksummed; decision affects wire compatibility across versions.
5. **Export retry/backoff:** control-plane retry policy (interval, cap, jitter) undefined; data-plane drop-on-pressure is final (no retry).
6. **Tail-sampling buffer size:** bounded queue for tail-based sampling; overflow policy (drop oldest vs drop newest) unquantified.
7. **SLO defaults:** target/compliance window/burn-rate thresholds are deployment-specific; defaults are placeholders pending measurement.
8. **Checkpoint frequency vs replay cost:** how often signed checkpoints are written trades storage against audit replay length; unmeasured.
9. **Cross-node trace correlation:** trace context must survive carrier hops — envelope must carry traceparent; hop-by-hop correlation vs end-to-end trace needs confirmation against carrier semantics.
10. **Retention class defaults:** per-class retention (audit) periods are compliance-driven; defaults TBD.

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-observability` (not a subfolder) | User directive: separate crates instead of more folders; keeps observability concerns isolated |
| Three pillars on one ingest bus | Metrics/tracing/audit share ingest + export; plane tagging separates data/control |
| `metrics`-style primitives + custom registry | Avoids OTel SDK OTLP (gRPC/HTTP) dependency conflict with carrier-agnostic byte envelopes |
| W3C trace-context `traceparent` | Standard, fixed-length, fast parsing; portable across vendors |
| HDR-style histograms | Constant-time record, fixed memory, percentile queries; latency SLOs |
| Cardinality limit 2000 + overflow rollup | Prevents metric-name explosion; OTel-proven pattern |
| Append-only WORM audit + hash chain + Merkle proofs | Tamper-evident, offline-verifiable; CT/RFC 6962 pattern |
| Ed25519 signed checkpoints | Signed tree head; client-verifiable; allowed crypto only |
| Data-plane drop-on-pressure; control-plane backpressure | Observability must never block the data path |
| Opaque `Vec<u8>` envelopes to `ids-carrier::Endpoint` | Carrier-agnostic; no std::net/tokio::net/libp2p/TCP/UDP/QUIC/HTTP/DNS |
| Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Complies with IDS crypto allowlist |
| Multi-window burn-rate alerting (long + short 1/12th) | SRE Workbook pattern; reduces false positives; clear reset |
| No dashboard rendering in crate | Dashboards consume exported envelopes; crate defines data model only |

---

### 12. Sources

[^1]: `metrics` Rust crate — registry/Receiver pattern, counters/gauges/histograms, 65–70ns ingest latency. https://docs.rs/metrics/0.10.3/metrics
[^2]: `tracing` — spans, events, `#[instrument]`, structured event-based diagnostics. https://tokio-rs.github.io/tracing/tracing
[^3]: `tracing-subscriber` — fmt JSON layer, EnvFilter directive syntax, layer-based filtering. https://docs.rs/tracing-subscriber/0.3.17/tracing_subscriber/fmt
[^4]: OpenTelemetry Metrics API — Meter/MeterProvider, instrument kinds (Counter/Gauge/Histogram/Summary), sync+async, cardinality limits default 2000 with overflow rollup. https://opentelemetry.io/docs/specs/otel/metrics/api
[^5]: OpenTelemetry metric cardinality limits — default 2000 combinations per stream, overflow attribute. https://opentelemetry.io/blog/2026/cardinality-limits-in-opentelemetry
[^6]: `opentelemetry-otlp` Rust — OTLP exporter over gRPC/HTTP (binary/JSON); batch span processor defaults queue 2048, batch 512. https://docs.rs/opentelemetry-otlp/0.10.0/opentelemetry_otlp
[^7]: OpenTelemetry SDK span processor — BatchSpanProcessor queue/batch constants. https://docs.rs/opentelemetry_sdk/0.33.1/opentelemetry_sdk/trace/span_processor
[^8]: W3C Trace Context — `traceparent` version-traceid-spanid-traceflags; trace-id 128-bit, span-id 64-bit. https://www.w3.org/TR/trace-context
[^9]: W3C Trace Context Level 2 — trace-id random ≥7 rightmost bytes, random-trace-id flag; span-id random. https://w3.org/TR/trace-context-2
[^10]: Coralogix W3C Trace Context guide — traceparent format, invalid values, trace-flags sampled bit. https://coralogix.com/guides/w3c-trace-context
[^11]: HdrHistogram — constant-time record, fixed memory footprint, percentile queries, no allocation on record. https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html
[^12]: `prometheus-client` — Registry, Collector, OpenMetrics text exposition format. https://docs.rs/prometheus-client/0.25.2/prometheus_client/encoding/text
[^13]: Certificate Transparency / RFC 6962 — Merkle tree, Signed Tree Head, inclusion & consistency proofs, domain-separated hashing. https://transparency.dev/verifiable-data-structures
[^14]: Settled / Merklon — append-only tamper-evident log, Ed25519-signed checkpoints, inclusion/consistency proofs, offline proof bundles. https://github.com/richardadalton/settled
[^15]: PALA-1 / IETF draft — Portable Append-only Log for Audit, hash chain, one-hash-per-record verification. https://ietf.org/archive/id/draft-sparysh-pala-audit-01.txt
[^16]: Google SRE Workbook — alerting on SLOs, multi-window multi-burn-rate, constants 14.4/6/1 for 99.9%. https://sre.google/workbook/alerting-on-slos
[^17]: Grafana multi-window multi-burn-rate alerts — long window + short window 1/12th, burn-rate thresholds. https://grafana.com/blog/how-to-implement-multi-window-multi-burn-rate-alerts-with-grafana-cloud
[^18]: HashiCorp Well-Architected — control plane vs data plane separation, least privilege. https://developer.hashicorp.com/well-architected-framework/design-resilient-systems/design-control-data-management-plane
[^19]: InfluxDB downsampling — rollup older points into wider intervals, retention tiers. https://influxdata.com/blog/downsampling-plugin-influxdb-3

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 044.
