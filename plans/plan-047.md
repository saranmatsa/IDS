## Phase 047 — Create the ids-testing crate: Property-based, adversarial, conformance testing.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 700-714)

```
47. Research only do not read plan.md **[Phase 047 — Create the ids-testing crate: Property-based, adversarial, conformance testing.]**

Crate: ids-testing
Purpose: Property-based, adversarial, conformance testing.
Contents: Unit tests, integration tests, property-based tests (proptest), adversarial tests, conformance tests, test vectors, determinism, fuzzing harness, state recovery tests, chaos tests, soak tests.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 047 — Create the ids-testing crate: Property-based, adversarial, conformance testing.]**

Create plan-047.md in C:/idfs/plans/ for the ids-testing crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 047 creates `ids-testing`, the IDS crate that owns property-based, adversarial, and conformance testing for the IDS system. It is a **separate crate** (per the "separate crates instead of more folders" directive), not a folder inside an existing crate. It provides: unit tests, integration tests, property-based tests via `proptest`, adversarial tests, conformance tests against the IDS specification, deterministic test vectors, a fuzzing harness (cargo-fuzz / libFuzzer structure-aware targets), state-recovery tests, chaos tests (deterministic simulation testing), and soak tests. It drives every other IDS crate through their public APIs only — it is a consumer, never a dependency of production crates. All carrier-agnostic traffic it generates rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); no networking stack may appear. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305, consumed from `ids-crypto`.

The crate's research basis: 30 parallel searches returned `proptest` (property-based testing with shrinking), `quickcheck` (QuickCheck family for Rust), `cargo-fuzz` + `arbitrary` crate (structure-aware fuzzing, libFuzzer backend), deterministic simulation testing (moonpool, madsim, fracture; FoundationDB-style single-threaded seeded simulation), Jepsen/Knossos (linearizability/history checking), conformance testing (NIST/ISO 9646-1/ETSI: IUT, test purposes, abstract test suite structure; MongoDB TLA+ conformance checking), state machine testing (proptest-stateful), determinism oracles (same seed → identical trace), soak/endurance testing (memory leaks, resource leaks over sustained load), adversarial testing (Turret; grammar-based fuzzing beyond basic functionality), differential testing (two implementations, same inputs, compare outputs), and fault injection / chaos testing.

`ids-testing` does NOT yet exist at `C:/idfs/ids-testing/`. `ids-000-aa` already declares all canonical types (`NodeId`, `RecordId`, `FragmentId`, `SnapshotId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `IdsTime`, `Carrier`, `Store`, `Transport`, …) and sealed trait contracts referenced by this crate but never redefined. Research only — no code, no source modifications, only this plan document.

---

### 2. Architecture Overview

`ids-testing` sits **outside** the production dependency graph: production crates (`ids-store`, `ids-fragment`, `ids-replication`, `ids-crypto`, `ids-carrier`, …) do NOT depend on it. It depends downward on `ids-000-aa`, `ids-carrier`, `ids-crypto`, and whatever production crates are under test. It is the single place where every IDS capability is exercised through test harnesses.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    IDS PRODUCTION CRATES (under test)                       │
│  ids-store, ids-fragment, ids-replication, ids-crypto, ids-carrier, …       │
│  Public APIs only — ids-testing is an external consumer                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                 Phase 047: ids-testing (THIS CRATE — test harness)          │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐    │
│  │ unit         │ │ integration  │ │ property     │ │ adversarial    │    │
│  │ tests        │ │ tests        │ │ (proptest)   │ │ tests          │    │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────────┘    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐    │
│  │ conformance  │ │ fuzzing      │ │ chaos        │ │ soak           │    │
│  │ tests        │ │ harness      │ │ (deterministic│ │ tests          │    │
│  │              │ │ (cargo-fuzz) │ │  simulation) │ │                │    │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────────┘    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌────────────────┐    │
│  │ state        │ │ test vectors │ │ determinism  │ │ chaos runner   │    │
│  │ recovery     │ │ (deterministic│ │ oracle       │ │ (injection)    │    │
│  │ tests        │ │  seeded)     │ │              │ │                │    │
│  └──────────────┘ └──────────────┘ └──────────────┘ └────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
                                              │ opaque Vec<u8> datagrams
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                   │
│  send_datagram(Endpoint{CarrierType ≤1024B addr, Vec<u8>}, payload: &[u8])  │
│  receive_datagram → (Endpoint, Vec<u8>)                                     │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Test data flow:**
1. A harness (unit/integration/property/adversarial/conformance) invokes production crate public APIs with generated or vector inputs.
2. Property-based harnesses generate inputs via `proptest` strategies or `arbitrary`-derived values, check invariants, shrink failures.
3. Fuzzing harnesses interpret raw bytes as structured inputs (`Arbitrary`), call target APIs, assert no panic/invariant violation.
4. Chaos/deterministic-simulation harnesses run the same seeded scenario repeatedly; same seed ⇒ identical trace (determinism oracle).
5. Conformance harnesses compare behavior against the IDS specification (assertions derived from `ids-doc` / `ids-000-aa` sealed contracts).
6. All cross-node test traffic is opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; `ids-testing` never touches a carrier directly.

---

### 3. Detailed Research Dimensions

#### 3.1 Property-based testing (proptest / quickcheck)

Property-based testing checks that certain properties hold for arbitrary auto-generated inputs; on failure it shrinks to a minimal counterexample. `proptest` (Rust, Hypothesis-inspired) defines generation and shrinking per-value (not per-type), includes `proptest!` macro, `TestRunner` with configurable `cases` (default 256), failure persistence, fork/timeout for crashing tests, and state-machine testing (`proptest-stateful` — generates operation sequences against a model, executes step by step, checks postconditions, shrinks failing sequences). `quickcheck` (QuickCheck family) generates/shrinks primitives, collections; `#[quickcheck]` attribute converts property functions to `#[test]`; `QUICKCHECK_TESTS` env var scales iterations for release cycles. Difference: proptest has superior shrinking and composition; quickcheck is simpler. **Decision for IDS:** use `proptest` as the primary property-based framework (per prompt), with `quickcheck` as a cross-check where useful.

#### 3.2 State machine / model-based testing

Stateful systems are tested by generating sequences of operations, executing them against the system under test (SUT) and an explicit reference model, checking that real state == model state after every step (lockstep), and shrinking failing sequences. Sources: `proptest-stateful` (state machine testing automates counter-example discovery and shrinking), `soroban-fuzzer` (property mode + coverage-guided mode; `setup`/`actions`/`apply`/`invariants` harness), `weighted-gss` (two-state lockstep against `BTreeMap` reference model; operation sequences as fuzz targets). Pattern: `setup` builds SUT + model; `actions` strategy generates one operation; `apply` runs it on both and compares; `invariants` must hold after each step. This is the backbone of IDS property testing for `ids-store`, `ids-crdt`, `ids-replication`, etc.

#### 3.3 Fuzzing (cargo-fuzz / libFuzzer / arbitrary)

`cargo-fuzz` is the de facto Rust fuzzing tool (libFuzzer backend; nightly toolchain). Fuzz targets use `fuzz_target!(|input| { ... })` with `&[u8]` or `Arbitrary`-derived structured input. The `arbitrary` crate derives `Arbitrary` for structs/enums (`#[derive(Arbitrary)]`) and implements it for all `std` types; structure-aware generation (not raw bytes) keeps the fuzzer deep in the system. Fuzzing harnesses parse bytes → call API → assert invariant. `honggfuzz` is an alternative (no nightly required). `global-state-detector` catches hidden global state between fuzz iterations (determinism hazard). **Decision for IDS:** `cargo-fuzz` + `arbitrary` derive, feature-gated behind `fuzzing` feature; one fuzz target per major API surface (store write/read, crypto, carrier, crdt, replication).

#### 3.4 Deterministic simulation testing / chaos

Deterministic simulation testing (DST) runs the system on logical time, one seeded RNG, simulated network + disk; single-threaded executor; no physical clocks; all I/O mocked. Same seed ⇒ same execution ⇒ reproducible bugs. Sources: `moonpool` (Deterministic simulation for Rust; chaos surfaces — network latency/clogs/partitions/bit-flips, storage corruption/EIO, attrition reboots, outages; FoundationDB-style), `madsim` (deterministic simulator runtime for distributed systems, tokio-like), `fracture` (deterministic chaos for async/Tokio; inject network/channel/task/time faults with rates; same seed = same failures), Antithesis DST docs (virtual environment, controlled entropy, state-space exploration, invariant checking), Martin Fowler DST article (quorum key-value store example; simulated clock/network/disk; single thread). Loom explores all C11 memory-model execution permutations for concurrent code. **Decision for IDS:** chaos tests run under a deterministic simulation harness (seeded, single-threaded, simulated carrier layer), so every chaos scenario is reproducible; chaos surfaces map to IDS carrier faults (drop, reorder, duplicate, delay, partition, node crash).

#### 3.5 Conformance testing

Conformance = testing whether an implementation faithfully meets a standard/specification; criteria must be specified in the spec (conformance clause/assertions). NIST: conformance program = standard + test tool + procedures; test suite = legal/illegal inputs + expected results; platform-independent; repeatable. ETSI/ISO 9646-1: test suite structure (groups/subgroups), test purposes (TP), abstract test cases, abstract test suite; implementation under test (IUT) at open interfaces; four test methods (remote/local/distributed/coordinated). MongoDB TLA+ conformance checking: implement against TLA+/PlusCal spec, then test code fidelity to spec. Differential testing: ≥2 implementations of same protocol, same inputs, compare outputs; deviations cluster by root cause. **Decision for IDS:** conformance tests assert IDS behavior against `ids-doc`/`ids-000-aa` sealed contracts (e.g., invariant `out_eq_in`, `quorum_intersection`, `no_global_master`), organized as test purposes → abstract test cases → conformance suite; differential mode runs two independent implementations (or real vs reference model) and flags mismatches.

#### 3.6 Determinism

Determinism oracle: same config + same seed ⇒ identical observable trace; `assert_deterministic` runs workload repeatedly and compares digests. `grommet_testkit::determinism::Deterministic` trait: `digest()` covers durable state + responses in order. Miri runs fully deterministic (isolated from host; fixed-seed RNG; `-Zmiri-deterministic-concurrency`). Loom permutes all concurrent executions. **Decision for IDS:** every test vector is seeded; property/chaos/conformance runs record a trace digest; a determinism test re-runs the same seed and asserts byte-identical trace + output. No wall-clock, no `HashMap` iteration order, no pointer hashing in test subjects under determinism.

#### 3.7 Adversarial testing

Adversarial testing pushes beyond basic functionality: edge cases, boundary conditions, destructive testing — malicious inputs, grammar-based fuzzing (message layout known, fields mutated), performance attacks (small input → disproportionate cost), replay/reorder/drop on channels. Sources: Turret (automated adversarial testing for message-passing systems; network emulator; grammar-based fuzz; 70 bugs/attacks found), Tofu (synthesizes drop/replay/reorder attacks on channels vs LTL specs), EVSEFuzz (protocol fuzzing with compliant + non-compliant messages incl. malicious payloads). **Decision for IDS:** adversarial tests inject malformed `Vec<u8>` datagrams, truncated frames, duplicate/reordered/replayed messages, oversized payloads, crypto-boundary inputs (bad signatures, reused nonces), and resource-exhaustion inputs; assert fail-safe (rejection, no panic, no state corruption).

#### 3.8 State recovery tests

Rollback recovery: periodically save state (checkpoint); on failure restart from checkpoint. Coordinated checkpointing avoids the domino effect (independent checkpoints risk cascading rollbacks). Sources: checkpointing/rollback-recovery literature; IDS `ids-snapshot` (Chandy-Lamport distributed snapshots), `ids-recovery` (multi-level recovery). **Decision for IDS:** state-recovery tests checkpoint a running system (via `ids-snapshot`), kill/restart a node, inject a fault, then verify recovery to the last consistent checkpoint with no lost committed ops and no phantom uncommitted ops.

#### 3.9 Soak tests

Soak/endurance/longevity testing: sustained load over extended periods to detect memory leaks, resource leaks, performance degradation, connection leaks, cache growth. Grafana: peak load held for hours/days; checks response-time degradation, memory/resource leaks, data saturation, storage depletion. Frontend soak: hundreds of iterations in one session with virtual clock advancing timers. **Decision for IDS:** soak tests run a steady workload (writes/reads/replication) for N iterations with metrics sampled; assert memory/RSS bounded, no fd leak, latency stable (no drift > X%), no queue growth.

#### 3.10 Test vectors

Deterministic test vectors: fixed input → fixed expected output, stored as JSON/binary; used for conformance and regression; every vector re-runs identically. Sources: differential testing paper (clustering deviations by root cause), MongoDB conformance (vectors from spec), IDS `ids-doc` formal models. **Decision for IDS:** `test_vectors/` holds seeded, deterministic input/output pairs for every major API; each vector has an expected digest; CI asserts exact match.

---

### Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Non-deterministic test (flaky) | Test reliability / CI trust | Seeded runs only; same seed ⇒ identical trace; determinism oracle; no wall-clock/HashMap order in subjects |
| Property test misses real bug | Coverage gap | High case counts (100k+), stateful operation sequences, shrinking to minimal counterexamples |
| Fuzzing stays shallow (invalid inputs only) | Coverage gap | Structure-aware `Arbitrary` inputs; one fuzz target per API surface; corpus seeded with valid vectors |
| Chaos scenario not reproducible | Debuggability | Deterministic simulation (seeded, single-threaded, simulated carrier); record + replay traces |
| Conformance test doesn't match spec | False assurance | Test purposes derived from `ids-doc`/`ids-000-aa` sealed contracts; assertions named per contract clause |
| Soak test too short to catch leaks | False pass | Long runs (hours/days equivalent via virtual clock + accelerated iterations); RSS/fd/queue monitored |
| Adversarial input crashes production | Availability / integrity | Fail-safe assertions: malformed input ⇒ rejection, never panic/corruption; fuzz targets assert invariants hold |
| Test harness depends on production internals | Coupling / fragility | Public API only; `ids-testing` never a dependency of production crates |
| Carrier assumptions leak into tests | Carrier-agnostic violation | All carrier traffic via `ids-carrier::Endpoint`; no `std::net`/`tokio::net`/IP/TCP/etc. in test code |
| Crypto beyond allowed list in test helpers | Policy violation | Test helpers use only `ids-crypto` primitives (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) |

The carrier is never trusted: test datagrams are signed/checksummed over payload bytes so they survive re-framing; the carrier boundary (opaque `Endpoint`) means no network stack is in the test trust base.

---

### Integration with Existing Crates

| Crate | Role | Relevance to ids-testing |
|-------|------|--------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | All types under test (`RecordId`, `FragmentId`, `SnapshotId`, `Checksum`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, `IdsTime`, `Carrier`, `Store`, `Transport`, …) consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram transport | All test traffic that crosses nodes rides `Endpoint`; tests never import a network stack |
| `ids-crypto` | Allowed primitives only | Test helpers sign/hash with ed25519/sha256/blake3/xchacha20poly1305/chacha20poly1305 |
| `ids-store` | Primary record store | Unit/integration/property/conformance targets for write/read/delete |
| `ids-crdt` | CRDT classes | Property tests for merge commutativity/associativity/idempotence; convergence under reorder/loss |
| `ids-replication` | Replication streams | Integration tests for ordered/versioned change propagation; convergence tests |
| `ids-fragment` | Record fragments/shards | Chaos tests for fragment-level faults; recovery tests |
| `ids-reclaim` (Phase 041) | Compaction / reclamation | Soak tests for space amplification; recovery tests after compaction |
| `ids-snapshot` (Phase 026) | Distributed snapshots | State-recovery tests (checkpoint → kill → restart → verify) |
| `ids-integrity` (Phase 030) | Checksums / Merkle / scrubbing | Conformance tests for end-to-end integrity |
| `ids-consistency` | Consistency profiles | Conformance tests per profile (strong/causal/eventual/…); committed-prefix tracking |
| `ids-broadcast` | Reliable/causal/total-order broadcast | Conformance tests for delivery guarantees |
| `ids-byzantine` | BFT consensus | Adversarial tests under Byzantine node behavior |
| `ids-chaos` (Phase 043) | Fault injection | Chaos tests reuse injection primitives; deterministic simulation layer |
| `ids-doc` (Phase 049) | Spec / TLA+ / formal models | Conformance test purposes derived from spec assertions |

`ids-testing` is a leaf crate: nothing in production depends on it. It is registered in the workspace but excluded from production builds (dev-only / `#[cfg(test)]` gated where feasible).

---

### Proposed File Structure (~90–1000 files in subfolders)

`C:/idfs/ids-testing/` (a separate crate, not a folder inside an existing crate):

```
ids-testing/
├── Cargo.toml                      # crate manifest (dev-dependencies only on production crates)
├── lib.rs                          # crate root, test-harness re-exports, feature gates
├── constants.rs                    # test case counts, soak durations, chaos budgets
├── errors.rs                       # TestingError (thiserror)
├── features.rs                     # feature flags (proptest, fuzzing, chaos, soak, conformance)
│
├── unit/                           # unit tests per production crate (~40 files)
│   ├── mod.rs
│   ├── store_unit.rs
│   ├── crypto_unit.rs
│   ├── carrier_unit.rs
│   ├── crdt_unit.rs
│   ├── replication_unit.rs
│   ├── fragment_unit.rs
│   ├── consistency_unit.rs
│   ├── broadcast_unit.rs
│   └── … (one file per production module under test)
│
├── integration/                    # integration tests across crates (~40 files)
│   ├── mod.rs
│   ├── store_integration.rs
│   ├── replication_integration.rs
│   ├── fragment_integration.rs
│   ├── crdt_convergence_integration.rs
│   ├── cross_crate_integration.rs
│   └── …
│
├── property/                       # property-based tests (proptest) (~50 files)
│   ├── mod.rs
│   ├── strategies.rs               # shared proptest strategies for IDS types
│   ├── proptest_config.rs          # runner config: 100k cases, fork, timeout
│   ├── store_properties.rs
│   ├── crdt_properties.rs          # merge laws: commutative, assoc, idempotent
│   ├── replication_properties.rs
│   ├── state_machine_tests.rs      # proptest-stateful: op sequences vs reference model
│   ├── consistency_properties.rs
│   ├── broadcast_properties.rs
│   └── …
│
├── adversarial/                    # adversarial tests (~40 files)
│   ├── mod.rs
│   ├── malformed_datagrams.rs      # truncated, oversized, garbage Vec<u8> inputs
│   ├── replay_reorder_drop.rs      # channel adversary: replay, reorder, drop, duplicate
│   ├── crypto_adversary.rs         # bad signatures, reused nonces, corrupted ciphertext
│   ├── resource_exhaustion.rs      # oversized payloads, deep nesting, fan-in bombs
│   ├── byzantine_inputs.rs         # Byzantine-node test inputs
│   ├── fail_safe_tests.rs          # assert: rejection, never panic/corruption
│   └── …
│
├── conformance/                    # conformance tests vs IDS spec (~60 files)
│   ├── mod.rs
│   ├── test_purposes.rs            # TP per spec assertion (ISO 9646-1 pattern)
│   ├── abstract_test_cases.rs      # concrete test cases per TP
│   ├── conformance_suite.rs        # suite runner, pass/fail reporting
│   ├── strong_consistency.rs       # conformance per consistency profile
│   ├── causal_consistency.rs
│   ├── eventual_consistency.rs
│   ├── committed_prefix.rs         # committed-prefix tracking conformance
│   ├── quorum_intersection.rs      # quorum conformance
│   ├── broadcast_guarantees.rs     # reliable/causal/total-order conformance
│   ├── integrity_conformance.rs    # end-to-end checksum/Merkle conformance
│   ├── snapshot_conformance.rs     # distributed snapshot conformance
│   ├── differential.rs             # differential testing: two implementations, same inputs
│   └── …
│
├── fuzzing/                        # fuzzing harness targets (~20 files)
│   ├── mod.rs
│   ├── fuzz_store.rs               # cargo-fuzz target: store write/read/delete
│   ├── fuzz_crypto.rs              # crypto primitive fuzzing
│   ├── fuzz_carrier.rs             # carrier datagram fuzzing
│   ├── fuzz_crdt.rs                # CRDT merge fuzzing
│   ├── fuzz_replication.rs         # replication stream fuzzing
│   ├── fuzz_fragment.rs            # fragment fuzzing
│   ├── arbitrary_impls.rs          # #[derive(Arbitrary)] gated behind fuzzing feature
│   └── …
│
├── chaos/                          # deterministic simulation / chaos tests (~40 files)
│   ├── mod.rs
│   ├── sim_runner.rs               # deterministic simulation harness (seeded, single-threaded)
│   ├── sim_network.rs              # simulated carrier layer: drop/reorder/delay/partition
│   ├── chaos_scenarios.rs          # partition, node crash, clock drift, storage faults
│   ├── determinism_oracle.rs       # same seed ⇒ identical trace assertion
│   ├── partition_healing.rs
│   ├── node_crash_reboot.rs
│   ├── storage_fault_injection.rs
│   ├── clock_drift_tests.rs
│   └── …
│
├── state_recovery/                 # state recovery tests (~20 files)
│   ├── mod.rs
│   ├── checkpoint_recovery.rs      # checkpoint → kill → restart → verify
│   ├── crash_recovery.rs           # crash mid-write, recover, verify no lost committed ops
│   ├── snapshot_restore.rs         # restore from Chandy-Lamport snapshot
│   ├── rollback_tests.rs           # rollback to last consistent state
│   └── …
│
├── soak/                           # soak / endurance tests (~20 files)
│   ├── mod.rs
│   ├── soak_runner.rs              # sustained load runner with metrics sampling
│   ├── memory_leak_tests.rs        # RSS bounded over N iterations
│   ├── fd_leak_tests.rs            # no file-descriptor growth
│   ├── queue_growth_tests.rs       # no unbounded queue growth
│   ├── latency_stability.rs        # latency stable over long run
│   └── …
│
├── determinism/                    # determinism tests (~15 files)
│   ├── mod.rs
│   ├── trace_digest.rs             # digest of durable state + responses in order
│   ├── determinism_oracle.rs       # re-run same seed, assert byte-identical trace
│   ├── seeded_property_runs.rs     # property tests with fixed seeds
│   └── …
│
├── test_vectors/                   # deterministic test vectors (~40 files)
│   ├── mod.rs
│   ├── store_vectors.json
│   ├── crypto_vectors.json
│   ├── crdt_vectors.json
│   ├── replication_vectors.json
│   ├── conformance_vectors.json
│   └── vector_runner.rs            # run each vector, assert exact output match
│
├── contracts/                      # sealed trait contracts for test harnesses (~15 files)
│   ├── mod.rs
│   ├── test_harness_contract.rs
│   ├── property_contract.rs
│   ├── fuzz_target_contract.rs
│   ├── chaos_contract.rs
│   └── …
│
├── fixtures/                       # shared test fixtures (~20 files)
│   ├── mod.rs
│   ├── node_fixture.rs
│   ├── carrier_fixture.rs          # mock CarrierType endpoint for tests
│   ├── store_fixture.rs
│   ├── crypto_fixture.rs
│   └── …
│
├── benches/                        # test-harness benchmarks (~10 files)
│   ├── mod.rs
│   ├── property_bench.rs
│   ├── fuzz_coverage_bench.rs
│   └── …
│
├── docs/                           # documentation (~15 files)
│   ├── architecture.md
│   ├── writing_property_tests.md
│   ├── writing_adversarial_tests.md
│   ├── conformance_suite_guide.md
│   ├── chaos_testing_guide.md
│   ├── soak_testing_guide.md
│   └── …
│
└── specs/                          # normative test specifications (~10 files)
    ├── property_test_spec.md
    ├── adversarial_test_spec.md
    ├── conformance_test_spec.md
    └── determinism_spec.md
```

Total: ~90–1000 files across `src/`, `unit/`, `integration/`, `property/`, `adversarial/`, `conformance/`, `fuzzing/`, `chaos/`, `state_recovery/`, `soak/`, `determinism/`, `test_vectors/`, `contracts/`, `fixtures/`, `benches/`, `docs/`, `specs/` — one file per concern, organized by subsystem. No stubs; every file holds test-grade code with documentation.

---

### Key Algorithms & Data Structures

- **Property test runner:** `proptest::TestRunner` with `Config { cases: 100_000, fork: true, timeout, max_shrink_iters }`; `proptest!` macro per property; custom strategies via `prop_compose!` for IDS types (`RecordId`, `OpRef`, `OrderingPos`, `Endpoint`, `CarrierType`, …).
- **Stateful model test:** `setup()` builds SUT + reference model; `actions` strategy generates one operation (write/read/delete/replicate/…); `apply()` runs op on both, compares real state == model state, checks invariants after every step; on failure, shrink the operation sequence to a minimal counterexample (proptest-stateful pattern).
- **Fuzz target:** `fuzz_target!(|data: &[u8]| { let input = Arbitrary::arbitrary(&mut Unstructured::new(data)); sut.api(input); assert_invariant!(); })` — `arbitrary` derive for structured input; libFuzzer coverage feedback.
- **Determinism oracle:** `digest() = hash(durable_state || responses_in_order)`; run same seed twice, assert `digest1 == digest2`; record trace for replay.
- **Deterministic simulation chaos:** single-threaded executor; simulated carrier layer injects drop/reorder/delay/duplicate/partition/node-crash per seeded RNG; same seed ⇒ identical execution; invariants checked at every simulated step.
- **Conformance suite:** test purposes (TP) derived from `ids-doc`/`ids-000-aa` sealed contracts; abstract test cases execute against public API; pass/fail per TP; differential mode compares two implementations on identical inputs, clusters deviations by root cause.
- **Soak:** sustained workload for N iterations; sample RSS, fd count, queue depth, latency each iteration; assert boundedness and stability (no drift > threshold).
- **State recovery:** checkpoint (via `ids-snapshot`), kill node, restart from checkpoint, replay journal, assert committed prefix intact, no phantom ops, no lost committed ops.
- **Adversarial input generator:** malformed/truncated/oversized `Vec<u8>` datagrams, replay/reorder/drop/duplicate channel behavior, bad signatures, reused nonces, corrupted ciphertext; assert fail-safe (rejection, no panic, state intact).
- **Test vector:** `{ seed, input_bytes, expected_output_bytes, expected_digest }`; runner re-executes and asserts exact match; stored under `test_vectors/`.

---

### Dependencies

- `ids-000-aa` — canonical types + sealed trait contracts + invariants + errors (consumed only).
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`), `Carrier` trait (consumed only).
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (consumed only).
- Production crates under test: `ids-store`, `ids-crdt`, `ids-replication`, `ids-fragment`, `ids-consistency`, `ids-broadcast`, `ids-snapshot`, `ids-integrity`, `ids-byzantine`, `ids-chaos`, … (consumed only via public API).
- `proptest` — property-based testing (dev-dependency).
- `quickcheck` — cross-check property tests (optional dev-dependency).
- `arbitrary` (derive feature) — structure-aware fuzzing inputs (dev-dependency, feature-gated).
- `libfuzzer-sys` / `cargo-fuzz` — fuzzing harnesses (dev-dependency, feature-gated).
- `serde`, `serde_json` — test vector serialization.
- `thiserror` — error types.
- `criterion` — test-harness benchmarks (optional).

No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. No crypto beyond the allowed list. `ids-testing` is never a dependency of any production crate.

---

### Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| AC-047.1 | Property tests: merge laws (commutative, associative, idempotent) hold | proptest, ≥100,000 iterations per law, pass/fail |
| AC-047.2 | Stateful model tests: real state == reference model after every op | proptest-stateful, shrink on mismatch |
| AC-047.3 | Conformance: every `ids-doc`/`ids-000-aa` sealed-contract assertion has ≥1 conformance test | conformance suite, 100% assertion coverage |
| AC-047.4 | Adversarial: 100% of malformed/truncated/oversized/replayed datagram inputs rejected safely (no panic, no corruption) | adversarial test suite, pass/fail |
| AC-047.5 | Fuzzing: ≥1 fuzz target per major API surface (store, crypto, carrier, crdt, replication) | cargo-fuzz target list, coverage measured |
| AC-047.6 | Chaos: every chaos scenario deterministic — same seed ⇒ identical trace | determinism oracle, repeated runs |
| AC-047.7 | State recovery: after checkpoint→kill→restart, committed prefix intact, no lost/phantom ops | state-recovery tests, pass/fail |
| AC-047.8 | Soak: sustained load N iterations, RSS bounded, no fd/queue growth, latency stable | soak tests, metric thresholds |
| AC-047.9 | Determinism: seeded property/chaos/conformance runs produce byte-identical traces | determinism oracle, repeated runs |
| AC-047.10 | Test vectors: every stored vector reproduces exact expected output | vector runner, exact match |
| AC-047.11 | Carrier agnosticism: zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS in crate | grep/audit, zero hits |
| AC-047.12 | Crypto allowlist: only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 in crypto surface | dependency audit, zero other crypto |
| AC-047.13 | Leaf crate: `ids-testing` is not a dependency of any production crate | Cargo dependency graph, zero reverse deps |

---

### Remaining Uncertainties

1. **Proptest case count tuning:** 100,000 iterations is a starting point; expensive properties (stateful model, convergence) may need fewer cases with longer per-case timeouts; to be validated at implementation.
2. **Deterministic simulation harness implementation:** IDS has no confirmed workspace-wide deterministic simulation runtime; the chaos layer must simulate carrier/network/disk on logical time — design of this harness is provisional.
3. **Fuzzing toolchain on Windows/MSVC:** cargo-fuzz/libFuzzer requires nightly and toolchain support; MSVC linker complications for libFuzzer `main` noted in research — fallback to honggfuzz or in-process fuzz targets may be needed.
4. **Conformance test source of truth:** `ids-doc` (Phase 049) formal models/TLA+ specs may not yet exist; conformance test purposes must be derived from `ids-000-aa` sealed contracts until then.
5. **Differential testing implementation:** running two independent implementations requires a reference implementation to exist; may defer to property+model testing until a second implementation is available.
6. **Soak duration equivalence:** virtual-clock acceleration (iterations ≈ wall time) mapping to real-world duration is unquantified; thresholds (RSS bound, latency drift) are placeholders pending measurement.
7. **Cross-replica chaos under partitions:** deterministic simulation of multi-node partitions needs a simulated membership/failure detector; integration with `ids-failure-detector` semantics undefined.
8. **Test vector format:** JSON vs binary vs canonical serialization; digest algorithm (BLAKE3 vs SHA-256) choice for vector integrity.

---

### Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| Separate crate `ids-testing` (not a folder inside an existing crate) | User directive: separate crates instead of more folders; keeps test code out of production dependency graph |
| `proptest` as primary property framework (per prompt) | Superior shrinking/composition vs quickcheck; state-machine testing via proptest-stateful |
| `cargo-fuzz` + `arbitrary` derive for fuzzing | De facto Rust fuzzing toolchain; structure-aware inputs keep fuzzer deep in the system |
| Deterministic simulation for chaos | Same seed ⇒ identical execution ⇒ reproducible bugs (FoundationDB/Antithesis pattern) |
| Conformance test purposes from `ids-doc`/`ids-000-aa` contracts | Conformance requires spec-specified assertions; ISO 9646-1 TP/abstract-test-case structure |
| Public API only; `ids-testing` never a production dependency | Prevents test code from coupling to/infecting production crates |
| All carrier traffic via `ids-carrier::Endpoint`; no network stack | Carrier-agnostic absolute rule (no exceptions) |
| Allowed crypto only (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) | IDS crypto policy |
| Seeded runs + determinism oracle | Test reliability and reproducibility; CI can re-run failures bit-for-bit |
| One file per concern (~90–1000 files) | Matches IDS plan format and "separate crates instead of more folders" applied intra-crate |

---

### Sources

[^1]: proptest — property-based testing for Rust (QuickCheck family, Hypothesis-inspired): https://docs.rs/proptest/latest/proptest ; https://github.com/proptest-rs/proptest
[^2]: proptest `TestRunner` config (cases, fork, timeout, shrink): https://docs.rs/proptest/latest/proptest/test_runner/struct.Config.html
[^3]: proptest state machine testing: https://proptest-rs.github.io/proptest/proptest/state-machine.html
[^4]: proptest-stateful (stateful property tests, Readyset): https://github.com/readysettech/proptest-stateful
[^5]: quickcheck — QuickCheck for Rust (with shrinking): https://github.com/jakoschiko/quickcheck ; https://docs.rs/crate/quickcheck/latest
[^6]: cargo-fuzz / libFuzzer fuzzing for Rust: https://rust-fuzz.github.io/book/cargo-fuzz/ ; https://github.com/rust-fuzz/cargo-fuzz
[^7]: arbitrary crate — structure-aware fuzzing inputs, `#[derive(Arbitrary)]`: https://github.com/rust-fuzz/arbitrary ; https://docs.rs/arbitrary/latest/arbitrary
[^8]: honggfuzz Rust fuzzing: https://docs.rs/honggfuzz/latest/src/honggfuzz/lib.rs.html
[^9]: moonpool — deterministic simulation testing for distributed systems in Rust: https://github.com/pierrez/moonpool ; https://docs.rs/moonpool/latest/moonpool
[^10]: madsim — Magical Deterministic Simulator for distributed systems in Rust: https://github.com/madsim-rs/madsim
[^11]: fracture — deterministic chaos testing for async Rust (drop-in for Tokio): https://docs.rs/fracture/latest
[^12]: Deterministic Simulation Testing (Martin Fowler / Tickloom): https://martinfowler.com/articles/deterministic-simulation-testing.html
[^13]: Antithesis deterministic simulation testing docs: https://antithesis.com/docs/resources/deterministic_simulation_testing
[^14]: loom — deterministic exploration of C11 concurrent executions: https://docs.rs/loom/latest/loom
[^15]: Jepsen — distributed systems safety testing; Knossos linearizability checker: https://github.com/jepsen-io/knossos
[^16]: Jepsen testing introduction (Couchbase; linearizable consistency checker): https://couchbase.com/blog/introduction-to-jepsen-testing-at-couchbase
[^17]: Conformance testing (NIST): https://www.nist.gov/itl/ssd/information-systems-group/conformance-testing
[^18]: ISO/IEC 9646-1 OSI conformance testing methodology and framework (ETSI): https://docbox.etsi.org/mts/mts/10-promotionalmaterial/mbs-20111118/testing/Conformance/conformance.htm ; https://link.springer.com/article/10.1007/bfb0026973
[^19]: Conformance Test Specifications — test purposes, abstract test method, test suite structure (ETSI/ISO 9646): https://docbox.etsi.org/mts/mts/10-promotionalmaterial/mbs-20111118/testing/Conformance/conformance.htm
[^20]: Conformance Testing (ETSI Centre for Testing and Interoperability): https://portal.etsi.org/Services/Centre-for-Testing-Interoperability/ETSI-Approach/Conformance
[^21]: QA Framework: Test Guidelines — test suite structure (W3C): https://w3.org/QA/WG/2002/12/qaframe-test-20021205.html
[^22]: Conformance Checking at MongoDB (TLA+ conformance): https://mongodb.com/company/blog/engineering/conformance-checking-at-mongodb-testing-our-code-matches-our-tla-specs
[^23]: Differential testing (Bundle Protocol v7; same inputs, compare outputs): https://indico.esa.int/event/571/attachments/7210/13591/Differential%20Testing%20of%20Bundle%20Protocol%20v7.pdf
[^24]: Determinism oracle / digest (grommet_testkit): https://docs.rs/grommet-testkit/latest/grommet_testkit/determinism/trait.Deterministic.html
[^25]: Miri — deterministic UB detection for Rust: https://github.com/rust-lang/rust/tree/main/src/tools/miri
[^26]: Adversarial testing of distributed systems and network protocols (Turret; SecDev '16): https://doi.org/10.1109/secdev.2016.038
[^27]: Automated Adversarial Testing of Unmodified Wireless Routing Implementations (Turret-W; TNET): https://dl.acm.org/doi/10.1109/TNET.2016.2520474
[^28]: Tofu — Automated Channel Fault Analysis (drop/replay/reorder attacks vs LTL): https://arxiv.org/pdf/2605.01721.pdf
[^29]: EVSEFuzz — protocol implementation fuzzing with compliant + malicious messages: https://arxiv.org/pdf/2403.14342
[^30]: Soak testing — endurance/longevity testing for memory/resource leaks: https://grafana.com/blog/soak-testing ; https://katalon.com/resources-center/blog/soak-testing
[^31]: Testing a database for race conditions with QuickCheck (state machine models, shrinking): https://dl.acm.org/doi/10.1145/2034654.2034667
[^32]: Soroban fuzzer — stateful fuzzing with property + coverage-guided modes: https://docs.rs/crate/soroban-fuzzer/latest
[^33]: weighted-gss — property-based state-machine tests vs reference model (lockstep): https://github.com/IsaacBreen/weighted-gss/blob/main/docs/validation.md
[^34]: global-state-detector — hidden global state between fuzz iterations (determinism hazard): https://github.com/srlabs/global-state-detector-rs
[^35]: Fracture chaos testing (deterministic, seed-reproducible): https://docs.rs/fracture/latest/fracture/chaos/index.html

**Research status:** Complete. Ready for implementation phase. No code written, no files created, no repository changes made. The blueprint above defines all architecture, research dimensions, threat model, integration points, file structure, key algorithms, dependencies, acceptance criteria, uncertainties, and engineering decisions for Phase 047 — ids-testing.
