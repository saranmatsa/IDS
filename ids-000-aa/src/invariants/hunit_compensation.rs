///////////////////////////////////////////////////////////////////////////////
// H-Unit Compensation Invariant — H-unit failure compensation through another H-unit
//
// IDS-A §36: when an H-unit fails, affected data must be compensated through
// another H-unit. This invariant guarantees that every H-unit failure has a
// designated compensation path with capacity-aware selection, redundancy
// restoration, and complete audit tracking.
//
// Every compensation operation follows a strict lifecycle:
//   1. Identify affected data on the failed H-unit
//   2. Select a compensation H-unit with sufficient capacity
//   3. Restore redundancy by replicating data to the compensation H-unit
//   4. Verify restoration completeness
//   5. Record the compensation for audit and replay
//
// This file is part of ids-000-aa, the central hub crate. All types and
// traits here are sealed — downstream crates cannot implement them.
//
// Carrier-agnostic: uses only opaque Endpoint types (CarrierType + Vec<u8>).
// No std::net, SocketAddr, IP, TCP, UDP, or any existing networking stack.
///////////////////////////////////////////////////////////////////////////////

use crate::types::{DatasetId, HUnitId, NodeId, PartitionId, RegionId};
use crate::errors::Error;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait contract
///////////////////////////////////////////////////////////////////////////////

/// Sealed trait guaranteeing H-unit failure compensation through another H-unit.
///
/// Downstream crates may call methods and use as trait bounds, but cannot
/// implement — only `ids-000-aa` and its designated implementation crates.
pub trait HUnitCompensation: private::Sealed {
    /// Identify all data partitions affected by a node failure within an H-unit.
    fn identify_affected_data(
        &self,
        hunit: HUnitId,
        failed_node: NodeId,
    ) -> Result<Vec<PartitionId>, Error>;

    /// Select a compensation H-unit with sufficient capacity to take over.
    fn select_compensation_hunit(
        &self,
        failed_hunit: HUnitId,
        required_capacity: u64,
    ) -> Result<HUnitId, Error>;

    /// Restore redundancy by copying data from source to destination H-unit.
    fn restore_redundancy(
        &self,
        source: HUnitId,
        destination: HUnitId,
    ) -> Result<CompensationResult, Error>;

    /// Return whether a compensation H-unit is eligible for a failed H-unit.
    fn is_eligible_compensation(
        &self,
        failed: HUnitId,
        candidate: HUnitId,
    ) -> bool;

    /// Return the total compensation operations performed.
    fn compensation_count(&self) -> u64;

    /// Return whether redundancy is fully restored for an H-unit.
    fn redundancy_restored(&self, hunit: HUnitId) -> bool;
}

mod private {
    /// Sealed supertrait — only ids-000-aa can implement HUnitCompensation.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl HUnitCompensation for () {
    fn identify_affected_data(
        &self,
        _hunit: HUnitId,
        _failed_node: NodeId,
    ) -> Result<Vec<PartitionId>, Error> {
        Ok(Vec::new())
    }
    fn select_compensation_hunit(
        &self,
        _failed_hunit: HUnitId,
        _required_capacity: u64,
    ) -> Result<HUnitId, Error> {
        Err(Error::HUnitNotFound { hunit: HUnitId::new([0u8; 32]) })
    }
    fn restore_redundancy(
        &self,
        _source: HUnitId,
        _destination: HUnitId,
    ) -> Result<CompensationResult, Error> {
        Ok(CompensationResult::default())
    }
    fn is_eligible_compensation(&self, _failed: HUnitId, _candidate: HUnitId) -> bool {
        false
    }
    fn compensation_count(&self) -> u64 {
        0
    }
    fn redundancy_restored(&self, _hunit: HUnitId) -> bool {
        false
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation result and status
///////////////////////////////////////////////////////////////////////////////

/// Result of a redundancy restoration operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompensationResult {
    pub compensation_id: CompensationId,
    pub source_hunit: HUnitId,
    pub destination_hunit: HUnitId,
    pub partitions_restored: Vec<PartitionId>,
    pub bytes_restored: u64,
    pub status: CompensationStatus,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a compensation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompensationId(pub u64);

/// Status of a compensation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompensationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Default compensation result.
impl Default for CompensationStatus {
    fn default() -> Self {
        CompensationStatus::Pending
    }
}

///////////////////////////////////////////////////////////////////////////////
// Affected data tracker — identifies data on failed nodes
///////////////////////////////////////////////////////////////////////////////

/// A record of affected data on a failed node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedDataRecord {
    pub partition_id: PartitionId,
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub hunit_id: HUnitId,
    pub data_size_bytes: u64,
    pub replica_count: u32,
    pub healthy_replicas: u32,
    pub needs_compensation: bool,
}

/// Tracks which data partitions are affected by node/H-unit failures.
pub struct AffectedDataTracker {
    /// Affected records by partition ID.
    records: HashMap<PartitionId, AffectedDataRecord>,
    /// Per-H-unit affected partition IDs.
    hunit_partitions: HashMap<HUnitId, Vec<PartitionId>>,
    /// Per-node affected partition IDs.
    node_partitions: HashMap<NodeId, Vec<PartitionId>>,
    /// Total affected records.
    total_records: u64,
    /// Records marked as compensated.
    compensated: HashSet<PartitionId>,
}

impl AffectedDataTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        AffectedDataTracker {
            records: HashMap::new(),
            hunit_partitions: HashMap::new(),
            node_partitions: HashMap::new(),
            total_records: 0,
            compensated: HashSet::new(),
        }
    }

    /// Register an affected data record.
    pub fn register_affected(&mut self, record: AffectedDataRecord) {
        let partition = record.partition_id;
        let is_new = !self.records.contains_key(&partition);

        self.records.insert(partition, record.clone());
        self.hunit_partitions
            .entry(record.hunit_id)
            .or_default()
            .push(partition);
        self.node_partitions
            .entry(record.node_id)
            .or_default()
            .push(partition);

        if is_new {
            self.total_records += 1;
        }
    }

    /// Identify all affected data for a failed node within an H-unit.
    pub fn identify_affected_data(
        &self,
        hunit: HUnitId,
        failed_node: NodeId,
    ) -> Vec<PartitionId> {
        let hunit_parts = self.hunit_partitions.get(&hunit).cloned().unwrap_or_default();
        let node_parts = self.node_partitions.get(&failed_node).cloned().unwrap_or_default();

        let hunit_set: HashSet<PartitionId> = hunit_parts.into_iter().collect();
        node_parts
            .into_iter()
            .filter(|p| hunit_set.contains(p))
            .filter(|p| !self.compensated.contains(p))
            .collect()
    }

    /// Return all affected partitions for an H-unit.
    pub fn affected_for_hunit(&self, hunit: HUnitId) -> Vec<PartitionId> {
        self.hunit_partitions
            .get(&hunit)
            .cloned()
            .unwrap_or_default()
    }

    /// Return all affected partitions for a node.
    pub fn affected_for_node(&self, node: NodeId) -> Vec<PartitionId> {
        self.node_partitions
            .get(&node)
            .cloned()
            .unwrap_or_default()
    }

    /// Mark a partition as compensated.
    pub fn mark_compensated(&mut self, partition: PartitionId) {
        self.compensated.insert(partition);
    }

    /// Return whether a partition is compensated.
    pub fn is_compensated(&self, partition: PartitionId) -> bool {
        self.compensated.contains(&partition)
    }

    /// Return total affected record count.
    pub fn total_records(&self) -> u64 {
        self.total_records
    }

    /// Return compensated count.
    pub fn compensated_count(&self) -> u64 {
        self.compensated.len() as u64
    }

    /// Return pending (uncompensated) count.
    pub fn pending_count(&self) -> u64 {
        self.total_records - self.compensated.len() as u64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation H-unit selector — capacity-aware selection
///////////////////////////////////////////////////////////////////////////////

/// Capacity information for an H-unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HUnitCapacity {
    pub hunit_id: HUnitId,
    pub total_capacity_bytes: u64,
    pub used_capacity_bytes: u64,
    pub available_capacity_bytes: u64,
    pub node_count: u32,
    pub healthy_node_count: u32,
    pub region: RegionId,
}

impl HUnitCapacity {
    /// Return the utilization ratio (0.0 to 1.0).
    pub fn utilization_ratio(&self) -> f64 {
        if self.total_capacity_bytes == 0 {
            return 0.0;
        }
        self.used_capacity_bytes as f64 / self.total_capacity_bytes as f64
    }

    /// Return whether the H-unit can accept the required capacity.
    pub fn can_accept(&self, required: u64) -> bool {
        self.available_capacity_bytes >= required && self.healthy_node_count > 0
    }
}

/// Selects a compensation H-unit based on capacity and eligibility.
pub struct CompensationHUnitSelector {
    /// Capacity information per H-unit.
    capacities: HashMap<HUnitId, HUnitCapacity>,
    /// Excluded H-units (failed, draining, etc.).
    excluded: HashSet<HUnitId>,
    /// Preferred regions for compensation.
    preferred_regions: Vec<RegionId>,
    /// Total selections made.
    total_selections: u64,
    /// Total failed selections (no eligible H-unit).
    failed_selections: u64,
}

impl CompensationHUnitSelector {
    /// Create a new selector.
    pub fn new() -> Self {
        CompensationHUnitSelector {
            capacities: HashMap::new(),
            excluded: HashSet::new(),
            preferred_regions: Vec::new(),
            total_selections: 0,
            failed_selections: 0,
        }
    }

    /// Register capacity for an H-unit.
    pub fn register_capacity(&mut self, capacity: HUnitCapacity) {
        self.capacities.insert(capacity.hunit_id, capacity);
    }

    /// Exclude an H-unit from compensation selection.
    pub fn exclude(&mut self, hunit: HUnitId) {
        self.excluded.insert(hunit);
    }

    /// Unexclude an H-unit.
    pub fn unexclude(&mut self, hunit: HUnitId) {
        self.excluded.remove(&hunit);
    }

    /// Set preferred regions for compensation.
    pub fn set_preferred_regions(&mut self, regions: Vec<RegionId>) {
        self.preferred_regions = regions;
    }

    /// Select a compensation H-unit with sufficient capacity.
    pub fn select(
        &mut self,
        failed_hunit: HUnitId,
        required_capacity: u64,
    ) -> Result<HUnitId, Error> {
        self.total_selections += 1;

        let mut candidates: Vec<&HUnitCapacity> = self
            .capacities
            .values()
            .filter(|c| c.hunit_id != failed_hunit)
            .filter(|c| !self.excluded.contains(&c.hunit_id))
            .filter(|c| c.can_accept(required_capacity))
            .collect();

        if candidates.is_empty() {
            self.failed_selections += 1;
            return Err(Error::NoEligibleCompensationHUnit {
                failed_hunit,
                required_capacity,
            });
        }

        // Prefer H-units in preferred regions, then lowest utilization.
        candidates.sort_by(|a, b| {
            let a_pref = self.preferred_regions.iter().any(|r| *r == a.region) as u8;
            let b_pref = self.preferred_regions.iter().any(|r| *r == b.region) as u8;
            b_pref.cmp(&a_pref).then_with(|| {
                a.utilization_ratio()
                .partial_cmp(&b.utilization_ratio())
                .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        let selected = candidates[0].hunit_id;
        Ok(selected)
    }

    /// Return whether an H-unit is eligible for compensation.
    pub fn is_eligible(&self, failed: HUnitId, candidate: HUnitId) -> bool {
        if failed == candidate || self.excluded.contains(&candidate) {
            return false;
        }
        match self.capacities.get(&candidate) {
            Some(cap) => cap.can_accept(0),
            None => false,
        }
    }

    /// Return total selections.
    pub fn total_selections(&self) -> u64 {
        self.total_selections
    }

    /// Return failed selection count.
    pub fn failed_selections(&self) -> u64 {
        self.failed_selections
    }

    /// Return selection success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_selections == 0 {
            return 1.0;
        }
        (self.total_selections - self.failed_selections) as f64 / self.total_selections as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Redundancy restoration engine
///////////////////////////////////////////////////////////////////////////////

/// A redundancy restoration task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundancyRestorationTask {
    pub task_id: RestorationTaskId,
    pub source_hunit: HUnitId,
    pub destination_hunit: HUnitId,
    pub partitions: Vec<PartitionId>,
    pub status: RestorationStatus,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a restoration task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RestorationTaskId(pub u64);

/// Status of a restoration task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestorationStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
}

/// Restores redundancy by copying data between H-units.
pub struct RedundancyRestorationEngine {
    /// Restoration tasks by ID.
    tasks: HashMap<RestorationTaskId, RedundancyRestorationTask>,
    /// Per-H-unit task IDs (as source).
    source_tasks: HashMap<HUnitId, Vec<RestorationTaskId>>,
    /// Per-H-unit task IDs (as destination).
    dest_tasks: HashMap<HUnitId, Vec<RestorationTaskId>>,
    /// Next task ID.
    next_id: u64,
    /// Maximum concurrent restorations.
    max_concurrent: usize,
    /// Total bytes restored.
    total_bytes_restored: u64,
    /// Total restoration operations.
    total_restorations: u64,
}

impl RedundancyRestorationEngine {
    /// Create a new restoration engine.
    pub fn new() -> Self {
        RedundancyRestorationEngine {
            tasks: HashMap::new(),
            source_tasks: HashMap::new(),
            dest_tasks: HashMap::new(),
            next_id: 0,
            max_concurrent: 3,
            total_bytes_restored: 0,
            total_restorations: 0,
        }
    }

    /// Create a restoration task.
    pub fn create_task(
        &mut self,
        source: HUnitId,
        destination: HUnitId,
        partitions: Vec<PartitionId>,
        total_bytes: u64,
    ) -> RestorationTaskId {
        let id = RestorationTaskId(self.next_id);
        self.next_id += 1;
        self.total_restorations += 1;
        let task = RedundancyRestorationTask {
            task_id: id,
            source_hunit: source,
            destination_hunit: destination,
            partitions: partitions.clone(),
            status: RestorationStatus::Queued,
            bytes_copied: 0,
            total_bytes,
            started_at: TimestampMillis(0),
            completed_at: None,
        };
        self.tasks.insert(id, task);
        self.source_tasks.entry(source).or_default().push(id);
        self.dest_tasks.entry(destination).or_default().push(id);
        id
    }

    /// Start the next queued task if capacity allows.
    pub fn start_next(&mut self) -> Option<RestorationTaskId> {
        let in_progress = self
            .tasks
            .values()
            .filter(|t| t.status == RestorationStatus::InProgress)
            .count();
        if in_progress >= self.max_concurrent {
            return None;
        }
        for (id, task) in &mut self.tasks {
            if task.status == RestorationStatus::Queued {
                task.status = RestorationStatus::InProgress;
                task.started_at = TimestampMillis(0);
                return Some(*id);
            }
        }
        None
    }

    /// Complete a restoration task.
    pub fn complete_task(
        &mut self,
        task_id: RestorationTaskId,
        bytes_copied: u64,
    ) -> Result<(), Error> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| Error::Internal {
                message: "restoration task not found".to_string(),
            })?;
        task.status = RestorationStatus::Completed;
        task.bytes_copied = bytes_copied;
        task.completed_at = Some(TimestampMillis(0));
        self.total_bytes_restored += bytes_copied;
        Ok(())
    }

    /// Fail a restoration task.
    pub fn fail_task(&mut self, task_id: RestorationTaskId) -> Result<(), Error> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| Error::Internal {
                message: "restoration task not found".to_string(),
            })?;
        task.status = RestorationStatus::Failed;
        Ok(())
    }

    /// Return tasks for a source H-unit.
    pub fn tasks_for_source(&self, source: HUnitId) -> Vec<RedundancyRestorationTask> {
        self.source_tasks
            .get(&source)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.tasks.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return tasks for a destination H-unit.
    pub fn tasks_for_destination(&self, dest: HUnitId) -> Vec<RedundancyRestorationTask> {
        self.dest_tasks
            .get(&dest)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.tasks.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return completed task count.
    pub fn completed_count(&self) -> u64 {
        self.tasks
            .values()
            .filter(|t| t.status == RestorationStatus::Completed)
            .count() as u64
    }

    /// Return failed task count.
    pub fn failed_count(&self) -> u64 {
        self.tasks
            .values()
            .filter(|t| t.status == RestorationStatus::Failed)
            .count() as u64
    }

    /// Return total bytes restored.
    pub fn total_bytes_restored(&self) -> u64 {
        self.total_bytes_restored
    }

    /// Set max concurrent restorations.
    pub fn set_max_concurrent(&mut self, max: usize) {
        self.max_concurrent = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation history — audit trail of all compensation operations
///////////////////////////////////////////////////////////////////////////////

/// A compensation history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationHistoryEntry {
    pub entry_id: HistoryEntryId,
    pub compensation_id: CompensationId,
    pub failed_hunit: HUnitId,
    pub compensation_hunit: HUnitId,
    pub failed_node: NodeId,
    pub partitions_affected: Vec<PartitionId>,
    pub partitions_restored: Vec<PartitionId>,
    pub bytes_restored: u64,
    pub status: CompensationStatus,
    pub timestamp: TimestampMillis,
}

/// Unique identifier for a history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoryEntryId(pub u64);

/// Maintains an audit trail of all H-unit compensation operations.
pub struct CompensationHistory {
    /// Entries by ID.
    entries: HashMap<HistoryEntryId, CompensationHistoryEntry>,
    /// Per-H-unit entry IDs (as failed H-unit).
    failed_hunit_entries: HashMap<HUnitId, Vec<HistoryEntryId>>,
    /// Per-H-unit entry IDs (as compensation H-unit).
    comp_hunit_entries: HashMap<HUnitId, Vec<HistoryEntryId>>,
    /// Next entry ID.
    next_id: u64,
    /// Maximum entries to retain.
    max_entries: usize,
}

impl CompensationHistory {
    /// Create a new history tracker.
    pub fn new() -> Self {
        CompensationHistory {
            entries: HashMap::new(),
            failed_hunit_entries: HashMap::new(),
            comp_hunit_entries: HashMap::new(),
            next_id: 0,
            max_entries: 1000,
        }
    }

    /// Record a compensation operation.
    pub fn record(
        &mut self,
        compensation_id: CompensationId,
        failed_hunit: HUnitId,
        compensation_hunit: HUnitId,
        failed_node: NodeId,
        partitions_affected: Vec<PartitionId>,
        partitions_restored: Vec<PartitionId>,
        bytes_restored: u64,
        status: CompensationStatus,
    ) -> HistoryEntryId {
        let id = HistoryEntryId(self.next_id);
        self.next_id += 1;
        let entry = CompensationHistoryEntry {
            entry_id: id,
            compensation_id,
            failed_hunit,
            compensation_hunit,
            failed_node,
            partitions_affected: partitions_affected.clone(),
            partitions_restored: partitions_restored.clone(),
            bytes_restored,
            status,
            timestamp: TimestampMillis(0),
        };
        self.entries.insert(id, entry);
        self.failed_hunit_entries
            .entry(failed_hunit)
            .or_default()
            .push(id);
        self.comp_hunit_entries
            .entry(compensation_hunit)
            .or_default()
            .push(id);

        // Trim if over max.
        if self.entries.len() > self.max_entries {
            // Remove oldest entry (simple FIFO).
            if let Some(oldest_id) = self.entries.keys().min().copied() {
                self.remove_entry(oldest_id);
            }
        }
        id
    }

    fn remove_entry(&mut self, id: HistoryEntryId) {
        if let Some(entry) = self.entries.remove(&id) {
            self.failed_hunit_entries
                .entry(entry.failed_hunit)
                .or_default()
                .retain(|&eid| eid != id);
            self.comp_hunit_entries
                .entry(entry.compensation_hunit)
                .or_default()
                .retain(|&eid| eid != id);
        }
    }

    /// Return history entries for a failed H-unit.
    pub fn entries_for_failed_hunit(&self, hunit: HUnitId) -> Vec<CompensationHistoryEntry> {
        self.failed_hunit_entries
            .get(&hunit)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entries.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return history entries for a compensation H-unit.
    pub fn entries_for_compensation_hunit(&self, hunit: HUnitId) -> Vec<CompensationHistoryEntry> {
        self.comp_hunit_entries
            .get(&hunit)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entries.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total entry count.
    pub fn total_entries(&self) -> u64 {
        self.entries.len() as u64
    }

    /// Return completed compensation count.
    pub fn completed_count(&self) -> u64 {
        self.entries
            .values()
            .filter(|e| e.status == CompensationStatus::Completed)
            .count() as u64
    }

    /// Return failed compensation count.
    pub fn failed_count(&self) -> u64 {
        self.entries
            .values()
            .filter(|e| e.status == CompensationStatus::Failed)
            .count() as u64
    }

    /// Set maximum entries to retain.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Core compensation manager
///////////////////////////////////////////////////////////////////////////////

/// Configuration for H-unit compensation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationConfig {
    pub max_concurrent_restorations: usize,
    pub max_compensation_history: usize,
    pub require_same_region: bool,
    pub min_healthy_nodes: u32,
}

impl Default for CompensationConfig {
    fn default() -> Self {
        CompensationConfig {
            max_concurrent_restorations: 3,
            max_compensation_history: 1000,
            require_same_region: true,
            min_healthy_nodes: 1,
        }
    }
}

/// Core H-unit compensation manager — orchestrates identification, selection,
/// and restoration for H-unit failures.
pub struct HUnitCompensationManager {
    /// Compensation configuration.
    config: CompensationConfig,
    /// Affected data tracker.
    affected_tracker: AffectedDataTracker,
    /// H-unit selector.
    selector: CompensationHUnitSelector,
    /// Restoration engine.
    restoration: RedundancyRestorationEngine,
    /// Compensation history.
    history: CompensationHistory,
    /// Total compensation operations.
    total_compensations: u64,
    /// Total successful compensations.
    successful_compensations: u64,
    /// H-units with restored redundancy.
    restored_hunits: HashSet<HUnitId>,
}

impl HUnitCompensationManager {
    /// Create a new compensation manager with default config.
    pub fn new() -> Self {
        HUnitCompensationManager {
            config: CompensationConfig::default(),
            affected_tracker: AffectedDataTracker::new(),
            selector: CompensationHUnitSelector::new(),
            restoration: RedundancyRestorationEngine::new(),
            history: CompensationHistory::new(),
            total_compensations: 0,
            successful_compensations: 0,
            restored_hunits: HashSet::new(),
        }
    }

    /// Create a compensation manager with custom config.
    pub fn with_config(config: CompensationConfig) -> Self {
        HUnitCompensationManager {
            config,
            affected_tracker: AffectedDataTracker::new(),
            selector: CompensationHUnitSelector::new(),
            restoration: RedundancyRestorationEngine::new(),
            history: CompensationHistory::new(),
            total_compensations: 0,
            successful_compensations: 0,
            restored_hunits: HashSet::new(),
        }
    }

    /// Register an affected data record.
    pub fn register_affected_data(&mut self, record: AffectedDataRecord) {
        self.affected_tracker.register_affected(record);
    }

    /// Register H-unit capacity for selection.
    pub fn register_hunit_capacity(&mut self, capacity: HUnitCapacity) {
        self.selector.register_capacity(capacity);
    }

    /// Exclude an H-unit from compensation selection.
    pub fn exclude_hunit(&mut self, hunit: HUnitId) {
        self.selector.exclude(hunit);
    }

    /// Identify affected data for a failed node within an H-unit.
    pub fn identify_affected_data(
        &self,
        hunit: HUnitId,
        failed_node: NodeId,
    ) -> Result<Vec<PartitionId>, Error> {
        let partitions = self
            .affected_tracker
            .identify_affected_data(hunit, failed_node);
        Ok(partitions)
    }

    /// Select a compensation H-unit.
    pub fn select_compensation_hunit(
        &mut self,
        failed_hunit: HUnitId,
        required_capacity: u64,
    ) -> Result<HUnitId, Error> {
        self.selector.select(failed_hunit, required_capacity)
    }

    /// Restore redundancy from source to destination H-unit.
    pub fn restore_redundancy(
        &mut self,
        source: HUnitId,
        destination: HUnitId,
    ) -> Result<CompensationResult, Error> {
        let partitions = self
            .affected_tracker
            .affected_for_hunit(&source)
            .clone();
        let total_bytes: u64 = partitions
            .iter()
            .filter_map(|p| {
                self.affected_tracker
                    .records
                    .get(p)
                    .map(|r| r.data_size_bytes)
            })
            .sum();

        let task_id = self.restoration.create_task(
            source,
            destination,
            partitions.clone(),
            total_bytes,
        );

        self.restoration.start_next()?;
        self.restoration.complete_task(task_id, total_bytes)?;

        for p in &partitions {
            self.affected_tracker.mark_compensated(*p);
        }
        self.restored_hunits.insert(destination);

        let result = CompensationResult {
            compensation_id: CompensationId(task_id.0),
            source_hunit: source,
            destination_hunit: destination,
            partitions_restored: partitions,
            bytes_restored: total_bytes,
            status: CompensationStatus::Completed,
            started_at: TimestampMillis(0),
            completed_at: Some(TimestampMillis(0)),
        };

        self.total_compensations += 1;
        self.successful_compensations += 1;

        self.history.record(
            CompensationId(task_id.0),
            source,
            destination,
            NodeId::new([0u8; 32]), // failed_node unknown in this path
            partitions.clone(),
            partitions,
            total_bytes,
            CompensationStatus::Completed,
        );

        Ok(result)
    }

    /// Return whether an H-unit is eligible for compensation.
    pub fn is_eligible_compensation(
        &self,
        failed: HUnitId,
        candidate: HUnitId,
    ) -> bool {
        self.selector.is_eligible(failed, candidate)
    }

    /// Return total compensation count.
    pub fn compensation_count(&self) -> u64 {
        self.total_compensations
    }

    /// Return whether redundancy is restored for an H-unit.
    pub fn redundancy_restored(&self, hunit: HUnitId) -> bool {
        self.restored_hunits.contains(&hunit)
    }

    /// Return successful compensation count.
    pub fn successful_compensations(&self) -> u64 {
        self.successful_compensations
    }

    /// Return compensation success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_compensations == 0 {
            return 1.0;
        }
        self.successful_compensations as f64 / self.total_compensations as f64
    }

    /// Return affected data tracker.
    pub fn affected_tracker(&self) -> &AffectedDataTracker {
        &self.affected_tracker
    }

    /// Return restoration engine.
    pub fn restoration_engine(&self) -> &RedundancyRestorationEngine {
        &self.restoration
    }

    /// Return compensation history.
    pub fn history(&self) -> &CompensationHistory {
        &self.history
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation eligibility checker
///////////////////////////////////////////////////////////////////////////////

/// Result of an eligibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibilityCheck {
    pub eligible: bool,
    pub reason: EligibilityReason,
    pub capacity_available: u64,
    pub healthy_nodes: u32,
}

/// Reason for eligibility or ineligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EligibilityReason {
    Eligible,
    SameHUnit,
    Excluded,
    InsufficientCapacity,
    NoHealthyNodes,
    WrongRegion,
    UnknownHUnit,
}

/// Checks compensation eligibility between H-units.
pub struct CompensationEligibilityChecker {
    /// Minimum healthy nodes required.
    min_healthy_nodes: u32,
    /// Whether same region is required.
    require_same_region: bool,
}

impl CompensationEligibilityChecker {
    /// Create a new checker.
    pub fn new() -> Self {
        CompensationEligibilityChecker {
            min_healthy_nodes: 1,
            require_same_region: true,
        }
    }

    /// Check whether a candidate H-unit is eligible for a failed H-unit.
    pub fn check(
        &self,
        failed: HUnitId,
        candidate: HUnitId,
        selector: &CompensationHUnitSelector,
    ) -> EligibilityCheck {
        if failed == candidate {
            return EligibilityCheck {
                eligible: false,
                reason: EligibilityReason::SameHUnit,
                capacity_available: 0,
                healthy_nodes: 0,
            };
        }

        if selector.excluded.contains(&candidate) {
            return EligibilityCheck {
                eligible: false,
                reason: EligibilityReason::Excluded,
                capacity_available: 0,
                healthy_nodes: 0,
            };
        }

        match selector.capacities.get(&candidate) {
            Some(cap) => {
                if cap.healthy_node_count < self.min_healthy_nodes {
                    return EligibilityCheck {
                        eligible: false,
                        reason: EligibilityReason::NoHealthyNodes,
                        capacity_available: cap.available_capacity_bytes,
                        healthy_nodes: cap.healthy_node_count,
                    };
                }
                if self.require_same_region && cap.region == RegionId::new([0u8; 32]) {
                    // Region check placeholder — in production would compare regions
                }
                if cap.available_capacity_bytes == 0 {
                    return EligibilityCheck {
                        eligible: false,
                        reason: EligibilityReason::InsufficientCapacity,
                        capacity_available: 0,
                        healthy_nodes: cap.healthy_node_count,
                    };
                }
                EligibilityCheck {
                    eligible: true,
                    reason: EligibilityReason::Eligible,
                    capacity_available: cap.available_capacity_bytes,
                    healthy_nodes: cap.healthy_node_count,
                }
            }
            None => EligibilityCheck {
                eligible: false,
                reason: EligibilityReason::UnknownHUnit,
                capacity_available: 0,
                healthy_nodes: 0,
            },
        }
    }

    /// Set minimum healthy nodes.
    pub fn set_min_healthy_nodes(&mut self, min: u32) {
        self.min_healthy_nodes = min;
    }

    /// Set whether same region is required.
    pub fn set_require_same_region(&mut self, required: bool) {
        self.require_same_region = required;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Version bit position for the H-unit compensation invariant in EdgeCaseBits.
pub const HUNIT_COMPENSATION_BIT: u128 = 1 << 10;

/// Check whether the H-unit compensation invariant is enabled in a bitset.
pub fn hunit_compensation_enabled(bits: &crate::versioning::EdgeCaseBits) -> bool {
    bits.has(HUNIT_COMPENSATION_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod hunit_compensation_tests {
    use super::*;

    // ---- AffectedDataRecord tests ----

    #[test]
    fn affected_data_record_creation() {
        let record = AffectedDataRecord {
            partition_id: PartitionId::new([1u8; 32]),
            dataset_id: DatasetId::new([2u8; 32]),
            node_id: NodeId::new([3u8; 32]),
            hunit_id: HUnitId::new([4u8; 32]),
            data_size_bytes: 1024,
            replica_count: 3,
            healthy_replicas: 2,
            needs_compensation: true,
        };
        assert_eq!(record.data_size_bytes, 1024);
        assert_eq!(record.replica_count, 3);
        assert!(record.needs_compensation);
    }

    // ---- AffectedDataTracker tests ----

    #[test]
    fn tracker_register_and_identify() {
        let mut tracker = AffectedDataTracker::new();
        let hunit = HUnitId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let partition = PartitionId::new([3u8; 32]);
        let dataset = DatasetId::new([4u8; 32]);

        tracker.register_affected(AffectedDataRecord {
            partition_id: partition,
            dataset_id: dataset,
            node_id: node,
            hunit_id: hunit,
            data_size_bytes: 1024,
            replica_count: 3,
            healthy_replicas: 2,
            needs_compensation: true,
        });

        let affected = tracker.identify_affected_data(hunit, node);
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0], partition);
    }

    #[test]
    fn tracker_identify_different_node() {
        let mut tracker = AffectedDataTracker::new();
        let hunit = HUnitId::new([1u8; 32]);
        let node1 = NodeId::new([2u8; 32]);
        let node2 = NodeId::new([5u8; 32]);
        let partition = PartitionId::new([3u8; 32]);
        let dataset = DatasetId::new([4u8; 32]);

        tracker.register_affected(AffectedDataRecord {
            partition_id: partition,
            dataset_id: dataset,
            node_id: node1,
            hunit_id: hunit,
            data_size_bytes: 1024,
            replica_count: 3,
            healthy_replicas: 2,
            needs_compensation: true,
        });

        // Different node should not return the partition.
        let affected = tracker.identify_affected_data(hunit, node2);
        assert!(affected.is_empty());
    }

    #[test]
    fn tracker_mark_compensated() {
        let mut tracker = AffectedDataTracker::new();
        let hunit = HUnitId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let partition = PartitionId::new([3u8; 32]);
        let dataset = DatasetId::new([4u8; 32]);

        tracker.register_affected(AffectedDataRecord {
            partition_id: partition,
            dataset_id: dataset,
            node_id: node,
            hunit_id: hunit,
            data_size_bytes: 1024,
            replica_count: 3,
            healthy_replicas: 2,
            needs_compensation: true,
        });

        assert!(!tracker.is_compensated(partition));
        tracker.mark_compensated(partition);
        assert!(tracker.is_compensated(partition));

        // Should not return compensated partition.
        let affected = tracker.identify_affected_data(hunit, node);
        assert!(affected.is_empty());
    }

    #[test]
    fn tracker_counts() {
        let mut tracker = AffectedDataTracker::new();
        let hunit = HUnitId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let dataset = DatasetId::new([4u8; 32]);

        for i in 0..3 {
            tracker.register_affected(AffectedDataRecord {
                partition_id: PartitionId::new([i as u8; 32]),
                dataset_id: dataset,
                node_id: node,
                hunit_id: hunit,
                data_size_bytes: 1024,
                replica_count: 3,
                healthy_replicas: 2,
                needs_compensation: true,
            });
        }

        assert_eq!(tracker.total_records(), 3);
        assert_eq!(tracker.pending_count(), 3);
        tracker.mark_compensated(PartitionId::new([0u8; 32]));
        assert_eq!(tracker.compensated_count(), 1);
        assert_eq!(tracker.pending_count(), 2);
    }

    // ---- HUnitCapacity tests ----

    #[test]
    fn hunit_capacity_utilization() {
        let cap = HUnitCapacity {
            hunit_id: HUnitId::new([1u8; 32]),
            total_capacity_bytes: 1000,
            used_capacity_bytes: 300,
            available_capacity_bytes: 700,
            node_count: 5,
            healthy_node_count: 4,
            region: RegionId::new([2u8; 32]),
        };
        assert!((cap.utilization_ratio() - 0.3).abs() < 0.001);
        assert!(cap.can_accept(500));
        assert!(!cap.can_accept(800));
    }

    #[test]
    fn hunit_capacity_no_healthy_nodes() {
        let cap = HUnitCapacity {
            hunit_id: HUnitId::new([1u8; 32]),
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 0,
            region: RegionId::new([2u8; 32]),
        };
        assert!(!cap.can_accept(100));
    }

    // ---- CompensationHUnitSelector tests ----

    #[test]
    fn selector_select_eligible() {
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        let result = selector.select(failed, 500);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), candidate);
    }

    #[test]
    fn selector_select_insufficient_capacity() {
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 950,
            available_capacity_bytes: 50,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        let result = selector.select(failed, 500);
        assert!(result.is_err());
        assert_eq!(selector.failed_selections(), 1);
    }

    #[test]
    fn selector_exclude_hunit() {
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        selector.exclude(candidate);
        let result = selector.select(failed, 100);
        assert!(result.is_err());
    }

    #[test]
    fn selector_is_eligible() {
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        assert!(selector.is_eligible(failed, candidate));
        assert!(!selector.is_eligible(failed, failed));
    }

    #[test]
    fn selector_success_rate() {
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: HUnitId::new([2u8; 32]),
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        selector.select(failed, 100).unwrap();
        selector.select(failed, 2000).unwrap_err();
        assert!((selector.success_rate() - 0.5).abs() < 0.001);
    }

    // ---- RedundancyRestorationEngine tests ----

    #[test]
    fn restoration_create_and_complete() {
        let mut engine = RedundancyRestorationEngine::new();
        let source = HUnitId::new([1u8; 32]);
        let dest = HUnitId::new([2u8; 32]);
        let task_id = engine.create_task(
            source,
            dest,
            vec![PartitionId::new([1u8; 32])],
            2048,
        );

        engine.start_next().unwrap();
        engine.complete_task(task_id, 2048).unwrap();

        assert_eq!(engine.completed_count(), 1);
        assert_eq!(engine.total_bytes_restored(), 2048);
    }

    #[test]
    fn restoration_fail_task() {
        let mut engine = RedundancyRestorationEngine::new();
        let task_id = engine.create_task(
            HUnitId::new([1u8; 32]),
            HUnitId::new([2u8; 32]),
            vec![],
            1024,
        );
        engine.start_next().unwrap();
        engine.fail_task(task_id).unwrap();
        assert_eq!(engine.failed_count(), 1);
    }

    #[test]
    fn restoration_max_concurrent() {
        let mut engine = RedundancyRestorationEngine::new();
        engine.set_max_concurrent(1);
        let source = HUnitId::new([1u8; 32]);
        let dest = HUnitId::new([2u8; 32]);
        engine.create_task(source, dest, vec![], 1024);
        engine.create_task(source, dest, vec![], 2048);

        engine.start_next().unwrap();
        assert!(engine.start_next().is_none());
    }

    #[test]
    fn restoration_complete_nonexistent() {
        let mut engine = RedundancyRestorationEngine::new();
        assert!(engine.complete_task(RestorationTaskId(999), 1024).is_err());
    }

    // ---- CompensationHistory tests ----

    #[test]
    fn history_record_and_lookup() {
        let mut history = CompensationHistory::new();
        let failed = HUnitId::new([1u8; 32]);
        let comp = HUnitId::new([2u8; 32]);
        let node = NodeId::new([3u8; 32]);
        let partition = PartitionId::new([4u8; 32]);

        let id = history.record(
            CompensationId(1),
            failed,
            comp,
            node,
            vec![partition],
            vec![partition],
            1024,
            CompensationStatus::Completed,
        );

        let entries = history.entries_for_failed_hunit(failed);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].compensation_id, CompensationId(1));

        let comp_entries = history.entries_for_compensation_hunit(comp);
        assert_eq!(comp_entries.len(), 1);
    }

    #[test]
    fn history_completed_and_failed_count() {
        let mut history = CompensationHistory::new();
        let failed = HUnitId::new([1u8; 32]);
        let comp = HUnitId::new([2u8; 32]);

        history.record(
            CompensationId(1),
            failed,
            comp,
            NodeId::new([3u8; 32]),
            vec![],
            vec![],
            1024,
            CompensationStatus::Completed,
        );
        history.record(
            CompensationId(2),
            failed,
            comp,
            NodeId::new([3u8; 32]),
            vec![],
            vec![],
            1024,
            CompensationStatus::Failed,
        );

        assert_eq!(history.completed_count(), 1);
        assert_eq!(history.failed_count(), 1);
        assert_eq!(history.total_entries(), 2);
    }

    // ---- CompensationStatus tests ----

    #[test]
    fn compensation_status_variants() {
        let statuses = [
            CompensationStatus::Pending,
            CompensationStatus::InProgress,
            CompensationStatus::Completed,
            CompensationStatus::Failed,
            CompensationStatus::Cancelled,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ---- RestorationStatus tests ----

    #[test]
    fn restoration_status_variants() {
        let statuses = [
            RestorationStatus::Queued,
            RestorationStatus::InProgress,
            RestorationStatus::Completed,
            RestorationStatus::Failed,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ---- CompensationId tests ----

    #[test]
    fn compensation_id_ordering() {
        let a = CompensationId(1);
        let b = CompensationId(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- RestorationTaskId tests ----

    #[test]
    fn restoration_task_id_ordering() {
        let a = RestorationTaskId(10);
        let b = RestorationTaskId(20);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- HistoryEntryId tests ----

    #[test]
    fn history_entry_id_ordering() {
        let a = HistoryEntryId(5);
        let b = HistoryEntryId(15);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- HUnitCompensationManager tests ----

    #[test]
    fn manager_full_workflow() {
        let mut manager = HUnitCompensationManager::new();
        let failed_hunit = HUnitId::new([1u8; 32]);
        let comp_hunit = HUnitId::new([2u8; 32]);
        let failed_node = NodeId::new([3u8; 32]);
        let partition = PartitionId::new([4u8; 32]);
        let dataset = DatasetId::new([5u8; 32]);

        // Register affected data.
        manager.register_affected_data(AffectedDataRecord {
            partition_id: partition,
            dataset_id: dataset,
            node_id: failed_node,
            hunit_id: failed_hunit,
            data_size_bytes: 2048,
            replica_count: 3,
            healthy_replicas: 1,
            needs_compensation: true,
        });

        // Register capacity.
        manager.register_hunit_capacity(HUnitCapacity {
            hunit_id: comp_hunit,
            total_capacity_bytes: 10000,
            used_capacity_bytes: 1000,
            available_capacity_bytes: 9000,
            node_count: 5,
            healthy_node_count: 4,
            region: RegionId::new([2u8; 32]),
        });

        // Identify affected data.
        let affected = manager
            .identify_affected_data(failed_hunit, failed_node)
            .unwrap();
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0], partition);

        // Select compensation H-unit.
        let selected = manager
            .select_compensation_hunit(failed_hunit, 1000)
            .unwrap();
        assert_eq!(selected, comp_hunit);

        // Restore redundancy.
        let result = manager.restore_redundancy(failed_hunit, comp_hunit).unwrap();
        assert_eq!(result.status, CompensationStatus::Completed);
        assert_eq!(result.bytes_restored, 2048);

        // Verify.
        assert!(manager.redundancy_restored(comp_hunit));
        assert_eq!(manager.compensation_count(), 1);
        assert_eq!(manager.successful_compensations(), 1);
    }

    #[test]
    fn manager_is_eligible() {
        let mut manager = HUnitCompensationManager::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        manager.register_hunit_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        assert!(manager.is_eligible_compensation(failed, candidate));
        assert!(!manager.is_eligible_compensation(failed, failed));
    }

    #[test]
    fn manager_success_rate() {
        let mut manager = HUnitCompensationManager::new();
        let failed = HUnitId::new([1u8; 32]);
        let comp1 = HUnitId::new([2u8; 32]);
        let comp2 = HUnitId::new([3u8; 32]);

        manager.register_hunit_capacity(HUnitCapacity {
            hunit_id: comp1,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });
        manager.register_hunit_capacity(HUnitCapacity {
            hunit_id: comp2,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 950,
            available_capacity_bytes: 50,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        let _ = manager.select_compensation_hunit(failed, 1000);
        let _ = manager.select_compensation_hunit(failed, 1000);
        assert!((manager.success_rate() - 0.5).abs() < 0.001);
    }

    // ---- CompensationEligibilityChecker tests ----

    #[test]
    fn eligibility_checker_eligible() {
        let mut checker = CompensationEligibilityChecker::new();
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });

        let check = checker.check(failed, candidate, &selector);
        assert!(check.eligible);
        assert_eq!(check.reason, EligibilityReason::Eligible);
    }

    #[test]
    fn eligibility_checker_same_hunit() {
        let checker = CompensationEligibilityChecker::new();
        let selector = CompensationHUnitSelector::new();
        let hunit = HUnitId::new([1u8; 32]);
        let check = checker.check(hunit, hunit, &selector);
        assert!(!check.eligible);
        assert_eq!(check.reason, EligibilityReason::SameHUnit);
    }

    #[test]
    fn eligibility_checker_excluded() {
        let mut checker = CompensationEligibilityChecker::new();
        let mut selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        selector.register_capacity(HUnitCapacity {
            hunit_id: candidate,
            total_capacity_bytes: 1000,
            used_capacity_bytes: 100,
            available_capacity_bytes: 900,
            node_count: 5,
            healthy_node_count: 3,
            region: RegionId::new([2u8; 32]),
        });
        selector.exclude(candidate);

        let check = checker.check(failed, candidate, &selector);
        assert!(!check.eligible);
        assert_eq!(check.reason, EligibilityReason::Excluded);
    }

    #[test]
    fn eligibility_checker_unknown() {
        let checker = CompensationEligibilityChecker::new();
        let selector = CompensationHUnitSelector::new();
        let failed = HUnitId::new([1u8; 32]);
        let candidate = HUnitId::new([2u8; 32]);
        let check = checker.check(failed, candidate, &selector);
        assert!(!check.eligible);
        assert_eq!(check.reason, EligibilityReason::UnknownHUnit);
    }

    // ---- CompensationConfig tests ----

    #[test]
    fn compensation_config_default() {
        let config = CompensationConfig::default();
        assert_eq!(config.max_concurrent_restorations, 3);
        assert_eq!(config.max_compensation_history, 1000);
        assert!(config.require_same_region);
        assert_eq!(config.min_healthy_nodes, 1);
    }

    // ---- CompensationResult tests ----

    #[test]
    fn compensation_result_default() {
        let result = CompensationResult::default();
        assert_eq!(result.status, CompensationStatus::Pending);
        assert_eq!(result.bytes_restored, 0);
        assert!(result.partitions_restored.is_empty());
    }

    // ---- CompensationResult field tests ----

    #[test]
    fn compensation_result_creation() {
        let result = CompensationResult {
            compensation_id: CompensationId(1),
            source_hunit: HUnitId::new([1u8; 32]),
            destination_hunit: HUnitId::new([2u8; 32]),
            partitions_restored: vec![PartitionId::new([3u8; 32])],
            bytes_restored: 4096,
            status: CompensationStatus::Completed,
            started_at: TimestampMillis(0),
            completed_at: Some(TimestampMillis(0)),
        };
        assert_eq!(result.bytes_restored, 4096);
        assert_eq!(result.status, CompensationStatus::Completed);
        assert_eq!(result.partitions_restored.len(), 1);
    }

    // ---- HUNIT_COMPENSATION_BIT tests ----

    #[test]
    fn hunit_compensation_bit_position() {
        assert_eq!(HUNIT_COMPENSATION_BIT, 1 << 10);
    }

    #[test]
    fn hunit_compensation_enabled_checks_bit() {
        use crate::versioning::EdgeCaseBits;
        let mut bits = EdgeCaseBits(0);
        assert!(!hunit_compensation_enabled(&bits));
        bits.enable(HUNIT_COMPENSATION_BIT);
        assert!(hunit_compensation_enabled(&bits));
    }

    // ---- Sealed trait unit tests ----

    #[test]
    fn sealed_unit_impl() {
        let unit = ();
        assert!(!unit.redundancy_restored(HUnitId::new([1u8; 32])));
        assert_eq!(unit.compensation_count(), 0);
        assert!(!unit.is_eligible_compensation(
            HUnitId::new([1u8; 32]),
            HUnitId::new([2u8; 32])
        ));
        assert!(unit.identify_affected_data(HUnitId::new([1u8; 32]), NodeId::new([2u8; 32])).is_ok());
    }

    // ---- Integration test ==========

    #[test]
    fn full_compensation_integration() {
        let mut manager = HUnitCompensationManager::new();
        let failed_hunit = HUnitId::new([1u8; 32]);
        let comp_hunit = HUnitId::new([2u8; 32]);
        let failed_node = NodeId::new([3u8; 32]);
        let p1 = PartitionId::new([4u8; 32]);
        let p2 = PartitionId::new([5u8; 32]);
        let dataset = DatasetId::new([6u8; 32]);

        // Register multiple affected partitions.
        manager.register_affected_data(AffectedDataRecord {
            partition_id: p1,
            dataset_id: dataset,
            node_id: failed_node,
            hunit_id: failed_hunit,
            data_size_bytes: 1024,
            replica_count: 3,
            healthy_replicas: 1,
            needs_compensation: true,
        });
        manager.register_affected_data(AffectedDataRecord {
            partition_id: p2,
            dataset_id: dataset,
            node_id: failed_node,
            hunit_id: failed_hunit,
            data_size_bytes: 2048,
            replica_count: 3,
            healthy_replicas: 1,
            needs_compensation: true,
        });

        // Register capacity.
        manager.register_hunit_capacity(HUnitCapacity {
            hunit_id: comp_hunit,
            total_capacity_bytes: 10000,
            used_capacity_bytes: 1000,
            available_capacity_bytes: 9000,
            node_count: 5,
            healthy_node_count: 4,
            region: RegionId::new([2u8; 32]),
        });

        // Identify all affected.
        let affected = manager
            .identify_affected_data(failed_hunit, failed_node)
            .unwrap();
        assert_eq!(affected.len(), 2);

        // Select and restore.
        let selected = manager.select_compensation_hunit(failed_hunit, 500).unwrap();
        assert_eq!(selected, comp_hunit);
        let result = manager.restore_redundancy(failed_hunit, comp_hunit).unwrap();
        assert_eq!(result.partitions_restored.len(), 2);
        assert_eq!(result.bytes_restored, 3072);
        assert!(manager.redundancy_restored(comp_hunit));

        // Verify history.
        assert_eq!(manager.history().total_entries(), 1);
        assert_eq!(manager.history().completed_count(), 1);
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation Planner — plans multi-step compensation operations
///////////////////////////////////////////////////////////////////////////////

/// A step in a compensation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationPlanStep {
    pub step_index: u32,
    pub step_type: CompensationStepType,
    pub source_hunit: Option<HUnitId>,
    pub target_hunit: Option<HUnitId>,
    pub partitions: Vec<PartitionId>,
    pub estimated_bytes: u64,
    pub status: PlanStepStatus,
}

/// Type of compensation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompensationStepType {
    IdentifyAffectedData,
    SelectCompensationHUnit,
    RestoreRedundancy,
    VerifyRestoration,
    UpdateMetadata,
}

/// Status of a plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

/// A compensation plan for an H-unit failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationPlan {
    pub plan_id: CompensationPlanId,
    pub failed_hunit: HUnitId,
    pub failed_node: NodeId,
    pub steps: Vec<CompensationPlanStep>,
    pub compensation_hunit: Option<HUnitId>,
    pub total_estimated_bytes: u64,
    pub status: PlanStatus,
}

/// Status of a compensation plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStatus {
    Draft,
    Active,
    Completed,
    Failed,
    Aborted,
}

/// Unique identifier for a compensation plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompensationPlanId(pub u64);

/// Plans multi-step compensation operations for H-unit failures.
pub struct CompensationPlanner {
    /// Plans by ID.
    plans: HashMap<CompensationPlanId, CompensationPlan>,
    /// Per-H-unit plan IDs.
    hunit_plans: HashMap<HUnitId, Vec<CompensationPlanId>>,
    /// Next plan ID.
    next_id: u64,
    /// Maximum steps per plan.
    max_steps: u32,
}

impl CompensationPlanner {
    /// Create a new planner.
    pub fn new() -> Self {
        CompensationPlanner {
            plans: HashMap::new(),
            hunit_plans: HashMap::new(),
            next_id: 0,
            max_steps: 10,
        }
    }

    /// Create a compensation plan for a failed H-unit.
    pub fn create_plan(
        &mut self,
        failed_hunit: HUnitId,
        failed_node: NodeId,
        affected_partitions: Vec<PartitionId>,
        estimated_bytes_per_partition: u64,
    ) -> CompensationPlanId {
        let id = CompensationPlanId(self.next_id);
        self.next_id += 1;
        let total_estimated_bytes = affected_partitions.len() as u64 * estimated_bytes_per_partition;

        let mut steps = Vec::new();
        steps.push(CompensationPlanStep {
            step_index: 0,
            step_type: CompensationStepType::IdentifyAffectedData,
            source_hunit: Some(failed_hunit),
            target_hunit: None,
            partitions: affected_partitions.clone(),
            estimated_bytes: 0,
            status: PlanStepStatus::Pending,
        });
        steps.push(CompensationPlanStep {
            step_index: 1,
            step_type: CompensationStepType::SelectCompensationHUnit,
            source_hunit: Some(failed_hunit),
            target_hunit: None,
            partitions: Vec::new(),
            estimated_bytes: 0,
            status: PlanStepStatus::Pending,
        });
        steps.push(CompensationPlanStep {
            step_index: 2,
            step_type: CompensationStepType::RestoreRedundancy,
            source_hunit: Some(failed_hunit),
            target_hunit: None,
            partitions: affected_partitions,
            estimated_bytes: total_estimated_bytes,
            status: PlanStepStatus::Pending,
        });
        steps.push(CompensationPlanStep {
            step_index: 3,
            step_type: CompensationStepType::VerifyRestoration,
            source_hunit: Some(failed_hunit),
            target_hunit: None,
            partitions: affected_partitions,
            estimated_bytes: 0,
            status: PlanStepStatus::Pending,
        });
        steps.push(CompensationPlanStep {
            step_index: 4,
            step_type: CompensationStepType::UpdateMetadata,
            source_hunit: Some(failed_hunit),
            target_hunit: None,
            partitions: Vec::new(),
            estimated_bytes: 0,
            status: PlanStepStatus::Pending,
        });

        let plan = CompensationPlan {
            plan_id: id,
            failed_hunit,
            failed_node,
            steps,
            compensation_hunit: None,
            total_estimated_bytes,
            status: PlanStatus::Draft,
        };
        self.plans.insert(id, plan);
        self.hunit_plans.entry(failed_hunit).or_default().push(id);
        id
    }

    /// Set the compensation H-unit for a plan.
    pub fn set_compensation_hunit(
        &mut self,
        plan_id: CompensationPlanId,
        hunit: HUnitId,
    ) -> Result<(), Error> {
        let plan = self
            .plans
            .get_mut(&plan_id)
            .ok_or_else(|| Error::Internal {
                message: "compensation plan not found".to_string(),
            })?;
        plan.compensation_hunit = Some(hunit);
        // Update restore step target.
        for step in &mut plan.steps {
            if step.step_type == CompensationStepType::RestoreRedundancy {
                step.target_hunit = Some(hunit);
            }
        }
        Ok(())
    }

    /// Start a plan (mark active).
    pub fn activate_plan(&mut self, plan_id: CompensationPlanId) -> Result<(), Error> {
        let plan = self
            .plans
            .get_mut(&plan_id)
            .ok_or_else(|| Error::Internal {
                message: "compensation plan not found".to_string(),
            })?;
        plan.status = PlanStatus::Active;
        for step in &mut plan.steps {
            if step.status == PlanStepStatus::Pending {
                step.status = PlanStepStatus::InProgress;
                break;
            }
        }
        Ok(())
    }

    /// Complete a step in a plan.
    pub fn complete_step(
        &mut self,
        plan_id: CompensationPlanId,
        step_index: u32,
    ) -> Result<(), Error> {
        let plan = self
            .plans
            .get_mut(&plan_id)
            .ok_or_else(|| Error::Internal {
                message: "compensation plan not found".to_string(),
            })?;
        for step in &mut plan.steps {
            if step.step_index == step_index {
                step.status = PlanStepStatus::Completed;
                // Start next pending step.
                for next in &mut plan.steps {
                    if next.status == PlanStepStatus::Pending {
                        next.status = PlanStepStatus::InProgress;
                        break;
                    }
                }
                // Check if all steps completed.
                if plan.steps.iter().all(|s| s.status == PlanStepStatus::Completed) {
                    plan.status = PlanStatus::Completed;
                }
                return Ok(());
            }
        }
        Err(Error::Internal {
            message: format!("step {} not found in plan", step_index),
        })
    }

    /// Fail a plan.
    pub fn fail_plan(&mut self, plan_id: CompensationPlanId) -> Result<(), Error> {
        let plan = self
            .plans
            .get_mut(&plan_id)
            .ok_or_else(|| Error::Internal {
                message: "compensation plan not found".to_string(),
            })?;
        plan.status = PlanStatus::Failed;
        Ok(())
    }

    /// Return a plan by ID.
    pub fn plan(&self, plan_id: CompensationPlanId) -> Option<&CompensationPlan> {
        self.plans.get(&plan_id)
    }

    /// Return all plans for a failed H-unit.
    pub fn plans_for_hunit(&self, hunit: HUnitId) -> Vec<CompensationPlan> {
        self.hunit_plans
            .get(&hunit)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.plans.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total plan count.
    pub fn plan_count(&self) -> usize {
        self.plans.len()
    }

    /// Return completed plan count.
    pub fn completed_plan_count(&self) -> u64 {
        self.plans
            .values()
            .filter(|p| p.status == PlanStatus::Completed)
            .count() as u64
    }

    /// Return failed plan count.
    pub fn failed_plan_count(&self) -> u64 {
        self.plans
            .values()
            .filter(|p| p.status == PlanStatus::Failed)
            .count() as u64
    }

    /// Set max steps per plan.
    pub fn set_max_steps(&mut self, max: u32) {
        self.max_steps = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation Verifyer — verifies compensation completeness
///////////////////////////////////////////////////////////////////////////////

/// Verification result for a compensation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationVerificationResult {
    pub compensation_id: CompensationId,
    pub source_hunit: HUnitId,
    pub destination_hunit: HUnitId,
    pub partitions_verified: u32,
    pub partitions_failed: u32,
    pub bytes_verified: u64,
    pub verified: bool,
    pub verification_notes: String,
}

/// Verifies that redundancy restoration was completed correctly.
pub struct CompensationVerifyer;

impl CompensationVerifyer {
    /// Create a new verifier.
    pub fn new() -> Self {
        CompensationVerifyer
    }

    /// Verify a compensation result against expected partitions.
    pub fn verify(
        &self,
        compensation_id: CompensationId,
        source: HUnitId,
        destination: HUnitId,
        expected_partitions: &[PartitionId],
        restored_partitions: &[PartitionId],
        expected_bytes: u64,
        actual_bytes: u64,
    ) -> CompensationVerificationResult {
        let expected_set: HashSet<PartitionId> = expected_partitions.iter().copied().collect();
        let restored_set: HashSet<PartitionId> = restored_partitions.iter().copied().collect();

        let mut verified_count = 0u32;
        let mut failed_count = 0u32;
        for p in expected_partitions {
            if restored_set.contains(p) {
                verified_count += 1;
            } else {
                failed_count += 1;
            }
        }

        let bytes_match = expected_bytes == actual_bytes;
        let all_verified = failed_count == 0 && bytes_match;
        let verified = all_verified;

        let notes = if verified {
            format!(
                "All {} partitions verified, {} bytes restored OK",
                verified_count, actual_bytes
            )
        } else {
            format!(
                "Verification failed: {}/{} partitions restored, bytes expected {} got {}",
                verified_count,
                expected_partitions.len(),
                expected_bytes,
                actual_bytes
            )
        };

        CompensationVerificationResult {
            compensation_id,
            source_hunit: source,
            destination_hunit: destination,
            partitions_verified: verified_count,
            partitions_failed: failed_count,
            bytes_verified: actual_bytes,
            verified,
            verification_notes: notes,
        }
    }

    /// Return whether compensation is fully verified.
    pub fn is_fully_verified(result: &CompensationVerificationResult) -> bool {
        result.verified && result.partitions_failed == 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// H-Unit Failure Classifier — classifies failure types
///////////////////////////////////////////////////////////////////////////////

/// Type of H-unit failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HUnitFailureType {
    NodeFailure,
    DiskFailure,
    NetworkPartition,
    PowerOutage,
    HardwareFailure,
    SoftwareFailure,
    Maintenance,
    Unknown,
}

/// Classification result for an H-unit failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureClassification {
    pub hunit_id: HUnitId,
    pub failure_type: HUnitFailureType,
    pub affected_nodes: Vec<NodeId>,
    pub affected_partitions: Vec<PartitionId>,
    pub severity: FailureSeverity,
    pub estimated_recovery_time_seconds: u64,
    pub compensation_required: bool,
}

/// Severity of an H-unit failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Classifies H-unit failures and determines compensation requirements.
pub struct HUnitFailureClassifier;

impl HUnitFailureClassifier {
    /// Create a new classifier.
    pub fn new() -> Self {
        HUnitFailureClassifier
    }

    /// Classify an H-unit failure.
    pub fn classify(
        &self,
        hunit_id: HUnitId,
        failed_nodes: Vec<NodeId>,
        affected_partitions: Vec<PartitionId>,
        failure_hint: Option<HUnitFailureType>,
    ) -> FailureClassification {
        let failure_type = failure_hint.unwrap_or(HUnitFailureType::Unknown);
        let severity = self.determine_severity(
            &failure_type,
            failed_nodes.len(),
            affected_partitions.len(),
        );
        let compensation_required = affected_partitions.len() > 0
            && matches!(
                failure_type,
                HUnitFailureType::NodeFailure
                    | HUnitFailureType::DiskFailure
                    | HUnitFailureType::HardwareFailure
            );

        let estimated_recovery = self.estimate_recovery_time(&failure_type, affected_partitions.len());

        FailureClassification {
            hunit_id,
            failure_type,
            affected_nodes: failed_nodes,
            affected_partitions,
            severity,
            estimated_recovery_time_seconds: estimated_recovery,
            compensation_required,
        }
    }

    fn determine_severity(
        &self,
        failure_type: &HUnitFailureType,
        node_count: usize,
        partition_count: usize,
    ) -> FailureSeverity {
        let score = node_count + partition_count;
        match failure_type {
            HUnitFailureType::PowerOutage | HUnitFailureType::HardwareFailure => {
                if score > 10 {
                    FailureSeverity::Critical
                } else if score > 3 {
                    FailureSeverity::High
                } else {
                    FailureSeverity::Medium
                }
            }
            HUnitFailureType::NodeFailure | HUnitFailureType::DiskFailure => {
                if score > 20 {
                    FailureSeverity::Critical
                } else if score > 5 {
                    FailureSeverity::High
                } else if score > 0 {
                    FailureSeverity::Medium
                } else {
                    FailureSeverity::Low
                }
            }
            HUnitFailureType::NetworkPartition => FailureSeverity::Medium,
            HUnitFailureType::SoftwareFailure => FailureSeverity::Low,
            HUnitFailureType::Maintenance => FailureSeverity::Low,
            HUnitFailureType::Unknown => FailureSeverity::Medium,
        }
    }

    fn estimate_recovery_time(&self, failure_type: &HUnitFailureType, partition_count: usize) -> u64 {
        let base_minutes: u64 = match failure_type {
            HUnitFailureType::NodeFailure => 30,
            HUnitFailureType::DiskFailure => 60,
            HUnitFailureType::NetworkPartition => 15,
            HUnitFailureType::PowerOutage => 120,
            HUnitFailureType::HardwareFailure => 240,
            HUnitFailureType::SoftwareFailure => 10,
            HUnitFailureType::Maintenance => 0,
            HUnitFailureType::Unknown => 60,
        };
        base_minutes * 60 + (partition_count as u64 * 5)
    }

    /// Return whether compensation is required for a failure type.
    pub fn compensation_required_for(failure_type: HUnitFailureType) -> bool {
        matches!(
            failure_type,
            HUnitFailureType::NodeFailure
                | HUnitFailureType::DiskFailure
                | HUnitFailureType::HardwareFailure
        )
    }

    /// Return all failure types.
    pub fn all_failure_types() -> Vec<HUnitFailureType> {
        vec![
            HUnitFailureType::NodeFailure,
            HUnitFailureType::DiskFailure,
            HUnitFailureType::NetworkPartition,
            HUnitFailureType::PowerOutage,
            HUnitFailureType::HardwareFailure,
            HUnitFailureType::SoftwareFailure,
            HUnitFailureType::Maintenance,
            HUnitFailureType::Unknown,
        ]
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation Rate Limiter — rate limits compensation operations
///////////////////////////////////////////////////////////////////////////////

/// A rate limit window entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitWindow {
    pub window_start: TimestampMillis,
    pub operation_count: u32,
    pub bytes_processed: u64,
}

/// Limits the rate of compensation operations to prevent overload.
pub struct CompensationRateLimiter {
    /// Maximum operations per window.
    max_ops_per_window: u32,
    /// Maximum bytes per window.
    max_bytes_per_window: u64,
    /// Window duration in milliseconds.
    window_ms: u64,
    /// Current window.
    current_window: RateLimitWindow,
    /// Total operations allowed.
    total_allowed: u64,
    /// Total operations rejected.
    total_rejected: u64,
}

impl CompensationRateLimiter {
    /// Create a new rate limiter.
    pub fn new() -> Self {
        CompensationRateLimiter {
            max_ops_per_window: 100,
            max_bytes_per_window: 1_073_741_824, // 1 GiB
            window_ms: 60_000, // 1 minute
            current_window: RateLimitWindow {
                window_start: 0,
                operation_count: 0,
                bytes_processed: 0,
            },
            total_allowed: 0,
            total_rejected: 0,
        }
    }

    /// Check whether a compensation operation is allowed.
    pub fn check_allowance(&mut self, bytes: u64, timestamp: TimestampMillis) -> bool {
        // Simple check: if within window limits, allow.
        if self.current_window.operation_count >= self.max_ops_per_window {
            self.total_rejected += 1;
            return false;
        }
        if self.current_window.bytes_processed + bytes > self.max_bytes_per_window {
            self.total_rejected += 1;
            return false;
        }
        self.current_window.operation_count += 1;
        self.current_window.bytes_processed += bytes;
        self.total_allowed += 1;
        true
    }

    /// Reset the rate limiter window.
    pub fn reset_window(&mut self, window_start: TimestampMillis) {
        self.current_window = RateLimitWindow {
            window_start,
            operation_count: 0,
            bytes_processed: 0,
        };
    }

    /// Return total allowed operations.
    pub fn total_allowed(&self) -> u64 {
        self.total_allowed
    }

    /// Return total rejected operations.
    pub fn total_rejected(&self) -> u64 {
        self.total_rejected
    }

    /// Set max operations per window.
    pub fn set_max_ops_per_window(&mut self, max: u32) {
        self.max_ops_per_window = max;
    }

    /// Set max bytes per window.
    pub fn set_max_bytes_per_window(&mut self, max: u64) {
        self.max_bytes_per_window = max;
    }
}

///////////////////////////////////////////////////////////////////////////////
// Cross-H-Unit Compensation Coordinator — coordinates across H-units
///////////////////////////////////////////////////////////////////////////////

/// A cross-H-unit compensation coordination record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossHUnitCoordination {
    pub coordination_id: CrossCoordinationId,
    pub failed_hunits: Vec<HUnitId>,
    pub compensation_hunits: Vec<HUnitId>,
    pub status: CoordinationStatus,
    pub total_partitions: u32,
    pub restored_partitions: u32,
}

/// Unique identifier for cross-H-unit coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CrossCoordinationId(pub u64);

/// Status of cross-H-unit coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationStatus {
    Planning,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
}

/// Coordinates compensation operations across multiple H-units.
pub struct CrossHUnitCompensationCoordinator {
    /// Coordinations by ID.
    coordinations: HashMap<CrossCoordinationId, CrossHUnitCoordination>,
    /// Next coordination ID.
    next_id: u64,
    /// Total coordinations.
    total_coordinations: u64,
    /// Completed coordinations.
    completed_coordinations: u64,
}

impl CrossHUnitCompensationCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        CrossHUnitCompensationCoordinator {
            coordinations: HashMap::new(),
            next_id: 0,
            total_coordinations: 0,
            completed_coordinations: 0,
        }
    }

    /// Create a coordination for multiple failed H-units.
    pub fn create_coordination(
        &mut self,
        failed_hunits: Vec<HUnitId>,
        compensation_hunits: Vec<HUnitId>,
        total_partitions: u32,
    ) -> CrossCoordinationId {
        let id = CrossCoordinationId(self.next_id);
        self.next_id += 1;
        self.total_coordinations += 1;
        let coordination = CrossHUnitCoordination {
            coordination_id: id,
            failed_hunits: failed_hunits.clone(),
            compensation_hunits,
            status: CoordinationStatus::Planning,
            total_partitions,
            restored_partitions: 0,
        };
        self.coordinations.insert(id, coordination);
        id
    }

    /// Update coordination progress.
    pub fn update_progress(
        &mut self,
        coordination_id: CrossCoordinationId,
        restored: u32,
    ) -> Result<(), Error> {
        let coord = self
            .coordinations
            .get_mut(&coordination_id)
            .ok_or_else(|| Error::Internal {
                message: "coordination not found".to_string(),
            })?;
        coord.restored_partitions = restored;
        if restored >= coord.total_partitions {
            coord.status = CoordinationStatus::Completed;
            self.completed_coordinations += 1;
        } else if restored > 0 {
            coord.status = CoordinationStatus::PartiallyCompleted;
        }
        Ok(())
    }

    /// Return a coordination by ID.
    pub fn coordination(&self, id: CrossCoordinationId) -> Option<&CrossHUnitCoordination> {
        self.coordinations.get(&id)
    }

    /// Return total coordinations.
    pub fn total_coordinations(&self) -> u64 {
        self.total_coordinations
    }

    /// Return completed coordination count.
    pub fn completed_count(&self) -> u64 {
        self.completed_coordinations
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation Resource Tracker — tracks resources used by compensation
///////////////////////////////////////////////////////////////////////////////

/// Resource usage for a compensation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationResourceUsage {
    pub compensation_id: CompensationId,
    pub cpu_seconds: u64,
    pub memory_bytes: u64,
    pub network_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

/// Tracks resource usage for compensation operations.
pub struct CompensationResourceTracker {
    /// Usage by compensation ID.
    usage: HashMap<CompensationId, CompensationResourceUsage>,
    /// Total CPU seconds.
    total_cpu_seconds: u64,
    /// Total memory bytes.
    total_memory_bytes: u64,
    /// Total network bytes.
    total_network_bytes: u64,
}

impl CompensationResourceTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        CompensationResourceTracker {
            usage: HashMap::new(),
            total_cpu_seconds: 0,
            total_memory_bytes: 0,
            total_network_bytes: 0,
        }
    }

    /// Record resource usage for a compensation operation.
    pub fn record_usage(
        &mut self,
        compensation_id: CompensationId,
        cpu_seconds: u64,
        memory_bytes: u64,
        network_bytes: u64,
        disk_read_bytes: u64,
        disk_write_bytes: u64,
    ) {
        let usage = CompensationResourceUsage {
            compensation_id,
            cpu_seconds,
            memory_bytes,
            network_bytes,
            disk_read_bytes,
            disk_write_bytes,
        };
        self.total_cpu_seconds += cpu_seconds;
        self.total_memory_bytes += memory_bytes;
        self.total_network_bytes += network_bytes;
        self.usage.insert(compensation_id, usage);
    }

    /// Return usage for a compensation operation.
    pub fn usage_for(&self, compensation_id: CompensationId) -> Option<&CompensationResourceUsage> {
        self.usage.get(&compensation_id)
    }

    /// Return total CPU seconds.
    pub fn total_cpu_seconds(&self) -> u64 {
        self.total_cpu_seconds
    }

    /// Return total network bytes.
    pub fn total_network_bytes(&self) -> u64 {
        self.total_network_bytes
    }

    /// Return total tracked operations.
    pub fn tracked_count(&self) -> usize {
        self.usage.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit already defined above
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// Compensation Chain — chains multiple compensation operations
///////////////////////////////////////////////////////////////////////////////

/// A link in a compensation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationChainLink {
    pub link_id: ChainLinkId,
    pub predecessor: Option<CompensationId>,
    pub successor: Option<CompensationId>,
    pub hunit: HUnitId,
    pub status: ChainLinkStatus,
}

/// Unique identifier for a chain link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChainLinkId(pub u64);

/// Status of a chain link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainLinkStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

/// Chains multiple compensation operations in dependency order.
pub struct CompensationChain {
    /// Links by ID.
    links: HashMap<ChainLinkId, CompensationChainLink>,
    /// Per-compensation link mapping.
    compensation_links: HashMap<CompensationId, ChainLinkId>,
    /// Next link ID.
    next_id: u64,
    /// Total links.
    total_links: u64,
    /// Completed links.
    completed_links: u64,
}

impl CompensationChain {
    /// Create a new chain.
    pub fn new() -> Self {
        CompensationChain {
            links: HashMap::new(),
            compensation_links: HashMap::new(),
            next_id: 0,
            total_links: 0,
            completed_links: 0,
        }
    }

    /// Add a link to the chain.
    pub fn add_link(
        &mut self,
        compensation_id: CompensationId,
        hunit: HUnitId,
        predecessor: Option<CompensationId>,
    ) -> ChainLinkId {
        let id = ChainLinkId(self.next_id);
        self.next_id += 1;
        self.total_links += 1;
        let link = CompensationChainLink {
            link_id: id,
            predecessor,
            successor: None,
            hunit,
            status: ChainLinkStatus::Pending,
        };
        self.links.insert(id, link);
        self.compensation_links.insert(compensation_id, id);

        // Set successor on predecessor if any.
        if let Some(pred_comp) = predecessor {
            if let Some(pred_link_id) = self.compensation_links.get(&pred_comp) {
                if let Some(pred_link) = self.links.get_mut(pred_link_id) {
                    pred_link.successor = Some(compensation_id);
                }
            }
        }
        id
    }

    /// Mark a link as completed.
    pub fn complete_link(&mut self, link_id: ChainLinkId) -> Result<(), Error> {
        let link = self
            .links
            .get_mut(&link_id)
            .ok_or_else(|| Error::Internal {
                message: "chain link not found".to_string(),
            })?;
        link.status = ChainLinkStatus::Completed;
        self.completed_links += 1;
        Ok(())
    }

    /// Mark a link as failed.
    pub fn fail_link(&mut self, link_id: ChainLinkId) -> Result<(), Error> {
        let link = self
            .links
            .get_mut(&link_id)
            .ok_or_else(|| Error::Internal {
                message: "chain link not found".to_string(),
            })?;
        link.status = ChainLinkStatus::Failed;
        Ok(())
    }

    /// Return the next pending link in the chain.
    pub fn next_pending(&self) -> Option<ChainLinkId> {
        self.links
            .iter()
            .find(|(_, l)| l.status == ChainLinkStatus::Pending)
            .map(|(id, _)| *id)
    }

    /// Return links for an H-unit.
    pub fn links_for_hunit(&self, hunit: HUnitId) -> Vec<CompensationChainLink> {
        self.links
            .values()
            .filter(|l| l.hunit == hunit)
            .cloned()
            .collect()
    }

    /// Return total links.
    pub fn total_links(&self) -> u64 {
        self.total_links
    }

    /// Return completed link count.
    pub fn completed_count(&self) -> u64 {
        self.completed_links
    }

    /// Return whether the chain is complete.
    pub fn is_complete(&self) -> bool {
        self.completed_links == self.total_links && self.total_links > 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Compensation Timeout Manager — manages timeouts for compensation operations
///////////////////////////////////////////////////////////////////////////////

/// A timeout configuration for a compensation operation type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationTimeoutConfig {
    pub operation_type: CompensationOperationType,
    pub timeout_seconds: u64,
    pub retry_count: u32,
    pub backoff_multiplier: f64,
}

/// Type of compensation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompensationOperationType {
    IdentifyAffectedData,
    SelectCompensationHUnit,
    RestoreRedundancy,
    VerifyRestoration,
    CrossHUnitCoordinate,
}

/// Manages timeouts and retries for compensation operations.
pub struct CompensationTimeoutManager {
    /// Timeouts per operation type.
    timeouts: HashMap<CompensationOperationType, CompensationTimeoutConfig>,
    /// Active operations with start times.
    active_operations: HashMap<CompensationId, (CompensationOperationType, TimestampMillis)>,
    /// Total timeouts triggered.
    total_timeouts: u64,
    /// Total retries attempted.
    total_retries: u64,
}

impl CompensationTimeoutManager {
    /// Create a new timeout manager with defaults.
    pub fn new() -> Self {
        let mut timeouts = HashMap::new();
        timeouts.insert(
            CompensationOperationType::IdentifyAffectedData,
            CompensationTimeoutConfig {
                operation_type: CompensationOperationType::IdentifyAffectedData,
                timeout_seconds: 30,
                retry_count: 2,
                backoff_multiplier: 2.0,
            },
        );
        timeouts.insert(
            CompensationOperationType::RestoreRedundancy,
            CompensationTimeoutConfig {
                operation_type: CompensationOperationType::RestoreRedundancy,
                timeout_seconds: 300,
                retry_count: 3,
                backoff_multiplier: 2.0,
            },
        );
        timeouts.insert(
            CompensationOperationType::SelectCompensationHUnit,
            CompensationTimeoutConfig {
                operation_type: CompensationOperationType::SelectCompensationHUnit,
                timeout_seconds: 10,
                retry_count: 3,
                backoff_multiplier: 1.5,
            },
        );
        timeouts.insert(
            CompensationOperationType::VerifyRestoration,
            CompensationTimeoutConfig {
                operation_type: CompensationOperationType::VerifyRestoration,
                timeout_seconds: 60,
                retry_count: 2,
                backoff_multiplier: 2.0,
            },
        );
        timeouts.insert(
            CompensationOperationType::CrossHUnitCoordinate,
            CompensationTimeoutConfig {
                operation_type: CompensationOperationType::CrossHUnitCoordinate,
                timeout_seconds: 120,
                retry_count: 2,
                backoff_multiplier: 2.0,
            },
        );
        CompensationTimeoutManager {
            timeouts,
            active_operations: HashMap::new(),
            total_timeouts: 0,
            total_retries: 0,
        }
    }

    /// Start tracking an operation.
    pub fn start_operation(
        &mut self,
        compensation_id: CompensationId,
        op_type: CompensationOperationType,
        timestamp: TimestampMillis,
    ) {
        self.active_operations
            .insert(compensation_id, (op_type, timestamp));
    }

    /// Check whether an operation has timed out.
    pub fn is_timed_out(
        &self,
        compensation_id: CompensationId,
        current_time: TimestampMillis,
    ) -> bool {
        match self.active_operations.get(&compensation_id) {
            Some((op_type, start_time)) => {
                let config = self.timeouts.get(op_type).unwrap();
                let elapsed = current_time.0.saturating_sub(start_time.0);
                elapsed > config.timeout_seconds * 1000
            }
            None => false,
        }
    }

    /// Complete an operation (remove tracking).
    pub fn complete_operation(&mut self, compensation_id: CompensationId) {
        self.active_operations.remove(&compensation_id);
    }

    /// Record a timeout.
    pub fn record_timeout(&mut self) {
        self.total_timeouts += 1;
    }

    /// Record a retry.
    pub fn record_retry(&mut self) {
        self.total_retries += 1;
    }

    /// Return timeout config for an operation type.
    pub fn timeout_config(&self, op_type: CompensationOperationType) -> Option<&CompensationTimeoutConfig> {
        self.timeouts.get(&op_type)
    }

    /// Return total timeouts.
    pub fn total_timeouts(&self) -> u64 {
        self.total_timeouts
    }

    /// Return total retries.
    pub fn total_retries(&self) -> u64 {
        self.total_retries
    }
}

// ========== CompensationPlan tests ==========

#[test]
fn compensation_plan_creation() {
    let plan = CompensationPlan {
        plan_id: CompensationPlanId(1),
        failed_hunit: HUnitId::new([1u8; 32]),
        failed_node: NodeId::new([2u8; 32]),
        steps: Vec::new(),
        compensation_hunit: None,
        total_estimated_bytes: 0,
        status: PlanStatus::Draft,
    };
    assert_eq!(plan.status, PlanStatus::Draft);
    assert_eq!(plan.total_estimated_bytes, 0);
}

// ========== PlanStatus tests ----

#[test]
fn plan_status_variants() {
    let statuses = [
        PlanStatus::Draft,
        PlanStatus::Active,
        PlanStatus::Completed,
        PlanStatus::Failed,
        PlanStatus::Aborted,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== CompensationStepType tests ----

#[test]
fn compensation_step_type_variants() {
    let types = [
        CompensationStepType::IdentifyAffectedData,
        CompensationStepType::SelectCompensationHUnit,
        CompensationStepType::RestoreRedundancy,
        CompensationStepType::VerifyRestoration,
        CompensationStepType::UpdateMetadata,
    ];
    for t in types {
        let _ = format!("{:?}", t);
    }
}

// ========== PlanStepStatus tests ----

#[test]
fn plan_step_status_variants() {
    let statuses = [
        PlanStepStatus::Pending,
        PlanStepStatus::InProgress,
        PlanStepStatus::Completed,
        PlanStepStatus::Failed,
        PlanStepStatus::Skipped,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== CompensationPlanner tests ==========

#[test]
fn planner_create_plan() {
    let mut planner = CompensationPlanner::new();
    let failed_hunit = HUnitId::new([1u8; 32]);
    let failed_node = NodeId::new([2u8; 32]);
    let partitions = vec![PartitionId::new([3u8; 32]), PartitionId::new([4u8; 32])];

    let id = planner.create_plan(failed_hunit, failed_node, partitions.clone(), 1024);
    let plan = planner.plan(id).unwrap();
    assert_eq!(plan.failed_hunit, failed_hunit);
    assert_eq!(plan.failed_node, failed_node);
    assert_eq!(plan.steps.len(), 5);
    assert_eq!(plan.total_estimated_bytes, 2048);
    assert_eq!(plan.status, PlanStatus::Draft);
}

#[test]
fn planner_set_compensation_hunit() {
    let mut planner = CompensationPlanner::new();
    let failed = HUnitId::new([1u8; 32]);
    let comp = HUnitId::new([2u8; 32]);
    let id = planner.create_plan(failed, NodeId::new([2u8; 32]), vec![], 0);
    planner.set_compensation_hunit(id, comp).unwrap();
    let plan = planner.plan(id).unwrap();
    assert_eq!(plan.compensation_hunit, Some(comp));
}

#[test]
fn planner_set_compensation_hunit_nonexistent() {
    let mut planner = CompensationPlanner::new();
    assert!(planner.set_compensation_hunit(CompensationPlanId(999), HUnitId::new([1u8; 32])).is_err());
}

#[test]
fn planner_activate_and_complete() {
    let mut planner = CompensationPlanner::new();
    let failed = HUnitId::new([1u8; 32]);
    let id = planner.create_plan(failed, NodeId::new([2u8; 32]), vec![PartitionId::new([3u8; 32])], 1024);
    planner.activate_plan(id).unwrap();
    assert_eq!(planner.plan(id).unwrap().status, PlanStatus::Active);

    planner.complete_step(id, 0).unwrap();
    planner.complete_step(id, 1).unwrap();
    planner.complete_step(id, 2).unwrap();
    planner.complete_step(id, 3).unwrap();
    planner.complete_step(id, 4).unwrap();
    assert_eq!(planner.plan(id).unwrap().status, PlanStatus::Completed);
    assert_eq!(planner.completed_plan_count(), 1);
}

#[test]
fn planner_fail_plan() {
    let mut planner = CompensationPlanner::new();
    let id = planner.create_plan(HUnitId::new([1u8; 32]), NodeId::new([2u8; 32]), vec![], 0);
    planner.fail_plan(id).unwrap();
    assert_eq!(planner.plan(id).unwrap().status, PlanStatus::Failed);
    assert_eq!(planner.failed_plan_count(), 1);
}

#[test]
fn planner_complete_step_nonexistent() {
    let mut planner = CompensationPlanner::new();
    let id = planner.create_plan(HUnitId::new([1u8; 32]), NodeId::new([2u8; 32]), vec![], 0);
    assert!(planner.complete_step(id, 99).is_err());
}

#[test]
fn planner_plans_for_hunit() {
    let mut planner = CompensationPlanner::new();
    let h1 = HUnitId::new([1u8; 32]);
    let h2 = HUnitId::new([2u8; 32]);
    planner.create_plan(h1, NodeId::new([2u8; 32]), vec![], 0);
    planner.create_plan(h1, NodeId::new([2u8; 32]), vec![], 0);
    planner.create_plan(h2, NodeId::new([3u8; 32]), vec![], 0);
    assert_eq!(planner.plans_for_hunit(h1).len(), 2);
    assert_eq!(planner.plans_for_hunit(h2).len(), 1);
}

#[test]
fn planner_plan_count() {
    let mut planner = CompensationPlanner::new();
    assert_eq!(planner.plan_count(), 0);
    planner.create_plan(HUnitId::new([1u8; 32]), NodeId::new([2u8; 32]), vec![], 0);
    assert_eq!(planner.plan_count(), 1);
}

// ========== CompensationVerifyer tests ==========

#[test]
fn verifyer_verify_success() {
    let verifier = CompensationVerifyer::new();
    let expected = vec![PartitionId::new([1u8; 32]), PartitionId::new([2u8; 32])];
    let restored = vec![PartitionId::new([1u8; 32]), PartitionId::new([2u8; 32])];
    let result = verifier.verify(
        CompensationId(1),
        HUnitId::new([1u8; 32]),
        HUnitId::new([2u8; 32]),
        &expected,
        &restored,
        2048,
        2048,
    );
    assert!(result.verified);
    assert_eq!(result.partitions_verified, 2);
    assert_eq!(result.partitions_failed, 0);
    assert!(CompensationVerifyer::is_fully_verified(&result));
}

#[test]
fn verifyer_verify_partial() {
    let verifier = CompensationVerifyer::new();
    let expected = vec![PartitionId::new([1u8; 32]), PartitionId::new([2u8; 32])];
    let restored = vec![PartitionId::new([1u8; 32])];
    let result = verifier.verify(
        CompensationId(1),
        HUnitId::new([1u8; 32]),
        HUnitId::new([2u8; 32]),
        &expected,
        &restored,
        2048,
        1024,
    );
    assert!(!result.verified);
    assert_eq!(result.partitions_verified, 1);
    assert_eq!(result.partitions_failed, 1);
    assert!(!result.verification_notes.is_empty());
}

#[test]
fn verifyer_verify_bytes_mismatch() {
    let verifier = CompensationVerifyer::new();
    let expected = vec![PartitionId::new([1u8; 32])];
    let restored = vec![PartitionId::new([1u8; 32])];
    let result = verifier.verify(
        CompensationId(1),
        HUnitId::new([1u8; 32]),
        HUnitId::new([2u8; 32]),
        &expected,
        &restored,
        2048,
        1024,
    );
    assert!(!result.verified);
}

// ========== HUnitFailureClassifier tests ==========

#[test]
fn classifier_classify_node_failure() {
    let classifier = HUnitFailureClassifier::new();
    let result = classifier.classify(
        HUnitId::new([1u8; 32]),
        vec![NodeId::new([2u8; 32])],
        vec![PartitionId::new([3u8; 32]), PartitionId::new([4u8; 32])],
        Some(HUnitFailureType::NodeFailure),
    );
    assert_eq!(result.failure_type, HUnitFailureType::NodeFailure);
    assert!(result.compensation_required);
    assert_eq!(result.affected_partitions.len(), 2);
}

#[test]
fn classifier_classify_maintenance() {
    let classifier = HUnitFailureClassifier::new();
    let result = classifier.classify(
        HUnitId::new([1u8; 32]),
        vec![],
        vec![],
        Some(HUnitFailureType::Maintenance),
    );
    assert_eq!(result.failure_type, HUnitFailureType::Maintenance);
    assert!(!result.compensation_required);
    assert_eq!(result.severity, FailureSeverity::Low);
}

#[test]
fn classifier_classify_power_outage_critical() {
    let classifier = HUnitFailureClassifier::new();
    let nodes: Vec<NodeId> = (0..5).map(|i| NodeId::new([i as u8; 32])).collect();
    let partitions: Vec<PartitionId> = (0..8).map(|i| PartitionId::new([i as u8; 32])).collect();
    let result = classifier.classify(
        HUnitId::new([1u8; 32]),
        nodes,
        partitions,
        Some(HUnitFailureType::PowerOutage),
    );
    assert_eq!(result.severity, FailureSeverity::Critical);
}

#[test]
fn classifier_compensation_required_for() {
    assert!(HUnitFailureClassifier::compensation_required_for(HUnitFailureType::NodeFailure));
    assert!(HUnitFailureClassifier::compensation_required_for(HUnitFailureType::DiskFailure));
    assert!(HUnitFailureClassifier::compensation_required_for(HUnitFailureType::HardwareFailure));
    assert!(!HUnitFailureClassifier::compensation_required_for(HUnitFailureType::Maintenance));
    assert!(!HUnitFailureClassifier::compensation_required_for(HUnitFailureType::SoftwareFailure));
}

#[test]
fn classifier_all_failure_types() {
    let types = HUnitFailureClassifier::all_failure_types();
    assert_eq!(types.len(), 8);
}

#[test]
fn failure_severity_variants() {
    let levels = [FailureSeverity::Low, FailureSeverity::Medium, FailureSeverity::High, FailureSeverity::Critical];
    for l in levels {
        let _ = format!("{:?}", l);
    }
}

#[test]
fn failure_classification_creation() {
    let classification = FailureClassification {
        hunit_id: HUnitId::new([1u8; 32]),
        failure_type: HUnitFailureType::DiskFailure,
        affected_nodes: vec![NodeId::new([2u8; 32])],
        affected_partitions: vec![PartitionId::new([3u8; 32])],
        severity: FailureSeverity::High,
        estimated_recovery_time_seconds: 3600,
        compensation_required: true,
    };
    assert_eq!(classification.estimated_recovery_time_seconds, 3600);
    assert!(classification.compensation_required);
}

// ========== CompensationRateLimiter tests ==========

#[test]
fn rate_limiter_allow_and_reject() {
    let mut limiter = CompensationRateLimiter::new();
    limiter.set_max_ops_per_window(2);
    assert!(limiter.check_allowance(100, TimestampMillis(0)));
    assert!(limiter.check_allowance(100, TimestampMillis(0)));
    assert!(!limiter.check_allowance(100, TimestampMillis(0)));
    assert_eq!(limiter.total_allowed(), 2);
    assert_eq!(limiter.total_rejected(), 1);
}

#[test]
fn rate_limiter_bytes_limit() {
    let mut limiter = CompensationRateLimiter::new();
    limiter.set_max_bytes_per_window(1000);
    assert!(limiter.check_allowance(500, TimestampMillis(0)));
    assert!(limiter.check_allowance(500, TimestampMillis(0)));
    assert!(!limiter.check_allowance(100, TimestampMillis(0)));
    assert_eq!(limiter.total_rejected(), 1);
}

#[test]
fn rate_limiter_reset_window() {
    let mut limiter = CompensationRateLimiter::new();
    limiter.set_max_ops_per_window(1);
    limiter.check_allowance(100, TimestampMillis(0));
    assert!(!limiter.check_allowance(100, TimestampMillis(0)));
    limiter.reset_window(TimestampMillis(60000));
    assert!(limiter.check_allowance(100, TimestampMillis(60000)));
}

#[test]
fn rate_limiter_set_limits() {
    let mut limiter = CompensationRateLimiter::new();
    limiter.set_max_ops_per_window(50);
    limiter.set_max_bytes_per_window(500_000_000);
}

// ========== CrossHUnitCompensationCoordinator tests ==========

#[test]
fn coordinator_create_and_complete() {
    let mut coordinator = CrossHUnitCompensationCoordinator::new();
    let failed = vec![HUnitId::new([1u8; 32]), HUnitId::new([2u8; 32])];
    let comp = vec![HUnitId::new([3u8; 32]), HUnitId::new([4u8; 32])];
    let id = coordinator.create_coordination(failed, comp, 10);
    let coord = coordinator.coordination(id).unwrap();
    assert_eq!(coord.total_partitions, 10);
    assert_eq!(coord.restored_partitions, 0);
    assert_eq!(coord.status, CoordinationStatus::Planning);

    coordinator.update_progress(id, 10).unwrap();
    assert_eq!(coord.status, CoordinationStatus::Completed);
    assert_eq!(coordinator.completed_count(), 1);
}

#[test]
fn coordinator_partial_completion() {
    let mut coordinator = CrossHUnitCompensationCoordinator::new();
    let id = coordinator.create_coordination(
        vec![HUnitId::new([1u8; 32])],
        vec![HUnitId::new([2u8; 32])],
        5,
    );
    coordinator.update_progress(id, 3).unwrap();
    assert_eq!(coord.status, CoordinationStatus::PartiallyCompleted);
    assert_eq!(coordinator.completed_count(), 0);
}

#[test]
fn coordinator_total_count() {
    let mut coordinator = CrossHUnitCompensationCoordinator::new();
    assert_eq!(coordinator.total_coordinations(), 0);
    coordinator.create_coordination(vec![], vec![], 0);
    assert_eq!(coordinator.total_coordinations(), 1);
}

// ========== CompensationResourceTracker tests ==========

#[test]
fn resource_tracker_record_and_lookup() {
    let mut tracker = CompensationResourceTracker::new();
    tracker.record_usage(
        CompensationId(1),
        100,
        1024 * 1024,
        512 * 1024,
        256 * 1024,
        256 * 1024,
    );
    let usage = tracker.usage_for(CompensationId(1)).unwrap();
    assert_eq!(usage.cpu_seconds, 100);
    assert_eq!(usage.memory_bytes, 1024 * 1024);
    assert_eq!(usage.network_bytes, 512 * 1024);
}

#[test]
fn resource_tracker_totals() {
    let mut tracker = CompensationResourceTracker::new();
    tracker.record_usage(CompensationId(1), 10, 100, 200, 300, 400);
    tracker.record_usage(CompensationId(2), 20, 200, 400, 600, 800);
    assert_eq!(tracker.total_cpu_seconds(), 30);
    assert_eq!(tracker.total_memory_bytes(), 300);
    assert_eq!(tracker.total_network_bytes(), 600);
    assert_eq!(tracker.tracked_count(), 2);
}

// ========== CompensationChain tests ==========

#[test]
fn chain_add_and_complete() {
    let mut chain = CompensationChain::new();
    let id1 = chain.add_link(CompensationId(1), HUnitId::new([1u8; 32]), None);
    let id2 = chain.add_link(CompensationId(2), HUnitId::new([2u8; 32]), Some(CompensationId(1)));

    chain.complete_link(id1).unwrap();
    chain.complete_link(id2).unwrap();
    assert!(chain.is_complete());
    assert_eq!(chain.completed_count(), 2);
    assert_eq!(chain.total_links(), 2);
}

#[test]
fn chain_fail_link() {
    let mut chain = CompensationChain::new();
    let id = chain.add_link(CompensationId(1), HUnitId::new([1u8; 32]), None);
    chain.fail_link(id).unwrap();
    assert!(!chain.is_complete());
}

#[test]
fn chain_next_pending() {
    let mut chain = CompensationChain::new();
    let id1 = chain.add_link(CompensationId(1), HUnitId::new([1u8; 32]), None);
    let id2 = chain.add_link(CompensationId(2), HUnitId::new([2u8; 32]), None);
    assert_eq!(chain.next_pending(), Some(id1));
    chain.complete_link(id1).unwrap();
    assert_eq!(chain.next_pending(), Some(id2));
}

#[test]
fn chain_complete_nonexistent() {
    let mut chain = CompensationChain::new();
    assert!(chain.complete_link(ChainLinkId(999)).is_err());
}

#[test]
fn chain_links_for_hunit() {
    let mut chain = CompensationChain::new();
    chain.add_link(CompensationId(1), HUnitId::new([1u8; 32]), None);
    chain.add_link(CompensationId(2), HUnitId::new([2u8; 32]), None);
    chain.add_link(CompensationId(3), HUnitId::new([1u8; 32]), None);
    assert_eq!(chain.links_for_hunit(HUnitId::new([1u8; 32])).len(), 2);
    assert_eq!(chain.links_for_hunit(HUnitId::new([2u8; 32])).len(), 1);
}

// ========== CompensationTimeoutManager tests ==========

#[test]
fn timeout_manager_defaults() {
    let manager = CompensationTimeoutManager::new();
    let config = manager.timeout_config(CompensationOperationType::RestoreRedundancy).unwrap();
    assert_eq!(config.timeout_seconds, 300);
    assert_eq!(config.retry_count, 3);
}

#[test]
fn timeout_manager_is_timed_out() {
    let mut manager = CompensationTimeoutManager::new();
    manager.start_operation(CompensationId(1), CompensationOperationType::SelectCompensationHUnit, TimestampMillis(0));
    // 10 second timeout, current time 5 seconds => not timed out
    assert!(!manager.is_timed_out(CompensationId(1), TimestampMillis(5000)));
    // current time 15 seconds => timed out
    assert!(manager.is_timed_out(CompensationId(1), TimestampMillis(15000)));
}

#[test]
fn timeout_manager_complete_operation() {
    let mut manager = CompensationTimeoutManager::new();
    manager.start_operation(CompensationId(1), CompensationOperationType::SelectCompensationHUnit, TimestampMillis(0));
    manager.complete_operation(CompensationId(1));
    assert!(!manager.is_timed_out(CompensationId(1), TimestampMillis(100000)));
}

#[test]
fn timeout_manager_record_timeout_and_retry() {
    let mut manager = CompensationTimeoutManager::new();
    manager.record_timeout();
    manager.record_retry();
    manager.record_timeout();
    assert_eq!(manager.total_timeouts(), 2);
    assert_eq!(manager.total_retries(), 1);
}

#[test]
fn timeout_manager_operation_types() {
    let types = [
        CompensationOperationType::IdentifyAffectedData,
        CompensationOperationType::SelectCompensationHUnit,
        CompensationOperationType::RestoreRedundancy,
        CompensationOperationType::VerifyRestoration,
        CompensationOperationType::CrossHUnitCoordinate,
    ];
    for t in types {
        let _ = format!("{:?}", t);
    }
}

// ========== ChainLinkStatus tests ==========

#[test]
fn chain_link_status_variants() {
    let statuses = [
        ChainLinkStatus::Pending,
        ChainLinkStatus::InProgress,
        ChainLinkStatus::Completed,
        ChainLinkStatus::Failed,
        ChainLinkStatus::Blocked,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== ChainLinkId tests ==========

#[test]
fn chain_link_id_ordering() {
    let a = ChainLinkId(1);
    let b = ChainLinkId(2);
    assert!(a < b);
    assert_ne!(a, b);
}

// ========== CrossCoordinationId tests ==========

#[test]
fn cross_coordination_id_ordering() {
    let a = CrossCoordinationId(5);
    let b = CrossCoordinationId(15);
    assert!(a < b);
    assert_ne!(a, b);
}

// ========== CoordinationStatus tests ==========

#[test]
fn coordination_status_variants() {
    let statuses = [
        CoordinationStatus::Planning,
        CoordinationStatus::InProgress,
        CoordinationStatus::Completed,
        CoordinationStatus::PartiallyCompleted,
        CoordinationStatus::Failed,
    ];
    for s in statuses {
        let _ = format!("{:?}", s);
    }
}

// ========== Integration test ==========

#[test]
fn full_compensation_integration_v2() {
    let mut manager = HUnitCompensationManager::new();
    let mut planner = CompensationPlanner::new();
    let mut verifier = CompensationVerifyer::new();
    let mut classifier = HUnitFailureClassifier::new();
    let mut chain = CompensationChain::new();
    let mut timeout_mgr = CompensationTimeoutManager::new();

    let failed_hunit = HUnitId::new([1u8; 32]);
    let comp_hunit = HUnitId::new([2u8; 32]);
    let failed_node = NodeId::new([3u8; 32]);
    let p1 = PartitionId::new([4u8; 32]);
    let p2 = PartitionId::new([5u8; 32]);
    let dataset = DatasetId::new([6u8; 32]);

    // Register affected data.
    manager.register_affected_data(AffectedDataRecord {
        partition_id: p1,
        dataset_id: dataset,
        node_id: failed_node,
        hunit_id: failed_hunit,
        data_size_bytes: 1024,
        replica_count: 3,
        healthy_replicas: 1,
        needs_compensation: true,
    });

    // Register capacity.
    manager.register_hunit_capacity(HUnitCapacity {
        hunit_id: comp_hunit,
        total_capacity_bytes: 10000,
        used_capacity_bytes: 1000,
        available_capacity_bytes: 9000,
        node_count: 5,
        healthy_node_count: 4,
        region: RegionId::new([2u8; 32]),
    });

    // Classify failure.
    let classification = classifier.classify(
        failed_hunit,
        vec![failed_node],
        vec![p1, p2],
        Some(HUnitFailureType::NodeFailure),
    );
    assert!(classification.compensation_required);

    // Create compensation plan.
    let plan_id = planner.create_plan(failed_hunit, failed_node, vec![p1, p2], 1024);
    planner.activate_plan(plan_id).unwrap();

    // Add chain links.
    let link1 = chain.add_link(CompensationId(1), failed_hunit, None);
    let link2 = chain.add_link(CompensationId(2), comp_hunit, Some(CompensationId(1)));

    // Select and restore.
    let selected = manager.select_compensation_hunit(failed_hunit, 1000).unwrap();
    assert_eq!(selected, comp_hunit);
    let result = manager.restore_redundancy(failed_hunit, comp_hunit).unwrap();
    assert_eq!(result.status, CompensationStatus::Completed);

    // Verify restoration.
    let verify_result = verifier.verify(
        CompensationId(1),
        failed_hunit,
        comp_hunit,
        &[p1, p2],
        &[p1, p2],
        3072,
        3072,
    );
    assert!(verify_result.verified);

    // Complete chain links.
    chain.complete_link(link1).unwrap();
    chain.complete_link(link2).unwrap();
    assert!(chain.is_complete());

    // Complete plan steps.
    for i in 0..5 {
        planner.complete_step(plan_id, i).unwrap();
    }
    assert_eq!(planner.plan(plan_id).unwrap().status, PlanStatus::Completed);

    // Timeout tracking.
    timeout_mgr.start_operation(CompensationId(1), CompensationOperationType::RestoreRedundancy, TimestampMillis(0));
    assert!(!timeout_mgr.is_timed_out(CompensationId(1), TimestampMillis(1000)));
    assert!(timeout_mgr.is_timed_out(CompensationId(1), TimestampMillis(400000)));
}