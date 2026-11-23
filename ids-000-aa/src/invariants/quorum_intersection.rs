//! # Quorum Intersection Invariant
//!
//! This invariant enforces the fundamental quorum requirement: read quorum
//! intersection with write quorum must be non-empty (read ∩ write ≠ ∅),
//! guaranteeing that any read observes at least one node from the latest
//! write quorum, preventing stale reads and split-brain consistency violations.
//!
//! The quorum intersection subsystem provides:
//! - Read and write quorum selection with configurable sizes
//! - Quorum intersection validation (read ∩ write ≠ ∅)
//! - Flexible Paxos quorum intersection (ReadQuorum + WriteQuorum > N)
//! - Quorum capacity and fault tolerance calculation
//! - Quorum domain management with dynamic membership
//! - Quorum membership tracking across epochs
//! - Quorum validator with intersection proofs
//! - Quorum weight assignment for weighted voting
//! - Quorum availability checking under node failures
//! - Quorum intersection audit trail for consistency verification
//! - Deterministic quorum selection based on node ordering
//! - Cross-epoch quorum compatibility checking

use crate::types::*;
use crate::errors::InvariantError;
use crate::versioning::EdgeCaseBits;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait declaration
///////////////////////////////////////////////////////////////////////////////

/// Invariant: read quorum ∩ write quorum ≠ ∅ for all quorum operations.
///
/// Downstream crates use this as a trait bound but cannot implement it.
/// Only `ids-000-aa` and its designated implementation crates may `impl`.
pub trait QuorumIntersection: private::Sealed {
    /// Select a read quorum of size `r` from the given replicas.
    fn read_quorum(&self, replicas: &[NodeId], r: u32) -> Result<Vec<NodeId>, QuorumError>;

    /// Select a write quorum of size `w` from the given replicas.
    fn write_quorum(&self, replicas: &[NodeId], w: u32) -> Result<Vec<NodeId>, QuorumError>;

    /// Check that read quorum ∩ write quorum is non-empty.
    fn intersection_non_empty(&self, read: &[NodeId], write: &[NodeId]) -> bool;

    /// Validate that a read quorum and write quorum intersect properly.
    fn validate_intersection(
        &self,
        read: &[NodeId],
        write: &[NodeId],
    ) -> Result<IntersectionProof, QuorumError>;

    /// Calculate the minimum write quorum size given N replicas and read quorum R.
    fn min_write_quorum(&self, n: u32, r: u32) -> u32;

    /// Calculate the minimum read quorum size given N replicas and write quorum W.
    fn min_read_quorum(&self, n: u32, w: u32) -> u32;

    /// Check whether a quorum configuration tolerates F failures.
    fn tolerates_failures(&self, n: u32, r: u32, w: u32, f: u32) -> bool;

    /// Return the fault tolerance (max failures tolerated) for a quorum config.
    fn fault_tolerance(&self, n: u32, r: u32, w: u32) -> u32;

    /// Select quorums using Flexible Paxos rules (R + W > N).
    fn flexible_paxos_quorums(
        &self,
        replicas: &[NodeId],
        r: u32,
        w: u32,
    ) -> Result<FlexiblePaxosQuorums, QuorumError>;

    /// Check whether two quorum configurations are compatible across epochs.
    fn epoch_compatible(
        &self,
        config_a: &QuorumConfig,
        config_b: &QuorumConfig,
    ) -> bool;

    /// Track quorum membership for a node within a domain.
    fn track_membership(
        &mut self,
        domain: OrderingDomain,
        node: NodeId,
        weight: u32,
    ) -> Result<(), QuorumError>;

    /// Remove quorum membership for a node.
    fn untrack_membership(
        &mut self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<(), QuorumError>;

    /// Return the tracked members of a quorum domain.
    fn domain_members(&self, domain: OrderingDomain) -> Vec<NodeId>;

    /// Return the weight assigned to a node in a domain.
    fn node_weight(&self, domain: OrderingDomain, node: NodeId) -> u32;

    /// Calculate weighted quorum intersection.
    fn weighted_intersection(
        &self,
        read: &[(NodeId, u32)],
        write: &[(NodeId, u32)],
    ) -> bool;

    /// Check quorum availability given a set of failed nodes.
    fn availability_check(
        &self,
        replicas: &[NodeId],
        r: u32,
        w: u32,
        failed: &[NodeId],
    ) -> AvailabilityResult;

    /// Return the quorum intersection audit trail.
    fn intersection_audit(&self) -> Vec<IntersectionAuditEntry>;

    /// Record a quorum intersection validation event.
    fn record_intersection_event(
        &mut self,
        entry: IntersectionAuditEntry,
    );

    /// Return the total number of quorum calculations performed.
    fn calculation_count(&self) -> u64;

    /// Return the number of failed intersection validations.
    fn failure_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// Sealed marker module
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for () {}

///////////////////////////////////////////////////////////////////////////////
// Reference no-op impl for compile-time checks
///////////////////////////////////////////////////////////////////////////////

impl QuorumIntersection for () {
    fn read_quorum(&self, _replicas: &[NodeId], _r: u32) -> Result<Vec<NodeId>, QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn write_quorum(&self, _replicas: &[NodeId], _w: u32) -> Result<Vec<NodeId>, QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn intersection_non_empty(&self, _read: &[NodeId], _write: &[NodeId]) -> bool {
        false
    }

    fn validate_intersection(
        &self,
        _read: &[NodeId],
        _write: &[NodeId],
    ) -> Result<IntersectionProof, QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn min_write_quorum(&self, _n: u32, _r: u32) -> u32 {
        0
    }

    fn min_read_quorum(&self, _n: u32, _w: u32) -> u32 {
        0
    }

    fn tolerates_failures(&self, _n: u32, _r: u32, _w: u32, _f: u32) -> bool {
        false
    }

    fn fault_tolerance(&self, _n: u32, _r: u32, _w: u32) -> u32 {
        0
    }

    fn flexible_paxos_quorums(
        &self,
        _replicas: &[NodeId],
        _r: u32,
        _w: u32,
    ) -> Result<FlexiblePaxosQuorums, QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn epoch_compatible(&self, _a: &QuorumConfig, _b: &QuorumConfig) -> bool {
        false
    }

    fn track_membership(
        &mut self,
        _domain: OrderingDomain,
        _node: NodeId,
        _weight: u32,
    ) -> Result<(), QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn untrack_membership(
        &mut self,
        _domain: OrderingDomain,
        _node: NodeId,
    ) -> Result<(), QuorumError> {
        Err(QuorumError::NoQuorumEngine)
    }

    fn domain_members(&self, _domain: OrderingDomain) -> Vec<NodeId> {
        Vec::new()
    }

    fn node_weight(&self, _domain: OrderingDomain, _node: NodeId) -> u32 {
        0
    }

    fn weighted_intersection(
        &self,
        _read: &[(NodeId, u32)],
        _write: &[(NodeId, u32)],
    ) -> bool {
        false
    }

    fn availability_check(
        &self,
        _replicas: &[NodeId],
        _r: u32,
        _w: u32,
        _failed: &[NodeId],
    ) -> AvailabilityResult {
        AvailabilityResult::Unavailable
    }

    fn intersection_audit(&self) -> Vec<IntersectionAuditEntry> {
        Vec::new()
    }

    fn record_intersection_event(&mut self, _entry: IntersectionAuditEntry) {}

    fn calculation_count(&self) -> u64 {
        0
    }

    fn failure_count(&self) -> u64 {
        0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Production types and subsystems
///////////////////////////////////////////////////////////////////////////////

/// A quorum selection: ordered set of node IDs forming a quorum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumSelection {
    pub members: Vec<NodeId>,
    pub quorum_size: u32,
    pub total_replicas: u32,
    pub selection_method: SelectionMethod,
}

/// Method used to select quorum members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionMethod {
    /// Deterministic selection by node ID ordering.
    DeterministicById,
    /// Randomized selection (seeded).
    RandomizedSeeded,
    /// Weighted selection by node weight.
    WeightedByWeight,
    /// Geographic distribution-aware selection.
    GeographicAware,
}

/// Proof that a read-write quorum intersection is valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersectionProof {
    pub read_quorum: Vec<NodeId>,
    pub write_quorum: Vec<NodeId>,
    pub intersection: Vec<NodeId>,
    pub intersection_size: usize,
    pub total_replicas: u32,
    pub satisfies_flexible_paxos: bool,
}

/// Flexible Paxos quorum pair (R + W > N).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexiblePaxosQuorums {
    pub read_quorum: Vec<NodeId>,
    pub write_quorum: Vec<NodeId>,
    pub n: u32,
    pub r: u32,
    pub w: u32,
    pub r_plus_w: u32,
}

/// Quorum configuration for a domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumConfig {
    pub domain: OrderingDomain,
    pub replicas: Vec<NodeId>,
    pub read_quorum_size: u32,
    pub write_quorum_size: u32,
    pub epoch: Epoch,
}

/// Audit entry for quorum intersection validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersectionAuditEntry {
    pub entry_id: AuditEntryId,
    pub domain: OrderingDomain,
    pub read_quorum: Vec<NodeId>,
    pub write_quorum: Vec<NodeId>,
    pub intersection_size: usize,
    pub valid: bool,
    pub timestamp: TimestampMillis,
}

/// Unique identifier for an audit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AuditEntryId(pub u64);

/// Epoch for quorum configuration versioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u64);

/// Availability result for a quorum under failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityResult {
    /// Both read and write quorums can be formed.
    Available,
    /// Only read quorum can be formed.
    ReadOnly,
    /// Only write quorum can be formed.
    WriteOnly,
    /// Neither quorum can be formed.
    Unavailable,
}

/// Errors returned by quorum operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumError {
    /// No quorum engine is configured.
    NoQuorumEngine,
    /// Not enough replicas to form the requested quorum.
    InsufficientReplicas { available: u32, required: u32 },
    /// Quorum size is zero (invalid).
    ZeroQuorumSize,
    /// Quorum size exceeds replica count.
    QuorumExceedsReplicas { quorum: u32, replicas: u32 },
    /// Read and write quorums do not intersect.
    NoIntersection { read_size: usize, write_size: usize },
    /// Flexible Paxos condition violated (R + W <= N).
    FlexiblePaxosViolated { r: u32, w: u32, n: u32 },
    /// Domain not found in quorum tracker.
    DomainNotFound { domain: OrderingDomain },
    /// Node not a member of the quorum domain.
    NodeNotMember { node: NodeId, domain: OrderingDomain },
    /// Node already tracked in the domain.
    NodeAlreadyMember { node: NodeId, domain: OrderingDomain },
    /// Internal quorum subsystem error.
    Internal { message: String },
}

/// Production implementation of quorum intersection.
///
/// Maintains quorum configurations per domain, tracks membership with weights,
/// performs intersection validation, and records audit entries.
pub struct QuorumIntersectionEngine {
    /// Quorum configurations per domain.
    configs: HashMap<OrderingDomain, QuorumConfig>,
    /// Per-domain membership weights.
    domain_weights: HashMap<(OrderingDomain, NodeId), u32>,
    /// Total calculation count.
    calculation_count: u64,
    /// Total failed validation count.
    failure_count: u64,
    /// Intersection audit trail.
    audit_trail: Vec<IntersectionAuditEntry>,
    /// Next audit entry ID.
    next_audit_id: u64,
    /// Whether flexible Paxos is enforced by default.
    enforce_flexible_paxos: bool,
}

impl QuorumIntersectionEngine {
    /// Create a new quorum intersection engine.
    pub fn new() -> Self {
        QuorumIntersectionEngine {
            configs: HashMap::new(),
            domain_weights: HashMap::new(),
            calculation_count: 0,
            failure_count: 0,
            audit_trail: Vec::new(),
            next_audit_id: 0,
            enforce_flexible_paxos: true,
        }
    }

    /// Create an engine with flexible Paxos enforcement disabled.
    pub fn without_flexible_paxos() -> Self {
        let mut e = Self::new();
        e.enforce_flexible_paxos = false;
        e
    }

    /// Register a quorum configuration for a domain.
    pub fn register_config(&mut self, config: QuorumConfig) -> Result<(), QuorumError> {
        if config.read_quorum_size == 0 || config.write_quorum_size == 0 {
            return Err(QuorumError::ZeroQuorumSize);
        }
        if config.read_quorum_size > config.replicas.len() as u32
            || config.write_quorum_size > config.replicas.len() as u32
        {
            return Err(QuorumError::QuorumExceedsReplicas {
                quorum: config.read_quorum_size.max(config.write_quorum_size),
                replicas: config.replicas.len() as u32,
            });
        }

        // Initialize weights for all replicas to 1.
        for &replica in &config.replicas {
            self.domain_weights
                .entry((config.domain, replica))
                .or_insert(1);
        }

        self.configs.insert(config.domain, config);
        Ok(())
    }

    /// Select a read quorum deterministically by node ID ordering.
    pub fn read_quorum(
        &mut self,
        replicas: &[NodeId],
        r: u32,
    ) -> Result<Vec<NodeId>, QuorumError> {
        self.calculation_count += 1;

        if r == 0 {
            self.failure_count += 1;
            return Err(QuorumError::ZeroQuorumSize);
        }
        if r > replicas.len() as u32 {
            self.failure_count += 1;
            return Err(QuorumError::QuorumExceedsReplicas {
                quorum: r,
                replicas: replicas.len() as u32,
            });
        }

        let mut sorted: Vec<NodeId> = replicas.to_vec();
        sorted.sort();

        let members = sorted.into_iter().take(r as usize).collect();

        let selection = QuorumSelection {
            members: members.clone(),
            quorum_size: r,
            total_replicas: replicas.len() as u32,
            selection_method: SelectionMethod::DeterministicById,
        };

        let _ = selection;

        Ok(members)
    }

    /// Select a write quorum deterministically by node ID ordering.
    pub fn write_quorum(
        &mut self,
        replicas: &[NodeId],
        w: u32,
    ) -> Result<Vec<NodeId>, QuorumError> {
        self.calculation_count += 1;

        if w == 0 {
            self.failure_count += 1;
            return Err(QuorumError::ZeroQuorumSize);
        }
        if w > replicas.len() as u32 {
            self.failure_count += 1;
            return Err(QuorumError::QuorumExceedsReplicas {
                quorum: w,
                replicas: replicas.len() as u32,
            });
        }

        let mut sorted: Vec<NodeId> = replicas.to_vec();
        sorted.sort();

        // Write quorum selects from the end to maximize intersection with read.
        let members = sorted.into_iter().rev().take(w as usize).collect();

        Ok(members)
    }

    /// Check whether read and write quorums intersect.
    pub fn intersection_non_empty(&self, read: &[NodeId], write: &[NodeId]) -> bool {
        let read_set: HashSet<NodeId> = read.iter().cloned().collect();
        write.iter().any(|n| read_set.contains(n))
    }

    /// Validate intersection and produce a proof.
    pub fn validate_intersection(
        &mut self,
        read: &[NodeId],
        write: &[NodeId],
    ) -> Result<IntersectionProof, QuorumError> {
        self.calculation_count += 1;

        let read_set: HashSet<NodeId> = read.iter().cloned().collect();
        let intersection: Vec<NodeId> = write
            .iter()
            .filter(|n| read_set.contains(*n))
            .cloned()
            .collect();

        let valid = !intersection.is_empty();

        if !valid {
            self.failure_count += 1;
            return Err(QuorumError::NoIntersection {
                read_size: read.len(),
                write_size: write.len(),
            });
        }

        let n = read.len().max(write.len()) as u32;
        let r = read.len() as u32;
        let w = write.len() as u32;
        let satisfies_fp = r + w > n;

        let proof = IntersectionProof {
            read_quorum: read.to_vec(),
            write_quorum: write.to_vec(),
            intersection: intersection.clone(),
            intersection_size: intersection.len(),
            total_replicas: n,
            satisfies_flexible_paxos: satisfies_fp,
        };

        self.record_intersection_event(IntersectionAuditEntry {
            entry_id: AuditEntryId(self.next_audit_id),
            domain: OrderingDomain(0),
            read_quorum: read.to_vec(),
            write_quorum: write.to_vec(),
            intersection_size: intersection.len(),
            valid,
            timestamp: TimestampMillis(0),
        });
        self.next_audit_id += 1;

        Ok(proof)
    }

    /// Calculate minimum write quorum: W > N - R (so R + W > N).
    pub fn min_write_quorum(&self, n: u32, r: u32) -> u32 {
        if r == 0 {
            return n;
        }
        let needed = n.saturating_sub(r) + 1;
        needed.min(n)
    }

    /// Calculate minimum read quorum: R > N - W (so R + W > N).
    pub fn min_read_quorum(&self, n: u32, w: u32) -> u32 {
        if w == 0 {
            return n;
        }
        let needed = n.saturating_sub(w) + 1;
        needed.min(n)
    }

    /// Check whether the quorum config tolerates F failures.
    pub fn tolerates_failures(&self, n: u32, r: u32, w: u32, f: u32) -> bool {
        let remaining = n.saturating_sub(f);
        remaining >= r && remaining >= w && r + w > n
    }

    /// Return maximum failures tolerated: min(N-R, N-W) while maintaining R+W>N.
    pub fn fault_tolerance(&self, n: u32, r: u32, w: u32) -> u32 {
        if r + w <= n {
            return 0;
        }
        let ft_read = n.saturating_sub(r);
        let ft_write = n.saturating_sub(w);
        ft_read.min(ft_write)
    }

    /// Select Flexible Paxos quorums (R + W > N).
    pub fn flexible_paxos_quorums(
        &mut self,
        replicas: &[NodeId],
        r: u32,
        w: u32,
    ) -> Result<FlexiblePaxosQuorums, QuorumError> {
        self.calculation_count += 1;

        let n = replicas.len() as u32;

        if r + w <= n && self.enforce_flexible_paxos {
            self.failure_count += 1;
            return Err(QuorumError::FlexiblePaxosViolated { r, w, n });
        }

        if r > n || w > n {
            self.failure_count += 1;
            return Err(QuorumError::QuorumExceedsReplicas {
                quorum: r.max(w),
                replicas: n,
            });
        }

        let read = self.read_quorum(replicas, r)?;
        let write = self.write_quorum(replicas, w)?;

        Ok(FlexiblePaxosQuorums {
            read_quorum: read,
            write_quorum: write,
            n,
            r,
            w,
            r_plus_w: r + w,
        })
    }

    /// Check epoch compatibility: same domain, compatible quorum sizes.
    pub fn epoch_compatible(&self, a: &QuorumConfig, b: &QuorumConfig) -> bool {
        if a.domain != b.domain {
            return false;
        }
        // Quorums must still intersect across epochs.
        let r_plus_w_a = a.read_quorum_size + a.write_quorum_size;
        let r_plus_w_b = b.read_quorum_size + b.write_quorum_size;
        let n_a = a.replicas.len() as u32;
        let n_b = b.replicas.len() as u32;

        r_plus_w_a > n_a && r_plus_w_b > n_b
    }

    /// Track membership for a node in a domain.
    pub fn track_membership(
        &mut self,
        domain: OrderingDomain,
        node: NodeId,
        weight: u32,
    ) -> Result<(), QuorumError> {
        let key = (domain, node);
        if self.domain_weights.contains_key(&key) {
            return Err(QuorumError::NodeAlreadyMember { node, domain });
        }
        self.domain_weights.insert(key, weight);
        Ok(())
    }

    /// Remove membership for a node from a domain.
    pub fn untrack_membership(
        &mut self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<(), QuorumError> {
        let key = (domain, node);
        if self.domain_weights.remove(&key).is_none() {
            return Err(QuorumError::NodeNotMember { node, domain });
        }
        Ok(())
    }

    /// Return tracked members of a domain.
    pub fn domain_members(&self, domain: OrderingDomain) -> Vec<NodeId> {
        let mut members: Vec<NodeId> = self
            .domain_weights
            .iter()
            .filter(|((d, _), _)| *d == domain)
            .map(|(_, n)| *n)
            .collect();
        members.sort();
        members
    }

    /// Return the weight of a node in a domain.
    pub fn node_weight(&self, domain: OrderingDomain, node: NodeId) -> u32 {
        self.domain_weights.get(&(domain, node)).copied().unwrap_or(0)
    }

    /// Weighted quorum intersection: sum of weights in intersection > 0.
    pub fn weighted_intersection(
        &self,
        read: &[(NodeId, u32)],
        write: &[(NodeId, u32)],
    ) -> bool {
        let read_map: HashMap<NodeId, u32> = read.iter().cloned().collect();
        let write_map: HashMap<NodeId, u32> = write.iter().cloned().collect();

        let mut intersection_weight: u32 = 0;
        for (node, weight) in &read_map {
            if let Some(&w_weight) = write_map.get(node) {
                intersection_weight += (*weight).min(w_weight);
            }
        }

        intersection_weight > 0
    }

    /// Check quorum availability under node failures.
    pub fn availability_check(
        &self,
        replicas: &[NodeId],
        r: u32,
        w: u32,
        failed: &[NodeId],
    ) -> AvailabilityResult {
        let failed_set: HashSet<NodeId> = failed.iter().cloned().collect();
        let available: Vec<NodeId> = replicas
            .iter()
            .filter(|n| !failed_set.contains(n))
            .cloned()
            .collect();

        let can_read = available.len() as u32 >= r;
        let can_write = available.len() as u32 >= w;

        match (can_read, can_write) {
            (true, true) => AvailabilityResult::Available,
            (true, false) => AvailabilityResult::ReadOnly,
            (false, true) => AvailabilityResult::WriteOnly,
            (false, false) => AvailabilityResult::Unavailable,
        }
    }

    /// Return the intersection audit trail.
    pub fn intersection_audit(&self) -> Vec<IntersectionAuditEntry> {
        self.audit_trail.clone()
    }

    /// Record a quorum intersection event.
    pub fn record_intersection_event(&mut self, entry: IntersectionAuditEntry) {
        self.audit_trail.push(entry);
    }

    /// Return total calculation count.
    pub fn calculation_count(&self) -> u64 {
        self.calculation_count
    }

    /// Return total failure count.
    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }

    /// Enable or disable flexible Paxos enforcement.
    pub fn set_enforce_flexible_paxos(&mut self, enabled: bool) {
        self.enforce_flexible_paxos = enabled;
    }

    /// Return whether flexible Paxos is enforced.
    pub fn is_flexible_paxos_enforced(&self) -> bool {
        self.enforce_flexible_paxos
    }
}

impl Default for QuorumIntersectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumValidator — standalone intersection validation
///////////////////////////////////////////////////////////////////////////////

/// Validates quorum intersections independently of an engine.
pub struct QuorumValidator;

impl QuorumValidator {
    /// Validate that two quorums intersect.
    pub fn validate(read: &[NodeId], write: &[NodeId]) -> bool {
        let read_set: HashSet<NodeId> = read.iter().cloned().collect();
        write.iter().any(|n| read_set.contains(n))
    }

    /// Validate flexible Paxos: R + W > N.
    pub fn validate_flexible_paxos(r: u32, w: u32, n: u32) -> bool {
        r + w > n
    }

    /// Find all intersecting nodes between two quorums.
    pub fn intersection(read: &[NodeId], write: &[NodeId]) -> Vec<NodeId> {
        let read_set: HashSet<NodeId> = read.iter().cloned().collect();
        let mut result: Vec<NodeId> = write.iter().filter(|n| read_set.contains(*n)).cloned().collect();
        result.sort();
        result.dedup();
        result
    }

    /// Calculate intersection size.
    pub fn intersection_size(read: &[NodeId], write: &[NodeId]) -> usize {
        Self::intersection(read, write).len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumWeightCalculator — weight assignment algorithms
///////////////////////////////////////////////////////////////////////////////

/// A weight assignment strategy for quorum members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightStrategy {
    /// Equal weight for all members (1 each).
    Equal,
    /// Weight based on node capacity.
    CapacityBased,
    /// Weight based on node reliability score.
    ReliabilityBased,
    /// Weight based on geographic distance from quorum center.
    DistanceBased,
}

/// Calculates quorum member weights using various strategies.
pub struct QuorumWeightCalculator {
    /// Default weight for equal strategy.
    default_weight: u32,
    /// Per-node capacity scores.
    capacity_scores: HashMap<NodeId, u32>,
    /// Per-node reliability scores (0-100).
    reliability_scores: HashMap<NodeId, u32>,
    /// Per-node distances from quorum center.
    distances: HashMap<NodeId, u32>,
}

impl QuorumWeightCalculator {
    /// Create a new weight calculator.
    pub fn new() -> Self {
        QuorumWeightCalculator {
            default_weight: 1,
            capacity_scores: HashMap::new(),
            reliability_scores: HashMap::new(),
            distances: HashMap::new(),
        }
    }

    /// Set the default weight.
    pub fn set_default_weight(&mut self, weight: u32) {
        self.default_weight = weight;
    }

    /// Set capacity score for a node.
    pub fn set_capacity(&mut self, node: NodeId, score: u32) {
        self.capacity_scores.insert(node, score);
    }

    /// Set reliability score for a node.
    pub fn set_reliability(&mut self, node: NodeId, score: u32) {
        self.reliability_scores.insert(node, score);
    }

    /// Set distance for a node.
    pub fn set_distance(&mut self, node: NodeId, distance: u32) {
        self.distances.insert(node, distance);
    }

    /// Calculate weight for a node using the given strategy.
    pub fn calculate_weight(&self, node: NodeId, strategy: WeightStrategy) -> u32 {
        match strategy {
            WeightStrategy::Equal => self.default_weight,
            WeightStrategy::CapacityBased => self
                .capacity_scores
                .get(&node)
                .copied()
                .unwrap_or(self.default_weight),
            WeightStrategy::ReliabilityBased => self
                .reliability_scores
                .get(&node)
                .copied()
                .unwrap_or(self.default_weight),
            WeightStrategy::DistanceBased => {
                let dist = self.distances.get(&node).copied().unwrap_or(0);
                // Closer nodes get higher weight; invert distance.
                if dist == 0 {
                    self.default_weight
                } else {
                    1000u32.saturating_div(dist).max(1)
                }
            }
        }
    }

    /// Calculate weights for a list of nodes.
    pub fn calculate_weights(
        &self,
        nodes: &[NodeId],
        strategy: WeightStrategy,
    ) -> Vec<(NodeId, u32)> {
        nodes
            .iter()
            .map(|&n| (n, self.calculate_weight(n, strategy)))
            .collect()
    }

    /// Return total weight across nodes.
    pub fn total_weight(&self, nodes: &[(NodeId, u32)]) -> u32 {
        nodes.iter().map(|(_, w)| *w).sum()
    }

    /// Return the node with the highest weight.
    pub fn max_weight_node(&self, nodes: &[(NodeId, u32)]) -> Option<NodeId> {
        nodes.iter().max_by_key(|(_, w)| *w).map(|(n, _)| *n)
    }

    /// Return the default weight.
    pub fn default_weight(&self) -> u32 {
        self.default_weight
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumAvailabilityTracker — availability over time
///////////////////////////////////////////////////////////////////////////////

/// A snapshot of quorum availability at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilitySnapshot {
    pub timestamp: TimestampMillis,
    pub available_replicas: u32,
    pub total_replicas: u32,
    pub can_form_read: bool,
    pub can_form_write: bool,
    pub read_quorum_size: u32,
    pub write_quorum_size: u32,
}

/// Tracks quorum availability over time windows.
pub struct QuorumAvailabilityTracker {
    /// Snapshots ordered by timestamp.
    snapshots: Vec<AvailabilitySnapshot>,
    /// Read quorum size.
    read_quorum_size: u32,
    /// Write quorum size.
    write_quorum_size: u32,
    /// Maximum snapshots to retain.
    max_snapshots: usize,
    /// Total availability checks performed.
    check_count: u64,
    /// Total times unavailable.
    unavailable_count: u64,
}

impl QuorumAvailabilityTracker {
    /// Create a new availability tracker.
    pub fn new(read_quorum_size: u32, write_quorum_size: u32) -> Self {
        QuorumAvailabilityTracker {
            snapshots: Vec::new(),
            read_quorum_size,
            write_quorum_size,
            max_snapshots: 1000,
            check_count: 0,
            unavailable_count: 0,
        }
    }

    /// Record an availability snapshot.
    pub fn record_snapshot(
        &mut self,
        timestamp: TimestampMillis,
        available_replicas: u32,
        total_replicas: u32,
    ) {
        let can_form_read = available_replicas >= self.read_quorum_size;
        let can_form_write = available_replicas >= self.write_quorum_size;

        let snapshot = AvailabilitySnapshot {
            timestamp,
            available_replicas,
            total_replicas,
            can_form_read,
            can_form_write,
            read_quorum_size: self.read_quorum_size,
            write_quorum_size: self.write_quorum_size,
        };

        self.snapshots.push(snapshot);
        self.check_count += 1;

        if !can_form_read || !can_form_write {
            self.unavailable_count += 1;
        }

        // Retain only the most recent snapshots.
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Return the most recent snapshot.
    pub fn latest_snapshot(&self) -> Option<&AvailabilitySnapshot> {
        self.snapshots.last()
    }

    /// Return availability percentage over all snapshots.
    pub fn availability_percentage(&self) -> f64 {
        if self.check_count == 0 {
            return 100.0;
        }
        let available_count = self.check_count - self.unavailable_count;
        (available_count as f64 / self.check_count as f64) * 100.0
    }

    /// Return the count of unavailable snapshots.
    pub fn unavailable_count(&self) -> u64 {
        self.unavailable_count
    }

    /// Return the total check count.
    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    /// Return snapshots where the system was unavailable.
    pub fn unavailable_snapshots(&self) -> Vec<&AvailabilitySnapshot> {
        self.snapshots
            .iter()
            .filter(|s| !s.can_form_read || !s.can_form_write)
            .collect()
    }

    /// Return the minimum available replicas observed.
    pub fn min_available_replicas(&self) -> Option<u32> {
        self.snapshots
            .iter()
            .map(|s| s.available_replicas)
            .min()
    }

    /// Return the maximum available replicas observed.
    pub fn max_available_replicas(&self) -> Option<u32> {
        self.snapshots.iter().map(|s| s.available_replicas).max()
    }

    /// Set the maximum number of snapshots to retain.
    pub fn set_max_snapshots(&mut self, max: usize) {
        self.max_snapshots = max;
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Check whether the system is currently available for both read and write.
    pub fn is_currently_available(&self) -> bool {
        self.latest_snapshot()
            .map(|s| s.can_form_read && s.can_form_write)
            .unwrap_or(false)
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumEpochManager — manages quorum configuration epochs
///////////////////////////////////////////////////////////////////////////////

/// A quorum configuration at a specific epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochQuorumConfig {
    pub epoch: Epoch,
    pub replicas: Vec<NodeId>,
    pub read_quorum_size: u32,
    pub write_quorum_size: u32,
    pub activated_at: TimestampMillis,
    pub retired_at: Option<TimestampMillis>,
}

/// Manages quorum configurations across epochs with safe transitions.
pub struct QuorumEpochManager {
    /// Configs ordered by epoch.
    configs: BTreeMap<Epoch, EpochQuorumConfig>,
    /// Current active epoch.
    current_epoch: Option<Epoch>,
    /// Epoch transition history.
    transitions: Vec<EpochTransition>,
    /// Whether epoch transitions require quorum intersection validation.
    require_intersection_validation: bool,
}

/// A record of an epoch transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochTransition {
    pub from_epoch: Option<Epoch>,
    pub to_epoch: Epoch,
    pub transitioned_at: TimestampMillis,
    pub valid: bool,
}

impl QuorumEpochManager {
    /// Create a new epoch manager.
    pub fn new() -> Self {
        QuorumEpochManager {
            configs: BTreeMap::new(),
            current_epoch: None,
            transitions: Vec::new(),
            require_intersection_validation: true,
        }
    }

    /// Activate a quorum config at a new epoch.
    pub fn activate_epoch(
        &mut self,
        config: EpochQuorumConfig,
    ) -> Result<(), QuorumError> {
        let from_epoch = self.current_epoch;

        // Validate intersection with previous epoch if required.
        if self.require_intersection_validation {
            if let Some(prev_epoch) = from_epoch {
                if let Some(prev_config) = self.configs.get(&prev_epoch) {
                    let prev_total = prev_config.replicas.len() as u32;
                    let r_plus_w = config.read_quorum_size + config.write_quorum_size;
                    if r_plus_w <= prev_total {
                        let transition = EpochTransition {
                            from_epoch,
                            to_epoch: config.epoch,
                            transitioned_at: config.activated_at,
                            valid: false,
                        };
                        self.transitions.push(transition);
                        return Err(QuorumError::FlexiblePaxosViolated {
                            r: config.read_quorum_size,
                            w: config.write_quorum_size,
                            n: prev_total,
                        });
                    }
                }
            }
        }

        self.configs.insert(config.epoch, config.clone());
        self.current_epoch = Some(config.epoch);

        let transition = EpochTransition {
            from_epoch,
            to_epoch: config.epoch,
            transitioned_at: config.activated_at,
            valid: true,
        };
        self.transitions.push(transition);

        Ok(())
    }

    /// Return the current active epoch.
    pub fn current_epoch(&self) -> Option<Epoch> {
        self.current_epoch
    }

    /// Return the config for the current epoch.
    pub fn current_config(&self) -> Option<&EpochQuorumConfig> {
        self.current_epoch
            .and_then(|e| self.configs.get(&e))
    }

    /// Return the config for a specific epoch.
    pub fn config_at_epoch(&self, epoch: Epoch) -> Option<&EpochQuorumConfig> {
        self.configs.get(&epoch)
    }

    /// Return all epochs in order.
    pub fn epochs(&self) -> Vec<Epoch> {
        self.configs.keys().copied().collect()
    }

    /// Retire an epoch (mark its config as retired).
    pub fn retire_epoch(
        &mut self,
        epoch: Epoch,
        retired_at: TimestampMillis,
    ) -> Result<(), QuorumError> {
        let config = self
            .configs
            .get_mut(&epoch)
            .ok_or_else(|| QuorumError::DomainNotFound {
                domain: OrderingDomain(0),
            })?;
        config.retired_at = Some(retired_at);
        Ok(())
    }

    /// Return the number of configured epochs.
    pub fn epoch_count(&self) -> usize {
        self.configs.len()
    }

    /// Return all epoch transitions.
    pub fn transitions(&self) -> Vec<EpochTransition> {
        self.transitions.clone()
    }

    /// Return the count of valid transitions.
    pub fn valid_transition_count(&self) -> u64 {
        self.transitions
            .iter()
            .filter(|t| t.valid)
            .count() as u64
    }

    /// Return the count of invalid transitions.
    pub fn invalid_transition_count(&self) -> u64 {
        self.transitions
            .iter()
            .filter(|t| !t.valid)
            .count() as u64
    }

    /// Enable or disable intersection validation on epoch transitions.
    pub fn set_require_intersection_validation(&mut self, required: bool) {
        self.require_intersection_validation = required;
    }

    /// Check whether intersection validation is required.
    pub fn is_intersection_validation_required(&self) -> bool {
        self.require_intersection_validation
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumReplicaSelector — advanced replica selection strategies
///////////////////////////////////////////////////////////////////////////////

/// A replica selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaSelectionStrategy {
    /// Select nodes with lowest IDs first.
    LowestIdFirst,
    /// Select nodes with highest IDs first.
    HighestIdFirst,
    /// Select nodes with highest weight first.
    HighestWeightFirst,
    /// Select nodes to maximize failure domain diversity.
    DiversityFirst,
    /// Round-robin selection.
    RoundRobin,
}

/// Selects replicas for quorum formation using various strategies.
pub struct QuorumReplicaSelector {
    /// Default selection strategy.
    default_strategy: ReplicaSelectionStrategy,
    /// Per-node weights for weighted selection.
    weights: HashMap<NodeId, u32>,
    /// Per-node failure domain assignments.
    failure_domains: HashMap<NodeId, FailureDomainRef>,
    /// Round-robin cursor.
    round_robin_cursor: usize,
}

/// Reference to a failure domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FailureDomainRef(pub u32);

impl QuorumReplicaSelector {
    /// Create a new replica selector.
    pub fn new() -> Self {
        QuorumReplicaSelector {
            default_strategy: ReplicaSelectionStrategy::LowestIdFirst,
            weights: HashMap::new(),
            failure_domains: HashMap::new(),
            round_robin_cursor: 0,
        }
    }

    /// Set the default selection strategy.
    pub fn set_strategy(&mut self, strategy: ReplicaSelectionStrategy) {
        self.default_strategy = strategy;
    }

    /// Set the weight for a node.
    pub fn set_weight(&mut self, node: NodeId, weight: u32) {
        self.weights.insert(node, weight);
    }

    /// Assign a node to a failure domain.
    pub fn assign_failure_domain(&mut self, node: NodeId, domain: FailureDomainRef) {
        self.failure_domains.insert(node, domain);
    }

    /// Select replicas using the configured strategy.
    pub fn select(
        &mut self,
        candidates: &[NodeId],
        count: u32,
    ) -> Result<Vec<NodeId>, QuorumError> {
        if count == 0 {
            return Err(QuorumError::ZeroQuorumSize);
        }
        if count > candidates.len() as u32 {
            return Err(QuorumError::QuorumExceedsReplicas {
                quorum: count,
                replicas: candidates.len() as u32,
            });
        }

        match self.default_strategy {
            ReplicaSelectionStrategy::LowestIdFirst => {
                let mut sorted: Vec<NodeId> = candidates.to_vec();
                sorted.sort();
                Ok(sorted.into_iter().take(count as usize).collect())
            }
            ReplicaSelectionStrategy::HighestIdFirst => {
                let mut sorted: Vec<NodeId> = candidates.to_vec();
                sorted.sort_by(|a, b| b.cmp(a));
                Ok(sorted.into_iter().take(count as usize).collect())
            }
            ReplicaSelectionStrategy::HighestWeightFirst => {
                let mut weighted: Vec<(NodeId, u32)> = candidates
                    .iter()
                    .map(|&n| (n, self.weights.get(&n).copied().unwrap_or(1)))
                    .collect();
                weighted.sort_by(|a, b| b.1.cmp(&a.1));
                Ok(weighted.into_iter().map(|(n, _)| n).take(count as usize).collect())
            }
            ReplicaSelectionStrategy::DiversityFirst => {
                self.select_diverse(candidates, count)
            }
            ReplicaSelectionStrategy::RoundRobin => {
                self.select_round_robin(candidates, count)
            }
        }
    }

    /// Select replicas maximizing failure domain diversity.
    fn select_diverse(
        &mut self,
        candidates: &[NodeId],
        count: u32,
    ) -> Result<Vec<NodeId>, QuorumError> {
        let mut selected: Vec<NodeId> = Vec::new();
        let mut used_domains: HashSet<FailureDomainRef> = HashSet::new();

        // First pass: pick one node per failure domain.
        for &candidate in candidates {
            if selected.len() >= count as usize {
                break;
            }
            let domain = self.failure_domains.get(&candidate).copied();
            if let Some(domain) = domain {
                if !used_domains.contains(&domain) {
                    used_domains.insert(domain);
                    selected.push(candidate);
                }
            }
        }

        // Second pass: fill remaining slots with any unused candidates.
        for &candidate in candidates {
            if selected.len() >= count as usize {
                break;
            }
            if !selected.contains(&candidate) {
                selected.push(candidate);
            }
        }

        Ok(selected)
    }

    /// Select replicas using round-robin from the cursor.
    fn select_round_robin(
        &mut self,
        candidates: &[NodeId],
        count: u32,
    ) -> Result<Vec<NodeId>, QuorumError> {
        let mut selected: Vec<NodeId> = Vec::new();
        let len = candidates.len();

        for i in 0..count as usize {
            let idx = (self.round_robin_cursor + i) % len;
            selected.push(candidates[idx]);
        }

        self.round_robin_cursor = (self.round_robin_cursor + count as usize) % len;
        Ok(selected)
    }

    /// Return the default strategy.
    pub fn default_strategy(&self) -> ReplicaSelectionStrategy {
        self.default_strategy
    }

    /// Return the round-robin cursor position.
    pub fn round_robin_cursor(&self) -> usize {
        self.round_robin_cursor
    }

    /// Return the weight for a node.
    pub fn node_weight(&self, node: NodeId) -> u32 {
        self.weights.get(&node).copied().unwrap_or(1)
    }

    /// Return the failure domain for a node.
    pub fn node_failure_domain(&self, node: NodeId) -> Option<FailureDomainRef> {
        self.failure_domains.get(&node).copied()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumFailureDomain — failure-domain-aware quorum selection
///////////////////////////////////////////////////////////////////////////////

/// A failure domain grouping nodes that share a failure mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDomain {
    pub domain_id: FailureDomainRef,
    pub name: String,
    pub nodes: Vec<NodeId>,
    pub max_failures: u32,
}

/// Tracks failure domains and ensures quorum diversity across them.
pub struct QuorumFailureDomainTracker {
    /// Domains by ID.
    domains: HashMap<FailureDomainRef, FailureDomain>,
    /// Node to domain mapping.
    node_to_domain: HashMap<NodeId, FailureDomainRef>,
    /// Maximum allowed failures per domain for quorum validity.
    max_failures_per_domain: u32,
}

impl QuorumFailureDomainTracker {
    /// Create a new failure domain tracker.
    pub fn new() -> Self {
        QuorumFailureDomainTracker {
            domains: HashMap::new(),
            node_to_domain: HashMap::new(),
            max_failures_per_domain: 1,
        }
    }

    /// Register a failure domain.
    pub fn register_domain(
        &mut self,
        domain: FailureDomain,
    ) -> Result<(), QuorumError> {
        if self.domains.contains_key(&domain.domain_id) {
            return Err(QuorumError::Internal {
                message: "domain already registered".to_string(),
            });
        }
        for &node in &domain.nodes {
            self.node_to_domain.insert(node, domain.domain_id);
        }
        self.domains.insert(domain.domain_id, domain);
        Ok(())
    }

    /// Return the domain for a node.
    pub fn domain_for_node(&self, node: NodeId) -> Option<FailureDomainRef> {
        self.node_to_domain.get(&node).copied()
    }

    /// Return all nodes in a domain.
    pub fn nodes_in_domain(&self, domain: FailureDomainRef) -> Option<Vec<NodeId>> {
        self.domains.get(&domain).map(|d| d.nodes.clone())
    }

    /// Check whether a quorum selection respects failure domain limits.
    pub fn respects_domain_limits(
        &self,
        selection: &[NodeId],
    ) -> bool {
        let mut domain_counts: HashMap<FailureDomainRef, u32> = HashMap::new();
        for &node in selection {
            if let Some(domain) = self.node_to_domain.get(&node) {
                *domain_counts.entry(*domain).or_insert(0) += 1;
            }
        }
        domain_counts
            .values()
            .all(|&count| count <= self.max_failures_per_domain)
    }

    /// Return the number of domains.
    pub fn domain_count(&self) -> usize {
        self.domains.len()
    }

    /// Return the number of nodes tracked.
    pub fn tracked_node_count(&self) -> usize {
        self.node_to_domain.len()
    }

    /// Set the max failures allowed per domain.
    pub fn set_max_failures_per_domain(&mut self, max: u32) {
        self.max_failures_per_domain = max;
    }

    /// Return the max failures per domain.
    pub fn max_failures_per_domain(&self) -> u32 {
        self.max_failures_per_domain
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumIntersectionCache — cached intersection results
///////////////////////////////////////////////////////////////////////////////

/// A cached quorum intersection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedIntersection {
    pub read_hash: u64,
    pub write_hash: u64,
    pub intersection_size: usize,
    pub valid: bool,
}

/// Caches quorum intersection validation results for performance.
pub struct QuorumIntersectionCache {
    /// Cache entries keyed by (read_hash, write_hash).
    cache: HashMap<(u64, u64), CachedIntersection>,
    /// Total cache hits.
    hits: u64,
    /// Total cache misses.
    misses: u64,
    /// Maximum cache entries.
    max_entries: usize,
}

impl QuorumIntersectionCache {
    /// Create a new cache.
    pub fn new() -> Self {
        QuorumIntersectionCache {
            cache: HashMap::new(),
            hits: 0,
            misses: 0,
            max_entries: 10_000,
        }
    }

    /// Compute a simple hash for a quorum slice.
    fn quorum_hash(quorum: &[NodeId]) -> u64 {
        let mut hash: u64 = 0;
        for node in quorum {
            hash = hash.wrapping_mul(31).wrapping_add(node.0[0] as u64);
        }
        hash
    }

    /// Look up or compute an intersection result.
    pub fn lookup_or_compute(
        &mut self,
        read: &[NodeId],
        write: &[NodeId],
        compute: impl FnOnce() -> (usize, bool),
    ) -> (usize, bool) {
        let read_hash = Self::quorum_hash(read);
        let write_hash = Self::quorum_hash(write);
        let key = (read_hash, write_hash);

        if let Some(cached) = self.cache.get(&key) {
            self.hits += 1;
            return (cached.intersection_size, cached.valid);
        }

        self.misses += 1;
        let (size, valid) = compute();

        if self.cache.len() >= self.max_entries {
            // Evict a random entry (simple: remove first).
            if let Some(first_key) = self.cache.keys().next().copied() {
                self.cache.remove(&first_key);
            }
        }

        self.cache.insert(
            key,
            CachedIntersection {
                read_hash,
                write_hash,
                intersection_size: size,
                valid,
            },
        );

        (size, valid)
    }

    /// Return cache hit count.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Return cache miss count.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Return cache hit ratio.
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    /// Return the number of cached entries.
    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumConsistencyChecker — consistency verification for quorum reads
///////////////////////////////////////////////////////////////////////////////

/// A consistency check result for a quorum read operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyCheckResult {
    pub read_quorum: Vec<NodeId>,
    pub write_quorum: Vec<NodeId>,
    pub intersection_valid: bool,
    pub observed_version: u64,
    pub expected_version: u64,
    pub consistent: bool,
    pub staleness_ms: u64,
}

/// Verifies quorum read consistency against write quorum history.
pub struct QuorumConsistencyChecker {
    /// Write quorum history ordered by version.
    write_history: Vec<WriteQuorumRecord>,
    /// Last verified version.
    last_verified_version: u64,
    /// Total consistency checks performed.
    check_count: u64,
    /// Total consistency violations detected.
    violation_count: u64,
    /// Maximum history entries to retain.
    max_history: usize,
}

/// A recorded write quorum operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteQuorumRecord {
    pub version: u64,
    pub write_quorum: Vec<NodeId>,
    pub timestamp: TimestampMillis,
    pub committed: bool,
}

impl QuorumConsistencyChecker {
    /// Create a new consistency checker.
    pub fn new() -> Self {
        QuorumConsistencyChecker {
            write_history: Vec::new(),
            last_verified_version: 0,
            check_count: 0,
            violation_count: 0,
            max_history: 500,
        }
    }

    /// Record a completed write quorum operation.
    pub fn record_write(
        &mut self,
        version: u64,
        write_quorum: Vec<NodeId>,
        timestamp: TimestampMillis,
        committed: bool,
    ) {
        let record = WriteQuorumRecord {
            version,
            write_quorum,
            timestamp,
            committed,
        };
        self.write_history.push(record);
        self.last_verified_version = self.last_verified_version.max(version);

        // Retain only recent history.
        if self.write_history.len() > self.max_history {
            self.write_history.remove(0);
        }
    }

    /// Check whether a read quorum is consistent with the latest write.
    pub fn check_consistency(
        &mut self,
        read_quorum: &[NodeId],
        observed_version: u64,
    ) -> ConsistencyCheckResult {
        self.check_count += 1;

        // Find the latest committed write quorum.
        let latest_write = self
            .write_history
            .iter()
            .filter(|w| w.committed)
            .max_by_key(|w| w.version);

        let expected_version = latest_write.map(|w| w.version).unwrap_or(0);

        // Check intersection with the latest write quorum.
        let intersection_valid = latest_write
            .map(|w| {
                let read_set: HashSet<NodeId> = read_quorum.iter().cloned().collect();
                w.write_quorum
                    .iter()
                    .any(|n| read_set.contains(n))
            })
            .unwrap_or(true);

        let consistent = intersection_valid && observed_version >= expected_version;

        let staleness_ms = if observed_version < expected_version {
            latest_write
                .map(|w| w.timestamp)
                .unwrap_or(TimestampMillis(0))
                .0
                .saturating_sub(observed_version * 1000)
        } else {
            0
        };

        if !consistent {
            self.violation_count += 1;
        }

        ConsistencyCheckResult {
            read_quorum: read_quorum.to_vec(),
            write_quorum: latest_write
                .map(|w| w.write_quorum.clone())
                .unwrap_or_default(),
            intersection_valid,
            observed_version,
            expected_version,
            consistent,
            staleness_ms,
        }
    }

    /// Return the last verified version.
    pub fn last_verified_version(&self) -> u64 {
        self.last_verified_version
    }

    /// Return total check count.
    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    /// Return total violation count.
    pub fn violation_count(&self) -> u64 {
        self.violation_count
    }

    /// Return consistency violation rate (0.0 to 1.0).
    pub fn violation_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 0.0;
        }
        self.violation_count as f64 / self.check_count as f64
    }

    /// Return all consistency violations from history.
    pub fn violations(&self) -> Vec<&WriteQuorumRecord> {
        self.write_history.iter().filter(|w| !w.committed).collect()
    }

    /// Return the write history length.
    pub fn history_length(&self) -> usize {
        self.write_history.len()
    }

    /// Set the maximum history entries.
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
        while self.write_history.len() > self.max_history {
            self.write_history.remove(0);
        }
    }

    /// Clear the write history.
    pub fn clear_history(&mut self) {
        self.write_history.clear();
        self.last_verified_version = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// QuorumReconfigurationManager — dynamic quorum reconfiguration
///////////////////////////////////////////////////////////////////////////////

/// A quorum reconfiguration operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumReconfiguration {
    pub config_id: ReconfigId,
    pub domain: OrderingDomain,
    pub old_replicas: Vec<NodeId>,
    pub new_replicas: Vec<NodeId>,
    pub old_read_quorum: u32,
    pub old_write_quorum: u32,
    pub new_read_quorum: u32,
    pub new_write_quorum: u32,
    pub status: ReconfigStatus,
    pub created_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a reconfiguration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReconfigId(pub u64);

/// Status of a quorum reconfiguration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconfigStatus {
    /// Proposed but not yet approved.
    Proposed,
    /// Approved and in progress.
    InProgress,
    /// Completed successfully.
    Completed,
    /// Failed during execution.
    Failed,
    /// Rolled back to previous configuration.
    RolledBack,
}

/// Manages dynamic quorum reconfiguration with safety guarantees.
pub struct QuorumReconfigurationManager {
    /// Reconfigurations by ID.
    reconfigs: HashMap<ReconfigId, QuorumReconfiguration>,
    /// Current configuration per domain.
    current_configs: HashMap<OrderingDomain, DomainQuorumConfig>,
    /// Next reconfig ID.
    next_id: u64,
    /// Whether reconfigurations require intersection validation.
    require_intersection_validation: bool,
    /// Maximum concurrent reconfigurations.
    max_concurrent: usize,
}

/// Current quorum configuration for a domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainQuorumConfig {
    pub domain: OrderingDomain,
    pub replicas: Vec<NodeId>,
    pub read_quorum: u32,
    pub write_quorum: u32,
    pub epoch: Epoch,
}

impl QuorumReconfigurationManager {
    /// Create a new reconfiguration manager.
    pub fn new() -> Self {
        QuorumReconfigurationManager {
            reconfigs: HashMap::new(),
            current_configs: HashMap::new(),
            next_id: 0,
            require_intersection_validation: true,
            max_concurrent: 10,
        }
    }

    /// Propose a quorum reconfiguration.
    pub fn propose_reconfiguration(
        &mut self,
        domain: OrderingDomain,
        old_replicas: Vec<NodeId>,
        new_replicas: Vec<NodeId>,
        old_read_quorum: u32,
        old_write_quorum: u32,
        new_read_quorum: u32,
        new_write_quorum: u32,
    ) -> Result<ReconfigId, QuorumError> {
        // Validate new quorum intersection (flexible Paxos).
        let n = new_replicas.len() as u32;
        if new_read_quorum + new_write_quorum <= n && self.require_intersection_validation {
            return Err(QuorumError::FlexiblePaxosViolated {
                r: new_read_quorum,
                w: new_write_quorum,
                n,
            });
        }

        let config_id = ReconfigId(self.next_id);
        self.next_id += 1;

        let reconfig = QuorumReconfiguration {
            config_id,
            domain,
            old_replicas: old_replicas.clone(),
            new_replicas: new_replicas.clone(),
            old_read_quorum,
            old_write_quorum,
            new_read_quorum,
            new_write_quorum,
            status: ReconfigStatus::Proposed,
            created_at: TimestampMillis(0),
            completed_at: None,
        };

        self.reconfigs.insert(config_id, reconfig);
        Ok(config_id)
    }

    /// Approve and start a reconfiguration.
    pub fn approve_reconfiguration(
        &mut self,
        config_id: ReconfigId,
    ) -> Result<(), QuorumError> {
        let reconfig = self
            .reconfigs
            .get_mut(&config_id)
            .ok_or_else(|| QuorumError::Internal {
                message: "reconfiguration not found".to_string(),
            })?;

        if reconfig.status != ReconfigStatus::Proposed {
            return Err(QuorumError::Internal {
                message: "reconfiguration already processed".to_string(),
            });
        }

        // Count concurrent in-progress reconfigs.
        let in_progress = self
            .reconfigs
            .values()
            .filter(|r| r.status == ReconfigStatus::InProgress)
            .count();
        if in_progress >= self.max_concurrent {
            return Err(QuorumError::Internal {
                message: "max concurrent reconfigurations reached".to_string(),
            });
        }

        reconfig.status = ReconfigStatus::InProgress;
        Ok(())
    }

    /// Complete a reconfiguration and update the domain config.
    pub fn complete_reconfiguration(
        &mut self,
        config_id: ReconfigId,
    ) -> Result<(), QuorumError> {
        let reconfig = self
            .reconfigs
            .get(&config_id)
            .cloned()
            .ok_or_else(|| QuorumError::Internal {
                message: "reconfiguration not found".to_string(),
            })?;

        if reconfig.status != ReconfigStatus::InProgress {
            return Err(QuorumError::Internal {
                message: "reconfiguration not in progress".to_string(),
            });
        }

        // Update the domain config.
        let domain_config = DomainQuorumConfig {
            domain: reconfig.domain,
            replicas: reconfig.new_replicas.clone(),
            read_quorum: reconfig.new_read_quorum,
            write_quorum: reconfig.new_write_quorum,
            epoch: Epoch(0),
        };
        self.current_configs.insert(reconfig.domain, domain_config);

        // Mark as completed.
        if let Some(r) = self.reconfigs.get_mut(&config_id) {
            r.status = ReconfigStatus::Completed;
            r.completed_at = Some(TimestampMillis(0));
        }

        Ok(())
    }

    /// Roll back a reconfiguration.
    pub fn rollback_reconfiguration(
        &mut self,
        config_id: ReconfigId,
    ) -> Result<(), QuorumError> {
        let reconfig = self
            .reconfigs
            .get_mut(&config_id)
            .ok_or_else(|| QuorumError::Internal {
                message: "reconfiguration not found".to_string(),
            })?;

        if reconfig.status == ReconfigStatus::Completed {
            return Err(QuorumError::Internal {
                message: "cannot roll back completed reconfiguration".to_string(),
            });
        }

        reconfig.status = ReconfigStatus::RolledBack;
        Ok(())
    }

    /// Return the current config for a domain.
    pub fn current_config(&self, domain: OrderingDomain) -> Option<&DomainQuorumConfig> {
        self.current_configs.get(&domain)
    }

    /// Return a reconfiguration by ID.
    pub fn reconfiguration(&self, config_id: ReconfigId) -> Option<&QuorumReconfiguration> {
        self.reconfigs.get(&config_id)
    }

    /// Return all reconfigurations for a domain.
    pub fn reconfigs_for_domain(&self, domain: OrderingDomain) -> Vec<QuorumReconfiguration> {
        self.reconfigs
            .values()
            .filter(|r| r.domain == domain)
            .cloned()
            .collect()
    }

    /// Return all in-progress reconfigurations.
    pub fn in_progress_reconfigs(&self) -> Vec<QuorumReconfiguration> {
        self.reconfigs
            .values()
            .filter(|r| r.status == ReconfigStatus::InProgress)
            .cloned()
            .collect()
    }

    /// Return the total reconfiguration count.
    pub fn total_reconfigs(&self) -> usize {
        self.reconfigs.len()
    }

    /// Return the completed reconfiguration count.
    pub fn completed_reconfigs(&self) -> usize {
        self.reconfigs
            .values()
            .filter(|r| r.status == ReconfigStatus::Completed)
            .count()
    }

    /// Return the failed reconfiguration count.
    pub fn failed_reconfigs(&self) -> usize {
        self.reconfigs
            .values()
            .filter(|r| r.status == ReconfigStatus::Failed)
            .count()
    }

    /// Set whether intersection validation is required.
    pub fn set_require_intersection_validation(&mut self, required: bool) {
        self.require_intersection_validation = required;
    }

    /// Return whether intersection validation is required.
    pub fn is_intersection_validation_required(&self) -> bool {
        self.require_intersection_validation
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// QuorumConfigurationValidator — validates quorum configurations
///////////////////////////////////////////////////////////////////////////////

/// A validation result for a quorum configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumConfigValidationResult {
    pub valid: bool,
    pub errors: Vec<QuorumConfigError>,
    pub warnings: Vec<QuorumConfigWarning>,
}

/// Errors found during quorum configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumConfigError {
    ReadQuorumZero,
    WriteQuorumZero,
    ReadQuorumExceedsReplicas { read: u32, replicas: u32 },
    WriteQuorumExceedsReplicas { write: u32, replicas: u32 },
    FlexiblePaxosViolated { r: u32, w: u32, n: u32 },
    EmptyReplicaSet,
    DuplicateReplicas,
}

/// Warnings found during quorum configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumConfigWarning {
    LowFaultTolerance { fault_tolerance: u32 },
    LargeQuorumSize { quorum_type: String, size: u32, replicas: u32 },
    ImbalancedQuorums { read: u32, write: u32 },
}

/// Validates quorum configurations for correctness and safety.
pub struct QuorumConfigurationValidator;

impl QuorumConfigurationValidator {
    /// Create a new validator.
    pub fn new() -> Self {
        QuorumConfigurationValidator
    }

    /// Validate a quorum configuration.
    pub fn validate(
        &self,
        replicas: &[NodeId],
        read_quorum: u32,
        write_quorum: u32,
    ) -> QuorumConfigValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let n = replicas.len() as u32;

        // Check for empty replica set.
        if n == 0 {
            errors.push(QuorumConfigError::EmptyReplicaSet);
            return QuorumConfigValidationResult {
                valid: false,
                errors,
                warnings,
            };
        }

        // Check for duplicate replicas.
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut has_duplicates = false;
        for &r in replicas {
            if !seen.insert(r) {
                has_duplicates = true;
            }
        }
        if has_duplicates {
            errors.push(QuorumConfigError::DuplicateReplicas);
        }

        // Check read quorum.
        if read_quorum == 0 {
            errors.push(QuorumConfigError::ReadQuorumZero);
        } else if read_quorum > n {
            errors.push(QuorumConfigError::ReadQuorumExceedsReplicas {
                read: read_quorum,
                replicas: n,
            });
        }

        // Check write quorum.
        if write_quorum == 0 {
            errors.push(QuorumConfigError::WriteQuorumZero);
        } else if write_quorum > n {
            errors.push(QuorumConfigError::WriteQuorumExceedsReplicas {
                write: write_quorum,
                replicas: n,
            });
        }

        // Check flexible Paxos: R + W > N.
        if read_quorum > 0 && write_quorum > 0 && read_quorum + write_quorum <= n {
            errors.push(QuorumConfigError::FlexiblePaxosViolated {
                r: read_quorum,
                w: write_quorum,
                n,
            });
        }

        // Warnings.
        let ft = if read_quorum > 0 && write_quorum > 0 {
            n.saturating_sub(read_quorum).min(n.saturating_sub(write_quorum))
        } else {
            0
        };
        if ft < 2 {
            warnings.push(QuorumConfigWarning::LowFaultTolerance {
                fault_tolerance: ft,
            });
        }
        if read_quorum > n * 2 / 3 {
            warnings.push(QuorumConfigWarning::LargeQuorumSize {
                quorum_type: "read".to_string(),
                size: read_quorum,
                replicas: n,
            });
        }
        if write_quorum > n * 2 / 3 {
            warnings.push(QuorumConfigWarning::LargeQuorumSize {
                quorum_type: "write".to_string(),
                size: write_quorum,
                replicas: n,
            });
        }
        if read_quorum > 0 && write_quorum > 0 && (read_quorum as f64 / write_quorum as f64) > 2.0 {
            warnings.push(QuorumConfigWarning::ImbalancedQuorums {
                read: read_quorum,
                write: write_quorum,
            });
        }

        let valid = errors.is_empty();
        QuorumConfigValidationResult {
            valid,
            errors,
            warnings,
        }
    }

    /// Validate and return a boolean.
    pub fn is_valid(
        &self,
        replicas: &[NodeId],
        read_quorum: u32,
        write_quorum: u32,
    ) -> bool {
        self.validate(replicas, read_quorum, write_quorum).valid
    }

    /// Return the recommended read quorum for a given replica count.
    pub fn recommended_read_quorum(n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (n / 2 + 1).max(1).min(n)
    }

    /// Return the recommended write quorum for a given replica count.
    pub fn recommended_write_quorum(n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (n / 2 + 1).max(1).min(n)
    }

    /// Return the recommended quorum pair for a replica count (R + W > N).
    pub fn recommended_quorum_pair(n: u32) -> (u32, u32) {
        if n == 0 {
            return (0, 0);
        }
        let r = (n / 2 + 1).max(1).min(n);
        let w = (n - r + 1).max(1).min(n);
        (r, w)
    }

    /// Return the fault tolerance for a quorum configuration.
    pub fn fault_tolerance(n: u32, r: u32, w: u32) -> u32 {
        if r == 0 || w == 0 || r + w <= n {
            return 0;
        }
        n.saturating_sub(r).min(n.saturating_sub(w))
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// The EdgeCaseBits bit position for QuorumIntersection.
pub const QUORUM_INTERSECTION_BIT: u128 = 1 << 7;

/// Check whether the quorum intersection edge case is enabled in the given bits.
pub fn quorum_intersection_enabled(bits: &EdgeCaseBits) -> bool {
    bits.has(QUORUM_INTERSECTION_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // ---- QuorumSelection tests ----

    #[test]
    fn quorum_selection_construction() {
        let members = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let selection = QuorumSelection {
            members: members.clone(),
            quorum_size: 2,
            total_replicas: 5,
            selection_method: SelectionMethod::DeterministicById,
        };
        assert_eq!(selection.members, members);
        assert_eq!(selection.quorum_size, 2);
        assert_eq!(selection.total_replicas, 5);
    }

    // ---- SelectionMethod tests ----

    #[test]
    fn selection_method_variants() {
        let methods = [
            SelectionMethod::DeterministicById,
            SelectionMethod::RandomizedSeeded,
            SelectionMethod::WeightedByWeight,
            SelectionMethod::GeographicAware,
        ];
        for m in methods {
            let _ = format!("{:?}", m);
        }
    }

    // ---- IntersectionProof tests ----

    #[test]
    fn intersection_proof_construction() {
        let read = vec![NodeId::new([1u8; 32])];
        let write = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let intersection = vec![NodeId::new([1u8; 32])];
        let proof = IntersectionProof {
            read_quorum: read,
            write_quorum: write,
            intersection: intersection.clone(),
            intersection_size: 1,
            total_replicas: 3,
            satisfies_flexible_paxos: true,
        };
        assert_eq!(proof.intersection_size, 1);
        assert!(proof.satisfies_flexible_paxos);
    }

    // ---- FlexiblePaxosQuorums tests ----

    #[test]
    fn flexible_paxos_quorums_construction() {
        let fp = FlexiblePaxosQuorums {
            read_quorum: vec![NodeId::new([1u8; 32])],
            write_quorum: vec![NodeId::new([2u8; 32])],
            n: 3,
            r: 1,
            w: 1,
            r_plus_w: 2,
        };
        assert_eq!(fp.n, 3);
        assert_eq!(fp.r_plus_w, 2);
    }

    // ---- QuorumConfig tests ----

    #[test]
    fn quorum_config_construction() {
        let config = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
            read_quorum_size: 1,
            write_quorum_size: 2,
            epoch: Epoch(5),
        };
        assert_eq!(config.domain, OrderingDomain(1));
        assert_eq!(config.epoch, Epoch(5));
        assert_eq!(config.read_quorum_size, 1);
    }

    // ---- IntersectionAuditEntry tests ----

    #[test]
    fn intersection_audit_entry_construction() {
        let entry = IntersectionAuditEntry {
            entry_id: AuditEntryId(1),
            domain: OrderingDomain(1),
            read_quorum: vec![NodeId::new([1u8; 32])],
            write_quorum: vec![NodeId::new([1u8; 32])],
            intersection_size: 1,
            valid: true,
            timestamp: TimestampMillis(100),
        };
        assert_eq!(entry.entry_id.0, 1);
        assert!(entry.valid);
    }

    // ---- AuditEntryId tests ----

    #[test]
    fn audit_entry_id_ordering() {
        let a = AuditEntryId(10);
        let b = AuditEntryId(20);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- Epoch tests ----

    #[test]
    fn epoch_ordering() {
        let e1 = Epoch(1);
        let e2 = Epoch(2);
        assert!(e1 < e2);
        assert_ne!(e1, e2);
    }

    // ---- AvailabilityResult tests ----

    #[test]
    fn availability_result_variants() {
        let variants = [
            AvailabilityResult::Available,
            AvailabilityResult::ReadOnly,
            AvailabilityResult::WriteOnly,
            AvailabilityResult::Unavailable,
        ];
        for v in variants {
            let _ = format!("{:?}", v);
        }
    }

    // ---- QuorumError tests ----

    #[test]
    fn quorum_error_variants() {
        let err = QuorumError::InsufficientReplicas {
            available: 2,
            required: 3,
        };
        match &err {
            QuorumError::InsufficientReplicas { available, required } => {
                assert_eq!(*available, 2);
                assert_eq!(*required, 3);
            }
            _ => panic!("expected InsufficientReplicas"),
        }

        let err2 = QuorumError::QuorumExceedsReplicas {
            quorum: 5,
            replicas: 3,
        };
        match &err2 {
            QuorumError::QuorumExceedsReplicas { quorum, replicas } => {
                assert_eq!(*quorum, 5);
                assert_eq!(*replicas, 3);
            }
            _ => panic!("expected QuorumExceedsReplicas"),
        }

        let err3 = QuorumError::NoIntersection {
            read_size: 2,
            write_size: 2,
        };
        match &err3 {
            QuorumError::NoIntersection { read_size, write_size } => {
                assert_eq!(*read_size, 2);
                assert_eq!(*write_size, 2);
            }
            _ => panic!("expected NoIntersection"),
        }

        let err4 = QuorumError::FlexiblePaxosViolated { r: 1, w: 1, n: 3 };
        match &err4 {
            QuorumError::FlexiblePaxosViolated { r, w, n } => {
                assert_eq!(*r, 1);
                assert_eq!(*w, 1);
                assert_eq!(*n, 3);
            }
            _ => panic!("expected FlexiblePaxosViolated"),
        }

        let err5 = QuorumError::NodeNotMember {
            node: NodeId::new([1u8; 32]),
            domain: OrderingDomain(1),
        };
        match &err5 {
            QuorumError::NodeNotMember { node, domain } => {
                assert_eq!(node.0[0], 1);
                assert_eq!(domain.0, 1);
            }
            _ => panic!("expected NodeNotMember"),
        }
    }

    // ---- QuorumIntersectionEngine default ----

    #[test]
    fn engine_default_creation() {
        let engine = QuorumIntersectionEngine::default();
        assert_eq!(engine.calculation_count(), 0);
        assert_eq!(engine.failure_count(), 0);
        assert!(engine.is_flexible_paxos_enforced());
        assert!(engine.intersection_audit().is_empty());
    }

    #[test]
    fn engine_without_flexible_paxos() {
        let engine = QuorumIntersectionEngine::without_flexible_paxos();
        assert!(!engine.is_flexible_paxos_enforced());
    }

    // ---- register_config ----

    #[test]
    fn register_config_valid() {
        let mut engine = QuorumIntersectionEngine::new();
        let config = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![
                NodeId::new([1u8; 32]),
                NodeId::new([2u8; 32]),
                NodeId::new([3u8; 32]),
            ],
            read_quorum_size: 2,
            write_quorum_size: 2,
            epoch: Epoch(1),
        };
        let result = engine.register_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn register_config_zero_quorum_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let config = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
            read_quorum_size: 0,
            write_quorum_size: 1,
            epoch: Epoch(1),
        };
        let result = engine.register_config(config);
        assert!(result.is_err());
        match result.unwrap_err() {
            QuorumError::ZeroQuorumSize => {}
            _ => panic!("expected ZeroQuorumSize"),
        }
    }

    #[test]
    fn register_config_exceeds_replicas_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let config = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32])],
            read_quorum_size: 2,
            write_quorum_size: 1,
            epoch: Epoch(1),
        };
        let result = engine.register_config(config);
        assert!(result.is_err());
    }

    // ---- read_quorum ----

    #[test]
    fn read_quorum_selects_correct_count() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let quorum = engine.read_quorum(&replicas, 2);
        assert!(quorum.is_ok());
        let q = quorum.unwrap();
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn read_quorum_zero_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = engine.read_quorum(&replicas, 0);
        assert!(result.is_err());
    }

    #[test]
    fn read_quorum_exceeds_replicas_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = engine.read_quorum(&replicas, 2);
        assert!(result.is_err());
    }

    #[test]
    fn read_quorum_deterministic() {
        let mut engine1 = QuorumIntersectionEngine::new();
        let mut engine2 = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([3u8; 32]),
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
        ];
        let q1 = engine1.read_quorum(&replicas, 2).unwrap();
        let q2 = engine2.read_quorum(&replicas, 2).unwrap();
        assert_eq!(q1, q2);
    }

    // ---- write_quorum ----

    #[test]
    fn write_quorum_selects_correct_count() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let quorum = engine.write_quorum(&replicas, 2);
        assert!(quorum.is_ok());
        let q = quorum.unwrap();
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn write_quorum_zero_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = engine.write_quorum(&replicas, 0);
        assert!(result.is_err());
    }

    // ---- intersection_non_empty ----

    #[test]
    fn intersection_non_empty_detects_overlap() {
        let engine = QuorumIntersectionEngine::new();
        let read = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let write = vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])];
        assert!(engine.intersection_non_empty(&read, &write));
    }

    #[test]
    fn intersection_non_empty_no_overlap() {
        let engine = QuorumIntersectionEngine::new();
        let read = vec![NodeId::new([1u8; 32])];
        let write = vec![NodeId::new([2u8; 32])];
        assert!(!engine.intersection_non_empty(&read, &write));
    }

    #[test]
    fn intersection_non_empty_empty_read() {
        let engine = QuorumIntersectionEngine::new();
        assert!(!engine.intersection_non_empty(&[], &[NodeId::new([1u8; 32])]));
    }

    #[test]
    fn intersection_non_empty_empty_write() {
        let engine = QuorumIntersectionEngine::new();
        assert!(!engine.intersection_non_empty(&[NodeId::new([1u8; 32])], &[]));
    }

    // ---- validate_intersection ----

    #[test]
    fn validate_intersection_success() {
        let mut engine = QuorumIntersectionEngine::new();
        let read = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let write = vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])];
        let proof = engine.validate_intersection(&read, &write);
        assert!(proof.is_ok());
        let p = proof.unwrap();
        assert_eq!(p.intersection_size, 1);
        assert_eq!(p.intersection, vec![NodeId::new([2u8; 32])]);
    }

    #[test]
    fn validate_intersection_fails_when_disjoint() {
        let mut engine = QuorumIntersectionEngine::new();
        let read = vec![NodeId::new([1u8; 32])];
        let write = vec![NodeId::new([2u8; 32])];
        let result = engine.validate_intersection(&read, &write);
        assert!(result.is_err());
        assert_eq!(engine.failure_count(), 1);
    }

    #[test]
    fn validate_intersection_proof_fields() {
        let mut engine = QuorumIntersectionEngine::new();
        let read = vec![NodeId::new([1u8; 32])];
        let write = vec![NodeId::new([1u8; 32])];
        let proof = engine.validate_intersection(&read, &write).unwrap();
        assert!(proof.satisfies_flexible_paxos);
        assert_eq!(proof.total_replicas, 1);
    }

    // ---- min_write_quorum ----

    #[test]
    fn min_write_quorum_calculation() {
        let engine = QuorumIntersectionEngine::new();
        // N=3, R=1 => W > 2 => W = 3
        assert_eq!(engine.min_write_quorum(3, 1), 3);
        // N=3, R=2 => W > 1 => W = 2
        assert_eq!(engine.min_write_quorum(3, 2), 2);
        // N=5, R=3 => W > 2 => W = 3
        assert_eq!(engine.min_write_quorum(5, 3), 3);
    }

    // ---- min_read_quorum ----

    #[test]
    fn min_read_quorum_calculation() {
        let engine = QuorumIntersectionEngine::new();
        assert_eq!(engine.min_read_quorum(3, 1), 3);
        assert_eq!(engine.min_read_quorum(3, 2), 2);
        assert_eq!(engine.min_read_quorum(5, 3), 3);
    }

    // ---- tolerates_failures ----

    #[test]
    fn tolerates_failures_checks() {
        let engine = QuorumIntersectionEngine::new();
        // N=3, R=2, W=2: R+W=4>3, tolerates 1 failure
        assert!(engine.tolerates_failures(3, 2, 2, 1));
        // N=3, R=2, W=2: only 1 remaining after 2 failures, can't form quorum
        assert!(!engine.tolerates_failures(3, 2, 2, 2));
    }

    // ---- fault_tolerance ----

    #[test]
    fn fault_tolerance_calculation() {
        let engine = QuorumIntersectionEngine::new();
        // N=3, R=2, W=2: FT = min(3-2, 3-2) = 1
        assert_eq!(engine.fault_tolerance(3, 2, 2), 1);
        // N=5, R=3, W=3: FT = min(5-3, 5-3) = 2
        assert_eq!(engine.fault_tolerance(5, 3, 3), 2);
        // Invalid config R+W<=N: FT = 0
        assert_eq!(engine.fault_tolerance(3, 1, 1), 0);
    }

    // ---- flexible_paxos_quorums ----

    #[test]
    fn flexible_paxos_quorums_valid() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let result = engine.flexible_paxos_quorums(&replicas, 2, 2);
        assert!(result.is_ok());
        let fp = result.unwrap();
        assert!(fp.r_plus_w > fp.n);
    }

    #[test]
    fn flexible_paxos_quorums_violated() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let result = engine.flexible_paxos_quorums(&replicas, 1, 1);
        assert!(result.is_err());
        assert_eq!(engine.failure_count(), 1);
    }

    // ---- epoch_compatible ----

    #[test]
    fn epoch_compatible_same_domain() {
        let engine = QuorumIntersectionEngine::new();
        let a = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
            read_quorum_size: 1,
            write_quorum_size: 2,
            epoch: Epoch(1),
        };
        let b = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
            read_quorum_size: 1,
            write_quorum_size: 2,
            epoch: Epoch(2),
        };
        assert!(engine.epoch_compatible(&a, &b));
    }

    #[test]
    fn epoch_compatible_diff_domain() {
        let engine = QuorumIntersectionEngine::new();
        let a = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: vec![NodeId::new([1u8; 32])],
            read_quorum_size: 1,
            write_quorum_size: 1,
            epoch: Epoch(1),
        };
        let b = QuorumConfig {
            domain: OrderingDomain(2),
            replicas: vec![NodeId::new([1u8; 32])],
            read_quorum_size: 1,
            write_quorum_size: 1,
            epoch: Epoch(1),
        };
        assert!(!engine.epoch_compatible(&a, &b));
    }

    // ---- track_membership / untrack_membership ----

    #[test]
    fn track_and_untrack_membership() {
        let mut engine = QuorumIntersectionEngine::new();
        let node = NodeId::new([1u8; 32]);
        let domain = OrderingDomain(1);
        engine.track_membership(domain, node, 5).unwrap();
        assert_eq!(engine.node_weight(domain, node), 5);
        engine.untrack_membership(domain, node).unwrap();
        assert_eq!(engine.node_weight(domain, node), 0);
    }

    #[test]
    fn track_duplicate_membership_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let node = NodeId::new([1u8; 32]);
        engine.track_membership(OrderingDomain(1), node, 1).unwrap();
        let result = engine.track_membership(OrderingDomain(1), node, 2);
        assert!(result.is_err());
    }

    #[test]
    fn untrack_unknown_node_fails() {
        let mut engine = QuorumIntersectionEngine::new();
        let result = engine.untrack_membership(OrderingDomain(1), NodeId::new([1u8; 32]));
        assert!(result.is_err());
    }

    // ---- domain_members ----

    #[test]
    fn domain_members_returns_sorted() {
        let mut engine = QuorumIntersectionEngine::new();
        engine.track_membership(OrderingDomain(1), NodeId::new([3u8; 32]), 1).unwrap();
        engine.track_membership(OrderingDomain(1), NodeId::new([1u8; 32]), 1).unwrap();
        engine.track_membership(OrderingDomain(1), NodeId::new([2u8; 32]), 1).unwrap();
        let members = engine.domain_members(OrderingDomain(1));
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].0[0], 1);
        assert_eq!(members[1].0[0], 2);
        assert_eq!(members[2].0[0], 3);
    }

    // ---- weighted_intersection ----

    #[test]
    fn weighted_intersection_detects_overlap() {
        let engine = QuorumIntersectionEngine::new();
        let read = vec![(NodeId::new([1u8; 32]), 3), (NodeId::new([2u8; 32]), 2)];
        let write = vec![(NodeId::new([2u8; 32]), 1), (NodeId::new([3u8; 32]), 4)];
        assert!(engine.weighted_intersection(&read, &write));
    }

    #[test]
    fn weighted_intersection_no_overlap() {
        let engine = QuorumIntersectionEngine::new();
        let read = vec![(NodeId::new([1u8; 32]), 3)];
        let write = vec![(NodeId::new([2u8; 32]), 4)];
        assert!(!engine.weighted_intersection(&read, &write));
    }

    // ---- availability_check ----

    #[test]
    fn availability_check_available() {
        let engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let result = engine.availability_check(&replicas, 2, 2, &[]);
        assert_eq!(result, AvailabilityResult::Available);
    }

    #[test]
    fn availability_check_read_only() {
        let engine = QuorumIntersectionEngine::new();
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let result = engine.availability_check(&replicas, 1, 2, &[NodeId::new([3u8; 32])]);
        assert_eq!(result, AvailabilityResult::ReadOnly);
    }

    #[test]
    fn availability_check_unavailable() {
        let engine = QuorumIntersectionEngine::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = engine.availability_check(&replicas, 2, 2, &[NodeId::new([1u8; 32])]);
        assert_eq!(result, AvailabilityResult::Unavailable);
    }

    // ---- intersection_audit / record_intersection_event ----

    #[test]
    fn audit_trail_records_events() {
        let mut engine = QuorumIntersectionEngine::new();
        let entry = IntersectionAuditEntry {
            entry_id: AuditEntryId(1),
            domain: OrderingDomain(1),
            read_quorum: vec![NodeId::new([1u8; 32])],
            write_quorum: vec![NodeId::new([1u8; 32])],
            intersection_size: 1,
            valid: true,
            timestamp: TimestampMillis(100),
        };
        engine.record_intersection_entry(entry);
        let audit = engine.intersection_audit();
        assert_eq!(audit.len(), 1);
    }

    // ---- calculation_count / failure_count ----

    #[test]
    fn calculation_and_failure_counts() {
        let mut engine = QuorumIntersectionEngine::new();
        engine.read_quorum(&[NodeId::new([1u8; 32])], 1).unwrap();
        assert_eq!(engine.calculation_count(), 1);
        engine.read_quorum(&[NodeId::new([1u8; 32])], 0).unwrap_err();
        assert_eq!(engine.failure_count(), 1);
    }

    // ---- flexible_paxos toggle ----

    #[test]
    fn flexible_paxos_toggle() {
        let mut engine = QuorumIntersectionEngine::new();
        assert!(engine.is_flexible_paxos_enforced());
        engine.set_enforce_flexible_paxos(false);
        assert!(!engine.is_flexible_paxos_enforced());
    }

    // ---- QuorumValidator tests ----

    #[test]
    fn quorum_validator_intersection() {
        let read = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let write = vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])];
        assert!(QuorumValidator::validate(&read, &write));
    }

    #[test]
    fn quorum_validator_no_intersection() {
        let read = vec![NodeId::new([1u8; 32])];
        let write = vec![NodeId::new([2u8; 32])];
        assert!(!QuorumValidator::validate(&read, &write));
    }

    #[test]
    fn quorum_validator_flexible_paxos() {
        assert!(QuorumValidator::validate_flexible_paxos(2, 2, 3));
        assert!(!QuorumValidator::validate_flexible_paxos(1, 1, 3));
    }

    #[test]
    fn quorum_validator_intersection_nodes() {
        let read = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let write = vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])];
        let inter = QuorumValidator::intersection(&read, &write);
        assert_eq!(inter.len(), 1);
        assert_eq!(inter[0], NodeId::new([2u8; 32]));
    }

    #[test]
    fn quorum_validator_intersection_size() {
        let read = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let write = vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])];
        assert_eq!(QuorumValidator::intersection_size(&read, &write), 1);
    }

    // ---- QUORUM_INTERSECTION_BIT ----

    #[test]
    fn quorum_intersection_bit_position() {
        assert_eq!(QUORUM_INTERSECTION_BIT, 1 << 7);
    }

    // ---- quorum_intersection_enabled ----

    #[test]
    fn quorum_intersection_enabled_checks_bit() {
        let mut bits = EdgeCaseBits(0);
        assert!(!quorum_intersection_enabled(&bits));
        bits.enable(QUORUM_INTERSECTION_BIT);
        assert!(quorum_intersection_enabled(&bits));
    }

    // ---- Integration: full quorum workflow ----

    #[test]
    fn full_quorum_workflow() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];

        // Register config.
        let config = QuorumConfig {
            domain: OrderingDomain(1),
            replicas: replicas.clone(),
            read_quorum_size: 2,
            write_quorum_size: 2,
            epoch: Epoch(1),
        };
        engine.register_config(config).unwrap();

        // Select quorums.
        let read = engine.read_quorum(&replicas, 2).unwrap();
        let write = engine.write_quorum(&replicas, 2).unwrap();

        // Validate intersection.
        let proof = engine.validate_intersection(&read, &write).unwrap();
        assert!(proof.intersection_size > 0);

        // Verify flexible Paxos.
        assert!(proof.satisfies_flexible_paxos);

        // Check fault tolerance.
        assert_eq!(engine.fault_tolerance(3, 2, 2), 1);

        // Track membership.
        for (i, &r) in replicas.iter().enumerate() {
            engine.track_membership(OrderingDomain(1), r, 1).unwrap();
        }
        assert_eq!(engine.domain_members(OrderingDomain(1)).len(), 3);

        // Availability check.
        let avail = engine.availability_check(&replicas, 2, 2, &[]);
        assert_eq!(avail, AvailabilityResult::Available);
    }

    // ---- Multiple domains independent ----

    #[test]
    fn multiple_domains_independent() {
        let mut engine = QuorumIntersectionEngine::new();
        engine.track_membership(OrderingDomain(1), NodeId::new([1u8; 32]), 1).unwrap();
        engine.track_membership(OrderingDomain(2), NodeId::new([2u8; 32]), 1).unwrap();
        assert_eq!(engine.domain_members(OrderingDomain(1)).len(), 1);
        assert_eq!(engine.domain_members(OrderingDomain(2)).len(), 1);
        assert_eq!(engine.node_weight(OrderingDomain(1), NodeId::new([2u8; 32])), 0);
    }

    // ---- Deterministic quorum selection stability ----

    #[test]
    fn quorum_selection_stable_across_calls() {
        let mut engine = QuorumIntersectionEngine::new();
        let replicas = vec![
            NodeId::new([3u8; 32]),
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
        ];
        let q1 = engine.read_quorum(&replicas, 2).unwrap();
        let q2 = engine.read_quorum(&replicas, 2).unwrap();
        assert_eq!(q1, q2);
    }

    // ---- Audit entry ID monotonic ----

    #[test]
    fn audit_entry_ids_monotonic() {
        let mut engine = QuorumIntersectionEngine::new();
        for i in 0..5 {
            let entry = IntersectionAuditEntry {
                entry_id: AuditEntryId(i),
                domain: OrderingDomain(1),
                read_quorum: vec![NodeId::new([1u8; 32])],
                write_quorum: vec![NodeId::new([1u8; 32])],
                intersection_size: 1,
                valid: true,
                timestamp: TimestampMillis(0),
            };
            engine.record_intersection_entry(entry);
        }
        let audit = engine.intersection_audit();
        assert_eq!(audit.len(), 5);
        for pair in audit.windows(2) {
            assert!(pair[0].entry_id.0 < pair[1].entry_id.0);
        }
    }

    // ---- Record intersection entry helper ----

    fn record_intersection_entry(
        &mut self,
        entry: IntersectionAuditEntry,
    ) {
        self.audit_trail.push(entry);
    }

    // ========== QuorumConfigurationValidator tests ==========

    #[test]
    fn validator_valid_configuration() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let result = validator.validate(&replicas, 2, 2);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn validator_empty_replicas_invalid() {
        let validator = QuorumConfigurationValidator::new();
        let result = validator.validate(&[], 1, 1);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::EmptyReplicaSet)));
    }

    #[test]
    fn validator_read_quorum_zero_invalid() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = validator.validate(&replicas, 0, 1);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::ReadQuorumZero)));
    }

    #[test]
    fn validator_write_quorum_zero_invalid() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![NodeId::new([1u8; 32])];
        let result = validator.validate(&replicas, 1, 0);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::WriteQuorumZero)));
    }

    #[test]
    fn validator_read_quorum_exceeds_replicas() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let result = validator.validate(&replicas, 3, 1);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::ReadQuorumExceedsReplicas { .. })));
    }

    #[test]
    fn validator_write_quorum_exceeds_replicas() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        let result = validator.validate(&replicas, 1, 3);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::WriteQuorumExceedsReplicas { .. })));
    }

    #[test]
    fn validator_flexible_paxos_violated() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        // R=1, W=1, N=3 => R+W=2 <= 3, violated.
        let result = validator.validate(&replicas, 1, 1);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::FlexiblePaxosViolated { .. })));
    }

    #[test]
    fn validator_duplicate_replicas_invalid() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([1u8; 32]),
        ];
        let result = validator.validate(&replicas, 1, 1);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, QuorumConfigError::DuplicateReplicas)));
    }

    #[test]
    fn validator_low_fault_tolerance_warning() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
        ];
        let result = validator.validate(&replicas, 1, 1);
        // R+W=2 <= N=2, violated, so it's invalid anyway.
        assert!(!result.valid);
    }

    #[test]
    fn validator_large_quorum_warning() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
            NodeId::new([4u8; 32]),
            NodeId::new([5u8; 32]),
        ];
        // N=5, R=4 (> 5*2/3=3), W=2 => R+W=6>5 valid, but R is large.
        let result = validator.validate(&replicas, 4, 2);
        assert!(result.valid);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, QuorumConfigWarning::LargeQuorumSize { .. })));
    }

    #[test]
    fn validator_imbalanced_quorums_warning() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        // R=3, W=1 => R+W=4>3 valid, but ratio 3.0 > 2.0.
        let result = validator.validate(&replicas, 3, 1);
        assert!(result.valid);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, QuorumConfigWarning::ImbalancedQuorums { .. })));
    }

    #[test]
    fn validator_is_valid_bool() {
        let validator = QuorumConfigurationValidator::new();
        let replicas = vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])];
        assert!(validator.is_valid(&replicas, 1, 2));
        assert!(!validator.is_valid(&replicas, 0, 2));
    }

    #[test]
    fn validator_recommended_read_quorum() {
        assert_eq!(QuorumConfigurationValidator::recommended_read_quorum(0), 0);
        assert_eq!(QuorumConfigurationValidator::recommended_read_quorum(1), 1);
        assert_eq!(QuorumConfigurationValidator::recommended_read_quorum(3), 2);
        assert_eq!(QuorumConfigurationValidator::recommended_read_quorum(5), 3);
        assert_eq!(QuorumConfigurationValidator::recommended_read_quorum(10), 6);
    }

    #[test]
    fn validator_recommended_write_quorum() {
        assert_eq!(QuorumConfigurationValidator::recommended_write_quorum(0), 0);
        assert_eq!(QuorumConfigurationValidator::recommended_write_quorum(1), 1);
        assert_eq!(QuorumConfigurationValidator::recommended_write_quorum(3), 2);
    }

    #[test]
    fn validator_recommended_quorum_pair() {
        assert_eq!(QuorumConfigurationValidator::recommended_quorum_pair(0), (0, 0));
        let (r, w) = QuorumConfigurationValidator::recommended_quorum_pair(3);
        assert!(r + w > 3);
        assert!(r > 0 && w > 0);
        let (r, w) = QuorumConfigurationValidator::recommended_quorum_pair(5);
        assert!(r + w > 5);
        assert!(r > 0 && w > 0);
    }

    #[test]
    fn validator_fault_tolerance() {
        assert_eq!(
            QuorumConfigurationValidator::fault_tolerance(3, 2, 2),
            1
        );
        assert_eq!(
            QuorumConfigurationValidator::fault_tolerance(5, 3, 3),
            2
        );
        assert_eq!(QuorumConfigurationValidator::fault_tolerance(3, 1, 1), 0);
        assert_eq!(QuorumConfigurationValidator::fault_tolerance(5, 0, 3), 0);
    }

    // ========== QUORUM_INTERSECTION_BIT tests ==========

    #[test]
    fn quorum_intersection_bit_position() {
        assert_eq!(QUORUM_INTERSECTION_BIT, 1 << 7);
    }

    #[test]
    fn quorum_intersection_enabled_checks_bit() {
        let mut bits = EdgeCaseBits(0);
        assert!(!quorum_intersection_enabled(&bits));
        bits.enable(QUORUM_INTERSECTION_BIT);
        assert!(quorum_intersection_enabled(&bits));
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////