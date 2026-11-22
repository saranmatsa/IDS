//! # Sealed Trait: Replication — streams, checkpoints, retries
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Replication trait defines how data is replicated across nodes/H-units
//! with stream management, checkpoints, retry logic, and durability tracking.
//! Implementations live downstream in ids-replication and ids-* crates.

use crate::types::{
    Capacity, ConsistencyProfile, DatasetId, DurabilityCondition, Endpoint, FailureDomain,
    HUnitId, NodeId, OrderingPos, PartitionId, RecordId, RedundancyMode, RegionId,
};
use crate::errors::ReplicationError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Replication
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::ReplicationEngine {}
impl private::Sealed for super::ReplicationStream {}
impl private::Sealed for super::ReplicationCheckpoint {}
impl private::Sealed for super::ReplicationRetry {}
impl private::Sealed for super::ReplicationLagMonitor {}
impl private::Sealed for super::StreamStateTracker {}
impl private::Sealed for super::RetryPolicy {}
impl private::Sealed for super::DurabilityTracker {}
impl private::Sealed for super::StreamManager {}
impl private::Sealed for super::CheckpointManager {}
impl private::Sealed for super::RetryScheduler {}
impl private::Sealed for super::ReplicationStats {}

/// Sealed trait for data replication — streams, checkpoints, retries.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Replication: private::Sealed {
    /// Start a replication stream for a dataset.
    fn start_stream(
        &self,
        dataset_id: DatasetId,
        source: NodeId,
        targets: Vec<NodeId>,
    ) -> Result<ReplicationStream, ReplicationError>;

    /// Stop a replication stream.
    fn stop_stream(&self, stream_id: StreamId) -> Result<(), ReplicationError>;

    /// Create a checkpoint at a given ordering position.
    fn create_checkpoint(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
    ) -> Result<ReplicationCheckpoint, ReplicationError>;

    /// Restore from a checkpoint.
    fn restore_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<ReplicationCheckpoint, ReplicationError>;

    /// Schedule a retry for a failed replication operation.
    fn schedule_retry(
        &self,
        stream_id: StreamId,
        backoff_ms: u64,
    ) -> Result<(), ReplicationError>;

    /// Get replication lag for a target node.
    fn get_lag(&self, target: NodeId) -> Result<LagInfo, ReplicationError>;

    /// Get replication stats.
    fn stats(&self) -> ReplicationStats;

    /// Get the replication name for debugging.
    fn replication_name(&self) -> &'static str;

    /// Get total stream count.
    fn stream_count(&self) -> u64;

    /// Get total checkpoint count.
    fn checkpoint_count(&self) -> u64;

    /// Get total retry count.
    fn retry_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ReplicationError {
    #[error("stream not found: {stream_id:?}")]
    StreamNotFound { stream_id: StreamId },

    #[error("checkpoint not found: {checkpoint_id:?}")]
    CheckpointNotFound { checkpoint_id: CheckpointId },

    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("source node not found: {source:?}")]
    SourceNotFound { source: NodeId },

    #[error("target node not found: {target:?}")]
    TargetNotFound { target: NodeId },

    #[error("stream already exists: {stream_id:?}")]
    StreamAlreadyExists { stream_id: StreamId },

    #[error("retry limit exceeded: retries={retries}")]
    RetryLimitExceeded { retries: u64 },

    #[error("checkpoint creation failed: reason={reason}")]
    CheckpointCreationFailed { reason: String },

    #[error("checkpoint restore failed: reason={reason}")]
    CheckpointRestoreFailed { reason: String },

    #[error("durability condition not met: condition={condition:?}")]
    DurabilityNotMet { condition: DurabilityCondition },

    #[error("stream in invalid state: state={state:?}")]
    InvalidStreamState { state: StreamState },

    #[error("replication lag too high: lag_ms={lag_ms}, threshold_ms={threshold_ms}")]
    LagTooHigh { lag_ms: u64, threshold_ms: u64 },

    #[error("internal replication error: {message}")]
    Internal { message: String },
}

impl ReplicationError {
    pub fn category(&self) -> &'static str {
        match self {
            ReplicationError::StreamNotFound { .. } => "stream_not_found",
            ReplicationError::CheckpointNotFound { .. } => "checkpoint_not_found",
            ReplicationError::DatasetNotFound { .. } => "dataset_not_found",
            ReplicationError::SourceNotFound { .. } => "source_not_found",
            ReplicationError::TargetNotFound { .. } => "target_not_found",
            ReplicationError::StreamAlreadyExists { .. } => "stream_already_exists",
            ReplicationError::RetryLimitExceeded { .. } => "retry_limit_exceeded",
            ReplicationError::CheckpointCreationFailed { .. } => "checkpoint_creation_failed",
            ReplicationError::CheckpointRestoreFailed { .. } => "checkpoint_restore_failed",
            ReplicationError::DurabilityNotMet { .. } => "durability_not_met",
            ReplicationError::InvalidStreamState { .. } => "invalid_stream_state",
            ReplicationError::LagTooHigh { .. } => "lag_too_high",
            ReplicationError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ReplicationError::RetryLimitExceeded { .. }
                | ReplicationError::LagTooHigh { .. }
                | ReplicationError::InvalidStreamState { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ReplicationError::StreamNotFound { .. } => "create stream first",
            ReplicationError::CheckpointNotFound { .. } => "create checkpoint first",
            ReplicationError::DatasetNotFound { .. } => "create dataset first",
            ReplicationError::SourceNotFound { .. } => "register source node",
            ReplicationError::TargetNotFound { .. } => "register target node",
            ReplicationError::StreamAlreadyExists { .. } => "use existing stream",
            ReplicationError::RetryLimitExceeded { .. } => "increase retry limit or wait",
            ReplicationError::CheckpointCreationFailed { .. } => "check disk space and permissions",
            ReplicationError::CheckpointRestoreFailed { .. } => "verify checkpoint integrity",
            ReplicationError::DurabilityNotMet { .. } => "wait for more replicas to catch up",
            ReplicationError::InvalidStreamState { .. } => "reset or recreate stream",
            ReplicationError::LagTooHigh { .. } => "throttle writes or add replicas",
            ReplicationError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM ID & CHECKPOINT ID & LAG INFO
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(pub [u8; 32]);

impl StreamId {
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
pub struct CheckpointId(pub [u8; 32]);

impl CheckpointId {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LagInfo {
    pub target: NodeId,
    pub lag_ms: u64,
    pub bytes_behind: u64,
    pub last_heartbeat_ms: u64,
    pub recovered: bool,
}

impl LagInfo {
    pub fn new(target: NodeId, lag_ms: u64, bytes_behind: u64, last_heartbeat_ms: u64) -> Self {
        Self {
            target,
            lag_ms,
            bytes_behind,
            last_heartbeat_ms,
            recovered: false,
        }
    }

    pub fn is_stale(&self, threshold_ms: u64) -> bool {
        self.lag_ms > threshold_ms
    }

    pub fn recovery_status(&self) -> &'static str {
        if self.recovered {
            "recovered"
        } else if self.lag_ms == 0 {
            "synced"
        } else {
            "lagging"
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Pending,
    Active,
    Paused,
    Stopped,
    Failed,
    Completed,
}

impl StreamState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamState::Pending => "pending",
            StreamState::Active => "active",
            StreamState::Paused => "paused",
            StreamState::Stopped => "stopped",
            StreamState::Failed => "failed",
            StreamState::Completed => "completed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, StreamState::Stopped | StreamState::Failed | StreamState::Completed)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, StreamState::Active)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION STREAM
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ReplicationStream {
    pub stream_id: StreamId,
    pub dataset_id: DatasetId,
    pub source: NodeId,
    pub targets: Vec<NodeId>,
    pub state: StreamState,
    pub bytes_replicated: u64,
    pub records_replicated: u64,
    pub started_at_ms: u64,
    pub last_activity_ms: u64,
    pub retry_count: u64,
    pub error_count: u64,
}

impl ReplicationStream {
    pub fn new(
        stream_id: StreamId,
        dataset_id: DatasetId,
        source: NodeId,
        targets: Vec<NodeId>,
    ) -> Self {
        Self {
            stream_id,
            dataset_id,
            source,
            targets,
            state: StreamState::Pending,
            bytes_replicated: 0,
            records_replicated: 0,
            started_at_ms: 0,
            last_activity_ms: 0,
            retry_count: 0,
            error_count: 0,
        }
    }

    pub fn activate(&mut self, started_at_ms: u64) {
        self.state = StreamState::Active;
        self.started_at_ms = started_at_ms;
        self.last_activity_ms = started_at_ms;
    }

    pub fn pause(&mut self) {
        if self.state == StreamState::Active {
            self.state = StreamState::Paused;
        }
    }

    pub fn resume(&mut self, last_activity_ms: u64) {
        if self.state == StreamState::Paused {
            self.state = StreamState::Active;
            self.last_activity_ms = last_activity_ms;
        }
    }

    pub fn stop(&mut self) {
        self.state = StreamState::Stopped;
    }

    pub fn fail(&mut self) {
        self.state = StreamState::Failed;
        self.error_count += 1;
    }

    pub fn complete(&mut self) {
        self.state = StreamState::Completed;
    }

    pub fn update_progress(&mut self, bytes: u64, records: u64, activity_ms: u64) {
        self.bytes_replicated += bytes;
        self.records_replicated += records;
        self.last_activity_ms = activity_ms;
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    pub fn is_healthy(&self) -> bool {
        self.state == StreamState::Active || self.state == StreamState::Completed
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION CHECKPOINT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ReplicationCheckpoint {
    pub checkpoint_id: CheckpointId,
    pub dataset_id: DatasetId,
    pub position: OrderingPos,
    pub node_id: NodeId,
    pub created_at_ms: u64,
    pub size_bytes: u64,
    pub verified: bool,
    pub restored: bool,
}

impl ReplicationCheckpoint {
    pub fn new(
        checkpoint_id: CheckpointId,
        dataset_id: DatasetId,
        position: OrderingPos,
        node_id: NodeId,
        created_at_ms: u64,
        size_bytes: u64,
    ) -> Self {
        Self {
            checkpoint_id,
            dataset_id,
            position,
            node_id,
            created_at_ms,
            size_bytes,
            verified: false,
            restored: false,
        }
    }

    pub fn verify(&mut self) {
        self.verified = true;
    }

    pub fn mark_restored(&mut self) {
        self.restored = true;
    }

    pub fn is_verified(&self) -> bool {
        self.verified
    }

    pub fn is_restored(&self) -> bool {
        self.restored
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETRY POLICY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryBackoff {
    Fixed,
    Exponential,
    Linear,
}

impl RetryBackoff {
    pub fn as_str(&self) -> &'static str {
        match self {
            RetryBackoff::Fixed => "fixed",
            RetryBackoff::Exponential => "exponential",
            RetryBackoff::Linear => "linear",
        }
    }
}

pub struct RetryPolicy {
    pub max_retries: u64,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff: RetryBackoff,
    pub retry_count: std::cell::RefCell<u64>,
}

impl RetryPolicy {
    pub fn new(max_retries: u64, base_delay_ms: u64, max_delay_ms: u64, backoff: RetryBackoff) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
            backoff,
            retry_count: std::cell::RefCell::new(0),
        }
    }

    pub fn next_delay(&self, attempt: u64) -> u64 {
        match self.backoff {
            RetryBackoff::Fixed => self.base_delay_ms,
            RetryBackoff::Exponential => {
                (self.base_delay_ms * 2u64.pow(attempt as u32)).min(self.max_delay_ms)
            }
            RetryBackoff::Linear => (self.base_delay_ms * attempt).min(self.max_delay_ms),
        }
    }

    pub fn should_retry(&self, attempt: u64) -> bool {
        attempt < self.max_retries
    }

    pub fn record_retry(&self) {
        *self.retry_count.borrow_mut() += 1;
    }

    pub fn retry_count(&self) -> u64 {
        *self.retry_count.borrow()
    }

    pub fn reset_count(&self) {
        *self.retry_count.borrow_mut() = 0;
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(5, 100, 10000, RetryBackoff::Exponential)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM STATE TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct StreamStateTracker {
    pub streams: std::cell::RefCell<std::collections::HashMap<StreamId, ReplicationStream>>,
    pub active_count: u64,
    pub failed_count: u64,
    pub completed_count: u64,
}

impl StreamStateTracker {
    pub fn new() -> Self {
        Self {
            streams: std::cell::RefCell::new(std::collections::HashMap::new()),
            active_count: 0,
            failed_count: 0,
            completed_count: 0,
        }
    }

    pub fn add_stream(&self, stream: ReplicationStream) {
        self.streams.borrow_mut().insert(stream.stream_id, stream);
        self.active_count += 1;
    }

    pub fn get_stream(&self, stream_id: &StreamId) -> Option<ReplicationStream> {
        self.streams.borrow().get(stream_id).cloned()
    }

    pub fn update_stream(&self, stream_id: &StreamId, state: StreamState) {
        if let Some(mut s) = self.streams.borrow_mut().get_mut(stream_id) {
            match state {
                StreamState::Failed => self.failed_count += 1,
                StreamState::Completed => self.completed_count += 1,
                StreamState::Active => self.active_count += 1,
                _ => {}
            }
            s.state = state;
        }
    }

    pub fn remove_stream(&self, stream_id: &StreamId) -> bool {
        self.streams.borrow_mut().remove(stream_id).is_some()
    }

    pub fn stream_count(&self) -> usize {
        self.streams.borrow().len()
    }

    pub fn active_stream_count(&self) -> u64 {
        self.active_count
    }

    pub fn failed_stream_count(&self) -> u64 {
        self.failed_count
    }

    pub fn completed_stream_count(&self) -> u64 {
        self.completed_count
    }
}

impl Default for StreamStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct DurabilityTracker {
    pub dataset_durability: std::cell::RefCell<std::collections::HashMap<DatasetId, DurabilityCondition>>,
    pub durability_checks: u64,
    pub durability_passed: u64,
    pub durability_failed: u64,
}

impl DurabilityTracker {
    pub fn new() -> Self {
        Self {
            dataset_durability: std::cell::RefCell::new(std::collections::HashMap::new()),
            durability_checks: 0,
            durability_passed: 0,
            durability_failed: 0,
        }
    }

    pub fn set_condition(&self, dataset_id: DatasetId, condition: DurabilityCondition) {
        self.dataset_durability
            .borrow_mut()
            .insert(dataset_id, condition);
    }

    pub fn check_durability(&self, dataset_id: &DatasetId) -> bool {
        self.durability_checks += 1;
        let condition = self.dataset_durability.borrow().get(dataset_id);
        let met = match condition {
            Some(DurabilityCondition::FullReplica) => true,
            Some(DurabilityCondition::Quorum { .. }) => true,
            Some(DurabilityCondition::AllReplicas) => true,
            Some(DurabilityCondition::None) => true,
            None => true,
        };
        if met {
            self.durability_passed += 1;
        } else {
            self.durability_failed += 1;
        }
        met
    }

    pub fn check_count(&self) -> u64 {
        self.durability_checks
    }

    pub fn passed_count(&self) -> u64 {
        self.durability_passed
    }

    pub fn failed_count(&self) -> u64 {
        self.durability_failed
    }

    pub fn pass_rate(&self) -> f64 {
        if self.durability_checks == 0 {
            return 1.0;
        }
        self.durability_passed as f64 / self.durability_checks as f64
    }
}

impl Default for DurabilityTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION LAG MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct ReplicationLagMonitor {
    pub lag_samples: std::cell::RefCell<std::collections::HashMap<NodeId, Vec<u64>>>,
    pub threshold_ms: u64,
    pub lag_alerts: u64,
    pub recovery_events: u64,
}

impl ReplicationLagMonitor {
    pub fn new(threshold_ms: u64) -> Self {
        Self {
            lag_samples: std::cell::RefCell::new(std::collections::HashMap::new()),
            threshold_ms,
            lag_alerts: 0,
            recovery_events: 0,
        }
    }

    pub fn record_lag(&self, target: NodeId, lag_ms: u64) {
        let mut samples = self.lag_samples.borrow_mut();
        samples.entry(target).or_default().push(lag_ms);
        if lag_ms > self.threshold_ms {
            self.lag_alerts += 1;
        } else {
            self.recovery_events += 1;
        }
    }

    pub fn get_lag(&self, target: &NodeId) -> Option<LagInfo> {
        let samples = self.lag_samples.borrow();
        samples.get(target).and_then(|v| v.last().copied().map(|lag_ms| {
            LagInfo::new(target.clone(), lag_ms, lag_ms * 1024, std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64)
        }))
    }

    pub fn average_lag(&self, target: &NodeId) -> Option<f64> {
        let samples = self.lag_samples.borrow();
        samples.get(target).map(|v| {
            if v.is_empty() {
                0.0
            } else {
                v.iter().sum::<u64>() as f64 / v.len() as f64
            }
        })
    }

    pub fn is_stale(&self, target: &NodeId) -> bool {
        match self.get_lag(target) {
            Some(info) => info.is_stale(self.threshold_ms),
            None => false,
        }
    }

    pub fn threshold_ms(&self) -> u64 {
        self.threshold_ms
    }

    pub fn alert_count(&self) -> u64 {
        self.lag_alerts
    }

    pub fn recovery_count(&self) -> u64 {
        self.recovery_events
    }
}

impl Default for ReplicationLagMonitor {
    fn default() -> Self {
        Self::new(5000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct StreamManager {
    pub stream_limit: u64,
    pub stream_counter: u64,
    pub active_streams: std::cell::RefCell<std::collections::HashMap<StreamId, StreamId>>,
}

impl StreamManager {
    pub fn new(stream_limit: u64) -> Self {
        Self {
            stream_limit,
            stream_counter: 0,
            active_streams: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn allocate_stream_id(&mut self) -> Option<StreamId> {
        if self.active_streams.borrow().len() as u64 >= self.stream_limit {
            return None;
        }
        self.stream_counter += 1;
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&self.stream_counter.to_le_bytes());
        Some(StreamId(bytes))
    }

    pub fn register_stream(&self, stream_id: StreamId) {
        self.active_streams.borrow_mut().insert(stream_id, stream_id);
    }

    pub fn unregister_stream(&self, stream_id: &StreamId) -> bool {
        self.active_streams.borrow_mut().remove(stream_id).is_some()
    }

    pub fn stream_count(&self) -> usize {
        self.active_streams.borrow().len()
    }

    pub fn stream_limit(&self) -> u64 {
        self.stream_limit
    }

    pub fn at_limit(&self) -> bool {
        self.active_streams.borrow().len() as u64 >= self.stream_limit
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHECKPOINT MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct CheckpointManager {
    pub checkpoints: std::cell::RefCell<std::collections::HashMap<CheckpointId, ReplicationCheckpoint>>,
    pub checkpoint_counter: u64,
    pub max_checkpoints: u64,
}

impl CheckpointManager {
    pub fn new(max_checkpoints: u64) -> Self {
        Self {
            checkpoints: std::cell::RefCell::new(std::collections::HashMap::new()),
            checkpoint_counter: 0,
            max_checkpoints,
        }
    }

    pub fn create_checkpoint(
        &mut self,
        dataset_id: DatasetId,
        position: OrderingPos,
        node_id: NodeId,
        created_at_ms: u64,
        size_bytes: u64,
    ) -> Option<CheckpointId> {
        if self.checkpoints.borrow().len() as u64 >= self.max_checkpoints {
            return None;
        }
        self.checkpoint_counter += 1;
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&self.checkpoint_counter.to_le_bytes());
        let id = CheckpointId(bytes);
        let cp = ReplicationCheckpoint::new(
            id, dataset_id, position, node_id, created_at_ms, size_bytes,
        );
        self.checkpoints.borrow_mut().insert(id, cp);
        Some(id)
    }

    pub fn get_checkpoint(&self, checkpoint_id: &CheckpointId) -> Option<ReplicationCheckpoint> {
        self.checkpoints.borrow().get(checkpoint_id).cloned()
    }

    pub fn verify_checkpoint(&self, checkpoint_id: &CheckpointId) -> bool {
        if let Some(cp) = self.checkpoints.borrow_mut().get_mut(checkpoint_id) {
            cp.verify();
            true
        } else {
            false
        }
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.borrow().len()
    }

    pub fn verified_count(&self) -> usize {
        self.checkpoints.borrow().values().filter(|c| c.verified).count()
    }
}

impl Default for CheckpointManager {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETRY SCHEDULER
///////////////////////////////////////////////////////////////////////////////

pub struct RetryScheduler {
    pub retries: std::cell::RefCell<std::collections::HashMap<StreamId, u64>>,
    pub policy: RetryPolicy,
    pub total_retries: u64,
    pub failed_retries: u64,
}

impl RetryScheduler {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            retries: std::cell::RefCell::new(std::collections::HashMap::new()),
            policy,
            total_retries: 0,
            failed_retries: 0,
        }
    }

    pub fn schedule(&self, stream_id: StreamId, attempt: u64) -> Result<u64, ReplicationError> {
        if !self.policy.should_retry(attempt) {
            self.failed_retries += 1;
            return Err(ReplicationError::RetryLimitExceeded { retries: attempt });
        }
        self.policy.record_retry();
        self.total_retries += 1;
        let delay = self.policy.next_delay(attempt);
        self.retries.borrow_mut().insert(stream_id, attempt);
        Ok(delay)
    }

    pub fn get_attempt(&self, stream_id: &StreamId) -> Option<u64> {
        self.retries.borrow().get(stream_id).copied()
    }

    pub fn retry_count(&self) -> u64 {
        self.total_retries
    }

    pub fn failed_retry_count(&self) -> u64 {
        self.failed_retries
    }
}

impl Default for RetryScheduler {
    fn default() -> Self {
        Self::new(RetryPolicy::default())
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct ReplicationStats {
    pub total_streams: u64,
    pub active_streams: u64,
    pub failed_streams: u64,
    pub completed_streams: u64,
    pub total_checkpoints: u64,
    pub verified_checkpoints: u64,
    pub total_retries: u64,
    pub total_bytes_replicated: u64,
    pub total_records_replicated: u64,
    pub avg_lag_ms: f64,
    pub durability_pass_rate: f64,
}

impl ReplicationStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn health_score(&self) -> f64 {
        let total = self.total_streams.max(1);
        let success = self.completed_streams as f64 / total as f64;
        let retry_penalty = (self.total_retries as f64 / total as f64).min(1.0) * 0.3;
        (success - retry_penalty).max(0.0).min(1.0)
    }

    pub fn stream_success_rate(&self) -> f64 {
        let total = self.active_streams + self.failed_streams + self.completed_streams;
        if total == 0 {
            return 1.0;
        }
        self.completed_streams as f64 / total as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct ReplicationEngine {
    pub stream_tracker: std::cell::RefCell<StreamStateTracker>,
    pub checkpoint_manager: std::cell::RefCell<CheckpointManager>,
    pub retry_scheduler: std::cell::RefCell<RetryScheduler>,
    pub lag_monitor: std::cell::RefCell<ReplicationLagMonitor>,
    pub durability_tracker: std::cell::RefCell<DurabilityTracker>,
    pub stream_manager: std::cell::RefCell<StreamManager>,
    pub stats: std::cell::RefCell<ReplicationStats>,
}

impl ReplicationEngine {
    pub fn new() -> Self {
        Self {
            stream_tracker: std::cell::RefCell::new(StreamStateTracker::new()),
            checkpoint_manager: std::cell::RefCell::new(CheckpointManager::new(100000)),
            retry_scheduler: std::cell::RefCell::new(RetryScheduler::new(RetryPolicy::default())),
            lag_monitor: std::cell::RefCell::new(ReplicationLagMonitor::new(5000)),
            durability_tracker: std::cell::RefCell::new(DurabilityTracker::new()),
            stream_manager: std::cell::RefCell::new(StreamManager::new(10000)),
            stats: std::cell::RefCell::new(ReplicationStats::new()),
        }
    }

    pub fn start_stream(
        &self,
        dataset_id: DatasetId,
        source: NodeId,
        targets: Vec<NodeId>,
    ) -> Result<ReplicationStream, ReplicationError> {
        let mut sm = self.stream_manager.borrow_mut();
        let stream_id = sm.allocate_stream_id().ok_or_else(|| ReplicationError::Internal {
            message: "stream limit reached".to_string(),
        })?;
        drop(sm);
        let stream = ReplicationStream::new(stream_id, dataset_id, source, targets.clone());
        self.stream_tracker.borrow_mut().add_stream(stream.clone());
        self.stream_manager.borrow_mut().register_stream(stream_id);
        self.stats.borrow_mut().total_streams += 1;
        self.stats.borrow_mut().active_streams += 1;
        Ok(stream)
    }

    pub fn stop_stream(&self, stream_id: StreamId) -> Result<(), ReplicationError> {
        let exists = self.stream_tracker.borrow().get_stream(&stream_id).is_some();
        if !exists {
            return Err(ReplicationError::StreamNotFound { stream_id });
        }
        self.stream_tracker.borrow_mut().update_stream(&stream_id, StreamState::Stopped);
        self.stream_manager.borrow_mut().unregister_stream(&stream_id);
        self.stats.borrow_mut().active_streams = self.stats.borrow().active_streams.saturating_sub(1);
        self.stats.borrow_mut().failed_streams += 1;
        Ok(())
    }

    pub fn create_checkpoint(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
    ) -> Result<ReplicationCheckpoint, ReplicationError> {
        let node_id = NodeId::new([1; 32]);
        let cp = self.checkpoint_manager.borrow_mut().create_checkpoint(
            dataset_id, position, node_id.clone(), 0, 0,
        );
        match cp {
            Some(id) => {
                let checkpoint = self.checkpoint_manager.borrow().get_checkpoint(&id).unwrap();
                self.stats.borrow_mut().total_checkpoints += 1;
                Ok(checkpoint)
            }
            None => Err(ReplicationError::CheckpointCreationFailed {
                reason: "max checkpoints reached".to_string(),
            }),
        }
    }

    pub fn restore_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<ReplicationCheckpoint, ReplicationError> {
        let checkpoint = self.checkpoint_manager.borrow().get_checkpoint(&checkpoint_id);
        match checkpoint {
            Some(cp) => {
                let mut cp = cp;
                cp.mark_restored();
                self.stats.borrow_mut().verified_checkpoints += 1;
                Ok(cp)
            }
            None => Err(ReplicationError::CheckpointNotFound { checkpoint_id }),
        }
    }

    pub fn schedule_retry(
        &self,
        stream_id: StreamId,
        attempt: u64,
    ) -> Result<(), ReplicationError> {
        let _delay = self.retry_scheduler.borrow_mut().schedule(stream_id, attempt)?;
        self.stats.borrow_mut().total_retries += 1;
        Ok(())
    }

    pub fn get_lag(&self, target: NodeId) -> Result<LagInfo, ReplicationError> {
        match self.lag_monitor.borrow().get_lag(&target) {
            Some(info) => Ok(info),
            None => Err(ReplicationError::StreamNotFound {
                stream_id: StreamId::zero(),
            }),
        }
    }

    pub fn record_lag(&self, target: NodeId, lag_ms: u64) {
        self.lag_monitor.borrow_mut().record_lag(target, lag_ms);
    }

    pub fn stats(&self) -> ReplicationStats {
        self.stats.borrow().clone()
    }

    pub fn replication_name(&self) -> &'static str {
        "replication_engine"
    }

    pub fn stream_count(&self) -> u64 {
        self.stream_tracker.borrow().stream_count() as u64
    }

    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoint_manager.borrow().checkpoint_count() as u64
    }

    pub fn retry_count(&self) -> u64 {
        self.retry_scheduler.borrow().retry_count()
    }

    pub fn health_score(&self) -> f64 {
        self.stats.borrow().health_score()
    }
}

impl Default for ReplicationEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM LIFECYCLE MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLifecycleEvent {
    Created,
    Activated,
    Paused,
    Resumed,
    Stopped,
    Failed,
    Completed,
    Checkpointed,
    Retried,
    Throttled,
}

impl StreamLifecycleEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamLifecycleEvent::Created => "created",
            StreamLifecycleEvent::Activated => "activated",
            StreamLifecycleEvent::Paused => "paused",
            StreamLifecycleEvent::Resumed => "resumed",
            StreamLifecycleEvent::Stopped => "stopped",
            StreamLifecycleEvent::Failed => "failed",
            StreamLifecycleEvent::Completed => "completed",
            StreamLifecycleEvent::Checkpointed => "checkpointed",
            StreamLifecycleEvent::Retried => "retried",
            StreamLifecycleEvent::Throttled => "throttled",
        }
    }
}

pub struct StreamLifecycleManager {
    pub events: std::cell::RefCell<Vec<(StreamId, StreamLifecycleEvent, u64)>>,
    pub event_count: u64,
    pub state_transitions: std::cell::RefCell<std::collections::HashMap<StreamId, Vec<(StreamState, u64)>>>,
}

impl StreamLifecycleManager {
    pub fn new() -> Self {
        Self {
            events: std::cell::RefCell::new(Vec::new()),
            event_count: 0,
            state_transitions: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn record_event(&self, stream_id: StreamId, event: StreamLifecycleEvent, at_ms: u64) {
        self.events.borrow_mut().push((stream_id, event, at_ms));
        self.event_count += 1;
    }

    pub fn record_state_transition(&self, stream_id: StreamId, from: StreamState, to: StreamState, at_ms: u64) {
        self.state_transitions.borrow_mut().entry(stream_id).or_default().push((to, at_ms));
    }

    pub fn events_for_stream(&self, stream_id: &StreamId) -> Vec<(StreamLifecycleEvent, u64)> {
        self.events.borrow().iter()
            .filter(|(sid, _, _)| sid == stream_id)
            .map(|(_, e, t)| (e.clone(), *t))
            .collect()
    }

    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    pub fn transition_count(&self, stream_id: &StreamId) -> usize {
        self.state_transitions.borrow().get(stream_id).map(|v| v.len()).unwrap_or(0)
    }

    pub fn last_event(&self, stream_id: &StreamId) -> Option<StreamLifecycleEvent> {
        self.events.borrow().iter()
            .rev()
            .find(|(sid, _, _)| sid == stream_id)
            .map(|(_, e, _)| e.clone())
    }

    pub fn clear_events(&self) {
        self.events.borrow_mut().clear();
        self.event_count = 0;
    }
}

impl Default for StreamLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHECKPOINT INTEGRITY & VERIFICATION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointIntegrity {
    Unknown,
    Valid,
    Corrupted,
    Verified,
}

impl CheckpointIntegrity {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckpointIntegrity::Unknown => "unknown",
            CheckpointIntegrity::Valid => "valid",
            CheckpointIntegrity::Corrupted => "corrupted",
            CheckpointIntegrity::Verified => "verified",
        }
    }

    pub fn is_usable(&self) -> bool {
        matches!(self, CheckpointIntegrity::Valid | CheckpointIntegrity::Verified)
    }
}

pub struct CheckpointIntegrityChecker {
    pub checksums: std::cell::RefCell<std::collections::HashMap<CheckpointId, [u8; 32]>>,
    pub integrity: std::cell::RefCell<std::collections::HashMap<CheckpointId, CheckpointIntegrity>>,
    pub verification_count: u64,
    pub corruption_count: u64,
}

impl CheckpointIntegrityChecker {
    pub fn new() -> Self {
        Self {
            checksums: std::cell::RefCell::new(std::collections::HashMap::new()),
            integrity: std::cell::RefCell::new(std::collections::HashMap::new()),
            verification_count: 0,
            corruption_count: 0,
        }
    }

    pub fn store_checksum(&self, checkpoint_id: CheckpointId, checksum: [u8; 32]) {
        self.checksums.borrow_mut().insert(checkpoint_id, checksum);
        self.integrity.borrow_mut().insert(checkpoint_id, CheckpointIntegrity::Valid);
    }

    pub fn verify_checksum(&self, checkpoint_id: CheckpointId, provided: [u8; 32]) -> bool {
        self.verification_count += 1;
        let stored = self.checksums.borrow().get(&checkpoint_id).copied();
        match stored {
            Some(cs) if cs == provided => {
                self.integrity.borrow_mut().insert(checkpoint_id, CheckpointIntegrity::Verified);
                true
            }
            Some(_) => {
                self.integrity.borrow_mut().insert(checkpoint_id, CheckpointIntegrity::Corrupted);
                self.corruption_count += 1;
                false
            }
            None => {
                self.integrity.borrow_mut().insert(checkpoint_id, CheckpointIntegrity::Unknown);
                false
            }
        }
    }

    pub fn get_integrity(&self, checkpoint_id: &CheckpointId) -> CheckpointIntegrity {
        self.integrity.borrow().get(checkpoint_id).copied().unwrap_or(CheckpointIntegrity::Unknown)
    }

    pub fn is_usable(&self, checkpoint_id: &CheckpointId) -> bool {
        self.get_integrity(checkpoint_id).is_usable()
    }

    pub fn verification_count(&self) -> u64 {
        self.verification_count
    }

    pub fn corruption_count(&self) -> u64 {
        self.corruption_count
    }

    pub fn corruption_rate(&self) -> f64 {
        if self.verification_count == 0 {
            return 0.0;
        }
        self.corruption_count as f64 / self.verification_count as f64
    }

    pub fn checksum_count(&self) -> usize {
        self.checksums.borrow().len()
    }
}

impl Default for CheckpointIntegrityChecker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETRY BACKOFF WITH JITTER
///////////////////////////////////////////////////////////////////////////////

pub struct JitteredRetryPolicy {
    pub base_policy: RetryPolicy,
    pub jitter_percent: u8,
    pub jitter_applied: u64,
}

impl JitteredRetryPolicy {
    pub fn new(base_policy: RetryPolicy, jitter_percent: u8) -> Self {
        Self {
            base_policy,
            jitter_percent,
            jitter_applied: 0,
        }
    }

    pub fn next_delay_with_jitter(&self, attempt: u64) -> u64 {
        let base = self.base_policy.next_delay(attempt);
        if self.jitter_percent == 0 {
            return base;
        }
        let jitter = (base as f64 * self.jitter_percent as f64 / 100.0) as u64;
        let delayed = base.saturating_add(jitter);
        self.jitter_applied += 1;
        delayed.min(self.base_policy.max_delay_ms)
    }

    pub fn should_retry(&self, attempt: u64) -> bool {
        self.base_policy.should_retry(attempt)
    }

    pub fn record_retry(&self) {
        self.base_policy.record_retry();
    }

    pub fn retry_count(&self) -> u64 {
        self.base_policy.retry_count()
    }

    pub fn jitter_applied_count(&self) -> u64 {
        self.jitter_applied
    }
}

impl Default for JitteredRetryPolicy {
    fn default() -> Self {
        Self::new(RetryPolicy::default(), 10)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LAG-BASED THROTTLER
///////////////////////////////////////////////////////////////////////////////

pub struct ReplicationThrottler {
    pub target_lags: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub throttle_threshold_ms: u64,
    pub throttle_active: std::cell::RefCell<std::collections::HashSet<NodeId>>,
    pub throttle_events: u64,
    pub unthrottle_events: u64,
}

impl ReplicationThrottler {
    pub fn new(throttle_threshold_ms: u64) -> Self {
        Self {
            target_lags: std::cell::RefCell::new(std::collections::HashMap::new()),
            throttle_threshold_ms,
            throttle_active: std::cell::RefCell::new(std::collections::HashSet::new()),
            throttle_events: 0,
            unthrottle_events: 0,
        }
    }

    pub fn update_lag(&self, target: NodeId, lag_ms: u64) {
        self.target_lags.borrow_mut().insert(target, lag_ms);
        let mut active = self.throttle_active.borrow_mut();
        if lag_ms > self.throttle_threshold_ms {
            if active.insert(target) {
                self.throttle_events += 1;
            }
        } else {
            if active.remove(&target) {
                self.unthrottle_events += 1;
            }
        }
    }

    pub fn is_throttled(&self, target: &NodeId) -> bool {
        self.throttle_active.borrow().contains(target)
    }

    pub fn get_lag(&self, target: &NodeId) -> Option<u64> {
        self.target_lags.borrow().get(target).copied()
    }

    pub fn throttle_count(&self) -> usize {
        self.throttle_active.borrow().len()
    }

    pub fn throttle_events(&self) -> u64 {
        self.throttle_events
    }

    pub fn unthrottle_events(&self) -> u64 {
        self.unthrottle_events
    }

    pub fn threshold_ms(&self) -> u64 {
        self.throttle_threshold_ms
    }

    pub fn clear_throttle(&self, target: &NodeId) {
        self.throttle_active.borrow_mut().remove(target);
    }
}

impl Default for ReplicationThrottler {
    fn default() -> Self {
        Self::new(5000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY CONDITION EVALUATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityEvaluation {
    Met,
    NotMet,
    Partial,
    Unknown,
}

impl DurabilityEvaluation {
    pub fn as_str(&self) -> &'static str {
        match self {
            DurabilityEvaluation::Met => "met",
            DurabilityEvaluation::NotMet => "not_met",
            DurabilityEvaluation::Partial => "partial",
            DurabilityEvaluation::Unknown => "unknown",
        }
    }

    pub fn is_satisfied(&self) -> bool {
        matches!(self, DurabilityEvaluation::Met)
    }
}

pub struct DurabilityEvaluator {
    pub evaluations: std::cell::RefCell<Vec<(DatasetId, DurabilityCondition, DurabilityEvaluation, u64)>>,
    pub eval_count: u64,
    pub met_count: u64,
    pub not_met_count: u64,
}

impl DurabilityEvaluator {
    pub fn new() -> Self {
        Self {
            evaluations: std::cell::RefCell::new(Vec::new()),
            eval_count: 0,
            met_count: 0,
            not_met_count: 0,
        }
    }

    pub fn evaluate(
        &self,
        dataset_id: DatasetId,
        condition: DurabilityCondition,
        acked_replicas: u64,
        total_replicas: u64,
        at_ms: u64,
    ) -> DurabilityEvaluation {
        self.eval_count += 1;
        let result = match condition {
            DurabilityCondition::FullReplica => {
                if acked_replicas >= total_replicas {
                    DurabilityEvaluation::Met
                } else if acked_replicas > 0 {
                    DurabilityEvaluation::Partial
                } else {
                    DurabilityEvaluation::NotMet
                }
            }
            DurabilityCondition::Quorum { required } => {
                if acked_replicas >= required {
                    DurabilityEvaluation::Met
                } else if acked_replicas > 0 {
                    DurabilityEvaluation::Partial
                } else {
                    DurabilityEvaluation::NotMet
                }
            }
            DurabilityCondition::AllReplicas => {
                if acked_replicas == total_replicas && total_replicas > 0 {
                    DurabilityEvaluation::Met
                } else if acked_replicas > 0 {
                    DurabilityEvaluation::Partial
                } else {
                    DurabilityEvaluation::NotMet
                }
            }
            DurabilityCondition::None => DurabilityEvaluation::Met,
        };
        match result {
            DurabilityEvaluation::Met => self.met_count += 1,
            DurabilityEvaluation::NotMet => self.not_met_count += 1,
            _ => {}
        }
        self.evaluations.borrow_mut().push((dataset_id, condition, result, at_ms));
        result
    }

    pub fn eval_count(&self) -> u64 {
        self.eval_count
    }

    pub fn met_count(&self) -> u64 {
        self.met_count
    }

    pub fn not_met_count(&self) -> u64 {
        self.not_met_count
    }

    pub fn met_rate(&self) -> f64 {
        if self.eval_count == 0 {
            return 1.0;
        }
        self.met_count as f64 / self.eval_count as f64
    }

    pub fn evaluations_for_dataset(&self, dataset_id: &DatasetId) -> Vec<(DurabilityCondition, DurabilityEvaluation, u64)> {
        self.evaluations.borrow().iter()
            .filter(|(did, _, _, _)| did == dataset_id)
            .map(|(_, c, e, t)| (c.clone(), e.clone(), *t))
            .collect()
    }
}

impl Default for DurabilityEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA SELECTOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaSelectionStrategy {
    RoundRobin,
    LeastLoaded,
    Closest,
    Random,
    FailureDomainAware,
}

impl ReplicaSelectionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplicaSelectionStrategy::RoundRobin => "round_robin",
            ReplicaSelectionStrategy::LeastLoaded => "least_loaded",
            ReplicaSelectionStrategy::Closest => "closest",
            ReplicaSelectionStrategy::Random => "random",
            ReplicaSelectionStrategy::FailureDomainAware => "failure_domain_aware",
        }
    }
}

pub struct ReplicaSelector {
    pub strategy: ReplicaSelectionStrategy,
    pub selection_count: u64,
    pub round_robin_index: std::cell::RefCell<usize>,
    pub node_scores: std::cell::RefCell<std::collections::HashMap<NodeId, f64>>,
}

impl ReplicaSelector {
    pub fn new(strategy: ReplicaSelectionStrategy) -> Self {
        Self {
            strategy,
            selection_count: 0,
            round_robin_index: std::cell::RefCell::new(0),
            node_scores: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn select(
        &self,
        candidates: &[NodeId],
        count: usize,
    ) -> Vec<NodeId> {
        self.selection_count += 1;
        match self.strategy {
            ReplicaSelectionStrategy::RoundRobin => {
                let mut idx = self.round_robin_index.borrow_mut();
                let mut selected = Vec::new();
                for i in 0..count.min(candidates.len()) {
                    let pos = (*idx + i) % candidates.len();
                    selected.push(candidates[pos]);
                }
                *idx = (*idx + count.min(candidates.len())) % candidates.len();
                selected
            }
            ReplicaSelectionStrategy::LeastLoaded => {
                let scores = self.node_scores.borrow();
                let mut sorted: Vec<&NodeId> = candidates.iter().collect();
                sorted.sort_by_key(|n| scores.get(*n).copied().unwrap_or(0.0) as u64);
                sorted.into_iter().take(count.min(candidates.len())).cloned().collect()
            }
            ReplicaSelectionStrategy::Closest => {
                candidates.iter().take(count.min(candidates.len())).cloned().collect()
            }
            ReplicaSelectionStrategy::Random => {
                candidates.iter().take(count.min(candidates.len())).cloned().collect()
            }
            ReplicaSelectionStrategy::FailureDomainAware => {
                candidates.iter().take(count.min(candidates.len())).cloned().collect()
            }
        }
    }

    pub fn set_node_score(&self, node_id: NodeId, score: f64) {
        self.node_scores.borrow_mut().insert(node_id, score);
    }

    pub fn get_node_score(&self, node_id: &NodeId) -> f64 {
        self.node_scores.borrow().get(node_id).copied().unwrap_or(0.0)
    }

    pub fn selection_count(&self) -> u64 {
        self.selection_count
    }
}

impl Default for ReplicaSelector {
    fn default() -> Self {
        Self::new(ReplicaSelectionStrategy::RoundRobin)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BANDWIDTH THROTTLER FOR STREAMS
///////////////////////////////////////////////////////////////////////////////

pub struct StreamBandwidthThrottler {
    pub stream_bandwidth: std::cell::RefCell<std::collections::HashMap<StreamId, u64>>,
    pub global_limit_bps: u64,
    pub total_throttled_bytes: u64,
}

impl StreamBandwidthThrottler {
    pub fn new(global_limit_bps: u64) -> Self {
        Self {
            stream_bandwidth: std::cell::RefCell::new(std::collections::HashMap::new()),
            global_limit_bps,
            total_throttled_bytes: 0,
        }
    }

    pub fn set_stream_limit(&self, stream_id: StreamId, bps: u64) {
        self.stream_bandwidth.borrow_mut().insert(stream_id, bps);
    }

    pub fn get_stream_limit(&self, stream_id: &StreamId) -> Option<u64> {
        self.stream_bandwidth.borrow().get(stream_id).copied()
    }

    pub fn throttle_bytes(&self, stream_id: &StreamId, bytes: u64) -> u64 {
        let limit = self.stream_bandwidth.borrow().get(stream_id).copied().unwrap_or(self.global_limit_bps);
        let allowed = bytes.min(limit);
        self.total_throttled_bytes += bytes.saturating_sub(allowed);
        allowed
    }

    pub fn global_limit(&self) -> u64 {
        self.global_limit_bps
    }

    pub fn throttled_byte_count(&self) -> u64 {
        self.total_throttled_bytes
    }

    pub fn stream_limit_count(&self) -> usize {
        self.stream_bandwidth.borrow().len()
    }
}

impl Default for StreamBandwidthThrottler {
    fn default() -> Self {
        Self::new(100_000_000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICATION CONSISTENCY MODE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationConsistency {
    Strong,
    Eventual,
    BoundedStaleness,
    Session,
    Prefix,
}

impl ReplicationConsistency {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplicationConsistency::Strong => "strong",
            ReplicationConsistency::Eventual => "eventual",
            ReplicationConsistency::BoundedStaleness => "bounded_staleness",
            ReplicationConsistency::Session => "session",
            ReplicationConsistency::Prefix => "prefix",
        }
    }

    pub fn requires_ack(&self) -> bool {
        matches!(self, ReplicationConsistency::Strong | ReplicationConsistency::Session)
    }

    pub fn max_staleness_ms(&self) -> Option<u64> {
        match self {
            ReplicationConsistency::BoundedStaleness => Some(5000),
            _ => None,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS-REGION REPLICATION CONFIG
///////////////////////////////////////////////////////////////////////////////

pub struct CrossRegionReplicationConfig {
    pub source_region: RegionId,
    pub target_regions: std::cell::RefCell<Vec<RegionId>>,
    pub async_mode: bool,
    pub max_lag_ms: u64,
    pub bandwidth_bps: u64,
}

impl CrossRegionReplicationConfig {
    pub fn new(source_region: RegionId) -> Self {
        Self {
            source_region,
            target_regions: std::cell::RefCell::new(Vec::new()),
            async_mode: true,
            max_lag_ms: 10000,
            bandwidth_bps: 1_000_000_000,
        }
    }

    pub fn add_target(&self, region: RegionId) {
        self.target_regions.borrow_mut().push(region);
    }

    pub fn remove_target(&self, region: &RegionId) -> bool {
        let mut targets = self.target_regions.borrow_mut();
        if let Some(pos) = targets.iter().position(|r| r == region) {
            targets.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn target_count(&self) -> usize {
        self.target_regions.borrow().len()
    }

    pub fn set_async(&self, async_mode: bool) {
        self.async_mode = async_mode;
    }

    pub fn set_max_lag(&self, max_lag_ms: u64) {
        self.max_lag_ms = max_lag_ms;
    }
}

impl Default for CrossRegionReplicationConfig {
    fn default() -> Self {
        Self::new(RegionId::new([1; 32]))
    }
}

///////////////////////////////////////////////////////////////////////////////
// STREAM HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Failed,
}

impl StreamHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamHealth::Healthy => "healthy",
            StreamHealth::Degraded => "degraded",
            StreamHealth::Unhealthy => "unhealthy",
            StreamHealth::Failed => "failed",
        }
    }

    pub fn is_operational(&self) -> bool {
        matches!(self, StreamHealth::Healthy | StreamHealth::Degraded)
    }
}

pub struct StreamHealthMonitor {
    pub health_states: std::cell::RefCell<std::collections::HashMap<StreamId, StreamHealth>>,
    pub health_checks: u64,
    pub degradation_events: u64,
    pub recovery_events: u64,
}

impl StreamHealthMonitor {
    pub fn new() -> Self {
        Self {
            health_states: std::cell::RefCell::new(std::collections::HashMap::new()),
            health_checks: 0,
            degradation_events: 0,
            recovery_events: 0,
        }
    }

    pub fn set_health(&self, stream_id: StreamId, health: StreamHealth) {
        let prev = self.health_states.borrow().get(&stream_id).copied();
        self.health_states.borrow_mut().insert(stream_id, health);
        match (prev, health) {
            (Some(StreamHealth::Healthy), StreamHealth::Degraded) | (Some(StreamHealth::Degraded), StreamHealth::Unhealthy) => {
                self.degradation_events += 1;
            }
            (Some(StreamHealth::Unhealthy), StreamHealth::Healthy) | (Some(StreamHealth::Failed), StreamHealth::Healthy) => {
                self.recovery_events += 1;
            }
            _ => {}
        }
        self.health_checks += 1;
    }

    pub fn get_health(&self, stream_id: &StreamId) -> StreamHealth {
        self.health_states.borrow().get(stream_id).copied().unwrap_or(StreamHealth::Healthy)
    }

    pub fn is_operational(&self, stream_id: &StreamId) -> bool {
        self.get_health(stream_id).is_operational()
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_checks
    }

    pub fn degradation_event_count(&self) -> u64 {
        self.degradation_events
    }

    pub fn recovery_event_count(&self) -> u64 {
        self.recovery_events
    }

    pub fn stream_count(&self) -> usize {
        self.health_states.borrow().len()
    }
}

impl Default for StreamHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHECKPOINT GARBAGE COLLECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct CheckpointGarbageCollector {
    pub retention_ms: u64,
    pub collected_count: u64,
    pub skipped_count: u64,
}

impl CheckpointGarbageCollector {
    pub fn new(retention_ms: u64) -> Self {
        Self {
            retention_ms,
            collected_count: 0,
            skipped_count: 0,
        }
    }

    pub fn should_collect(&self, created_at_ms: u64, now_ms: u64) -> bool {
        now_ms.saturating_sub(created_at_ms) > self.retention_ms
    }

    pub fn collect(&self, checkpoints: &[ReplicationCheckpoint], now_ms: u64) -> Vec<CheckpointId> {
        let mut to_collect = Vec::new();
        for cp in checkpoints {
            if self.should_collect(cp.created_at_ms, now_ms) {
                to_collect.push(cp.checkpoint_id);
                self.collected_count += 1;
            } else {
                self.skipped_count += 1;
            }
        }
        to_collect
    }

    pub fn retention_ms(&self) -> u64 {
        self.retention_ms
    }

    pub fn collected_count(&self) -> u64 {
        self.collected_count
    }

    pub fn skipped_count(&self) -> u64 {
        self.skipped_count
    }
}

impl Default for CheckpointGarbageCollector {
    fn default() -> Self {
        Self::new(86400000) // 24 hours
    }
}

///////////////////////////////////////////////////////////////////////////////
// DURABILITY CONDITION - add missing variants used by evaluator
///////////////////////////////////////////////////////////////////////////////

impl DurabilityCondition {
    pub fn required_replicas(&self) -> Option<u64> {
        match self {
            DurabilityCondition::Quorum { required } => Some(*required),
            _ => None,
        }
    }

    pub fn is_strict(&self) -> bool {
        matches!(self, DurabilityCondition::AllReplicas | DurabilityCondition::FullReplica)
    }

    pub fn allows_partial(&self) -> bool {
        !matches!(self, DurabilityCondition::None)
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED REPLICATION ENGINE
///////////////////////////////////////////////////////////////////////////////

impl ReplicationEngine {
    pub fn new_with_features() -> Self {
        Self {
            stream_tracker: std::cell::RefCell::new(StreamStateTracker::new()),
            checkpoint_manager: std::cell::RefCell::new(CheckpointManager::new(100000)),
            retry_scheduler: std::cell::RefCell::new(RetryScheduler::new(RetryPolicy::default())),
            lag_monitor: std::cell::RefCell::new(ReplicationLagMonitor::new(5000)),
            durability_tracker: std::cell::RefCell::new(DurabilityTracker::new()),
            stream_manager: std::cell::RefCell::new(StreamManager::new(10000)),
            stats: std::cell::RefCell::new(ReplicationStats::new()),
        }
    }

    pub fn evaluate_durability(
        &self,
        dataset_id: DatasetId,
        condition: DurabilityCondition,
        acked_replicas: u64,
        total_replicas: u64,
    ) -> DurabilityEvaluation {
        let evaluator = DurabilityEvaluator::default();
        evaluator.evaluate(dataset_id, condition, acked_replicas, total_replicas, 0)
    }

    pub fn select_replicas(
        &self,
        candidates: &[NodeId],
        count: usize,
    ) -> Vec<NodeId> {
        ReplicaSelector::default().select(candidates, count)
    }

    pub fn throttle_stream_bytes(&self, stream_id: &StreamId, bytes: u64) -> u64 {
        StreamBandwidthThrottler::default().throttle_bytes(stream_id, bytes)
    }

    pub fn record_lifecycle_event(
        &self,
        stream_id: StreamId,
        event: StreamLifecycleEvent,
        at_ms: u64,
    ) {
        let mgr = StreamLifecycleManager::default();
        mgr.record_event(stream_id, event, at_ms);
    }

    pub fn verify_checkpoint_integrity(
        &self,
        checkpoint_id: CheckpointId,
        provided: [u8; 32],
    ) -> bool {
        let checker = CheckpointIntegrityChecker::default();
        checker.verify_checksum(checkpoint_id, provided)
    }

    pub fn jittered_delay(&self, attempt: u64) -> u64 {
        let policy = JitteredRetryPolicy::default();
        policy.next_delay_with_jitter(attempt)
    }

    pub fn should_throttle_target(&self, target: NodeId, lag_ms: u64) -> bool {
        let throttler = ReplicationThrottler::default();
        throttler.update_lag(target, lag_ms);
        throttler.is_throttled(&target)
    }

    pub fn stream_health(&self, stream_id: &StreamId) -> StreamHealth {
        let monitor = StreamHealthMonitor::default();
        monitor.get_health(stream_id)
    }

    pub fn set_stream_health(&self, stream_id: StreamId, health: StreamHealth) {
        let monitor = StreamHealthMonitor::default();
        monitor.set_health(stream_id, health);
    }

    pub fn garbage_collect_checkpoints(
        &self,
        checkpoints: &[ReplicationCheckpoint],
        now_ms: u64,
    ) -> Vec<CheckpointId> {
        let gc = CheckpointGarbageCollector::default();
        gc.collect(checkpoints, now_ms)
    }

    pub fn cross_region_config(&self) -> CrossRegionReplicationConfig {
        CrossRegionReplicationConfig::default()
    }

    pub fn consistency_requires_ack(&self, consistency: ReplicationConsistency) -> bool {
        consistency.requires_ack()
    }

    pub fn durability_condition_is_strict(&self, condition: DurabilityCondition) -> bool {
        condition.is_strict()
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

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_ordering_pos() -> OrderingPos {
        OrderingPos::new(1000)
    }

    fn test_stream_id() -> StreamId {
        StreamId::new([1; 32])
    }

    fn test_checkpoint_id() -> CheckpointId {
        CheckpointId::new([1; 32])
    }

    fn test_durability_condition() -> DurabilityCondition {
        DurabilityCondition::FullReplica
    }

    // === ReplicationError tests ===
    #[test]
    fn replication_error_category() {
        assert_eq!(
            ReplicationError::StreamNotFound { stream_id: test_stream_id() }.category(),
            "stream_not_found"
        );
        assert_eq!(
            ReplicationError::CheckpointNotFound { checkpoint_id: test_checkpoint_id() }.category(),
            "checkpoint_not_found"
        );
        assert_eq!(
            ReplicationError::DatasetNotFound { dataset_id: test_dataset_id() }.category(),
            "dataset_not_found"
        );
        assert_eq!(
            ReplicationError::SourceNotFound { source: test_node_id() }.category(),
            "source_not_found"
        );
        assert_eq!(
            ReplicationError::TargetNotFound { target: test_node_id() }.category(),
            "target_not_found"
        );
        assert_eq!(
            ReplicationError::StreamAlreadyExists { stream_id: test_stream_id() }.category(),
            "stream_already_exists"
        );
        assert_eq!(
            ReplicationError::RetryLimitExceeded { retries: 10 }.category(),
            "retry_limit_exceeded"
        );
        assert_eq!(
            ReplicationError::CheckpointCreationFailed { reason: "disk".to_string() }.category(),
            "checkpoint_creation_failed"
        );
        assert_eq!(
            ReplicationError::CheckpointRestoreFailed { reason: "crc".to_string() }.category(),
            "checkpoint_restore_failed"
        );
        assert_eq!(
            ReplicationError::DurabilityNotMet { condition: test_durability_condition() }.category(),
            "durability_not_met"
        );
        assert_eq!(
            ReplicationError::InvalidStreamState { state: StreamState::Failed }.category(),
            "invalid_stream_state"
        );
        assert_eq!(
            ReplicationError::LagTooHigh { lag_ms: 10000, threshold_ms: 5000 }.category(),
            "lag_too_high"
        );
        assert_eq!(
            ReplicationError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn replication_error_is_recoverable() {
        assert!(ReplicationError::RetryLimitExceeded { retries: 10 }.is_recoverable());
        assert!(ReplicationError::LagTooHigh { lag_ms: 10000, threshold_ms: 5000 }.is_recoverable());
        assert!(ReplicationError::InvalidStreamState { state: StreamState::Failed }.is_recoverable());
        assert!(!ReplicationError::DatasetNotFound { dataset_id: test_dataset_id() }.is_recoverable());
        assert!(!ReplicationError::StreamNotFound { stream_id: test_stream_id() }.is_recoverable());
    }

    #[test]
    fn replication_error_suggested_action() {
        assert_eq!(
            ReplicationError::StreamNotFound { stream_id: test_stream_id() }.suggested_action(),
            "create stream first"
        );
        assert_eq!(
            ReplicationError::RetryLimitExceeded { retries: 10 }.suggested_action(),
            "increase retry limit or wait"
        );
        assert_eq!(
            ReplicationError::DurabilityNotMet { condition: test_durability_condition() }.suggested_action(),
            "wait for more replicas to catch up"
        );
        assert_eq!(
            ReplicationError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === StreamId & CheckpointId tests ===
    #[test]
    fn stream_id_zero_and_is_zero() {
        assert!(StreamId::zero().is_zero());
        assert!(!test_stream_id().is_zero());
    }

    #[test]
    fn checkpoint_id_zero_and_is_zero() {
        assert!(CheckpointId::zero().is_zero());
        assert!(!test_checkpoint_id().is_zero());
    }

    // === LagInfo tests ===
    #[test]
    fn lag_info_new_and_stale() {
        let li = LagInfo::new(test_node_id(), 6000, 1024 * 6000, 1000);
        assert!(li.is_stale(5000));
        assert!(!li.is_stale(10000));
        assert_eq!(li.recovery_status(), "lagging");
    }

    #[test]
    fn lag_info_synced_and_recovered() {
        let mut li = LagInfo::new(test_node_id(), 0, 0, 1000);
        assert_eq!(li.recovery_status(), "synced");
        li.recovered = true;
        assert_eq!(li.recovery_status(), "recovered");
    }

    // === StreamState tests ===
    #[test]
    fn stream_state_as_str() {
        assert_eq!(StreamState::Pending.as_str(), "pending");
        assert_eq!(StreamState::Active.as_str(), "active");
        assert_eq!(StreamState::Paused.as_str(), "paused");
        assert_eq!(StreamState::Stopped.as_str(), "stopped");
        assert_eq!(StreamState::Failed.as_str(), "failed");
        assert_eq!(StreamState::Completed.as_str(), "completed");
    }

    #[test]
    fn stream_state_terminal_and_active() {
        assert!(StreamState::Stopped.is_terminal());
        assert!(StreamState::Failed.is_terminal());
        assert!(StreamState::Completed.is_terminal());
        assert!(!StreamState::Pending.is_terminal());
        assert!(!StreamState::Completed.is_terminal());
        assert!(StreamState::Active.is_active());
        assert!(!StreamState::Paused.is_active());
    }

    // === ReplicationStream tests ===
    #[test]
    fn replication_stream_activate_pause_resume_stop_fail_complete() {
        let mut s = ReplicationStream::new(
            test_stream_id(),
            test_dataset_id(),
            test_node_id(),
            vec![test_node_id(), NodeId::new([2; 32])],
        );
        assert_eq!(s.state, StreamState::Pending);
        s.activate(1000);
        assert_eq!(s.state, StreamState::Active);
        assert_eq!(s.target_count(), 2);
        s.pause();
        assert_eq!(s.state, StreamState::Paused);
        s.resume(2000);
        assert_eq!(s.state, StreamState::Active);
        assert_eq!(s.last_activity_ms, 2000);
        s.stop();
        assert_eq!(s.state, StreamState::Stopped);
    }

    #[test]
    fn replication_stream_fail_and_complete() {
        let mut s = ReplicationStream::new(test_stream_id(), test_dataset_id(), test_node_id(), vec![]);
        s.fail();
        assert_eq!(s.state, StreamState::Failed);
        assert_eq!(s.error_count, 1);
        s.complete();
        assert_eq!(s.state, StreamState::Completed);
        assert!(s.is_healthy());
    }

    #[test]
    fn replication_stream_update_progress() {
        let mut s = ReplicationStream::new(test_stream_id(), test_dataset_id(), test_node_id(), vec![]);
        s.update_progress(1024, 10, 5000);
        assert_eq!(s.bytes_replicated, 1024);
        assert_eq!(s.records_replicated, 10);
        assert_eq!(s.last_activity_ms, 5000);
        s.increment_retry();
        assert_eq!(s.retry_count, 1);
    }

    #[test]
    fn replication_stream_is_healthy() {
        let mut s = ReplicationStream::new(test_stream_id(), test_dataset_id(), test_node_id(), vec![]);
        assert!(s.is_healthy());
        s.fail();
        assert!(!s.is_healthy());
        s.complete();
        assert!(s.is_healthy());
    }

    // === RetryPolicy tests ===
    #[test]
    fn retry_policy_next_delay() {
        let rp = RetryPolicy::new(5, 100, 10000, RetryBackoff::Fixed);
        assert_eq!(rp.next_delay(0), 100);
        assert_eq!(rp.next_delay(5), 100);
    }

    #[test]
    fn retry_policy_exponential() {
        let rp = RetryPolicy::new(5, 100, 10000, RetryBackoff::Exponential);
        assert_eq!(rp.next_delay(0), 100);
        assert_eq!(rp.next_delay(1), 200);
        assert_eq!(rp.next_delay(2), 400);
    }

    #[test]
    fn retry_policy_linear() {
        let rp = RetryPolicy::new(5, 100, 10000, RetryBackoff::Linear);
        assert_eq!(rp.next_delay(0), 0);
        assert_eq!(rp.next_delay(1), 100);
        assert_eq!(rp.next_delay(3), 300);
    }

    #[test]
    fn retry_policy_should_and_count() {
        let rp = RetryPolicy::new(3, 100, 10000, RetryBackoff::Fixed);
        assert!(rp.should_retry(0));
        assert!(rp.should_retry(2));
        assert!(!rp.should_retry(3));
        rp.record_retry();
        rp.record_retry();
        assert_eq!(rp.retry_count(), 2);
        rp.reset_count();
        assert_eq!(rp.retry_count(), 0);
    }

    #[test]
    fn retry_backoff_as_str() {
        assert_eq!(RetryBackoff::Fixed.as_str(), "fixed");
        assert_eq!(RetryBackoff::Exponential.as_str(), "exponential");
        assert_eq!(RetryBackoff::Linear.as_str(), "linear");
    }

    // === StreamStateTracker tests ===
    #[test]
    fn stream_state_tracker_add_get_update_remove() {
        let tracker = StreamStateTracker::default();
        let s = ReplicationStream::new(test_stream_id(), test_dataset_id(), test_node_id(), vec![]);
        tracker.add_stream(s.clone());
        assert_eq!(tracker.stream_count(), 1);
        let found = tracker.get_stream(&test_stream_id()).unwrap();
        assert_eq!(found.stream_id, test_stream_id());
        tracker.update_stream(&test_stream_id(), StreamState::Completed);
        tracker.remove_stream(&test_stream_id());
        assert_eq!(tracker.stream_count(), 0);
    }

    #[test]
    fn stream_state_tracker_counts() {
        let tracker = StreamStateTracker::default();
        tracker.add_stream(ReplicationStream::new(test_stream_id(), test_dataset_id(), test_node_id(), vec![]));
        assert_eq!(tracker.active_stream_count(), 1);
    }

    // === DurabilityTracker tests ===
    #[test]
    fn durability_tracker_check_and_pass_rate() {
        let dt = DurabilityTracker::default();
        dt.set_condition(test_dataset_id(), test_durability_condition());
        assert!(dt.check_durability(&test_dataset_id()));
        assert_eq!(dt.check_count(), 1);
        assert_eq!(dt.passed_count(), 1);
        assert_eq!(dt.failed_count(), 0);
        assert_eq!(dt.pass_rate(), 1.0);
    }

    #[test]
    fn durability_tracker_no_condition() {
        let dt = DurabilityTracker::default();
        assert!(dt.check_durability(&test_dataset_id()));
    }

    // === ReplicationLagMonitor tests ===
    #[test]
    fn lag_monitor_record_and_get() {
        let lm = ReplicationLagMonitor::new(5000);
        lm.record_lag(test_node_id(), 6000);
        assert!(lm.is_stale(&test_node_id()));
        assert_eq!(lm.alert_count(), 1);
        let info = lm.get_lag(&test_node_id()).unwrap();
        assert_eq!(info.lag_ms, 6000);
    }

    #[test]
    fn lag_monitor_recovery() {
        let lm = ReplicationLagMonitor::new(5000);
        lm.record_lag(test_node_id(), 3000);
        assert!(!lm.is_stale(&test_node_id()));
        assert_eq!(lm.recovery_count(), 1);
    }

    #[test]
    fn lag_monitor_average() {
        let lm = ReplicationLagMonitor::new(5000);
        lm.record_lag(test_node_id(), 1000);
        lm.record_lag(test_node_id(), 3000);
        assert_eq!(lm.average_lag(&test_node_id()), Some(2000.0));
    }

    // === StreamManager tests ===
    #[test]
    fn stream_manager_allocate_and_limit() {
        let mut sm = StreamManager::new(2);
        assert!(sm.allocate_stream_id().is_some());
        assert!(sm.allocate_stream_id().is_some());
        assert!(sm.allocate_stream_id().is_none());
    }

    #[test]
    fn stream_manager_register_unregister() {
        let sm = StreamManager::new(100);
        sm.register_stream(test_stream_id());
        assert_eq!(sm.stream_count(), 1);
        assert!(sm.unregister_stream(&test_stream_id()));
        assert_eq!(sm.stream_count(), 0);
        assert!(!sm.unregister_stream(&test_stream_id()));
    }

    #[test]
    fn stream_manager_at_limit() {
        let mut sm = StreamManager::new(1);
        sm.allocate_stream_id();
        assert!(sm.at_limit());
    }

    // === CheckpointManager tests ===
    #[test]
    fn checkpoint_manager_create_get_verify() {
        let mut cm = CheckpointManager::new(100);
        let id = cm.create_checkpoint(test_dataset_id(), test_ordering_pos(), test_node_id(), 1000, 4096).unwrap();
        let cp = cm.get_checkpoint(&id).unwrap();
        assert_eq!(cp.dataset_id, test_dataset_id());
        assert!(!cp.verified);
        assert!(cm.verify_checkpoint(&id));
        let cp2 = cm.get_checkpoint(&id).unwrap();
        assert!(cp2.verified);
    }

    #[test]
    fn checkpoint_manager_limit() {
        let mut cm = CheckpointManager::new(1);
        cm.create_checkpoint(test_dataset_id(), test_ordering_pos(), test_node_id(), 1000, 4096).unwrap();
        assert!(cm.create_checkpoint(test_dataset_id(), test_ordering_pos(), test_node_id(), 1000, 4096).is_none());
    }

    #[test]
    fn checkpoint_manager_counts() {
        let mut cm = CheckpointManager::new(100);
        cm.create_checkpoint(test_dataset_id(), test_ordering_pos(), test_node_id(), 1000, 4096).unwrap();
        assert_eq!(cm.checkpoint_count(), 1);
        assert_eq!(cm.verified_count(), 0);
    }

    // === RetryScheduler tests ===
    #[test]
    fn retry_scheduler_schedule_and_fail() {
        let rs = RetryScheduler::default();
        let delay = rs.schedule(test_stream_id(), 0).unwrap();
        assert!(delay > 0);
        assert_eq!(rs.retry_count(), 1);
        // Exceed max retries
        for _ in 0..5 {
            let _ = rs.schedule(test_stream_id(), 10);
        }
        assert_eq!(rs.failed_retry_count(), 5);
    }

    #[test]
    fn retry_scheduler_get_attempt() {
        let rs = RetryScheduler::default();
        rs.schedule(test_stream_id(), 3).unwrap();
        assert_eq!(rs.get_attempt(&test_stream_id()), Some(3));
        assert_eq!(rs.get_attempt(&StreamId::new([9; 32])), None);
    }

    // === ReplicationStats tests ===
    #[test]
    fn replication_stats_health_and_success_rate() {
        let mut s = ReplicationStats::new();
        s.total_streams = 10;
        s.completed_streams = 7;
        s.failed_streams = 2;
        s.active_streams = 1;
        s.total_retries = 3;
        assert!((s.stream_success_rate() - 0.7).abs() < 0.01);
        assert!(s.health_score() > 0.0);
        assert!(s.health_score() <= 1.0);
    }

    #[test]
    fn replication_stats_default() {
        let s = ReplicationStats::default();
        assert_eq!(s.total_streams, 0);
        assert_eq!(s.health_score(), 0.0);
    }

    // === ReplicationEngine tests ===
    #[test]
    fn replication_engine_start_stop_stream() {
        let engine = ReplicationEngine::default();
        let stream = engine.start_stream(test_dataset_id(), test_node_id(), vec![NodeId::new([2; 32])]).unwrap();
        assert_eq!(stream.state, StreamState::Pending);
        assert_eq!(engine.stream_count(), 1);
        engine.stop_stream(stream.stream_id).unwrap();
        assert_eq!(engine.stats().failed_streams, 1);
    }

    #[test]
    fn replication_engine_stop_not_found() {
        let engine = ReplicationEngine::default();
        let err = engine.stop_stream(test_stream_id()).unwrap_err();
        assert!(matches!(err, ReplicationError::StreamNotFound { .. }));
    }

    #[test]
    fn replication_engine_create_restore_checkpoint() {
        let engine = ReplicationEngine::default();
        let cp = engine.create_checkpoint(test_dataset_id(), test_ordering_pos()).unwrap();
        assert_eq!(cp.dataset_id, test_dataset_id());
        assert_eq!(engine.checkpoint_count(), 1);
        let restored = engine.restore_checkpoint(cp.checkpoint_id).unwrap();
        assert!(restored.is_restored());
    }

    #[test]
    fn replication_engine_restore_not_found() {
        let engine = ReplicationEngine::default();
        let err = engine.restore_checkpoint(test_checkpoint_id()).unwrap_err();
        assert!(matches!(err, ReplicationError::CheckpointNotFound { .. }));
    }

    #[test]
    fn replication_engine_schedule_retry() {
        let engine = ReplicationEngine::default();
        engine.schedule_retry(test_stream_id(), 0).unwrap();
        assert_eq!(engine.retry_count(), 1);
    }

    #[test]
    fn replication_engine_get_lag() {
        let engine = ReplicationEngine::default();
        engine.record_lag(test_node_id(), 2000);
        let info = engine.get_lag(test_node_id()).unwrap();
        assert_eq!(info.lag_ms, 2000);
    }

    #[test]
    fn replication_engine_stats_and_health() {
        let engine = ReplicationEngine::default();
        engine.start_stream(test_dataset_id(), test_node_id(), vec![test_node_id()]).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_streams, 1);
        assert_eq!(engine.replication_name(), "replication_engine");
    }

    #[test]
    fn replication_engine_durability() {
        let engine = ReplicationEngine::default();
        engine.durability_tracker.borrow_mut().set_condition(test_dataset_id(), test_durability_condition());
        assert!(engine.durability_tracker.borrow().check_durability(&test_dataset_id()));
    }

    // === DurabilityCondition tests ===
    #[test]
    fn durability_condition_full_replica() {
        assert!(matches!(DurabilityCondition::FullReplica, DurabilityCondition::FullReplica));
    }

    // === StreamState tests extended ===
    #[test]
    fn stream_state_all_variants() {
        let states = vec![
            StreamState::Pending,
            StreamState::Active,
            StreamState::Paused,
            StreamState::Stopped,
            StreamState::Failed,
            StreamState::Completed,
        ];
        assert_eq!(states.len(), 6);
    }
// === StreamLifecycleManager tests ===
#[test]
fn stream_lifecycle_event_as_str() {
    assert_eq!(StreamLifecycleEvent::Created.as_str(), "created");
    assert_eq!(StreamLifecycleEvent::Activated.as_str(), "activated");
    assert_eq!(StreamLifecycleEvent::Paused.as_str(), "paused");
    assert_eq!(StreamLifecycleEvent::Resumed.as_str(), "resumed");
    assert_eq!(StreamLifecycleEvent::Stopped.as_str(), "stopped");
    assert_eq!(StreamLifecycleEvent::Failed.as_str(), "failed");
    assert_eq!(StreamLifecycleEvent::Completed.as_str(), "completed");
    assert_eq!(StreamLifecycleEvent::Checkpointed.as_str(), "checkpointed");
    assert_eq!(StreamLifecycleEvent::Retried.as_str(), "retried");
    assert_eq!(StreamLifecycleEvent::Throttled.as_str(), "throttled");
}

#[test]
fn stream_lifecycle_manager_record_and_query() {
    let mgr = StreamLifecycleManager::default();
    mgr.record_event(test_stream_id(), StreamLifecycleEvent::Created, 1000);
    mgr.record_event(test_stream_id(), StreamLifecycleEvent::Activated, 2000);
    assert_eq!(mgr.event_count(), 2);
    let events = mgr.events_for_stream(&test_stream_id());
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, StreamLifecycleEvent::Created);
    assert_eq!(events[0].1, 1000);
    assert_eq!(events[1].0, StreamLifecycleEvent::Activated);
    assert_eq!(events[1].1, 2000);
}

#[test]
fn stream_lifecycle_manager_state_transition() {
    let mgr = StreamLifecycleManager::default();
    mgr.record_state_transition(test_stream_id(), StreamState::Pending, StreamState::Active, 1000);
    mgr.record_state_transition(test_stream_id(), StreamState::Active, StreamState::Paused, 2000);
    assert_eq!(mgr.transition_count(&test_stream_id()), 2);
    assert_eq!(mgr.transition_count(&StreamId::new([9; 32])), 0);
}

#[test]
fn stream_lifecycle_manager_last_event() {
    let mgr = StreamLifecycleManager::default();
    assert_eq!(mgr.last_event(&test_stream_id()), None);
    mgr.record_event(test_stream_id(), StreamLifecycleEvent::Failed, 5000);
    assert_eq!(mgr.last_event(&test_stream_id()), Some(StreamLifecycleEvent::Failed));
}

#[test]
fn stream_lifecycle_manager_clear() {
    let mgr = StreamLifecycleManager::default();
    mgr.record_event(test_stream_id(), StreamLifecycleEvent::Created, 1000);
    assert_eq!(mgr.event_count(), 1);
    mgr.clear_events();
    assert_eq!(mgr.event_count(), 0);
}

// === CheckpointIntegrityChecker tests ===
#[test]
fn checkpoint_integrity_as_str() {
    assert_eq!(CheckpointIntegrity::Unknown.as_str(), "unknown");
    assert_eq!(CheckpointIntegrity::Valid.as_str(), "valid");
    assert_eq!(CheckpointIntegrity::Corrupted.as_str(), "corrupted");
    assert_eq!(CheckpointIntegrity::Verified.as_str(), "verified");
    assert!(CheckpointIntegrity::Valid.is_usable());
    assert!(CheckpointIntegrity::Verified.is_usable());
    assert!(!CheckpointIntegrity::Corrupted.is_usable());
    assert!(!CheckpointIntegrity::Unknown.is_usable());
}

#[test]
fn checkpoint_integrity_store_and_verify() {
    let checker = CheckpointIntegrityChecker::default();
    let cp_id = test_checkpoint_id();
    let checksum = [42u8; 32];
    checker.store_checksum(cp_id, checksum);
    assert!(checker.verify_checksum(cp_id, checksum));
    assert_eq!(checker.get_integrity(&cp_id), CheckpointIntegrity::Verified);
    assert!(checker.is_usable(&cp_id));
}

#[test]
fn checkpoint_integrity_mismatch() {
    let checker = CheckpointIntegrityChecker::default();
    let cp_id = test_checkpoint_id();
    checker.store_checksum(cp_id, [1u8; 32]);
    assert!(!checker.verify_checksum(cp_id, [2u8; 32]));
    assert_eq!(checker.get_integrity(&cp_id), CheckpointIntegrity::Corrupted);
    assert!(!checker.is_usable(&cp_id));
}

#[test]
fn checkpoint_integrity_unknown() {
    let checker = CheckpointIntegrityChecker::default();
    assert_eq!(checker.get_integrity(&test_checkpoint_id()), CheckpointIntegrity::Unknown);
    assert!(!checker.is_usable(&test_checkpoint_id()));
    assert_eq!(checker.checksum_count(), 0);
}

#[test]
fn checkpoint_integrity_rates() {
    let checker = CheckpointIntegrityChecker::default();
    checker.store_checksum(test_checkpoint_id(), [1u8; 32]);
    checker.verify_checksum(test_checkpoint_id(), [1u8; 32]);
    checker.verify_checksum(test_checkpoint_id(), [2u8; 32]);
    assert_eq!(checker.verification_count(), 2);
    assert_eq!(checker.corruption_count(), 1);
    assert!((checker.corruption_rate() - 0.5).abs() < 0.01);
}

// === JitteredRetryPolicy tests ===
#[test]
fn jittered_retry_delay() {
    let policy = JitteredRetryPolicy::default();
    let delay = policy.next_delay_with_jitter(0);
    assert!(delay >= 100);
    assert!(delay <= 10000);
}

#[test]
fn jittered_retry_should_and_count() {
    let policy = JitteredRetryPolicy::default();
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(4));
    assert!(!policy.should_retry(5));
    policy.record_retry();
    assert_eq!(policy.retry_count(), 1);
}

#[test]
fn jittered_retry_jitter_applied() {
    let policy = JitteredRetryPolicy::default();
    policy.next_delay_with_jitter(0);
    policy.next_delay_with_jitter(1);
    assert_eq!(policy.jitter_applied_count(), 2);
}

#[test]
fn jittered_retry_zero_jitter() {
    let base = RetryPolicy::new(5, 100, 10000, RetryBackoff::Fixed);
    let policy = JitteredRetryPolicy::new(base, 0);
    assert_eq!(policy.next_delay_with_jitter(0), 100);
}

// === ReplicationThrottler tests ===
#[test]
fn replication_throttler_update_and_check() {
    let throttler = ReplicationThrottler::default();
    throttler.update_lag(test_node_id(), 6000);
    assert!(throttler.is_throttled(&test_node_id()));
    assert_eq!(throttler.throttle_count(), 1);
}

#[test]
fn replication_throttler_unthrottle() {
    let throttler = ReplicationThrottler::new(5000);
    throttler.update_lag(test_node_id(), 6000);
    assert!(throttler.is_throttled(&test_node_id()));
    throttler.update_lag(test_node_id(), 3000);
    assert!(!throttler.is_throttled(&test_node_id()));
    assert_eq!(throttler.throttle_events(), 1);
    assert_eq!(throttler.unthrottle_events(), 1);
}

#[test]
fn replication_throttler_get_lag_and_clear() {
    let throttler = ReplicationThrottler::default();
    throttler.update_lag(test_node_id(), 4000);
    assert_eq!(throttler.get_lag(&test_node_id()), Some(4000));
    throttler.clear_throttle(&test_node_id());
    assert!(!throttler.is_throttled(&test_node_id()));
}

#[test]
fn replication_throttler_threshold() {
    let throttler = ReplicationThrottler::new(1000);
    assert_eq!(throttler.threshold_ms(), 1000);
    throttler.update_lag(test_node_id(), 500);
    assert!(!throttler.is_throttled(&test_node_id()));
}

// === DurabilityEvaluator tests ===
#[test]
fn durability_evaluation_as_str() {
    assert_eq!(DurabilityEvaluation::Met.as_str(), "met");
    assert_eq!(DurabilityEvaluation::NotMet.as_str(), "not_met");
    assert_eq!(DurabilityEvaluation::Partial.as_str(), "partial");
    assert_eq!(DurabilityEvaluation::Unknown.as_str(), "unknown");
    assert!(DurabilityEvaluation::Met.is_satisfied());
    assert!(!DurabilityEvaluation::NotMet.is_satisfied());
}

#[test]
fn durability_evaluator_full_replica() {
    let eval = DurabilityEvaluator::default();
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 3, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Met);
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 2, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Partial);
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 0, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::NotMet);
}

#[test]
fn durability_evaluator_quorum() {
    let eval = DurabilityEvaluator::default();
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::Quorum { required: 2 }, 2, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Met);
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::Quorum { required: 2 }, 1, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Partial);
}

#[test]
fn durability_evaluator_all_replicas() {
    let eval = DurabilityEvaluator::default();
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::AllReplicas, 3, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Met);
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::AllReplicas, 2, 3, 1000);
    assert_eq!(result, DurabilityEvaluation::Partial);
}

#[test]
fn durability_evaluator_none() {
    let eval = DurabilityEvaluator::default();
    let result = eval.evaluate(test_dataset_id(), DurabilityCondition::None, 0, 0, 1000);
    assert_eq!(result, DurabilityEvaluation::Met);
}

#[test]
fn durability_evaluator_rates() {
    let eval = DurabilityEvaluator::default();
    eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 3, 3, 1000);
    eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 0, 3, 1000);
    assert_eq!(eval.eval_count(), 2);
    assert_eq!(eval.met_count(), 1);
    assert_eq!(eval.not_met_count(), 1);
    assert!((eval.met_rate() - 0.5).abs() < 0.01);
}

#[test]
fn durability_evaluator_for_dataset() {
    let eval = DurabilityEvaluator::default();
    eval.evaluate(test_dataset_id(), DurabilityCondition::FullReplica, 3, 3, 1000);
    let results = eval.evaluations_for_dataset(&test_dataset_id());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].2, DurabilityEvaluation::Met);
}

// === ReplicaSelector tests ===
#[test]
fn replica_selection_strategy_as_str() {
    assert_eq!(ReplicaSelectionStrategy::RoundRobin.as_str(), "round_robin");
    assert_eq!(ReplicaSelectionStrategy::LeastLoaded.as_str(), "least_loaded");
    assert_eq!(ReplicaSelectionStrategy::Closest.as_str(), "closest");
    assert_eq!(ReplicaSelectionStrategy::Random.as_str(), "random");
    assert_eq!(ReplicaSelectionStrategy::FailureDomainAware.as_str(), "failure_domain_aware");
}

#[test]
fn replica_selector_round_robin() {
    let selector = ReplicaSelector::new(ReplicaSelectionStrategy::RoundRobin);
    let candidates = vec![test_node_id(), NodeId::new([2; 32]), NodeId::new([3; 32])];
    let selected = selector.select(&candidates, 2);
    assert_eq!(selected.len(), 2);
    assert_eq!(selector.selection_count(), 1);
}

#[test]
fn replica_selector_least_loaded() {
    let selector = ReplicaSelector::new(ReplicaSelectionStrategy::LeastLoaded);
    selector.set_node_score(test_node_id(), 10.0);
    selector.set_node_score(NodeId::new([2; 32]), 5.0);
    let candidates = vec![test_node_id(), NodeId::new([2; 32])];
    let selected = selector.select(&candidates, 1);
    assert_eq!(selected[0], NodeId::new([2; 32]));
}

#[test]
fn replica_selector_closest_and_random() {
    let selector = ReplicaSelector::new(ReplicaSelectionStrategy::Closest);
    let candidates = vec![test_node_id(), NodeId::new([2; 32])];
    let selected = selector.select(&candidates, 1);
    assert_eq!(selected.len(), 1);

    let selector = ReplicaSelector::new(ReplicaSelectionStrategy::Random);
    let selected = selector.select(&candidates, 1);
    assert_eq!(selected.len(), 1);
}

#[test]
fn replica_selector_count_and_score() {
    let selector = ReplicaSelector::default();
    let candidates = vec![test_node_id()];
    selector.select(&candidates, 1);
    assert_eq!(selector.selection_count(), 1);
    selector.set_node_score(test_node_id(), 42.0);
    assert!((selector.get_node_score(&test_node_id()) - 42.0).abs() < 0.01);
    assert_eq!(selector.get_node_score(&NodeId::new([9; 32])), 0.0);
}

// === StreamBandwidthThrottler tests ===
#[test]
fn stream_bandwidth_throttler_set_and_get() {
    let throttler = StreamBandwidthThrottler::default();
    throttler.set_stream_limit(test_stream_id(), 5000000);
    assert_eq!(throttler.get_stream_limit(&test_stream_id()), Some(5000000));
    assert_eq!(throttler.get_stream_limit(&StreamId::new([9; 32])), None);
}

#[test]
fn stream_bandwidth_throttler_throttle() {
    let throttler = StreamBandwidthThrottler::new(1000);
    let allowed = throttler.throttle_bytes(&test_stream_id(), 500);
    assert_eq!(allowed, 500);
    let allowed = throttler.throttle_bytes(&test_stream_id(), 2000);
    assert_eq!(allowed, 1000);
    assert_eq!(throttler.throttled_byte_count(), 1000);
}

#[test]
fn stream_bandwidth_throttler_default_limit() {
    let throttler = StreamBandwidthThrottler::default();
    assert_eq!(throttler.global_limit(), 100_000_000);
    let allowed = throttler.throttle_bytes(&test_stream_id(), 10);
    assert_eq!(allowed, 10);
    assert_eq!(throttler.stream_limit_count(), 0);
}

// === ReplicationConsistency tests ===
#[test]
fn replication_consistency_as_str() {
    assert_eq!(ReplicationConsistency::Strong.as_str(), "strong");
    assert_eq!(ReplicationConsistency::Eventual.as_str(), "eventual");
    assert_eq!(ReplicationConsistency::BoundedStaleness.as_str(), "bounded_staleness");
    assert_eq!(ReplicationConsistency::Session.as_str(), "session");
    assert_eq!(ReplicationConsistency::Prefix.as_str(), "prefix");
}

#[test]
fn replication_consistency_requires_ack() {
    assert!(ReplicationConsistency::Strong.requires_ack());
    assert!(ReplicationConsistency::Session.requires_ack());
    assert!(!ReplicationConsistency::Eventual.requires_ack());
    assert!(!ReplicationConsistency::Prefix.requires_ack());
}

#[test]
fn replication_consistency_max_staleness() {
    assert_eq!(ReplicationConsistency::BoundedStaleness.max_staleness_ms(), Some(5000));
    assert_eq!(ReplicationConsistency::Strong.max_staleness_ms(), None);
    assert_eq!(ReplicationConsistency::Eventual.max_staleness_ms(), None);
}

// === CrossRegionReplicationConfig tests ===
#[test]
fn cross_region_config_add_remove_target() {
    let config = CrossRegionReplicationConfig::default();
    config.add_target(RegionId::new([2; 32]));
    config.add_target(RegionId::new([3; 32]));
    assert_eq!(config.target_count(), 2);
    assert!(config.remove_target(&RegionId::new([2; 32])));
    assert_eq!(config.target_count(), 1);
    assert!(!config.remove_target(&RegionId::new([9; 32])));
}

#[test]
fn cross_region_config_async_and_lag() {
    let config = CrossRegionReplicationConfig::default();
    assert!(config.async_mode);
    assert_eq!(config.max_lag_ms, 10000);
    config.set_async(false);
    assert!(!config.async_mode);
    config.set_max_lag(2000);
    assert_eq!(config.max_lag_ms, 2000);
}

// === StreamHealthMonitor tests ===
#[test]
fn stream_health_as_str() {
    assert_eq!(StreamHealth::Healthy.as_str(), "healthy");
    assert_eq!(StreamHealth::Degraded.as_str(), "degraded");
    assert_eq!(StreamHealth::Unhealthy.as_str(), "unhealthy");
    assert_eq!(StreamHealth::Failed.as_str(), "failed");
    assert!(StreamHealth::Healthy.is_operational());
    assert!(StreamHealth::Degraded.is_operational());
    assert!(!StreamHealth::Unhealthy.is_operational());
    assert!(!StreamHealth::Failed.is_operational());
}

#[test]
fn stream_health_monitor_set_and_get() {
    let monitor = StreamHealthMonitor::default();
    assert_eq!(monitor.get_health(&test_stream_id()), StreamHealth::Healthy);
    monitor.set_health(test_stream_id(), StreamHealth::Degraded);
    assert_eq!(monitor.get_health(&test_stream_id()), StreamHealth::Degraded);
    assert!(monitor.is_operational(&test_stream_id()));
}

#[test]
fn stream_health_monitor_degradation_and_recovery() {
    let monitor = StreamHealthMonitor::default();
    monitor.set_health(test_stream_id(), StreamHealth::Degraded);
    monitor.set_health(test_stream_id(), StreamHealth::Unhealthy);
    assert_eq!(monitor.degradation_event_count(), 2);
    monitor.set_health(test_stream_id(), StreamHealth::Healthy);
    assert_eq!(monitor.recovery_event_count(), 1);
    assert_eq!(monitor.health_check_count(), 3);
}

#[test]
fn stream_health_monitor_stream_count() {
    let monitor = StreamHealthMonitor::default();
    assert_eq!(monitor.stream_count(), 0);
    monitor.set_health(test_stream_id(), StreamHealth::Healthy);
    monitor.set_health(StreamId::new([2; 32]), StreamHealth::Failed);
    assert_eq!(monitor.stream_count(), 2);
}

// === CheckpointGarbageCollector tests ===
#[test]
fn checkpoint_garbage_collector_should_collect() {
    let gc = CheckpointGarbageCollector::new(1000);
    assert!(!gc.should_collect(9000, 10000));
    assert!(gc.should_collect(8000, 10000));
}

#[test]
fn checkpoint_garbage_collector_collect() {
    let gc = CheckpointGarbageCollector::new(1000);
    let checkpoints = vec![
        ReplicationCheckpoint::new(test_checkpoint_id(), test_dataset_id(), test_ordering_pos(), test_node_id(), 5000, 4096),
        ReplicationCheckpoint::new(CheckpointId::new([2; 32]), test_dataset_id(), test_ordering_pos(), test_node_id(), 5000, 4096),
    ];
    let to_collect = gc.collect(&checkpoints, 10000);
    assert_eq!(to_collect.len(), 2);
    assert_eq!(gc.collected_count(), 2);
}

#[test]
fn checkpoint_garbage_collector_counts() {
    let gc = CheckpointGarbageCollector::default();
    assert_eq!(gc.retention_ms(), 86400000);
    assert_eq!(gc.collected_count(), 0);
    assert_eq!(gc.skipped_count(), 0);
}

// === DurabilityCondition extended tests ===
#[test]
fn durability_condition_required_replicas() {
    assert_eq!(DurabilityCondition::Quorum { required: 3 }.required_replicas(), Some(3));
    assert_eq!(DurabilityCondition::FullReplica.required_replicas(), None);
    assert_eq!(DurabilityCondition::None.required_replicas(), None);
}

#[test]
fn durability_condition_is_strict_and_allows_partial() {
    assert!(DurabilityCondition::AllReplicas.is_strict());
    assert!(DurabilityCondition::FullReplica.is_strict());
    assert!(DurabilityCondition::FullReplica.allows_partial());
    assert!(!DurabilityCondition::None.allows_partial());
}

// === ReplicationEngine extended tests ===
#[test]
fn replication_engine_evaluate_durability() {
    let engine = ReplicationEngine::default();
    let result = engine.evaluate_durability(test_dataset_id(), DurabilityCondition::FullReplica, 3, 3);
    assert_eq!(result, DurabilityEvaluation::Met);
}

#[test]
fn replication_engine_select_replicas() {
    let engine = ReplicationEngine::default();
    let candidates = vec![test_node_id(), NodeId::new([2; 32])];
    let selected = engine.select_replicas(&candidates, 1);
    assert_eq!(selected.len(), 1);
}

#[test]
fn replication_engine_throttle_stream_bytes() {
    let engine = ReplicationEngine::default();
    let allowed = engine.throttle_stream_bytes(&test_stream_id(), 100);
    assert_eq!(allowed, 100);
}

#[test]
fn replication_engine_lifecycle_and_integrity() {
    let engine = ReplicationEngine::default();
    engine.record_lifecycle_event(test_stream_id(), StreamLifecycleEvent::Created, 1000);
    assert!(engine.verify_checkpoint_integrity(test_checkpoint_id(), [1u8; 32]));
}

#[test]
fn replication_engine_jittered_delay() {
    let engine = ReplicationEngine::default();
    let delay = engine.jittered_delay(0);
    assert!(delay >= 100);
    assert!(delay <= 10000);
}

#[test]
fn replication_engine_throttle_target() {
    let engine = ReplicationEngine::default();
    assert!(engine.should_throttle_target(test_node_id(), 6000));
    assert!(!engine.should_throttle_target(test_node_id(), 3000));
}

#[test]
fn replication_engine_stream_health() {
    let engine = ReplicationEngine::default();
    assert_eq!(engine.stream_health(&test_stream_id()), StreamHealth::Healthy);
    engine.set_stream_health(test_stream_id(), StreamHealth::Degraded);
    assert_eq!(engine.stream_health(&test_stream_id()), StreamHealth::Degraded);
}

#[test]
fn replication_engine_garbage_collect() {
    let engine = ReplicationEngine::default();
    let cps = vec![ReplicationCheckpoint::new(test_checkpoint_id(), test_dataset_id(), test_ordering_pos(), test_node_id(), 1000, 4096)];
    let collected = engine.garbage_collect_checkpoints(&cps, 2000000);
    assert_eq!(collected.len(), 1);
}

#[test]
fn replication_engine_cross_region_and_consistency() {
    let engine = ReplicationEngine::default();
    let config = engine.cross_region_config();
    assert_eq!(config.target_count(), 0);
    assert!(engine.consistency_requires_ack(ReplicationConsistency::Strong));
    assert!(!engine.consistency_requires_ack(ReplicationConsistency::Eventual));
    assert!(engine.durability_condition_is_strict(DurabilityCondition::AllReplicas));
}

// === RetryBackoff tests ===
#[test]
fn retry_backoff_as_str() {
    assert_eq!(RetryBackoff::Fixed.as_str(), "fixed");
    assert_eq!(RetryBackoff::Exponential.as_str(), "exponential");
    assert_eq!(RetryBackoff::Linear.as_str(), "linear");
}

// === ReplicationStats additional tests ===
#[test]
fn replication_stats_zero_total() {
    let s = ReplicationStats::default();
    assert_eq!(s.stream_success_rate(), 1.0);
    assert_eq!(s.health_score(), 0.0);
}

#[test]
fn replication_stats_all_fields() {
    let mut s = ReplicationStats::new();
    s.total_streams = 100;
    s.active_streams = 10;
    s.failed_streams = 5;
    s.completed_streams = 85;
    s.total_checkpoints = 50;
    s.verified_checkpoints = 45;
    s.total_retries = 12;
    s.total_bytes_replicated = 1048576;
    s.total_records_replicated = 10000;
    s.avg_lag_ms = 45.5;
    s.durability_pass_rate = 0.98;
    assert_eq!(s.total_streams, 100);
    assert_eq!(s.completed_streams, 85);
    assert!((s.durability_pass_rate - 0.98).abs() < 0.01);
}
}
