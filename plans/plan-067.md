## Phase 067 — Create the ids-hlc crate: Hybrid logical clock for ordering.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 1000-1014)

```
67. Research only do not read plan.md **[Phase 067 — Create the ids-hlc crate: Hybrid logical clock for ordering.]**

Crate: ids-hlc
Purpose: Hybrid logical clock for ordering.
Contents: HLC implementation, physical+logical time combination, clock skew handling, ordering guarantees, fallback to IDS standard time.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 067 — Create the ids-hlc crate: Hybrid logical clock for ordering.]**

Create plan-067.md in C:/idfs/plans/ for the ids-hlc crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 067 creates `ids-hlc`, a **separate crate** (not a folder inside an existing crate, per the project rule of separate crates for new capabilities), providing the hybrid logical clock (HLC) used for ordering events across the IDS workspace. An HLC timestamp combines a physical time component with a logical counter: it stays close to wall-clock time (human-readable, usable for TTLs and snapshot reads) while preserving the happened-before causality relation of logical clocks — if event `e` happened before event `f`, then `hlc.e < hlc.f` (Kulkarni et al., "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases," OPODIS 2014 / SUNY Buffalo TR 2014-04).

`ids-hlc` is **local clock logic only**: it generates, merges, compares, and packs HLC timestamps. It never opens a socket and imports no networking crate. Any HLC-stamped payload exchanged between nodes rides as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>` ≤ 1024 bytes); the framing and transport belong to `ids-carrier` / the caller, never to `ids-hlc`.

**Contents (per prompts.txt entry 67):** HLC implementation, physical+logical time combination, clock skew handling, ordering guarantees, fallback to IDS standard time. The fallback means: when the physical clock source is unavailable or untrusted (backward wall-clock jump, no NTP), `ids-hlc` derives its physical component from `ids-time` (`IdsTime`, monotonic u64 ticks defined via `ids-000-aa`), degrading gracefully to a logical-clock-over-monotonic-ticks mode while staying monotonic and ordered.

**Constraints:** carrier-agnostic (no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh); allowed crypto only `ed25519`, `xchacha20poly1305`, `sha256`, `blake3`, `chacha20poly1305` (implemented in `ids-crypto`; `ids-hlc` consumes primitive IDs/types from `ids-000-aa`/`ids-crypto` only, or stays crypto-free); `ids-000-aa` is the master crate holding canonical types (`NodeId`, `IdsTime`, `Error`, …) and sealed trait contracts.

A skeleton `ids-hlc` crate does NOT yet exist at `C:/idfs/ids-hlc/`. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-hlc` sits above `ids-000-aa` (canonical types, sealed contracts, errors) and `ids-time` (IDS standard time fallback). It is a pure, local, sync-first clock module: every node holds its own HLC instance; timestamps are minted locally, merged on message receipt, compared lexicographically, and optionally packed into a fixed-width integer for wire storage. The crate itself never sends or receives bytes — stamped data is framed by the caller into opaque `Vec<u8>` datagrams addressed to `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                      CALLER LAYER (ids-*, clients)                           │
│  ids-store (MVCC version timestamps), ids-consistency-profile (Phase 060)   │
├──────────────────────────────────────────────────────────────────────────────┤
│                    Phase 067: ids-hlc (THIS CRATE — local clock)             │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌────────────────┐   │
│  │ clock source │ │   hlc core   │ │ ordering /      │ │  fallback to   │   │
│  │ (physical)   │ │  (l, c, id)  │ │ total-order tie │ │  IdsTime ticks │   │
│  └──────────────┘ └──────────────┘ └─────────────────┘ └────────────────┘   │
│  ┌──────────────┐ ┌──────────────┐ ┌─────────────────┐                       │
│  │   encode     │ │   skew bound │ │   crypto surf.  │                       │
│  │   (64/u128)  │ │   (epsilon)  │ │   (optional)    │                       │
│  └──────────────┘ └──────────────┘ └─────────────────┘                       │
├──────────────────────────────────────────────────────────────────────────────┤
│              NO std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC /     │
│              HTTP / DNS — HLC math is local; no network stack in trust base  │
│  HLC-stamped bytes leave the node only as opaque Vec<u8> via                 │
│  ids-carrier::Endpoint (CarrierType + Vec<u8> ≤ 1024 B), sent by the caller  │
└──────────────────────────────────────────────────────────────────────────────┘
```

**HLC data model** (Kulkarni et al. 2014): each node maintains `(l, c)` where `l` is the highest physical time observed (from the node's own clock or received messages) and `c` is a bounded logical counter. Comparison is lexicographic: `l` first, then `c`; a `node_id` (from `ids-000-aa`) breaks remaining ties for a total order.

**Core rules (research-verified):**
- Local event / send: `l' = max(l, pt)` where `pt` is the current physical time; if `l' == l` then `c += 1` (physical stalled — break tie logically), else `c = 0`.
- Receive with remote stamp `(lm, cm)`: `l' = max(l, pt, lm)`; counter update by source of the maximum: if `l' == l == lm` → `c = max(c, cm) + 1`; elif `l' == l` → `c += 1`; elif `l' == lm` → `c = cm + 1`; else `c = 0`.
- Strict monotonicity: the physical component never decreases, so backward wall-clock jumps (NTP steps, leap seconds) are absorbed into the logical counter rather than moving time backward.
- Bounded skew assumption `ε`: `|l − pt| ≤ ε`; production systems (CockroachDB) use a max clock offset of ~500 ms; outliers beyond the guard are clamped or rejected.
- Counter bounded in practice (O(n·ε) in theory); if the counter ceiling is reached while physical time stalls, the clock blocks or raises a defined overflow error (never wraps silently).

**Fallback to IDS standard time:** when the physical source is missing or distrusted, `ids-hlc` sources `pt` from `ids-time::IdsTime` (monotonic u64 ticks, never wall-clock), so ordering guarantees survive total loss of physical time — the clock degrades to a pure logical counter over monotonic ticks. Fallback entry/exit conditions and hysteresis are configurable per node.

**Crypto surface:** `ids-hlc` performs no cryptography by default. If a caller wants timestamps signed or hashed (audit checkpoints on profile decisions), it uses only `ed25519` (signing), `blake3`/`sha256` (digesting timestamps), via `ids-crypto` primitive IDs exposed through `ids-000-aa`. `xchacha20poly1305`/`chacha20poly1305` are available for encrypting clock state at rest. No other algorithm may enter the crate.

---

### 3. Detailed Research Dimensions

Each dimension maps to one or more of the 30 targeted searches executed. Sources are cited inline; full list in Section 12.

#### 3.1 HLC fundamentals (Kulkarni et al., 2014)

**Source:** S. Kulkarni, M. Demirbas, D. Madeppa, B. Avva, M. Leone, "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases," OPODIS 2014 (LNCS 8878) / SUNY Buffalo Tech Report 2014-04. https://cse.buffalo.edu/tech-reports/2014-04.pdf ; https://doi.org/10.1007/978-3-319-14472-6_2

HLC bridges logical clocks (causality tracking, no physical time) and physical clocks (real-time correspondence, best-effort NTP). Properties:
- **Causality preservation:** `e hb f ⇒ hlc.e < hlc.f` (one-way happened-before captured).
- **Close to physical time:** `l` stays within clock-skew bound `ε` of NTP/pt; HLC substitutable for physical clocks in applications such as snapshot reads in MVCC globally distributed databases.
- **Strictly monotonic:** tolerates NTP kinks — non-monotonic POSIX updates, leap seconds — without time going backward.
- **Compact:** fits in a 64-bit NTP timestamp format; masking-tolerant to NTP uncertainties.
- **Benefits:** wait-free transaction ordering; consistent snapshots without waiting out clock-sync uncertainty or prior coordination (a posteriori snapshot identification).

`ids-hlc` adopts this as its core contract: causality + bounded real-time proximity + monotonicity.

#### 3.2 HLC data structure and bit packing

**Sources:** https://github.com/glycerine/hlc (int64: upper 48 bits physical ≈ 0.1 ms resolution, lower 16 bits logical counter); https://lib.rs/crates/hlc-gen (lock-free Rust, AtomicU64, 42-bit wall-clock from 2024-01-01 epoch + 22-bit logical, ~4M events/ms, no_std compatible); https://docs.rs/hlc (wall time + logical component); https://docs.rs/crate/uhlc (unique HLC, u128 node id, max-delta drift guard); https://docs.rs/meridian-core (wall_ms u64, logical u16, node_id u64, `Ord` by field order); https://arxiv.org/html/1702.04263v1 (Okapi: 64-bit hybrid timestamp, 48 MSB physical microsecond + 16 LSB logical); https://pingcap.com/blog/time-in-distributed-systems (TiDB TSO centralized HLC: 46-bit physical + 18-bit logical, 262M timestamps/s, sleeps 50 ms when logical ceiling reached).

Common packing: a single `u64` = physical high bits + logical low bits. For `ids-hlc` the natural representation is the unpacked tuple `(physical: u64, logical: u64, node_id: Option<NodeId>)` with optional pack/unpack to `u64` (configurable split, default 48/16) or `u128` (full 64-bit physical + logical + id). Decision deferred (see Section 10).

#### 3.3 Local event / send algorithm

**Sources:** https://singhajit.com/distributed-systems/hybrid-clock ; https://martinfowler.com/articles/patterns-of-distributed-systems/hybrid-clock.html ; Kulkarni 2014 Algorithm 1.

```
now = max(l, pt)          // pt = physical clock read
if now == l:   c += 1     // physical did not advance → break tie logically
else:          c = 0      // physical advanced → reset counter
l = now
return (l, c[, node_id])
```

The attached stamp travels with every outgoing message/datagram. Sender and receiver each maintain independent HLC state — no centralized timestamp oracle (contrast TiDB TSO / Percolator), so no single point of failure and no round-trip per timestamp (research: unmeshjoshi/hybridclocktxn).

#### 3.4 Receive / merge algorithm (four-branch)

**Sources:** Kulkarni 2014; https://singhajit.com/distributed-systems/hybrid-clock ; https://github.com/cockroachdb/cockroach/blob/master/pkg/util/hlc/hlc.go.

On receiving remote stamp `(lm, cm)` with local physical `pt`:
```
l' = max(l, pt, lm)
if   l' == l  == lm: c = max(c, cm) + 1
elif l' == l:        c = c + 1
elif l' == lm:       c = cm + 1
else:                c = 0
l = l'
```

This is the Lamport-style `max` generalized to include physical time: the receiver never moves its clock backward, and every received message that carries newer knowledge pulls the local clock forward. Merge is idempotent: applying the same stamp twice yields the same state as applying it once.

#### 3.5 Ordering guarantees

- **Lexicographic compare:** `(l1, c1, id1) < (l2, c2, id2)` iff `l1 < l2`, or `l1 == l2 ∧ c1 < c2`, or `l1 == l2 ∧ c1 == c2 ∧ id1 < id2`. The `node_id` tie-break gives a deterministic total order even for concurrent events on different nodes (arbitrary but consistent — same convention as Lamport/vector-clock total-order tie-breaks).
- **Causality:** `e → f` (program order or send/receive chain) ⇒ `hlc(e) < hlc(f)`. One-way causality — `hlc.e < hlc.f` does NOT imply `e hb f` (concurrent events can compare either way); that is inherent to scalar clocks.
- **NOT linearizable/external consistency by itself:** HLC gives causal ordering and monotonicity; real-time (external) consistency additionally needs bounded skew + uncertainty windows / read restarts (CockroachDB max-offset 500 ms, follower-read staleness ~4 s; Spanner TrueTime needs GPS/atomic hardware + commit wait — out of scope for IDS commodity assumption).
- **Snapshot reads:** a read at HLC `t` returns a consistent cut across replicas — the core benefit cited in Kulkarni 2014 and used by CockroachDB/YugabyteDB MVCC.

#### 3.6 Clock skew handling

**Sources:** https://arxiv.org/html/2104.15099v2 (PWC paper: HLC overview, `|l.e − pt.e| ≤ ε`, counter bound linear in n and ε in theory, small in practice; 48-bit packing stores `l − pt` (12 bits) + `c` (4 bits)); https://yugabyte.com/blog/evolving-clock-sync-for-distributed-databases (NTP typical accuracy 5 ms–100 ms+; CockroachDB/YugabyteDB observe 100–250 ms max skew; 500 ms safe default max offset); https://docs.cockroachlabs.com (max clock offset, uncertainty interval `[T, T + max offset]`, read restart above ambiguous values).

- **Assumption:** physical clocks of any two nodes differ by at most `ε` (loosely synchronized via NTP/PTP; correctness of ordering does not depend on precision, only on the bound existing).
- **Guard:** `max_offset` — how far ahead of the local physical clock/cluster time the wall time may advance; remote stamps beyond the guard are logged as skew violations (clamped or rejected per policy).
- **Backward jumps:** physical component of HLC = max(observed physical times); a wall clock that steps backward simply fails to advance `l`, so the logical counter absorbs the events — time never regresses.
- **Counter bound:** `c` ceiling; when reached and physical time has not advanced, the clock must wait (Okapi) or raise — configurable, never silent wraparound.
- **Drift observation:** krabka-hlc reports clock drift on the data path (records pulling the clock forward = measured skew).

#### 3.7 Fallback to IDS standard time (`ids-time` / `IdsTime`)

**Prompt requirement (entry 67 line 1004):** "fallback to IDS standard time." `ids-000-aa` defines `IdsTime`; `ids-time` (Phase 005) builds the proprietary `Idstime` on it. `ids-hlc`'s physical clock source is injectable:
- Primary: system physical time (NTP-disciplined wall clock, monotonicized via max()).
- Fallback: `IdsTime` monotonic ticks when the physical source is absent, untrusted (backward jump beyond tolerance), or explicitly disabled.
- In fallback mode the clock degenerates to a Lamport-style logical counter over monotonic ticks — still strictly ordered, still causality-preserving, just no real-time correspondence until physical time returns.
- Fallback entry/exit: hysteresis to avoid flapping; timestamped log of fallback duration (observable, not silent).

#### 3.8 Physical + logical time combination

The combination rule is exactly the local-event and receive rules above: `l` tracks the maximum physical time ever observed (own or remote), `c` captures ordering that physical time cannot express (same-tick events, skew-absorbed events). `|l − pt|` stays bounded by `ε` provided messages carry stamps and clocks are loosely synchronized — this bounded distance is what makes HLC timestamps meaningful for humans, TTLs, and log correlation, unlike pure Lamport counters.

#### 3.9 HLC vs Lamport clocks vs vector clocks vs TrueTime

**Sources:** https://cs.stackexchange.com/questions/101496 (Lamport vs vector clocks; total order via node ID tie-break; linearizability stronger than either); https://en.wikipedia.org/wiki/Logical_clock; https://martinfowler.com/articles/patterns-of-distributed-systems/hybrid-clock.html.

| Clock | Real-time meaning | Causality | Size | Linearizable? |
|-------|-------------------|-----------|------|---------------|
| Physical (NTP) | Yes (best effort) | No | 64 bits | No (skew) |
| Lamport | No | Yes (one-way) | 1 counter | No |
| Vector clock | No | Yes (exact, concurrent detectable) | O(n) | No |
| HLC | Yes (bounded by ε) | Yes (one-way) | 64–128 bits | No (needs uncertainty windows) |
| TrueTime | Yes (bounded ε, hardware) | Yes | interval | Yes (commit wait) |

IDS adopts HLC because it needs both ordering and human-meaningful timestamps without GPS/atomic hardware and without a centralized timestamp oracle.

#### 3.10 Production precedent

- **CockroachDB:** HLC = transaction timestamps for MVCC; `maxOffset` 500 ms default; uncertainty interval on reads; follower reads with bounded staleness (~4 s closed timestamp). https://docs.cockroachlabs.com/docs/stable/architecture/transaction-layer
- **YugabyteDB:** 64-bit HLC (physical microseconds + logical counter); xCluster last-writer-wins picks greater HLC; monotonic per node. https://docs.yugabyte.com/stable/architecture/transactions/transactions-overview/#hybrid-logical-clocks
- **TiDB:** centralized TSO hybrid logical clock (PD), 46+18-bit packing. https://pingcap.com/blog/time-in-distributed-systems
- **MongoDB:** HLC/ClusterTime as read and commit timestamps; causal consistency sessions; operationTime propagated in every message; NTP not enforced between nodes. https://mongodb.com/company/blog/engineering/casual-guarantees-anything-casual ; https://arxiv.org/html/2111.14946v3
- **Okapi:** HLC for causally consistent geo-replication; responds immediately despite skew by advancing HLC to snapshot time. https://arxiv.org/html/1702.04263v1

#### 3.11 Testing: clock skew injection and fault injection

**Sources:** https://onenoughtone.com/learn/failure-injection/5 (clock skew causes silent correctness violations; skew injection makes nodes disagree about time); CockroachDB HLC tests for backward jumps and `maxOffset` violations.

`ids-hlc` tests must inject: backward wall-clock jumps, per-node skew (±ε), message delay/reordering/duplication (idempotent merge), counter-ceiling reach, fallback entry/exit, and monotonicity assertions under adversarial schedules. Deterministic injectable physical clock (like CockroachDB's `physicalClock func()` or Flynn's injectable clock) makes this reproducible.

#### 3.12 Crypto surface (allowed set only)

**Sources:** https://docs.rs/ed25519-dalek ; https://docs.rs/chacha20poly1305 ; https://docs.rs/sha2 ; https://github.com/BLAKE3-team/BLAKE3.

`ids-hlc` is crypto-free by default. Optional, declared-only surface (implemented in `ids-crypto`, referenced via `ids-000-aa` primitive IDs):
- `ed25519`: signing HLC-stamped decisions/audit checkpoints.
- `blake3` / `sha256`: digesting timestamps, timestamped state hashing.
- `xchacha20poly1305` / `chacha20poly1305`: encrypting clock state at rest (bounded nonce use).
No other algorithm; allowlist enforced by unit test and CI dependency audit.

---

### 4. Threat Model & Mitigations

|| Threat | Asset at risk | Mitigation (this crate) ||
||--------|-----------|-------------------------------|--------------------------||
|| Clock skew between nodes > ε | Ordering correctness | Bounded-skew assumption; `max_offset` guard; remote outliers clamped/rejected and logged ||
|| Wall clock jumps backward (NTP step, leap second) | Monotonicity | Physical component = max(observed); never decreases; logical counter absorbs ||
|| Logical counter ceiling reached | Liveness / correctness | Defined overflow policy (wait for physical advance or configurable error); never wrap silently ||
|| Duplicate/delayed datagram (carrier replays) | Idempotence | Receive merge is idempotent; applying same stamp twice == once ||
|| Node-id tie-break collision | Total order determinism | `NodeId` from `ids-000-aa` (32-byte, collision-resistant); tie-break arbitrary but fixed ||
|| Fallback hides loss of physical time forever | Observability | Timestamped fallback entry/exit with hysteresis; metrics on fallback duration ||
|| Unchecked drift | Real-time correspondence | `|l − pt| ≤ ε` enforced; drift observed on data path (krabka-style observation seam) ||
|| Prohibited crate sneaking in | Carrier-agnostic rule | CI dependency audit (`cargo tree`); grep bans `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches ||
|| Disallowed crypto algorithm | Crypto policy | Only `ed25519`, `xchacha20poly1305`, `sha256`, `blake3`, `chacha20poly1305` reachable; via `ids-crypto` only; allowlist unit test ||
|| Timestamp forgery / replay | Decision authenticity | Optional `ed25519`-signed audit checkpoints over stamped bytes (survives carrier re-framing) ||

The carrier is never trusted: HLC ordering is enforced by the clock math, and stamped bytes are self-describing; the carrier-agnostic boundary (`Endpoint = CarrierType + Vec<u8>`) keeps the network stack out of the trust base.

---

### 5. Integration with Existing Crates

|| Crate | Role | Relevance to ids-hlc ||
||-------|------|--------------------------|--------------------------------------||
|| `ids-000-aa` | Master crate: canonical types, sealed trait contracts, invariants, errors | `NodeId` (tie-breaker), `Error`, sealed contracts consumed — never redefined ||
|| `ids-time` | IDS standard time: `IdsTime` monotonic u64 ticks | Fallback physical source when wall clock untrusted ||
|| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`), `send_datagram`/`receive_datagram` | HLC-stamped payloads ride `Endpoint` datagrams; `ids-hlc` never sends/receives directly ||
|| `ids-crypto` | Allowed primitives only | Optional signing/digest of HLC-stamped decisions; `ids-hlc` declares type surface only ||
|| `ids-store` (later phase) | MVCC versioned store | HLC as version/timestamp component for MVCC ordering ||
|| `ids-consistency-profile` (Phase 060) | Read/write behavior per profile | HLC as causality input for profile admission decisions ||
|| `ids-replication` (later phase) | Replica synchronization | Ships HLC-stamped ops as `Endpoint` datagrams ||
|| All other `ids-*` | Consumers of timestamps | Import `ids-hlc::HlcTimestamp` via public API; never reach into internals ||

**Integration points:** `ids-hlc` exposes `HlcClock` (`now()`, `send_stamp()`, `receive(stamp)`), `HlcTimestamp` (comparable, encodable), and `HlcConfig` (skew bound, overflow policy, fallback toggle). All traffic that leaves a node is opaque `Vec<u8>` to `ids-carrier::Endpoint`; `ids-hlc` has no `send`/`receive` of its own and depends on no transport crate.

---

### 6. Proposed File Structure (~90–150 files, within the ~90–1000 target)

**Crate:** `ids-hlc` (new crate at `C:/idfs/ids-hlc/`; depends on `ids-000-aa`, `ids-time`; serde/thiserror):

```
ids-hlc/
├── Cargo.toml                          # ids-000-aa, ids-time, serde, thiserror; workspace member
├── lib.rs                              # crate root; public API re-exports; feature gates
├── constants.rs                        # defaults: skew epsilon, max offset, counter ceiling, bit split
├── error.rs                            # HlcError (thiserror): skew, overflow, fallback, encoding errors
├── types.rs                            # HlcTimestamp {physical, logical, node_id}, HlcConfig, SkewBound
├── clock.rs                            # injectable physical clock source (system now / IdsTime fallback / test clock)
├── hlc.rs                              # core HLC state: now(), send_stamp(), receive()
├── merge.rs                            # receive four-branch merge (Kulkarni Algorithm 1)
├── ordering.rs                         # lexicographic compare + node_id total-order tie-break
├── encode.rs                           # pack/unpack to u64 (48/16 default) or u128; round-trip
├── skew.rs                             # epsilon bound check, max_offset guard, monotonicity error counter
├── fallback.rs                         # IdsTime fallback trigger, entry/exit hysteresis
├── crypto_surface.rs                   # declared optional surface: digest/sign types (ids-crypto implements)
│
├── contracts/                          # sealed trait contracts (~6 files)
│   ├── mod.rs
│   ├── hlc_clock.rs                    # trait HlcClock (now / send / receive)
│   ├── hlc_source.rs                   # injectable physical-time source
│   ├── hlc_encoder.rs                  # pack/unpack contract
│   ├── fallback_policy.rs              # fallback trigger policy
│   └── ordering_contract.rs            # total-order comparison contract
│
├── invariants/                         # runtime/compile-time invariants (~8 files)
│   ├── mod.rs
│   ├── monotonic_physical.rs           # physical component never decreases
│   ├── bounded_skew.rs                 # |l − pt| ≤ ε
│   ├── bounded_counter.rs              # counter ceiling, overflow policy
│   ├── total_order.rs                  # compare is antisymmetric, transitive, total with node_id
│   ├── merge_idempotent.rs             # duplicate receive == single receive
│   ├── fallback_sound.rs               # fallback preserves monotonicity/ordering
│   ├── carrier_agnostic.rs             # no networking types in crate
│   └── crypto_allowlist.rs             # only allowed algorithms reachable
│
├── tests/                              (~20 files)
│   ├── hlc_tests.rs                    # local tick, send/receive, causality e hb f ⇒ hlc.e < hlc.f
│   ├── merge_tests.rs                  # four-branch merge; idempotence
│   ├── ordering_tests.rs               # lexicographic + node_id tie-break total order
│   ├── skew_tests.rs                   # backward jumps, per-node skew, max_offset guard
│   ├── counter_tests.rs                # ceiling, overflow policy, no wrap
│   ├── encode_tests.rs                 # u64/u128 pack round-trip, compare consistency
│   ├── fallback_tests.rs               # IdsTime fallback entry/exit, hysteresis
│   ├── determinism_tests.rs            # injectable clock ⇒ reproducible timestamps
│   ├── dependency_audit_test.rs        # no prohibited crates in tree
│   └── crypto_allowlist_test.rs        # allowed-crypto test
│
├── benches/                            (~6 files)
│   ├── now_bench.rs                    # timestamp minting throughput
│   ├── merge_bench.rs                  # receive-merge latency
│   ├── compare_bench.rs                # comparison throughput
│   ├── skew_bench.rs                   # skew-guard cost
│   ├── encode_bench.rs                 # pack/unpack cost
│   └── fallback_bench.rs               # fallback switch cost
│
├── docs/                               (~8 files)
│   ├── architecture.md
│   ├── algorithm.md                    # Kulkarni rules restated for implementers
│   ├── skew_policy.md                  # epsilon / max_offset tuning
│   ├── fallback.md                     # IdsTime fallback contract
│   ├── encoding.md                     # 64-bit / 128-bit packing
│   ├── testing.md                      # skew injection procedure
│   ├── crypto_policy.md                # allowed-crypto allowlist
│   └── adding_a_node_id.md             # tie-break configuration
│
├── spec/                               (~3 files)
│   ├── hlc_spec.md                     # normative: timestamp, merge, compare
│   ├── fallback_spec.md                # fallback-to-IdsTime contract
│   └── ordering_spec.md                # total-order contract
│
└── README.md
```

**Why ~90–150 files:** one file per responsibility — clock source abstraction, core HLC, merge, ordering, encoding, skew bounds, fallback, contracts, invariants, plus tests, benches, docs, and specs. Matches the "separate crates instead of more folders" directive applied intra-crate as separate files; room to grow toward ~1000 as bit-packing variants and clock-source adapters are added.

---

### 7. Key Algorithms & Data Structures

1. **HLC timestamp tuple** — `HlcTimestamp { physical: u64, logical: u64, node_id: Option<NodeId> }`; `NodeId` from `ids-000-aa` breaks final ties.
2. **Local tick / send stamp** — `now = max(l, pt)`; if `now == l` then `c += 1` else `c = 0`; `l = now`. Physical advance resets counter; stall bumps it.
3. **Receive merge** — `l' = max(l, pt, lm)`; counter set by source of the max (four-branch, Kulkarni 2014). Monotonic, idempotent, no backward movement.
4. **Total order** — compare `l`, then `c`, then `node_id`; yields a deterministic total order consistent with causality without a central coordinator.
5. **Monotonic physical enforcement** — `l := max(l, new_physical)`; backward wall-clock jumps absorbed into `c`; never decreases.
6. **Bounded skew guard** — `|l − pt| ≤ ε`; `max_offset` rejects/clamps remote stamps too far ahead; monotonicity error counter for observability.
7. **Counter overflow policy** — bounded `c`; at ceiling with stalled physical time: block or raise `HlcError::CounterCeiling` (configurable); never silent wraparound.
8. **Fixed-width encoding** — pack `(physical, logical)` into `u64` (default 48-bit physical / 16-bit logical per CockroachDB/Okapi precedent; configurable split) or `u128` (full 64-bit physical + logical + id); packed `u64` comparison equals tuple comparison.
9. **IDS standard-time fallback** — injectable clock source; on physical-source failure/untrust, `pt := IdsTime` ticks (monotonic); ordering guarantees preserved, real-time proximity suspended until physical time returns.
10. **Idempotent merge** — receiving an already-seen stamp leaves state unchanged; safe under carrier duplication/reordering.

---

### 8. Dependencies

`ids-hlc/Cargo.toml`:
- `ids-000-aa = { path = "../ids-000-aa" }` — `NodeId`, `Error`, sealed trait contract patterns
- `ids-time = { path = "../ids-time" }` — `IdsTime` monotonic ticks (fallback physical source)
- `serde = { version = "1", features = ["derive"] }` — timestamp serialization
- `thiserror = "2.0"` — `HlcError` with context variants

Optional feature-gated:
- `crypto` — declares digest/sign types consumed from `ids-crypto` (allowed set only); no crypto implemented here

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required (sync core; optional async adapter downstream). `cargo tree` must show zero networking crates.

Downstream crate dependencies: any `ids-*` crate that needs timestamps declares `ids-hlc = { path = "../ids-hlc" }` and depends on `ids-000-aa` (transitively).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------||
|| AC-067.1 | `HlcTimestamp` = (physical u64, logical u64, node_id); logical resets to 0 when physical advances, else increments | Unit tests on `now()` under synthetic clock ||
|| AC-067.2 | Causality: e hb f ⇒ hlc.e < hlc.f | Send/receive test harness with injectable clock; 100% of causally-linked pairs ordered ||
|| AC-067.3 | Total order: compare(l, c, node_id) is total, antisymmetric, transitive; no ambiguous distinct events | Property test over random timestamps ||
|| AC-067.4 | Monotonicity: physical component never decreases, even under backward wall-clock jumps | Skew-injection test: backward jumps of up to ±60 s; assert monotonic output ||
|| AC-067.5 | Bounded skew: |l − pt| ≤ ε enforced; `max_offset` guard rejects/clamps outliers | skew guard unit test; violation counter increments ||
|| AC-067.6 | Bounded logical counter: ceiling enforced; overflow → defined wait/error, never wrap | counter-ceiling test under stalled physical time ||
|| AC-067.7 | Fallback to `IdsTime`: on physical-source failure, clock remains monotonic/ordered over ticks | fallback entry/exit test; results identical modulo real-time proximity ||
|| AC-067.8 | Encoding: u64 (48/16 default) and u128 pack/unpack round-trip; packed comparison == tuple comparison | encode/decode property test ||
|| AC-067.9 | Idempotent receive: duplicate stamp application == single application | receive(stamp) twice == once ||
|| AC-067.10 | Carrier agnostic: grep for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches | dependency audit + source grep test passes ||
|| AC-067.11 | Crypto allowlist: only `ed25519`, `xchacha20poly1305`, `sha256`, `blake3`, `chacha20poly1305` reachable, via `ids-crypto` only | allowlist unit test passes ||
|| AC-067.12 | Compiles cleanly | `cargo check -p ids-hlc` exits 0 with no new warnings ||
|| AC-067.13 | Tests pass | `cargo test -p ids-hlc` passes all unit/property/integration tests ||

---

### 10. Remaining Uncertainties

1. **Physical clock source abstraction:** injectable function pointer (CockroachDB `physicalClock fn`), trait object, or `IdsTime`-only? Affects testability and fallback design.
2. **Bit allocation:** 64-bit packed (`u64`, e.g. 48/16 or 52/12 per YugabyteDB; 42/22 per `hlc-gen`) vs `u128` (full 64-bit physical + logical + node id)? Trade wire size vs timestamp longevity vs node-id inclusion.
3. **Fallback trigger thresholds:** how far behind / how many consecutive backward jumps before switching to `IdsTime`, and hysteresis for switching back — workload-dependent, unquantified.
4. **Counter overflow policy:** block, error, or forcibly advance physical? Depends on latency SLAs of consumers (ids-store MVCC, ids-consistency-profile).
5. **Node ID inclusion:** is `node_id` part of `HlcTimestamp` itself or appended by the caller at comparison time? Affects wire encoding size.
6. **Interaction with `ids-consistency-profile` (Phase 060):** does the profile consume `HlcTimestamp` directly or through a wrapper? Defined here, validated at integration.
7. **`IdsTime` tick resolution:** sufficient for sub-millisecond ordering needs of MVCC/read paths? To be confirmed against `ids-time` spec.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------||
|| Separate `ids-hlc` crate (not a folder) | Project rule: separate crates for new capabilities; keeps clock logic independent of stores ||
|| Tuple `(physical, logical, node_id)` | Total order without central coordinator; node_id tie-break per Lamport/vector-clock convention and HLC implementations (meridian-core, uhlc) ||
|| Kulkarni four-branch receive merge | Research-verified algorithm; monotonic, idempotent, skew-absorbing ||
|| Bounded-skew assumption `ε` + `max_offset` guard | Matches production (CockroachDB 500 ms max offset, YugabyteDB 100–250 ms observed skew); ordering correct under the bound ||
|| Fallback to `ids-time::IdsTime` | Prompt requirement (fallback to IDS standard time); monotonic ticks preserve ordering without physical time ||
|| Sync core, no async runtime | Clock math is CPU-local; no runtime needed; optional async adapters downstream ||
|| Allowed crypto only, via `ids-crypto` | IDS crypto policy; `ids-hlc` crypto-free by default ||
|| No networking types in crate | Carrier-agnostic absolute rule; timestamps leave only as `ids-carrier::Endpoint` datagrams framed by caller ||

---

### 12. Sources

1. Kulkarni, Demirbas, Madeppa, Avva, Leone — "Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases," OPODIS 2014 (LNCS 8878) / SUNY Buffalo TR 2014-04 — https://cse.buffalo.edu/tech-reports/2014-04.pdf ; https://doi.org/10.1007/978-3-319-14472-6_2 — HLC definition, causality `e hb f ⇒ hlc.e < hlc.f`, merge algorithm, 64-bit NTP fit, NTP-kink tolerance.
2. CockroachDB HLC implementation — https://github.com/cockroachdb/cockroach/blob/master/pkg/util/hlc/hlc.go — `Clock` struct, `maxOffset` guard, backward-jump detection, remote-stamp merge.
3. CockroachDB docs, "Transaction Layer" — HLC as transaction timestamps for MVCC; max clock offset ~500 ms; uncertainty interval `[T, T + max offset]` — https://docs.cockroachlabs.com/docs/stable/architecture/transaction-layer
4. YugabyteDB docs, "Hybrid Logical Clocks" — 64-bit HLC (physical microseconds + logical counter), monotonic per node, xCluster LWW — https://docs.yugabyte.com/stable/architecture/transactions/transactions-overview/#hybrid-logical-clocks
5. YugabyteDB blog, "Evolving Clock Sync for Distributed Databases" — NTP accuracy 5 ms–100 ms+, 100–250 ms observed max skew, 500 ms safe default — https://yugabyte.com/blog/evolving-clock-sync-for-distributed-databases
6. TiDB/PingCAP, "Time Synchronization in Distributed Systems: TiDB's Timestamp Oracle" — centralized HLC TSO, 46-bit physical + 18-bit logical, 262M timestamps/s, 50 ms sleep at logical ceiling — https://pingcap.com/blog/time-in-distributed-systems
7. MongoDB engineering, "Causal Guarantees Are Anything but Casual" — HLC/ClusterTime as read/commit timestamps, operationTime in every message, NTP not enforced between nodes — https://mongodb.com/company/blog/engineering/casual-guarantees-anything-casual
8. arXiv 2111.14946 — "Verifying Transactional Consistency of MongoDB" — HLC read and commit timestamps, lexicographic total order, tick increment — https://arxiv.org/html/2111.14946v3
9. arXiv 2104.15099 — "Achieving Causality with Physical Clocks" (PWC) — HLC overview, `|l − pt| ≤ ε`, counter bound linear in n and ε, 48-bit packing (l−pt 12 bits, c 4 bits) — https://arxiv.org/html/2104.15099v2
10. arXiv 1702.04263 — "Causally Consistent Geo-Replication Made Faster, Cheaper and More Available" (Okapi) — HLC under clock skew, 48/16-bit hybrid timestamp, microsecond granularity — https://arxiv.org/html/1702.04263v1
11. glycerine/hlc (Go) — hybrid logical/physical clock, int64 = upper 48 bits physical (~0.1 ms) + lower 16 bits logical — https://github.com/glycerine/hlc
12. hlc-gen (Rust, lib.rs) — lock-free HLC, AtomicU64, 42-bit wall + 22-bit logical, no_std — https://lib.rs/crates/hlc-gen ; https://docs.rs/hlc-gen
13. hlc (Rust, docs.rs) — wall time + logical component timestamp — https://docs.rs/hlc
14. uhlc (Rust, docs.rs) — unique HLC with u128 node id, max-delta drift guard — https://docs.rs/crate/uhlc
15. meridian-core (Rust, docs.rs) — `HybridLogicalClock { wall_ms: u64, logical: u16, node_id: u64 }`, `Ord` by field order — https://docs.rs/meridian-core
16. Lamport, "Time, Clocks, and the Ordering of Events in a Distributed System," CACM 1978 — DOI 10.1145/130704.130708 — happened-before, logical clocks, node-ID tie-break.
17. cs.stackexchange — Lamport vs vector clocks; total order via node ID tie-break; linearizability stronger than scalar clocks — https://cs.stackexchange.com/questions/101496
18. Martin Fowler, "Hybrid Clock" pattern — hybrid timestamp = system time + counter; used by MongoDB/CockroachDB — https://martinfowler.com/articles/patterns-of-distributed-systems/hybrid-clock.html
19. onenoughtone.com, "Failure Injection" — clock skew injection causes silent correctness violations — https://onenoughtone.com/learn/failure-injection/5
20. IDS prompts.txt entry 67 (verbatim) — C:/idfs/prompts.txt lines 1000-1014 — crate definition, contents, constraints.
21. C:/idfs/ids-000-aa/ skeleton — `types.rs`, `lib.rs`, `contracts/`, `invariants/`, `errors.rs`, `versioning.rs`, `Cargo.toml` — canonical types and contracts `ids-hlc` consumes.
22. plan-001.md / plan-048.md / plan-060.md — C:/idfs/plans/ — IDS plan format reference (Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources).

---

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made beyond this plan document. The blueprint above defines the HLC algorithm set, skew handling, fallback to IDS standard time, ordering guarantees, file structure, dependencies, acceptance criteria, uncertainties, and engineering decisions for Phase 067 (ids-hlc).
