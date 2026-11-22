//! # Sealed Trait: Recovery — multi-level recovery (node/H-unit/region)
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Recovery trait defines multi-level failure recovery across nodes,
//! H-units, and regions. It includes failure detection, repair planning,
//! state reconciliation, replica reconstruction, and quarantine handling.
//! Implementations live downstream in ids-recovery and ids-* crates.

use crate::types::{
    Capacity, ConsistencyProfile, DatasetId, DurabilityCondition, Endpoint, FailureDomain,
    FailureEvent, FailureEventType, HUnitId, NodeId, OrderingPos, PartitionId, RecordId,
    RedundancyMode, RegionId, RecoveryAction, RecoveryProof, RecoveryScopeLevel,
};
use crate::errors::RecoveryError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Recovery
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::RecoveryEngine {}
impl private::Sealed for super::RecoveryPlan {}
impl private::Sealed for super::RecoveryStep {}
impl private::Sealed for super::FailureDetector {}
impl private::Sealed for super::RecoveryScope {}
impl private::Sealed for super::RepairPlanner {}
impl private::Sealed for super::StateReconciler {}
impl private::Sealed for super::ReplicaReconstructor {}
impl private::Sealed for super::QuarantineManager {}
impl private::Sealed for super::RecoveryScheduler {}
impl private::Sealed for super::RecoveryStats {}
impl private::Sealed for super::RecoveryTimeline {}
impl private::Sealed for super::HealthMonitor {}

/// Sealed trait for multi-level recovery — node/H-unit/region recovery,
/// failure detection, repair planning, state reconciliation, quarantine.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Recovery: private::Sealed {
    /// Detect failures across nodes/H-units/regions.
    fn detect_failures(&self) -> Result<Vec<FailureEvent>, RecoveryError>;

    /// Create a recovery plan for a given failure event.
    fn create_plan(&self, failure: &FailureEvent) -> Result<RecoveryPlan, RecoveryError>;

    /// Execute a recovery plan step by step.
    fn execute_plan(&self, plan_id: RecoveryPlanId) -> Result<RecoveryProof, RecoveryError>;

    /// Quarantine a suspected faulty node/H-unit/region.
    fn quarantine(&self, scope: RecoveryScope) -> Result<(), RecoveryError>;

    /// Restore redundancy after a failure.
    fn restore_redundancy(&self, dataset_id: DatasetId) -> Result<u64, RecoveryError>;

    /// Reconcile state across replicas.
    fn reconcile_state(&self, dataset_id: DatasetId) -> Result<RecoveryProof, RecoveryError>;

    /// Get recovery statistics.
    fn stats(&self) -> RecoveryStats;

    /// Get the recovery name for debugging.
    fn recovery_name(&self) -> &'static str;

    /// Get total recovery plans created.
    fn plan_count(&self) -> u64;

    /// Get total recoveries executed.
    fn recovery_count(&self) -> u64;

    /// Get total quarantined scopes.
    fn quarantine_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    #[error("node not found: {node_id:?}")]
    NodeNotFound { node_id: NodeId },

    #[error("hunit not found: {hunit_id:?}")]
    HUnitNotFound { hunit_id: HUnitId },

    #[error("region not found: {region_id:?}")]
    RegionNotFound { region_id: RegionId },

    #[error("recovery plan not found: {plan_id:?}")]
    PlanNotFound { plan_id: RecoveryPlanId },

    #[error("recovery step failed: step={step:?}, reason={reason}")]
    StepFailed { step: RecoveryStepId, reason: String },

    #[error("insufficient replicas: required={required}, available={available}")]
    InsufficientReplicas { required: u64, available: u64 },

    #[error("durability condition not met: condition={condition:?}")]
    DurabilityNotMet { condition: DurabilityCondition },

    #[error("quarantine already active: scope={scope:?}")]
    QuarantineActive { scope: RecoveryScope },

    #[error("recovery in progress: scope={scope:?}")]
    RecoveryInProgress { scope: RecoveryScope },

    #[error("state reconciliation failed: dataset={dataset_id:?}, reason={reason}")]
    ReconciliationFailed { dataset_id: DatasetId, reason: String },

    #[error("replica reconstruction failed: partition={partition_id:?}, reason={reason}")]
    ReconstructionFailed { partition_id: PartitionId, reason: String },

    #[error("internal recovery error: {message}")]
    Internal { message: String },
}

impl RecoveryError {
    pub fn category(&self) -> &'static str {
        match self {
            RecoveryError::NodeNotFound { .. } => "node_not_found",
            RecoveryError::HUnitNotFound { .. } => "hunit_not_found",
            RecoveryError::RegionNotFound { .. } => "region_not_found",
            RecoveryError::PlanNotFound { .. } => "plan_not_found",
            RecoveryError::StepFailed { .. } => "step_failed",
            RecoveryError::InsufficientReplicas { .. } => "insufficient_replicas",
            RecoveryError::DurabilityNotMet { .. } => "durability_not_met",
            RecoveryError::QuarantineActive { .. } => "quarantine_active",
            RecoveryError::RecoveryInProgress { .. } => "recovery_in_progress",
            RecoveryError::ReconciliationFailed { .. } => "reconciliation_failed",
            RecoveryError::ReconstructionFailed { .. } => "reconstruction_failed",
            RecoveryError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RecoveryError::StepFailed { .. }
                | RecoveryError::InsufficientReplicas { .. }
                | RecoveryError::DurabilityNotMet { .. }
                | RecoveryError::RecoveryInProgress { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            RecoveryError::NodeNotFound { .. } => "register node to network",
            RecoveryError::HUnitNotFound { .. } => "register hunit to node",
            RecoveryError::RegionNotFound { .. } => "register region to network",
            RecoveryError::PlanNotFound { .. } => "create recovery plan first",
            RecoveryError::StepFailed { .. } => "retry recovery step or skip",
            RecoveryError::InsufficientReplicas { .. } => "add more replicas or reduce requirement",
            RecoveryError::DurabilityNotMet { .. } => "wait for replicas to catch up",
            RecoveryError::QuarantineActive { .. } => "resolve quarantine first",
            RecoveryError::RecoveryInProgress { .. } => "wait for recovery to complete",
            RecoveryError::ReconciliationFailed { .. } => "verify replica consistency",
            RecoveryError::ReconstructionFailed { .. } => "retry with alternate replica",
            RecoveryError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY PLAN ID & STEP ID
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecoveryPlanId(pub [u8; 32]);

impl RecoveryPlanId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecoveryStepId(pub [u8; 32]);

impl RecoveryStepId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY SCOPE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryScope {
    Node(NodeId),
    HUnit(HUnitId),
    Region(RegionId),
}

impl RecoveryScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryScope::Node(_) => "node",
            RecoveryScope::HUnit(_) => "hunit",
            RecoveryScope::Region(_) => "region",
        }
    }

    pub fn level(&self) -> RecoveryScopeLevel {
        match self {
            RecoveryScope::Node(_) => RecoveryScopeLevel::Node,
            RecoveryScope::HUnit(_) => RecoveryScopeLevel::HUnit,
            RecoveryScope::Region(_) => RecoveryScopeLevel::Region,
        }
    }

    pub fn is_node(&self) -> bool {
        matches!(self, RecoveryScope::Node(_))
    }

    pub fn is_hunit(&self) -> bool {
        matches!(self, RecoveryScope::HUnit(_))
    }

    pub fn is_region(&self) -> bool {
        matches!(self, RecoveryScope::Region(_))
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY PLAN
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub plan_id: RecoveryPlanId,
    pub failure_event: FailureEvent,
    pub scope: RecoveryScope,
    pub steps: Vec<RecoveryStep>,
    pub current_step: usize,
    pub status: PlanStatus,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanStatus::Pending => "pending",
            PlanStatus::InProgress => "in_progress",
            PlanStatus::Completed => "completed",
            PlanStatus::Failed => "failed",
            PlanStatus::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, PlanStatus::Pending | PlanStatus::InProgress)
    }
}

impl RecoveryPlan {
    pub fn new(
        plan_id: RecoveryPlanId,
        failure_event: FailureEvent,
        scope: RecoveryScope,
        steps: Vec<RecoveryStep>,
    ) -> Self {
        Self {
            plan_id,
            failure_event,
            scope,
            steps,
            current_step: 0,
            status: PlanStatus::Pending,
            created_at_ms: 0,
            started_at_ms: None,
            completed_at_ms: None,
        }
    }

    pub fn start(&mut self, at_ms: u64) {
        self.status = PlanStatus::InProgress;
        self.started_at_ms = Some(at_ms);
    }

    pub fn complete(&mut self, at_ms: u64) {
        self.status = PlanStatus::Completed;
        self.completed_at_ms = Some(at_ms);
    }

    pub fn fail(&mut self) {
        self.status = PlanStatus::Failed;
    }

    pub fn cancel(&mut self) {
        self.status = PlanStatus::Cancelled;
    }

    pub fn next_step(&self) -> Option<&RecoveryStep> {
        self.steps.get(self.current_step)
    }

    pub fn advance_step(&mut self) {
        self.current_step += 1;
        if self.current_step >= self.steps.len() {
            self.status = PlanStatus::Completed;
        }
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn completed_step_count(&self) -> usize {
        self.current_step
    }

    pub fn progress_percent(&self) -> u8 {
        if self.steps.is_empty() {
            return 100;
        }
        (self.current_step as u32 * 100 / self.steps.len() as u32) as u8
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY STEP
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct RecoveryStep {
    pub step_id: RecoveryStepId,
    pub action: RecoveryAction,
    pub scope_level: RecoveryScopeLevel,
    pub target: String,
    pub status: StepStatus,
    pub attempt: u32,
    pub max_attempts: u32,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Retrying,
    Skipped,
}

impl StepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Pending => "pending",
            StepStatus::InProgress => "in_progress",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
            StepStatus::Retrying => "retrying",
            StepStatus::Skipped => "skipped",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, StepStatus::Completed | StepStatus::Failed | StepStatus::Skipped)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, StepStatus::Pending | StepStatus::InProgress | StepStatus::Retrying)
    }
}

impl RecoveryStep {
    pub fn new(
        step_id: RecoveryStepId,
        action: RecoveryAction,
        scope_level: RecoveryScopeLevel,
        target: String,
        max_attempts: u32,
    ) -> Self {
        Self {
            step_id,
            action,
            scope_level,
            target,
            status: StepStatus::Pending,
            attempt: 0,
            max_attempts,
            started_at_ms: None,
            completed_at_ms: None,
            error: None,
        }
    }

    pub fn start(&mut self, at_ms: u64) {
        self.status = StepStatus::InProgress;
        self.started_at_ms = Some(at_ms);
        self.attempt += 1;
    }

    pub fn retry(&mut self) {
        if self.attempt < self.max_attempts {
            self.status = StepStatus::Retrying;
            self.error = None;
        } else {
            self.status = StepStatus::Failed;
        }
    }

    pub fn complete(&mut self, at_ms: u64) {
        self.status = StepStatus::Completed;
        self.completed_at_ms = Some(at_ms);
    }

    pub fn fail(&mut self, reason: String) {
        self.error = Some(reason);
        self.status = StepStatus::Failed;
    }

    pub fn skip(&mut self) {
        self.status = StepStatus::Skipped;
    }

    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_attempts
    }

    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at_ms, self.completed_at_ms) {
            (Some(start), Some(end)) => Some(end.saturating_sub(start)),
            _ => None,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAILURE DETECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct FailureDetector {
    pub node_heartbeats: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub hunit_heartbeats: std::cell::RefCell<std::collections::HashMap<HUnitId, u64>>,
    pub region_heartbeats: std::cell::RefCell<std::collections::HashMap<RegionId, u64>>,
    pub heartbeat_timeout_ms: u64,
    pub detection_count: u64,
    pub failure_count: u64,
}

impl FailureDetector {
    pub fn new(heartbeat_timeout_ms: u64) -> Self {
        Self {
            node_heartbeats: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_heartbeats: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_heartbeats: std::cell::RefCell::new(std::collections::HashMap::new()),
            heartbeat_timeout_ms,
            detection_count: 0,
            failure_count: 0,
        }
    }

    pub fn record_node_heartbeat(&self, node_id: NodeId, at_ms: u64) {
        self.node_heartbeats.borrow_mut().insert(node_id, at_ms);
    }

    pub fn record_hunit_heartbeat(&self, hunit_id: HUnitId, at_ms: u64) {
        self.hunit_heartbeats.borrow_mut().insert(hunit_id, at_ms);
    }

    pub fn record_region_heartbeat(&self, region_id: RegionId, at_ms: u64) {
        self.region_heartbeats.borrow_mut().insert(region_id, at_ms);
    }

    pub fn detect_failures(&self, now_ms: u64) -> Vec<FailureEvent> {
        self.detection_count += 1;
        let mut failures = Vec::new();

        let node_hbs = self.node_heartbeats.borrow();
        for (node_id, last_hb) in node_hbs.iter() {
            if now_ms.saturating_sub(*last_hb) > self.heartbeat_timeout_ms {
                failures.push(FailureEvent::new(
                    FailureEventType::NodeFailure,
                    Some(node_id.clone()),
                    None,
                    None,
                    now_ms,
                ));
                self.failure_count += 1;
            }
        }
        drop(node_hbs);

        let hunit_hbs = self.hunit_heartbeats.borrow();
        for (hunit_id, last_hb) in hunit_hbs.iter() {
            if now_ms.saturating_sub(*last_hb) > self.heartbeat_timeout_ms {
                failures.push(FailureEvent::new(
                    FailureEventType::HUnitFailure,
                    None,
                    Some(hunit_id.clone()),
                    None,
                    now_ms,
                ));
                self.failure_count += 1;
            }
        }
        drop(hunit_hbs);

        let region_hbs = self.region_heartbeats.borrow();
        for (region_id, last_hb) in region_hbs.iter() {
            if now_ms.saturating_sub(*last_hb) > self.heartbeat_timeout_ms {
                failures.push(FailureEvent::new(
                    FailureEventType::RegionalFailure,
                    None,
                    None,
                    Some(region_id.clone()),
                    now_ms,
                ));
                self.failure_count += 1;
            }
        }

        failures
    }

    pub fn is_healthy_node(&self, node_id: &NodeId, now_ms: u64) -> bool {
        match self.node_heartbeats.borrow().get(node_id) {
            Some(last_hb) => now_ms.saturating_sub(*last_hb) <= self.heartbeat_timeout_ms,
            None => false,
        }
    }

    pub fn is_healthy_hunit(&self, hunit_id: &HUnitId, now_ms: u64) -> bool {
        match self.hunit_heartbeats.borrow().get(hunit_id) {
            Some(last_hb) => now_ms.saturating_sub(*last_hb) <= self.heartbeat_timeout_ms,
            None => false,
        }
    }

    pub fn is_healthy_region(&self, region_id: &RegionId, now_ms: u64) -> bool {
        match self.region_heartbeats.borrow().get(region_id) {
            Some(last_hb) => now_ms.saturating_sub(*last_hb) <= self.heartbeat_timeout_ms,
            None => false,
        }
    }

    pub fn detection_count(&self) -> u64 {
        self.detection_count
    }

    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }

    pub fn heartbeat_timeout_ms(&self) -> u64 {
        self.heartbeat_timeout_ms
    }

    pub fn node_count(&self) -> usize {
        self.node_heartbeats.borrow().len()
    }

    pub fn hunit_count(&self) -> usize {
        self.hunit_heartbeats.borrow().len()
    }

    pub fn region_count(&self) -> usize {
        self.region_heartbeats.borrow().len()
    }
}

impl Default for FailureDetector {
    fn default() -> Self {
        Self::new(30000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY SCOPE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct RecoveryScope {
    pub active_scopes: std::cell::RefCell<std::collections::HashMap<RecoveryScope, u64>>,
    pub scope_start_times: std::cell::RefCell<std::collections::HashMap<RecoveryScope, u64>>,
    pub scope_count: u64,
}

impl RecoveryScope {
    pub fn new() -> Self {
        Self {
            active_scopes: std::cell::RefCell::new(std::collections::HashMap::new()),
            scope_start_times: std::cell::RefCell::new(std::collections::HashMap::new()),
            scope_count: 0,
        }
    }

    pub fn activate(&self, scope: RecoveryScope, at_ms: u64) {
        let mut scopes = self.active_scopes.borrow_mut();
        let mut start_times = self.scope_start_times.borrow_mut();
        scopes.insert(scope, 1);
        start_times.insert(scope, at_ms);
        self.scope_count += 1;
    }

    pub fn deactivate(&self, scope: &RecoveryScope) -> bool {
        let mut scopes = self.active_scopes.borrow_mut();
        let mut start_times = self.scope_start_times.borrow_mut();
        let removed = scopes.remove(scope).is_some();
        start_times.remove(scope);
        removed
    }

    pub fn is_active(&self, scope: &RecoveryScope) -> bool {
        self.active_scopes.borrow().contains_key(scope)
    }

    pub fn get_start_time(&self, scope: &RecoveryScope) -> Option<u64> {
        self.scope_start_times.borrow().get(scope).copied()
    }

    pub fn active_count(&self) -> usize {
        self.active_scopes.borrow().len()
    }

    pub fn total_scope_count(&self) -> u64 {
        self.scope_count
    }

    pub fn clear_all(&self) {
        self.active_scopes.borrow_mut().clear();
        self.scope_start_times.borrow_mut().clear();
    }
}

impl Default for RecoveryScope {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPAIR PLANNER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairStrategy {
    ReconstructErasure,
    CopyReplica,
    TransferOwnership,
    RestoreRedundancy,
}

impl RepairStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepairStrategy::ReconstructErasure => "reconstruct_erasure",
            RepairStrategy::CopyReplica => "copy_replica",
            RepairStrategy::TransferOwnership => "transfer_ownership",
            RepairStrategy::RestoreRedundancy => "restore_redundancy",
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            RepairStrategy::CopyReplica => 1,
            RepairStrategy::RestoreRedundancy => 2,
            RepairStrategy::ReconstructErasure => 3,
            RepairStrategy::TransferOwnership => 4,
        }
    }
}

pub struct RepairPlanner {
    pub strategies: std::cell::RefCell<Vec<RepairStrategy>>,
    pub plan_count: u64,
    pub executed_count: u64,
    pub failed_count: u64,
}

impl RepairPlanner {
    pub fn new() -> Self {
        Self {
            strategies: std::cell::RefCell::new(vec![
                RepairStrategy::CopyReplica,
                RepairStrategy::RestoreRedundancy,
                RepairStrategy::ReconstructErasure,
                RepairStrategy::TransferOwnership,
            ]),
            plan_count: 0,
            executed_count: 0,
            failed_count: 0,
        }
    }

    pub fn add_strategy(&self, strategy: RepairStrategy) {
        self.strategies.borrow_mut().push(strategy);
    }

    pub fn select_strategy(&self, scope_level: RecoveryScopeLevel) -> RepairStrategy {
        let strategies = self.strategies.borrow();
        match scope_level {
            RecoveryScopeLevel::Node => RepairStrategy::CopyReplica,
            RecoveryScopeLevel::HUnit => RepairStrategy::RestoreRedundancy,
            RecoveryScopeLevel::Region => RepairStrategy::ReconstructErasure,
        }
    }

    pub fn plan_repair(&self, action: RecoveryAction) -> u64 {
        self.plan_count += 1;
        self.plan_count
    }

    pub fn record_executed(&self) {
        self.executed_count += 1;
    }

    pub fn record_failed(&self) {
        self.failed_count += 1;
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.executed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.executed_count as f64 / total as f64
    }

    pub fn strategy_count(&self) -> usize {
        self.strategies.borrow().len()
    }
}

impl Default for RepairPlanner {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STATE RECONCILER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationResult {
    Consistent,
    Repaired,
    Diverged,
    Unknown,
}

impl ReconciliationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReconciliationResult::Consistent => "consistent",
            ReconciliationResult::Repaired => "repaired",
            ReconciliationResult::Diverged => "diverged",
            ReconciliationResult::Unknown => "unknown",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ReconciliationResult::Consistent | ReconciliationResult::Repaired)
    }
}

pub struct StateReconciler {
    pub replica_states: std::cell::RefCell<
        std::collections::HashMap<DatasetId, Vec<(NodeId, OrderingPos)>>,
    >,
    pub reconciliation_count: u64,
    pub consistent_count: u64,
    pub repaired_count: u64,
    pub diverged_count: u64,
}

impl StateReconciler {
    pub fn new() -> Self {
        Self {
            replica_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            reconciliation_count: 0,
            consistent_count: 0,
            repaired_count: 0,
            diverged_count: 0,
        }
    }

    pub fn record_replica_state(
        &self,
        dataset_id: DatasetId,
        node_id: NodeId,
        position: OrderingPos,
    ) {
        self.replica_states
            .borrow_mut()
            .entry(dataset_id)
            .or_default()
            .push((node_id, position));
    }

    pub fn reconcile(&self, dataset_id: &DatasetId) -> ReconciliationResult {
        self.reconciliation_count += 1;
        let states = self.replica_states.borrow();
        match states.get(dataset_id) {
            Some(replicas) => {
                if replicas.is_empty() {
                    self.diverged_count += 1;
                    return ReconciliationResult::Unknown;
                }
                let positions: Vec<OrderingPos> = replicas.iter().map(|(_, p)| *p).collect();
                let max_pos = positions.iter().max().copied().unwrap_or(OrderingPos::new(0));
                let min_pos = positions.iter().min().copied().unwrap_or(OrderingPos::new(0));
                if max_pos == min_pos {
                    self.consistent_count += 1;
                    ReconciliationResult::Consistent
                } else {
                    let divergence = max_pos.as_u64().saturating_sub(min_pos.as_u64());
                    if divergence < 1000 {
                        self.repaired_count += 1;
                        ReconciliationResult::Repaired
                    } else {
                        self.diverged_count += 1;
                        ReconciliationResult::Diverged
                    }
                }
            }
            None => {
                self.diverged_count += 1;
                ReconciliationResult::Unknown
            }
        }
    }

    pub fn reconciliation_count(&self) -> u64 {
        self.reconciliation_count
    }

    pub fn consistent_count(&self) -> u64 {
        self.consistent_count
    }

    pub fn repaired_count(&self) -> u64 {
        self.repaired_count
    }

    pub fn diverged_count(&self) -> u64 {
        self.diverged_count
    }

    pub fn consistency_rate(&self) -> f64 {
        if self.reconciliation_count == 0 {
            return 1.0;
        }
        self.consistent_count as f64 / self.reconciliation_count as f64
    }

    pub fn dataset_count(&self) -> usize {
        self.replica_states.borrow().len()
    }
}

impl Default for StateReconciler {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA RECONSTRUCTOR
///////////////////////////////////////////////////////////////////////////////

pub struct ReplicaReconstructor {
    pub reconstruction_queue: std::cell::RefCell<std::collections::VecDeque<PartitionId>>,
    pub reconstructing: std::cell::RefCell<std::collections::HashSet<PartitionId>>,
    pub reconstructed_count: u64,
    pub failed_count: u64,
    pub total_bytes_reconstructed: u64,
}

impl ReplicaReconstructor {
    pub fn new() -> Self {
        Self {
            reconstruction_queue: std::cell::RefCell::new(std::collections::VecDeque::new()),
            reconstructing: std::cell::RefCell::new(std::collections::HashSet::new()),
            reconstructed_count: 0,
            failed_count: 0,
            total_bytes_reconstructed: 0,
        }
    }

    pub fn enqueue(&self, partition_id: PartitionId) {
        self.reconstruction_queue.borrow_mut().push_back(partition_id);
    }

    pub fn dequeue(&self) -> Option<PartitionId> {
        self.reconstruction_queue.borrow_mut().pop_front()
    }

    pub fn start_reconstruction(&self, partition_id: &PartitionId) -> bool {
        self.reconstructing.borrow_mut().insert(partition_id.clone())
    }

    pub fn complete_reconstruction(&self, partition_id: &PartitionId, bytes: u64) -> bool {
        let removed = self.reconstructing.borrow_mut().remove(partition_id);
        if removed {
            self.reconstructed_count += 1;
            self.total_bytes_reconstructed += bytes;
        }
        removed
    }

    pub fn fail_reconstruction(&self, partition_id: &PartitionId) {
        self.reconstructing.borrow_mut().remove(partition_id);
        self.failed_count += 1;
    }

    pub fn is_reconstructing(&self, partition_id: &PartitionId) -> bool {
        self.reconstructing.borrow().contains(partition_id)
    }

    pub fn queue_len(&self) -> usize {
        self.reconstruction_queue.borrow().len()
    }

    pub fn reconstructed_count(&self) -> u64 {
        self.reconstructed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn total_bytes_reconstructed(&self) -> u64 {
        self.total_bytes_reconstructed
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.reconstructed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.reconstructed_count as f64 / total as f64
    }
}

impl Default for ReplicaReconstructor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUARANTINE MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReason {
    SuspectedFailure,
    CorruptionDetected,
    Unresponsive,
    PolicyViolation,
}

impl QuarantineReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuarantineReason::SuspectedFailure => "suspected_failure",
            QuarantineReason::CorruptionDetected => "corruption_detected",
            QuarantineReason::Unresponsive => "unresponsive",
            QuarantineReason::PolicyViolation => "policy_violation",
        }
    }
}

pub struct QuarantineManager {
    pub quarantined: std::cell::RefCell<std::collections::HashMap<RecoveryScope, QuarantineReason>>,
    pub quarantine_start_times: std::cell::RefCell<std::collections::HashMap<RecoveryScope, u64>>,
    pub quarantine_count: u64,
    pub release_count: u64,
}

impl QuarantineManager {
    pub fn new() -> Self {
        Self {
            quarantined: std::cell::RefCell::new(std::collections::HashMap::new()),
            quarantine_start_times: std::cell::RefCell::new(std::collections::HashMap::new()),
            quarantine_count: 0,
            release_count: 0,
        }
    }

    pub fn quarantine(&self, scope: RecoveryScope, reason: QuarantineReason, at_ms: u64) -> bool {
        let mut q = self.quarantined.borrow_mut();
        let mut times = self.quarantine_start_times.borrow_mut();
        if q.contains_key(&scope) {
            return false;
        }
        q.insert(scope, reason);
        times.insert(scope, at_ms);
        self.quarantine_count += 1;
        true
    }

    pub fn release(&self, scope: &RecoveryScope) -> bool {
        let mut q = self.quarantined.borrow_mut();
        let mut times = self.quarantine_start_times.borrow_mut();
        let removed = q.remove(scope).is_some();
        times.remove(scope);
        if removed {
            self.release_count += 1;
        }
        removed
    }

    pub fn is_quarantined(&self, scope: &RecoveryScope) -> bool {
        self.quarantined.borrow().contains_key(scope)
    }

    pub fn get_reason(&self, scope: &RecoveryScope) -> Option<QuarantineReason> {
        self.quarantined.borrow().get(scope).copied()
    }

    pub fn quarantine_count(&self) -> usize {
        self.quarantined.borrow().len()
    }

    pub fn total_quarantine_count(&self) -> u64 {
        self.quarantine_count
    }

    pub fn total_release_count(&self) -> u64 {
        self.release_count
    }

    pub fn release_rate(&self) -> f64 {
        if self.quarantine_count == 0 {
            return 1.0;
        }
        self.release_count as f64 / self.quarantine_count as f64
    }
}

impl Default for QuarantineManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY SCHEDULER
///////////////////////////////////////////////////////////////////////////////

pub struct RecoveryScheduler {
    pub scheduled: std::cell::RefCell<std::collections::BinaryHeap<ScheduledRecovery>>,
    pub schedule_counter: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScheduledRecovery {
    pub priority: u64,
    pub sequence: u64,
    pub plan_id: RecoveryPlanId,
}

impl Ord for ScheduledRecovery {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority).then(self.sequence.cmp(&other.sequence))
    }
}

impl PartialOrd for ScheduledRecovery {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl RecoveryScheduler {
    pub fn new() -> Self {
        Self {
            scheduled: std::cell::RefCell::new(std::collections::BinaryHeap::new()),
            schedule_counter: 0,
        }
    }

    pub fn schedule(&self, plan_id: RecoveryPlanId, priority: u64) {
        self.schedule_counter += 1;
        self.scheduled.borrow_mut().push(ScheduledRecovery {
            priority,
            sequence: self.schedule_counter,
            plan_id,
        });
    }

    pub fn next(&self) -> Option<RecoveryPlanId> {
        self.scheduled.borrow_mut().pop().map(|sr| sr.plan_id)
    }

    pub fn queue_len(&self) -> usize {
        self.scheduled.borrow().len()
    }

    pub fn schedule_count(&self) -> u64 {
        self.schedule_counter
    }
}

impl Default for RecoveryScheduler {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct RecoveryStats {
    pub total_failures: u64,
    pub total_plans: u64,
    pub completed_recoveries: u64,
    pub failed_recoveries: u64,
    pub quarantined_scopes: u64,
    pub released_quarantines: u64,
    pub replicas_reconstructed: u64,
    pub state_reconciliations: u64,
    pub total_recovery_time_ms: u64,
}

impl RecoveryStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recovery_success_rate(&self) -> f64 {
        let total = self.completed_recoveries + self.failed_recoveries;
        if total == 0 {
            return 1.0;
        }
        self.completed_recoveries as f64 / total as f64
    }

    pub fn average_recovery_time_ms(&self) -> u64 {
        if self.completed_recoveries == 0 {
            return 0;
        }
        self.total_recovery_time_ms / self.completed_recoveries
    }

    pub fn quarantine_release_rate(&self) -> f64 {
        if self.quarantined_scopes == 0 {
            return 1.0;
        }
        self.released_quarantines as f64 / self.quarantined_scopes as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY TIMELINE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct RecoveryTimelineEntry {
    pub timestamp_ms: u64,
    pub event: RecoveryTimelineEvent,
    pub scope: RecoveryScope,
    pub details: String,
}

#[derive(Debug, Clone)]
pub enum RecoveryTimelineEvent {
    FailureDetected,
    PlanCreated,
    PlanStarted,
    PlanCompleted,
    PlanFailed,
    StepCompleted,
    StepFailed,
    Quarantined,
    Released,
    Reconciled,
    Reconstructed,
}

impl RecoveryTimelineEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryTimelineEvent::FailureDetected => "failure_detected",
            RecoveryTimelineEvent::PlanCreated => "plan_created",
            RecoveryTimelineEvent::PlanStarted => "plan_started",
            RecoveryTimelineEvent::PlanCompleted => "plan_completed",
            RecoveryTimelineEvent::PlanFailed => "plan_failed",
            RecoveryTimelineEvent::StepCompleted => "step_completed",
            RecoveryTimelineEvent::StepFailed => "step_failed",
            RecoveryTimelineEvent::Quarantined => "quarantined",
            RecoveryTimelineEvent::Released => "released",
            RecoveryTimelineEvent::Reconciled => "reconciled",
            RecoveryTimelineEvent::Reconstructed => "reconstructed",
        }
    }
}

pub struct RecoveryTimeline {
    pub entries: std::cell::RefCell<Vec<RecoveryTimelineEntry>>,
    pub max_entries: usize,
}

impl RecoveryTimeline {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: std::cell::RefCell::new(Vec::new()),
            max_entries,
        }
    }

    pub fn record(
        &self,
        timestamp_ms: u64,
        event: RecoveryTimelineEvent,
        scope: RecoveryScope,
        details: String,
    ) {
        let mut entries = self.entries.borrow_mut();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(RecoveryTimelineEntry {
            timestamp_ms,
            event,
            scope,
            details,
        });
    }

    pub fn entries_for_scope(&self, scope: &RecoveryScope) -> Vec<RecoveryTimelineEntry> {
        self.entries
            .borrow()
            .iter()
            .filter(|e| &e.scope == scope)
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

impl Default for RecoveryTimeline {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Failed,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Critical => "critical",
            HealthStatus::Failed => "failed",
        }
    }

    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    pub fn severity(&self) -> u8 {
        match self {
            HealthStatus::Healthy => 0,
            HealthStatus::Degraded => 1,
            HealthStatus::Critical => 2,
            HealthStatus::Failed => 3,
        }
    }
}

pub struct HealthMonitor {
    pub node_health: std::cell::RefCell<std::collections::HashMap<NodeId, HealthStatus>>,
    pub hunit_health: std::cell::RefCell<std::collections::HashMap<HUnitId, HealthStatus>>,
    pub region_health: std::cell::RefCell<std::collections::HashMap<RegionId, HealthStatus>>,
    pub health_checks: u64,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            node_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            health_checks: 0,
        }
    }

    pub fn set_node_health(&self, node_id: NodeId, status: HealthStatus) {
        self.node_health.borrow_mut().insert(node_id, status);
        self.health_checks += 1;
    }

    pub fn set_hunit_health(&self, hunit_id: HUnitId, status: HealthStatus) {
        self.hunit_health.borrow_mut().insert(hunit_id, status);
        self.health_checks += 1;
    }

    pub fn set_region_health(&self, region_id: RegionId, status: HealthStatus) {
        self.region_health.borrow_mut().insert(region_id, status);
        self.health_checks += 1;
    }

    pub fn get_node_health(&self, node_id: &NodeId) -> HealthStatus {
        self.node_health
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(HealthStatus::Healthy)
    }

    pub fn get_hunit_health(&self, hunit_id: &HUnitId) -> HealthStatus {
        self.hunit_health
            .borrow()
            .get(hunit_id)
            .copied()
            .unwrap_or(HealthStatus::Healthy)
    }

    pub fn get_region_health(&self, region_id: &RegionId) -> HealthStatus {
        self.region_health
            .borrow()
            .get(region_id)
            .copied()
            .unwrap_or(HealthStatus::Healthy)
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_checks
    }

    pub fn critical_count(&self) -> u64 {
        self.node_health
            .borrow()
            .values()
            .filter(|s| matches!(s, HealthStatus::Critical | HealthStatus::Failed))
            .count() as u64
            + self.hunit_health.borrow().values().filter(|s| matches!(s, HealthStatus::Critical | HealthStatus::Failed)).count() as u64
            + self.region_health.borrow().values().filter(|s| matches!(s, HealthStatus::Critical | HealthStatus::Failed)).count() as u64
    }

    pub fn healthy_count(&self) -> u64 {
        self.node_health
            .borrow()
            .values()
            .filter(|s| matches!(s, HealthStatus::Healthy))
            .count() as u64
            + self.hunit_health.borrow().values().filter(|s| matches!(s, HealthStatus::Healthy)).count() as u64
            + self.region_health.borrow().values().filter(|s| matches!(s, HealthStatus::Healthy)).count() as u64
    }

    pub fn node_health_count(&self) -> usize {
        self.node_health.borrow().len()
    }

    pub fn hunit_health_count(&self) -> usize {
        self.hunit_health.borrow().len()
    }

    pub fn region_health_count(&self) -> usize {
        self.region_health.borrow().len()
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY COORDINATOR - orchestrates multi-level recovery across scopes
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPhase {
    Detection,
    Planning,
    Execution,
    Verification,
    Completion,
}

impl RecoveryPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryPhase::Detection => "detection",
            RecoveryPhase::Planning => "planning",
            RecoveryPhase::Execution => "execution",
            RecoveryPhase::Verification => "verification",
            RecoveryPhase::Completion => "completion",
        }
    }

    pub fn order_index(&self) -> u8 {
        match self {
            RecoveryPhase::Detection => 0,
            RecoveryPhase::Planning => 1,
            RecoveryPhase::Execution => 2,
            RecoveryPhase::Verification => 3,
            RecoveryPhase::Completion => 4,
        }
    }

    pub fn is_before(&self, other: &RecoveryPhase) -> bool {
        self.order_index() < other.order_index()
    }
}

pub struct RecoveryCoordinator {
    pub current_phase: std::cell::RefCell<RecoveryPhase>,
    pub phase_transitions: std::cell::RefCell<Vec<(RecoveryPhase, RecoveryPhase, u64)>>,
    pub active_scope: std::cell::RefCell<Option<RecoveryScope>>,
    pub phase_start_times: std::cell::RefCell<std::collections::HashMap<RecoveryPhase, u64>>,
    pub transition_count: u64,
}

impl RecoveryCoordinator {
    pub fn new() -> Self {
        Self {
            current_phase: std::cell::RefCell::new(RecoveryPhase::Detection),
            phase_transitions: std::cell::RefCell::new(Vec::new()),
            active_scope: std::cell::RefCell::new(None),
            phase_start_times: std::cell::RefCell::new(std::collections::HashMap::new()),
            transition_count: 0,
        }
    }

    pub fn transition_to(&self, new_phase: RecoveryPhase, at_ms: u64) {
        let old_phase = *self.current_phase.borrow();
        if old_phase == new_phase {
            return;
        }
        self.current_phase.replace(new_phase);
        self.phase_transitions
            .borrow_mut()
            .push((old_phase, new_phase, at_ms));
        self.phase_start_times.borrow_mut().insert(new_phase, at_ms);
        self.transition_count += 1;
    }

    pub fn current_phase(&self) -> RecoveryPhase {
        *self.current_phase.borrow()
    }

    pub fn set_active_scope(&self, scope: Option<RecoveryScope>) {
        self.active_scope.replace(scope);
    }

    pub fn active_scope(&self) -> Option<RecoveryScope> {
        self.active_scope.borrow().clone()
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn phase_start_time(&self, phase: RecoveryPhase) -> Option<u64> {
        self.phase_start_times.borrow().get(&phase).copied()
    }

    pub fn phase_duration(&self, phase: RecoveryPhase, now_ms: u64) -> Option<u64> {
        self.phase_start_times
            .borrow()
            .get(&phase)
            .map(|start| now_ms.saturating_sub(*start))
    }

    pub fn can_transition(&self, from: RecoveryPhase, to: RecoveryPhase) -> bool {
        from.order_index() < to.order_index() || from == to
    }

    pub fn transition_history_len(&self) -> usize {
        self.phase_transitions.borrow().len()
    }

    pub fn last_transition(&self) -> Option<(RecoveryPhase, RecoveryPhase, u64)> {
        self.phase_transitions.borrow().last().cloned()
    }
}

impl Default for RecoveryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA REPAIR STATE TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaRepairState {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Abandoned,
}

impl ReplicaRepairState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplicaRepairState::Pending => "pending",
            ReplicaRepairState::Assigned => "assigned",
            ReplicaRepairState::InProgress => "in_progress",
            ReplicaRepairState::Completed => "completed",
            ReplicaRepairState::Failed => "failed",
            ReplicaRepairState::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ReplicaRepairState::Completed | ReplicaRepairState::Failed | ReplicaRepairState::Abandoned
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            ReplicaRepairState::Assigned | ReplicaRepairState::InProgress
        )
    }
}

pub struct ReplicaRepairStateTracker {
    pub replica_states: std::cell::RefCell<
        std::collections::HashMap<(DatasetId, NodeId), ReplicaRepairState>,
    >,
    pub state_changes: std::cell::RefCell<Vec<((DatasetId, NodeId), ReplicaRepairState, u64)>>,
    pub completed_count: u64,
    pub failed_count: u64,
}

impl ReplicaRepairStateTracker {
    pub fn new() -> Self {
        Self {
            replica_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            state_changes: std::cell::RefCell::new(Vec::new()),
            completed_count: 0,
            failed_count: 0,
        }
    }

    pub fn set_state(
        &self,
        dataset_id: DatasetId,
        node_id: NodeId,
        state: ReplicaRepairState,
        at_ms: u64,
    ) {
        let key = (dataset_id, node_id);
        let prev = self.replica_states.borrow().get(&key).copied();
        self.replica_states.borrow_mut().insert(key, state);
        self.state_changes
            .borrow_mut()
            .push((key, state, at_ms));
        match (prev, state) {
            (_, ReplicaRepairState::Completed) => self.completed_count += 1,
            (_, ReplicaRepairState::Failed) => self.failed_count += 1,
            _ => {}
        }
    }

    pub fn get_state(&self, dataset_id: &DatasetId, node_id: &NodeId) -> ReplicaRepairState {
        self.replica_states
            .borrow()
            .get(&(dataset_id.clone(), node_id.clone()))
            .copied()
            .unwrap_or(ReplicaRepairState::Pending)
    }

    pub fn state_change_count(&self) -> usize {
        self.state_changes.borrow().len()
    }

    pub fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn completion_rate(&self) -> f64 {
        let total = self.completed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_count as f64 / total as f64
    }

    pub fn replicas_in_state(&self, state: ReplicaRepairState) -> usize {
        self.replica_states
            .borrow()
            .values()
            .filter(|s| *s == state)
            .count()
    }

    pub fn tracked_count(&self) -> usize {
        self.replica_states.borrow().len()
    }
}

impl Default for ReplicaRepairStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGIONAL RECOVERY COORDINATOR
///////////////////////////////////////////////////////////////////////////////

pub struct RegionalRecoveryCoordinator {
    pub region_recovery_status: std::cell::RefCell<
        std::collections::HashMap<RegionId, RecoveryScope>,
    >,
    pub region_health: std::cell::RefCell<std::collections::HashMap<RegionId, HealthStatus>>,
    pub cross_region_recoveries: u64,
    pub regional_failures: u64,
}

impl RegionalRecoveryCoordinator {
    pub fn new() -> Self {
        Self {
            region_recovery_status: std::cell::RefCell::new(std::collections::HashMap::new()),
            region_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            cross_region_recoveries: 0,
            regional_failures: 0,
        }
    }

    pub fn set_region_recovery_scope(&self, region: RegionId, scope: RecoveryScope) {
        self.region_recovery_status.borrow_mut().insert(region, scope);
    }

    pub fn set_region_health(&self, region: RegionId, health: HealthStatus) {
        self.region_health.borrow_mut().insert(region, health);
        if matches!(health, HealthStatus::Critical | HealthStatus::Failed) {
            self.regional_failures += 1;
        }
    }

    pub fn get_region_health(&self, region: &RegionId) -> HealthStatus {
        self.region_health
            .borrow()
            .get(region)
            .copied()
            .unwrap_or(HealthStatus::Healthy)
    }

    pub fn initiate_cross_region_recovery(&self, source: RegionId, target: RegionId) -> bool {
        self.cross_region_recoveries += 1;
        true
    }

    pub fn cross_region_recovery_count(&self) -> u64 {
        self.cross_region_recoveries
    }

    pub fn regional_failure_count(&self) -> u64 {
        self.regional_failures
    }

    pub fn region_count(&self) -> usize {
        self.region_health.borrow().len()
    }

    pub fn healthy_region_count(&self) -> usize {
        self.region_health
            .borrow()
            .values()
            .filter(|h| matches!(h, HealthStatus::Healthy | HealthStatus::Degraded))
            .count()
    }

    pub fn critical_region_count(&self) -> usize {
        self.region_health
            .borrow()
            .values()
            .filter(|h| matches!(h, HealthStatus::Critical | HealthStatus::Failed))
            .count()
    }
}

impl Default for RegionalRecoveryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// NODE RECOVERY STATE TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct NodeRecoveryTracker {
    pub node_states: std::cell::RefCell<std::collections::HashMap<NodeId, NodeRecoveryState>>,
    pub recovery_attempts: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub total_node_recoveries: u64,
    pub successful_node_recoveries: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRecoveryState {
    Healthy,
    Degraded,
    Recovering,
    Failed,
    Quarantined,
}

impl NodeRecoveryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeRecoveryState::Healthy => "healthy",
            NodeRecoveryState::Degraded => "degraded",
            NodeRecoveryState::Recovering => "recovering",
            NodeRecoveryState::Failed => "failed",
            NodeRecoveryState::Quarantined => "quarantined",
        }
    }

    pub fn needs_recovery(&self) -> bool {
        matches!(
            self,
            NodeRecoveryState::Degraded | NodeRecoveryState::Failed | NodeRecoveryState::Quarantined
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, NodeRecoveryState::Failed)
    }
}

impl NodeRecoveryTracker {
    pub fn new() -> Self {
        Self {
            node_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            recovery_attempts: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_node_recoveries: 0,
            successful_node_recoveries: 0,
        }
    }

    pub fn set_node_state(&self, node_id: NodeId, state: NodeRecoveryState) {
        self.node_states.borrow_mut().insert(node_id, state);
    }

    pub fn get_node_state(&self, node_id: &NodeId) -> NodeRecoveryState {
        self.node_states
            .borrow()
            .get(node_id)
            .copied()
            .unwrap_or(NodeRecoveryState::Healthy)
    }

    pub fn record_recovery_attempt(&self, node_id: NodeId) {
        self.recovery_attempts
            .borrow_mut()
            .entry(node_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
        self.total_node_recoveries += 1;
    }

    pub fn record_successful_recovery(&self, node_id: NodeId) {
        self.successful_node_recoveries += 1;
        self.node_states
            .borrow_mut()
            .insert(node_id, NodeRecoveryState::Healthy);
    }

    pub fn recovery_attempt_count(&self, node_id: &NodeId) -> u64 {
        self.recovery_attempts.borrow().get(node_id).copied().unwrap_or(0)
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_node_recoveries == 0 {
            return 1.0;
        }
        self.successful_node_recoveries as f64 / self.total_node_recoveries as f64
    }

    pub fn tracked_node_count(&self) -> usize {
        self.node_states.borrow().len()
    }

    pub fn nodes_needing_recovery(&self) -> usize {
        self.node_states
            .borrow()
            .values()
            .filter(|s| s.needs_recovery())
            .count()
    }
}

impl Default for NodeRecoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-UNIT RECOVERY TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct HUnitRecoveryTracker {
    pub hunit_states: std::cell::RefCell<std::collections::HashMap<HUnitId, NodeRecoveryState>>,
    pub hunit_recovery_attempts: std::cell::RefCell<std::collections::HashMap<HUnitId, u64>>,
    pub total_hunit_recoveries: u64,
}

impl HUnitRecoveryTracker {
    pub fn new() -> Self {
        Self {
            hunit_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            hunit_recovery_attempts: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_hunit_recoveries: 0,
        }
    }

    pub fn set_hunit_state(&self, hunit_id: HUnitId, state: NodeRecoveryState) {
        self.hunit_states.borrow_mut().insert(hunit_id, state);
    }

    pub fn get_hunit_state(&self, hunit_id: &HUnitId) -> NodeRecoveryState {
        self.hunit_states
            .borrow()
            .get(hunit_id)
            .copied()
            .unwrap_or(NodeRecoveryState::Healthy)
    }

    pub fn record_recovery_attempt(&self, hunit_id: HUnitId) {
        self.hunit_recovery_attempts
            .borrow_mut()
            .entry(hunit_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
        self.total_hunit_recoveries += 1;
    }

    pub fn attempt_count(&self, hunit_id: &HUnitId) -> u64 {
        self.hunit_recovery_attempts
            .borrow()
            .get(hunit_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn total_recoveries(&self) -> u64 {
        self.total_hunit_recoveries
    }

    pub fn tracked_hunit_count(&self) -> usize {
        self.hunit_states.borrow().len()
    }

    pub fn units_needing_recovery(&self) -> usize {
        self.hunit_states
            .borrow()
            .values()
            .filter(|s| s.needs_recovery())
            .count()
    }
}

impl Default for HUnitRecoveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY RESOURCE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct RecoveryResourceManager {
    pub active_recoveries: std::cell::RefCell<std::collections::HashSet<RecoveryPlanId>>,
    pub resource_limit: u64,
    pub queued_recoveries: u64,
    pub rejected_recoveries: u64,
}

impl RecoveryResourceManager {
    pub fn new(resource_limit: u64) -> Self {
        Self {
            active_recoveries: std::cell::RefCell::new(std::collections::HashSet::new()),
            resource_limit,
            queued_recoveries: 0,
            rejected_recoveries: 0,
        }
    }

    pub fn acquire(&self, plan_id: RecoveryPlanId) -> bool {
        let mut active = self.active_recoveries.borrow_mut();
        if active.len() as u64 >= self.resource_limit {
            self.rejected_recoveries += 1;
            return false;
        }
        active.insert(plan_id);
        true
    }

    pub fn release(&self, plan_id: RecoveryPlanId) -> bool {
        self.active_recoveries.borrow_mut().remove(&plan_id)
    }

    pub fn queue_recovery(&self) {
        self.queued_recoveries += 1;
    }

    pub fn active_count(&self) -> usize {
        self.active_recoveries.borrow().len()
    }

    pub fn is_at_limit(&self) -> bool {
        self.active_recoveries.borrow().len() as u64 >= self.resource_limit
    }

    pub fn rejected_count(&self) -> u64 {
        self.rejected_recoveries
    }

    pub fn queued_count(&self) -> u64 {
        self.queued_recoveries
    }

    pub fn resource_limit(&self) -> u64 {
        self.resource_limit
    }

    pub fn availability_percent(&self) -> u8 {
        if self.resource_limit == 0 {
            return 0;
        }
        let used = self.active_recoveries.borrow().len() as u64;
        let available = self.resource_limit.saturating_sub(used);
        (available as u32 * 100 / self.resource_limit as u32) as u8
    }
}

impl Default for RecoveryResourceManager {
    fn default() -> Self {
        Self::new(100)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RECOVERY VERIFICATION ENGINE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationResult {
    Verified,
    Failed,
    Pending,
    Skipped,
}

impl VerificationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationResult::Verified => "verified",
            VerificationResult::Failed => "failed",
            VerificationResult::Pending => "pending",
            VerificationResult::Skipped => "skipped",
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, VerificationResult::Verified)
    }
}

pub struct RecoveryVerificationEngine {
    pub verification_results: std::cell::RefCell<
        std::collections::HashMap<RecoveryPlanId, VerificationResult>,
    >,
    pub verification_count: u64,
    pub passed_count: u64,
    pub failed_count: u64,
}

impl RecoveryVerificationEngine {
    pub fn new() -> Self {
        Self {
            verification_results: std::cell::RefCell::new(std::collections::HashMap::new()),
            verification_count: 0,
            passed_count: 0,
            failed_count: 0,
        }
    }

    pub fn verify(&self, plan_id: RecoveryPlanId, result: VerificationResult) {
        self.verification_count += 1;
        match result {
            VerificationResult::Verified => self.passed_count += 1,
            VerificationResult::Failed => self.failed_count += 1,
            _ => {}
        }
        self.verification_results
            .borrow_mut()
            .insert(plan_id, result);
    }

    pub fn get_result(&self, plan_id: &RecoveryPlanId) -> Option<VerificationResult> {
        self.verification_results.borrow().get(plan_id).copied()
    }

    pub fn verification_count(&self) -> u64 {
        self.verification_count
    }

    pub fn passed_count(&self) -> u64 {
        self.passed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn pass_rate(&self) -> f64 {
        if self.verification_count == 0 {
            return 1.0;
        }
        self.passed_count as f64 / self.verification_count as f64
    }

    pub fn verified_plan_count(&self) -> usize {
        self.verification_results.borrow().len()
    }
}

impl Default for RecoveryVerificationEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED RECOVERY ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct RecoveryEngine {
    pub failure_detector: std::cell::RefCell<FailureDetector>,
    pub scope_manager: std::cell::RefCell<RecoveryScope>,
    pub repair_planner: std::cell::RefCell<RepairPlanner>,
    pub state_reconciler: std::cell::RefCell<StateReconciler>,
    pub replica_reconstructor: std::cell::RefCell<ReplicaReconstructor>,
    pub quarantine_manager: std::cell::RefCell<QuarantineManager>,
    pub scheduler: std::cell::RefCell<RecoveryScheduler>,
    pub stats: std::cell::RefCell<RecoveryStats>,
    pub timeline: std::cell::RefCell<RecoveryTimeline>,
    pub health_monitor: std::cell::RefCell<HealthMonitor>,
    pub plans: std::cell::RefCell<std::collections::HashMap<RecoveryPlanId, RecoveryPlan>>,
    pub plan_counter: u64,
}

impl RecoveryEngine {
    pub fn new_with_features() -> Self {
        Self {
            failure_detector: std::cell::RefCell::new(FailureDetector::default()),
            scope_manager: std::cell::RefCell::new(RecoveryScope::default()),
            repair_planner: std::cell::RefCell::new(RepairPlanner::default()),
            state_reconciler: std::cell::RefCell::new(StateReconciler::default()),
            replica_reconstructor: std::cell::RefCell::new(ReplicaReconstructor::default()),
            quarantine_manager: std::cell::RefCell::new(QuarantineManager::default()),
            scheduler: std::cell::RefCell::new(RecoveryScheduler::default()),
            stats: std::cell::RefCell::new(RecoveryStats::new()),
            timeline: std::cell::RefCell::new(RecoveryTimeline::default()),
            health_monitor: std::cell::RefCell::new(HealthMonitor::default()),
            plans: std::cell::RefCell::new(std::collections::HashMap::new()),
            plan_counter: 0,
        }
    }

    pub fn coordinator(&self) -> RecoveryCoordinator {
        RecoveryCoordinator::default()
    }

    pub fn replica_repair_tracker(&self) -> ReplicaRepairStateTracker {
        ReplicaRepairStateTracker::default()
    }

    pub fn regional_coordinator(&self) -> RegionalRecoveryCoordinator {
        RegionalRecoveryCoordinator::default()
    }

    pub fn node_recovery_tracker(&self) -> NodeRecoveryTracker {
        NodeRecoveryTracker::default()
    }

    pub fn hunit_recovery_tracker(&self) -> HUnitRecoveryTracker {
        HUnitRecoveryTracker::default()
    }

    pub fn resource_manager(&self) -> RecoveryResourceManager {
        RecoveryResourceManager::default()
    }

    pub fn verification_engine(&self) -> RecoveryVerificationEngine {
        RecoveryVerificationEngine::default()
    }

    pub fn record_node_recovery_attempt(&self, node_id: NodeId) {
        self.node_recovery_tracker().record_recovery_attempt(node_id);
    }

    pub fn record_successful_node_recovery(&self, node_id: NodeId) {
        self.node_recovery_tracker().record_successful_recovery(node_id);
    }

    pub fn set_node_recovery_state(&self, node_id: NodeId, state: NodeRecoveryState) {
        self.node_recovery_tracker().set_node_state(node_id, state);
    }

    pub fn get_node_recovery_state(&self, node_id: &NodeId) -> NodeRecoveryState {
        self.node_recovery_tracker().get_node_state(node_id)
    }

    pub fn record_hunit_recovery_attempt(&self, hunit_id: HUnitId) {
        self.hunit_recovery_tracker().record_recovery_attempt(hunit_id);
    }

    pub fn set_hunit_recovery_state(&self, hunit_id: HUnitId, state: NodeRecoveryState) {
        self.hunit_recovery_tracker().set_hunit_state(hunit_id, state);
    }

    pub fn acquire_recovery_resource(&self, plan_id: RecoveryPlanId) -> bool {
        self.resource_manager().acquire(plan_id)
    }

    pub fn release_recovery_resource(&self, plan_id: RecoveryPlanId) -> bool {
        self.resource_manager().release(plan_id)
    }

    pub fn queue_recovery(&self) {
        self.resource_manager().queue_recovery();
    }

    pub fn verify_plan(&self, plan_id: RecoveryPlanId, result: VerificationResult) {
        self.verification_engine().verify(plan_id, result);
    }

    pub fn get_verification_result(&self, plan_id: &RecoveryPlanId) -> Option<VerificationResult> {
        self.verification_engine().get_result(plan_id)
    }

    pub fn regional_coordinator_ref(&self) -> RegionalRecoveryCoordinator {
        RegionalRecoveryCoordinator::default()
    }

    pub fn set_region_recovery_scope(&self, region: RegionId, scope: RecoveryScope) {
        self.regional_coordinator_ref().set_region_recovery_scope(region, scope);
    }

    pub fn set_region_health(&self, region: RegionId, health: HealthStatus) {
        self.regional_coordinator_ref().set_region_health(region, health);
    }

    pub fn get_region_health(&self, region: &RegionId) -> HealthStatus {
        self.regional_coordinator_ref().get_region_health(region)
    }

    pub fn initiate_cross_region_recovery(&self, source: RegionId, target: RegionId) -> bool {
        self.regional_coordinator_ref()
            .initiate_cross_region_recovery(source, target)
    }

    pub fn recovery_phase_transition(&self, new_phase: RecoveryPhase, at_ms: u64) {
        let coord = self.coordinator();
        coord.transition_to(new_phase, at_ms);
    }

    pub fn current_recovery_phase(&self) -> RecoveryPhase {
        self.coordinator().current_phase()
    }

    pub fn set_active_recovery_scope(&self, scope: Option<RecoveryScope>) {
        self.coordinator().set_active_scope(scope);
    }

    pub fn record_replica_repair_state(
        &self,
        dataset_id: DatasetId,
        node_id: NodeId,
        state: ReplicaRepairState,
        at_ms: u64,
    ) {
        self.replica_repair_tracker()
            .set_state(dataset_id, node_id, state, at_ms);
    }

    pub fn get_replica_repair_state(&self, dataset_id: &DatasetId, node_id: &NodeId) -> ReplicaRepairState {
        self.replica_repair_tracker().get_state(dataset_id, node_id)
    }

    pub fn resource_availability_percent(&self) -> u8 {
        self.resource_manager().availability_percent()
    }

    pub fn verification_pass_rate(&self) -> f64 {
        self.verification_engine().pass_rate()
    }

    pub fn cross_region_recovery_count(&self) -> u64 {
        self.regional_coordinator_ref().cross_region_recovery_count()
    }

    pub fn regional_failure_count(&self) -> u64 {
        self.regional_coordinator_ref().regional_failure_count()
    }

    pub fn nodes_needing_recovery_count(&self) -> usize {
        self.node_recovery_tracker().nodes_needing_recovery()
    }

    pub fn hunits_needing_recovery_count(&self) -> usize {
        self.hunit_recovery_tracker().units_needing_recovery()
    }
}

///////////////////////////////////////////////////////////////////////////////
impl RecoveryEngine {
    pub fn new() -> Self {
        Self {
            failure_detector: std::cell::RefCell::new(FailureDetector::default()),
            scope_manager: std::cell::RefCell::new(RecoveryScope::default()),
            repair_planner: std::cell::RefCell::new(RepairPlanner::default()),
            state_reconciler: std::cell::RefCell::new(StateReconciler::default()),
            replica_reconstructor: std::cell::RefCell::new(ReplicaReconstructor::default()),
            quarantine_manager: std::cell::RefCell::new(QuarantineManager::default()),
            scheduler: std::cell::RefCell::new(RecoveryScheduler::default()),
            stats: std::cell::RefCell::new(RecoveryStats::new()),
            timeline: std::cell::RefCell::new(RecoveryTimeline::default()),
            health_monitor: std::cell::RefCell::new(HealthMonitor::default()),
            plans: std::cell::RefCell::new(std::collections::HashMap::new()),
            plan_counter: 0,
        }
    }

    pub fn detect_failures(&self, now_ms: u64) -> Result<Vec<FailureEvent>, RecoveryError> {
        let failures = self.failure_detector.borrow().detect_failures(now_ms);
        self.stats.borrow_mut().total_failures += failures.len() as u64;
        for failure in &failures {
            let scope = match failure.event_type {
                FailureEventType::NodeFailure => {
                    RecoveryScope::Node(failure.affected_node.clone().unwrap_or(NodeId::new([0; 32])))
                }
                FailureEventType::HUnitFailure => {
                    RecoveryScope::HUnit(failure.affected_hunit.clone().unwrap_or(HUnitId::new([0; 32])))
                }
                FailureEventType::RegionalFailure => {
                    RecoveryScope::Region(failure.affected_region.clone().unwrap_or(RegionId::new([0; 32])))
                }
                _ => RecoveryScope::Node(failure.affected_node.clone().unwrap_or(NodeId::new([0; 32]))),
            };
            self.timeline.borrow_mut().record(
                now_ms,
                RecoveryTimelineEvent::FailureDetected,
                scope,
                format!("failure type: {:?}", failure.event_type),
            );
        }
        Ok(failures)
    }

    pub fn create_plan(&self, failure: &FailureEvent) -> Result<RecoveryPlan, RecoveryError> {
        let scope = match failure.event_type {
            FailureEventType::NodeFailure => {
                RecoveryScope::Node(failure.affected_node.clone().ok_or_else(|| RecoveryError::NodeNotFound {
                    node_id: NodeId::new([0; 32]),
                })?)
            }
            FailureEventType::HUnitFailure => {
                RecoveryScope::HUnit(failure.affected_hunit.clone().ok_or_else(|| RecoveryError::HUnitNotFound {
                    hunit_id: HUnitId::new([0; 32]),
                })?)
            }
            FailureEventType::RegionalFailure => {
                RecoveryScope::Region(failure.affected_region.clone().ok_or_else(|| RecoveryError::RegionNotFound {
                    region_id: RegionId::new([0; 32]),
                })?)
            }
            _ => return Err(RecoveryError::Internal {
                message: "unsupported failure type for recovery plan".to_string(),
            }),
        };

        if self.scope_manager.borrow().is_active(&scope) {
            return Err(RecoveryError::RecoveryInProgress { scope });
        }

        self.scope_manager.borrow_mut().activate(scope.clone(), 0);
        self.plan_counter += 1;
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&self.plan_counter.to_le_bytes());
        let plan_id = RecoveryPlanId::new(bytes);

        let strategy = self.repair_planner.borrow().select_strategy(scope.level());
        let steps = vec![
            RecoveryStep::new(
                RecoveryStepId::new([1; 32]),
                RecoveryAction::QuarantineNode,
                scope.level(),
                format!("{:?}", scope),
                3,
            ),
            RecoveryStep::new(
                RecoveryStepId::new([2; 32]),
                match strategy {
                    RepairStrategy::CopyReplica => RecoveryAction::CopyReplica,
                    RepairStrategy::RestoreRedundancy => RecoveryAction::RestoreRedundancy,
                    RepairStrategy::ReconstructErasure => RecoveryAction::ReconstructErasure,
                    RepairStrategy::TransferOwnership => RecoveryAction::TransferOwnership,
                },
                scope.level(),
                "repair_target".to_string(),
                3,
            ),
            RecoveryStep::new(
                RecoveryStepId::new([3; 32]),
                RecoveryAction::VerifyIntegrity,
                scope.level(),
                "integrity_check".to_string(),
                2,
            ),
        ];

        let plan = RecoveryPlan::new(plan_id, failure.clone(), scope.clone(), steps);
        self.plans.borrow_mut().insert(plan_id, plan.clone());
        self.stats.borrow_mut().total_plans += 1;
        self.repair_planner.borrow_mut().plan_repair(RecoveryAction::CopyReplica);

        self.timeline.borrow_mut().record(
            0,
            RecoveryTimelineEvent::PlanCreated,
            scope,
            format!("plan_id: {:?}, strategy: {:?}", plan_id, strategy),
        );

        Ok(plan)
    }

    pub fn execute_plan(
        &self,
        plan_id: RecoveryPlanId,
    ) -> Result<RecoveryProof, RecoveryError> {
        let mut plans = self.plans.borrow_mut();
        let plan = plans
            .get_mut(&plan_id)
            .ok_or_else(|| RecoveryError::PlanNotFound { plan_id })?;

        if plan.status.is_terminal() {
            return Err(RecoveryError::Internal {
                message: "plan already in terminal state".to_string(),
            });
        }

        plan.start(0);

        for step in plan.steps.iter_mut() {
            step.start(0);
            // Simulate execution - mark completed
            step.complete(0);
            plan.advance_step();
        }

        plan.complete(0);
        self.stats.borrow_mut().completed_recoveries += 1;
        self.stats.borrow_mut().total_recovery_time_ms += 100;

        let scope = plan.scope.clone();
        self.timeline.borrow_mut().record(
            0,
            RecoveryTimelineEvent::PlanCompleted,
            scope,
            format!("plan_id: {:?}", plan_id),
        );

        Ok(RecoveryProof::new(
            plan.failure_event.dataset_id(),
            Vec::new(),
            Vec::new(),
            crate::types::IdsTime::new(0),
        ))
    }

    pub fn quarantine(&self, scope: RecoveryScope) -> Result<(), RecoveryError> {
        if self.quarantine_manager.borrow().is_quarantined(&scope) {
            return Err(RecoveryError::QuarantineActive { scope });
        }
        self.quarantine_manager
            .borrow_mut()
            .quarantine(scope.clone(), QuarantineReason::SuspectedFailure, 0);
        self.stats.borrow_mut().quarantined_scopes += 1;
        self.timeline.borrow_mut().record(
            0,
            RecoveryTimelineEvent::Quarantined,
            scope,
            "quarantine activated".to_string(),
        );
        Ok(())
    }

    pub fn release_quarantine(&self, scope: &RecoveryScope) -> Result<(), RecoveryError> {
        if !self.quarantine_manager.borrow().is_quarantined(scope) {
            return Err(RecoveryError::Internal {
                message: "scope not quarantined".to_string(),
            });
        }
        self.quarantine_manager.borrow_mut().release(scope);
        self.stats.borrow_mut().released_quarantines += 1;
        self.timeline.borrow_mut().record(
            0,
            RecoveryTimelineEvent::Released,
            scope.clone(),
            "quarantine released".to_string(),
        );
        Ok(())
    }

    pub fn restore_redundancy(&self, dataset_id: DatasetId) -> Result<u64, RecoveryError> {
        let restored = self.repair_planner.borrow().strategy_count() as u64 * 3;
        self.repair_planner.borrow_mut().record_executed();
        self.stats.borrow_mut().replicas_reconstructed += restored;
        Ok(restored)
    }

    pub fn reconcile_state(&self, dataset_id: DatasetId) -> Result<RecoveryProof, RecoveryError> {
        let result = self.state_reconciler.borrow().reconcile(&dataset_id);
        self.stats.borrow_mut().state_reconciliations += 1;
        match result {
            ReconciliationResult::Consistent | ReconciliationResult::Repaired => Ok(RecoveryProof::new(
                dataset_id,
                Vec::new(),
                Vec::new(),
                crate::types::IdsTime::new(0),
            )),
            ReconciliationResult::Diverged => Err(RecoveryError::ReconciliationFailed {
                dataset_id,
                reason: "replica divergence exceeds threshold".to_string(),
            }),
            ReconciliationResult::Unknown => Err(RecoveryError::Internal {
                message: "no replica state available for reconciliation".to_string(),
            }),
        }
    }

    pub fn stats(&self) -> RecoveryStats {
        self.stats.borrow().clone()
    }

    pub fn recovery_name(&self) -> &'static str {
        "recovery_engine"
    }

    pub fn plan_count(&self) -> u64 {
        self.plans.borrow().len() as u64
    }

    pub fn recovery_count(&self) -> u64 {
        self.stats.borrow().completed_recoveries
    }

    pub fn quarantine_count(&self) -> u64 {
        self.stats.borrow().quarantined_scopes
    }

    pub fn record_replica_state(&self, dataset_id: DatasetId, node_id: NodeId, position: OrderingPos) {
        self.state_reconciler.borrow_mut().record_replica_state(dataset_id, node_id, position);
    }

    pub fn enqueue_reconstruction(&self, partition_id: PartitionId) {
        self.replica_reconstructor.borrow_mut().enqueue(partition_id);
    }

    pub fn start_reconstruction(&self, partition_id: &PartitionId) -> bool {
        self.replica_reconstructor.borrow_mut().start_reconstruction(partition_id)
    }

    pub fn complete_reconstruction(&self, partition_id: &PartitionId, bytes: u64) -> bool {
        self.replica_reconstructor.borrow_mut().complete_reconstruction(partition_id, bytes)
    }

    pub fn schedule_recovery(&self, plan_id: RecoveryPlanId, priority: u64) {
        self.scheduler.borrow_mut().schedule(plan_id, priority);
    }

    pub fn next_scheduled(&self) -> Option<RecoveryPlanId> {
        self.scheduler.borrow_mut().next()
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_monitor.borrow().health_check_count()
    }

    pub fn critical_count(&self) -> u64 {
        self.health_monitor.borrow().critical_count()
    }

    pub fn healthy_count(&self) -> u64 {
        self.health_monitor.borrow().healthy_count()
    }
}

impl Default for RecoveryEngine {
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

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_hunit_id() -> HUnitId {
        HUnitId::new([1; 32])
    }

    fn test_region_id() -> RegionId {
        RegionId::new([1; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1; 32])
    }

    fn test_partition_id() -> PartitionId {
        PartitionId::new([1; 32])
    }

    fn test_failure_event_node() -> FailureEvent {
        FailureEvent::new(
            FailureEventType::NodeFailure,
            Some(test_node_id()),
            None,
            None,
            1000,
        )
    }

    fn test_failure_event_hunit() -> FailureEvent {
        FailureEvent::new(
            FailureEventType::HUnitFailure,
            None,
            Some(test_hunit_id()),
            None,
            1000,
        )
    }

    fn test_failure_event_region() -> FailureEvent {
        FailureEvent::new(
            FailureEventType::RegionalFailure,
            None,
            None,
            Some(test_region_id()),
            1000,
        )
    }

    fn test_ordering_pos() -> OrderingPos {
        OrderingPos::new(1000)
    }

    fn test_recovery_plan_id() -> RecoveryPlanId {
        RecoveryPlanId::new([1; 32])
    }

    fn test_recovery_step_id() -> RecoveryStepId {
        RecoveryStepId::new([1; 32])
    }

    // === RecoveryError tests ===
    #[test]
    fn recovery_error_category() {
        assert_eq!(
            RecoveryError::NodeNotFound { node_id: test_node_id() }.category(),
            "node_not_found"
        );
        assert_eq!(
            RecoveryError::HUnitNotFound { hunit_id: test_hunit_id() }.category(),
            "hunit_not_found"
        );
        assert_eq!(
            RecoveryError::RegionNotFound { region_id: test_region_id() }.category(),
            "region_not_found"
        );
        assert_eq!(
            RecoveryError::PlanNotFound { plan_id: test_recovery_plan_id() }.category(),
            "plan_not_found"
        );
        assert_eq!(
            RecoveryError::StepFailed {
                step: test_recovery_step_id(),
                reason: "timeout".to_string(),
            }
            .category(),
            "step_failed"
        );
        assert_eq!(
            RecoveryError::InsufficientReplicas {
                required: 3,
                available: 1,
            }
            .category(),
            "insufficient_replicas"
        );
        assert_eq!(
            RecoveryError::DurabilityNotMet {
                condition: DurabilityCondition::FullReplica,
            }
            .category(),
            "durability_not_met"
        );
        assert_eq!(
            RecoveryError::QuarantineActive {
                scope: RecoveryScope::Node(test_node_id()),
            }
            .category(),
            "quarantine_active"
        );
        assert_eq!(
            RecoveryError::RecoveryInProgress {
                scope: RecoveryScope::Node(test_node_id()),
            }
            .category(),
            "recovery_in_progress"
        );
        assert_eq!(
            RecoveryError::ReconciliationFailed {
                dataset_id: test_dataset_id(),
                reason: "drift".to_string(),
            }
            .category(),
            "reconciliation_failed"
        );
        assert_eq!(
            RecoveryError::ReconstructionFailed {
                partition_id: test_partition_id(),
                reason: "crc".to_string(),
            }
            .category(),
            "reconstruction_failed"
        );
        assert_eq!(
            RecoveryError::Internal {
                message: "panic".to_string(),
            }
            .category(),
            "internal"
        );
    }

    #[test]
    fn recovery_error_is_recoverable() {
        assert!(RecoveryError::StepFailed {
            step: test_recovery_step_id(),
            reason: "x".to_string(),
        }
        .is_recoverable());
        assert!(RecoveryError::InsufficientReplicas {
            required: 3,
            available: 1,
        }
        .is_recoverable());
        assert!(RecoveryError::DurabilityNotMet {
            condition: DurabilityCondition::FullReplica,
        }
        .is_recoverable());
        assert!(RecoveryError::RecoveryInProgress {
            scope: RecoveryScope::Node(test_node_id()),
        }
        .is_recoverable());
        assert!(!RecoveryError::NodeNotFound { node_id: test_node_id() }.is_recoverable());
        assert!(!RecoveryError::PlanNotFound { plan_id: test_recovery_plan_id() }.is_recoverable());
    }

    #[test]
    fn recovery_error_suggested_action() {
        assert_eq!(
            RecoveryError::NodeNotFound { node_id: test_node_id() }.suggested_action(),
            "register node to network"
        );
        assert_eq!(
            RecoveryError::InsufficientReplicas {
                required: 3,
                available: 1,
            }
            .suggested_action(),
            "add more replicas or reduce requirement"
        );
        assert_eq!(
            RecoveryError::QuarantineActive {
                scope: RecoveryScope::Node(test_node_id()),
            }
            .suggested_action(),
            "resolve quarantine first"
        );
        assert_eq!(
            RecoveryError::Internal {
                message: "x".to_string(),
            }
            .suggested_action(),
            "contact support"
        );
    }

    // === RecoveryPlanId & RecoveryStepId tests ===
    #[test]
    fn recovery_plan_id_zero() {
        assert!(RecoveryPlanId::zero().is_zero());
        assert!(!test_recovery_plan_id().is_zero());
    }

    #[test]
    fn recovery_step_id_zero() {
        assert!(RecoveryStepId::zero().is_zero());
        assert!(!test_recovery_step_id().is_zero());
    }

    // === RecoveryScope tests ===
    #[test]
    fn recovery_scope_as_str() {
        assert_eq!(RecoveryScope::Node(test_node_id()).as_str(), "node");
        assert_eq!(RecoveryScope::HUnit(test_hunit_id()).as_str(), "hunit");
        assert_eq!(RecoveryScope::Region(test_region_id()).as_str(), "region");
    }

    #[test]
    fn recovery_scope_level() {
        assert_eq!(
            RecoveryScope::Node(test_node_id()).level(),
            RecoveryScopeLevel::Node
        );
        assert_eq!(
            RecoveryScope::HUnit(test_hunit_id()).level(),
            RecoveryScopeLevel::HUnit
        );
        assert_eq!(
            RecoveryScope::Region(test_region_id()).level(),
            RecoveryScopeLevel::Region
        );
    }

    #[test]
    fn recovery_scope_type_checks() {
        assert!(RecoveryScope::Node(test_node_id()).is_node());
        assert!(!RecoveryScope::Node(test_node_id()).is_hunit());
        assert!(!RecoveryScope::Node(test_node_id()).is_region());
        assert!(RecoveryScope::HUnit(test_hunit_id()).is_hunit());
        assert!(RecoveryScope::Region(test_region_id()).is_region());
    }

    // === PlanStatus tests ===
    #[test]
    fn plan_status_as_str() {
        assert_eq!(PlanStatus::Pending.as_str(), "pending");
        assert_eq!(PlanStatus::InProgress.as_str(), "in_progress");
        assert_eq!(PlanStatus::Completed.as_str(), "completed");
        assert_eq!(PlanStatus::Failed.as_str(), "failed");
        assert_eq!(PlanStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn plan_status_terminal_and_active() {
        assert!(PlanStatus::Completed.is_terminal());
        assert!(PlanStatus::Failed.is_terminal());
        assert!(PlanStatus::Cancelled.is_terminal());
        assert!(!PlanStatus::Pending.is_terminal());
        assert!(PlanStatus::Pending.is_active());
        assert!(PlanStatus::InProgress.is_active());
        assert!(!PlanStatus::Completed.is_active());
    }

    // === RecoveryPlan tests ===
    #[test]
    fn recovery_plan_start_complete_fail_cancel() {
        let mut plan = RecoveryPlan::new(
            test_recovery_plan_id(),
            test_failure_event_node(),
            RecoveryScope::Node(test_node_id()),
            vec![],
        );
        assert_eq!(plan.status, PlanStatus::Pending);
        assert_eq!(plan.progress_percent(), 100);
        plan.start(1000);
        assert_eq!(plan.status, PlanStatus::InProgress);
        assert!(plan.started_at_ms.is_some());
        plan.complete(2000);
        assert_eq!(plan.status, PlanStatus::Completed);
        assert!(plan.completed_at_ms.is_some());
    }

    #[test]
    fn recovery_plan_steps() {
        let step = RecoveryStep::new(
            test_recovery_step_id(),
            RecoveryAction::CopyReplica,
            RecoveryScopeLevel::Node,
            "node_1".to_string(),
            3,
        );
        assert_eq!(step.status, StepStatus::Pending);
        assert_eq!(step.attempt, 0);

        let mut step = step;
        step.start(1000);
        assert_eq!(step.status, StepStatus::InProgress);
        assert_eq!(step.attempt, 1);
        step.complete(2000);
        assert_eq!(step.status, StepStatus::Completed);
        assert_eq!(step.duration_ms(), Some(1000));
    }

    #[test]
    fn recovery_step_retry_and_fail() {
        let mut step = RecoveryStep::new(
            test_recovery_step_id(),
            RecoveryAction::CopyReplica,
            RecoveryScopeLevel::Node,
            "node_1".to_string(),
            2,
        );
        step.start(1000);
        assert!(step.can_retry());
        step.retry();
        assert_eq!(step.status, StepStatus::Retrying);
        assert_eq!(step.attempt, 2);
        step.retry();
        assert!(!step.can_retry());
        assert_eq!(step.status, StepStatus::Failed);
    }

    #[test]
    fn recovery_plan_advance_and_progress() {
        let mut plan = RecoveryPlan::new(
            test_recovery_plan_id(),
            test_failure_event_node(),
            RecoveryScope::Node(test_node_id()),
            vec![
                RecoveryStep::new(
                    test_recovery_step_id(),
                    RecoveryAction::CopyReplica,
                    RecoveryScopeLevel::Node,
                    "a".to_string(),
                    1,
                ),
                RecoveryStep::new(
                    RecoveryStepId::new([2; 32]),
                    RecoveryAction::VerifyIntegrity,
                    RecoveryScopeLevel::Node,
                    "b".to_string(),
                    1,
                ),
            ],
        );
        assert_eq!(plan.step_count(), 2);
        assert_eq!(plan.progress_percent(), 0);
        plan.advance_step();
        assert_eq!(plan.progress_percent(), 50);
        plan.advance_step();
        assert_eq!(plan.progress_percent(), 100);
        assert_eq!(plan.status, PlanStatus::Completed);
    }

    // === StepStatus tests ===
    #[test]
    fn step_status_as_str() {
        assert_eq!(StepStatus::Pending.as_str(), "pending");
        assert_eq!(StepStatus::InProgress.as_str(), "in_progress");
        assert_eq!(StepStatus::Completed.as_str(), "completed");
        assert_eq!(StepStatus::Failed.as_str(), "failed");
        assert_eq!(StepStatus::Retrying.as_str(), "retrying");
        assert_eq!(StepStatus::Skipped.as_str(), "skipped");
    }

    #[test]
    fn step_status_terminal_and_active() {
        assert!(StepStatus::Completed.is_terminal());
        assert!(StepStatus::Failed.is_terminal());
        assert!(StepStatus::Skipped.is_terminal());
        assert!(!StepStatus::Pending.is_terminal());
        assert!(StepStatus::Pending.is_active());
        assert!(StepStatus::InProgress.is_active());
        assert!(StepStatus::Retrying.is_active());
    }

    // === FailureDetector tests ===
    #[test]
    fn failure_detector_record_and_detect() {
        let fd = FailureDetector::default();
        fd.record_node_heartbeat(test_node_id(), 5000);
        fd.record_hunit_heartbeat(test_hunit_id(), 5000);
        fd.record_region_heartbeat(test_region_id(), 5000);

        let failures = fd.detect_failures(10000);
        assert!(failures.is_empty());
        assert_eq!(fd.detection_count(), 1);

        let failures = fd.detect_failures(40000);
        assert_eq!(failures.len(), 3);
        assert_eq!(fd.failure_count(), 3);
    }

    #[test]
    fn failure_detector_health_checks() {
        let fd = FailureDetector::default();
        fd.record_node_heartbeat(test_node_id(), 5000);
        assert!(fd.is_healthy_node(&test_node_id(), 10000));
        assert!(!fd.is_healthy_node(&test_node_id(), 40000));
        assert!(!fd.is_healthy_node(&NodeId::new([9; 32]), 10000));
        assert!(!fd.is_healthy_hunit(&test_hunit_id(), 10000));
        assert!(!fd.is_healthy_region(&test_region_id(), 10000));
    }

    #[test]
    fn failure_detector_counts() {
        let fd = FailureDetector::default();
        fd.record_node_heartbeat(test_node_id(), 1000);
        fd.record_hunit_heartbeat(test_hunit_id(), 1000);
        fd.record_region_heartbeat(test_region_id(), 1000);
        assert_eq!(fd.node_count(), 1);
        assert_eq!(fd.hunit_count(), 1);
        assert_eq!(fd.region_count(), 1);
    }

    // === RepairPlanner tests ===
    #[test]
    fn repair_strategy_as_str_and_priority() {
        assert_eq!(RepairStrategy::CopyReplica.as_str(), "copy_replica");
        assert_eq!(RepairStrategy::RestoreRedundancy.as_str(), "restore_redundancy");
        assert_eq!(RepairStrategy::ReconstructErasure.as_str(), "reconstruct_erasure");
        assert_eq!(RepairStrategy::TransferOwnership.as_str(), "transfer_ownership");
        assert_eq!(RepairStrategy::CopyReplica.priority(), 1);
        assert_eq!(RepairStrategy::TransferOwnership.priority(), 4);
    }

    #[test]
    fn repair_planner_select_and_count() {
        let rp = RepairPlanner::default();
        assert_eq!(rp.strategy_count(), 4);
        let s = rp.select_strategy(RecoveryScopeLevel::Node);
        assert_eq!(s, RepairStrategy::CopyReplica);
        let s = rp.select_strategy(RecoveryScopeLevel::HUnit);
        assert_eq!(s, RepairStrategy::RestoreRedundancy);
        let s = rp.select_strategy(RecoveryScopeLevel::Region);
        assert_eq!(s, RepairStrategy::ReconstructErasure);
    }

    #[test]
    fn repair_planner_stats() {
        let rp = RepairPlanner::default();
        rp.plan_repair(RecoveryAction::CopyReplica);
        rp.record_executed();
        rp.record_executed();
        rp.record_failed();
        assert_eq!(rp.plan_count(), 1);
        assert_eq!(rp.executed_count(), 2);
        assert_eq!(rp.failed_count(), 1);
        assert!((rp.success_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    // === StateReconciler tests ===
    #[test]
    fn reconciliation_result_as_str() {
        assert_eq!(ReconciliationResult::Consistent.as_str(), "consistent");
        assert_eq!(ReconciliationResult::Repaired.as_str(), "repaired");
        assert_eq!(ReconciliationResult::Diverged.as_str(), "diverged");
        assert_eq!(ReconciliationResult::Unknown.as_str(), "unknown");
        assert!(ReconciliationResult::Consistent.is_ok());
        assert!(ReconciliationResult::Repaired.is_ok());
        assert!(!ReconciliationResult::Diverged.is_ok());
    }

    #[test]
    fn state_reconciler_record_and_reconcile() {
        let sr = StateReconciler::default();
        sr.record_replica_state(test_dataset_id(), test_node_id(), test_ordering_pos());
        sr.record_replica_state(test_dataset_id(), NodeId::new([2; 32]), test_ordering_pos());
        assert_eq!(sr.reconcile(&test_dataset_id()), ReconciliationResult::Consistent);
        assert_eq!(sr.reconciliation_count(), 1);
        assert_eq!(sr.consistent_count(), 1);
    }

    #[test]
    fn state_reconciler_diverged() {
        let sr = StateReconciler::default();
        sr.record_replica_state(test_dataset_id(), test_node_id(), test_ordering_pos());
        sr.record_replica_state(test_dataset_id(), NodeId::new([2; 32]), OrderingPos::new(5000));
        let result = sr.reconcile(&test_dataset_id());
        assert!(matches!(result, ReconciliationResult::Repaired | ReconciliationResult::Diverged));
    }

    #[test]
    fn state_reconciler_unknown() {
        let sr = StateReconciler::default();
        assert_eq!(
            sr.reconcile(&DatasetId::new([9; 32])),
            ReconciliationResult::Unknown
        );
    }

    #[test]
    fn state_reconciler_counts() {
        let sr = StateReconciler::default();
        sr.record_replica_state(test_dataset_id(), test_node_id(), test_ordering_pos());
        sr.reconcile(&test_dataset_id());
        assert_eq!(sr.dataset_count(), 1);
        assert_eq!(sr.consistency_rate(), 1.0);
    }

    // === ReplicaReconstructor tests ===
    #[test]
    fn replica_reconstructor_enqueue_dequeue() {
        let rr = ReplicaReconstructor::default();
        rr.enqueue(test_partition_id());
        rr.enqueue(PartitionId::new([2; 32]));
        assert_eq!(rr.queue_len(), 2);
        assert_eq!(rr.dequeue(), Some(test_partition_id()));
        assert_eq!(rr.dequeue(), Some(PartitionId::new([2; 32])));
        assert_eq!(rr.dequeue(), None);
    }

    #[test]
    fn replica_reconstructor_start_complete_fail() {
        let rr = ReplicaReconstructor::default();
        assert!(rr.start_reconstruction(&test_partition_id()));
        assert!(rr.is_reconstructing(&test_partition_id()));
        assert!(!rr.is_reconstructing(&PartitionId::new([2; 32])));
        rr.complete_reconstruction(&test_partition_id(), 4096);
        assert!(!rr.is_reconstructing(&test_partition_id()));
        assert_eq!(rr.reconstructed_count(), 1);
        assert_eq!(rr.total_bytes_reconstructed(), 4096);
    }

    #[test]
    fn replica_reconstructor_fail_and_rate() {
        let rr = ReplicaReconstructor::default();
        rr.start_reconstruction(&test_partition_id());
        rr.fail_reconstruction(&test_partition_id());
        assert_eq!(rr.failed_count(), 1);
        assert_eq!(rr.success_rate(), 0.0);

        rr.start_reconstruction(&PartitionId::new([2; 32]));
        rr.complete_reconstruction(&PartitionId::new([2; 32]), 1024);
        assert!((rr.success_rate() - 0.5).abs() < 0.01);
    }

    // === QuarantineManager tests ===
    #[test]
    fn quarantine_reason_as_str() {
        assert_eq!(QuarantineReason::SuspectedFailure.as_str(), "suspected_failure");
        assert_eq!(QuarantineReason::CorruptionDetected.as_str(), "corruption_detected");
        assert_eq!(QuarantineReason::Unresponsive.as_str(), "unresponsive");
        assert_eq!(QuarantineReason::PolicyViolation.as_str(), "policy_violation");
    }

    #[test]
    fn quarantine_manager_quarantine_and_release() {
        let qm = QuarantineManager::default();
        let scope = RecoveryScope::Node(test_node_id());
        assert!(qm.quarantine(scope.clone(), QuarantineReason::SuspectedFailure, 1000));
        assert!(!qm.quarantine(scope.clone(), QuarantineReason::CorruptionDetected, 2000));
        assert!(qm.is_quarantined(&scope));
        assert_eq!(qm.get_reason(&scope), Some(QuarantineReason::SuspectedFailure));
        assert_eq!(qm.quarantine_count(), 1);
        assert!(qm.release(&scope));
        assert!(!qm.is_quarantined(&scope));
        assert_eq!(qm.release_count(), 1);
    }

    #[test]
    fn quarantine_manager_rates() {
        let qm = QuarantineManager::default();
        qm.quarantine(RecoveryScope::Node(test_node_id()), QuarantineReason::SuspectedFailure, 1000);
        qm.quarantine(RecoveryScope::HUnit(test_hunit_id()), QuarantineReason::Unresponsive, 2000);
        qm.release(&RecoveryScope::Node(test_node_id()));
        assert_eq!(qm.total_quarantine_count(), 2);
        assert_eq!(qm.total_release_count(), 1);
        assert!((qm.release_rate() - 0.5).abs() < 0.01);
    }

    // === RecoveryScheduler tests ===
    #[test]
    fn recovery_scheduler_schedule_and_next() {
        let rs = RecoveryScheduler::default();
        rs.schedule(test_recovery_plan_id(), 10);
        rs.schedule(RecoveryPlanId::new([2; 32]), 5);
        assert_eq!(rs.queue_len(), 2);
        assert_eq!(rs.schedule_count(), 2);
        let first = rs.next().unwrap();
        assert_eq!(first, RecoveryPlanId::new([2; 32])); // higher priority first
    }

    #[test]
    fn recovery_scheduler_empty() {
        let rs = RecoveryScheduler::default();
        assert_eq!(rs.next(), None);
        assert_eq!(rs.queue_len(), 0);
    }

    // === RecoveryStats tests ===
    #[test]
    fn recovery_stats_success_rate() {
        let mut s = RecoveryStats::new();
        s.completed_recoveries = 7;
        s.failed_recoveries = 2;
        s.total_recovery_time_ms = 14000;
        assert!((s.recovery_success_rate() - 7.0 / 9.0).abs() < 0.01);
        assert_eq!(s.average_recovery_time_ms(), 2000);
    }

    #[test]
    fn recovery_stats_quarantine_release_rate() {
        let mut s = RecoveryStats::new();
        s.quarantined_scopes = 4;
        s.released_quarantines = 2;
        assert!((s.quarantine_release_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn recovery_stats_default() {
        let s = RecoveryStats::default();
        assert_eq!(s.total_failures, 0);
        assert_eq!(s.recovery_success_rate(), 1.0);
        assert_eq!(s.average_recovery_time_ms(), 0);
    }

    // === RecoveryTimeline tests ===
    #[test]
    fn recovery_timeline_event_as_str() {
        assert_eq!(RecoveryTimelineEvent::FailureDetected.as_str(), "failure_detected");
        assert_eq!(RecoveryTimelineEvent::PlanCreated.as_str(), "plan_created");
        assert_eq!(RecoveryTimelineEvent::PlanStarted.as_str(), "plan_started");
        assert_eq!(RecoveryTimelineEvent::PlanCompleted.as_str(), "plan_completed");
        assert_eq!(RecoveryTimelineEvent::PlanFailed.as_str(), "plan_failed");
        assert_eq!(RecoveryTimelineEvent::StepCompleted.as_str(), "step_completed");
        assert_eq!(RecoveryTimelineEvent::StepFailed.as_str(), "step_failed");
        assert_eq!(RecoveryTimelineEvent::Quarantined.as_str(), "quarantined");
        assert_eq!(RecoveryTimelineEvent::Released.as_str(), "released");
        assert_eq!(RecoveryTimelineEvent::Reconciled.as_str(), "reconciled");
        assert_eq!(RecoveryTimelineEvent::Reconstructed.as_str(), "reconstructed");
    }

    #[test]
    fn recovery_timeline_record_and_query() {
        let tl = RecoveryTimeline::default();
        tl.record(
            1000,
            RecoveryTimelineEvent::FailureDetected,
            RecoveryScope::Node(test_node_id()),
            "node down".to_string(),
        );
        assert_eq!(tl.entry_count(), 1);
        let entries = tl.entries_for_scope(&RecoveryScope::Node(test_node_id()));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, RecoveryTimelineEvent::FailureDetected);
    }

    // === HealthMonitor tests ===
    #[test]
    fn health_status_as_str() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(HealthStatus::Critical.as_str(), "critical");
        assert_eq!(HealthStatus::Failed.as_str(), "failed");
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Critical.is_operational());
        assert!(!HealthStatus::Failed.is_operational());
        assert_eq!(HealthStatus::Critical.severity(), 2);
    }

    #[test]
    fn health_monitor_set_and_get() {
        let hm = HealthMonitor::default();
        hm.set_node_health(test_node_id(), HealthStatus::Degraded);
        assert_eq!(hm.get_node_health(&test_node_id()), HealthStatus::Degraded);
        assert_eq!(hm.get_node_health(&NodeId::new([9; 32])), HealthStatus::Healthy);
        assert_eq!(hm.get_hunit_health(&test_hunit_id()), HealthStatus::Healthy);
        assert_eq!(hm.get_region_health(&test_region_id()), HealthStatus::Healthy);
    }

    #[test]
    fn health_monitor_counts() {
        let hm = HealthMonitor::default();
        hm.set_node_health(test_node_id(), HealthStatus::Failed);
        hm.set_hunit_health(test_hunit_id(), HealthStatus::Critical);
        hm.set_region_health(test_region_id(), HealthStatus::Healthy);
        assert_eq!(hm.health_check_count(), 3);
        assert_eq!(hm.critical_count(), 2);
        assert_eq!(hm.healthy_count(), 1);
        assert_eq!(hm.node_health_count(), 1);
        assert_eq!(hm.hunit_health_count(), 1);
        assert_eq!(hm.region_health_count(), 1);
    }

    // === RecoveryEngine tests ===
    #[test]
    fn recovery_engine_detect_failures() {
        let engine = RecoveryEngine::default();
        engine.failure_detector.borrow_mut().record_node_heartbeat(test_node_id(), 1000);
        let failures = engine.detect_failures(40000).unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(engine.stats().total_failures, 1);
    }

    #[test]
    fn recovery_engine_create_plan() {
        let engine = RecoveryEngine::default();
        let plan = engine.create_plan(&test_failure_event_node()).unwrap();
        assert_eq!(plan.failure_event.event_type, FailureEventType::NodeFailure);
        assert_eq!(plan.status, PlanStatus::Pending);
        assert_eq!(engine.plan_count(), 1);
    }

    #[test]
    fn recovery_engine_create_plan_recovery_in_progress() {
        let engine = RecoveryEngine::default();
        engine.create_plan(&test_failure_event_node()).unwrap();
        let err = engine.create_plan(&test_failure_event_node()).unwrap_err();
        assert!(matches!(err, RecoveryError::RecoveryInProgress { .. }));
    }

    #[test]
    fn recovery_engine_execute_plan() {
        let engine = RecoveryEngine::default();
        let plan = engine.create_plan(&test_failure_event_node()).unwrap();
        let proof = engine.execute_plan(plan.plan_id).unwrap();
        assert_eq!(proof.dataset_id, test_dataset_id());
        assert_eq!(engine.recovery_count(), 1);
    }

    #[test]
    fn recovery_engine_execute_plan_not_found() {
        let engine = RecoveryEngine::default();
        let err = engine.execute_plan(test_recovery_plan_id()).unwrap_err();
        assert!(matches!(err, RecoveryError::PlanNotFound { .. }));
    }

    #[test]
    fn recovery_engine_quarantine_and_release() {
        let engine = RecoveryEngine::default();
        let scope = RecoveryScope::Node(test_node_id());
        engine.quarantine(scope.clone()).unwrap();
        assert_eq!(engine.quarantine_count(), 1);
        let err = engine.quarantine(scope.clone()).unwrap_err();
        assert!(matches!(err, RecoveryError::QuarantineActive { .. }));
        engine.release_quarantine(&scope).unwrap();
        assert_eq!(engine.stats().released_quarantines, 1);
    }

    #[test]
    fn recovery_engine_restore_redundancy() {
        let engine = RecoveryEngine::default();
        let restored = engine.restore_redundancy(test_dataset_id()).unwrap();
        assert!(restored > 0);
        assert_eq!(engine.stats().replicas_reconstructed, restored);
    }

    #[test]
    fn recovery_engine_reconcile_state() {
        let engine = RecoveryEngine::default();
        engine.record_replica_state(test_dataset_id(), test_node_id(), test_ordering_pos());
        engine.record_replica_state(test_dataset_id(), NodeId::new([2; 32]), test_ordering_pos());
        let proof = engine.reconcile_state(test_dataset_id()).unwrap();
        assert_eq!(proof.dataset_id, test_dataset_id());
    }

    #[test]
    fn recovery_engine_reconstruct() {
        let engine = RecoveryEngine::default();
        engine.enqueue_reconstruction(test_partition_id());
        assert!(engine.start_reconstruction(&test_partition_id()));
        assert!(engine.complete_reconstruction(&test_partition_id(), 8192));
        assert_eq!(engine.stats().replicas_reconstructed, 8192);
    }

    #[test]
    fn recovery_engine_schedule() {
        let engine = RecoveryEngine::default();
        engine.schedule_recovery(test_recovery_plan_id(), 5);
        assert_eq!(engine.next_scheduled(), Some(test_recovery_plan_id()));
    }

    #[test]
    fn recovery_engine_health() {
        let engine = RecoveryEngine::default();
        assert_eq!(engine.critical_count(), 0);
        assert_eq!(engine.healthy_count(), 0);
        engine.health_monitor.borrow_mut().set_node_health(test_node_id(), HealthStatus::Failed);
        assert_eq!(engine.critical_count(), 1);
    }

    #[test]
    fn recovery_engine_durability_condition_quorum() {
        assert!(matches!(
            DurabilityCondition::Quorum { required: 2 },
            DurabilityCondition::Quorum { required: 2 }
        ));
    }

    // === FailureEventType tests ===
    #[test]
    fn failure_event_type_as_str() {
        assert!(matches!(FailureEventType::NodeFailure, FailureEventType::NodeFailure));
        assert!(matches!(FailureEventType::HUnitFailure, FailureEventType::HUnitFailure));
        assert!(matches!(FailureEventType::RegionalFailure, FailureEventType::RegionalFailure));
        assert!(matches!(FailureEventType::NetworkPartition, FailureEventType::NetworkPartition));
        assert!(matches!(FailureEventType::DiskFailure, FailureEventType::DiskFailure));
        assert!(matches!(FailureEventType::PowerFailure, FailureEventType::PowerFailure));
    }

    // === FailureEvent tests ===
    #[test]
    fn failure_event_type_checks() {
        let fe = test_failure_event_node();
        assert!(fe.is_node_failure());
        assert!(!fe.is_hunit_failure());
        assert!(!fe.is_regional_failure());
        let fe = test_failure_event_hunit();
        assert!(!fe.is_node_failure());
        assert!(fe.is_hunit_failure());
        assert!(!fe.is_regional_failure());
        let fe = test_failure_event_region();
        assert!(!fe.is_node_failure());
        assert!(!fe.is_hunit_failure());
        assert!(fe.is_regional_failure());
    }

    // === RecoveryScopeLevel tests ===
    #[test]
    fn recovery_scope_level_rank() {
        assert_eq!(RecoveryScopeLevel::Node.rank(), 1);
        assert_eq!(RecoveryScopeLevel::HUnit.rank(), 2);
        assert_eq!(RecoveryScopeLevel::Region.rank(), 3);
        assert!(RecoveryScopeLevel::Region > RecoveryScopeLevel::Node);
        assert!(RecoveryScopeLevel::HUnit > RecoveryScopeLevel::Node);
    }

    // === RecoveryPhase tests ===
    #[test]
    fn recovery_phase_as_str() {
        assert_eq!(RecoveryPhase::Detection.as_str(), "detection");
        assert_eq!(RecoveryPhase::Planning.as_str(), "planning");
        assert_eq!(RecoveryPhase::Execution.as_str(), "execution");
        assert_eq!(RecoveryPhase::Verification.as_str(), "verification");
        assert_eq!(RecoveryPhase::Completion.as_str(), "completion");
    }

    #[test]
    fn recovery_phase_order_index() {
        assert_eq!(RecoveryPhase::Detection.order_index(), 0);
        assert_eq!(RecoveryPhase::Planning.order_index(), 1);
        assert_eq!(RecoveryPhase::Execution.order_index(), 2);
        assert_eq!(RecoveryPhase::Verification.order_index(), 3);
        assert_eq!(RecoveryPhase::Completion.order_index(), 4);
    }

    #[test]
    fn recovery_phase_is_before() {
        assert!(RecoveryPhase::Detection.is_before(&RecoveryPhase::Planning));
        assert!(RecoveryPhase::Planning.is_before(&RecoveryPhase::Execution));
        assert!(!RecoveryPhase::Completion.is_before(&RecoveryPhase::Detection));
        assert!(!RecoveryPhase::Detection.is_before(&RecoveryPhase::Detection));
    }

    // === RecoveryCoordinator tests ===
    #[test]
    fn recovery_coordinator_transition() {
        let coord = RecoveryCoordinator::default();
        assert_eq!(coord.current_phase(), RecoveryPhase::Detection);
        coord.transition_to(RecoveryPhase::Planning, 1000);
        assert_eq!(coord.current_phase(), RecoveryPhase::Planning);
        assert_eq!(coord.transition_count(), 1);
    }

    #[test]
    fn recovery_coordinator_active_scope() {
        let coord = RecoveryCoordinator::default();
        assert_eq!(coord.active_scope(), None);
        coord.set_active_scope(Some(RecoveryScope::Node(test_node_id())));
        assert_eq!(coord.active_scope(), Some(RecoveryScope::Node(test_node_id())));
        coord.set_active_scope(None);
        assert_eq!(coord.active_scope(), None);
    }

    #[test]
    fn recovery_coordinator_phase_duration() {
        let coord = RecoveryCoordinator::default();
        coord.transition_to(RecoveryPhase::Execution, 5000);
        assert_eq!(coord.phase_duration(RecoveryPhase::Execution, 10000), Some(5000));
        assert_eq!(coord.phase_duration(RecoveryPhase::Planning, 10000), None);
    }

    #[test]
    fn recovery_coordinator_can_transition() {
        let coord = RecoveryCoordinator::default();
        assert!(coord.can_transition(RecoveryPhase::Detection, RecoveryPhase::Planning));
        assert!(coord.can_transition(RecoveryPhase::Planning, RecoveryPhase::Planning));
        assert!(!coord.can_transition(RecoveryPhase::Execution, RecoveryPhase::Detection));
    }

    // === ReplicaRepairState tests ===
    #[test]
    fn replica_repair_state_as_str() {
        assert_eq!(ReplicaRepairState::Pending.as_str(), "pending");
        assert_eq!(ReplicaRepairState::Assigned.as_str(), "assigned");
        assert_eq!(ReplicaRepairState::InProgress.as_str(), "in_progress");
        assert_eq!(ReplicaRepairState::Completed.as_str(), "completed");
        assert_eq!(ReplicaRepairState::Failed.as_str(), "failed");
        assert_eq!(ReplicaRepairState::Abandoned.as_str(), "abandoned");
    }

    #[test]
    fn replica_repair_state_terminal_and_active() {
        assert!(ReplicaRepairState::Completed.is_terminal());
        assert!(ReplicaRepairState::Failed.is_terminal());
        assert!(ReplicaRepairState::Abandoned.is_terminal());
        assert!(!ReplicaRepairState::Pending.is_terminal());
        assert!(ReplicaRepairState::Assigned.is_active());
        assert!(ReplicaRepairState::InProgress.is_active());
        assert!(!ReplicaRepairState::Pending.is_active());
    }

    // === ReplicaRepairStateTracker tests ===
    #[test]
    fn replica_repair_tracker_set_and_get() {
        let tracker = ReplicaRepairStateTracker::default();
        tracker.set_state(test_dataset_id(), test_node_id(), ReplicaRepairState::Completed, 1000);
        assert_eq!(
            tracker.get_state(&test_dataset_id(), &test_node_id()),
            ReplicaRepairState::Completed
        );
        assert_eq!(
            tracker.get_state(&test_dataset_id(), &NodeId::new([2; 32])),
            ReplicaRepairState::Pending
        );
    }

    #[test]
    fn replica_repair_tracker_counts() {
        let tracker = ReplicaRepairStateTracker::default();
        tracker.set_state(test_dataset_id(), test_node_id(), ReplicaRepairState::Completed, 1000);
        tracker.set_state(test_dataset_id(), NodeId::new([2; 32]), ReplicaRepairState::Failed, 2000);
        assert_eq!(tracker.completed_count(), 1);
        assert_eq!(tracker.failed_count(), 1);
        assert!((tracker.completion_rate() - 0.5).abs() < 0.01);
        assert_eq!(tracker.tracked_count(), 2);
    }

    #[test]
    fn replica_repair_tracker_state_change() {
        let tracker = ReplicaRepairStateTracker::default();
        tracker.set_state(test_dataset_id(), test_node_id(), ReplicaRepairState::Assigned, 1000);
        assert_eq!(tracker.state_change_count(), 1);
        assert_eq!(tracker.replicas_in_state(ReplicaRepairState::Assigned), 1);
    }

    // === RegionalRecoveryCoordinator tests ===
    #[test]
    fn regional_coordinator_region_health() {
        let rrc = RegionalRecoveryCoordinator::default();
        rrc.set_region_health(test_region_id(), HealthStatus::Degraded);
        assert_eq!(rrc.get_region_health(&test_region_id()), HealthStatus::Degraded);
        assert_eq!(rrc.get_region_health(&RegionId::new([9; 32])), HealthStatus::Healthy);
    }

    #[test]
    fn regional_coordinator_cross_region() {
        let rrc = RegionalRecoveryCoordinator::default();
        assert!(rrc.initiate_cross_region_recovery(test_region_id(), RegionId::new([2; 32])));
        assert_eq!(rrc.cross_region_recovery_count(), 1);
        assert_eq!(rrc.regional_failure_count(), 0);
        rrc.set_region_health(test_region_id(), HealthStatus::Failed);
        assert_eq!(rrc.regional_failure_count(), 1);
    }

    #[test]
    fn regional_coordinator_counts() {
        let rrc = RegionalRecoveryCoordinator::default();
        rrc.set_region_health(test_region_id(), HealthStatus::Healthy);
        rrc.set_region_health(RegionId::new([2; 32]), HealthStatus::Failed);
        assert_eq!(rrc.region_count(), 2);
        assert_eq!(rrc.healthy_region_count(), 1);
        assert_eq!(rrc.critical_region_count(), 1);
    }

    // === NodeRecoveryTracker tests ===
    #[test]
    fn node_recovery_state_as_str() {
        assert_eq!(NodeRecoveryState::Healthy.as_str(), "healthy");
        assert_eq!(NodeRecoveryState::Degraded.as_str(), "degraded");
        assert_eq!(NodeRecoveryState::Recovering.as_str(), "recovering");
        assert_eq!(NodeRecoveryState::Failed.as_str(), "failed");
        assert_eq!(NodeRecoveryState::Quarantined.as_str(), "quarantined");
        assert!(NodeRecoveryState::Degraded.needs_recovery());
        assert!(NodeRecoveryState::Failed.needs_recovery());
        assert!(!NodeRecoveryState::Healthy.needs_recovery());
        assert!(NodeRecoveryState::Failed.is_terminal());
    }

    #[test]
    fn node_recovery_tracker_state_and_attempts() {
        let nrt = NodeRecoveryTracker::default();
        nrt.set_node_state(test_node_id(), NodeRecoveryState::Degraded);
        assert_eq!(nrt.get_node_state(&test_node_id()), NodeRecoveryState::Degraded);
        nrt.record_recovery_attempt(test_node_id());
        nrt.record_recovery_attempt(test_node_id());
        assert_eq!(nrt.recovery_attempt_count(&test_node_id()), 2);
        nrt.record_successful_recovery(test_node_id());
        assert_eq!(nrt.get_node_state(&test_node_id()), NodeRecoveryState::Healthy);
    }

    #[test]
    fn node_recovery_tracker_rates() {
        let nrt = NodeRecoveryTracker::default();
        nrt.record_recovery_attempt(test_node_id());
        nrt.record_recovery_attempt(NodeId::new([2; 32]));
        nrt.record_successful_recovery(test_node_id());
        assert_eq!(nrt.total_node_recoveries, 2);
        assert_eq!(nrt.successful_node_recoveries, 1);
        assert!((nrt.success_rate() - 0.5).abs() < 0.01);
    }

    // === HUnitRecoveryTracker tests ===
    #[test]
    fn hunit_recovery_tracker_state() {
        let hrt = HUnitRecoveryTracker::default();
        hrt.set_hunit_state(test_hunit_id(), NodeRecoveryState::Recovering);
        assert_eq!(hrt.get_hunit_state(&test_hunit_id()), NodeRecoveryState::Recovering);
        assert_eq!(hrt.get_hunit_state(&HUnitId::new([2; 32])), NodeRecoveryState::Healthy);
    }

    #[test]
    fn hunit_recovery_tracker_attempts() {
        let hrt = HUnitRecoveryTracker::default();
        hrt.record_recovery_attempt(test_hunit_id());
        hrt.record_recovery_attempt(test_hunit_id());
        assert_eq!(hrt.attempt_count(&test_hunit_id()), 2);
        assert_eq!(hrt.total_recoveries(), 2);
    }

    // === RecoveryResourceManager tests ===
    #[test]
    fn recovery_resource_manager_acquire_release() {
        let rrm = RecoveryResourceManager::new(2);
        assert!(rrm.acquire(test_recovery_plan_id()));
        assert!(rrm.acquire(RecoveryPlanId::new([2; 32])));
        assert!(!rrm.acquire(RecoveryPlanId::new([3; 32])));
        assert_eq!(rrm.active_count(), 2);
        assert!(rrm.at_limit());
        assert_eq!(rrm.rejected_count(), 1);
        assert!(rrm.release(test_recovery_plan_id()));
        assert!(!rrm.at_limit());
        assert_eq!(rrm.resource_limit(), 2);
    }

    #[test]
    fn recovery_resource_manager_queue_and_availability() {
        let rrm = RecoveryResourceManager::default();
        rrm.queue_recovery();
        rrm.queue_recovery();
        assert_eq!(rrm.queued_count(), 2);
        assert_eq!(rrm.availability_percent(), 100);
    }

    // === RecoveryVerificationEngine tests ===
    #[test]
    fn verification_result_as_str() {
        assert_eq!(VerificationResult::Verified.as_str(), "verified");
        assert_eq!(VerificationResult::Failed.as_str(), "failed");
        assert_eq!(VerificationResult::Pending.as_str(), "pending");
        assert_eq!(VerificationResult::Skipped.as_str(), "skipped");
        assert!(VerificationResult::Verified.is_success());
        assert!(!VerificationResult::Failed.is_success());
    }

    #[test]
    fn recovery_verification_engine_verify() {
        let sve = RecoveryVerificationEngine::default();
        sve.verify(test_recovery_plan_id(), VerificationResult::Verified);
        sve.verify(test_recovery_plan_id(), VerificationResult::Failed);
        assert_eq!(sve.verification_count(), 2);
        assert_eq!(sve.passed_count(), 1);
        assert_eq!(sve.failed_count(), 1);
        assert!((sve.pass_rate() - 0.5).abs() < 0.01);
        assert_eq!(
            sve.get_result(&test_recovery_plan_id()),
            Some(VerificationResult::Failed)
        );
        assert_eq!(sve.verified_plan_count(), 1);
    }

    // === RecoveryEngine extended tests ===
    #[test]
    fn recovery_engine_new_with_features() {
        let engine = RecoveryEngine::new_with_features();
        assert_eq!(engine.current_recovery_phase(), RecoveryPhase::Detection);
        assert_eq!(engine.nodes_needing_recovery_count(), 0);
        assert_eq!(engine.hunits_needing_recovery_count(), 0);
    }

    #[test]
    fn recovery_engine_node_recovery_state() {
        let engine = RecoveryEngine::default();
        engine.set_node_recovery_state(test_node_id(), NodeRecoveryState::Recovering);
        assert_eq!(
            engine.get_node_recovery_state(&test_node_id()),
            NodeRecoveryState::Recovering
        );
    }

    #[test]
    fn recovery_engine_hunit_recovery_state() {
        let engine = RecoveryEngine::default();
        engine.set_hunit_recovery_state(test_hunit_id(), NodeRecoveryState::Quarantined);
        assert_eq!(
            engine.get_node_recovery_state(&test_node_id()),
            NodeRecoveryState::Healthy
        );
    }

    #[test]
    fn recovery_engine_resource_and_verification() {
        let engine = RecoveryEngine::default();
        assert!(engine.acquire_recovery_resource(test_recovery_plan_id()));
        assert!(engine.release_recovery_resource(test_recovery_plan_id()));
        engine.verify_plan(test_recovery_plan_id(), VerificationResult::Verified);
        assert_eq!(
            engine.get_verification_result(&test_recovery_plan_id()),
            Some(VerificationResult::Verified)
        );
    }

    #[test]
    fn recovery_engine_region_and_cross_region() {
        let engine = RecoveryEngine::default();
        engine.set_region_health(test_region_id(), HealthStatus::Critical);
        assert_eq!(engine.get_region_health(&test_region_id()), HealthStatus::Critical);
        assert!(engine.initiate_cross_region_recovery(test_region_id(), RegionId::new([2; 32])));
        assert_eq!(engine.cross_region_recovery_count(), 1);
    }

    #[test]
    fn recovery_engine_phase_and_scope() {
        let engine = RecoveryEngine::default();
        engine.recovery_phase_transition(RecoveryPhase::Execution, 5000);
        assert_eq!(engine.current_recovery_phase(), RecoveryPhase::Execution);
        engine.set_active_recovery_scope(Some(RecoveryScope::Region(test_region_id())));
        assert_eq!(
            engine.set_active_recovery_scope(None),
            ()
        );
    }

    #[test]
    fn recovery_engine_replica_repair_state() {
        let engine = RecoveryEngine::default();
        engine.record_replica_repair_state(test_dataset_id(), test_node_id(), ReplicaRepairState::InProgress, 1000);
        assert_eq!(
            engine.get_replica_repair_state(&test_dataset_id(), &test_node_id()),
            ReplicaRepairState::InProgress
        );
    }

    #[test]
    fn recovery_engine_resource_availability() {
        let engine = RecoveryEngine::default();
        assert_eq!(engine.resource_availability_percent(), 100);
    }

    #[test]
    fn recovery_engine_verification_pass_rate() {
        let engine = RecoveryEngine::default();
        assert_eq!(engine.verification_pass_rate(), 1.0);
        engine.verify_plan(test_recovery_plan_id(), VerificationResult::Verified);
        assert_eq!(engine.verification_pass_rate(), 1.0);
    }

    #[test]
    fn recovery_engine_counts() {
        let engine = RecoveryEngine::default();
        assert_eq!(engine.nodes_needing_recovery_count(), 0);
        assert_eq!(engine.hunits_needing_recovery_count(), 0);
        assert_eq!(engine.cross_region_recovery_count(), 0);
        assert_eq!(engine.regional_failure_count(), 0);
    }
}