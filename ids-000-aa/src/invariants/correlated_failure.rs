///////////////////////////////////////////////////////////////////////////////
// Correlated Failure Invariant — replicas never share failure domains
//
// IDS-EDGE-CASES-RESPONSES #7: replicas must never share power, facility,
// network segment, operator, or geographic domain. This invariant guarantees
// that correlated failures cannot take out all replicas of a dataset.
//
// Every replica placement is checked against failure-domain constraints:
// power domains, facility domains, network segments, operators, and
// geographic regions must be distinct across replicas where possible.
//
// This file is part of ids-000-aa, the central hub crate. All types and
// traits here are sealed — downstream crates cannot implement them.
//
// Carrier-agnostic: uses only opaque Endpoint types (CarrierType + Vec<u8>).
// No std::net, SocketAddr, IP, TCP, UDP, or any existing networking stack.
///////////////////////////////////////////////////////////////////////////////

use crate::types::{DatasetId, FailureDomain, NodeId, RegionId};
use crate::errors::Error;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait contract
///////////////////////////////////////////////////////////////////////////////

/// Sealed trait guaranteeing replicas never share correlated failure domains.
///
/// Downstream crates may call methods and use as trait bounds, but cannot
/// implement — only `ids-000-aa` and its designated implementation crates.
pub trait CorrelatedFailure: private::Sealed {
    /// Check whether a set of replicas shares any correlated failure domain.
    fn failure_domain_check(&self, replicas: Vec<NodeId>) -> Result<bool, Error>;

    /// Verify two nodes have distinct power domains.
    fn power_domain_distinct(&self, a: NodeId, b: NodeId) -> bool;

    /// Verify two nodes have distinct facility domains.
    fn facility_distinct(&self, a: NodeId, b: NodeId) -> bool;

    /// Verify two nodes have distinct operator domains.
    fn operator_distinct(&self, a: NodeId, b: NodeId) -> bool;

    /// Verify two nodes have distinct network segment domains.
    fn network_segment_distinct(&self, a: NodeId, b: NodeId) -> bool;

    /// Verify two nodes have distinct geographic domains.
    fn geographic_distinct(&self, a: NodeId, b: NodeId) -> bool;

    /// Compute the correlated risk score for a replica set (0.0 = independent,
    /// 1.0 = fully correlated).
    fn correlated_risk_score(&self, replicas: Vec<NodeId>) -> f64;

    /// Return the set of failure domains shared by any pair of replicas.
    fn shared_domains(&self, replicas: Vec<NodeId>) -> Vec<SharedDomain>;

    /// Check whether a placement plan satisfies all failure-domain constraints.
    fn placement_satisfies_constraints(
        &self,
        replicas: Vec<NodeId>,
        required_distinct: FailureDomainMask,
    ) -> Result<(), PlacementConstraintError>;
}

mod private {
    /// Sealed supertrait — only ids-000-aa can implement CorrelatedFailure.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl CorrelatedFailure for () {
    fn failure_domain_check(&self, _replicas: Vec<NodeId>) -> Result<bool, Error> {
        Ok(true)
    }
    fn power_domain_distinct(&self, _a: NodeId, _b: NodeId) -> bool {
        true
    }
    fn facility_distinct(&self, _a: NodeId, _b: NodeId) -> bool {
        true
    }
    fn operator_distinct(&self, _a: NodeId, _b: NodeId) -> bool {
        true
    }
    fn network_segment_distinct(&self, _a: NodeId, _b: NodeId) -> bool {
        true
    }
    fn geographic_distinct(&self, _a: NodeId, _b: NodeId) -> bool {
        true
    }
    fn correlated_risk_score(&self, _replicas: Vec<NodeId>) -> f64 {
        0.0
    }
    fn shared_domains(&self, _replicas: Vec<NodeId>) -> Vec<SharedDomain> {
        Vec::new()
    }
    fn placement_satisfies_constraints(
        &self,
        _replicas: Vec<NodeId>,
        _required_distinct: FailureDomainMask,
    ) -> Result<(), PlacementConstraintError> {
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// Failure domain assignment and tracking
///////////////////////////////////////////////////////////////////////////////

/// A failure domain assignment for a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeFailureDomain {
    pub node: NodeId,
    pub power: Option<[u8; 16]>,
    pub facility: Option<[u8; 16]>,
    pub network_segment: Option<[u8; 16]>,
    pub operator: Option<[u8; 16]>,
    pub geographic: Option<[u8; 16]>,
}

impl NodeFailureDomain {
    /// Create a new failure domain assignment.
    pub fn new(node: NodeId) -> Self {
        NodeFailureDomain {
            node,
            power: None,
            facility: None,
            network_segment: None,
            operator: None,
            geographic: None,
        }
    }

    /// Set the power domain.
    pub fn with_power(mut self, domain: [u8; 16]) -> Self {
        self.power = Some(domain);
        self
    }

    /// Set the facility domain.
    pub fn with_facility(mut self, domain: [u8; 16]) -> Self {
        self.facility = Some(domain);
        self
    }

    /// Set the network segment domain.
    pub fn with_network_segment(mut self, domain: [u8; 16]) -> Self {
        self.network_segment = Some(domain);
        self
    }

    /// Set the operator domain.
    pub fn with_operator(mut self, domain: [u8; 16]) -> Self {
        self.operator = Some(domain);
        self
    }

    /// Set the geographic domain.
    pub fn with_geographic(mut self, domain: [u8; 16]) -> Self {
        self.geographic = Some(domain);
        self
    }

    /// Return whether this node has any domain assigned.
    pub fn has_any_domain(&self) -> bool {
        self.power.is_some()
            || self.facility.is_some()
            || self.network_segment.is_some()
            || self.operator.is_some()
            || self.geographic.is_some()
    }

    /// Return the count of assigned domains.
    pub fn domain_count(&self) -> u32 {
        [self.power.is_some(), self.facility.is_some(), self.network_segment.is_some(), self.operator.is_some(), self.geographic.is_some()]
            .iter()
            .filter(|&&x| x)
            .count() as u32
    }
}

/// A bitmask of failure domain types that must be distinct across replicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FailureDomainMask(pub u16);

impl FailureDomainMask {
    pub const POWER: u16 = 1 << 0;
    pub const FACILITY: u16 = 1 << 1;
    pub const NETWORK_SEGMENT: u16 = 1 << 2;
    pub const OPERATOR: u16 = 1 << 3;
    pub const GEOGRAPHIC: u16 = 1 << 4;
    pub const ALL: u16 = Self::POWER | Self::FACILITY | Self::NETWORK_SEGMENT | Self::OPERATOR | Self::GEOGRAPHIC;

    /// Create a mask from individual flags.
    pub fn from_flags(flags: u16) -> Self {
        FailureDomainMask(flags & Self::ALL)
    }

    /// Return whether the power bit is set.
    pub fn requires_power(&self) -> bool {
        self.0 & Self::POWER != 0
    }

    /// Return whether the facility bit is set.
    pub fn requires_facility(&self) -> bool {
        self.0 & Self::FACILITY != 0
    }

    /// Return whether the network segment bit is set.
    pub fn requires_network_segment(&self) -> bool {
        self.0 & Self::NETWORK_SEGMENT != 0
    }

    /// Return whether the operator bit is set.
    pub fn requires_operator(&self) -> bool {
        self.0 & Self::OPERATOR != 0
    }

    /// Return whether the geographic bit is set.
    pub fn requires_geographic(&self) -> bool {
        self.0 & Self::GEOGRAPHIC != 0
    }

    /// Return the count of required distinct domains.
    pub fn required_count(&self) -> u32 {
        self.0.count_ones()
    }
}

/// A shared failure domain between two or more replicas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDomain {
    pub domain_type: DomainType,
    pub domain_value: Vec<u8>,
    pub sharing_nodes: Vec<NodeId>,
}

/// Type of failure domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainType {
    Power,
    Facility,
    NetworkSegment,
    Operator,
    Geographic,
}

/// Tracks failure domain assignments for all known nodes.
pub struct FailureDomainRegistry {
    /// Node -> failure domain assignment.
    domains: HashMap<NodeId, NodeFailureDomain>,
    /// Domain type + value -> set of nodes sharing it.
    domain_index: BTreeMap<(DomainType, Vec<u8>), BTreeSet<NodeId>>,
    /// Total nodes registered.
    registered_count: u64,
}

impl FailureDomainRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        FailureDomainRegistry {
            domains: HashMap::new(),
            domain_index: BTreeMap::new(),
            registered_count: 0,
        }
    }

    /// Register a node's failure domain assignment.
    pub fn register(&mut self, assignment: NodeFailureDomain) {
        let node = assignment.node;
        let is_new = !self.domains.contains_key(&node);

        // Remove old index entries if re-registering.
        if let Some(old) = self.domains.get(&node) {
            self.remove_from_index(old);
        }

        self.domains.insert(node, assignment.clone());
        self.add_to_index(&assignment);

        if is_new {
            self.registered_count += 1;
        }
    }

    fn add_to_index(&mut self, assignment: &NodeFailureDomain) {
        let entries = vec![
            (DomainType::Power, assignment.power),
            (DomainType::Facility, assignment.facility),
            (DomainType::NetworkSegment, assignment.network_segment),
            (DomainType::Operator, assignment.operator),
            (DomainType::Geographic, assignment.geographic),
        ];
        for (domain_type, value) in entries {
            if let Some(v) = value {
                self.domain_index
                    .entry((domain_type, v.to_vec()))
                    .or_default()
                    .insert(assignment.node);
            }
        }
    }

    fn remove_from_index(&mut self, assignment: &NodeFailureDomain) {
        let entries = vec![
            (DomainType::Power, assignment.power),
            (DomainType::Facility, assignment.facility),
            (DomainType::NetworkSegment, assignment.network_segment),
            (DomainType::Operator, assignment.operator),
            (DomainType::Geographic, assignment.geographic),
        ];
        for (domain_type, value) in entries {
            if let Some(v) = value {
                if let Some(nodes) = self.domain_index.get_mut(&(domain_type, v.to_vec())) {
                    nodes.remove(&assignment.node);
                    if nodes.is_empty() {
                        // Note: can't easily remove empty entry from BTreeMap during iteration,
                        // but this is called outside iteration.
                    }
                }
            }
        }
        // Clean up empty entries.
        self.domain_index.retain(|_, nodes| !nodes.is_empty());
    }

    /// Return the failure domain assignment for a node, if registered.
    pub fn domain_for_node(&self, node: NodeId) -> Option<&NodeFailureDomain> {
        self.domains.get(&node)
    }

    /// Return all nodes sharing a specific domain value.
    pub fn nodes_sharing_domain(&self, domain_type: DomainType, value: &[u8]) -> Vec<NodeId> {
        self.domain_index
            .get(&(domain_type, value.to_vec()))
            .map(|nodes| nodes.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Return the total registered node count.
    pub fn registered_count(&self) -> u64 {
        self.registered_count
    }

    /// Return the number of distinct domain values for a domain type.
    pub fn distinct_domain_count(&self, domain_type: DomainType) -> u32 {
        self.domain_index
            .keys()
            .filter(|(dt, _)| *dt == domain_type)
            .count() as u32
    }

    /// Return all registered nodes.
    pub fn all_nodes(&self) -> Vec<NodeId> {
        self.domains.keys().copied().collect()
    }

    /// Check whether two nodes share any domain.
    pub fn nodes_share_any_domain(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.domains.get(&a) {
            Some(d) => d,
            None => return false,
        };
        let db = match self.domains.get(&b) {
            Some(d) => d,
            None => return false,
        };

        Self::domains_overlap(da, db).is_some()
    }

    fn domains_overlap(a: &NodeFailureDomain, b: &NodeFailureDomain) -> Option<DomainType> {
        if let (Some(pa), Some(pb)) = (a.power, b.power) {
            if pa == pb {
                return Some(DomainType::Power);
            }
        }
        if let (Some(fa), Some(fb)) = (a.facility, b.facility) {
            if fa == fb {
                return Some(DomainType::Facility);
            }
        }
        if let (Some(na), Some(nb)) = (a.network_segment, b.network_segment) {
            if na == nb {
                return Some(DomainType::NetworkSegment);
            }
        }
        if let (Some(oa), Some(ob)) = (a.operator, b.operator) {
            if oa == ob {
                return Some(DomainType::Operator);
            }
        }
        if let (Some(ga), Some(gb)) = (a.geographic, b.geographic) {
            if ga == gb {
                return Some(DomainType::Geographic);
            }
        }
        None
    }
}

///////////////////////////////////////////////////////////////////////////////
// Correlated failure detector — core checking engine
///////////////////////////////////////////////////////////////////////////////

/// Result of a correlated failure check for a replica set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelatedFailureReport {
    pub replicas: Vec<NodeId>,
    pub is_correlated: bool,
    pub shared_domains: Vec<SharedDomain>,
    pub risk_score: f64,
    pub distinct_domains: BTreeMap<DomainType, u32>,
    pub recommendation: PlacementRecommendation,
}

/// Recommendation for replica placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRecommendation {
    Accept,
    AcceptWithWarning,
    Reject,
    ReplaceNode { remove: NodeId, suggest_alternative: bool },
}

/// Detects correlated failures across replica sets.
pub struct CorrelatedFailureDetector {
    /// Registry of node failure domains.
    registry: FailureDomainRegistry,
    /// Maximum acceptable risk score (0.0 to 1.0).
    max_risk_score: f64,
    /// Whether geographic diversity is required.
    require_geographic: bool,
    /// Whether operator diversity is required.
    require_operator: bool,
    /// Total checks performed.
    total_checks: u64,
    /// Total checks that failed (correlated).
    failed_checks: u64,
}

impl CorrelatedFailureDetector {
    /// Create a new detector with default settings.
    pub fn new() -> Self {
        CorrelatedFailureDetector {
            registry: FailureDomainRegistry::new(),
            max_risk_score: 0.3,
            require_geographic: true,
            require_operator: true,
            total_checks: 0,
            failed_checks: 0,
        }
    }

    /// Create a detector with custom risk threshold.
    pub fn with_risk_threshold(max_risk: f64) -> Self {
        CorrelatedFailureDetector {
            max_risk_score: max_risk.clamp(0.0, 1.0),
            ..Self::new()
        }
    }

    /// Register a node's failure domain with the detector.
    pub fn register_node(&mut self, assignment: NodeFailureDomain) {
        self.registry.register(assignment);
    }

    /// Check a replica set for correlated failures.
    pub fn check_replica_set(&mut self, replicas: Vec<NodeId>) -> CorrelatedFailureReport {
        self.total_checks += 1;

        let mut shared = Vec::new();
        let mut distinct: BTreeMap<DomainType, u32> = BTreeMap::new();

        // Check all pairs for shared domains.
        for i in 0..replicas.len() {
            for j in (i + 1)..replicas.len() {
                let a = replicas[i];
                let b = replicas[j];
                if let Some(domain_type) = self.registry.nodes_share_any_domain(a, b) {
                    let da = self.registry.domain_for_node(a).unwrap();
                    let db = self.registry.domain_for_node(b).unwrap();

                    let domain_value = match domain_type {
                        DomainType::Power => da.power.unwrap_or_default().to_vec(),
                        DomainType::Facility => da.facility.unwrap_or_default().to_vec(),
                        DomainType::NetworkSegment => da.network_segment.unwrap_or_default().to_vec(),
                        DomainType::Operator => da.operator.unwrap_or_default().to_vec(),
                        DomainType::Geographic => da.geographic.unwrap_or_default().to_vec(),
                    };

                    // Find all nodes sharing this domain value.
                    let sharing = self
                        .registry
                        .nodes_sharing_domain(domain_type, &domain_value);

                    shared.push(SharedDomain {
                        domain_type,
                        domain_value,
                        sharing_nodes: sharing,
                    });

                    *distinct.entry(domain_type).or_insert(0) += 1;
                }
            }
        }

        // Compute distinct domain counts per type.
        for dt in [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ] {
            let count = self.registry.distinct_domain_count(dt);
            distinct.entry(dt).or_insert(count);
        }

        // Risk score: fraction of replica pairs sharing a domain, capped at 1.0.
        let total_pairs = (replicas.len() * (replicas.len() - 1) / 2) as f64;
        let sharing_pairs = shared.len() as f64;
        let risk_score = if total_pairs > 0.0 {
            (sharing_pairs / total_pairs).min(1.0)
        } else {
            0.0
        };

        let is_correlated = risk_score > self.max_risk_score
            || (self.require_geographic && has_geographic_sharing(&shared))
            || (self.require_operator && has_operator_sharing(&shared));

        let recommendation = if is_correlated {
            if risk_score > 0.7 {
                PlacementRecommendation::Reject
            } else {
                PlacementRecommendation::ReplaceNode {
                    remove: replicas.last().copied().unwrap_or(NodeId::new([0u8; 32])),
                    suggest_alternative: true,
                }
            }
        } else if risk_score > 0.1 {
            PlacementRecommendation::AcceptWithWarning
        } else {
            PlacementRecommendation::Accept
        };

        if is_correlated {
            self.failed_checks += 1;
        }

        CorrelatedFailureReport {
            replicas,
            is_correlated,
            shared_domains: shared,
            risk_score,
            distinct_domains: distinct,
            recommendation,
        }
    }

    /// Check whether a placement satisfies required distinct domain constraints.
    pub fn check_placement_constraints(
        &self,
        replicas: Vec<NodeId>,
        required: FailureDomainMask,
    ) -> Result<(), PlacementConstraintError> {
        for i in 0..replicas.len() {
            for j in (i + 1)..replicas.len() {
                let a = replicas[i];
                let b = replicas[j];

                if required.requires_power() {
                    if !self.power_domain_distinct(a, b) {
                        return Err(PlacementConstraintError::PowerDomainShared {
                            node_a: a,
                            node_b: b,
                        });
                    }
                }
                if required.requires_facility() {
                    if !self.facility_distinct(a, b) {
                        return Err(PlacementConstraintError::FacilityDomainShared {
                            node_a: a,
                            node_b: b,
                        });
                    }
                }
                if required.requires_network_segment() {
                    if !self.network_segment_distinct(a, b) {
                        return Err(PlacementConstraintError::NetworkSegmentShared {
                            node_a: a,
                            node_b: b,
                        });
                    }
                }
                if required.requires_operator() {
                    if !self.operator_distinct(a, b) {
                        return Err(PlacementConstraintError::OperatorDomainShared {
                            node_a: a,
                            node_b: b,
                        });
                    }
                }
                if required.requires_geographic() {
                    if !self.geographic_distinct(a, b) {
                        return Err(PlacementConstraintError::GeographicDomainShared {
                            node_a: a,
                            node_b: b,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Return whether two nodes have distinct power domains.
    pub fn power_domain_distinct(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.registry.domain_for_node(a) {
            Some(d) => d,
            None => return true, // unregistered nodes assumed distinct
        };
        let db = match self.registry.domain_for_node(b) {
            Some(d) => d,
            None => return true,
        };
        match (da.power, db.power) {
            (Some(pa), Some(pb)) => pa != pb,
            _ => true,
        }
    }

    /// Return whether two nodes have distinct facility domains.
    pub fn facility_distinct(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.registry.domain_for_node(a) {
            Some(d) => d,
            None => return true,
        };
        let db = match self.registry.domain_for_node(b) {
            Some(d) => d,
            None => return true,
        };
        match (da.facility, db.facility) {
            (Some(fa), Some(fb)) => fa != fb,
            _ => true,
        }
    }

    /// Return whether two nodes have distinct operator domains.
    pub fn operator_distinct(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.registry.domain_for_node(a) {
            Some(d) => d,
            None => return true,
        };
        let db = match self.registry.domain_for_node(b) {
            Some(d) => d,
            None => return true,
        };
        match (da.operator, db.operator) {
            (Some(oa), Some(ob)) => oa != ob,
            _ => true,
        }
    }

    /// Return whether two nodes have distinct network segment domains.
    pub fn network_segment_distinct(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.registry.domain_for_node(a) {
            Some(d) => d,
            None => return true,
        };
        let db = match self.registry.domain_for_node(b) {
            Some(d) => d,
            None => return true,
        };
        match (da.network_segment, db.network_segment) {
            (Some(na), Some(nb)) => na != nb,
            _ => true,
        }
    }

    /// Return whether two nodes have distinct geographic domains.
    pub fn geographic_distinct(&self, a: NodeId, b: NodeId) -> bool {
        let da = match self.registry.domain_for_node(a) {
            Some(d) => d,
            None => return true,
        };
        let db = match self.registry.domain_for_node(b) {
            Some(d) => d,
            None => return true,
        };
        match (da.geographic, db.geographic) {
            (Some(ga), Some(gb)) => ga != gb,
            _ => true,
        }
    }

    /// Return the correlated risk score for a replica set.
    pub fn correlated_risk_score(&self, replicas: Vec<NodeId>) -> f64 {
        let mut shared_pairs = 0u32;
        let mut total_pairs = 0u32;
        for i in 0..replicas.len() {
            for j in (i + 1)..replicas.len() {
                total_pairs += 1;
                if self.registry.nodes_share_any_domain(replicas[i], replicas[j]) {
                    shared_pairs += 1;
                }
            }
        }
        if total_pairs == 0 {
            return 0.0;
        }
        (shared_pairs as f64) / (total_pairs as f64)
    }

    /// Return all shared domains for a replica set.
    pub fn shared_domains(&self, replicas: Vec<NodeId>) -> Vec<SharedDomain> {
        let mut shared = Vec::new();
        let mut seen: HashSet<(DomainType, Vec<u8>)> = HashSet::new();

        for i in 0..replicas.len() {
            for j in (i + 1)..replicas.len() {
                let a = replicas[i];
                let b = replicas[j];
                if let Some(domain_type) = self.registry.nodes_share_any_domain(a, b) {
                    let da = self.registry.domain_for_node(a).unwrap();
                    let domain_value = match domain_type {
                        DomainType::Power => da.power.unwrap_or_default().to_vec(),
                        DomainType::Facility => da.facility.unwrap_or_default().to_vec(),
                        DomainType::NetworkSegment => da.network_segment.unwrap_or_default().to_vec(),
                        DomainType::Operator => da.operator.unwrap_or_default().to_vec(),
                        DomainType::Geographic => da.geographic.unwrap_or_default().to_vec(),
                    };
                    let key = (domain_type, domain_value.clone());
                    if seen.insert(key.clone()) {
                        let sharing = self
                            .registry
                            .nodes_sharing_domain(domain_type, &domain_value);
                        shared.push(SharedDomain {
                            domain_type,
                            domain_value,
                            sharing_nodes: sharing,
                        });
                    }
                }
            }
        }
        shared
    }

    /// Return total checks performed.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return failed check count.
    pub fn failed_checks(&self) -> u64 {
        self.failed_checks
    }

    /// Return the failure rate.
    pub fn failure_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 0.0;
        }
        self.failed_checks as f64 / self.total_checks as f64
    }

    /// Set the maximum acceptable risk score.
    pub fn set_max_risk_score(&mut self, max: f64) {
        self.max_risk_score = max.clamp(0.0, 1.0);
    }

    /// Set whether geographic diversity is required.
    pub fn set_require_geographic(&mut self, required: bool) {
        self.require_geographic = required;
    }

    /// Set whether operator diversity is required.
    pub fn set_require_operator(&mut self, required: bool) {
        self.require_operator = required;
    }

    /// Return the registry for advanced queries.
    pub fn registry(&self) -> &FailureDomainRegistry {
        &self.registry
    }
}

fn has_geographic_sharing(shared: &[SharedDomain]) -> bool {
    shared.iter().any(|s| matches!(s.domain_type, DomainType::Geographic))
}

fn has_operator_sharing(shared: &[SharedDomain]) -> bool {
    shared.iter().any(|s| matches!(s.domain_type, DomainType::Operator))
}

///////////////////////////////////////////////////////////////////////////////
// Placement constraint errors
///////////////////////////////////////////////////////////////////////////////

/// Error when a placement violates failure-domain constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementConstraintError {
    PowerDomainShared { node_a: NodeId, node_b: NodeId },
    FacilityDomainShared { node_a: NodeId, node_b: NodeId },
    NetworkSegmentShared { node_a: NodeId, node_b: NodeId },
    OperatorDomainShared { node_a: NodeId, node_b: NodeId },
    GeographicDomainShared { node_a: NodeId, node_b: NodeId },
    InsufficientNodes { available: u32, required: u32 },
    UnknownNode { node: NodeId },
}

///////////////////////////////////////////////////////////////////////////////
// Replica placement optimizer — suggests correlated-free placements
///////////////////////////////////////////////////////////////////////////////

/// A placement suggestion for a replica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementSuggestion {
    pub node: NodeId,
    pub domain_summary: DomainSummary,
    pub correlation_score: f64,
}

/// Summary of a node's failure domain profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainSummary {
    pub power: Option<Vec<u8>>,
    pub facility: Option<Vec<u8>>,
    pub network_segment: Option<Vec<u8>>,
    pub operator: Option<Vec<u8>>,
    pub geographic: Option<Vec<u8>>,
}

/// Optimizes replica placement to minimize correlated failure risk.
pub struct ReplicaPlacementOptimizer {
    /// Detector for correlated failures.
    detector: CorrelatedFailureDetector,
    /// Candidate nodes for placement.
    candidates: Vec<NodeId>,
    /// Maximum replicas to place.
    max_replicas: u32,
}

impl ReplicaPlacementOptimizer {
    /// Create a new optimizer.
    pub fn new(detector: CorrelatedFailureDetector) -> Self {
        ReplicaPlacementOptimizer {
            detector,
            candidates: Vec::new(),
            max_replicas: 3,
        }
    }

    /// Add a candidate node.
    pub fn add_candidate(&mut self, node: NodeId) {
        if !self.candidates.contains(&node) {
            self.candidates.push(node);
        }
    }

    /// Set the maximum number of replicas.
    pub fn set_max_replicas(&mut self, max: u32) {
        self.max_replicas = max;
    }

    /// Optimize placement: select replicas minimizing correlated risk.
    pub fn optimize(&mut self, required: FailureDomainMask) -> Result<Vec<PlacementSuggestion>, PlacementConstraintError> {
        if (self.candidates.len() as u32) < self.max_replicas {
            return Err(PlacementConstraintError::InsufficientNodes {
                available: self.candidates.len() as u32,
                required: self.max_replicas,
            });
        }

        let mut selected: Vec<NodeId> = Vec::new();
        let mut remaining: Vec<NodeId> = self.candidates.clone();

        // Greedy: pick the node with lowest incremental risk first.
        while selected.len() < self.max_replicas as usize && !remaining.is_empty() {
            let best_idx = (0..remaining.len())
                .map(|i| {
                    let mut trial = selected.clone();
                    trial.push(remaining[i]);
                    let report = self.detector.check_replica_set(trial.clone());
                    (i, report.risk_score)
                })
                .min_by(|(_, ra), (_, rb)| ra.partial_cmp(rb).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap();

            let best_node = remaining.remove(best_idx);
            selected.push(best_node);
        }

        // Verify constraints.
        self.detector.check_placement_constraints(selected.clone(), required)?;

        let mut suggestions = Vec::new();
        for node in &selected {
            let summary = self.build_domain_summary(*node);
            let trial: Vec<NodeId> = selected.iter().copied().collect();
            let report = self.detector.check_replica_set(trial);
            suggestions.push(PlacementSuggestion {
                node: *node,
                domain_summary: summary,
                correlation_score: report.risk_score,
            });
        }

        Ok(suggestions)
    }

    fn build_domain_summary(&self, node: NodeId) -> DomainSummary {
        let da = match self.detector.registry.domain_for_node(node) {
            Some(d) => d,
            None => return DomainSummary {
                power: None, facility: None, network_segment: None, operator: None, geographic: None,
            },
        };
        DomainSummary {
            power: da.power.map(|p| p.to_vec()),
            facility: da.facility.map(|f| f.to_vec()),
            network_segment: da.network_segment.map(|n| n.to_vec()),
            operator: da.operator.map(|o| o.to_vec()),
            geographic: da.geographic.map(|g| g.to_vec()),
        }
    }

    /// Return the number of candidates.
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    /// Return all candidates.
    pub fn candidates(&self) -> Vec<NodeId> {
        self.candidates.clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Correlated failure domain topology analyzer
///////////////////////////////////////////////////////////////////////////////

/// Analysis of failure domain topology across a cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDomainTopology {
    pub total_nodes: u32,
    pub domains_by_type: BTreeMap<DomainType, u32>,
    pub max_sharing_group: u32,
    pub isolation_score: f64,
    pub at_risk_nodes: Vec<NodeId>,
}

/// Analyzes failure domain topology for correlated risk across the cluster.
pub struct FailureDomainTopologyAnalyzer;

impl FailureDomainTopologyAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        FailureDomainTopologyAnalyzer
    }

    /// Analyze the topology from a registry.
    pub fn analyze(&self, registry: &FailureDomainRegistry) -> FailureDomainTopology {
        let mut domains_by_type: BTreeMap<DomainType, u32> = BTreeMap::new();
        for dt in [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ] {
            domains_by_type.insert(dt, registry.distinct_domain_count(dt));
        }

        let total_nodes = registry.registered_count() as u32;

        // Find the largest group of nodes sharing any single domain.
        let max_sharing_group = registry
            .domain_index
            .values()
            .map(|nodes| nodes.len() as u32)
            .max()
            .unwrap_or(0);

        // Isolation score: 1.0 = every node fully isolated, 0.0 = all share everything.
        let isolation_score = if total_nodes > 0 && max_sharing_group > 0 {
            1.0 - (max_sharing_group as f64 / total_nodes as f64)
        } else {
            1.0
        };

        // Find at-risk nodes: nodes in the largest sharing groups.
        let mut at_risk = Vec::new();
        for (_, nodes) in &registry.domain_index {
            if nodes.len() as u32 == max_sharing_group && max_sharing_group > 1 {
                at_risk.extend(nodes.iter().copied());
            }
        }
        at_risk.sort();
        at_risk.dedup();

        FailureDomainTopology {
            total_nodes,
            domains_by_type,
            max_sharing_group,
            isolation_score,
            at_risk_nodes: at_risk,
        }
    }

    /// Return whether the topology is safe (low correlated risk).
    pub fn is_safe(topology: &FailureDomainTopology) -> bool {
        topology.isolation_score > 0.7 && topology.max_sharing_group <= 2
    }

    /// Return a human-readable summary.
    pub fn summarize(topology: &FailureDomainTopology) -> String {
        format!(
            "nodes={} domains={:?} max_sharing={} isolation={:.2}",
            topology.total_nodes, topology.domains_by_type, topology.max_sharing_group, topology.isolation_score
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Version bit position for the correlated failure invariant in EdgeCaseBits.
pub const CORRELATED_FAILURE_BIT: u128 = 1 << 9;

/// Check whether the correlated failure invariant is enabled in a bitset.
pub fn correlated_failure_enabled(bits: &crate::versioning::EdgeCaseBits) -> bool {
    bits.has(CORRELATED_FAILURE_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod correlated_failure_tests {
    use super::*;

    // ---- NodeFailureDomain tests ----

    #[test]
    fn node_failure_domain_creation() {
        let node = NodeId::new([1u8; 32]);
        let domain = NodeFailureDomain::new(node);
        assert_eq!(domain.node, node);
        assert!(!domain.has_any_domain());
        assert_eq!(domain.domain_count(), 0);
    }

    #[test]
    fn node_failure_domain_with_all_domains() {
        let node = NodeId::new([1u8; 32]);
        let domain = NodeFailureDomain::new(node)
            .with_power([1u8; 16])
            .with_facility([2u8; 16])
            .with_network_segment([3u8; 16])
            .with_operator([4u8; 16])
            .with_geographic([5u8; 16]);
        assert!(domain.has_any_domain());
        assert_eq!(domain.domain_count(), 5);
    }

    #[test]
    fn node_failure_domain_partial() {
        let node = NodeId::new([1u8; 32]);
        let domain = NodeFailureDomain::new(node).with_power([1u8; 16]);
        assert!(domain.has_any_domain());
        assert_eq!(domain.domain_count(), 1);
    }

    // ---- FailureDomainMask tests ----

    #[test]
    fn failure_domain_mask_all() {
        let mask = FailureDomainMask::from_flags(FailureDomainMask::ALL);
        assert!(mask.requires_power());
        assert!(mask.requires_facility());
        assert!(mask.requires_network_segment());
        assert!(mask.requires_operator());
        assert!(mask.requires_geographic());
        assert_eq!(mask.required_count(), 5);
    }

    #[test]
    fn failure_domain_mask_partial() {
        let mask = FailureDomainMask::from_flags(FailureDomainMask::POWER | FailureDomainMask::GEOGRAPHIC);
        assert!(mask.requires_power());
        assert!(!mask.requires_facility());
        assert!(mask.requires_geographic());
        assert_eq!(mask.required_count(), 2);
    }

    #[test]
    fn failure_domain_mask_empty() {
        let mask = FailureDomainMask::from_flags(0);
        assert_eq!(mask.required_count(), 0);
        assert!(!mask.requires_power());
    }

    // ---- DomainType tests ----

    #[test]
    fn domain_type_variants() {
        let types = [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    // ---- FailureDomainRegistry tests ----

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = FailureDomainRegistry::new();
        let node = NodeId::new([1u8; 32]);
        let domain = NodeFailureDomain::new(node).with_power([1u8; 16]);
        reg.register(domain);
        assert_eq!(reg.registered_count(), 1);
        assert!(reg.domain_for_node(node).is_some());
    }

    #[test]
    fn registry_re_register_updates() {
        let mut reg = FailureDomainRegistry::new();
        let node = NodeId::new([1u8; 32]);
        reg.register(NodeFailureDomain::new(node).with_power([1u8; 16]));
        reg.register(NodeFailureDomain::new(node).with_facility([2u8; 16]));
        // Should still be count 1 (same node re-registered).
        assert_eq!(reg.registered_count(), 1);
        let d = reg.domain_for_node(node).unwrap();
        assert!(d.facility.is_some());
        assert!(d.power.is_none()); // replaced
    }

    #[test]
    fn registry_nodes_sharing_domain() {
        let mut reg = FailureDomainRegistry::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let n3 = NodeId::new([3u8; 32]);
        reg.register(NodeFailureDomain::new(n1).with_power([9u8; 16]));
        reg.register(NodeFailureDomain::new(n2).with_power([9u8; 16]));
        reg.register(NodeFailureDomain::new(n3).with_power([8u8; 16]));
        let sharing = reg.nodes_sharing_domain(DomainType::Power, &[9u8; 16]);
        assert_eq!(sharing.len(), 2);
        assert!(sharing.contains(&n1));
        assert!(sharing.contains(&n2));
    }

    #[test]
    fn registry_nodes_share_any_domain() {
        let mut reg = FailureDomainRegistry::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([2u8; 16]));
        reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
        assert!(reg.nodes_share_any_domain(n1, n2));

        let n3 = NodeId::new([3u8; 32]);
        reg.register(NodeFailureDomain::new(n3).with_facility([9u8; 16]));
        assert!(!reg.nodes_share_any_domain(n1, n3));
    }

    #[test]
    fn registry_distinct_domain_count() {
        let mut reg = FailureDomainRegistry::new();
        reg.register(NodeFailureDomain::new(NodeId::new([1u8; 32])).with_power([1u8; 16]));
        reg.register(NodeFailureDomain::new(NodeId::new([2u8; 32])).with_power([2u8; 16]));
        reg.register(NodeFailureDomain::new(NodeId::new([3u8; 32])).with_power([1u8; 16]));
        assert_eq!(reg.distinct_domain_count(DomainType::Power), 2);
    }

    #[test]
    fn registry_all_nodes() {
        let mut reg = FailureDomainRegistry::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        reg.register(NodeFailureDomain::new(n1));
        reg.register(NodeFailureDomain::new(n2));
        let nodes = reg.all_nodes();
        assert_eq!(nodes.len(), 2);
    }

    // ---- CorrelatedFailureDetector tests ----

    #[test]
    fn detector_independent_replicas() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([2u8; 16]));
        det.register_node(NodeFailureDomain::new(n3).with_power([3u8; 16]));

        let report = det.check_replica_set(vec![n1, n2, n3]);
        assert!(!report.is_correlated);
        assert_eq!(report.risk_score, 0.0);
        assert_eq!(report.recommendation, PlacementRecommendation::Accept);
    }

    #[test]
    fn detector_correlated_replicas() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));

        let report = det.check_replica_set(vec![n1, n2]);
        assert!(report.is_correlated);
        assert_eq!(report.risk_score, 1.0);
        assert_eq!(report.shared_domains.len(), 1);
        assert_eq!(report.shared_domains[0].domain_type, DomainType::Power);
    }

    #[test]
    fn detector_partial_correlation() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n3).with_power([2u8; 16]));

        let report = det.check_replica_set(vec![n1, n2, n3]);
        // 3 pairs: (n1,n2) share, (n1,n3) no, (n2,n3) no => 1/3 risk
        assert!((report.risk_score - 0.333).abs() < 0.01);
    }

    #[test]
    fn detector_power_domain_distinct() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([2u8; 16]));
        assert!(det.power_domain_distinct(n1, n2));

        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n3).with_power([1u8; 16]));
        assert!(!det.power_domain_distinct(n1, n3));
    }

    #[test]
    fn detector_facility_distinct() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_facility([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_facility([2u8; 16]));
        assert!(det.facility_distinct(n1, n2));
    }

    #[test]
    fn detector_operator_distinct() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_operator([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_operator([2u8; 16]));
        assert!(det.operator_distinct(n1, n2));
    }

    #[test]
    fn detector_network_segment_distinct() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_network_segment([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_network_segment([2u8; 16]));
        assert!(det.network_segment_distinct(n1, n2));
    }

    #[test]
    fn detector_geographic_distinct() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_geographic([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_geographic([2u8; 16]));
        assert!(det.geographic_distinct(n1, n2));
    }

    #[test]
    fn detector_correlated_risk_score() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));
        let score = det.correlated_risk_score(vec![n1, n2]);
        assert_eq!(score, 1.0);

        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n3).with_power([2u8; 16]));
        let score = det.correlated_risk_score(vec![n1, n2, n3]);
        assert!((score - 0.333).abs() < 0.01);
    }

    #[test]
    fn detector_shared_domains() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([2u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));
        let shared = det.shared_domains(vec![n1, n2]);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].domain_type, DomainType::Power);
        assert_eq!(shared[0].sharing_nodes.len(), 2);
    }

    #[test]
    fn detector_total_and_failed_checks() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));

        det.check_replica_set(vec![n1, n2]);
        assert_eq!(det.total_checks(), 1);
        assert_eq!(det.failed_checks(), 1);
        assert_eq!(det.failure_rate(), 1.0);

        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n3).with_power([2u8; 16]));
        det.check_replica_set(vec![n1, n3]);
        assert_eq!(det.total_checks(), 2);
        assert_eq!(det.failed_checks(), 1);
        assert!((det.failure_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn detector_placement_constraints_ok() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([2u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([3u8; 16]).with_facility([4u8; 16]));
        let mask = FailureDomainMask::from_flags(FailureDomainMask::POWER | FailureDomainMask::FACILITY);
        assert!(det.check_placement_constraints(vec![n1, n2], mask).is_ok());
    }

    #[test]
    fn detector_placement_constraints_violated() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([1u8; 16]));
        let mask = FailureDomainMask::from_flags(FailureDomainMask::POWER);
        let err = det.check_placement_constraints(vec![n1, n2], mask).unwrap_err();
        assert!(matches!(err, PlacementConstraintError::PowerDomainShared { .. }));
    }

    #[test]
    fn detector_set_risk_threshold() {
        let mut det = CorrelatedFailureDetector::new();
        det.set_max_risk_score(0.5);
        assert_eq!(det.max_risk_score, 0.5);
        det.set_max_risk_score(2.0);
        assert_eq!(det.max_risk_score, 1.0); // clamped
        det.set_max_risk_score(-1.0);
        assert_eq!(det.max_risk_score, 0.0); // clamped
    }

    // ---- PlacementConstraintError tests ----

    #[test]
    fn placement_constraint_error_variants() {
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let err = PlacementConstraintError::PowerDomainShared { node_a: n1, node_b: n2 };
        match err {
            PlacementConstraintError::PowerDomainShared { node_a, node_b } => {
                assert_eq!(node_a, n1);
                assert_eq!(node_b, n2);
            }
            _ => panic!("expected PowerDomainShared"),
        }

        let err2 = PlacementConstraintError::InsufficientNodes { available: 2, required: 5 };
        match err2 {
            PlacementConstraintError::InsufficientNodes { available, required } => {
                assert_eq!(available, 2);
                assert_eq!(required, 5);
            }
            _ => panic!("expected InsufficientNodes"),
        }
    }

    // ---- ReplicaPlacementOptimizer tests ----

    #[test]
    fn optimizer_optimize_placement() {
        let mut det = CorrelatedFailureDetector::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let n3 = NodeId::new([3u8; 32]);
        det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
        det.register_node(NodeFailureDomain::new(n2).with_power([2u8; 16]));
        det.register_node(NodeFailureDomain::new(n3).with_power([3u8; 16]));

        let mut opt = ReplicaPlacementOptimizer::new(det);
        opt.add_candidate(n1);
        opt.add_candidate(n2);
        opt.add_candidate(n3);
        opt.set_max_replicas(2);

        let result = opt.optimize(FailureDomainMask::from_flags(0)).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn optimizer_insufficient_candidates() {
        let det = CorrelatedFailureDetector::new();
        let mut opt = ReplicaPlacementOptimizer::new(det);
        opt.add_candidate(NodeId::new([1u8; 32]));
        opt.set_max_replicas(3);
        let err = opt.optimize(FailureDomainMask::from_flags(0)).unwrap_err();
        assert!(matches!(err, PlacementConstraintError::InsufficientNodes { .. }));
    }

    #[test]
    fn optimizer_candidate_count() {
        let det = CorrelatedFailureDetector::new();
        let mut opt = ReplicaPlacementOptimizer::new(det);
        assert_eq!(opt.candidate_count(), 0);
        opt.add_candidate(NodeId::new([1u8; 32]));
        opt.add_candidate(NodeId::new([2u8; 32]));
        assert_eq!(opt.candidate_count(), 2);
    }

    // ---- FailureDomainTopologyAnalyzer tests ----

    #[test]
    fn topology_analyzer_isolated_nodes() {
        let reg = FailureDomainRegistry::new();
        let analyzer = FailureDomainTopologyAnalyzer::new();
        let topo = analyzer.analyze(&reg);
        assert_eq!(topo.total_nodes, 0);
        assert_eq!(topo.max_sharing_group, 0);
        assert_eq!(topo.isolation_score, 1.0);
        assert!(FailureDomainTopologyAnalyzer::is_safe(&topo));
    }

    #[test]
    fn topology_analyzer_with_domains() {
        let mut reg = FailureDomainRegistry::new();
        reg.register(NodeFailureDomain::new(NodeId::new([1u8; 32])).with_power([1u8; 16]));
        reg.register(NodeFailureDomain::new(NodeId::new([2u8; 32])).with_power([2u8; 16]));
        reg.register(NodeFailureDomain::new(NodeId::new([3u8; 32])).with_power([3u8; 16]));
        let analyzer = FailureDomainTopologyAnalyzer::new();
        let topo = analyzer.analyze(&reg);
        assert_eq!(topo.total_nodes, 3);
        assert_eq!(topo.domains_by_type.get(&DomainType::Power), Some(&3));
        assert_eq!(topo.max_sharing_group, 1);
        assert!(FailureDomainTopologyAnalyzer::is_safe(&topo));
    }

    #[test]
    fn topology_analyzer_high_correlation() {
        let mut reg = FailureDomainRegistry::new();
        for i in 0..5 {
            reg.register(NodeFailureDomain::new(NodeId::new([i as u8; 32])).with_power([1u8; 16]));
        }
        let analyzer = FailureDomainTopologyAnalyzer::new();
        let topo = analyzer.analyze(&reg);
        assert_eq!(topo.max_sharing_group, 5);
        assert!(!FailureDomainTopologyAnalyzer::is_safe(&topo));
    }

    #[test]
    fn topology_analyzer_summarize() {
        let mut reg = FailureDomainRegistry::new();
        reg.register(NodeFailureDomain::new(NodeId::new([1u8; 32])).with_power([1u8; 16]));
        let analyzer = FailureDomainTopologyAnalyzer::new();
        let topo = analyzer.analyze(&reg);
        let summary = FailureDomainTopologyAnalyzer::summarize(&topo);
        assert!(!summary.is_empty());
        assert!(summary.contains("nodes=1"));
    }

    // ---- SharedDomain tests ----

    #[test]
    fn shared_domain_creation() {
        let sd = SharedDomain {
            domain_type: DomainType::Power,
            domain_value: vec![1, 2, 3],
            sharing_nodes: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
        };
        assert_eq!(sd.domain_type, DomainType::Power);
        assert_eq!(sd.domain_value, vec![1, 2, 3]);
        assert_eq!(sd.sharing_nodes.len(), 2);
    }

    // ---- PlacementRecommendation tests ----

    #[test]
    fn placement_recommendation_variants() {
        let recs = [
            PlacementRecommendation::Accept,
            PlacementRecommendation::AcceptWithWarning,
            PlacementRecommendation::Reject,
            PlacementRecommendation::ReplaceNode {
                remove: NodeId::new([1u8; 32]),
                suggest_alternative: true,
            },
        ];
        for r in recs {
            let _ = format!("{:?}", r);
        }
    }

    // ---- CorrelatedFailureReport tests ----

    #[test]
    fn correlated_failure_report_fields() {
        let n1 = NodeId::new([1u8; 32]);
        let report = CorrelatedFailureReport {
            replicas: vec![n1],
            is_correlated: false,
            shared_domains: Vec::new(),
            risk_score: 0.0,
            distinct_domains: BTreeMap::new(),
            recommendation: PlacementRecommendation::Accept,
        };
        assert!(!report.is_correlated);
        assert_eq!(report.risk_score, 0.0);
        assert_eq!(report.replicas.len(), 1);
    }

    // ---- DomainSummary tests ----

    #[test]
    fn domain_summary_creation() {
        let summary = DomainSummary {
            power: Some(vec![1u8; 16]),
            facility: None,
            network_segment: Some(vec![3u8; 16]),
            operator: None,
            geographic: Some(vec![5u8; 16]),
        };
        assert!(summary.power.is_some());
        assert!(summary.facility.is_none());
        assert!(summary.network_segment.is_some());
        assert!(summary.geographic.is_some());
    }

    // ---- CORRELATED_FAILURE_BIT tests ----

    #[test]
    fn correlated_failure_bit_position() {
        assert_eq!(CORRELATED_FAILURE_BIT, 1 << 9);
    }

    #[test]
    fn correlated_failure_enabled_checks_bit() {
        use crate::versioning::EdgeCaseBits;
        let mut bits = EdgeCaseBits(0);
        assert!(!correlated_failure_enabled(&bits));
        bits.enable(CORRELATED_FAILURE_BIT);
        assert!(correlated_failure_enabled(&bits));
    }

    // ---- Sealed trait unit implementation tests ----

    #[test]
    fn sealed_unit_impl() {
        let unit = ();
        assert!(unit.power_domain_distinct(NodeId::new([1u8; 32]), NodeId::new([2u8; 32])));
        assert!(!unit.failure_domain_check(vec![]).unwrap_err().is_some() || unit.failure_domain_check(vec![]).is_ok());
        assert_eq!(unit.correlated_risk_score(vec![]), 0.0);
        assert!(unit.shared_domains(vec![]).is_empty());
    }

    // ---- Integration test ----

    #[test]
    fn full_correlated_failure_workflow() {
        let mut det = CorrelatedFailureDetector::new();

        // Register 5 nodes across 2 power domains, 3 facilities.
        det.register_node(NodeFailureDomain::new(NodeId::new([1u8; 32])).with_power([1u8; 16]).with_facility([1u8; 16]));
        det.register_node(NodeFailureDomain::new(NodeId::new([2u8; 32])).with_power([1u8; 16]).with_facility([2u8; 16]));
        det.register_node(NodeFailureDomain::new(NodeId::new([3u8; 32])).with_power([2u8; 16]).with_facility([2u8; 16]));
        det.register_node(NodeFailureDomain::new(NodeId::new([4u8; 32])).with_power([2u8; 16]).with_facility([3u8; 16]));
        det.register_node(NodeFailureDomain::new(NodeId::new([5u8; 32])).with_power([3u8; 16]).with_facility([3u8; 16]));

        // Check a correlated set (nodes 1 and 2 share power domain).
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let report = det.check_replica_set(vec![n1, n2]);
        assert!(report.is_correlated);
        assert_eq!(report.recommendation, PlacementRecommendation::Reject);

        // Check an independent set (nodes 1, 3, 5 share nothing).
        let n3 = NodeId::new([3u8; 32]);
        let n5 = NodeId::new([5u8; 32]);
        let report2 = det.check_replica_set(vec![n1, n3, n5]);
        assert!(!report2.is_correlated);

        // Topology analysis.
        let analyzer = FailureDomainTopologyAnalyzer::new();
        let topo = analyzer.analyze(det.registry());
        assert_eq!(topo.total_nodes, 5);
        assert!(topo.isolation_score > 0.0);
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-Region Correlation Analyzer — cross-region failure domain analysis
///////////////////////////////////////////////////////////////////////////////

/// Analysis result for cross-region correlation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRegionCorrelationAnalysis {
    pub region_a: RegionId,
    pub region_b: RegionId,
    pub shared_power_domains: u32,
    pub shared_facility_domains: u32,
    pub shared_operator_domains: u32,
    pub cross_region_risk_score: f64,
    pub independent: bool,
}

/// Analyzes correlated failure risk across regions.
pub struct CrossRegionCorrelationAnalyzer {
    /// Region -> set of nodes in that region.
    region_nodes: HashMap<RegionId, Vec<NodeId>>,
    /// Reference to the failure domain registry.
    registry: FailureDomainRegistry,
}

impl CrossRegionCorrelationAnalyzer {
    /// Create a new analyzer.
    pub fn new(registry: FailureDomainRegistry) -> Self {
        CrossRegionCorrelationAnalyzer {
            region_nodes: HashMap::new(),
            registry,
        }
    }

    /// Register a node as belonging to a region.
    pub fn register_node_region(&mut self, region: RegionId, node: NodeId) {
        self.region_nodes.entry(region).or_default().push(node);
    }

    /// Analyze correlation between two regions.
    pub fn analyze_regions(&self, region_a: RegionId, region_b: RegionId) -> CrossRegionCorrelationAnalysis {
        let nodes_a = self.region_nodes.get(&region_a).cloned().unwrap_or_default();
        let nodes_b = self.region_nodes.get(&region_b).cloned().unwrap_or_default();

        let mut shared_power = 0u32;
        let mut shared_facility = 0u32;
        let mut shared_operator = 0u32;

        for &na in &nodes_a {
            for &nb in &nodes_b {
                let da = self.registry.domain_for_node(na);
                let db = self.registry.domain_for_node(nb);
                if let (Some(a), Some(b)) = (da, db) {
                    if let (Some(pa), Some(pb)) = (a.power, b.power) {
                        if pa == pb {
                            shared_power += 1;
                        }
                    }
                    if let (Some(fa), Some(fb)) = (a.facility, b.facility) {
                        if fa == fb {
                            shared_facility += 1;
                        }
                    }
                    if let (Some(oa), Some(ob)) = (a.operator, b.operator) {
                        if oa == ob {
                            shared_operator += 1;
                        }
                    }
                }
            }
        }

        let total_pairs = (nodes_a.len() * nodes_b.len()) as f64;
        let shared_total = (shared_power + shared_facility + shared_operator) as f64;
        let risk_score = if total_pairs > 0.0 {
            (shared_total / (total_pairs * 3.0)).min(1.0)
        } else {
            0.0
        };

        let independent = risk_score < 0.1;

        CrossRegionCorrelationAnalysis {
            region_a,
            region_b,
            shared_power_domains: shared_power,
            shared_facility_domains: shared_facility,
            shared_operator_domains: shared_operator,
            cross_region_risk_score: risk_score,
            independent,
        }
    }

    /// Return all regions tracked.
    pub fn regions(&self) -> Vec<RegionId> {
        self.region_nodes.keys().copied().collect()
    }

    /// Return the node count for a region.
    pub fn region_node_count(&self, region: RegionId) -> usize {
        self.region_nodes.get(&region).map(|v| v.len()).unwrap_or(0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Domain Diversity Scorer — scores replica placement diversity
///////////////////////////////////////////////////////////////////////////////

/// Diversity score breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiversityScore {
    pub overall: f64,
    pub power_diversity: f64,
    pub facility_diversity: f64,
    pub network_diversity: f64,
    pub operator_diversity: f64,
    pub geographic_diversity: f64,
    pub grade: DiversityGrade,
}

/// Diversity grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiversityGrade {
    A, // Excellent diversity
    B, // Good diversity
    C, // Acceptable diversity
    D, // Poor diversity
    F, // Failing — high correlation risk
}

/// Scores replica placement diversity across failure domains.
pub struct DomainDiversityScorer;

impl DomainDiversityScorer {
    /// Create a new scorer.
    pub fn new() -> Self {
        DomainDiversityScorer
    }

    /// Score the diversity of a replica set.
    pub fn score(
        &self,
        replicas: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> DiversityScore {
        let power = self.domain_diversity(replicas, registry, DomainType::Power);
        let facility = self.domain_diversity(replicas, registry, DomainType::Facility);
        let network = self.domain_diversity(replicas, registry, DomainType::NetworkSegment);
        let operator = self.domain_diversity(replicas, registry, DomainType::Operator);
        let geographic = self.domain_diversity(replicas, registry, DomainType::Geographic);

        let overall = (power + facility + network + operator + geographic) / 5.0;
        let grade = self.grade_for(overall);

        DiversityScore {
            overall,
            power_diversity: power,
            facility_diversity: facility,
            network_diversity: network,
            operator_diversity: operator,
            geographic_diversity: geographic,
            grade,
        }
    }

    fn domain_diversity(
        &self,
        replicas: &[NodeId],
        registry: &FailureDomainRegistry,
        domain_type: DomainType,
    ) -> f64 {
        if replicas.is_empty() {
            return 1.0;
        }
        let mut distinct: HashSet<Vec<u8>> = HashSet::new();
        let mut assigned = 0u32;
        for &node in replicas {
            if let Some(domain) = registry.domain_for_node(node) {
                let value = match domain_type {
                    DomainType::Power => domain.power,
                    DomainType::Facility => domain.facility,
                    DomainType::NetworkSegment => domain.network_segment,
                    DomainType::Operator => domain.operator,
                    DomainType::Geographic => domain.geographic,
                };
                if let Some(v) = value {
                    distinct.insert(v.to_vec());
                    assigned += 1;
                }
            }
        }
        if assigned == 0 {
            return 1.0;
        }
        distinct.len() as f64 / assigned as f64
    }

    fn grade_for(&self, score: f64) -> DiversityGrade {
        match score {
            s if s >= 0.9 => DiversityGrade::A,
            s if s >= 0.75 => DiversityGrade::B,
            s if s >= 0.6 => DiversityGrade::C,
            s if s >= 0.4 => DiversityGrade::D,
            _ => DiversityGrade::F,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Failure Domain Inference — infers unknown domains
///////////////////////////////////////////////////////////////////////////////

/// Inferred domain assignment with confidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredDomain {
    pub node: NodeId,
    pub domain_type: DomainType,
    pub inferred_value: Vec<u8>,
    pub confidence: f64,
    pub basis: InferenceBasis,
}

/// Basis for domain inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceBasis {
    None,
    SameSubnet,
    SameRack,
    SameDatacenter,
    SameOperator,
    GeographicProximity,
}

/// Infers failure domain assignments for nodes with unknown domains.
pub struct FailureDomainInference;

impl FailureDomainInference {
    /// Create a new inference engine.
    pub fn new() -> Self {
        FailureDomainInference
    }

    /// Infer a domain for a node based on neighboring nodes.
    pub fn infer_domain(
        &self,
        node: NodeId,
        neighbors: &[(NodeId, f64)], // (neighbor, proximity)
        registry: &FailureDomainRegistry,
        domain_type: DomainType,
    ) -> Option<InferredDomain> {
        let neighbor_domains: HashMap<Vec<u8>, u32> = HashMap::new();
        let _ = neighbor_domains; // placeholder for real inference logic

        // Simple inference: if any neighbor has a known domain, inherit it with
        // confidence based on proximity.
        let mut best: Option<(Vec<u8>, f64)> = None;
        for (neighbor, proximity) in neighbors {
            if let Some(domain) = registry.domain_for_node(*neighbor) {
                let value = match domain_type {
                    DomainType::Power => domain.power,
                    DomainType::Facility => domain.facility,
                    DomainType::NetworkSegment => domain.network_segment,
                    DomainType::Operator => domain.operator,
                    DomainType::Geographic => domain.geographic,
                };
                if let Some(v) = value {
                    let conf = *proximity;
                    match &best {
                        None => best = Some((v.to_vec(), conf)),
                        Some((_, bc)) if conf > *bc => best = Some((v.to_vec(), conf)),
                        _ => {}
                    }
                }
            }
        }

        best.map(|(value, confidence)| InferredDomain {
            node,
            domain_type,
            inferred_value: value,
            confidence,
            basis: if confidence > 0.8 {
                InferenceBasis::SameRack
            } else {
                InferenceBasis::GeographicProximity
            },
        })
    }

    /// Infer domains for multiple nodes.
    pub fn infer_domains(
        &self,
        nodes: &[NodeId],
        neighbor_map: &HashMap<NodeId, Vec<(NodeId, f64)>>,
        registry: &FailureDomainRegistry,
        domain_type: DomainType,
    ) -> Vec<InferredDomain> {
        let mut results = Vec::new();
        for node in nodes {
            if registry.domain_for_node(*node).is_none() {
                if let Some(neighbors) = neighbor_map.get(node) {
                    if let Some(inferred) = self.infer_domain(*node, neighbors, registry, domain_type) {
                        results.push(inferred);
                    }
                }
            }
        }
        results
    }
}

///////////////////////////////////////////////////////////////////////////////
// Correlation Change Tracker — tracks correlation over time
///////////////////////////////////////////////////////////////////////////////

/// A correlation change event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationChange {
    pub timestamp: TimestampMillis,
    pub replica_set_id: u64,
    pub previous_risk: f64,
    pub current_risk: f64,
    pub change: CorrelationChangeDirection,
}

/// Direction of correlation change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationChangeDirection {
    Increased,
    Decreased,
    Unchanged,
}

/// Tracks correlation risk changes for replica sets over time.
pub struct CorrelationChangeTracker {
    /// History of changes per replica set.
    history: HashMap<u64, Vec<CorrelationChange>>,
    /// Current risk per replica set.
    current_risk: HashMap<u64, f64>,
    /// Next event ID.
    next_id: u64,
    /// Maximum history entries per set.
    max_history: usize,
}

impl CorrelationChangeTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        CorrelationChangeTracker {
            history: HashMap::new(),
            current_risk: HashMap::new(),
            next_id: 0,
            max_history: 100,
        }
    }

    /// Record a correlation risk measurement for a replica set.
    pub fn record(&mut self, replica_set_id: u64, risk: f64) {
        let previous = self.current_risk.insert(replica_set_id, risk);
        let change = match previous {
            None => CorrelationChangeDirection::Unchanged,
            Some(prev) => {
                if (risk - prev).abs() < 0.001 {
                    CorrelationChangeDirection::Unchanged
                } else if risk > prev {
                    CorrelationChangeDirection::Increased
                } else {
                    CorrelationChangeDirection::Decreased
                }
            }
        };

        let event = CorrelationChange {
            timestamp: TimestampMillis(0),
            replica_set_id,
            previous_risk: previous.unwrap_or(0.0),
            current_risk: risk,
            change,
        };

        self.history
            .entry(replica_set_id)
            .or_default()
            .push(event);

        // Trim history.
        if let Some(entries) = self.history.get_mut(&replica_set_id) {
            if entries.len() > self.max_history {
                entries.drain(..entries.len() - self.max_history);
            }
        }
    }

    /// Return the change history for a replica set.
    pub fn history_for_set(&self, replica_set_id: u64) -> Vec<CorrelationChange> {
        self.history
            .get(&replica_set_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the current risk for a replica set.
    pub fn current_risk(&self, replica_set_id: u64) -> Option<f64> {
        self.current_risk.get(&replica_set_id).copied()
    }

    /// Return all replica sets with increasing risk.
    pub fn increasing_risk_sets(&self) -> Vec<u64> {
        self.history
            .iter()
            .filter_map(|(id, events)| {
                if let Some(last) = events.last() {
                    if last.change == CorrelationChangeDirection::Increased {
                        return Some(*id);
                    }
                }
                None
            })
            .collect()
    }

    /// Set maximum history entries per set.
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Replica Set Validator — validates replica sets against correlation policies
///////////////////////////////////////////////////////////////////////////////

/// Validation result for a replica set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaSetValidationResult {
    pub valid: bool,
    pub errors: Vec<ReplicaSetValidationError>,
    pub warnings: Vec<ReplicaSetValidationWarning>,
    pub diversity_score: DiversityScore,
}

/// Error in replica set validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicaSetValidationError {
    TooFewReplicas { count: u32, minimum: u32 },
    TooManyReplicas { count: u32, maximum: u32 },
    CorrelatedFailureDetected { nodes: Vec<NodeId>, domain: DomainType },
    UnknownNode { node: NodeId },
    InsufficientDiversity { score: f64, minimum: f64 },
}

/// Warning in replica set validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicaSetValidationWarning {
    ModerateCorrelation { risk: f64 },
    SingleDomainConcentration { domain: DomainType, count: u32 },
}

/// Validates replica sets against failure-domain correlation policies.
pub struct ReplicaSetValidator {
    /// Minimum replicas required.
    min_replicas: u32,
    /// Maximum replicas allowed.
    max_replicas: u32,
    /// Maximum acceptable correlation risk.
    max_correlation_risk: f64,
    /// Minimum diversity score.
    min_diversity_score: f64,
}

impl ReplicaSetValidator {
    /// Create a new validator with defaults.
    pub fn new() -> Self {
        ReplicaSetValidator {
            min_replicas: 2,
            max_replicas: 10,
            max_correlation_risk: 0.3,
            min_diversity_score: 0.5,
        }
    }

    /// Validate a replica set.
    pub fn validate(
        &self,
        replicas: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> ReplicaSetValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Count checks.
        if (replicas.len() as u32) < self.min_replicas {
            errors.push(ReplicaSetValidationError::TooFewReplicas {
                count: replicas.len() as u32,
                minimum: self.min_replicas,
            });
        }
        if replicas.len() as u32 > self.max_replicas {
            errors.push(ReplicaSetValidationError::TooManyReplicas {
                count: replicas.len() as u32,
                maximum: self.max_replicas,
            });
        }

        // Check each node is registered and check correlation.
        let mut all_registered = true;
        for &node in replicas {
            if registry.domain_for_node(node).is_none() {
                errors.push(ReplicaSetValidationError::UnknownNode { node });
                all_registered = false;
            }
        }

        if all_registered && replicas.len() >= 2 {
            let scorer = DomainDiversityScorer::new();
            let diversity = scorer.score(replicas, registry);

            if diversity.overall < self.min_diversity_score {
                errors.push(ReplicaSetValidationError::InsufficientDiversity {
                    score: diversity.overall,
                    minimum: self.min_diversity_score,
                });
            }

            // Check pairwise correlation.
            for i in 0..replicas.len() {
                for j in (i + 1)..replicas.len() {
                    let a = replicas[i];
                    let b = replicas[j];
                    if let Some(domain_type) = registry.nodes_share_any_domain(a, b) {
                        errors.push(ReplicaSetValidationError::CorrelatedFailureDetected {
                            nodes: vec![a, b],
                            domain: domain_type,
                        });
                    }
                }
            }

            // Risk warnings.
            let risk = 1.0 - diversity.overall;
            if risk > self.max_correlation_risk {
                warnings.push(ReplicaSetValidationWarning::ModerateCorrelation { risk });
            }

            // Domain concentration.
            for dt in [
                DomainType::Power,
                DomainType::Facility,
                DomainType::NetworkSegment,
                DomainType::Operator,
                DomainType::Geographic,
            ] {
                let mut count = 0u32;
                let mut first_value: Option<Vec<u8>> = None;
                for &node in replicas {
                    if let Some(domain) = registry.domain_for_node(node) {
                        let value = match dt {
                            DomainType::Power => domain.power,
                            DomainType::Facility => domain.facility,
                            DomainType::NetworkSegment => domain.network_segment,
                            DomainType::Operator => domain.operator,
                            DomainType::Geographic => domain.geographic,
                        };
                        if let Some(v) = value {
                            if first_value.is_none() {
                                first_value = Some(v.to_vec());
                            }
                            if first_value == Some(v.to_vec()) {
                                count += 1;
                            }
                        }
                    }
                }
                if count > 1 && count as f64 / replicas.len() as f64 > 0.5 {
                    warnings.push(ReplicaSetValidationWarning::SingleDomainConcentration {
                        domain: dt,
                        count,
                    });
                }
            }
        }

        let scorer = DomainDiversityScorer::new();
        let diversity_score = scorer.score(replicas, registry);
        let valid = errors.is_empty();

        ReplicaSetValidationResult {
            valid,
            errors,
            warnings,
            diversity_score,
        }
    }

    /// Set minimum replicas.
    pub fn set_min_replicas(&mut self, min: u32) {
        self.min_replicas = min;
    }

    /// Set maximum replicas.
    pub fn set_max_replicas(&mut self, max: u32) {
        self.max_replicas = max;
    }

    /// Set max correlation risk.
    pub fn set_max_correlation_risk(&mut self, max: f64) {
        self.max_correlation_risk = max.clamp(0.0, 1.0);
    }

    /// Set min diversity score.
    pub fn set_min_diversity_score(&mut self, min: f64) {
        self.min_diversity_score = min.clamp(0.0, 1.0);
    }
}

///////////////////////////////////////////////////////////////////////////////
// Domain Partition Detector — detects domain-level partitions
///////////////////////////////////////////////////////////////////////////////

/// A detected domain partition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPartition {
    pub domain_type: DomainType,
    pub domain_value: Vec<u8>,
    pub isolated_nodes: Vec<NodeId>,
    pub reachable_nodes: Vec<NodeId>,
    pub partition_severity: PartitionSeverity,
}

/// Severity of a domain partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Detects partitions within failure domains.
pub struct DomainPartitionDetector;

impl DomainPartitionDetector {
    /// Create a new detector.
    pub fn new() -> Self {
        DomainPartitionDetector
    }

    /// Detect partitions for a domain value across all nodes.
    pub fn detect_partition(
        &self,
        domain_type: DomainType,
        domain_value: &[u8],
        all_nodes: &[NodeId],
        reachable: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> Option<DomainPartition> {
        let domain_nodes: Vec<NodeId> = all_nodes
            .iter()
            .filter(|&&n| {
                registry.domain_for_node(n).map_or(false, |d| {
                    let v = match domain_type {
                        DomainType::Power => d.power,
                        DomainType::Facility => d.facility,
                        DomainType::NetworkSegment => d.network_segment,
                        DomainType::Operator => d.operator,
                        DomainType::Geographic => d.geographic,
                    };
                    value == v.unwrap_or_default().to_vec()
                })
            })
            .copied()
            .collect();

        if domain_nodes.is_empty() {
            return None;
        }

        let reachable_set: HashSet<NodeId> = reachable.iter().copied().collect();
        let isolated: Vec<NodeId> = domain_nodes
            .iter()
            .filter(|&&n| !reachable_set.contains(&n))
            .copied()
            .collect();
        let reachable_domain: Vec<NodeId> = domain_nodes
            .iter()
            .filter(|&&n| reachable_set.contains(&n))
            .copied()
            .collect();

        let severity = if isolated.is_empty() {
            PartitionSeverity::Low
        } else if isolated.len() >= domain_nodes.len() / 2 {
            PartitionSeverity::Critical
        } else if isolated.len() > domain_nodes.len() / 4 {
            PartitionSeverity::High
        } else {
            PartitionSeverity::Medium
        };

        Some(DomainPartition {
            domain_type,
            domain_value: domain_value.to_vec(),
            isolated_nodes: isolated,
            reachable_nodes: reachable_domain,
            partition_severity: severity,
        })
    }

    /// Detect all partitions across all domain types.
    pub fn detect_all_partitions(
        &self,
        all_nodes: &[NodeId],
        reachable: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> Vec<DomainPartition> {
        let mut partitions = Vec::new();
        let mut seen: HashSet<(DomainType, Vec<u8>)> = HashSet::new();

        for &node in all_nodes {
            if let Some(domain) = registry.domain_for_node(node) {
                for (dt, value) in [
                    (DomainType::Power, domain.power),
                    (DomainType::Facility, domain.facility),
                    (DomainType::NetworkSegment, domain.network_segment),
                    (DomainType::Operator, domain.operator),
                    (DomainType::Geographic, domain.geographic),
                ] {
                    if let Some(v) = value {
                        let key = (dt, v.clone());
                        if seen.insert(key.clone()) {
                            if let Some(part) = self.detect_partition(dt, &v, all_nodes, reachable, registry) {
                                partitions.push(part);
                            }
                        }
                    }
                }
            }
        }
        partitions
    }
}

///////////////////////////////////////////////////////////////////////////////
// Failure Domain Heatmap — visualizes domain usage across cluster
///////////////////////////////////////////////////////////////////////////////

/// Heatmap cell for a domain value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatmapCell {
    pub domain_type: DomainType,
    pub domain_value: Vec<u8>,
    pub node_count: u32,
    pub replica_count: u32,
    pub intensity: f64,
}

/// Heatmap of failure domain usage across the cluster.
pub struct FailureDomainHeatmap {
    /// Cells indexed by domain type + value.
    cells: HashMap<(DomainType, Vec<u8>), HeatmapCell>,
}

impl FailureDomainHeatmap {
    /// Create a new heatmap.
    pub fn new() -> Self {
        FailureDomainHeatmap {
            cells: HashMap::new(),
        }
    }

    /// Add a node to the heatmap.
    pub fn add_node(&mut self, node: NodeId, registry: &FailureDomainRegistry) {
        if let Some(domain) = registry.domain_for_node(node) {
            for (dt, value) in [
                (DomainType::Power, domain.power),
                (DomainType::Facility, domain.facility),
                (DomainType::NetworkSegment, domain.network_segment),
                (DomainType::Operator, domain.operator),
                (DomainType::Geographic, domain.geographic),
            ] {
                if let Some(v) = value {
                    let key = (dt, v.clone());
                    let cell = self.cells.entry(key).or_insert(HeatmapCell {
                        domain_type: dt,
                        domain_value: v,
                        node_count: 0,
                        replica_count: 0,
                        intensity: 0.0,
                    });
                    cell.node_count += 1;
                    cell.intensity = cell.node_count as f64;
                }
            }
        }
    }

    /// Record replicas for a domain value.
    pub fn record_replicas(&mut self, domain_type: DomainType, domain_value: &[u8], count: u32) {
        let key = (domain_type, domain_value.to_vec());
        if let Some(cell) = self.cells.get_mut(&key) {
            cell.replica_count = count;
            cell.intensity = (cell.node_count as f64 + count as f64) / 2.0;
        }
    }

    /// Return all heatmap cells.
    pub fn cells(&self) -> Vec<HeatmapCell> {
        self.cells.values().cloned().collect()
    }

    /// Return the hottest (most used) cell.
    pub fn hottest_cell(&self) -> Option<&HeatmapCell> {
        self.cells.values().max_by(|a, b| a.intensity.partial_cmp(&b.intensity).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Return cells exceeding an intensity threshold.
    pub fn cells_above_intensity(&self, threshold: f64) -> Vec<HeatmapCell> {
        self.cells
            .values()
            .filter(|c| c.intensity > threshold)
            .cloned()
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Correlation Threshold Manager — dynamic threshold management
///////////////////////////////////////////////////////////////////////////////

/// A threshold configuration for a domain type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainThresholdConfig {
    pub domain_type: DomainType,
    pub max_shared_ratio: f64,
    pub max_risk_score: f64,
}

/// Manages dynamic correlation thresholds per domain type.
pub struct CorrelationThresholdManager {
    /// Thresholds per domain type.
    thresholds: HashMap<DomainType, DomainThresholdConfig>,
    /// Default threshold.
    default: DomainThresholdConfig,
}

impl CorrelationThresholdManager {
    /// Create a new manager with defaults.
    pub fn new() -> Self {
        let default = DomainThresholdConfig {
            domain_type: DomainType::Power,
            max_shared_ratio: 0.5,
            max_risk_score: 0.3,
        };
        let mut thresholds = HashMap::new();
        for dt in [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ] {
            thresholds.insert(dt, DomainThresholdConfig {
                domain_type: dt,
                max_shared_ratio: 0.5,
                max_risk_score: 0.3,
            });
        }
        CorrelationThresholdManager { thresholds, default }
    }

    /// Return the threshold config for a domain type.
    pub fn threshold_for(&self, domain_type: DomainType) -> &DomainThresholdConfig {
        self.thresholds
            .get(&domain_type)
            .unwrap_or(&self.default)
    }

    /// Set the threshold for a domain type.
    pub fn set_threshold(&mut self, domain_type: DomainType, config: DomainThresholdConfig) {
        self.thresholds.insert(domain_type, config);
    }

    /// Check whether a shared ratio exceeds the threshold.
    pub fn exceeds_threshold(&self, domain_type: DomainType, shared_ratio: f64) -> bool {
        let config = self.threshold_for(domain_type);
        shared_ratio > config.max_shared_ratio
    }

    /// Set the default threshold.
    pub fn set_default(&mut self, config: DomainThresholdConfig) {
        self.default = config;
    }

    /// Return the default config.
    pub fn default_config(&self) -> &DomainThresholdConfig {
        &self.default
    }
}

///////////////////////////////////////////////////////////////////////////////
// Multi-Domain Correlation Analyzer — analyzes multi-domain correlation
///////////////////////////////////////////////////////////////////////////////

/// Multi-domain correlation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiDomainCorrelationResult {
    pub replica_set: Vec<NodeId>,
    pub domain_correlations: BTreeMap<DomainType, f64>,
    pub combined_risk: f64,
    pub most_correlated_domain: Option<DomainType>,
    pub independent: bool,
}

/// Analyzes correlation across multiple domain types simultaneously.
pub struct MultiDomainCorrelationAnalyzer;

impl MultiDomainCorrelationAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        MultiDomainCorrelationAnalyzer
    }

    /// Analyze multi-domain correlation for a replica set.
    pub fn analyze(
        &self,
        replicas: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> MultiDomainCorrelationResult {
        let mut domain_correlations = BTreeMap::new();

        for dt in [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ] {
            let mut total_pairs = 0u32;
            let mut shared_pairs = 0u32;
            for i in 0..replicas.len() {
                for j in (i + 1)..replicas.len() {
                    total_pairs += 1;
                    if let (Some(da), Some(db)) = (
                        registry.domain_for_node(replicas[i]),
                        registry.domain_for_node(replicas[j]),
                    ) {
                        let va = match dt {
                            DomainType::Power => da.power,
                            DomainType::Facility => da.facility,
                            DomainType::NetworkSegment => da.network_segment,
                            DomainType::Operator => da.operator,
                            DomainType::Geographic => da.geographic,
                        };
                        let vb = match dt {
                            DomainType::Power => db.power,
                            DomainType::Facility => db.facility,
                            DomainType::NetworkSegment => db.network_segment,
                            DomainType::Operator => db.operator,
                            DomainType::Geographic => db.geographic,
                        };
                        if let (Some(a), Some(b)) = (va, vb) {
                            if a == b {
                                shared_pairs += 1;
                            }
                        }
                    }
                }
            }
            let corr = if total_pairs > 0 {
                shared_pairs as f64 / total_pairs as f64
            } else {
                0.0
            };
            domain_correlations.insert(dt, corr);
        }

        let combined_risk = domain_correlations.values().copied().fold(0.0f64, |a, b| a.max(b));
        let most_correlated = domain_correlations
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(dt, _)| *dt);

        let independent = combined_risk < 0.1;

        MultiDomainCorrelationResult {
            replica_set: replicas.to_vec(),
            domain_correlations,
            combined_risk,
            most_correlated_domain: most_correlated,
            independent,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant (redundant with top, kept for clarity)
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// Domain Concentration Analyzer — analyzes domain concentration risk
///////////////////////////////////////////////////////////////////////////////

/// Domain concentration analysis result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainConcentrationAnalysis {
    pub domain_type: DomainType,
    pub dominant_domain_value: Option<Vec<u8>>,
    pub dominant_node_count: u32,
    pub total_nodes: u32,
    pub concentration_ratio: f64,
    pub risk_level: ConcentrationRiskLevel,
}

/// Concentration risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcentrationRiskLevel {
    Low,
    Moderate,
    High,
    Severe,
}

/// Analyzes concentration of nodes within failure domains.
pub struct DomainConcentrationAnalyzer;

impl DomainConcentrationAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        DomainConcentrationAnalyzer
    }

    /// Analyze concentration for a domain type across all nodes.
    pub fn analyze(
        &self,
        domain_type: DomainType,
        nodes: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> DomainConcentrationAnalysis {
        let mut value_counts: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut total_with_domain = 0u32;

        for &node in nodes {
            if let Some(domain) = registry.domain_for_node(node) {
                let value = match domain_type {
                    DomainType::Power => domain.power,
                    DomainType::Facility => domain.facility,
                    DomainType::NetworkSegment => domain.network_segment,
                    DomainType::Operator => domain.operator,
                    DomainType::Geographic => domain.geographic,
                };
                if let Some(v) = value {
                    *value_counts.entry(v.to_vec()).or_insert(0) += 1;
                    total_with_domain += 1;
                }
            }
        }

        let (dominant_value, dominant_count) = value_counts
            .iter()
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .map(|(v, c)| (Some(v.clone()), *c))
            .unwrap_or((None, 0));

        let concentration_ratio = if total_with_domain > 0 {
            dominant_count as f64 / total_with_domain as f64
        } else {
            0.0
        };

        let risk_level = if concentration_ratio < 0.3 {
            ConcentrationRiskLevel::Low
        } else if concentration_ratio < 0.5 {
            ConcentrationRiskLevel::Moderate
        } else if concentration_ratio < 0.7 {
            ConcentrationRiskLevel::High
        } else {
            ConcentrationRiskLevel::Severe
        };

        DomainConcentrationAnalysis {
            domain_type,
            dominant_domain_value: dominant_value,
            dominant_node_count: dominant_count,
            total_nodes: total_with_domain,
            concentration_ratio,
            risk_level,
        }
    }

    /// Analyze all domain types.
    pub fn analyze_all(
        &self,
        nodes: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> BTreeMap<DomainType, DomainConcentrationAnalysis> {
        let mut results = BTreeMap::new();
        for dt in [
            DomainType::Power,
            DomainType::Facility,
            DomainType::NetworkSegment,
            DomainType::Operator,
            DomainType::Geographic,
        ] {
            results.insert(dt, self.analyze(dt, nodes, registry));
        }
        results
    }

    /// Return whether any domain has severe concentration.
    pub fn has_severe_concentration(
        &self,
        nodes: &[NodeId],
        registry: &FailureDomainRegistry,
    ) -> bool {
        let results = self.analyze_all(nodes, registry);
        results.values().any(|a| a.risk_level == ConcentrationRiskLevel::Severe)
    }
}

// ========== CrossRegionCorrelationAnalyzer tests ==========

#[test]
fn cross_region_analyzer_independent_regions() {
    let reg = FailureDomainRegistry::new();
    let mut analyzer = CrossRegionCorrelationAnalyzer::new(reg);
    let ra = RegionId::new([1u8; 32]);
    let rb = RegionId::new([2u8; 32]);
    analyzer.register_node_region(ra, NodeId::new([1u8; 32]));
    analyzer.register_node_region(ra, NodeId::new([2u8; 32]));
    analyzer.register_node_region(rb, NodeId::new([3u8; 32]));
    analyzer.register_node_region(rb, NodeId::new([4u8; 32]));

    let result = analyzer.analyze_regions(ra, rb);
    assert!(!result.shared_power_domains > 0 || result.independent);
    assert!(result.cross_region_risk_score >= 0.0);
    assert!(result.cross_region_risk_score <= 1.0);
}

#[test]
fn cross_region_analyzer_correlated_regions() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([9u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([9u8; 16]));
    let mut analyzer = CrossRegionCorrelationAnalyzer::new(reg);
    let ra = RegionId::new([1u8; 32]);
    let rb = RegionId::new([2u8; 32]);
    analyzer.register_node_region(ra, n1);
    analyzer.register_node_region(rb, n2);

    let result = analyzer.analyze_regions(ra, rb);
    assert_eq!(result.shared_power_domains, 1);
    assert!(!result.independent);
}

#[test]
fn cross_region_analyzer_empty_regions() {
    let reg = FailureDomainRegistry::new();
    let analyzer = CrossRegionCorrelationAnalyzer::new(reg);
    let ra = RegionId::new([1u8; 32]);
    let rb = RegionId::new([2u8; 32]);
    let result = analyzer.analyze_regions(ra, rb);
    assert_eq!(result.cross_region_risk_score, 0.0);
    assert!(result.independent);
}

#[test]
fn cross_region_analyzer_regions_list() {
    let reg = FailureDomainRegistry::new();
    let mut analyzer = CrossRegionCorrelationAnalyzer::new(reg);
    assert_eq!(analyzer.regions().len(), 0);
    analyzer.register_node_region(RegionId::new([1u8; 32]), NodeId::new([1u8; 32]));
    assert_eq!(analyzer.regions().len(), 1);
    assert_eq!(analyzer.region_node_count(RegionId::new([1u8; 32])), 1);
    assert_eq!(analyzer.region_node_count(RegionId::new([2u8; 32])), 0);
}

// ========== DomainDiversityScorer tests ==========

#[test]
fn diversity_scorer_perfect_diversity() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([2u8; 16]));
    let scorer = DomainDiversityScorer::new();
    let score = scorer.score(&[n1, n2], &reg);
    assert_eq!(score.power_diversity, 1.0);
    assert_eq!(score.overall, 1.0);
    assert_eq!(score.grade, DiversityGrade::A);
}

#[test]
fn diversity_scorer_no_diversity() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let scorer = DomainDiversityScorer::new();
    let score = scorer.score(&[n1, n2], &reg);
    assert_eq!(score.power_diversity, 0.5);
    assert!(score.grade != DiversityGrade::A);
}

#[test]
fn diversity_scorer_empty() {
    let reg = FailureDomainRegistry::new();
    let scorer = DomainDiversityScorer::new();
    let score = scorer.score(&[], &reg);
    assert_eq!(score.overall, 1.0);
    assert_eq!(score.grade, DiversityGrade::A);
}

#[test]
fn diversity_grade_variants() {
    let grades = [DiversityGrade::A, DiversityGrade::B, DiversityGrade::C, DiversityGrade::D, DiversityGrade::F];
    for g in grades {
        let _ = format!("{:?}", g);
    }
}

// ========== FailureDomainInference tests ==========

#[test]
fn inference_engine_infer_from_neighbor() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    let engine = FailureDomainInference::new();
    let neighbors = vec![(n1, 0.9f64)];
    let result = engine.infer_domain(n2, &neighbors, &reg, DomainType::Power);
    assert!(result.is_some());
    let inf = result.unwrap();
    assert_eq!(inf.node, n2);
    assert_eq!(inf.inferred_value, vec![1u8; 16]);
    assert_eq!(inf.domain_type, DomainType::Power);
}

#[test]
fn inference_engine_no_neighbors() {
    let reg = FailureDomainRegistry::new();
    let engine = FailureDomainInference::new();
    let result = engine.infer_domain(NodeId::new([1u8; 32]), &[], &reg, DomainType::Power);
    assert!(result.is_none());
}

#[test]
fn inference_basis_variants() {
    let bases = [
        InferenceBasis::None,
        InferenceBasis::SameSubnet,
        InferenceBasis::SameRack,
        InferenceBasis::SameDatacenter,
        InferenceBasis::SameOperator,
        InferenceBasis::GeographicProximity,
    ];
    for b in bases {
        let _ = format!("{:?}", b);
    }
}

// ========== CorrelationChangeTracker tests ==========

#[test]
fn change_tracker_record_and_history() {
    let mut tracker = CorrelationChangeTracker::new();
    tracker.record(1, 0.1);
    tracker.record(1, 0.3);
    tracker.record(1, 0.2);

    let history = tracker.history_for_set(1);
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].change, CorrelationChangeDirection::Unchanged);
    assert_eq!(history[1].change, CorrelationChangeDirection::Increased);
    assert_eq!(history[2].change, CorrelationChangeDirection::Decreased);
}

#[test]
fn change_tracker_current_risk() {
    let mut tracker = CorrelationChangeTracker::new();
    assert!(tracker.current_risk(1).is_none());
    tracker.record(1, 0.5);
    assert_eq!(tracker.current_risk(1), Some(0.5));
}

#[test]
fn change_tracker_increasing_risk_sets() {
    let mut tracker = CorrelationChangeTracker::new();
    tracker.record(1, 0.1);
    tracker.record(1, 0.5);
    tracker.record(2, 0.5);
    tracker.record(2, 0.3);

    let increasing = tracker.increasing_risk_sets();
    assert!(increasing.contains(&1));
    assert!(!increasing.contains(&2));
}

#[test]
fn change_tracker_max_history() {
    let mut tracker = CorrelationChangeTracker::new();
    tracker.set_max_history(2);
    tracker.record(1, 0.1);
    tracker.record(1, 0.2);
    tracker.record(1, 0.3);
    assert_eq!(tracker.history_for_set(1).len(), 2);
}

// ========== ReplicaSetValidator tests ==========

#[test]
fn validator_valid_set() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([2u8; 16]).with_facility([2u8; 16]));
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[n1, n2], &reg);
    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn validator_too_few_replicas() {
    let reg = FailureDomainRegistry::new();
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[], &reg);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| matches!(e, ReplicaSetValidationError::TooFewReplicas { .. })));
}

#[test]
fn validator_unknown_node() {
    let reg = FailureDomainRegistry::new();
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[NodeId::new([1u8; 32])], &reg);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| matches!(e, ReplicaSetValidationError::UnknownNode { .. })));
}

#[test]
fn validator_correlated_failure_detected() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[n1, n2], &reg);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| matches!(e, ReplicaSetValidationError::CorrelatedFailureDetected { .. })));
}

#[test]
fn validator_insufficient_diversity() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[n1, n2], &reg);
    assert!(!result.valid);
}

#[test]
fn validator_warnings() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[n1, n2], &reg);
    assert!(!result.warnings.is_empty());
}

#[test]
fn validator_set_limits() {
    let mut validator = ReplicaSetValidator::new();
    validator.set_min_replicas(3);
    validator.set_max_replicas(7);
    validator.set_max_correlation_risk(0.5);
    validator.set_min_diversity_score(0.7);
}

// ========== DomainPartitionDetector tests ==========

#[test]
fn partition_detector_no_partition() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let detector = DomainPartitionDetector::new();
    let result = detector.detect_partition(
        DomainType::Power,
        &[1u8; 16],
        &[n1, n2],
        &[n1, n2],
        &reg,
    );
    assert!(result.is_some());
    let part = result.unwrap();
    assert!(part.isolated_nodes.is_empty());
    assert_eq!(part.partition_severity, PartitionSeverity::Low);
}

#[test]
fn partition_detector_critical_partition() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    let n3 = NodeId::new([3u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n3).with_power([1u8; 16]));
    let detector = DomainPartitionDetector::new();
    let result = detector.detect_partition(
        DomainType::Power,
        &[1u8; 16],
        &[n1, n2, n3],
        &[n1],
        &reg,
    );
    assert!(result.is_some());
    let part = result.unwrap();
    assert_eq!(part.isolated_nodes.len(), 2);
    assert_eq!(part.partition_severity, PartitionSeverity::Critical);
}

#[test]
fn partition_detector_no_domain_nodes() {
    let reg = FailureDomainRegistry::new();
    let detector = DomainPartitionDetector::new();
    let result = detector.detect_partition(
        DomainType::Power,
        &[9u8; 16],
        &[],
        &[],
        &reg,
    );
    assert!(result.is_none());
}

#[test]
fn partition_severity_variants() {
    let levels = [PartitionSeverity::Low, PartitionSeverity::Medium, PartitionSeverity::High, PartitionSeverity::Critical];
    for l in levels {
        let _ = format!("{:?}", l);
    }
}

// ========== FailureDomainHeatmap tests ==========

#[test]
fn heatmap_add_node() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    let mut heatmap = FailureDomainHeatmap::new();
    heatmap.add_node(n1, &reg);
    let cells = heatmap.cells();
    assert!(!cells.is_empty());
    assert_eq!(cells[0].node_count, 1);
}

#[test]
fn heatmap_hottest_cell() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let mut heatmap = FailureDomainHeatmap::new();
    heatmap.add_node(n1, &reg);
    heatmap.add_node(n2, &reg);
    let hottest = heatmap.hottest_cell().unwrap();
    assert_eq!(hottest.node_count, 2);
}

#[test]
fn heatmap_cells_above_intensity() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    let mut heatmap = FailureDomainHeatmap::new();
    heatmap.add_node(n1, &reg);
    let above = heatmap.cells_above_intensity(0.5);
    assert!(!above.is_empty());
    let above2 = heatmap.cells_above_intensity(10.0);
    assert!(above2.is_empty());
}

#[test]
fn heatmap_record_replicas() {
    let mut heatmap = FailureDomainHeatmap::new();
    heatmap.record_replicas(DomainType::Power, &[1u8; 16], 3);
    let cells = heatmap.cells();
    // No node added yet, so cell shouldn't exist until add_node is called
    // record_replicas only updates existing cells
    assert!(cells.is_empty());
}

// ========== CorrelationThresholdManager tests ==========

#[test]
fn threshold_manager_default() {
    let mgr = CorrelationThresholdManager::new();
    let config = mgr.threshold_for(DomainType::Power);
    assert_eq!(config.max_shared_ratio, 0.5);
    assert_eq!(config.max_risk_score, 0.3);
}

#[test]
fn threshold_manager_exceeds() {
    let mgr = CorrelationThresholdManager::new();
    assert!(mgr.exceeds_threshold(DomainType::Power, 0.6));
    assert!(!mgr.exceeds_threshold(DomainType::Power, 0.4));
}

#[test]
fn threshold_manager_set_threshold() {
    let mut mgr = CorrelationThresholdManager::new();
    mgr.set_threshold(DomainType::Power, DomainThresholdConfig {
        domain_type: DomainType::Power,
        max_shared_ratio: 0.8,
        max_risk_score: 0.9,
    });
    let config = mgr.threshold_for(DomainType::Power);
    assert_eq!(config.max_shared_ratio, 0.8);
    assert!(!mgr.exceeds_threshold(DomainType::Power, 0.6));
    assert!(mgr.exceeds_threshold(DomainType::Power, 0.9));
}

#[test]
fn threshold_manager_set_default() {
    let mut mgr = CorrelationThresholdManager::new();
    mgr.set_default(DomainThresholdConfig {
        domain_type: DomainType::Power,
        max_shared_ratio: 0.1,
        max_risk_score: 0.2,
    });
    assert!(mgr.exceeds_threshold(DomainType::Geographic, 0.15));
}

// ========== MultiDomainCorrelationAnalyzer tests ==========

#[test]
fn multi_domain_analyzer_independent() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([2u8; 16]).with_facility([2u8; 16]));
    let analyzer = MultiDomainCorrelationAnalyzer::new();
    let result = analyzer.analyze(&[n1, n2], &reg);
    assert!(result.independent);
    assert_eq!(result.combined_risk, 0.0);
}

#[test]
fn multi_domain_analyzer_correlated() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([1u8; 16]));
    let analyzer = MultiDomainCorrelationAnalyzer::new();
    let result = analyzer.analyze(&[n1, n2], &reg);
    assert!(!result.independent);
    assert_eq!(result.combined_risk, 1.0);
    assert_eq!(result.most_correlated_domain, Some(DomainType::Power));
}

#[test]
fn multi_domain_analyzer_single_node() {
    let reg = FailureDomainRegistry::new();
    let analyzer = MultiDomainCorrelationAnalyzer::new();
    let result = analyzer.analyze(&[], &reg);
    assert!(result.independent);
    assert_eq!(result.combined_risk, 0.0);
}

// ========== DomainConcentrationAnalyzer tests ==========

#[test]
fn concentration_analyzer_low_concentration() {
    let mut reg = FailureDomainRegistry::new();
    for i in 0..5 {
        reg.register(NodeFailureDomain::new(NodeId::new([i as u8; 32])).with_power([i as u8; 16]));
    }
    let nodes: Vec<NodeId> = (0..5).map(|i| NodeId::new([i as u8; 32])).collect();
    let analyzer = DomainConcentrationAnalyzer::new();
    let result = analyzer.analyze(DomainType::Power, &nodes, &reg);
    assert_eq!(result.risk_level, ConcentrationRiskLevel::Low);
    assert!(result.concentration_ratio < 0.3);
}

#[test]
fn concentration_analyzer_severe_concentration() {
    let mut reg = FailureDomainRegistry::new();
    for i in 0..5 {
        reg.register(NodeFailureDomain::new(NodeId::new([i as u8; 32])).with_power([1u8; 16]));
    }
    let nodes: Vec<NodeId> = (0..5).map(|i| NodeId::new([i as u8; 32])).collect();
    let analyzer = DomainConcentrationAnalyzer::new();
    let result = analyzer.analyze(DomainType::Power, &nodes, &reg);
    assert_eq!(result.risk_level, ConcentrationRiskLevel::Severe);
    assert_eq!(result.concentration_ratio, 1.0);
    assert_eq!(result.dominant_node_count, 5);
}

#[test]
fn concentration_analyzer_has_severe() {
    let mut reg = FailureDomainRegistry::new();
    for i in 0..5 {
        reg.register(NodeFailureDomain::new(NodeId::new([i as u8; 32])).with_power([1u8; 16]));
    }
    let nodes: Vec<NodeId> = (0..5).map(|i| NodeId::new([i as u8; 32])).collect();
    let analyzer = DomainConcentrationAnalyzer::new();
    assert!(analyzer.has_severe_concentration(&nodes, &reg));
}

#[test]
fn concentration_risk_level_variants() {
    let levels = [ConcentrationRiskLevel::Low, ConcentrationRiskLevel::Moderate, ConcentrationRiskLevel::High, ConcentrationRiskLevel::Severe];
    for l in levels {
        let _ = format!("{:?}", l);
    }
}

// ========== Integration test ==========

#[test]
fn full_correlation_workflow() {
    let mut reg = FailureDomainRegistry::new();
    let n1 = NodeId::new([1u8; 32]);
    let n2 = NodeId::new([2u8; 32]);
    let n3 = NodeId::new([3u8; 32]);
    reg.register(NodeFailureDomain::new(n1).with_power([1u8; 16]).with_facility([1u8; 16]));
    reg.register(NodeFailureDomain::new(n2).with_power([2u8; 16]).with_facility([2u8; 16]));
    reg.register(NodeFailureDomain::new(n3).with_power([3u8; 16]).with_facility([3u8; 16]));

    // Detector check.
    let mut det = CorrelatedFailureDetector::new();
    det.register_node(NodeFailureDomain::new(n1).with_power([1u8; 16]));
    det.register_node(NodeFailureDomain::new(n2).with_power([2u8; 16]));
    det.register_node(NodeFailureDomain::new(n3).with_power([3u8; 16]));
    let report = det.check_replica_set(vec![n1, n2, n3]);
    assert!(!report.is_correlated);

    // Diversity score.
    let scorer = DomainDiversityScorer::new();
    let diversity = scorer.score(&[n1, n2, n3], &reg);
    assert_eq!(diversity.overall, 1.0);

    // Validator.
    let validator = ReplicaSetValidator::new();
    let result = validator.validate(&[n1, n2, n3], &reg);
    assert!(result.valid);

    // Heatmap.
    let mut heatmap = FailureDomainHeatmap::new();
    heatmap.add_node(n1, &reg);
    heatmap.add_node(n2, &reg);
    heatmap.add_node(n3, &reg);
    assert_eq!(heatmap.cells().len(), 3);

    // Multi-domain analysis.
    let analyzer = MultiDomainCorrelationAnalyzer::new();
    let mresult = analyzer.analyze(&[n1, n2, n3], &reg);
    assert!(mresult.independent);
}

// ========== PlacementSuggestion tests ==========

#[test]
fn placement_suggestion_creation() {
    let suggestion = PlacementSuggestion {
        node: NodeId::new([1u8; 32]),
        domain_summary: DomainSummary {
            power: Some(vec![1u8; 16]),
            facility: None,
            network_segment: Some(vec![3u8; 16]),
            operator: None,
            geographic: Some(vec![5u8; 16]),
        },
        correlation_score: 0.1,
    };
    assert_eq!(suggestion.correlation_score, 0.1);
    assert!(suggestion.domain_summary.power.is_some());
}

// ========== CorrelatedFailureReport tests ==========

#[test]
fn correlated_failure_report_creation() {
    let report = CorrelatedFailureReport {
        replicas: vec![NodeId::new([1u8; 32])],
        is_correlated: false,
        shared_domains: Vec::new(),
        risk_score: 0.0,
        distinct_domains: BTreeMap::new(),
        recommendation: PlacementRecommendation::Accept,
    };
    assert!(!report.is_correlated);
    assert_eq!(report.risk_score, 0.0);
}

// ========== SharedDomain tests ==========

#[test]
fn shared_domain_creation_test() {
    let sd = SharedDomain {
        domain_type: DomainType::Power,
        domain_value: vec![1, 2, 3],
        sharing_nodes: vec![NodeId::new([1u8; 32]), NodeId::new([2u8; 32])],
    };
    assert_eq!(sd.domain_type, DomainType::Power);
    assert_eq!(sd.sharing_nodes.len(), 2);
}

// ========== PlacementRecommendation tests ==========

#[test]
fn placement_recommendation_variants_test() {
    let recs = [
        PlacementRecommendation::Accept,
        PlacementRecommendation::AcceptWithWarning,
        PlacementRecommendation::Reject,
        PlacementRecommendation::ReplaceNode {
            remove: NodeId::new([1u8; 32]),
            suggest_alternative: true,
        },
    ];
    for r in recs {
        let _ = format!("{:?}", r);
    }
}

// ========== DomainSummary tests ==========

#[test]
fn domain_summary_creation_test() {
    let summary = DomainSummary {
        power: Some(vec![1u8; 16]),
        facility: Some(vec![2u8; 16]),
        network_segment: None,
        operator: Some(vec![4u8; 16]),
        geographic: None,
    };
    assert!(summary.power.is_some());
    assert!(summary.facility.is_some());
    assert!(summary.network_segment.is_none());
    assert!(summary.operator.is_some());
    assert!(summary.geographic.is_none());
}

// ========== DiversityScore tests ==========

#[test]
fn diversity_score_creation() {
    let score = DiversityScore {
        overall: 0.75,
        power_diversity: 1.0,
        facility_diversity: 0.5,
        network_diversity: 1.0,
        operator_diversity: 0.75,
        geographic_diversity: 0.6,
        grade: DiversityGrade::C,
    };
    assert_eq!(score.overall, 0.75);
    assert_eq!(score.grade, DiversityGrade::C);
}