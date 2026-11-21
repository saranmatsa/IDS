//! # ids-000-aa Contract: Store
//!
//! `trait Store` — distributed data put/get/delete, replication, consistency.
//! Carrier-agnostic: operates on opaque byte objects and IDs only.

use crate::types::{
    ChunkID, DatasetId, Endpoint, HUnitId, ObjectID, OpRef, RegionId,
};
use crate::errors::StoreError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Store
///////////////////////////////////////////////////////////////////////////////

/// Distributed object store: put, get, delete with replication and consistency.
/// Sealed: only ids-000-aa and designated impl crates can implement.
pub trait Store: private::Sealed {
    /// Store an object, returning an OpRef for tracking.
    fn put(&self, dataset: DatasetId, object_id: ObjectID, bytes: Vec<u8>)
        -> Result<OpRef, StoreError>;

    /// Retrieve an object by ID.
    fn get(&self, dataset: DatasetId, object_id: ObjectID)
        -> Result<Option<Vec<u8>>, StoreError>;

    /// Delete an object by ID.
    fn delete(&self, dataset: DatasetId, object_id: ObjectID)
        -> Result<bool, StoreError>;

    /// Check if an object exists.
    fn exists(&self, dataset: DatasetId, object_id: ObjectID)
        -> Result<bool, StoreError>;

    /// Get the size in bytes of a stored object.
    fn object_size(&self, dataset: DatasetId, object_id: ObjectID)
        -> Result<Option<u64>, StoreError>;

    /// List object IDs in a dataset (with optional prefix filter).
    fn list(&self, dataset: DatasetId, prefix: Option<&[u8]>)
        -> Result<Vec<ObjectID>, StoreError>;

    /// Replicate an object to a target region/HUnit.
    fn replicate(&self, object_id: ObjectID, target: Endpoint)
        -> Result<(), StoreError>;

    /// Get the replication factor for a dataset.
    fn replication_factor(&self, dataset: DatasetId) -> u32 {
        3
    }

    /// Get the consistency level for a dataset.
    fn consistency_level(&self, dataset: DatasetId) -> ConsistencyLevel {
        ConsistencyLevel::Eventual
    }

    /// Get the region this store instance is local to.
    fn local_region(&self) -> RegionId {
        RegionId::zero()
    }

    /// Get the HUnit this store instance belongs to.
    fn local_hunit(&self) -> HUnitId {
        HUnitId::zero()
    }

    /// Get total objects stored.
    fn object_count(&self) -> u64 {
        0
    }

    /// Get total bytes stored.
    fn byte_count(&self) -> u64 {
        0
    }

    /// Flush pending writes to durable storage.
    fn flush(&self) -> Result<(), StoreError> {
        Ok(())
    }

    /// Compact storage, reclaiming space from deleted objects.
    fn compact(&self) -> Result<(), StoreError> {
        Ok(())
    }

    /// Get the store's name for debugging.
    fn store_name(&self) -> &'static str {
        "store"
    }

    /// Check if the store is healthy.
    fn is_healthy(&self) -> bool {
        true
    }

    /// Get the maximum object size in bytes.
    fn max_object_size(&self) -> u64 {
        1048576
    }

    /// Get the number of pending replication operations.
    fn pending_replications(&self) -> u64 {
        0
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED SUPERTRAIT MODULE
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed supertrait — prevents downstream crates from implementing Store.
    pub trait Sealed {}
    impl Sealed for crate::types::ObjectID {}
}

///////////////////////////////////////////////////////////////////////////////
// ERROR TYPE: StoreError
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("object not found: dataset={dataset}, object={object_id}")]
    ObjectNotFound { dataset: DatasetId, object_id: ObjectID },

    #[error("object already exists: dataset={dataset}, object={object_id}")]
    ObjectAlreadyExists { dataset: DatasetId, object_id: ObjectID },

    #[error("dataset not found: {dataset}")]
    DatasetNotFound { dataset: DatasetId },

    #[error("dataset already exists: {dataset}")]
    DatasetAlreadyExists { dataset: DatasetId },

    #[error("write failed: dataset={dataset}, object={object_id}, reason={reason}")]
    WriteFailed {
        dataset: DatasetId,
        object_id: ObjectID,
        reason: String,
    },

    #[error("read failed: dataset={dataset}, object={object_id}, reason={reason}")]
    ReadFailed {
        dataset: DatasetId,
        object_id: ObjectID,
        reason: String,
    },

    #[error("delete failed: dataset={dataset}, object={object_id}, reason={reason}")]
    DeleteFailed {
        dataset: DatasetId,
        object_id: ObjectID,
        reason: String,
    },

    #[error("replication failed: object={object_id}, target={target:?}, reason={reason}")]
    ReplicationFailed {
        object_id: ObjectID,
        target: Endpoint,
        reason: String,
    },

    #[error("consistency violation: expected {expected}, got {got}")]
    ConsistencyViolation { expected: u32, got: u32 },

    #[error("object too large: {size} bytes (max {max})")]
    ObjectTooLarge { size: u64, max: u64 },

    #[error("store not initialized")]
    NotInitialized,

    #[error("store already initialized")]
    AlreadyInitialized,

    #[error("flush failed: {reason}")]
    FlushFailed { reason: String },

    #[error("compact failed: {reason}")]
    CompactFailed { reason: String },

    #[error("store internal error: {message}")]
    Internal { message: String },

    #[error("invalid object ID: {object_id}")]
    InvalidObjectId { object_id: ObjectID },
}

impl StoreError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StoreError::WriteFailed { .. }
                | StoreError::ReplicationFailed { .. }
                | StoreError::FlushFailed { .. }
                | StoreError::CompactFailed { .. }
                | StoreError::Internal { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        !self.is_retryable()
    }

    pub fn category(&self) -> &'static str {
        match self {
            StoreError::ObjectNotFound { .. } => "object_not_found",
            StoreError::ObjectAlreadyExists { .. } => "object_already_exists",
            StoreError::DatasetNotFound { .. } => "dataset_not_found",
            StoreError::DatasetAlreadyExists { .. } => "dataset_already_exists",
            StoreError::WriteFailed { .. } => "write_failed",
            StoreError::ReadFailed { .. } => "read_failed",
            StoreError::DeleteFailed { .. } => "delete_failed",
            StoreError::ReplicationFailed { .. } => "replication_failed",
            StoreError::ConsistencyViolation { .. } => "consistency_violation",
            StoreError::ObjectTooLarge { .. } => "object_too_large",
            StoreError::NotInitialized => "not_initialized",
            StoreError::AlreadyInitialized => "already_initialized",
            StoreError::FlushFailed { .. } => "flush_failed",
            StoreError::CompactFailed { .. } => "compact_failed",
            StoreError::Internal { .. } => "internal",
            StoreError::InvalidObjectId { .. } => "invalid_object_id",
        }
    }

    pub fn plain_message(&self) -> String {
        match self {
            StoreError::ObjectNotFound { dataset, object_id } => {
                format!("object not found: dataset={dataset}, object={object_id}")
            }
            StoreError::ObjectAlreadyExists { dataset, object_id } => {
                format!("object already exists: dataset={dataset}, object={object_id}")
            }
            StoreError::DatasetNotFound { dataset } => {
                format!("dataset not found: {dataset}")
            }
            StoreError::DatasetAlreadyExists { dataset } => {
                format!("dataset already exists: {dataset}")
            }
            StoreError::WriteFailed {
                dataset,
                object_id,
                reason,
            } => format!(
                "write failed: dataset={dataset}, object={object_id}, reason={reason}"
            ),
            StoreError::ReadFailed {
                dataset,
                object_id,
                reason,
            } => format!(
                "read failed: dataset={dataset}, object={object_id}, reason={reason}"
            ),
            StoreError::DeleteFailed {
                dataset,
                object_id,
                reason,
            } => format!(
                "delete failed: dataset={dataset}, object={object_id}, reason={reason}"
            ),
            StoreError::ReplicationFailed {
                object_id,
                target,
                reason,
            } => format!(
                "replication failed: object={object_id}, target={target:?}, reason={reason}"
            ),
            StoreError::ConsistencyViolation { expected, got } => {
                format!("consistency violation: expected {expected}, got {got}")
            }
            StoreError::ObjectTooLarge { size, max } => {
                format!("object too large: {size} bytes (max {max})")
            }
            StoreError::NotInitialized => "store not initialized".to_string(),
            StoreError::AlreadyInitialized => "store already initialized".to_string(),
            StoreError::FlushFailed { reason } => {
                format!("flush failed: {reason}")
            }
            StoreError::CompactFailed { reason } => {
                format!("compact failed: {reason}")
            }
            StoreError::Internal { message } => {
                format!("store internal error: {message}")
            }
            StoreError::InvalidObjectId { object_id } => {
                format!("invalid object ID: {object_id}")
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY LEVEL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsistencyLevel {
    Any,
    One,
    Quorum,
    All,
    Eventual,
    Strong,
}

impl ConsistencyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConsistencyLevel::Any => "any",
            ConsistencyLevel::One => "one",
            ConsistencyLevel::Quorum => "quorum",
            ConsistencyLevel::All => "all",
            ConsistencyLevel::Eventual => "eventual",
            ConsistencyLevel::Strong => "strong",
        }
    }

    pub fn required_acks(&self, replication_factor: u32) -> u32 {
        match self {
            ConsistencyLevel::Any => 1,
            ConsistencyLevel::One => 1,
            ConsistencyLevel::Quorum => replication_factor / 2 + 1,
            ConsistencyLevel::All => replication_factor,
            ConsistencyLevel::Eventual => 1,
            ConsistencyLevel::Strong => replication_factor,
        }
    }

    pub fn is_strong(&self) -> bool {
        matches!(self, ConsistencyLevel::Strong | ConsistencyLevel::All)
    }

    pub fn is_eventual(&self) -> bool {
        matches!(self, ConsistencyLevel::Eventual | ConsistencyLevel::Any)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OBJECT METADATA
///////////////////////////////////////////////////////////////////////////////

/// Metadata associated with a stored object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
    pub object_id: ObjectID,
    pub dataset: DatasetId,
    pub size_bytes: u64,
    pub created_at_ms: u64,
    pub modified_at_ms: u64,
    pub version: u64,
    pub replication_factor: u32,
    pub consistency_level: ConsistencyLevel,
    pub region: RegionId,
    pub hunit: HUnitId,
}

impl ObjectMetadata {
    pub fn new(
        object_id: ObjectID,
        dataset: DatasetId,
        size_bytes: u64,
        region: RegionId,
        hunit: HUnitId,
    ) -> Self {
        let now = 0; // placeholder timestamp
        Self {
            object_id,
            dataset,
            size_bytes,
            created_at_ms: now,
            modified_at_ms: now,
            version: 1,
            replication_factor: 3,
            consistency_level: ConsistencyLevel::Eventual,
            region,
            hunit,
        }
    }

    pub fn with_replication_factor(mut self, rf: u32) -> Self {
        self.replication_factor = rf;
        self
    }

    pub fn with_consistency_level(mut self, cl: ConsistencyLevel) -> Self {
        self.consistency_level = cl;
        self
    }

    pub fn bump_version(&mut self) {
        self.version += 1;
        self.modified_at_ms = 0;
    }

    pub fn required_acks(&self) -> u32 {
        self.consistency_level.required_acks(self.replication_factor)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaState {
    Pending,
    Replicating,
    Replicated,
    Failed,
    Deleted,
}

impl ReplicaState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplicaState::Pending => "pending",
            ReplicaState::Replicating => "replicating",
            ReplicaState::Replicated => "replicated",
            ReplicaState::Failed => "failed",
            ReplicaState::Deleted => "deleted",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ReplicaState::Replicated | ReplicaState::Deleted)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ReplicaState::Replicated | ReplicaState::Replicating)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA INFO
///////////////////////////////////////////////////////////////////////////////

/// Information about a single replica of an object.
#[derive(Debug, Clone)]
pub struct ReplicaInfo {
    pub object_id: ObjectID,
    pub endpoint: Endpoint,
    pub state: ReplicaState,
    pub created_at_ms: u64,
    pub last_heartbeat_ms: u64,
    pub replication_attempts: u32,
}

impl ReplicaInfo {
    pub fn new(object_id: ObjectID, endpoint: Endpoint) -> Self {
        Self {
            object_id,
            endpoint,
            state: ReplicaState::Pending,
            created_at_ms: 0,
            last_heartbeat_ms: 0,
            replication_attempts: 0,
        }
    }

    pub fn mark_replicating(&mut self, now: u64) {
        self.state = ReplicaState::Replicating;
        self.last_heartbeat_ms = now;
        self.replication_attempts += 1;
    }

    pub fn mark_replicated(&mut self, now: u64) {
        self.state = ReplicaState::Replicated;
        self.last_heartbeat_ms = now;
    }

    pub fn mark_failed(&mut self, now: u64) {
        self.state = ReplicaState::Failed;
        self.last_heartbeat_ms = now;
    }

    pub fn mark_deleted(&mut self, now: u64) {
        self.state = ReplicaState::Deleted;
        self.last_heartbeat_ms = now;
    }

    pub fn heartbeat(&mut self, now: u64) {
        self.last_heartbeat_ms = now;
    }

    pub fn is_stale(&self, now: u64, threshold_ms: u64) -> bool {
        now.saturating_sub(self.last_heartbeat_ms) > threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORE STATISTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct StoreStats {
    pub puts: u64,
    pub gets: u64,
    pub deletes: u64,
    pub puts_failed: u64,
    pub gets_failed: u64,
    pub deletes_failed: u64,
    pub replications: u64,
    pub replications_failed: u64,
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub object_count: u64,
    pub byte_count: u64,
}

impl StoreStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_operations(&self) -> u64 {
        self.puts + self.gets + self.deletes
    }

    pub fn total_failures(&self) -> u64 {
        self.puts_failed + self.gets_failed + self.deletes_failed
            + self.replications_failed
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.total_operations();
        if total == 0 {
            return 1.0;
        }
        (total - self.total_failures()) as f64 / total as f64
    }

    pub fn replication_success_rate(&self) -> f64 {
        if self.replications == 0 {
            return 1.0;
        }
        (self.replications - self.replications_failed) as f64
            / self.replications as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORE CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for store behavior.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub default_replication_factor: u32,
    pub default_consistency_level: ConsistencyLevel,
    pub max_object_size: u64,
    pub enable_replication: bool,
    pub enable_compaction: bool,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            default_replication_factor: 3,
            default_consistency_level: ConsistencyLevel::Eventual,
            max_object_size: 1048576,
            enable_replication: true,
            enable_compaction: false,
        }
    }
}

impl StoreConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_replication_factor(mut self, rf: u32) -> Self {
        self.default_replication_factor = rf;
        self
    }

    pub fn with_consistency_level(mut self, cl: ConsistencyLevel) -> Self {
        self.default_consistency_level = cl;
        self
    }

    pub fn with_max_object_size(mut self, size: u64) -> Self {
        self.max_object_size = size;
        self
    }

    pub fn with_replication(mut self, enabled: bool) -> Self {
        self.enable_replication = enabled;
        self
    }

    pub fn validate(&self) -> Result<(), StoreError> {
        if self.default_replication_factor == 0 {
            return Err(StoreError::Internal {
                message: "replication factor must be non-zero".to_string(),
            });
        }
        if self.max_object_size == 0 {
            return Err(StoreError::Internal {
                message: "max object size must be non-zero".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// OBJECT REFERENCE TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks object references across datasets for garbage collection.
pub struct ObjectRefTracker {
    pub refs: std::collections::HashMap<ObjectID, u64>,
    pub datasets: std::collections::HashMap<ObjectID, DatasetId>,
}

impl ObjectRefTracker {
    pub fn new() -> Self {
        Self {
            refs: std::collections::HashMap::new(),
            datasets: std::collections::HashMap::new(),
        }
    }

    pub fn add_ref(&mut self, object_id: ObjectID, dataset: DatasetId) {
        *self.refs.entry(object_id).or_insert(0) += 1;
        self.datasets.insert(object_id, dataset);
    }

    pub fn remove_ref(&mut self, object_id: ObjectID) -> bool {
        if let Some(count) = self.refs.get_mut(&object_id) {
            *count -= 1;
            if *count == 0 {
                self.refs.remove(&object_id);
                self.datasets.remove(&object_id);
                return true;
            }
        }
        false
    }

    pub fn ref_count(&self, object_id: ObjectID) -> u64 {
        self.refs.get(&object_id).copied().unwrap_or(0)
    }

    pub fn has_refs(&self, object_id: ObjectID) -> bool {
        self.ref_count(object_id) > 0
    }

    pub fn total_tracked(&self) -> usize {
        self.refs.len()
    }
}

impl Default for ObjectRefTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks object versions for conflict detection.
pub struct VersionTracker {
    pub versions: std::collections::HashMap<ObjectID, u64>,
}

impl VersionTracker {
    pub fn new() -> Self {
        Self {
            versions: std::collections::HashMap::new(),
        }
    }

    pub fn current_version(&self, object_id: ObjectID) -> u64 {
        self.versions.get(&object_id).copied().unwrap_or(0)
    }

    pub fn set_version(&mut self, object_id: ObjectID, version: u64) {
        self.versions.insert(object_id, version);
    }

    pub fn bump_version(&mut self, object_id: ObjectID) -> u64 {
        let current = self.current_version(object_id);
        let next = current + 1;
        self.versions.insert(object_id, next);
        next
    }

    pub fn is_conflict(&self, object_id: ObjectID, expected: u64) -> bool {
        self.current_version(object_id) != expected
    }

    pub fn total_tracked(&self) -> usize {
        self.versions.len()
    }
}

impl Default for VersionTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE BACKEND
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    Memory,
    Disk,
    Hybrid,
    Remote,
}

impl StorageBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageBackend::Memory => "memory",
            StorageBackend::Disk => "disk",
            StorageBackend::Hybrid => "hybrid",
            StorageBackend::Remote => "remote",
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, StorageBackend::Memory | StorageBackend::Disk)
    }

    pub fn is_durable(&self) -> bool {
        matches!(self, StorageBackend::Disk | StorageBackend::Hybrid)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE DEVICE
///////////////////////////////////////////////////////////////////////////////

pub struct StorageDevice {
    pub device_id: u64,
    pub backend: StorageBackend,
    pub capacity_bytes: u64,
    pub used_bytes: u64,
    pub read_only: bool,
}

impl StorageDevice {
    pub fn new(device_id: u64, backend: StorageBackend, capacity_bytes: u64) -> Self {
        Self {
            device_id,
            backend,
            capacity_bytes,
            used_bytes: 0,
            read_only: false,
        }
    }

    pub fn available_bytes(&self) -> u64 {
        self.capacity_bytes.saturating_sub(self.used_bytes)
    }

    pub fn utilization(&self) -> f64 {
        if self.capacity_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.capacity_bytes as f64
    }

    pub fn can_write(&self, bytes: u64) -> bool {
        !self.read_only && self.available_bytes() >= bytes
    }

    pub fn write(&mut self, bytes: u64) -> Result<(), StoreError> {
        if self.read_only {
            return Err(StoreError::Internal {
                message: "storage device is read-only".to_string(),
            });
        }
        if !self.can_write(bytes) {
            return Err(StoreError::Internal {
                message: "insufficient storage space".to_string(),
            });
        }
        self.used_bytes += bytes;
        Ok(())
    }

    pub fn free(&mut self, bytes: u64) {
        self.used_bytes = self.used_bytes.saturating_sub(bytes);
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE QUOTA
///////////////////////////////////////////////////////////////////////////////

pub struct StorageQuota {
    pub max_objects: u64,
    pub max_bytes: u64,
    pub current_objects: u64,
    pub current_bytes: u64,
}

impl StorageQuota {
    pub fn new(max_objects: u64, max_bytes: u64) -> Self {
        Self {
            max_objects,
            max_bytes,
            current_objects: 0,
            current_bytes: 0,
        }
    }

    pub fn can_allocate(&self, objects: u64, bytes: u64) -> bool {
        self.current_objects + objects <= self.max_objects
            && self.current_bytes + bytes <= self.max_bytes
    }

    pub fn allocate(&mut self, objects: u64, bytes: u64) -> Result<(), StoreError> {
        if !self.can_allocate(objects, bytes) {
            return Err(StoreError::Internal {
                message: "quota exceeded".to_string(),
            });
        }
        self.current_objects += objects;
        self.current_bytes += bytes;
        Ok(())
    }

    pub fn release(&mut self, objects: u64, bytes: u64) {
        self.current_objects = self.current_objects.saturating_sub(objects);
        self.current_bytes = self.current_bytes.saturating_sub(bytes);
    }

    pub fn object_utilization(&self) -> f64 {
        if self.max_objects == 0 {
            return 0.0;
        }
        self.current_objects as f64 / self.max_objects as f64
    }

    pub fn byte_utilization(&self) -> f64 {
        if self.max_bytes == 0 {
            return 0.0;
        }
        self.current_bytes as f64 / self.max_bytes as f64
    }
}

impl Default for StorageQuota {
    fn default() -> Self {
        Self::new(1000000, 1073741824)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE CACHE
///////////////////////////////////////////////////////////////////////////////

pub struct StorageCache {
    pub entries: std::cell::RefCell<std::collections::HashMap<ObjectID, Vec<u8>>>,
    pub max_entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl StorageCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_entries,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    pub fn get(&self, object_id: ObjectID) -> Option<Vec<u8>> {
        if let Some(data) = self.entries.borrow().get(&object_id) {
            self.hits += 1;
            Some(data.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn put(&self, object_id: ObjectID, data: Vec<u8>) {
        if self.entries.borrow().len() >= self.max_entries {
            self.evict_one();
        }
        self.entries.borrow_mut().insert(object_id, data);
    }

    fn evict_one(&self) {
        if let Some(first_key) = self.entries.borrow().keys().next().copied() {
            self.entries.borrow_mut().remove(&first_key);
            self.evictions += 1;
        }
    }

    pub fn remove(&self, object_id: ObjectID) -> bool {
        self.entries.borrow_mut().remove(&object_id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

impl Default for StorageCache {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE INDEX
///////////////////////////////////////////////////////////////////////////////

pub struct StorageIndex {
    pub entries: std::cell::RefCell<std::collections::BTreeMap<ObjectID, u64>>,
}

impl StorageIndex {
    pub fn new() -> Self {
        Self {
            entries: std::cell::RefCell::new(std::collections::BTreeMap::new()),
        }
    }

    pub fn insert(&self, object_id: ObjectID, offset: u64) {
        self.entries.borrow_mut().insert(object_id, offset);
    }

    pub fn get(&self, object_id: ObjectID) -> Option<u64> {
        self.entries.borrow().get(&object_id).copied()
    }

    pub fn remove(&self, object_id: ObjectID) -> bool {
        self.entries.borrow_mut().remove(&object_id).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    pub fn contains(&self, object_id: ObjectID) -> bool {
        self.entries.borrow().contains_key(&object_id)
    }
}

impl Default for StorageIndex {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    pub snapshot_id: u64,
    pub created_at_ms: u64,
    pub object_count: u64,
    pub byte_count: u64,
    pub dataset_ids: Vec<DatasetId>,
}

impl StorageSnapshot {
    pub fn new(snapshot_id: u64, object_count: u64, byte_count: u64) -> Self {
        Self {
            snapshot_id,
            created_at_ms: 0,
            object_count,
            byte_count,
            dataset_ids: Vec::new(),
        }
    }

    pub fn with_dataset(mut self, dataset: DatasetId) -> Self {
        self.dataset_ids.push(dataset);
        self
    }

    pub fn contains_dataset(&self, dataset: DatasetId) -> bool {
        self.dataset_ids.contains(&dataset)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE COMPACTION MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageCompactionManager {
    pub last_compact_ms: u64,
    pub compact_interval_ms: u64,
    pub total_compactions: u64,
    pub bytes_reclaimed: u64,
}

impl StorageCompactionManager {
    pub fn new(compact_interval_ms: u64) -> Self {
        Self {
            last_compact_ms: 0,
            compact_interval_ms,
            total_compactions: 0,
            bytes_reclaimed: 0,
        }
    }

    pub fn should_compact(&self, now: u64) -> bool {
        now.saturating_sub(self.last_compact_ms) >= self.compact_interval_ms
    }

    pub fn record_compaction(&mut self, now: u64, bytes_reclaimed: u64) {
        self.last_compact_ms = now;
        self.total_compactions += 1;
        self.bytes_reclaimed += bytes_reclaimed;
    }

    pub fn compaction_rate(&self) -> f64 {
        if self.total_compactions == 0 {
            return 0.0;
        }
        self.bytes_reclaimed as f64 / self.total_compactions as f64
    }
}

impl Default for StorageCompactionManager {
    fn default() -> Self {
        Self::new(3600000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE WRITER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageWriter {
    pub writes: u64,
    pub bytes_written: u64,
    pub write_errors: u64,
    pub last_write_ms: u64,
}

impl StorageWriter {
    pub fn new() -> Self {
        Self {
            writes: 0,
            bytes_written: 0,
            write_errors: 0,
            last_write_ms: 0,
        }
    }

    pub fn record_write(&mut self, bytes: u64, timestamp_ms: u64) {
        self.writes += 1;
        self.bytes_written += bytes;
        self.last_write_ms = timestamp_ms;
    }

    pub fn record_error(&mut self) {
        self.write_errors += 1;
    }

    pub fn average_write_size(&self) -> f64 {
        if self.writes == 0 {
            return 0.0;
        }
        self.bytes_written as f64 / self.writes as f64
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.writes + self.write_errors;
        if total == 0 {
            return 0.0;
        }
        self.write_errors as f64 / total as f64
    }
}

impl Default for StorageWriter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE READER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageReader {
    pub reads: u64,
    pub bytes_read: u64,
    pub read_errors: u64,
    pub last_read_ms: u64,
}

impl StorageReader {
    pub fn new() -> Self {
        Self {
            reads: 0,
            bytes_read: 0,
            read_errors: 0,
            last_read_ms: 0,
        }
    }

    pub fn record_read(&mut self, bytes: u64, timestamp_ms: u64) {
        self.reads += 1;
        self.bytes_read += bytes;
        self.last_read_ms = timestamp_ms;
    }

    pub fn record_error(&mut self) {
        self.read_errors += 1;
    }

    pub fn average_read_size(&self) -> f64 {
        if self.reads == 0 {
            return 0.0;
        }
        self.bytes_read as f64 / self.reads as f64
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.reads + self.read_errors;
        if total == 0 {
            return 0.0;
        }
        self.read_errors as f64 / total as f64
    }
}

impl Default for StorageReader {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE REPLICATOR
///////////////////////////////////////////////////////////////////////////////

pub struct StorageReplicator {
    pub replications_sent: u64,
    pub replications_received: u64,
    pub replications_failed: u64,
    pub pending_replications: u64,
}

impl StorageReplicator {
    pub fn new() -> Self {
        Self {
            replications_sent: 0,
            replications_received: 0,
            replications_failed: 0,
            pending_replications: 0,
        }
    }

    pub fn record_sent(&mut self) {
        self.replications_sent += 1;
        self.pending_replications += 1;
    }

    pub fn record_received(&mut self) {
        self.replications_received += 1;
        self.pending_replications = self.pending_replications.saturating_sub(1);
    }

    pub fn record_failed(&mut self) {
        self.replications_failed += 1;
        self.pending_replications = self.pending_replications.saturating_sub(1);
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.replications_sent;
        if total == 0 {
            return 1.0;
        }
        (self.replications_sent - self.replications_failed) as f64 / total as f64
    }
}

impl Default for StorageReplicator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct StorageValidator {
    pub validations: u64,
    pub validation_failures: u64,
    pub checksum_errors: u64,
}

impl StorageValidator {
    pub fn new() -> Self {
        Self {
            validations: 0,
            validation_failures: 0,
            checksum_errors: 0,
        }
    }

    pub fn record_validation(&mut self, passed: bool) {
        self.validations += 1;
        if !passed {
            self.validation_failures += 1;
        }
    }

    pub fn record_checksum_error(&mut self) {
        self.checksum_errors += 1;
        self.validation_failures += 1;
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validations == 0 {
            return 1.0;
        }
        (self.validations - self.validation_failures) as f64 / self.validations as f64
    }
}

impl Default for StorageValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE LIFECYCLE STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageLifecycleState {
    Uninitialized,
    Initializing,
    Active,
    Maintenance,
    Stopping,
    Stopped,
    Failed,
}

impl StorageLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageLifecycleState::Uninitialized => "uninitialized",
            StorageLifecycleState::Initializing => "initializing",
            StorageLifecycleState::Active => "active",
            StorageLifecycleState::Maintenance => "maintenance",
            StorageLifecycleState::Stopping => "stopping",
            StorageLifecycleState::Stopped => "stopped",
            StorageLifecycleState::Failed => "failed",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, StorageLifecycleState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StorageLifecycleState::Stopped | StorageLifecycleState::Failed
        )
    }

    pub fn can_accept_writes(&self) -> bool {
        matches!(self, StorageLifecycleState::Active)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORE CONFIG EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl StoreConfig {
    pub fn required_acks_for_dataset(&self, _dataset: DatasetId) -> u32 {
        self.default_consistency_level.required_acks(self.default_replication_factor)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OBJECT METADATA EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ObjectMetadata {
    pub fn is_expired(&self, now_ms: u64, ttl_ms: u64) -> bool {
        now_ms.saturating_sub(self.modified_at_ms) > ttl_ms
    }

    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.modified_at_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLICA INFO EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ReplicaInfo {
    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.created_at_ms)
    }

    pub fn replication_rate(&self) -> f64 {
        if self.age_ms(0) == 0 {
            return 0.0;
        }
        self.replication_attempts as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE BACKUP MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageBackupManager {
    pub backups: std::cell::RefCell<std::collections::HashMap<u64, StorageSnapshot>>,
    pub next_backup_id: std::cell::Cell<u64>,
    pub total_backups: u64,
    pub total_restores: u64,
}

impl StorageBackupManager {
    pub fn new() -> Self {
        Self {
            backups: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_backup_id: std::cell::Cell::new(0),
            total_backups: 0,
            total_restores: 0,
        }
    }

    pub fn create_backup(
        &self,
        object_count: u64,
        byte_count: u64,
        dataset: DatasetId,
    ) -> u64 {
        let id = self.next_backup_id.get();
        self.next_backup_id.set(id + 1);
        let snapshot = StorageSnapshot::new(id, object_count, byte_count)
            .with_dataset(dataset);
        self.backups.borrow_mut().insert(id, snapshot);
        self.total_backups += 1;
        id
    }

    pub fn restore_backup(&self, backup_id: u64) -> bool {
        if self.backups.borrow().contains_key(&backup_id) {
            self.total_restores += 1;
            true
        } else {
            false
        }
    }

    pub fn delete_backup(&self, backup_id: u64) -> bool {
        self.backups.borrow_mut().remove(&backup_id).is_some()
    }

    pub fn backup_count(&self) -> usize {
        self.backups.borrow().len()
    }

    pub fn total_backups(&self) -> u64 {
        self.total_backups
    }

    pub fn total_restores(&self) -> u64 {
        self.total_restores
    }
}

impl Default for StorageBackupManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE MIGRATION MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMigrationState {
    Idle,
    Migrating,
    Completed,
    Failed,
}

impl StorageMigrationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageMigrationState::Idle => "idle",
            StorageMigrationState::Migrating => "migrating",
            StorageMigrationState::Completed => "completed",
            StorageMigrationState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StorageMigrationState::Completed | StorageMigrationState::Failed
        )
    }
}

pub struct StorageMigrationManager {
    pub state: std::cell::Cell<StorageMigrationState>,
    pub source_region: RegionId,
    pub target_region: RegionId,
    pub objects_migrated: u64,
    pub objects_total: u64,
    pub bytes_migrated: u64,
    pub bytes_total: u64,
}

impl StorageMigrationManager {
    pub fn new(source: RegionId, target: RegionId) -> Self {
        Self {
            state: std::cell::Cell::new(StorageMigrationState::Idle),
            source_region: source,
            target_region: target,
            objects_migrated: 0,
            objects_total: 0,
            bytes_migrated: 0,
            bytes_total: 0,
        }
    }

    pub fn start_migration(&self, objects: u64, bytes: u64) {
        self.state.set(StorageMigrationState::Migrating);
        self.objects_total = objects;
        self.bytes_total = bytes;
    }

    pub fn report_progress(&self, objects: u64, bytes: u64) {
        self.objects_migrated = objects;
        self.bytes_migrated = bytes;
        if self.objects_migrated >= self.objects_total {
            self.state.set(StorageMigrationState::Completed);
        }
    }

    pub fn fail_migration(&self) {
        self.state.set(StorageMigrationState::Failed);
    }

    pub fn progress_percent(&self) -> f64 {
        if self.objects_total == 0 {
            return 0.0;
        }
        self.objects_migrated as f64 / self.objects_total as f64 * 100.0
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.state.get(), StorageMigrationState::Completed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE GARBAGE COLLECTOR
///////////////////////////////////////////////////////////////////////////////

pub struct StorageGarbageCollector {
    pub collections: u64,
    pub objects_collected: u64,
    pub bytes_collected: u64,
    pub last_collection_ms: u64,
}

impl StorageGarbageCollector {
    pub fn new() -> Self {
        Self {
            collections: 0,
            objects_collected: 0,
            bytes_collected: 0,
            last_collection_ms: 0,
        }
    }

    pub fn record_collection(
        &mut self,
        objects: u64,
        bytes: u64,
        timestamp_ms: u64,
    ) {
        self.collections += 1;
        self.objects_collected += objects;
        self.bytes_collected += bytes;
        self.last_collection_ms = timestamp_ms;
    }

    pub fn collection_rate(&self) -> f64 {
        if self.collections == 0 {
            return 0.0;
        }
        self.objects_collected as f64 / self.collections as f64
    }

    pub fn average_bytes_per_collection(&self) -> f64 {
        if self.collections == 0 {
            return 0.0;
        }
        self.bytes_collected as f64 / self.collections as f64
    }
}

impl Default for StorageGarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE WARMUP MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageWarmupManager {
    pub warmed_objects: u64,
    pub warm_target_objects: u64,
    pub warm_started_ms: u64,
    pub warm_completed_ms: u64,
}

impl StorageWarmupManager {
    pub fn new() -> Self {
        Self {
            warmed_objects: 0,
            warm_target_objects: 0,
            warm_started_ms: 0,
            warm_completed_ms: 0,
        }
    }

    pub fn start_warmup(&mut self, target: u64, started_ms: u64) {
        self.warm_target_objects = target;
        self.warm_started_ms = started_ms;
        self.warmed_objects = 0;
    }

    pub fn record_warmed(&mut self, count: u64) {
        self.warmed_objects += count;
        if self.warmed_objects >= self.warm_target_objects {
            self.warm_completed_ms = self.warm_started_ms + 1000;
        }
    }

    pub fn warmup_percent(&self) -> f64 {
        if self.warm_target_objects == 0 {
            return 0.0;
        }
        self.warmed_objects as f64 / self.warm_target_objects as f64 * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.warmed_objects >= self.warm_target_objects
    }
}

impl Default for StorageWarmupManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE THROTTLE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct StorageThrottleManager {
    pub max_ops_per_sec: u64,
    pub current_ops: std::cell::Cell<u64>,
    pub window_start_ms: std::cell::Cell<u64>,
    pub throttled_ops: u64,
}

impl StorageThrottleManager {
    pub fn new(max_ops_per_sec: u64) -> Self {
        Self {
            max_ops_per_sec,
            current_ops: std::cell::Cell::new(0),
            window_start_ms: std::cell::Cell::new(0),
            throttled_ops: 0,
        }
    }

    pub fn try_acquire(&self, now_ms: u64) -> bool {
        let window_start = self.window_start_ms.get();
        if now_ms.saturating_sub(window_start) >= 1000 {
            self.current_ops.set(0);
            self.window_start_ms.set(now_ms);
        }
        let current = self.current_ops.get();
        if current >= self.max_ops_per_sec {
            self.throttled_ops += 1;
            false
        } else {
            self.current_ops.set(current + 1);
            true
        }
    }

    pub fn throttled_count(&self) -> u64 {
        self.throttled_ops
    }

    pub fn current_ops(&self) -> u64 {
        self.current_ops.get()
    }
}

impl Default for StorageThrottleManager {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE METRICS COLLECTOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct StorageMetrics {
    pub total_puts: u64,
    pub total_gets: u64,
    pub total_deletes: u64,
    pub total_replications: u64,
    pub total_backups: u64,
    pub total_restores: u64,
    pub total_compactions: u64,
    pub total_garbage_collections: u64,
    pub total_writes: u64,
    pub total_reads: u64,
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub error_count: u64,
}

impl StorageMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_operations(&self) -> u64 {
        self.total_puts + self.total_gets + self.total_deletes
    }

    pub fn total_data_operations(&self) -> u64 {
        self.total_writes + self.total_reads
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.total_operations() + self.error_count;
        if total == 0 {
            return 0.0;
        }
        self.error_count as f64 / total as f64
    }

    pub fn read_write_ratio(&self) -> f64 {
        if self.total_writes == 0 {
            return 0.0;
        }
        self.total_reads as f64 / self.total_writes as f64
    }

    pub fn replication_ratio(&self) -> f64 {
        if self.total_puts == 0 {
            return 0.0;
        }
        self.total_replications as f64 / self.total_puts as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE HEALTH CHECKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageHealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

impl StorageHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageHealthStatus::Healthy => "healthy",
            StorageHealthStatus::Degraded => "degraded",
            StorageHealthStatus::Unhealthy => "unhealthy",
            StorageHealthStatus::Critical => "critical",
        }
    }

    pub fn is_acceptable(&self) -> bool {
        matches!(
            self,
            StorageHealthStatus::Healthy | StorageHealthStatus::Degraded
        )
    }
}

pub struct StorageHealthChecker {
    pub status: std::cell::Cell<StorageHealthStatus>,
    pub checks_performed: u64,
    pub failures: u64,
    pub last_check_ms: u64,
}

impl StorageHealthChecker {
    pub fn new() -> Self {
        Self {
            status: std::cell::Cell::new(StorageHealthStatus::Healthy),
            checks_performed: 0,
            failures: 0,
            last_check_ms: 0,
        }
    }

    pub fn record_check(&mut self, passed: bool, now_ms: u64) {
        self.checks_performed += 1;
        self.last_check_ms = now_ms;
        if !passed {
            self.failures += 1;
        }
        let failure_rate = if self.checks_performed > 0 {
            self.failures as f64 / self.checks_performed as f64
        } else {
            0.0
        };
        self.status.set(if failure_rate > 0.5 {
            StorageHealthStatus::Critical
        } else if failure_rate > 0.2 {
            StorageHealthStatus::Unhealthy
        } else if failure_rate > 0.05 {
            StorageHealthStatus::Degraded
        } else {
            StorageHealthStatus::Healthy
        });
    }

    pub fn current_status(&self) -> StorageHealthStatus {
        self.status.get()
    }

    pub fn failure_rate(&self) -> f64 {
        if self.checks_performed == 0 {
            return 0.0;
        }
        self.failures as f64 / self.checks_performed as f64
    }
}

impl Default for StorageHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE EVENT LOGGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct StorageEvent {
    pub event_id: u64,
    pub event_type: StorageEventType,
    pub object_id: Option<ObjectID>,
    pub dataset_id: Option<DatasetId>,
    pub timestamp_ms: u64,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageEventType {
    Put,
    Get,
    Delete,
    Replicate,
    Backup,
    Restore,
    Compact,
    Error,
}

impl StorageEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageEventType::Put => "put",
            StorageEventType::Get => "get",
            StorageEventType::Delete => "delete",
            StorageEventType::Replicate => "replicate",
            StorageEventType::Backup => "backup",
            StorageEventType::Restore => "restore",
            StorageEventType::Compact => "compact",
            StorageEventType::Error => "error",
        }
    }
}

pub struct StorageEventLogger {
    pub events: std::cell::RefCell<Vec<StorageEvent>>,
    pub max_events: usize,
    pub next_event_id: std::cell::Cell<u64>,
}

impl StorageEventLogger {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: std::cell::RefCell::new(Vec::new()),
            max_events,
            next_event_id: std::cell::Cell::new(0),
        }
    }

    pub fn log(
        &self,
        event_type: StorageEventType,
        object_id: Option<ObjectID>,
        dataset_id: Option<DatasetId>,
        timestamp_ms: u64,
        details: &str,
    ) {
        let id = self.next_event_id.get();
        self.next_event_id.set(id + 1);
        let event = StorageEvent {
            event_id: id,
            event_type,
            object_id,
            dataset_id,
            timestamp_ms,
            details: details.to_string(),
        };
        let mut events = self.events.borrow_mut();
        if events.len() >= self.max_events {
            events.remove(0);
        }
        events.push(event);
    }

    pub fn event_count(&self) -> usize {
        self.events.borrow().len()
    }

    pub fn events_by_type(&self, event_type: StorageEventType) -> usize {
        self.events
            .borrow()
            .iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }
}

impl Default for StorageEventLogger {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE SHARD INDEX
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardId(pub u64);

impl ShardId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

pub struct StorageShardIndex {
    pub shards: std::cell::RefCell<std::collections::BTreeMap<ShardId, Vec<ObjectID>>>,
    pub next_shard: std::cell::Cell<ShardId>,
}

impl StorageShardIndex {
    pub fn new() -> Self {
        Self {
            shards: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            next_shard: std::cell::Cell::new(ShardId::new(1)),
        }
    }

    pub fn assign_shard(&self, object_id: ObjectID) -> ShardId {
        let shard = self.next_shard.get();
        self.next_shard.set(ShardId::new(shard.0 + 1));
        self.shards.borrow_mut().entry(shard).or_default().push(object_id);
        shard
    }

    pub fn get_shard_objects(&self, shard: ShardId) -> Vec<ObjectID> {
        self.shards.borrow().get(&shard).cloned().unwrap_or_default()
    }

    pub fn shard_count(&self) -> usize {
        self.shards.borrow().len()
    }

    pub fn total_objects(&self) -> usize {
        self.shards
            .borrow()
            .values()
            .map(|v| v.len())
            .sum()
    }
}

impl Default for StorageShardIndex {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// STORAGE TIER POLICY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
    Archived,
}

impl StorageTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageTier::Hot => "hot",
            StorageTier::Warm => "warm",
            StorageTier::Cold => "cold",
            StorageTier::Archived => "archived",
        }
    }

    pub fn access_cost_multiplier(&self) -> f64 {
        match self {
            StorageTier::Hot => 1.0,
            StorageTier::Warm => 2.0,
            StorageTier::Cold => 5.0,
            StorageTier::Archived => 10.0,
        }
    }

    pub fn is_hot(&self) -> bool {
        matches!(self, StorageTier::Hot)
    }

    pub fn is_archived(&self) -> bool {
        matches!(self, StorageTier::Archived)
    }
}

pub struct StorageTierPolicy {
    pub default_tier: StorageTier,
    pub hot_threshold_bytes: u64,
    pub warm_threshold_bytes: u64,
    pub cold_threshold_bytes: u64,
}

impl StorageTierPolicy {
    pub fn new() -> Self {
        Self {
            default_tier: StorageTier::Hot,
            hot_threshold_bytes: 1048576,
            warm_threshold_bytes: 1073741824,
            cold_threshold_bytes: 1099511627776,
        }
    }

    pub fn tier_for_size(&self, size_bytes: u64) -> StorageTier {
        if size_bytes <= self.hot_threshold_bytes {
            StorageTier::Hot
        } else if size_bytes <= self.warm_threshold_bytes {
            StorageTier::Warm
        } else if size_bytes <= self.cold_threshold_bytes {
            StorageTier::Cold
        } else {
            StorageTier::Archived
        }
    }

    pub fn cost_multiplier_for_size(&self, size_bytes: u64) -> f64 {
        self.tier_for_size(size_bytes).access_cost_multiplier()
    }
}

impl Default for StorageTierPolicy {
    fn default() -> Self {
        Self::new()
    }
}

pub fn store_validate_object_id(object_id: ObjectID) -> bool {
    !object_id.as_bytes().is_empty()
}

pub fn store_validate_dataset_id(dataset: DatasetId) -> bool {
    !dataset.as_bytes().is_empty()
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation of Store for testing the trait contract.
    struct TestStore {
        name: &'static str,
        objects: std::cell::RefCell<
            std::collections::HashMap<(DatasetId, ObjectID), Vec<u8>>,
        >,
        puts: std::cell::Cell<u64>,
        gets: std::cell::Cell<u64>,
        deletes: std::cell::Cell<u64>,
    }

    impl TestStore {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                objects: std::cell::RefCell::new(std::collections::HashMap::new()),
                puts: std::cell::Cell::new(0),
                gets: std::cell::Cell::new(0),
                deletes: std::cell::Cell::new(0),
            }
        }
    }

    impl Store for TestStore {
        fn put(
            &self,
            dataset: DatasetId,
            object_id: ObjectID,
            bytes: Vec<u8>,
        ) -> Result<OpRef, StoreError> {
            let key = (dataset, object_id);
            if self.objects.borrow().contains_key(&key) {
                return Err(StoreError::ObjectAlreadyExists {
                    dataset,
                    object_id,
                });
            }
            self.objects.borrow_mut().insert(key, bytes.clone());
            self.puts.set(self.puts.get() + 1);
            Ok(OpRef::new(object_id, 0))
        }

        fn get(
            &self,
            dataset: DatasetId,
            object_id: ObjectID,
        ) -> Result<Option<Vec<u8>>, StoreError> {
            self.gets.set(self.gets.get() + 1);
            Ok(self.objects.borrow().get(&(dataset, object_id)).cloned())
        }

        fn delete(
            &self,
            dataset: DatasetId,
            object_id: ObjectID,
        ) -> Result<bool, StoreError> {
            self.deletes.set(self.deletes.get() + 1);
            Ok(self.objects.borrow_mut().remove(&(dataset, object_id)).is_some())
        }

        fn exists(
            &self,
            dataset: DatasetId,
            object_id: ObjectID,
        ) -> Result<bool, StoreError> {
            Ok(self.objects.borrow().contains_key(&(dataset, object_id)))
        }

        fn object_size(
            &self,
            dataset: DatasetId,
            object_id: ObjectID,
        ) -> Result<Option<u64>, StoreError> {
            Ok(
                self.objects
                    .borrow()
                    .get(&(dataset, object_id))
                    .map(|b| b.len() as u64),
            )
        }

        fn list(
            &self,
            dataset: DatasetId,
            prefix: Option<&[u8]>,
        ) -> Result<Vec<ObjectID>, StoreError> {
            let mut result = Vec::new();
            for ((d, id), _) in self.objects.borrow().iter() {
                if *d == dataset {
                    let bytes = id.as_bytes();
                    match prefix {
                        Some(p) if bytes.starts_with(p) => result.push(*id),
                        None => result.push(*id),
                        _ => {}
                    }
                }
            }
            Ok(result)
        }

        fn store_name(&self) -> &'static str {
            self.name
        }

        fn object_count(&self) -> u64 {
            self.objects.borrow().len() as u64
        }

        fn is_healthy(&self) -> bool {
            true
        }
    }

    #[test]
    fn store_put_get_delete() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1, 2, 3]);
        let oid = ObjectID::new([4, 5, 6]);
        let data = vec![10, 20, 30, 40, 50];

        let opref = store.put(ds, oid, data.clone()).unwrap();
        assert_eq!(opref.object_id(), oid);

        let got = store.get(ds, oid).unwrap();
        assert_eq!(got, Some(data));

        let deleted = store.delete(ds, oid).unwrap();
        assert!(deleted);

        let gone = store.get(ds, oid).unwrap();
        assert_eq!(gone, None);
    }

    #[test]
    fn store_put_twice_errors() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        let oid = ObjectID::new([2]);
        store.put(ds, oid, vec![1]).unwrap();
        let err = store.put(ds, oid, vec![2]).unwrap_err();
        assert!(matches!(err, StoreError::ObjectAlreadyExists { .. }));
    }

    #[test]
    fn store_get_missing() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        let oid = ObjectID::new([2]);
        let got = store.get(ds, oid).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn store_exists() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        let oid = ObjectID::new([2]);
        assert!(!store.exists(ds, oid).unwrap());
        store.put(ds, oid, vec![1]).unwrap();
        assert!(store.exists(ds, oid).unwrap());
    }

    #[test]
    fn store_object_size() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        let oid = ObjectID::new([2]);
        let data = vec![1, 2, 3, 4, 5];
        store.put(ds, oid, data.clone()).unwrap();
        assert_eq!(store.object_size(ds, oid).unwrap(), Some(5));
        assert_eq!(store.object_size(ds, ObjectID::new([9])).unwrap(), None);
    }

    #[test]
    fn store_list() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        for i in 0..5 {
            store
                .put(ds, ObjectID::new([i, 0]), vec![i])
                .unwrap();
        }
        let ids = store.list(ds, None).unwrap();
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn store_list_with_prefix() {
        let store = TestStore::new("test");
        let ds = DatasetId::new([1]);
        store.put(ds, ObjectID::new([1, 0]), vec![1]).unwrap();
        store.put(ds, ObjectID::new([1, 1]), vec![2]).unwrap();
        store.put(ds, ObjectID::new([2, 0]), vec![3]).unwrap();
        let ids = store.list(ds, Some(&[1])).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn store_replication_factor_default() {
        let store = TestStore::new("test");
        assert_eq!(store.replication_factor(DatasetId::new([1])), 3);
    }

    #[test]
    fn store_consistency_level_default() {
        let store = TestStore::new("test");
        assert_eq!(
            store.consistency_level(DatasetId::new([1])),
            ConsistencyLevel::Eventual
        );
    }

    #[test]
    fn store_flush_and_compact() {
        let store = TestStore::new("test");
        assert!(store.flush().is_ok());
        assert!(store.compact().is_ok());
    }

    #[test]
    fn store_object_count_and_bytes() {
        let store = TestStore::new("test");
        assert_eq!(store.object_count(), 0);
        store.put(DatasetId::new([1]), ObjectID::new([1]), vec![1]).unwrap();
        store.put(DatasetId::new([1]), ObjectID::new([2]), vec![2]).unwrap();
        assert_eq!(store.object_count(), 2);
    }

    #[test]
    fn store_healthy() {
        let store = TestStore::new("test");
        assert!(store.is_healthy());
    }

    #[test]
    fn store_error_category() {
        assert_eq!(
            StoreError::ObjectNotFound {
                dataset: DatasetId::new([1]),
                object_id: ObjectID::new([2]),
            }
            .category(),
            "object_not_found"
        );
        assert_eq!(
            StoreError::ObjectAlreadyExists {
                dataset: DatasetId::new([1]),
                object_id: ObjectID::new([2]),
            }
            .category(),
            "object_already_exists"
        );
        assert_eq!(
            StoreError::DatasetNotFound {
                dataset: DatasetId::new([1])
            }
            .category(),
            "dataset_not_found"
        );
        assert_eq!(
            StoreError::WriteFailed {
                dataset: DatasetId::new([1]),
                object_id: ObjectID::new([2]),
                reason: "x".to_string()
            }
            .category(),
            "write_failed"
        );
        assert_eq!(
            StoreError::ReplicationFailed {
                object_id: ObjectID::new([1]),
                target: Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap(),
                reason: "x".to_string()
            }
            .category(),
            "replication_failed"
        );
        assert_eq!(
            StoreError::ObjectTooLarge { size: 100, max: 50 }.category(),
            "object_too_large"
        );
        assert_eq!(StoreError::NotInitialized.category(), "not_initialized");
        assert_eq!(
            StoreError::Internal { message: "x".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn store_error_retryable_and_fatal() {
        assert!(StoreError::WriteFailed {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
            reason: "x".to_string()
        }
        .is_retryable());
        assert!(StoreError::Internal { message: "x".to_string() }.is_retryable());
        assert!(!StoreError::ObjectNotFound {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
        }
        .is_retryable());
        assert!(StoreError::NotInitialized.is_fatal());
    }

    #[test]
    fn store_error_plain_message() {
        let msg = StoreError::ObjectNotFound {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
        }
        .plain_message();
        assert!(msg.contains("object not found"));
        assert!(msg.contains("dataset=DatasetId"));
    }

    #[test]
    fn consistency_level_acks() {
        assert_eq!(ConsistencyLevel::Any.required_acks(3), 1);
        assert_eq!(ConsistencyLevel::One.required_acks(3), 1);
        assert_eq!(ConsistencyLevel::Quorum.required_acks(3), 2);
        assert_eq!(ConsistencyLevel::All.required_acks(3), 3);
        assert_eq!(ConsistencyLevel::Eventual.required_acks(3), 1);
        assert_eq!(ConsistencyLevel::Strong.required_acks(3), 3);
        assert!(ConsistencyLevel::Strong.is_strong());
        assert!(ConsistencyLevel::All.is_strong());
        assert!(!ConsistencyLevel::Eventual.is_strong());
        assert!(ConsistencyLevel::Eventual.is_eventual());
        assert!(ConsistencyLevel::Any.is_eventual());
    }

    #[test]
    fn consistency_level_as_str() {
        assert_eq!(ConsistencyLevel::Any.as_str(), "any");
        assert_eq!(ConsistencyLevel::One.as_str(), "one");
        assert_eq!(ConsistencyLevel::Quorum.as_str(), "quorum");
        assert_eq!(ConsistencyLevel::All.as_str(), "all");
        assert_eq!(ConsistencyLevel::Eventual.as_str(), "eventual");
        assert_eq!(ConsistencyLevel::Strong.as_str(), "strong");
    }

    #[test]
    fn object_metadata_new() {
        let oid = ObjectID::new([1]);
        let ds = DatasetId::new([2]);
        let meta = ObjectMetadata::new(oid, ds, 100, RegionId::new([3]), HUnitId::new([4]));
        assert_eq!(meta.object_id, oid);
        assert_eq!(meta.dataset, ds);
        assert_eq!(meta.size_bytes, 100);
        assert_eq!(meta.version, 1);
        assert_eq!(meta.replication_factor, 3);
    }

    #[test]
    fn object_metadata_with_options() {
        let oid = ObjectID::new([1]);
        let ds = DatasetId::new([2]);
        let meta = ObjectMetadata::new(oid, ds, 100, RegionId::new([3]), HUnitId::new([4]))
            .with_replication_factor(5)
            .with_consistency_level(ConsistencyLevel::Quorum);
        assert_eq!(meta.replication_factor, 5);
        assert_eq!(meta.consistency_level, ConsistencyLevel::Quorum);
        assert_eq!(meta.required_acks(), ConsistencyLevel::Quorum.required_acks(5));
    }

    #[test]
    fn object_metadata_bump_version() {
        let oid = ObjectID::new([1]);
        let ds = DatasetId::new([2]);
        let mut meta = ObjectMetadata::new(oid, ds, 100, RegionId::new([3]), HUnitId::new([4]));
        meta.bump_version();
        assert_eq!(meta.version, 2);
        meta.bump_version();
        assert_eq!(meta.version, 3);
    }

    #[test]
    fn replica_state_variants() {
        assert_eq!(ReplicaState::Pending.as_str(), "pending");
        assert_eq!(ReplicaState::Replicating.as_str(), "replicating");
        assert_eq!(ReplicaState::Replicated.as_str(), "replicated");
        assert_eq!(ReplicaState::Failed.as_str(), "failed");
        assert_eq!(ReplicaState::Deleted.as_str(), "deleted");
        assert!(ReplicaState::Replicated.is_terminal());
        assert!(ReplicaState::Deleted.is_terminal());
        assert!(!ReplicaState::Pending.is_terminal());
        assert!(ReplicaState::Replicated.is_active());
        assert!(ReplicaState::Replicating.is_active());
        assert!(!ReplicaState::Pending.is_active());
    }

    #[test]
    fn replica_info_lifecycle() {
        let oid = ObjectID::new([1]);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let mut replica = ReplicaInfo::new(oid, ep.clone());
        assert_eq!(replica.state, ReplicaState::Pending);
        assert_eq!(replica.replication_attempts, 0);

        replica.mark_replicating(100);
        assert_eq!(replica.state, ReplicaState::Replicating);
        assert_eq!(replica.replication_attempts, 1);
        assert_eq!(replica.last_heartbeat_ms, 100);

        replica.mark_replicated(200);
        assert_eq!(replica.state, ReplicaState::Replicated);
        assert_eq!(replica.last_heartbeat_ms, 200);
        assert!(replica.is_terminal());

        replica.heartbeat(300);
        assert_eq!(replica.last_heartbeat_ms, 300);
        assert!(!replica.is_stale(350, 100));
        assert!(replica.is_stale(500, 100));
    }

    #[test]
    fn store_stats_default_and_totals() {
        let stats = StoreStats::default();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.total_failures(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert!((stats.replication_success_rate() - 1.0).abs() < f64::EPSILON);

        let mut stats = stats;
        stats.puts = 100;
        stats.gets = 200;
        stats.deletes = 50;
        stats.puts_failed = 5;
        stats.gets_failed = 3;
        stats.deletes_failed = 2;
        assert_eq!(stats.total_operations(), 350);
        assert_eq!(stats.total_failures(), 10);
        assert!((stats.success_rate() - (340.0 / 350.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn store_stats_replication() {
        let mut stats = StoreStats::default();
        stats.replications = 50;
        stats.replications_failed = 10;
        assert!((stats.replication_success_rate() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn store_config_default_and_validate() {
        let config = StoreConfig::default();
        assert_eq!(config.default_replication_factor, 3);
        assert_eq!(config.default_consistency_level, ConsistencyLevel::Eventual);
        assert_eq!(config.max_object_size, 1048576);
        assert!(config.enable_replication);
        assert!(!config.enable_compaction);
        assert!(config.validate().is_ok());

        let mut bad = StoreConfig::default();
        bad.default_replication_factor = 0;
        assert!(bad.validate().is_err());
        let mut bad2 = StoreConfig::default();
        bad2.max_object_size = 0;
        assert!(bad2.validate().is_err());
    }

    #[test]
    fn store_config_builder() {
        let config = StoreConfig::new()
            .with_replication_factor(5)
            .with_consistency_level(ConsistencyLevel::Strong)
            .with_max_object_size(2097152)
            .with_replication(false)
            .with_compaction(true);
        assert_eq!(config.default_replication_factor, 5);
        assert_eq!(config.default_consistency_level, ConsistencyLevel::Strong);
        assert_eq!(config.max_object_size, 2097152);
        assert!(!config.enable_replication);
        assert!(config.enable_compaction);
    }

    #[test]
    fn object_ref_tracker() {
        let mut tracker = ObjectRefTracker::new();
        let oid = ObjectID::new([1]);
        let ds = DatasetId::new([2]);
        tracker.add_ref(oid, ds);
        assert_eq!(tracker.ref_count(oid), 1);
        assert!(tracker.has_refs(oid));
        tracker.add_ref(oid, ds);
        assert_eq!(tracker.ref_count(oid), 2);
        tracker.remove_ref(oid);
        assert_eq!(tracker.ref_count(oid), 1);
        tracker.remove_ref(oid);
        assert_eq!(tracker.ref_count(oid), 0);
        assert!(!tracker.has_refs(oid));
        assert_eq!(tracker.total_tracked(), 0);
    }

    #[test]
    fn version_tracker() {
        let mut tracker = VersionTracker::new();
        let oid = ObjectID::new([1]);
        assert_eq!(tracker.current_version(oid), 0);
        assert_eq!(tracker.bump_version(oid), 1);
        assert_eq!(tracker.current_version(oid), 1);
        assert_eq!(tracker.bump_version(oid), 2);
        assert!(!tracker.is_conflict(oid, 2));
        assert!(tracker.is_conflict(oid, 1));
        assert_eq!(tracker.total_tracked(), 1);
    }

    #[test]
    fn store_name_and_max_object_size() {
        let store = TestStore::new("teststore");
        assert_eq!(store.store_name(), "teststore");
        assert_eq!(store.max_object_size(), 1048576);
        assert_eq!(store.pending_replications(), 0);
    }

    // === StorageBackend tests ===
    #[test]
    fn storage_backend_variants() {
        assert_eq!(StorageBackend::Memory.as_str(), "memory");
        assert_eq!(StorageBackend::Disk.as_str(), "disk");
        assert_eq!(StorageBackend::Hybrid.as_str(), "hybrid");
        assert_eq!(StorageBackend::Remote.as_str(), "remote");
        assert!(StorageBackend::Memory.is_local());
        assert!(StorageBackend::Disk.is_local());
        assert!(!StorageBackend::Remote.is_local());
        assert!(StorageBackend::Disk.is_durable());
        assert!(StorageBackend::Hybrid.is_durable());
        assert!(!StorageBackend::Memory.is_durable());
        assert!(!StorageBackend::Remote.is_durable());
    }

    // === StorageDevice tests ===
    #[test]
    fn storage_device_new_and_capacity() {
        let device = StorageDevice::new(1, StorageBackend::Disk, 1000000);
        assert_eq!(device.device_id, 1);
        assert_eq!(device.capacity_bytes, 1000000);
        assert_eq!(device.used_bytes, 0);
        assert_eq!(device.available_bytes(), 1000000);
        assert!(!device.read_only);
        assert!((device.utilization() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_device_write_and_free() {
        let mut device = StorageDevice::new(2, StorageBackend::Memory, 1000);
        assert!(device.write(500).is_ok());
        assert_eq!(device.used_bytes, 500);
        assert_eq!(device.available_bytes(), 500);
        device.free(200);
        assert_eq!(device.used_bytes, 300);
    }

    #[test]
    fn storage_device_read_only() {
        let mut device = StorageDevice::new(3, StorageBackend::Disk, 1000);
        device.read_only = true;
        let err = device.write(100).unwrap_err();
        assert!(matches!(err, StoreError::Internal { .. }));
    }

    #[test]
    fn storage_device_insufficient_space() {
        let mut device = StorageDevice::new(4, StorageBackend::Disk, 100);
        assert!(device.write(200).is_err());
    }

    // === StorageQuota tests ===
    #[test]
    fn storage_quota_new_and_allocate() {
        let mut quota = StorageQuota::new(100, 10000);
        assert!(quota.can_allocate(10, 5000));
        assert!(quota.allocate(10, 5000).is_ok());
        assert_eq!(quota.current_objects, 10);
        assert_eq!(quota.current_bytes, 5000);
    }

    #[test]
    fn storage_quota_exceeded() {
        let mut quota = StorageQuota::new(5, 1000);
        assert!(quota.allocate(5, 500).is_ok());
        assert!(!quota.can_allocate(1, 1));
        assert!(quota.allocate(1, 1).is_err());
    }

    #[test]
    fn storage_quota_release() {
        let mut quota = StorageQuota::new(100, 10000);
        quota.allocate(50, 5000).unwrap();
        quota.release(20, 2000);
        assert_eq!(quota.current_objects, 30);
        assert_eq!(quota.current_bytes, 3000);
    }

    #[test]
    fn storage_quota_utilization() {
        let mut quota = StorageQuota::new(100, 10000);
        quota.allocate(50, 5000).unwrap();
        assert!((quota.object_utilization() - 0.5).abs() < f64::EPSILON);
        assert!((quota.byte_utilization() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_quota_default() {
        let quota = StorageQuota::default();
        assert_eq!(quota.max_objects, 1000000);
        assert_eq!(quota.max_bytes, 1073741824);
    }

    // === StorageCache tests ===
    #[test]
    fn storage_cache_put_and_get() {
        let cache = StorageCache::new(10);
        let oid = ObjectID::new([1]);
        cache.put(oid, vec![1, 2, 3]);
        assert_eq!(cache.get(oid), Some(vec![1, 2, 3]));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn storage_cache_miss() {
        let cache = StorageCache::new(10);
        let oid = ObjectID::new([999]);
        assert_eq!(cache.get(oid), None);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 0);
    }

    #[test]
    fn storage_cache_hit_rate() {
        let cache = StorageCache::new(10);
        let oid = ObjectID::new([1]);
        cache.put(oid, vec![1]);
        cache.get(oid);
        cache.get(oid);
        cache.get(ObjectID::new([2]));
        assert!((cache.hit_rate() - (2.0 / 3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_cache_eviction() {
        let cache = StorageCache::new(2);
        cache.put(ObjectID::new([1]), vec![1]);
        cache.put(ObjectID::new([2]), vec![2]);
        cache.put(ObjectID::new([3]), vec![3]);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.evictions, 1);
    }

    #[test]
    fn storage_cache_remove() {
        let cache = StorageCache::new(10);
        let oid = ObjectID::new([1]);
        cache.put(oid, vec![1]);
        assert!(cache.remove(oid));
        assert!(!cache.remove(oid));
    }

    // === StorageIndex tests ===
    #[test]
    fn storage_index_insert_and_get() {
        let index = StorageIndex::new();
        let oid = ObjectID::new([1]);
        index.insert(oid, 100);
        assert_eq!(index.get(oid), Some(100));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn storage_index_remove_and_contains() {
        let index = StorageIndex::new();
        let oid = ObjectID::new([1]);
        index.insert(oid, 100);
        assert!(index.contains(oid));
        assert!(index.remove(oid));
        assert!(!index.contains(oid));
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn storage_index_default_empty() {
        let index = StorageIndex::default();
        assert!(index.is_empty());
    }

    // === StorageSnapshot tests ===
    #[test]
    fn storage_snapshot_new() {
        let snapshot = StorageSnapshot::new(1, 100, 5000);
        assert_eq!(snapshot.snapshot_id, 1);
        assert_eq!(snapshot.object_count, 100);
        assert_eq!(snapshot.byte_count, 5000);
        assert!(snapshot.dataset_ids.is_empty());
    }

    #[test]
    fn storage_snapshot_with_dataset() {
        let snapshot = StorageSnapshot::new(2, 50, 2500)
            .with_dataset(DatasetId::new([1]))
            .with_dataset(DatasetId::new([2]));
        assert_eq!(snapshot.dataset_ids.len(), 2);
        assert!(snapshot.contains_dataset(DatasetId::new([1])));
        assert!(!snapshot.contains_dataset(DatasetId::new([99])));
    }

    // === StorageCompactionManager tests ===
    #[test]
    fn storage_compaction_manager_should_and_record() {
        let mut cm = StorageCompactionManager::new(1000);
        assert!(!cm.should_compact(0));
        assert!(!cm.should_compact(500));
        cm.record_compaction(1000, 5000);
        assert!(!cm.should_compact(1500));
        assert!(cm.should_compact(2000));
        assert_eq!(cm.total_compactions, 1);
        assert_eq!(cm.bytes_reclaimed, 5000);
    }

    #[test]
    fn storage_compaction_manager_rate() {
        let mut cm = StorageCompactionManager::new(1000);
        cm.record_compaction(0, 1000);
        cm.record_compaction(1000, 3000);
        assert!((cm.compaction_rate() - 2000.0).abs() < f64::EPSILON);
    }

    // === StorageWriter tests ===
    #[test]
    fn storage_writer_record() {
        let mut writer = StorageWriter::new();
        writer.record_write(100, 10);
        writer.record_write(200, 20);
        assert_eq!(writer.writes, 2);
        assert_eq!(writer.bytes_written, 300);
        assert_eq!(writer.last_write_ms, 20);
        assert!((writer.average_write_size() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_writer_error_rate() {
        let mut writer = StorageWriter::new();
        writer.record_write(100, 10);
        writer.record_error();
        writer.record_error();
        assert_eq!(writer.write_errors, 2);
        assert!((writer.error_rate() - (2.0 / 3.0)).abs() < f64::EPSILON);
    }

    // === StorageReader tests ===
    #[test]
    fn storage_reader_record() {
        let mut reader = StorageReader::new();
        reader.record_read(500, 100);
        reader.record_read(300, 200);
        assert_eq!(reader.reads, 2);
        assert_eq!(reader.bytes_read, 800);
        assert!((reader.average_read_size() - 400.0).abs() < f64::EPSILON);
    }

    // === StorageReplicator tests ===
    #[test]
    fn storage_replicator_lifecycle() {
        let mut rep = StorageReplicator::new();
        rep.record_sent();
        rep.record_sent();
        rep.record_received();
        rep.record_failed();
        assert_eq!(rep.replications_sent, 2);
        assert_eq!(rep.replications_received, 1);
        assert_eq!(rep.replications_failed, 1);
        assert_eq!(rep.pending_replications, 0);
        assert!((rep.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    // === StorageValidator tests ===
    #[test]
    fn storage_validator_record() {
        let mut validator = StorageValidator::new();
        validator.record_validation(true);
        validator.record_validation(true);
        validator.record_validation(false);
        validator.record_checksum_error();
        assert_eq!(validator.validations, 3);
        assert_eq!(validator.validation_failures, 2);
        assert_eq!(validator.checksum_errors, 1);
        assert!((validator.pass_rate() - (1.0 / 3.0)).abs() < f64::EPSILON);
    }

    // === StorageLifecycleState tests ===
    #[test]
    fn storage_lifecycle_state_variants() {
        assert_eq!(StorageLifecycleState::Uninitialized.as_str(), "uninitialized");
        assert_eq!(StorageLifecycleState::Active.as_str(), "active");
        assert_eq!(StorageLifecycleState::Failed.as_str(), "failed");
        assert!(StorageLifecycleState::Active.is_active());
        assert!(StorageLifecycleState::Active.can_accept_writes());
        assert!(!StorageLifecycleState::Stopped.can_accept_writes());
        assert!(StorageLifecycleState::Stopped.is_terminal());
        assert!(StorageLifecycleState::Failed.is_terminal());
        assert!(!StorageLifecycleState::Active.is_terminal());
    }

    // === StorageBackupManager tests ===
    #[test]
    fn storage_backup_manager_create_and_restore() {
        let bm = StorageBackupManager::new();
        let id1 = bm.create_backup(10, 1000, DatasetId::new([1]));
        let id2 = bm.create_backup(20, 2000, DatasetId::new([2]));
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(bm.backup_count(), 2);
        assert_eq!(bm.total_backups(), 2);
        assert!(bm.restore_backup(0));
        assert_eq!(bm.total_restores(), 1);
        assert!(!bm.restore_backup(999));
        assert!(bm.delete_backup(0));
        assert_eq!(bm.backup_count(), 1);
    }

    // === StorageMigrationManager tests ===
    #[test]
    fn storage_migration_manager_progress() {
        let mut mm = StorageMigrationManager::new(RegionId::new([1]), RegionId::new([2]));
        assert_eq!(mm.state.get(), StorageMigrationState::Idle);
        mm.start_migration(100, 50000);
        assert_eq!(mm.state.get(), StorageMigrationState::Migrating);
        mm.report_progress(50, 25000);
        assert!((mm.progress_percent() - 50.0).abs() < f64::EPSILON);
        mm.report_progress(100, 50000);
        assert!(mm.is_complete());
    }

    #[test]
    fn storage_migration_manager_fail() {
        let mut mm = StorageMigrationManager::new(RegionId::new([1]), RegionId::new([2]));
        mm.fail_migration();
        assert_eq!(mm.state.get(), StorageMigrationState::Failed);
        assert!(mm.state.get().is_terminal());
    }

    // === StorageGarbageCollector tests ===
    #[test]
    fn storage_garbage_collector_record() {
        let mut gc = StorageGarbageCollector::new();
        gc.record_collection(10, 1000, 100);
        gc.record_collection(20, 2000, 200);
        assert_eq!(gc.collections, 2);
        assert_eq!(gc.objects_collected, 30);
        assert_eq!(gc.bytes_collected, 3000);
        assert!((gc.collection_rate() - 15.0).abs() < f64::EPSILON);
        assert!((gc.average_bytes_per_collection() - 1500.0).abs() < f64::EPSILON);
    }

    // === StorageWarmupManager tests ===
    #[test]
    fn storage_warmup_manager_progress() {
        let mut wm = StorageWarmupManager::new();
        wm.start_warmup(100, 1000);
        assert!((wm.warmup_percent() - 0.0).abs() < f64::EPSILON);
        wm.record_warmed(50);
        assert!((wm.warmup_percent() - 50.0).abs() < f64::EPSILON);
        wm.record_warmed(50);
        assert!(wm.is_complete());
    }

    // === StorageThrottleManager tests ===
    #[test]
    fn storage_throttle_manager_acquire() {
        let tm = StorageThrottleManager::new(3);
        assert!(tm.try_acquire(0));
        assert!(tm.try_acquire(0));
        assert!(tm.try_acquire(0));
        assert!(!tm.try_acquire(0));
        assert_eq!(tm.throttled_count(), 1);
        assert_eq!(tm.current_ops(), 3);
    }

    #[test]
    fn storage_throttle_manager_window_reset() {
        let tm = StorageThrottleManager::new(1);
        assert!(tm.try_acquire(0));
        assert!(!tm.try_acquire(0));
        assert!(tm.try_acquire(1500));
        assert_eq!(tm.current_ops(), 1);
    }

    // === StorageMetrics tests ===
    #[test]
    fn storage_metrics_totals() {
        let mut metrics = StorageMetrics::new();
        metrics.total_puts = 100;
        metrics.total_gets = 200;
        metrics.total_deletes = 50;
        metrics.total_writes = 1000;
        metrics.total_reads = 2000;
        metrics.bytes_written = 50000;
        metrics.bytes_read = 100000;
        metrics.error_count = 10;
        assert_eq!(metrics.total_operations(), 350);
        assert_eq!(metrics.total_data_operations(), 3000);
        assert!((metrics.read_write_ratio() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_metrics_error_rate() {
        let metrics = StorageMetrics::default();
        assert!((metrics.error_rate() - 0.0).abs() < f64::EPSILON);
        assert!((metrics.replication_ratio() - 0.0).abs() < f64::EPSILON);
    }

    // === StorageHealthChecker tests ===
    #[test]
    fn storage_health_checker_record() {
        let mut hc = StorageHealthChecker::new();
        hc.record_check(true, 100);
        hc.record_check(true, 200);
        hc.record_check(false, 300);
        assert_eq!(hc.checks_performed, 3);
        assert_eq!(hc.failures, 1);
        assert!((hc.failure_rate() - (1.0 / 3.0)).abs() < f64::EPSILON);
        assert_eq!(hc.current_status(), StorageHealthStatus::Healthy);
    }

    #[test]
    fn storage_health_checker_critical() {
        let mut hc = StorageHealthChecker::new();
        for _ in 0..10 {
            hc.record_check(false, 100);
        }
        assert_eq!(hc.current_status(), StorageHealthStatus::Critical);
    }

    // === StorageEventLogger tests ===
    #[test]
    fn storage_event_logger_log_and_count() {
        let el = StorageEventLogger::new(100);
        let oid = ObjectID::new([1]);
        el.log(StorageEventType::Put, Some(oid), Some(DatasetId::new([1])), 100, "test put");
        el.log(StorageEventType::Get, None, None, 200, "test get");
        assert_eq!(el.event_count(), 2);
        assert_eq!(el.events_by_type(StorageEventType::Put), 1);
        assert_eq!(el.events_by_type(StorageEventType::Get), 1);
        assert_eq!(el.events_by_type(StorageEventType::Delete), 0);
    }

    #[test]
    fn storage_event_logger_clear() {
        let el = StorageEventLogger::new(100);
        el.log(StorageEventType::Put, None, None, 100, "x");
        assert_eq!(el.event_count(), 1);
        el.clear();
        assert_eq!(el.event_count(), 0);
    }

    #[test]
    fn storage_event_type_as_str() {
        assert_eq!(StorageEventType::Put.as_str(), "put");
        assert_eq!(StorageEventType::Get.as_str(), "get");
        assert_eq!(StorageEventType::Delete.as_str(), "delete");
        assert_eq!(StorageEventType::Replicate.as_str(), "replicate");
        assert_eq!(StorageEventType::Backup.as_str(), "backup");
        assert_eq!(StorageEventType::Restore.as_str(), "restore");
        assert_eq!(StorageEventType::Compact.as_str(), "compact");
        assert_eq!(StorageEventType::Error.as_str(), "error");
    }

    // === StorageShardIndex tests ===
    #[test]
    fn storage_shard_index_assign_and_get() {
        let si = StorageShardIndex::new();
        let oid1 = ObjectID::new([1]);
        let oid2 = ObjectID::new([2]);
        let shard1 = si.assign_shard(oid1);
        let shard2 = si.assign_shard(oid2);
        assert_ne!(shard1, shard2);
        assert_eq!(si.shard_count(), 2);
        assert_eq!(si.total_objects(), 2);
        let objects = si.get_shard_objects(shard1);
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0], oid1);
    }

    #[test]
    fn storage_shard_id_variants() {
        let sid = ShardId::new(42);
        assert_eq!(sid.value(), 42);
        assert!(!sid.is_zero());
        assert!(ShardId::new(0).is_zero());
    }

    // === StorageTierPolicy tests ===
    #[test]
    fn storage_tier_policy_tier_for_size() {
        let policy = StorageTierPolicy::new();
        assert_eq!(policy.tier_for_size(100), StorageTier::Hot);
        assert_eq!(policy.tier_for_size(1048576), StorageTier::Hot);
        assert_eq!(policy.tier_for_size(1048577), StorageTier::Warm);
        assert_eq!(policy.tier_for_size(1073741824), StorageTier::Warm);
        assert_eq!(policy.tier_for_size(1073741825), StorageTier::Cold);
        assert_eq!(
            policy.tier_for_size(1099511627777),
            StorageTier::Archived
        );
    }

    #[test]
    fn storage_tier_cost_multiplier() {
        let policy = StorageTierPolicy::new();
        assert!((policy.cost_multiplier_for_size(100) - 1.0).abs() < f64::EPSILON);
        assert!((policy.cost_multiplier_for_size(1073741825) - 5.0).abs() < f64::EPSILON);
        assert!((policy.cost_multiplier_for_size(1099511627777) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn storage_tier_as_str() {
        assert_eq!(StorageTier::Hot.as_str(), "hot");
        assert_eq!(StorageTier::Warm.as_str(), "warm");
        assert_eq!(StorageTier::Cold.as_str(), "cold");
        assert_eq!(StorageTier::Archived.as_str(), "archived");
        assert!(StorageTier::Hot.is_hot());
        assert!(!StorageTier::Cold.is_hot());
        assert!(StorageTier::Archived.is_archived());
        assert!(!StorageTier::Hot.is_archived());
    }

    // === store_validate functions tests ===
    #[test]
    fn store_validate_object_id_and_dataset() {
        let valid_oid = ObjectID::new([1]);
        let invalid_oid = ObjectID::new([]);
        let valid_ds = DatasetId::new([1]);
        let invalid_ds = DatasetId::new([]);
        assert!(store_validate_object_id(valid_oid));
        assert!(!store_validate_object_id(invalid_oid));
        assert!(store_validate_dataset_id(valid_ds));
        assert!(!store_validate_dataset_id(invalid_ds));
    }

    // === StoreError suggested_action tests ===
    #[test]
    fn store_error_suggested_action() {
        assert_eq!(
            StoreError::ObjectNotFound {
                dataset: DatasetId::new([1]),
                object_id: ObjectID::new([2]),
            }
            .suggested_action(),
            "create the object first"
        );
        assert_eq!(
            StoreError::ObjectAlreadyExists {
                dataset: DatasetId::new([1]),
                object_id: ObjectID::new([2]),
            }
            .suggested_action(),
            "use a different object ID"
        );
        assert_eq!(
            StoreError::NotInitialized.suggested_action(),
            "initialize the store"
        );
        assert_eq!(
            StoreError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === StoreError is_not_found and is_already_exists tests ===
    #[test]
    fn store_error_is_not_found_and_already_exists() {
        assert!(StoreError::ObjectNotFound {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
        }
        .is_not_found());
        assert!(StoreError::DatasetNotFound {
            dataset: DatasetId::new([1])
        }
        .is_not_found());
        assert!(!StoreError::WriteFailed {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
            reason: "x".to_string()
        }
        .is_not_found());
        assert!(StoreError::ObjectAlreadyExists {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
        }
        .is_already_exists());
        assert!(!StoreError::ObjectNotFound {
            dataset: DatasetId::new([1]),
            object_id: ObjectID::new([2]),
        }
        .is_already_exists());
    }

    // === StoreConfig required_acks_for_dataset test ===
    #[test]
    fn store_config_required_acks_for_dataset() {
        let config = StoreConfig::new();
        assert_eq!(
            config.required_acks_for_dataset(DatasetId::new([1])),
            ConsistencyLevel::Eventual.required_acks(3)
        );
        let config = StoreConfig::new().with_replication_factor(5).with_consistency_level(ConsistencyLevel::Strong);
        assert_eq!(
            config.required_acks_for_dataset(DatasetId::new([1])),
            5
        );
    }
}