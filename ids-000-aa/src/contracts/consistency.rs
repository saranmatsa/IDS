//! # Sealed Trait: Consistency — profiles, committed-prefix, MVCC
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Consistency trait defines consistency profiles, committed-prefix ordering,
//! multi-version concurrency control, and read visibility guarantees across
//! datasets and partitions. Implementations live downstream in ids-consistency
//! and ids-* crates.

use crate::types::{
    Capacity, ConsistencyProfile, DatasetId, DurabilityCondition, Endpoint, FailureDomain,
    HUnitId, NodeId, OrderingPos, PartitionId, RecordId, RedundancyMode, RegionId,
};
use crate::errors::ConsistencyError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Consistency
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::ConsistencyEngine {}
impl private::Sealed for super::ConsistencyProfileMgr {}
impl private::Sealed for super::CommittedPrefixTracker {}
impl private::Sealed for super::MVCCManager {}
impl private::Sealed for super::VersionStore {}
impl private::Sealed for super::ReadVisibilityManager {}
impl private::Sealed for super::WriteOrderingManager {}
impl private::Sealed for super::ConsistencyChecker {}
impl private::Sealed for super::PrefixValidator {}
impl private::Sealed for super::ConsistencyStats {}
impl private::Sealed for super::ConsistencyMonitor {}

/// Sealed trait for consistency — profiles, committed-prefix, MVCC.
/// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Consistency: private::Sealed {
    /// Set the consistency profile for a dataset.
    fn set_profile(
        &self,
        dataset_id: DatasetId,
        profile: ConsistencyProfile,
    ) -> Result<(), ConsistencyError>;

    /// Get the consistency profile for a dataset.
    fn get_profile(&self, dataset_id: &DatasetId) -> ConsistencyProfile;

    /// Record a write at an ordering position (committed-prefix tracking).
    fn record_write(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
        node_id: NodeId,
    ) -> Result<(), ConsistencyError>;

    /// Get the committed prefix position for a dataset.
    fn committed_prefix(&self, dataset_id: &DatasetId) -> OrderingPos;

    /// Check if a read at a given position is visible.
    fn is_read_visible(
        &self,
        dataset_id: &DatasetId,
        position: OrderingPos,
    ) -> Result<bool, ConsistencyError>;

    /// Create a new MVCC version for a record.
    fn create_version(
        &self,
        dataset_id: DatasetId,
        partition_id: PartitionId,
        record_id: RecordId,
        position: OrderingPos,
        value: Vec<u8>,
    ) -> Result<VersionId, ConsistencyError>;

    /// Read a version at or before a given position.
    fn read_version(
        &self,
        dataset_id: &DatasetId,
        partition_id: &PartitionId,
        record_id: &RecordId,
        at_position: OrderingPos,
    ) -> Result<Option<VersionRef>, ConsistencyError>;

    /// Get consistency stats.
    fn stats(&self) -> ConsistencyStats;

    /// Get the consistency name for debugging.
    fn consistency_name(&self) -> &'static str;

    /// Get total profile count.
    fn profile_count(&self) -> u64;

    /// Get total version count.
    fn version_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ConsistencyError {
    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("partition not found: {partition_id:?}")]
    PartitionNotFound { partition_id: PartitionId },

    #[error("record not found: {record_id:?}")]
    RecordNotFound { record_id: RecordId },

    #[error("version not found: {version_id:?}")]
    VersionNotFound { version_id: VersionId },

    #[error("profile not found: {dataset_id:?}")]
    ProfileNotFound { dataset_id: DatasetId },

    #[error("position out of order: expected={expected}, got={got}")]
    PositionOutOfOrder { expected: u64, got: u64 },

    #[error("read not visible: position={position}, prefix={prefix}")]
    ReadNotVisible { position: u64, prefix: u64 },

    #[error("version conflict: position={position}, existing={existing}")]
    VersionConflict { position: u64, existing: u64 },

    #[error("invalid consistency profile: profile={profile:?}")]
    InvalidProfile { profile: ConsistencyProfile },

    #[error("MVCC error: {message}")]
    MvccError { message: String },

    #[error("internal consistency error: {message}")]
    Internal { message: String },
}

impl ConsistencyError {
    pub fn category(&self) -> &'static str {
        match self {
            ConsistencyError::DatasetNotFound { .. } => "dataset_not_found",
            ConsistencyError::PartitionNotFound { .. } => "partition_not_found",
            ConsistencyError::RecordNotFound { .. } => "record_not_found",
            ConsistencyError::VersionNotFound { .. } => "version_not_found",
            ConsistencyError::ProfileNotFound { .. } => "profile_not_found",
            ConsistencyError::PositionOutOfOrder { .. } => "position_out_of_order",
            ConsistencyError::ReadNotVisible { .. } => "read_not_visible",
            ConsistencyError::VersionConflict { .. } => "version_conflict",
            ConsistencyError::InvalidProfile { .. } => "invalid_profile",
            ConsistencyError::MvccError { .. } => "mvcc_error",
            ConsistencyError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ConsistencyError::PositionOutOfOrder { .. }
                | ConsistencyError::ReadNotVisible { .. }
                | ConsistencyError::VersionConflict { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ConsistencyError::DatasetNotFound { .. } => "create dataset first",
            ConsistencyError::PartitionNotFound { .. } => "create partition first",
            ConsistencyError::RecordNotFound { .. } => "create record first",
            ConsistencyError::VersionNotFound { .. } => "create version first",
            ConsistencyError::ProfileNotFound { .. } => "set consistency profile first",
            ConsistencyError::PositionOutOfOrder { .. } => "retry with correct position",
            ConsistencyError::ReadNotVisible { .. } => "wait for commit to advance prefix",
            ConsistencyError::VersionConflict { .. } => "resolve conflict or use latest version",
            ConsistencyError::InvalidProfile { .. } => "use a valid consistency profile",
            ConsistencyError::MvccError { .. } => "check MVCC store integrity",
            ConsistencyError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION ID & VERSION REF
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VersionId(pub [u8; 32]);

impl VersionId {
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

#[derive(Debug, Clone)]
pub struct VersionRef {
    pub version_id: VersionId,
    pub position: OrderingPos,
    pub value: Vec<u8>,
    pub created_at_ms: u64,
    pub visible: bool,
}

impl VersionRef {
    pub fn new(
        version_id: VersionId,
        position: OrderingPos,
        value: Vec<u8>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            version_id,
            position,
            value,
            created_at_ms,
            visible: false,
        }
    }

    pub fn mark_visible(&mut self) {
        self.visible = true;
    }

    pub fn mark_invisible(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn value_len(&self) -> usize {
        self.value.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY PROFILE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct ConsistencyProfileMgr {
    pub profiles: std::cell::RefCell<std::collections::HashMap<DatasetId, ConsistencyProfile>>,
    pub profile_count: u64,
    pub change_count: u64,
}

impl ConsistencyProfileMgr {
    pub fn new() -> Self {
        Self {
            profiles: std::cell::RefCell::new(std::collections::HashMap::new()),
            profile_count: 0,
            change_count: 0,
        }
    }

    pub fn set_profile(
        &self,
        dataset_id: DatasetId,
        profile: ConsistencyProfile,
    ) -> Result<(), ConsistencyError> {
        let mut p = self.profiles.borrow_mut();
        let existed = p.contains_key(&dataset_id);
        p.insert(dataset_id, profile);
        if !existed {
            self.profile_count += 1;
        }
        self.change_count += 1;
        Ok(())
    }

    pub fn get_profile(&self, dataset_id: &DatasetId) -> ConsistencyProfile {
        self.profiles
            .borrow()
            .get(dataset_id)
            .copied()
            .unwrap_or(ConsistencyProfile::Strong)
    }

    pub fn remove_profile(&self, dataset_id: &DatasetId) -> bool {
        let removed = self.profiles.borrow_mut().remove(dataset_id).is_some();
        if removed {
            self.change_count += 1;
        }
        removed
    }

    pub fn profile_count(&self) -> u64 {
        self.profile_count
    }

    pub fn tracked_count(&self) -> usize {
        self.profiles.borrow().len()
    }

    pub fn change_count(&self) -> u64 {
        self.change_count
    }
}

impl Default for ConsistencyProfileMgr {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMMITTED PREFIX TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct CommittedPrefixTracker {
    pub prefixes: std::cell::RefCell<std::collections::HashMap<DatasetId, OrderingPos>>,
    pub prefix_updates: u64,
    pub max_position_seen: u64,
}

impl CommittedPrefixTracker {
    pub fn new() -> Self {
        Self {
            prefixes: std::cell::RefCell::new(std::collections::HashMap::new()),
            prefix_updates: 0,
            max_position_seen: 0,
        }
    }

    pub fn update_prefix(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
    ) -> Result<(), ConsistencyError> {
        let mut prefixes = self.prefixes.borrow_mut();
        let current = prefixes.get(&dataset_id).copied().unwrap_or(OrderingPos::new(0));
        if position.as_u64() < current.as_u64() {
            return Err(ConsistencyError::PositionOutOfOrder {
                expected: current.as_u64(),
                got: position.as_u64(),
            });
        }
        prefixes.insert(dataset_id, position);
        self.prefix_updates += 1;
        self.max_position_seen = self.max_position_seen.max(position.as_u64());
        Ok(())
    }

    pub fn get_prefix(&self, dataset_id: &DatasetId) -> OrderingPos {
        self.prefixes
            .borrow()
            .get(dataset_id)
            .copied()
            .unwrap_or(OrderingPos::new(0))
    }

    pub fn advance_prefix(&self, dataset_id: DatasetId, delta: u64) -> Result<(), ConsistencyError> {
        let mut prefixes = self.prefixes.borrow_mut();
        let current = prefixes.get(&dataset_id).copied().unwrap_or(OrderingPos::new(0));
        let new_pos = OrderingPos::new(current.as_u64() + delta);
        prefixes.insert(dataset_id, new_pos);
        self.prefix_updates += 1;
        self.max_position_seen = self.max_position_seen.max(new_pos.as_u64());
        Ok(())
    }

    pub fn prefix_update_count(&self) -> u64 {
        self.prefix_updates
    }

    pub fn max_position_seen(&self) -> u64 {
        self.max_position_seen
    }

    pub fn tracked_count(&self) -> usize {
        self.prefixes.borrow().len()
    }
}

impl Default for CommittedPrefixTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// MVCC MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct MVCCManager {
    pub versions: std::cell::RefCell<
        std::collections::HashMap<(DatasetId, PartitionId, RecordId), Vec<VersionRef>>,
    >,
    pub version_counter: u64,
    pub create_count: u64,
    pub read_count: u64,
    pub conflict_count: u64,
}

impl MVCCManager {
    pub fn new() -> Self {
        Self {
            versions: std::cell::RefCell::new(std::collections::HashMap::new()),
            version_counter: 0,
            create_count: 0,
            read_count: 0,
            conflict_count: 0,
        }
    }

    pub fn create_version(
        &self,
        dataset_id: DatasetId,
        partition_id: PartitionId,
        record_id: RecordId,
        position: OrderingPos,
        value: Vec<u8>,
    ) -> Result<VersionId, ConsistencyError> {
        let mut versions = self.versions.borrow_mut();
        let key = (dataset_id, partition_id, record_id);
        let existing = versions.get(&key);
        if let Some(vlist) = existing {
            if let Some(last) = vlist.last() {
                if position.as_u64() <= last.position.as_u64() {
                    self.conflict_count += 1;
                    return Err(ConsistencyError::VersionConflict {
                        position: position.as_u64(),
                        existing: last.position.as_u64(),
                    });
                }
            }
        }
        self.version_counter += 1;
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&self.version_counter.to_le_bytes());
        let version_id = VersionId::new(bytes);
        let version = VersionRef::new(version_id, position, value, 0);
        versions.entry(key).or_default().push(version);
        self.create_count += 1;
        Ok(version_id)
    }

    pub fn read_version(
        &self,
        dataset_id: &DatasetId,
        partition_id: &PartitionId,
        record_id: &RecordId,
        at_position: OrderingPos,
    ) -> Result<Option<VersionRef>, ConsistencyError> {
        self.read_count += 1;
        let versions = self.versions.borrow();
        let key = (dataset_id.clone(), *partition_id, record_id.clone());
        match versions.get(&key) {
            Some(vlist) => {
                let best = vlist
                    .iter()
                    .filter(|v| v.position.as_u64() <= at_position.as_u64())
                    .max_by_key(|v| v.position.as_u64());
                Ok(best.cloned())
            }
            None => Ok(None),
        }
    }

    pub fn get_latest_version(
        &self,
        dataset_id: &DatasetId,
        partition_id: &PartitionId,
        record_id: &RecordId,
    ) -> Option<VersionRef> {
        let versions = self.versions.borrow();
        let key = (dataset_id.clone(), *partition_id, record_id.clone());
        versions.get(&key).and_then(|v| v.last().cloned())
    }

    pub fn version_count_for_record(
        &self,
        dataset_id: &DatasetId,
        partition_id: &PartitionId,
        record_id: &RecordId,
    ) -> usize {
        let versions = self.versions.borrow();
        let key = (dataset_id.clone(), *partition_id, record_id.clone());
        versions.get(&key).map(|v| v.len()).unwrap_or(0)
    }

    pub fn create_count(&self) -> u64 {
        self.create_count
    }

    pub fn read_count(&self) -> u64 {
        self.read_count
    }

    pub fn conflict_count(&self) -> u64 {
        self.conflict_count
    }

    pub fn total_version_count(&self) -> u64 {
        self.versions.borrow().values().map(|v| v.len() as u64).sum()
    }

    pub fn record_count(&self) -> usize {
        self.versions.borrow().len()
    }
}

impl Default for MVCCManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION STORE
///////////////////////////////////////////////////////////////////////////////

pub struct VersionStore {
    pub stored_versions: std::cell::RefCell<std::collections::HashMap<VersionId, VersionRef>>,
    pub store_count: u64,
    pub evict_count: u64,
    pub max_versions: u64,
}

impl VersionStore {
    pub fn new(max_versions: u64) -> Self {
        Self {
            stored_versions: std::cell::RefCell::new(std::collections::HashMap::new()),
            store_count: 0,
            evict_count: 0,
            max_versions,
        }
    }

    pub fn store(&self, version: VersionRef) -> Result<(), ConsistencyError> {
        let mut store = self.stored_versions.borrow_mut();
        if store.len() as u64 >= self.max_versions {
            self.evict_count += 1;
            return Err(ConsistencyError::Internal {
                message: "version store at capacity".to_string(),
            });
        }
        store.insert(version.version_id, version);
        self.store_count += 1;
        Ok(())
    }

    pub fn get(&self, version_id: &VersionId) -> Option<VersionRef> {
        self.stored_versions.borrow().get(version_id).cloned()
    }

    pub fn remove(&self, version_id: &VersionId) -> bool {
        self.stored_versions.borrow_mut().remove(version_id).is_some()
    }

    pub fn store_count(&self) -> u64 {
        self.store_count
    }

    pub fn evict_count(&self) -> u64 {
        self.evict_count
    }

    pub fn stored_count(&self) -> usize {
        self.stored_versions.borrow().len()
    }

    pub fn is_at_capacity(&self) -> bool {
        self.stored_versions.borrow().len() as u64 >= self.max_versions
    }
}

impl Default for VersionStore {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// READ VISIBILITY MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct ReadVisibilityManager {
    pub visibility_rules: std::cell::RefCell<
        std::collections::HashMap<DatasetId, ConsistencyProfile>,
    >,
    pub visibility_checks: u64,
    pub visible_count: u64,
    pub invisible_count: u64,
}

impl ReadVisibilityManager {
    pub fn new() -> Self {
        Self {
            visibility_rules: std::cell::RefCell::new(std::collections::HashMap::new()),
            visibility_checks: 0,
            visible_count: 0,
            invisible_count: 0,
        }
    }

    pub fn set_rule(&self, dataset_id: DatasetId, profile: ConsistencyProfile) {
        self.visibility_rules.borrow_mut().insert(dataset_id, profile);
    }

    pub fn check_visibility(
        &self,
        dataset_id: &DatasetId,
        version_position: OrderingPos,
        committed_prefix: OrderingPos,
    ) -> bool {
        self.visibility_checks += 1;
        let profile = self
            .visibility_rules
            .borrow()
            .get(dataset_id)
            .copied()
            .unwrap_or(ConsistencyProfile::Strong);
        let visible = match profile {
            ConsistencyProfile::Strong => version_position.as_u64() <= committed_prefix.as_u64(),
            ConsistencyProfile::Eventual => true,
            ConsistencyProfile::BoundedStaleness { .. } => {
                version_position.as_u64() <= committed_prefix.as_u64()
            }
            ConsistencyProfile::Session { .. } => {
                version_position.as_u64() <= committed_prefix.as_u64()
            }
            ConsistencyProfile::Prefix => {
                version_position.as_u64() <= committed_prefix.as_u64()
            }
        };
        if visible {
            self.visible_count += 1;
        } else {
            self.invisible_count += 1;
        }
        visible
    }

    pub fn visibility_check_count(&self) -> u64 {
        self.visibility_checks
    }

    pub fn visible_count(&self) -> u64 {
        self.visible_count
    }

    pub fn invisible_count(&self) -> u64 {
        self.invisible_count
    }

    pub fn visibility_rate(&self) -> f64 {
        if self.visibility_checks == 0 {
            return 1.0;
        }
        self.visible_count as f64 / self.visibility_checks as f64
    }

    pub fn rule_count(&self) -> usize {
        self.visibility_rules.borrow().len()
    }
}

impl Default for ReadVisibilityManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// WRITE ORDERING MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct WriteOrderingManager {
    pub write_positions: std::cell::RefCell<std::collections::HashMap<DatasetId, Vec<OrderingPos>>>,
    pub write_count: u64,
    pub out_of_order_count: u64,
    pub max_position: u64,
}

impl WriteOrderingManager {
    pub fn new() -> Self {
        Self {
            write_positions: std::cell::RefCell::new(std::collections::HashMap::new()),
            write_count: 0,
            out_of_order_count: 0,
            max_position: 0,
        }
    }

    pub fn record_write_position(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
    ) -> Result<(), ConsistencyError> {
        self.write_count += 1;
        let mut positions = self.write_positions.borrow_mut();
        let dataset_positions = positions.entry(dataset_id).or_default();
        if let Some(last) = dataset_positions.last() {
            if position.as_u64() <= last.as_u64() {
                self.out_of_order_count += 1;
                return Err(ConsistencyError::PositionOutOfOrder {
                    expected: last.as_u64() + 1,
                    got: position.as_u64(),
                });
            }
        }
        dataset_positions.push(position);
        self.max_position = self.max_position.max(position.as_u64());
        Ok(())
    }

    pub fn get_positions(&self, dataset_id: &DatasetId) -> Vec<OrderingPos> {
        self.write_positions
            .borrow()
            .get(dataset_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn last_position(&self, dataset_id: &DatasetId) -> Option<OrderingPos> {
        self.write_positions
            .borrow()
            .get(dataset_id)
            .and_then(|v| v.last().copied())
    }

    pub fn write_count(&self) -> u64 {
        self.write_count
    }

    pub fn out_of_order_count(&self) -> u64 {
        self.out_of_order_count
    }

    pub fn max_position(&self) -> u64 {
        self.max_position
    }

    pub fn dataset_count(&self) -> usize {
        self.write_positions.borrow().len()
    }
}

impl Default for WriteOrderingManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY CHECKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyCheckResult {
    Consistent,
    Stale,
    Diverged,
    Unknown,
}

impl ConsistencyCheckResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConsistencyCheckResult::Consistent => "consistent",
            ConsistencyCheckResult::Stale => "stale",
            ConsistencyCheckResult::Diverged => "diverged",
            ConsistencyCheckResult::Unknown => "unknown",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ConsistencyCheckResult::Consistent)
    }
}

pub struct ConsistencyChecker {
    pub check_results: std::cell::RefCell<
        std::collections::HashMap<DatasetId, ConsistencyCheckResult>,
    >,
    pub check_count: u64,
    pub consistent_count: u64,
    pub stale_count: u64,
    pub diverged_count: u64,
}

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self {
            check_results: std::cell::RefCell::new(std::collections::HashMap::new()),
            check_count: 0,
            consistent_count: 0,
            stale_count: 0,
            diverged_count: 0,
        }
    }

    pub fn check(
        &self,
        dataset_id: DatasetId,
        replica_positions: Vec<OrderingPos>,
    ) -> ConsistencyCheckResult {
        self.check_count += 1;
        let result = if replica_positions.is_empty() {
            ConsistencyCheckResult::Unknown
        } else {
            let max = replica_positions
                .iter()
                .map(|p| p.as_u64())
                .max()
                .unwrap_or(0);
            let min = replica_positions
                .iter()
                .map(|p| p.as_u64())
                .min()
                .unwrap_or(0);
            let divergence = max.saturating_sub(min);
            if divergence == 0 {
                ConsistencyCheckResult::Consistent
            } else if divergence < 1000 {
                ConsistencyCheckResult::Stale
            } else {
                ConsistencyCheckResult::Diverged
            }
        };
        match result {
            ConsistencyCheckResult::Consistent => self.consistent_count += 1,
            ConsistencyCheckResult::Stale => self.stale_count += 1,
            ConsistencyCheckResult::Diverged => self.diverged_count += 1,
            _ => {}
        }
        self.check_results
            .borrow_mut()
            .insert(dataset_id, result);
        result
    }

    pub fn get_result(&self, dataset_id: &DatasetId) -> Option<ConsistencyCheckResult> {
        self.check_results.borrow().get(dataset_id).copied()
    }

    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    pub fn consistent_count(&self) -> u64 {
        self.consistent_count
    }

    pub fn stale_count(&self) -> u64 {
        self.stale_count
    }

    pub fn diverged_count(&self) -> u64 {
        self.diverged_count
    }

    pub fn consistency_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 1.0;
        }
        self.consistent_count as f64 / self.check_count as f64
    }

    pub fn checked_dataset_count(&self) -> usize {
        self.check_results.borrow().len()
    }
}

impl Default for ConsistencyChecker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PREFIX VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct PrefixValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<DatasetId, OrderingPos>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl PrefixValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate(
        &self,
        dataset_id: DatasetId,
        candidate_prefix: OrderingPos,
        committed_prefix: OrderingPos,
    ) -> bool {
        self.validation_count += 1;
        let valid = candidate_prefix.as_u64() <= committed_prefix.as_u64();
        if valid {
            self.valid_count += 1;
            self.validations
                .borrow_mut()
                .insert(dataset_id, candidate_prefix);
        } else {
            self.invalid_count += 1;
        }
        valid
    }

    pub fn get_validated_prefix(&self, dataset_id: &DatasetId) -> Option<OrderingPos> {
        self.validations.borrow().get(dataset_id).copied()
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

    pub fn validated_count(&self) -> usize {
        self.validations.borrow().len()
    }
}

impl Default for PrefixValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct ConsistencyStats {
    pub total_profiles: u64,
    pub total_versions: u64,
    pub total_writes: u64,
    pub total_reads: u64,
    pub total_conflicts: u64,
    pub consistency_checks: u64,
    pub consistent_results: u64,
    pub avg_divergence_ms: f64,
    pub read_visibility_rate: f64,
}

impl ConsistencyStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn consistency_health(&self) -> f64 {
        if self.consistency_checks == 0 {
            return 1.0;
        }
        self.consistent_results as f64 / self.consistency_checks as f64
    }

    pub fn conflict_rate(&self) -> f64 {
        if self.total_writes == 0 {
            return 0.0;
        }
        self.total_conflicts as f64 / self.total_writes as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct ConsistencyMonitor {
    pub dataset_health: std::cell::RefCell<
        std::collections::HashMap<DatasetId, ConsistencyCheckResult>,
    >,
    pub alert_count: u64,
    pub recovery_count: u64,
}

impl ConsistencyMonitor {
    pub fn new() -> Self {
        Self {
            dataset_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            recovery_count: 0,
        }
    }

    pub fn report_health(&self, dataset_id: DatasetId, result: ConsistencyCheckResult) {
        let mut health = self.dataset_health.borrow_mut();
        let prev = health.get(&dataset_id).copied();
        health.insert(dataset_id, result);
        match (prev, result) {
            (Some(ConsistencyCheckResult::Consistent), ConsistencyCheckResult::Diverged) => {
                self.alert_count += 1;
            }
            (Some(ConsistencyCheckResult::Diverged), ConsistencyCheckResult::Consistent) => {
                self.recovery_count += 1;
            }
            _ => {}
        }
    }

    pub fn get_health(&self, dataset_id: &DatasetId) -> Option<ConsistencyCheckResult> {
        self.dataset_health.borrow().get(dataset_id).copied()
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn recovery_count(&self) -> u64 {
        self.recovery_count
    }

    pub fn monitored_count(&self) -> usize {
        self.dataset_health.borrow().len()
    }

    pub fn unhealthy_count(&self) -> usize {
        self.dataset_health
            .borrow()
            .values()
            .filter(|r| !r.is_ok())
            .count()
    }
}

impl Default for ConsistencyMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct ConsistencyEngine {
    pub profile_mgr: std::cell::RefCell<ConsistencyProfileMgr>,
    pub prefix_tracker: std::cell::RefCell<CommittedPrefixTracker>,
    pub mvcc: std::cell::RefCell<MVCCManager>,
    pub version_store: std::cell::RefCell<VersionStore>,
    pub read_visibility: std::cell::RefCell<ReadVisibilityManager>,
    pub write_ordering: std::cell::RefCell<WriteOrderingManager>,
    pub checker: std::cell::RefCell<ConsistencyChecker>,
    pub prefix_validator: std::cell::RefCell<PrefixValidator>,
    pub stats: std::cell::RefCell<ConsistencyStats>,
    pub monitor: std::cell::RefCell<ConsistencyMonitor>,
    pub mvcc_garbage_collector: std::cell::RefCell<MVCCGarbageCollector>,
    pub snapshot_isolation: std::cell::RefCell<SnapshotIsolationManager>,
    pub conflict_resolver: std::cell::RefCell<ConflictResolver>,
    pub read_consistency_level: std::cell::RefCell<ReadConsistencyLevelManager>,
    pub write_ahead_consistency: std::cell::RefCell<WriteAheadConsistencyManager>,
    pub consistency_violations: std::cell::RefCell<Vec<ConsistencyViolation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MVCCGarbageCollectionPolicy {
    KeepAll,
    KeepLastN { count: u64 },
    KeepByTime { retention_ms: u64 },
    KeepByVersions { count: u64 },
}

impl MVCCGarbageCollectionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            MVCCGarbageCollectionPolicy::KeepAll => "keep_all",
            MVCCGarbageCollectionPolicy::KeepLastN { .. } => "keep_last_n",
            MVCCGarbageCollectionPolicy::KeepByTime { .. } => "keep_by_time",
            MVCCGarbageCollectionPolicy::KeepByVersions { .. } => "keep_by_versions",
        }
    }
}

pub struct MVCCGarbageCollector {
    pub policy: MVCCGarbageCollectionPolicy,
    pub gc_runs: u64,
    pub versions_collected: u64,
    pub versions_retained: u64,
    pub last_gc_ms: Option<u64>,
}

impl MVCCGarbageCollector {
    pub fn new(policy: MVCCGarbageCollectionPolicy) -> Self {
        Self {
            policy,
            gc_runs: 0,
            versions_collected: 0,
            versions_retained: 0,
            last_gc_ms: None,
        }
    }

    pub fn run_gc(&self, now_ms: u64) -> u64 {
        self.gc_runs += 1;
        self.last_gc_ms = Some(now_ms);
        let collected = match self.policy {
            MVCCGarbageCollectionPolicy::KeepAll => 0,
            MVCCGarbageCollectionPolicy::KeepLastN { count } => {
                let to_collect = count.min(100);
                self.versions_collected += to_collect;
                to_collect
            }
            MVCCGarbageCollectionPolicy::KeepByTime { retention_ms } => {
                let cutoff = now_ms.saturating_sub(retention_ms);
                let collected = cutoff / 1000;
                self.versions_collected += collected;
                collected
            }
            MVCCGarbageCollectionPolicy::KeepByVersions { count } => {
                let collected = count.min(50);
                self.versions_collected += collected;
                collected
            }
        };
        self.versions_retained = self.versions_retained.saturating_add(collected);
        collected
    }

    pub fn gc_run_count(&self) -> u64 {
        self.gc_runs
    }

    pub fn collected_count(&self) -> u64 {
        self.versions_collected
    }

    pub fn retained_count(&self) -> u64 {
        self.versions_retained
    }
}

impl Default for MVCCGarbageCollector {
    fn default() -> Self {
        Self::new(MVCCGarbageCollectionPolicy::KeepLastN { count: 1000 })
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT ISOLATION MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotIsolationLevel {
    None,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl SnapshotIsolationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotIsolationLevel::None => "none",
            SnapshotIsolationLevel::ReadCommitted => "read_committed",
            SnapshotIsolationLevel::RepeatableRead => "repeatable_read",
            SnapshotIsolationLevel::Serializable => "serializable",
        }
    }

    pub fn requires_snapshot(&self) -> bool {
        matches!(
            self,
            SnapshotIsolationLevel::RepeatableRead | SnapshotIsolationLevel::Serializable
        )
    }

    pub fn isolation_strength(&self) -> u8 {
        match self {
            SnapshotIsolationLevel::None => 0,
            SnapshotIsolationLevel::ReadCommitted => 1,
            SnapshotIsolationLevel::RepeatableRead => 2,
            SnapshotIsolationLevel::Serializable => 3,
        }
    }
}

pub struct SnapshotIsolationManager {
    pub snapshots: std::cell::RefCell<std::collections::HashMap<u64, OrderingPos>>,
    pub snapshot_counter: u64,
    pub active_snapshots: u64,
    pub committed_snapshots: u64,
    pub aborted_snapshots: u64,
    pub level: SnapshotIsolationLevel,
}

impl SnapshotIsolationManager {
    pub fn new(level: SnapshotIsolationLevel) -> Self {
        Self {
            snapshots: std::cell::RefCell::new(std::collections::HashMap::new()),
            snapshot_counter: 0,
            active_snapshots: 0,
            committed_snapshots: 0,
            aborted_snapshots: 0,
            level,
        }
    }

    pub fn begin_snapshot(&self, at_position: OrderingPos) -> u64 {
        self.snapshot_counter += 1;
        self.active_snapshots += 1;
        let id = self.snapshot_counter;
        self.snapshots.borrow_mut().insert(id, at_position);
        id
    }

    pub fn commit_snapshot(&self, snapshot_id: u64) -> bool {
        if self.snapshots.borrow_mut().remove(&snapshot_id).is_some() {
            self.active_snapshots = self.active_snapshots.saturating_sub(1);
            self.committed_snapshots += 1;
            true
        } else {
            false
        }
    }

    pub fn abort_snapshot(&self, snapshot_id: u64) -> bool {
        if self.snapshots.borrow_mut().remove(&snapshot_id).is_some() {
            self.active_snapshots = self.active_snapshots.saturating_sub(1);
            self.aborted_snapshots += 1;
            true
        } else {
            false
        }
    }

    pub fn get_snapshot_position(&self, snapshot_id: u64) -> Option<OrderingPos> {
        self.snapshots.borrow().get(&snapshot_id).copied()
    }

    pub fn active_count(&self) -> u64 {
        self.active_snapshots
    }

    pub fn committed_count(&self) -> u64 {
        self.committed_snapshots
    }

    pub fn aborted_count(&self) -> u64 {
        self.aborted_snapshots
    }

    pub fn level(&self) -> SnapshotIsolationLevel {
        self.level
    }

    pub fn total_snapshots(&self) -> u64 {
        self.snapshot_counter
    }
}

impl Default for SnapshotIsolationManager {
    fn default() -> Self {
        Self::new(SnapshotIsolationLevel::ReadCommitted)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONFLICT RESOLVER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolutionStrategy {
    LastWriteWins,
    FirstWriteWins,
    HighestVersionWins,
    Merge,
    Manual,
}

impl ConflictResolutionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictResolutionStrategy::LastWriteWins => "last_write_wins",
            ConflictResolutionStrategy::FirstWriteWins => "first_write_wins",
            ConflictResolutionStrategy::HighestVersionWins => "highest_version_wins",
            ConflictResolutionStrategy::Merge => "merge",
            ConflictResolutionStrategy::Manual => "manual",
        }
    }

    pub fn resolves_automatically(&self) -> bool {
        matches!(
            self,
            ConflictResolutionStrategy::LastWriteWins
                | ConflictResolutionStrategy::FirstWriteWins
                | ConflictResolutionStrategy::HighestVersionWins
                | ConflictResolutionStrategy::Merge
        )
    }
}

pub struct ConflictResolver {
    pub strategy: ConflictResolutionStrategy,
    pub conflicts_detected: u64,
    pub conflicts_resolved: u64,
    pub conflicts_pending: u64,
    pub resolution_history: std::cell::RefCell<Vec<(u64, ConflictResolutionStrategy)>>,
}

impl ConflictResolver {
    pub fn new(strategy: ConflictResolutionStrategy) -> Self {
        Self {
            strategy,
            conflicts_detected: 0,
            conflicts_resolved: 0,
            conflicts_pending: 0,
            resolution_history: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn detect_conflict(&self) -> u64 {
        self.conflicts_detected += 1;
        self.conflicts_pending += 1;
        self.conflicts_detected
    }

    pub fn resolve_conflict(&self) -> bool {
        if self.conflicts_pending > 0 {
            self.conflicts_pending -= 1;
            self.conflicts_resolved += 1;
            self.resolution_history
                .borrow_mut()
                .push((self.conflicts_resolved, self.strategy));
            true
        } else {
            false
        }
    }

    pub fn auto_resolve(&self) -> bool {
        if self.strategy.resolves_automatically() && self.conflicts_pending > 0 {
            self.conflicts_pending -= 1;
            self.conflicts_resolved += 1;
            true
        } else {
            false
        }
    }

    pub fn conflicts_detected(&self) -> u64 {
        self.conflicts_detected
    }

    pub fn conflicts_resolved(&self) -> u64 {
        self.conflicts_resolved
    }

    pub fn conflicts_pending(&self) -> u64 {
        self.conflicts_pending
    }

    pub fn resolution_rate(&self) -> f64 {
        if self.conflicts_detected == 0 {
            return 1.0;
        }
        self.conflicts_resolved as f64 / self.conflicts_detected as f64
    }

    pub fn history_len(&self) -> usize {
        self.resolution_history.borrow().len()
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ConflictResolutionStrategy::LastWriteWins)
    }
}

///////////////////////////////////////////////////////////////////////////////
// READ CONSISTENCY LEVEL MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadConsistencyLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl ReadConsistencyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReadConsistencyLevel::ReadUncommitted => "read_uncommitted",
            ReadConsistencyLevel::ReadCommitted => "read_committed",
            ReadConsistencyLevel::RepeatableRead => "repeatable_read",
            ReadConsistencyLevel::Serializable => "serializable",
        }
    }

    pub fn requires_prefix_check(&self) -> bool {
        matches!(
            self,
            ReadConsistencyLevel::ReadCommitted
                | ReadConsistencyLevel::RepeatableRead
                | ReadConsistencyLevel::Serializable
        )
    }

    pub fn strength(&self) -> u8 {
        match self {
            ReadConsistencyLevel::ReadUncommitted => 0,
            ReadConsistencyLevel::ReadCommitted => 1,
            ReadConsistencyLevel::RepeatableRead => 2,
            ReadConsistencyLevel::Serializable => 3,
        }
    }
}

pub struct ReadConsistencyLevelManager {
    pub level: ReadConsistencyLevel,
    pub reads_performed: u64,
    pub prefix_checks_performed: u64,
    pub prefix_violations: u64,
    pub per_dataset_levels: std::cell::RefCell<std::collections::HashMap<DatasetId, ReadConsistencyLevel>>,
}

impl ReadConsistencyLevelManager {
    pub fn new(level: ReadConsistencyLevel) -> Self {
        Self {
            level,
            reads_performed: 0,
            prefix_checks_performed: 0,
            prefix_violations: 0,
            per_dataset_levels: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_level(&self, dataset_id: DatasetId, level: ReadConsistencyLevel) {
        self.per_dataset_levels.borrow_mut().insert(dataset_id, level);
    }

    pub fn get_level(&self, dataset_id: &DatasetId) -> ReadConsistencyLevel {
        self.per_dataset_levels
            .borrow()
            .get(dataset_id)
            .copied()
            .unwrap_or(self.level)
    }

    pub fn record_read(&self, dataset_id: &DatasetId) {
        self.reads_performed += 1;
        let rcl = self.get_level(dataset_id);
        if rcl.requires_prefix_check() {
            self.prefix_checks_performed += 1;
        }
    }

    pub fn record_prefix_violation(&self) {
        self.prefix_violations += 1;
    }

    pub fn reads_performed(&self) -> u64 {
        self.reads_performed
    }

    pub fn prefix_checks_performed(&self) -> u64 {
        self.prefix_checks_performed
    }

    pub fn prefix_violations(&self) -> u64 {
        self.prefix_violations
    }

    pub fn violation_rate(&self) -> f64 {
        if self.prefix_checks_performed == 0 {
            return 0.0;
        }
        self.prefix_violations as f64 / self.prefix_checks_performed as f64
    }

    pub fn level_count(&self) -> usize {
        self.per_dataset_levels.borrow().len()
    }
}

impl Default for ReadConsistencyLevelManager {
    fn default() -> Self {
        Self::new(ReadConsistencyLevel::ReadCommitted)
    }
}

///////////////////////////////////////////////////////////////////////////////
// WRITE AHEAD CONSISTENCY MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct WriteAheadConsistencyManager {
    pub pending_writes: std::cell::RefCell<std::collections::VecDeque<(DatasetId, OrderingPos, u64)>>,
    pub committed_writes: u64,
    pub pending_count: u64,
    pub max_pending: u64,
    pub write_barrier_count: u64,
}

impl WriteAheadConsistencyManager {
    pub fn new(max_pending: u64) -> Self {
        Self {
            pending_writes: std::cell::RefCell::new(std::collections::VecDeque::new()),
            committed_writes: 0,
            pending_count: 0,
            max_pending,
            write_barrier_count: 0,
        }
    }

    pub fn enqueue_write(&self, dataset_id: DatasetId, position: OrderingPos, at_ms: u64) -> bool {
        let mut pending = self.pending_writes.borrow_mut();
        if pending.len() as u64 >= self.max_pending {
            return false;
        }
        pending.push_back((dataset_id, position, at_ms));
        self.pending_count += 1;
        true
    }

    pub fn commit_write(&self) -> Option<(DatasetId, OrderingPos, u64)> {
        let popped = self.pending_writes.borrow_mut().pop_front();
        if popped.is_some() {
            self.committed_writes += 1;
            self.pending_count = self.pending_count.saturating_sub(1);
        }
        popped
    }

    pub fn write_barrier(&self) -> u64 {
        self.write_barrier_count += 1;
        self.committed_writes
    }

    pub fn pending_write_count(&self) -> usize {
        self.pending_writes.borrow().len()
    }

    pub fn committed_count(&self) -> u64 {
        self.committed_writes
    }

    pub fn is_at_pending_limit(&self) -> bool {
        self.pending_writes.borrow().len() as u64 >= self.max_pending
    }

    pub fn barrier_count(&self) -> u64 {
        self.write_barrier_count
    }
}

impl Default for WriteAheadConsistencyManager {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY VIOLATION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ConsistencyViolation {
    pub dataset_id: DatasetId,
    pub violation_type: ViolationType,
    pub position: OrderingPos,
    pub detected_at_ms: u64,
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    StaleRead,
    LostUpdate,
    WriteSkew,
    PhantomRead,
    DirtyRead,
    NonRepeatableRead,
}

impl ViolationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ViolationType::StaleRead => "stale_read",
            ViolationType::LostUpdate => "lost_update",
            ViolationType::WriteSkew => "write_skew",
            ViolationType::PhantomRead => "phantom_read",
            ViolationType::DirtyRead => "dirty_read",
            ViolationType::NonRepeatableRead => "non_repeatable_read",
        }
    }
}

impl ConsistencyViolation {
    pub fn new(
        dataset_id: DatasetId,
        violation_type: ViolationType,
        position: OrderingPos,
        detected_at_ms: u64,
    ) -> Self {
        Self {
            dataset_id,
            violation_type,
            position,
            detected_at_ms,
            resolved: false,
        }
    }

    pub fn mark_resolved(&mut self) {
        self.resolved = true;
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED CONSISTENCY ENGINE
///////////////////////////////////////////////////////////////////////////////

impl ConsistencyEngine {
    pub fn new_with_features() -> Self {
        Self {
            profile_mgr: std::cell::RefCell::new(ConsistencyProfileMgr::new()),
            prefix_tracker: std::cell::RefCell::new(CommittedPrefixTracker::new()),
            mvcc: std::cell::RefCell::new(MVCCManager::new()),
            version_store: std::cell::RefCell::new(VersionStore::new(100000)),
            read_visibility: std::cell::RefCell::new(ReadVisibilityManager::new()),
            write_ordering: std::cell::RefCell::new(WriteOrderingManager::new()),
            checker: std::cell::RefCell::new(ConsistencyChecker::new()),
            prefix_validator: std::cell::RefCell::new(PrefixValidator::new()),
            stats: std::cell::RefCell::new(ConsistencyStats::new()),
            monitor: std::cell::RefCell::new(ConsistencyMonitor::new()),
            mvcc_garbage_collector: std::cell::RefCell::new(MVCCGarbageCollector::default()),
            snapshot_isolation: std::cell::RefCell::new(SnapshotIsolationManager::default()),
            conflict_resolver: std::cell::RefCell::new(ConflictResolver::default()),
            read_consistency_level: std::cell::RefCell::new(ReadConsistencyLevelManager::default()),
            write_ahead_consistency: std::cell::RefCell::new(WriteAheadConsistencyManager::default()),
            consistency_violations: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn mvcc_garbage_collect(&self, now_ms: u64) -> u64 {
        self.mvcc_garbage_collector.borrow_mut().run_gc(now_ms)
    }

    pub fn gc_run_count(&self) -> u64 {
        self.mvcc_garbage_collector.borrow().gc_run_count()
    }

    pub fn gc_collected_count(&self) -> u64 {
        self.mvcc_garbage_collector.borrow().collected_count()
    }

    pub fn begin_snapshot(&self, at_position: OrderingPos) -> u64 {
        self.snapshot_isolation.borrow_mut().begin_snapshot(at_position)
    }

    pub fn commit_snapshot(&self, snapshot_id: u64) -> bool {
        self.snapshot_isolation.borrow_mut().commit_snapshot(snapshot_id)
    }

    pub fn abort_snapshot(&self, snapshot_id: u64) -> bool {
        self.snapshot_isolation.borrow_mut().abort_snapshot(snapshot_id)
    }

    pub fn snapshot_active_count(&self) -> u64 {
        self.snapshot_isolation.borrow().active_count()
    }

    pub fn snapshot_committed_count(&self) -> u64 {
        self.snapshot_isolation.borrow().committed_count()
    }

    pub fn snapshot_aborted_count(&self) -> u64 {
        self.snapshot_isolation.borrow().aborted_count()
    }

    pub fn detect_conflict(&self) -> u64 {
        self.conflict_resolver.borrow_mut().detect_conflict()
    }

    pub fn resolve_conflict(&self) -> bool {
        self.conflict_resolver.borrow_mut().resolve_conflict()
    }

    pub fn auto_resolve_conflicts(&self) -> bool {
        self.conflict_resolver.borrow_mut().auto_resolve()
    }

    pub fn conflict_resolution_rate(&self) -> f64 {
        self.conflict_resolver.borrow().resolution_rate()
    }

    pub fn set_read_level(&self, dataset_id: DatasetId, level: ReadConsistencyLevel) {
        self.read_consistency_level.borrow_mut().set_level(dataset_id, level);
    }

    pub fn get_read_level(&self, dataset_id: &DatasetId) -> ReadConsistencyLevel {
        self.read_consistency_level.borrow().get_level(dataset_id)
    }

    pub fn record_read(&self, dataset_id: &DatasetId) {
        self.read_consistency_level.borrow_mut().record_read(dataset_id);
    }

    pub fn record_prefix_violation(&self) {
        self.read_consistency_level.borrow_mut().record_prefix_violation();
    }

    pub fn reads_performed(&self) -> u64 {
        self.read_consistency_level.borrow().reads_performed()
    }

    pub fn prefix_violation_rate(&self) -> f64 {
        self.read_consistency_level.borrow().violation_rate()
    }

    pub fn enqueue_pending_write(&self, dataset_id: DatasetId, position: OrderingPos, at_ms: u64) -> bool {
        self.write_ahead_consistency.borrow_mut().enqueue_write(dataset_id, position, at_ms)
    }

    pub fn commit_pending_write(&self) -> Option<(DatasetId, OrderingPos, u64)> {
        self.write_ahead_consistency.borrow_mut().commit_write()
    }

    pub fn write_barrier(&self) -> u64 {
        self.write_ahead_consistency.borrow_mut().write_barrier()
    }

    pub fn pending_write_count(&self) -> usize {
        self.write_ahead_consistency.borrow().pending_write_count()
    }

    pub fn record_violation(
        &self,
        dataset_id: DatasetId,
        violation_type: ViolationType,
        position: OrderingPos,
        detected_at_ms: u64,
    ) {
        self.consistency_violations
            .borrow_mut()
            .push(ConsistencyViolation::new(dataset_id, violation_type, position, detected_at_ms));
    }

    pub fn violation_count(&self) -> usize {
        self.consistency_violations.borrow().len()
    }

    pub fn resolved_violation_count(&self) -> usize {
        self.consistency_violations
            .borrow()
            .iter()
            .filter(|v| v.resolved)
            .count()
    }

    pub fn resolve_violation(&self, index: usize) -> bool {
        let mut violations = self.consistency_violations.borrow_mut();
        if let Some(v) = violations.get_mut(index) {
            v.mark_resolved();
            true
        } else {
            false
        }
    }

    pub fn violations_by_type(&self, vtype: ViolationType) -> Vec<ConsistencyViolation> {
        self.consistency_violations
            .borrow()
            .iter()
            .filter(|v| v.violation_type == vtype)
            .cloned()
            .collect()
    }

    pub fn set_profile(
        &self,
        dataset_id: DatasetId,
        profile: ConsistencyProfile,
    ) -> Result<(), ConsistencyError> {
        self.profile_mgr.borrow_mut().set_profile(dataset_id, profile)?;
        self.stats.borrow_mut().total_profiles += 1;
        self.read_visibility
            .borrow_mut()
            .set_rule(dataset_id, profile);
        Ok(())
    }

    pub fn get_profile(&self, dataset_id: &DatasetId) -> ConsistencyProfile {
        self.profile_mgr.borrow().get_profile(dataset_id)
    }

    pub fn record_write(
        &self,
        dataset_id: DatasetId,
        position: OrderingPos,
        node_id: NodeId,
    ) -> Result<(), ConsistencyError> {
        self.prefix_tracker
            .borrow_mut()
            .update_prefix(dataset_id, position)?;
        self.write_ordering
            .borrow_mut()
            .record_write_position(dataset_id, position)?;
        self.stats.borrow_mut().total_writes += 1;
        Ok(())
    }

    pub fn committed_prefix(&self, dataset_id: &DatasetId) -> OrderingPos {
        self.prefix_tracker.borrow().get_prefix(dataset_id)
    }

    pub fn is_read_visible(
        &self,
        dataset_id: &DatasetId,
        position: OrderingPos,
    ) -> Result<bool, ConsistencyError> {
        let prefix = self.committed_prefix(dataset_id);
        let visible = self
            .read_visibility
            .borrow_mut()
            .check_visibility(dataset_id, position, prefix);
        if visible {
            Ok(true)
        } else {
            Err(ConsistencyError::ReadNotVisible {
                position: position.as_u64(),
                prefix: prefix.as_u64(),
            })
        }
    }

    pub fn create_version(
        &self,
        dataset_id: DatasetId,
        partition_id: PartitionId,
        record_id: RecordId,
        position: OrderingPos,
        value: Vec<u8>,
    ) -> Result<VersionId, ConsistencyError> {
        let version_id = self.mvcc.borrow_mut().create_version(
            dataset_id,
            partition_id,
            record_id,
            position,
            value,
        )?;
        self.stats.borrow_mut().total_versions += 1;
        Ok(version_id)
    }

    pub fn read_version(
        &self,
        dataset_id: &DatasetId,
        partition_id: &PartitionId,
        record_id: &RecordId,
        at_position: OrderingPos,
    ) -> Result<Option<VersionRef>, ConsistencyError> {
        let result = self.mvcc.borrow_mut().read_version(
            dataset_id,
            partition_id,
            record_id,
            at_position,
        )?;
        self.stats.borrow_mut().total_reads += 1;
        Ok(result)
    }

    pub fn stats(&self) -> ConsistencyStats {
        self.stats.borrow().clone()
    }

    pub fn consistency_name(&self) -> &'static str {
        "consistency_engine"
    }

    pub fn profile_count(&self) -> u64 {
        self.profile_mgr.borrow().profile_count()
    }

    pub fn version_count(&self) -> u64 {
        self.mvcc.borrow().total_version_count()
    }

    pub fn check_consistency(
        &self,
        dataset_id: DatasetId,
        replica_positions: Vec<OrderingPos>,
    ) -> ConsistencyCheckResult {
        let result = self.checker.borrow_mut().check(dataset_id, replica_positions);
        self.monitor.borrow_mut().report_health(dataset_id, result);
        self.stats.borrow_mut().consistency_checks += 1;
        if result.is_ok() {
            self.stats.borrow_mut().consistent_results += 1;
        }
        result
    }

    pub fn validate_prefix(
        &self,
        dataset_id: DatasetId,
        candidate_prefix: OrderingPos,
        committed_prefix: OrderingPos,
    ) -> bool {
        self.prefix_validator
            .borrow_mut()
            .validate(dataset_id, candidate_prefix, committed_prefix)
    }

    pub fn store_version(&self, version: VersionRef) -> Result<(), ConsistencyError> {
        self.version_store.borrow_mut().store(version)
    }

    pub fn get_stored_version(&self, version_id: &VersionId) -> Option<VersionRef> {
        self.version_store.borrow().get(version_id)
    }

    pub fn record_count(&self) -> usize {
        self.mvcc.borrow().record_count()
    }
}

impl Default for ConsistencyEngine {
    fn default() -> Self {
        Self::new_with_features()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONSISTENCY CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

pub struct ConsistencyConfig {
    pub default_profile: ConsistencyProfile,
    pub enable_mvcc: bool,
    pub enable_prefix_tracking: bool,
    pub enable_violation_tracking: bool,
    pub max_versions_per_record: u64,
    pub gc_enabled: bool,
}

impl ConsistencyConfig {
    pub fn new() -> Self {
        Self {
            default_profile: ConsistencyProfile::Strong,
            enable_mvcc: true,
            enable_prefix_tracking: true,
            enable_violation_tracking: true,
            max_versions_per_record: 1000,
            gc_enabled: true,
        }
    }

    pub fn with_default_profile(mut self, profile: ConsistencyProfile) -> Self {
        self.default_profile = profile;
        self
    }

    pub fn with_mvcc(mut self, enabled: bool) -> Self {
        self.enable_mvcc = enabled;
        self
    }

    pub fn with_prefix_tracking(mut self, enabled: bool) -> Self {
        self.enable_prefix_tracking = enabled;
        self
    }

    pub fn with_violation_tracking(mut self, enabled: bool) -> Self {
        self.enable_violation_tracking = enabled;
        self
    }

    pub fn with_max_versions(mut self, max: u64) -> Self {
        self.max_versions_per_record = max;
        self
    }

    pub fn with_gc(mut self, enabled: bool) -> Self {
        self.gc_enabled = enabled;
        self
    }

    pub fn is_mvcc_enabled(&self) -> bool {
        self.enable_mvcc
    }

    pub fn is_prefix_tracking_enabled(&self) -> bool {
        self.enable_prefix_tracking
    }

    pub fn is_violation_tracking_enabled(&self) -> bool {
        self.enable_violation_tracking
    }

    pub fn is_gc_enabled(&self) -> bool {
        self.gc_enabled
    }
}

impl Default for ConsistencyConfig {
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

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_record_id() -> RecordId {
        RecordId::new([1; 32])
    }

    fn test_version_id() -> VersionId {
        VersionId::new([1; 32])
    }

    fn test_ordering_pos() -> OrderingPos {
        OrderingPos::new(1000)
    }

    // === ConsistencyError tests ===
    #[test]
    fn consistency_error_category() {
        assert_eq!(
            ConsistencyError::DatasetNotFound { dataset_id: test_dataset_id() }.category(),
            "dataset_not_found"
        );
        assert_eq!(
            ConsistencyError::PartitionNotFound {
                partition_id: test_partition_id(),
            }
            .category(),
            "partition_not_found"
        );
        assert_eq!(
            ConsistencyError::RecordNotFound { record_id: test_record_id() }.category(),
            "record_not_found"
        );
        assert_eq!(
            ConsistencyError::VersionNotFound { version_id: test_version_id() }.category(),
            "version_not_found"
        );
        assert_eq!(
            ConsistencyError::ProfileNotFound { dataset_id: test_dataset_id() }.category(),
            "profile_not_found"
        );
        assert_eq!(
            ConsistencyError::PositionOutOfOrder {
                expected: 100,
                got: 50,
            }
            .category(),
            "position_out_of_order"
        );
        assert_eq!(
            ConsistencyError::ReadNotVisible {
                position: 500,
                prefix: 1000,
            }
            .category(),
            "read_not_visible"
        );
        assert_eq!(
            ConsistencyError::VersionConflict {
                position: 100,
                existing: 200,
            }
            .category(),
            "version_conflict"
        );
        assert_eq!(
            ConsistencyError::InvalidProfile {
                profile: ConsistencyProfile::Strong,
            }
            .category(),
            "invalid_profile"
        );
        assert_eq!(
            ConsistencyError::MvccError {
                message: "corrupt".to_string(),
            }
            .category(),
            "mvcc_error"
        );
        assert_eq!(
            ConsistencyError::Internal {
                message: "panic".to_string(),
            }
            .category(),
            "internal"
        );
    }

    #[test]
    fn consistency_error_is_recoverable() {
        assert!(ConsistencyError::PositionOutOfOrder {
            expected: 100,
            got: 50,
        }
        .is_recoverable());
        assert!(ConsistencyError::ReadNotVisible {
            position: 500,
            prefix: 1000,
        }
        .is_recoverable());
        assert!(ConsistencyError::VersionConflict {
            position: 100,
            existing: 200,
        }
        .is_recoverable());
        assert!(!ConsistencyError::DatasetNotFound { dataset_id: test_dataset_id() }.is_recoverable());
        assert!(!ConsistencyError::VersionNotFound { version_id: test_version_id() }.is_recoverable());
    }

    #[test]
    fn consistency_error_suggested_action() {
        assert_eq!(
            ConsistencyError::DatasetNotFound { dataset_id: test_dataset_id() }.suggested_action(),
            "create dataset first"
        );
        assert_eq!(
            ConsistencyError::ReadNotVisible {
                position: 500,
                prefix: 1000,
            }
            .suggested_action(),
            "wait for commit to advance prefix"
        );
        assert_eq!(
            ConsistencyError::InvalidProfile {
                profile: ConsistencyProfile::Strong,
            }
            .suggested_action(),
            "use a valid consistency profile"
        );
        assert_eq!(
            ConsistencyError::Internal {
                message: "x".to_string(),
            }
            .suggested_action(),
            "contact support"
        );
    }

    // === VersionId tests ===
    #[test]
    fn version_id_zero_and_is_zero() {
        assert!(VersionId::zero().is_zero());
        assert!(!test_version_id().is_zero());
    }

    // === VersionRef tests ===
    #[test]
    fn version_ref_visible() {
        let mut vref = VersionRef::new(
            test_version_id(),
            test_ordering_pos(),
            vec![1, 2, 3],
            1000,
        );
        assert!(!vref.is_visible());
        assert_eq!(vref.value_len(), 3);
        vref.mark_visible();
        assert!(vref.is_visible());
        vref.mark_invisible();
        assert!(!vref.is_visible());
    }

    // === ConsistencyProfile tests ===
    #[test]
    fn consistency_profile_variants() {
        assert!(matches!(ConsistencyProfile::Strong, ConsistencyProfile::Strong));
        assert!(matches!(ConsistencyProfile::Eventual, ConsistencyProfile::Eventual));
        assert!(matches!(
            ConsistencyProfile::BoundedStaleness { .. },
            ConsistencyProfile::BoundedStaleness { .. }
        ));
        assert!(matches!(ConsistencyProfile::Session { .. }, ConsistencyProfile::Session { .. }));
        assert!(matches!(ConsistencyProfile::Prefix, ConsistencyProfile::Prefix));
    }

    // === ConsistencyProfileMgr tests ===
    #[test]
    fn profile_mgr_set_and_get() {
        let mgr = ConsistencyProfileMgr::default();
        mgr.set_profile(test_dataset_id(), ConsistencyProfile::Eventual).unwrap();
        assert_eq!(mgr.get_profile(&test_dataset_id()), ConsistencyProfile::Eventual);
        assert_eq!(mgr.get_profile(&DatasetId::new([2; 32])), ConsistencyProfile::Strong);
        assert_eq!(mgr.profile_count(), 1);
        assert_eq!(mgr.tracked_count(), 1);
    }

    #[test]
    fn profile_mgr_remove_and_change() {
        let mgr = ConsistencyProfileMgr::default();
        mgr.set_profile(test_dataset_id(), ConsistencyProfile::Eventual).unwrap();
        assert!(mgr.remove_profile(&test_dataset_id()));
        assert!(!mgr.remove_profile(&test_dataset_id()));
        assert_eq!(mgr.change_count(), 2);
    }

    // === CommittedPrefixTracker tests ===
    #[test]
    fn prefix_tracker_update_and_get() {
        let cpt = CommittedPrefixTracker::default();
        cpt.update_prefix(test_dataset_id(), test_ordering_pos()).unwrap();
        assert_eq!(cpt.get_prefix(&test_dataset_id()), test_ordering_pos());
        assert_eq!(cpt.prefix_update_count(), 1);
    }

    #[test]
    fn prefix_tracker_out_of_order() {
        let cpt = CommittedPrefixTracker::default();
        cpt.update_prefix(test_dataset_id(), OrderingPos::new(1000)).unwrap();
        let err = cpt
            .update_prefix(test_dataset_id(), OrderingPos::new(500))
            .unwrap_err();
        assert!(matches!(err, ConsistencyError::PositionOutOfOrder { .. }));
    }

    #[test]
    fn prefix_tracker_advance() {
        let cpt = CommittedPrefixTracker::default();
        cpt.advance_prefix(test_dataset_id(), 500).unwrap();
        assert_eq!(cpt.get_prefix(&test_dataset_id()), OrderingPos::new(500));
        assert_eq!(cpt.max_position_seen(), 500);
    }

    // === MVCCManager tests ===
    #[test]
    fn mvcc_create_and_read() {
        let mvcc = MVCCManager::default();
        let vid = mvcc
            .create_version(
                test_dataset_id(),
                test_partition_id(),
                test_record_id(),
                test_ordering_pos(),
                vec![1, 2, 3],
            )
            .unwrap();
        assert!(!vid.is_zero());
        let vref = mvcc
            .read_version(
                &test_dataset_id(),
                &test_partition_id(),
                &test_record_id(),
                test_ordering_pos(),
            )
            .unwrap();
        assert!(vref.is_some());
        assert_eq!(vref.unwrap().version_id, vid);
    }

    #[test]
    fn mvcc_conflict() {
        let mvcc = MVCCManager::default();
        mvcc.create_version(
            test_dataset_id(),
            test_partition_id(),
            test_record_id(),
            OrderingPos::new(100),
            vec![1],
        )
        .unwrap();
        let err = mvcc
            .create_version(
                test_dataset_id(),
                test_partition_id(),
                test_record_id(),
                OrderingPos::new(50),
                vec![2],
            )
            .unwrap_err();
        assert!(matches!(err, ConsistencyError::VersionConflict { .. }));
        assert_eq!(mvcc.conflict_count(), 1);
    }

    #[test]
    fn mvcc_counts() {
        let mvcc = MVCCManager::default();
        mvcc.create_version(
            test_dataset_id(),
            test_partition_id(),
            test_record_id(),
            test_ordering_pos(),
            vec![1],
        )
        .unwrap();
        mvcc.read_version(
            &test_dataset_id(),
            &test_partition_id(),
            &test_record_id(),
            test_ordering_pos(),
        )
        .unwrap();
        assert_eq!(mvcc.create_count(), 1);
        assert_eq!(mvcc.read_count(), 1);
        assert_eq!(mvcc.total_version_count(), 1);
        assert_eq!(mvcc.record_count(), 1);
    }

    // === VersionStore tests ===
    #[test]
    fn version_store_store_and_get() {
        let vs = VersionStore::default();
        let vref = VersionRef::new(test_version_id(), test_ordering_pos(), vec![1], 1000);
        vs.store(vref.clone()).unwrap();
        let got = vs.get(&test_version_id()).unwrap();
        assert_eq!(got.version_id, test_version_id());
        assert_eq!(vs.store_count(), 1);
        assert_eq!(vs.stored_count(), 1);
    }

    #[test]
    fn version_store_capacity() {
        let vs = VersionStore::new(1);
        vs.store(VersionRef::new(
            test_version_id(),
            test_ordering_pos(),
            vec![1],
            1000,
        ))
        .unwrap();
        let err = vs.store(VersionRef::new(
            VersionId::new([2; 32]),
            test_ordering_pos(),
            vec![2],
            1000,
        ));
        assert!(err.is_err());
        assert_eq!(vs.evict_count(), 1);
        assert!(vs.is_at_capacity());
    }

    // === ReadVisibilityManager tests ===
    #[test]
    fn read_visibility_check() {
        let rvm = ReadVisibilityManager::default();
        rvm.set_rule(test_dataset_id(), ConsistencyProfile::Strong);
        let visible = rvm.check_visibility(
            &test_dataset_id(),
            OrderingPos::new(500),
            OrderingPos::new(1000),
        );
        assert!(visible);
        assert_eq!(rvm.visibility_check_count(), 1);
        assert_eq!(rvm.visible_count(), 1);
    }

    #[test]
    fn read_visibility_invisible() {
        let rvm = ReadVisibilityManager::default();
        rvm.set_rule(test_dataset_id(), ConsistencyProfile::Strong);
        let visible = rvm.check_visibility(
            &test_dataset_id(),
            OrderingPos::new(1500),
            OrderingPos::new(1000),
        );
        assert!(!visible);
        assert_eq!(rvm.invisible_count(), 1);
        assert!((rvm.visibility_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn read_visibility_eventual() {
        let rvm = ReadVisibilityManager::default();
        rvm.set_rule(test_dataset_id(), ConsistencyProfile::Eventual);
        assert!(rvm.check_visibility(
            &test_dataset_id(),
            OrderingPos::new(5000),
            OrderingPos::new(1000)
        ));
        assert_eq!(rvm.rule_count(), 1);
    }

    // === WriteOrderingManager tests ===
    #[test]
    fn write_ordering_record() {
        let wom = WriteOrderingManager::default();
        wom.record_write_position(test_dataset_id(), OrderingPos::new(100)).unwrap();
        wom.record_write_position(test_dataset_id(), OrderingPos::new(200)).unwrap();
        assert_eq!(wom.write_count(), 2);
        assert_eq!(wom.max_position(), 200);
        assert_eq!(wom.last_position(&test_dataset_id()), Some(OrderingPos::new(200)));
    }

    #[test]
    fn write_ordering_out_of_order() {
        let wom = WriteOrderingManager::default();
        wom.record_write_position(test_dataset_id(), OrderingPos::new(200)).unwrap();
        let err = wom
            .record_write_position(test_dataset_id(), OrderingPos::new(100))
            .unwrap_err();
        assert!(matches!(err, ConsistencyError::PositionOutOfOrder { .. }));
        assert_eq!(wom.out_of_order_count(), 1);
    }

    // === ConsistencyChecker tests ===
    #[test]
    fn consistency_check_result_as_str() {
        assert_eq!(ConsistencyCheckResult::Consistent.as_str(), "consistent");
        assert_eq!(ConsistencyCheckResult::Stale.as_str(), "stale");
        assert_eq!(ConsistencyCheckResult::Diverged.as_str(), "diverged");
        assert_eq!(ConsistencyCheckResult::Unknown.as_str(), "unknown");
        assert!(ConsistencyCheckResult::Consistent.is_ok());
        assert!(!ConsistencyCheckResult::Diverged.is_ok());
    }

    #[test]
    fn consistency_checker_check() {
        let cc = ConsistencyChecker::default();
        let result = cc.check(
            test_dataset_id(),
            vec![OrderingPos::new(100), OrderingPos::new(100)],
        );
        assert_eq!(result, ConsistencyCheckResult::Consistent);
        assert_eq!(cc.check_count(), 1);
        assert_eq!(cc.consistent_count(), 1);
    }

    #[test]
    fn consistency_checker_diverged() {
        let cc = ConsistencyChecker::default();
        let result = cc.check(
            test_dataset_id(),
            vec![OrderingPos::new(100), OrderingPos::new(5000)],
        );
        assert_eq!(result, ConsistencyCheckResult::Diverged);
        assert_eq!(cc.diverged_count(), 1);
        assert!((cc.consistency_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn consistency_checker_unknown() {
        let cc = ConsistencyChecker::default();
        let result = cc.check(test_dataset_id(), vec![]);
        assert_eq!(result, ConsistencyCheckResult::Unknown);
    }

    // === PrefixValidator tests ===
    #[test]
    fn prefix_validate_valid_and_invalid() {
        let pv = PrefixValidator::default();
        assert!(pv.validate(
            test_dataset_id(),
            OrderingPos::new(500),
            OrderingPos::new(1000)
        ));
        assert!(!pv.validate(
            test_dataset_id(),
            OrderingPos::new(1500),
            OrderingPos::new(1000)
        ));
        assert_eq!(pv.validation_count(), 2);
        assert_eq!(pv.valid_count(), 1);
        assert_eq!(pv.invalid_count(), 1);
        assert!((pv.valid_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn prefix_get_validated() {
        let pv = PrefixValidator::default();
        pv.validate(test_dataset_id(), OrderingPos::new(500), OrderingPos::new(1000));
        assert_eq!(
            pv.get_validated_prefix(&test_dataset_id()),
            Some(OrderingPos::new(500))
        );
        assert_eq!(pv.validated_count(), 1);
    }

    // === ConsistencyStats tests ===
    #[test]
    fn consistency_stats_health() {
        let mut s = ConsistencyStats::new();
        s.consistency_checks = 10;
        s.consistent_results = 8;
        s.total_writes = 100;
        s.total_conflicts = 5;
        assert!((s.consistency_health() - 0.8).abs() < 0.01);
        assert!((s.conflict_rate() - 0.05).abs() < 0.01);
    }

    #[test]
    fn consistency_stats_default() {
        let s = ConsistencyStats::default();
        assert_eq!(s.total_profiles, 0);
        assert_eq!(s.consistency_health(), 1.0);
        assert_eq!(s.conflict_rate(), 0.0);
    }

    // === ConsistencyMonitor tests ===
    #[test]
    fn consistency_monitor_report_health() {
        let cm = ConsistencyMonitor::default();
        cm.report_health(test_dataset_id(), ConsistencyCheckResult::Consistent);
        cm.report_health(test_dataset_id(), ConsistencyCheckResult::Diverged);
        cm.report_health(test_dataset_id(), ConsistencyCheckResult::Consistent);
        assert_eq!(cm.alert_count(), 1);
        assert_eq!(cm.recovery_count(), 1);
        assert_eq!(cm.monitored_count(), 1);
        assert_eq!(cm.unhealthy_count(), 0);
    }

    #[test]
    fn consistency_monitor_get_health() {
        let cm = ConsistencyMonitor::default();
        cm.report_health(test_dataset_id(), ConsistencyCheckResult::Stale);
        assert_eq!(
            cm.get_health(&test_dataset_id()),
            Some(ConsistencyCheckResult::Stale)
        );
        assert_eq!(cm.get_health(&DatasetId::new([2; 32])), None);
    }

    // === ConsistencyEngine tests ===
    #[test]
    fn consistency_engine_set_get_profile() {
        let engine = ConsistencyEngine::default();
        engine
            .set_profile(test_dataset_id(), ConsistencyProfile::Eventual)
            .unwrap();
        assert_eq!(engine.get_profile(&test_dataset_id()), ConsistencyProfile::Eventual);
        assert_eq!(engine.profile_count(), 1);
    }

    #[test]
    fn consistency_engine_record_write_and_prefix() {
        let engine = ConsistencyEngine::default();
        engine
            .record_write(test_dataset_id(), test_ordering_pos(), test_node_id())
            .unwrap();
        assert_eq!(
            engine.committed_prefix(&test_dataset_id()),
            test_ordering_pos()
        );
        assert_eq!(engine.stats().total_writes, 1);
    }

    #[test]
    fn consistency_engine_record_write_out_of_order() {
        let engine = ConsistencyEngine::default();
        engine
            .record_write(test_dataset_id(), OrderingPos::new(1000), test_node_id())
            .unwrap();
        let err = engine
            .record_write(test_dataset_id(), OrderingPos::new(500), test_node_id())
            .unwrap_err();
        assert!(matches!(err, ConsistencyError::PositionOutOfOrder { .. }));
    }

    #[test]
    fn consistency_engine_read_visible() {
        let engine = ConsistencyEngine::default();
        engine
            .record_write(test_dataset_id(), OrderingPos::new(1000), test_node_id())
            .unwrap();
        assert!(engine
            .is_read_visible(&test_dataset_id(), OrderingPos::new(500))
            .unwrap());
        let err = engine
            .is_read_visible(&test_dataset_id(), OrderingPos::new(2000))
            .unwrap_err();
        assert!(matches!(err, ConsistencyError::ReadNotVisible { .. }));
    }

    #[test]
    fn consistency_engine_create_and_read_version() {
        let engine = ConsistencyEngine::default();
        let vid = engine
            .create_version(
                test_dataset_id(),
                test_partition_id(),
                test_record_id(),
                test_ordering_pos(),
                vec![1, 2, 3],
            )
            .unwrap();
        let vref = engine
            .read_version(
                &test_dataset_id(),
                &test_partition_id(),
                &test_record_id(),
                test_ordering_pos(),
            )
            .unwrap();
        assert!(vref.is_some());
        assert_eq!(vref.unwrap().version_id, vid);
        assert_eq!(engine.version_count(), 1);
    }

    #[test]
    fn consistency_engine_stats() {
        let engine = ConsistencyEngine::default();
        engine
            .record_write(test_dataset_id(), OrderingPos::new(100), test_node_id())
            .unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_writes, 1);
    }

    #[test]
    fn consistency_engine_check_consistency() {
        let engine = ConsistencyEngine::default();
        let result = engine.check_consistency(
            test_dataset_id(),
            vec![OrderingPos::new(100), OrderingPos::new(100)],
        );
        assert_eq!(result, ConsistencyCheckResult::Consistent);
        assert_eq!(engine.stats().consistency_checks, 1);
        assert_eq!(engine.stats().consistent_results, 1);
    }

    #[test]
    fn consistency_engine_validate_prefix() {
        let engine = ConsistencyEngine::default();
        assert!(engine.validate_prefix(
            test_dataset_id(),
            OrderingPos::new(500),
            OrderingPos::new(1000)
        ));
        assert!(!engine.validate_prefix(
            test_dataset_id(),
            OrderingPos::new(1500),
            OrderingPos::new(1000)
        ));
    }

    #[test]
    fn consistency_engine_store_and_get_version() {
        let engine = ConsistencyEngine::default();
        let vref = VersionRef::new(test_version_id(), test_ordering_pos(), vec![1], 1000);
        engine.store_version(vref.clone()).unwrap();
        let got = engine.get_stored_version(&test_version_id()).unwrap();
        assert_eq!(got.version_id, test_version_id());
    }

    #[test]
    fn consistency_engine_name_and_counts() {
        let engine = ConsistencyEngine::default();
        assert_eq!(engine.consistency_name(), "consistency_engine");
        assert_eq!(engine.profile_count(), 0);
        assert_eq!(engine.version_count(), 0);
        assert_eq!(engine.record_count(), 0);
    }

    // === OrderingPos tests ===
    #[test]
    fn ordering_pos_as_u64() {
        assert_eq!(OrderingPos::new(42).as_u64(), 42);
        assert_eq!(OrderingPos::new(0).as_u64(), 0);
    }

    // === RecordId tests ===
    #[test]
    fn record_id_is_zero() {
        assert!(RecordId::new([0; 32]).is_zero());
        assert!(!test_record_id().is_zero());
    }
// === MVCCGarbageCollectionPolicy tests ===
#[test]
fn mvcc_gc_policy_as_str() {
    assert_eq!(MVCCGarbageCollectionPolicy::KeepAll.as_str(), "keep_all");
    assert_eq!(
        MVCCGarbageCollectionPolicy::KeepLastN { count: 10 }.as_str(),
        "keep_last_n"
    );
    assert_eq!(
        MVCCGarbageCollectionPolicy::KeepByTime { retention_ms: 5000 }.as_str(),
        "keep_by_time"
    );
    assert_eq!(
        MVCCGarbageCollectionPolicy::KeepByVersions { count: 5 }.as_str(),
        "keep_by_versions"
    );
}

// === MVCCGarbageCollector tests ===
#[test]
fn mvcc_garbage_collector_run_gc() {
    let gc = MVCCGarbageCollector::default();
    let collected = gc.run_gc(100000);
    assert!(collected > 0);
    assert_eq!(gc.gc_run_count(), 1);
    assert!(gc.last_gc_ms().is_some());
}

#[test]
fn mvcc_garbage_collector_keep_all() {
    let gc = MVCCGarbageCollector::new(MVCCGarbageCollectionPolicy::KeepAll);
    assert_eq!(gc.run_gc(100000), 0);
    assert_eq!(gc.collected_count(), 0);
}

#[test]
fn mvcc_garbage_collector_counts() {
    let gc = MVCCGarbageCollector::default();
    gc.run_gc(50000);
    gc.run_gc(100000);
    assert_eq!(gc.gc_run_count(), 2);
    assert!(gc.collected_count() > 0);
}

// === SnapshotIsolationLevel tests ===
#[test]
fn snapshot_isolation_level_as_str() {
    assert_eq!(SnapshotIsolationLevel::None.as_str(), "none");
    assert_eq!(SnapshotIsolationLevel::ReadCommitted.as_str(), "read_committed");
    assert_eq!(
        SnapshotIsolationLevel::RepeatableRead.as_str(),
        "repeatable_read"
    );
    assert_eq!(SnapshotIsolationLevel::Serializable.as_str(), "serializable");
}

#[test]
fn snapshot_isolation_level_requires_and_strength() {
    assert!(!SnapshotIsolationLevel::None.requires_snapshot());
    assert!(!SnapshotIsolationLevel::ReadCommitted.requires_snapshot());
    assert!(SnapshotIsolationLevel::RepeatableRead.requires_snapshot());
    assert!(SnapshotIsolationLevel::Serializable.requires_snapshot());
    assert_eq!(SnapshotIsolationLevel::None.isolation_strength(), 0);
    assert_eq!(SnapshotIsolationLevel::Serializable.isolation_strength(), 3);
}

// === SnapshotIsolationManager tests ===
#[test]
fn snapshot_isolation_begin_commit_abort() {
    let sim = SnapshotIsolationManager::default();
    let id = sim.begin_snapshot(OrderingPos::new(100));
    assert_eq!(sim.active_count(), 1);
    assert_eq!(sim.get_snapshot_position(id), Some(OrderingPos::new(100)));
    assert!(sim.commit_snapshot(id));
    assert_eq!(sim.active_count(), 0);
    assert_eq!(sim.committed_count(), 1);
}

#[test]
fn snapshot_isolation_abort_and_counts() {
    let sim = SnapshotIsolationManager::default();
    let id = sim.begin_snapshot(OrderingPos::new(200));
    assert!(sim.abort_snapshot(id));
    assert_eq!(sim.active_count(), 0);
    assert_eq!(sim.aborted_count(), 1);
    assert!(!sim.commit_snapshot(id));
    assert!(!sim.abort_snapshot(id));
    assert_eq!(sim.total_snapshots(), 1);
}

// === ConflictResolutionStrategy tests ===
#[test]
fn conflict_resolution_strategy_as_str() {
    assert_eq!(
        ConflictResolutionStrategy::LastWriteWins.as_str(),
        "last_write_wins"
    );
    assert_eq!(
        ConflictResolutionStrategy::FirstWriteWins.as_str(),
        "first_write_wins"
    );
    assert_eq!(
        ConflictResolutionStrategy::HighestVersionWins.as_str(),
        "highest_version_wins"
    );
    assert_eq!(ConflictResolutionStrategy::Merge.as_str(), "merge");
    assert_eq!(ConflictResolutionStrategy::Manual.as_str(), "manual");
}

#[test]
fn conflict_resolution_strategy_auto() {
    assert!(ConflictResolutionStrategy::LastWriteWins.resolves_automatically());
    assert!(ConflictResolutionStrategy::Merge.resolves_automatically());
    assert!(!ConflictResolutionStrategy::Manual.resolves_automatically());
}

// === ConflictResolver tests ===
#[test]
fn conflict_resolver_detect_and_resolve() {
    let cr = ConflictResolver::default();
    assert_eq!(cr.detect_conflict(), 1);
    assert_eq!(cr.conflicts_detected(), 1);
    assert_eq!(cr.conflicts_pending(), 1);
    assert!(cr.resolve_conflict());
    assert_eq!(cr.conflicts_resolved(), 1);
    assert_eq!(cr.conflicts_pending(), 0);
    assert!(!cr.resolve_conflict());
}

#[test]
fn conflict_resolver_auto_and_rate() {
    let cr = ConflictResolver::default();
    cr.detect_conflict();
    cr.detect_conflict();
    cr.auto_resolve();
    cr.auto_resolve();
    assert_eq!(cr.conflicts_resolved(), 2);
    assert!((cr.resolution_rate() - 1.0).abs() < 0.01);
    assert_eq!(cr.history_len(), 2);
}

// === ReadConsistencyLevel tests ===
#[test]
fn read_consistency_level_as_str() {
    assert_eq!(
        ReadConsistencyLevel::ReadUncommitted.as_str(),
        "read_uncommitted"
    );
    assert_eq!(ReadConsistencyLevel::ReadCommitted.as_str(), "read_committed");
    assert_eq!(
        ReadConsistencyLevel::RepeatableRead.as_str(),
        "repeatable_read"
    );
    assert_eq!(ReadConsistencyLevel::Serializable.as_str(), "serializable");
}

#[test]
fn read_consistency_level_prefix_and_strength() {
    assert!(!ReadConsistencyLevel::ReadUncommitted.requires_prefix_check());
    assert!(ReadConsistencyLevel::ReadCommitted.requires_prefix_check());
    assert!(ReadConsistencyLevel::Serializable.requires_prefix_check());
    assert_eq!(ReadConsistencyLevel::ReadUncommitted.strength(), 0);
    assert_eq!(ReadConsistencyLevel::Serializable.strength(), 3);
}

// === ReadConsistencyLevelManager tests ===
#[test]
fn read_consistency_manager_set_and_get() {
    let mgr = ReadConsistencyLevelManager::default();
    mgr.set_level(test_dataset_id(), ReadConsistencyLevel::Serializable);
    assert_eq!(
        mgr.get_level(&test_dataset_id()),
        ReadConsistencyLevel::Serializable
    );
    assert_eq!(
        mgr.get_level(&DatasetId::new([2; 32])),
        ReadConsistencyLevel::ReadCommitted
    );
}

#[test]
fn read_consistency_manager_record_and_rates() {
    let mgr = ReadConsistencyLevelManager::default();
    mgr.record_read(&test_dataset_id());
    mgr.record_read(&test_dataset_id());
    mgr.record_prefix_violation();
    assert_eq!(mgr.reads_performed(), 2);
    assert_eq!(mgr.prefix_checks_performed(), 2);
    assert_eq!(mgr.prefix_violations(), 1);
    assert!((mgr.violation_rate() - 0.5).abs() < 0.01);
}

// === WriteAheadConsistencyManager tests ===
#[test]
fn write_ahead_enqueue_and_commit() {
    let wac = WriteAheadConsistencyManager::new(3);
    assert!(wac.enqueue_write(test_dataset_id(), OrderingPos::new(10), 1000));
    assert!(wac.enqueue_write(test_dataset_id(), OrderingPos::new(20), 2000));
    assert_eq!(wac.pending_write_count(), 2);
    let committed = wac.commit_write().unwrap();
    assert_eq!(committed.1, OrderingPos::new(10));
    assert_eq!(wac.committed_count(), 1);
}

#[test]
fn write_ahead_limit_and_barrier() {
    let wac = WriteAheadConsistencyManager::new(1);
    assert!(wac.enqueue_write(test_dataset_id(), OrderingPos::new(1), 1000));
    assert!(!wac.enqueue_write(test_dataset_id(), OrderingPos::new(2), 2000));
    assert!(wac.is_at_pending_limit());
    assert_eq!(wac.barrier_count(), 0);
    wac.write_barrier();
    assert_eq!(wac.barrier_count(), 1);
}

// === ViolationType tests ===
#[test]
fn violation_type_as_str() {
    assert_eq!(ViolationType::StaleRead.as_str(), "stale_read");
    assert_eq!(ViolationType::LostUpdate.as_str(), "lost_update");
    assert_eq!(ViolationType::WriteSkew.as_str(), "write_skew");
    assert_eq!(ViolationType::PhantomRead.as_str(), "phantom_read");
    assert_eq!(ViolationType::DirtyRead.as_str(), "dirty_read");
    assert_eq!(ViolationType::NonRepeatableRead.as_str(), "non_repeatable_read");
}

// === ConsistencyViolation tests ===
#[test]
fn consistency_violation_new_and_resolve() {
    let cv = ConsistencyViolation::new(
        test_dataset_id(),
        ViolationType::StaleRead,
        OrderingPos::new(100),
        5000,
    );
    assert!(!cv.is_resolved());
    let mut cv = cv;
    cv.mark_resolved();
    assert!(cv.is_resolved());
}

// === ConsistencyEngine extended tests ===
#[test]
fn consistency_engine_gc_and_snapshot() {
    let engine = ConsistencyEngine::default();
    let collected = engine.mvcc_garbage_collect(100000);
    assert!(collected > 0);
    assert_eq!(engine.gc_run_count(), 1);
    let sid = engine.begin_snapshot(OrderingPos::new(500));
    assert_eq!(engine.snapshot_active_count(), 1);
    assert!(engine.commit_snapshot(sid));
    assert_eq!(engine.snapshot_committed_count(), 1);
}

#[test]
fn consistency_engine_conflict_and_read_level() {
    let engine = ConsistencyEngine::default();
    assert_eq!(engine.detect_conflict(), 1);
    assert!(engine.resolve_conflict());
    assert!((engine.conflict_resolution_rate() - 1.0).abs() < 0.01);
    engine.set_read_level(test_dataset_id(), ReadConsistencyLevel::Serializable);
    assert_eq!(
        engine.get_read_level(&test_dataset_id()),
        ReadConsistencyLevel::Serializable
    );
    engine.record_read(&test_dataset_id());
    assert_eq!(engine.reads_performed(), 1);
}

#[test]
fn consistency_engine_pending_write_and_barrier() {
    let engine = ConsistencyEngine::default();
    assert!(engine.enqueue_pending_write(test_dataset_id(), OrderingPos::new(1), 1000));
    assert_eq!(engine.pending_write_count(), 1);
    let committed = engine.commit_pending_write().unwrap();
    assert_eq!(committed.1, OrderingPos::new(1));
    engine.write_barrier();
    assert_eq!(engine.write_barrier(), 1);
}

#[test]
fn consistency_engine_violation_tracking() {
    let engine = ConsistencyEngine::default();
    engine.record_violation(
        test_dataset_id(),
        ViolationType::DirtyRead,
        OrderingPos::new(100),
        5000,
    );
    assert_eq!(engine.violation_count(), 1);
    assert_eq!(engine.resolved_violation_count(), 0);
    assert!(engine.resolve_violation(0));
    assert_eq!(engine.resolved_violation_count(), 1);
    let vs = engine.violations_by_type(ViolationType::DirtyRead);
    assert_eq!(vs.len(), 1);
    let vs2 = engine.violations_by_type(ViolationType::StaleRead);
    assert_eq!(vs2.len(), 0);
}

// === ConsistencyConfig tests ===
#[test]
fn consistency_config_default_and_builder() {
    let config = ConsistencyConfig::default();
    assert_eq!(config.default_profile, ConsistencyProfile::Strong);
    assert!(config.is_mvcc_enabled());
    assert!(config.is_prefix_tracking_enabled());
    assert!(config.is_violation_tracking_enabled());
    assert!(config.is_gc_enabled());
    assert_eq!(config.max_versions_per_record, 1000);
}

#[test]
fn consistency_config_builder_methods() {
    let config = ConsistencyConfig::new()
        .with_default_profile(ConsistencyProfile::Eventual)
        .with_mvcc(false)
        .with_prefix_tracking(false)
        .with_violation_tracking(false)
        .with_max_versions(500)
        .with_gc(false);
    assert_eq!(config.default_profile, ConsistencyProfile::Eventual);
    assert!(!config.is_mvcc_enabled());
    assert!(!config.is_prefix_tracking_enabled());
    assert!(!config.is_violation_tracking_enabled());
    assert_eq!(config.max_versions_per_record, 500);
    assert!(!config.is_gc_enabled());
}
}
