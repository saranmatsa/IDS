//! # Protocol Invariants — `ids-000-aa/src/invariants/`
//!
//! This module declares every protocol invariant as a sealed trait. Each invariant
//! trait is `pub trait Name: private::Sealed {}` — downstream crates can use them as
//! trait bounds and call their methods, but cannot implement or weaken them. Only
//! `ids-000-aa` and its designated implementation crates may `impl` an invariant trait.
//!
//! The 40 invariants here are drawn from IDS.md, IDS-A.md, IDS-B.md, CAPABILITIES.md,
//! DIFFERENCES.md, NOVEL.md, EDGE-CASES.md, and the edge-case position / response docs.
//! Each invariant enforces one edge-case hardening rule that the IDS architecture
//! requires to hold at all times, across every node, H-unit, region, and dataset.
//!
//! ## Design principles
//!
//! 1. **Sealed**: `private::Sealed` supertrait blocks external impls. Downstream crates
//!    reference `ids_000_aa::invariants::BinaryCompletion` as a bound but never implement it.
//! 2. **Compile-time enforceable**: every invariant method has a `Result<_, InvariantError>`
//!    contract so violations surface as type-level rejections, not runtime surprises.
//! 3. **One definition each**: each invariant trait is defined exactly once in this crate.
//! 4. **Carrier-agnostic**: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC,
//!    HTTP, or DNS anywhere in this module.
//! 5. **Versioned**: every invariant maps to one bit in `EdgeCaseBits` (`src/versioning.rs`),
//!    so enabling/disabling an edge case is a one-line change here plus one downstream impl.
//!
//! ## Module index (40 invariants)
//!
//! - `binary_completion` — 100% done or failed; no partial success terminal state
//! - `dead_node_identity` — dead node never returns with old identity; fresh ID assigned
//! - `idle_rotation` — RNI rotates only while node idle
//! - `no_global_master` — dataset-scoped ordering only; no permanent global ordering master
//! - `out_eq_in` — byte accounting boundary: bytes-out == bytes-in + acked
//! - `coordinated_deletion` — all holders delete simultaneously within milliseconds or not at all
//! - `fencing_stonith` — STONITH/fencing before any failover; split-brain prevention
//! - `quorum_intersection` — read quorum ∩ write quorum ≠ ∅
//! - `erasure_threshold` — any k of n chunks reconstructs; tolerates n-k failures
//! - `correlated_failure` — replicas never share power/facility/network/operator domain
//! - `hunit_compensation` — H-unit failure compensation through another H-unit
//! - `regional_autonomy` — regional clusters operate independently for local operations
//! - `hybrid_redundancy` — full replication + erasure coding coexist per dataset
//! - `operation_lifecycle` — received→ordered→executing→committed→synchronized→verified→failed
//! - `durability_ack` — durability-aware acknowledgement; no overstating uncompleted replication
//! - `ordered_read_visibility` — reads respect committed position prefix
//! - `node_roster` — client receives fixed node roster at join; stable
//! - `region_affinity` — client does not abruptly change regions; near-to-far routing
//! - `identity_rotation` — stable internal identity + rotating RNI; mapping protected
//! - `ids_standard_time` — proprietary time basis for ordering/commit, not wall-clock
//! - `daily_integrity` — daily integrity checks; "fishy" state detection
//! - `db_retirement` — temporary DB retirement during RNI rotation; returns online after window
//! - `read_fallback` — automatic fallback to next closest eligible replica
//! - `topology_diversity` — different locations, H-units, regions, failure domains per copy
//! - `write_fanout` — ~30 databases written simultaneously for durability
//! - `custody_graph` — data custody and responsibility graph
//! - `checksum_boundary` — end-to-end checksum verification boundary
//! - `replica_freshness` — replica freshness validation for reads
//! - `commit_prefix` — committed-prefix tracking for ordered reads
//! - `version_vector` — vector clock for causality tracking
//! - `membership_epoch` — membership change epoch tracking
//! - `placement_policy` — placement policy enforcement
//! - `replication_lag` — replication lag observability
//! - `recovery_scope` — multi-level recovery scope classification
//! - `data_lifecycle` — data lifecycle state enforcement
//! - `snapshot_consistency` — snapshot consistency verification
//! - `compute_to_data` — compute-to-data execution boundary
//! - `hot_cold_tier` — hot/cold storage tier transition rules
//! - `mobility_handoff` — dataset mobility across regions handoff
//! - `federation_trust` — multi-owner federation trust boundary
//!
//! Every file below follows the same shape:
//! - `pub trait Name: private::Sealed { ... }` — the sealed invariant trait
//! - `mod private { pub trait Sealed {} }` — the sealed marker
//! - `impl Name for () { ... }` — reference no-op impl for compile checks
//! - `#[cfg(test)] mod tests` — comprehensive test coverage (≥800 test LOC per file)

pub mod binary_completion;
pub mod dead_node_identity;
pub mod idle_rotation;
pub mod no_global_master;
pub mod out_eq_in;
pub mod coordinated_deletion;
pub mod fencing_stonith;
pub mod quorum_intersection;
pub mod erasure_threshold;
pub mod correlated_failure;
pub mod hunit_compensation;
pub mod regional_autonomy;
pub mod hybrid_redundancy;
pub mod operation_lifecycle;
pub mod durability_ack;
pub mod ordered_read_visibility;
pub mod node_roster;
pub mod region_affinity;
pub mod identity_rotation;
pub mod ids_standard_time;
pub mod daily_integrity;
pub mod db_retirement;
pub mod read_fallback;
pub mod topology_diversity;
pub mod write_fanout;
pub mod custody_graph;
pub mod checksum_boundary;
pub mod replica_freshness;
pub mod commit_prefix;
pub mod version_vector;
pub mod membership_epoch;
pub mod placement_policy;
pub mod replication_lag;
pub mod recovery_scope;
pub mod data_lifecycle;
pub mod snapshot_consistency;
pub mod compute_to_data;
pub mod hot_cold_tier;
pub mod mobility_handoff;
pub mod federation_trust;
