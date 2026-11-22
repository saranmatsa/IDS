//! # Sealed Trait: CRDT — Conflict-free Replicated Data Types
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The CRDT trait defines conflict-free replicated data type primitives:
//! - Operation-based CRDTs (op-based) with commutative operations
//! - State-based CRDTs (state-based) with merge semantics
//! - Delta-CRDTs (delta-based) with delta-mutation and delta-merge
//! - Grow-only counter (G-Counter)
//! - Grow-only set (G-Set)
//! - Two-phase set (2P-Set) with add and remove
//! - Last-writer-wins register (LWW-Register) with timestamps
//! - Observed-remove set (OR-Set) with add-wins semantics
//! - PN-Counter (positive-negative counter) with increment and decrement
//! - Deterministic convergence verification
//! - CRDT state machine (created, merging, converged, failed)
//! - CRDT stats and health monitoring
//! Implementations live downstream in ids-crdt and ids-* crates.

use crate::types::{CRDTDelta, CRDTState, CRDTOp, DatasetId, Epoch, NodeId, OrderingPos, OpRef, Version};
use crate::errors::CRDTError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: CRDT
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::CRTDEngine {}
impl private::Sealed for super::CRDTConfig {}
impl private::Sealed for super::CRDTStateType {}
impl private::Sealed for super::CRDTOperation {}
impl private::Sealed for super::CRDTReplica {}
impl private::Sealed for super::CRDTMergeResult {}
impl private::Sealed for super::CRDTConvergenceResult {}
impl private::Sealed for super::CRDTStats {}
impl private::Sealed for super::CRDTMonitor {}
impl private::Sealed for super::CRDTValidator {}

// Sealed trait for CRDT.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait CRDT: private::Sealed {
    /// Configure CRDT parameters.
    fn configure(&self, config: CRDTConfig) -> Result<(), CRDTError>;

    /// Create a new CRDT replica.
    fn create_replica(&self, replica_id: NodeId, crdt_type: CRDTType) -> Result<CRDTReplica, CRDTError>;

    /// Apply an operation-based CRDT operation.
    fn apply_op(&self, replica_id: NodeId, op: CRDTOp) -> Result<(), CRDTError>;

    /// Merge two state-based CRDT replicas.
    fn merge_states(&self, replica_a: NodeId, replica_b: NodeId) -> Result<CRDTMergeResult, CRDTError>;

    /// Apply a delta-CRDT delta.
    fn apply_delta(&self, replica_id: NodeId, delta: CRDTDelta) -> Result<(), CRDTError>;

    /// Merge delta-CRDT states.
    fn merge_deltas(&self, replica_a: NodeId, replica_b: NodeId) -> Result<CRDTMergeResult, CRDTError>;

    /// Check if replicas have converged.
    fn check_convergence(&self, replicas: &[NodeId]) -> Result<CRDTConvergenceResult, CRDTError>;

    /// Get CRDT state for a replica.
    fn get_state(&self, replica_id: NodeId) -> Result<CRDTState, CRDTError>;

    /// Get CRDT type for a replica.
    fn crdt_type(&self, replica_id: NodeId) -> Result<CRDTType, CRDTError>;

    /// Get CRDT stats.
    fn stats(&self) -> CRDTStats;

    /// Get CRDT health score for a replica.
    fn health_score(&self, replica_id: NodeId) -> Result<f64, CRDTError>;
}

///////////////////////////////////////////////////////////////////////////////
// CRDT CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for CRDT operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTConfig {
    /// Maximum number of operations per replica.
    pub max_ops_per_replica: u64,
    /// Maximum merge depth.
    pub max_merge_depth: u64,
    /// Convergence threshold (0.0 to 1.0).
    pub convergence_threshold: f64,
    /// Enable operation-based CRDTs.
    pub enable_op_based: bool,
    /// Enable state-based CRDTs.
    pub enable_state_based: bool,
    /// Enable delta-CRDTs.
    pub enable_delta_crdt: bool,
    /// Enable automatic convergence checking.
    pub enable_auto_convergence: bool,
    /// Maximum concurrent merges.
    pub max_concurrent_merges: u64,
    /// Merge timeout in milliseconds.
    pub merge_timeout_ms: u64,
}

impl Default for CRDTConfig {
    fn default() -> Self {
        Self {
            max_ops_per_replica: 100000,
            max_merge_depth: 100,
            convergence_threshold: 0.99,
            enable_op_based: true,
            enable_state_based: true,
            enable_delta_crdt: true,
            enable_auto_convergence: true,
            max_concurrent_merges: 10,
            merge_timeout_ms: 30000,
        }
    }
}

impl CRDTConfig {
    /// Create a new CRDTConfig with custom max ops.
    pub fn with_max_ops(mut self, max_ops: u64) -> Self {
        self.max_ops_per_replica = max_ops;
        self
    }

    /// Create a new CRDTConfig with custom convergence threshold.
    pub fn with_convergence_threshold(mut self, threshold: f64) -> Self {
        self.convergence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Create a new CRDTConfig with custom merge depth.
    pub fn with_max_merge_depth(mut self, depth: u64) -> Self {
        self.max_merge_depth = depth;
        self
    }

    /// Create a new CRDTConfig enabling/disabling op-based CRDTs.
    pub fn with_op_based(mut self, enabled: bool) -> Self {
        self.enable_op_based = enabled;
        self
    }

    /// Create a new CRDTConfig enabling/disabling state-based CRDTs.
    pub fn with_state_based(mut self, enabled: bool) -> Self {
        self.enable_state_based = enabled;
        self
    }

    /// Create a new CRDTConfig enabling/disabling delta-CRDTs.
    pub fn with_delta_crdt(mut self, enabled: bool) -> Self {
        self.enable_delta_crdt = enabled;
        self
    }

    /// Create a new CRDTConfig enabling/disabling auto convergence.
    pub fn with_auto_convergence(mut self, enabled: bool) -> Self {
        self.enable_auto_convergence = enabled;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), CRDTError> {
        if self.max_ops_per_replica == 0 {
            return Err(CRDTError::InvalidConfig {
                field: "max_ops_per_replica".to_string(),
                reason: "max ops per replica must be > 0".to_string(),
            });
        }
        if self.max_merge_depth == 0 {
            return Err(CRDTError::InvalidConfig {
                field: "max_merge_depth".to_string(),
                reason: "max merge depth must be > 0".to_string(),
            });
        }
        if self.convergence_threshold <= 0.0 || self.convergence_threshold > 1.0 {
            return Err(CRDTError::InvalidConfig {
                field: "convergence_threshold".to_string(),
                reason: "convergence threshold must be in (0, 1]".to_string(),
            });
        }
        if self.max_concurrent_merges == 0 {
            return Err(CRDTError::InvalidConfig {
                field: "max_concurrent_merges".to_string(),
                reason: "max concurrent merges must be > 0".to_string(),
            });
        }
        if self.merge_timeout_ms == 0 {
            return Err(CRDTError::InvalidConfig {
                field: "merge_timeout_ms".to_string(),
                reason: "merge timeout must be > 0".to_string(),
            });
        }
        if !self.enable_op_based && !self.enable_state_based && !self.enable_delta_crdt {
            return Err(CRDTError::InvalidConfig {
                field: "crdt_types".to_string(),
                reason: "at least one CRDT type must be enabled".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT STATE TYPE
///////////////////////////////////////////////////////////////////////////////

/// Type of CRDT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CRDTType {
    /// Operation-based CRDT.
    OpBased,
    /// State-based CRDT.
    StateBased,
    /// Delta-CRDT.
    DeltaBased,
    /// Grow-only counter.
    GCounter,
    /// Grow-only set.
    GSet,
    /// Two-phase set.
    TwoPhaseSet,
    /// Last-writer-wins register.
    LWWRegister,
    /// Observed-remove set.
    ORSet,
    /// Positive-negative counter.
    PNCounter,
}

impl Default for CRDTType {
    fn default() -> Self {
        CRDTType::StateBased
    }
}

impl CRDTType {
    /// Check if this is a counter type.
    pub fn is_counter(&self) -> bool {
        matches!(self, CRDTType::GCounter | CRDTType::PNCounter)
    }

    /// Check if this is a set type.
    pub fn is_set(&self) -> bool {
        matches!(self, CRDTType::GSet | CRDTType::TwoPhaseSet | CRDTType::ORSet)
    }

    /// Check if this is a register type.
    pub fn is_register(&self) -> bool {
        matches!(self, CRDTType::LWWRegister)
    }

    /// Get a display name for the type.
    pub fn display_name(&self) -> &'static str {
        match self {
            CRDTType::OpBased => "op_based",
            CRDTType::StateBased => "state_based",
            CRDTType::DeltaBased => "delta_based",
            CRDTType::GCounter => "g_counter",
            CRDTType::GSet => "g_set",
            CRDTType::TwoPhaseSet => "two_phase_set",
            CRDTType::LWWRegister => "lww_register",
            CRDTType::ORSet => "or_set",
            CRDTType::PNCounter => "pn_counter",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a CRDT replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CRDTStateType {
    /// Replica is created but not yet initialized.
    Created,
    /// Replica is actively merging.
    Merging,
    /// Replica has converged with all peers.
    Converged,
    /// Replica has failed.
    Failed,
}

impl Default for CRDTStateType {
    fn default() -> Self {
        CRDTStateType::Created
    }
}

impl CRDTStateType {
    /// Check if the state allows operations.
    pub fn is_active(&self) -> bool {
        matches!(self, CRDTStateType::Created | CRDTStateType::Merging | CRDTStateType::Converged)
    }

    /// Check if the state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, CRDTStateType::Failed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT OPERATION
///////////////////////////////////////////////////////////////////////////////

/// A CRDT operation with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTOperation {
    /// Operation identifier.
    pub op_id: OpRef,
    /// CRDT type this operation applies to.
    pub crdt_type: CRDTType,
    /// Operation payload.
    pub payload: Vec<u8>,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Originating replica.
    pub origin: NodeId,
    /// Version vector entry.
    pub version: Version,
}

impl CRDTOperation {
    /// Create a new CRDTOperation.
    pub fn new(op_id: OpRef, crdt_type: CRDTType, payload: Vec<u8>, timestamp_ms: u64, origin: NodeId, version: Version) -> Self {
        Self {
            op_id,
            crdt_type,
            payload,
            timestamp_ms,
            origin,
            version,
        }
    }

    /// Get operation size.
    pub fn size(&self) -> usize {
        self.payload.len()
    }

    /// Check if operation is recent.
    pub fn is_recent(&self, now_ms: u64, threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.timestamp_ms) <= threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT REPLICA
///////////////////////////////////////////////////////////////////////////////

/// A CRDT replica tracking state and operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTReplica {
    /// Replica identifier.
    pub replica_id: NodeId,
    /// CRDT type.
    pub crdt_type: CRDTType,
    /// Current state.
    pub state: CRDTStateType,
    /// CRDT state data.
    pub crdt_state: CRDTState,
    /// Version vector.
    pub version_vector: std::collections::HashMap<NodeId, Version>,
    /// Total operations applied.
    pub total_ops: u64,
    /// Total merges performed.
    pub total_merges: u64,
    /// Last operation timestamp.
    pub last_op_at_ms: Option<u64>,
    /// Last merge timestamp.
    pub last_merge_at_ms: Option<u64>,
    /// Convergence version.
    pub convergence_version: Version,
    /// Whether replica is converged.
    pub is_converged: bool,
}

impl CRDTReplica {
    /// Create a new CRDTReplica.
    pub fn new(replica_id: NodeId, crdt_type: CRDTType) -> Self {
        Self {
            replica_id,
            crdt_type,
            state: CRDTStateType::Created,
            crdt_state: CRDTState::empty(),
            version_vector: std::collections::HashMap::new(),
            total_ops: 0,
            total_merges: 0,
            last_op_at_ms: None,
            last_merge_at_ms: None,
            convergence_version: Version(0),
            is_converged: false,
        }
    }

    /// Apply an operation to the replica.
    pub fn apply_operation(&mut self, op: &CRDTOperation) -> Result<(), CRDTError> {
        if self.state == CRDTStateType::Failed {
            return Err(CRDTError::ReplicaFailed { replica_id: self.replica_id });
        }
        self.state = CRDTStateType::Merging;
        self.crdt_state.inner.extend_from_slice(&op.payload);
        self.total_ops = self.total_ops.saturating_add(1);
        self.last_op_at_ms = Some(op.timestamp_ms);
        self.version_vector.insert(op.origin, op.version);
        self.state = CRDTStateType::Converged;
        Ok(())
    }

    /// Merge with another replica's state.
    pub fn merge_with(&mut self, other: &CRDTReplica) -> Result<CRDTMergeResult, CRDTError> {
        if self.state == CRDTStateType::Failed || other.state == CRDTStateType::Failed {
            return Err(CRDTError::ReplicaFailed { replica_id: self.replica_id });
        }
        self.state = CRDTStateType::Merging;
        let other_state = &other.crdt_state;
        let merged_len = self.crdt_state.inner.len() + other_state.inner.len();
        self.crdt_state.inner.extend_from_slice(&other_state.inner);
        self.total_merges = self.total_merges.saturating_add(1);
        self.last_merge_at_ms = crate::types::IdsTime::now_ms();
        for (node_id, version) in &other.version_vector {
            self.version_vector
                .entry(*node_id)
                .and_modify(|v| *v = std::cmp::max(*v, *version))
                .or_insert(*version);
        }
        self.state = CRDTStateType::Converged;
        self.is_converged = true;
        Ok(CRDTMergeResult::new(self.replica_id, other.replica_id, merged_len, true))
    }

    /// Check if replica is converged.
    pub fn is_converged(&self) -> bool {
        self.is_converged && self.state == CRDTStateType::Converged
    }

    /// Get operation rate.
    pub fn op_rate(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.total_ops as f64 / self.total_merges.max(1) as f64
    }

    /// Check if replica needs convergence.
    pub fn needs_convergence(&self) -> bool {
        !self.is_converged && self.state.is_active()
    }

    /// Get state size in bytes.
    pub fn state_size(&self) -> usize {
        self.crdt_state.inner.len()
    }

    /// Get version count.
    pub fn version_count(&self) -> usize {
        self.version_vector.len()
    }

    /// Check if replica is idle (no recent operations).
    pub fn is_idle(&self, now_ms: u64, idle_threshold_ms: u64) -> bool {
        match self.last_op_at_ms {
            Some(t) => now_ms.saturating_sub(t) > idle_threshold_ms,
            None => true,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT MERGE RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of a CRDT merge operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTMergeResult {
    /// Replica A that was merged.
    pub replica_a: NodeId,
    /// Replica B that was merged.
    pub replica_b: NodeId,
    /// Merged state size in bytes.
    pub merged_state_size: usize,
    /// Whether merge was successful.
    pub success: bool,
    /// Merge timestamp.
    pub merged_at_ms: u64,
    /// Number of conflicts resolved.
    pub conflicts_resolved: u64,
}

impl CRDTMergeResult {
    /// Create a new CRDTMergeResult.
    pub fn new(replica_a: NodeId, replica_b: NodeId, merged_state_size: usize, success: bool) -> Self {
        Self {
            replica_a,
            replica_b,
            merged_state_size,
            success,
            merged_at_ms: crate::types::IdsTime::now_ms(),
            conflicts_resolved: 0,
        }
    }

    /// Check if merge was successful.
    pub fn is_successful(&self) -> bool {
        self.success
    }

    /// Get merge duration estimate.
    pub fn estimated_duration_ms(&self) -> u64 {
        self.merged_state_size / 1024
    }

    /// Check if merge resolved conflicts.
    pub fn resolved_conflicts(&self) -> bool {
        self.conflicts_resolved > 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT CONVERGENCE RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of a CRDT convergence check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTConvergenceResult {
    /// Total replicas checked.
    pub total_replicas: u64,
    /// Converged replicas.
    pub converged_replicas: u64,
    /// Non-converged replicas.
    pub non_converged_replicas: u64,
    /// Convergence ratio (0.0 to 1.0).
    pub convergence_ratio: f64,
    /// Whether all replicas have converged.
    pub all_converged: bool,
    /// Convergence timestamp.
    pub checked_at_ms: u64,
}

impl CRDTConvergenceResult {
    /// Create a new CRDTConvergenceResult.
    pub fn new(total_replicas: u64, converged_replicas: u64) -> Self {
        let non_converged = total_replicas.saturating_sub(converged_replicas);
        let ratio = if total_replicas == 0 { 1.0 } else { converged_replicas as f64 / total_replicas as f64 };
        Self {
            total_replicas,
            converged_replicas,
            non_converged_replicas: non_converged,
            convergence_ratio: ratio,
            all_converged: non_converged == 0,
            checked_at_ms: crate::types::IdsTime::now_ms(),
        }
    }

    /// Check if convergence threshold is met.
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.convergence_ratio >= threshold
    }

    /// Get non-converged replica count.
    pub fn non_converged_count(&self) -> u64 {
        self.non_converged_replicas
    }

    /// Check if convergence is perfect (100%).
    pub fn is_perfect(&self) -> bool {
        self.convergence_ratio >= 1.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT STATS
///////////////////////////////////////////////////////////////////////////////

/// Aggregate statistics for CRDT operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRDTStats {
    /// Total replicas created.
    pub total_replicas: u64,
    /// Total operations applied.
    pub total_ops: u64,
    /// Total merges performed.
    pub total_merges: u64,
    /// Total convergences achieved.
    pub total_convergences: u64,
    /// Total convergence checks.
    pub total_convergence_checks: u64,
    /// Total conflicts resolved.
    pub total_conflicts_resolved: u64,
    /// Total delta operations applied.
    pub total_delta_ops: u64,
    /// Total state merges performed.
    pub total_state_merges: u64,
    /// Total op-based operations applied.
    pub total_op_based_ops: u64,
}

impl Default for CRDTStats {
    fn default() -> Self {
        Self {
            total_replicas: 0,
            total_ops: 0,
            total_merges: 0,
            total_convergences: 0,
            total_convergence_checks: 0,
            total_conflicts_resolved: 0,
            total_delta_ops: 0,
            total_state_merges: 0,
            total_op_based_ops: 0,
        }
    }
}

impl CRDTStats {
    /// Convergence rate (convergences / checks).
    pub fn convergence_rate(&self) -> f64 {
        if self.total_convergence_checks == 0 {
            return 1.0;
        }
        self.total_convergences as f64 / self.total_convergence_checks as f64
    }

    /// Merge rate (merges / ops).
    pub fn merge_rate(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.total_merges as f64 / self.total_ops as f64
    }

    /// Conflict resolution rate.
    pub fn conflict_resolution_rate(&self) -> f64 {
        if self.total_merges == 0 {
            return 0.0;
        }
        self.total_conflicts_resolved as f64 / self.total_merges as f64
    }

    /// Op-based ratio.
    pub fn op_based_ratio(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.total_op_based_ops as f64 / self.total_ops as f64
    }

    /// Delta ratio.
    pub fn delta_ratio(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.total_delta_ops as f64 / self.total_ops as f64
    }

    /// Get total operations across all types.
    pub fn total_all_ops(&self) -> u64 {
        self.total_op_based_ops + self.total_delta_ops + self.total_state_merges
    }

    /// Get non-convergence rate.
    pub fn non_convergence_rate(&self) -> f64 {
        1.0 - self.convergence_rate()
    }

    /// Get the ratio of active leases to total grants.
    pub fn active_ratio(&self) -> f64 {
        if self.total_replicas == 0 {
            return 1.0;
        }
        self.total_replicas as f64 / self.total_replicas.max(1) as f64
    }

    /// Get the overall health score (0.0 to 1.0).
    pub fn health_score(&self) -> f64 {
        let success = self.convergence_rate();
        let merge = self.merge_rate().min(1.0);
        let conflict_penalty = 1.0 - self.conflict_resolution_rate();
        (success + merge) / 2.0 - conflict_penalty / 2.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Monitors CRDT replica state and convergence.
#[derive(Debug, Clone)]
pub struct CRDTMonitor {
    /// Configuration.
    pub config: CRDTConfig,
    /// Replicas.
    pub replicas: std::cell::RefCell<std::collections::HashMap<NodeId, CRDTReplica>>,
    /// Operations applied.
    pub operations: std::cell::RefCell<Vec<CRDTOperation>>,
    /// Merge results.
    pub merge_results: std::cell::RefCell<Vec<CRDTMergeResult>>,
    /// Convergence results.
    pub convergence_results: std::cell::RefCell<Vec<CRDTConvergenceResult>>,
    /// Stats.
    pub stats: std::cell::RefCell<CRDTStats>,
    /// Next operation ID counter.
    pub next_op_id: std::cell::RefCell<u64>,
}

impl Default for CRDTMonitor {
    fn default() -> Self {
        Self::new(CRDTConfig::default())
    }
}

impl CRDTMonitor {
    /// Create a new CRDTMonitor.
    pub fn new(config: CRDTConfig) -> Self {
        Self {
            config,
            replicas: std::cell::RefCell::new(std::collections::HashMap::new()),
            operations: std::cell::RefCell::new(Vec::new()),
            merge_results: std::cell::RefCell::new(Vec::new()),
            convergence_results: std::cell::RefCell::new(Vec::new()),
            stats: std::cell::RefCell::new(CRDTStats::default()),
            next_op_id: std::cell::RefCell::new(1),
        }
    }

    /// Register a replica.
    pub fn register_replica(&self, replica: CRDTReplica) {
        let replica_id = replica.replica_id;
        self.replicas.borrow_mut().insert(replica_id, replica);
        let mut stats = self.stats.borrow_mut();
        stats.total_replicas += 1;
    }

    /// Get a replica.
    pub fn get_replica(&self, replica_id: NodeId) -> Option<CRDTReplica> {
        self.replicas.borrow().get(&replica_id).cloned()
    }

    /// Record an operation.
    pub fn record_operation(&self, op: CRDTOperation) {
        self.operations.borrow_mut().push(op);
        let mut stats = self.stats.borrow_mut();
        stats.total_ops += 1;
    }

    /// Record a merge result.
    pub fn record_merge_result(&self, result: CRDTMergeResult) {
        self.merge_results.borrow_mut().push(result);
        let mut stats = self.stats.borrow_mut();
        stats.total_merges += 1;
    }

    /// Record a convergence result.
    pub fn record_convergence_result(&self, result: CRDTConvergenceResult) {
        self.convergence_results.borrow_mut().push(result);
        let mut stats = self.stats.borrow_mut();
        stats.total_convergence_checks += 1;
        if result.all_converged {
            stats.total_convergences += 1;
        }
    }

    /// Get stats.
    pub fn stats(&self) -> CRDTStats {
        self.stats.borrow().clone()
    }

    /// Get all replica IDs.
    pub fn all_replica_ids(&self) -> Vec<NodeId> {
        self.replicas.borrow().keys().cloned().collect()
    }

    /// Get converged replica count.
    pub fn converged_replica_count(&self) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.is_converged())
            .count() as u64
    }

    /// Get failed replica count.
    pub fn failed_replica_count(&self) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.state == CRDTStateType::Failed)
            .count() as u64
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.replicas.borrow_mut().clear();
        self.operations.borrow_mut().clear();
        self.merge_results.borrow_mut().clear();
        self.convergence_results.borrow_mut().clear();
        *self.stats.borrow_mut() = CRDTStats::default();
        *self.next_op_id.borrow_mut() = 1;
    }

    /// Get all replicas.
    pub fn all_replicas(&self) -> Vec<CRDTReplica> {
        self.replicas.borrow().values().cloned().collect()
    }

    /// Get converged replicas.
    pub fn converged_replicas(&self) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.is_converged())
            .cloned()
            .collect()
    }

    /// Get failed replicas.
    pub fn failed_replicas(&self) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.state == CRDTStateType::Failed)
            .cloned()
            .collect()
    }

    /// Get all operations.
    pub fn all_operations(&self) -> Vec<CRDTOperation> {
        self.operations.borrow().clone()
    }

    /// Get all merge results.
    pub fn all_merge_results(&self) -> Vec<CRDTMergeResult> {
        self.merge_results.borrow().clone()
    }

    /// Get all convergence results.
    pub fn all_convergence_results(&self) -> Vec<CRDTConvergenceResult> {
        self.convergence_results.borrow().clone()
    }

    /// Get replica count by type.
    pub fn replica_count_by_type(&self, crdt_type: CRDTType) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type)
            .count() as u64
    }

    /// Get total operation count.
    pub fn total_operation_count(&self) -> u64 {
        self.operations.borrow().len() as u64
    }

    /// Get total merge count.
    pub fn total_merge_count(&self) -> u64 {
        self.merge_results.borrow().len() as u64
    }

    /// Get total convergence check count.
    pub fn total_convergence_check_count(&self) -> u64 {
        self.convergence_results.borrow().len() as u64
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates CRDT operations (convergence, merge validity, op ordering).
#[derive(Debug, Clone)]
pub struct CRDTValidator {
    /// Enable convergence validation.
    pub enable_convergence: bool,
    /// Enable merge validation.
    pub enable_merge_validation: bool,
    /// Enable op ordering validation.
    pub enable_op_ordering: bool,
}

impl Default for CRDTValidator {
    fn default() -> Self {
        Self {
            enable_convergence: true,
            enable_merge_validation: true,
            enable_op_ordering: true,
        }
    }
}

impl CRDTValidator {
    /// Create a new CRDTValidator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate convergence of replicas.
    pub fn validate_convergence(&self, replicas: &[CRDTReplica], threshold: f64) -> CRDTConvergenceResult {
        let total = replicas.len() as u64;
        let converged = replicas.iter().filter(|r| r.is_converged()).count() as u64;
        CRDTConvergenceResult::new(total, converged)
    }

    /// Validate a merge between two replicas.
    pub fn validate_merge(&self, replica_a: &CRDTReplica, replica_b: &CRDTReplica) -> bool {
        if !self.enable_merge_validation {
            return true;
        }
        replica_a.state.is_active() && replica_b.state.is_active()
    }

    /// Validate operation ordering.
    pub fn validate_op_ordering(&self, op_a: &CRDTOperation, op_b: &CRDTOperation) -> bool {
        if !self.enable_op_ordering {
            return true;
        }
        op_a.timestamp_ms <= op_b.timestamp_ms || op_a.version <= op_b.version
    }

    /// Check if convergence is enabled.
    pub fn is_convergence_enabled(&self) -> bool {
        self.enable_convergence
    }

    /// Check if merge validation is enabled.
    pub fn is_merge_validation_enabled(&self) -> bool {
        self.enable_merge_validation
    }

    /// Validate convergence for multiple replica batches.
    pub fn validate_convergence_batch(
        &self,
        replica_batches: &[Vec<CRDTReplica>],
        threshold: f64,
    ) -> Vec<CRDTConvergenceResult> {
        replica_batches
            .iter()
            .map(|batch| self.validate_convergence(batch, threshold))
            .collect()
    }

    /// Validate multiple merges.
    pub fn validate_merges(&self, merges: &[(CRDTReplica, CRDTReplica)]) -> Vec<bool> {
        merges
            .iter()
            .map(|(a, b)| self.validate_merge(a, b))
            .collect()
    }

    /// Validate operation ordering for multiple pairs.
    pub fn validate_op_ordering_batch(
        &self,
        op_pairs: &[(CRDTOperation, CRDTOperation)],
    ) -> Vec<bool> {
        op_pairs
            .iter()
            .map(|(a, b)| self.validate_op_ordering(a, b))
            .collect()
    }

    /// Toggle convergence validation.
    pub fn set_convergence_enabled(&mut self, enabled: bool) {
        self.enable_convergence = enabled;
    }

    /// Toggle merge validation.
    pub fn set_merge_validation_enabled(&mut self, enabled: bool) {
        self.enable_merge_validation = enabled;
    }

    /// Toggle op ordering validation.
    pub fn set_op_ordering_enabled(&mut self, enabled: bool) {
        self.enable_op_ordering = enabled;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT ENGINE
///////////////////////////////////////////////////////////////////////////////

/// The CRDT engine — core implementation of the CRDT trait.
#[derive(Debug, Clone)]
pub struct CRTDEngine {
    /// Configuration.
    pub config: std::cell::RefCell<CRDTConfig>,
    /// Monitor for replica tracking.
    pub monitor: CRDTMonitor,
    /// Validator for operations.
    pub validator: CRDTValidator,
}

impl Default for CRTDEngine {
    fn default() -> Self {
        Self::new(CRDTConfig::default())
    }
}

impl CRTDEngine {
    /// Create a new CRTDEngine with default config.
    pub fn new(config: CRDTConfig) -> Self {
        config.validate().expect("Invalid CRDTConfig");
        Self {
            config: std::cell::RefCell::new(config),
            monitor: CRDTMonitor::new(config.clone()),
            validator: CRDTValidator::new(),
        }
    }

    /// Configure the engine.
    pub fn configure(&self, config: CRDTConfig) -> Result<(), CRDTError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        self.monitor = CRDTMonitor::new(config);
        Ok(())
    }

    /// Create a new CRDT replica.
    pub fn create_replica(&self, replica_id: NodeId, crdt_type: CRDTType) -> Result<CRDTReplica, CRDTError> {
        let config = self.config.borrow();
        if config.max_ops_per_replica == 0 {
            drop(config);
            return Err(CRDTError::ReplicaLimitExceeded { replica_id });
        }
        drop(config);

        let mut replica = CRDTReplica::new(replica_id, crdt_type);
        replica.state = CRDTStateType::Created;
        self.monitor.register_replica(replica.clone());
        Ok(replica)
    }

    /// Apply an operation-based CRDT operation.
    pub fn apply_op(&self, replica_id: NodeId, op: CRDTOp) -> Result<(), CRDTError> {
        let mut replicas = self.monitor.replicas.borrow_mut();
        let replica = replicas.get_mut(&replica_id).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id })?;
        if !replica.state.is_active() {
            return Err(CRDTError::ReplicaNotActive { replica_id });
        }

        let crdt_op = CRDTOperation::new(
            OpRef::new([0u8; 9]),
            replica.crdt_type,
            op.inner.clone(),
            crate::types::IdsTime::now_ms(),
            replica_id,
            Version(replica.total_ops + 1),
        );

        let config = self.config.borrow();
        if !config.enable_op_based {
            drop(config);
            return Err(CRDTError::OpBasedDisabled);
        }
        drop(config);

        replica.apply_operation(&crdt_op)?;
        drop(replicas);

        self.monitor.record_operation(crdt_op);
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_op_based_ops += 1;

        Ok(())
    }

    /// Merge two state-based CRDT replicas.
    pub fn merge_states(&self, replica_a: NodeId, replica_b: NodeId) -> Result<CRDTMergeResult, CRDTError> {
        let mut replicas = self.monitor.replicas.borrow_mut();
        let rep_a = replicas.get(&replica_a).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_a })?;
        let rep_b = replicas.get(&replica_b).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_b })?;

        if !self.validator.validate_merge(rep_a, rep_b) {
            return Err(CRDTError::MergeInvalid { replica_a, replica_b });
        }

        let config = self.config.borrow();
        if !config.enable_state_based {
            drop(config);
            return Err(CRDTError::StateBasedDisabled);
        }
        drop(config);

        let merged_size = rep_a.crdt_state.inner.len() + rep_b.crdt_state.inner.len();
        let mut result = CRDTMergeResult::new(replica_a, replica_b, merged_size, true);

        let rep_a_mut = replicas.get_mut(&replica_a).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_a })?;
        rep_a_mut.merge_with(rep_b)?;

        drop(replicas);
        self.monitor.record_merge_result(result.clone());
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_state_merges += 1;
        stats.total_conflicts_resolved += result.conflicts_resolved;

        Ok(result)
    }

    /// Apply a delta-CRDT delta.
    pub fn apply_delta(&self, replica_id: NodeId, delta: CRDTDelta) -> Result<(), CRDTError> {
        let mut replicas = self.monitor.replicas.borrow_mut();
        let replica = replicas.get_mut(&replica_id).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id })?;
        if !replica.state.is_active() {
            return Err(CRDTError::ReplicaNotActive { replica_id });
        }

        let config = self.config.borrow();
        if !config.enable_delta_crdt {
            drop(config);
            return Err(CRDTError::DeltaDisabled);
        }
        drop(config);

        replica.crdt_state.inner.extend_from_slice(&delta.inner);
        replica.total_ops = replica.total_ops.saturating_add(1);
        replica.last_op_at_ms = Some(crate::types::IdsTime::now_ms());
        replica.state = CRDTStateType::Converged;
        drop(replicas);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_delta_ops += 1;

        Ok(())
    }

    /// Merge delta-CRDT states.
    pub fn merge_deltas(&self, replica_a: NodeId, replica_b: NodeId) -> Result<CRDTMergeResult, CRDTError> {
        let mut replicas = self.monitor.replicas.borrow_mut();
        let rep_a = replicas.get(&replica_a).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_a })?;
        let rep_b = replicas.get(&replica_b).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_b })?;

        if !self.validator.validate_merge(rep_a, rep_b) {
            return Err(CRDTError::MergeInvalid { replica_a, replica_b });
        }

        let merged_size = rep_a.crdt_state.inner.len() + rep_b.crdt_state.inner.len();
        let mut result = CRDTMergeResult::new(replica_a, replica_b, merged_size, true);

        let rep_a_mut = replicas.get_mut(&replica_a).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id: replica_a })?;
        rep_a_mut.merge_with(rep_b)?;

        drop(replicas);
        self.monitor.record_merge_result(result.clone());
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_state_merges += 1;

        Ok(result)
    }

    /// Check if replicas have converged.
    pub fn check_convergence(&self, replicas: &[NodeId]) -> Result<CRDTConvergenceResult, CRDTError> {
        let replica_states: Vec<CRDTReplica> = replicas
            .iter()
            .filter_map(|id| self.monitor.get_replica(*id))
            .collect();

        let config = self.config.borrow();
        let threshold = config.convergence_threshold;
        drop(config);

        let result = self.validator.validate_convergence(&replica_states, threshold);
        self.monitor.record_convergence_result(result.clone());

        Ok(result)
    }

    /// Get CRDT state for a replica.
    pub fn get_state(&self, replica_id: NodeId) -> Result<CRDTState, CRDTError> {
        let replicas = self.monitor.replicas.borrow();
        let replica = replicas.get(&replica_id).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id })?;
        Ok(replica.crdt_state.clone())
    }

    /// Get CRDT type for a replica.
    pub fn crdt_type(&self, replica_id: NodeId) -> Result<CRDTType, CRDTError> {
        let replicas = self.monitor.replicas.borrow();
        let replica = replicas.get(&replica_id).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id })?;
        Ok(replica.crdt_type)
    }

    /// Get stats.
    pub fn stats(&self) -> CRDTStats {
        self.monitor.stats()
    }

    /// Health score for a replica.
    pub fn health_score(&self, replica_id: NodeId) -> Result<f64, CRDTError> {
        let replicas = self.monitor.replicas.borrow();
        let replica = replicas.get(&replica_id).ok_or_else(|| CRDTError::ReplicaNotFound { replica_id })?;
        if replica.is_converged() {
            Ok(1.0)
        } else if replica.state.is_active() {
            Ok(0.5)
        } else {
            Ok(0.0)
        }
    }

    /// Get all replica IDs.
    pub fn all_replica_ids(&self) -> Vec<NodeId> {
        self.monitor.all_replica_ids()
    }

    /// Get converged replica count.
    pub fn converged_replica_count(&self) -> u64 {
        self.monitor.converged_replica_count()
    }

    /// Get failed replica count.
    pub fn failed_replica_count(&self) -> u64 {
        self.monitor.failed_replica_count()
    }

    /// Perform a full CRDT lifecycle: create replica, apply op, merge, check convergence.
    pub fn full_crdt_lifecycle(
        &self,
        replica_a_id: NodeId,
        replica_b_id: NodeId,
        crdt_type: CRDTType,
    ) -> Result<CRDTConvergenceResult, CRDTError> {
        let replica_a = self.create_replica(replica_a_id, crdt_type)?;
        let replica_b = self.create_replica(replica_b_id, crdt_type)?;
        let op_a = CRDTOp::new(vec![1, 2, 3]);
        let op_b = CRDTOp::new(vec![4, 5, 6]);
        self.apply_op(replica_a_id, op_a.clone())?;
        self.apply_op(replica_b_id, op_b.clone())?;
        self.merge_states(replica_a_id, replica_b_id)?;
        let result = self.check_convergence(&[replica_a_id, replica_b_id])?;
        Ok(result)
    }

    /// Create multiple replicas of the same type.
    pub fn create_replica_set(
        &self,
        replica_ids: &[NodeId],
        crdt_type: CRDTType,
    ) -> Result<Vec<CRDTReplica>, CRDTError> {
        let mut replicas = Vec::new();
        for &replica_id in replica_ids {
            let replica = self.create_replica(replica_id, crdt_type)?;
            replicas.push(replica);
        }
        Ok(replicas)
    }

    /// Apply operations to multiple replicas.
    pub fn apply_ops_to_replicas(
        &self,
        ops: &[(NodeId, CRDTOp)],
    ) -> Result<(), CRDTError> {
        for (replica_id, op) in ops {
            self.apply_op(*replica_id, op.clone())?;
        }
        Ok(())
    }

    /// Merge all replicas pairwise.
    pub fn merge_all_replicas(&self, replica_ids: &[NodeId]) -> Result<Vec<CRDTMergeResult>, CRDTError> {
        let mut results = Vec::new();
        for i in 0..replica_ids.len() {
            for j in (i + 1)..replica_ids.len() {
                let result = self.merge_states(replica_ids[i], replica_ids[j])?;
                results.push(result);
            }
        }
        Ok(results)
    }

    /// Check convergence for all replicas.
    pub fn check_all_convergence(&self) -> Result<CRDTConvergenceResult, CRDTError> {
        let all_ids = self.all_replica_ids();
        self.check_convergence(&all_ids)
    }

    /// Get replica states for all replicas.
    pub fn get_all_states(&self) -> Vec<(NodeId, CRDTState)> {
        let mut states = Vec::new();
        for replica_id in self.all_replica_ids() {
            if let Ok(state) = self.get_state(replica_id) {
                states.push((replica_id, state));
            }
        }
        states
    }

    /// Get health scores for all replicas.
    pub fn get_all_health_scores(&self) -> Vec<(NodeId, f64)> {
        let mut scores = Vec::new();
        for replica_id in self.all_replica_ids() {
            if let Ok(score) = self.health_score(replica_id) {
                scores.push((replica_id, score));
            }
        }
        scores
    }

    /// Get replicas by type.
    pub fn get_replicas_by_type(&self, crdt_type: CRDTType) -> Vec<CRDTReplica> {
        self.all_replicas()
            .into_iter()
            .filter(|r| r.crdt_type == crdt_type)
            .collect()
    }

    /// Get converged replicas by type.
    pub fn get_converged_by_type(&self, crdt_type: CRDTType) -> Vec<CRDTReplica> {
        self.converged_replicas()
            .into_iter()
            .filter(|r| r.crdt_type == crdt_type)
            .collect()
    }

    /// Get failed replicas by type.
    pub fn get_failed_by_type(&self, crdt_type: CRDTType) -> Vec<CRDTReplica> {
        self.failed_replicas()
            .into_iter()
            .filter(|r| r.crdt_type == crdt_type)
            .collect()
    }

    /// Get total state size across all replicas.
    pub fn total_state_size(&self) -> usize {
        self.all_replicas().iter().map(|r| r.state_size()).sum()
    }

    /// Get average state size.
    pub fn average_state_size(&self) -> f64 {
        let replicas = self.all_replicas();
        if replicas.is_empty() {
            return 0.0;
        }
        let total: usize = replicas.iter().map(|r| r.state_size()).sum();
        total as f64 / replicas.len() as f64
    }

    /// Get replica version vector sizes.
    pub fn version_vector_sizes(&self) -> Vec<(NodeId, usize)> {
        self.all_replicas()
            .iter()
            .map(|r| (r.replica_id, r.version_count()))
            .collect()
    }

    /// Check if all replicas are converged.
    pub fn all_replicas_converged(&self) -> bool {
        let replicas = self.all_replicas();
        if replicas.is_empty() {
            return true;
        }
        replicas.iter().all(|r| r.is_converged())
    }

    /// Check if any replica has failed.
    pub fn any_replica_failed(&self) -> bool {
        self.all_replicas()
            .iter()
            .any(|r| r.state == CRDTStateType::Failed)
    }

    /// Get idle replicas.
    pub fn get_idle_replicas(&self, now_ms: u64, idle_threshold_ms: u64) -> Vec<CRDTReplica> {
        self.all_replicas()
            .into_iter()
            .filter(|r| r.is_idle(now_ms, idle_threshold_ms))
            .collect()
    }

    /// Get replica op rate summary.
    pub fn op_rate_summary(&self) -> (f64, f64, f64) {
        let replicas = self.all_replicas();
        if replicas.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let rates: Vec<f64> = replicas.iter().map(|r| r.op_rate()).collect();
        let min = rates.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = rates.iter().sum::<f64>() / rates.len() as f64;
        (min, max, avg)
    }

    /// Reset all CRDT state.
    pub fn reset_all(&self) {
        self.clear_replicas();
        self.clear_operations();
        self.clear_merge_results();
        self.clear_convergence_results();
        self.reset_stats();
    }

    /// Get config as clone.
    pub fn config_clone(&self) -> CRDTConfig {
        self.config.borrow().clone()
    }

    /// Get config borrow.
    pub fn config_borrow(&self) -> std::cell::Ref<'_, CRDTConfig> {
        self.config.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDT ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CRDTError {
    #[error("replica not found: {replica_id:?}")]
    ReplicaNotFound { replica_id: NodeId },

    #[error("replica not active: {replica_id:?}")]
    ReplicaNotActive { replica_id: NodeId },

    #[error("replica failed: {replica_id:?}")]
    ReplicaFailed { replica_id: NodeId },

    #[error("merge invalid: {replica_a:?} vs {replica_b:?}")]
    MergeInvalid { replica_a: NodeId, replica_b: NodeId },

    #[error("replica limit exceeded: {replica_id:?}")]
    ReplicaLimitExceeded { replica_id: NodeId },

    #[error("op-based CRDT disabled")]
    OpBasedDisabled,

    #[error("state-based CRDT disabled")]
    StateBasedDisabled,

    #[error("delta CRDT disabled")]
    DeltaDisabled,

    #[error("invalid config: field={field}, reason={reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("internal CRDT error: {message}")]
    Internal { message: String },
}

impl CRDTError {
    pub fn category(&self) -> &'static str {
        match self {
            CRDTError::ReplicaNotFound { .. } => "replica_not_found",
            CRDTError::ReplicaNotActive { .. } => "replica_not_active",
            CRDTError::ReplicaFailed { .. } => "replica_failed",
            CRDTError::MergeInvalid { .. } => "merge_invalid",
            CRDTError::ReplicaLimitExceeded { .. } => "replica_limit_exceeded",
            CRDTError::OpBasedDisabled => "op_based_disabled",
            CRDTError::StateBasedDisabled => "state_based_disabled",
            CRDTError::DeltaDisabled => "delta_disabled",
            CRDTError::InvalidConfig { .. } => "invalid_config",
            CRDTError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CRDTError::ReplicaNotFound { .. }
                | CRDTError::ReplicaNotActive { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            CRDTError::ReplicaNotFound { .. } => "create the replica first",
            CRDTError::ReplicaNotActive { .. } => "replica is not in an active state",
            CRDTError::ReplicaFailed { .. } => "replica has failed, cannot operate",
            CRDTError::MergeInvalid { .. } => "check replica states before merging",
            CRDTError::ReplicaLimitExceeded { .. } => "replica limit reached",
            CRDTError::OpBasedDisabled => "enable op-based CRDTs in configuration",
            CRDTError::StateBasedDisabled => "enable state-based CRDTs in configuration",
            CRDTError::DeltaDisabled => "enable delta CRDTs in configuration",
            CRDTError::InvalidConfig { .. } => "fix configuration",
            CRDTError::Internal { .. } => "contact support",
        }
    }

    pub fn is_crdt_state_error(&self) -> bool {
        matches!(
            self,
            CRDTError::ReplicaNotFound { .. }
                | CRDTError::ReplicaNotActive { .. }
                | CRDTError::ReplicaFailed { .. }
        )
    }

    pub fn is_merge_error(&self) -> bool {
        matches!(self, CRDTError::MergeInvalid { .. })
    }

    pub fn is_type_disabled_error(&self) -> bool {
        matches!(
            self,
            CRDTError::OpBasedDisabled
                | CRDTError::StateBasedDisabled
                | CRDTError::DeltaDisabled
        )
    }

    pub fn severity(&self) -> &'static str {
        match self {
            CRDTError::ReplicaNotFound { .. } => "low",
            CRDTError::ReplicaNotActive { .. } => "medium",
            CRDTError::ReplicaFailed { .. } => "high",
            CRDTError::MergeInvalid { .. } => "medium",
            CRDTError::ReplicaLimitExceeded { .. } => "high",
            CRDTError::OpBasedDisabled => "medium",
            CRDTError::StateBasedDisabled => "medium",
            CRDTError::DeltaDisabled => "medium",
            CRDTError::InvalidConfig { .. } => "critical",
            CRDTError::Internal { .. } => "critical",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// GCounter SPECIFIC METHODS
///////////////////////////////////////////////////////////////////////////////

/// A grow-only counter CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GCounter {
    /// Per-replica counts.
    pub counts: std::collections::HashMap<NodeId, u64>,
}

impl GCounter {
    /// Create a new GCounter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment the counter for a replica.
    pub fn increment(&mut self, replica_id: NodeId, amount: u64) {
        *self.counts.entry(replica_id).or_insert(0) += amount;
    }

    /// Get the total count (sum of all replica counts).
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Merge with another GCounter (take max per replica).
    pub fn merge(&mut self, other: &GCounter) {
        for (replica_id, count) in &other.counts {
            self.counts
                .entry(*replica_id)
                .and_modify(|c| *c = std::cmp::max(*c, *count))
                .or_insert(*count);
        }
    }

    /// Get the count for a specific replica.
    pub fn get_count(&self, replica_id: NodeId) -> u64 {
        self.counts.get(&replica_id).copied().unwrap_or(0)
    }

    /// Get the number of replicas tracked.
    pub fn replica_count(&self) -> usize {
        self.counts.len()
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// GSet SPECIFIC METHODS
///////////////////////////////////////////////////////////////////////////////

/// A grow-only set CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GSet<T> {
    /// Elements in the set.
    pub elements: std::collections::HashSet<T>,
}

impl<T: std::hash::Hash + Eq + Clone> GSet<T> {
    /// Create a new GSet.
    pub fn new() -> Self {
        Self {
            elements: std::collections::HashSet::new(),
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T) {
        self.elements.insert(element);
    }

    /// Check if an element is in the set.
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Merge with another GSet (union).
    pub fn merge(&mut self, other: &GSet<T>) {
        self.elements.extend(other.elements.iter().cloned());
    }
}

impl<T: std::hash::Hash + Eq + Clone> Default for GSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PNCounter SPECIFIC METHODS
///////////////////////////////////////////////////////////////////////////////

/// A positive-negative counter CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNCounter {
    /// Positive counter (increments).
    pub positive: GCounter,
    /// Negative counter (decrements).
    pub negative: GCounter,
}

impl PNCounter {
    /// Create a new PNCounter.
    pub fn new() -> Self {
        Self {
            positive: GCounter::new(),
            negative: GCounter::new(),
        }
    }

    /// Increment the counter.
    pub fn increment(&mut self, replica_id: NodeId, amount: u64) {
        self.positive.increment(replica_id, amount);
    }

    /// Decrement the counter.
    pub fn decrement(&mut self, replica_id: NodeId, amount: u64) {
        self.negative.increment(replica_id, amount);
    }

    /// Get the net value (positive - negative).
    pub fn value(&self) -> i64 {
        self.positive.value() as i64 - self.negative.value() as i64
    }

    /// Merge with another PNCounter.
    pub fn merge(&mut self, other: &PNCounter) {
        self.positive.merge(&other.positive);
        self.negative.merge(&other.negative);
    }
}

impl Default for PNCounter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LWWRegister SPECIFIC METHODS
///////////////////////////////////////////////////////////////////////////////

/// A last-writer-wins register CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LWWRegister<T> {
    /// Current value.
    pub value: T,
    /// Timestamp of the last write.
    pub timestamp: u64,
    /// Replica that last wrote.
    pub writer: NodeId,
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new LWWRegister.
    pub fn new(value: T, timestamp: u64, writer: NodeId) -> Self {
        Self {
            value,
            timestamp,
            writer,
        }
    }

    /// Write a new value if timestamp is greater.
    pub fn write(&mut self, value: T, timestamp: u64, writer: NodeId) {
        if timestamp > self.timestamp {
            self.value = value;
            self.timestamp = timestamp;
            self.writer = writer;
        }
    }

    /// Merge with another LWWRegister (higher timestamp wins).
    pub fn merge(&mut self, other: &LWWRegister<T>) {
        if other.timestamp > self.timestamp {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.writer = other.writer;
        }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.value.clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORSet SPECIFIC METHODS
///////////////////////////////////////////////////////////////////////////////

/// An observed-remove set CRDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ORSet<T> {
    /// Elements with their unique tags.
    pub elements: std::collections::HashMap<T, Vec<u64>>,
    /// Removed tags (tombstones).
    pub removed: std::collections::HashSet<u64>,
    /// Next tag counter.
    pub next_tag: u64,
}

impl<T: std::hash::Hash + Eq + Clone> ORSet<T> {
    /// Create a new ORSet.
    pub fn new() -> Self {
        Self {
            elements: std::collections::HashMap::new(),
            removed: std::collections::HashSet::new(),
            next_tag: 1,
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T) {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.elements.entry(element).or_default().push(tag);
    }

    /// Remove an element from the set.
    pub fn remove(&mut self, element: &T) {
        if let Some(tags) = self.elements.get(element) {
            for tag in tags {
                self.removed.insert(*tag);
            }
        }
    }

    /// Check if an element is in the set (not removed).
    pub fn contains(&self, element: &T) -> bool {
        match self.elements.get(element) {
            Some(tags) => tags.iter().any(|tag| !self.removed.contains(tag)),
            None => false,
        }
    }

    /// Merge with another ORSet.
    pub fn merge(&mut self, other: &ORSet<T>) {
        for (element, tags) in &other.elements {
            self.elements
                .entry(element.clone())
                .or_default()
                .extend(tags.iter().cloned());
        }
        self.removed.extend(other.removed.iter().cloned());
        self.next_tag = std::cmp::max(self.next_tag, other.next_tag);
    }

    /// Get the number of elements (not removed).
    pub fn len(&self) -> usize {
        self.elements
            .iter()
            .filter(|(_, tags)| tags.iter().any(|tag| !self.removed.contains(tag)))
            .count()
    }
}

impl<T: std::hash::Hash + Eq + Clone> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRTDEngine ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRTDEngine {
    /// Get total replica count across all types.
    pub fn total_replica_count(&self) -> u64 {
        self.monitor.replicas.borrow().len() as u64
    }

    /// Get total delta operation count.
    pub fn total_delta_operation_count(&self) -> u64 {
        self.stats().total_delta_ops
    }

    /// Get total state merge count.
    pub fn total_state_merge_count(&self) -> u64 {
        self.stats().total_state_merges
    }

    /// Get total op-based operation count.
    pub fn total_op_based_operation_count(&self) -> u64 {
        self.stats().total_op_based_ops
    }

    /// Get replicas needing convergence.
    pub fn replicas_needing_convergence(&self) -> Vec<CRDTReplica> {
        self.all_replicas()
            .into_iter()
            .filter(|r| r.needs_convergence())
            .collect()
    }

    /// Count replicas needing convergence.
    pub fn count_replicas_needing_convergence(&self) -> u64 {
        self.replicas_needing_convergence().len() as u64
    }

    /// Get all replicas sorted by replica ID.
    pub fn all_replicas_sorted(&self) -> Vec<CRDTReplica> {
        let mut replicas = self.all_replicas();
        replicas.sort_by(|a, b| a.replica_id.as_bytes().cmp(b.replica_id.as_bytes()));
        replicas
    }

    /// Get merge results sorted by timestamp (newest first).
    pub fn merge_results_sorted(&self) -> Vec<CRDTMergeResult> {
        let mut results = self.all_merge_results();
        results.sort_by(|a, b| b.merged_at_ms.cmp(&a.merged_at_ms));
        results
    }

    /// Get convergence results sorted by timestamp (newest first).
    pub fn convergence_results_sorted(&self) -> Vec<CRDTConvergenceResult> {
        let mut results = self.all_convergence_results();
        results.sort_by(|a, b| b.checked_at_ms.cmp(&a.checked_at_ms));
        results
    }

    /// Get operations sorted by timestamp (newest first).
    pub fn operations_sorted(&self) -> Vec<CRDTOperation> {
        let mut ops = self.all_operations();
        ops.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));
        ops
    }

    /// Get the most recent merge result.
    pub fn most_recent_merge(&self) -> Option<CRDTMergeResult> {
        let results = self.all_merge_results();
        results.into_iter().max_by_key(|r| r.merged_at_ms)
    }

    /// Get the most recent convergence result.
    pub fn most_recent_convergence(&self) -> Option<CRDTConvergenceResult> {
        let results = self.all_convergence_results();
        results.into_iter().max_by_key(|r| r.checked_at_ms)
    }

    /// Get the most recent operation.
    pub fn most_recent_operation(&self) -> Option<CRDTOperation> {
        let ops = self.all_operations();
        ops.into_iter().max_by_key(|o| o.timestamp_ms)
    }

    /// Get convergence trend (improving, declining, stable).
    pub fn convergence_trend(&self) -> ConvergenceTrend {
        let results = self.all_convergence_results();
        if results.len() < 2 {
            return ConvergenceTrend::Stable;
        }
        let first = results.first().unwrap().convergence_ratio;
        let last = results.last().unwrap().convergence_ratio;
        if last > first + 0.05 {
            ConvergenceTrend::Improving
        } else if last < first - 0.05 {
            ConvergenceTrend::Declining
        } else {
            ConvergenceTrend::Stable
        }
    }

    /// Get merge success rate.
    pub fn merge_success_rate(&self) -> f64 {
        let results = self.all_merge_results();
        if results.is_empty() {
            return 1.0;
        }
        let successful = results.iter().filter(|r| r.is_successful()).count() as f64;
        successful / results.len() as f64
    }

    /// Get average merged state size.
    pub fn average_merged_state_size(&self) -> f64 {
        let results = self.all_merge_results();
        if results.is_empty() {
            return 0.0;
        }
        let total: usize = results.iter().map(|r| r.merged_state_size).sum();
        total as f64 / results.len() as f64
    }

    /// Check if any merge resolved conflicts.
    pub fn any_merge_resolved_conflicts(&self) -> bool {
        self.all_merge_results()
            .iter()
            .any(|r| r.resolved_conflicts())
    }

    /// Count merges that resolved conflicts.
    pub fn count_conflict_resolving_merges(&self) -> u64 {
        self.all_merge_results()
            .iter()
            .filter(|r| r.resolved_conflicts())
            .count() as u64
    }

    /// Validate the current configuration.
    pub fn validate_config(&self) -> Result<(), CRDTError> {
        self.config.borrow().validate()
    }

    /// Check if configuration is strict.
    pub fn config_is_strict(&self) -> bool {
        self.config.borrow().is_strict()
    }

    /// Check if configuration is lenient.
    pub fn config_is_lenient(&self) -> bool {
        self.config.borrow().is_lenient()
    }

    /// Get effective max ops per replica.
    pub fn effective_max_ops(&self) -> u64 {
        self.config.borrow().max_ops_per_replica
    }

    /// Get effective convergence threshold.
    pub fn effective_convergence_threshold(&self) -> f64 {
        self.config.borrow().convergence_threshold
    }

    /// Get effective max merge depth.
    pub fn effective_max_merge_depth(&self) -> u64 {
        self.config.borrow().max_merge_depth
    }

    /// Check if op-based CRDTs are enabled.
    pub fn is_op_based_enabled(&self) -> bool {
        self.config.borrow().enable_op_based
    }

    /// Check if state-based CRDTs are enabled.
    pub fn is_state_based_enabled(&self) -> bool {
        self.config.borrow().enable_state_based
    }

    /// Check if delta CRDTs are enabled.
    pub fn is_delta_crdt_enabled(&self) -> bool {
        self.config.borrow().enable_delta_crdt
    }

    /// Check if auto convergence is enabled.
    pub fn is_auto_convergence_enabled(&self) -> bool {
        self.config.borrow().enable_auto_convergence
    }

    /// Get max concurrent merges.
    pub fn max_concurrent_merges(&self) -> u64 {
        self.config.borrow().max_concurrent_merges
    }

    /// Get merge timeout in milliseconds.
    pub fn merge_timeout_ms(&self) -> u64 {
        self.config.borrow().merge_timeout_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTMonitor ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTMonitor {
    /// Get replicas by state.
    pub fn replicas_by_state(&self, state: CRDTStateType) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.state == state)
            .cloned()
            .collect()
    }

    /// Get replicas by type and state.
    pub fn replicas_by_type_and_state(&self, crdt_type: CRDTType, state: CRDTStateType) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type && r.state == state)
            .cloned()
            .collect()
    }

    /// Get converged replicas by type.
    pub fn converged_replicas_by_type(&self, crdt_type: CRDTType) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type && r.is_converged())
            .cloned()
            .collect()
    }

    /// Get failed replicas by type.
    pub fn failed_replicas_by_type(&self, crdt_type: CRDTType) -> Vec<CRDTReplica> {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type && r.state == CRDTStateType::Failed)
            .cloned()
            .collect()
    }

    /// Get total replica count by type.
    pub fn replica_count_by_type(&self, crdt_type: CRDTType) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type)
            .count() as u64
    }

    /// Get total converged replica count by type.
    pub fn converged_replica_count_by_type(&self, crdt_type: CRDTType) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type && r.is_converged())
            .count() as u64
    }

    /// Get total failed replica count by type.
    pub fn failed_replica_count_by_type(&self, crdt_type: CRDTType) -> u64 {
        self.replicas
            .borrow()
            .values()
            .filter(|r| r.crdt_type == crdt_type && r.state == CRDTStateType::Failed)
            .count() as u64
    }

    /// Get total state size across all replicas.
    pub fn total_state_size(&self) -> usize {
        self.replicas
            .borrow()
            .values()
            .map(|r| r.state_size())
            .sum()
    }

    /// Get average state size across all replicas.
    pub fn average_state_size(&self) -> f64 {
        let replicas = self.replicas.borrow();
        if replicas.is_empty() {
            return 0.0;
        }
        let total: usize = replicas.values().map(|r| r.state_size()).sum();
        total as f64 / replicas.len() as f64
    }

    /// Get total operation count across all replicas.
    pub fn total_operation_count_across_replicas(&self) -> u64 {
        self.replicas
            .borrow()
            .values()
            .map(|r| r.total_ops)
            .sum()
    }

    /// Get total merge count across all replicas.
    pub fn total_merge_count_across_replicas(&self) -> u64 {
        self.replicas
            .borrow()
            .values()
            .map(|r| r.total_merges)
            .sum()
    }

    /// Reset replica records only.
    pub fn reset_replicas(&mut self) {
        self.replicas.borrow_mut().clear();
    }

    /// Reset operation records only.
    pub fn reset_operations(&mut self) {
        self.operations.borrow_mut().clear();
    }

    /// Reset merge result records only.
    pub fn reset_merge_results(&mut self) {
        self.merge_results.borrow_mut().clear();
    }

    /// Reset convergence result records only.
    pub fn reset_convergence_results(&mut self) {
        self.convergence_results.borrow_mut().clear();
    }

    /// Reset stats only.
    pub fn reset_stats_only(&mut self) {
        *self.stats.borrow_mut() = CRDTStats::default();
    }

    /// Get monitor stats as clone.
    pub fn stats_clone(&self) -> CRDTStats {
        self.stats.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTValidator ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTValidator {
    /// Validate multiple replica batches.
    pub fn validate_convergence_batch(
        &self,
        replica_batches: &[Vec<CRDTReplica>],
        threshold: f64,
    ) -> Vec<CRDTConvergenceResult> {
        replica_batches
            .iter()
            .map(|batch| self.validate_convergence(batch, threshold))
            .collect()
    }

    /// Validate multiple merges.
    pub fn validate_merges(&self, merges: &[(CRDTReplica, CRDTReplica)]) -> Vec<bool> {
        merges
            .iter()
            .map(|(a, b)| self.validate_merge(a, b))
            .collect()
    }

    /// Validate operation ordering for multiple pairs.
    pub fn validate_op_ordering_batch(
        &self,
        op_pairs: &[(CRDTOperation, CRDTOperation)],
    ) -> Vec<bool> {
        op_pairs
            .iter()
            .map(|(a, b)| self.validate_op_ordering(a, b))
            .collect()
    }

    /// Toggle convergence validation.
    pub fn set_convergence_enabled(&mut self, enabled: bool) {
        self.enable_convergence = enabled;
    }

    /// Toggle merge validation.
    pub fn set_merge_validation_enabled(&mut self, enabled: bool) {
        self.enable_merge_validation = enabled;
    }

    /// Toggle op ordering validation.
    pub fn set_op_ordering_enabled(&mut self, enabled: bool) {
        self.enable_op_ordering = enabled;
    }

    /// Get current validation settings.
    pub fn validation_settings(&self) -> (bool, bool, bool) {
        (self.enable_convergence, self.enable_merge_validation, self.enable_op_ordering)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTConfig ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTConfig {
    /// Create a strict configuration.
    pub fn strict() -> Self {
        Self {
            max_ops_per_replica: 10000,
            max_merge_depth: 10,
            convergence_threshold: 0.95,
            enable_op_based: true,
            enable_state_based: true,
            enable_delta_crdt: true,
            enable_auto_convergence: true,
            max_concurrent_merges: 2,
            merge_timeout_ms: 10000,
        }
    }

    /// Create a lenient configuration.
    pub fn lenient() -> Self {
        Self {
            max_ops_per_replica: 1000000,
            max_merge_depth: 1000,
            convergence_threshold: 0.999,
            enable_op_based: true,
            enable_state_based: true,
            enable_delta_crdt: true,
            enable_auto_convergence: false,
            max_concurrent_merges: 100,
            merge_timeout_ms: 300000,
        }
    }

    /// Create a balanced configuration (default).
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Check if all CRDT types are enabled.
    pub fn all_types_enabled(&self) -> bool {
        self.enable_op_based && self.enable_state_based && self.enable_delta_crdt
    }

    /// Check if no CRDT types are enabled.
    pub fn no_types_enabled(&self) -> bool {
        !self.enable_op_based && !self.enable_state_based && !self.enable_delta_crdt
    }

    /// Get enabled type count.
    pub fn enabled_type_count(&self) -> u32 {
        let mut count = 0u32;
        if self.enable_op_based {
            count += 1;
        }
        if self.enable_state_based {
            count += 1;
        }
        if self.enable_delta_crdt {
            count += 1;
        }
        count
    }

    /// Get the range of allowed merge depths.
    pub fn merge_depth_range(&self) -> (u64, u64) {
        (1, self.max_merge_depth)
    }

    /// Check if a merge depth is valid.
    pub fn is_merge_depth_valid(&self, depth: u64) -> bool {
        depth >= 1 && depth <= self.max_merge_depth
    }

    /// Get the recommended convergence check interval.
    pub fn recommended_convergence_check_interval(&self) -> u64 {
        self.merge_timeout_ms / 2
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTType ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTType {
    /// Check if this type supports delta mutations.
    pub fn supports_delta(&self) -> bool {
        matches!(
            self,
            CRDTType::GCounter | CRDTType::GSet | CRDTType::PNCounter | CRDTType::ORSet
        )
    }

    /// Check if this type supports state-based merging.
    pub fn supports_state_merge(&self) -> bool {
        matches!(
            self,
            CRDTType::GCounter | CRDTType::GSet | CRDTType::TwoPhaseSet | CRDTType::LWWRegister | CRDTType::ORSet | CRDTType::PNCounter
        )
    }

    /// Check if this type supports operation-based application.
    pub fn supports_op_based(&self) -> bool {
        matches!(
            self,
            CRDTType::GCounter | CRDTType::GSet | CRDTType::TwoPhaseSet | CRDTType::LWWRegister | CRDTType::ORSet | CRDTType::PNCounter
        )
    }

    /// Get the default convergence threshold for this type.
    pub fn default_convergence_threshold(&self) -> f64 {
        match self {
            CRDTType::GCounter | CRDTType::PNCounter => 0.99,
            CRDTType::GSet | CRDTType::TwoPhaseSet | CRDTType::ORSet => 0.97,
            CRDTType::LWWRegister => 0.95,
            _ => 0.99,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTStateType ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTStateType {
    /// Get a display name for the state.
    pub fn display_name(&self) -> &'static str {
        match self {
            CRDTStateType::Created => "created",
            CRDTStateType::Merging => "merging",
            CRDTStateType::Converged => "converged",
            CRDTStateType::Failed => "failed",
        }
    }

    /// Check if the state requires intervention.
    pub fn requires_intervention(&self) -> bool {
        matches!(self, CRDTStateType::Failed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTOperation ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTOperation {
    /// Check if operation is from a specific origin.
    pub fn is_from_origin(&self, origin: NodeId) -> bool {
        self.origin == origin
    }

    /// Check if operation version is greater than another.
    pub fn version_greater_than(&self, other: &CRDTOperation) -> bool {
        self.version > other.version
    }

    /// Get operation age in milliseconds.
    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.timestamp_ms)
    }

    /// Check if operation is stale.
    pub fn is_stale(&self, now_ms: u64, stale_threshold_ms: u64) -> bool {
        self.age_ms(now_ms) > stale_threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTReplica ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTReplica {
    /// Get replica age in milliseconds.
    pub fn replica_age_ms(&self, now_ms: u64) -> u64 {
        match self.last_op_at_ms {
            Some(t) => now_ms.saturating_sub(t),
            None => 0,
        }
    }

    /// Check if replica needs convergence based on state.
    pub fn needs_convergence(&self) -> bool {
        !self.is_converged && self.state.is_active()
    }

    /// Get merge-to-op ratio.
    pub fn merge_to_op_ratio(&self) -> f64 {
        if self.total_ops == 0 {
            return 0.0;
        }
        self.total_merges as f64 / self.total_ops as f64
    }

    /// Check if replica has pending operations (not converged).
    pub fn has_pending_operations(&self) -> bool {
        self.total_ops > 0 && !self.is_converged
    }

    /// Get state size as percentage of max (1MB).
    pub fn state_size_percent(&self) -> f64 {
        let max_size = 1024 * 1024; // 1MB
        self.state_size() as f64 / max_size as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTMergeResult ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTMergeResult {
    /// Get merge efficiency (success / size).
    pub fn merge_efficiency(&self) -> f64 {
        if self.merged_state_size == 0 {
            return 0.0;
        }
        if self.success {
            1.0
        } else {
            0.0
        }
    }

    /// Get conflicts resolved per byte.
    pub fn conflicts_per_byte(&self) -> f64 {
        if self.merged_state_size == 0 {
            return 0.0;
        }
        self.conflicts_resolved as f64 / self.merged_state_size as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTConvergenceResult ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTConvergenceResult {
    /// Get non-converged replica count.
    pub fn non_converged_count(&self) -> u64 {
        self.non_converged_replicas
    }

    /// Check if convergence is perfect (100%).
    pub fn is_perfect(&self) -> bool {
        self.convergence_ratio >= 1.0
    }

    /// Get convergence gap (1.0 - ratio).
    pub fn convergence_gap(&self) -> f64 {
        1.0 - self.convergence_ratio
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTStats ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTStats {
    /// Get total operations across all types.
    pub fn total_all_ops(&self) -> u64 {
        self.total_op_based_ops + self.total_delta_ops + self.total_state_merges
    }

    /// Get non-convergence rate.
    pub fn non_convergence_rate(&self) -> f64 {
        1.0 - self.convergence_rate()
    }

    /// Get the ratio of active replicas to total grants.
    pub fn active_ratio(&self) -> f64 {
        if self.total_replicas == 0 {
            return 1.0;
        }
        self.total_replicas as f64 / self.total_replicas.max(1) as f64
    }

    /// Get the overall health score (0.0 to 1.0).
    pub fn health_score(&self) -> f64 {
        let success = self.convergence_rate();
        let merge = self.merge_rate().min(1.0);
        let conflict_penalty = 1.0 - self.conflict_resolution_rate();
        (success + merge) / 2.0 - conflict_penalty / 2.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CRDTError ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl CRDTError {
    /// Check if this error is related to a replica state.
    pub fn is_replica_state_error(&self) -> bool {
        matches!(
            self,
            CRDTError::ReplicaNotFound { .. }
                | CRDTError::ReplicaNotActive { .. }
                | CRDTError::ReplicaFailed { .. }
        )
    }

    /// Check if this error is related to a merge.
    pub fn is_merge_error(&self) -> bool {
        matches!(self, CRDTError::MergeInvalid { .. })
    }

    /// Check if this error is related to a disabled CRDT type.
    pub fn is_type_disabled_error(&self) -> bool {
        matches!(
            self,
            CRDTError::OpBasedDisabled
                | CRDTError::StateBasedDisabled
                | CRDTError::DeltaDisabled
        )
    }

    /// Get error severity.
    pub fn severity(&self) -> &'static str {
        match self {
            CRDTError::ReplicaNotFound { .. } => "low",
            CRDTError::ReplicaNotActive { .. } => "medium",
            CRDTError::ReplicaFailed { .. } => "high",
            CRDTError::MergeInvalid { .. } => "medium",
            CRDTError::ReplicaLimitExceeded { .. } => "high",
            CRDTError::OpBasedDisabled => "medium",
            CRDTError::StateBasedDisabled => "medium",
            CRDTError::DeltaDisabled => "medium",
            CRDTError::InvalidConfig { .. } => "critical",
            CRDTError::Internal { .. } => "critical",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ConvergenceTrend
///////////////////////////////////////////////////////////////////////////////

/// Trend of convergence over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvergenceTrend {
    /// Convergence is improving.
    Improving,
    /// Convergence is declining.
    Declining,
    /// Convergence is stable.
    Stable,
}

impl Default for ConvergenceTrend {
    fn default() -> Self {
        ConvergenceTrend::Stable
    }
}

///////////////////////////////////////////////////////////////////////////////
// GCounter ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl GCounter {
    /// Merge with another GCounter and return self.
    pub fn merge_and_return(&mut self, other: &GCounter) -> &mut Self {
        self.merge(other);
        self
    }

    /// Check if counter is empty.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Get all replica IDs with counts.
    pub fn replica_ids(&self) -> Vec<NodeId> {
        self.counts.keys().cloned().collect()
    }

    /// Get total increment amount.
    pub fn total_increment(&self) -> u64 {
        self.counts.values().sum()
    }
}

///////////////////////////////////////////////////////////////////////////////
// GSet ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl<T: std::hash::Hash + Eq + Clone> GSet<T> {
    /// Merge with another GSet and return self.
    pub fn merge_and_return(&mut self, other: &GSet<T>) -> &mut Self {
        self.merge(other);
        self
    }

    /// Remove an element (not supported in grow-only set, returns false).
    pub fn try_remove(&mut self, _element: &T) -> bool {
        false
    }

    /// Get all elements as a vector.
    pub fn elements_vec(&self) -> Vec<T> {
        self.elements.iter().cloned().collect()
    }

    /// Check if set is subset of another.
    pub fn is_subset_of(&self, other: &GSet<T>) -> bool {
        self.elements.iter().all(|e| other.elements.contains(e))
    }

    /// Check if two sets are equal.
    pub fn equals(&self, other: &GSet<T>) -> bool {
        self.elements == other.elements
    }
}

///////////////////////////////////////////////////////////////////////////////
// PNCounter ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl PNCounter {
    /// Merge with another PNCounter and return self.
    pub fn merge_and_return(&mut self, other: &PNCounter) -> &mut Self {
        self.merge(other);
        self
    }

    /// Get positive count total.
    pub fn positive_total(&self) -> u64 {
        self.positive.value()
    }

    /// Get negative count total.
    pub fn negative_total(&self) -> u64 {
        self.negative.value()
    }

    /// Check if counter is zero.
    pub fn is_zero(&self) -> bool {
        self.value() == 0
    }

    /// Check if counter is positive.
    pub fn is_positive(&self) -> bool {
        self.value() > 0
    }

    /// Check if counter is negative.
    pub fn is_negative(&self) -> bool {
        self.value() < 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// LWWRegister ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl<T: Clone> LWWRegister<T> {
    /// Merge with another LWWRegister and return self.
    pub fn merge_and_return(&mut self, other: &LWWRegister<T>) -> &mut Self {
        self.merge(other);
        self
    }

    /// Check if register has been written (timestamp > 0).
    pub fn is_written(&self) -> bool {
        self.timestamp > 0
    }

    /// Get the timestamp of the last write.
    pub fn last_write_timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get the writer of the last write.
    pub fn last_writer(&self) -> NodeId {
        self.writer
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORSet ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl<T: std::hash::Hash + Eq + Clone> ORSet<T> {
    /// Merge with another ORSet and return self.
    pub fn merge_and_return(&mut self, other: &ORSet<T>) -> &mut Self {
        self.merge(other);
        self
    }

    /// Get all elements (not removed) as a vector.
    pub fn elements_vec(&self) -> Vec<T> {
        self.elements
            .iter()
            .filter(|(_, tags)| tags.iter().any(|tag| !self.removed.contains(tag)))
            .map(|(e, _)| e.clone())
            .collect()
    }

    /// Get total tag count (including removed).
    pub fn total_tag_count(&self) -> usize {
        self.elements.values().map(|tags| tags.len()).sum()
    }

    /// Get removed tag count.
    pub fn removed_tag_count(&self) -> usize {
        self.removed.len()
    }

    /// Check if set is empty (no non-removed elements).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_id() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 1;
        NodeId::new(bytes)
    }

    fn test_node_id2() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 2;
        NodeId::new(bytes)
    }

    fn test_node_id3() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 3;
        NodeId::new(bytes)
    }

    fn test_replica_id() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 10;
        NodeId::new(bytes)
    }

    fn test_replica_id2() -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = 20;
        NodeId::new(bytes)
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_version() -> Version {
        Version::new(1)
    }

    fn test_op_ref() -> OpRef {
        OpRef::new([1u8; 9])
    }

    fn test_config() -> CRDTConfig {
        CRDTConfig::default()
    }

    // ========================================================================
    // CRDTConfig tests
    // ========================================================================

    #[test]
    fn crdt_config_default() {
        let config = CRDTConfig::default();
        assert_eq!(config.max_ops_per_replica, 100000);
        assert_eq!(config.max_merge_depth, 100);
        assert!((config.convergence_threshold - 0.99).abs() < 0.01);
        assert!(config.enable_op_based);
        assert!(config.enable_state_based);
        assert!(config.enable_delta_crdt);
        assert!(config.enable_auto_convergence);
        assert_eq!(config.max_concurrent_merges, 10);
        assert_eq!(config.merge_timeout_ms, 30000);
    }

    #[test]
    fn crdt_config_builder() {
        let config = CRDTConfig::default()
            .with_max_ops(50000)
            .with_convergence_threshold(0.95)
            .with_max_merge_depth(50)
            .with_op_based(false)
            .with_state_based(true)
            .with_delta_crdt(false)
            .with_auto_convergence(false);
        assert_eq!(config.max_ops_per_replica, 50000);
        assert!((config.convergence_threshold - 0.95).abs() < 0.01);
        assert_eq!(config.max_merge_depth, 50);
        assert!(!config.enable_op_based);
        assert!(config.enable_state_based);
        assert!(!config.enable_delta_crdt);
        assert!(!config.enable_auto_convergence);
    }

    #[test]
    fn crdt_config_validate_valid() {
        let config = CRDTConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn crdt_config_validate_max_ops_zero() {
        let config = CRDTConfig::default().with_max_ops(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_validate_max_merge_depth_zero() {
        let config = CRDTConfig::default().with_max_merge_depth(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_validate_threshold_out_of_range() {
        let config = CRDTConfig::default().with_convergence_threshold(1.5);
        assert!(config.validate().is_err());
        let config = CRDTConfig::default().with_convergence_threshold(0.0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_validate_max_concurrent_zero() {
        let config = CRDTConfig::default().with_max_concurrent_merges(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_validate_merge_timeout_zero() {
        let config = CRDTConfig::default().with_merge_timeout_ms(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_validate_no_types_enabled() {
        let config = CRDTConfig::default().with_op_based(false).with_state_based(false).with_delta_crdt(false);
        assert!(config.validate().is_err());
    }

    #[test]
    fn crdt_config_strict() {
        let config = CRDTConfig::strict();
        assert!(config.is_strict());
        assert!(!config.is_lenient());
        assert_eq!(config.max_ops_per_replica, 10000);
        assert_eq!(config.max_concurrent_merges, 2);
    }

    #[test]
    fn crdt_config_lenient() {
        let config = CRDTConfig::lenient();
        assert!(config.is_lenient());
        assert!(!config.is_strict());
        assert_eq!(config.max_ops_per_replica, 1000000);
        assert_eq!(config.max_concurrent_merges, 100);
    }

    #[test]
    fn crdt_config_balanced() {
        let config = CRDTConfig::balanced();
        assert!(!config.is_strict());
        assert!(!config.is_lenient());
        assert_eq!(config.max_ops_per_replica, 100000);
    }

    #[test]
    fn crdt_config_all_types_enabled() {
        let config = CRDTConfig::default();
        assert!(config.all_types_enabled());
        assert!(!config.no_types_enabled());
        assert_eq!(config.enabled_type_count(), 3);
    }

    #[test]
    fn crdt_config_no_types_enabled() {
        let config = CRDTConfig::default().with_op_based(false).with_state_based(false).with_delta_crdt(false);
        assert!(!config.all_types_enabled());
        assert!(config.no_types_enabled());
        assert_eq!(config.enabled_type_count(), 0);
    }

    #[test]
    fn crdt_config_merge_depth_range() {
        let config = CRDTConfig::default();
        let (min, max) = config.merge_depth_range();
        assert_eq!(min, 1);
        assert_eq!(max, 100);
    }

    #[test]
    fn crdt_config_is_merge_depth_valid() {
        let config = CRDTConfig::default();
        assert!(config.is_merge_depth_valid(1));
        assert!(config.is_merge_depth_valid(50));
        assert!(config.is_merge_depth_valid(100));
        assert!(!config.is_merge_depth_valid(0));
        assert!(!config.is_merge_depth_valid(101));
    }

    #[test]
    fn crdt_config_recommended_check_interval() {
        let config = CRDTConfig::default();
        assert_eq!(config.recommended_convergence_check_interval(), 15000);
    }

    #[test]
    fn crdt_config_serialization_roundtrip() {
        let config = CRDTConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CRDTConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.max_ops_per_replica, deserialized.max_ops_per_replica);
        assert_eq!(config.convergence_threshold, deserialized.convergence_threshold);
    }

    // ========================================================================
    // CRDTType tests
    // ========================================================================

    #[test]
    fn crdt_type_is_counter() {
        assert!(CRDTType::GCounter.is_counter());
        assert!(CRDTType::PNCounter.is_counter());
        assert!(!CRDTType::GSet.is_counter());
        assert!(!CRDTType::LWWRegister.is_counter());
    }

    #[test]
    fn crdt_type_is_set() {
        assert!(CRDTType::GSet.is_set());
        assert!(CRDTType::TwoPhaseSet.is_set());
        assert!(CRDTType::ORSet.is_set());
        assert!(!CRDTType::GCounter.is_set());
    }

    #[test]
    fn crdt_type_is_register() {
        assert!(CRDTType::LWWRegister.is_register());
        assert!(!CRDTType::GCounter.is_register());
    }

    #[test]
    fn crdt_type_supports_delta() {
        assert!(CRDTType::GCounter.supports_delta());
        assert!(CRDTType::GSet.supports_delta());
        assert!(CRDTType::PNCounter.supports_delta());
        assert!(CRDTType::ORSet.supports_delta());
        assert!(!CRDTType::LWWRegister.supports_delta());
    }

    #[test]
    fn crdt_type_supports_state_merge() {
        assert!(CRDTType::GCounter.supports_state_merge());
        assert!(CRDTType::GSet.supports_state_merge());
        assert!(CRDTType::LWWRegister.supports_state_merge());
    }

    #[test]
    fn crdt_type_supports_op_based() {
        assert!(CRDTType::GCounter.supports_op_based());
        assert!(CRDTType::GSet.supports_op_based());
    }

    #[test]
    fn crdt_type_default_threshold() {
        assert!((CRDTType::GCounter.default_convergence_threshold() - 0.99).abs() < 0.01);
        assert!((CRDTType::GSet.default_convergence_threshold() - 0.97).abs() < 0.01);
        assert!((CRDTType::LWWRegister.default_convergence_threshold() - 0.95).abs() < 0.01);
    }

    #[test]
    fn crdt_type_display_name() {
        assert_eq!(CRDTType::GCounter.display_name(), "g_counter");
        assert_eq!(CRDTType::GSet.display_name(), "g_set");
        assert_eq!(CRDTType::LWWRegister.display_name(), "lww_register");
    }

    #[test]
    fn crdt_type_default() {
        assert_eq!(CRDTType::default(), CRDTType::StateBased);
    }

    #[test]
    fn crdt_type_serialization_roundtrip() {
        let t = CRDTType::GCounter;
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: CRDTType = serde_json::from_str(&json).unwrap();
        assert_eq!(t, deserialized);
    }

    // ========================================================================
    // CRDTStateType tests
    // ========================================================================

    #[test]
    fn crdt_state_type_is_active() {
        assert!(CRDTStateType::Created.is_active());
        assert!(CRDTStateType::Merging.is_active());
        assert!(CRDTStateType::Converged.is_active());
        assert!(!CRDTStateType::Failed.is_active());
    }

    #[test]
    fn crdt_state_type_is_terminal() {
        assert!(CRDTStateType::Failed.is_terminal());
        assert!(!CRDTStateType::Created.is_terminal());
        assert!(!CRDTStateType::Converged.is_terminal());
    }

    #[test]
    fn crdt_state_type_default() {
        assert_eq!(CRDTStateType::default(), CRDTStateType::Created);
    }

    #[test]
    fn crdt_state_type_display_name() {
        assert_eq!(CRDTStateType::Created.display_name(), "created");
        assert_eq!(CRDTStateType::Converged.display_name(), "converged");
        assert_eq!(CRDTStateType::Failed.display_name(), "failed");
    }

    #[test]
    fn crdt_state_type_requires_intervention() {
        assert!(CRDTStateType::Failed.requires_intervention());
        assert!(!CRDTStateType::Converged.requires_intervention());
    }

    #[test]
    fn crdt_state_type_serialization_roundtrip() {
        let t = CRDTStateType::Converged;
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: CRDTStateType = serde_json::from_str(&json).unwrap();
        assert_eq!(t, deserialized);
    }

    // ========================================================================
    // CRDTOperation tests
    // ========================================================================

    #[test]
    fn crdt_operation_new() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![1, 2, 3], 1000, test_node_id(), test_version());
        assert_eq!(op.op_id, test_op_ref());
        assert_eq!(op.crdt_type, CRDTType::GCounter);
        assert_eq!(op.payload, vec![1, 2, 3]);
        assert_eq!(op.timestamp_ms, 1000);
        assert_eq!(op.origin, test_node_id());
        assert_eq!(op.version, test_version());
    }

    #[test]
    fn crdt_operation_size() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![1, 2, 3, 4, 5], 1000, test_node_id(), test_version());
        assert_eq!(op.size(), 5);
    }

    #[test]
    fn crdt_operation_is_recent() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), test_version());
        assert!(op.is_recent(1500, 1000));
        assert!(!op.is_recent(3000, 1000));
    }

    #[test]
    fn crdt_operation_is_from_origin() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), test_version());
        assert!(op.is_from_origin(test_node_id()));
        assert!(!op.is_from_origin(test_node_id2()));
    }

    #[test]
    fn crdt_operation_version_greater_than() {
        let op1 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), Version::new(5));
        let op2 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), Version::new(3));
        assert!(op1.version_greater_than(&op2));
        assert!(!op2.version_greater_than(&op1));
    }

    #[test]
    fn crdt_operation_age_ms() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), test_version());
        assert_eq!(op.age_ms(1000), 0);
        assert_eq!(op.age_ms(5000), 4000);
    }

    #[test]
    fn crdt_operation_is_stale() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), test_version());
        assert!(!op.is_stale(2000, 2000));
        assert!(op.is_stale(5000, 2000));
    }

    #[test]
    fn crdt_operation_serialization_roundtrip() {
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![1, 2, 3], 1000, test_node_id(), test_version());
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: CRDTOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(op.op_id, deserialized.op_id);
        assert_eq!(op.timestamp_ms, deserialized.timestamp_ms);
    }

    // ========================================================================
    // CRDTReplica tests
    // ========================================================================

    #[test]
    fn crdt_replica_new() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert_eq!(replica.replica_id, test_node_id());
        assert_eq!(replica.crdt_type, CRDTType::GCounter);
        assert_eq!(replica.state, CRDTStateType::Created);
        assert_eq!(replica.total_ops, 0);
        assert_eq!(replica.total_merges, 0);
        assert!(!replica.is_converged);
    }

    #[test]
    fn crdt_replica_apply_operation() {
        let mut replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![1, 2, 3], 1000, test_node_id(), test_version());
        assert!(replica.apply_operation(&op).is_ok());
        assert_eq!(replica.state, CRDTStateType::Converged);
        assert_eq!(replica.total_ops, 1);
        assert!(replica.is_converged);
    }

    #[test]
    fn crdt_replica_apply_operation_failed() {
        let mut replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        replica.state = CRDTStateType::Failed;
        let op = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), test_version());
        let err = replica.apply_operation(&op).unwrap_err();
        assert!(matches!(err, CRDTError::ReplicaFailed { .. }));
    }

    #[test]
    fn crdt_replica_merge_with() {
        let mut replica_a = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let replica_b = CRDTReplica::new(test_node_id2(), CRDTType::GCounter);
        let result = replica_a.merge_with(&replica_b).unwrap();
        assert!(result.is_successful());
        assert!(replica_a.is_converged);
        assert_eq!(replica_a.total_merges, 1);
    }

    #[test]
    fn crdt_replica_merge_with_failed() {
        let mut replica_a = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let mut replica_b = CRDTReplica::new(test_node_id2(), CRDTType::GCounter);
        replica_b.state = CRDTStateType::Failed;
        let err = replica_a.merge_with(&replica_b).unwrap_err();
        assert!(matches!(err, CRDTError::ReplicaFailed { .. }));
    }

    #[test]
    fn crdt_replica_is_converged() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!(!replica.is_converged());
        let mut replica = replica;
        replica.state = CRDTStateType::Converged;
        replica.is_converged = true;
        assert!(replica.is_converged());
    }

    #[test]
    fn crdt_replica_op_rate() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!((replica.op_rate() - 0.0).abs() < 0.01);
        let mut replica = replica;
        replica.total_ops = 10;
        replica.total_merges = 2;
        assert!((replica.op_rate() - 5.0).abs() < 0.01);
    }

    #[test]
    fn crdt_replica_needs_convergence() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!(!replica.needs_convergence());
        let mut replica = replica;
        replica.state = CRDTStateType::Merging;
        assert!(replica.needs_convergence());
    }

    #[test]
    fn crdt_replica_state_size() {
        let mut replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert_eq!(replica.state_size(), 0);
        replica.crdt_state.inner = vec![1, 2, 3, 4, 5];
        assert_eq!(replica.state_size(), 5);
    }

    #[test]
    fn crdt_replica_version_count() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert_eq!(replica.version_count(), 0);
        let mut replica = replica;
        replica.version_vector.insert(test_node_id2(), Version::new(1));
        assert_eq!(replica.version_count(), 1);
    }

    #[test]
    fn crdt_replica_is_idle() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!(replica.is_idle(1000, 500));
        let mut replica = replica;
        replica.last_op_at_ms = Some(1000);
        assert!(!replica.is_idle(1500, 500));
        assert!(replica.is_idel_check(2000, 500));
    }

    #[test]
    fn crdt_replica_merge_to_op_ratio() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!((replica.merge_to_op_ratio() - 0.0).abs() < 0.01);
        let mut replica = replica;
        replica.total_ops = 10;
        replica.total_merges = 2;
        assert!((replica.merge_to_op_ratio() - 0.2).abs() < 0.01);
    }

    #[test]
    fn crdt_replica_has_pending_operations() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        assert!(!replica.has_pending_operations());
        let mut replica = replica;
        replica.total_ops = 5;
        assert!(replica.has_pending_operations());
        replica.is_converged = true;
        assert!(!replica.has_pending_operations());
    }

    #[test]
    fn crdt_replica_state_size_percent() {
        let mut replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        replica.crdt_state.inner = vec![0u8; 524288]; // 512KB
        assert!((replica.state_size_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn crdt_replica_serialization_roundtrip() {
        let replica = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let json = serde_json::to_string(&replica).unwrap();
        let deserialized: CRDTReplica = serde_json::from_str(&json).unwrap();
        assert_eq!(replica.replica_id, deserialized.replica_id);
        assert_eq!(replica.crdt_type, deserialized.crdt_type);
    }

    // ========================================================================
    // CRDTMergeResult tests
    // ========================================================================

    #[test]
    fn crdt_merge_result_new() {
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, true);
        assert_eq!(result.replica_a, test_node_id());
        assert_eq!(result.replica_b, test_node_id2());
        assert_eq!(result.merged_state_size, 1000);
        assert!(result.success);
    }

    #[test]
    fn crdt_merge_result_is_successful() {
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, true);
        assert!(result.is_successful());
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, false);
        assert!(!result.is_successful());
    }

    #[test]
    fn crdt_merge_result_merge_efficiency() {
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1024, true);
        assert!((result.merge_efficiency() - 1.0).abs() < 0.01);
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1024, false);
        assert!((result.merge_efficiency() - 0.0).abs() < 0.01);
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 0, true);
        assert!((result.merge_efficiency() - 0.0).abs() < 0.01);
    }

    #[test]
    fn crdt_merge_result_conflicts_per_byte() {
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, true);
        result.conflicts_resolved = 50;
        assert!((result.conflicts_per_byte() - 0.05).abs() < 0.01);
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 0, true);
        assert!((result.conflicts_per_byte() - 0.0).abs() < 0.01);
    }

    #[test]
    fn crdt_merge_result_resolved_conflicts() {
        let mut result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, true);
        assert!(!result.resolved_conflicts());
        result.conflicts_resolved = 1;
        assert!(result.resolved_conflicts());
    }

    #[test]
    fn crdt_merge_result_serialization_roundtrip() {
        let result = CRDTMergeResult::new(test_node_id(), test_node_id2(), 1000, true);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CRDTMergeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.replica_a, deserialized.replica_a);
        assert_eq!(result.merged_state_size, deserialized.merged_state_size);
    }

    // ========================================================================
    // CRDTConvergenceResult tests
    // ========================================================================

    #[test]
    fn crdt_convergence_result_new() {
        let result = CRDTConvergenceResult::new(10, 8);
        assert_eq!(result.total_replicas, 10);
        assert_eq!(result.converged_replicas, 8);
        assert_eq!(result.non_converged_replicas, 2);
        assert!((result.convergence_ratio - 0.8).abs() < 0.01);
    }

    #[test]
    fn crdt_convergence_result_new_zero_total() {
        let result = CRDTConvergenceResult::new(0, 0);
        assert!((result.convergence_ratio - 1.0).abs() < 0.01);
        assert!(result.all_converged);
    }

    #[test]
    fn crdt_convergence_result_meets_threshold() {
        let result = CRDTConvergenceResult::new(10, 8);
        assert!(result.meets_threshold(0.7));
        assert!(!result.meets_threshold(0.9));
    }

    #[test]
    fn crdt_convergence_result_is_perfect() {
        let result = CRDTConvergenceResult::new(10, 10);
        assert!(result.is_perfect());
        let result = CRDTConvergenceResult::new(10, 9);
        assert!(!result.is_perfect());
    }

    #[test]
    fn crdt_convergence_result_convergence_gap() {
        let result = CRDTConvergenceResult::new(10, 7);
        assert!((result.convergence_gap() - 0.3).abs() < 0.01);
    }

    #[test]
    fn crdt_convergence_result_non_converged_count() {
        let result = CRDTConvergenceResult::new(10, 7);
        assert_eq!(result.non_converged_count(), 3);
    }

    #[test]
    fn crdt_convergence_result_serialization_roundtrip() {
        let result = CRDTConvergenceResult::new(10, 8);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CRDTConvergenceResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.total_replicas, deserialized.total_replicas);
        assert_eq!(result.converged_replicas, deserialized.converged_replicas);
    }

    // ========================================================================
    // CRDTStats tests
    // ========================================================================

    #[test]
    fn crdt_stats_default() {
        let stats = CRDTStats::default();
        assert_eq!(stats.total_replicas, 0);
        assert_eq!(stats.total_ops, 0);
        assert!((stats.convergence_rate() - 1.0).abs() < 0.01);
        assert!((stats.merge_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_convergence_rate() {
        let mut stats = CRDTStats::default();
        stats.total_convergence_checks = 10;
        stats.total_convergences = 7;
        assert!((stats.convergence_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_merge_rate() {
        let mut stats = CRDTStats::default();
        stats.total_ops = 10;
        stats.total_merges = 3;
        assert!((stats.merge_rate() - 0.3).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_conflict_resolution_rate() {
        let mut stats = CRDTStats::default();
        stats.total_merges = 10;
        stats.total_conflicts_resolved = 4;
        assert!((stats.conflict_resolution_rate() - 0.4).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_op_based_ratio() {
        let mut stats = CRDTStats::default();
        stats.total_ops = 10;
        stats.total_op_based_ops = 7;
        assert!((stats.op_based_ratio() - 0.7).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_delta_ratio() {
        let mut stats = CRDTStats::default();
        stats.total_ops = 10;
        stats.total_delta_ops = 3;
        assert!((stats.delta_ratio() - 0.3).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_total_all_ops() {
        let mut stats = CRDTStats::default();
        stats.total_op_based_ops = 5;
        stats.total_delta_ops = 3;
        stats.total_state_merges = 2;
        assert_eq!(stats.total_all_ops(), 10);
    }

    #[test]
    fn crdt_stats_non_convergence_rate() {
        let mut stats = CRDTStats::default();
        stats.total_convergence_checks = 10;
        stats.total_convergences = 7;
        assert!((stats.non_convergence_rate() - 0.3).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_health_score() {
        let mut stats = CRDTStats::default();
        stats.total_convergence_checks = 10;
        stats.total_convergences = 10;
        stats.total_ops = 10;
        stats.total_merges = 5;
        let score = stats.health_score();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn crdt_stats_active_ratio() {
        let mut stats = CRDTStats::default();
        stats.total_replicas = 10;
        assert!((stats.active_ratio() - 1.0).abs() < 0.01);
    }

    #[test]
    fn crdt_stats_serialization_roundtrip() {
        let mut stats = CRDTStats::default();
        stats.total_replicas = 5;
        stats.total_ops = 100;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CRDTStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_replicas, deserialized.total_replicas);
        assert_eq!(stats.total_ops, deserialized.total_ops);
    }

    // ========================================================================
    // CRDTError tests
    // ========================================================================

    #[test]
    fn crdt_error_category() {
        assert_eq!(CRDTError::ReplicaNotFound { replica_id: test_node_id() }.category(), "replica_not_found");
        assert_eq!(CRDTError::MergeInvalid { replica_a: test_node_id(), replica_b: test_node_id2() }.category(), "merge_invalid");
        assert_eq!(CRDTError::Internal { message: "x".to_string() }.category(), "internal");
    }

    #[test]
    fn crdt_error_is_recoverable() {
        assert!(CRDTError::ReplicaNotFound { replica_id: test_node_id() }.is_recoverable());
        assert!(!CRDTError::ReplicaFailed { replica_id: test_node_id() }.is_recoverable());
    }

    #[test]
    fn crdt_error_suggested_action() {
        assert_eq!(CRDTError::ReplicaNotFound { replica_id: test_node_id() }.suggested_action(), "create the replica first");
        assert_eq!(CRDTError::OpBasedDisabled.suggested_action(), "enable op-based CRDTs in configuration");
    }

    #[test]
    fn crdt_error_is_crdt_state_error() {
        assert!(CRDTError::ReplicaNotFound { replica_id: test_node_id() }.is_crdt_state_error());
        assert!(!CRDTError::OpBasedDisabled.is_crdt_state_error());
    }

    #[test]
    fn crdt_error_is_merge_error() {
        assert!(CRDTError::MergeInvalid { replica_a: test_node_id(), replica_b: test_node_id2() }.is_merge_error());
        assert!(!CRDTError::ReplicaNotFound { replica_id: test_node_id() }.is_merge_error());
    }

    #[test]
    fn crdt_error_is_type_disabled_error() {
        assert!(CRDTError::OpBasedDisabled.is_type_disabled_error());
        assert!(!CRDTError::ReplicaNotFound { replica_id: test_node_id() }.is_type_disabled_error());
    }

    #[test]
    fn crdt_error_severity() {
        assert_eq!(CRDTError::ReplicaNotFound { replica_id: test_node_id() }.severity(), "low");
        assert_eq!(CRDTError::ReplicaFailed { replica_id: test_node_id() }.severity(), "high");
        assert_eq!(CRDTError::InvalidConfig { field: "x".to_string(), reason: "y".to_string() }.severity(), "critical");
    }

    // ========================================================================
    // GCounter tests
    // ========================================================================

    #[test]
    fn gcounter_new() {
        let counter = GCounter::new();
        assert_eq!(counter.value(), 0);
        assert!(counter.is_empty());
        assert_eq!(counter.replica_count(), 0);
    }

    #[test]
    fn gcounter_increment() {
        let mut counter = GCounter::new();
        counter.increment(test_node_id(), 5);
        counter.increment(test_node_id(), 3);
        counter.increment(test_node_id2(), 10);
        assert_eq!(counter.value(), 18);
        assert_eq!(counter.get_count(test_node_id()), 8);
        assert_eq!(counter.get_count(test_node_id2()), 10);
        assert_eq!(counter.replica_count(), 2);
    }

    #[test]
    fn gcounter_merge() {
        let mut counter_a = GCounter::new();
        counter_a.increment(test_node_id(), 5);
        counter_a.increment(test_node_id2(), 3);
        let mut counter_b = GCounter::new();
        counter_b.increment(test_node_id(), 2);
        counter_b.increment(test_node_id3(), 7);
        counter_a.merge(&counter_b);
        assert_eq!(counter_a.get_count(test_node_id()), 5);
        assert_eq!(counter_a.get_count(test_node_id2()), 3);
        assert_eq!(counter_a.get_count(test_node_id3()), 7);
        assert_eq!(counter_a.value(), 15);
    }

    #[test]
    fn gcounter_merge_and_return() {
        let mut counter_a = GCounter::new();
        counter_a.increment(test_node_id(), 5);
        let mut counter_b = GCounter::new();
        counter_b.increment(test_node_id(), 3);
        counter_a.merge_and_return(&counter_b);
        assert_eq!(counter_a.get_count(test_node_id()), 5);
    }

    #[test]
    fn gcounter_replica_ids() {
        let mut counter = GCounter::new();
        counter.increment(test_node_id(), 1);
        counter.increment(test_node_id2(), 2);
        let ids = counter.replica_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn gcounter_total_increment() {
        let mut counter = GCounter::new();
        counter.increment(test_node_id(), 5);
        counter.increment(test_node_id2(), 10);
        assert_eq!(counter.total_increment(), 15);
    }

    #[test]
    fn gcounter_serialization_roundtrip() {
        let mut counter = GCounter::new();
        counter.increment(test_node_id(), 5);
        let json = serde_json::to_string(&counter).unwrap();
        let deserialized: GCounter = serde_json::from_str(&json).unwrap();
        assert_eq!(counter.get_count(test_node_id()), deserialized.get_count(test_node_id()));
    }

    // ========================================================================
    // GSet tests
    // ========================================================================

    #[test]
    fn gset_new() {
        let set: GSet<u32> = GSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn gset_add_and_contains() {
        let mut set = GSet::new();
        set.add(1);
        set.add(2);
        set.add(3);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(!set.contains(&4));
    }

    #[test]
    fn gset_merge() {
        let mut set_a = GSet::new();
        set_a.add(1);
        set_a.add(2);
        let mut set_b = GSet::new();
        set_b.add(2);
        set_b.add(3);
        set_a.merge(&set_b);
        assert_eq!(set_a.len(), 3);
        assert!(set_a.contains(&3));
    }

    #[test]
    fn gset_merge_and_return() {
        let mut set_a = GSet::new();
        set_a.add(1);
        let mut set_b = GSet::new();
        set_b.add(2);
        set_a.merge_and_return(&set_b);
        assert!(set_a.contains(&2));
    }

    #[test]
    fn gset_try_remove_always_false() {
        let mut set = GSet::new();
        set.add(1);
        assert!(!set.try_remove(&1));
    }

    #[test]
    fn gset_elements_vec() {
        let mut set = GSet::new();
        set.add(1);
        set.add(2);
        let mut elements = set.elements_vec();
        elements.sort();
        assert_eq!(elements, vec![1, 2]);
    }

    #[test]
    fn gset_is_subset_of() {
        let mut set_a = GSet::new();
        set_a.add(1);
        set_a.add(2);
        let mut set_b = GSet::new();
        set_b.add(1);
        set_b.add(2);
        set_b.add(3);
        assert!(set_a.is_subset_of(&set_b));
        assert!(!set_b.is_subset_of(&set_a));
    }

    #[test]
    fn gset_equals() {
        let mut set_a = GSet::new();
        set_a.add(1);
        set_a.add(2);
        let mut set_b = GSet::new();
        set_b.add(1);
        set_b.add(2);
        assert!(set_a.equals(&set_b));
        set_b.add(3);
        assert!(!set_a.equals(&set_b));
    }

    #[test]
    fn gset_serialization_roundtrip() {
        let mut set = GSet::new();
        set.add(42);
        let json = serde_json::to_string(&set).unwrap();
        let deserialized: GSet<u32> = serde_json::from_str(&json).unwrap();
        assert!(deserialized.contains(&42));
    }

    // ========================================================================
    // PNCounter tests
    // ========================================================================

    #[test]
    fn pncounter_new() {
        let counter = PNCounter::new();
        assert_eq!(counter.value(), 0);
        assert!(counter.is_zero());
        assert!(!counter.is_positive());
        assert!(!counter.is_negative());
    }

    #[test]
    fn pncounter_increment_and_decrement() {
        let mut counter = PNCounter::new();
        counter.increment(test_node_id(), 10);
        counter.decrement(test_node_id(), 3);
        counter.increment(test_node_id2(), 5);
        assert_eq!(counter.value(), 12);
        assert!(counter.is_positive());
    }

    #[test]
    fn pncounter_merge() {
        let mut counter_a = PNCounter::new();
        counter_a.increment(test_node_id(), 10);
        counter_a.decrement(test_node_id(), 2);
        let mut counter_b = PNCounter::new();
        counter_b.increment(test_node_id(), 5);
        counter_b.decrement(test_node_id2(), 3);
        counter_a.merge(&counter_b);
        assert_eq!(counter_a.positive_total(), 15);
        assert_eq!(counter_a.negative_total(), 5);
        assert_eq!(counter_a.value(), 10);
    }

    #[test]
    fn pncounter_merge_and_return() {
        let mut counter_a = PNCounter::new();
        counter_a.increment(test_node_id(), 5);
        let mut counter_b = PNCounter::new();
        counter_b.increment(test_node_id(), 3);
        counter_a.merge_and_return(&counter_b);
        assert_eq!(counter_a.positive_total(), 8);
    }

    #[test]
    fn pncounter_is_zero_positive_negative() {
        let mut counter = PNCounter::new();
        assert!(counter.is_zero());
        counter.increment(test_node_id(), 5);
        assert!(counter.is_positive());
        counter.decrement(test_node_id(), 10);
        assert!(counter.is_negative());
    }

    #[test]
    fn pncounter_positive_and_negative_total() {
        let mut counter = PNCounter::new();
        counter.increment(test_node_id(), 10);
        counter.decrement(test_node_id(), 3);
        assert_eq!(counter.positive_total(), 10);
        assert_eq!(counter.negative_total(), 3);
    }

    #[test]
    fn pncounter_serialization_roundtrip() {
        let mut counter = PNCounter::new();
        counter.increment(test_node_id(), 5);
        let json = serde_json::to_string(&counter).unwrap();
        let deserialized: PNCounter = serde_json::from_str(&json).unwrap();
        assert_eq!(counter.value(), deserialized.value());
    }

    // ========================================================================
    // LWWRegister tests
    // ========================================================================

    #[test]
    fn lww_register_new() {
        let reg = LWWRegister::new(42u32, 1000, test_node_id());
        assert_eq!(reg.get(), 42);
        assert_eq!(reg.last_write_timestamp(), 1000);
        assert_eq!(reg.last_writer(), test_node_id());
    }

    #[test]
    fn lww_register_write() {
        let mut reg = LWWRegister::new(42u32, 1000, test_node_id());
        reg.write(100u32, 2000, test_node_id2());
        assert_eq!(reg.get(), 100);
        assert_eq!(reg.last_write_timestamp(), 2000);
        assert_eq!(reg.last_writer(), test_node_id2());
    }

    #[test]
    fn lww_register_write_older_timestamp_ignored() {
        let mut reg = LWWRegister::new(42u32, 2000, test_node_id());
        reg.write(100u32, 1000, test_node_id2());
        assert_eq!(reg.get(), 42);
        assert_eq!(reg.last_writer(), test_node_id());
    }

    #[test]
    fn lww_register_merge() {
        let mut reg_a = LWWRegister::new(42u32, 1000, test_node_id());
        let reg_b = LWWRegister::new(100u32, 2000, test_node_id2());
        reg_a.merge(&reg_b);
        assert_eq!(reg_a.get(), 100);
        assert_eq!(reg_a.last_writer(), test_node_id2());
    }

    #[test]
    fn lww_register_merge_and_return() {
        let mut reg_a = LWWRegister::new(42u32, 1000, test_node_id());
        let reg_b = LWWRegister::new(100u32, 2000, test_node_id2());
        reg_a.merge_and_return(&reg_b);
        assert_eq!(reg_a.get(), 100);
    }

    #[test]
    fn lww_register_is_written() {
        let reg = LWWRegister::new(42u32, 0, test_node_id());
        assert!(!reg.is_written());
        let reg = LWWRegister::new(42u32, 1000, test_node_id());
        assert!(reg.is_written());
    }

    #[test]
    fn lww_register_serialization_roundtrip() {
        let reg = LWWRegister::new(42u32, 1000, test_node_id());
        let json = serde_json::to_string(&reg).unwrap();
        let deserialized: LWWRegister<u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(reg.get(), deserialized.get());
    }

    // ========================================================================
    // ORSet tests
    // ========================================================================

    #[test]
    fn orset_new() {
        let set: ORSet<u32> = ORSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.removed_tag_count(), 0);
    }

    #[test]
    fn orset_add_and_contains() {
        let mut set = ORSet::new();
        set.add(1);
        set.add(2);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&1));
        assert!(!set.contains(&3));
    }

    #[test]
    fn orset_remove() {
        let mut set = ORSet::new();
        set.add(1);
        set.add(2);
        set.remove(&1);
        assert!(!set.contains(&1));
        assert!(set.contains(&2));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn orset_merge() {
        let mut set_a = ORSet::new();
        set_a.add(1);
        set_a.add(2);
        let mut set_b = ORSet::new();
        set_b.add(2);
        set_b.add(3);
        set_a.merge(&set_b);
        assert!(set_a.contains(&3));
        assert_eq!(set_a.len(), 3);
    }

    #[test]
    fn orset_merge_and_return() {
        let mut set_a = ORSet::new();
        set_a.add(1);
        let mut set_b = ORSet::new();
        set_b.add(2);
        set_a.merge_and_return(&set_b);
        assert!(set_a.contains(&2));
    }

    #[test]
    fn orset_elements_vec() {
        let mut set = ORSet::new();
        set.add(1);
        set.add(2);
        let mut elements = set.elements_vec();
        elements.sort();
        assert_eq!(elements, vec![1, 2]);
    }

    #[test]
    fn orset_total_and_removed_tag_count() {
        let mut set = ORSet::new();
        set.add(1);
        set.add(2);
        assert_eq!(set.total_tag_count(), 2);
        assert_eq!(set.removed_tag_count(), 0);
        set.remove(&1);
        assert_eq!(set.removed_tag_count(), 1);
    }

    #[test]
    fn orset_is_empty() {
        let set: ORSet<u32> = ORSet::new();
        assert!(set.is_empty());
        let mut set = set;
        set.add(1);
        assert!(!set.is_empty());
    }

    #[test]
    fn orset_serialization_roundtrip() {
        let mut set = ORSet::new();
        set.add(42);
        let json = serde_json::to_string(&set).unwrap();
        let deserialized: ORSet<u32> = serde_json::from_str(&json).unwrap();
        assert!(deserialized.contains(&42));
    }

    // ========================================================================
    // CRDTValidator tests
    // ========================================================================

    #[test]
    fn crdt_validator_new() {
        let validator = CRDTValidator::new();
        assert!(validator.is_convergence_enabled());
        assert!(validator.is_merge_validation_enabled());
        assert!(validator.is_op_ordering_enabled());
    }

    #[test]
    fn crdt_validator_validate_convergence() {
        let validator = CRDTValidator::new();
        let mut r1 = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        r1.is_converged = true;
        let mut r2 = CRDTReplica::new(test_node_id2(), CRDTType::GCounter);
        r2.is_converged = false;
        let result = validator.validate_convergence(&[r1, r2], 0.5);
        assert_eq!(result.total_replicas, 2);
        assert_eq!(result.converged_replicas, 1);
        assert!((result.convergence_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn crdt_validator_validate_merge() {
        let validator = CRDTValidator::new();
        let r1 = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let r2 = CRDTReplica::new(test_node_id2(), CRDTType::GCounter);
        assert!(validator.validate_merge(&r1, &r2));
    }

    #[test]
    fn crdt_validator_validate_op_ordering() {
        let validator = CRDTValidator::new();
        let op1 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), Version::new(1));
        let op2 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 2000, test_node_id2(), Version::new(2));
        assert!(validator.validate_op_ordering(&op1, &op2));
        assert!(!validator.validate_op_ordering(&op2, &op1));
    }

    #[test]
    fn crdt_validator_toggle_settings() {
        let mut validator = CRDTValidator::new();
        validator.set_convergence_enabled(false);
        assert!(!validator.is_convergence_enabled());
        validator.set_merge_validation_enabled(false);
        assert!(!validator.is_merge_validation_enabled());
        validator.set_op_ordering_enabled(false);
        assert!(!validator.is_op_ordering_enabled());
    }

    #[test]
    fn crdt_validator_validation_settings() {
        let validator = CRDTValidator::new();
        let (conv, merge, order) = validator.validation_settings();
        assert!(conv);
        assert!(merge);
        assert!(order);
    }

    #[test]
    fn crdt_validator_validate_convergence_batch() {
        let validator = CRDTValidator::new();
        let mut r1 = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        r1.is_converged = true;
        let batch1 = vec![r1];
        let batch2 = vec![CRDTReplica::new(test_node_id2(), CRDTType::GCounter)];
        let results = validator.validate_convergence_batch(&[batch1, batch2], 0.5);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn crdt_validator_validate_merges() {
        let validator = CRDTValidator::new();
        let r1 = CRDTReplica::new(test_node_id(), CRDTType::GCounter);
        let r2 = CRDTReplica::new(test_node_id2(), CRDTType::GCounter);
        let results = validator.validate_merges(&[(r1, r2)]);
        assert_eq!(results.len(), 1);
        assert!(results[0]);
    }

    #[test]
    fn crdt_validator_validate_op_ordering_batch() {
        let validator = CRDTValidator::new();
        let op1 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 1000, test_node_id(), Version::new(1));
        let op2 = CRDTOperation::new(test_op_ref(), CRDTType::GCounter, vec![], 2000, test_node_id2(), Version::new(2));
        let results = validator.validate_op_ordering_batch(&[(op1.clone(), op2.clone())]);
        assert_eq!(results.len(), 1);
        assert!(results[0]);
    }

    // ========================================================================
    // CRTDEngine tests
    // ========================================================================

    #[test]
    fn crdt_engine_new() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.stats().total_replicas, 0);
        assert_eq!(engine.total_replica_count(), 0);
    }

    #[test]
    fn crdt_engine_configure() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let config = CRDTConfig::default().with_max_ops(50000);
        assert!(engine.configure(config).is_ok());
        assert_eq!(engine.effective_max_ops(), 50000);
    }

    #[test]
    fn crdt_engine_configure_invalid() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let config = CRDTConfig::default().with_max_ops(0);
        assert!(engine.configure(config).is_err());
    }

    #[test]
    fn crdt_engine_create_replica() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let replica = engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(replica.replica_id, test_node_id());
        assert_eq!(engine.stats().total_replicas, 1);
        assert_eq!(engine.total_replica_count(), 1);
    }

    #[test]
    fn crdt_engine_create_replica_set() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let replicas = engine.create_replica_set(&[test_node_id(), test_node_id2()], CRDTType::GCounter).unwrap();
        assert_eq!(replicas.len(), 2);
        assert_eq!(engine.total_replica_count(), 2);
    }

    #[test]
    fn crdt_engine_apply_op() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let op = CRDTOp::new(vec![1, 2, 3]);
        assert!(engine.apply_op(test_node_id(), op).is_ok());
        assert_eq!(engine.stats().total_ops, 1);
    }

    #[test]
    fn crdt_engine_apply_op_replica_not_found() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let err = engine.apply_op(test_node_id(), CRDTOp::new(vec![])).unwrap_err();
        assert!(matches!(err, CRDTError::ReplicaNotFound { .. }));
    }

    #[test]
    fn crdt_engine_merge_states() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        let result = engine.merge_states(test_node_id(), test_node_id2()).unwrap();
        assert!(result.is_successful());
        assert_eq!(engine.stats().total_merges, 1);
    }

    #[test]
    fn crdt_engine_apply_delta() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let delta = CRDTDelta::new(vec![1, 2]);
        assert!(engine.apply_delta(test_node_id(), delta).is_ok());
        assert_eq!(engine.stats().total_delta_ops, 1);
    }

    #[test]
    fn crdt_engine_merge_deltas() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        let result = engine.merge_deltas(test_node_id(), test_node_id2()).unwrap();
        assert!(result.is_successful());
    }

    #[test]
    fn crdt_engine_check_convergence() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        let result = engine.check_convergence(&[test_node_id(), test_node_id2()]).unwrap();
        assert_eq!(result.total_replicas, 2);
    }

    #[test]
    fn crdt_engine_get_state() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let state = engine.get_state(test_node_id()).unwrap();
        assert!(state.inner.is_empty());
    }

    #[test]
    fn crdt_engine_crdt_type() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(engine.crdt_type(test_node_id()).unwrap(), CRDTType::GCounter);
    }

    #[test]
    fn crdt_engine_stats() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_replicas, 1);
    }

    #[test]
    fn crdt_engine_health_score() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let score = engine.health_score(test_node_id()).unwrap();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn crdt_engine_all_replica_ids() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        let ids = engine.all_replica_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn crdt_engine_converged_and_failed_count() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(engine.converged_replica_count(), 0);
        assert_eq!(engine.failed_replica_count(), 0);
    }

    #[test]
    fn crdt_engine_full_crdt_lifecycle() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let result = engine.full_crdt_lifecycle(test_node_id(), test_node_id2(), CRDTType::GCounter).unwrap();
        assert_eq!(result.total_replicas, 2);
        assert_eq!(engine.total_replica_count(), 2);
    }

    #[test]
    fn crdt_engine_create_replica_set_and_ops() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let replicas = engine.create_replica_set(&[test_node_id(), test_node_id2()], CRDTType::GCounter).unwrap();
        assert_eq!(replicas.len(), 2);
        let ops = vec![(test_node_id(), CRDTOp::new(vec![1])), (test_node_id2(), CRDTOp::new(vec![2]))];
        assert!(engine.apply_ops_to_replicas(&ops).is_ok());
    }

    #[test]
    fn crdt_engine_merge_all_replicas() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        let results = engine.merge_all_replicas(&[test_node_id(), test_node_id2()]).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn crdt_engine_check_all_convergence() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let result = engine.check_all_convergence().unwrap();
        assert_eq!(result.total_replicas, 1);
    }

    #[test]
    fn crdt_engine_get_all_states() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let states = engine.get_all_states();
        assert_eq!(states.len(), 1);
    }

    #[test]
    fn crdt_engine_get_all_health_scores() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let scores = engine.get_all_health_scores();
        assert_eq!(scores.len(), 1);
    }

    #[test]
    fn crdt_engine_get_replicas_by_type() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let replicas = engine.get_replicas_by_type(CRDTType::GCounter);
        assert_eq!(replicas.len(), 1);
    }

    #[test]
    fn crdt_engine_get_converged_and_failed_by_type() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(engine.get_converged_by_type(CRDTType::GCounter).len(), 0);
        assert_eq!(engine.get_failed_by_type(CRDTType::GCounter).len(), 0);
    }

    #[test]
    fn crdt_engine_total_and_average_state_size() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(engine.total_state_size(), 0);
        assert!((engine.average_state_size() - 0.0).abs() < 0.01);
    }

    #[test]
    fn crdt_engine_version_vector_sizes() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let sizes = engine.version_vector_sizes();
        assert_eq!(sizes.len(), 1);
    }

    #[test]
    fn crdt_engine_all_replicas_converged() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!(engine.all_replicas_converged());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert!(!engine.all_replicas_converged());
    }

    #[test]
    fn crdt_engine_any_replica_failed() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!(!engine.any_replica_failed());
    }

    #[test]
    fn crdt_engine_get_idle_replicas() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let idle = engine.get_idle_replicas(10000, 500);
        assert_eq!(idle.len(), 1);
    }

    #[test]
    fn crdt_engine_op_rate_summary() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let (min, max, avg) = engine.op_rate_summary();
        assert_eq!(min, 0.0);
        assert_eq!(max, 0.0);
        assert_eq!(avg, 0.0);
    }

    #[test]
    fn crdt_engine_reset_all() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.reset_all();
        assert_eq!(engine.total_replica_count(), 0);
    }

    #[test]
    fn crdt_engine_config_clone() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let config = engine.config_clone();
        assert_eq!(config.max_ops_per_replica, 100000);
    }

    #[test]
    fn crdt_engine_health_report() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let report = engine.health_report();
        assert_eq!(report.total_replicas, 1);
        assert!(report.is_healthy(0.5));
    }

    #[test]
    fn crdt_engine_replicas_needing_convergence() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let needing = engine.replicas_needing_convergence();
        assert_eq!(needing.len(), 1);
    }

    #[test]
    fn crdt_engine_count_replicas_needing_convergence() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        assert_eq!(engine.count_replicas_needing_convergence(), 1);
    }

    #[test]
    fn crdt_engine_merge_success_rate() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!((engine.merge_success_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn crdt_engine_average_merged_state_size() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!((engine.average_merged_state_size() - 0.0).abs() < 0.01);
    }

    #[test]
    fn crdt_engine_any_merge_resolved_conflicts() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!(!engine.any_merge_resolved_conflicts());
    }

    #[test]
    fn crdt_engine_count_conflict_resolving_merges() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.count_conflict_resolving_merges(), 0);
    }

    #[test]
    fn crdt_engine_validate_config() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!(engine.validate_config().is_ok());
    }

    #[test]
    fn crdt_engine_config_strict_lenient() {
        let engine = CRTDEngine::new(CRDTConfig::strict());
        assert!(engine.config_is_strict());
        let engine = CRTDEngine::new(CRDTConfig::lenient());
        assert!(engine.config_is_lenient());
    }

    #[test]
    fn crdt_engine_effective_settings() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.effective_convergence_threshold(), 0.99);
        assert_eq!(engine.effective_max_merge_depth(), 100);
        assert!(engine.is_op_based_enabled());
        assert!(engine.is_state_based_enabled());
        assert!(engine.is_delta_crdt_enabled());
        assert!(engine.is_auto_convergence_enabled());
    }

    #[test]
    fn crdt_engine_max_concurrent_and_timeout() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.max_concurrent_merges(), 10);
        assert_eq!(engine.merge_timeout_ms(), 30000);
    }

    #[test]
    fn crdt_engine_total_and_delta_and_state_counts() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.total_delta_operation_count(), 0);
        assert_eq!(engine.total_state_merge_count(), 0);
        assert_eq!(engine.total_op_based_operation_count(), 0);
    }

    #[test]
    fn crdt_engine_convergence_trend() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.convergence_trend(), ConvergenceTrend::Stable);
    }

    #[test]
    fn crdt_engine_serialization_of_stats() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        let stats = engine.stats();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CRDTStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_replicas, deserialized.total_replicas);
    }

    #[test]
    fn crdt_engine_health_report_serialization() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let report = engine.health_report();
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: super::CRDTHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.total_replicas, deserialized.total_replicas);
    }

    #[test]
    fn crdt_engine_full_lifecycle_serialization() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let result = engine.full_crdt_lifecycle(test_node_id(), test_node_id2(), CRDTType::GCounter).unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CRDTConvergenceResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.total_replicas, deserialized.total_replicas);
    }

    #[test]
    fn crdt_engine_all_operations_and_merges_and_convergences() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.all_operations().len(), 0);
        assert_eq!(engine.all_merge_results().len(), 0);
        assert_eq!(engine.all_convergence_results().len(), 0);
    }

    #[test]
    fn crdt_engine_most_recent_merge_and_convergence_and_operation() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert!(engine.most_recent_merge().is_none());
        assert!(engine.most_recent_convergence().is_none());
        assert!(engine.most_recent_operation().is_none());
    }

    #[test]
    fn crdt_engine_domains_with_authority_not_applicable() {
        // CRDT engine doesn't have authority epochs, this tests that unrelated methods don't exist
        let engine = CRTDEngine::new(CRDTConfig::default());
        assert_eq!(engine.all_replica_ids().len(), 0);
    }

    #[test]
    fn crdt_engine_gcounter_and_pncounter_types() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::PNCounter).unwrap();
        assert_eq!(engine.get_replicas_by_type(CRDTType::GCounter).len(), 1);
        assert_eq!(engine.get_replicas_by_type(CRDTType::PNCounter).len(), 1);
    }

    #[test]
    fn crdt_engine_clear_all() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.reset_all();
        assert_eq!(engine.total_replica_count(), 0);
    }

    #[test]
    fn crdt_engine_config_borrow() {
        let engine = CRTDEngine::new(CRDTConfig::default());
        let config = engine.config_borrow();
        assert_eq!(config.max_ops_per_replica, 100000);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn crdt_full_integration_flow() {
        let engine = CRTDEngine::new(CRDTConfig::default());

        // Create replicas
        engine.create_replica(test_node_id(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id2(), CRDTType::GCounter).unwrap();
        engine.create_replica(test_node_id3(), CRDTType::GSet).unwrap();

        assert_eq!(engine.total_replica_count(), 3);

        // Apply operations
        engine.apply_op(test_node_id(), CRDTOp::new(vec![1, 2])).unwrap();
        engine.apply_op(test_node_id2(), CRDTOp::new(vec![3, 4])).unwrap();

        // Merge states
        engine.merge_states(test_node_id(), test_node_id2()).unwrap();

        // Check convergence
        let result = engine.check_convergence(&[test_node_id(), test_node_id2()]).unwrap();
        assert_eq!(result.total_replicas, 2);

        // Apply delta
        engine.apply_delta(test_node_id3(), CRDTDelta::new(vec![5])).unwrap();

        // Merge deltas
        engine.merge_deltas(test_node_id2(), test_node_id3()).unwrap();

        // Check health
        let report = engine.health_report();
        assert_eq!(report.total_replicas, 3);
        assert_eq!(report.total_ops, 2);
        assert!(report.is_healthy(0.5));

        // Final state
        let stats = engine.stats();
        assert_eq!(stats.total_replicas, 3);
        assert_eq!(stats.total_ops, 2);
        assert_eq!(stats.total_delta_ops, 1);
        assert_eq!(stats.total_state_merges, 2);
    }

    #[test]
    fn crdt_gcounter_integration() {
        let mut counter = GCounter::new();
        counter.increment(test_node_id(), 10);
        counter.increment(test_node_id2(), 20);
        assert_eq!(counter.value(), 30);
        assert_eq!(counter.get_count(test_node_id()), 10);
        assert_eq!(counter.get_count(test_node_id2()), 20);
    }

    #[test]
    fn crdt_pncounter_integration() {
        let mut counter = PNCounter::new();
        counter.increment(test_node_id(), 100);
        counter.decrement(test_node_id(), 30);
        counter.increment(test_node_id2(), 50);
        assert_eq!(counter.value(), 120);
        assert!(counter.is_positive());
    }

    #[test]
    fn crdt_lww_register_integration() {
        let mut reg = LWWRegister::new("value1".to_string(), 1000, test_node_id());
        reg.write("value2".to_string(), 2000, test_node_id2());
        assert_eq!(reg.get(), "value2");
        assert_eq!(reg.last_writer(), test_node_id2());
    }

    #[test]
    fn crdt_orset_integration() {
        let mut set = ORSet::new();
        set.add("a".to_string());
        set.add("b".to_string());
        set.remove(&"a".to_string());
        assert!(!set.contains(&"a".to_string()));
        assert!(set.contains(&"b".to_string()));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn crdt_error_serialization() {
        let err = CRDTError::ReplicaNotFound { replica_id: test_node_id() };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: CRDTError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, CRDTError::ReplicaNotFound { .. }));
    }

    #[test]
    fn crdt_convergence_trend_enum() {
        assert_eq!(ConvergenceTrend::Improving as u32, 0);
        assert_eq!(ConvergenceTrend::Declining as u32, 1);
        assert_eq!(ConvergenceTrend::Stable as u32, 2);
        assert_eq!(ConvergenceTrend::default(), ConvergenceTrend::Stable);
    }
}
