///////////////////////////////////////////////////////////////////////////////
// Regional Autonomy Invariant — regional clusters operate independently
//
// IDS §9: regional clusters operate independently for local operations.
// This invariant guarantees that operations within a region do not require
// cross-region coordination, and that regions can remain operational during
// outages in other regions.
//
// Every regional autonomy decision follows a strict model:
//   1. Determine whether an operation is local to a region
//   2. Check whether cross-region coordination is required
//   3. Verify region isolation during outages in peer regions
//   4. Route operations appropriately (local vs cross-region)
//   5. Track region health and isolation status
//
// This file is part of ids-000-aa, the central hub crate. All types and
// traits here are sealed — downstream crates cannot implement them.
//
// Carrier-agnostic: uses only opaque Endpoint types (CarrierType + Vec<u8>).
// No std::net, SocketAddr, IP, TCP, UDP, or any existing networking stack.
///////////////////////////////////////////////////////////////////////////////

use crate::types::{DatasetId, NodeId, Operation, RegionId};
use crate::errors::Error;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait contract
///////////////////////////////////////////////////////////////////////////////

/// Sealed trait guaranteeing regional clusters operate independently.
///
/// Downstream crates may call methods and use as trait bounds, but cannot
/// implement — only `ids-000-aa` and its designated implementation crates.
pub trait RegionalAutonomy: private::Sealed {
    /// Return whether an operation on a dataset is allowed within a region
    /// without cross-region coordination.
    fn local_operation_allowed(&self, region: RegionId, dataset: DatasetId) -> bool;

    /// Return whether an operation requires cross-region coordination.
    fn cross_region_required(&self, op: Operation) -> bool;

    /// Return whether a region is isolated during an outage in another region.
    fn region_isolated_during_outage(&self, region: RegionId, other: RegionId) -> bool;

    /// Return whether a region is currently healthy.
    fn region_healthy(&self, region: RegionId) -> bool;

    /// Return the set of regions that can serve a dataset locally.
    fn local_serving_regions(&self, dataset: DatasetId) -> Vec<RegionId>;

    /// Return the total number of regions tracked.
    fn region_count(&self) -> u64;
}

mod private {
    /// Sealed supertrait — only ids-000-aa can implement RegionalAutonomy.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl RegionalAutonomy for () {
    fn local_operation_allowed(&self, _region: RegionId, _dataset: DatasetId) -> bool {
        true
    }
    fn cross_region_required(&self, _op: Operation) -> bool {
        false
    }
    fn region_isolated_during_outage(&self, _region: RegionId, _other: RegionId) -> bool {
        true
    }
    fn region_healthy(&self, _region: RegionId) -> bool {
        true
    }
    fn local_serving_regions(&self, _dataset: DatasetId) -> Vec<RegionId> {
        Vec::new()
    }
    fn region_count(&self) -> u64 {
        0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region health tracking
///////////////////////////////////////////////////////////////////////////////

/// Health status of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionHealth {
    Healthy,
    Degraded,
    Isolated,
    Failed,
    Recovering,
}

/// Health record for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHealthRecord {
    pub region_id: RegionId,
    pub health: RegionHealth,
    pub last_heartbeat: TimestampMillis,
    pub outage_start: Option<TimestampMillis>,
    pub peer_regions: Vec<RegionId>,
    pub local_datasets: Vec<DatasetId>,
}

/// Tracks health status for all regions.
pub struct RegionHealthTracker {
    /// Health records by region ID.
    records: HashMap<RegionId, RegionHealthRecord>,
    /// Total regions tracked.
    total_regions: u64,
    /// Regions currently healthy.
    healthy_count: u64,
    /// Regions currently isolated or failed.
    unhealthy_count: u64,
}

impl RegionHealthTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        RegionHealthTracker {
            records: HashMap::new(),
            total_regions: 0,
            healthy_count: 0,
            unhealthy_count: 0,
        }
    }

    /// Register a region with its health record.
    pub fn register_region(&mut self, record: RegionHealthRecord) {
        let is_new = !self.records.contains_key(&record.region_id);
        let old_health = self.records.get(&record.region_id).map(|r| r.health);

        self.records.insert(record.region_id, record.clone());

        if is_new {
            self.total_regions += 1;
        }

        // Update health counts.
        if let Some(old) = old_health {
            if self.is_healthy(old) {
                self.healthy_count = self.healthy_count.saturating_sub(1);
            } else {
                self.unhealthy_count = self.unhealthy_count.saturating_sub(1);
            }
        }
        if self.is_healthy(record.health) {
            self.healthy_count += 1;
        } else {
            self.unhealthy_count += 1;
        }
    }

    /// Update the health of a region.
    pub fn update_health(&mut self, region: RegionId, health: RegionHealth) {
        if let Some(record) = self.records.get_mut(&region) {
            let old = record.health;
            record.health = health;
            if old != health {
                if self.is_healthy(old) {
                    self.healthy_count = self.healthy_count.saturating_sub(1);
                } else {
                    self.unhealthy_count = self.unhealthy_count.saturating_sub(1);
                }
                if self.is_healthy(health) {
                    self.healthy_count += 1;
                } else {
                    self.unhealthy_count += 1;
                }
            }
        }
    }

    /// Record an outage start for a region.
    pub fn record_outage(&mut self, region: RegionId, timestamp: TimestampMillis) {
        if let Some(record) = self.records.get_mut(&region) {
            record.outage_start = Some(timestamp);
            record.health = RegionHealth::Failed;
            if self.is_healthy(RegionHealth::Failed) {
                // unreachable
            } else {
                // Already counted as unhealthy if transitioning from healthy
                let was_healthy = matches!(record.health, RegionHealth::Healthy | RegionHealth::Degraded);
                if was_healthy {
                    self.healthy_count = self.healthy_count.saturating_sub(1);
                    self.unhealthy_count += 1;
                }
            }
        }
    }

    /// Record a heartbeat for a region.
    pub fn record_heartbeat(&mut self, region: RegionId, timestamp: TimestampMillis) {
        if let Some(record) = self.records.get_mut(&region) {
            record.last_heartbeat = timestamp;
            if record.health == RegionHealth::Failed {
                record.health = RegionHealth::Recovering;
                self.unhealthy_count = self.unhealthy_count.saturating_sub(1);
                self.healthy_count += 1;
            }
        }
    }

    /// Return whether a region is healthy.
    pub fn region_healthy(&self, region: RegionId) -> bool {
        match self.records.get(&region) {
            Some(r) => self.is_healthy(r.health),
            None => false,
        }
    }

    /// Return the health record for a region.
    pub fn health_record(&self, region: RegionId) -> Option<&RegionHealthRecord> {
        self.records.get(&region)
    }

    /// Return all healthy regions.
    pub fn healthy_regions(&self) -> Vec<RegionId> {
        self.records
            .iter()
            .filter(|(_, r)| self.is_healthy(r.health))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return all unhealthy regions.
    pub fn unhealthy_regions(&self) -> Vec<RegionId> {
        self.records
            .iter()
            .filter(|(_, r)| !self.is_healthy(r.health))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return total region count.
    pub fn region_count(&self) -> u64 {
        self.total_regions
    }

    /// Return healthy count.
    pub fn healthy_count(&self) -> u64 {
        self.healthy_count
    }

    /// Return unhealthy count.
    pub fn unhealthy_count(&self) -> u64 {
        self.unhealthy_count
    }

    fn is_healthy(&self, health: RegionHealth) -> bool {
        matches!(health, RegionHealth::Healthy | RegionHealth::Degraded | RegionHealth::Recovering)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Local operation router — determines if operations can be served locally
///////////////////////////////////////////////////////////////////////////////

/// A local operation routing decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRoutingDecision {
    pub region: RegionId,
    pub dataset: DatasetId,
    pub local_allowed: bool,
    pub preferred_region: Option<RegionId>,
    pub reason: LocalRoutingReason,
}

/// Reason for a local routing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalRoutingReason {
    LocalReplicaAvailable,
    NoLocalReplica,
    RegionUnhealthy,
    CrossRegionRequired,
    DatasetNotFound,
}

/// Routes operations to local regions when possible.
pub struct LocalOperationRouter {
    /// Dataset -> set of regions that have local replicas.
    dataset_regions: HashMap<DatasetId, BTreeSet<RegionId>>,
    /// Region -> set of datasets served locally.
    region_datasets: HashMap<RegionId, BTreeSet<DatasetId>>,
    /// Region health tracker.
    health_tracker: RegionHealthTracker,
    /// Total routing decisions made.
    total_decisions: u64,
    /// Total local decisions.
    local_decisions: u64,
    /// Total cross-region decisions.
    cross_region_decisions: u64,
}

impl LocalOperationRouter {
    /// Create a new router.
    pub fn new() -> Self {
        LocalOperationRouter {
            dataset_regions: HashMap::new(),
            region_datasets: HashMap::new(),
            health_tracker: RegionHealthTracker::new(),
            total_decisions: 0,
            local_decisions: 0,
            cross_region_decisions: 0,
        }
    }

    /// Register that a dataset has a local replica in a region.
    pub fn register_local_replica(&mut self, dataset: DatasetId, region: RegionId) {
        self.dataset_regions.entry(dataset).or_default().insert(region);
        self.region_datasets.entry(region).or_default().insert(dataset);
    }

    /// Remove a local replica registration.
    pub fn remove_local_replica(&mut self, dataset: DatasetId, region: RegionId) {
        if let Some(regions) = self.dataset_regions.get_mut(&dataset) {
            regions.remove(&region);
        }
        if let Some(datasets) = self.region_datasets.get_mut(&region) {
            datasets.remove(&dataset);
        }
    }

    /// Register a region with the health tracker.
    pub fn register_region_health(&mut self, record: RegionHealthRecord) {
        self.health_tracker.register_region(record);
    }

    /// Route an operation: determine if it can be served locally.
    pub fn route_operation(&mut self, region: RegionId, dataset: DatasetId) -> LocalRoutingDecision {
        self.total_decisions += 1;

        let region_healthy = self.health_tracker.region_healthy(region);
        let local_regions = self.dataset_regions.get(&dataset).cloned().unwrap_or_default();

        if !local_regions.contains(&region) {
            self.cross_region_decisions += 1;
            return LocalRoutingDecision {
                region,
                dataset,
                local_allowed: false,
                preferred_region: local_regions.iter().next().copied(),
                reason: LocalRoutingReason::NoLocalReplica,
            };
        }

        if !region_healthy {
            self.cross_region_decisions += 1;
            return LocalRoutingDecision {
                region,
                dataset,
                local_allowed: false,
                preferred_region: self
                    .health_tracker
                    .healthy_regions()
                    .iter()
                    .find(|r| local_regions.contains(*r))
                    .copied(),
                reason: LocalRoutingReason::RegionUnhealthy,
            };
        }

        self.local_decisions += 1;
        LocalRoutingDecision {
            region,
            dataset,
            local_allowed: true,
            preferred_region: Some(region),
            reason: LocalRoutingReason::LocalReplicaAvailable,
        }
    }

    /// Return whether a local operation is allowed.
    pub fn local_operation_allowed(&self, region: RegionId, dataset: DatasetId) -> bool {
        let local_regions = self.dataset_regions.get(&dataset).cloned().unwrap_or_default();
        local_regions.contains(&region) && self.health_tracker.region_healthy(region)
    }

    /// Return regions that can serve a dataset locally.
    pub fn local_serving_regions(&self, dataset: DatasetId) -> Vec<RegionId> {
        self.dataset_regions
            .get(&dataset)
            .map(|regions| {
                regions
                    .iter()
                    .filter(|r| self.health_tracker.region_healthy(**r))
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total routing decisions.
    pub fn total_decisions(&self) -> u64 {
        self.total_decisions
    }

    /// Return local decision count.
    pub fn local_decisions(&self) -> u64 {
        self.local_decisions
    }

    /// Return cross-region decision count.
    pub fn cross_region_decisions(&self) -> u64 {
        self.cross_region_decisions
    }

    /// Return local decision ratio.
    pub fn local_ratio(&self) -> f64 {
        if self.total_decisions == 0 {
            return 1.0;
        }
        self.local_decisions as f64 / self.total_decisions as f64
    }

    /// Return the health tracker for advanced queries.
    pub fn health_tracker(&self) -> &RegionHealthTracker {
        &self.health_tracker
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-region operation detector
///////////////////////////////////////////////////////////////////////////////

/// A cross-region operation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRegionOperation {
    pub op_id: u64,
    pub source_region: RegionId,
    pub target_region: RegionId,
    pub dataset: DatasetId,
    pub operation_type: CrossRegionOpType,
    pub required: bool,
    pub completed: bool,
}

/// Type of cross-region operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRegionOpType {
    Replicate,
    Rebalance,
    Failover,
    Query,
    Write,
    Read,
}

/// Detects and tracks cross-region operations.
pub struct CrossRegionOperationDetector {
    /// Operations by ID.
    operations: HashMap<u64, CrossRegionOperation>,
    /// Per-source-region operation IDs.
    source_ops: HashMap<RegionId, Vec<u64>>,
    /// Per-target-region operation IDs.
    target_ops: HashMap<RegionId, Vec<u64>>,
    /// Next operation ID.
    next_id: u64,
    /// Total cross-region operations.
    total_ops: u64,
    /// Total completed operations.
    completed_ops: u64,
}

impl CrossRegionOperationDetector {
    /// Create a new detector.
    pub fn new() -> Self {
        CrossRegionOperationDetector {
            operations: HashMap::new(),
            source_ops: HashMap::new(),
            target_ops: HashMap::new(),
            next_id: 0,
            total_ops: 0,
            completed_ops: 0,
        }
    }

    /// Record a cross-region operation.
    pub fn record_operation(
        &mut self,
        source_region: RegionId,
        target_region: RegionId,
        dataset: DatasetId,
        op_type: CrossRegionOpType,
        required: bool,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.total_ops += 1;
        let op = CrossRegionOperation {
            op_id: id,
            source_region,
            target_region,
            dataset,
            operation_type: op_type,
            required,
            completed: false,
        };
        self.operations.insert(id, op);
        self.source_ops.entry(source_region).or_default().push(id);
        self.target_ops.entry(target_region).or_default().push(id);
        id
    }

    /// Mark an operation as completed.
    pub fn complete_operation(&mut self, op_id: u64) -> Result<(), Error> {
        let op = self
            .operations
            .get_mut(&op_id)
            .ok_or_else(|| Error::Internal {
                message: "cross-region operation not found".to_string(),
            })?;
        op.completed = true;
        self.completed_ops += 1;
        Ok(())
    }

    /// Return whether cross-region is required for an operation.
    pub fn cross_region_required(&self, source: RegionId, target: RegionId) -> bool {
        source != target
    }

    /// Return operations for a source region.
    pub fn ops_for_source(&self, region: RegionId) -> Vec<CrossRegionOperation> {
        self.source_ops
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.operations.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return operations for a target region.
    pub fn ops_for_target(&self, region: RegionId) -> Vec<CrossRegionOperation> {
        self.target_ops
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.operations.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total operations.
    pub fn total_ops(&self) -> u64 {
        self.total_ops
    }

    /// Return completed operation count.
    pub fn completed_ops(&self) -> u64 {
        self.completed_ops
    }

    /// Return completion ratio.
    pub fn completion_ratio(&self) -> f64 {
        if self.total_ops == 0 {
            return 1.0;
        }
        self.completed_ops as f64 / self.total_ops as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Regional isolation manager — tracks isolation during outages
///////////////////////////////////////////////////////////////////////////////

/// An isolation record between two regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionIsolationRecord {
    pub region_a: RegionId,
    pub region_b: RegionId,
    pub isolated: bool,
    pub isolation_start: Option<TimestampMillis>,
    pub outage_region: Option<RegionId>,
}

/// Manages regional isolation during outages.
pub struct RegionalIsolationManager {
    /// Isolation records by region pair.
    isolations: HashMap<(RegionId, RegionId), RegionIsolationRecord>,
    /// Regions currently in outage.
    outage_regions: HashSet<RegionId>,
    /// Total isolation records.
    total_records: u64,
    /// Currently isolated pairs.
    isolated_count: u64,
}

impl RegionalIsolationManager {
    /// Create a new manager.
    pub fn new() -> Self {
        RegionalIsolationManager {
            isolations: HashMap::new(),
            outage_regions: HashSet::new(),
            total_records: 0,
            isolated_count: 0,
        }
    }

    /// Record an outage in a region.
    pub fn record_outage(&mut self, region: RegionId) {
        self.outage_regions.insert(region);
    }

    /// Record that a region has recovered from outage.
    pub fn record_recovery(&mut self, region: RegionId) {
        self.outage_regions.remove(&region);
    }

    /// Set isolation between two regions.
    pub fn set_isolation(
        &mut self,
        region_a: RegionId,
        region_b: RegionId,
        isolated: bool,
        outage_region: Option<RegionId>,
    ) {
        let was_isolated = self
            .isolations
            .get(&(region_a, region_b))
            .map(|r| r.isolated)
            .unwrap_or(false);

        let record = RegionIsolationRecord {
            region_a,
            region_b,
            isolated,
            isolation_start: if isolated {
                Some(TimestampMillis(0))
            } else {
                None
            },
            outage_region,
        };
        self.isolations.insert((region_a, region_b), record);
        self.total_records += 1;

        if isolated && !was_isolated {
            self.isolated_count += 1;
        } else if !isolated && was_isolated {
            self.isolated_count = self.isolated_count.saturating_sub(1);
        }
    }

    /// Return whether a region is isolated during an outage in another region.
    pub fn region_isolated_during_outage(&self, region: RegionId, other: RegionId) -> bool {
        // If other is not in outage, region is not isolated due to it.
        if !self.outage_regions.contains(&other) {
            return false;
        }
        // Check isolation record.
        self.isolations
            .get(&(region, other))
            .map(|r| r.isolated)
            .unwrap_or(false)
    }

    /// Return whether a region is in outage.
    pub fn region_in_outage(&self, region: RegionId) -> bool {
        self.outage_regions.contains(&region)
    }

    /// Return regions in outage.
    pub fn outage_regions(&self) -> Vec<RegionId> {
        self.outage_regions.iter().copied().collect()
    }

    /// Return total isolation records.
    pub fn total_records(&self) -> u64 {
        self.total_records
    }

    /// Return isolated pair count.
    pub fn isolated_count(&self) -> u64 {
        self.isolated_count
    }

    /// Return isolation status for a region pair.
    pub fn isolation_status(&self, a: RegionId, b: RegionId) -> Option<bool> {
        self.isolations.get(&(a, b)).map(|r| r.isolated)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Regional autonomy manager — core coordinator
///////////////////////////////////////////////////////////////////////////////

/// Configuration for regional autonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalAutonomyConfig {
    pub require_healthy_for_local: bool,
    pub allow_cross_region_failover: bool,
    pub max_cross_region_ops: u32,
}

impl Default for RegionalAutonomyConfig {
    fn default() -> Self {
        RegionalAutonomyConfig {
            require_healthy_for_local: true,
            allow_cross_region_failover: true,
            max_cross_region_ops: 100,
        }
    }
}

/// Core regional autonomy manager — coordinates local operations, cross-region
/// detection, and isolation during outages.
pub struct RegionalAutonomyManager {
    /// Autonomy configuration.
    config: RegionalAutonomyConfig,
    /// Local operation router.
    router: LocalOperationRouter,
    /// Cross-region detector.
    cross_region: CrossRegionOperationDetector,
    /// Isolation manager.
    isolation: RegionalIsolationManager,
    /// Total autonomy checks.
    total_checks: u64,
    /// Total local operations allowed.
    local_allowed_count: u64,
    /// Total cross-region operations required.
    cross_region_required_count: u64,
}

impl RegionalAutonomyManager {
    /// Create a new manager with default config.
    pub fn new() -> Self {
        RegionalAutonomyManager {
            config: RegionalAutonomyConfig::default(),
            router: LocalOperationRouter::new(),
            cross_region: CrossRegionOperationDetector::new(),
            isolation: RegionalIsolationManager::new(),
            total_checks: 0,
            local_allowed_count: 0,
            cross_region_required_count: 0,
        }
    }

    /// Create a manager with custom config.
    pub fn with_config(config: RegionalAutonomyConfig) -> Self {
        RegionalAutonomyManager {
            config,
            router: LocalOperationRouter::new(),
            cross_region: CrossRegionOperationDetector::new(),
            isolation: RegionalIsolationManager::new(),
            total_checks: 0,
            local_allowed_count: 0,
            cross_region_required_count: 0,
        }
    }

    /// Register a local replica for a dataset in a region.
    pub fn register_local_replica(&mut self, dataset: DatasetId, region: RegionId) {
        self.router.register_local_replica(dataset, region);
    }

    /// Register region health.
    pub fn register_region_health(&mut self, record: RegionHealthRecord) {
        self.router.register_region_health(record);
    }

    /// Record a region outage.
    pub fn record_outage(&mut self, region: RegionId) {
        self.isolation.record_outage(region);
    }

    /// Record region recovery.
    pub fn record_recovery(&mut self, region: RegionId) {
        self.isolation.record_recovery(region);
    }

    /// Set isolation between two regions.
    pub fn set_isolation(
        &mut self,
        region_a: RegionId,
        region_b: RegionId,
        isolated: bool,
    ) {
        self.isolation.set_isolation(region_a, region_b, isolated, None);
    }

    /// Check whether a local operation is allowed.
    pub fn local_operation_allowed(&mut self, region: RegionId, dataset: DatasetId) -> bool {
        self.total_checks += 1;
        let allowed = self.router.local_operation_allowed(region, dataset);
        if allowed {
            self.local_allowed_count += 1;
        } else {
            self.cross_region_required_count += 1;
        }
        allowed
    }

    /// Check whether cross-region is required for an operation.
    pub fn cross_region_required(&mut self, source: RegionId, target: RegionId) -> bool {
        self.total_checks += 1;
        let required = self.cross_region.cross_region_required(source, target);
        if required {
            self.cross_region_required_count += 1;
        } else {
            self.local_allowed_count += 1;
        }
        required
    }

    /// Check whether a region is isolated during an outage in another.
    pub fn region_isolated_during_outage(&mut self, region: RegionId, other: RegionId) -> bool {
        self.total_checks += 1;
        let isolated = self.isolation.region_isolated_during_outage(region, other);
        if isolated {
            self.cross_region_required_count += 1;
        } else {
            self.local_allowed_count += 1;
        }
        isolated
    }

    /// Check whether a region is healthy.
    pub fn region_healthy(&self, region: RegionId) -> bool {
        self.router.health_tracker().region_healthy(region)
    }

    /// Return regions that can serve a dataset locally.
    pub fn local_serving_regions(&self, dataset: DatasetId) -> Vec<RegionId> {
        self.router.local_serving_regions(dataset)
    }

    /// Record a cross-region operation.
    pub fn record_cross_region_op(
        &mut self,
        source: RegionId,
        target: RegionId,
        dataset: DatasetId,
        op_type: CrossRegionOpType,
        required: bool,
    ) -> u64 {
        self.cross_region
            .record_operation(source, target, dataset, op_type, required)
    }

    /// Complete a cross-region operation.
    pub fn complete_cross_region_op(&mut self, op_id: u64) -> Result<(), Error> {
        self.cross_region.complete_operation(op_id)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return local allowed count.
    pub fn local_allowed_count(&self) -> u64 {
        self.local_allowed_count
    }

    /// Return cross-region required count.
    pub fn cross_region_required_count(&self) -> u64 {
        self.cross_region_required_count
    }

    /// Return local allowance ratio.
    pub fn local_ratio(&self) -> f64 {
        self.router.local_ratio()
    }

    /// Return the router for advanced queries.
    pub fn router(&self) -> &LocalOperationRouter {
        &self.router
    }

    /// Return the isolation manager.
    pub fn isolation_manager(&self) -> &RegionalIsolationManager {
        &self.isolation
    }

    /// Return the cross-region detector.
    pub fn cross_region_detector(&self) -> &CrossRegionOperationDetector {
        &self.cross_region
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Version bit position for the regional autonomy invariant in EdgeCaseBits.
pub const REGIONAL_AUTONOMY_BIT: u128 = 1 << 11;

/// Check whether the regional autonomy invariant is enabled in a bitset.
pub fn regional_autonomy_enabled(bits: &crate::versioning::EdgeCaseBits) -> bool {
    bits.has(REGIONAL_AUTONOMY_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod regional_autonomy_tests {
    use super::*;

    // ---- RegionHealth tests ----

    #[test]
    fn region_health_variants() {
        let statuses = [
            RegionHealth::Healthy,
            RegionHealth::Degraded,
            RegionHealth::Isolated,
            RegionHealth::Failed,
            RegionHealth::Recovering,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ---- RegionHealthRecord tests ----

    #[test]
    fn region_health_record_creation() {
        let record = RegionHealthRecord {
            region_id: RegionId::new([1u8; 32]),
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: vec![RegionId::new([2u8; 32])],
            local_datasets: vec![DatasetId::new([3u8; 32])],
        };
        assert_eq!(record.health, RegionHealth::Healthy);
        assert_eq!(record.peer_regions.len(), 1);
    }

    // ---- RegionHealthTracker tests ----

    #[test]
    fn tracker_register_region() {
        let mut tracker = RegionHealthTracker::new();
        let region = RegionId::new([1u8; 32]);
        tracker.register_region(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        assert_eq!(tracker.region_count(), 1);
        assert_eq!(tracker.healthy_count(), 1);
        assert_eq!(tracker.unhealthy_count(), 0);
        assert!(tracker.region_healthy(region));
    }

    #[test]
    fn tracker_update_health_to_failed() {
        let mut tracker = RegionHealthTracker::new();
        let region = RegionId::new([1u8; 32]);
        tracker.register_region(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        tracker.update_health(region, RegionHealth::Failed);
        assert!(!tracker.region_healthy(region));
        assert_eq!(tracker.healthy_count(), 0);
        assert_eq!(tracker.unhealthy_count(), 1);
    }

    #[test]
    fn tracker_record_outage() {
        let mut tracker = RegionHealthTracker::new();
        let region = RegionId::new([1u8; 32]);
        tracker.register_region(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        tracker.record_outage(region, TimestampMillis(1000));
        assert!(!tracker.region_healthy(region));
        assert_eq!(tracker.unhealthy_count(), 1);
    }

    #[test]
    fn tracker_record_heartbeat_recovery() {
        let mut tracker = RegionHealthTracker::new();
        let region = RegionId::new([1u8; 32]);
        tracker.register_region(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Failed,
            last_heartbeat: TimestampMillis(0),
            outage_start: Some(TimestampMillis(500)),
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        tracker.record_heartbeat(region, TimestampMillis(2000));
        assert!(tracker.region_healthy(region));
    }

    #[test]
    fn tracker_healthy_and_unhealthy_regions() {
        let mut tracker = RegionHealthTracker::new();
        let r1 = RegionId::new([1u8; 32]);
        let r2 = RegionId::new([2u8; 32]);
        tracker.register_region(RegionHealthRecord {
            region_id: r1,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        tracker.register_region(RegionHealthRecord {
            region_id: r2,
            health: RegionHealth::Failed,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        assert_eq!(tracker.healthy_regions().len(), 1);
        assert_eq!(tracker.unhealthy_regions().len(), 1);
    }

    // ---- LocalRoutingDecision tests ----

    #[test]
    fn local_routing_decision_creation() {
        let decision = LocalRoutingDecision {
            region: RegionId::new([1u8; 32]),
            dataset: DatasetId::new([2u8; 32]),
            local_allowed: true,
            preferred_region: Some(RegionId::new([1u8; 32])),
            reason: LocalRoutingReason::LocalReplicaAvailable,
        };
        assert!(decision.local_allowed);
        assert_eq!(decision.reason, LocalRoutingReason::LocalReplicaAvailable);
    }

    // ---- LocalRoutingReason tests ----

    #[test]
    fn local_routing_reason_variants() {
        let reasons = [
            LocalRoutingReason::LocalReplicaAvailable,
            LocalRoutingReason::NoLocalReplica,
            LocalRoutingReason::RegionUnhealthy,
            LocalRoutingReason::CrossRegionRequired,
            LocalRoutingReason::DatasetNotFound,
        ];
        for r in reasons {
            let _ = format!("{:?}", r);
        }
    }

    // ---- LocalOperationRouter tests ----

    #[test]
    fn router_route_local_allowed() {
        let mut router = LocalOperationRouter::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        router.register_local_replica(dataset, region);
        router.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        let decision = router.route_operation(region, dataset);
        assert!(decision.local_allowed);
        assert_eq!(decision.reason, LocalRoutingReason::LocalReplicaAvailable);
        assert_eq!(router.local_decisions(), 1);
    }

    #[test]
    fn router_route_no_local_replica() {
        let mut router = LocalOperationRouter::new();
        let region = RegionId::new([1u8; 32]);
        let other = RegionId::new([2u8; 32]);
        let dataset = DatasetId::new([3u8; 32]);
        router.register_local_replica(dataset, other);
        let decision = router.route_operation(region, dataset);
        assert!(!decision.local_allowed);
        assert_eq!(decision.reason, LocalRoutingReason::NoLocalReplica);
        assert_eq!(router.cross_region_decisions(), 1);
    }

    #[test]
    fn router_route_region_unhealthy() {
        let mut router = LocalOperationRouter::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        router.register_local_replica(dataset, region);
        router.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Failed,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        let decision = router.route_operation(region, dataset);
        assert!(!decision.local_allowed);
        assert_eq!(decision.reason, LocalRoutingReason::RegionUnhealthy);
    }

    #[test]
    fn router_local_operation_allowed() {
        let mut router = LocalOperationRouter::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        router.register_local_replica(dataset, region);
        router.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        assert!(router.local_operation_allowed(region, dataset));
        assert!(!router.local_operation_allowed(RegionId::new([9u8; 32]), dataset));
    }

    #[test]
    fn router_local_serving_regions() {
        let mut router = LocalOperationRouter::new();
        let dataset = DatasetId::new([2u8; 32]);
        let r1 = RegionId::new([1u8; 32]);
        let r2 = RegionId::new([2u8; 32]);
        router.register_local_replica(dataset, r1);
        router.register_local_replica(dataset, r2);
        router.register_region_health(RegionHealthRecord {
            region_id: r1,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        router.register_region_health(RegionHealthRecord {
            region_id: r2,
            health: RegionHealth::Failed,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        let serving = router.local_serving_regions(dataset);
        assert_eq!(serving.len(), 1);
        assert_eq!(serving[0], r1);
    }

    #[test]
    fn router_local_ratio() {
        let mut router = LocalOperationRouter::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        router.register_local_replica(dataset, region);
        router.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        router.route_operation(region, dataset);
        router.route_operation(RegionId::new([9u8; 32]), dataset);
        assert!((router.local_ratio() - 0.5).abs() < 0.001);
    }

    // ---- CrossRegionOpType tests ----

    #[test]
    fn cross_region_op_type_variants() {
        let types = [
            CrossRegionOpType::Replicate,
            CrossRegionOpType::Rebalance,
            CrossRegionOpType::Failover,
            CrossRegionOpType::Query,
            CrossRegionOpType::Write,
            CrossRegionOpType::Read,
        ];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    // ---- CrossRegionOperation tests ----

    #[test]
    fn cross_region_operation_creation() {
        let op = CrossRegionOperation {
            op_id: 1,
            source_region: RegionId::new([1u8; 32]),
            target_region: RegionId::new([2u8; 32]),
            dataset: DatasetId::new([3u8; 32]),
            operation_type: CrossRegionOpType::Replicate,
            required: true,
            completed: false,
        };
        assert_eq!(op.op_id, 1);
        assert!(op.required);
        assert!(!op.completed);
    }

    // ---- CrossRegionOperationDetector tests ----

    #[test]
    fn detector_record_and_complete() {
        let mut detector = CrossRegionOperationDetector::new();
        let id = detector.record_operation(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            DatasetId::new([3u8; 32]),
            CrossRegionOpType::Replicate,
            true,
        );
        detector.complete_operation(id).unwrap();
        assert_eq!(detector.total_ops(), 1);
        assert_eq!(detector.completed_ops(), 1);
        assert!((detector.completion_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn detector_complete_nonexistent() {
        let mut detector = CrossRegionOperationDetector::new();
        assert!(detector.complete_operation(999).is_err());
    }

    #[test]
    fn detector_cross_region_required() {
        let detector = CrossRegionOperationDetector::new();
        assert!(detector.cross_region_required(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32])
        ));
        assert!(!detector.cross_region_required(
            RegionId::new([1u8; 32]),
            RegionId::new([1u8; 32])
        ));
    }

    #[test]
    fn detector_ops_for_source_and_target() {
        let mut detector = CrossRegionOperationDetector::new();
        let src = RegionId::new([1u8; 32]);
        let tgt = RegionId::new([2u8; 32]);
        detector.record_operation(src, tgt, DatasetId::new([3u8; 32]), CrossRegionOpType::Write, true);
        assert_eq!(detector.ops_for_source(src).len(), 1);
        assert_eq!(detector.ops_for_target(tgt).len(), 1);
        assert_eq!(detector.ops_for_target(src).len(), 0);
    }

    // ---- RegionalIsolationRecord tests ----

    #[test]
    fn isolation_record_creation() {
        let record = RegionIsolationRecord {
            region_a: RegionId::new([1u8; 32]),
            region_b: RegionId::new([2u8; 32]),
            isolated: true,
            isolation_start: Some(TimestampMillis(1000)),
            outage_region: Some(RegionId::new([3u8; 32])),
        };
        assert!(record.isolated);
        assert_eq!(record.outage_region, Some(RegionId::new([3u8; 32])));
    }

    // ---- RegionalIsolationManager tests ----

    #[test]
    fn isolation_manager_outage_and_recovery() {
        let mut mgr = RegionalIsolationManager::new();
        let region = RegionId::new([1u8; 32]);
        assert!(!mgr.region_in_outage(region));
        mgr.record_outage(region);
        assert!(mgr.region_in_outage(region));
        mgr.record_recovery(region);
        assert!(!mgr.region_in_outage(region));
    }

    #[test]
    fn isolation_manager_set_and_check() {
        let mut mgr = RegionalIsolationManager::new();
        let a = RegionId::new([1u8; 32]);
        let b = RegionId::new([2u8; 32]);
        mgr.record_outage(b);
        mgr.set_isolation(a, b, true, Some(b));
        assert!(mgr.region_isolated_during_outage(a, b));
        assert!(!mgr.region_isolated_during_outage(a, RegionId::new([9u8; 32])));
    }

    #[test]
    fn isolation_manager_not_isolated_without_outage() {
        let mut mgr = RegionalIsolationManager::new();
        let a = RegionId::new([1u8; 32]);
        let b = RegionId::new([2u8; 32]);
        mgr.set_isolation(a, b, true, Some(b));
        // b is not in outage, so not isolated due to outage.
        assert!(!mgr.region_isolated_during_outage(a, b));
    }

    #[test]
    fn isolation_manager_counts() {
        let mut mgr = RegionalIsolationManager::new();
        let a = RegionId::new([1u8; 32]);
        let b = RegionId::new([2u8; 32]);
        let c = RegionId::new([3u8; 32]);
        mgr.record_outage(b);
        mgr.set_isolation(a, b, true, Some(b));
        mgr.set_isolation(a, c, true, Some(c));
        mgr.record_outage(c);
        assert_eq!(mgr.isolated_count(), 2);
        assert_eq!(mgr.outage_regions().len(), 2);
    }

    #[test]
    fn isolation_status() {
        let mut mgr = RegionalIsolationManager::new();
        let a = RegionId::new([1u8; 32]);
        let b = RegionId::new([2u8; 32]);
        assert_eq!(mgr.isolation_status(a, b), None);
        mgr.set_isolation(a, b, false, None);
        assert_eq!(mgr.isolation_status(a, b), Some(false));
    }

    // ---- RegionalAutonomyConfig tests ----

    #[test]
    fn autonomy_config_default() {
        let config = RegionalAutonomyConfig::default();
        assert!(config.require_healthy_for_local);
        assert!(config.allow_cross_region_failover);
        assert_eq!(config.max_cross_region_ops, 100);
    }

    // ---- RegionalAutonomyManager tests ----

    #[test]
    fn manager_local_operation_allowed() {
        let mut manager = RegionalAutonomyManager::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        manager.register_local_replica(dataset, region);
        manager.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        assert!(manager.local_operation_allowed(region, dataset));
        assert_eq!(manager.local_allowed_count(), 1);
    }

    #[test]
    fn manager_cross_region_required() {
        let mut manager = RegionalAutonomyManager::new();
        assert!(manager.cross_region_required(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32])
        ));
        assert!(!manager.cross_region_required(
            RegionId::new([1u8; 32]),
            RegionId::new([1u8; 32])
        ));
        assert_eq!(manager.cross_region_required_count(), 1);
    }

    #[test]
    fn manager_region_isolated_during_outage() {
        let mut manager = RegionalAutonomyManager::new();
        let a = RegionId::new([1u8; 32]);
        let b = RegionId::new([2u8; 32]);
        manager.record_outage(b);
        manager.set_isolation(a, b, true, None);
        assert!(manager.region_isolated_during_outage(a, b));
    }

    #[test]
    fn manager_region_healthy() {
        let mut manager = RegionalAutonomyManager::new();
        let region = RegionId::new([1u8; 32]);
        manager.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: Vec::new(),
        });
        assert!(manager.region_healthy(region));
    }

    #[test]
    fn manager_local_serving_regions() {
        let mut manager = RegionalAutonomyManager::new();
        let dataset = DatasetId::new([2u8; 32]);
        let region = RegionId::new([1u8; 32]);
        manager.register_local_replica(dataset, region);
        manager.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        let serving = manager.local_serving_regions(dataset);
        assert_eq!(serving.len(), 1);
    }

    #[test]
    fn manager_record_cross_region_op() {
        let mut manager = RegionalAutonomyManager::new();
        let id = manager.record_cross_region_op(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            DatasetId::new([3u8; 32]),
            CrossRegionOpType::Replicate,
            true,
        );
        manager.complete_cross_region_op(id).unwrap();
        assert_eq!(manager.cross_region_detector().total_ops(), 1);
    }

    #[test]
    fn manager_local_ratio() {
        let mut manager = RegionalAutonomyManager::new();
        let region = RegionId::new([1u8; 32]);
        let dataset = DatasetId::new([2u8; 32]);
        manager.register_local_replica(dataset, region);
        manager.register_region_health(RegionHealthRecord {
            region_id: region,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: Vec::new(),
            local_datasets: vec![dataset],
        });
        manager.local_operation_allowed(region, dataset);
        manager.local_operation_allowed(RegionId::new([9u8; 32]), dataset);
        assert!((manager.local_ratio() - 0.5).abs() < 0.001);
    }

    #[test]
    fn manager_with_config() {
        let config = RegionalAutonomyConfig {
            require_healthy_for_local: false,
            allow_cross_region_failover: false,
            max_cross_region_ops: 50,
        };
        let manager = RegionalAutonomyManager::with_config(config);
        assert!(!manager.config.allow_cross_region_failover);
        assert_eq!(manager.config.max_cross_region_ops, 50);
    }

    // ---- Sealed trait unit tests ----

    #[test]
    fn sealed_unit_impl() {
        let unit = ();
        assert!(unit.local_operation_allowed(RegionId::new([1u8; 32]), DatasetId::new([2u8; 32])));
        assert!(!unit.cross_region_required(Operation::new([0u8; 32])));
        assert!(unit.region_isolated_during_outage(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32])
        ));
        assert!(unit.region_healthy(RegionId::new([1u8; 32])));
        assert!(unit.local_serving_regions(DatasetId::new([2u8; 32])).is_empty());
        assert_eq!(unit.region_count(), 0);
    }

    // ---- Operation tests ----

    #[test]
    fn operation_creation() {
        let op = Operation::new([1u8; 32]);
        let _ = format!("{:?}", op);
    }

    // ---- REGIONAL_AUTONOMY_BIT tests ----

    #[test]
    fn regional_autonomy_bit_position() {
        assert_eq!(REGIONAL_AUTONOMY_BIT, 1 << 11);
    }

    #[test]
    fn regional_autonomy_enabled_checks_bit() {
        use crate::versioning::EdgeCaseBits;
        let mut bits = EdgeCaseBits(0);
        assert!(!regional_autonomy_enabled(&bits));
        bits.enable(REGIONAL_AUTONOMY_BIT);
        assert!(regional_autonomy_enabled(&bits));
    }

    // ---- Integration test ==========

    #[test]
    fn full_regional_autonomy_integration() {
        let mut manager = RegionalAutonomyManager::new();
        let r1 = RegionId::new([1u8; 32]);
        let r2 = RegionId::new([2u8; 32]);
        let d1 = DatasetId::new([3u8; 32]);
        let d2 = DatasetId::new([4u8; 32]);

        // Register local replicas.
        manager.register_local_replica(d1, r1);
        manager.register_local_replica(d2, r2);

        // Register health.
        manager.register_region_health(RegionHealthRecord {
            region_id: r1,
            health: RegionHealth::Healthy,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: vec![r2],
            local_datasets: vec![d1],
        });
        manager.register_region_health(RegionHealthRecord {
            region_id: r2,
            health: RegionHealth::Degraded,
            last_heartbeat: TimestampMillis(0),
            outage_start: None,
            peer_regions: vec![r1],
            local_datasets: vec![d2],
        });

        // Local operation in r1 for d1.
        assert!(manager.local_operation_allowed(r1, d1));

        // Cross-region operation.
        assert!(manager.cross_region_required(r1, r2));

        // Record outage in r2.
        manager.record_outage(r2);
        assert!(manager.region_in_outage(r2));

        // Record cross-region op.
        let op_id = manager.record_cross_region_op(
            r1,
            r2,
            d1,
            CrossRegionOpType::Failover,
            true,
        );
        manager.complete_cross_region_op(op_id).unwrap();

        // Verify state.
        assert!(!manager.region_healthy(r2));
        assert_eq!(manager.cross_region_detector().completed_ops(), 1);
        assert!(manager.local_ratio() < 1.0);
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-Region Traffic Analyzer — analyzes traffic patterns across regions
///////////////////////////////////////////////////////////////////////////////

/// A cross-region traffic flow record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRegionTrafficFlow {
    pub flow_id: TrafficFlowId,
    pub source_region: RegionId,
    pub destination_region: RegionId,
    pub dataset: DatasetId,
    pub bytes_transferred: u64,
    pub operation_count: u32,
    pub timestamp: TimestampMillis,
}

/// Unique identifier for a traffic flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrafficFlowId(pub u64);

/// Analyzes cross-region traffic patterns.
pub struct CrossRegionTrafficAnalyzer {
    /// Traffic flows by ID.
    flows: HashMap<TrafficFlowId, CrossRegionTrafficFlow>,
    /// Per-source-region flow IDs.
    source_flows: HashMap<RegionId, Vec<TrafficFlowId>>,
    /// Per-destination-region flow IDs.
    dest_flows: HashMap<RegionId, Vec<TrafficFlowId>>,
    /// Per-region-pair aggregated bytes.
    pair_bytes: BTreeMap<(RegionId, RegionId), u64>,
    /// Next flow ID.
    next_id: u64,
    /// Total flows recorded.
    total_flows: u64,
    /// Total bytes transferred across regions.
    total_bytes: u64,
}

impl CrossRegionTrafficAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        CrossRegionTrafficAnalyzer {
            flows: HashMap::new(),
            source_flows: HashMap::new(),
            dest_flows: HashMap::new(),
            pair_bytes: BTreeMap::new(),
            next_id: 0,
            total_flows: 0,
            total_bytes: 0,
        }
    }

    /// Record a cross-region traffic flow.
    pub fn record_flow(
        &mut self,
        source_region: RegionId,
        destination_region: RegionId,
        dataset: DatasetId,
        bytes_transferred: u64,
        operation_count: u32,
    ) -> TrafficFlowId {
        let id = TrafficFlowId(self.next_id);
        self.next_id += 1;
        self.total_flows += 1;
        self.total_bytes += bytes_transferred;

        let flow = CrossRegionTrafficFlow {
            flow_id: id,
            source_region,
            destination_region,
            dataset,
            bytes_transferred,
            operation_count,
            timestamp: TimestampMillis(0),
        };
        self.flows.insert(id, flow);
        self.source_flows
            .entry(source_region)
            .or_default()
            .push(id);
        self.dest_flows
            .entry(destination_region)
            .or_default()
            .push(id);

        let pair_key = (source_region, destination_region);
        *self.pair_bytes.entry(pair_key).or_insert(0) += bytes_transferred;

        id
    }

    /// Return flows for a source region.
    pub fn flows_from_region(&self, region: RegionId) -> Vec<CrossRegionTrafficFlow> {
        self.source_flows
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.flows.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return flows for a destination region.
    pub fn flows_to_region(&self, region: RegionId) -> Vec<CrossRegionTrafficFlow> {
        self.dest_flows
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.flows.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return aggregated bytes for a region pair.
    pub fn pair_bytes(&self, source: RegionId, destination: RegionId) -> u64 {
        self.pair_bytes.get(&(source, destination)).copied().unwrap_or(0)
    }

    /// Return the busiest region pair.
    pub fn busiest_pair(&self) -> Option<((RegionId, RegionId), u64)> {
        self.pair_bytes
            .iter()
            .max_by_key(|(_, bytes)| *bytes)
            .map(|(pair, bytes)| (*pair, *bytes))
    }

    /// Return total flows.
    pub fn total_flows(&self) -> u64 {
        self.total_flows
    }

    /// Return total bytes transferred.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Return average bytes per flow.
    pub fn avg_bytes_per_flow(&self) -> f64 {
        if self.total_flows == 0 {
            return 0.0;
        }
        self.total_bytes as f64 / self.total_flows as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region Load Balancer — balances load across regions
///////////////////////////////////////////////////////////////////////////////

/// Load information for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionLoad {
    pub region_id: RegionId,
    pub cpu_load_percent: u32,
    pub memory_load_percent: u32,
    pub disk_load_percent: u32,
    pub network_load_percent: u32,
    pub active_operations: u32,
    pub queued_operations: u32,
}

impl RegionLoad {
    /// Return the overall load score (0-100).
    pub fn overall_load(&self) -> u32 {
        let loads = [
            self.cpu_load_percent,
            self.memory_load_percent,
            self.disk_load_percent,
            self.network_load_percent,
        ];
        loads.iter().sum::<u32>() / loads.len() as u32
    }

    /// Return whether the region is overloaded (load > 80%).
    pub fn is_overloaded(&self) -> bool {
        self.overall_load() > 80
    }
}

/// Balances load across regions.
pub struct RegionLoadBalancer {
    /// Load information per region.
    loads: HashMap<RegionId, RegionLoad>,
    /// Total load balancing decisions.
    total_decisions: u64,
    /// Total operations redirected.
    redirected_operations: u64,
}

impl RegionLoadBalancer {
    /// Create a new load balancer.
    pub fn new() -> Self {
        RegionLoadBalancer {
            loads: HashMap::new(),
            total_decisions: 0,
            redirected_operations: 0,
        }
    }

    /// Update load information for a region.
    pub fn update_load(&mut self, load: RegionLoad) {
        self.loads.insert(load.region_id, load);
    }

    /// Select the least-loaded region from candidates.
    pub fn select_least_loaded(&mut self, candidates: Vec<RegionId>) -> Option<RegionId> {
        self.total_decisions += 1;
        let best = candidates
            .iter()
            .filter_map(|r| self.loads.get(r).map(|l| (*r, l.overall_load())))
            .min_by_key(|(_, load)| *load)
            .map(|(r, _)| r);
        if best.is_some() {
            self.redirected_operations += 1;
        }
        best
    }

    /// Return whether a region is overloaded.
    pub fn is_region_overloaded(&self, region: RegionId) -> bool {
        self.loads
            .get(&region)
            .map(|l| l.is_overloaded())
            .unwrap_or(false)
    }

    /// Return overloaded regions.
    pub fn overloaded_regions(&self) -> Vec<RegionId> {
        self.loads
            .iter()
            .filter(|(_, l)| l.is_overloaded())
            .map(|(r, _)| *r)
            .collect()
    }

    /// Return load for a region.
    pub fn region_load(&self, region: RegionId) -> Option<&RegionLoad> {
        self.loads.get(&region)
    }

    /// Return total decisions.
    pub fn total_decisions(&self) -> u64 {
        self.total_decisions
    }

    /// Return redirected operations.
    pub fn redirected_operations(&self) -> u64 {
        self.redirected_operations
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region Failover Coordinator — coordinates failover between regions
///////////////////////////////////////////////////////////////////////////////

/// A failover operation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailoverOperation {
    pub operation_id: FailoverOperationId,
    pub failed_region: RegionId,
    pub target_region: RegionId,
    pub datasets: Vec<DatasetId>,
    pub status: FailoverStatus,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a failover operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FailoverOperationId(pub u64);

/// Status of a failover operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Aborted,
}

/// Coordinates failover operations between regions.
pub struct RegionFailoverCoordinator {
    /// Failover operations by ID.
    operations: HashMap<FailoverOperationId, FailoverOperation>,
    /// Per-failed-region operation IDs.
    failed_region_ops: HashMap<RegionId, Vec<FailoverOperationId>>,
    /// Next operation ID.
    next_id: u64,
    /// Total failovers.
    total_failovers: u64,
    /// Completed failovers.
    completed_failovers: u64,
}

impl RegionFailoverCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        RegionFailoverCoordinator {
            operations: HashMap::new(),
            failed_region_ops: HashMap::new(),
            next_id: 0,
            total_failovers: 0,
            completed_failovers: 0,
        }
    }

    /// Initiate a failover from a failed region to a target region.
    pub fn initiate_failover(
        &mut self,
        failed_region: RegionId,
        target_region: RegionId,
        datasets: Vec<DatasetId>,
    ) -> FailoverOperationId {
        let id = FailoverOperationId(self.next_id);
        self.next_id += 1;
        self.total_failovers += 1;
        let op = FailoverOperation {
            operation_id: id,
            failed_region,
            target_region,
            datasets,
            status: FailoverStatus::Pending,
            started_at: TimestampMillis(0),
            completed_at: None,
        };
        self.operations.insert(id, op);
        self.failed_region_ops
            .entry(failed_region)
            .or_default()
            .push(id);
        id
    }

    /// Mark a failover as in progress.
    pub fn start_failover(&mut self, operation_id: FailoverOperationId) -> Result<(), Error> {
        let op = self
            .operations
            .get_mut(&operation_id)
            .ok_or_else(|| Error::Internal {
                message: "failover operation not found".to_string(),
            })?;
        op.status = FailoverStatus::InProgress;
        Ok(())
    }

    /// Mark a failover as completed.
    pub fn complete_failover(&mut self, operation_id: FailoverOperationId) -> Result<(), Error> {
        let op = self
            .operations
            .get_mut(&operation_id)
            .ok_or_else(|| Error::Internal {
                message: "failover operation not found".to_string(),
            })?;
        op.status = FailoverStatus::Completed;
        op.completed_at = Some(TimestampMillis(0));
        self.completed_failovers += 1;
        Ok(())
    }

    /// Mark a failover as failed.
    pub fn fail_failover(&mut self, operation_id: FailoverOperationId) -> Result<(), Error> {
        let op = self
            .operations
            .get_mut(&operation_id)
            .ok_or_else(|| Error::Internal {
                message: "failover operation not found".to_string(),
            })?;
        op.status = FailoverStatus::Failed;
        op.completed_at = Some(TimestampMillis(0));
        Ok(())
    }

    /// Return failovers for a failed region.
    pub fn failovers_for_region(&self, region: RegionId) -> Vec<FailoverOperation> {
        self.failed_region_ops
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.operations.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total failovers.
    pub fn total_failovers(&self) -> u64 {
        self.total_failovers
    }

    /// Return completed failover count.
    pub fn completed_failovers(&self) -> u64 {
        self.completed_failovers
    }

    /// Return failover success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_failovers == 0 {
            return 1.0;
        }
        self.completed_failovers as f64 / self.total_failovers as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Regional Quota Manager — enforces quotas per region
///////////////////////////////////////////////////////////////////////////////

/// A quota for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalQuota {
    pub region_id: RegionId,
    pub max_bytes: u64,
    pub max_operations: u32,
    pub current_bytes: u64,
    pub current_operations: u32,
}

impl RegionalQuota {
    /// Return whether the quota is exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.current_bytes > self.max_bytes || self.current_operations > self.max_operations
    }

    /// Return the byte utilization ratio (0.0 to 1.0+).
    pub fn byte_utilization(&self) -> f64 {
        if self.max_bytes == 0 {
            return 0.0;
        }
        self.current_bytes as f64 / self.max_bytes as f64
    }

    /// Return the operation utilization ratio (0.0 to 1.0+).
    pub fn operation_utilization(&self) -> f64 {
        if self.max_operations == 0 {
            return 0.0;
        }
        self.current_operations as f64 / self.max_operations as f64
    }
}

/// Enforces quotas per region.
pub struct RegionalQuotaManager {
    /// Quotas per region.
    quotas: HashMap<RegionId, RegionalQuota>,
    /// Total quota violations.
    total_violations: u64,
    /// Total quota checks.
    total_checks: u64,
}

impl RegionalQuotaManager {
    /// Create a new quota manager.
    pub fn new() -> Self {
        RegionalQuotaManager {
            quotas: HashMap::new(),
            total_violations: 0,
            total_checks: 0,
        }
    }

    /// Set a quota for a region.
    pub fn set_quota(&mut self, region: RegionId, max_bytes: u64, max_operations: u32) {
        self.quotas.insert(
            region,
            RegionalQuota {
                region_id: region,
                max_bytes,
                max_operations,
                current_bytes: 0,
                current_operations: 0,
            },
        );
    }

    /// Check whether an operation fits within quota.
    pub fn check_quota(
        &mut self,
        region: RegionId,
        bytes: u64,
        operations: u32,
    ) -> Result<(), QuotaExceededError> {
        self.total_checks += 1;
        let quota = match self.quotas.get(&region) {
            Some(q) => q,
            None => return Ok(()), // No quota set, allow
        };

        if quota.current_bytes + bytes > quota.max_bytes
            || quota.current_operations + operations > quota.max_operations
        {
            self.total_violations += 1;
            return Err(QuotaExceededError {
                region,
                requested_bytes: bytes,
                requested_operations: operations,
                available_bytes: quota.max_bytes.saturating_sub(quota.current_bytes),
                available_operations: quota.max_operations.saturating_sub(quota.current_operations),
            });
        }
        Ok(())
    }

    /// Record resource usage against quota.
    pub fn record_usage(&mut self, region: RegionId, bytes: u64, operations: u32) {
        if let Some(quota) = self.quotas.get_mut(&region) {
            quota.current_bytes = quota.current_bytes.saturating_add(bytes);
            quota.current_operations = quota.current_operations.saturating_add(operations);
        }
    }

    /// Return whether a region's quota is exceeded.
    pub fn is_quota_exceeded(&self, region: RegionId) -> bool {
        self.quotas
            .get(&region)
            .map(|q| q.is_exceeded())
            .unwrap_or(false)
    }

    /// Return total violations.
    pub fn total_violations(&self) -> u64 {
        self.total_violations
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return violation rate.
    pub fn violation_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 0.0;
        }
        self.total_violations as f64 / self.total_checks as f64
    }
}

/// Error when a regional quota is exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaExceededError {
    pub region: RegionId,
    pub requested_bytes: u64,
    pub requested_operations: u32,
    pub available_bytes: u64,
    pub available_operations: u32,
}

///////////////////////////////////////////////////////////////////////////////
// Region Affinity Tracker — tracks client-to-region affinity
///////////////////////////////////////////////////////////////////////////////

/// A client-to-region affinity record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRegionAffinity {
    pub client_id: NodeId,
    pub region_id: RegionId,
    pub affinity_score: f64,
    pub last_access: TimestampMillis,
    pub access_count: u32,
}

/// Tracks client-to-region affinity for routing decisions.
pub struct RegionAffinityTracker {
    /// Affinities by client ID.
    affinities: HashMap<NodeId, ClientRegionAffinity>,
    /// Per-region client IDs.
    region_clients: HashMap<RegionId, Vec<NodeId>>,
    /// Total affinity lookups.
    total_lookups: u64,
    /// Total affinity hits (client has affinity for requested region).
    affinity_hits: u64,
}

impl RegionAffinityTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        RegionAffinityTracker {
            affinities: HashMap::new(),
            region_clients: HashMap::new(),
            total_lookups: 0,
            affinity_hits: 0,
        }
    }

    /// Record client access to a region.
    pub fn record_access(&mut self, client: NodeId, region: RegionId) {
        self.total_lookups += 1;
        let affinity = self.affinities.entry(client).or_insert(ClientRegionAffinity {
            client_id: client,
            region_id: region,
            affinity_score: 0.0,
            last_access: TimestampMillis(0),
            access_count: 0,
        });
        affinity.access_count += 1;
        affinity.last_access = TimestampMillis(0);
        if affinity.region_id == region {
            self.affinity_hits += 1;
            affinity.affinity_score = (affinity.affinity_score + 1.0).min(10.0);
        } else {
            affinity.region_id = region;
            affinity.affinity_score = affinity.affinity_score.max(0.0);
        }
        self.region_clients.entry(region).or_default().push(client);
    }

    /// Return the affinity for a client.
    pub fn affinity_for_client(&self, client: NodeId) -> Option<&ClientRegionAffinity> {
        self.affinities.get(&client)
    }

    /// Return clients associated with a region.
    pub fn clients_for_region(&self, region: RegionId) -> Vec<NodeId> {
        self.region_clients.get(&region).cloned().unwrap_or_default()
    }

    /// Return total lookups.
    pub fn total_lookups(&self) -> u64 {
        self.total_lookups
    }

    /// Return affinity hits.
    pub fn affinity_hits(&self) -> u64 {
        self.affinity_hits
    }

    /// Return affinity hit rate.
    pub fn hit_rate(&self) -> f64 {
        if self.total_lookups == 0 {
            return 0.0;
        }
        self.affinity_hits as f64 / self.total_lookups as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-Region Sync Manager — coordinates sync across regions
///////////////////////////////////////////////////////////////////////////////

/// A cross-region sync operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRegionSync {
    pub sync_id: SyncId,
    pub source_region: RegionId,
    pub target_regions: Vec<RegionId>,
    pub dataset: DatasetId,
    pub status: SyncStatus,
    pub bytes_synced: u64,
    pub total_bytes: u64,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SyncId(pub u64);

/// Status of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartiallyCompleted,
}

/// Coordinates synchronization across regions.
pub struct CrossRegionSyncManager {
    /// Sync operations by ID.
    syncs: HashMap<SyncId, CrossRegionSync>,
    /// Per-source-region sync IDs.
    source_syncs: HashMap<RegionId, Vec<SyncId>>,
    /// Next sync ID.
    next_id: u64,
    /// Total syncs.
    total_syncs: u64,
    /// Total bytes synced.
    total_bytes_synced: u64,
}

impl CrossRegionSyncManager {
    /// Create a new sync manager.
    pub fn new() -> Self {
        CrossRegionSyncManager {
            syncs: HashMap::new(),
            source_syncs: HashMap::new(),
            next_id: 0,
            total_syncs: 0,
            total_bytes_synced: 0,
        }
    }

    /// Initiate a cross-region sync.
    pub fn initiate_sync(
        &mut self,
        source_region: RegionId,
        target_regions: Vec<RegionId>,
        dataset: DatasetId,
        total_bytes: u64,
    ) -> SyncId {
        let id = SyncId(self.next_id);
        self.next_id += 1;
        self.total_syncs += 1;
        let sync = CrossRegionSync {
            sync_id: id,
            source_region,
            target_regions: target_regions.clone(),
            dataset,
            status: SyncStatus::Pending,
            bytes_synced: 0,
            total_bytes,
            started_at: TimestampMillis(0),
            completed_at: None,
        };
        self.syncs.insert(id, sync);
        self.source_syncs
            .entry(source_region)
            .or_default()
            .push(id);
        id
    }

    /// Update sync progress.
    pub fn update_progress(
        &mut self,
        sync_id: SyncId,
        bytes_synced: u64,
    ) -> Result<(), Error> {
        let sync = self
            .syncs
            .get_mut(&sync_id)
            .ok_or_else(|| Error::Internal {
                message: "sync operation not found".to_string(),
            })?;
        sync.bytes_synced = bytes_synced;
        if bytes_synced >= sync.total_bytes {
            sync.status = SyncStatus::Completed;
            sync.completed_at = Some(TimestampMillis(0));
            self.total_bytes_synced += bytes_synced;
        } else if bytes_synced > 0 {
            sync.status = SyncStatus::InProgress;
        }
        Ok(())
    }

    /// Mark a sync as failed.
    pub fn fail_sync(&mut self, sync_id: SyncId) -> Result<(), Error> {
        let sync = self
            .syncs
            .get_mut(&sync_id)
            .ok_or_else(|| Error::Internal {
                message: "sync operation not found".to_string(),
            })?;
        sync.status = SyncStatus::Failed;
        sync.completed_at = Some(TimestampMillis(0));
        Ok(())
    }

    /// Return syncs for a source region.
    pub fn syncs_for_region(&self, region: RegionId) -> Vec<CrossRegionSync> {
        self.source_syncs
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.syncs.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total syncs.
    pub fn total_syncs(&self) -> u64 {
        self.total_syncs
    }

    /// Return total bytes synced.
    pub fn total_bytes_synced(&self) -> u64 {
        self.total_bytes_synced
    }
}

///////////////////////////////////////////////////////////////////////////////
// Regional Circuit Breaker — circuit breaker for region calls
///////////////////////////////////////////////////////////////////////////////

/// A circuit breaker state for a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionCircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// A regional circuit breaker record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionCircuitBreaker {
    pub region_id: RegionId,
    pub state: RegionCircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure: Option<TimestampMillis>,
    pub last_success: Option<TimestampMillis>,
}

/// Manages circuit breakers for regional calls.
pub struct RegionalCircuitBreakerManager {
    /// Circuit breakers per region.
    breakers: HashMap<RegionId, RegionCircuitBreaker>,
    /// Failure threshold to open circuit.
    failure_threshold: u32,
    /// Success threshold to close circuit from half-open.
    success_threshold: u32,
    /// Total circuit trips.
    total_trips: u64,
    /// Total calls allowed.
    total_allowed: u64,
    /// Total calls rejected.
    total_rejected: u64,
}

impl RegionalCircuitBreakerManager {
    /// Create a new manager.
    pub fn new() -> Self {
        RegionalCircuitBreakerManager {
            breakers: HashMap::new(),
            failure_threshold: 5,
            success_threshold: 3,
            total_trips: 0,
            total_allowed: 0,
            total_rejected: 0,
        }
    }

    /// Check whether a call to a region is allowed.
    pub fn is_call_allowed(&mut self, region: RegionId) -> bool {
        let breaker = self.breakers.entry(region).or_insert(RegionCircuitBreaker {
            region_id: region,
            state: RegionCircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            last_success: None,
        });

        match breaker.state {
            RegionCircuitState::Open => {
                self.total_rejected += 1;
                false
            }
            RegionCircuitState::Closed | RegionCircuitState::HalfOpen => {
                self.total_allowed += 1;
                true
            }
        }
    }

    /// Record a successful call to a region.
    pub fn record_success(&mut self, region: RegionId) {
        let breaker = self.breakers.entry(region).or_insert(RegionCircuitBreaker {
            region_id: region,
            state: RegionCircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            last_success: None,
        });
        breaker.success_count += 1;
        breaker.last_success = Some(TimestampMillis(0));
        breaker.failure_count = breaker.failure_count.saturating_sub(1);

        if breaker.state == RegionCircuitState::HalfOpen
            && breaker.success_count >= self.success_threshold
        {
            breaker.state = RegionCircuitState::Closed;
            breaker.failure_count = 0;
        }
    }

    /// Record a failed call to a region.
    pub fn record_failure(&mut self, region: RegionId) {
        let breaker = self.breakers.entry(region).or_insert(RegionCircuitBreaker {
            region_id: region,
            state: RegionCircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure: None,
            last_success: None,
        });
        breaker.failure_count += 1;
        breaker.last_failure = Some(TimestampMillis(0));
        breaker.success_count = breaker.success_count.saturating_sub(1);

        if breaker.failure_count >= self.failure_threshold {
            breaker.state = RegionCircuitState::Open;
            self.total_trips += 1;
        }
    }

    /// Return the circuit state for a region.
    pub fn circuit_state(&self, region: RegionId) -> RegionCircuitState {
        self.breakers
            .get(&region)
            .map(|b| b.state)
            .unwrap_or(RegionCircuitState::Closed)
    }

    /// Return total circuit trips.
    pub fn total_trips(&self) -> u64 {
        self.total_trips
    }

    /// Return total allowed calls.
    pub fn total_allowed(&self) -> u64 {
        self.total_allowed
    }

    /// Return total rejected calls.
    pub fn total_rejected(&self) -> u64 {
        self.total_rejected
    }

    /// Set failure threshold.
    pub fn set_failure_threshold(&mut self, threshold: u32) {
        self.failure_threshold = threshold;
    }

    /// Set success threshold.
    pub fn set_success_threshold(&mut self, threshold: u32) {
        self.success_threshold = threshold;
    }
}

// ========== CrossRegionTrafficAnalyzer tests ==========

#[test]
fn traffic_analyzer_record_flow() {
    let mut analyzer = CrossRegionTrafficAnalyzer::new();
    let id = analyzer.record_flow(
        RegionId::new([1u8; 32]),
        RegionId::new([2u8; 32]),
        DatasetId::new([3u8; 32]),
        1024,
        5,
    );
    assert_eq!(id, TrafficFlowId(0));
    assert_eq!(analyzer.total_flows(), 1);
    assert_eq!(analyzer.total_bytes(), 1024);
    assert_eq!(analyzer.pair_bytes(RegionId::new([1u8; 32]), RegionId::new([2u8; 32])), 1024);
    assert_eq!(analyzer.avg_bytes_per_flow(), 1024.0);
}

#[test]
fn traffic_analyzer_flows_from_and_to() {
    let mut analyzer = CrossRegionTrafficAnalyzer::new();
    let src = RegionId::new([1u8; 32]);
    let dst = RegionId::new([2u8; 32]);
    let data = DatasetId::new([3u8; 32]);
    analyzer.record_flow(src, dst, data, 100, 1);
    analyzer.record_flow(src, dst, data, 200, 2);
    analyzer.record_flow(dst, src, data, 300, 3);

    assert_eq!(analyzer.flows_from_region(src).len(), 2);
    assert_eq!(analyzer.flows_to_region(dst).len(), 2);
    assert_eq!(analyzer.flows_from_region(dst).len(), 1);
    assert_eq!(analyzer.flows_to_region(src).len(), 1);
}

#[test]
fn traffic_analyzer_busiest_pair() {
    let mut analyzer = CrossRegionTrafficAnalyzer::new();
    analyzer.record_flow(
        RegionId::new([1u8; 32]),
        RegionId::new([2u8; 32]),
        DatasetId::new([3u8; 32]),
        100,
        1,
    );
    analyzer.record_flow(
        RegionId::new([1u8; 32]),
        RegionId::new([2u8; 32]),
        DatasetId::new([3u8; 32]),
        500,
        1,
    );
    analyzer.record_flow(
        RegionId::new([2u8; 32]),
        RegionId::new([3u8; 32]),
        DatasetId::new([3u8; 32]),
        50,
        1,
    );
    let busiest = analyzer.busiest_pair().unwrap();
    assert_eq!(busiest.1, 600);
}

#[test]
fn traffic_analyzer_empty() {
    let analyzer = CrossRegionTrafficAnalyzer::new();
    assert_eq!(analyzer.total_flows(), 0);
    assert_eq!(analyzer.total_bytes(), 0);
    assert_eq!(analyzer.avg_bytes_per_flow(), 0.0);
    assert!(analyzer.busiest_pair().is_none());
}

// ========== RegionLoadBalancer tests ==========

#[test]
fn load_balancer_select_least_loaded() {
    let mut balancer = RegionLoadBalancer::new();
    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    let r3 = RegionId::new([3u8; 32]);
    balancer.update_load(RegionLoad {
        region_id: r1,
        cpu_load_percent: 50,
        memory_load_percent: 50,
        disk_load_percent: 50,
        network_load_percent: 50,
        active_operations: 10,
        queued_operations: 0,
    });
    balancer.update_load(RegionLoad {
        region_id: r2,
        cpu_load_percent: 10,
        memory_load_percent: 10,
        disk_load_percent: 10,
        network_load_percent: 10,
        active_operations: 1,
        queued_operations: 0,
    });
    balancer.update_load(RegionLoad {
        region_id: r3,
        cpu_load_percent: 90,
        memory_load_percent: 90,
        disk_load_percent: 90,
        network_load_percent: 90,
        active_operations: 100,
        queued_operations: 50,
    });

    let selected = balancer.select_least_loaded(vec![r1, r2, r3]).unwrap();
    assert_eq!(selected, r2);
    assert_eq!(balancer.total_decisions(), 1);
    assert_eq!(balancer.redirected_operations(), 1);
}

#[test]
fn load_balancer_overloaded() {
    let mut balancer = RegionLoadBalancer::new();
    let r1 = RegionId::new([1u8; 32]);
    balancer.update_load(RegionLoad {
        region_id: r1,
        cpu_load_percent: 90,
        memory_load_percent: 90,
        disk_load_percent: 90,
        network_load_percent: 90,
        active_operations: 0,
        queued_operations: 0,
    });
    assert!(balancer.is_region_overloaded(r1));
    assert_eq!(balancer.overloaded_regions().len(), 1);
}

#[test]
fn load_balancer_empty_candidates() {
    let mut balancer = RegionLoadBalancer::new();
    assert!(balancer.select_least_loaded(vec![]).is_none());
    assert_eq!(balancer.total_decisions(), 1);
}

// ========== FailoverStatus tests ==========

#[test]
fn failover_status_variants() {
    let statuses = [
        FailoverStatus::Pending,
        FailoverStatus::InProgress,
        FailoverStatus::Completed,
        FailoverStatus::Failed,
        FailoverStatus::Aborted,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== RegionFailoverCoordinator tests ==========

#[test]
fn failover_coordinator_initiate_and_complete() {
    let mut coordinator = RegionFailoverCoordinator::new();
    let failed = RegionId::new([1u8; 32]);
    let target = RegionId::new([2u8; 32]);
    let datasets = vec![DatasetId::new([3u8; 32]), DatasetId::new([4u8; 32])];
    let id = coordinator.initiate_failover(failed, target, datasets.clone());
    assert_eq!(id, FailoverOperationId(0));
    assert_eq!(coordinator.total_failovers(), 1);

    coordinator.start_failover(id).unwrap();
    coordinator.complete_failover(id).unwrap();
    assert_eq!(coordinator.completed_failovers(), 1);
    assert!((coordinator.success_rate() - 1.0).abs() < 0.001);
}

#[test]
fn failover_coordinator_fail_failover() {
    let mut coordinator = RegionFailoverCoordinator::new();
    let id = coordinator.initiate_failover(
        RegionId::new([1u8; 32]),
        RegionId::new([2u8; 32]),
        vec![],
    );
    coordinator.start_failover(id).unwrap();
    coordinator.fail_failover(id).unwrap();
    assert_eq!(coordinator.completed_failovers(), 0);
    assert!((coordinator.success_rate() - 0.0).abs() < 0.001);
}

#[test]
fn failover_coordinator_failovers_for_region() {
    let mut coordinator = RegionFailoverCoordinator::new();
    let failed1 = RegionId::new([1u8; 32]);
    let failed2 = RegionId::new([2u8; 32]);
    coordinator.initiate_failover(failed1, RegionId::new([3u8; 32]), vec![]);
    coordinator.initiate_failover(failed1, RegionId::new([3u8; 32]), vec![]);
    coordinator.initiate_failover(failed2, RegionId::new([3u8; 32]), vec![]);
    assert_eq!(coordinator.failovers_for_region(failed1).len(), 2);
    assert_eq!(coordinator.failovers_for_region(failed2).len(), 1);
}

#[test]
fn failover_coordinator_complete_nonexistent() {
    let mut coordinator = RegionFailoverCoordinator::new();
    assert!(coordinator.complete_failover(FailoverOperationId(999)).is_err());
}

// ========== QuotaExceededError tests ==========

#[test]
fn quota_exceeded_error_creation() {
    let err = QuotaExceededError {
        region: RegionId::new([1u8; 32]),
        requested_bytes: 1000,
        requested_operations: 10,
        available_bytes: 100,
        available_operations: 2,
    };
    assert_eq!(err.requested_bytes, 1000);
    assert_eq!(err.available_bytes, 100);
    let _ = format!("{:?}", err);
}

// ========== RegionalQuotaManager tests ==========

#[test]
fn quota_manager_check_and_exceed() {
    let mut manager = RegionalQuotaManager::new();
    let region = RegionId::new([1u8; 32]);
    manager.set_quota(region, 1000, 10);

    assert!(manager.check_quota(region, 500, 5).is_ok());
    manager.record_usage(region, 500, 5);
    assert!(manager.check_quota(region, 600, 6).is_err());
    assert_eq!(manager.total_violations(), 1);
    assert_eq!(manager.total_checks(), 2);
    assert!((manager.violation_rate() - 0.5).abs() < 0.001);
}

#[test]
fn quota_manager_no_quota_allowed() {
    let mut manager = RegionalQuotaManager::new();
    assert!(manager.check_quota(RegionId::new([1u8; 32]), 999999, 999).is_ok());
}

#[test]
fn quota_manager_is_exceeded() {
    let mut manager = RegionalQuotaManager::new();
    let region = RegionId::new([1u8; 32]);
    manager.set_quota(region, 100, 10);
    manager.record_usage(region, 150, 5);
    assert!(manager.is_quota_exceeded(region));
}

#[test]
fn quota_manager_byte_utilization() {
    let quota = RegionalQuota {
        region_id: RegionId::new([1u8; 32]),
        max_bytes: 1000,
        max_operations: 10,
        current_bytes: 300,
        current_operations: 2,
    };
    assert!((quota.byte_utilization() - 0.3).abs() < 0.001);
    assert!((quota.operation_utilization() - 0.2).abs() < 0.001);
    assert!(!quota.is_exceeded());
}

// ========== ClientRegionAffinity tests ==========

#[test]
fn client_region_affinity_creation() {
    let affinity = ClientRegionAffinity {
        client_id: NodeId::new([1u8; 32]),
        region_id: RegionId::new([2u8; 32]),
        affinity_score: 5.0,
        last_access: TimestampMillis(0),
        access_count: 3,
    };
    assert_eq!(affinity.affinity_score, 5.0);
    assert_eq!(affinity.access_count, 3);
}

// ========== RegionAffinityTracker tests ==========

#[test]
fn affinity_tracker_record_access() {
    let mut tracker = RegionAffinityTracker::new();
    let client = NodeId::new([1u8; 32]);
    let region = RegionId::new([2u8; 32]);
    tracker.record_access(client, region);
    tracker.record_access(client, region);
    tracker.record_access(client, RegionId::new([3u8; 32]));

    let aff = tracker.affinity_for_client(client).unwrap();
    assert_eq!(aff.access_count, 3);
    assert_eq!(tracker.total_lookups(), 3);
    assert_eq!(tracker.affinity_hits(), 2);
    assert!((tracker.hit_rate() - (2.0 / 3.0)).abs() < 0.001);
}

#[test]
fn affinity_tracker_clients_for_region() {
    let mut tracker = RegionAffinityTracker::new();
    let c1 = NodeId::new([1u8; 32]);
    let c2 = NodeId::new([2u8; 32]);
    let c3 = NodeId::new([3u8; 32]);
    let r1 = RegionId::new([1u8; 32]);
    tracker.record_access(c1, r1);
    tracker.record_access(c2, r1);
    tracker.record_access(c3, RegionId::new([2u8; 32]));
    assert_eq!(tracker.clients_for_region(r1).len(), 2);
}

#[test]
fn affinity_tracker_empty() {
    let tracker = RegionAffinityTracker::new();
    assert_eq!(tracker.total_lookups(), 0);
    assert_eq!(tracker.affinity_hits(), 0);
    assert_eq!(tracker.hit_rate(), 0.0);
    assert!(tracker.clients_for_region(RegionId::new([1u8; 32])).is_empty());
}

// ========== SyncStatus tests ==========

#[test]
fn sync_status_variants() {
    let statuses = [
        SyncStatus::Pending,
        SyncStatus::InProgress,
        SyncStatus::Completed,
        SyncStatus::Failed,
        SyncStatus::PartiallyCompleted,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== CrossRegionSyncManager tests ==========

#[test]
fn sync_manager_initiate_and_complete() {
    let mut manager = CrossRegionSyncManager::new();
    let source = RegionId::new([1u8; 32]);
    let targets = vec![RegionId::new([2u8; 32]), RegionId::new([3u8; 32])];
    let id = manager.initiate_sync(source, targets, DatasetId::new([4u8; 32]), 1024);
    assert_eq!(id, SyncId(0));
    assert_eq!(manager.total_syncs(), 1);

    manager.update_progress(id, 512).unwrap();
    assert_eq!(manager.syncs_for_region(source).first().unwrap().bytes_synced, 512);

    manager.update_progress(id, 1024).unwrap();
    let sync = manager.syncs_for_region(source).first().unwrap();
    assert_eq!(sync.status, SyncStatus::Completed);
    assert_eq!(manager.total_bytes_synced(), 1024);
}

#[test]
fn sync_manager_fail_sync() {
    let mut manager = CrossRegionSyncManager::new();
    let id = manager.initiate_sync(
        RegionId::new([1u8; 32]),
        vec![RegionId::new([2u8; 32])],
        DatasetId::new([3u8; 32]),
        1024,
    );
    manager.fail_sync(id).unwrap();
    assert_eq!(manager.syncs_for_region(RegionId::new([1u8; 32])).first().unwrap().status, SyncStatus::Failed);
}

#[test]
fn sync_manager_update_nonexistent() {
    let mut manager = CrossRegionSyncManager::new();
    assert!(manager.update_progress(SyncId(999), 100).is_err());
}

// ========== RegionCircuitState tests ==========

#[test]
fn region_circuit_state_variants() {
    let states = [
        RegionCircuitState::Closed,
        RegionCircuitState::Open,
        RegionCircuitState::HalfOpen,
    ];
    for s in states {
        let _ = format!("{:?}", s);
    }
}

// ========== RegionalCircuitBreakerManager tests ==========

#[test]
fn circuit_breaker_allow_and_reject() {
    let mut mgr = RegionalCircuitBreakerManager::new();
    let region = RegionId::new([1u8; 32]);
    mgr.set_failure_threshold(3);

    // First 3 calls allowed.
    for _ in 0..3 {
        assert!(mgr.is_call_allowed(region));
        mgr.record_failure(region);
    }
    // Circuit now open.
    assert!(!mgr.is_call_allowed(region));
    assert_eq!(mgr.total_rejected(), 1);
    assert_eq!(mgr.total_trips(), 1);
}

#[test]
fn circuit_breaker_success_recovers() {
    let mut mgr = RegionalCircuitBreakerManager::new();
    let region = RegionId::new([1u8; 32]);
    mgr.set_failure_threshold(2);
    mgr.set_success_threshold(2);

    // Trip the circuit.
    mgr.record_failure(region);
    mgr.record_failure(region);
    assert_eq!(mgr.circuit_state(region), RegionCircuitState::Open);

    // Reject calls while open.
    assert!(!mgr.is_call_allowed(region));

    // Manually set to half-open and record successes.
    let breaker = mgr.breakers.entry(region).or_insert(RegionCircuitBreaker {
        region_id: region,
        state: RegionCircuitState::HalfOpen,
        failure_count: 0,
        success_count: 0,
        last_failure: None,
        last_success: None,
    });
    breaker.success_count = 0;
    mgr.record_success(region);
    mgr.record_success(region);
    assert_eq!(mgr.circuit_state(region), RegionCircuitState::Closed);
}

#[test]
fn circuit_breaker_counts() {
    let mut mgr = RegionalCircuitBreakerManager::new();
    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    mgr.is_call_allowed(r1);
    mgr.is_call_allowed(r1);
    mgr.is_call_allowed(r2);
    mgr.record_failure(r1);
    mgr.record_success(r1);
    assert_eq!(mgr.total_allowed(), 3);
    assert_eq!(mgr.total_trips(), 0);
}

// ========== CrossRegionOpType extra tests ==========

#[test]
fn cross_region_op_type_all() {
    let types = [
        CrossRegionOpType::Replicate,
        CrossRegionOpType::Rebalance,
        CrossRegionOpType::Failover,
        CrossRegionOpType::Query,
        CrossRegionOpType::Write,
        CrossRegionOpType::Read,
    ];
    for t in types {
        assert_ne!(format!("{:?}", t), "");
    }
}

// ========== TrafficFlowId tests ==========

#[test]
fn traffic_flow_id_ordering() {
    let a = TrafficFlowId(1);
    let b = TrafficFlowId(2);
    assert!(a < b);
    assert_ne!(a, b);
}

// ========== FailoverOperationId tests ==========

#[test]
fn failover_operation_id_ordering() {
    let a = FailoverOperationId(10);
    let b = FailoverOperationId(20);
    assert!(a < b);
    assert_ne!(a, b);
}

// ========== SyncId tests ==========

#[test]
fn sync_id_ordering() {
    let a = SyncId(5);
    let b = SyncId(15);
    assert!(a < b);
    assert_ne!(a, b);
}

// ========== RegionCircuitBreaker tests ==========

#[test]
fn region_circuit_breaker_creation() {
    let breaker = RegionCircuitBreaker {
        region_id: RegionId::new([1u8; 32]),
        state: RegionCircuitState::Closed,
        failure_count: 0,
        success_count: 10,
        last_failure: None,
        last_success: Some(TimestampMillis(0)),
    };
    assert_eq!(breaker.state, RegionCircuitState::Closed);
    assert_eq!(breaker.success_count, 10);
}

// ========== RegionLoad tests ==========

#[test]
fn region_load_creation() {
    let load = RegionLoad {
        region_id: RegionId::new([1u8; 32]),
        cpu_load_percent: 80,
        memory_load_percent: 70,
        disk_load_percent: 60,
        network_load_percent: 50,
        active_operations: 5,
        queued_operations: 2,
    };
    assert_eq!(load.overall_load(), 65);
    assert!(!load.is_overloaded());
}

// ========== Full regional autonomy integration v2 ==========

#[test]
fn full_regional_autonomy_integration_v2() {
    let mut manager = RegionalAutonomyManager::new();
    let mut traffic = CrossRegionTrafficAnalyzer::new();
    let mut balancer = RegionLoadBalancer::new();
    let mut failover = RegionFailoverCoordinator::new();
    let mut quota = RegionalQuotaManager::new();
    let mut affinity = RegionAffinityTracker::new();
    let mut sync = CrossRegionSyncManager::new();
    let mut breaker = RegionalCircuitBreakerManager::new();

    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    let r3 = RegionId::new([3u8; 32]);
    let d1 = DatasetId::new([4u8; 32]);
    let d2 = DatasetId::new([5u8; 32]);
    let c1 = NodeId::new([6u8; 32]);

    // Register local replicas.
    manager.register_local_replica(d1, r1);
    manager.register_local_replica(d2, r2);

    // Register health.
    manager.register_region_health(RegionHealthRecord {
        region_id: r1,
        health: RegionHealth::Healthy,
        last_heartbeat: TimestampMillis(0),
        outage_start: None,
        peer_regions: vec![r2, r3],
        local_datasets: vec![d1],
    });
    manager.register_region_health(RegionHealthRecord {
        region_id: r2,
        health: RegionHealth::Degraded,
        last_heartbeat: TimestampMillis(0),
        outage_start: None,
        peer_regions: vec![r1, r3],
        local_datasets: vec![d2],
    });

    // Record traffic flows.
    traffic.record_flow(r1, r2, d1, 2048, 10);
    traffic.record_flow(r2, r3, d2, 4096, 20);
    assert_eq!(traffic.total_flows(), 2);
    assert_eq!(traffic.total_bytes(), 6144);

    // Update loads.
    balancer.update_load(RegionLoad {
        region_id: r1,
        cpu_load_percent: 30,
        memory_load_percent: 30,
        disk_load_percent: 30,
        network_load_percent: 30,
        active_operations: 1,
        queued_operations: 0,
    });
    balancer.update_load(RegionLoad {
        region_id: r2,
        cpu_load_percent: 90,
        memory_load_percent: 90,
        disk_load_percent: 90,
        network_load_percent: 90,
        active_operations: 50,
        queued_operations: 20,
    });

    // Select least loaded for failover target.
    let target = balancer.select_least_loaded(vec![r1, r2, r3]).unwrap();
    assert_eq!(target, r1);

    // Initiate failover from r2 to r1.
    let fo_id = failover.initiate_failover(r2, r1, vec![d2]);
    failover.start_failover(fo_id).unwrap();
    failover.complete_failover(fo_id).unwrap();
    assert_eq!(failover.completed_failovers(), 1);

    // Set quota.
    quota.set_quota(r1, 10000, 100);
    assert!(quota.check_quota(r1, 5000, 50).is_ok());
    quota.record_usage(r1, 5000, 50);
    assert!(quota.check_quota(r1, 6000, 60).is_err());

    // Record affinity.
    affinity.record_access(c1, r1);
    affinity.record_access(c1, r1);
    affinity.record_access(c1, r2);
    assert!((affinity.hit_rate() - (2.0 / 3.0)).abs() < 0.001);

    // Initiate sync.
    let sync_id = sync.initiate_sync(r1, vec![r2, r3], d1, 8192);
    sync.update_progress(sync_id, 8192).unwrap();
    assert_eq!(sync.total_bytes_synced(), 8192);

    // Circuit breaker.
    breaker.record_failure(r2);
    breaker.record_failure(r2);
    breaker.record_failure(r2);
    breaker.record_failure(r2);
    breaker.record_failure(r2);
    assert_eq!(breaker.circuit_state(r2), RegionCircuitState::Open);
    assert!(!breaker.is_call_allowed(r2));

    // Verify local operation still works for r1.
    assert!(manager.local_operation_allowed(r1, d1));

    // Verify cross-region.
    assert!(manager.cross_region_required(r1, r2));

    // Verify isolation.
    manager.record_outage(r3);
    manager.set_isolation(r1, r3, true, None);
    assert!(manager.region_isolated_during_outage(r1, r3));

    // All subsystems integrated.
    assert!(manager.local_ratio() <= 1.0);
    assert!(traffic.avg_bytes_per_flow() > 0.0);
    assert!(failover.success_rate() >= 0.0);
    assert!(quota.violation_rate() >= 0.0);
    assert!(affinity.hit_rate() >= 0.0);
    assert!(sync.total_syncs() > 0);
    assert!(breaker.total_trips() > 0);
}

// ========== RegionalAutonomyManager integration with subsystems ==========

#[test]
fn manager_traffic_integration() {
    let mut manager = RegionalAutonomyManager::new();
    let mut traffic = CrossRegionTrafficAnalyzer::new();
    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    let d1 = DatasetId::new([3u8; 32]);

    // Record traffic through manager's cross-region op.
    let op_id = manager.record_cross_region_op(r1, r2, d1, CrossRegionOpType::Replicate, true);
    traffic.record_flow(r1, r2, d1, 512, 1);
    manager.complete_cross_region_op(op_id).unwrap();

    assert_eq!(traffic.total_flows(), 1);
    assert_eq!(traffic.pair_bytes(r1, r2), 512);
    assert_eq!(manager.cross_region_detector().completed_ops(), 1);
}

// ========== RegionalAutonomyConfig integration ==========

#[test]
fn autonomy_config_custom() {
    let config = RegionalAutonomyConfig {
        require_healthy_for_local: false,
        allow_cross_region_failover: false,
        max_cross_region_ops: 200,
    };
    assert!(!config.require_healthy_for_local);
    assert!(!config.allow_cross_region_failover);
    assert_eq!(config.max_cross_region_ops, 200);

    let manager = RegionalAutonomyManager::with_config(config);
    assert!(!manager.config.require_healthy_for_local);
}

// ========== LocalRoutingReason all variants ==========

#[test]
fn local_routing_reason_all() {
    let reasons = [
        LocalRoutingReason::LocalReplicaAvailable,
        LocalRoutingReason::NoLocalReplica,
        LocalRoutingReason::RegionUnhealthy,
        LocalRoutingReason::CrossRegionRequired,
        LocalRoutingReason::DatasetNotFound,
    ];
    for r in reasons {
        assert_ne!(format!("{:?}"), "");
    }
}

// ========== RegionHealth all variants ==========

#[test]
fn region_health_all() {
    let statuses = [
        RegionHealth::Healthy,
        RegionHealth::Degraded,
        RegionHealth::Isolated,
        RegionHealth::Failed,
        RegionHealth::Recovering,
    ];
    for s in statuses {
        assert_ne!(format!("{:?}"), "");
    }
}

// ========== LocalRoutingDecision integration ==========

#[test]
fn local_routing_decision_all_fields() {
    let decision = LocalRoutingDecision {
        region: RegionId::new([1u8; 32]),
        dataset: DatasetId::new([2u8; 32]),
        local_allowed: false,
        preferred_region: Some(RegionId::new([3u8; 32])),
        reason: LocalRoutingReason::RegionUnhealthy,
    };
    assert!(!decision.local_allowed);
    assert_eq!(decision.preferred_region, Some(RegionId::new([3u8; 32])));
    assert_eq!(decision.reason, LocalRoutingReason::RegionUnhealthy);
    let _ = format!("{:?}", decision);
}

// ========== RegionalQuota edge cases ==========

#[test]
fn quota_exact_boundary() {
    let mut manager = RegionalQuotaManager::new();
    let region = RegionId::new([1u8; 32]);
    manager.set_quota(region, 1000, 10);
    // Exactly at boundary should pass.
    assert!(manager.check_quota(region, 1000, 10).is_ok());
    manager.record_usage(region, 1000, 10);
    // One byte over should fail.
    assert!(manager.check_quota(region, 1, 1).is_err());
}

// ========== RegionalIsolationManager recovery ==========

#[test]
fn isolation_manager_recovery_clears_outage() {
    let mut mgr = RegionalIsolationManager::new();
    let a = RegionId::new([1u8; 32]);
    let b = RegionId::new([2u8; 32]);
    mgr.record_outage(b);
    mgr.set_isolation(a, b, true, Some(b));
    assert!(mgr.region_isolated_during_outage(a, b));

    mgr.record_recovery(b);
    assert!(!mgr.region_in_outage(b));
    // After recovery, isolation due to outage should not apply.
    assert!(!mgr.region_isolated_during_outage(a, b));
}

// ========== RegionalAutonomyManager total checks ==========

#[test]
fn manager_total_checks() {
    let mut manager = RegionalAutonomyManager::new();
    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    manager.local_operation_allowed(r1, DatasetId::new([3u8; 32]));
    manager.cross_region_required(r1, r2);
    manager.region_isolated_during_outage(r1, r2);
    assert_eq!(manager.total_checks(), 3);
}

// ========== CrossRegionTrafficAnalyzer pair_bytes ==========

#[test]
fn traffic_analyzer_pair_bytes_zero_for_unknown() {
    let analyzer = CrossRegionTrafficAnalyzer::new();
    assert_eq!(
        analyzer.pair_bytes(
            RegionId::new([1u8; 32]),
            RegionId::new([2u8; 32])
        ),
        0
    );
}

// ========== RegionalQuotaManager violation_rate_zero ==========

#[test]
fn quota_manager_violation_rate_zero() {
    let manager = RegionalQuotaManager::new();
    assert!((manager.violation_rate() - 0.0).abs() < 0.001);
}

// ========== RegionAffinityTracker hit_rate_zero ==========

#[test]
fn affinity_tracker_hit_rate_zero() {
    let tracker = RegionAffinityTracker::new();
    assert!((tracker.hit_rate() - 0.0).abs() < 0.001);
}

// ========== CrossRegionSyncManager empty ==========

#[test]
fn sync_manager_empty() {
    let manager = CrossRegionSyncManager::new();
    assert_eq!(manager.total_syncs(), 0);
    assert_eq!(manager.total_bytes_synced(), 0);
    assert!(manager.syncs_for_region(RegionId::new([1u8; 32])).is_empty());
}

// ========== RegionalCircuitBreakerManager closed by default ==========

#[test]
fn circuit_breaker_closed_by_default() {
    let mgr = RegionalCircuitBreakerManager::new();
    assert_eq!(mgr.circuit_state(RegionId::new([1u8; 32])), RegionCircuitState::Closed);
}

// ========== RegionalAutonomyManager router integration ==========

#[test]
fn manager_router_local_ratio() {
    let mut manager = RegionalAutonomyManager::new();
    let r1 = RegionId::new([1u8; 32]);
    let r2 = RegionId::new([2u8; 32]);
    let d1 = DatasetId::new([3u8; 32]);
    manager.register_local_replica(d1, r1);
    manager.register_region_health(RegionHealthRecord {
        region_id: r1,
        health: RegionHealth::Healthy,
        last_heartbeat: TimestampMillis(0),
        outage_start: None,
        peer_regions: Vec::new(),
        local_datasets: vec![d1],
    });
    manager.local_operation_allowed(r1, d1);
    manager.local_operation_allowed(r2, d1);
    assert!((manager.local_ratio() - 0.5).abs() < 0.001);
}

///////////////////////////////////////////////////////////////////////////////
// Region Replication Policy — manages replication rules per region
///////////////////////////////////////////////////////////////////////////////

/// A replication policy for a dataset in a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionReplicationPolicy {
    pub region_id: RegionId,
    pub dataset_id: DatasetId,
    pub replica_count: u32,
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub require_cross_region: bool,
}

/// Manages replication policies per region.
pub struct RegionReplicationPolicyManager {
    /// Policies per (region, dataset) pair.
    policies: HashMap<(RegionId, DatasetId), RegionReplicationPolicy>,
    /// Total policies.
    total_policies: u64,
}

impl RegionReplicationPolicyManager {
    /// Create a new manager.
    pub fn new() -> Self {
        RegionReplicationPolicyManager {
            policies: HashMap::new(),
            total_policies: 0,
        }
    }

    /// Set a replication policy.
    pub fn set_policy(&mut self, policy: RegionReplicationPolicy) {
        self.total_policies += 1;
        self.policies.insert((policy.region_id, policy.dataset_id), policy);
    }

    /// Return the policy for a region/dataset.
    pub fn policy_for(
        &self,
        region: RegionId,
        dataset: DatasetId,
    ) -> Option<&RegionReplicationPolicy> {
        self.policies.get(&(region, dataset))
    }

    /// Return whether replication meets minimum requirements.
    pub fn meets_minimum(
        &self,
        region: RegionId,
        dataset: DatasetId,
        current_replicas: u32,
    ) -> bool {
        match self.policy_for(region, dataset) {
            Some(policy) => current_replicas >= policy.min_replicas,
            None => current_replicas >= 1,
        }
    }

    /// Return whether replication exceeds maximum.
    pub fn exceeds_maximum(
        &self,
        region: RegionId,
        dataset: DatasetId,
        current_replicas: u32,
    ) -> bool {
        match self.policy_for(region, dataset) {
            Some(policy) => current_replicas > policy.max_replicas,
            None => false,
        }
    }

    /// Return total policies.
    pub fn total_policies(&self) -> u64 {
        self.total_policies
    }

    /// Return policies for a region.
    pub fn policies_for_region(&self, region: RegionId) -> Vec<&RegionReplicationPolicy> {
        self.policies
            .iter()
            .filter_map(|((r, _), p)| if *r == region { Some(p) } else { None })
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region Downtime Tracker — tracks regional downtime
///////////////////////////////////////////////////////////////////////////////

/// A downtime event for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionDowntimeEvent {
    pub event_id: DowntimeEventId,
    pub region_id: RegionId,
    pub started_at: TimestampMillis,
    pub ended_at: Option<TimestampMillis>,
    pub duration_ms: u64,
    pub cause: DowntimeCause,
}

/// Unique identifier for a downtime event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DowntimeEventId(pub u64);

/// Cause of regional downtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DowntimeCause {
    PowerFailure,
    NetworkPartition,
    HardwareFailure,
    SoftwareFailure,
    Maintenance,
    NaturalDisaster,
    Unknown,
}

/// Tracks regional downtime events.
pub struct RegionDowntimeTracker {
    /// Events by ID.
    events: HashMap<DowntimeEventId, RegionDowntimeEvent>,
    /// Per-region event IDs.
    region_events: HashMap<RegionId, Vec<DowntimeEventId>>,
    /// Next event ID.
    next_id: u64,
    /// Total events.
    total_events: u64,
    /// Total downtime milliseconds.
    total_downtime_ms: u64,
}

impl RegionDowntimeTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        RegionDowntimeTracker {
            events: HashMap::new(),
            region_events: HashMap::new(),
            next_id: 0,
            total_events: 0,
            total_downtime_ms: 0,
        }
    }

    /// Record a downtime event start.
    pub fn record_downtime_start(
        &mut self,
        region: RegionId,
        cause: DowntimeCause,
    ) -> DowntimeEventId {
        let id = DowntimeEventId(self.next_id);
        self.next_id += 1;
        self.total_events += 1;
        let event = RegionDowntimeEvent {
            event_id: id,
            region_id: region,
            started_at: TimestampMillis(0),
            ended_at: None,
            duration_ms: 0,
            cause,
        };
        self.events.insert(id, event);
        self.region_events.entry(region).or_default().push(id);
        id
    }

    /// Record a downtime event end.
    pub fn record_downtime_end(
        &mut self,
        event_id: DowntimeEventId,
        ended_at: TimestampMillis,
    ) -> Result<(), Error> {
        let event = self
            .events
            .get_mut(&event_id)
            .ok_or_else(|| Error::Internal {
                message: "downtime event not found".to_string(),
            })?;
        if event.ended_at.is_some() {
            return Err(Error::Internal {
                message: "downtime event already ended".to_string(),
            });
        }
        event.ended_at = Some(ended_at);
        event.duration_ms = ended_at.saturating_sub(event.started_at);
        self.total_downtime_ms += event.duration_ms;
        Ok(())
    }

    /// Return events for a region.
    pub fn events_for_region(&self, region: RegionId) -> Vec<RegionDowntimeEvent> {
        self.region_events
            .get(&region)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.events.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total events.
    pub fn total_events(&self) -> u64 {
        self.total_events
    }

    /// Return total downtime ms.
    pub fn total_downtime_ms(&self) -> u64 {
        self.total_downtime_ms
    }

    /// Return average downtime duration.
    pub fn avg_downtime_duration_ms(&self) -> f64 {
        if self.total_events == 0 {
            return 0.0;
        }
        self.total_downtime_ms as f64 / self.total_events as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-Region Consistency Checker — verifies cross-region consistency
///////////////////////////////////////////////////////////////////////////////

/// A consistency check result between two regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionConsistencyResult {
    pub region_a: RegionId,
    pub region_b: RegionId,
    pub dataset: DatasetId,
    pub consistent: bool,
    pub divergent_bytes: u64,
    pub checked_at: TimestampMillis,
}

/// Checks consistency between regions.
pub struct CrossRegionConsistencyChecker {
    /// Check results per region pair + dataset.
    results: HashMap<(RegionId, RegionId, DatasetId), RegionConsistencyResult>,
    /// Total checks.
    total_checks: u64,
    /// Total inconsistent results.
    inconsistent_count: u64,
}

impl CrossRegionConsistencyChecker {
    /// Create a new checker.
    pub fn new() -> Self {
        CrossRegionConsistencyChecker {
            results: HashMap::new(),
            total_checks: 0,
            inconsistent_count: 0,
        }
    }

    /// Record a consistency check result.
    pub fn record_check(
        &mut self,
        region_a: RegionId,
        region_b: RegionId,
        dataset: DatasetId,
        consistent: bool,
        divergent_bytes: u64,
    ) {
        self.total_checks += 1;
        if !consistent {
            self.inconsistent_count += 1;
        }
        let result = RegionConsistencyResult {
            region_a,
            region_b,
            dataset,
            consistent,
            divergent_bytes,
            checked_at: TimestampMillis(0),
        };
        self.results.insert((region_a, region_b, dataset), result);
    }

    /// Return whether regions are consistent for a dataset.
    pub fn is_consistent(
        &self,
        region_a: RegionId,
        region_b: RegionId,
        dataset: DatasetId,
    ) -> bool {
        self.results
            .get(&(region_a, region_b, dataset))
            .map(|r| r.consistent)
            .unwrap_or(true)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return inconsistent count.
    pub fn inconsistent_count(&self) -> u64 {
        self.inconsistent_count
    }

    /// Return consistency rate.
    pub fn consistency_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 1.0;
        }
        (self.total_checks - self.inconsistent_count) as f64 / self.total_checks as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Regional Resource Allocator — allocates resources across regions
///////////////////////////////////////////////////////////////////////////////

/// A resource allocation for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalResourceAllocation {
    pub region_id: RegionId,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub storage_gb: u64,
    pub network_mbps: u32,
}

/// Allocates resources across regions.
pub struct RegionalResourceAllocator {
    /// Allocations per region.
    allocations: HashMap<RegionId, RegionalResourceAllocation>,
    /// Total allocated CPU cores.
    total_cpu_cores: u32,
    /// Total allocated memory MB.
    total_memory_mb: u64,
    /// Total allocated storage GB.
    total_storage_gb: u64,
}

impl RegionalResourceAllocator {
    /// Create a new allocator.
    pub fn new() -> Self {
        RegionalResourceAllocator {
            allocations: HashMap::new(),
            total_cpu_cores: 0,
            total_memory_mb: 0,
            total_storage_gb: 0,
        }
    }

    /// Allocate resources to a region.
    pub fn allocate(
        &mut self,
        region: RegionId,
        cpu_cores: u32,
        memory_mb: u64,
        storage_gb: u64,
        network_mbps: u32,
    ) {
        let allocation = RegionalResourceAllocation {
            region_id: region,
            cpu_cores,
            memory_mb,
            storage_gb,
            network_mbps,
        };
        self.total_cpu_cores += cpu_cores;
        self.total_memory_mb += memory_mb;
        self.total_storage_gb += storage_gb;
        self.allocations.insert(region, allocation);
    }

    /// Return allocation for a region.
    pub fn allocation_for(&self, region: RegionId) -> Option<&RegionalResourceAllocation> {
        self.allocations.get(&region)
    }

    /// Return total CPU cores.
    pub fn total_cpu_cores(&self) -> u32 {
        self.total_cpu_cores
    }

    /// Return total memory MB.
    pub fn total_memory_mb(&self) -> u64 {
        self.total_memory_mb
    }

    /// Return total storage GB.
    pub fn total_storage_gb(&self) -> u64 {
        self.total_storage_gb
    }

    /// Return total regions with allocations.
    pub fn allocated_region_count(&self) -> usize {
        self.allocations.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region Health Monitor — continuous health monitoring
///////////////////////////////////////////////////////////////////////////////

/// A health check result for a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHealthCheck {
    pub region_id: RegionId,
    pub healthy: bool,
    pub latency_ms: u64,
    pub error_count: u32,
    pub checked_at: TimestampMillis,
}

/// Monitors region health continuously.
pub struct RegionHealthMonitor {
    /// Latest check per region.
    checks: HashMap<RegionId, RegionHealthCheck>,
    /// Total checks performed.
    total_checks: u64,
    /// Total healthy results.
    healthy_count: u64,
    /// Total unhealthy results.
    unhealthy_count: u64,
}

impl RegionHealthMonitor {
    /// Create a new monitor.
    pub fn new() -> Self {
        RegionHealthMonitor {
            checks: HashMap::new(),
            total_checks: 0,
            healthy_count: 0,
            unhealthy_count: 0,
        }
    }

    /// Record a health check result.
    pub fn record_check(
        &mut self,
        region: RegionId,
        healthy: bool,
        latency_ms: u64,
        error_count: u32,
    ) {
        self.total_checks += 1;
        if healthy {
            self.healthy_count += 1;
        } else {
            self.unhealthy_count += 1;
        }
        let check = RegionHealthCheck {
            region_id: region,
            healthy,
            latency_ms,
            error_count,
            checked_at: TimestampMillis(0),
        };
        self.checks.insert(region, check);
    }

    /// Return the latest check for a region.
    pub fn latest_check(&self, region: RegionId) -> Option<&RegionHealthCheck> {
        self.checks.get(&region)
    }

    /// Return whether a region is currently healthy.
    pub fn is_healthy(&self, region: RegionId) -> bool {
        self.checks
            .get(&region)
            .map(|c| c.healthy)
            .unwrap_or(false)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return healthy count.
    pub fn healthy_count(&self) -> u64 {
        self.healthy_count
    }

    /// Return unhealthy count.
    pub fn unhealthy_count(&self) -> u64 {
        self.unhealthy_count
    }

    /// Return health rate.
    pub fn health_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 1.0;
        }
        self.healthy_count as f64 / self.total_checks as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////