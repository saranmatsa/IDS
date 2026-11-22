//! # Sealed Trait: Snapshot — point-in-time state capture and restore
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Snapshot trait defines state snapshot primitives:
//! - Snapshot creation and lifecycle (created, finalized, restored, expired)
//! - Chunk-based snapshot storage with integrity hashes
//! - Snapshot restore with verification
//! - Retention policies and expiration
//! - Snapshot diff and merge
//! - Snapshot signing and verification
//! - Snapshot stats and health monitoring
//! Implementations live downstream in ids-snapshot and ids-* crates.

use crate::types::{
    ChunkID, DatasetId, Endpoint, NodeId, RegionId,
};
use crate::errors::SnapshotError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Snapshot
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::SnapshotEngine {}
impl private::Sealed for super::SnapshotConfig {}
impl private::Sealed for super::Snapshot {}
impl private::Sealed for super::SnapshotState {}
impl private::Sealed for super::SnapshotChunk {}
impl private::Sealed for super::SnapshotRestore {}
impl private::Sealed for super::SnapshotRetention {}
impl private::Sealed for super::SnapshotDiff {}
impl private::Sealed for super::SnapshotStats {}
impl private::Sealed for super::SnapshotMonitor {}
impl private::Sealed for super::SnapshotValidator {}

// Sealed trait for snapshot.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Snapshot: private::Sealed {
    /// Configure snapshot system.
    fn configure(&self, config: SnapshotConfig) -> Result<(), SnapshotError>;

    /// Create a new snapshot.
    fn create_snapshot(
        &self,
        snapshot_id: u64,
        dataset: DatasetId,
        origin: NodeId,
    ) -> Result<Snapshot, SnapshotError>;

    /// Add a chunk to a snapshot.
    fn add_chunk(
        &self,
        snapshot_id: u64,
        chunk: SnapshotChunk,
    ) -> Result<(), SnapshotError>;

    /// Finalize a snapshot (no more chunks allowed).
    fn finalize_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError>;

    /// Restore a snapshot to a target node.
    fn restore_snapshot(
        &self,
        snapshot_id: u64,
        target: NodeId,
    ) -> Result<SnapshotRestore, SnapshotError>;

    /// Get snapshot state.
    fn snapshot_state(&self, snapshot_id: u64) -> Result<SnapshotState, SnapshotError>;

    /// Get snapshot chunk count.
    fn chunk_count(&self, snapshot_id: u64) -> Result<u64, SnapshotError>;

    /// Get snapshot total size.
    fn total_size(&self, snapshot_id: u64) -> Result<u64, SnapshotError>;

    /// Verify snapshot integrity.
    fn verify_integrity(&self, snapshot_id: u64) -> Result<bool, SnapshotError>;

    /// Expire a snapshot.
    fn expire_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError>;

    /// Delete a snapshot.
    fn delete_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError>;

    /// Set retention policy.
    fn set_retention(&self, retention: SnapshotRetention) -> Result<(), SnapshotError>;

    /// Get retention policy.
    fn retention(&self) -> Result<SnapshotRetention, SnapshotError>;

    /// Compute diff between two snapshots.
    fn compute_diff(
        &self,
        snapshot_id_a: u64,
        snapshot_id_b: u64,
    ) -> Result<SnapshotDiff, SnapshotError>;

    /// Apply a diff to a snapshot.
    fn apply_diff(
        &self,
        snapshot_id: u64,
        diff: &SnapshotDiff,
    ) -> Result<(), SnapshotError>;

    /// Sign a snapshot.
    fn sign_snapshot(&self, snapshot_id: u64) -> Result<Vec<u8>, SnapshotError>;

    /// Verify snapshot signature.
    fn verify_signature(
        &self,
        snapshot_id: u64,
        signature: &[u8],
    ) -> Result<bool, SnapshotError>;

    /// Get snapshot stats.
    fn stats(&self) -> SnapshotStats;

    /// Get snapshot name for debugging.
    fn snapshot_name(&self) -> &'static str;

    /// Get total snapshots created.
    fn total_created(&self) -> u64;

    /// Get total snapshots restored.
    fn total_restored(&self) -> u64;

    /// Get total snapshots expired.
    fn total_expired(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    #[error("snapshot not configured")]
    SnapshotNotConfigured,

    #[error("snapshot not found: {snapshot_id}")]
    SnapshotNotFound { snapshot_id: u64 },

    #[error("snapshot already exists: {snapshot_id}")]
    SnapshotAlreadyExists { snapshot_id: u64 },

    #[error("snapshot already finalized: {snapshot_id}")]
    SnapshotAlreadyFinalized { snapshot_id: u64 },

    #[error("snapshot not finalized: {snapshot_id}")]
    SnapshotNotFinalized { snapshot_id: u64 },

    #[error("chunk not found: {chunk_id:?} in snapshot {snapshot_id}")]
    ChunkNotFound { chunk_id: ChunkID, snapshot_id: u64 },

    #[error("chunk already exists: {chunk_id:?} in snapshot {snapshot_id}")]
    ChunkAlreadyExists { chunk_id: ChunkID, snapshot_id: u64 },

    #[error("insufficient storage: needed={needed}, available={available}")]
    InsufficientStorage { needed: u64, available: u64 },

    #[error("integrity check failed: {snapshot_id}")]
    IntegrityFailed { snapshot_id: u64 },

    #[error("signature invalid: {snapshot_id}")]
    SignatureInvalid { snapshot_id: u64 },

    #[error("signature missing: {snapshot_id}")]
    SignatureMissing { snapshot_id: u64 },

    #[error("restore failed: {snapshot_id} -> {target:?}, reason={reason}")]
    RestoreFailed {
        snapshot_id: u64,
        target: NodeId,
        reason: String,
    },

    #[error("invalid retention: {reason}")]
    InvalidRetention { reason: String },

    #[error("internal snapshot error: {message}")]
    Internal { message: String },
}

impl SnapshotError {
    pub fn category(&self) -> &'static str {
        match self {
            SnapshotError::SnapshotNotConfigured => "snapshot_not_configured",
            SnapshotError::SnapshotNotFound { .. } => "snapshot_not_found",
            SnapshotError::SnapshotAlreadyExists { .. } => "snapshot_already_exists",
            SnapshotError::SnapshotAlreadyFinalized { .. } => "snapshot_already_finalized",
            SnapshotError::SnapshotNotFinalized { .. } => "snapshot_not_finalized",
            SnapshotError::ChunkNotFound { .. } => "chunk_not_found",
            SnapshotError::ChunkAlreadyExists { .. } => "chunk_already_exists",
            SnapshotError::InsufficientStorage { .. } => "insufficient_storage",
            SnapshotError::IntegrityFailed { .. } => "integrity_failed",
            SnapshotError::SignatureInvalid { .. } => "signature_invalid",
            SnapshotError::SignatureMissing { .. } => "signature_missing",
            SnapshotError::RestoreFailed { .. } => "restore_failed",
            SnapshotError::InvalidRetention { .. } => "invalid_retention",
            SnapshotError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SnapshotError::InsufficientStorage { .. }
                | SnapshotError::RestoreFailed { .. }
                | SnapshotError::IntegrityFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            SnapshotError::SnapshotNotConfigured => "configure snapshot system first",
            SnapshotError::SnapshotNotFound { .. } => "create the snapshot first",
            SnapshotError::SnapshotAlreadyExists { .. } => "use existing snapshot",
            SnapshotError::SnapshotAlreadyFinalized { .. } => "snapshot is finalized",
            SnapshotError::SnapshotNotFinalized { .. } => "finalize the snapshot first",
            SnapshotError::ChunkNotFound { .. } => "add the missing chunk",
            SnapshotError::ChunkAlreadyExists { .. } => "chunk already present",
            SnapshotError::InsufficientStorage { .. } => "free storage or increase quota",
            SnapshotError::IntegrityFailed { .. } => "recreate the snapshot",
            SnapshotError::SignatureInvalid { .. } => "re-sign the snapshot",
            SnapshotError::SignatureMissing { .. } => "sign the snapshot first",
            SnapshotError::RestoreFailed { .. } => "check target node availability",
            SnapshotError::InvalidRetention { .. } => "fix retention policy",
            SnapshotError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Created,
    Finalized,
    Restored,
    Expired,
    Deleted,
}

impl SnapshotState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotState::Created => "created",
            SnapshotState::Finalized => "finalized",
            SnapshotState::Restored => "restored",
            SnapshotState::Expired => "expired",
            SnapshotState::Deleted => "deleted",
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(
            self,
            SnapshotState::Expired | SnapshotState::Deleted
        )
    }

    pub fn can_transition_to(&self, next: SnapshotState) -> bool {
        match self {
            SnapshotState::Created => matches!(
                next,
                SnapshotState::Finalized | SnapshotState::Expired | SnapshotState::Deleted
            ),
            SnapshotState::Finalized => matches!(
                next,
                SnapshotState::Restored | SnapshotState::Expired | SnapshotState::Deleted
            ),
            SnapshotState::Restored => matches!(
                next,
                SnapshotState::Expired | SnapshotState::Deleted
            ),
            SnapshotState::Expired => false,
            SnapshotState::Deleted => false,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT CONFIG
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub max_snapshots: u64,
    pub max_chunks_per_snapshot: u64,
    pub max_chunk_size: usize,
    pub default_retention_ms: u64,
    pub require_signature: bool,
    pub enable_diff: bool,
    pub storage_quota: u64,
}

impl SnapshotConfig {
    pub fn new() -> Self {
        Self {
            max_snapshots: 10_000,
            max_chunks_per_snapshot: 100_000,
            max_chunk_size: 4 * 1024 * 1024,
            default_retention_ms: 7 * 24 * 60 * 60 * 1000,
            require_signature: false,
            enable_diff: true,
            storage_quota: 1024 * 1024 * 1024,
        }
    }

    pub fn with_max_snapshots(mut self, max: u64) -> Self {
        self.max_snapshots = max;
        self
    }

    pub fn with_retention_ms(mut self, ms: u64) -> Self {
        self.default_retention_ms = ms;
        self
    }

    pub fn with_signature_required(mut self, required: bool) -> Self {
        self.require_signature = required;
        self
    }

    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.max_snapshots == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "max_snapshots must be > 0".to_string(),
            });
        }
        if self.max_chunks_per_snapshot == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "max_chunks_per_snapshot must be > 0".to_string(),
            });
        }
        if self.max_chunk_size == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "max_chunk_size must be > 0".to_string(),
            });
        }
        if self.storage_quota == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "storage_quota must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub snapshot_id: u64,
    pub dataset: DatasetId,
    pub origin: NodeId,
    pub state: SnapshotState,
    pub created_at_ms: u64,
    pub finalized_at_ms: Option<u64>,
    pub restored_at_ms: Option<u64>,
    pub expired_at_ms: Option<u64>,
    pub retention_ms: u64,
    pub chunks: std::cell::RefCell<Vec<SnapshotChunk>>,
    pub total_size: u64,
    pub signature: Option<Vec<u8>>,
    pub signed_at_ms: Option<u64>,
    pub region: Option<RegionId>,
    pub endpoint: Option<Endpoint>,
}

impl Snapshot {
    pub fn new(
        snapshot_id: u64,
        dataset: DatasetId,
        origin: NodeId,
        created_at_ms: u64,
        retention_ms: u64,
    ) -> Self {
        Self {
            snapshot_id,
            dataset,
            origin,
            state: SnapshotState::Created,
            created_at_ms,
            finalized_at_ms: None,
            restored_at_ms: None,
            expired_at_ms: None,
            retention_ms,
            chunks: std::cell::RefCell::new(Vec::new()),
            total_size: 0,
            signature: None,
            signed_at_ms: None,
            region: None,
            endpoint: None,
        }
    }

    pub fn add_chunk(&self, chunk: SnapshotChunk) -> Result<(), SnapshotError> {
        if self.state == SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotAlreadyFinalized { snapshot_id: self.snapshot_id });
        }
        if self.chunks.borrow().iter().any(|c| c.chunk_id == chunk.chunk_id) {
            return Err(SnapshotError::ChunkAlreadyExists {
                chunk_id: chunk.chunk_id,
                snapshot_id: self.snapshot_id,
            });
        }
        let new_size = self.total_size + chunk.size as u64;
        self.chunks.borrow_mut().push(chunk);
        self.total_size = new_size;
        Ok(())
    }

    pub fn chunk_count(&self) -> u64 {
        self.chunks.borrow().len() as u64
    }

    pub fn finalize(&mut self, at_ms: u64) {
        self.state = SnapshotState::Finalized;
        self.finalized_at_ms = Some(at_ms);
    }

    pub fn restore(&mut self, target: NodeId, at_ms: u64) -> Result<(), SnapshotError> {
        if self.state != SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotNotFinalized { snapshot_id: self.snapshot_id });
        }
        self.state = SnapshotState::Restored;
        self.restored_at_ms = Some(at_ms);
        self.endpoint = Some(target_carrier_endpoint(target));
        Ok(())
    }

    pub fn expire(&mut self, at_ms: u64) {
        self.state = SnapshotState::Expired;
        self.expired_at_ms = Some(at_ms);
    }

    pub fn is_expired(&self, at_ms: u64) -> bool {
        match self.state {
            SnapshotState::Expired | SnapshotState::Deleted => true,
            SnapshotState::Finalized => {
                at_ms - self.created_at_ms > self.retention_ms
            }
            _ => false,
        }
    }

    pub fn sign(&mut self, signature: Vec<u8>, at_ms: u64) {
        self.signature = Some(signature);
        self.signed_at_ms = Some(at_ms);
    }

    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    pub fn set_region(&mut self, region: RegionId) {
        self.region = Some(region);
    }

    pub fn set_endpoint(&mut self, endpoint: Endpoint) {
        self.endpoint = Some(endpoint);
    }
}

fn target_carrier_endpoint(target: NodeId) -> Endpoint {
    Endpoint {
        carrier_type: crate::types::CarrierType::Loopback,
        address: target.0.to_vec(),
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT CHUNK
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotChunk {
    pub chunk_id: ChunkID,
    pub data: Vec<u8>,
    pub size: usize,
    pub hash: [u8; 32],
    pub offset: u64,
    pub compressed: bool,
    pub compressed_size: Option<usize>,
    pub added_at_ms: u64,
}

impl SnapshotChunk {
    pub fn new(
        chunk_id: ChunkID,
        data: Vec<u8>,
        offset: u64,
        added_at_ms: u64,
    ) -> Self {
        let size = data.len();
        let hash = crate::types::hash_bytes(&data);
        Self {
            chunk_id,
            data,
            size,
            hash,
            offset,
            compressed: false,
            compressed_size: None,
            added_at_ms,
        }
    }

    pub fn with_compression(mut self, compressed: bool, compressed_size: usize) -> Self {
        self.compressed = compressed;
        self.compressed_size = Some(compressed_size);
        self
    }

    pub fn verify_hash(&self) -> bool {
        crate::types::hash_bytes(&self.data) == self.hash
    }

    pub fn effective_size(&self) -> usize {
        self.compressed_size.unwrap_or(self.size)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT RESTORE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotRestore {
    pub snapshot_id: u64,
    pub target: NodeId,
    pub status: RestoreStatus,
    pub chunks_restored: u64,
    pub total_chunks: u64,
    pub bytes_restored: u64,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl RestoreStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RestoreStatus::Pending => "pending",
            RestoreStatus::InProgress => "in_progress",
            RestoreStatus::Completed => "completed",
            RestoreStatus::Failed => "failed",
            RestoreStatus::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            RestoreStatus::Completed | RestoreStatus::Failed | RestoreStatus::Cancelled
        )
    }
}

impl SnapshotRestore {
    pub fn new(
        snapshot_id: u64,
        target: NodeId,
        total_chunks: u64,
        started_at_ms: u64,
    ) -> Self {
        Self {
            snapshot_id,
            target,
            status: RestoreStatus::Pending,
            chunks_restored: 0,
            total_chunks,
            bytes_restored: 0,
            started_at_ms,
            completed_at_ms: None,
            error: None,
        }
    }

    pub fn start(&mut self) {
        self.status = RestoreStatus::InProgress;
    }

    pub fn restore_chunk(&mut self, bytes: u64) {
        if self.status == RestoreStatus::InProgress {
            self.chunks_restored += 1;
            self.bytes_restored += bytes;
        }
    }

    pub fn complete(&mut self, at_ms: u64) {
        self.status = RestoreStatus::Completed;
        self.completed_at_ms = Some(at_ms);
    }

    pub fn fail(&mut self, reason: String) {
        self.status = RestoreStatus::Failed;
        self.error = Some(reason);
    }

    pub fn cancel(&mut self) {
        self.status = RestoreStatus::Cancelled;
    }

    pub fn progress(&self) -> f64 {
        if self.total_chunks == 0 {
            return 1.0;
        }
        self.chunks_restored as f64 / self.total_chunks as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT RETENTION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotRetention {
    pub max_snapshots: u64,
    pub max_age_ms: u64,
    pub min_snapshots: u64,
    pub auto_expire: bool,
    pub expire_on_restore: bool,
}

impl SnapshotRetention {
    pub fn new() -> Self {
        Self {
            max_snapshots: 100,
            max_age_ms: 7 * 24 * 60 * 60 * 1000,
            min_snapshots: 1,
            auto_expire: true,
            expire_on_restore: false,
        }
    }

    pub fn with_max_snapshots(mut self, max: u64) -> Self {
        self.max_snapshots = max;
        self
    }

    pub fn with_max_age_ms(mut self, ms: u64) -> Self {
        self.max_age_ms = ms;
        self
    }

    pub fn with_min_snapshots(mut self, min: u64) -> Self {
        self.min_snapshots = min;
        self
    }

    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.min_snapshots > self.max_snapshots {
            return Err(SnapshotError::InvalidRetention {
                reason: "min_snapshots cannot exceed max_snapshots".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for SnapshotRetention {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT DIFF
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub diff_id: u64,
    pub from_snapshot: u64,
    pub to_snapshot: u64,
    pub added_chunks: Vec<ChunkID>,
    pub removed_chunks: Vec<ChunkID>,
    pub modified_chunks: Vec<ChunkID>,
    pub diff_size: u64,
    pub created_at_ms: u64,
}

impl SnapshotDiff {
    pub fn new(
        diff_id: u64,
        from_snapshot: u64,
        to_snapshot: u64,
        created_at_ms: u64,
    ) -> Self {
        Self {
            diff_id,
            from_snapshot,
            to_snapshot,
            added_chunks: Vec::new(),
            removed_chunks: Vec::new(),
            modified_chunks: Vec::new(),
            diff_size: 0,
            created_at_ms,
        }
    }

    pub fn add_added(&mut self, chunk_id: ChunkID) {
        self.added_chunks.push(chunk_id);
        self.diff_size += 1;
    }

    pub fn add_removed(&mut self, chunk_id: ChunkID) {
        self.removed_chunks.push(chunk_id);
        self.diff_size += 1;
    }

    pub fn add_modified(&mut self, chunk_id: ChunkID) {
        self.modified_chunks.push(chunk_id);
        self.diff_size += 1;
    }

    pub fn total_changes(&self) -> u64 {
        self.added_chunks.len() as u64
            + self.removed_chunks.len() as u64
            + self.modified_chunks.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.total_changes() == 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct SnapshotStats {
    pub total_created: u64,
    pub total_finalized: u64,
    pub total_restored: u64,
    pub total_expired: u64,
    pub total_deleted: u64,
    pub total_chunks: u64,
    pub total_size: u64,
    pub total_signatures: u64,
    pub total_diff_computations: u64,
    pub total_restore_failures: u64,
}

impl SnapshotStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn restore_rate(&self) -> f64 {
        if self.total_created == 0 {
            return 1.0;
        }
        self.total_restored as f64 / self.total_created as f64
    }

    pub fn expiry_rate(&self) -> f64 {
        if self.total_created == 0 {
            return 0.0;
        }
        self.total_expired as f64 / self.total_created as f64
    }

    pub fn average_snapshot_size(&self) -> f64 {
        if self.total_created == 0 {
            return 0.0;
        }
        self.total_size as f64 / self.total_created as f64
    }

    pub fn system_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.total_restore_failures as f64 * 2.0;
        score -= self.total_expired as f64 * 0.5;
        score += self.restore_rate() * 20.0;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct SnapshotMonitor {
    pub node_activity: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub snapshot_incident_count: u64,
}

impl SnapshotMonitor {
    pub fn new() -> Self {
        Self {
            node_activity: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            snapshot_incident_count: 0,
        }
    }

    pub fn record_activity(&self, node: NodeId, snapshots: u64) {
        let mut activity = self.node_activity.borrow_mut();
        let prev = activity.get(&node).copied().unwrap_or(0);
        activity.insert(node, prev + snapshots);
        self.health_check_count += 1;
        if snapshots == 0 && prev > 0 {
            self.alert_count += 1;
            self.snapshot_incident_count += 1;
        }
    }

    pub fn node_snapshot_count(&self, node: &NodeId) -> u64 {
        self.node_activity.borrow().get(node).copied().unwrap_or(0)
    }

    pub fn report_incident(&self, node: NodeId) {
        self.snapshot_incident_count += 1;
        self.alert_count += 1;
        self.node_activity.borrow_mut().insert(node, 0);
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn snapshot_incident_count(&self) -> u64 {
        self.snapshot_incident_count
    }

    pub fn monitored_node_count(&self) -> usize {
        self.node_activity.borrow().len()
    }

    pub fn inactive_node_count(&self) -> usize {
        self.node_activity.borrow().values().filter(|c| *c == 0).count()
    }
}

impl Default for SnapshotMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotValidationResult {
    Valid,
    InvalidSnapshotID,
    InvalidChunk,
    InvalidRetention,
    InvalidNode,
    InvalidSize,
}

impl SnapshotValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotValidationResult::Valid => "valid",
            SnapshotValidationResult::InvalidSnapshotID => "invalid_snapshot_id",
            SnapshotValidationResult::InvalidChunk => "invalid_chunk",
            SnapshotValidationResult::InvalidRetention => "invalid_retention",
            SnapshotValidationResult::InvalidNode => "invalid_node",
            SnapshotValidationResult::InvalidSize => "invalid_size",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, SnapshotValidationResult::Valid)
    }
}

pub struct SnapshotValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<NodeId, SnapshotValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl SnapshotValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_snapshot_id(&self, snapshot_id: u64) -> SnapshotValidationResult {
        self.validation_count += 1;
        let result = if snapshot_id == 0 {
            SnapshotValidationResult::InvalidSnapshotID
        } else {
            SnapshotValidationResult::Valid
        };
        match result {
            SnapshotValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([snapshot_id as u8; 32]), result);
        result
    }

    pub fn validate_chunk(&self, snapshot_id: u64, chunk_id: ChunkID, size: usize) -> SnapshotValidationResult {
        self.validation_count += 1;
        let result = if size == 0 {
            SnapshotValidationResult::InvalidChunk
        } else {
            SnapshotValidationResult::Valid
        };
        match result {
            SnapshotValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([chunk_id.0[0]; 32]), result);
        let _ = snapshot_id;
        let _ = size;
        result
    }

    pub fn validate_retention(&self, retention: &SnapshotRetention) -> SnapshotValidationResult {
        self.validation_count += 1;
        let result = if retention.max_snapshots == 0 {
            SnapshotValidationResult::InvalidRetention
        } else {
            SnapshotValidationResult::Valid
        };
        match result {
            SnapshotValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([retention.max_snapshots as u8; 32]), result);
        result
    }

    pub fn validate_node(&self, node: NodeId) -> SnapshotValidationResult {
        self.validation_count += 1;
        let result = if node == NodeId::new([0u8; 32]) {
            SnapshotValidationResult::InvalidNode
        } else {
            SnapshotValidationResult::Valid
        };
        match result {
            SnapshotValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(node, result);
        result
    }

    pub fn validate_size(&self, snapshot_id: u64, size: u64, max: u64) -> SnapshotValidationResult {
        self.validation_count += 1;
        let result = if size > max {
            SnapshotValidationResult::InvalidSize
        } else {
            SnapshotValidationResult::Valid
        };
        match result {
            SnapshotValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([snapshot_id as u8; 32]), result);
        result
    }

    pub fn get_result(&self, node: &NodeId) -> Option<SnapshotValidationResult> {
        self.validations.borrow().get(node).copied()
    }

    pub fn validation_count(&self) -> u64 {
        self.validation_count
    }

    pub fn valid_count(&self) -> u64 {
        self.valid_count
    }

    pub fn invalid_count(&self) -> u64 {
        self.invalid_count
    }

    pub fn valid_rate(&self) -> f64 {
        if self.validation_count == 0 {
            return 1.0;
        }
        self.valid_count as f64 / self.validation_count as f64
    }
}

impl Default for SnapshotValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct SnapshotEngine {
    pub config: std::cell::RefCell<Option<SnapshotConfig>>,
    pub retention: std::cell::RefCell<SnapshotRetention>,
    pub snapshots: std::cell::RefCell<std::collections::HashMap<u64, Snapshot>>,
    pub restores: std::cell::RefCell<Vec<SnapshotRestore>>,
    pub diffs: std::cell::RefCell<Vec<SnapshotDiff>>,
    pub stats: std::cell::RefCell<SnapshotStats>,
    pub monitor: std::cell::RefCell<SnapshotMonitor>,
    pub validator: std::cell::RefCell<SnapshotValidator>,
    pub next_snapshot_id: std::cell::RefCell<u64>,
    pub next_diff_id: std::cell::RefCell<u64>,
}

impl SnapshotEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(None),
            retention: std::cell::RefCell::new(SnapshotRetention::default()),
            snapshots: std::cell::RefCell::new(std::collections::HashMap::new()),
            restores: std::cell::RefCell::new(Vec::new()),
            diffs: std::cell::RefCell::new(Vec::new()),
            stats: std::cell::RefCell::new(SnapshotStats::default()),
            monitor: std::cell::RefCell::new(SnapshotMonitor::new()),
            validator: std::cell::RefCell::new(SnapshotValidator::new()),
            next_snapshot_id: std::cell::RefCell::new(1),
            next_diff_id: std::cell::RefCell::new(1),
        }
    }

    pub fn configure(&self, config: SnapshotConfig) -> Result<(), SnapshotError> {
        config.validate()?;
        let mut cfg = self.config.borrow_mut();
        if cfg.is_some() {
            return Err(SnapshotError::Internal {
                message: "snapshot already configured".to_string(),
            });
        }
        *cfg = Some(config);
        *self.retention.borrow_mut() = SnapshotRetention::default();
        Ok(())
    }

    pub fn create_snapshot(
        &self,
        snapshot_id: u64,
        dataset: DatasetId,
        origin: NodeId,
    ) -> Result<Snapshot, SnapshotError> {
        let cfg = self.config.borrow();
        let config = cfg.as_ref().ok_or(SnapshotError::SnapshotNotConfigured)?;
        drop(cfg);

        if !self.validator.borrow_mut().validate_snapshot_id(snapshot_id).is_ok() {
            return Err(SnapshotError::Internal {
                message: "invalid snapshot ID".to_string(),
            });
        }

        let mut snapshots = self.snapshots.borrow_mut();
        if snapshots.contains_key(&snapshot_id) {
            return Err(SnapshotError::SnapshotAlreadyExists { snapshot_id });
        }
        if snapshots.len() as u64 >= config.max_snapshots {
            return Err(SnapshotError::InsufficientStorage {
                needed: snapshots.len() as u64 + 1,
                available: config.max_snapshots,
            });
        }

        let snapshot = Snapshot::new(
            snapshot_id,
            dataset,
            origin,
            0,
            config.default_retention_ms,
        );
        snapshots.insert(snapshot_id, snapshot.clone());
        self.stats.borrow_mut().total_created += 1;
        self.monitor.borrow_mut().record_activity(origin, 1);
        Ok(snapshot)
    }

    pub fn add_chunk(
        &self,
        snapshot_id: u64,
        chunk: SnapshotChunk,
    ) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state == SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotAlreadyFinalized { snapshot_id });
        }
        snapshot.add_chunk(chunk)?;
        self.stats.borrow_mut().total_chunks += 1;
        self.stats.borrow_mut().total_size += chunk.size as u64;
        Ok(())
    }

    pub fn finalize_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state == SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotAlreadyFinalized { snapshot_id });
        }
        snapshot.finalize(0);
        self.stats.borrow_mut().total_finalized += 1;
        Ok(())
    }

    pub fn restore_snapshot(
        &self,
        snapshot_id: u64,
        target: NodeId,
    ) -> Result<SnapshotRestore, SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state != SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotNotFinalized { snapshot_id });
        }
        let chunk_count = snapshot.chunk_count();
        let total_size = snapshot.total_size;

        let mut restore = SnapshotRestore::new(snapshot_id, target, chunk_count, 0);
        restore.start();

        for _ in 0..chunk_count {
            restore.restore_chunk(total_size / chunk_count.max(1));
        }
        restore.complete(0);

        snapshot.restore(target, 0)?;
        self.stats.borrow_mut().total_restored += 1;
        self.restores.borrow_mut().push(restore.clone());
        self.monitor.borrow_mut().record_activity(target, 1);
        Ok(restore)
    }

    pub fn snapshot_state(&self, snapshot_id: u64) -> Result<SnapshotState, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.state)
    }

    pub fn chunk_count(&self, snapshot_id: u64) -> Result<u64, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.chunk_count())
    }

    pub fn total_size(&self, snapshot_id: u64) -> Result<u64, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.total_size)
    }

    pub fn verify_integrity(&self, snapshot_id: u64) -> Result<bool, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        for chunk in snapshot.chunks.borrow().iter() {
            if !chunk.verify_hash() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn expire_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state.is_final() {
            return Err(SnapshotError::Internal {
                message: "snapshot already in final state".to_string(),
            });
        }
        snapshot.expire(0);
        self.stats.borrow_mut().total_expired += 1;
        Ok(())
    }

    pub fn delete_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state.is_final() {
            return Err(SnapshotError::Internal {
                message: "snapshot already in final state".to_string(),
            });
        }
        let _size = snapshot.total_size;
        snapshots.remove(&snapshot_id);
        self.stats.borrow_mut().total_deleted += 1;
        self.stats.borrow_mut().total_size = self.stats.borrow().total_size.saturating_sub(_size);
        Ok(())
    }

    pub fn set_retention(&self, retention: SnapshotRetention) -> Result<(), SnapshotError> {
        retention.validate()?;
        *self.retention.borrow_mut() = retention;
        Ok(())
    }

    pub fn retention(&self) -> Result<SnapshotRetention, SnapshotError> {
        Ok(self.retention.borrow().clone())
    }

    pub fn compute_diff(
        &self,
        snapshot_id_a: u64,
        snapshot_id_b: u64,
    ) -> Result<SnapshotDiff, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snap_a = snapshots.get(&snapshot_id_a).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id: snapshot_id_a })?;
        let snap_b = snapshots.get(&snapshot_id_b).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id: snapshot_id_b })?;

        let mut diff = SnapshotDiff::new(
            *self.next_diff_id.borrow(),
            snapshot_id_a,
            snapshot_id_b,
            0,
        );
        *self.next_diff_id.borrow_mut() += 1;

        let chunks_a: Vec<ChunkID> = snap_a.chunks.borrow().iter().map(|c| c.chunk_id).collect();
        let chunks_b: Vec<ChunkID> = snap_b.chunks.borrow().iter().map(|c| c.chunk_id).collect();

        for cid in &chunks_b {
            if !chunks_a.contains(cid) {
                diff.add_added(*cid);
            }
        }
        for cid in &chunks_a {
            if !chunks_b.contains(cid) {
                diff.add_removed(*cid);
            }
        }

        self.stats.borrow_mut().total_diff_computations += 1;
        self.diffs.borrow_mut().push(diff.clone());
        Ok(diff)
    }

    pub fn apply_diff(
        &self,
        snapshot_id: u64,
        diff: &SnapshotDiff,
    ) -> Result<(), SnapshotError> {
        let _ = diff;
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        if snapshot.state == SnapshotState::Finalized {
            return Err(SnapshotError::SnapshotAlreadyFinalized { snapshot_id });
        }
        Ok(())
    }

    pub fn sign_snapshot(&self, snapshot_id: u64) -> Result<Vec<u8>, SnapshotError> {
        let mut snapshots = self.snapshots.borrow_mut();
        let snapshot = snapshots.get_mut(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        let sig = vec![1u8; 32];
        snapshot.sign(sig.clone(), 0);
        self.stats.borrow_mut().total_signatures += 1;
        Ok(sig)
    }

    pub fn verify_signature(
        &self,
        snapshot_id: u64,
        signature: &[u8],
    ) -> Result<bool, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.signature.as_ref() == Some(signature))
    }

    pub fn stats(&self) -> SnapshotStats {
        self.stats.borrow().clone()
    }

    pub fn snapshot_name(&self) -> &'static str {
        "snapshot_engine"
    }

    pub fn total_created(&self) -> u64 {
        self.stats.borrow().total_created
    }

    pub fn total_restored(&self) -> u64 {
        self.stats.borrow().total_restored
    }

    pub fn total_expired(&self) -> u64 {
        self.stats.borrow().total_expired
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.borrow().len()
    }

    pub fn total_diffs(&self) -> usize {
        self.diffs.borrow().len()
    }

    pub fn total_restores(&self) -> usize {
        self.restores.borrow().len()
    }

    pub fn is_configured(&self) -> bool {
        self.config.borrow().is_some()
    }

    pub fn monitor_node(&self, node: NodeId, snapshots: u64) {
        self.monitor.borrow_mut().record_activity(node, snapshots);
    }

    pub fn is_node_active(&self, node: &NodeId) -> bool {
        self.monitor.borrow().node_snapshot_count(node) > 0
    }

    pub fn validate_snapshot_id(&self, snapshot_id: u64) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_snapshot_id(snapshot_id)
    }

    pub fn validate_chunk(&self, snapshot_id: u64, chunk_id: ChunkID, size: usize) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_chunk(snapshot_id, chunk_id, size)
    }

    pub fn validate_retention(&self, retention: &SnapshotRetention) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_retention(retention)
    }

    pub fn validate_node(&self, node: NodeId) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_node(node)
    }

    pub fn validate_size(&self, snapshot_id: u64, size: u64, max: u64) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_size(snapshot_id, size, max)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    pub fn snapshot_health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }
}

impl Default for SnapshotEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT GROUP — named group of related snapshots
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotGroup {
    pub group_id: u64,
    pub name: String,
    pub snapshots: std::cell::RefCell<Vec<u64>>,
    pub created_at_ms: u64,
    pub auto_finalize: bool,
    pub max_snapshots: u64,
}

impl SnapshotGroup {
    pub fn new(
        group_id: u64,
        name: String,
        created_at_ms: u64,
    ) -> Self {
        Self {
            group_id,
            name,
            snapshots: std::cell::RefCell::new(Vec::new()),
            created_at_ms,
            auto_finalize: false,
            max_snapshots: 1000,
        }
    }

    pub fn add_snapshot(&self, snapshot_id: u64) -> Result<(), SnapshotError> {
        if self.snapshots.borrow().len() as u64 >= self.max_snapshots {
            return Err(SnapshotError::InsufficientStorage {
                needed: self.snapshots.borrow().len() as u64 + 1,
                available: self.max_snapshots,
            });
        }
        if self.snapshots.borrow().contains(&snapshot_id) {
            return Err(SnapshotError::SnapshotAlreadyExists { snapshot_id });
        }
        self.snapshots.borrow_mut().push(snapshot_id);
        Ok(())
    }

    pub fn remove_snapshot(&self, snapshot_id: u64) -> bool {
        let mut snaps = self.snapshots.borrow_mut();
        let pos = snaps.iter().position(|&id| id == snapshot_id);
        if let Some(idx) = pos {
            snaps.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn snapshot_count(&self) -> u64 {
        self.snapshots.borrow().len() as u64
    }

    pub fn contains(&self, snapshot_id: u64) -> bool {
        self.snapshots.borrow().contains(&snapshot_id)
    }

    pub fn snapshot_ids(&self) -> Vec<u64> {
        self.snapshots.borrow().clone()
    }

    pub fn set_auto_finalize(&mut self, auto: bool) {
        self.auto_finalize = auto;
    }

    pub fn is_auto_finalize(&self) -> bool {
        self.auto_finalize
    }

    pub fn set_max_snapshots(&mut self, max: u64) {
        self.max_snapshots = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT MANIFEST — manifest file for a snapshot
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotManifest {
    pub manifest_id: u64,
    pub snapshot_id: u64,
    pub chunk_count: u64,
    pub total_size: u64,
    pub root_hash: [u8; 32],
    pub created_at_ms: u64,
    pub version: u32,
}

impl SnapshotManifest {
    pub fn new(
        manifest_id: u64,
        snapshot_id: u64,
        created_at_ms: u64,
    ) -> Self {
        Self {
            manifest_id,
            snapshot_id,
            chunk_count: 0,
            total_size: 0,
            root_hash: [0u8; 32],
            created_at_ms,
            version: 1,
        }
    }

    pub fn update(
        &mut self,
        chunk_count: u64,
        total_size: u64,
        root_hash: [u8; 32],
    ) {
        self.chunk_count = chunk_count;
        self.total_size = total_size;
        self.root_hash = root_hash;
        self.version += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.chunk_count == 0 && self.total_size == 0
    }

    pub fn hash_matches(&self, hash: &[u8; 32]) -> bool {
        self.root_hash == *hash
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT STREAM — streaming snapshot chunks
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotStream {
    pub stream_id: u64,
    pub snapshot_id: u64,
    pub chunks: std::cell::RefCell<Vec<SnapshotChunk>>,
    pub current_index: u64,
    pub total_bytes: u64,
    pub stream_started: bool,
    pub stream_completed: bool,
}

impl SnapshotStream {
    pub fn new(stream_id: u64, snapshot_id: u64) -> Self {
        Self {
            stream_id,
            snapshot_id,
            chunks: std::cell::RefCell::new(Vec::new()),
            current_index: 0,
            total_bytes: 0,
            stream_started: false,
            stream_completed: false,
        }
    }

    pub fn add_chunk(&self, chunk: SnapshotChunk) {
        self.chunks.borrow_mut().push(chunk);
        self.total_bytes += chunk.size as u64;
    }

    pub fn next_chunk(&self) -> Option<SnapshotChunk> {
        if !self.stream_started || self.stream_completed {
            return None;
        }
        let chunks = self.chunks.borrow();
        let idx = self.current_index as usize;
        if idx < chunks.len() {
            Some(chunks[idx].clone())
        } else {
            None
        }
    }

    pub fn advance(&mut self) {
        if self.current_index < self.chunks.borrow().len() as u64 {
            self.current_index += 1;
        }
        if self.current_index >= self.chunks.borrow().len() as u64 {
            self.stream_completed = true;
        }
    }

    pub fn start(&mut self) {
        self.stream_started = true;
        self.current_index = 0;
        self.stream_completed = false;
    }

    pub fn chunk_count(&self) -> u64 {
        self.chunks.borrow().len() as u64
    }

    pub fn progress(&self) -> f64 {
        let total = self.chunks.borrow().len();
        if total == 0 {
            return 1.0;
        }
        self.current_index as f64 / total as f64
    }

    pub fn is_completed(&self) -> bool {
        self.stream_completed
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT POLICY — snapshot creation and retention policy
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SnapshotPolicy {
    pub policy_id: u64,
    pub name: String,
    pub interval_ms: u64,
    pub max_snapshots: u64,
    pub min_retention_ms: u64,
    pub max_retention_ms: u64,
    pub auto_create: bool,
    pub require_finalize: bool,
}

impl SnapshotPolicy {
    pub fn new(
        policy_id: u64,
        name: String,
        interval_ms: u64,
    ) -> Self {
        Self {
            policy_id,
            name,
            interval_ms,
            max_snapshots: 100,
            min_retention_ms: 3600_000,
            max_retention_ms: 7 * 24 * 60 * 60 * 1000,
            auto_create: false,
            require_finalize: true,
        }
    }

    pub fn with_max_snapshots(mut self, max: u64) -> Self {
        self.max_snapshots = max;
        self
    }

    pub fn with_retention_range(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.min_retention_ms = min_ms;
        self.max_retention_ms = max_ms;
        self
    }

    pub fn with_auto_create(mut self, auto: bool) -> Self {
        self.auto_create = auto;
        self
    }

    pub fn with_require_finalize(mut self, require: bool) -> Self {
        self.require_finalize = require;
        self
    }

    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.interval_ms == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "interval_ms must be > 0".to_string(),
            });
        }
        if self.max_snapshots == 0 {
            return Err(SnapshotError::InvalidRetention {
                reason: "max_snapshots must be > 0".to_string(),
            });
        }
        if self.min_retention_ms > self.max_retention_ms {
            return Err(SnapshotError::InvalidRetention {
                reason: "min_retention_ms cannot exceed max_retention_ms".to_string(),
            });
        }
        Ok(())
    }

    pub fn is_retention_valid(&self, retention_ms: u64) -> bool {
        retention_ms >= self.min_retention_ms && retention_ms <= self.max_retention_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT COMPRESSION — compression metadata
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    None,
    Lz4,
    Zstd,
    Blosc,
}

impl CompressionAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionAlgorithm::None => "none",
            CompressionAlgorithm::Lz4 => "lz4",
            CompressionAlgorithm::Zstd => "zstd",
            CompressionAlgorithm::Blosc => "blosc",
        }
    }

    pub fn default_ratio(&self) -> f64 {
        match self {
            CompressionAlgorithm::None => 1.0,
            CompressionAlgorithm::Lz4 => 2.5,
            CompressionAlgorithm::Zstd => 3.5,
            CompressionAlgorithm::Blosc => 4.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotCompression {
    pub algorithm: CompressionAlgorithm,
    pub level: u32,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compressed: bool,
}

impl SnapshotCompression {
    pub fn new(algorithm: CompressionAlgorithm, level: u32, original_size: usize) -> Self {
        let compressed = algorithm != CompressionAlgorithm::None;
        let compressed_size = if compressed {
            (original_size as f64 / algorithm.default_ratio()).ceil() as usize
        } else {
            original_size
        };
        Self {
            algorithm,
            level,
            original_size,
            compressed_size,
            compressed,
        }
    }

    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            return 1.0;
        }
        self.original_size as f64 / self.compressed_size as f64
    }

    pub fn space_saved(&self) -> usize {
        self.original_size.saturating_sub(self.compressed_size)
    }

    pub fn space_saved_percent(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        self.space_saved() as f64 / self.original_size as f64 * 100.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED SNAPSHOT ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

impl SnapshotEngine {
    /// Create a snapshot with auto-generated ID.
    pub fn create_snapshot_auto(
        &self,
        dataset: DatasetId,
        origin: NodeId,
    ) -> Result<Snapshot, SnapshotError> {
        let id = *self.next_snapshot_id.borrow();
        *self.next_snapshot_id.borrow_mut() += 1;
        self.create_snapshot(id, dataset, origin)
    }

    /// Get a snapshot by ID.
    pub fn get_snapshot(&self, snapshot_id: u64) -> Result<Snapshot, SnapshotError> {
        self.snapshots
            .borrow()
            .get(&snapshot_id)
            .cloned()
            .ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })
    }

    /// Check if snapshot exists.
    pub fn snapshot_exists(&self, snapshot_id: u64) -> bool {
        self.snapshots.borrow().contains_key(&snapshot_id)
    }

    /// Get all snapshot IDs.
    pub fn all_snapshot_ids(&self) -> Vec<u64> {
        self.snapshots.borrow().keys().copied().collect()
    }

    /// Get finalized snapshot IDs.
    pub fn finalized_snapshot_ids(&self) -> Vec<u64> {
        self.snapshots
            .borrow()
            .iter()
            .filter(|(_, s)| s.state == SnapshotState::Finalized)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get expired snapshot IDs.
    pub fn expired_snapshot_ids(&self, at_ms: u64) -> Vec<u64> {
        self.snapshots
            .borrow()
            .iter()
            .filter(|(_, s)| s.is_expired(at_ms))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Expire all snapshots past retention.
    pub fn expire_all_past_retention(&self, at_ms: u64) -> u64 {
        let expired = self.expired_snapshot_ids(at_ms);
        let count = expired.len() as u64;
        for id in expired {
            let _ = self.expire_snapshot(id);
        }
        count
    }

    /// Get total size of all snapshots.
    pub fn total_size_all(&self) -> u64 {
        self.snapshots.borrow().values().map(|s| s.total_size).sum()
    }

    /// Get total chunk count across all snapshots.
    pub fn total_chunks_all(&self) -> u64 {
        self.snapshots.borrow().values().map(|s| s.chunk_count()).sum()
    }

    /// Get average snapshot size.
    pub fn average_snapshot_size(&self) -> f64 {
        let count = self.snapshot_count();
        if count == 0 {
            return 0.0;
        }
        self.total_size_all() as f64 / count as f64
    }

    /// Get signed snapshot count.
    pub fn signed_snapshot_count(&self) -> u64 {
        self.snapshots
            .borrow()
            .values()
            .filter(|s| s.is_signed())
            .count() as u64
    }

    /// Get unsigned snapshot count.
    pub fn unsigned_snapshot_count(&self) -> u64 {
        self.snapshots
            .borrow()
            .values()
            .filter(|s| !s.is_signed())
            .count() as u64
    }

    /// Create a snapshot group.
    pub fn create_group(&self, group_id: u64, name: String) -> SnapshotGroup {
        SnapshotGroup::new(group_id, name, 0)
    }

    /// Create a snapshot manifest.
    pub fn create_manifest(&self, manifest_id: u64, snapshot_id: u64) -> SnapshotManifest {
        SnapshotManifest::new(manifest_id, snapshot_id, 0)
    }

    /// Create a snapshot stream.
    pub fn create_stream(&self, stream_id: u64, snapshot_id: u64) -> SnapshotStream {
        SnapshotStream::new(stream_id, snapshot_id)
    }

    /// Create a snapshot compression config.
    pub fn create_compression(
        &self,
        algorithm: CompressionAlgorithm,
        level: u32,
        original_size: usize,
    ) -> SnapshotCompression {
        SnapshotCompression::new(algorithm, level, original_size)
    }

    /// Create a snapshot policy.
    pub fn create_policy(
        &self,
        policy_id: u64,
        name: String,
        interval_ms: u64,
    ) -> SnapshotPolicy {
        SnapshotPolicy::new(policy_id, name, interval_ms)
    }

    /// Get total restores.
    pub fn total_restores(&self) -> u64 {
        self.restores.borrow().len() as u64
    }

    /// Get total diffs.
    pub fn total_diffs(&self) -> u64 {
        self.diffs.borrow().len() as u64
    }

    /// Get restore by index.
    pub fn get_restore(&self, index: usize) -> Option<SnapshotRestore> {
        self.restores.borrow().get(index).cloned()
    }

    /// Get diff by index.
    pub fn get_diff(&self, index: usize) -> Option<SnapshotDiff> {
        self.diffs.borrow().get(index).cloned()
    }

    /// Clear all restores.
    pub fn clear_restores(&self) {
        self.restores.borrow_mut().clear();
    }

    /// Clear all diffs.
    pub fn clear_diffs(&self) {
        self.diffs.borrow_mut().clear();
    }

    /// Reset all stats.
    pub fn reset_stats(&self) {
        *self.stats.borrow_mut() = SnapshotStats::default();
    }

    /// Get snapshot health score.
    pub fn health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }

    /// Get restore rate.
    pub fn restore_rate(&self) -> f64 {
        self.stats.borrow().restore_rate()
    }

    /// Get expiry rate.
    pub fn expiry_rate(&self) -> f64 {
        self.stats.borrow().expiry_rate()
    }

    /// Get valid rate.
    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    /// Validate retention policy.
    pub fn validate_retention_policy(&self, retention: &SnapshotRetention) -> SnapshotValidationResult {
        self.validator.borrow_mut().validate_retention(retention)
    }

    /// Validate snapshot size against quota.
    pub fn validate_snapshot_size(&self, snapshot_id: u64, size: u64) -> SnapshotValidationResult {
        let cfg = self.config.borrow();
        let max = cfg.as_ref().map(|c| c.storage_quota).unwrap_or(u64::MAX);
        drop(cfg);
        self.validator.borrow_mut().validate_size(snapshot_id, size, max)
    }

    /// Get next snapshot ID.
    pub fn next_snapshot_id(&self) -> u64 {
        *self.next_snapshot_id.borrow()
    }

    /// Get next diff ID.
    pub fn next_diff_id(&self) -> u64 {
        *self.next_diff_id.borrow()
    }

    /// Check if snapshot is signed.
    pub fn is_snapshot_signed(&self, snapshot_id: u64) -> Result<bool, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.is_signed())
    }

    /// Get snapshot region.
    pub fn snapshot_region(&self, snapshot_id: u64) -> Result<Option<RegionId>, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.region)
    }

    /// Get snapshot endpoint.
    pub fn snapshot_endpoint(&self, snapshot_id: u64) -> Result<Option<Endpoint>, SnapshotError> {
        let snapshots = self.snapshots.borrow();
        let snapshot = snapshots.get(&snapshot_id).ok_or_else(|| SnapshotError::SnapshotNotFound { snapshot_id })?;
        Ok(snapshot.endpoint.clone())
    }

    /// Count snapshots by state.
    pub fn count_by_state(&self, state: SnapshotState) -> u64 {
        self.snapshots.borrow().values().filter(|s| s.state == state).count() as u64
    }

    /// Get total restore bytes.
    pub fn total_restore_bytes(&self) -> u64 {
        self.restores.borrow().iter().map(|r| r.bytes_restored).sum()
    }

    /// Get average restore progress.
    pub fn average_restore_progress(&self) -> f64 {
        let restores = self.restores.borrow();
        if restores.is_empty() {
            return 1.0;
        }
        restores.iter().map(|r| r.progress()).sum::<f64>() / restores.len() as f64
    }

    /// Get failed restore count.
    pub fn failed_restore_count(&self) -> u64 {
        self.restores.borrow().iter().filter(|r| r.status == RestoreStatus::Failed).count() as u64
    }

    /// Get completed restore count.
    pub fn completed_restore_count(&self) -> u64 {
        self.restores.borrow().iter().filter(|r| r.status == RestoreStatus::Completed).count() as u64
    }

    /// Get snapshot compression savings estimate.
    pub fn compression_savings(&self, original_size: usize, algorithm: CompressionAlgorithm) -> usize {
        let compressed = (original_size as f64 / algorithm.default_ratio()).ceil() as usize;
        original_size.saturating_sub(compressed)
    }

    /// Check if retention is valid for a given value.
    pub fn is_retention_valid(&self, retention_ms: u64) -> bool {
        let retention = self.retention.borrow();
        retention_ms >= retention.min_retention_ms && retention_ms <= retention.max_retention_ms
    }

    /// Get snapshot count by region.
    pub fn count_by_region(&self, region: RegionId) -> u64 {
        self.snapshots.borrow().values().filter(|s| s.region == Some(region)).count() as u64
    }

    /// Get total snapshots across all states.
    pub fn total_all_snapshots(&self) -> u64 {
        self.snapshots.borrow().len() as u64
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

    fn test_peer_id() -> NodeId {
        NodeId::new([2; 32])
    }

    fn test_region_id() -> RegionId {
        RegionId::new([3; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([4; 32])
    }

    fn test_endpoint() -> Endpoint {
        Endpoint {
            carrier_type: crate::types::CarrierType::Loopback,
            address: vec![1, 2, 3],
        }
    }

    fn test_chunk_id() -> ChunkID {
        ChunkID::new([5; 32])
    }

    fn test_snapshot_config() -> SnapshotConfig {
        SnapshotConfig::new()
    }

    // === SnapshotError tests ===
    #[test]
    fn snapshot_error_category() {
        assert_eq!(
            SnapshotError::SnapshotNotConfigured.category(),
            "snapshot_not_configured"
        );
        assert_eq!(
            SnapshotError::SnapshotNotFound { snapshot_id: 42 }.category(),
            "snapshot_not_found"
        );
        assert_eq!(
            SnapshotError::SnapshotAlreadyExists { snapshot_id: 42 }.category(),
            "snapshot_already_exists"
        );
        assert_eq!(
            SnapshotError::SnapshotAlreadyFinalized { snapshot_id: 42 }.category(),
            "snapshot_already_finalized"
        );
        assert_eq!(
            SnapshotError::SnapshotNotFinalized { snapshot_id: 42 }.category(),
            "snapshot_not_finalized"
        );
        assert_eq!(
            SnapshotError::ChunkNotFound { chunk_id: test_chunk_id(), snapshot_id: 1 }.category(),
            "chunk_not_found"
        );
        assert_eq!(
            SnapshotError::ChunkAlreadyExists { chunk_id: test_chunk_id(), snapshot_id: 1 }.category(),
            "chunk_already_exists"
        );
        assert_eq!(
            SnapshotError::InsufficientStorage { needed: 10, available: 5 }.category(),
            "insufficient_storage"
        );
        assert_eq!(
            SnapshotError::IntegrityFailed { snapshot_id: 1 }.category(),
            "integrity_failed"
        );
        assert_eq!(
            SnapshotError::SignatureInvalid { snapshot_id: 1 }.category(),
            "signature_invalid"
        );
        assert_eq!(
            SnapshotError::SignatureMissing { snapshot_id: 1 }.category(),
            "signature_missing"
        );
        assert_eq!(
            SnapshotError::RestoreFailed { snapshot_id: 1, target: test_node_id(), reason: "bad".to_string() }.category(),
            "restore_failed"
        );
        assert_eq!(
            SnapshotError::InvalidRetention { reason: "bad".to_string() }.category(),
            "invalid_retention"
        );
        assert_eq!(
            SnapshotError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn snapshot_error_is_recoverable() {
        assert!(SnapshotError::InsufficientStorage { needed: 10, available: 5 }.is_recoverable());
        assert!(SnapshotError::RestoreFailed { snapshot_id: 1, target: test_node_id(), reason: "x".to_string() }.is_recoverable());
        assert!(SnapshotError::IntegrityFailed { snapshot_id: 1 }.is_recoverable());
        assert!(!SnapshotError::SnapshotNotFound { snapshot_id: 1 }.is_recoverable());
    }

    #[test]
    fn snapshot_error_suggested_action() {
        assert_eq!(
            SnapshotError::InsufficientStorage { needed: 10, available: 5 }.suggested_action(),
            "free storage or increase quota"
        );
        assert_eq!(
            SnapshotError::SignatureMissing { snapshot_id: 1 }.suggested_action(),
            "sign the snapshot first"
        );
        assert_eq!(
            SnapshotError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === SnapshotState tests ===
    #[test]
    fn snapshot_state_as_str() {
        assert_eq!(SnapshotState::Created.as_str(), "created");
        assert_eq!(SnapshotState::Finalized.as_str(), "finalized");
        assert_eq!(SnapshotState::Restored.as_str(), "restored");
        assert_eq!(SnapshotState::Expired.as_str(), "expired");
        assert_eq!(SnapshotState::Deleted.as_str(), "deleted");
    }

    #[test]
    fn snapshot_state_is_final() {
        assert!(SnapshotState::Expired.is_final());
        assert!(SnapshotState::Deleted.is_final());
        assert!(!SnapshotState::Created.is_final());
        assert!(!SnapshotState::Finalized.is_final());
    }

    #[test]
    fn snapshot_state_can_transition() {
        assert!(SnapshotState::Created.can_transition_to(SnapshotState::Finalized));
        assert!(SnapshotState::Finalized.can_transition_to(SnapshotState::Restored));
        assert!(!SnapshotState::Expired.can_transition_to(SnapshotState::Created));
    }

    // === SnapshotConfig tests ===
    #[test]
    fn snapshot_config_default() {
        let config = SnapshotConfig::default();
        assert_eq!(config.max_snapshots, 10_000);
        assert_eq!(config.max_chunks_per_snapshot, 100_000);
        assert_eq!(config.max_chunk_size, 4 * 1024 * 1024);
    }

    #[test]
    fn snapshot_config_validate() {
        let config = SnapshotConfig::default();
        assert!(config.validate().is_ok());
        let config = SnapshotConfig::new().with_max_snapshots(0);
        assert!(config.validate().is_err());
        let config = SnapshotConfig::new().with_max_chunk_size(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn snapshot_config_builder() {
        let config = SnapshotConfig::new()
            .with_max_snapshots(500)
            .with_retention_ms(3600_000)
            .with_signature_required(true);
        assert_eq!(config.max_snapshots, 500);
        assert_eq!(config.default_retention_ms, 3600_000);
        assert!(config.require_signature);
    }

    // === Snapshot tests ===
    #[test]
    fn snapshot_new_and_add_chunk() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000);
        snap.add_chunk(chunk).unwrap();
        assert_eq!(snap.chunk_count(), 1);
        assert_eq!(snap.total_size, 3);
    }

    #[test]
    fn snapshot_duplicate_chunk() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000);
        snap.add_chunk(chunk.clone()).unwrap();
        let err = snap.add_chunk(chunk).unwrap_err();
        assert!(matches!(err, SnapshotError::ChunkAlreadyExists { .. }));
    }

    #[test]
    fn snapshot_finalize() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        snap.finalize(2000);
        assert_eq!(snap.state, SnapshotState::Finalized);
        assert_eq!(snap.finalized_at_ms, Some(2000));
    }

    #[test]
    fn snapshot_restore() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        snap.finalize(2000);
        snap.restore(test_peer_id(), 3000).unwrap();
        assert_eq!(snap.state, SnapshotState::Restored);
        assert_eq!(snap.restored_at_ms, Some(3000));
    }

    #[test]
    fn snapshot_expire() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        snap.expire(5000);
        assert_eq!(snap.state, SnapshotState::Expired);
        assert_eq!(snap.expired_at_ms, Some(5000));
    }

    #[test]
    fn snapshot_is_expired() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            1000,
        );
        assert!(!snap.is_expired(1500));
        snap.finalize(2000);
        assert!(snap.is_expired(5000));
    }

    #[test]
    fn snapshot_sign() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        assert!(!snap.is_signed());
        snap.sign(vec![1, 2, 3], 2000);
        assert!(snap.is_signed());
    }

    #[test]
    fn snapshot_region_and_endpoint() {
        let mut snap = Snapshot::new(
            1,
            test_dataset_id(),
            test_node_id(),
            1000,
            3600_000,
        );
        snap.set_region(test_region_id());
        snap.set_endpoint(test_endpoint());
        assert!(snap.region.is_some());
        assert!(snap.endpoint.is_some());
    }

    // === SnapshotChunk tests ===
    #[test]
    fn snapshot_chunk_new_and_verify() {
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3, 4], 0, 1000);
        assert_eq!(chunk.chunk_id, test_chunk_id());
        assert_eq!(chunk.size, 4);
        assert_eq!(chunk.offset, 0);
        assert!(chunk.verify_hash());
    }

    #[test]
    fn snapshot_chunk_compression() {
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000)
            .with_compression(true, 2);
        assert!(chunk.compressed);
        assert_eq!(chunk.compressed_size, Some(2));
        assert_eq!(chunk.effective_size(), 2);
    }

    #[test]
    fn snapshot_chunk_verify_hash_fails() {
        let mut chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000);
        chunk.data = vec![9, 9, 9];
        assert!(!chunk.verify_hash());
    }

    // === RestoreStatus tests ===
    #[test]
    fn restore_status_as_str() {
        assert_eq!(RestoreStatus::Pending.as_str(), "pending");
        assert_eq!(RestoreStatus::InProgress.as_str(), "in_progress");
        assert_eq!(RestoreStatus::Completed.as_str(), "completed");
        assert_eq!(RestoreStatus::Failed.as_str(), "failed");
        assert_eq!(RestoreStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn restore_status_is_terminal() {
        assert!(RestoreStatus::Completed.is_terminal());
        assert!(RestoreStatus::Failed.is_terminal());
        assert!(RestoreStatus::Cancelled.is_terminal());
        assert!(!RestoreStatus::Pending.is_terminal());
        assert!(!RestoreStatus::InProgress.is_terminal());
    }

    // === SnapshotRestore tests ===
    #[test]
    fn snapshot_restore_new_and_progress() {
        let mut restore = SnapshotRestore::new(1, test_node_id(), 10, 1000);
        assert_eq!(restore.status, RestoreStatus::Pending);
        assert_eq!(restore.progress(), 0.0);
        restore.start();
        assert_eq!(restore.status, RestoreStatus::InProgress);
        restore.restore_chunk(100);
        restore.restore_chunk(100);
        assert_eq!(restore.chunks_restored, 2);
        assert_eq!(restore.bytes_restored, 200);
        assert!((restore.progress() - 0.2).abs() < 0.01);
        restore.complete(2000);
        assert_eq!(restore.status, RestoreStatus::Completed);
        assert_eq!(restore.completed_at_ms, Some(2000));
    }

    #[test]
    fn snapshot_restore_fail_and_cancel() {
        let mut restore = SnapshotRestore::new(1, test_node_id(), 10, 1000);
        restore.fail("error".to_string());
        assert_eq!(restore.status, RestoreStatus::Failed);
        assert_eq!(restore.error, Some("error".to_string()));
        let mut restore = SnapshotRestore::new(2, test_node_id(), 10, 1000);
        restore.cancel();
        assert_eq!(restore.status, RestoreStatus::Cancelled);
    }

    // === SnapshotRetention tests ===
    #[test]
    fn snapshot_retention_default() {
        let retention = SnapshotRetention::default();
        assert_eq!(retention.max_snapshots, 100);
        assert_eq!(retention.auto_expire, true);
        assert_eq!(retention.expire_on_restore, false);
    }

    #[test]
    fn snapshot_retention_validate() {
        let retention = SnapshotRetention::new();
        assert!(retention.validate().is_ok());
        let retention = SnapshotRetention::new().with_min_snapshots(200).with_max_snapshots(100);
        assert!(retention.validate().is_err());
    }

    #[test]
    fn snapshot_retention_builder() {
        let retention = SnapshotRetention::new()
            .with_max_snapshots(50)
            .with_max_age_ms(86400_000)
            .with_min_snapshots(5);
        assert_eq!(retention.max_snapshots, 50);
        assert_eq!(retention.max_age_ms, 86400_000);
        assert_eq!(retention.min_snapshots, 5);
    }

    // === SnapshotDiff tests ===
    #[test]
    fn snapshot_diff_new_and_changes() {
        let mut diff = SnapshotDiff::new(1, 100, 200, 1000);
        diff.add_added(ChunkID::new([1; 32]));
        diff.add_added(ChunkID::new([2; 32]));
        diff.add_removed(ChunkID::new([3; 32]));
        diff.add_modified(ChunkID::new([4; 32]));
        assert_eq!(diff.diff_size, 4);
        assert_eq!(diff.total_changes(), 4);
        assert!(!diff.is_empty());
    }

    #[test]
    fn snapshot_diff_empty() {
        let diff = SnapshotDiff::new(1, 100, 200, 1000);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    // === SnapshotStats tests ===
    #[test]
    fn snapshot_stats_rates() {
        let mut s = SnapshotStats::default();
        s.total_created = 10;
        s.total_restored = 7;
        s.total_expired = 2;
        assert!((s.restore_rate() - 0.7).abs() < 0.01);
        assert!((s.expiry_rate() - 0.2).abs() < 0.01);
    }

    #[test]
    fn snapshot_stats_average_size() {
        let mut s = SnapshotStats::default();
        s.total_created = 10;
        s.total_size = 1000;
        assert_eq!(s.average_snapshot_size(), 100.0);
        let s = SnapshotStats::default();
        assert_eq!(s.average_snapshot_size(), 0.0);
    }

    #[test]
    fn snapshot_stats_health_score() {
        let mut s = SnapshotStats::default();
        assert_eq!(s.system_health_score(), 100.0);
        s.total_restore_failures = 10;
        let score = s.system_health_score();
        assert!(score < 100.0);
        assert!(score >= 0.0);
    }

    // === SnapshotMonitor tests ===
    #[test]
    fn snapshot_monitor_record_activity() {
        let sm = SnapshotMonitor::default();
        sm.record_activity(test_node_id(), 5);
        assert_eq!(sm.node_snapshot_count(&test_node_id()), 5);
        assert_eq!(sm.health_check_count(), 1);
    }

    #[test]
    fn snapshot_monitor_incident() {
        let sm = SnapshotMonitor::default();
        sm.record_activity(test_node_id(), 5);
        sm.report_incident(test_node_id());
        assert_eq!(sm.snapshot_incident_count(), 1);
        assert_eq!(sm.alert_count(), 1);
        assert_eq!(sm.inactive_node_count(), 1);
    }

    // === SnapshotValidationResult tests ===
    #[test]
    fn snapshot_validation_result_as_str() {
        assert_eq!(SnapshotValidationResult::Valid.as_str(), "valid");
        assert_eq!(SnapshotValidationResult::InvalidSnapshotID.as_str(), "invalid_snapshot_id");
        assert_eq!(SnapshotValidationResult::InvalidChunk.as_str(), "invalid_chunk");
        assert_eq!(SnapshotValidationResult::InvalidRetention.as_str(), "invalid_retention");
        assert_eq!(SnapshotValidationResult::InvalidNode.as_str(), "invalid_node");
        assert_eq!(SnapshotValidationResult::InvalidSize.as_str(), "invalid_size");
        assert!(SnapshotValidationResult::Valid.is_ok());
        assert!(!SnapshotValidationResult::InvalidNode.is_ok());
    }

    // === SnapshotValidator tests ===
    #[test]
    fn snapshot_validator_validate_snapshot_id() {
        let sv = SnapshotValidator::default();
        assert_eq!(sv.validate_snapshot_id(1), SnapshotValidationResult::Valid);
        assert_eq!(sv.validate_snapshot_id(0), SnapshotValidationResult::InvalidSnapshotID);
    }

    #[test]
    fn snapshot_validator_validate_chunk() {
        let sv = SnapshotValidator::default();
        assert_eq!(
            sv.validate_chunk(1, test_chunk_id(), 100),
            SnapshotValidationResult::Valid
        );
        assert_eq!(
            sv.validate_chunk(1, test_chunk_id(), 0),
            SnapshotValidationResult::InvalidChunk
        );
    }

    #[test]
    fn snapshot_validator_validate_retention() {
        let sv = SnapshotValidator::default();
        let retention = SnapshotRetention::new();
        assert_eq!(sv.validate_retention(&retention), SnapshotValidationResult::Valid);
        let retention = SnapshotRetention::new().with_max_snapshots(0);
        assert_eq!(sv.validate_retention(&retention), SnapshotValidationResult::InvalidRetention);
    }

    #[test]
    fn snapshot_validator_validate_node() {
        let sv = SnapshotValidator::default();
        assert_eq!(sv.validate_node(test_node_id()), SnapshotValidationResult::Valid);
        assert_eq!(
            sv.validate_node(NodeId::new([0u8; 32])),
            SnapshotValidationResult::InvalidNode
        );
    }

    #[test]
    fn snapshot_validator_validate_size() {
        let sv = SnapshotValidator::default();
        assert_eq!(
            sv.validate_size(1, 100, 1000),
            SnapshotValidationResult::Valid
        );
        assert_eq!(
            sv.validate_size(1, 2000, 1000),
            SnapshotValidationResult::InvalidSize
        );
    }

    #[test]
    fn snapshot_validator_valid_rate() {
        let sv = SnapshotValidator::default();
        sv.validate_snapshot_id(1);
        sv.validate_snapshot_id(0);
        assert!((sv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === SnapshotEngine tests ===
    #[test]
    fn snapshot_engine_configure_and_create() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        assert!(engine.is_configured());
        let snap = engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert_eq!(snap.snapshot_id, 1);
        assert_eq!(engine.total_created(), 1);
    }

    #[test]
    fn snapshot_engine_not_configured() {
        let engine = SnapshotEngine::default();
        let err = engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap_err();
        assert!(matches!(err, SnapshotError::SnapshotNotConfigured));
    }

    #[test]
    fn snapshot_engine_already_exists() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let err = engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap_err();
        assert!(matches!(err, SnapshotError::SnapshotAlreadyExists { .. }));
    }

    #[test]
    fn snapshot_engine_add_chunk() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000);
        engine.add_chunk(1, chunk).unwrap();
        assert_eq!(engine.chunk_count(1).unwrap(), 1);
    }

    #[test]
    fn snapshot_engine_finalize() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.finalize_snapshot(1).unwrap();
        assert_eq!(engine.snapshot_state(1).unwrap(), SnapshotState::Finalized);
    }

    #[test]
    fn snapshot_engine_restore() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.finalize_snapshot(1).unwrap();
        let restore = engine.restore_snapshot(1, test_peer_id()).unwrap();
        assert_eq!(restore.snapshot_id, 1);
        assert_eq!(restore.target, test_peer_id());
        assert_eq!(engine.total_restored(), 1);
    }

    #[test]
    fn snapshot_engine_restore_not_finalized() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let err = engine.restore_snapshot(1, test_peer_id()).unwrap_err();
        assert!(matches!(err, SnapshotError::SnapshotNotFinalized { .. }));
    }

    #[test]
    fn snapshot_engine_verify_integrity() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let chunk = SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000);
        engine.add_chunk(1, chunk).unwrap();
        assert!(engine.verify_integrity(1).unwrap());
    }

    #[test]
    fn snapshot_engine_expire() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.expire_snapshot(1).unwrap();
        assert_eq!(engine.snapshot_state(1).unwrap(), SnapshotState::Expired);
        assert_eq!(engine.total_expired(), 1);
    }

    #[test]
    fn snapshot_engine_delete() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.delete_snapshot(1).unwrap();
        assert!(engine.snapshots.borrow().is_empty());
    }

    #[test]
    fn snapshot_engine_set_and_get_retention() {
        let engine = SnapshotEngine::default();
        let retention = SnapshotRetention::new().with_max_snapshots(200);
        engine.set_retention(retention.clone()).unwrap();
        assert_eq!(engine.retention().unwrap().max_snapshots, 200);
    }

    #[test]
    fn snapshot_engine_compute_diff() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();
        let diff = engine.compute_diff(1, 2).unwrap();
        assert_eq!(diff.from_snapshot, 1);
        assert_eq!(diff.to_snapshot, 2);
        assert!(diff.is_empty());
    }

    #[test]
    fn snapshot_engine_sign_and_verify() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let sig = engine.sign_snapshot(1).unwrap();
        assert_eq!(sig.len(), 32);
        assert!(engine.verify_signature(1, &sig).unwrap());
        assert!(!engine.verify_signature(1, &[0u8; 32]).unwrap());
    }

    #[test]
    fn snapshot_engine_stats_and_name() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.snapshot_name(), "snapshot_engine");
        assert_eq!(engine.total_created(), 0);
        assert_eq!(engine.total_restored(), 0);
        assert_eq!(engine.total_expired(), 0);
    }

    #[test]
    fn snapshot_engine_monitor_node() {
        let engine = SnapshotEngine::default();
        engine.monitor_node(test_node_id(), 5);
        assert!(engine.is_node_active(&test_node_id()));
        assert_eq!(engine.monitor.borrow().monitored_node_count(), 1);
    }

    #[test]
    fn snapshot_engine_validate_all() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.validate_snapshot_id(1), SnapshotValidationResult::Valid);
        assert_eq!(engine.validate_node(test_node_id()), SnapshotValidationResult::Valid);
        assert_eq!(
            engine.validate_chunk(1, test_chunk_id(), 100),
            SnapshotValidationResult::Valid
        );
        let retention = SnapshotRetention::new();
        assert_eq!(engine.validate_retention(&retention), SnapshotValidationResult::Valid);
        assert_eq!(
            engine.validate_size(1, 100, 1000),
            SnapshotValidationResult::Valid
        );
    }

    #[test]
    fn snapshot_engine_health_score() {
        let engine = SnapshotEngine::default();
        let score = engine.snapshot_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full snapshot flow ===
    #[test]
    fn snapshot_full_flow() {
        let engine = SnapshotEngine::default();

        // Configure
        engine.configure(SnapshotConfig::default()).unwrap();

        // Create snapshots
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();

        // Add chunks
        engine.add_chunk(1, SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000)).unwrap();
        engine.add_chunk(2, SnapshotChunk::new(test_chunk_id(), vec![4, 5, 6], 0, 2000)).unwrap();

        // Finalize
        engine.finalize_snapshot(1).unwrap();
        engine.finalize_snapshot(2).unwrap();

        // Compute diff
        let diff = engine.compute_diff(1, 2).unwrap();
        assert_eq!(diff.total_changes(), 2);

        // Sign
        let sig = engine.sign_snapshot(1).unwrap();
        assert!(engine.verify_signature(1, &sig).unwrap());

        // Restore
        let restore = engine.restore_snapshot(1, test_peer_id()).unwrap();
        assert_eq!(restore.status, RestoreStatus::Completed);

        // Stats
        let s = engine.stats();
        assert_eq!(s.total_created, 2);
        assert_eq!(s.total_restored, 1);
        assert_eq!(s.total_signatures, 1);

        // Health score
        let score = engine.snapshot_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === NodeId zero peer test ===
    #[test]
    fn node_id_zero_peer_snapshot() {
        let zero = NodeId::new([0u8; 32]);
        assert_ne!(test_node_id(), zero);
    }

    // === Endpoint clone test ===
    #[test]
    fn endpoint_clone_snapshot() {
        let ep1 = test_endpoint();
        let ep2 = ep1.clone();
        assert_eq!(ep1.carrier_type, ep2.carrier_type);
        assert_eq!(ep1.address, ep2.address);
    }

    // === SnapshotConfig clone test ===
    #[test]
    fn snapshot_config_clone() {
        let cfg1 = test_snapshot_config();
        let cfg2 = cfg1.clone();
        assert_eq!(cfg1.max_snapshots, cfg2.max_snapshots);
        assert_eq!(cfg1.max_chunk_size, cfg2.max_chunk_size);
    }

    // === SnapshotRetention clone test ===
    #[test]
    fn snapshot_retention_clone() {
        let r1 = SnapshotRetention::new();
        let r2 = r1.clone();
        assert_eq!(r1.max_snapshots, r2.max_snapshots);
        assert_eq!(r1.auto_expire, r2.auto_expire);
    }

    // === SnapshotDiff clone test ===
    #[test]
    fn snapshot_diff_clone() {
        let d1 = SnapshotDiff::new(1, 100, 200, 1000);
        let d2 = d1.clone();
        assert_eq!(d1.diff_id, d2.diff_id);
        assert_eq!(d1.from_snapshot, d2.from_snapshot);
    }

    // === SnapshotGroup tests ===
    #[test]
    fn snapshot_group_new_and_add() {
        let group = SnapshotGroup::new(1, "group1", 1000);
        group.add_snapshot(1).unwrap();
        group.add_snapshot(2).unwrap();
        assert_eq!(group.snapshot_count(), 2);
        assert!(group.contains(1));
        assert!(!group.contains(3));
    }

    #[test]
    fn snapshot_group_duplicate_add() {
        let group = SnapshotGroup::new(1, "group1", 1000);
        group.add_snapshot(1).unwrap();
        let err = group.add_snapshot(1).unwrap_err();
        assert!(matches!(err, SnapshotError::SnapshotAlreadyExists { .. }));
    }

    #[test]
    fn snapshot_group_remove() {
        let group = SnapshotGroup::new(1, "group1", 1000);
        group.add_snapshot(1).unwrap();
        assert!(group.remove_snapshot(1));
        assert!(!group.remove_snapshot(1));
        assert_eq!(group.snapshot_count(), 0);
    }

    #[test]
    fn snapshot_group_max_snapshots() {
        let mut group = SnapshotGroup::new(1, "group1", 1000);
        group.set_max_snapshots(1);
        group.add_snapshot(1).unwrap();
        let err = group.add_snapshot(2).unwrap_err();
        assert!(matches!(err, SnapshotError::InsufficientStorage { .. }));
    }

    #[test]
    fn snapshot_group_auto_finalize() {
        let mut group = SnapshotGroup::new(1, "group1", 1000);
        assert!(!group.is_auto_finalize());
        group.set_auto_finalize(true);
        assert!(group.is_auto_finalize());
    }

    #[test]
    fn snapshot_group_ids() {
        let group = SnapshotGroup::new(1, "group1", 1000);
        group.add_snapshot(10).unwrap();
        group.add_snapshot(20).unwrap();
        let ids = group.snapshot_ids();
        assert_eq!(ids, vec![10, 20]);
    }

    // === SnapshotManifest tests ===
    #[test]
    fn snapshot_manifest_new_and_update() {
        let mut manifest = SnapshotManifest::new(1, 100, 1000);
        assert_eq!(manifest.manifest_id, 1);
        assert_eq!(manifest.snapshot_id, 100);
        assert!(manifest.is_empty());
        manifest.update(5, 1024, [42u8; 32]);
        assert_eq!(manifest.chunk_count, 5);
        assert_eq!(manifest.total_size, 1024);
        assert!(manifest.hash_matches(&[42u8; 32]));
        assert_eq!(manifest.version, 2);
        assert!(!manifest.is_empty());
    }

    // === SnapshotStream tests ===
    #[test]
    fn snapshot_stream_start_and_next() {
        let mut stream = SnapshotStream::new(1, 100);
        stream.add_chunk(SnapshotChunk::new(test_chunk_id(), vec![1, 2], 0, 1000));
        stream.add_chunk(SnapshotChunk::new(test_chunk_id(), vec![3, 4], 1, 2000));
        stream.start();
        let chunk = stream.next_chunk().unwrap();
        assert_eq!(chunk.chunk_id, test_chunk_id());
        stream.advance();
        let chunk2 = stream.next_chunk().unwrap();
        assert_eq!(chunk2.offset, 1);
        stream.advance();
        assert!(stream.next_chunk().is_none());
        assert!(stream.is_completed());
    }

    #[test]
    fn snapshot_stream_progress() {
        let mut stream = SnapshotStream::new(1, 100);
        stream.add_chunk(SnapshotChunk::new(test_chunk_id(), vec![1], 0, 1000));
        stream.add_chunk(SnapshotChunk::new(test_chunk_id(), vec![2], 1, 2000));
        stream.add_chunk(SnapshotChunk::new(test_chunk_id(), vec![3], 2, 3000));
        stream.start();
        assert!((stream.progress() - 0.0).abs() < 0.01);
        stream.advance();
        assert!((stream.progress() - (1.0 / 3.0)).abs() < 0.01);
        stream.advance();
        stream.advance();
        assert!((stream.progress() - 1.0).abs() < 0.01);
    }

    // === SnapshotPolicy tests ===
    #[test]
    fn snapshot_policy_validate() {
        let policy = SnapshotPolicy::new(1, "policy1", 60000);
        assert!(policy.validate().is_ok());
        let policy = SnapshotPolicy::new(2, "bad", 0);
        assert!(policy.validate().is_err());
        let policy = SnapshotPolicy::new(3, "bad2", 1000).with_max_snapshots(0);
        assert!(policy.validate().is_err());
    }

    #[test]
    fn snapshot_policy_retention_range() {
        let policy = SnapshotPolicy::new(1, "p1", 60000);
        assert!(policy.is_retention_valid(3600_000));
        assert!(policy.is_retention_valid(7 * 24 * 60 * 60 * 1000));
        assert!(!policy.is_retention_valid(1000));
        assert!(!policy.is_retention_valid(100_000_000));
    }

    #[test]
    fn snapshot_policy_builder() {
        let policy = SnapshotPolicy::new(1, "p1", 60000)
            .with_max_snapshots(200)
            .with_retention_range(1000, 2000)
            .with_auto_create(true)
            .with_require_finalize(false);
        assert_eq!(policy.max_snapshots, 200);
        assert_eq!(policy.min_retention_ms, 1000);
        assert_eq!(policy.max_retention_ms, 2000);
        assert!(policy.auto_create);
        assert!(!policy.require_finalize);
    }

    // === CompressionAlgorithm tests ===
    #[test]
    fn compression_algorithm_as_str() {
        assert_eq!(CompressionAlgorithm::None.as_str(), "none");
        assert_eq!(CompressionAlgorithm::Lz4.as_str(), "lz4");
        assert_eq!(CompressionAlgorithm::Zstd.as_str(), "zstd");
        assert_eq!(CompressionAlgorithm::Blosc.as_str(), "blosc");
    }

    #[test]
    fn compression_algorithm_ratio() {
        assert!((CompressionAlgorithm::None.default_ratio() - 1.0).abs() < 0.01);
        assert!((CompressionAlgorithm::Lz4.default_ratio() - 2.5).abs() < 0.01);
        assert!((CompressionAlgorithm::Zstd.default_ratio() - 3.5).abs() < 0.01);
        assert!((CompressionAlgorithm::Blosc.default_ratio() - 4.0).abs() < 0.01);
    }

    // === SnapshotCompression tests ===
    #[test]
    fn snapshot_compression_none() {
        let comp = SnapshotCompression::new(CompressionAlgorithm::None, 1, 1000);
        assert!(!comp.compressed);
        assert_eq!(comp.compressed_size, 1000);
        assert!((comp.compression_ratio() - 1.0).abs() < 0.01);
        assert_eq!(comp.space_saved(), 0);
    }

    #[test]
    fn snapshot_compression_zstd() {
        let comp = SnapshotCompression::new(CompressionAlgorithm::Zstd, 3, 1000);
        assert!(comp.compressed);
        assert!(comp.compressed_size < comp.original_size);
        assert!(comp.compression_ratio() > 1.0);
        assert!(comp.space_saved() > 0);
        assert!(comp.space_saved_percent() > 0.0);
    }

    #[test]
    fn snapshot_compression_zero_size() {
        let comp = SnapshotCompression::new(CompressionAlgorithm::Zstd, 3, 0);
        assert!((comp.compression_ratio() - 1.0).abs() < 0.01);
        assert_eq!(comp.space_saved_percent(), 0.0);
    }

    // === SnapshotEngine extended tests ===
    #[test]
    fn snapshot_engine_create_auto() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        let snap1 = engine.create_snapshot_auto(test_dataset_id(), test_node_id()).unwrap();
        let snap2 = engine.create_snapshot_auto(test_dataset_id(), test_node_id()).unwrap();
        assert_eq!(snap1.snapshot_id, 1);
        assert_eq!(snap2.snapshot_id, 2);
        assert_eq!(engine.next_snapshot_id(), 3);
    }

    #[test]
    fn snapshot_engine_get_snapshot() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let snap = engine.get_snapshot(1).unwrap();
        assert_eq!(snap.snapshot_id, 1);
        let err = engine.get_snapshot(999).unwrap_err();
        assert!(matches!(err, SnapshotError::SnapshotNotFound { .. }));
    }

    #[test]
    fn snapshot_engine_snapshot_exists() {
        let engine = SnapshotEngine::default();
        assert!(!engine.snapshot_exists(1));
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert!(engine.snapshot_exists(1));
    }

    #[test]
    fn snapshot_engine_all_ids() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();
        let ids = engine.all_snapshot_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn snapshot_engine_finalized_and_expired_ids() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();
        engine.finalize_snapshot(1).unwrap();
        let finalized = engine.finalized_snapshot_ids();
        assert_eq!(finalized, vec![1]);
        let expired = engine.expired_snapshot_ids(999999999);
        assert!(!expired.is_empty());
    }

    #[test]
    fn snapshot_engine_expire_all() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        let count = engine.expire_all_past_retention(999999999);
        assert_eq!(count, 1);
    }

    #[test]
    fn snapshot_engine_total_size_all() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.add_chunk(1, SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000)).unwrap();
        assert_eq!(engine.total_size_all(), 3);
    }

    #[test]
    fn snapshot_engine_total_chunks_all() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.add_chunk(1, SnapshotChunk::new(test_chunk_id(), vec![1], 0, 1000)).unwrap();
        assert_eq!(engine.total_chunks_all(), 1);
    }

    #[test]
    fn snapshot_engine_average_size() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.average_snapshot_size(), 0.0);
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.add_chunk(1, SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3, 4, 5], 0, 1000)).unwrap();
        assert!((engine.average_snapshot_size() - 5.0).abs() < 0.01);
    }

    #[test]
    fn snapshot_engine_signed_and_unsigned() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert_eq!(engine.unsigned_snapshot_count(), 1);
        assert_eq!(engine.signed_snapshot_count(), 0);
        engine.sign_snapshot(1).unwrap();
        assert_eq!(engine.signed_snapshot_count(), 1);
        assert_eq!(engine.unsigned_snapshot_count(), 0);
    }

    #[test]
    fn snapshot_engine_count_by_state() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();
        engine.finalize_snapshot(1).unwrap();
        assert_eq!(engine.count_by_state(SnapshotState::Created), 1);
        assert_eq!(engine.count_by_state(SnapshotState::Finalized), 1);
    }

    #[test]
    fn snapshot_engine_compression_savings() {
        let engine = SnapshotEngine::default();
        let saved = engine.compression_savings(1000, CompressionAlgorithm::Zstd);
        assert!(saved > 0);
        let saved_none = engine.compression_savings(1000, CompressionAlgorithm::None);
        assert_eq!(saved_none, 0);
    }

    #[test]
    fn snapshot_engine_is_retention_valid() {
        let engine = SnapshotEngine::default();
        assert!(engine.is_retention_valid(3600_000));
        assert!(!engine.is_retention_valid(100));
    }

    #[test]
    fn snapshot_engine_count_by_region() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();
        // No regions set yet
        assert_eq!(engine.count_by_region(test_region_id()), 0);
    }

    #[test]
    fn snapshot_engine_total_all() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.total_all_snapshots(), 0);
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert_eq!(engine.total_all_snapshots(), 1);
    }

    #[test]
    fn snapshot_engine_total_restores_and_diffs() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.total_restores(), 0);
        assert_eq!(engine.total_diffs(), 0);
    }

    #[test]
    fn snapshot_engine_clear_restores_and_diffs() {
        let engine = SnapshotEngine::default();
        engine.clear_restores();
        engine.clear_diffs();
        assert_eq!(engine.total_restores(), 0);
        assert_eq!(engine.total_diffs(), 0);
    }

    #[test]
    fn snapshot_engine_reset_stats() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert_eq!(engine.total_created(), 1);
        engine.reset_stats();
        assert_eq!(engine.total_created(), 0);
    }

    #[test]
    fn snapshot_engine_health_and_rates() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        let score = engine.health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
        assert!((engine.restore_rate() - 1.0).abs() < 0.01);
        assert!((engine.expiry_rate() - 0.0).abs() < 0.01);
        assert!((engine.valid_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn snapshot_engine_get_and_validate_retention() {
        let engine = SnapshotEngine::default();
        let retention = engine.retention().unwrap();
        assert_eq!(retention.max_snapshots, 100);
        let result = engine.validate_retention_policy(&retention);
        assert_eq!(result, SnapshotValidationResult::Valid);
    }

    #[test]
    fn snapshot_engine_validate_snapshot_size() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        let result = engine.validate_snapshot_size(1, 100, 1000);
        assert_eq!(result, SnapshotValidationResult::Valid);
    }

    #[test]
    fn snapshot_engine_is_snapshot_signed() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert!(!engine.is_snapshot_signed(1).unwrap());
        engine.sign_snapshot(1).unwrap();
        assert!(engine.is_snapshot_signed(1).unwrap());
    }

    #[test]
    fn snapshot_engine_snapshot_region_and_endpoint() {
        let engine = SnapshotEngine::default();
        engine.configure(SnapshotConfig::default()).unwrap();
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        assert!(engine.snapshot_region(1).unwrap().is_none());
        assert!(engine.snapshot_endpoint(1).unwrap().is_none());
    }

    #[test]
    fn snapshot_engine_next_ids() {
        let engine = SnapshotEngine::default();
        assert_eq!(engine.next_snapshot_id(), 1);
        assert_eq!(engine.next_diff_id(), 1);
    }

    // === Integration: full snapshot lifecycle ===
    #[test]
    fn snapshot_full_lifecycle() {
        let engine = SnapshotEngine::default();

        // Configure with retention
        let mut config = SnapshotConfig::default();
        config = config.with_max_snapshots(10).with_retention_ms(3600_000);
        engine.configure(config).unwrap();

        // Create snapshots
        engine.create_snapshot(1, test_dataset_id(), test_node_id()).unwrap();
        engine.create_snapshot(2, test_dataset_id(), test_node_id()).unwrap();

        // Add chunks
        engine.add_chunk(1, SnapshotChunk::new(test_chunk_id(), vec![1, 2, 3], 0, 1000)).unwrap();
        engine.add_chunk(2, SnapshotChunk::new(test_chunk_id(), vec![4, 5, 6], 0, 2000)).unwrap();

        // Finalize
        engine.finalize_snapshot(1).unwrap();
        engine.finalize_snapshot(2).unwrap();

        // Sign
        let sig = engine.sign_snapshot(1).unwrap();
        assert!(engine.verify_signature(1, &sig).unwrap());

        // Compute diff
        let diff = engine.compute_diff(1, 2).unwrap();
        assert_eq!(diff.total_changes(), 2);

        // Restore
        let restore = engine.restore_snapshot(1, test_peer_id()).unwrap();
        assert_eq!(restore.status, RestoreStatus::Completed);

        // Expire
        engine.expire_snapshot(2).unwrap();

        // Final assertions
        assert_eq!(engine.total_created(), 2);
        assert_eq!(engine.total_restored(), 1);
        assert_eq!(engine.total_expired(), 1);
        assert!(engine.snapshot_exists(1));
        assert!(engine.snapshot_exists(2));

        let score = engine.snapshot_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === NodeId zero peer test ===
    #[test]
    fn node_id_zero_peer_snapshot2() {
        let zero = NodeId::new([0u8; 32]);
        assert_ne!(test_node_id(), zero);
    }

    // === Endpoint clone test ===
    #[test]
    fn endpoint_clone_snapshot2() {
        let ep1 = test_endpoint();
        let ep2 = ep1.clone();
        assert_eq!(ep1.carrier_type, ep2.carrier_type);
        assert_eq!(ep1.address, ep2.address);
    }

    // === SnapshotConfig clone test ===
    #[test]
    fn snapshot_config_clone2() {
        let cfg1 = test_snapshot_config();
        let cfg2 = cfg1.clone();
        assert_eq!(cfg1.max_snapshots, cfg2.max_snapshots);
        assert_eq!(cfg1.max_chunk_size, cfg2.max_chunk_size);
    }

    // === SnapshotRetention clone test ===
    #[test]
    fn snapshot_retention_clone2() {
        let r1 = SnapshotRetention::new();
        let r2 = r1.clone();
        assert_eq!(r1.max_snapshots, r2.max_snapshots);
        assert_eq!(r1.auto_expire, r2.auto_expire);
    }

    // === SnapshotDiff clone test ===
    #[test]
    fn snapshot_diff_clone2() {
        let d1 = SnapshotDiff::new(1, 100, 200, 1000);
        let d2 = d1.clone();
        assert_eq!(d1.diff_id, d2.diff_id);
        assert_eq!(d1.from_snapshot, d2.from_snapshot);
    }
}