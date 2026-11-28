///////////////////////////////////////////////////////////////////////////////
// Hybrid Redundancy Invariant — full replication + erasure coding coexist
//
// IDS-A §37: full replication and erasure coding coexist per dataset. Each
// dataset can independently choose its redundancy mode, and the system must
// guarantee that both modes operate correctly and consistently alongside each
// other. Mode switching must be safe — data is never lost during transition.
//
// Every hybrid redundancy decision follows a strict model:
//   1. Determine the redundancy mode for a dataset
//   2. Validate that replica counts meet the mode's requirements
//   3. Track which replicas are full copies vs erasure-coded fragments
//   4. Enforce safe mode transitions (full ↔ erasure-coded)
//   5. Verify data integrity across both redundancy types
//
// This file is part of ids-000-aa, the central hub crate. All types and
// traits here are sealed — downstream crates cannot implement them.
//
// Carrier-agnostic: uses only opaque Endpoint types (CarrierType + Vec<u8>).
// No std::net, SocketAddr, IP, TCP, UDP, or any existing networking stack.
///////////////////////////////////////////////////////////////////////////////

use crate::types::{DatasetId, NodeId, RedundancyMode};
use crate::errors::Error;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait contract
///////////////////////////////////////////////////////////////////////////////

/// Sealed trait guaranteeing full replication and erasure coding coexist
/// per dataset.
///
/// Downstream crates may call methods and use as trait bounds, but cannot
/// implement — only `ids-000-aa` and its designated implementation crates.
pub trait HybridRedundancy: private::Sealed {
    /// Return the redundancy mode for a dataset.
    fn redundancy_mode(&self, dataset_id: DatasetId) -> RedundancyMode;

    /// Return the full replica count for a dataset.
    fn full_replica_count(&self, dataset_id: DatasetId) -> u32;

    /// Return the erasure-coded parameters (k, n) for a dataset.
    fn erasure_coded_params(&self, dataset_id: DatasetId) -> (u32, u32);

    /// Return whether a mode switch is safe for a dataset.
    fn mode_switch_safe(
        &self,
        dataset_id: DatasetId,
        from: RedundancyMode,
        to: RedundancyMode,
    ) -> bool;

    /// Return the total redundancy factor for a dataset.
    fn total_redundancy_factor(&self, dataset_id: DatasetId) -> f64;

    /// Return whether a dataset uses hybrid redundancy.
    fn is_hybrid(&self, dataset_id: DatasetId) -> bool;
}

mod private {
    /// Sealed supertrait — only ids-000-aa can implement HybridRedundancy.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl HybridRedundancy for () {
    fn redundancy_mode(&self, _dataset_id: DatasetId) -> RedundancyMode {
        RedundancyMode::FullReplica
    }
    fn full_replica_count(&self, _dataset_id: DatasetId) -> u32 {
        1
    }
    fn erasure_coded_params(&self, _dataset_id: DatasetId) -> (u32, u32) {
        (0, 0)
    }
    fn mode_switch_safe(
        &self,
        _dataset_id: DatasetId,
        _from: RedundancyMode,
        _to: RedundancyMode,
    ) -> bool {
        false
    }
    fn total_redundancy_factor(&self, _dataset_id: DatasetId) -> f64 {
        1.0
    }
    fn is_hybrid(&self, _dataset_id: DatasetId) -> bool {
        false
    }
}

///////////////////////////////////////////////////////////////////////////////
// Redundancy mode tracking
///////////////////////////////////////////////////////////////////////////////

/// Redundancy configuration for a dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundancyConfig {
    pub dataset_id: DatasetId,
    pub mode: RedundancyMode,
    pub full_replicas: u32,
    pub erasure_k: u32,
    pub erasure_n: u32,
    pub hybrid_full_count: u32,
    pub hybrid_erasure_count: u32,
    pub last_mode_change: TimestampMillis,
    pub mode_change_count: u64,
}

impl RedundancyConfig {
    /// Create a new full replication config.
    pub fn full_replica(dataset_id: DatasetId, replica_count: u32) -> Self {
        RedundancyConfig {
            dataset_id,
            mode: RedundancyMode::FullReplica,
            full_replicas: replica_count,
            erasure_k: 0,
            erasure_n: 0,
            hybrid_full_count: 0,
            hybrid_erasure_count: 0,
            last_mode_change: TimestampMillis(0),
            mode_change_count: 0,
        }
    }

    /// Create a new erasure-coded config.
    pub fn erasure_coded(dataset_id: DatasetId, k: u32, n: u32) -> Self {
        RedundancyConfig {
            dataset_id,
            mode: RedundancyMode::ErasureCoded { k, n },
            full_replicas: 0,
            erasure_k: k,
            erasure_n: n,
            hybrid_full_count: 0,
            hybrid_erasure_count: 0,
            last_mode_change: TimestampMillis(0),
            mode_change_count: 0,
        }
    }

    /// Create a new hybrid config.
    pub fn hybrid(
        dataset_id: DatasetId,
        full_count: u32,
        erasure_k: u32,
        erasure_n: u32,
    ) -> Self {
        RedundancyConfig {
            dataset_id,
            mode: RedundancyMode::Hybrid,
            full_replicas: full_count,
            erasure_k,
            erasure_n,
            hybrid_full_count: full_count,
            hybrid_erasure_count: n,
            last_mode_change: TimestampMillis(0),
            mode_change_count: 0,
        }
    }

    /// Return whether the config is hybrid.
    pub fn is_hybrid(&self) -> bool {
        matches!(self.mode, RedundancyMode::Hybrid)
    }

    /// Return the total number of copies (full + erasure).
    pub fn total_copies(&self) -> u32 {
        match self.mode {
            RedundancyMode::FullReplica => self.full_replicas,
            RedundancyMode::ErasureCoded { n, .. } => n,
            RedundancyMode::Hybrid => self.hybrid_full_count + self.hybrid_erasure_count,
        }
    }

    /// Return the fault tolerance of the config.
    pub fn fault_tolerance(&self) -> u32 {
        match self.mode {
            RedundancyMode::FullReplica => self.full_replicas.saturating_sub(1),
            RedundancyMode::ErasureCoded { k, n } => n.saturating_sub(k),
            RedundancyMode::Hybrid => {
                self.full_replicas.saturating_sub(1) + self.erasure_n.saturating_sub(self.erasure_k)
            }
        }
    }
}

/// Tracks redundancy configurations for all datasets.
pub struct RedundancyConfigTracker {
    /// Configs by dataset ID.
    configs: HashMap<DatasetId, RedundancyConfig>,
    /// Per-mode dataset sets.
    mode_datasets: HashMap<RedundancyMode, BTreeSet<DatasetId>>,
    /// Total configs.
    total_configs: u64,
    /// Total mode changes.
    total_mode_changes: u64,
}

impl RedundancyConfigTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        RedundancyConfigTracker {
            configs: HashMap::new(),
            mode_datasets: HashMap::new(),
            total_configs: 0,
            total_mode_changes: 0,
        }
    }

    /// Register a redundancy config for a dataset.
    pub fn register_config(&mut self, config: RedundancyConfig) {
        let dataset = config.dataset_id;
        let old_mode = self.configs.get(&dataset).map(|c| c.mode.clone());

        // Remove from old mode set.
        if let Some(old) = old_mode {
            self.mode_datasets.entry(old).or_default().remove(&dataset);
        }

        self.configs.insert(dataset, config.clone());
        self.mode_datasets
            .entry(config.mode.clone())
            .or_default()
            .insert(dataset);

        if old_mode.is_none() {
            self.total_configs += 1;
        } else if old_mode != Some(config.mode.clone()) {
            self.total_mode_changes += 1;
        }
    }

    /// Return the config for a dataset.
    pub fn config_for(&self, dataset_id: DatasetId) -> Option<&RedundancyConfig> {
        self.configs.get(&dataset_id)
    }

    /// Return datasets for a mode.
    pub fn datasets_for_mode(&self, mode: &RedundancyMode) -> Vec<DatasetId> {
        self.mode_datasets
            .get(mode)
            .map(|ds| ds.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Return total configs.
    pub fn total_configs(&self) -> u64 {
        self.total_configs
    }

    /// Return total mode changes.
    pub fn total_mode_changes(&self) -> u64 {
        self.total_mode_changes
    }

    /// Return all dataset IDs.
    pub fn all_datasets(&self) -> Vec<DatasetId> {
        self.configs.keys().copied().collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Replica tracker — tracks which nodes hold full vs erasure replicas
///////////////////////////////////////////////////////////////////////////////

/// A replica placement record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaPlacement {
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub replica_type: ReplicaType,
    pub fragment_index: Option<u32>,
    pub size_bytes: u64,
    pub healthy: bool,
}

/// Type of replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaType {
    Full,
    ErasureFragment,
}

/// Tracks replica placements for hybrid redundancy.
pub struct ReplicaTracker {
    /// Placements by (dataset, node).
    placements: HashMap<(DatasetId, NodeId), ReplicaPlacement>,
    /// Per-dataset placements.
    dataset_placements: HashMap<DatasetId, Vec<(DatasetId, NodeId)>>,
    /// Per-node placements.
    node_placements: HashMap<NodeId, Vec<(DatasetId, NodeId)>>,
    /// Full replicas per dataset.
    full_replicas: HashMap<DatasetId, BTreeSet<NodeId>>,
    /// Erasure fragments per dataset.
    erasure_fragments: HashMap<DatasetId, BTreeSet<NodeId>>,
}

impl ReplicaTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        ReplicaTracker {
            placements: HashMap::new(),
            dataset_placements: HashMap::new(),
            node_placements: HashMap::new(),
            full_replicas: HashMap::new(),
            erasure_fragments: HashMap::new(),
        }
    }

    /// Register a replica placement.
    pub fn register_placement(&mut self, placement: ReplicaPlacement) {
        let key = (placement.dataset_id, placement.node_id);
        let is_new = !self.placements.contains_key(&key);

        // Remove old placement tracking if re-registering.
        if let Some(old) = self.placements.get(&key) {
            self.remove_tracking(old);
        }

        self.placements.insert(key, placement.clone());
        self.dataset_placements
            .entry(placement.dataset_id)
            .or_default()
            .push(key);
        self.node_placements
            .entry(placement.node_id)
            .or_default()
            .push(key);

        match placement.replica_type {
            ReplicaType::Full => {
                self.full_replicas
                    .entry(placement.dataset_id)
                    .or_default()
                    .insert(placement.node_id);
            }
            ReplicaType::ErasureFragment => {
                self.erasure_fragments
                    .entry(placement.dataset_id)
                    .or_default()
                    .insert(placement.node_id);
            }
        }

        if is_new {
            // tracked implicitly
        }
    }

    fn remove_tracking(&mut self, placement: &ReplicaPlacement) {
        let key = (placement.dataset_id, placement.node_id);
        self.dataset_placements
            .entry(placement.dataset_id)
            .or_default()
            .retain(|k| *k != key);
        self.node_placements
            .entry(placement.node_id)
            .or_default()
            .retain(|k| *k != key);
        match placement.replica_type {
            ReplicaType::Full => {
                self.full_replicas
                    .entry(placement.dataset_id)
                    .or_default()
                    .remove(&placement.node_id);
            }
            ReplicaType::ErasureFragment => {
                self.erasure_fragments
                    .entry(placement.dataset_id)
                    .or_default()
                    .remove(&placement.node_id);
            }
        }
    }

    /// Return placements for a dataset.
    pub fn placements_for_dataset(&self, dataset_id: DatasetId) -> Vec<ReplicaPlacement> {
        self.dataset_placements
            .get(&dataset_id)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.placements.get(k).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return full replica nodes for a dataset.
    pub fn full_replica_nodes(&self, dataset_id: DatasetId) -> Vec<NodeId> {
        self.full_replicas
            .get(&dataset_id)
            .map(|nodes| nodes.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Return erasure fragment nodes for a dataset.
    pub fn erasure_fragment_nodes(&self, dataset_id: DatasetId) -> Vec<NodeId> {
        self.erasure_fragments
            .get(&dataset_id)
            .map(|nodes| nodes.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Return placements for a node.
    pub fn placements_for_node(&self, node_id: NodeId) -> Vec<ReplicaPlacement> {
        self.node_placements
            .get(&node_id)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.placements.get(k).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return full replica count for a dataset.
    pub fn full_replica_count(&self, dataset_id: DatasetId) -> u32 {
        self.full_replicas
            .get(&dataset_id)
            .map(|n| n.len() as u32)
            .unwrap_or(0)
    }

    /// Return erasure fragment count for a dataset.
    pub fn erasure_fragment_count(&self, dataset_id: DatasetId) -> u32 {
        self.erasure_fragments
            .get(&dataset_id)
            .map(|n| n.len() as u32)
            .unwrap_or(0)
    }

    /// Return whether a node has a full replica of a dataset.
    pub fn has_full_replica(&self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        self.full_replicas
            .get(&dataset_id)
            .map(|n| n.contains(&node_id))
            .unwrap_or(false)
    }

    /// Return whether a node has an erasure fragment of a dataset.
    pub fn has_erasure_fragment(&self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        self.erasure_fragments
            .get(&dataset_id)
            .map(|n| n.contains(&node_id))
            .unwrap_or(false)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Mode transition validator — validates safe mode switches
///////////////////////////////////////////////////////////////////////////////

/// A mode transition record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeTransition {
    pub dataset_id: DatasetId,
    pub from_mode: RedundancyMode,
    pub to_mode: RedundancyMode,
    pub transition_id: TransitionId,
    pub status: TransitionStatus,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a mode transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransitionId(pub u64);

/// Status of a mode transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

/// Validates and tracks mode transitions for hybrid redundancy.
pub struct ModeTransitionValidator {
    /// Transitions by ID.
    transitions: HashMap<TransitionId, ModeTransition>,
    /// Per-dataset transition IDs.
    dataset_transitions: HashMap<DatasetId, Vec<TransitionId>>,
    /// Pending transitions waiting to complete.
    pending_transitions: Vec<TransitionId>,
    /// Next transition ID.
    next_id: u64,
    /// Total transitions.
    total_transitions: u64,
    /// Total successful transitions.
    successful_transitions: u64,
    /// Total failed transitions.
    failed_transitions: u64,
}

impl ModeTransitionValidator {
    /// Create a new validator.
    pub fn new() -> Self {
        ModeTransitionValidator {
            transitions: HashMap::new(),
            dataset_transitions: HashMap::new(),
            pending_transitions: Vec::new(),
            next_id: 0,
            total_transitions: 0,
            successful_transitions: 0,
            failed_transitions: 0,
        }
    }

    /// Validate whether a mode switch is safe.
    pub fn validate_transition(
        &self,
        dataset_id: DatasetId,
        from: &RedundancyMode,
        to: &RedundancyMode,
        current_full_replicas: u32,
        current_erasure_k: u32,
        current_erasure_n: u32,
    ) -> bool {
        // Same mode is always safe (no-op).
        if from == to {
            return true;
        }

        match (from, to) {
            // Full -> ErasureCoded: need enough data to reconstruct.
            (RedundancyMode::FullReplica, RedundancyMode::ErasureCoded { k, n }) => {
                current_full_replicas >= k && n >= k
            }
            // ErasureCoded -> Full: need enough surviving fragments.
            (RedundancyMode::ErasureCoded { k, n }, RedundancyMode::FullReplica) => {
                current_erasure_n >= current_erasure_k && current_full_replicas >= 1
            }
            // Full -> Hybrid: need at least 1 full + erasure params valid.
            (RedundancyMode::FullReplica, RedundancyMode::Hybrid) => {
                current_full_replicas >= 1
            }
            // ErasureCoded -> Hybrid: need erasure params valid.
            (RedundancyMode::ErasureCoded { k, n }, RedundancyMode::Hybrid) => {
                n >= k && k > 0
            }
            // Hybrid -> Full: need enough full replicas.
            (RedundancyMode::Hybrid, RedundancyMode::FullReplica) => {
                current_full_replicas >= 1
            }
            // Hybrid -> ErasureCoded: need erasure params valid.
            (RedundancyMode::Hybrid, RedundancyMode::ErasureCoded { k, n }) => {
                n >= k && k > 0
            }
        }
    }

    /// Record a mode transition.
    pub fn record_transition(
        &mut self,
        dataset_id: DatasetId,
        from: RedundancyMode,
        to: RedundancyMode,
    ) -> TransitionId {
        let id = TransitionId(self.next_id);
        self.next_id += 1;
        self.total_transitions += 1;
        let transition = ModeTransition {
            dataset_id,
            from_mode: from.clone(),
            to_mode: to.clone(),
            transition_id: id,
            status: TransitionStatus::Pending,
            started_at: TimestampMillis(0),
            completed_at: None,
        };
        self.transitions.insert(id, transition);
        self.dataset_transitions
            .entry(dataset_id)
            .or_default()
            .push(id);
        self.pending_transitions.push(id);
        id
    }

    /// Start a transition.
    pub fn start_transition(&mut self, transition_id: TransitionId) -> Result<(), Error> {
        let transition = self
            .transitions
            .get_mut(&transition_id)
            .ok_or_else(|| Error::Internal {
                message: "transition not found".to_string(),
            })?;
        if transition.status != TransitionStatus::Pending {
            return Err(Error::Internal {
                message: "transition not pending".to_string(),
            });
        }
        transition.status = TransitionStatus::InProgress;
        self.pending_transitions.retain(|&id| id != transition_id);
        Ok(())
    }

    /// Complete a transition.
    pub fn complete_transition(&mut self, transition_id: TransitionId) -> Result<(), Error> {
        let transition = self
            .transitions
            .get_mut(&transition_id)
            .ok_or_else(|| Error::Internal {
                message: "transition not found".to_string(),
            })?;
        transition.status = TransitionStatus::Completed;
        transition.completed_at = Some(TimestampMillis(0));
        self.successful_transitions += 1;
        Ok(())
    }

    /// Fail a transition.
    pub fn fail_transition(&mut self, transition_id: TransitionId) -> Result<(), Error> {
        let transition = self
            .transitions
            .get_mut(&transition_id)
            .ok_or_else(|| Error::Internal {
                message: "transition not found".to_string(),
            })?;
        transition.status = TransitionStatus::Failed;
        transition.completed_at = Some(TimestampMillis(0));
        self.failed_transitions += 1;
        Ok(())
    }

    /// Roll back a transition.
    pub fn rollback_transition(&mut self, transition_id: TransitionId) -> Result<(), Error> {
        let transition = self
            .transitions
            .get_mut(&transition_id)
            .ok_or_else(|| Error::Internal {
                message: "transition not found".to_string(),
            })?;
        transition.status = TransitionStatus::RolledBack;
        transition.completed_at = Some(TimestampMillis(0));
        self.failed_transitions += 1;
        Ok(())
    }

    /// Return transitions for a dataset.
    pub fn transitions_for_dataset(&self, dataset_id: DatasetId) -> Vec<ModeTransition> {
        self.dataset_transitions
            .get(&dataset_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.transitions.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return total transitions.
    pub fn total_transitions(&self) -> u64 {
        self.total_transitions
    }

    /// Return successful transition count.
    pub fn successful_transitions(&self) -> u64 {
        self.successful_transitions
    }

    /// Return failed transition count.
    pub fn failed_transitions(&self) -> u64 {
        self.failed_transitions
    }

    /// Return transition success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_transitions == 0 {
            return 1.0;
        }
        self.successful_transitions as f64 / self.total_transitions as f64
    }

    /// Return pending transition count.
    pub fn pending_count(&self) -> u64 {
        self.pending_transitions.len() as u64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Redundancy factor calculator — computes total redundancy
///////////////////////////////////////////////////////////////////////////////

/// A redundancy factor calculation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundancyFactorResult {
    pub dataset_id: DatasetId,
    pub full_replica_factor: f64,
    pub erasure_factor: f64,
    pub hybrid_factor: f64,
    pub total_factor: f64,
    pub storage_overhead: f64,
}

/// Calculates redundancy factors for datasets.
pub struct RedundancyFactorCalculator;

impl RedundancyFactorCalculator {
    /// Create a new calculator.
    pub fn new() -> Self {
        RedundancyFactorCalculator
    }

    /// Calculate the redundancy factor for a dataset config.
    pub fn calculate(
        &self,
        dataset_id: DatasetId,
        config: &RedundancyConfig,
    ) -> RedundancyFactorResult {
        let full_factor = config.full_replicas as f64;
        let erasure_factor = if config.erasure_n > 0 {
            config.erasure_n as f64 / config.erasure_k.max(1) as f64
        } else {
            1.0
        };
        let hybrid_factor = if config.is_hybrid() {
            (config.hybrid_full_count as f64 + config.hybrid_erasure_count as f64)
                / config.erasure_k.max(1) as f64
        } else {
            0.0
        };
        let total_factor = match config.mode {
            RedundancyMode::FullReplica => full_factor,
            RedundancyMode::ErasureCoded { .. } => erasure_factor,
            RedundancyMode::Hybrid => hybrid_factor,
        };
        let storage_overhead = total_factor - 1.0;

        RedundancyFactorResult {
            dataset_id,
            full_replica_factor: full_factor,
            erasure_factor,
            hybrid_factor,
            total_factor,
            storage_overhead,
        }
    }

    /// Return whether the redundancy factor meets a minimum threshold.
    pub fn meets_threshold(result: &RedundancyFactorResult, threshold: f64) -> bool {
        result.total_factor >= threshold
    }

    /// Return the recommended min replicas for fault tolerance f.
    pub fn recommended_min_replicas(fault_tolerance: u32) -> u32 {
        fault_tolerance + 1
    }

    /// Return the recommended erasure (k, n) for fault tolerance f.
    pub fn recommended_erasure_params(fault_tolerance: u32) -> (u32, u32) {
        let k = fault_tolerance.max(1) * 2;
        let n = k + fault_tolerance;
        (k, n)
    }
}

///////////////////////////////////////////////////////////////////////////////
// Data Integrity Verifier — verifies integrity across redundancy types
///////////////////////////////////////////////////////////////////////////////

/// A data integrity verification result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataIntegrityResult {
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub replica_type: ReplicaType,
    pub fragment_index: Option<u32>,
    pub checksum_valid: bool,
    pub size_match: bool,
    pub healthy: bool,
}

/// Verifies data integrity across full replicas and erasure fragments.
pub struct DataIntegrityVerifier {
    /// Verification results by (dataset, node).
    results: HashMap<(DatasetId, NodeId), DataIntegrityResult>,
    /// Total verifications.
    total_verifications: u64,
    /// Total failed verifications.
    failed_verifications: u64,
}

impl DataIntegrityVerifier {
    /// Create a new verifier.
    pub fn new() -> Self {
        DataIntegrityVerifier {
            results: HashMap::new(),
            total_verifications: 0,
            failed_verifications: 0,
        }
    }

    /// Record a verification result.
    pub fn record_verification(
        &mut self,
        dataset_id: DatasetId,
        node_id: NodeId,
        replica_type: ReplicaType,
        fragment_index: Option<u32>,
        expected_size: u64,
        actual_size: u64,
        checksum_valid: bool,
    ) -> DataIntegrityResult {
        self.total_verifications += 1;
        let size_match = expected_size == actual_size;
        let healthy = checksum_valid && size_match;
        if !healthy {
            self.failed_verifications += 1;
        }
        let result = DataIntegrityResult {
            dataset_id,
            node_id,
            replica_type,
            fragment_index,
            checksum_valid,
            size_match,
            healthy,
        };
        self.results.insert((dataset_id, node_id), result.clone());
        result
    }

    /// Return whether a replica is healthy.
    pub fn is_healthy(&self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        self.results
            .get(&(dataset_id, node_id))
            .map(|r| r.healthy)
            .unwrap_or(false)
    }

    /// Return verification results for a dataset.
    pub fn results_for_dataset(&self, dataset_id: DatasetId) -> Vec<DataIntegrityResult> {
        self.results
            .iter()
            .filter_map(|((ds, _), r)| if *ds == dataset_id { Some(r.clone()) } else { None })
            .collect()
    }

    /// Return total verifications.
    pub fn total_verifications(&self) -> u64 {
        self.total_verifications
    }

    /// Return failed verification count.
    pub fn failed_verifications(&self) -> u64 {
        self.failed_verifications
    }

    /// Return verification success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_verifications == 0 {
            return 1.0;
        }
        (self.total_verifications - self.failed_verifications) as f64 / self.total_verifications as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Hybrid Redundancy Manager — core coordinator
///////////////////////////////////////////////////////////////////////////////

/// Configuration for hybrid redundancy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridRedundancyConfig {
    pub default_full_replicas: u32,
    pub default_erasure_k: u32,
    pub default_erasure_n: u32,
    pub min_full_replicas: u32,
    pub min_erasure_k: u32,
    pub allow_mode_switch: bool,
}

impl Default for HybridRedundancyConfig {
    fn default() -> Self {
        HybridRedundancyConfig {
            default_full_replicas: 3,
            default_erasure_k: 4,
            default_erasure_n: 10,
            min_full_replicas: 1,
            min_erasure_k: 2,
            allow_mode_switch: true,
        }
    }
}

/// Core hybrid redundancy manager — coordinates mode selection, replica
/// tracking, transition validation, and integrity verification.
pub struct HybridRedundancyManager {
    /// Redundancy configuration.
    config: HybridRedundancyConfig,
    /// Config tracker.
    config_tracker: RedundancyConfigTracker,
    /// Replica tracker.
    replica_tracker: ReplicaTracker,
    /// Mode transition validator.
    transition_validator: ModeTransitionValidator,
    /// Factor calculator.
    factor_calculator: RedundancyFactorCalculator,
    /// Integrity verifier.
    integrity_verifier: DataIntegrityVerifier,
    /// Total redundancy checks.
    total_checks: u64,
    /// Total healthy datasets.
    healthy_datasets: u64,
}

impl HybridRedundancyManager {
    /// Create a new manager with default config.
    pub fn new() -> Self {
        HybridRedundancyManager {
            config: HybridRedundancyConfig::default(),
            config_tracker: RedundancyConfigTracker::new(),
            replica_tracker: ReplicaTracker::new(),
            transition_validator: ModeTransitionValidator::new(),
            factor_calculator: RedundancyFactorCalculator::new(),
            integrity_verifier: DataIntegrityVerifier::new(),
            total_checks: 0,
            healthy_datasets: 0,
        }
    }

    /// Create a manager with custom config.
    pub fn with_config(config: HybridRedundancyConfig) -> Self {
        HybridRedundancyManager {
            config,
            config_tracker: RedundancyConfigTracker::new(),
            replica_tracker: ReplicaTracker::new(),
            transition_validator: ModeTransitionValidator::new(),
            factor_calculator: RedundancyFactorCalculator::new(),
            integrity_verifier: DataIntegrityVerifier::new(),
            total_checks: 0,
            healthy_datasets: 0,
        }
    }

    /// Set the redundancy mode for a dataset.
    pub fn set_redundancy_mode(
        &mut self,
        dataset_id: DatasetId,
        mode: RedundancyMode,
    ) -> Result<(), Error> {
        let config = match mode {
            RedundancyMode::FullReplica => {
                RedundancyConfig::full_replica(dataset_id, self.config.default_full_replicas)
            }
            RedundancyMode::ErasureCoded { k, n } => {
                RedundancyConfig::erasure_coded(dataset_id, k, n)
            }
            RedundancyMode::Hybrid => RedundancyConfig::hybrid(
                dataset_id,
                self.config.min_full_replicas,
                self.config.default_erasure_k,
                self.config.default_erasure_n,
            ),
        };
        self.config_tracker.register_config(config);
        self.total_checks += 1;
        Ok(())
    }

    /// Register a replica placement.
    pub fn register_replica(&mut self, placement: ReplicaPlacement) {
        self.replica_tracker.register_placement(placement);
        self.total_checks += 1;
    }

    /// Return the redundancy mode for a dataset.
    pub fn redundancy_mode(&self, dataset_id: DatasetId) -> RedundancyMode {
        self.config_tracker
            .config_for(dataset_id)
            .map(|c| c.mode.clone())
            .unwrap_or(RedundancyMode::FullReplica)
    }

    /// Return the full replica count for a dataset.
    pub fn full_replica_count(&self, dataset_id: DatasetId) -> u32 {
        self.replica_tracker.full_replica_count(dataset_id)
    }

    /// Return the erasure-coded parameters for a dataset.
    pub fn erasure_coded_params(&self, dataset_id: DatasetId) -> (u32, u32) {
        self.config_tracker
            .config_for(dataset_id)
            .map(|c| (c.erasure_k, c.erasure_n))
            .unwrap_or((0, 0))
    }

    /// Check whether a mode switch is safe.
    pub fn mode_switch_safe(
        &self,
        dataset_id: DatasetId,
        from: RedundancyMode,
        to: RedundancyMode,
    ) -> bool {
        let config = match self.config_tracker.config_for(dataset_id) {
            Some(c) => c,
            None => return false,
        };
        self.transition_validator.validate_transition(
            dataset_id,
            &from,
            &to,
            config.full_replicas,
            config.erasure_k,
            config.erasure_n,
        )
    }

    /// Return the total redundancy factor for a dataset.
    pub fn total_redundancy_factor(&self, dataset_id: DatasetId) -> f64 {
        let config = match self.config_tracker.config_for(dataset_id) {
            Some(c) => c,
            None => return 1.0,
        };
        self.factor_calculator.calculate(dataset_id, config).total_factor
    }

    /// Return whether a dataset uses hybrid redundancy.
    pub fn is_hybrid(&self, dataset_id: DatasetId) -> bool {
        self.config_tracker
            .config_for(dataset_id)
            .map(|c| c.is_hybrid())
            .unwrap_or(false)
    }

    /// Record a mode transition.
    pub fn record_mode_transition(
        &mut self,
        dataset_id: DatasetId,
        from: RedundancyMode,
        to: RedundancyMode,
    ) -> TransitionId {
        self.transition_validator.record_transition(dataset_id, from, to)
    }

    /// Verify data integrity for a replica.
    pub fn verify_integrity(
        &mut self,
        dataset_id: DatasetId,
        node_id: NodeId,
        replica_type: ReplicaType,
        fragment_index: Option<u32>,
        expected_size: u64,
        actual_size: u64,
        checksum_valid: bool,
    ) -> DataIntegrityResult {
        self.healthy_datasets += 1;
        self.integrity_verifier.record_verification(
            dataset_id, node_id, replica_type, fragment_index, expected_size, actual_size,
            checksum_valid,
        )
    }

    /// Return whether a dataset is healthy.
    pub fn is_dataset_healthy(&self, dataset_id: DatasetId) -> bool {
        let results = self.integrity_verifier.results_for_dataset(dataset_id);
        if results.is_empty() {
            return true;
        }
        results.iter().all(|r| r.healthy)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return healthy dataset count.
    pub fn healthy_dataset_count(&self) -> u64 {
        self.healthy_datasets
    }

    /// Return integrity verifier.
    pub fn integrity_verifier(&self) -> &DataIntegrityVerifier {
        &self.integrity_verifier
    }

    /// Return transition validator.
    pub fn transition_validator(&self) -> &ModeTransitionValidator {
        &self.transition_validator
    }

    /// Return replica tracker.
    pub fn replica_tracker(&self) -> &ReplicaTracker {
        &self.replica_tracker
    }

    /// Return config tracker.
    pub fn config_tracker(&self) -> &RedundancyConfigTracker {
        &self.config_tracker
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Version bit position for the hybrid redundancy invariant in EdgeCaseBits.
pub const HYBRID_REDUNDANCY_BIT: u128 = 1 << 12;

/// Check whether the hybrid redundancy invariant is enabled in a bitset.
pub fn hybrid_redundancy_enabled(bits: &crate::versioning::EdgeCaseBits) -> bool {
    bits.has(HYBRID_REDUNDANCY_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod hybrid_redundancy_tests {
    use super::*;

    // ---- RedundancyMode tests ----

    #[test]
    fn redundancy_mode_variants() {
        let modes = [
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
            RedundancyMode::Hybrid,
        ];
        for m in modes {
            let _ = format!("{:?}", m);
        }
    }

    // ---- RedundancyConfig tests ----

    #[test]
    fn redundancy_config_full_replica() {
        let config = RedundancyConfig::full_replica(DatasetId::new([1u8; 32]), 3);
        assert_eq!(config.mode, RedundancyMode::FullReplica);
        assert_eq!(config.full_replicas, 3);
        assert!(!config.is_hybrid());
        assert_eq!(config.total_copies(), 3);
        assert_eq!(config.fault_tolerance(), 2);
    }

    #[test]
    fn redundancy_config_erasure_coded() {
        let config = RedundancyConfig::erasure_coded(DatasetId::new([1u8; 32]), 4, 10);
        assert_eq!(config.erasure_k, 4);
        assert_eq!(config.erasure_n, 10);
        assert_eq!(config.total_copies(), 10);
        assert_eq!(config.fault_tolerance(), 6);
    }

    #[test]
    fn redundancy_config_hybrid() {
        let config = RedundancyConfig::hybrid(DatasetId::new([1u8; 32]), 2, 4, 8);
        assert!(config.is_hybrid());
        assert_eq!(config.total_copies(), 10);
        assert_eq!(config.hybrid_full_count, 2);
        assert_eq!(config.hybrid_erasure_count, 8);
    }

    #[test]
    fn redundancy_config_fault_tolerance_full() {
        let config = RedundancyConfig::full_replica(DatasetId::new([1u8; 32]), 1);
        assert_eq!(config.fault_tolerance(), 0);
    }

    #[test]
    fn redundancy_config_fault_tolerance_erasure() {
        let config = RedundancyConfig::erasure_coded(DatasetId::new([1u8; 32]), 5, 5);
        assert_eq!(config.fault_tolerance(), 0);
    }

    // ---- RedundancyConfigTracker tests ----

    #[test]
    fn tracker_register_config() {
        let mut tracker = RedundancyConfigTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        tracker.register_config(RedundancyConfig::full_replica(dataset, 3));
        assert_eq!(tracker.total_configs(), 1);
        assert!(tracker.config_for(dataset).is_some());
    }

    #[test]
    fn tracker_datasets_for_mode() {
        let mut tracker = RedundancyConfigTracker::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        tracker.register_config(RedundancyConfig::full_replica(d1, 3));
        tracker.register_config(RedundancyConfig::erasure_coded(d2, 4, 8));
        let full_ds = tracker.datasets_for_mode(&RedundancyMode::FullReplica);
        assert_eq!(full_ds.len(), 1);
        assert_eq!(full_ds[0], d1);
        let ec_ds = tracker.datasets_for_mode(&RedundancyMode::ErasureCoded { k: 4, n: 8 });
        assert_eq!(ec_ds.len(), 1);
        assert_eq!(ec_ds[0], d2);
    }

    #[test]
    fn tracker_total_mode_changes() {
        let mut tracker = RedundancyConfigTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        tracker.register_config(RedundancyConfig::full_replica(dataset, 3));
        tracker.register_config(RedundancyConfig::erasure_coded(dataset, 4, 8));
        assert_eq!(tracker.total_mode_changes(), 1);
    }

    #[test]
    fn tracker_all_datasets() {
        let mut tracker = RedundancyConfigTracker::new();
        tracker.register_config(RedundancyConfig::full_replica(DatasetId::new([1u8; 32]), 3));
        tracker.register_config(RedundancyConfig::full_replica(DatasetId::new([2u8; 32]), 3));
        assert_eq!(tracker.all_datasets().len(), 2);
    }

    // ---- ReplicaType tests ----

    #[test]
    fn replica_type_variants() {
        let types = [ReplicaType::Full, ReplicaType::ErasureFragment];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    // ---- ReplicaPlacement tests ----

    #[test]
    fn replica_placement_creation() {
        let placement = ReplicaPlacement {
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        };
        assert_eq!(placement.size_bytes, 1024);
        assert!(placement.healthy);
    }

    // ---- ReplicaTracker tests ----

    #[test]
    fn tracker_register_placement() {
        let mut tracker = ReplicaTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: node,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });
        assert_eq!(tracker.full_replica_count(dataset), 1);
        assert_eq!(tracker.erasure_fragment_count(dataset), 0);
        assert!(tracker.has_full_replica(dataset, node));
        assert!(!tracker.has_erasure_fragment(dataset, node));
    }

    #[test]
    fn tracker_register_erasure_fragment() {
        let mut tracker = ReplicaTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: node,
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(0),
            size_bytes: 256,
            healthy: true,
        });
        assert_eq!(tracker.erasure_fragment_count(dataset), 1);
        assert!(tracker.has_erasure_fragment(dataset, node));
    }

    #[test]
    fn tracker_placements_for_dataset() {
        let mut tracker = ReplicaTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        let n1 = NodeId::new([2u8; 32]);
        let n2 = NodeId::new([3u8; 32]);
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n1,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n2,
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(1),
            size_bytes: 512,
            healthy: true,
        });
        let placements = tracker.placements_for_dataset(dataset);
        assert_eq!(placements.len(), 2);
    }

    #[test]
    fn tracker_placements_for_node() {
        let mut tracker = ReplicaTracker::new();
        let node = NodeId::new([1u8; 32]);
        tracker.register_placement(ReplicaPlacement {
            dataset_id: DatasetId::new([2u8; 32]),
            node_id: node,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });
        tracker.register_placement(ReplicaPlacement {
            dataset_id: DatasetId::new([3u8; 32]),
            node_id: node,
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(0),
            size_bytes: 256,
            healthy: true,
        });
        let placements = tracker.placements_for_node(node);
        assert_eq!(placements.len(), 2);
    }

    // ---- ModeTransition tests ----

    #[test]
    fn mode_transition_creation() {
        let transition = ModeTransition {
            dataset_id: DatasetId::new([1u8; 32]),
            from_mode: RedundancyMode::FullReplica,
            to_mode: RedundancyMode::ErasureCoded { k: 4, n: 10 },
            transition_id: TransitionId(1),
            status: TransitionStatus::Pending,
            started_at: TimestampMillis(0),
            completed_at: None,
        };
        assert_eq!(transition.transition_id, TransitionId(1));
        assert_eq!(transition.status, TransitionStatus::Pending);
    }

    // ---- TransitionStatus tests ----

    #[test]
    fn transition_status_variants() {
        let statuses = [
            TransitionStatus::Pending,
            TransitionStatus::InProgress,
            TransitionStatus::Completed,
            TransitionStatus::Failed,
            TransitionStatus::RolledBack,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    // ---- TransitionId tests ----

    #[test]
    fn transition_id_ordering() {
        let a = TransitionId(1);
        let b = TransitionId(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- ModeTransitionValidator tests ----

    #[test]
    fn validator_same_mode_safe() {
        let validator = ModeTransitionValidator::new();
        assert!(validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::FullReplica,
            &RedundancyMode::FullReplica,
            3,
            0,
            0
        ));
    }

    #[test]
    fn validator_full_to_erasure_safe() {
        let validator = ModeTransitionValidator::new();
        assert!(validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::FullReplica,
            &RedundancyMode::ErasureCoded { k: 4, n: 10 },
            5,
            0,
            0
        ));
    }

    #[test]
    fn validator_full_to_erasure_unsafe() {
        let validator = ModeTransitionValidator::new();
        assert!(!validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::FullReplica,
            &RedundancyMode::ErasureCoded { k: 10, n: 10 },
            3,
            0,
            0
        ));
    }

    #[test]
    fn validator_erasure_to_full_safe() {
        let validator = ModeTransitionValidator::new();
        assert!(validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::ErasureCoded { k: 4, n: 10 },
            &RedundancyMode::FullReplica,
            1,
            4,
            10
        ));
    }

    #[test]
    fn validator_hybrid_transitions() {
        let validator = ModeTransitionValidator::new();
        assert!(validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::FullReplica,
            &RedundancyMode::Hybrid,
            2,
            0,
            0
        ));
        assert!(validator.validate_transition(
            DatasetId::new([1u8; 32]),
            &RedundancyMode::ErasureCoded { k: 4, n: 10 },
            &RedundancyMode::Hybrid,
            0,
            4,
            10
        ));
    }

    #[test]
    fn validator_record_and_complete() {
        let mut validator = ModeTransitionValidator::new();
        let id = validator.record_transition(
            DatasetId::new([1u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
        );
        validator.start_transition(id).unwrap();
        validator.complete_transition(id).unwrap();
        assert_eq!(validator.successful_transitions(), 1);
        assert_eq!(validator.total_transitions(), 1);
    }

    #[test]
    fn validator_fail_and_rollback() {
        let mut validator = ModeTransitionValidator::new();
        let id = validator.record_transition(
            DatasetId::new([1u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
        );
        validator.fail_transition(id).unwrap();
        assert_eq!(validator.failed_transitions(), 1);

        let id2 = validator.record_transition(
            DatasetId::new([1u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
        );
        validator.rollback_transition(id2).unwrap();
        assert_eq!(validator.failed_transitions(), 2);
    }

    #[test]
    fn validator_success_rate() {
        let mut validator = ModeTransitionValidator::new();
        let id1 = validator.record_transition(
            DatasetId::new([1u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
        );
        let id2 = validator.record_transition(
            DatasetId::new([2u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 4, n: 10 },
        );
        validator.complete_transition(id1).unwrap();
        validator.fail_transition(id2).unwrap();
        assert!((validator.success_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn validator_transitions_for_dataset() {
        let mut validator = ModeTransitionValidator::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        validator.record_transition(d1, RedundancyMode::FullReplica, RedundancyMode::Hybrid);
        validator.record_transition(d2, RedundancyMode::FullReplica, RedundancyMode::Hybrid);
        validator.record_transition(d1, RedundancyMode::Hybrid, RedundancyMode::FullReplica);
        assert_eq!(validator.transitions_for_dataset(d1).len(), 2);
        assert_eq!(validator.transitions_for_dataset(d2).len(), 1);
    }

    // ---- RedundancyFactorResult tests ----

    #[test]
    fn redundancy_factor_result_creation() {
        let result = RedundancyFactorResult {
            dataset_id: DatasetId::new([1u8; 32]),
            full_replica_factor: 3.0,
            erasure_factor: 2.5,
            hybrid_factor: 3.5,
            total_factor: 3.5,
            storage_overhead: 2.5,
        };
        assert_eq!(result.total_factor, 3.5);
        assert_eq!(result.storage_overhead, 2.5);
    }

    // ---- RedundancyFactorCalculator tests ----

    #[test]
    fn calculator_full_replica_factor() {
        let calc = RedundancyFactorCalculator::new();
        let config = RedundancyConfig::full_replica(DatasetId::new([1u8; 32]), 3);
        let result = calc.calculate(DatasetId::new([1u8; 32]), &config);
        assert_eq!(result.full_replica_factor, 3.0);
        assert_eq!(result.total_factor, 3.0);
    }

    #[test]
    fn calculator_erasure_factor() {
        let calc = RedundancyFactorCalculator::new();
        let config = RedundancyConfig::erasure_coded(DatasetId::new([1u8; 32]), 4, 10);
        let result = calc.calculate(DatasetId::new([1u8; 32]), &config);
        assert!((result.erasure_factor - 2.5).abs() < 0.001);
        assert_eq!(result.total_factor, 2.5);
    }

    #[test]
    fn calculator_hybrid_factor() {
        let calc = RedundancyFactorCalculator::new();
        let config = RedundancyConfig::hybrid(DatasetId::new([1u8; 32]), 2, 4, 8);
        let result = calc.calculate(DatasetId::new([1u8; 32]), &config);
        assert_eq!(result.hybrid_factor, 2.5); // (2+8)/4
        assert_eq!(result.total_factor, 2.5);
    }

    #[test]
    fn calculator_meets_threshold() {
        let result = RedundancyFactorResult {
            dataset_id: DatasetId::new([1u8; 32]),
            full_replica_factor: 3.0,
            erasure_factor: 1.0,
            hybrid_factor: 0.0,
            total_factor: 3.0,
            storage_overhead: 2.0,
        };
        assert!(RedundancyFactorCalculator::meets_threshold(&result, 2.0));
        assert!(!RedundancyFactorCalculator::meets_threshold(&result, 4.0));
    }

    #[test]
    fn calculator_recommended_params() {
        let replicas = RedundancyFactorCalculator::recommended_min_replicas(3);
        assert_eq!(replicas, 4);
        let (k, n) = RedundancyFactorCalculator::recommended_erasure_params(3);
        assert_eq!(k, 6);
        assert_eq!(n, 9);
    }

    // ---- DataIntegrityResult tests ----

    #[test]
    fn integrity_result_creation() {
        let result = DataIntegrityResult {
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(3),
            checksum_valid: true,
            size_match: true,
            healthy: true,
        };
        assert!(result.healthy);
        assert_eq!(result.fragment_index, Some(3));
    }

    // ---- DataIntegrityVerifier tests ----

    #[test]
    fn verifier_record_and_check() {
        let mut verifier = DataIntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let result = verifier.record_verification(
            dataset, node, ReplicaType::Full, None, 1024, 1024, true,
        );
        assert!(result.healthy);
        assert_eq!(verifier.total_verifications(), 1);
        assert_eq!(verifier.failed_verifications(), 0);
        assert!(verifier.is_healthy(dataset, node));
    }

    #[test]
    fn verifier_record_failure() {
        let mut verifier = DataIntegrityVerifier::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        verifier.record_verification(
            dataset, node, ReplicaType::Full, None, 1024, 512, false,
        );
        assert!(!verifier.is_healthy(dataset, node));
        assert_eq!(verifier.failed_verifications(), 1);
        assert!((verifier.success_rate() - 0.0).abs() < 0.001);
    }

    #[test]
    fn verifier_results_for_dataset() {
        let mut verifier = DataIntegrityVerifier::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        let n1 = NodeId::new([3u8; 32]);
        verifier.record_verification(d1, n1, ReplicaType::Full, None, 100, 100, true);
        verifier.record_verification(d2, n1, ReplicaType::Full, None, 100, 100, true);
        assert_eq!(verifier.results_for_dataset(d1).len(), 1);
        assert_eq!(verifier.results_for_dataset(d2).len(), 1);
    }

    #[test]
    fn verifier_empty() {
        let verifier = DataIntegrityVerifier::new();
        assert_eq!(verifier.total_verifications(), 0);
        assert!((verifier.success_rate() - 1.0).abs() < 0.001);
    }

    // ---- HybridRedundancyConfig tests ----

    #[test]
    fn hybrid_config_default() {
        let config = HybridRedundancyConfig::default();
        assert_eq!(config.default_full_replicas, 3);
        assert_eq!(config.default_erasure_k, 4);
        assert_eq!(config.default_erasure_n, 10);
        assert_eq!(config.min_full_replicas, 1);
        assert_eq!(config.min_erasure_k, 2);
        assert!(config.allow_mode_switch);
    }

    // ---- HybridRedundancyManager tests ----

    #[test]
    fn manager_set_and_get_mode() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        manager.set_redundancy_mode(dataset, RedundancyMode::ErasureCoded { k: 4, n: 10 })
            .unwrap();
        assert_eq!(
            manager.redundancy_mode(dataset),
            RedundancyMode::ErasureCoded { k: 4, n: 10 }
        );
    }

    #[test]
    fn manager_register_and_count_replicas() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let n1 = NodeId::new([2u8; 32]);
        let n2 = NodeId::new([3u8; 32]);
        manager.register_replica(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n1,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });
        manager.register_replica(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n2,
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(0),
            size_bytes: 256,
            healthy: true,
        });
        assert_eq!(manager.full_replica_count(dataset), 1);
        let (k, n) = manager.erasure_coded_params(dataset);
        assert_eq!((k, n), (0, 0)); // not set via config
    }

    #[test]
    fn manager_mode_switch_safe() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        manager.set_redundancy_mode(dataset, RedundancyMode::FullReplica).unwrap();
        assert!(manager.mode_switch_safe(
            dataset,
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 2, n: 5 }
        ));
    }

    #[test]
    fn manager_total_redundancy_factor() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        manager.set_redundancy_mode(dataset, RedundancyMode::FullReplica).unwrap();
        assert!((manager.total_redundancy_factor(dataset) - 3.0).abs() < 0.001);
    }

    #[test]
    fn manager_is_hybrid() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        manager.set_redundancy_mode(dataset, RedundancyMode::Hybrid).unwrap();
        assert!(manager.is_hybrid(dataset));
        assert!(!manager.is_hybrid(DatasetId::new([2u8; 32])));
    }

    #[test]
    fn manager_record_transition() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let id = manager.record_mode_transition(
            dataset,
            RedundancyMode::FullReplica,
            RedundancyMode::Hybrid,
        );
        assert_eq!(id, TransitionId(0));
    }

    #[test]
    fn manager_verify_integrity() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let result = manager.verify_integrity(
            dataset, node, ReplicaType::Full, None, 1024, 1024, true,
        );
        assert!(result.healthy);
        assert!(manager.is_dataset_healthy(dataset));
    }

    #[test]
    fn manager_total_checks() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        manager.set_redundancy_mode(dataset, RedundancyMode::FullReplica).unwrap();
        manager.register_replica(ReplicaPlacement {
            dataset_id: dataset,
            node_id: NodeId::new([2u8; 32]),
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 100,
            healthy: true,
        });
        assert_eq!(manager.total_checks(), 2);
    }

    #[test]
    fn manager_healthy_dataset_count() {
        let mut manager = HybridRedundancyManager::new();
        manager.verify_integrity(
            DatasetId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            ReplicaType::Full,
            None,
            100,
            100,
            true,
        );
        assert_eq!(manager.healthy_dataset_count(), 1);
    }

    // ---- HYBRID_REDUNDANCY_BIT tests ----

    #[test]
    fn hybrid_redundancy_bit_position() {
        assert_eq!(HYBRID_REDUNDANCY_BIT, 1 << 12);
    }

    #[test]
    fn hybrid_redundancy_enabled_checks_bit() {
        use crate::versioning::EdgeCaseBits;
        let mut bits = EdgeCaseBits(0);
        assert!(!hybrid_redundancy_enabled(&bits));
        bits.enable(HYBRID_REDUNDANCY_BIT);
        assert!(hybrid_redundancy_enabled(&bits));
    }

    // ---- Sealed trait unit tests ----

    #[test]
    fn sealed_unit_impl() {
        let unit = ();
        assert_eq!(
            unit.redundancy_mode(DatasetId::new([1u8; 32])),
            RedundancyMode::FullReplica
        );
        assert_eq!(unit.full_replica_count(DatasetId::new([1u8; 32])), 1);
        assert_eq!(unit.erasure_coded_params(DatasetId::new([1u8; 32])), (0, 0));
        assert!(!unit.mode_switch_safe(
            DatasetId::new([1u8; 32]),
            RedundancyMode::FullReplica,
            RedundancyMode::Hybrid
        ));
        assert!((unit.total_redundancy_factor(DatasetId::new([1u8; 32])) - 1.0).abs() < 0.001);
        assert!(!unit.is_hybrid(DatasetId::new([1u8; 32])));
    }

    // ---- Integration test ==========

    #[test]
    fn full_hybrid_redundancy_workflow() {
        let mut manager = HybridRedundancyManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let n1 = NodeId::new([2u8; 32]);
        let n2 = NodeId::new([3u8; 32]);
        let n3 = NodeId::new([4u8; 32]);

        // Set initial mode.
        manager.set_redundancy_mode(dataset, RedundancyMode::FullReplica).unwrap();
        assert_eq!(manager.redundancy_mode(dataset), RedundancyMode::FullReplica);

        // Register replicas.
        manager.register_replica(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n1,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });
        manager.register_replica(ReplicaPlacement {
            dataset_id: dataset,
            node_id: n2,
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1024,
            healthy: true,
        });

        // Validate mode switch to erasure coding.
        assert!(manager.mode_switch_safe(
            dataset,
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 2, n: 3 }
        ));

        // Record and complete transition.
        let transition_id = manager.record_mode_transition(
            dataset,
            RedundancyMode::FullReplica,
            RedundancyMode::ErasureCoded { k: 2, n: 3 },
        );
        manager.transition_validator().start_transition(transition_id).unwrap();
        manager.transition_validator().complete_transition(transition_id).unwrap();

        // Verify integrity of fragments.
        manager.verify_integrity(dataset, n1, ReplicaType::ErasureFragment, Some(0), 512, 512, true);
        manager.verify_integrity(dataset, n2, ReplicaType::ErasureFragment, Some(1), 512, 512, true);
        manager.verify_integrity(dataset, n3, ReplicaType::ErasureFragment, Some(2), 512, 512, true);

        // Check redundancy factor.
        assert!((manager.total_redundancy_factor(dataset) - 3.0).abs() < 0.001);
        assert!(manager.is_dataset_healthy(dataset));
    }

    // ---- Config tracker integration ==========

    #[test]
    fn config_tracker_integration() {
        let mut tracker = RedundancyConfigTracker::new();
        let d1 = DatasetId::new([1u8; 32]);
        let d2 = DatasetId::new([2u8; 32]);
        tracker.register_config(RedundancyConfig::full_replica(d1, 3));
        tracker.register_config(RedundancyConfig::hybrid(d2, 1, 4, 6));
        assert_eq!(tracker.total_configs(), 2);
        assert_eq!(tracker.datasets_for_mode(&RedundancyMode::Hybrid).len(), 1);
        assert_eq!(tracker.config_for(d1).unwrap().total_copies(), 3);
        assert_eq!(tracker.config_for(d2).unwrap().total_copies(), 7);
    }

    // ---- Replica tracker integration ==========

    #[test]
    fn replica_tracker_mixed_types() {
        let mut tracker = ReplicaTracker::new();
        let dataset = DatasetId::new([1u8; 32]);
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: NodeId::new([1u8; 32]),
            replica_type: ReplicaType::Full,
            fragment_index: None,
            size_bytes: 1000,
            healthy: true,
        });
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: NodeId::new([2u8; 32]),
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(0),
            size_bytes: 250,
            healthy: true,
        });
        tracker.register_placement(ReplicaPlacement {
            dataset_id: dataset,
            node_id: NodeId::new([3u8; 32]),
            replica_type: ReplicaType::ErasureFragment,
            fragment_index: Some(1),
            size_bytes: 250,
            healthy: true,
        });
        assert_eq!(tracker.full_replica_count(dataset), 1);
        assert_eq!(tracker.erasure_fragment_count(dataset), 2);
        assert!(tracker.has_full_replica(dataset, NodeId::new([1u8; 32])));
        assert!(tracker.has_erasure_fragment(dataset, NodeId::new([2u8; 32])));
    }

    // ---- Factor calculator integration ==========

    #[test]
    fn calculator_all_modes() {
        let calc = RedundancyFactorCalculator::new();
        let full = calc.calculate(
            DatasetId::new([1u8; 32]),
            &RedundancyConfig::full_replica(DatasetId::new([1u8; 32]), 5),
        );
        assert_eq!(full.total_factor, 5.0);

        let ec = calc.calculate(
            DatasetId::new([2u8; 32]),
            &RedundancyConfig::erasure_coded(DatasetId::new([2u8; 32]), 3, 7),
        );
        assert!((ec.total_factor - (7.0 / 3.0)).abs() < 0.01);

        let hyb = calc.calculate(
            DatasetId::new([3u8; 32]),
            &RedundancyConfig::hybrid(DatasetId::new([3u8; 32]), 2, 3, 6),
        );
        assert_eq!(hyb.total_factor, (2.0 + 6.0) / 3.0);
    }

    // ---- Integrity verifier integration ==========

    #[test]
    fn integrity_verifier_mixed() {
        let mut verifier = DataIntegrityVerifier::new();
        let d = DatasetId::new([1u8; 32]);
        verifier.record_verification(d, NodeId::new([1u8; 32]), ReplicaType::Full, None, 100, 100, true);
        verifier.record_verification(d, NodeId::new([2u8; 32]), ReplicaType::ErasureFragment, Some(0), 50, 50, true);
        verifier.record_verification(d, NodeId::new([3u8; 32]), ReplicaType::ErasureFragment, Some(1), 50, 40, false);
        assert_eq!(verifier.total_verifications(), 3);
        assert_eq!(verifier.failed_verifications(), 1);
        assert!((verifier.success_rate() - (2.0 / 3.0)).abs() < 0.01);
    }

    // ---- Transition validator integration ==========

    #[test]
    fn transition_validator_edge_cases() {
        let validator = ModeTransitionValidator::new();
        let d = DatasetId::new([1u8; 32]);
        // Hybrid to Full with enough replicas.
        assert!(validator.validate_transition(
            d,
            &RedundancyMode::Hybrid,
            &RedundancyMode::FullReplica,
            1,
            4,
            8
        ));
        // Hybrid to ErasureCoded with valid params.
        assert!(validator.validate_transition(
            d,
            &RedundancyMode::Hybrid,
            &RedundancyMode::ErasureCoded { k: 3, n: 6 },
            1,
            3,
            6
        ));
        // ErasureCoded to Full with insufficient data.
        assert!(!validator.validate_transition(
            d,
            &RedundancyMode::ErasureCoded { k: 5, n: 5 },
            &RedundancyMode::FullReplica,
            0,
            5,
            5
        ));
    }
}

// ========== Replica Placement Optimizer — optimizes replica placement
///////////////////////////////////////////////////////////////////////////////

/// A replica placement recommendation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaPlacementRecommendation {
    pub dataset_id: DatasetId,
    pub recommended_nodes: Vec<NodeId>,
    pub placement_score: f64,
    pub reasoning: PlacementReasoning,
}

/// Reasoning for a placement recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementReasoning {
    LeastLoaded,
    MostAvailable,
    FailureDomainDiverse,
    RegionAffinity,
    LoadBalanced,
    CapacityConstrained,
}

/// Optimizes replica placement for hybrid redundancy.
pub struct ReplicaPlacementOptimizer {
    /// Node load scores (0.0 to 1.0).
    node_loads: HashMap<NodeId, f64>,
    /// Node available capacity in bytes.
    node_capacity: HashMap<NodeId, u64>,
    /// Node failure domain assignments.
    node_failure_domains: HashMap<NodeId, FailureDomainRef>,
    /// Total optimization runs.
    total_optimizations: u64,
    /// Total placements recommended.
    total_recommendations: u64,
}

/// A failure domain reference for placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FailureDomainRef(pub u32);

impl ReplicaPlacementOptimizer {
    /// Create a new optimizer.
    pub fn new() -> Self {
        ReplicaPlacementOptimizer {
            node_loads: HashMap::new(),
            node_capacity: HashMap::new(),
            node_failure_domains: HashMap::new(),
            total_optimizations: 0,
            total_recommendations: 0,
        }
    }

    /// Register node load score.
    pub fn register_node_load(&mut self, node: NodeId, load: f64) {
        self.node_loads.insert(node, load.clamp(0.0, 1.0));
    }

    /// Register node available capacity.
    pub fn register_node_capacity(&mut self, node: NodeId, capacity_bytes: u64) {
        self.node_capacity.insert(node, capacity_bytes);
    }

    /// Register node failure domain.
    pub fn register_node_failure_domain(&mut self, node: NodeId, domain: FailureDomainRef) {
        self.node_failure_domains.insert(node, domain);
    }

    /// Optimize placement for a dataset requiring n replicas.
    pub fn optimize_placement(
        &mut self,
        dataset_id: DatasetId,
        required_replicas: u32,
        candidate_nodes: Vec<NodeId>,
    ) -> ReplicaPlacementRecommendation {
        self.total_optimizations += 1;

        // Score each candidate node.
        let mut scored: Vec<(NodeId, f64)> = candidate_nodes
            .iter()
            .map(|&node| {
                let load = self.node_loads.get(&node).copied().unwrap_or(0.5);
                let capacity = self.node_capacity.get(&node).copied().unwrap_or(0);
                // Lower load and higher capacity = better score.
                let score = (1.0 - load) * 0.6 + (capacity as f64 / 1_000_000_000.0).min(1.0) * 0.4;
                (node, score)
            })
            .collect();

        // Sort by score descending.
        scored.sort_by(|(_, sa), (_, sb)| sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal));

        // Select top nodes, preferring failure domain diversity.
        let mut selected: Vec<NodeId> = Vec::new();
        let mut used_domains: HashSet<FailureDomainRef> = HashSet::new();
        for (node, _) in &scored {
            if selected.len() >= required_replicas as usize {
                break;
            }
            let domain = self.node_failure_domains.get(&node);
            if let Some(domain) = domain {
                if used_domains.contains(domain) && selected.len() < required_replicas as usize / 2 {
                    continue; // Skip if we already have a node in this domain and need diversity
                }
                used_domains.insert(*domain);
            }
            selected.push(*node);
        }

        // If we didn't select enough, fill from remaining scored nodes.
        for (node, _) in &scored {
            if selected.len() >= required_replicas as usize {
                break;
            }
            if !selected.contains(node) {
                selected.push(*node);
            }
        }

        let avg_score: f64 = selected
            .iter()
            .filter_map(|n| scored.iter().find(|(sn, _)| sn == n).map(|(_, s)| *s))
            .sum::<f64>()
            / selected.len().max(1) as f64;

        let reasoning = if selected.len() >= required_replicas as usize {
            PlacementReasoning::LoadBalanced
        } else {
            PlacementReasoning::CapacityConstrained
        };

        self.total_recommendations += selected.len() as u64;

        ReplicaPlacementRecommendation {
            dataset_id,
            recommended_nodes: selected,
            placement_score: avg_score,
            reasoning,
        }
    }

    /// Return total optimizations.
    pub fn total_optimizations(&self) -> u64 {
        self.total_optimizations
    }

    /// Return total recommendations.
    pub fn total_recommendations(&self) -> u64 {
        self.total_recommendations
    }
}

///////////////////////////////////////////////////////////////////////////////
// Correlated Failure Protector — protects against correlated failures
///////////////////////////////////////////////////////////////////////////////

/// A correlated failure protection rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationProtectionRule {
    pub rule_id: ProtectionRuleId,
    pub failure_domain: FailureDomainRef,
    pub max_replicas_per_domain: u32,
    pub enabled: bool,
}

/// Unique identifier for a protection rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtectionRuleId(pub u64);

/// Protects against correlated failures in hybrid redundancy.
pub struct CorrelatedFailureProtector {
    /// Protection rules by ID.
    rules: HashMap<ProtectionRuleId, CorrelationProtectionRule>,
    /// Per-domain rules.
    domain_rules: HashMap<FailureDomainRef, Vec<ProtectionRuleId>>,
    /// Total rules.
    total_rules: u64,
    /// Total protections applied.
    total_protections: u64,
}

impl CorrelatedFailureProtector {
    /// Create a new protector.
    pub fn new() -> Self {
        CorrelatedFailureProtector {
            rules: HashMap::new(),
            domain_rules: HashMap::new(),
            total_rules: 0,
            total_protections: 0,
        }
    }

    /// Add a protection rule.
    pub fn add_rule(&mut self, domain: FailureDomainRef, max_replicas: u32) -> ProtectionRuleId {
        let id = ProtectionRuleId(self.total_rules);
        self.total_rules += 1;
        let rule = CorrelationProtectionRule {
            rule_id: id,
            failure_domain: domain,
            max_replicas_per_domain: max_replicas,
            enabled: true,
        };
        self.rules.insert(id, rule);
        self.domain_rules.entry(domain).or_default().push(id);
        id
    }

    /// Check whether a placement violates any protection rule.
    pub fn check_placement(
        &self,
        dataset_id: DatasetId,
        placements: &[(NodeId, FailureDomainRef)],
    ) -> bool {
        let mut domain_counts: HashMap<FailureDomainRef, u32> = HashMap::new();
        for (_, domain) in placements {
            *domain_counts.entry(*domain).or_insert(0) += 1;
        }

        for (domain, count) in &domain_counts {
            if let Some(rule_ids) = self.domain_rules.get(domain) {
                for rule_id in rule_ids {
                    if let Some(rule) = self.rules.get(rule_id) {
                        if rule.enabled && *count > rule.max_replicas_per_domain {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Return the max replicas allowed for a domain.
    pub fn max_replicas_for_domain(&self, domain: FailureDomainRef) -> Option<u32> {
        self.domain_rules
            .get(&domain)
            .and_then(|ids| ids.first())
            .and_then(|id| self.rules.get(id))
            .map(|r| r.max_replicas_per_domain)
    }

    /// Return total rules.
    pub fn total_rules(&self) -> u64 {
        self.total_rules
    }

    /// Return total protections applied.
    pub fn total_protections(&self) -> u64 {
        self.total_protections
    }
}

///////////////////////////////////////////////////////////////////////////////
// Region-Aware Placer — places replicas with region awareness
///////////////////////////////////////////////////////////////////////////////

/// A region-aware placement decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionAwarePlacement {
    pub dataset_id: DatasetId,
    pub primary_region: RegionId,
    pub replica_regions: Vec<RegionId>,
    pub cross_region_replicas: u32,
    pub local_replicas: u32,
}

/// Places replicas with region awareness.
pub struct RegionAwarePlacer {
    /// Node-to-region mapping.
    node_regions: HashMap<NodeId, RegionId>,
    /// Region capacity in bytes.
    region_capacity: HashMap<RegionId, u64>,
    /// Region used capacity in bytes.
    region_used: HashMap<RegionId, u64>,
    /// Total placement decisions.
    total_decisions: u64,
    /// Total cross-region placements.
    total_cross_region: u64,
}

impl RegionAwarePlacer {
    /// Create a new placer.
    pub fn new() -> Self {
        RegionAwarePlacer {
            node_regions: HashMap::new(),
            region_capacity: HashMap::new(),
            region_used: HashMap::new(),
            total_decisions: 0,
            total_cross_region: 0,
        }
    }

    /// Register a node's region.
    pub fn register_node_region(&mut self, node: NodeId, region: RegionId) {
        self.node_regions.insert(node, region);
    }

    /// Register region capacity.
    pub fn register_region_capacity(&mut self, region: RegionId, capacity_bytes: u64) {
        self.region_capacity.insert(region, capacity_bytes);
        self.region_used.entry(region).or_insert(0);
    }

    /// Place replicas across regions.
    pub fn place_replicas(
        &mut self,
        dataset_id: DatasetId,
        primary_region: RegionId,
        total_replicas: u32,
        candidate_nodes: Vec<NodeId>,
    ) -> RegionAwarePlacement {
        self.total_decisions += 1;

        let mut replica_regions: Vec<RegionId> = Vec::new();
        let mut local_replicas = 0u32;
        let mut cross_region_replicas = 0u32;

        // First, place replicas in the primary region.
        let primary_nodes: Vec<NodeId> = candidate_nodes
            .iter()
            .filter(|&&n| self.node_regions.get(&n) == Some(&primary_region))
            .copied()
            .collect();

        let primary_count = primary_nodes.len().min(total_replicas as usize) as u32;
        local_replicas = primary_count;

        for _ in 0..primary_count {
            replica_regions.push(primary_region);
        }

        // Then place remaining replicas in other regions.
        let remaining = total_replicas.saturating_sub(primary_count);
        let other_regions: Vec<RegionId> = candidate_nodes
            .iter()
            .filter(|&&n| self.node_regions.get(&n) != Some(&primary_region))
            .filter_map(|&n| self.node_regions.get(&n).copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(remaining as usize)
            .collect();

        for region in &other_regions {
            replica_regions.push(*region);
            cross_region_replicas += 1;
            self.total_cross_region += 1;
        }

        // Update region used capacity (estimate).
        let bytes_per_replica = 1_048_576; // 1 MiB estimate
        for region in &replica_regions {
            *self.region_used.entry(*region).or_insert(0) += bytes_per_replica;
        }

        RegionAwarePlacement {
            dataset_id,
            primary_region,
            replica_regions,
            cross_region_replicas,
            local_replicas,
        }
    }

    /// Return region capacity.
    pub fn region_capacity(&self, region: RegionId) -> Option<u64> {
        self.region_capacity.get(&region).copied()
    }

    /// Return region used capacity.
    pub fn region_used_capacity(&self, region: RegionId) -> u64 {
        self.region_used.get(&region).copied().unwrap_or(0)
    }

    /// Return total decisions.
    pub fn total_decisions(&self) -> u64 {
        self.total_decisions
    }

    /// Return total cross-region placements.
    pub fn total_cross_region(&self) -> u64 {
        self.total_cross_region
    }
}

///////////////////////////////////////////////////////////////////////////////
// Bandwidth Throttler — throttles replication bandwidth
///////////////////////////////////////////////////////////////////////////////

/// A bandwidth throttle record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandwidthThrottle {
    pub dataset_id: DatasetId,
    pub max_bytes_per_second: u64,
    pub current_bytes_per_second: u64,
    pub throttled: bool,
}

/// Throttles replication bandwidth for hybrid redundancy.
pub struct BandwidthThrottler {
    /// Throttles per dataset.
    throttles: HashMap<DatasetId, BandwidthThrottle>,
    /// Total throttles.
    total_throttles: u64,
    /// Total throttled operations.
    total_throttled: u64,
}

impl BandwidthThrottler {
    /// Create a new throttler.
    pub fn new() -> Self {
        BandwidthThrottler {
            throttles: HashMap::new(),
            total_throttles: 0,
            total_throttled: 0,
        }
    }

    /// Set a bandwidth throttle for a dataset.
    pub fn set_throttle(&mut self, dataset_id: DatasetId, max_bytes_per_second: u64) {
        self.total_throttles += 1;
        self.throttles.insert(
            dataset_id,
            BandwidthThrottle {
                dataset_id,
                max_bytes_per_second,
                current_bytes_per_second: 0,
                throttled: false,
            },
        );
    }

    /// Check whether a bandwidth usage is throttled.
    pub fn check_throttle(&mut self, dataset_id: DatasetId, bytes_per_second: u64) -> bool {
        match self.throttles.get_mut(&dataset_id) {
            Some(throttle) => {
                throttle.current_bytes_per_second = bytes_per_second;
                if bytes_per_second > throttle.max_bytes_per_second {
                    throttle.throttled = true;
                    self.total_throttled += 1;
                    true
                } else {
                    throttle.throttled = false;
                    false
                }
            }
            None => false, // No throttle set, allow
        }
    }

    /// Return whether a dataset is throttled.
    pub fn is_throttled(&self, dataset_id: DatasetId) -> bool {
        self.throttles
            .get(&dataset_id)
            .map(|t| t.throttled)
            .unwrap_or(false)
    }

    /// Return total throttles.
    pub fn total_throttles(&self) -> u64 {
        self.total_throttles
    }

    /// Return total throttled operations.
    pub fn total_throttled(&self) -> u64 {
        self.total_throttled
    }
}

///////////////////////////////////////////////////////////////////////////////
// Recovery Priority Queue — prioritizes recovery operations
///////////////////////////////////////////////////////////////////////////////

/// A recovery priority entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPriorityEntry {
    pub dataset_id: DatasetId,
    pub priority: RecoveryPriority,
    pub lost_fragments: u32,
    pub total_fragments: u32,
    pub urgency_score: f64,
}

/// Recovery priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoveryPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Prioritizes recovery operations for hybrid redundancy.
pub struct RecoveryPriorityQueue {
    /// Entries by dataset.
    entries: HashMap<DatasetId, RecoveryPriorityEntry>,
    /// Priority-ordered queue.
    queue: Vec<DatasetId>,
    /// Total entries.
    total_entries: u64,
    /// Total processed recoveries.
    total_processed: u64,
}

impl RecoveryPriorityQueue {
    /// Create a new queue.
    pub fn new() -> Self {
        RecoveryPriorityQueue {
            entries: HashMap::new(),
            queue: Vec::new(),
            total_entries: 0,
            total_processed: 0,
        }
    }

    /// Add or update a recovery entry.
    pub fn add_entry(
        &mut self,
        dataset_id: DatasetId,
        priority: RecoveryPriority,
        lost_fragments: u32,
        total_fragments: u32,
    ) {
        let urgency = if total_fragments > 0 {
            lost_fragments as f64 / total_fragments as f64
        } else {
            0.0
        };
        let entry = RecoveryPriorityEntry {
            dataset_id,
            priority,
            lost_fragments,
            total_fragments,
            urgency_score: urgency,
        };
        self.entries.insert(dataset_id, entry);
        if !self.queue.contains(&dataset_id) {
            self.queue.push(dataset_id);
            self.total_entries += 1;
        }
        self.reorder_queue();
    }

    fn reorder_queue(&mut self) {
        self.queue.sort_by(|a, b| {
            let ea = self.entries.get(a).map(|e| (e.priority, e.urgency_score));
            let eb = self.entries.get(b).map(|e| (e.priority, e.urgency_score));
            eb.cmp(&ea)
        });
    }

    /// Pop the highest priority entry.
    pub fn pop(&mut self) -> Option<RecoveryPriorityEntry> {
        if let Some(dataset_id) = self.queue.first().copied() {
            self.queue.remove(0);
            self.entries.remove(&dataset_id).map(|e| {
                self.total_processed += 1;
                e
            })
        } else {
            None
        }
    }

    /// Return the next entry without removing.
    pub fn peek(&self) -> Option<&RecoveryPriorityEntry> {
        self.queue.first().and_then(|id| self.entries.get(id))
    }

    /// Return total entries.
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Return total processed.
    pub fn total_processed(&self) -> u64 {
        self.total_processed
    }
}

///////////////////////////////////////////////////////////////////////////////
// Storage Quota Enforcer — enforces storage quotas per dataset
///////////////////////////////////////////////////////////////////////////////

/// A storage quota record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageQuota {
    pub dataset_id: DatasetId,
    pub max_bytes: u64,
    pub used_bytes: u64,
    pub replica_bytes: u64,
    pub erasure_bytes: u64,
}

/// Enforces storage quotas for hybrid redundancy datasets.
pub struct StorageQuotaEnforcer {
    /// Quotas per dataset.
    quotas: HashMap<DatasetId, StorageQuota>,
    /// Total quotas.
    total_quotas: u64,
    /// Total quota violations.
    total_violations: u64,
}

impl StorageQuotaEnforcer {
    /// Create a new enforcer.
    pub fn new() -> Self {
        StorageQuotaEnforcer {
            quotas: HashMap::new(),
            total_quotas: 0,
            total_violations: 0,
        }
    }

    /// Set a storage quota for a dataset.
    pub fn set_quota(&mut self, dataset_id: DatasetId, max_bytes: u64) {
        self.total_quotas += 1;
        self.quotas.insert(
            dataset_id,
            StorageQuota {
                dataset_id,
                max_bytes,
                used_bytes: 0,
                replica_bytes: 0,
                erasure_bytes: 0,
            },
        );
    }

    /// Record storage usage for a dataset.
    pub fn record_usage(
        &mut self,
        dataset_id: DatasetId,
        replica_bytes: u64,
        erasure_bytes: u64,
    ) -> Result<(), QuotaExceededError> {
        match self.quotas.get_mut(&dataset_id) {
            Some(quota) => {
                quota.replica_bytes = replica_bytes;
                quota.erasure_bytes = erasure_bytes;
                quota.used_bytes = replica_bytes + erasure_bytes;
                if quota.used_bytes > quota.max_bytes {
                    self.total_violations += 1;
                    return Err(QuotaExceededError {
                        region: RegionId::new([0u8; 32]), // Not region-specific
                        requested_bytes: quota.used_bytes,
                        requested_operations: 0,
                        available_bytes: quota.max_bytes,
                        available_operations: 0,
                    });
                }
                Ok(())
            }
            None => Ok(()), // No quota set, allow
        }
    }

    /// Return whether a dataset exceeds its quota.
    pub fn is_over_quota(&self, dataset_id: DatasetId) -> bool {
        self.quotas
            .get(&dataset_id)
            .map(|q| q.used_bytes > q.max_bytes)
            .unwrap_or(false)
    }

    /// Return quota for a dataset.
    pub fn quota_for(&self, dataset_id: DatasetId) -> Option<&StorageQuota> {
        self.quotas.get(&dataset_id)
    }

    /// Return total quotas.
    pub fn total_quotas(&self) -> u64 {
        self.total_quotas
    }

    /// Return total violations.
    pub fn total_violations(&self) -> u64 {
        self.total_violations
    }
}

// ========== PlacementReasoning tests ==========

#[test]
fn placement_reasoning_variants() {
   let reasons = [
       PlacementReasoning::LeastLoaded,
       PlacementReasoning::MostAvailable,
       PlacementReasoning::FailureDomainDiverse,
       PlacementReasoning::RegionAffinity,
       PlacementReasoning::LoadBalanced,
       PlacementReasoning::CapacityConstrained,
   ];
   for r in reasons {
       let _ = format!("{:?}", r);
   }
}

// ========== FailureDomainRef tests ==========

#[test]
fn failure_domain_ref_ordering() {
   let a = FailureDomainRef(1);
   let b = FailureDomainRef(2);
   assert!(a < b);
   assert_ne!(a, b);
}

// ========== ReplicaPlacementRecommendation tests ==========

#[test]
fn placement_recommendation_creation() {
   let recommendation = ReplicaPlacementRecommendation {
       dataset_id: DatasetId::new([1u8; 32]),
       recommended_nodes: vec![NodeId::new([2u8; 32]), NodeId::new([3u8; 32])],
       placement_score: 0.85,
       reasoning: PlacementReasoning::FailureDomainDiverse,
   };
   assert_eq!(recommendation.recommended_nodes.len(), 2);
   assert!((recommendation.placement_score - 0.85).abs() < 0.001);
}

// ========== ReplicaPlacementOptimizer tests ==========

#[test]
fn placement_optimizer_register_and_optimize() {
   let mut optimizer = ReplicaPlacementOptimizer::new();
   let dataset = DatasetId::new([1u8; 32]);
   let n1 = NodeId::new([2u8; 32]);
   let n2 = NodeId::new([3u8; 32]);
   let n3 = NodeId::new([4u8; 32]);

   optimizer.register_node_load(n1, 0.1);
   optimizer.register_node_load(n2, 0.5);
   optimizer.register_node_load(n3, 0.9);
   optimizer.register_node_capacity(n1, 5_000_000_000);
   optimizer.register_node_capacity(n2, 1_000_000_000);
   optimizer.register_node_capacity(n3, 100_000_000);

   let recommendation = optimizer.optimize_placement(dataset, 2, vec![n1, n2, n3]);
   assert_eq!(recommendation.recommended_nodes.len(), 2);
   assert!(recommendation.placement_score >= 0.0);
   assert!(recommendation.placement_score <= 1.0);
   assert_eq!(optimizer.total_optimizations(), 1);
   assert_eq!(optimizer.total_recommendations(), 2);
}

#[test]
fn placement_optimizer_failure_domain_diversity() {
   let mut optimizer = ReplicaPlacementOptimizer::new();
   let dataset = DatasetId::new([1u8; 32]);
   let n1 = NodeId::new([2u8; 32]);
   let n2 = NodeId::new([3u8; 32]);
   let n3 = NodeId::new([4u8; 32]);

   optimizer.register_node_load(n1, 0.1);
   optimizer.register_node_load(n2, 0.1);
   optimizer.register_node_load(n3, 0.1);
   optimizer.register_node_capacity(n1, 5_000_000_000);
   optimizer.register_node_capacity(n2, 5_000_000_000);
   optimizer.register_node_capacity(n3, 5_000_000_000);
   optimizer.register_node_failure_domain(n1, FailureDomainRef(1));
   optimizer.register_node_failure_domain(n2, FailureDomainRef(1));
   optimizer.register_node_failure_domain(n3, FailureDomainRef(2));

   let recommendation = optimizer.optimize_placement(dataset, 3, vec![n1, n2, n3]);
   assert_eq!(recommendation.recommended_nodes.len(), 3);
}

#[test]
fn placement_optimizer_empty_candidates() {
   let mut optimizer = ReplicaPlacementOptimizer::new();
   let recommendation = optimizer.optimize_placement(DatasetId::new([1u8; 32]), 3, vec![]);
   assert!(recommendation.recommended_nodes.is_empty());
   assert_eq!(recommendation.placement_score, 0.0);
}

// ========== CorrelationProtectionRule tests ==========

#[test]
fn protection_rule_creation() {
   let rule = CorrelationProtectionRule {
       rule_id: ProtectionRuleId(1),
       failure_domain: FailureDomainRef(1),
       max_replicas_per_domain: 2,
       enabled: true,
   };
   assert_eq!(rule.max_replicas_per_domain, 2);
   assert!(rule.enabled);
}

// ========== ProtectionRuleId tests ==========

#[test]
fn protection_rule_id_ordering() {
   let a = ProtectionRuleId(10);
   let b = ProtectionRuleId(20);
   assert!(a < b);
   assert_ne!(a, b);
}

// ========== CorrelatedFailureProtector tests ==========

#[test]
fn protector_add_and_check() {
   let mut protector = CorrelatedFailureProtector::new();
   let domain = FailureDomainRef(1);
   protector.add_rule(domain, 2);

   let placements = vec![
       (NodeId::new([1u8; 32]), domain),
       (NodeId::new([2u8; 32]), domain),
   ];
   assert!(protector.check_placement(DatasetId::new([1u8; 32]), &placements));

   let placements = vec![
       (NodeId::new([1u8; 32]), domain),
       (NodeId::new([2u8; 32]), domain),
       (NodeId::new([3u8; 32]), domain),
   ];
   assert!(!protector.check_placement(DatasetId::new([1u8; 32]), &placements));
}

#[test]
fn protector_max_replicas_for_domain() {
   let mut protector = CorrelatedFailureProtector::new();
   let domain = FailureDomainRef(1);
   protector.add_rule(domain, 5);
   assert_eq!(protector.max_replicas_for_domain(domain), Some(5));
   assert_eq!(protector.max_replicas_for_domain(FailureDomainRef(2)), None);
}

#[test]
fn protector_total_rules() {
   let mut protector = CorrelatedFailureProtector::new();
   assert_eq!(protector.total_rules(), 0);
   protector.add_rule(FailureDomainRef(1), 2);
   protector.add_rule(FailureDomainRef(2), 3);
   assert_eq!(protector.total_rules(), 2);
}

// ========== RegionAwarePlacement tests ==========

#[test]
fn region_aware_placement_creation() {
   let placement = RegionAwarePlacement {
       dataset_id: DatasetId::new([1u8; 32]),
       primary_region: RegionId::new([2u8; 32]),
       replica_regions: vec![
           RegionId::new([2u8; 32]),
           RegionId::new([3u8; 32]),
           RegionId::new([4u8; 32]),
       ],
       cross_region_replicas: 2,
       local_replicas: 1,
   };
   assert_eq!(placement.local_replicas, 1);
   assert_eq!(placement.cross_region_replicas, 2);
}

// ========== RegionAwarePlacer tests ==========

#[test]
fn placer_place_replicas() {
   let mut placer = RegionAwarePlacer::new();
   let dataset = DatasetId::new([1u8; 32]);
   let primary = RegionId::new([1u8; 32]);
   let other = RegionId::new([2u8; 32]);

   placer.register_node_region(NodeId::new([1u8; 32]), primary);
   placer.register_node_region(NodeId::new([2u8; 32]), primary);
   placer.register_node_region(NodeId::new([3u8; 32]), other);
   placer.register_region_capacity(primary, 1_000_000_000_000);
   placer.register_region_capacity(other, 500_000_000_000);

   let placement = placer.place_replicas(dataset, primary, 3, vec![
       NodeId::new([1u8; 32]),
       NodeId::new([2u8; 32]),
       NodeId::new([3u8; 32]),
   ]);
   assert_eq!(placement.primary_region, primary);
   assert_eq!(placement.local_replicas, 2);
   assert_eq!(placement.cross_region_replicas, 1);
   assert_eq!(placement.replica_regions.len(), 3);
}

#[test]
fn placer_region_capacity() {
   let mut placer = RegionAwarePlacer::new();
   assert!(placer.region_capacity(RegionId::new([1u8; 32])).is_none());
   placer.register_region_capacity(RegionId::new([1u8; 32]), 1000);
   assert_eq!(placer.region_capacity(RegionId::new([1u8; 32])), Some(1000));
}

#[test]
fn placer_total_decisions() {
   let mut placer = RegionAwarePlacer::new();
   assert_eq!(placer.total_decisions(), 0);
   placer.place_replicas(
       DatasetId::new([1u8; 32]),
       RegionId::new([1u8; 32]),
       1,
       vec![NodeId::new([1u8; 32])],
   );
   assert_eq!(placer.total_decisions(), 1);
}

// ========== BandwidthThrottle tests ==========

#[test]
fn bandwidth_throttle_creation() {
   let throttle = BandwidthThrottle {
       dataset_id: DatasetId::new([1u8; 32]),
       max_bytes_per_second: 1_000_000,
       current_bytes_per_second: 500_000,
       throttled: false,
   };
   assert!(!throttle.throttled);
   assert_eq!(throttle.max_bytes_per_second, 1_000_000);
}

// ========== BandwidthThrottler tests ==========

#[test]
fn throttler_set_and_check() {
   let mut throttler = BandwidthThrottler::new();
   let dataset = DatasetId::new([1u8; 32]);
   throttler.set_throttle(dataset, 1_000_000);

   assert!(!throttler.check_throttle(dataset, 500_000));
   assert!(!throttler.is_throttled(dataset));

   assert!(throttler.check_throttle(dataset, 2_000_000));
   assert!(throttler.is_throttled(dataset));
}

#[test]
fn throttler_no_throttle_set() {
   let mut throttler = BandwidthThrottler::new();
   assert!(!throttler.check_throttle(DatasetId::new([1u8; 32]), 999999));
}

#[test]
fn throttler_counts() {
   let mut throttler = BandwidthThrottler::new();
   let d1 = DatasetId::new([1u8; 32]);
   let d2 = DatasetId::new([2u8; 32]);
   throttler.set_throttle(d1, 100);
   throttler.set_throttle(d2, 100);
   assert_eq!(throttler.total_throttles(), 2);
   throttler.check_throttle(d1, 200);
   throttler.check_throttle(d2, 50);
   assert_eq!(throttler.total_throttled(), 1);
}

// ========== RecoveryPriority tests ==========

#[test]
fn recovery_priority_ordering() {
   assert!(RecoveryPriority::Critical > RecoveryPriority::High);
   assert!(RecoveryPriority::High > RecoveryPriority::Normal);
   assert!(RecoveryPriority::Normal > RecoveryPriority::Low);
}

// ========== RecoveryPriorityEntry tests ==========

#[test]
fn recovery_entry_creation() {
   let entry = RecoveryPriorityEntry {
       dataset_id: DatasetId::new([1u8; 32]),
       priority: RecoveryPriority::Critical,
       lost_fragments: 5,
       total_fragments: 10,
       urgency_score: 0.5,
   };
   assert_eq!(entry.priority, RecoveryPriority::Critical);
   assert_eq!(entry.urgency_score, 0.5);
}

// ========== RecoveryPriorityQueue tests ==========

#[test]
fn priority_queue_add_and_pop() {
   let mut queue = RecoveryPriorityQueue::new();
   let d1 = DatasetId::new([1u8; 32]);
   let d2 = DatasetId::new([2u8; 32]);
   let d3 = DatasetId::new([3u8; 32]);

   queue.add_entry(d1, RecoveryPriority::Low, 1, 10);
   queue.add_entry(d2, RecoveryPriority::Critical, 5, 10);
   queue.add_entry(d3, RecoveryPriority::Normal, 3, 10);

   assert_eq!(queue.total_entries(), 3);

   let first = queue.pop().unwrap();
   assert_eq!(first.dataset_id, d2); // Critical first

   let second = queue.pop().unwrap();
   assert_eq!(second.dataset_id, d3); // Normal second

   let third = queue.pop().unwrap();
   assert_eq!(third.dataset_id, d1); // Low last

   assert!(queue.pop().is_none());
   assert_eq!(queue.total_processed(), 3);
}

#[test]
fn priority_queue_peek() {
   let mut queue = RecoveryPriorityQueue::new();
   assert!(queue.peek().is_none());
   queue.add_entry(DatasetId::new([1u8; 32]), RecoveryPriority::High, 2, 10);
   assert!(queue.peek().is_some());
   assert_eq!(queue.peek().unwrap().priority, RecoveryPriority::High);
}

#[test]
fn priority_queue_update_entry() {
   let mut queue = RecoveryPriorityQueue::new();
   let dataset = DatasetId::new([1u8; 32]);
   queue.add_entry(dataset, RecoveryPriority::Low, 1, 10);
   queue.add_entry(dataset, RecoveryPriority::Critical, 9, 10);
   assert_eq!(queue.total_entries(), 1);
   let entry = queue.peek().unwrap();
   assert_eq!(entry.priority, RecoveryPriority::Critical);
   assert_eq!(entry.lost_fragments, 9);
}

// ========== StorageQuota tests ==========

#[test]
fn storage_quota_creation() {
   let quota = StorageQuota {
       dataset_id: DatasetId::new([1u8; 32]),
       max_bytes: 10_000_000_000,
       used_bytes: 5_000_000_000,
       replica_bytes: 3_000_000_000,
       erasure_bytes: 2_000_000_000,
   };
   assert_eq!(quota.max_bytes, 10_000_000_000);
   assert_eq!(quota.used_bytes, 5_000_000_000);
}

// ========== StorageQuotaEnforcer tests ==========

#[test]
fn quota_enforcer_set_and_check() {
   let mut enforcer = StorageQuotaEnforcer::new();
   let dataset = DatasetId::new([1u8; 32]);
   enforcer.set_quota(dataset, 1_000_000);

   assert!(enforcer.record_usage(dataset, 400_000, 400_000).is_ok());
   assert!(!enforcer.is_over_quota(dataset));

   assert!(enforcer.record_usage(dataset, 300_000, 400_000).is_err());
   assert!(enforcer.is_over_quota(dataset));
}

#[test]
fn quota_enforcer_no_quota() {
   let mut enforcer = StorageQuotaEnforcer::new();
   assert!(enforcer.record_usage(DatasetId::new([1u8; 32]), 999999, 0).is_ok());
}

#[test]
fn quota_enforcer_counts() {
   let mut enforcer = StorageQuotaEnforcer::new();
   let d1 = DatasetId::new([1u8; 32]);
   let d2 = DatasetId::new([2u8; 32]);
   enforcer.set_quota(d1, 100);
   enforcer.set_quota(d2, 100);
   assert_eq!(enforcer.total_quotas(), 2);
   enforcer.record_usage(d1, 200, 0).unwrap_err();
   assert_eq!(enforcer.total_violations(), 1);
}

// ========== QuotaExceededError tests ==========

#[test]
fn quota_exceeded_error_creation() {
   let err = QuotaExceededError {
       region: RegionId::new([1u8; 32]),
       requested_bytes: 5000,
       requested_operations: 10,
       available_bytes: 1000,
       available_operations: 5,
   };
   assert_eq!(err.requested_bytes, 5000);
   assert_eq!(err.available_bytes, 1000);
   let _ = format!("{:?}", err);
}

// ========== Integration test ==========

#[test]
fn full_hybrid_redundancy_integration_v2() {
   let mut manager = HybridRedundancyManager::new();
   let mut optimizer = ReplicaPlacementOptimizer::new();
   let mut protector = CorrelatedFailureProtector::new();
   let mut placer = RegionAwarePlacer::new();
   let mut throttler = BandwidthThrottler::new();
   let mut queue = RecoveryPriorityQueue::new();
   let mut quota = StorageQuotaEnforcer::new();

   let dataset = DatasetId::new([1u8; 32]);
   let r1 = RegionId::new([1u8; 32]);
   let r2 = RegionId::new([2u8; 32]);
   let n1 = NodeId::new([2u8; 32]);
   let n2 = NodeId::new([3u8; 32]);
   let n3 = NodeId::new([4u8; 32]);

   // Set redundancy mode.
   manager.set_redundancy_mode(dataset, RedundancyMode::Hybrid).unwrap();

   // Register node loads and capacities.
   optimizer.register_node_load(n1, 0.2);
   optimizer.register_node_load(n2, 0.3);
   optimizer.register_node_load(n3, 0.1);
   optimizer.register_node_capacity(n1, 5_000_000_000);
   optimizer.register_node_capacity(n2, 3_000_000_000);
   optimizer.register_node_capacity(n3, 8_000_000_000);
   optimizer.register_node_failure_domain(n1, FailureDomainRef(1));
   optimizer.register_node_failure_domain(n2, FailureDomainRef(2));
   optimizer.register_node_failure_domain(n3, FailureDomainRef(1));

   // Optimize placement.
   let recommendation = optimizer.optimize_placement(dataset, 3, vec![n1, n2, n3]);
   assert_eq!(recommendation.recommended_nodes.len(), 3);

   // Register node regions.
   placer.register_node_region(n1, r1);
   placer.register_node_region(n2, r2);
   placer.register_node_region(n3, r1);
   placer.register_region_capacity(r1, 10_000_000_000);
   placer.register_region_capacity(r2, 5_000_000_000);

   // Place replicas with region awareness.
   let placement = placer.place_replicas(dataset, r1, 3, vec![n1, n2, n3]);
   assert_eq!(placement.primary_region, r1);
   assert_eq!(placement.replica_regions.len(), 3);

   // Add correlated failure protection.
   protector.add_rule(FailureDomainRef(1), 2);
   let placements = vec![(n1, FailureDomainRef(1)), (n3, FailureDomainRef(1))];
   assert!(protector.check_placement(dataset, &placements));

   // Set bandwidth throttle.
   throttler.set_throttle(dataset, 1_000_000);
   assert!(!throttler.check_throttle(dataset, 500_000));

   // Add recovery priority entries.
   queue.add_entry(dataset, RecoveryPriority::High, 3, 10);
   let entry = queue.peek().unwrap();
   assert_eq!(entry.dataset_id, dataset);

   // Set storage quota.
   quota.set_quota(dataset, 10_000_000);
   assert!(quota.record_usage(dataset, 4_000_000, 4_000_000).is_ok());
   assert!(!quota.is_over_quota(dataset));

   // Verify integrity.
   manager.verify_integrity(dataset, n1, ReplicaType::Full, None, 1024, 1024, true);
   manager.verify_integrity(dataset, n2, ReplicaType::ErasureFragment, Some(0), 512, 512, true);

   // All subsystems integrated.
   assert!(manager.is_dataset_healthy(dataset));
   assert!(!quota.is_over_quota(dataset));
   assert!(!throttler.is_throttled(dataset));
   assert!(protector.check_placement(dataset, &placements));
}

// ========== ReplicaType all variants ==========

#[test]
fn replica_type_all() {
   let types = [ReplicaType::Full, ReplicaType::ErasureFragment];
   for t in types {
       assert_ne!(format!("{:?}"), "");
   }
}

// ========== RegionAwarePlacer edge cases ==========

#[test]
fn placer_no_candidates() {
   let mut placer = RegionAwarePlacer::new();
   let placement = placer.place_replicas(
       DatasetId::new([1u8; 32]),
       RegionId::new([1u8; 32]),
       3,
       vec![],
   );
   assert!(placement.replica_regions.is_empty());
   assert_eq!(placement.local_replicas, 0);
   assert_eq!(placement.cross_region_replicas, 0);
}

// ========== RecoveryPriorityQueue empty ==========

#[test]
fn priority_queue_empty() {
   let queue = RecoveryPriorityQueue::new();
   assert_eq!(queue.total_entries(), 0);
   assert_eq!(queue.total_processed(), 0);
   assert!(queue.pop().is_none());
   assert!(queue.peek().is_none());
}

// ========== StorageQuotaEnforcer edge cases ==========

#[test]
fn quota_exact_boundary() {
   let mut enforcer = StorageQuotaEnforcer::new();
   let dataset = DatasetId::new([1u8; 32]);
   enforcer.set_quota(dataset, 1000);
   assert!(enforcer.record_usage(dataset, 500, 500).is_ok()); // exactly at boundary
   assert!(!enforcer.is_over_quota(dataset));
   assert!(enforcer.record_usage(dataset, 1, 0).is_err()); // one byte over
   assert!(enforcer.is_over_quota(dataset));
}

// ========== CorrelatedFailureProtector edge cases ==========

#[test]
fn protector_no_rules() {
   let protector = CorrelatedFailureProtector::new();
   let placements = vec![
       (NodeId::new([1u8; 32]), FailureDomainRef(1)),
       (NodeId::new([2u8; 32]), FailureDomainRef(1)),
   ];
   assert!(protector.check_placement(DatasetId::new([1u8; 32]), &placements));
}

// ========== BandwidthThrottler edge cases ==========

#[test]
fn throttler_exact_boundary() {
   let mut throttler = BandwidthThrottler::new();
   let dataset = DatasetId::new([1u8; 32]);
   throttler.set_throttle(dataset, 1000);
   assert!(!throttler.check_throttle(dataset, 1000)); // exactly at boundary
   assert!(throttler.check_throttle(dataset, 1001)); // one byte over
}

// ========== PlacementReasoning all variants ==========

#[test]
fn placement_reasoning_all() {
    let reasons = [
        PlacementReasoning::LeastLoaded,
        PlacementReasoning::MostAvailable,
        PlacementReasoning::FailureDomainDiverse,
        PlacementReasoning::RegionAffinity,
        PlacementReasoning::LoadBalanced,
        PlacementReasoning::CapacityConstrained,
    ];
    for r in reasons {
        assert_ne!(format!("{:?}"), "");
    }
}

// ========== Data Locality Tracker — tracks data locality across nodes
///////////////////////////////////////////////////////////////////////////////

/// A data locality record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLocalityRecord {
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub locality_score: f64,
    pub access_count: u64,
    pub last_access: TimestampMillis,
    pub is_local: bool,
}

/// Tracks data locality for hybrid redundancy placement decisions.
pub struct DataLocalityTracker {
    /// Locality records per dataset.
    records: HashMap<DatasetId, Vec<DataLocalityRecord>>,
    /// Per-node records.
    node_records: HashMap<NodeId, Vec<DataLocalityRecord>>,
    /// Total locality updates.
    total_updates: u64,
    /// Total locality queries.
    total_queries: u64,
}

impl DataLocalityTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        DataLocalityTracker {
            records: HashMap::new(),
            node_records: HashMap::new(),
            total_updates: 0,
            total_queries: 0,
        }
    }

    /// Update locality for a dataset-node pair.
    pub fn update_locality(
        &mut self,
        dataset_id: DatasetId,
        node_id: NodeId,
        locality_score: f64,
        is_local: bool,
    ) {
        self.total_updates += 1;
        let record = DataLocalityRecord {
            dataset_id,
            node_id,
            locality_score: locality_score.clamp(0.0, 1.0),
            access_count: 1,
            last_access: TimestampMillis(0),
            is_local,
        };

        self.records
            .entry(dataset_id)
            .or_default()
            .push(record.clone());
        self.node_records
            .entry(node_id)
            .or_default()
            .push(record);
    }

    /// Record an access to a dataset on a node.
    pub fn record_access(&mut self, dataset_id: DatasetId, node_id: NodeId) {
        self.total_queries += 1;
        // Increment access count for existing records.
        if let Some(dataset_records) = self.records.get_mut(&dataset_id) {
            for record in dataset_records.iter_mut() {
                if record.node_id == node_id {
                    record.access_count += 1;
                    break;
                }
            }
        }
    }

    /// Return locality score for a dataset-node pair.
    pub fn locality_score(&self, dataset_id: DatasetId, node_id: NodeId) -> f64 {
        self.records
            .get(&dataset_id)
            .and_then(|records| {
                records
                    .iter()
                    .find(|r| r.node_id == node_id)
                    .map(|r| r.locality_score)
            })
            .unwrap_or(0.5)
    }

    /// Return whether data is local to a node.
    pub fn is_local(&self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        self.records
            .get(&dataset_id)
            .and_then(|records| {
                records
                    .iter()
                    .find(|r| r.node_id == node_id)
                    .map(|r| r.is_local)
            })
            .unwrap_or(false)
    }

    /// Return records for a dataset.
    pub fn records_for_dataset(&self, dataset_id: DatasetId) -> Vec<DataLocalityRecord> {
        self.records
            .get(&dataset_id)
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Return records for a node.
    pub fn records_for_node(&self, node_id: NodeId) -> Vec<DataLocalityRecord> {
        self.node_records
            .get(&node_id)
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Return total updates.
    pub fn total_updates(&self) -> u64 {
        self.total_updates
    }

    /// Return total queries.
    pub fn total_queries(&self) -> u64 {
        self.total_queries
    }
}

// ========== Redundancy Health Monitor — monitors redundancy health
///////////////////////////////////////////////////////////////////////////////

/// A redundancy health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedundancyHealthStatus {
    Healthy,
    Degraded,
    Critical,
    Failed,
}

/// A redundancy health report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundancyHealthReport {
    pub dataset_id: DatasetId,
    pub status: RedundancyHealthStatus,
    pub full_replicas_healthy: u32,
    pub erasure_fragments_healthy: u32,
    pub total_replicas: u32,
    pub fault_tolerance_remaining: u32,
    pub recommended_action: HealthAction,
}

/// Recommended health action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthAction {
    NoAction,
    RepairReplica,
    AddReplica,
    MigrateData,
    DeclareFailure,
}

/// Monitors redundancy health across datasets.
pub struct RedundancyHealthMonitor {
    /// Health reports per dataset.
    reports: HashMap<DatasetId, RedundancyHealthReport>,
    /// Total reports.
    total_reports: u64,
    /// Total healthy datasets.
    healthy_count: u64,
    /// Total degraded datasets.
    degraded_count: u64,
    /// Total critical datasets.
    critical_count: u64,
}

impl RedundancyHealthMonitor {
    /// Create a new monitor.
    pub fn new() -> Self {
        RedundancyHealthMonitor {
            reports: HashMap::new(),
            total_reports: 0,
            healthy_count: 0,
            degraded_count: 0,
            critical_count: 0,
        }
    }

    /// Record a health report for a dataset.
    pub fn record_health(
        &mut self,
        dataset_id: DatasetId,
        full_healthy: u32,
        erasure_healthy: u32,
        total: u32,
        fault_tolerance: u32,
    ) -> RedundancyHealthReport {
        self.total_reports += 1;
        let healthy_count = full_healthy + erasure_healthy;
        let status = if fault_tolerance == 0 {
            RedundancyHealthStatus::Critical
        } else if healthy_count < total / 2 {
            RedundancyHealthStatus::Degraded
        } else if healthy_count == total {
            RedundancyHealthStatus::Healthy
        } else {
            RedundancyHealthStatus::Degraded
        };

        let recommended_action = match status {
            RedundancyHealthStatus::Healthy => HealthAction::NoAction,
            RedundancyHealthStatus::Degraded => HealthAction::RepairReplica,
            RedundancyHealthStatus::Critical => HealthAction::AddReplica,
            RedundancyHealthStatus::Failed => HealthAction::DeclareFailure,
        };

        let report = RedundancyHealthReport {
            dataset_id,
            status,
            full_replicas_healthy: full_healthy,
            erasure_fragments_healthy: erasure_healthy,
            total_replicas: total,
            fault_tolerance_remaining: fault_tolerance,
            recommended_action,
        };

        // Update counters based on old report.
        if let Some(old) = self.reports.get(&dataset_id) {
            match old.status {
                RedundancyHealthStatus::Healthy => self.healthy_count -= 1,
                RedundancyHealthStatus::Degraded => self.degraded_count -= 1,
                RedundancyHealthStatus::Critical => self.critical_count -= 1,
                _ => {}
            }
        }

        match status {
            RedundancyHealthStatus::Healthy => self.healthy_count += 1,
            RedundancyHealthStatus::Degraded => self.degraded_count += 1,
            RedundancyHealthStatus::Critical => self.critical_count += 1,
            _ => {}
        }

        self.reports.insert(dataset_id, report.clone());
       report
    }

    /// Return the health status for a dataset.
    pub fn health_status(&self, dataset_id: DatasetId) -> RedundancyHealthStatus {
        self.reports
            .get(&dataset_id)
            .map(|r| r.status)
            .unwrap_or(RedundancyHealthStatus::Failed)
    }

    /// Return whether a dataset is healthy.
    pub fn is_healthy(&self, dataset_id: DatasetId) -> bool {
        matches!(
            self.health_status(dataset_id),
            RedundancyHealthStatus::Healthy
        )
    }

    /// Return the report for a dataset.
    pub fn report_for(&self, dataset_id: DatasetId) -> Option<&RedundancyHealthReport> {
        self.reports.get(&dataset_id)
    }

    /// Return total reports.
    pub fn total_reports(&self) -> u64 {
        self.total_reports
    }

    /// Return healthy dataset count.
    pub fn healthy_count(&self) -> u64 {
        self.healthy_count
    }

    /// Return degraded dataset count.
    pub fn degraded_count(&self) -> u64 {
        self.degraded_count
    }

    /// Return critical dataset count.
    pub fn critical_count(&self) -> u64 {
        self.critical_count
    }
}

// ========== Placement History — tracks replica placement history
///////////////////////////////////////////////////////////////////////////////

/// A placement history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementHistoryEntry {
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub action: PlacementAction,
    pub timestamp: TimestampMillis,
    pub replica_type: ReplicaType,
    pub success: bool,
}

/// A placement action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementAction {
    Added,
    Removed,
    Migrated,
    Rebalanced,
    Failed,
}

/// Tracks replica placement history for hybrid redundancy.
pub struct PlacementHistory {
    /// Entries per dataset.
    entries: HashMap<DatasetId, Vec<PlacementHistoryEntry>>,
    /// Total entries.
    total_entries: u64,
    /// Total additions.
    total_additions: u64,
    /// Total removals.
    total_removals: u64,
    /// Total migrations.
    total_migrations: u64,
}

impl PlacementHistory {
    /// Create a new history tracker.
    pub fn new() -> Self {
        PlacementHistory {
            entries: HashMap::new(),
            total_entries: 0,
            total_additions: 0,
            total_removals: 0,
            total_migrations: 0,
        }
    }

    /// Record a placement action.
    pub fn record_action(
        &mut self,
        dataset_id: DatasetId,
        node_id: NodeId,
        action: PlacementAction,
        replica_type: ReplicaType,
        success: bool,
    ) {
        self.total_entries += 1;
        let entry = PlacementHistoryEntry {
            dataset_id,
            node_id,
            action,
            timestamp: TimestampMillis(0),
            replica_type,
            success,
        };

        self.entries.entry(dataset_id).or_default().push(entry);

        match action {
            PlacementAction::Added => self.total_additions += 1,
            PlacementAction::Removed => self.total_removals += 1,
            PlacementAction::Migrated => self.total_migrations += 1,
            _ => {}
        }
    }

    /// Return entries for a dataset.
    pub fn entries_for_dataset(&self, dataset_id: DatasetId) -> Vec<PlacementHistoryEntry> {
        self.entries
            .get(&dataset_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    /// Return successful entries for a dataset.
    pub fn successful_entries_for_dataset(
        &self,
        dataset_id: DatasetId,
    ) -> Vec<PlacementHistoryEntry> {
        self.entries
            .get(&dataset_id)
            .map(|e| e.iter().filter(|e| e.success).cloned().collect())
            .unwrap_or_default()
    }

    /// Return total entries.
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Return total additions.
    pub fn total_additions(&self) -> u64 {
        self.total_additions
    }

    /// Return total removals.
    pub fn total_removals(&self) -> u64 {
        self.total_removals
    }

    /// Return total migrations.
    pub fn total_migrations(&self) -> u64 {
        self.total_migrations
    }
}

// ========== Replica Consistency Checker — checks replica consistency
///////////////////////////////////////////////////////////////////////////////

/// A consistency check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyCheckResult {
    pub dataset_id: DatasetId,
    pub consistent: bool,
    pub replicas_checked: u32,
    pub mismatches: u32,
    pub mismatch_nodes: Vec<NodeId>,
}

/// Checks consistency across replicas for hybrid redundancy.
pub struct ReplicaConsistencyChecker {
    /// Check results per dataset.
    results: HashMap<DatasetId, ConsistencyCheckResult>,
    /// Total checks performed.
    total_checks: u64,
    /// Total inconsistencies found.
    total_inconsistencies: u64,
}

impl ReplicaConsistencyChecker {
    /// Create a new checker.
    pub fn new() -> Self {
        ReplicaConsistencyChecker {
            results: HashMap::new(),
            total_checks: 0,
            total_inconsistencies: 0,
        }
    }

    /// Perform a consistency check across replicas.
    pub fn check_consistency(
        &mut self,
        dataset_id: DatasetId,
        replica_checksums: HashMap<NodeId, u64>,
    ) -> ConsistencyCheckResult {
        self.total_checks += 1;
        let replicas_checked = replica_checksums.len() as u32;

        if replicas_checked <= 1 {
            let result = ConsistencyCheckResult {
                dataset_id,
                consistent: true,
                replicas_checked,
                mismatches: 0,
                mismatch_nodes: Vec::new(),
            };
            self.results.insert(dataset_id, result.clone());
            return result;
        }

        // Find the majority checksum.
        let mut checksum_counts: HashMap<u64, u32> = HashMap::new();
        for &checksum in replica_checksums.values() {
            *checksum_counts.entry(checksum).or_insert(0) += 1;
        }

        let majority_checksum = checksum_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&checksum, _)| checksum)
            .unwrap_or(0);

        let mut mismatches = 0u32;
        let mut mismatch_nodes: Vec<NodeId> = Vec::new();
        for (node, &checksum) in &replica_checksums {
            if checksum != majority_checksum {
                mismatches += 1;
                mismatch_nodes.push(*node);
            }
        }

        let consistent = mismatches == 0;
        if !consistent {
            self.total_inconsistencies += 1;
        }

        let result = ConsistencyCheckResult {
            dataset_id,
            consistent,
            replicas_checked,
            mismatches,
            mismatch_nodes,
        };

        self.results.insert(dataset_id, result.clone());
       result
    }

    /// Return whether a dataset is consistent.
    pub fn is_consistent(&self, dataset_id: DatasetId) -> bool {
        self.results
            .get(&dataset_id)
            .map(|r| r.consistent)
            .unwrap_or(true)
    }

    /// Return the result for a dataset.
    pub fn result_for(&self, dataset_id: DatasetId) -> Option<&ConsistencyCheckResult> {
        self.results.get(&dataset_id)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return total inconsistencies.
    pub fn total_inconsistencies(&self) -> u64 {
        self.total_inconsistencies
    }
}