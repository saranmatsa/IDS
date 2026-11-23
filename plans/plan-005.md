## Phase 005 — Create the ids-time crate: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 005 creates **`ids-time`**, the IDS standard time crate: a proprietary time basis for ordering and commit decisions that is explicitly **not wall-clock time and not a hybrid logical clock (HLC)**. The crate provides the `Idstime` type, a tick source, drift tolerance bounds, a synchronization mechanism, a cross-region reference frame, clock drift absorption, ordering timestamps, commit timestamps, and durability windows. All time in IDS is derived from `Idstime`; wall-clock (system `SystemTime`, `Instant`) is permitted only as a coarse sanity input to the tick source, never as an ordering authority. The crate is carrier-agnostic — it carries no networking, speaks no IP/TCP/UDP/QUIC/HTTP/DNS, and any time sync messages ride over `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`). Allowed crypto is limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, used only for authenticated time announcements and drift-claim verification; `ids-time` does not depend on `ids-crypto` directly (it consumes primitives re-exported through `ids-000-aa` where needed, per the dependency rule).

Design philosophy: IDS time is **logical, causal, and bounded**. A tick is a monotonically advancing unit with a per-node drift tolerance and an uncertainty interval (epsilon), inspired by TrueTime's interval model but decoupled from GPS/atomic hardware and from wall-clock. Ordering between events at different nodes is established by comparing `(l, c)` pairs where `l` is the propagated logical component and `c` is a bounded causality counter, with drift absorption applied when remote ticks arrive. Commit timestamps are `Idstime` values chosen to be definitely in the past relative to the local uncertainty bound before visibility, preventing causal-reverse anomalies without a global master or atomic clocks.

Scope:
- `Idstime` type — opaque, comparable, serializable time value (64-bit logical component + bounded logical counter + node origin).
- Tick source — pluggable clock source abstraction (monotonic counter primary, wall-clock only as coarse hint).
- Drift tolerance — configurable per-node epsilon (uncertainty interval) and max tolerated skew between regions.
- Synchronization mechanism — tick exchange over `ids-carrier::Endpoint`, signed with ed25519, drift absorption via max-update rule.
- Cross-region reference frame — region-scoped tick roots with bounded uncertainty, no single global time authority.
- Clock drift absorption — apply incoming remote ticks only if they advance the local state; reject outliers beyond tolerance.
- Ordering timestamps — assigned at operation receipt, used for write ordering.
- Commit timestamps — `Idstime` chosen at commit, guaranteed in-the-past via commit-wait-style bounded delay.
- Durability windows — time-bounded durability commitments tied to `Idstime` values, not wall-clock deadlines.

---

### 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-write                  │
│            ordering timestamps, commit timestamps           │
├─────────────────────────────────────────────────────────────┤
│                    Phase 005: ids-time (NEW CRATE)          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Idstime type (l, c, origin, epsilon)               │   │
│  │  tick source (monotonic primary, wall-clock hint)   │   │
│  │  drift tolerance & absorption                       │   │
│  │  synchronization over ids-carrier::Endpoint         │   │
│  │  cross-region reference frame                       │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  ordering timestamp allocator                       │   │
│  │  commit timestamp allocator (commit-wait)           │   │
│  │  durability window tracker                          │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  wire format: signed tick announcements (ed25519)   │   │
│  │  drift-claim verification (sha256/blake3)           │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                 ids-carrier (Endpoint, Datagram)            │
│           CarrierType + Vec<u8>, opaque datagrams           │
├─────────────────────────────────────────────────────────────┤
│             ids-000-aa (canonical types, contracts)         │
│         NodeId, RegionId, Endpoint, CarrierType, …          │
└─────────────────────────────────────────────────────────────┘
```

**Data flow — local tick advance:** A local event (operation receipt, commit) reads the tick source, advances the local `Idstime` (`l = max(l, now_hint)`, `c` incremented if `l` did not increase), stamps the event, and records the value. The `Idstime` is strictly monotonic per node and causally consistent across nodes via tick exchange.

**Data flow — remote tick sync:** A peer sends a signed tick announcement over `ids-carrier::Endpoint`. On receipt, `ids-time` verifies the ed25519 signature, checks the announced `(l, c)` against the local state: if `l_remote > l_local`, adopt `l = l_remote`, reset `c = 0`; if `l_remote == l_local`, `c = max(c_local, c_remote) + 1`; reject if outside drift tolerance (epsilon exceeded). Absorbed ticks update the cross-region reference frame.

**Ordering vs commit timestamps:** Ordering timestamps are assigned at operation receipt (cheap, local). Commit timestamps are assigned at commit time and must be provably in the past relative to the local uncertainty bound (commit-wait: delay visibility until `TT.after(commit_ts)` equivalent using local epsilon), ensuring no later-starting operation can observe an earlier commit timestamp — preventing causal-reverse anomalies without TrueTime hardware.

---

### 3. Detailed Research Dimensions

#### 3.1 Why Not Wall-Clock and Why Not HLC

The crate is explicitly **proprietary, not wall-clock and not HLC**. Wall-clock (`SystemTime`) is unsuitable as an ordering authority because NTP-synchronized clocks exhibit unbounded skew (10–800ms on WAN, per research), can step backward on sync corrections, and leap seconds violate monotonicity — a node with a lagging clock can assign a commit timestamp "in the past" relative to an already-committed operation on another node, causing causal-reverse anomalies (documented in CockroachDB's analysis of clock skew reordering). HLC (Kulkarni & Demirbas, 2014) combines physical and logical time but still relies on NTP-synchronized physical closeness (`|l - pt|` bounded by epsilon) and is therefore vulnerable to the same clock anomalies if synchronization degrades; ids-time is deliberately weaker in its physical-time assumptions and stronger in its logical-time guarantees: ordering is established by causal propagation of ticks, not by trusting that `l` approximates wall time.

#### 3.2 Idstime Type Design

`Idstime` is an opaque 128-bit value: a 64-bit logical component `l` (tick counter, advances spontaneously like a physical clock but is purely logical), a bounded logical counter `c` (resolves ordering among events sharing the same `l`), and a node origin identifier (`NodeId`). Comparison is lexicographic on `(l, c, origin)`, giving a total order consistent with happens-before for causally related events (e hb f ⇒ Idstime.e < Idstime.f). Serialization uses deterministic CBOR/bincode with SHA-256 or BLAKE3 content hashing for wire integrity. The type is `Copy`, `Ord`, `PartialOrd`, `Eq`, `Hash`, serde-serializable.

#### 3.3 Tick Source

The tick source is a pluggable trait abstracting the primitive that advances `l`. The default implementation is a **monotonic counter** (kernel `CLOCK_MONOTONIC`-equivalent, never steps backward) incremented on each tick request; wall-clock (`SystemTime`) is read only as a coarse sanity hint (`now_hint`) to bound how far `l` may lag behind observed wall time, never as the value of `l`. Alternative sources: hardware cycle counter (TSC), atomic-clock/GPSCLOCK where available, or a testable manual source. The tick source must never produce decreasing values; on source reset (restart), `l` is initialized from persisted high-water mark plus a safety margin.

#### 3.4 Drift Tolerance and Absorption

Each node tracks a per-peer drift bound: the maximum acceptable difference between its own `l` and any received remote `l`. When a remote tick arrives:
- If `l_remote <= l_local - epsilon` (stale, beyond tolerance): reject, record for audit.
- If `l_remote` within `[l_local - epsilon, l_local + epsilon]`: absorb via max-update rule (`l = max(l_local, l_remote)`, adjust `c`).
- If `l_remote > l_local + epsilon` (far future, suspicious): reject and flag the peer — a clock advertising far-future ticks is either faulty or malicious.

Epsilon is configurable per region pair (cross-region links have higher uncertainty than intra-region). Drift is **absorbed** (not corrected by slewing the local clock): the local `l` never decreases, and remote information only ever raises it. This avoids the NTP slew/step problem entirely.

#### 3.5 Synchronization Mechanism

Tick announcements are small datagrams (`CarrierType` + `Vec<u8>`) sent over `ids-carrier::Endpoint`. Each announcement carries `(origin_node_id, l, c, epsilon_claim, signature_ed25519)`. The signature covers the canonical encoding of the announcement so a compromised carrier cannot forge time. Announcements are sent gossip-style (periodic + on significant local advance) and on-demand during cross-region operations. No TCP/UDP/HTTP/DNS is used; the carrier abstraction is the sole transport. Receipt of a signed announcement advances the local reference frame; no round-trip or clock-offset computation is performed.

#### 3.6 Cross-Region Reference Frame

Regions are the primary failure/location domain (per `ids-000-aa` `RegionId`). Each region maintains a **region tick root** — the maximum `Idstime` observed from any node in that region — plus a per-region uncertainty interval (epsilon_region). The cross-region reference frame is the vector of region roots, compared componentwise. Operations spanning regions derive their ordering timestamp from the componentwise maximum of involved regions' roots, ensuring that a commit in region A is never ordered before an event in region B that region A has learned about. There is **no global master** (consistent with `ids-000-aa` invariant `no_global_master`); the frame is eventually consistent via gossip.

#### 3.7 Ordering Timestamps

Assigned at operation receipt (write path, `ids-write`). An ordering timestamp is a fresh `Idstime` from the local tick source, stamped on the operation record. Operations are totally ordered per ordering domain (`ids-ordering-domain`) by `(Idstime, OpRef)`. The ordering timestamp is not required to be in the past — it only needs causal consistency (e hb f ⇒ ts.e < ts.f), which the tick source guarantees.

#### 3.8 Commit Timestamps and Commit-Wait

A commit timestamp is an `Idstime` chosen at commit and must be **provably in the past** before the commit is made visible: the local node waits until its local uncertainty bound has definitely advanced beyond the commit timestamp (commit-wait, analogous to Spanner's `TT.after(s)` but using local epsilon instead of TrueTime). Specifically: `commit_ts.l + epsilon_local <= local_tick_now().l` before visibility. This ensures that no later-starting operation on any node can be assigned a timestamp less than `commit_ts`, preventing the lagging-clock causal-reverse anomaly. Commit-wait overlaps with durability/persistence work (WAL flush, replica acks) so the observable cost is typically hidden.

#### 3.9 Durability Windows

A durability window is a time-bounded durability commitment expressed in `Idstime`: "all writes with ordering timestamp <= T are durable on quorum by durability window ending at T + delta". Windows are tracked per dataset/region and used by `ids-durability-policy` and `ids-commit-state` to decide when an acknowledgement may be sent. Windows close when the local tick advances past their end; unacked writes past a closed window trigger recovery. Durability windows are purely logical (keyed to `Idstime`), so they remain correct across arbitrary wall-clock skew and region latency.

#### 3.10 Convergence and Safety Properties

- **Monotonicity:** per node, `Idstime` never decreases.
- **Causal consistency:** e hb f ⇒ Idstime.e < Idstime.f (via tick propagation on every message send/receive, Lamport-style).
- **Total order:** `(Idstime, NodeId)` is a total order; ties broken by `NodeId`.
- **Bounded uncertainty:** every `Idstime` carries an epsilon; commit visibility requires `ts + epsilon` definitely in the past.
- **No wall-clock dependence:** ordering/correctness holds even if all wall clocks are unsynchronized or step arbitrarily.

---

### 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Forged tick announcements (compromised carrier injects false time) | ed25519 signature over canonical announcement; unsigned announcements dropped |
| Replay of old tick announcements | `(origin, l, c)` is monotonically increasing; stale/duplicate announcements rejected |
| Far-future clock (peer advertises l beyond tolerance) | Reject if `l_remote > l_local + epsilon`; flag peer; epsilon is per-peer configurable |
| Clock steps backward locally | Monotonic tick source never decreases; restart initializes from persisted high-water mark + margin |
| Denial of service via tick flood | Per-peer announcement rate budget; bounded announcement queue; tick messages are small and authenticated |
| Cross-region partition stalls reference frame | Frame is eventually consistent; per-region roots advance independently; no operation blocks on remote regions |
| Commit timestamp in the future (causal reverse) | Commit-wait: visibility delayed until `ts + epsilon` definitely in the past |
| Uncertainty interval underestimation | Epsilon is conservatively configured; drift tolerance violations trigger peer flagging |

---

### 5. Integration with Existing Crates

| Crate | Role in Phase 005 |
|-------|-------------------|
| `ids-000-aa` | `NodeId`, `RegionId`, `Endpoint`, `CarrierType` types; sealed trait contracts; versioning; error contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`) is the sole transport for tick announcements |
| `ids-crypto` | Consumes ed25519 signing/verification, sha256/blake3 hashing (re-exported via ids-000-aa; no direct dependency per crate rule) |
| `ids-write` | Consumer: ordering timestamps stamped on operations at receipt |
| `ids-commit-state` | Consumer: commit timestamps assigned at commit; commit-wait integration |
| `ids-durability-policy` | Consumer: durability windows keyed to Idstime |
| `ids-ordering-domain` | Consumer: operations ordered by `(Idstime, OpRef)` per domain |
| `ids-identity` | Consumer: `NodeId` origin binding for tick announcements |
| `ids-topology` | Consumer: region boundaries for cross-region reference frame |

**Integration points:**
- `ids-time` exposes `Idstime`, `TickSource`, `DriftTolerance`, `ReferenceFrame`, `OrderingTimestamp`, `CommitTimestamp`, `DurabilityWindow` types and traits.
- `ids-write` calls `ids-time::ordering_timestamp()` on operation receipt.
- `ids-commit-state` calls `ids-time::commit_timestamp()` then `ids-time::commit_wait(ts)` before visibility.
- Tick announcements ride `ids-carrier::Endpoint` datagrams — no direct carrier dependency beyond the `Endpoint` type from `ids-000-aa`.
- No dependency on libp2p/TCP/IP/UDP/QUIC/HTTP/DNS — carrier-agnostic by construction (I-CARR-001).
- `ids-crypto` is the only crate that does NOT depend on `ids-000-aa`; `ids-time` depends on `ids-000-aa` and consumes crypto primitives through it.

---

### 6. Proposed File Structure (~150–300 files, all production-grade)

**Crate:** `ids-time` (new crate; depends on `ids-000-aa`, `ids-carrier` types; consumes `ids-crypto` primitives via re-exports; `serde`, `thiserror`, `blake3`, `sha2`, `bytes`, `proptest`/`bolero` for testing)

```
ids-time/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (default epsilon, tick interval, commit-wait margin)
├── error.rs                        # IdsTimeError (thiserror, comprehensive)
├── features.rs                     # Feature flags (sync, testing, fuzzing)
│
├── types/                          # Core Idstime type and related value types
│   ├── mod.rs
│   ├── idstime.rs                  # Idstime (l, c, origin, epsilon) — opaque, comparable, serializable
│   ├── tick.rs                     # Tick — a single tick event with origin and causality
│   ├── timestamp.rs                # OrderingTimestamp, CommitTimestamp wrapper types
│   ├── window.rs                   # DurabilityWindow — time-bounded durability commitment
│   ├── epsilon.rs                  # Epsilon — bounded uncertainty interval per node/peer/region
│   └── origin.rs                   # NodeId origin binding for tick announcements
│
├── tick_source/                    # Pluggable tick source abstraction
│   ├── mod.rs
│   ├── source.rs                   # TickSource trait (next_tick() -> Idstime)
│   ├── monotonic.rs                # Monotonic counter source (default, never decreases)
│   ├── wall_hint.rs                # Wall-clock as coarse sanity hint only (never authority)
│   ├── hardware.rs                 # Hardware cycle counter source (optional)
│   ├── manual.rs                   # Manual/testable source (for tests and simulation)
│   ├── persisted.rs                # Persisted high-water mark across restarts
│   └── composite.rs                # Composite source with fallback chain
│
├── drift/                          # Drift tolerance and absorption
│   ├── mod.rs
│   ├── tolerance.rs                # DriftTolerance — per-peer/per-region epsilon bounds
│   ├── absorption.rs               # Drift absorption: max-update rule, outlier rejection
│   ├── bound.rs                    # DriftBound — max acceptable l difference
│   ├── tracker.rs                  # Per-peer drift state tracking
│   └── violation.rs                # Drift violation recording and peer flagging
│
├── sync/                           # Synchronization over ids-carrier::Endpoint
│   ├── mod.rs
│   ├── announcement.rs             # TickAnnouncement — signed tick datagram
│   ├── signer.rs                   # ed25519 signing of announcements
│   ├── verifier.rs                 # ed25519 verification of announcements
│   ├── gossip.rs                   # Gossip scheduler (periodic + on-advance)
│   ├── receiver.rs                 # Incoming announcement handling and absorption
│   ├── frame.rs                    # Cross-region reference frame (vector of region roots)
│   └── transport.rs                # Carrier-agnostic send/receive over ids-carrier::Endpoint
│
├── ordering/                       # Ordering timestamps
│   ├── mod.rs
│   ├── allocator.rs                # OrderingTimestamp allocator (fresh Idstime per operation)
│   └── sequencer.rs                # Per-ordering-domain sequencer (Idstime + OpRef)
│
├── commit/                         # Commit timestamps and commit-wait
│   ├── mod.rs
│   ├── allocator.rs                # CommitTimestamp allocator
│   ├── commit_wait.rs              # Commit-wait: visibility delayed until ts + epsilon in past
│   └── visibility.rs               # Commit visibility gate
│
├── durability/                     # Durability windows
│   ├── mod.rs
│   ├── window.rs                   # DurabilityWindow tracker
│   ├── scheduler.rs                # Window close scheduling keyed to Idstime
│   └── ack_gate.rs                 # Acknowledgement gate keyed to window state
│
├── wire/                           # Serialization
│   ├── mod.rs
│   ├── format.rs                   # Versioned wire envelope (CBOR/bincode)
│   ├── cbor.rs                     # Deterministic CBOR encoding
│   └── checksum.rs                 # Payload checksums (sha256/blake3)
│
├── crypto/                         # Allowed-crypto boundary (consumes via ids-000-aa)
│   ├── mod.rs
│   ├── ed25519.rs                  # ed25519 sign/verify for announcements
│   ├── hash.rs                     # sha256/blake3 content hashing
│   └── nonce.rs                    # Nonce management for announcement freshness
│
├── tests/                          # Integration & property tests
│   ├── mod.rs
│   ├── idstime_tests.rs            # Idstime comparison, serialization, total order
│   ├── tick_source_tests.rs        # Monotonicity, wall-clock hint behavior
│   ├── drift_tests.rs              # Absorption, outlier rejection, tolerance bounds
│   ├── sync_tests.rs               # Announcement send/receive, gossip, frame convergence
│   ├── ordering_tests.rs           # Ordering timestamp allocation per domain
│   ├── commit_tests.rs             # Commit timestamp, commit-wait, visibility
│   ├── durability_tests.rs         # Durability window close, ack gate
│   ├── frame_tests.rs              # Cross-region reference frame, region roots
│   ├── wire_tests.rs               # Wire format round-trip
│   ├── adversarial_tests.rs        # Forged announcements, replay, far-future rejection
│   ├── chaos_tests.rs              # Partition, clock step, restart recovery
│   └── property_tests.rs           # Proptest/bolero semilattice + causal properties
│
├── fuzz/                           # Fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_idstime.rs
│   ├── fuzz_drift.rs
│   ├── fuzz_sync.rs
│   └── fuzz_commit_wait.rs
│
├── benches/                        # Benchmarks
│   ├── mod.rs
│   ├── tick_bench.rs               # Tick allocation latency
│   ├── sync_bench.rs               # Announcement processing throughput
│   ├── commit_wait_bench.rs        # Commit-wait latency
│   └── frame_bench.rs              # Reference frame merge cost
│
├── spec/                           # Formal specifications
│   ├── idstime_spec.md             # Idstime type and ordering laws
│   ├── drift_spec.md               # Drift absorption rules
│   ├── commit_wait_spec.md         # Commit-wait visibility guarantee
│   └── tla/                        # TLA+/PlusCal (in ids-doc crate)
│
├── docs/                           # Documentation
│   ├── architecture.md             # ids-time architecture
│   ├── integration_guide.md        # Integrating with ids-write/ids-commit-state
│   ├── time_model.md               # Proprietary time model rationale (not wall-clock, not HLC)
│   └── api_docs.md                 # Public API documentation
│
└── test_vectors/                   # Deterministic test vectors
    ├── idstime_vectors.json
    ├── drift_vectors.json
    ├── sync_vectors.json
    └── commit_wait_vectors.json
```

**Why ~150–300 files:** one file per responsibility — core types, tick source variants, drift absorption, synchronization, ordering, commit-wait, durability, wire format, crypto boundary, tests, fuzz, benchmarks, specs, docs, test vectors. No stubs; every file holds production-grade code with error handling, documentation, and tests.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Idstime Type and Total Order
```
Idstime { l: u64, c: u64, origin: NodeId, epsilon: Epsilon }

// Comparison: lexicographic on (l, c, origin)
// Laws: e hb f ⇒ Idstime.e < Idstime.f (causal consistency)
//       Idstime is strictly monotonic per origin (never decreases)
//       (l, c, origin) forms a total order
```

#### 7.2 Tick Advance (local event)
```
on_local_event():
    hint = wall_clock_hint()          // coarse, never authority
    l = max(local.l, hint.bounded_l()) // never decrease
    if l == local.l: c = local.c + 1   // same logical tick, advance counter
    else: c = 0                         // new logical tick, reset counter
    local = Idstime(l, c, origin, epsilon)
    return local
```

#### 7.3 Drift Absorption (remote announcement)
```
on_remote_announcement(a):  // a = (origin, l_a, c_a, eps_a, sig)
    verify ed25519 sig over canonical(a); drop if invalid
    if l_a > local.l + local.epsilon[origin]:       // far future
        flag_peer(origin); reject                   // suspicious
    elif l_a < local.l - local.epsilon[origin]:     // stale
        record_audit; reject
    else:                                           // within tolerance — absorb
        if l_a > local.l: local.l = l_a; local.c = 0
        elif l_a == local.l: local.c = max(local.c, c_a) + 1
        update reference frame region root for origin
```

#### 7.4 Ordering Timestamp Allocation
```
ordering_timestamp(domain):
    ts = tick_source.next_tick()       // fresh Idstime
    domain.pending_order.insert((ts, op_ref))
    return ts
```

#### 7.5 Commit Timestamp and Commit-Wait
```
commit_timestamp():
    ts = tick_source.next_tick()
    return ts

commit_wait(ts):
    // Visibility gate: ts + epsilon must be definitely in the past
    while tick_source.now().l < ts.l + ts.epsilon:
        wait(tick_source.tick_interval())
    // ts is now provably in the past; commit visible
```

#### 7.6 Cross-Region Reference Frame
```
ReferenceFrame: Map<RegionId, RegionRoot>
RegionRoot { l: u64, c: u64, epsilon: Epsilon }

frame_merge(remote_frame): componentwise max of (l, c) per region
frame_timestamp(regions): componentwise max over involved regions → Idstime
```

#### 7.7 Durability Window
```
DurabilityWindow { end_ts: Idstime, acked: bool }
window_close_check(): if tick_source.now() >= window.end_ts: close window
```

---

### 8. Dependencies

| Crate | Purpose |
|-------|---------|
| `ids-000-aa` | NodeId, RegionId, Endpoint, CarrierType, canonical types, sealed contracts |
| `ids-carrier` | Endpoint type (CarrierType + Vec<u8>) — sole transport |
| `ids-crypto` | ed25519, sha256, blake3 primitives (consumed via ids-000-aa re-exports) |
| `serde` + `serde_bytes` | Serialization |
| `thiserror` | Error types |
| `blake3` | Content hashing (workspace dependency) |
| `sha2` | SHA-256 hashing |
| `bytes` | Buffer handling |
| `proptest` / `bolero` | Property-based testing |
| `criterion` | Benchmarks |

No networking crates (libp2p/TCP/IP/UDP/QUIC/HTTP/DNS) — `ids-time` is transport-agnostic over `ids-carrier::Endpoint`. No `std::net` or `tokio::net`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 — only ed25519 (announcement signing) and sha256/blake3 (content hashing) are used here; xchacha20poly1305/chacha20poly1305 are permitted but not required.

---

### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement Method |
|---|-----------|-------------------|
| AC-005.1 | Idstime total order | Unit test: (l, c, origin) lexicographic comparison is total, transitive, antisymmetric |
| AC-005.2 | Causal consistency | Property test: e hb f ⇒ Idstime.e < Idstime.f over message-passing histories |
| AC-005.3 | Monotonicity | Property test: per-node Idstime never decreases across restarts and local events |
| AC-005.4 | Drift absorption | Unit test: remote l within epsilon adopted via max-update; outside epsilon rejected |
| AC-005.5 | Far-future rejection | Unit test: announcement with l > local + epsilon rejected and peer flagged |
| AC-005.6 | Replay resistance | Unit test: duplicate/stale (origin, l, c) announcements rejected |
| AC-005.7 | Signature verification | Zero announcements accepted without valid ed25519 signature (audit-log assertion) |
| AC-005.8 | Ordering timestamp assignment | Unit test: fresh Idstime per operation, strictly increasing per domain |
| AC-005.9 | Commit-wait visibility | Test: commit not visible until now().l >= ts.l + ts.epsilon |
| AC-005.10 | Causal-reverse prevention | Adversarial test: lagging-clock node cannot commit earlier than already-committed op |
| AC-005.11 | Reference frame convergence | Property test: gossiped region roots converge to componentwise max |
| AC-005.12 | Durability window | Unit test: window closes when now() >= end_ts; ack gate blocks past close |
| AC-005.13 | Wall-clock independence | Test: correct ordering with unsynchronized/stepping wall clocks (hint only) |
| AC-005.14 | Carrier agnosticism | Compiles and behaves identically with zero std::net/tokio.net/libp2p/TCP/UDP/QUIC/HTTP/DNS |
| AC-005.15 | Property tests | 100,000+ iterations (proptest/bolero) on merge/ordering/drift properties |
| AC-005.16 | Adversarial tests | Forged announcements, replay, far-future, tick flood — all fail-safe |

---

### 10. Remaining Uncertainties

1. **Tick source abstraction boundary:** The concrete monotonic clock source (OS `CLOCK_MONOTONIC` equivalent vs. hardware counter) is platform-specific; a minimal trait with default implementations needs definition against `ids-000-aa` types.
2. **Epsilon defaults:** Per-region and per-peer epsilon values (cross-region vs intra-region) are workload-dependent; defaults are placeholders pending deployment measurement.
3. **Wire format:** CBOR (deterministic, self-describing) vs bincode vs protobuf for tick announcements; CBOR favored for compactness and schema evolution.
4. **Commit-wait cost visibility:** The commit-wait delay (ts + epsilon in past) may be observable under high commit rates; overlap with WAL flush/replica acks mitigates but needs benchmarking.
5. **Reference frame GC:** Region roots grow unboundedly if regions are created and never pruned; retention/eviction policy for stale region roots unresolved.
6. **Formal verification scope:** TLA+/PlusCal spec in ids-doc proves causal consistency and commit-wait safety; extending to drift absorption and frame GC is additional effort.
7. **Restart recovery:** Persisted high-water mark initialization + safety margin after node restart needs crash-consistency design (WAL-like or periodic snapshot).

---

### 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Proprietary logical time, not wall-clock, not HLC | Wall-clock skew causes causal-reverse anomalies; HLC still depends on NTP-closeness; ids-time orders by causal tick propagation only |
| Idstime = (l, c, origin, epsilon) | O(1) size, total order, causal consistency, bounded uncertainty; fits in 128 bits |
| Monotonic tick source default, wall-clock as hint only | Never steps backward; wall-clock never an ordering authority |
| Drift absorption via max-update, never slew | Local l never decreases; remote info only raises it; avoids NTP step/slew problems |
| Signed tick announcements over ids-carrier::Endpoint | ed25519 prevents forged time; carrier-agnostic transport |
| Commit-wait: visibility until ts + epsilon definitely in past | Prevents causal-reverse without TrueTime hardware or global master |
| Cross-region reference frame (vector of region roots) | No global master; eventually consistent; per-region uncertainty |
| Durability windows keyed to Idstime | Logical (not wall-clock) durability commitments; correct across skew/latency |
| Allowed crypto only: ed25519 (signing), sha256/blake3 (hashing) | Within IDS crypto policy; xchacha20poly1305/chacha20poly1305 permitted but unused |
| ids-time depends on ids-000-aa; consumes crypto via re-exports | Follows crate dependency rule (ids-crypto is the sole exception) |

---

### 12. Sources Consulted (30 targeted searches)

1. Kulkarni & Demirbas, "Logical Physical Clocks" (HLC), OPODIS 2014 — https://cse.buffalo.edu/tech-reports/2014-04.pdf
2. Lamport, "Time, Clocks, and the Ordering of Events in a Distributed System," CACM 1978 — https://amturing.acm.org/p558-lamport.pdf
3. Google Spanner OSDI 2012 — TrueTime API, TTinterval, commit wait, external consistency — https://research.google.com/archive/spanner-osdi2012.pdf
4. Spanner: TrueTime and external consistency (Cloud Docs) — https://docs.cloud.google.com/spanner-omni/true-time-external-consistency
5. TrueTime implementation: time masters (GPS + atomic), timeslave daemon, Marzullo's algorithm, epsilon sawtooth 1–7ms — https://cs.brown.edu/courses/cs227/archives/2013/slides/spanner.pdf
6. CockroachDB HLC timestamps, max clock offset, causality token, linearizable mode commit-wait — https://pkg.go.dev/github.com/cockroachdb/cockroachdb-parser/pkg/util/hlc
7. CockroachDB "Living without atomic clocks" — clock skew causal-reverse anomaly, uncertainty interval — https://cockroachlabs.com/blog/living-without-atomic-clocks
8. CockroachDB Transaction Layer — HLC, MVCC timestamps, read refreshing — https://docs.cockroachlabs.com/docs/stable/architecture/transaction-layer
9. TiKV / TiDB Timestamp Oracle (Percolator TSO) — strictly increasing timestamps, batching, physical + logical parts — https://tikv.github.io/deep-dive-tikv/distributed-transaction/timestamp-oracle.html
10. K2: Optimizing Distributed Transactions with TrueTime Clocks — commit wait CWT = 2ε×(1+D), TTC uncertainty — https://arxiv.org/html/2504.01460
11. YugaByte DB: HLC + Raft log, CLOCK_MONOTONIC for leases — https://jepsen.io/analyses/yugabyte-db-1.1.9
12. Amazon Aurora DSQL: HLC, time bounds, known past/present/future intervals — https://aws.amazon.com/blogs/database/everything-you-dont-need-to-know-about-amazon-aurora-dsql-part-5
13. uhlc-rs Rust HLC implementation — max delta, unique identifiers — https://github.com/atolab/uhlc-rs
14. PWC: Physical clock With Causality — robustness to clock skew — https://arxiv.org/pdf/2104.15099
15. HLC vs HVC comparison — O(1) vs O(n), one-way vs two-way causality — https://ieeexplore.ieee.org/document/8323242
16. Efficient Algorithms for Predicate Detection using HLC — https://doi.org/10.1145/3007748.3007780
17. NTPv5 draft — offset/delay computation, root delay/dispersion, asymmetry — https://ietf.org/archive/id/draft-ietf-ntp-ntpv5-07.html
18. iHorology: path asymmetry in Internet time — https://arxiv.org/pdf/2011.06713
19. Clock synchronization with asymmetric delays — https://cs.stackexchange.com/questions/103/
20. IEEE 1588 PTP — sub-microsecond sync, grandmaster, transparent clocks — https://standards.ieee.org/ieee/1588/11795
21. G-SINC: Byzantine fault-tolerant global clock synchronization — https://arxiv.org/html/2207.06116v1
22. NTP root delay and root dispersion — https://blog.meinbergglobal.com/2021/02/25/the-root-of-all-timing-understanding-root-delay-and-root-dispersion-in-ntp
23. Tick source / kernel timekeeping — monotonic counter, cyc2ns conversion — https://kernel.org/doc/html/v6.0/_sources/timers/timekeeping.rst.txt
24. TaaS (Timestamp as a Service) — logical timestamps from consensusless clock cluster — https://vldb.org/pvldb/vol17/p994-li.pdf
25. Timestamp ordering concurrency control (CMU 15-445) — https://15445.courses.cs.cmu.edu/fall2025/notes/19-timestampordering.pdf
26. Jepsen CRDT convergence testing — property-based testing, deterministic simulation — https://cs.cmu.edu/~aldrich/courses/17-350/slides/11-testing.pdf
27. Hybrid logical clock linearizability/serializability — https://stackoverflow.com/questions/77699961/
28. HLC testing strategy — convergence as oracle — https://github.com/bindian0509/system-design/blob/master/collaborative-editor/docs/07-testing-strategy.md
29. Spanner safe time / tsafe — replica up-to-date timestamp — https://cs.cornell.edu/courses/cs5414/2017fa/papers/Spanner.pdf
30. Clock drift compensation / Kalman filter tracking — https://ieeexplore.ieee.org/document/10649634

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, interfaces, data structures, algorithms, dependencies, integration points, acceptance criteria, and file structure for Phase 005 (ids-time).