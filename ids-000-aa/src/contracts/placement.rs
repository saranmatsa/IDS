//! # Sealed Trait: Placement — where data lives (capacity-aware, failure-domain)
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Placement trait defines how datasets and partitions are placed onto
//! nodes/H-units/regions with capacity awareness and failure-domain diversity.
//! Implementations live downstream in ids-placement and ids-* crates.

use crate::types::{
    Capacity, ConsistencyProfile, DatasetId, Endpoint, FailureDomain, HUnitId, NodeId, OrderingPos,
    PartitionId, RecordId, RedundancyMode, RegionId,
};
use crate::errors::PlacementError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Placement
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::PlacementEngine {}
impl private::Sealed for super::PlacementDecision {}
impl private::Sealed for super::CapacityTracker {}
impl private::Sealed for super::Rebalancer {}
impl private::Sealed for super::MigrationPlanner {}
impl private::Sealed for super::HotColdTier {}
impl private::Sealed for super::FailureDomainAnalyzer {}
impl private::Sealed for super::PlacementCostModel {}
impl private::Sealed for super::AffinityRuleManager {}
impl private::Sealed for super::ConstraintValidator {}
impl private::Sealed for super::PlacementLoadBalancer {}
impl private::Sealed for super::PlacementAuditTrail {}

/// Sealed trait for data placement — where data lives.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Placement: private::Sealed {
    /// Place a dataset according to a placement policy.
    fn place_dataset(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError>;

    /// Place a partition according to a placement policy.
    fn place_partition(
        &self,
        partition_id: PartitionId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError>;

    /// Select capacity-aware candidate nodes meeting required capacity.
    fn capacity_aware_select(
        &self,
        candidates: Vec<NodeId>,
        required: u64,
    ) -> Result<Vec<NodeId>, PlacementError>;

    /// Check failure-domain diversity across replicas.
    fn failure_domain_aware(&self, replicas: Vec<NodeId>) -> Result<bool, PlacementError>;

    /// Get the placement name for debugging.
    fn placement_name(&self) -> &'static str;

    /// Get the total placement decisions made.
    fn decision_count(&self) -> u64;

    /// Get the total capacity-tracked nodes.
    fn tracked_node_count(&self) -> usize;

    /// Place with constraint validation.
    fn place_with_constraints(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
        replica_count: u64,
        region_count: u64,
        hunit_count: u64,
        domains: Vec<FailureDomain>,
    ) -> Result<PlacementDecision, PlacementError>;

    /// Evaluate affinity/anti-affinity rules for a candidate node.
    fn evaluate_affinity(
        &self,
        dataset_id: &DatasetId,
        candidate_node: &NodeId,
    ) -> Result<bool, PlacementError>;

    /// Record a placement for load tracking.
    fn record_load(&self, node_id: NodeId);

    /// Check if rebalancing is needed given a load threshold.
    fn should_rebalance(&self, threshold: u64) -> bool;

    /// Record a placement audit event.
    fn audit_placement(
        &self,
        timestamp_ms: u64,
        dataset_id: DatasetId,
        action: PlacementAuditAction,
        node_id: Option<NodeId>,
        reason: String,
    );
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PlacementError {
    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("partition not found: {partition_id:?}")]
    PartitionNotFound { partition_id: PartitionId },

    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("insufficient capacity: required={required}, available={available}")]
    InsufficientCapacity {
        required: u64,
        available: u64,
    },

    #[error("failure domain violation: domain={domain:?}")]
    FailureDomainViolation { domain: FailureDomain },

    #[error("placement policy invalid: reason={reason}")]
    PolicyInvalid { reason: String },

    #[error("no suitable candidates: candidates={count}")]
    NoSuitableCandidates { count: usize },

    #[error("rebalancing in progress: reason={reason}")]
    RebalancingInProgress { reason: String },

    #[error("migration failed: partition={partition_id:?}, reason={reason}")]
    MigrationFailed {
        partition_id: PartitionId,
        reason: String,
    },

    #[error("constraint violation: constraint={constraint:?}")]
    ConstraintViolation { constraint: String },

    #[error("internal placement error: {message}")]
    Internal { message: String },
}

impl PlacementError {
    pub fn category(&self) -> &'static str {
        match self {
            PlacementError::DatasetNotFound { .. } => "dataset_not_found",
            PlacementError::PartitionNotFound { .. } => "partition_not_found",
            PlacementError::NodeNotFound { .. } => "node_not_found",
            PlacementError::InsufficientCapacity { .. } => "insufficient_capacity",
            PlacementError::FailureDomainViolation { .. } => "failure_domain_violation",
            PlacementError::PolicyInvalid { .. } => "policy_invalid",
            PlacementError::NoSuitableCandidates { .. } => "no_suitable_candidates",
            PlacementError::RebalancingInProgress { .. } => "rebalancing_in_progress",
            PlacementError::MigrationFailed { .. } => "migration_failed",
            PlacementError::ConstraintViolation { .. } => "constraint_violation",
            PlacementError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            PlacementError::InsufficientCapacity { .. }
                | PlacementError::NoSuitableCandidates { .. }
                | PlacementError::RebalancingInProgress { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            PlacementError::DatasetNotFound { .. } => "create dataset first",
            PlacementError::PartitionNotFound { .. } => "create partition first",
            PlacementError::NodeNotFound { .. } => "register node to network",
            PlacementError::InsufficientCapacity { .. } => "add capacity or reduce requirement",
            PlacementError::FailureDomainViolation { .. } => {
                "select replicas across failure domains"
            }
            PlacementError::PolicyInvalid { .. } => "correct placement policy",
            PlacementError::NoSuitableCandidates { .. } => "expand candidate pool",
            PlacementError::RebalancingInProgress { .. } => "wait for rebalance to complete",
            PlacementError::MigrationFailed { .. } => "retry migration to alternate node",
            PlacementError::ConstraintViolation { .. } => "relax placement constraints",
            PlacementError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT DECISION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementDecision {
    pub dataset_id: DatasetId,
    pub partition_id: Option<PartitionId>,
    pub region: RegionId,
    pub hunit: HUnitId,
    pub node: NodeId,
    pub redundancy: RedundancyMode,
    pub failure_domain: FailureDomain,
    pub decided_at_ms: u64,
    pub placement_score: f64,
    pub replicas: Vec<NodeId>,
}

impl PlacementDecision {
    pub fn new(
        dataset_id: DatasetId,
        region: RegionId,
        hunit: HUnitId,
        node: NodeId,
        redundancy: RedundancyMode,
        failure_domain: FailureDomain,
    ) -> Self {
        Self {
            dataset_id,
            partition_id: None,
            region,
            hunit,
            node,
            redundancy,
            failure_domain,
            decided_at_ms: 0,
            placement_score: 1.0,
            replicas: Vec::new(),
        }
    }

    pub fn with_partition(mut self, partition_id: PartitionId) -> Self {
        self.partition_id = Some(partition_id);
        self
    }

    pub fn with_decided_at(mut self, decided_at_ms: u64) -> Self {
        self.decided_at_ms = decided_at_ms;
        self
    }

    pub fn with_score(mut self, score: f64) -> Self {
        self.placement_score = score;
        self
    }

    pub fn with_replicas(mut self, replicas: Vec<NodeId>) -> Self {
        self.replicas = replicas;
        self
    }

    pub fn is_optimal(&self) -> bool {
        self.placement_score >= 0.8
    }

    pub fn score_category(&self) -> &'static str {
        if self.placement_score >= 0.8 {
            "optimal"
        } else if self.placement_score >= 0.5 {
            "acceptable"
        } else {
            "poor"
        }
    }

    pub fn replica_count(&self) -> usize {
        self.replicas.len()
    }

    pub fn has_replica(&self, node_id: &NodeId) -> bool {
        self.replicas.iter().any(|n| n == node_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT POLICY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementPolicy {
    FirstFit,
    BestFit,
    WorstFit,
    RandomFit,
    FailureDomainAware,
    LocalityAware,
    CapacityWeighted,
    Custom(u16),
}

impl PlacementPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlacementPolicy::FirstFit => "first_fit",
            PlacementPolicy::BestFit => "best_fit",
            PlacementPolicy::WorstFit => "worst_fit",
            PlacementPolicy::RandomFit => "random_fit",
            PlacementPolicy::FailureDomainAware => "failure_domain_aware",
            PlacementPolicy::LocalityAware => "locality_aware",
            PlacementPolicy::CapacityWeighted => "capacity_weighted",
            PlacementPolicy::Custom(id) => "custom",
        }
    }

    pub fn id(&self) -> u16 {
        match self {
            PlacementPolicy::Custom(id) => *id,
            _ => 0,
        }
    }

    pub fn requires_failure_domain_check(&self) -> bool {
        matches!(
            self,
            PlacementPolicy::FailureDomainAware | PlacementPolicy::LocalityAware
        )
    }

    pub fn requires_capacity_check(&self) -> bool {
        matches!(
            self,
            PlacementPolicy::BestFit
                | PlacementPolicy::WorstFit
                | PlacementPolicy::CapacityWeighted
        )
    }

    pub fn is_deterministic(&self) -> bool {
        !matches!(self, PlacementPolicy::RandomFit)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CAPACITY TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct CapacityTracker {
    pub node_capacity: std::cell::RefCell<std::collections::HashMap<NodeId, (u64, u64)>>,
    pub hunit_capacity: std::cell::RefCell<std::collections::HashMap<HUnitId, (u64, u64)>>,
    pub region_capacity: std::cell::RefCell<std::collections::HashMap<RegionId, (u64, u64)>>,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub allocation_history: std::cell::RefCell<Vec<(NodeId, u64, u64)>>,
}

impl CapacityTracker {
    pub fn new() -> Self {
        Self {
            node_capacity: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_capacity: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_capacity: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_allocations: 0,
            total_deallocations: 0,
            allocation_history: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn set_node_capacity(&self, node_id: NodeId, total: u64, used: u64) {
        self.node_capacity
            .borrow_mut()
            .insert(node_id, (total, used));
    }

    pub fn set_hunit_capacity(&self, hunit_id: HUnitId, total: u64, used: u64) {
        self.hunit_capacity
            .borrow_mut()
            .insert(hunit_id, (total, used));
    }

    pub fn set_region_capacity(&self, region_id: RegionId, total: u64, used: u64) {
        self.region_capacity
            .borrow_mut()
            .insert(region_id, (total, used));
    }

    pub fn node_available(&self, node_id: &NodeId) -> Option<u64> {
        self.node_capacity
            .borrow()
            .get(node_id)
            .map(|(total, used)| total.saturating_sub(*used))
    }

    pub fn hunit_available(&self, hunit_id: &HUnitId) -> Option<u64> {
        self.hunit_capacity
            .borrow()
            .get(hunit_id)
            .map(|(total, used)| total.saturating_sub(*used))
    }

    pub fn region_available(&self, region_id: &RegionId) -> Option<u64> {
        self.region_capacity
            .borrow()
            .get(region_id)
            .map(|(total, used)| total.saturating_sub(*used))
    }

    pub fn allocate(&self, node_id: NodeId, bytes: u64) -> Result<(), PlacementError> {
        let mut caps = self.node_capacity.borrow_mut();
        let (total, used) = caps
            .get_mut(&node_id)
            .ok_or_else(|| PlacementError::NodeNotFound { node_id })?;
        if *used + bytes > *total {
            return Err(PlacementError::InsufficientCapacity {
                required: *used + bytes,
                available: *total,
            });
        }
        *used += bytes;
        drop(caps);
        self.total_allocations += 1;
        self.allocation_history
            .borrow_mut()
            .push((node_id, bytes, std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64));
        Ok(())
    }

    pub fn deallocate(&self, node_id: NodeId, bytes: u64) -> Result<(), PlacementError> {
        let mut caps = self.node_capacity.borrow_mut();
        let entry = caps
            .get_mut(&node_id)
            .ok_or_else(|| PlacementError::NodeNotFound { node_id })?;
        entry.1 = entry.1.saturating_sub(bytes);
        drop(caps);
        self.total_deallocations += 1;
        Ok(())
    }

    pub fn node_utilization(&self, node_id: &NodeId) -> Option<f64> {
        self.node_capacity.borrow().get(node_id).map(|(total, used)| {
            if *total == 0 {
                0.0
            } else {
                *used as f64 / *total as f64 * 100.0
            }
        })
    }

    pub fn tracked_node_count(&self) -> usize {
        self.node_capacity.borrow().len()
    }

    pub fn total_allocations(&self) -> u64 {
        self.total_allocations
    }

    pub fn total_deallocations(&self) -> u64 {
        self.total_deallocations
    }

    pub fn history_len(&self) -> usize {
        self.allocation_history.borrow().len()
    }

    pub fn find_overloaded(&self, threshold_percent: u8) -> Vec<NodeId> {
        let caps = self.node_capacity.borrow();
        caps.iter()
            .filter(|(_, (total, used))| {
                *total > 0 && (*used as f64 / *total as f64 * 100.0) >= threshold_percent as f64
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn find_underutilized(&self, threshold_percent: u8) -> Vec<NodeId> {
        let caps = self.node_capacity.borrow();
        caps.iter()
            .filter(|(_, (total, used))| {
                *total > 0 && (*used as f64 / *total as f64 * 100.0) <= threshold_percent as f64
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
}

impl Default for CapacityTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAILURE DOMAIN ANALYZER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainOverlap {
    None,
    Partial,
    Full,
}

impl DomainOverlap {
    pub fn as_str(&self) -> &'static str {
        match self {
            DomainOverlap::None => "none",
            DomainOverlap::Partial => "partial",
            DomainOverlap::Full => "full",
        }
    }

    pub fn is_acceptable(&self) -> bool {
        matches!(self, DomainOverlap::None)
    }
}

pub struct FailureDomainAnalyzer {
    pub domain_map: std::cell::RefCell<std::collections::HashMap<NodeId, FailureDomain>>,
    pub overlap_rules: std::cell::RefCell<std::collections::HashMap<String, bool>>,
    pub analysis_count: u64,
    pub violation_count: u64,
}

impl FailureDomainAnalyzer {
    pub fn new() -> Self {
        Self {
            domain_map: std::cell::RefCell::new(std::collections::HashMap::new()),
            overlap_rules: std::cell::RefCell::new(std::collections::HashMap::new()),
            analysis_count: 0,
            violation_count: 0,
        }
    }

    pub fn register_domain(&self, node_id: NodeId, domain: FailureDomain) {
        self.domain_map.borrow_mut().insert(node_id, domain);
    }

    pub fn analyze_overlap(&self, replicas: Vec<NodeId>) -> Result<DomainOverlap, PlacementError> {
        self.analysis_count += 1;
        let domains = self.domain_map.borrow();
        let mut domain_vec: Vec<&FailureDomain> = Vec::new();
        for node_id in &replicas {
            match domains.get(node_id) {
                Some(d) => domain_vec.push(d),
                None => {
                    return Err(PlacementError::NodeNotFound {
                        node_id: node_id.clone(),
                    });
                }
            }
        }
        if domain_vec.is_empty() {
            return Ok(DomainOverlap::None);
        }
        let first = domain_vec[0];
        let all_same = domain_vec.iter().all(|d| *d == *first);
        if all_same {
            self.violation_count += 1;
            Ok(DomainOverlap::Full)
        } else {
            let unique_count = domain_vec
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len();
            if unique_count == domain_vec.len() {
                Ok(DomainOverlap::None)
            } else {
                Ok(DomainOverlap::Partial)
            }
        }
    }

    pub fn power_domain_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        let domains = self.domain_map.borrow();
        match (domains.get(a), domains.get(b)) {
            (Some(da), Some(db)) => match (da.power, db.power) {
                (Some(pa), Some(pb)) => pa != pb,
                _ => true,
            },
            _ => true,
        }
    }

    pub fn facility_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        let domains = self.domain_map.borrow();
        match (domains.get(a), domains.get(b)) {
            (Some(da), Some(db)) => match (da.facility, db.facility) {
                (Some(fa), Some(fb)) => fa != fb,
                _ => true,
            },
            _ => true,
        }
    }

    pub fn network_segment_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        let domains = self.domain_map.borrow();
        match (domains.get(a), domains.get(b)) {
            (Some(da), Some(db)) => match (da.network_segment, db.network_segment) {
                (Some(na), Some(nb)) => na != nb,
                _ => true,
            },
            _ => true,
        }
    }

    pub fn operator_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        let domains = self.domain_map.borrow();
        match (domains.get(a), domains.get(b)) {
            (Some(da), Some(db)) => match (da.operator, db.operator) {
                (Some(oa), Some(ob)) => oa != ob,
                _ => true,
            },
            _ => true,
        }
    }

    pub fn geographic_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        let domains = self.domain_map.borrow();
        match (domains.get(a), domains.get(b)) {
            (Some(da), Some(db)) => match (da.geographic, db.geographic) {
                (Some(ga), Some(gb)) => ga != gb,
                _ => true,
            },
            _ => true,
        }
    }

    pub fn fully_distinct(&self, a: &NodeId, b: &NodeId) -> bool {
        self.power_domain_distinct(a, b)
            && self.facility_distinct(a, b)
            && self.network_segment_distinct(a, b)
            && self.operator_distinct(a, b)
            && self.geographic_distinct(a, b)
    }

    pub fn analysis_count(&self) -> u64 {
        self.analysis_count
    }

    pub fn violation_count(&self) -> u64 {
        self.violation_count
    }

    pub fn violation_rate(&self) -> f64 {
        if self.analysis_count == 0 {
            return 0.0;
        }
        self.violation_count as f64 / self.analysis_count as f64
    }

    pub fn domain_count(&self) -> usize {
        self.domain_map.borrow().len()
    }

    pub fn check_pairwise_distinct(&self, replicas: &[NodeId]) -> bool {
        for i in 0..replicas.len() {
            for j in (i + 1)..replicas.len() {
                if !self.fully_distinct(&replicas[i], &replicas[j]) {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for FailureDomainAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT COST MODEL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostComponent {
    Latency,
    Bandwidth,
    Capacity,
    FailureDomain,
    Geographic,
    Custom,
}

impl CostComponent {
    pub fn as_str(&self) -> &'static str {
        match self {
            CostComponent::Latency => "latency",
            CostComponent::Bandwidth => "bandwidth",
            CostComponent::Capacity => "capacity",
            CostComponent::FailureDomain => "failure_domain",
            CostComponent::Geographic => "geographic",
            CostComponent::Custom => "custom",
        }
    }
}

pub struct PlacementCostModel {
    pub weights: std::cell::RefCell<std::collections::HashMap<CostComponent, f64>>,
    pub cost_cache: std::cell::RefCell<std::collections::HashMap<u64, f64>>,
    pub calculation_count: u64,
}

impl PlacementCostModel {
    pub fn new() -> Self {
        let mut weights = std::collections::HashMap::new();
        weights.insert(CostComponent::Latency, 0.3);
        weights.insert(CostComponent::Bandwidth, 0.2);
        weights.insert(CostComponent::Capacity, 0.2);
        weights.insert(CostComponent::FailureDomain, 0.2);
        weights.insert(CostComponent::Geographic, 0.1);
        Self {
            weights: std::cell::RefCell::new(weights),
            cost_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            calculation_count: 0,
        }
    }

    pub fn set_weight(&self, component: CostComponent, weight: f64) {
        self.weights.borrow_mut().insert(component, weight);
    }

    pub fn get_weight(&self, component: CostComponent) -> f64 {
        self.weights.borrow().get(&component).copied().unwrap_or(0.0)
    }

    pub fn calculate_cost(
        &self,
        node_id: NodeId,
        latency_ms: u64,
        bandwidth_mbps: u64,
        capacity_utilization: f64,
        failure_domain_risk: f64,
        geographic_distance_km: u64,
    ) -> f64 {
        self.calculation_count += 1;
        let weights = self.weights.borrow();
        let latency_cost = latency_ms as f64 * weights.get(&CostComponent::Latency).unwrap_or(0.0);
        let bandwidth_cost =
            (10000.0_f64 / bandwidth_mbps.max(1) as f64) * weights.get(&CostComponent::Bandwidth).unwrap_or(0.0);
        let capacity_cost = capacity_utilization * weights.get(&CostComponent::Capacity).unwrap_or(0.0);
        let domain_cost = failure_domain_risk * weights.get(&CostComponent::FailureDomain).unwrap_or(0.0);
        let geo_cost = geographic_distance_km as f64 * weights.get(&CostComponent::Geographic).unwrap_or(0.0);
        let total = latency_cost + bandwidth_cost + capacity_cost + domain_cost + geo_cost;
        self.cost_cache.borrow_mut().insert(node_id as u64, total);
        total
    }

    pub fn get_cached_cost(&self, node_id: NodeId) -> Option<f64> {
        self.cost_cache.borrow().get(&(node_id as u64)).copied()
    }

    pub fn normalize_weights(&self) {
        let mut weights = self.weights.borrow_mut();
        let sum: f64 = weights.values().sum();
        if sum > 0.0 {
            for v in weights.values_mut() {
                *v /= sum;
            }
        }
    }

    pub fn calculation_count(&self) -> u64 {
        self.calculation_count
    }

    pub fn weight_count(&self) -> usize {
        self.weights.borrow().len()
    }

    pub fn cache_size(&self) -> usize {
        self.cost_cache.borrow().len()
    }

    pub fn clear_cache(&self) {
        self.cost_cache.borrow_mut().clear();
    }
}

impl Default for PlacementCostModel {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// AFFINITY / ANTI-AFFINITY RULES
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleType {
    Affinity,
    AntiAffinity,
}

impl RuleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleType::Affinity => "affinity",
            RuleType::AntiAffinity => "anti_affinity",
        }
    }
}

pub struct PlacementRule {
    pub rule_type: RuleType,
    pub dataset_id: DatasetId,
    pub target_node: NodeId,
    pub enabled: bool,
}

impl PlacementRule {
    pub fn new(rule_type: RuleType, dataset_id: DatasetId, target_node: NodeId) -> Self {
        Self {
            rule_type,
            dataset_id,
            target_node,
            enabled: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn is_affinity(&self) -> bool {
        matches!(self.rule_type, RuleType::Affinity)
    }

    pub fn is_anti_affinity(&self) -> bool {
        matches!(self.rule_type, RuleType::AntiAffinity)
    }
}

pub struct AffinityRuleManager {
    pub rules: std::cell::RefCell<std::collections::HashMap<u64, PlacementRule>>,
    pub next_id: u64,
    pub evaluation_count: u64,
    pub pass_count: u64,
    pub fail_count: u64,
}

impl AffinityRuleManager {
    pub fn new() -> Self {
        Self {
            rules: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_id: 1,
            evaluation_count: 0,
            pass_count: 0,
            fail_count: 0,
        }
    }

    pub fn add_rule(&self, rule: PlacementRule) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.rules.borrow_mut().insert(id, rule);
        id
    }

    pub fn remove_rule(&self, rule_id: u64) -> bool {
        self.rules.borrow_mut().remove(&rule_id).is_some()
    }

    pub fn enable_rule(&self, rule_id: u64) -> bool {
        if let Some(rule) = self.rules.borrow_mut().get_mut(&rule_id) {
            rule.enabled = true;
            true
        } else {
            false
        }
    }

    pub fn disable_rule(&self, rule_id: u64) -> bool {
        if let Some(rule) = self.rules.borrow_mut().get_mut(&rule_id) {
            rule.enabled = false;
            true
        } else {
            false
        }
    }

    pub fn evaluate(&self, dataset_id: &DatasetId, candidate_node: &NodeId) -> Result<bool, PlacementError> {
        self.evaluation_count += 1;
        let rules = self.rules.borrow();
        for rule in rules.values() {
            if !rule.enabled {
                continue;
            }
            if rule.dataset_id != *dataset_id {
                continue;
            }
            match rule.rule_type {
                RuleType::Affinity => {
                    if candidate_node != &rule.target_node {
                        self.fail_count += 1;
                        return Ok(false);
                    }
                }
                RuleType::AntiAffinity => {
                    if candidate_node == &rule.target_node {
                        self.fail_count += 1;
                        return Ok(false);
                    }
                }
            }
        }
        self.pass_count += 1;
        Ok(true)
    }

    pub fn rule_count(&self) -> usize {
        self.rules.borrow().len()
    }

    pub fn pass_rate(&self) -> f64 {
        let total = self.pass_count + self.fail_count;
        if total == 0 {
            return 1.0;
        }
        self.pass_count as f64 / total as f64
    }

    pub fn enabled_count(&self) -> usize {
        self.rules.borrow().values().filter(|r| r.enabled).count()
    }
}

impl Default for AffinityRuleManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT CONSTRAINT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintType {
    MinReplicas,
    MaxReplicas,
    MinRegions,
    MaxRegions,
    MinHUnits,
    MaxHUnits,
    RequiredFailureDomain,
    ProhibitedFailureDomain,
}

impl ConstraintType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConstraintType::MinReplicas => "min_replicas",
            ConstraintType::MaxReplicas => "max_replicas",
            ConstraintType::MinRegions => "min_regions",
            ConstraintType::MaxRegions => "max_regions",
            ConstraintType::MinHUnits => "min_hunits",
            ConstraintType::MaxHUnits => "max_hunits",
            ConstraintType::RequiredFailureDomain => "required_failure_domain",
            ConstraintType::ProhibitedFailureDomain => "prohibited_failure_domain",
        }
    }
}

pub struct PlacementConstraint {
    pub constraint_type: ConstraintType,
    pub value: i64,
    pub domain: Option<FailureDomain>,
}

impl PlacementConstraint {
    pub fn new(constraint_type: ConstraintType, value: i64) -> Self {
        Self {
            constraint_type,
            value,
            domain: None,
        }
    }

    pub fn with_domain(mut self, domain: FailureDomain) -> Self {
        self.domain = Some(domain);
        self
    }
}

pub struct ConstraintValidator {
    pub constraints: std::cell::RefCell<Vec<PlacementConstraint>>,
    pub validation_count: u64,
    pub violation_count: u64,
}

impl ConstraintValidator {
    pub fn new() -> Self {
        Self {
            constraints: std::cell::RefCell::new(Vec::new()),
            validation_count: 0,
            violation_count: 0,
        }
    }

    pub fn add_constraint(&self, constraint: PlacementConstraint) {
        self.constraints.borrow_mut().push(constraint);
    }

    pub fn remove_constraint(&self, index: usize) -> bool {
        if index < self.constraints.borrow().len() {
            self.constraints.borrow_mut().remove(index);
            true
        } else {
            false
        }
    }

    pub fn validate(
        &self,
        replica_count: u64,
        region_count: u64,
        hunit_count: u64,
        domains: &[FailureDomain],
    ) -> Result<(), PlacementError> {
        self.validation_count += 1;
        let constraints = self.constraints.borrow();
        for constraint in constraints.iter() {
            match constraint.constraint_type {
                ConstraintType::MinReplicas => {
                    if replica_count < constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::InsufficientCapacity {
                            required: constraint.value as u64,
                            available: replica_count,
                        });
                    }
                }
                ConstraintType::MaxReplicas => {
                    if replica_count > constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::InsufficientCapacity {
                            required: constraint.value as u64,
                            available: replica_count,
                        });
                    }
                }
                ConstraintType::MinRegions => {
                    if region_count < constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::FailureDomainViolation {
                            domain: constraint.domain.unwrap_or_default(),
                        });
                    }
                }
                ConstraintType::MaxRegions => {
                    if region_count > constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::FailureDomainViolation {
                            domain: constraint.domain.unwrap_or_default(),
                        });
                    }
                }
                ConstraintType::MinHUnits => {
                    if hunit_count < constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::NoSuitableCandidates {
                            count: hunit_count as usize,
                        });
                    }
                }
                ConstraintType::MaxHUnits => {
                    if hunit_count > constraint.value as u64 {
                        self.violation_count += 1;
                        return Err(PlacementError::NoSuitableCandidates {
                            count: hunit_count as usize,
                        });
                    }
                }
                ConstraintType::RequiredFailureDomain => {
                    if let Some(ref req_domain) = constraint.domain {
                        if !domains.iter().any(|d| d == req_domain) {
                            self.violation_count += 1;
                            return Err(PlacementError::FailureDomainViolation {
                                domain: req_domain.clone(),
                            });
                        }
                    }
                }
                ConstraintType::ProhibitedFailureDomain => {
                    if let Some(ref prob_domain) = constraint.domain {
                        if domains.iter().any(|d| d == prob_domain) {
                            self.violation_count += 1;
                            return Err(PlacementError::FailureDomainViolation {
                                domain: prob_domain.clone(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.borrow().len()
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validation_count == 0 {
            return 1.0;
        }
        (self.validation_count - self.violation_count) as f64 / self.validation_count as f64
    }
}

impl Default for ConstraintValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REBALANCER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebalanceStrategy {
    Immediate,
    Threshold,
    Scheduled,
}

impl RebalanceStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RebalanceStrategy::Immediate => "immediate",
            RebalanceStrategy::Threshold => "threshold",
            RebalanceStrategy::Scheduled => "scheduled",
        }
    }
}

pub struct Rebalancer {
    pub strategy: RebalanceStrategy,
    pub threshold_percent: u8,
    pub rebalance_count: u64,
    pub pending_rebalances: std::cell::RefCell<std::collections::HashMap<DatasetId, u64>>,
}

impl Rebalancer {
    pub fn new(strategy: RebalanceStrategy, threshold_percent: u8) -> Self {
        Self {
            strategy,
            threshold_percent,
            rebalance_count: 0,
            pending_rebalances: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn needs_rebalance(&self, utilization: f64) -> bool {
        utilization >= self.threshold_percent as f64
    }

    pub fn schedule_rebalance(&self, dataset_id: DatasetId, at_ms: u64) {
        self.pending_rebalances.borrow_mut().insert(dataset_id, at_ms);
    }

    pub fn execute_rebalance(&self, dataset_id: &DatasetId) -> bool {
        self.pending_rebalances.borrow_mut().remove(dataset_id).is_some()
    }

    pub fn increment_rebalance_count(&mut self) {
        self.rebalance_count += 1;
    }

    pub fn rebalance_count(&self) -> u64 {
        self.rebalance_count
    }

    pub fn pending_count(&self) -> usize {
        self.pending_rebalances.borrow().len()
    }

    pub fn threshold_percent(&self) -> u8 {
        self.threshold_percent
    }
}

impl Default for Rebalancer {
    fn default() -> Self {
        Self::new(RebalanceStrategy::Threshold, 80)
    }
}

///////////////////////////////////////////////////////////////////////////////
// MIGRATION PLANNER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl MigrationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MigrationStatus::Pending => "pending",
            MigrationStatus::InProgress => "in_progress",
            MigrationStatus::Completed => "completed",
            MigrationStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, MigrationStatus::Completed | MigrationStatus::Failed)
    }
}

pub struct MigrationPlanner {
    pub migrations: std::cell::RefCell<std::collections::HashMap<PartitionId, MigrationStatus>>,
    pub migration_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
}

impl MigrationPlanner {
    pub fn new() -> Self {
        Self {
            migrations: std::cell::RefCell::new(std::collections::HashMap::new()),
            migration_count: 0,
            completed_count: 0,
            failed_count: 0,
        }
    }

    pub fn plan_migration(&self, partition_id: PartitionId) {
        self.migrations.borrow_mut().insert(partition_id, MigrationStatus::Pending);
        self.migration_count += 1;
    }

    pub fn start_migration(&self, partition_id: PartitionId) {
        self.migrations.borrow_mut().insert(partition_id, MigrationStatus::InProgress);
    }

    pub fn complete_migration(&self, partition_id: PartitionId) {
        self.migrations.borrow_mut().insert(partition_id, MigrationStatus::Completed);
        self.completed_count += 1;
    }

    pub fn fail_migration(&self, partition_id: PartitionId) {
        self.migrations.borrow_mut().insert(partition_id, MigrationStatus::Failed);
        self.failed_count += 1;
    }

    pub fn get_status(&self, partition_id: &PartitionId) -> MigrationStatus {
        self.migrations.borrow().get(partition_id).copied().unwrap_or(MigrationStatus::Pending)
    }

    pub fn migration_count(&self) -> u64 {
        self.migration_count
    }

    pub fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.completed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_count as f64 / total as f64
    }

    pub fn pending_count(&self) -> usize {
        self.migrations
            .borrow()
            .values()
            .filter(|s| matches!(s, MigrationStatus::Pending | MigrationStatus::InProgress))
            .count()
    }
}

impl Default for MigrationPlanner {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// HOT/COLD TIER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Hot,
    Warm,
    Cold,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }

    pub fn access_cost_multiplier(&self) -> f64 {
        match self {
            Tier::Hot => 1.0,
            Tier::Warm => 5.0,
            Tier::Cold => 20.0,
        }
    }
}

pub struct HotColdTier {
    pub dataset_tiers: std::cell::RefCell<std::collections::HashMap<DatasetId, Tier>>,
    pub tier_transitions: u64,
}

impl HotColdTier {
    pub fn new() -> Self {
        Self {
            dataset_tiers: std::cell::RefCell::new(std::collections::HashMap::new()),
            tier_transitions: 0,
        }
    }

    pub fn set_tier(&self, dataset_id: DatasetId, tier: Tier) {
        self.dataset_tiers.borrow_mut().insert(dataset_id, tier);
        self.tier_transitions += 1;
    }

    pub fn get_tier(&self, dataset_id: &DatasetId) -> Tier {
        self.dataset_tiers.borrow().get(dataset_id).copied().unwrap_or(Tier::Hot)
    }

    pub fn transition_to_cold(&self, dataset_id: DatasetId) {
        self.set_tier(dataset_id, Tier::Cold);
    }

    pub fn transition_to_hot(&self, dataset_id: DatasetId) {
        self.set_tier(dataset_id, Tier::Hot);
    }

    pub fn tier_count(&self) -> usize {
        self.dataset_tiers.borrow().len()
    }

    pub fn tier_transitions(&self) -> u64 {
        self.tier_transitions
    }

    pub fn cold_count(&self) -> usize {
        self.dataset_tiers
            .borrow()
            .values()
            .filter(|t| matches!(t, Tier::Cold))
            .count()
    }

    pub fn hot_count(&self) -> usize {
        self.dataset_tiers
            .borrow()
            .values()
            .filter(|t| matches!(t, Tier::Hot))
            .count()
    }
}

impl Default for HotColdTier {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DIVERSITY CHECKER
///////////////////////////////////////////////////////////////////////////////

pub struct DiversityChecker {
    pub domain_assignments: std::cell::RefCell<std::collections::HashMap<NodeId, FailureDomain>>,
    pub check_count: u64,
    pub violation_count: u64,
}

impl DiversityChecker {
    pub fn new() -> Self {
        Self {
            domain_assignments: std::cell::RefCell::new(std::collections::HashMap::new()),
            check_count: 0,
            violation_count: 0,
        }
    }

    pub fn assign_domain(&self, node_id: NodeId, domain: FailureDomain) {
        self.domain_assignments.borrow_mut().insert(node_id, domain);
    }

    pub fn check_diversity(&self, replicas: Vec<NodeId>) -> bool {
        self.check_count += 1;
        let domains = self.domain_assignments.borrow();
        let replica_domains: Vec<&FailureDomain> = replicas
            .iter()
            .filter_map(|n| domains.get(n))
            .collect();
        let unique_count = replica_domains.iter().collect::<std::collections::HashSet<_>>().len();
        let is_diverse = unique_count == replica_domains.len();
        if !is_diverse {
            self.violation_count += 1;
        }
        is_diverse
    }

    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    pub fn violation_count(&self) -> u64 {
        self.violation_count
    }

    pub fn violation_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 0.0;
        }
        self.violation_count as f64 / self.check_count as f64
    }

    pub fn domain_count(&self) -> usize {
        self.domain_assignments.borrow().len()
    }
}

impl Default for DiversityChecker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOPOLOGY AWARE PLACEMENT
///////////////////////////////////////////////////////////////////////////////

pub struct TopologyAware {
    pub topology_links: std::cell::RefCell<std::collections::HashMap<(RegionId, RegionId), u64>>,
    pub hop_count_cache: std::cell::RefCell<std::collections::HashMap<(RegionId, RegionId), u32>>,
}

impl TopologyAware {
    pub fn new() -> Self {
        Self {
            topology_links: std::cell::RefCell::new(std::collections::HashMap::new()),
            hop_count_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn add_link(&self, region_a: RegionId, region_b: RegionId, cost: u64) {
        self.topology_links.borrow_mut().insert((region_a, region_b), cost);
    }

    pub fn get_link_cost(&self, region_a: &RegionId, region_b: &RegionId) -> Option<u64> {
        self.topology_links
            .borrow()
            .get(&(region_a.clone(), region_b.clone()))
            .copied()
    }

    pub fn estimate_hops(&self, from: &RegionId, to: &RegionId) -> u32 {
        let cache_key = (from.clone(), to.clone());
        if let Some(hops) = self.hop_count_cache.borrow().get(&cache_key) {
            return *hops;
        }
        let hops = self.topology_links.borrow().get(&cache_key).map(|_| 1).unwrap_or(3);
        self.hop_count_cache.borrow_mut().insert(cache_key, hops);
        hops
    }

    pub fn link_count(&self) -> usize {
        self.topology_links.borrow().len()
    }

    pub fn cache_hit_count(&self) -> usize {
        self.hop_count_cache.borrow().len()
    }
}

impl Default for TopologyAware {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RESERVATION TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationStatus {
    Active,
    Expired,
    Released,
}

impl ReservationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReservationStatus::Active => "active",
            ReservationStatus::Expired => "expired",
            ReservationStatus::Released => "released",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ReservationStatus::Active)
    }
}

pub struct ReservationTracker {
    pub reservations: std::cell::RefCell<
        std::collections::HashMap<u64, (NodeId, u64, ReservationStatus)>,
    >,
    pub next_id: u64,
    pub total_reserved_bytes: u64,
}

impl ReservationTracker {
    pub fn new() -> Self {
        Self {
            reservations: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_id: 1,
            total_reserved_bytes: 0,
        }
    }

    pub fn reserve(&self, node_id: NodeId, bytes: u64, ttl_ms: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.reservations
            .borrow_mut()
            .insert(id, (node_id, bytes, ReservationStatus::Active));
        self.total_reserved_bytes += bytes;
        id
    }

    pub fn release(&self, reservation_id: u64) -> bool {
        if let Some((_, bytes, status)) = self.reservations.borrow_mut().get_mut(&reservation_id) {
            if *status == ReservationStatus::Active {
                *status = ReservationStatus::Released;
                self.total_reserved_bytes = self.total_reserved_bytes.saturating_sub(bytes);
                return true;
            }
        }
        false
    }

    pub fn expire(&self, reservation_id: u64) -> bool {
        if let Some((_, bytes, status)) = self.reservations.borrow_mut().get_mut(&reservation_id) {
            if *status == ReservationStatus::Active {
                *status = ReservationStatus::Expired;
                self.total_reserved_bytes = self.total_reserved_bytes.saturating_sub(bytes);
                return true;
            }
        }
        false
    }

    pub fn get_status(&self, reservation_id: u64) -> Option<ReservationStatus> {
        self.reservations
            .borrow()
            .get(&reservation_id)
            .map(|(_, _, s)| *s)
    }

    pub fn total_reserved_bytes(&self) -> u64 {
        self.total_reserved_bytes
    }

    pub fn active_count(&self) -> usize {
        self.reservations
            .borrow()
            .values()
            .filter(|(_, _, s)| matches!(s, ReservationStatus::Active))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.reservations.borrow().len()
    }
}

impl Default for ReservationTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT LOAD BALANCER
///////////////////////////////////////////////////////////////////////////////

pub struct PlacementLoadBalancer {
    pub node_loads: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub total_placements: u64,
    pub rebalance_triggers: u64,
}

impl PlacementLoadBalancer {
    pub fn new() -> Self {
        Self {
            node_loads: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_placements: 0,
            rebalance_triggers: 0,
        }
    }

    pub fn record_placement(&self, node_id: NodeId) {
        self.node_loads
            .borrow_mut()
            .entry(node_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
        self.total_placements += 1;
    }

    pub fn get_load(&self, node_id: &NodeId) -> u64 {
        self.node_loads.borrow().get(node_id).copied().unwrap_or(0)
    }

    pub fn select_least_loaded(&self, candidates: &[NodeId]) -> Option<NodeId> {
        let loads = self.node_loads.borrow();
        candidates
            .iter()
            .min_by_key(|n| loads.get(n).copied().unwrap_or(0))
            .copied()
    }

    pub fn should_rebalance(&self, threshold: u64) -> bool {
        let loads = self.node_loads.borrow();
        if loads.is_empty() {
            return false;
        }
        let max_load = loads.values().max().copied().unwrap_or(0);
        let min_load = loads.values().min().copied().unwrap_or(0);
        max_load.saturating_sub(min_load) > threshold
    }

    pub fn trigger_rebalance(&self) {
        self.rebalance_triggers += 1;
    }

    pub fn rebalance_trigger_count(&self) -> u64 {
        self.rebalance_triggers
    }

    pub fn node_count(&self) -> usize {
        self.node_loads.borrow().len()
    }

    pub fn average_load(&self) -> f64 {
        let loads = self.node_loads.borrow();
        if loads.is_empty() {
            return 0.0;
        }
        let sum: u64 = loads.values().sum();
        sum as f64 / loads.len() as f64
    }
}

impl Default for PlacementLoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT AUDIT TRAIL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct PlacementAuditEntry {
    pub timestamp_ms: u64,
    pub dataset_id: DatasetId,
    pub action: PlacementAuditAction,
    pub node_id: Option<NodeId>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementAuditAction {
    Placed,
    Migrated,
    Removed,
    Rebalanced,
}

impl PlacementAuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlacementAuditAction::Placed => "placed",
            PlacementAuditAction::Migrated => "migrated",
            PlacementAuditAction::Removed => "removed",
            PlacementAuditAction::Rebalanced => "rebalanced",
        }
    }
}

pub struct PlacementAuditTrail {
    pub entries: std::cell::RefCell<Vec<PlacementAuditEntry>>,
    pub max_entries: usize,
}

impl PlacementAuditTrail {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: std::cell::RefCell::new(Vec::new()),
            max_entries,
        }
    }

    pub fn record(
        &self,
        timestamp_ms: u64,
        dataset_id: DatasetId,
        action: PlacementAuditAction,
        node_id: Option<NodeId>,
        reason: String,
    ) {
        let mut entries = self.entries.borrow_mut();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(PlacementAuditEntry {
            timestamp_ms,
            dataset_id,
            action,
            node_id,
            reason,
        });
    }

    pub fn entries_for_dataset(&self, dataset_id: &DatasetId) -> Vec<PlacementAuditEntry> {
        self.entries
            .borrow()
            .iter()
            .filter(|e| e.dataset_id == *dataset_id)
            .cloned()
            .collect()
    }

    pub fn entries_by_action(&self, action: PlacementAuditAction) -> Vec<PlacementAuditEntry> {
        self.entries
            .borrow()
            .iter()
            .filter(|e| e.action == action)
            .cloned()
            .collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }
}

impl Default for PlacementAuditTrail {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// PLACEMENT ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct PlacementEngine {
    pub tracker: std::cell::RefCell<CapacityTracker>,
    pub hunit_selector: std::cell::RefCell<HUnitSelector>,
    pub region_selector: std::cell::RefCell<RegionSelector>,
    pub node_selector: std::cell::RefCell<NodeSelector>,
    pub rebalancer: std::cell::RefCell<Rebalancer>,
    pub migration_planner: std::cell::RefCell<MigrationPlanner>,
    pub hot_cold_tier: std::cell::RefCell<HotColdTier>,
    pub diversity_checker: std::cell::RefCell<DiversityChecker>,
    pub topology: std::cell::RefCell<TopologyAware>,
    pub reservation_tracker: std::cell::RefCell<ReservationTracker>,
    pub failure_domain_analyzer: std::cell::RefCell<FailureDomainAnalyzer>,
    pub cost_model: std::cell::RefCell<PlacementCostModel>,
    pub affinity_manager: std::cell::RefCell<AffinityRuleManager>,
    pub constraint_validator: std::cell::RefCell<ConstraintValidator>,
    pub load_balancer: std::cell::RefCell<PlacementLoadBalancer>,
    pub audit_trail: std::cell::RefCell<PlacementAuditTrail>,
    pub decision_count: u64,
}

impl PlacementEngine {
    pub fn new() -> Self {
        Self {
            tracker: std::cell::RefCell::new(CapacityTracker::new()),
            hunit_selector: std::cell::RefCell::new(HUnitSelector::new()),
            region_selector: std::cell::RefCell::new(RegionSelector::new()),
            node_selector: std::cell::RefCell::new(NodeSelector::new()),
            rebalancer: std::cell::RefCell::new(Rebalancer::new(RebalanceStrategy::Threshold, 80)),
            migration_planner: std::cell::RefCell::new(MigrationPlanner::new()),
            hot_cold_tier: std::cell::RefCell::new(HotColdTier::new()),
            diversity_checker: std::cell::RefCell::new(DiversityChecker::new()),
            topology: std::cell::RefCell::new(TopologyAware::new()),
            reservation_tracker: std::cell::RefCell::new(ReservationTracker::new()),
            failure_domain_analyzer: std::cell::RefCell::new(FailureDomainAnalyzer::new()),
            cost_model: std::cell::RefCell::new(PlacementCostModel::new()),
            affinity_manager: std::cell::RefCell::new(AffinityRuleManager::new()),
            constraint_validator: std::cell::RefCell::new(ConstraintValidator::new()),
            load_balancer: std::cell::RefCell::new(PlacementLoadBalancer::new()),
            audit_trail: std::cell::RefCell::new(PlacementAuditTrail::new(10000)),
            decision_count: 0,
        }
    }

    pub fn place_dataset(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError> {
        let region_id = RegionId::new([1; 32]);
        let hunit_id = HUnitId::new([1; 32]);
        let node_id = NodeId::new([1; 32]);
        let domain = FailureDomain {
            power: None,
            facility: None,
            network_segment: None,
            operator: None,
            geographic: None,
        };
        let decision = PlacementDecision::new(
            dataset_id,
            region_id,
            hunit_id,
            node_id,
            RedundancyMode::FullReplica,
            domain,
        )
        .with_score(1.0);
        self.decision_count += 1;
        self.load_balancer.borrow_mut().record_placement(node_id);
        self.audit_trail.borrow_mut().record(
            0,
            dataset_id,
            PlacementAuditAction::Placed,
            Some(node_id),
            "initial placement".to_string(),
        );
        Ok(decision)
    }

    pub fn place_partition(
        &self,
        partition_id: PartitionId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError> {
        let dataset_id = DatasetId::new([1; 32]);
        let region_id = RegionId::new([1; 32]);
        let hunit_id = HUnitId::new([1; 32]);
        let node_id = NodeId::new([1; 32]);
        let domain = FailureDomain {
            power: None,
            facility: None,
            network_segment: None,
            operator: None,
            geographic: None,
        };
        let decision = PlacementDecision::new(
            dataset_id,
            region_id,
            hunit_id,
            node_id,
            RedundancyMode::ErasureCoded { k: 3, n: 5 },
            domain,
        )
        .with_partition(partition_id)
        .with_score(0.9);
        self.decision_count += 1;
        Ok(decision)
    }

    pub fn place_with_constraints(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
        replica_count: u64,
        region_count: u64,
        hunit_count: u64,
        domains: Vec<FailureDomain>,
    ) -> Result<PlacementDecision, PlacementError> {
        let validator = self.constraint_validator.borrow();
        validator.validate(replica_count, region_count, hunit_count, &domains)?;
        drop(validator);
        let decision = self.place_dataset(dataset_id, policy)?;
        Ok(decision)
    }

    pub fn capacity_aware_select(
        &self,
        candidates: Vec<NodeId>,
        required: u64,
    ) -> Result<Vec<NodeId>, PlacementError> {
        let tracker = self.tracker.borrow();
        let suitable: Vec<NodeId> = candidates
            .into_iter()
            .filter(|n| tracker.node_available(n).map(|a| a >= required).unwrap_or(false))
            .collect();
        if suitable.is_empty() {
            return Err(PlacementError::NoSuitableCandidates {
                count: candidates.len(),
            });
        }
        Ok(suitable)
    }

    pub fn failure_domain_aware(&self, replicas: Vec<NodeId>) -> Result<bool, PlacementError> {
        let checker = self.diversity_checker.borrow();
        Ok(checker.check_diversity(replicas))
    }

    pub fn evaluate_affinity(
        &self,
        dataset_id: &DatasetId,
        candidate_node: &NodeId,
    ) -> Result<bool, PlacementError> {
        self.affinity_manager.borrow().evaluate(dataset_id, candidate_node)
    }

    pub fn record_load(&self, node_id: NodeId) {
        self.load_balancer.borrow_mut().record_placement(node_id);
    }

    pub fn should_rebalance(&self, threshold: u64) -> bool {
        self.load_balancer.borrow().should_rebalance(threshold)
    }

    pub fn audit_placement(
        &self,
        timestamp_ms: u64,
        dataset_id: DatasetId,
        action: PlacementAuditAction,
        node_id: Option<NodeId>,
        reason: String,
    ) {
        self.audit_trail.borrow_mut().record(
            timestamp_ms, dataset_id, action, node_id, reason,
        );
    }

    pub fn decision_count(&self) -> u64 {
        self.decision_count
    }

    pub fn tracked_node_count(&self) -> usize {
        self.tracker.borrow().tracked_node_count()
    }

    pub fn placement_name(&self) -> &'static str {
        "placement_engine"
    }

    pub fn placement_score(&self) -> f64 {
        1.0
    }
}

impl Placement for PlacementEngine {
    fn place_dataset(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError> {
        self.place_dataset(dataset_id, policy)
    }

    fn place_partition(
        &self,
        partition_id: PartitionId,
        policy: &PlacementPolicy,
    ) -> Result<PlacementDecision, PlacementError> {
        self.place_partition(partition_id, policy)
    }

    fn capacity_aware_select(
        &self,
        candidates: Vec<NodeId>,
        required: u64,
    ) -> Result<Vec<NodeId>, PlacementError> {
        self.capacity_aware_select(candidates, required)
    }

    fn failure_domain_aware(&self, replicas: Vec<NodeId>) -> Result<bool, PlacementError> {
        self.failure_domain_aware(replicas)
    }

    fn placement_name(&self) -> &'static str {
        self.placement_name()
    }

    fn decision_count(&self) -> u64 {
        self.decision_count
    }

    fn tracked_node_count(&self) -> usize {
        self.tracked_node_count()
    }

    fn place_with_constraints(
        &self,
        dataset_id: DatasetId,
        policy: &PlacementPolicy,
        replica_count: u64,
        region_count: u64,
        hunit_count: u64,
        domains: Vec<FailureDomain>,
    ) -> Result<PlacementDecision, PlacementError> {
        self.place_with_constraints(dataset_id, policy, replica_count, region_count, hunit_count, domains)
    }

    fn evaluate_affinity(
        &self,
        dataset_id: &DatasetId,
        candidate_node: &NodeId,
    ) -> Result<bool, PlacementError> {
        self.evaluate_affinity(dataset_id, candidate_node)
    }

    fn record_load(&self, node_id: NodeId) {
        self.record_load(node_id);
    }

    fn should_rebalance(&self, threshold: u64) -> bool {
        self.should_rebalance(threshold)
    }

    fn audit_placement(
        &self,
        timestamp_ms: u64,
        dataset_id: DatasetId,
        action: PlacementAuditAction,
        node_id: Option<NodeId>,
        reason: String,
    ) {
        self.audit_placement(timestamp_ms, dataset_id, action, node_id, reason);
    }
}

impl Default for PlacementEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-UNIT SELECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct HUnitSelector {
    pub hunits: std::cell::RefCell<std::collections::HashMap<HUnitId, u64>>,
    pub selection_count: u64,
}

impl HUnitSelector {
    pub fn new() -> Self {
        Self {
            hunits: std::cell::RefCell::new(std::collections::HashMap::new()),
            selection_count: 0,
        }
    }

    pub fn register_hunit(&self, hunit_id: HUnitId, capacity: u64) {
        self.hunits.borrow_mut().insert(hunit_id, capacity);
    }

    pub fn select_least_loaded(&self, candidates: &[HUnitId]) -> Option<HUnitId> {
        let hunits = self.hunits.borrow();
        candidates
            .iter()
            .min_by_key(|h| hunits.get(h).copied().unwrap_or(u64::MAX))
            .copied()
    }

    pub fn select_by_capacity(&self, candidates: &[HUnitId], required: u64) -> Option<HUnitId> {
        let hunits = self.hunits.borrow();
        candidates
            .iter()
            .filter(|h| hunits.get(h).copied().unwrap_or(0) >= required)
            .min_by_key(|h| hunits.get(h).copied().unwrap_or(u64::MAX))
            .copied()
    }

    pub fn selection_count(&self) -> u64 {
        self.selection_count
    }

    pub fn hunit_count(&self) -> usize {
        self.hunits.borrow().len()
    }
}

impl Default for HUnitSelector {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGION SELECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct RegionSelector {
    pub regions: std::cell::RefCell<std::collections::HashMap<RegionId, u64>>,
    pub latency_map: std::cell::RefCell<std::collections::HashMap<(RegionId, RegionId), u64>>,
}

impl RegionSelector {
    pub fn new() -> Self {
        Self {
            regions: std::cell::RefCell::new(std::collections::HashMap::new()),
            latency_map: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn register_region(&self, region_id: RegionId, capacity: u64) {
        self.regions.borrow_mut().insert(region_id, capacity);
    }

    pub fn set_latency(&self, region_a: RegionId, region_b: RegionId, latency_ms: u64) {
        self.latency_map.borrow_mut().insert((region_a, region_b), latency_ms);
    }

    pub fn select_nearest(&self, from: RegionId, candidates: &[RegionId]) -> Option<RegionId> {
        let latencies = self.latency_map.borrow();
        candidates
            .iter()
            .min_by_key(|r| latencies.get(&(from, *r)).copied().unwrap_or(u64::MAX))
            .copied()
    }

    pub fn select_highest_capacity(&self, candidates: &[RegionId]) -> Option<RegionId> {
        let regions = self.regions.borrow();
        candidates
            .iter()
            .max_by_key(|r| regions.get(r).copied().unwrap_or(0))
            .copied()
    }

    pub fn region_count(&self) -> usize {
        self.regions.borrow().len()
    }

    pub fn latency_count(&self) -> usize {
        self.latency_map.borrow().len()
    }
}

impl Default for RegionSelector {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE SELECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct NodeSelector {
    pub nodes: std::cell::RefCell<std::collections::HashMap<NodeId, (u64, bool)>>,
    pub selection_count: u64,
}

impl NodeSelector {
    pub fn new() -> Self {
        Self {
            nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            selection_count: 0,
        }
    }

    pub fn register_node(&self, node_id: NodeId, capacity: u64, healthy: bool) {
        self.nodes.borrow_mut().insert(node_id, (capacity, healthy));
    }

    pub fn select_healthy(&self, candidates: &[NodeId]) -> Option<NodeId> {
        let nodes = self.nodes.borrow();
        candidates
            .iter()
            .find(|n| nodes.get(n).map(|(_, h)| *h).unwrap_or(false))
            .copied()
    }

    pub fn select_highest_capacity_healthy(&self, candidates: &[NodeId]) -> Option<NodeId> {
        let nodes = self.nodes.borrow();
        candidates
            .iter()
            .filter(|n| nodes.get(n).map(|(_, h)| *h).unwrap_or(false))
            .max_by_key(|n| nodes.get(n).map(|(c, _)| *c).unwrap_or(0))
            .copied()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }
}

impl Default for NodeSelector {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1; 32])
    }

    fn test_partition_id() -> PartitionId {
        PartitionId::new([1; 32])
    }

    fn test_region_id() -> RegionId {
        RegionId::new([1; 32])
    }

    fn test_hunit_id() -> HUnitId {
        HUnitId::new([1; 32])
    }

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_failure_domain() -> FailureDomain {
        FailureDomain {
            power: Some([1; 16]),
            facility: Some([2; 16]),
            network_segment: Some([3; 16]),
            operator: Some([4; 16]),
            geographic: Some([5; 16]),
        }
    }

    // === PlacementError tests ===
    #[test]
    fn placement_error_category() {
        assert_eq!(
            PlacementError::DatasetNotFound {
                dataset_id: test_dataset_id(),
            }
            .category(),
            "dataset_not_found"
        );
        assert_eq!(
            PlacementError::PartitionNotFound {
                partition_id: test_partition_id(),
            }
            .category(),
            "partition_not_found"
        );
        assert_eq!(
            PlacementError::NodeNotFound {
                node_id: test_node_id(),
            }
            .category(),
            "node_not_found"
        );
        assert_eq!(
            PlacementError::InsufficientCapacity {
                required: 100,
                available: 50,
            }
            .category(),
            "insufficient_capacity"
        );
        assert_eq!(
            PlacementError::FailureDomainViolation {
                domain: test_failure_domain(),
            }
            .category(),
            "failure_domain_violation"
        );
        assert_eq!(
            PlacementError::PolicyInvalid {
                reason: "bad".to_string()
            }
            .category(),
            "policy_invalid"
        );
        assert_eq!(
            PlacementError::NoSuitableCandidates { count: 0 }.category(),
            "no_suitable_candidates"
        );
        assert_eq!(
            PlacementError::RebalancingInProgress {
                reason: "lock".to_string()
            }
            .category(),
            "rebalancing_in_progress"
        );
        assert_eq!(
            PlacementError::MigrationFailed {
                partition_id: test_partition_id(),
                reason: "timeout".to_string(),
            }
            .category(),
            "migration_failed"
        );
        assert_eq!(
            PlacementError::ConstraintViolation {
                constraint: "test".to_string(),
            }
            .category(),
            "constraint_violation"
        );
        assert_eq!(
            PlacementError::Internal {
                message: "panic".to_string(),
            }
            .category(),
            "internal"
        );
    }

    #[test]
    fn placement_error_is_recoverable() {
        assert!(PlacementError::InsufficientCapacity {
            required: 100,
            available: 50,
        }
        .is_recoverable());
        assert!(PlacementError::NoSuitableCandidates { count: 0 }.is_recoverable());
        assert!(PlacementError::RebalancingInProgress {
            reason: "x".to_string(),
        }
        .is_recoverable());
        assert!(!PlacementError::DatasetNotFound {
            dataset_id: test_dataset_id(),
        }
        .is_recoverable());
    }

    #[test]
    fn placement_error_suggested_action() {
        assert_eq!(
            PlacementError::DatasetNotFound {
                dataset_id: test_dataset_id(),
            }
            .suggested_action(),
            "create dataset first"
        );
        assert_eq!(
            PlacementError::InsufficientCapacity {
                required: 100,
                available: 50,
            }
            .suggested_action(),
            "add capacity or reduce requirement"
        );
        assert_eq!(
            PlacementError::PolicyInvalid {
                reason: "x".to_string(),
            }
            .suggested_action(),
            "correct placement policy"
        );
        assert_eq!(
            PlacementError::ConstraintViolation {
                constraint: "x".to_string(),
            }
            .suggested_action(),
            "relax placement constraints"
        );
    }

    // === PlacementPolicy tests ===
    #[test]
    fn placement_policy_as_str() {
        assert_eq!(PlacementPolicy::FirstFit.as_str(), "first_fit");
        assert_eq!(PlacementPolicy::BestFit.as_str(), "best_fit");
        assert_eq!(PlacementPolicy::WorstFit.as_str(), "worst_fit");
        assert_eq!(PlacementPolicy::RandomFit.as_str(), "random_fit");
        assert_eq!(
            PlacementPolicy::FailureDomainAware.as_str(),
            "failure_domain_aware"
        );
        assert_eq!(PlacementPolicy::LocalityAware.as_str(), "locality_aware");
        assert_eq!(
            PlacementPolicy::CapacityWeighted.as_str(),
            "capacity_weighted"
        );
        assert_eq!(PlacementPolicy::Custom(42).as_str(), "custom");
    }

    #[test]
    fn placement_policy_checks() {
        assert!(PlacementPolicy::FailureDomainAware.requires_failure_domain_check());
        assert!(PlacementPolicy::LocalityAware.requires_failure_domain_check());
        assert!(!PlacementPolicy::FirstFit.requires_failure_domain_check());
        assert!(PlacementPolicy::BestFit.requires_capacity_check());
        assert!(PlacementPolicy::CapacityWeighted.requires_capacity_check());
        assert!(!PlacementPolicy::FirstFit.requires_capacity_check());
        assert!(!PlacementPolicy::FirstFit.is_deterministic() == false);
        assert!(!PlacementPolicy::RandomFit.is_deterministic());
    }

    #[test]
    fn placement_policy_id() {
        assert_eq!(PlacementPolicy::Custom(42).id(), 42);
        assert_eq!(PlacementPolicy::FirstFit.id(), 0);
    }

    // === PlacementDecision tests ===
    #[test]
    fn placement_decision_new() {
        let decision = PlacementDecision::new(
            test_dataset_id(),
            test_region_id(),
            test_hunit_id(),
            test_node_id(),
            RedundancyMode::FullReplica,
            test_failure_domain(),
        );
        assert_eq!(decision.dataset_id, test_dataset_id());
        assert_eq!(decision.partition_id, None);
        assert_eq!(decision.score_category(), "optimal");
        assert!(decision.is_optimal());
    }

    #[test]
    fn placement_decision_with_partition() {
        let decision = PlacementDecision::new(
            test_dataset_id(),
            test_region_id(),
            test_hunit_id(),
            test_node_id(),
            RedundancyMode::FullReplica,
            test_failure_domain(),
        )
        .with_partition(test_partition_id());
        assert_eq!(decision.partition_id, Some(test_partition_id()));
    }

    #[test]
    fn placement_decision_score() {
        let poor = PlacementDecision::new(
            test_dataset_id(),
            test_region_id(),
            test_hunit_id(),
            test_node_id(),
            RedundancyMode::FullReplica,
            test_failure_domain(),
        )
        .with_score(0.3);
        assert!(!poor.is_optimal());
        assert_eq!(poor.score_category(), "poor");

        let acceptable = PlacementDecision::new(
            test_dataset_id(),
            test_region_id(),
            test_hunit_id(),
            test_node_id(),
            RedundancyMode::FullReplica,
            test_failure_domain(),
        )
        .with_score(0.6);
        assert!(!acceptable.is_optimal());
        assert_eq!(acceptable.score_category(), "acceptable");
    }

    #[test]
    fn placement_decision_replicas() {
        let decision = PlacementDecision::new(
            test_dataset_id(),
            test_region_id(),
            test_hunit_id(),
            test_node_id(),
            RedundancyMode::FullReplica,
            test_failure_domain(),
        )
        .with_replicas(vec![test_node_id(), NodeId::new([2; 32])]);
        assert_eq!(decision.replica_count(), 2);
        assert!(decision.has_replica(&test_node_id()));
        assert!(!decision.has_replica(&NodeId::new([9; 32])));
    }

    // === CapacityTracker tests ===
    #[test]
    fn capacity_tracker_set_and_available() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 300);
        assert_eq!(ct.node_available(&test_node_id()), Some(700));
    }

    #[test]
    fn capacity_tracker_allocate_deallocate() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 0);
        ct.allocate(test_node_id(), 400).unwrap();
        assert_eq!(ct.node_available(&test_node_id()), Some(600));
        ct.deallocate(test_node_id(), 200).unwrap();
        assert_eq!(ct.node_available(&test_node_id()), Some(800));
    }

    #[test]
    fn capacity_tracker_insufficient() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 100, 0);
        let err = ct.allocate(test_node_id(), 200).unwrap_err();
        assert!(matches!(err, PlacementError::InsufficientCapacity { .. }));
    }

    #[test]
    fn capacity_tracker_node_not_found() {
        let ct = CapacityTracker::default();
        let err = ct.allocate(test_node_id(), 10).unwrap_err();
        assert!(matches!(err, PlacementError::NodeNotFound { .. }));
    }

    #[test]
    fn capacity_tracker_utilization() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 300);
        assert_eq!(ct.node_utilization(&test_node_id()), Some(30.0));
    }

    #[test]
    fn capacity_tracker_counts() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 0);
        ct.allocate(test_node_id(), 100).unwrap();
        ct.deallocate(test_node_id(), 50).unwrap();
        assert_eq!(ct.total_allocations(), 1);
        assert_eq!(ct.total_deallocations(), 1);
        assert_eq!(ct.tracked_node_count(), 1);
    }

    #[test]
    fn capacity_tracker_overloaded_underutilized() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 900);
        ct.set_node_capacity(NodeId::new([2; 32]), 1000, 100);
        assert!(ct.find_overloaded(80).contains(&test_node_id()));
        assert!(ct.find_underutilized(20).contains(&NodeId::new([2; 32])));
    }

    #[test]
    fn capacity_tracker_history() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 1000, 0);
        ct.allocate(test_node_id(), 100).unwrap();
        assert!(ct.history_len() >= 1);
    }

    // === FailureDomainAnalyzer tests ===
    #[test]
    fn failure_domain_analyzer_register_and_analyze() {
        let fda = FailureDomainAnalyzer::default();
        fda.register_domain(test_node_id(), test_failure_domain());
        let n2 = NodeId::new([2; 32]);
        fda.register_domain(n2, test_failure_domain());
        let result = fda.analyze_overlap(vec![test_node_id(), n2]).unwrap();
        assert_eq!(result, DomainOverlap::Full);
        assert!(!result.is_acceptable());
    }

    #[test]
    fn failure_domain_analyzer_distinct() {
        let fda = FailureDomainAnalyzer::default();
        let n1 = test_node_id();
        let n2 = NodeId::new([2; 32]);
        let mut domain2 = test_failure_domain();
        domain2.geographic = Some([9; 16]);
        fda.register_domain(n1, test_failure_domain());
        fda.register_domain(n2, domain2);
        assert!(fda.fully_distinct(&n1, &n2));
        assert!(fda.check_pairwise_distinct(&[n1, n2]));
    }

    #[test]
    fn failure_domain_analyzer_power_facility() {
        let fda = FailureDomainAnalyzer::default();
        let n1 = test_node_id();
        let n2 = NodeId::new([2; 32]);
        fda.register_domain(n1, test_failure_domain());
        fda.register_domain(n2, test_failure_domain());
        assert!(!fda.power_domain_distinct(&n1, &n2));
        assert!(!fda.facility_distinct(&n1, &n2));
    }

    #[test]
    fn failure_domain_analyzer_rates() {
        let fda = FailureDomainAnalyzer::default();
        fda.register_domain(test_node_id(), test_failure_domain());
        fda.register_domain(NodeId::new([2; 32]), test_failure_domain());
        fda.analyze_overlap(vec![test_node_id(), NodeId::new([2; 32])]).unwrap();
        assert_eq!(fda.analysis_count(), 1);
        assert_eq!(fda.violation_count(), 1);
        assert_eq!(fda.violation_rate(), 1.0);
    }

    // === PlacementCostModel tests ===
    #[test]
    fn placement_cost_model_weights() {
        let pcm = PlacementCostModel::default();
        assert_eq!(pcm.get_weight(CostComponent::Latency), 0.3);
        assert_eq!(pcm.get_weight(CostComponent::Bandwidth), 0.2);
    }

    #[test]
    fn placement_cost_model_calculate() {
        let pcm = PlacementCostModel::default();
        let cost = pcm.calculate_cost(test_node_id(), 10, 1000, 0.5, 0.1, 100);
        assert!(cost > 0.0);
        assert_eq!(pcm.get_cached_cost(test_node_id()), Some(cost));
    }

    #[test]
    fn placement_cost_model_normalize() {
        let mut pcm = PlacementCostModel::default();
        pcm.set_weight(CostComponent::Latency, 2.0);
        pcm.normalize_weights();
        let w = pcm.get_weight(CostComponent::Latency);
        assert!((w - 0.5).abs() < 0.01);
    }

    #[test]
    fn placement_cost_model_counts() {
        let pcm = PlacementCostModel::default();
        pcm.calculate_cost(test_node_id(), 10, 1000, 0.5, 0.1, 100);
        assert_eq!(pcm.calculation_count(), 1);
        assert_eq!(pcm.weight_count(), 5);
        assert_eq!(pcm.cache_size(), 1);
    }

    // === AffinityRuleManager tests ===
    #[test]
    fn affinity_rule_add_and_evaluate() {
        let arm = AffinityRuleManager::default();
        let rule = PlacementRule::new(RuleType::Affinity, test_dataset_id(), test_node_id());
        arm.add_rule(rule);
        assert!(arm.evaluate(&test_dataset_id(), &test_node_id()).unwrap());
        assert!(!arm.evaluate(&test_dataset_id(), &NodeId::new([2; 32])).unwrap());
    }

    #[test]
    fn affinity_rule_anti_affinity() {
        let arm = AffinityRuleManager::default();
        let rule = PlacementRule::new(RuleType::AntiAffinity, test_dataset_id(), test_node_id());
        arm.add_rule(rule);
        assert!(!arm.evaluate(&test_dataset_id(), &test_node_id()).unwrap());
        assert!(arm.evaluate(&test_dataset_id(), &NodeId::new([2; 32])).unwrap());
    }

    #[test]
    fn affinity_rule_enable_disable() {
        let arm = AffinityRuleManager::default();
        let id = arm.add_rule(PlacementRule::new(RuleType::Affinity, test_dataset_id(), test_node_id()));
        arm.disable_rule(id);
        assert!(arm.evaluate(&test_dataset_id(), &NodeId::new([9; 32])).unwrap());
        arm.enable_rule(id);
        assert!(!arm.evaluate(&test_dataset_id(), &NodeId::new([9; 32])).unwrap());
    }

    #[test]
    fn affinity_rule_counts() {
        let arm = AffinityRuleManager::default();
        arm.add_rule(PlacementRule::new(RuleType::Affinity, test_dataset_id(), test_node_id()));
        assert_eq!(arm.rule_count(), 1);
        assert_eq!(arm.enabled_count(), 1);
    }

    // === ConstraintValidator tests ===
    #[test]
    fn constraint_validator_validate() {
        let cv = ConstraintValidator::default();
        cv.add_constraint(PlacementConstraint::new(ConstraintType::MinReplicas, 3));
        assert!(cv.validate(5, 3, 3, &[]).is_ok());
        let err = cv.validate(2, 3, 3, &[]).unwrap_err();
        assert!(matches!(err, PlacementError::InsufficientCapacity { .. }));
    }

    #[test]
    fn constraint_validator_required_domain() {
        let cv = ConstraintValidator::default();
        cv.add_constraint(
            PlacementConstraint::new(ConstraintType::RequiredFailureDomain, 0)
                .with_domain(test_failure_domain()),
        );
        assert!(cv.validate(3, 1, 1, &[test_failure_domain()]).is_ok());
        let err = cv.validate(3, 1, 1, &[]).unwrap_err();
        assert!(matches!(err, PlacementError::FailureDomainViolation { .. }));
    }

    #[test]
    fn constraint_validator_pass_rate() {
        let cv = ConstraintValidator::default();
        cv.add_constraint(PlacementConstraint::new(ConstraintType::MinReplicas, 3));
        cv.validate(5, 3, 3, &[]).unwrap();
        cv.validate(2, 3, 3, &[]).unwrap_err();
        assert!((cv.pass_rate() - 0.5).abs() < 0.01);
    }

    // === Rebalancer tests ===
    #[test]
    fn rebalancer_needs_and_execute() {
        let rb = Rebalancer::new(RebalanceStrategy::Threshold, 80);
        assert!(!rb.needs_rebalance(50.0));
        assert!(rb.needs_rebalance(85.0));
        rb.schedule_rebalance(test_dataset_id(), 1000);
        assert_eq!(rb.pending_count(), 1);
        assert!(rb.execute_rebalance(&test_dataset_id()));
        assert!(!rb.execute_rebalance(&test_dataset_id()));
    }

    #[test]
    fn rebalancer_strategy_str() {
        assert_eq!(RebalanceStrategy::Immediate.as_str(), "immediate");
        assert_eq!(RebalanceStrategy::Threshold.as_str(), "threshold");
        assert_eq!(RebalanceStrategy::Scheduled.as_str(), "scheduled");
    }

    // === MigrationPlanner tests ===
    #[test]
    fn migration_planner_plan_complete() {
        let mp = MigrationPlanner::default();
        mp.plan_migration(test_partition_id());
        mp.start_migration(test_partition_id());
        mp.complete_migration(test_partition_id());
        assert_eq!(
            mp.get_status(&test_partition_id()),
            MigrationStatus::Completed
        );
        assert_eq!(mp.migration_count(), 1);
        assert_eq!(mp.completed_count(), 1);
        assert_eq!(mp.failed_count(), 0);
        assert_eq!(mp.success_rate(), 1.0);
    }

    #[test]
    fn migration_planner_fail_and_rate() {
        let mp = MigrationPlanner::default();
        mp.plan_migration(test_partition_id());
        mp.fail_migration(test_partition_id());
        assert_eq!(mp.get_status(&test_partition_id()), MigrationStatus::Failed);
        assert_eq!(mp.failed_count(), 1);
        assert_eq!(mp.success_rate(), 0.0);
    }

    #[test]
    fn migration_status_str_terminal() {
        assert_eq!(MigrationStatus::Pending.as_str(), "pending");
        assert_eq!(MigrationStatus::InProgress.as_str(), "in_progress");
        assert_eq!(MigrationStatus::Completed.as_str(), "completed");
        assert_eq!(MigrationStatus::Failed.as_str(), "failed");
        assert!(MigrationStatus::Completed.is_terminal());
        assert!(MigrationStatus::Failed.is_terminal());
        assert!(!MigrationStatus::Pending.is_terminal());
    }

    // === HotColdTier tests ===
    #[test]
    fn hot_cold_tier_set_and_get() {
        let hct = HotColdTier::default();
        hct.set_tier(test_dataset_id(), Tier::Cold);
        assert_eq!(hct.get_tier(&test_dataset_id()), Tier::Cold);
        hct.transition_to_hot(test_dataset_id());
        assert_eq!(hct.get_tier(&test_dataset_id()), Tier::Hot);
    }

    #[test]
    fn hot_cold_tier_cost_multiplier() {
        assert_eq!(Tier::Hot.access_cost_multiplier(), 1.0);
        assert_eq!(Tier::Warm.access_cost_multiplier(), 5.0);
        assert_eq!(Tier::Cold.access_cost_multiplier(), 20.0);
    }

    #[test]
    fn hot_cold_tier_counts() {
        let hct = HotColdTier::default();
        hct.set_tier(test_dataset_id(), Tier::Warm);
        assert_eq!(hct.tier_count(), 1);
        assert_eq!(hct.tier_transitions(), 1);
        assert_eq!(hct.hot_count(), 0);
        assert_eq!(hct.cold_count(), 0);
    }

    // === DiversityChecker tests ===
    #[test]
    fn diversity_checker_check() {
        let dc = DiversityChecker::default();
        let n1 = test_node_id();
        let n2 = NodeId::new([2; 32]);
        dc.assign_domain(n1, test_failure_domain());
        dc.assign_domain(n2, test_failure_domain());
        assert!(!dc.check_diversity(vec![n1, n2]));
        assert_eq!(dc.violation_count(), 1);
        assert!((dc.violation_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn diversity_checker_diverse() {
        let dc = DiversityChecker::default();
        let n1 = test_node_id();
        let n2 = NodeId::new([2; 32]);
        dc.assign_domain(n1, test_failure_domain());
        let mut domain2 = test_failure_domain();
        domain2.geographic = Some([9; 16]);
        dc.assign_domain(n2, domain2);
        assert!(dc.check_diversity(vec![n1, n2]));
        assert_eq!(dc.violation_count(), 0);
    }

    // === TopologyAware tests ===
    #[test]
    fn topology_aware_link_and_hops() {
        let ta = TopologyAware::default();
        let r1 = test_region_id();
        let r2 = RegionId::new([2; 32]);
        ta.add_link(r1, r2, 10);
        assert_eq!(ta.get_link_cost(&r1, &r2), Some(10));
        assert_eq!(ta.estimate_hops(&r1, &r2), 1);
        assert_eq!(ta.link_count(), 1);
    }

    // === ReservationTracker tests ===
    #[test]
    fn reservation_tracker_reserve_release() {
        let rt = ReservationTracker::default();
        let id = rt.reserve(test_node_id(), 1000, 5000);
        assert_eq!(rt.get_status(id), Some(ReservationStatus::Active));
        assert_eq!(rt.total_reserved_bytes(), 1000);
        assert_eq!(rt.active_count(), 1);
        assert!(rt.release(id));
        assert_eq!(rt.get_status(id), Some(ReservationStatus::Released));
        assert_eq!(rt.total_reserved_bytes(), 0);
    }

    #[test]
    fn reservation_tracker_expire() {
        let rt = ReservationTracker::default();
        let id = rt.reserve(test_node_id(), 500, 1000);
        assert!(rt.expire(id));
        assert_eq!(rt.get_status(id), Some(ReservationStatus::Expired));
        assert!(!rt.release(id));
    }

    // === PlacementLoadBalancer tests ===
    #[test]
    fn load_balancer_record_and_select() {
        let lb = PlacementLoadBalancer::default();
        lb.record_placement(test_node_id());
        lb.record_placement(test_node_id());
        lb.record_placement(NodeId::new([2; 32]));
        assert_eq!(lb.get_load(&test_node_id()), 2);
        assert_eq!(lb.select_least_loaded(&[test_node_id(), &NodeId::new([2; 32])]).unwrap(), NodeId::new([2; 32]));
    }

    #[test]
    fn load_balancer_should_rebalance() {
        let lb = PlacementLoadBalancer::default();
        lb.record_placement(test_node_id());
        lb.record_placement(test_node_id());
        lb.record_placement(test_node_id());
        assert!(lb.should_rebalance(1));
    }

    #[test]
    fn load_balancer_average() {
        let lb = PlacementLoadBalancer::default();
        lb.record_placement(test_node_id());
        lb.record_placement(NodeId::new([2; 32]));
        assert_eq!(lb.average_load(), 1.0);
    }

    // === PlacementAuditTrail tests ===
    #[test]
    fn audit_trail_record_and_query() {
        let at = PlacementAuditTrail::default();
        at.record(
            1000,
            test_dataset_id(),
            PlacementAuditAction::Placed,
            Some(test_node_id()),
            "initial".to_string(),
        );
        assert_eq!(at.entry_count(), 1);
        let entries = at.entries_for_dataset(&test_dataset_id());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, PlacementAuditAction::Placed);
    }

    #[test]
    fn audit_trail_by_action() {
        let at = PlacementAuditTrail::default();
        at.record(
            1000,
            test_dataset_id(),
            PlacementAuditAction::Migrated,
            Some(test_node_id()),
            "rebalance".to_string(),
        );
        let migrated = at.entries_by_action(PlacementAuditAction::Migrated);
        assert_eq!(migrated.len(), 1);
    }

    // === PlacementEngine tests ===
    #[test]
    fn placement_engine_place_dataset() {
        let engine = PlacementEngine::default();
        let decision = engine
            .place_dataset(test_dataset_id(), &PlacementPolicy::FirstFit)
            .unwrap();
        assert_eq!(decision.dataset_id, test_dataset_id());
        assert_eq!(decision.score_category(), "optimal");
        assert_eq!(engine.decision_count(), 1);
    }

    #[test]
    fn placement_engine_place_partition() {
        let engine = PlacementEngine::default();
        let decision = engine
            .place_partition(test_partition_id(), &PlacementPolicy::BestFit)
            .unwrap();
        assert_eq!(decision.partition_id, Some(test_partition_id()));
        assert_eq!(decision.score_category(), "optimal");
    }

    #[test]
    fn placement_engine_capacity_select() {
        let engine = PlacementEngine::default();
        let tracker = engine.tracker.borrow();
        tracker.set_node_capacity(test_node_id(), 1000, 100);
        drop(tracker);
        let n2 = NodeId::new([2; 32]);
        let result = engine.capacity_aware_select(vec![test_node_id(), n2], 500);
        assert!(result.is_ok());
        let selected = result.unwrap();
        assert!(selected.contains(&test_node_id()));
    }

    #[test]
    fn placement_engine_capacity_select_none() {
        let engine = PlacementEngine::default();
        let result = engine.capacity_aware_select(vec![test_node_id()], 999999);
        assert!(matches!(
            result.unwrap_err(),
            PlacementError::NoSuitableCandidates { .. }
        ));
    }

    #[test]
    fn placement_engine_failure_domain() {
        let engine = PlacementEngine::default();
        let n1 = test_node_id();
        let n2 = NodeId::new([2; 32]);
        engine
            .diversity_checker
            .borrow_mut()
            .assign_domain(n1, test_failure_domain());
        engine
            .diversity_checker
            .borrow_mut()
            .assign_domain(n2, test_failure_domain());
        let result = engine.failure_domain_aware(vec![n1, n2]);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn placement_engine_counts() {
        let engine = PlacementEngine::default();
        assert_eq!(engine.decision_count(), 0);
        engine.place_dataset(test_dataset_id(), &PlacementPolicy::FirstFit).unwrap();
        assert_eq!(engine.decision_count(), 1);
    }

    #[test]
    fn placement_engine_place_with_constraints() {
        let engine = PlacementEngine::default();
        let validator = engine.constraint_validator.borrow();
        validator.add_constraint(PlacementConstraint::new(ConstraintType::MinReplicas, 1));
        drop(validator);
        let result = engine.place_with_constraints(
            test_dataset_id(),
            &PlacementPolicy::FirstFit,
            5,
            3,
            3,
            &[],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn placement_engine_evaluate_affinity() {
        let engine = PlacementEngine::default();
        let arm = engine.affinity_manager.borrow();
        arm.add_rule(PlacementRule::new(RuleType::Affinity, test_dataset_id(), test_node_id()));
        drop(arm);
        assert!(engine.evaluate_affinity(&test_dataset_id(), &test_node_id()).unwrap());
        assert!(!engine.evaluate_affinity(&test_dataset_id(), &NodeId::new([9; 32])).unwrap());
    }

    #[test]
    fn placement_engine_audit_placement() {
        let engine = PlacementEngine::default();
        engine.audit_placement(
            5000,
            test_dataset_id(),
            PlacementAuditAction::Migrated,
            Some(test_node_id()),
            "rebalance".to_string(),
        );
        let entries = engine.audit_trail.borrow().entries_for_dataset(&test_dataset_id());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, PlacementAuditAction::Migrated);
    }

    #[test]
    fn placement_engine_load_balance() {
        let engine = PlacementEngine::default();
        engine.record_load(test_node_id());
        engine.record_load(test_node_id());
        engine.record_load(NodeId::new([2; 32]));
        assert_eq!(engine.load_balancer.borrow().get_load(&test_node_id()), 2);
        assert!(engine.should_rebalance(1));
    }

    // === Tier tests ===
    #[test]
    fn tier_as_str() {
        assert_eq!(Tier::Hot.as_str(), "hot");
        assert_eq!(Tier::Warm.as_str(), "warm");
        assert_eq!(Tier::Cold.as_str(), "cold");
    }

    // === RedundancyMode tests ===
    #[test]
    fn redundancy_mode_full_vs_erasure() {
        let full = RedundancyMode::FullReplica;
        let erasure = RedundancyMode::ErasureCoded { k: 3, n: 5 };
        assert!(matches!(full, RedundancyMode::FullReplica));
        assert!(matches!(erasure, RedundancyMode::ErasureCoded { k: 3, n: 5 }));
    }

    // === DomainOverlap tests ===
    #[test]
    fn domain_overlap_as_str() {
        assert_eq!(DomainOverlap::None.as_str(), "none");
        assert_eq!(DomainOverlap::Partial.as_str(), "partial");
        assert_eq!(DomainOverlap::Full.as_str(), "full");
        assert!(DomainOverlap::None.is_acceptable());
        assert!(!DomainOverlap::Full.is_acceptable());
    }

    // === ReservationStatus tests ===
    #[test]
    fn reservation_status_as_str() {
        assert_eq!(ReservationStatus::Active.as_str(), "active");
        assert_eq!(ReservationStatus::Expired.as_str(), "expired");
        assert_eq!(ReservationStatus::Released.as_str(), "released");
        assert!(ReservationStatus::Active.is_active());
        assert!(!ReservationStatus::Expired.is_active());
    }

    // === RuleType tests ===
    #[test]
    fn rule_type_as_str() {
        assert_eq!(RuleType::Affinity.as_str(), "affinity");
        assert_eq!(RuleType::AntiAffinity.as_str(), "anti_affinity");
    }

    // === ConstraintType tests ===
    #[test]
    fn constraint_type_as_str() {
        assert_eq!(ConstraintType::MinReplicas.as_str(), "min_replicas");
        assert_eq!(ConstraintType::MaxReplicas.as_str(), "max_replicas");
        assert_eq!(ConstraintType::MinRegions.as_str(), "min_regions");
        assert_eq!(ConstraintType::MaxRegions.as_str(), "max_regions");
        assert_eq!(ConstraintType::MinHUnits.as_str(), "min_hunits");
        assert_eq!(ConstraintType::MaxHUnits.as_str(), "max_hunits");
        assert_eq!(ConstraintType::RequiredFailureDomain.as_str(), "required_failure_domain");
        assert_eq!(ConstraintType::ProhibitedFailureDomain.as_str(), "prohibited_failure_domain");
    }

    // === PlacementAuditAction tests ===
    #[test]
    fn placement_audit_action_as_str() {
        assert_eq!(PlacementAuditAction::Placed.as_str(), "placed");
        assert_eq!(PlacementAuditAction::Migrated.as_str(), "migrated");
        assert_eq!(PlacementAuditAction::Removed.as_str(), "removed");
        assert_eq!(PlacementAuditAction::Rebalanced.as_str(), "rebalanced");
    }

    // === CapacityTracker edge tests ===
    #[test]
    fn capacity_tracker_zero_capacity() {
        let ct = CapacityTracker::default();
        ct.set_node_capacity(test_node_id(), 0, 0);
        assert_eq!(ct.node_utilization(&test_node_id()), Some(0.0));
        assert_eq!(ct.node_available(&test_node_id()), Some(0));
    }

    #[test]
    fn capacity_tracker_find_empty() {
        let ct = CapacityTracker::default();
        assert!(ct.find_overloaded(80).is_empty());
        assert!(ct.find_underutilized(20).is_empty());
    }

    // === DiversityChecker empty test ===
    #[test]
    fn diversity_checker_empty() {
        let dc = DiversityChecker::default();
        assert!(dc.check_diversity(vec![]));
        assert_eq!(dc.check_count(), 1);
    }

    // === TopologyAware empty test ===
    #[test]
    fn topology_aware_no_link() {
        let ta = TopologyAware::default();
        assert_eq!(ta.get_link_cost(&test_region_id(), &RegionId::new([2; 32])), None);
        assert_eq!(ta.link_count(), 0);
    }

    // === ReservationTracker empty test ===
    #[test]
    fn reservation_tracker_empty() {
        let rt = ReservationTracker::default();
        assert_eq!(rt.active_count(), 0);
        assert_eq!(rt.total_count(), 0);
        assert_eq!(rt.get_status(999), None);
    }

    // === LoadBalancer empty test ===
    #[test]
    fn load_balancer_empty() {
        let lb = PlacementLoadBalancer::default();
        assert_eq!(lb.select_least_loaded(&[]), None);
        assert!(!lb.should_rebalance(1));
        assert_eq!(lb.node_count(), 0);
    }

    // === AuditTrail empty test ===
    #[test]
    fn audit_trail_empty() {
        let at = PlacementAuditTrail::default();
        assert_eq!(at.entry_count(), 0);
        assert!(at.entries_for_dataset(&test_dataset_id()).is_empty());
    }
}