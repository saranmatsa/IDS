//! # ids-000-aa Sealed Trait Contract: Durability
//!
//! Distributed durability contract for IDS.
//! Provides durability condition tracking, replica acknowledgment counting,
//! cross-region replication coordination, checkpoint management, and
//! recovery verification. All types are canonical and sealed — only
//! ids-000-aa and its designated implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Durability messages are opaque
//! byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::errors::DurabilityError;
use crate::types::{
    CheckpointId, DatasetId, DurabilityCondition, DurabilityTrackState, Epoch, NodeId, OpRef,
    OrderingPos, Version,
};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Durability
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed durability tracking trait.
/// Implementations live downstream in ids-durability and ids-* crates.
pub trait Durability: private::Sealed {
    /// Configure durability parameters for a dataset.
    fn configure_durability(
        &self,
        dataset_id: DatasetId,
        config: DurabilityConfig,
    ) -> Result<(), DurabilityError>;

    /// Record that an operation has reached a durability condition.
    fn record_durability(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        condition: DurabilityCondition,
        node_id: NodeId,
    ) -> Result<DurabilityRecord, DurabilityError>;

    /// Get the current durability state for an operation.
    fn durability_state(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityTrackState, DurabilityError>;

    /// Get the highest durability condition reached for an operation.
    fn highest_condition(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityCondition, DurabilityError>;

    /// Check if an operation meets a required durability condition.
    fn meets_condition(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        required: DurabilityCondition,
    ) -> Result<bool, DurabilityError>;

    /// Register a replica acknowledgment for an operation.
    fn register_replica_ack(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        replica_node: NodeId,
    ) -> Result<ReplicaAckRecord, DurabilityError>;

    /// Get the number of replicas that have acknowledged an operation.
    fn replica_ack_count(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<u32, DurabilityError>;

    /// Get the set of replica nodes that have acknowledged an operation.
    fn acknowledging_replicas(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<Vec<NodeId>, DurabilityError>;

    /// Check if replica acknowledgment quorum is met for an operation.
    fn replica_quorum_met(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        required_acks: u32,
    ) -> Result<bool, DurabilityError>;

    /// Record cross-region replication progress for an operation.
    fn record_cross_region_progress(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        region_id: OrderingPos,
        condition: DurabilityCondition,
    ) -> Result<CrossRegionRecord, DurabilityError>;

    /// Get cross-region replication status for an operation.
    fn cross_region_status(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<CrossRegionStatus, DurabilityError>;

    /// Check if cross-region replication is complete for an operation.
    fn cross_region_complete(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<bool, DurabilityError>;

    /// Create a checkpoint for a dataset.
    fn create_checkpoint(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
        op_position: OrderingPos,
    ) -> Result<CheckpointRecord, DurabilityError>;

    /// Get checkpoint information for a dataset.
    fn checkpoint_info(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
    ) -> Result<CheckpointRecord, DurabilityError>;

    /// Get all checkpoints for a dataset.
    fn all_checkpoints(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Vec<CheckpointRecord>, DurabilityError>;

    /// Get the latest checkpoint for a dataset.
    fn latest_checkpoint(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Option<CheckpointRecord>, DurabilityError>;

    /// Delete a checkpoint for a dataset.
    fn delete_checkpoint(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
    ) -> Result<(), DurabilityError>;

    /// Verify durability integrity for an operation.
    fn verify_durability(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityVerification, DurabilityError>;

    /// Get durability statistics for a dataset.
    fn stats(
        &self,
        dataset_id: DatasetId,
    ) -> Result<DurabilityStats, DurabilityError>;

    /// Get all tracked datasets.
    fn datasets(&self) -> Vec<DatasetId>;

    /// Check if a dataset is tracked.
    fn is_tracked(&self, dataset_id: DatasetId) -> bool;

    /// Get the default durability condition for a dataset.
    fn default_condition(
        &self,
        dataset_id: DatasetId,
    ) -> Result<DurabilityCondition, DurabilityError>;

    /// Get the required replica ack count for a dataset.
    fn required_replica_acks(
        &self,
        dataset_id: DatasetId,
    ) -> Result<u32, DurabilityError>;

    /// Validate durability configuration.
    fn validate_config(
        &self,
        config: &DurabilityConfig,
    ) -> Result<(), DurabilityError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Durability configuration for a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityConfig {
    /// Default durability condition required for operations.
    pub default_condition: DurabilityCondition,
    /// Required number of replica acknowledgments.
    pub required_replica_acks: u32,
    /// Total number of replicas in the dataset.
    pub total_replicas: u32,
    /// Enable cross-region replication tracking.
    pub enable_cross_region: bool,
    /// Number of regions required for cross-region completion.
    pub required_regions: u32,
    /// Enable automatic checkpointing.
    pub enable_auto_checkpoint: bool,
    /// Checkpoint interval in operations.
    pub checkpoint_interval_ops: u64,
    /// Maximum checkpoints to retain per dataset.
    pub max_checkpoints: u32,
    /// Durability timeout in milliseconds.
    pub durability_timeout_ms: u64,
    /// Enable durability verification on read.
    pub enable_verify_on_read: bool,
}

impl DurabilityConfig {
    pub fn new(default_condition: DurabilityCondition) -> Result<Self, DurabilityError> {
        let config = Self {
            default_condition,
            required_replica_acks: 2,
            total_replicas: 3,
            enable_cross_region: true,
            required_regions: 2,
            enable_auto_checkpoint: true,
            checkpoint_interval_ops: 1000,
            max_checkpoints: 10,
            durability_timeout_ms: 300000,
            enable_verify_on_read: false,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), DurabilityError> {
        if self.required_replica_acks > self.total_replicas {
            return Err(DurabilityError::InvalidConfig {
                field: "required_replica_acks".to_string(),
                reason: "required_replica_acks cannot exceed total_replicas".to_string(),
            });
        }
        if self.total_replicas == 0 {
            return Err(DurabilityError::InvalidConfig {
                field: "total_replicas".to_string(),
                reason: "total_replicas must be greater than 0".to_string(),
            });
        }
        if self.required_regions == 0 && self.enable_cross_region {
            return Err(DurabilityError::InvalidConfig {
                field: "required_regions".to_string(),
                reason: "required_regions must be greater than 0 when cross-region is enabled".to_string(),
            });
        }
        if self.checkpoint_interval_ops == 0 {
            return Err(DurabilityError::InvalidConfig {
                field: "checkpoint_interval_ops".to_string(),
                reason: "checkpoint_interval_ops must be greater than 0".to_string(),
            });
        }
        if self.max_checkpoints == 0 {
            return Err(DurabilityError::InvalidConfig {
                field: "max_checkpoints".to_string(),
                reason: "max_checkpoints must be greater than 0".to_string(),
            });
        }
        if self.durability_timeout_ms == 0 {
            return Err(DurabilityError::InvalidConfig {
                field: "durability_timeout_ms".to_string(),
                reason: "durability_timeout_ms must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            default_condition: DurabilityCondition::ReplicasAcked,
            required_replica_acks: 2,
            total_replicas: 3,
            enable_cross_region: true,
            required_regions: 2,
            enable_auto_checkpoint: true,
            checkpoint_interval_ops: 1000,
            max_checkpoints: 10,
            durability_timeout_ms: 300000,
            enable_verify_on_read: false,
        }
    }

    pub fn strict() -> Self {
        Self {
            default_condition: DurabilityCondition::CrossRegionCompleted,
            required_replica_acks: 3,
            total_replicas: 5,
            enable_cross_region: true,
            required_regions: 3,
            enable_auto_checkpoint: true,
            checkpoint_interval_ops: 500,
            max_checkpoints: 20,
            durability_timeout_ms: 600000,
            enable_verify_on_read: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            default_condition: DurabilityCondition::Received,
            required_replica_acks: 1,
            total_replicas: 1,
            enable_cross_region: false,
            required_regions: 1,
            enable_auto_checkpoint: false,
            checkpoint_interval_ops: 10000,
            max_checkpoints: 5,
            durability_timeout_ms: 60000,
            enable_verify_on_read: false,
        }
    }

    pub fn ack_ratio(&self) -> f64 {
        if self.total_replicas == 0 {
            return 0.0;
        }
        self.required_replica_acks as f64 / self.total_replicas as f64
    }

    pub fn region_ratio(&self) -> f64 {
        if self.required_regions == 0 {
            return 0.0;
        }
        1.0
    }
}

impl Default for DurabilityConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY RECORD
///////////////////////////////////////////////////////////////////////////////

/// Record of a durability condition update for an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityRecord {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Operation reference.
    pub op_ref: OpRef,
    /// Durability condition reached.
    pub condition: DurabilityCondition,
    /// Node that recorded the condition.
    pub node_id: NodeId,
    /// Timestamp when condition was reached.
    pub recorded_at: u64,
    /// Condition level (numeric).
    pub level: u32,
}

impl DurabilityRecord {
    pub fn new(
        dataset_id: DatasetId,
        op_ref: OpRef,
        condition: DurabilityCondition,
        node_id: NodeId,
        recorded_at: u64,
    ) -> Self {
        Self {
            dataset_id,
            op_ref,
            condition,
            node_id,
            recorded_at,
            level: condition.level(),
        }
    }

    pub fn meets(&self, required: DurabilityCondition) -> bool {
        self.condition.meets(required)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.condition,
            DurabilityCondition::CrossRegionCompleted
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA ACK RECORD
///////////////////////////////////////////////////////////////////////////////

/// Record of a replica acknowledgment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaAckRecord {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Operation reference.
    pub op_ref: OpRef,
    /// Replica node that acknowledged.
    pub replica_node: NodeId,
    /// Timestamp of acknowledgment.
    pub acked_at: u64,
    /// Acknowledgment sequence number.
    pub ack_sequence: u64,
}

impl ReplicaAckRecord {
    pub fn new(
        dataset_id: DatasetId,
        op_ref: OpRef,
        replica_node: NodeId,
        acked_at: u64,
        ack_sequence: u64,
    ) -> Self {
        Self {
            dataset_id,
            op_ref,
            replica_node,
            acked_at,
            ack_sequence,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS REGION RECORD
///////////////////////////////////////////////////////////////////////////////

/// Record of cross-region replication progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRegionRecord {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Operation reference.
    pub op_ref: OpRef,
    /// Region position.
    pub region_id: OrderingPos,
    /// Durability condition in that region.
    pub condition: DurabilityCondition,
    /// Timestamp of replication.
    pub replicated_at: u64,
}

impl CrossRegionRecord {
    pub fn new(
        dataset_id: DatasetId,
        op_ref: OpRef,
        region_id: OrderingPos,
        condition: DurabilityCondition,
        replicated_at: u64,
    ) -> Self {
        Self {
            dataset_id,
            op_ref,
            region_id,
            condition,
            replicated_at,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS REGION STATUS
///////////////////////////////////////////////////////////////////////////////

/// Status of cross-region replication for an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRegionStatus {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Operation reference.
    pub op_ref: OpRef,
    /// Regions that have replicated the operation.
    pub replicated_regions: Vec<OrderingPos>,
    /// Total regions required.
    pub required_regions: u32,
    /// Is cross-region replication complete.
    pub complete: bool,
    /// Highest condition across all regions.
    pub highest_condition: DurabilityCondition,
}

impl CrossRegionStatus {
    pub fn completion_percent(&self) -> f64 {
        if self.required_regions == 0 {
            return 100.0;
        }
        self.replicated_regions.len() as f64 / self.required_regions as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHECKPOINT RECORD
///////////////////////////////////////////////////////////////////////////////

/// Record of a dataset checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Checkpoint identifier.
    pub checkpoint_id: CheckpointId,
    /// Operation position at checkpoint.
    pub op_position: OrderingPos,
    /// Timestamp of checkpoint creation.
    pub created_at: u64,
    /// Number of operations covered.
    pub ops_covered: u64,
    /// Checkpoint size in bytes.
    pub size_bytes: u64,
    /// Checkpoint checksum.
    pub checksum: Vec<u8>,
}

impl CheckpointRecord {
    pub fn new(
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
        op_position: OrderingPos,
        created_at: u64,
        ops_covered: u64,
        size_bytes: u64,
        checksum: Vec<u8>,
    ) -> Self {
        Self {
            dataset_id,
            checkpoint_id,
            op_position,
            created_at,
            ops_covered,
            size_bytes,
            checksum,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops_covered == 0 && self.size_bytes == 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY VERIFICATION
///////////////////////////////////////////////////////////////////////////////

/// Result of verifying durability for an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityVerification {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Operation reference.
    pub op_ref: OpRef,
    /// Is durability verified.
    pub verified: bool,
    /// Current condition.
    pub current_condition: DurabilityCondition,
    /// Required condition.
    pub required_condition: DurabilityCondition,
    /// Replica ack count.
    pub replica_ack_count: u32,
    /// Required replica acks.
    pub required_replica_acks: u32,
    /// Cross-region complete.
    pub cross_region_complete: bool,
    /// Verification timestamp.
    pub verified_at: u64,
    /// Verification details.
    pub details: String,
}

impl DurabilityVerification {
    pub fn meets_all_requirements(&self) -> bool {
        self.verified
            && self.current_condition.meets(self.required_condition)
            && self.replica_ack_count >= self.required_replica_acks
            && self.cross_region_complete
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY STATS
///////////////////////////////////////////////////////////////////////////////

/// Statistics for durability tracking of a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityStats {
    /// Dataset ID.
    pub dataset_id: DatasetId,
    /// Total operations tracked.
    pub total_ops: u64,
    /// Operations meeting default condition.
    pub ops_meeting_condition: u64,
    /// Operations at CrossRegionCompleted.
    pub ops_cross_region_complete: u64,
    /// Total replica acknowledgments recorded.
    pub total_replica_acks: u64,
    /// Total cross-region records.
    pub total_cross_region_records: u64,
    /// Total checkpoints created.
    pub total_checkpoints: u64,
    /// Total durability verifications.
    pub total_verifications: u64,
    /// Failed verifications.
    pub failed_verifications: u64,
    /// Average time to reach default condition (ms).
    pub avg_time_to_condition_ms: f64,
    /// Durability coverage percent.
    pub coverage_percent: f64,
}

impl DurabilityStats {
    pub fn new(dataset_id: DatasetId) -> Self {
        Self {
            dataset_id,
            total_ops: 0,
            ops_meeting_condition: 0,
            ops_cross_region_complete: 0,
            total_replica_acks: 0,
            total_cross_region_records: 0,
            total_checkpoints: 0,
            total_verifications: 0,
            failed_verifications: 0,
            avg_time_to_condition_ms: 0.0,
            coverage_percent: 0.0,
        }
    }

    pub fn condition_meet_rate(&self) -> f64 {
        if self.total_ops == 0 {
            return 100.0;
        }
        self.ops_meeting_condition as f64 / self.total_ops as f64 * 100.0
    }

    pub fn cross_region_complete_rate(&self) -> f64 {
        if self.total_ops == 0 {
            return 100.0;
        }
        self.ops_cross_region_complete as f64 / self.total_ops as f64 * 100.0
    }

    pub fn verification_failure_rate(&self) -> f64 {
        if self.total_verifications == 0 {
            return 0.0;
        }
        self.failed_verifications as f64 / self.total_verifications as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY ERROR
///////////////////////////////////////////////////////////////////////////////

/// Durability tracking errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum DurabilityError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("operation not found: {op_ref:?}")]
    OperationNotFound { op_ref: OpRef },

    #[error("checkpoint not found: {checkpoint_id:?}")]
    CheckpointNotFound { checkpoint_id: CheckpointId },

    #[error("durability condition not met: current {current:?}, required {required:?}")]
    ConditionNotMet {
        current: DurabilityCondition,
        required: DurabilityCondition,
    },

    #[error("replica quorum not met: {acks}/{required}")]
    ReplicaQuorumNotMet { acks: u32, required: u32 },

    #[error("cross-region not complete: {regions}/{required}")]
    CrossRegionNotComplete { regions: u32, required: u32 },

    #[error("checkpoint limit exceeded: {max_checkpoints}")]
    CheckpointLimitExceeded { max_checkpoints: u32 },

    #[error("duplicate replica ack: {node_id:?}")]
    DuplicateReplicaAck { node_id: NodeId },

    #[error("invalid condition transition: {from:?} -> {to:?}")]
    InvalidConditionTransition {
        from: DurabilityCondition,
        to: DurabilityCondition,
    },

    #[error("durability timeout: {op_ref:?}")]
    DurabilityTimeout { op_ref: OpRef },

    #[error("verification failed: {reason}")]
    VerificationFailed { reason: String },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl DurabilityError {
    pub fn category(&self) -> &'static str {
        match self {
            DurabilityError::InvalidConfig { .. } => "invalid_config",
            DurabilityError::DatasetNotFound { .. } => "dataset_not_found",
            DurabilityError::OperationNotFound { .. } => "operation_not_found",
            DurabilityError::CheckpointNotFound { .. } => "checkpoint_not_found",
            DurabilityError::ConditionNotMet { .. } => "condition_not_met",
            DurabilityError::ReplicaQuorumNotMet { .. } => "replica_quorum_not_met",
            DurabilityError::CrossRegionNotComplete { .. } => "cross_region_not_complete",
            DurabilityError::CheckpointLimitExceeded { .. } => "checkpoint_limit_exceeded",
            DurabilityError::DuplicateReplicaAck { .. } => "duplicate_replica_ack",
            DurabilityError::InvalidConditionTransition { .. } => "invalid_condition_transition",
            DurabilityError::DurabilityTimeout { .. } => "durability_timeout",
            DurabilityError::VerificationFailed { .. } => "verification_failed",
            DurabilityError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            DurabilityError::ConditionNotMet { .. }
                | DurabilityError::ReplicaQuorumNotMet { .. }
                | DurabilityError::CrossRegionNotComplete { .. }
                | DurabilityError::DurabilityTimeout { .. }
                | DurabilityError::VerificationFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            DurabilityError::InvalidConfig { .. } => "correct the durability configuration",
            DurabilityError::DatasetNotFound { .. } => "create the dataset first",
            DurabilityError::OperationNotFound { .. } => "register the operation first",
            DurabilityError::CheckpointNotFound { .. } => "create the checkpoint first",
            DurabilityError::ConditionNotMet { .. } => "wait for operation to reach required condition",
            DurabilityError::ReplicaQuorumNotMet { .. } => "wait for more replica acknowledgments",
            DurabilityError::CrossRegionNotComplete { .. } => "wait for cross-region replication",
            DurabilityError::CheckpointLimitExceeded { .. } => "delete old checkpoints",
            DurabilityError::DuplicateReplicaAck { .. } => "ignore duplicate ack",
            DurabilityError::InvalidConditionTransition { .. } => "check condition ordering",
            DurabilityError::DurabilityTimeout { .. } => "increase timeout or check replica health",
            DurabilityError::VerificationFailed { .. } => "re-verify operation durability",
            DurabilityError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// OPERATION DURABILITY STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal state tracking for an operation's durability.
#[derive(Debug, Clone)]
struct OpDurabilityState {
    op_ref: OpRef,
    current_condition: DurabilityCondition,
    condition_reached_at: u64,
    replica_acks: HashSet<NodeId>,
    ack_sequence: u64,
    cross_region_records: Vec<CrossRegionRecord>,
    verified: bool,
    verification_count: u32,
}

impl OpDurabilityState {
    pub fn new(op_ref: OpRef) -> Self {
        Self {
            op_ref,
            current_condition: DurabilityCondition::Received,
            condition_reached_at: 0,
            replica_acks: HashSet::new(),
            ack_sequence: 0,
            cross_region_records: Vec::new(),
            verified: false,
            verification_count: 0,
        }
    }

    pub fn meets_condition(&self, required: DurabilityCondition) -> bool {
        self.current_condition.meets(required)
    }

    pub fn ack_count(&self) -> u32 {
        self.replica_acks.len() as u32
    }

    pub fn has_cross_region(&self, region: OrderingPos) -> bool {
        self.cross_region_records.iter().any(|r| r.region_id == region)
    }

    pub fn cross_region_complete(&self, required_regions: u32) -> bool {
        let regions: HashSet<OrderingPos> =
            self.cross_region_records.iter().map(|r| r.region_id).collect();
        regions.len() as u32 >= required_regions
    }

    pub fn highest_cross_region_condition(&self) -> DurabilityCondition {
        self.cross_region_records
            .iter()
            .map(|r| r.condition)
            .max()
            .unwrap_or(DurabilityCondition::Received)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DATASET DURABILITY STATE
///////////////////////////////////////////////////////////////////////////////

/// Internal state for a dataset's durability tracking.
#[derive(Debug, Clone)]
struct DatasetDurabilityState {
    dataset_id: DatasetId,
    config: DurabilityConfig,
    ops: HashMap<OpRef, OpDurabilityState>,
    checkpoints: HashMap<CheckpointId, CheckpointRecord>,
    stats: DurabilityStats,
    next_checkpoint_op: u64,
}

impl DatasetDurabilityState {
    pub fn new(dataset_id: DatasetId, config: DurabilityConfig) -> Self {
        Self {
            dataset_id,
            config,
            ops: HashMap::new(),
            checkpoints: HashMap::new(),
            stats: DurabilityStats::new(dataset_id),
            next_checkpoint_op: config.checkpoint_interval_ops,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference durability engine implementing the Durability trait.
pub struct DurabilityEngine {
    config: std::cell::RefCell<DurabilityConfig>,
    datasets: std::cell::RefCell<HashMap<DatasetId, DatasetDurabilityState>>,
    global_clock: std::cell::RefCell<u64>,
}

impl DurabilityEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(DurabilityConfig::default()),
            datasets: std::cell::RefCell::new(HashMap::new()),
            global_clock: std::cell::RefCell::new(0),
        }
    }

    pub fn with_config(config: DurabilityConfig) -> Result<Self, DurabilityError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            datasets: std::cell::RefCell::new(HashMap::new()),
            global_clock: std::cell::RefCell::new(0),
        })
    }

    fn ensure_dataset(&self, dataset_id: DatasetId) {
        let mut datasets = self.datasets.borrow_mut();
        if !datasets.contains_key(&dataset_id) {
            let config = self.config.borrow().clone();
            datasets.insert(dataset_id, DatasetDurabilityState::new(dataset_id, config));
        }
    }

    fn advance_clock(&self) -> u64 {
        let mut clock = self.global_clock.borrow_mut();
        *clock += 1;
        *clock
    }

    fn get_clock(&self) -> u64 {
        *self.global_clock.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL DURABILITY FOR DURABILITYENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for DurabilityEngine {}

impl Durability for DurabilityEngine {
    fn configure_durability(
        &self,
        dataset_id: DatasetId,
        config: DurabilityConfig,
    ) -> Result<(), DurabilityError> {
        config.validate()?;
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        if let Some(state) = datasets.get_mut(&dataset_id) {
            state.config = config;
            state.stats = DurabilityStats::new(dataset_id);
            state.ops.clear();
            state.checkpoints.clear();
            state.next_checkpoint_op = config.checkpoint_interval_ops;
        }
        Ok(())
    }

    fn record_durability(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        condition: DurabilityCondition,
        node_id: NodeId,
    ) -> Result<DurabilityRecord, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .entry(op_ref)
            .or_insert_with(|| OpDurabilityState::new(op_ref));

        // Validate condition transition (monotonically increasing)
        if condition.level() < op_state.current_condition.level() {
            return Err(DurabilityError::InvalidConditionTransition {
                from: op_state.current_condition,
                to: condition,
            });
        }

        let now = self.advance_clock();
        op_state.current_condition = condition;
        op_state.condition_reached_at = now;

        // Update stats
        state.stats.total_ops += 1;
        if condition.meets(state.config.default_condition) {
            state.stats.ops_meeting_condition += 1;
        }
        if condition == DurabilityCondition::CrossRegionCompleted {
            state.stats.ops_cross_region_complete += 1;
        }

        // Auto-checkpoint
        if state.config.enable_auto_checkpoint {
            let ops_count = state.ops.len() as u64;
            if ops_count >= state.next_checkpoint_op {
                state.next_checkpoint_op += state.config.checkpoint_interval_ops;
                // Checkpoint creation handled separately via create_checkpoint
            }
        }

        let record = DurabilityRecord::new(dataset_id, op_ref, condition, node_id, now);
        Ok(record)
    }

    fn durability_state(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityTrackState, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        Ok(match op_state.current_condition {
            DurabilityCondition::Received => DurabilityTrackState::NotStarted,
            DurabilityCondition::Accepted | DurabilityCondition::Executed => {
                DurabilityTrackState::InProgress
            }
            DurabilityCondition::LocalCommitted | DurabilityCondition::ReplicasAcked => {
                DurabilityTrackState::ReplicasAcked
            }
            DurabilityCondition::CrossRegionCompleted => DurabilityTrackState::CrossRegionCompleted,
        })
    }

    fn highest_condition(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityCondition, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        Ok(op_state.current_condition)
    }

    fn meets_condition(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        required: DurabilityCondition,
    ) -> Result<bool, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        Ok(op_state.meets_condition(required))
    }

    fn register_replica_ack(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        replica_node: NodeId,
    ) -> Result<ReplicaAckRecord, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get_mut(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        if op_state.replica_acks.contains(&replica_node) {
            return Err(DurabilityError::DuplicateReplicaAck {
                node_id: replica_node,
            });
        }

        op_state.ack_sequence += 1;
        op_state.replica_acks.insert(replica_node);

        let now = self.advance_clock();
        state.stats.total_replica_acks += 1;

        let record = ReplicaAckRecord::new(
            dataset_id,
            op_ref,
            replica_node,
            now,
            op_state.ack_sequence,
        );
        Ok(record)
    }

    fn replica_ack_count(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<u32, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        Ok(op_state.ack_count())
    }

    fn acknowledging_replicas(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<Vec<NodeId>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        Ok(op_state.replica_acks.iter().cloned().collect())
    }

    fn replica_quorum_met(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        required_acks: u32,
    ) -> Result<bool, DurabilityError> {
        let count = self.replica_ack_count(dataset_id, op_ref)?;
        Ok(count >= required_acks)
    }

    fn record_cross_region_progress(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        region_id: OrderingPos,
        condition: DurabilityCondition,
    ) -> Result<CrossRegionRecord, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get_mut(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        // Check for duplicate region
        if op_state.has_cross_region(region_id) {
            return Err(DurabilityError::Internal {
                message: format!("region {region_id:?} already recorded for operation"),
            });
        }

        let now = self.advance_clock();
        let record = CrossRegionRecord::new(dataset_id, op_ref, region_id, condition, now);
        op_state.cross_region_records.push(record.clone());
        state.stats.total_cross_region_records += 1;

        Ok(record)
    }

    fn cross_region_status(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<CrossRegionStatus, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        let regions: Vec<OrderingPos> =
            op_state.cross_region_records.iter().map(|r| r.region_id).collect();
        let complete = op_state.cross_region_complete(state.config.required_regions);
        let highest = op_state.highest_cross_region_condition();

        Ok(CrossRegionStatus {
            dataset_id,
            op_ref,
            replicated_regions: regions,
            required_regions: state.config.required_regions,
            complete,
            highest_condition: highest,
        })
    }

    fn cross_region_complete(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<bool, DurabilityError> {
        let status = self.cross_region_status(dataset_id, op_ref)?;
        Ok(status.complete)
    }

    fn create_checkpoint(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
        op_position: OrderingPos,
    ) -> Result<CheckpointRecord, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        if state.checkpoints.contains_key(&checkpoint_id) {
            return Err(DurabilityError::Internal {
                message: "checkpoint already exists".to_string(),
            });
        }

        if state.checkpoints.len() >= state.config.max_checkpoints as usize {
            return Err(DurabilityError::CheckpointLimitExceeded {
                max_checkpoints: state.config.max_checkpoints,
            });
        }

        let now = self.advance_clock();
        let record = CheckpointRecord::new(
            dataset_id,
            checkpoint_id,
            op_position,
            now,
            state.ops.len() as u64,
            0,
            Vec::new(),
        );

        state.checkpoints.insert(checkpoint_id, record.clone());
        state.stats.total_checkpoints += 1;

        Ok(record)
    }

    fn checkpoint_info(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
    ) -> Result<CheckpointRecord, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        state
            .checkpoints
            .get(&checkpoint_id)
            .cloned()
            .ok_or_else(|| DurabilityError::CheckpointNotFound { checkpoint_id })
    }

    fn all_checkpoints(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Vec<CheckpointRecord>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        Ok(state.checkpoints.values().cloned().collect())
    }

    fn latest_checkpoint(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Option<CheckpointRecord>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let latest = state
            .checkpoints
            .values()
            .max_by_key(|c| c.created_at)
            .cloned();
        Ok(latest)
    }

    fn delete_checkpoint(
        &self,
        dataset_id: DatasetId,
        checkpoint_id: CheckpointId,
    ) -> Result<(), DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        if state.checkpoints.remove(&checkpoint_id).is_none() {
            return Err(DurabilityError::CheckpointNotFound { checkpoint_id });
        }
        Ok(())
    }

    fn verify_durability(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
    ) -> Result<DurabilityVerification, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        let now = self.advance_clock();
        let required = state.config.default_condition;
        let condition_met = op_state.meets_condition(required);
        let replica_quorum_met =
            op_state.ack_count() >= state.config.required_replica_acks;
        let cross_region_complete =
            op_state.cross_region_complete(state.config.required_regions);

        let verified = condition_met && replica_quorum_met && cross_region_complete;

        if verified {
            op_state.verified = true;
        }

        state.stats.total_verifications += 1;
        if !verified {
            state.stats.failed_verifications += 1;
        }

        let details = format!(
            "condition: {:?}, acks: {}/{}, cross_region: {}",
            op_state.current_condition,
            op_state.ack_count(),
            state.config.required_replica_acks,
            cross_region_complete
        );

        Ok(DurabilityVerification {
            dataset_id,
            op_ref,
            verified,
            current_condition: op_state.current_condition,
            required_condition: required,
            replica_ack_count: op_state.ack_count(),
            required_replica_acks: state.config.required_replica_acks,
            cross_region_complete,
            verified_at: now,
            details,
        })
    }

    fn stats(&self, dataset_id: DatasetId) -> Result<DurabilityStats, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut stats = state.stats.clone();
        if state.stats.total_ops > 0 {
            stats.coverage_percent =
                state.stats.ops_meeting_condition as f64 / state.stats.total_ops as f64 * 100.0;
        }
        Ok(stats)
    }

    fn datasets(&self) -> Vec<DatasetId> {
        let datasets = self.datasets.borrow();
        datasets.keys().cloned().collect()
    }

    fn is_tracked(&self, dataset_id: DatasetId) -> bool {
        let datasets = self.datasets.borrow();
        datasets.contains_key(&dataset_id)
    }

    fn default_condition(
        &self,
        dataset_id: DatasetId,
    ) -> Result<DurabilityCondition, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;
        Ok(state.config.default_condition)
    }

    fn required_replica_acks(
        &self,
        dataset_id: DatasetId,
    ) -> Result<u32, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;
        Ok(state.config.required_replica_acks)
    }

    fn validate_config(
        &self,
        config: &DurabilityConfig,
    ) -> Result<(), DurabilityError> {
        config.validate()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADVANCED DURABILITY ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

/// Checkpoint lifecycle manager for durability.
#[derive(Debug, Clone)]
pub struct CheckpointLifecycle {
    pub dataset_id: DatasetId,
    pub auto_checkpoint_enabled: bool,
    pub checkpoint_interval_ops: u64,
    pub max_checkpoints: u32,
    pub checkpoints_created: u64,
    pub checkpoints_deleted: u64,
    pub last_checkpoint_op: u64,
}

impl CheckpointLifecycle {
    pub fn new(dataset_id: DatasetId, config: &DurabilityConfig) -> Self {
        Self {
            dataset_id,
            auto_checkpoint_enabled: config.enable_auto_checkpoint,
            checkpoint_interval_ops: config.checkpoint_interval_ops,
            max_checkpoints: config.max_checkpoints,
            checkpoints_created: 0,
            checkpoints_deleted: 0,
            last_checkpoint_op: 0,
        }
    }

    pub fn should_checkpoint(&self, ops_since_last: u64) -> bool {
        if !self.auto_checkpoint_enabled {
            return false;
        }
        ops_since_last >= self.checkpoint_interval_ops
    }

    pub fn record_created(&mut self) {
        self.checkpoints_created += 1;
    }

    pub fn record_deleted(&mut self) {
        self.checkpoints_deleted += 1;
    }

    pub fn net_checkpoints(&self) -> u64 {
        self.checkpoints_created.saturating_sub(self.checkpoints_deleted)
    }
}

/// Durability recovery result for an operation after failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityRecoveryResult {
    pub dataset_id: DatasetId,
    pub op_ref: OpRef,
    pub recovered: bool,
    pub recovered_condition: DurabilityCondition,
    pub recovery_source: RecoverySource,
    pub recovered_at: u64,
}

/// Source of durability recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoverySource {
    LocalLog,
    Replica,
    CrossRegion,
    Checkpoint,
}

impl RecoverySource {
    pub fn display_name(&self) -> &'static str {
        match self {
            RecoverySource::LocalLog => "local_log",
            RecoverySource::Replica => "replica",
            RecoverySource::CrossRegion => "cross_region",
            RecoverySource::Checkpoint => "checkpoint",
        }
    }
}

/// Durability advancement request for batch operations.
#[derive(Debug, Clone)]
pub struct DurabilityAdvanceRequest {
    pub dataset_id: DatasetId,
    pub op_refs: Vec<OpRef>,
    pub target_condition: DurabilityCondition,
    pub node_id: NodeId,
    pub timestamp_ms: u64,
}

impl DurabilityAdvanceRequest {
    pub fn new(
        dataset_id: DatasetId,
        op_refs: Vec<OpRef>,
        target_condition: DurabilityCondition,
        node_id: NodeId,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            dataset_id,
            op_refs,
            target_condition,
            node_id,
            timestamp_ms,
        }
    }

    pub fn op_count(&self) -> usize {
        self.op_refs.len()
    }
}

/// Durability advancement result for batch operations.
#[derive(Debug, Clone)]
pub struct DurabilityAdvanceResult {
    pub request: DurabilityAdvanceRequest,
    pub advanced_count: u32,
    pub failed_count: u32,
    pub failures: Vec<(OpRef, DurabilityError)>,
    pub completed_at: u64,
}

impl DurabilityAdvanceResult {
    pub fn success_rate(&self) -> f64 {
        let total = self.advanced_count + self.failed_count;
        if total == 0 {
            return 100.0;
        }
        self.advanced_count as f64 / total as f64 * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.failed_count == 0
    }
}

/// Durability condition distribution across operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionDistribution {
    pub received: u64,
    pub accepted: u64,
    pub executed: u64,
    pub local_committed: u64,
    pub replicas_acked: u64,
    pub cross_region_completed: u64,
}

impl ConditionDistribution {
    pub fn total(&self) -> u64 {
        self.received
            + self.accepted
            + self.executed
            + self.local_committed
            + self.replicas_acked
            + self.cross_region_completed
    }

    pub fn percent_at_or_above(&self, condition: DurabilityCondition) -> f64 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        let count = match condition {
            DurabilityCondition::Received => self.total(),
            DurabilityCondition::Accepted => {
                self.accepted + self.executed + self.local_committed + self.replicas_acked + self.cross_region_completed
            }
            DurabilityCondition::Executed => {
                self.executed + self.local_committed + self.replicas_acked + self.cross_region_completed
            }
            DurabilityCondition::LocalCommitted => {
                self.local_committed + self.replicas_acked + self.cross_region_completed
            }
            DurabilityCondition::ReplicasAcked => self.replicas_acked + self.cross_region_completed,
            DurabilityCondition::CrossRegionCompleted => self.cross_region_completed,
        };
        count as f64 / total as f64 * 100.0
    }
}

/// Durability health summary for a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurabilityHealth {
    pub dataset_id: DatasetId,
    pub total_ops: u64,
    pub healthy_ops: u64,
    pub degraded_ops: u64,
    pub failed_ops: u64,
    pub health_percent: f64,
    pub avg_condition_level: f64,
}

impl DurabilityHealth {
    pub fn is_healthy(&self, threshold: f64) -> bool {
        self.health_percent >= threshold
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADVANCED DURABILITY ENGINE IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////

impl DurabilityEngine {
    /// Get checkpoint lifecycle for a dataset.
    pub fn checkpoint_lifecycle(
        &self,
        dataset_id: DatasetId,
    ) -> Result<CheckpointLifecycle, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;
        Ok(CheckpointLifecycle::new(dataset_id, &state.config))
    }

    /// Recover an operation from a specified source.
    pub fn recover_operation(
        &self,
        dataset_id: DatasetId,
        op_ref: OpRef,
        source: RecoverySource,
    ) -> Result<DurabilityRecoveryResult, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let op_state = state
            .ops
            .get_mut(&op_ref)
            .ok_or_else(|| DurabilityError::OperationNotFound { op_ref })?;

        let now = self.advance_clock();
        let recovered_condition = match source {
            RecoverySource::LocalLog => op_state.current_condition,
            RecoverySource::Replica => {
                if op_state.ack_count() > 0 {
                    DurabilityCondition::ReplicasAcked
                } else {
                    op_state.current_condition
                }
            }
            RecoverySource::CrossRegion => op_state.highest_cross_region_condition(),
            RecoverySource::Checkpoint => DurabilityCondition::LocalCommitted,
        };

        op_state.current_condition = recovered_condition;
        op_state.verified = true;
        op_state.verification_count += 1;

        state.stats.total_verifications += 1;

        Ok(DurabilityRecoveryResult {
            dataset_id,
            op_ref,
            recovered: true,
            recovered_condition,
            recovery_source: source,
            recovered_at: now,
        })
    }

    /// Advance multiple operations to a target condition in batch.
    pub fn advance_batch(
        &self,
        request: DurabilityAdvanceRequest,
    ) -> Result<DurabilityAdvanceResult, DurabilityError> {
        self.ensure_dataset(request.dataset_id);
        let mut datasets = self.datasets.borrow_mut();
        let state = datasets
            .get_mut(&request.dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id: request.dataset_id })?;

        let mut advanced_count = 0u32;
        let mut failed_count = 0u32;
        let mut failures = Vec::new();
        let now = self.advance_clock();

        for op_ref in &request.op_refs {
            match state.ops.get_mut(op_ref) {
                Some(op_state) => {
                    if request.target_condition.level() > op_state.current_condition.level() {
                        op_state.current_condition = request.target_condition;
                        op_state.condition_reached_at = now;
                        advanced_count += 1;
                    }
                }
                None => {
                    failed_count += 1;
                    failures.push((
                        *op_ref,
                        DurabilityError::OperationNotFound { op_ref: *op_ref },
                    ));
                }
            }
        }

        state.stats.total_ops += advanced_count as u64;
        state.stats.ops_meeting_condition += advanced_count as u64;

        Ok(DurabilityAdvanceResult {
            request,
            advanced_count,
            failed_count,
            failures,
            completed_at: now,
        })
    }

    /// Get condition distribution for a dataset.
    pub fn condition_distribution(
        &self,
        dataset_id: DatasetId,
    ) -> Result<ConditionDistribution, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut dist = ConditionDistribution {
            received: 0,
            accepted: 0,
            executed: 0,
            local_committed: 0,
            replicas_acked: 0,
            cross_region_completed: 0,
        };

        for op_state in state.ops.values() {
            match op_state.current_condition {
                DurabilityCondition::Received => dist.received += 1,
                DurabilityCondition::Accepted => dist.accepted += 1,
                DurabilityCondition::Executed => dist.executed += 1,
                DurabilityCondition::LocalCommitted => dist.local_committed += 1,
                DurabilityCondition::ReplicasAcked => dist.replicas_acked += 1,
                DurabilityCondition::CrossRegionCompleted => dist.cross_region_completed += 1,
            }
        }

        Ok(dist)
    }

    /// Get durability health for a dataset.
    pub fn durability_health(
        &self,
        dataset_id: DatasetId,
        threshold: f64,
    ) -> Result<DurabilityHealth, DurabilityError> {
        let stats = self.stats(dataset_id)?;
        let dist = self.condition_distribution(dataset_id)?;

        let total = dist.total();
        let healthy = dist.replicas_acked + dist.cross_region_completed;
        let degraded = dist.local_committed;
        let failed = dist.received + dist.accepted + dist.executed;

        let health_percent = if total == 0 {
            100.0
        } else {
            healthy as f64 / total as f64 * 100.0
        };

        let avg_condition_level = if total == 0 {
            0.0
        } else {
            (dist.received * 1
                + dist.accepted * 2
                + dist.executed * 3
                + dist.local_committed * 4
                + dist.replicas_acked * 5
                + dist.cross_region_completed * 6) as f64
                / total as f64
        };

        Ok(DurabilityHealth {
            dataset_id,
            total_ops: stats.total_ops,
            healthy_ops: healthy,
            degraded_ops: degraded,
            failed_ops: failed,
            health_percent,
            avg_condition_level,
        })
    }

    /// Get operations below a durability condition.
    pub fn operations_below_condition(
        &self,
        dataset_id: DatasetId,
        condition: DurabilityCondition,
    ) -> Result<Vec<OpRef>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut below = Vec::new();
        for (op_ref, op_state) in state.ops.iter() {
            if !op_state.meets_condition(condition) {
                below.push(*op_ref);
            }
        }
        Ok(below)
    }

    /// Get operations at or above a durability condition.
    pub fn operations_at_or_above(
        &self,
        dataset_id: DatasetId,
        condition: DurabilityCondition,
    ) -> Result<Vec<OpRef>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut at_or_above = Vec::new();
        for (op_ref, op_state) in state.ops.iter() {
            if op_state.meets_condition(condition) {
                at_or_above.push(*op_ref);
            }
        }
        Ok(at_or_above)
    }

    /// Get total operations across all datasets.
    pub fn total_ops_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.stats.total_ops).sum()
    }

    /// Get total checkpoints across all datasets.
    pub fn total_checkpoints_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.checkpoints.len() as u64).sum()
    }

    /// Get total replica acks across all datasets.
    pub fn total_replica_acks_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.stats.total_replica_acks).sum()
    }

    /// Get total cross-region records across all datasets.
    pub fn total_cross_region_records_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.stats.total_cross_region_records).sum()
    }

    /// Get total verifications across all datasets.
    pub fn total_verifications_all_datasets(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.values().map(|s| s.stats.total_verifications).sum()
    }

    /// Get dataset count.
    pub fn dataset_count(&self) -> u64 {
        let datasets = self.datasets.borrow();
        datasets.len() as u64
    }

    /// Clear all durability state (for testing/reset).
    pub fn clear_all(&self) {
        self.datasets.borrow_mut().clear();
    }

    /// Get config clone.
    pub fn config_clone(&self) -> DurabilityConfig {
        self.config.borrow().clone()
    }

    /// Advance the global clock manually.
    pub fn advance_clock_ms(&self, ms: u64) -> u64 {
        let mut clock = self.global_clock.borrow_mut();
        *clock += ms;
        *clock
    }

    /// Get current clock time.
    pub fn current_clock(&self) -> u64 {
        *self.global_clock.borrow()
    }

    /// Check if any operation has timed out (not reached condition within timeout).
    pub fn timed_out_operations(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Vec<OpRef>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let now = self.get_clock();
        let timeout = state.config.durability_timeout_ms;

        let mut timed_out = Vec::new();
        for (op_ref, op_state) in state.ops.iter() {
            if !op_state.meets_condition(state.config.default_condition) {
                let elapsed = now.saturating_sub(op_state.condition_reached_at);
                if elapsed > timeout {
                    timed_out.push(*op_ref);
                }
            }
        }
        Ok(timed_out)
    }

    /// Get operations pending cross-region completion.
    pub fn pending_cross_region(
        &self,
        dataset_id: DatasetId,
    ) -> Result<Vec<OpRef>, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut pending = Vec::new();
        for (op_ref, op_state) in state.ops.iter() {
            if !op_state.cross_region_complete(state.config.required_regions) {
                pending.push(*op_ref);
            }
        }
        Ok(pending)
    }

    /// Get durability summary for all datasets.
    pub fn all_dataset_summaries(&self) -> Vec<(DatasetId, DurabilityStats)> {
        let datasets = self.datasets.borrow();
        datasets
            .iter()
            .map(|(id, state)| (*id, state.stats.clone()))
            .collect()
    }

    /// Get durability verification trend for a dataset over time.
    pub fn verification_trend(&self, dataset_id: DatasetId) -> Result<VerificationTrend, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let total_ops = state.stats.total_ops;
        let verified_ops = state.stats.total_verifications;
        let failed_ops = state.stats.failed_verifications;

        Ok(VerificationTrend {
            dataset_id,
            total_operations: total_ops,
            verified_operations: verified_ops,
            failed_verifications: failed_ops,
            verification_rate: if total_ops > 0 {
                verified_ops as f64 / total_ops as f64 * 100.0
            } else {
                100.0
            },
            failure_rate: if verified_ops > 0 {
                failed_ops as f64 / verified_ops as f64 * 100.0
            } else {
                0.0
            },
        })
    }

    /// Get replica acknowledgment distribution across nodes.
    pub fn replica_ack_distribution(
        &self,
        dataset_id: DatasetId,
    ) -> Result<ReplicaAckDistribution, DurabilityError> {
        self.ensure_dataset(dataset_id);
        let datasets = self.datasets.borrow();
        let state = datasets
            .get(&dataset_id)
            .ok_or_else(|| DurabilityError::DatasetNotFound { dataset_id })?;

        let mut ack_counts: HashMap<NodeId, u32> = HashMap::new();
        let mut total_acks = 0u32;

        for op_state in state.ops.values() {
            for node_id in &op_state.replica_acks {
                *ack_counts.entry(*node_id).or_insert(0) += 1;
                total_acks += 1;
            }
        }

        Ok(ReplicaAckDistribution {
            dataset_id,
            ack_counts,
            total_acks,
            unique_replicas: ack_counts.len() as u32,
        })
    }
}

/// Durability verification trend over time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationTrend {
    pub dataset_id: DatasetId,
    pub total_operations: u64,
    pub verified_operations: u64,
    pub failed_verifications: u64,
    pub verification_rate: f64,
    pub failure_rate: f64,
}

/// Replica acknowledgment distribution across nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaAckDistribution {
    pub dataset_id: DatasetId,
    pub ack_counts: HashMap<NodeId, u32>,
    pub total_acks: u32,
    pub unique_replicas: u32,
}

impl ReplicaAckDistribution {
    pub fn acks_for_node(&self, node_id: NodeId) -> u32 {
        self.ack_counts.get(&node_id).copied().unwrap_or(0)
    }

    pub fn top_replica(&self) -> Option<(NodeId, u32)> {
        self.ack_counts.iter().max_by_key(|(_, v)| *v).map(|(k, v)| (*k, *v))
    }
}
///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1u8; 16])
    }

    fn test_dataset_id2() -> DatasetId {
        DatasetId::new([2u8; 16])
    }

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

    fn test_op_ref() -> OpRef {
        OpRef::new([1u8; 9])
    }

    fn test_op_ref2() -> OpRef {
        OpRef::new([2u8; 9])
    }

    fn test_ordering_pos() -> OrderingPos {
        OrderingPos::new([1u8; 16])
    }

    fn test_checkpoint_id() -> CheckpointId {
        CheckpointId::new([1u8; 16])
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_version() -> Version {
        Version::new(1)
    }

    fn test_config() -> DurabilityConfig {
        DurabilityConfig::default()
    }

    // ========================================================================
    // DurabilityConfig tests
    // ========================================================================

    #[test]
    fn durability_config_default() {
        let config = DurabilityConfig::default();
        assert_eq!(config.default_condition, DurabilityCondition::ReplicasAcked);
        assert_eq!(config.required_replica_acks, 2);
        assert_eq!(config.total_replicas, 3);
        assert!(config.enable_cross_region);
        assert_eq!(config.required_regions, 2);
        assert!(config.enable_auto_checkpoint);
        assert_eq!(config.checkpoint_interval_ops, 1000);
        assert_eq!(config.max_checkpoints, 10);
        assert_eq!(config.durability_timeout_ms, 300000);
        assert!(!config.enable_verify_on_read);
    }

    #[test]
    fn durability_config_new_valid() {
        let config = DurabilityConfig::new(DurabilityCondition::Executed).unwrap();
        assert_eq!(config.default_condition, DurabilityCondition::Executed);
    }

    #[test]
    fn durability_config_validate_acks_exceed_replicas() {
        let mut config = DurabilityConfig::default();
        config.required_replica_acks = 5;
        config.total_replicas = 3;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_validate_zero_replicas() {
        let mut config = DurabilityConfig::default();
        config.total_replicas = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_validate_zero_regions_with_cross_region() {
        let mut config = DurabilityConfig::default();
        config.required_regions = 0;
        config.enable_cross_region = true;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_validate_zero_checkpoint_interval() {
        let mut config = DurabilityConfig::default();
        config.checkpoint_interval_ops = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_validate_zero_max_checkpoints() {
        let mut config = DurabilityConfig::default();
        config.max_checkpoints = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_validate_zero_timeout() {
        let mut config = DurabilityConfig::default();
        config.durability_timeout_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn durability_config_ack_ratio() {
        let config = DurabilityConfig::default();
        assert!((config.ack_ratio() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn durability_config_region_ratio() {
        let config = DurabilityConfig::default();
        assert!((config.region_ratio() - 1.0).abs() < 0.01);
    }

    #[test]
    fn durability_config_strict() {
        let config = DurabilityConfig::strict();
        assert_eq!(config.default_condition, DurabilityCondition::CrossRegionCompleted);
        assert_eq!(config.required_replica_acks, 3);
        assert_eq!(config.total_replicas, 5);
        assert_eq!(config.required_regions, 3);
        assert!(config.enable_verify_on_read);
    }

    #[test]
    fn durability_config_lenient() {
        let config = DurabilityConfig::lenient();
        assert_eq!(config.default_condition, DurabilityCondition::Received);
        assert_eq!(config.required_replica_acks, 1);
        assert_eq!(config.total_replicas, 1);
        assert!(!config.enable_cross_region);
        assert!(!config.enable_auto_checkpoint);
        assert!(!config.enable_verify_on_read);
    }

    #[test]
    fn durability_config_serialization_roundtrip() {
        let config = DurabilityConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DurabilityConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_condition, deserialized.default_condition);
        assert_eq!(config.required_replica_acks, deserialized.required_replica_acks);
    }

    // ========================================================================
    // DurabilityCondition tests
    // ========================================================================

    #[test]
    fn durability_condition_level() {
        assert_eq!(DurabilityCondition::Received.level(), 1);
        assert_eq!(DurabilityCondition::Accepted.level(), 2);
        assert_eq!(DurabilityCondition::Executed.level(), 3);
        assert_eq!(DurabilityCondition::LocalCommitted.level(), 4);
        assert_eq!(DurabilityCondition::ReplicasAcked.level(), 5);
        assert_eq!(DurabilityCondition::CrossRegionCompleted.level(), 6);
    }

    #[test]
    fn durability_condition_meets() {
        assert!(DurabilityCondition::ReplicasAcked.meets(DurabilityCondition::Executed));
        assert!(DurabilityCondition::CrossRegionCompleted.meets(DurabilityCondition::ReplicasAcked));
        assert!(!DurabilityCondition::Executed.meets(DurabilityCondition::ReplicasAcked));
        assert!(!DurabilityCondition::Received.meets(DurabilityCondition::Accepted));
    }

    #[test]
    fn durability_condition_as_str() {
        assert_eq!(DurabilityCondition::Received.as_str(), "received");
        assert_eq!(DurabilityCondition::CrossRegionCompleted.as_str(), "cross_region_completed");
    }

    #[test]
    fn durability_condition_display() {
        let cond = DurabilityCondition::Executed;
        assert_eq!(format!("{}", cond), "executed");
    }

    #[test]
    fn durability_condition_serialization_roundtrip() {
        let cond = DurabilityCondition::CrossRegionCompleted;
        let json = serde_json::to_string(&cond).unwrap();
        let deserialized: DurabilityCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(cond, deserialized);
    }

    // ========================================================================
    // DurabilityRecord tests
    // ========================================================================

    #[test]
    fn durability_record_new() {
        let now = 1000u64;
        let record = DurabilityRecord::new(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
            now,
        );
        assert_eq!(record.dataset_id, test_dataset_id());
        assert_eq!(record.op_ref, test_op_ref());
        assert_eq!(record.condition, DurabilityCondition::ReplicasAcked);
        assert_eq!(record.node_id, test_node_id());
        assert_eq!(record.recorded_at, now);
        assert_eq!(record.level, 5);
    }

    #[test]
    fn durability_record_meets() {
        let record = DurabilityRecord::new(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
            0,
        );
        assert!(record.meets(DurabilityCondition::Executed));
        assert!(!record.meets(DurabilityCondition::CrossRegionCompleted));
    }

    #[test]
    fn durability_record_is_terminal() {
        let record = DurabilityRecord::new(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::CrossRegionCompleted,
            test_node_id(),
            0,
        );
        assert!(record.is_terminal());
        let record = DurabilityRecord::new(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
            0,
        );
        assert!(!record.is_terminal());
    }

    #[test]
    fn durability_record_serialization_roundtrip() {
        let record = DurabilityRecord::new(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::Executed,
            test_node_id(),
            500,
        );
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: DurabilityRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.condition, deserialized.condition);
        assert_eq!(record.recorded_at, deserialized.recorded_at);
    }

    // ========================================================================
    // ReplicaAckRecord tests
    // ========================================================================

    #[test]
    fn replica_ack_record_new() {
        let record = ReplicaAckRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_node_id(),
            1000,
            1,
        );
        assert_eq!(record.dataset_id, test_dataset_id());
        assert_eq!(record.op_ref, test_op_ref());
        assert_eq!(record.replica_node, test_node_id());
        assert_eq!(record.acked_at, 1000);
        assert_eq!(record.ack_sequence, 1);
    }

    #[test]
    fn replica_ack_record_serialization_roundtrip() {
        let record = ReplicaAckRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_node_id2(),
            2000,
            5,
        );
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ReplicaAckRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.replica_node, deserialized.replica_node);
        assert_eq!(record.ack_sequence, deserialized.ack_sequence);
    }

    // ========================================================================
    // CrossRegionRecord tests
    // ========================================================================

    #[test]
    fn cross_region_record_new() {
        let record = CrossRegionRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
            3000,
        );
        assert_eq!(record.dataset_id, test_dataset_id());
        assert_eq!(record.op_ref, test_op_ref());
        assert_eq!(record.region_id, test_ordering_pos());
        assert_eq!(record.condition, DurabilityCondition::LocalCommitted);
        assert_eq!(record.replicated_at, 3000);
    }

    #[test]
    fn cross_region_record_serialization_roundtrip() {
        let record = CrossRegionRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::CrossRegionCompleted,
            4000,
        );
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: CrossRegionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.condition, deserialized.condition);
        assert_eq!(record.region_id, deserialized.region_id);
    }

    // ========================================================================
    // CrossRegionStatus tests
    // ========================================================================

    #[test]
    fn cross_region_status_completion_percent() {
        let status = CrossRegionStatus {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            replicated_regions: vec![test_ordering_pos()],
            required_regions: 2,
            complete: false,
            highest_condition: DurabilityCondition::LocalCommitted,
        };
        assert!((status.completion_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn cross_region_status_completion_percent_all() {
        let status = CrossRegionStatus {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            replicated_regions: vec![test_ordering_pos(), OrderingPos::new([2u8; 16])],
            required_regions: 2,
            complete: true,
            highest_condition: DurabilityCondition::CrossRegionCompleted,
        };
        assert!((status.completion_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn cross_region_status_zero_regions() {
        let status = CrossRegionStatus {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            replicated_regions: vec![],
            required_regions: 0,
            complete: true,
            highest_condition: DurabilityCondition::Received,
        };
        assert!((status.completion_percent() - 100.0).abs() < 0.01);
    }

    // ========================================================================
    // CheckpointRecord tests
    // ========================================================================

    #[test]
    fn checkpoint_record_new() {
        let checkpoint = CheckpointRecord::new(
            test_dataset_id(),
            test_checkpoint_id(),
            test_ordering_pos(),
            5000,
            100,
            1024,
            vec![1, 2, 3],
        );
        assert_eq!(checkpoint.dataset_id, test_dataset_id());
        assert_eq!(checkpoint.checkpoint_id, test_checkpoint_id());
        assert_eq!(checkpoint.op_position, test_ordering_pos());
        assert_eq!(checkpoint.created_at, 5000);
        assert_eq!(checkpoint.ops_covered, 100);
        assert_eq!(checkpoint.size_bytes, 1024);
        assert_eq!(checkpoint.checksum, vec![1, 2, 3]);
    }

    #[test]
    fn checkpoint_record_is_empty() {
        let checkpoint = CheckpointRecord::new(
            test_dataset_id(),
            test_checkpoint_id(),
            test_ordering_pos(),
            0,
            0,
            0,
            vec![],
        );
        assert!(checkpoint.is_empty());
        let checkpoint = CheckpointRecord::new(
            test_dataset_id(),
            test_checkpoint_id(),
            test_ordering_pos(),
            0,
            1,
            10,
            vec![],
        );
        assert!(!checkpoint.is_empty());
    }

    #[test]
    fn checkpoint_record_serialization_roundtrip() {
        let checkpoint = CheckpointRecord::new(
            test_dataset_id(),
            test_checkpoint_id(),
            test_ordering_pos(),
            5000,
            100,
            1024,
            vec![1, 2, 3],
        );
        let json = serde_json::to_string(&checkpoint).unwrap();
        let deserialized: CheckpointRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(checkpoint.checkpoint_id, deserialized.checkpoint_id);
        assert_eq!(checkpoint.ops_covered, deserialized.ops_covered);
    }

    // ========================================================================
    // DurabilityVerification tests
    // ========================================================================

    #[test]
    fn durability_verification_meets_all_requirements() {
        let verification = DurabilityVerification {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            verified: true,
            current_condition: DurabilityCondition::CrossRegionCompleted,
            required_condition: DurabilityCondition::ReplicasAcked,
            replica_ack_count: 3,
            required_replica_acks: 2,
            cross_region_complete: true,
            verified_at: 1000,
            details: "all good".to_string(),
        };
        assert!(verification.meets_all_requirements());
    }

    #[test]
    fn durability_verification_not_meeting_requirements() {
        let verification = DurabilityVerification {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            verified: false,
            current_condition: DurabilityCondition::Executed,
            required_condition: DurabilityCondition::ReplicasAcked,
            replica_ack_count: 1,
            required_replica_acks: 2,
            cross_region_complete: false,
            verified_at: 1000,
            details: "insufficient".to_string(),
        };
        assert!(!verification.meets_all_requirements());
    }

    #[test]
    fn durability_verification_serialization_roundtrip() {
        let verification = DurabilityVerification {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            verified: true,
            current_condition: DurabilityCondition::Executed,
            required_condition: DurabilityCondition::Accepted,
            replica_ack_count: 2,
            required_replica_acks: 2,
            cross_region_complete: true,
            verified_at: 1000,
            details: "ok".to_string(),
        };
        let json = serde_json::to_string(&verification).unwrap();
        let deserialized: DurabilityVerification = serde_json::from_str(&json).unwrap();
        assert_eq!(verification.verified, deserialized.verified);
        assert_eq!(verification.current_condition, deserialized.current_condition);
    }

    // ========================================================================
    // DurabilityStats tests
    // ========================================================================

    #[test]
    fn durability_stats_new() {
        let stats = DurabilityStats::new(test_dataset_id());
        assert_eq!(stats.dataset_id, test_dataset_id());
        assert_eq!(stats.total_ops, 0);
        assert_eq!(stats.ops_meeting_condition, 0);
        assert_eq!(stats.coverage_percent, 0.0);
    }

    #[test]
    fn durability_stats_condition_meet_rate() {
        let mut stats = DurabilityStats::new(test_dataset_id());
        stats.total_ops = 100;
        stats.ops_meeting_condition = 75;
        assert!((stats.condition_meet_rate() - 75.0).abs() < 0.01);
    }

    #[test]
    fn durability_stats_condition_meet_rate_zero_ops() {
        let stats = DurabilityStats::new(test_dataset_id());
        assert!((stats.condition_meet_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn durability_stats_cross_region_complete_rate() {
        let mut stats = DurabilityStats::new(test_dataset_id());
        stats.total_ops = 50;
        stats.ops_cross_region_complete = 25;
        assert!((stats.cross_region_complete_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn durability_stats_verification_failure_rate() {
        let mut stats = DurabilityStats::new(test_dataset_id());
        stats.total_verifications = 20;
        stats.failed_verifications = 4;
        assert!((stats.verification_failure_rate() - 20.0).abs() < 0.01);
    }

    #[test]
    fn durability_stats_serialization_roundtrip() {
        let mut stats = DurabilityStats::new(test_dataset_id());
        stats.total_ops = 10;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: DurabilityStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_ops, deserialized.total_ops);
    }

    // ========================================================================
    // DurabilityError tests
    // ========================================================================

    #[test]
    fn durability_error_category() {
        assert_eq!(
            DurabilityError::InvalidConfig { field: "x".to_string(), reason: "y".to_string() }.category(),
            "invalid_config"
        );
        assert_eq!(
            DurabilityError::DatasetNotFound { dataset_id: test_dataset_id() }.category(),
            "dataset_not_found"
        );
        assert_eq!(
            DurabilityError::ConditionNotMet { current: DurabilityCondition::Received, required: DurabilityCondition::Executed }.category(),
            "condition_not_met"
        );
    }

    #[test]
    fn durability_error_is_recoverable() {
        assert!(DurabilityError::ConditionNotMet { current: DurabilityCondition::Received, required: DurabilityCondition::Executed }.is_recoverable());
        assert!(DurabilityError::ReplicaQuorumNotMet { acks: 1, required: 2 }.is_recoverable());
        assert!(!DurabilityError::DatasetNotFound { dataset_id: test_dataset_id() }.is_recoverable());
    }

    #[test]
    fn durability_error_suggested_action() {
        assert_eq!(
            DurabilityError::DatasetNotFound { dataset_id: test_dataset_id() }.suggested_action(),
            "create the dataset first"
        );
    }

    // ========================================================================
    // OpDurabilityState tests
    // ========================================================================

    #[test]
    fn op_durability_state_new() {
        let state = OpDurabilityState::new(test_op_ref());
        assert_eq!(state.op_ref, test_op_ref());
        assert_eq!(state.current_condition, DurabilityCondition::Received);
        assert_eq!(state.ack_count(), 0);
    }

    #[test]
    fn op_durability_state_meets_condition() {
        let mut state = OpDurabilityState::new(test_op_ref());
        state.current_condition = DurabilityCondition::ReplicasAcked;
        assert!(state.meets_condition(DurabilityCondition::Executed));
        assert!(!state.meets_condition(DurabilityCondition::CrossRegionCompleted));
    }

    #[test]
    fn op_durability_state_has_cross_region() {
        let mut state = OpDurabilityState::new(test_op_ref());
        assert!(!state.has_cross_region(test_ordering_pos()));
        state.cross_region_records.push(CrossRegionRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
            100,
        ));
        assert!(state.has_cross_region(test_ordering_pos()));
    }

    #[test]
    fn op_durability_state_cross_region_complete() {
        let mut state = OpDurabilityState::new(test_op_ref());
        assert!(!state.cross_region_complete(2));
        state.cross_region_records.push(CrossRegionRecord::new(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
            100,
        ));
        state.cross_region_records.push(CrossRegionRecord::new(
            test_dataset_id(),
            test_op_ref(),
            OrderingPos::new([2u8; 16]),
            DurabilityCondition::LocalCommitted,
            200,
        ));
        assert!(state.cross_region_complete(2));
    }

    #[test]
    fn op_durability_state_highest_cross_region_condition() {
        let state = OpDurabilityState::new(test_op_ref());
        assert_eq!(
            state.highest_cross_region_condition(),
            DurabilityCondition::Received
        );
    }

    // ========================================================================
    // DatasetDurabilityState tests
    // ========================================================================

    #[test]
    fn dataset_durability_state_new() {
        let state = DatasetDurabilityState::new(test_dataset_id(), DurabilityConfig::default());
        assert_eq!(state.dataset_id, test_dataset_id());
        assert_eq!(state.ops.len(), 0);
        assert_eq!(state.checkpoints.len(), 0);
    }

    // ========================================================================
    // DurabilityEngine tests
    // ========================================================================

    #[test]
    fn durability_engine_new() {
        let engine = DurabilityEngine::new();
        assert_eq!(engine.datasets().len(), 0);
        assert_eq!(engine.dataset_count(), 0);
    }

    #[test]
    fn durability_engine_with_config() {
        let config = DurabilityConfig::new(DurabilityCondition::Executed).unwrap();
        let engine = DurabilityEngine::with_config(config).unwrap();
        assert_eq!(engine.datasets().len(), 0);
    }

    #[test]
    fn durability_engine_with_config_invalid() {
        let mut config = DurabilityConfig::default();
        config.required_replica_acks = 5;
        config.total_replicas = 3;
        let result = DurabilityEngine::with_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn durability_engine_configure_durability() {
        let engine = DurabilityEngine::new();
        let config = DurabilityConfig::new(DurabilityCondition::Executed).unwrap();
        assert!(engine.configure_durability(test_dataset_id(), config).is_ok());
        assert!(engine.is_tracked(test_dataset_id()));
    }

    #[test]
    fn durability_engine_configure_durability_invalid() {
        let engine = DurabilityEngine::new();
        let mut config = DurabilityConfig::default();
        config.required_replica_acks = 5;
        config.total_replicas = 3;
        let err = engine.configure_durability(test_dataset_id(), config).unwrap_err();
        assert!(matches!(err, DurabilityError::InvalidConfig { .. }));
    }

    #[test]
    fn durability_engine_record_durability() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let record = engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::Accepted,
            test_node_id(),
        ).unwrap();
        assert_eq!(record.condition, DurabilityCondition::Accepted);
        assert_eq!(record.node_id, test_node_id());
    }

    #[test]
    fn durability_engine_record_durability_invalid_transition() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::Executed,
            test_node_id(),
        ).unwrap();
        let err = engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::Accepted,
            test_node_id(),
        ).unwrap_err();
        assert!(matches!(err, DurabilityError::InvalidConditionTransition { .. }));
    }

    #[test]
    fn durability_engine_durability_state() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::Executed,
            test_node_id(),
        ).unwrap();
        let state = engine.durability_state(test_dataset_id(), test_op_ref()).unwrap();
        assert!(matches!(state, DurabilityTrackState::InProgress));
    }

    #[test]
    fn durability_engine_highest_condition() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::LocalCommitted,
            test_node_id(),
        ).unwrap();
        let cond = engine.highest_condition(test_dataset_id(), test_op_ref()).unwrap();
        assert_eq!(cond, DurabilityCondition::LocalCommitted);
    }

    #[test]
    fn durability_engine_meets_condition() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
        ).unwrap();
        assert!(engine.meets_condition(test_dataset_id(), test_op_ref(), DurabilityCondition::Executed).unwrap());
        assert!(!engine.meets_condition(test_dataset_id(), test_op_ref(), DurabilityCondition::CrossRegionCompleted).unwrap());
    }

    #[test]
    fn durability_engine_register_replica_ack() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let record = engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        assert_eq!(record.replica_node, test_node_id());
        assert_eq!(record.ack_sequence, 1);
        assert_eq!(engine.replica_ack_count(test_dataset_id(), test_op_ref()).unwrap(), 1);
    }

    #[test]
    fn durability_engine_register_duplicate_replica_ack() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        let err = engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap_err();
        assert!(matches!(err, DurabilityError::DuplicateReplicaAck { .. }));
    }

    #[test]
    fn durability_engine_acknowledging_replicas() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id2()).unwrap();
        let replicas = engine.acknowledging_replicas(test_dataset_id(), test_op_ref()).unwrap();
        assert_eq!(replicas.len(), 2);
    }

    #[test]
    fn durability_engine_replica_quorum_met() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        assert!(engine.replica_quorum_met(test_dataset_id(), test_op_ref(), 1).unwrap());
        assert!(!engine.replica_quorum_met(test_dataset_id(), test_op_ref(), 2).unwrap());
    }

    #[test]
    fn durability_engine_record_cross_region_progress() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let record = engine.record_cross_region_progress(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
        ).unwrap();
        assert_eq!(record.region_id, test_ordering_pos());
        assert_eq!(record.condition, DurabilityCondition::LocalCommitted);
    }

    #[test]
    fn durability_engine_cross_region_status() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_cross_region_progress(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
        ).unwrap();
        let status = engine.cross_region_status(test_dataset_id(), test_op_ref()).unwrap();
        assert_eq!(status.replicated_regions.len(), 1);
        assert_eq!(status.required_regions, 2);
        assert!(!status.complete);
    }

    #[test]
    fn durability_engine_cross_region_complete() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_cross_region_progress(
            test_dataset_id(),
            test_op_ref(),
            test_ordering_pos(),
            DurabilityCondition::LocalCommitted,
        ).unwrap();
        engine.record_cross_region_progress(
            test_dataset_id(),
            test_op_ref(),
            OrderingPos::new([2u8; 16]),
            DurabilityCondition::LocalCommitted,
        ).unwrap();
        assert!(engine.cross_region_complete(test_dataset_id(), test_op_ref()).unwrap());
    }

    #[test]
    fn durability_engine_create_checkpoint() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let checkpoint = engine.create_checkpoint(
            test_dataset_id(),
            test_checkpoint_id(),
            test_ordering_pos(),
        ).unwrap();
        assert_eq!(checkpoint.dataset_id, test_dataset_id());
        assert_eq!(checkpoint.checkpoint_id, test_checkpoint_id());
    }

    #[test]
    fn durability_engine_checkpoint_limit() {
        let engine = DurabilityEngine::new();
        let mut config = DurabilityConfig::default();
        config.max_checkpoints = 1;
        engine.configure_durability(test_dataset_id(), config).unwrap();
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();
        let err = engine.create_checkpoint(
            test_dataset_id(),
            CheckpointId::new([2u8; 16]),
            test_ordering_pos(),
        ).unwrap_err();
        assert!(matches!(err, DurabilityError::CheckpointLimitExceeded { .. }));
    }

    #[test]
    fn durability_engine_checkpoint_info() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();
        let info = engine.checkpoint_info(test_dataset_id(), test_checkpoint_id()).unwrap();
        assert_eq!(info.checkpoint_id, test_checkpoint_id());
    }

    #[test]
    fn durability_engine_all_checkpoints() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();
        let checkpoints = engine.all_checkpoints(test_dataset_id()).unwrap();
        assert_eq!(checkpoints.len(), 1);
    }

    #[test]
    fn durability_engine_latest_checkpoint() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        assert!(engine.latest_checkpoint(test_dataset_id()).unwrap().is_none());
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();
        let latest = engine.latest_checkpoint(test_dataset_id()).unwrap().unwrap();
        assert_eq!(latest.checkpoint_id, test_checkpoint_id());
    }

    #[test]
    fn durability_engine_delete_checkpoint() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();
        engine.delete_checkpoint(test_dataset_id(), test_checkpoint_id()).unwrap();
        let err = engine.checkpoint_info(test_dataset_id(), test_checkpoint_id()).unwrap_err();
        assert!(matches!(err, DurabilityError::CheckpointNotFound { .. }));
    }

    #[test]
    fn durability_engine_verify_durability() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::CrossRegionCompleted,
            test_node_id(),
        ).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id2()).unwrap();
        let result = engine.verify_durability(test_dataset_id(), test_op_ref()).unwrap();
        assert!(result.verified);
    }

    #[test]
    fn durability_engine_stats() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(
            test_dataset_id(),
            test_op_ref(),
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
        ).unwrap();
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.dataset_id, test_dataset_id());
        assert_eq!(stats.total_ops, 1);
    }

    #[test]
    fn durability_engine_datasets() {
        let engine = DurabilityEngine::new();
        assert!(engine.datasets().is_empty());
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.configure_durability(test_dataset_id2(), DurabilityConfig::default()).unwrap();
        assert_eq!(engine.datasets().len(), 2);
    }

    #[test]
    fn durability_engine_is_tracked() {
        let engine = DurabilityEngine::new();
        assert!(!engine.is_tracked(test_dataset_id()));
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        assert!(engine.is_tracked(test_dataset_id()));
    }

    #[test]
    fn durability_engine_default_condition() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::strict()).unwrap();
        let cond = engine.default_condition(test_dataset_id()).unwrap();
        assert_eq!(cond, DurabilityCondition::CrossRegionCompleted);
    }

    #[test]
    fn durability_engine_required_replica_acks() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let acks = engine.required_replica_acks(test_dataset_id()).unwrap();
        assert_eq!(acks, 2);
    }

    #[test]
    fn durability_engine_validate_config() {
        let engine = DurabilityEngine::new();
        let config = DurabilityConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    // ========================================================================
    // CheckpointLifecycle tests
    // ========================================================================

    #[test]
    fn checkpoint_lifecycle_new() {
        let config = DurabilityConfig::default();
        let lifecycle = CheckpointLifecycle::new(test_dataset_id(), &config);
        assert_eq!(lifecycle.dataset_id, test_dataset_id());
        assert!(lifecycle.auto_checkpoint_enabled);
        assert_eq!(lifecycle.max_checkpoints, 10);
    }

    #[test]
    fn checkpoint_lifecycle_should_checkpoint() {
        let config = DurabilityConfig::default();
        let lifecycle = CheckpointLifecycle::new(test_dataset_id(), &config);
        assert!(!lifecycle.should_checkpoint(500));
        assert!(lifecycle.should_checkpoint(1000));
    }

    #[test]
    fn checkpoint_lifecycle_disabled() {
        let mut config = DurabilityConfig::default();
        config.enable_auto_checkpoint = false;
        let lifecycle = CheckpointLifecycle::new(test_dataset_id(), &config);
        assert!(!lifecycle.should_checkpoint(5000));
    }

    #[test]
    fn checkpoint_lifecycle_record_created_deleted() {
        let config = DurabilityConfig::default();
        let mut lifecycle = CheckpointLifecycle::new(test_dataset_id(), &config);
        lifecycle.record_created();
        lifecycle.record_created();
        lifecycle.record_deleted();
        assert_eq!(lifecycle.checkpoints_created, 2);
        assert_eq!(lifecycle.checkpoints_deleted, 1);
        assert_eq!(lifecycle.net_checkpoints(), 1);
    }

    // ========================================================================
    // RecoverySource tests
    // ========================================================================

    #[test]
    fn recovery_source_display_name() {
        assert_eq!(RecoverySource::LocalLog.display_name(), "local_log");
        assert_eq!(RecoverySource::Replica.display_name(), "replica");
        assert_eq!(RecoverySource::CrossRegion.display_name(), "cross_region");
        assert_eq!(RecoverySource::Checkpoint.display_name(), "checkpoint");
    }

    #[test]
    fn recovery_source_serialization_roundtrip() {
        let source = RecoverySource::Replica;
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: RecoverySource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, deserialized);
    }

    // ========================================================================
    // DurabilityRecoveryResult tests
    // ========================================================================

    #[test]
    fn durability_recovery_result_new() {
        let result = DurabilityRecoveryResult {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            recovered: true,
            recovered_condition: DurabilityCondition::ReplicasAcked,
            recovery_source: RecoverySource::Replica,
            recovered_at: 5000,
        };
        assert!(result.recovered);
        assert_eq!(result.recovery_source, RecoverySource::Replica);
    }

    #[test]
    fn durability_recovery_result_serialization_roundtrip() {
        let result = DurabilityRecoveryResult {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            recovered: false,
            recovered_condition: DurabilityCondition::Received,
            recovery_source: RecoverySource::LocalLog,
            recovered_at: 0,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DurabilityRecoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.recovered, deserialized.recovered);
    }

    // ========================================================================
    // DurabilityAdvanceRequest / Result tests
    // ========================================================================

    #[test]
    fn durability_advance_request_new() {
        let request = DurabilityAdvanceRequest::new(
            test_dataset_id(),
            vec![test_op_ref(), test_op_ref2()],
            DurabilityCondition::ReplicasAcked,
            test_node_id(),
            1000,
        );
        assert_eq!(request.op_count(), 2);
    }

    #[test]
    fn durability_advance_result_success_rate() {
        let request = DurabilityAdvanceRequest::new(
            test_dataset_id(),
            vec![test_op_ref()],
            DurabilityCondition::Received,
            test_node_id(),
            0,
        );
        let result = DurabilityAdvanceResult {
            request,
            advanced_count: 8,
            failed_count: 2,
            failures: vec![],
            completed_at: 1000,
        };
        assert!((result.success_rate() - 80.0).abs() < 0.01);
        assert!(!result.is_complete());
    }

    #[test]
    fn durability_advance_result_complete() {
        let request = DurabilityAdvanceRequest::new(
            test_dataset_id(),
            vec![],
            DurabilityCondition::Received,
            test_node_id(),
            0,
        );
        let result = DurabilityAdvanceResult {
            request,
            advanced_count: 5,
            failed_count: 0,
            failures: vec![],
            completed_at: 0,
        };
        assert!((result.success_rate() - 100.0).abs() < 0.01);
        assert!(result.is_complete());
    }

    // ========================================================================
    // ConditionDistribution tests
    // ========================================================================

    #[test]
    fn condition_distribution_total() {
        let dist = ConditionDistribution {
            received: 10,
            accepted: 5,
            executed: 3,
            local_committed: 2,
            replicas_acked: 5,
            cross_region_completed: 5,
        };
        assert_eq!(dist.total(), 30);
    }

    #[test]
    fn condition_distribution_percent_at_or_above() {
        let dist = ConditionDistribution {
            received: 0,
            accepted: 0,
            executed: 0,
            local_committed: 0,
            replicas_acked: 5,
            cross_region_completed: 5,
        };
        assert!((dist.percent_at_or_above(DurabilityCondition::ReplicasAcked) - 100.0).abs() < 0.01);
        assert!((dist.percent_at_or_above(DurabilityCondition::Executed) - 100.0).abs() < 0.01);
        assert!((dist.percent_at_or_above(DurabilityCondition::CrossRegionCompleted) - 50.0).abs() < 0.01);
    }

    #[test]
    fn condition_distribution_percent_zero_total() {
        let dist = ConditionDistribution {
            received: 0,
            accepted: 0,
            executed: 0,
            local_committed: 0,
            replicas_acked: 0,
            cross_region_completed: 0,
        };
        assert!((dist.percent_at_or_above(DurabilityCondition::Received) - 100.0).abs() < 0.01);
    }

    // ========================================================================
    // DurabilityHealth tests
    // ========================================================================

    #[test]
    fn durability_health_is_healthy() {
        let health = DurabilityHealth {
            dataset_id: test_dataset_id(),
            total_ops: 100,
            healthy_ops: 80,
            degraded_ops: 10,
            failed_ops: 10,
            health_percent: 80.0,
            avg_condition_level: 4.0,
        };
        assert!(health.is_healthy(50.0));
        assert!(!health.is_healthy(90.0));
    }

    #[test]
    fn durability_health_serialization_roundtrip() {
        let health = DurabilityHealth {
            dataset_id: test_dataset_id(),
            total_ops: 50,
            healthy_ops: 40,
            degraded_ops: 5,
            failed_ops: 5,
            health_percent: 80.0,
            avg_condition_level: 3.5,
        };
        let json = serde_json::to_string(&health).unwrap();
        let deserialized: DurabilityHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(health.health_percent, deserialized.health_percent);
    }

    // ========================================================================
    // VerificationTrend tests
    // ========================================================================

    #[test]
    fn verification_trend_new() {
        let trend = VerificationTrend {
            dataset_id: test_dataset_id(),
            total_operations: 100,
            verified_operations: 80,
            failed_verifications: 5,
            verification_rate: 80.0,
            failure_rate: 6.25,
        };
        assert_eq!(trend.total_operations, 100);
        assert_eq!(trend.verification_rate, 80.0);
    }

    // ========================================================================
    // ReplicaAckDistribution tests
    // ========================================================================

    #[test]
    fn replica_ack_distribution_acks_for_node() {
        let mut ack_counts = HashMap::new();
        ack_counts.insert(test_node_id(), 5);
        ack_counts.insert(test_node_id2(), 3);
        let dist = ReplicaAckDistribution {
            dataset_id: test_dataset_id(),
            ack_counts,
            total_acks: 8,
            unique_replicas: 2,
        };
        assert_eq!(dist.acks_for_node(test_node_id()), 5);
        assert_eq!(dist.acks_for_node(test_node_id2()), 3);
        assert_eq!(dist.acks_for_node(test_node_id3()), 0);
    }

    #[test]
    fn replica_ack_distribution_top_replica() {
        let mut ack_counts = HashMap::new();
        ack_counts.insert(test_node_id(), 5);
        ack_counts.insert(test_node_id2(), 10);
        let dist = ReplicaAckDistribution {
            dataset_id: test_dataset_id(),
            ack_counts,
            total_acks: 15,
            unique_replicas: 2,
        };
        let top = dist.top_replica().unwrap();
        assert_eq!(top.0, test_node_id2());
        assert_eq!(top.1, 10);
    }

    // ========================================================================
    // DurabilityEngine advanced methods tests
    // ========================================================================

    #[test]
    fn durability_engine_checkpoint_lifecycle() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let lifecycle = engine.checkpoint_lifecycle(test_dataset_id()).unwrap();
        assert_eq!(lifecycle.dataset_id, test_dataset_id());
        assert!(lifecycle.auto_checkpoint_enabled);
    }

    #[test]
    fn durability_engine_recover_operation() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        let result = engine.recover_operation(test_dataset_id(), test_op_ref(), RecoverySource::Replica).unwrap();
        assert!(result.recovered);
        assert_eq!(result.recovery_source, RecoverySource::Replica);
    }

    #[test]
    fn durability_engine_recover_operation_not_found() {
        let engine = DurabilityEngine::new();
        let err = engine.recover_operation(test_dataset_id(), test_op_ref(), RecoverySource::Replica).unwrap_err();
        assert!(matches!(err, DurabilityError::OperationNotFound { .. }));
    }

    #[test]
    fn durability_engine_advance_batch() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref2(), DurabilityCondition::Received, test_node_id()).unwrap();
        let request = DurabilityAdvanceRequest::new(
            test_dataset_id(),
            vec![test_op_ref(), test_op_ref2()],
            DurabilityCondition::Executed,
            test_node_id(),
            0,
        );
        let result = engine.advance_batch(request).unwrap();
        assert_eq!(result.advanced_count, 2);
        assert_eq!(result.failed_count, 0);
    }

    #[test]
    fn durability_engine_advance_batch_with_failures() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let request = DurabilityAdvanceRequest::new(
            test_dataset_id(),
            vec![test_op_ref()],
            DurabilityCondition::Executed,
            test_node_id(),
            0,
        );
        let result = engine.advance_batch(request).unwrap();
        assert_eq!(result.advanced_count, 0);
        assert_eq!(result.failed_count, 1);
    }

    #[test]
    fn durability_engine_condition_distribution() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::ReplicasAcked, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref2(), DurabilityCondition::Executed, test_node_id2()).unwrap();
        let dist = engine.condition_distribution(test_dataset_id()).unwrap();
        assert_eq!(dist.replicas_acked, 1);
        assert_eq!(dist.executed, 1);
    }

    #[test]
    fn durability_engine_durability_health() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::ReplicasAcked, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref2(), DurabilityCondition::Received, test_node_id2()).unwrap();
        let health = engine.durability_health(test_dataset_id(), 50.0).unwrap();
        assert_eq!(health.dataset_id, test_dataset_id());
        assert_eq!(health.total_ops, 2);
        assert_eq!(health.healthy_ops, 1);
    }

    #[test]
    fn durability_engine_operations_below_condition() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref2(), DurabilityCondition::ReplicasAcked, test_node_id2()).unwrap();
        let below = engine.operations_below_condition(test_dataset_id(), DurabilityCondition::Executed).unwrap();
        assert_eq!(below.len(), 2);
    }

    #[test]
    fn durability_engine_operations_at_or_above() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::ReplicasAcked, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref2(), DurabilityCondition::Received, test_node_id2()).unwrap();
        let at_or_above = engine.operations_at_or_above(test_dataset_id(), DurabilityCondition::Executed).unwrap();
        assert_eq!(at_or_above.len(), 1);
    }

    #[test]
    fn durability_engine_totals_all_datasets() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.configure_durability(test_dataset_id2(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id2(), test_op_ref2(), DurabilityCondition::Received, test_node_id2()).unwrap();
        assert_eq!(engine.total_ops_all_datasets(), 2);
        assert_eq!(engine.dataset_count(), 2);
    }

    #[test]
    fn durability_engine_clear_all() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        assert_eq!(engine.dataset_count(), 1);
        engine.clear_all();
        assert_eq!(engine.dataset_count(), 0);
    }

    #[test]
    fn durability_engine_config_clone() {
        let engine = DurabilityEngine::new();
        let config = engine.config_clone();
        assert_eq!(config.default_condition, DurabilityConfig::default().default_condition);
    }

    #[test]
    fn durability_engine_clock() {
        let engine = DurabilityEngine::new();
        assert_eq!(engine.current_clock(), 0);
        engine.advance_clock_ms(5000);
        assert_eq!(engine.current_clock(), 5000);
    }

    #[test]
    fn durability_engine_timed_out_operations() {
        let engine = DurabilityEngine::new();
        let mut config = DurabilityConfig::default();
        config.durability_timeout_ms = 100;
        engine.configure_durability(test_dataset_id(), config).unwrap();
        engine.advance_clock_ms(10);
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.advance_clock_ms(200);
        let timed_out = engine.timed_out_operations(test_dataset_id()).unwrap();
        assert_eq!(timed_out.len(), 1);
    }

    #[test]
    fn durability_engine_pending_cross_region() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        let pending = engine.pending_cross_region(test_dataset_id()).unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn durability_engine_all_dataset_summaries() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.configure_durability(test_dataset_id2(), DurabilityConfig::default()).unwrap();
        let summaries = engine.all_dataset_summaries();
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn durability_engine_verification_trend() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        let trend = engine.verification_trend(test_dataset_id()).unwrap();
        assert_eq!(trend.dataset_id, test_dataset_id());
        assert_eq!(trend.total_operations, 0);
    }

    #[test]
    fn durability_engine_replica_ack_distribution() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id2()).unwrap();
        let dist = engine.replica_ack_distribution(test_dataset_id()).unwrap();
        assert_eq!(dist.total_acks, 2);
        assert_eq!(dist.unique_replicas, 2);
        assert_eq!(dist.acks_for_node(test_node_id()), 1);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn durability_full_lifecycle() {
        let engine = DurabilityEngine::new();
        let config = DurabilityConfig::strict();
        engine.configure_durability(test_dataset_id(), config).unwrap();

        // Record durability progression
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Accepted, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Executed, test_node_id()).unwrap();

        // Register replica acks
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id()).unwrap();
        engine.register_replica_ack(test_dataset_id(), test_op_ref(), test_node_id2()).unwrap();

        // Cross-region replication
        engine.record_cross_region_progress(test_dataset_id(), test_op_ref(), test_ordering_pos(), DurabilityCondition::LocalCommitted).unwrap();
        engine.record_cross_region_progress(test_dataset_id(), test_op_ref(), OrderingPos::new([2u8; 16]), DurabilityCondition::CrossRegionCompleted).unwrap();

        // Create checkpoint
        engine.create_checkpoint(test_dataset_id(), test_checkpoint_id(), test_ordering_pos()).unwrap();

        // Verify durability
        let result = engine.verify_durability(test_dataset_id(), test_op_ref()).unwrap();
        assert!(result.verified);
        assert!(result.cross_region_complete);
        assert!(result.replica_ack_count >= 2);

        // Check stats
        let stats = engine.stats(test_dataset_id()).unwrap();
        assert_eq!(stats.total_ops, 1);
        assert_eq!(stats.total_replica_acks, 2);
        assert_eq!(stats.total_cross_region_records, 2);
        assert_eq!(stats.total_checkpoints, 1);
    }

    #[test]
    fn durability_multiple_datasets() {
        let engine = DurabilityEngine::new();
        engine.configure_durability(test_dataset_id(), DurabilityConfig::default()).unwrap();
        engine.configure_durability(test_dataset_id2(), DurabilityConfig::default()).unwrap();
        engine.record_durability(test_dataset_id(), test_op_ref(), DurabilityCondition::Received, test_node_id()).unwrap();
        engine.record_durability(test_dataset_id2(), test_op_ref2(), DurabilityCondition::Executed, test_node_id2()).unwrap();
        assert_eq!(engine.datasets().len(), 2);
        assert_eq!(engine.total_ops_all_datasets(), 2);
    }

    #[test]
    fn durability_serialization_roundtrip_condition_distribution() {
        let dist = ConditionDistribution {
            received: 10,
            accepted: 5,
            executed: 3,
            local_committed: 2,
            replicas_acked: 5,
            cross_region_completed: 5,
        };
        let json = serde_json::to_string(&dist).unwrap();
        let deserialized: ConditionDistribution = serde_json::from_str(&json).unwrap();
        assert_eq!(dist.total(), deserialized.total());
    }

    #[test]
    fn durability_serialization_roundtrip_verification_trend() {
        let trend = VerificationTrend {
            dataset_id: test_dataset_id(),
            total_operations: 100,
            verified_operations: 90,
            failed_verifications: 5,
            verification_rate: 90.0,
            failure_rate: 5.56,
        };
        let json = serde_json::to_string(&trend).unwrap();
        let deserialized: VerificationTrend = serde_json::from_str(&json).unwrap();
        assert_eq!(trend.total_operations, deserialized.total_operations);
    }

    #[test]
    fn durability_serialization_roundtrip_replica_ack_distribution() {
        let mut ack_counts = HashMap::new();
        ack_counts.insert(test_node_id(), 5);
        let dist = ReplicaAckDistribution {
            dataset_id: test_dataset_id(),
            ack_counts,
            total_acks: 5,
            unique_replicas: 1,
        };
        let json = serde_json::to_string(&dist).unwrap();
        let deserialized: ReplicaAckDistribution = serde_json::from_str(&json).unwrap();
        assert_eq!(dist.total_acks, deserialized.total_acks);
    }

    #[test]
    fn durability_error_display() {
        let err = DurabilityError::DatasetNotFound { dataset_id: test_dataset_id() };
        assert!(!err.to_string().is_empty());
        let err = DurabilityError::ConditionNotMet { current: DurabilityCondition::Received, required: DurabilityCondition::Executed };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn durability_clone_types() {
        let record = DurabilityRecord::new(test_dataset_id(), test_op_ref(), DurabilityCondition::Executed, test_node_id(), 100);
        let cloned = record.clone();
        assert_eq!(record, cloned);

        let checkpoint = CheckpointRecord::new(test_dataset_id(), test_checkpoint_id(), test_ordering_pos(), 1000, 10, 100, vec![1]);
        let cloned = checkpoint.clone();
        assert_eq!(checkpoint, cloned);

        let status = CrossRegionStatus {
            dataset_id: test_dataset_id(),
            op_ref: test_op_ref(),
            replicated_regions: vec![test_ordering_pos()],
            required_regions: 2,
            complete: false,
            highest_condition: DurabilityCondition::LocalCommitted,
        };
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
