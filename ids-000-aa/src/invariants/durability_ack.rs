///////////////////////////////////////////////////////////////////////////////
// Durability Ack Invariant — durability-aware acknowledgement
//
// IDS-A §42: Durability-aware acknowledgement ensures that no operation
// overstates its replication status. An ack is only sent when the operation
// has been durably stored on enough replicas to satisfy the dataset's
// durability requirements. Ack levels:
//   None      — received, not yet durable
//   Primary   — durably stored on primary replica only
//   Quorum    — durably stored on a quorum of replicas
//   All       — durably stored on all replicas
//   Unknown   — replication status cannot be determined
//
// Every ack carries:
//   1. The operation ID being acknowledged
//   2. The ack level (None/Primary/Quorum/All/Unknown)
//   3. The number of replicas that have durably stored the operation
//   4. The required durability threshold for the dataset
//   5. Whether the ack is final (no more replicas will be added)
//   6. Timestamps for ack sent/received
//
// This file is part of ids-000-aa, the central hub crate. All types and
// traits here are sealed — downstream crates cannot implement them.
//
// Carrier-agnostic: uses only opaque types (DatasetId, NodeId, RegionId).
// No std::net, SocketAddr, IP, TCP, UDP, or any existing networking stack.
///////////////////////////////////////////////////////////////////////////////

use crate::types::{DatasetId, NodeId, RegionId};
use crate::errors::Error;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// Sealed trait contract
///////////////////////////////////////////////////////////////////////////////

/// Sealed trait guaranteeing durability-aware acknowledgement.
///
/// Downstream crates may call methods and use as trait bounds, but cannot
/// implement — only `ids-000-aa` and its designated implementation crates.
pub trait DurabilityAck: private::Sealed {
    /// Return the ack level for an operation.
    fn ack_level(&self, operation_id: AckOperationId) -> AckLevel;

    /// Return whether an ack is durable (level >= Primary).
    fn is_durable(&self, operation_id: AckOperationId) -> bool;

    /// Return the number of durable replicas for an operation.
    fn durable_replica_count(&self, operation_id: AckOperationId) -> u32;

    /// Return the required durability threshold for a dataset.
    fn durability_threshold(&self, dataset_id: DatasetId) -> u32;

    /// Return whether an ack meets the dataset's durability threshold.
    fn meets_threshold(&self, operation_id: AckOperationId, dataset_id: DatasetId) -> bool;

    /// Return the total acks sent for a dataset.
    fn ack_count_for_dataset(&self, dataset_id: DatasetId) -> u64;
}

mod private {
    /// Sealed supertrait — only ids-000-aa can implement DurabilityAck.
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl DurabilityAck for () {
    fn ack_level(&self, _operation_id: AckOperationId) -> AckLevel {
        AckLevel::None
    }
    fn is_durable(&self, _operation_id: AckOperationId) -> bool {
        false
    }
    fn durable_replica_count(&self, _operation_id: AckOperationId) -> u32 {
        0
    }
    fn durability_threshold(&self, _dataset_id: DatasetId) -> u32 {
        1
    }
    fn meets_threshold(&self, _operation_id: AckOperationId, _dataset_id: DatasetId) -> bool {
        false
    }
    fn ack_count_for_dataset(&self, _dataset_id: DatasetId) -> u64 {
        0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack level
///////////////////////////////////////////////////////////////////////////////

/// The durability level of an acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AckLevel {
    /// Operation received but not yet durably stored.
    None,
    /// Durably stored on the primary replica only.
    Primary,
    /// Durably stored on a quorum of replicas.
    Quorum,
    /// Durably stored on all replicas.
    All,
    /// Replication status cannot be determined.
    Unknown,
}

impl AckLevel {
    /// Return whether this level counts as durable (at least Primary).
    pub fn is_durable(self) -> bool {
        matches!(self, AckLevel::Primary | AckLevel::Quorum | AckLevel::All)
    }

    /// Return whether this level represents full durability (All).
    pub fn is_full(self) -> bool {
        matches!(self, AckLevel::All)
    }

    /// Return whether this level represents quorum durability.
    pub fn is_quorum(self) -> bool {
        matches!(self, AckLevel::Quorum | AckLevel::All)
    }

    /// Return the numeric rank of this level (0..=3).
    pub fn rank(self) -> u8 {
        match self {
            AckLevel::None => 0,
            AckLevel::Primary => 1,
            AckLevel::Quorum => 2,
            AckLevel::All => 3,
            AckLevel::Unknown => 0,
        }
    }

    /// Return whether this level meets or exceeds another level.
    pub fn meets_or_exceeds(self, other: AckLevel) -> bool {
        self.rank() >= other.rank()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack operation identifier
///////////////////////////////////////////////////////////////////////////////

/// Unique identifier for an acknowledgement operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AckOperationId(pub u64);

impl AckOperationId {
    /// Create a new ack operation ID.
    pub fn new(value: u64) -> Self {
        AckOperationId(value)
    }

    /// Return the raw value.
    pub fn value(self) -> u64 {
        self.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack record
///////////////////////////////////////////////////////////////////////////////

/// A complete durability acknowledgement record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckRecord {
    pub ack_id: AckOperationId,
    pub operation_id: OperationId,
    pub dataset_id: DatasetId,
    pub node_id: NodeId,
    pub level: AckLevel,
    pub durable_replicas: u32,
    pub total_replicas: u32,
    pub threshold: u32,
    pub is_final: bool,
    pub sent_at: TimestampMillis,
    pub received_at: Option<TimestampMillis>,
    pub acknowledged_by: NodeId,
}

impl AckRecord {
    /// Return whether this ack meets the durability threshold.
    pub fn meets_threshold(&self) -> bool {
        self.durable_replicas >= self.threshold
    }

    /// Return the replication ratio (durable / total).
    pub fn replication_ratio(&self) -> f64 {
        if self.total_replicas == 0 {
            return 0.0;
        }
        self.durable_replicas as f64 / self.total_replicas as f64
    }

    /// Return whether all replicas have acknowledged.
    pub fn all_acked(&self) -> bool {
        self.durable_replicas >= self.total_replicas
    }
}

// OperationId and TimestampMillis are defined in this file below.
// We define them here to keep this file self-contained.

/// Unique identifier for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(pub u64);

impl OperationId {
    pub fn new(value: u64) -> Self {
        OperationId(value)
    }
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Timestamp in milliseconds since an arbitrary epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimestampMillis(pub u64);

impl TimestampMillis {
    pub fn new(millis: u64) -> Self {
        TimestampMillis(millis)
    }
    pub fn millis(self) -> u64 {
        self.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack tracker — tracks acks per operation and dataset
///////////////////////////////////////////////////////////////////////////////

/// Tracks durability acknowledgements across all operations.
pub struct AckTracker {
    /// Ack records by ack ID.
    acks: HashMap<AckOperationId, AckRecord>,
    /// Per-operation ack IDs.
    operation_acks: HashMap<OperationId, Vec<AckOperationId>>,
    /// Per-dataset ack IDs.
    dataset_acks: HashMap<DatasetId, Vec<AckOperationId>>,
    /// Per-node ack IDs.
    node_acks: HashMap<NodeId, Vec<AckOperationId>>,
    /// Next ack ID.
    next_id: u64,
    /// Total acks tracked.
    total_acks: u64,
    /// Total durable acks (level >= Primary).
    total_durable_acks: u64,
    /// Total final acks.
    total_final_acks: u64,
}

impl AckTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        AckTracker {
            acks: HashMap::new(),
            operation_acks: HashMap::new(),
            dataset_acks: HashMap::new(),
            node_acks: HashMap::new(),
            next_id: 0,
            total_acks: 0,
            total_durable_acks: 0,
            total_final_acks: 0,
        }
    }

    /// Register a new ack record.
    pub fn register_ack(
        &mut self,
        operation_id: OperationId,
        dataset_id: DatasetId,
        node_id: NodeId,
        level: AckLevel,
        durable_replicas: u32,
        total_replicas: u32,
        threshold: u32,
        is_final: bool,
        acknowledged_by: NodeId,
    ) -> AckOperationId {
        let id = AckOperationId(self.next_id);
        self.next_id += 1;
        self.total_acks += 1;
        if level.is_durable() {
            self.total_durable_acks += 1;
        }
        if is_final {
            self.total_final_acks += 1;
        }
        let record = AckRecord {
            ack_id: id,
            operation_id,
            dataset_id,
            node_id,
            level,
            durable_replicas,
            total_replicas,
            threshold,
            is_final,
            sent_at: TimestampMillis(0),
            received_at: None,
            acknowledged_by,
        };
        self.acks.insert(id, record.clone());
        self.operation_acks.entry(operation_id).or_default().push(id);
        self.dataset_acks.entry(dataset_id).or_default().push(id);
        self.node_acks.entry(node_id).or_default().push(id);
        id
    }

    /// Return the ack record for an ack ID.
    pub fn ack_for(&self, ack_id: AckOperationId) -> Option<&AckRecord> {
        self.acks.get(&ack_id)
    }

    /// Return the ack level for an operation (latest ack).
    pub fn ack_level_for_operation(&self, operation_id: OperationId) -> Option<AckLevel> {
        self.operation_acks
            .get(&operation_id)
            .and_then(|ids| ids.last())
            .and_then(|id| self.acks.get(id).map(|r| r.level))
    }

    /// Return whether an operation has a durable ack.
    pub fn is_durable_for_operation(&self, operation_id: OperationId) -> bool {
        self.operation_acks
            .get(&operation_id)
            .and_then(|ids| ids.last())
            .and_then(|id| self.acks.get(id).map(|r| r.level.is_durable()))
            .unwrap_or(false)
    }

    /// Return durable replica count for an operation's latest ack.
    pub fn durable_replica_count_for_operation(&self, operation_id: OperationId) -> u32 {
        self.operation_acks
            .get(&operation_id)
            .and_then(|ids| ids.last())
            .and_then(|id| self.acks.get(id).map(|r| r.durable_replicas))
            .unwrap_or(0)
    }

    /// Return acks for a dataset.
    pub fn acks_for_dataset(&self, dataset_id: DatasetId) -> Vec<AckRecord> {
        self.dataset_acks
            .get(&dataset_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.acks.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return acks for a node.
    pub fn acks_for_node(&self, node_id: NodeId) -> Vec<AckRecord> {
        self.node_acks
            .get(&node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.acks.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return ack count for a dataset.
    pub fn ack_count_for_dataset(&self, dataset_id: DatasetId) -> u64 {
        self.dataset_acks
            .get(&dataset_id)
            .map(|ids| ids.len() as u64)
            .unwrap_or(0)
    }

    /// Return total acks.
    pub fn total_acks(&self) -> u64 {
        self.total_acks
    }

    /// Return total durable acks.
    pub fn total_durable_acks(&self) -> u64 {
        self.total_durable_acks
    }

    /// Return total final acks.
    pub fn total_final_acks(&self) -> u64 {
        self.total_final_acks
    }

    /// Return durable ack rate.
    pub fn durable_ack_rate(&self) -> f64 {
        if self.total_acks == 0 {
            return 1.0;
        }
        self.total_durable_acks as f64 / self.total_acks as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Durability threshold calculator
///////////////////////////////////////////////////////////////////////////////

/// A durability threshold calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityThresholdCalculation {
    pub dataset_id: DatasetId,
    pub replica_count: u32,
    pub threshold: u32,
    pub ack_level_required: AckLevel,
    pub fault_tolerance: u32,
}

/// Calculates durability thresholds for datasets.
pub struct DurabilityThresholdCalculator;

impl DurabilityThresholdCalculator {
    /// Create a new calculator.
    pub fn new() -> Self {
        DurabilityThresholdCalculator
    }

    /// Calculate the durability threshold for a dataset.
    pub fn calculate(
        &self,
        dataset_id: DatasetId,
        replica_count: u32,
        fault_tolerance: u32,
    ) -> DurabilityThresholdCalculation {
        // Threshold = fault_tolerance + 1 (must survive that many failures).
        let threshold = (fault_tolerance + 1).min(replica_count.max(1));
        let ack_level_required = if replica_count == 0 {
            AckLevel::None
        } else if threshold >= replica_count {
            AckLevel::All
        } else if threshold > replica_count / 2 {
            AckLevel::Quorum
        } else {
            AckLevel::Primary
        };

        DurabilityThresholdCalculation {
            dataset_id,
            replica_count,
            threshold,
            ack_level_required,
            fault_tolerance,
        }
    }

    /// Return the quorum size for n replicas.
    pub fn quorum_size(replica_count: u32) -> u32 {
        replica_count / 2 + 1
    }

    /// Return whether a threshold is valid for a replica count.
    pub fn is_valid_threshold(replica_count: u32, threshold: u32) -> bool {
        threshold <= replica_count && threshold > 0
    }

    /// Return the default fault tolerance for a replica count.
    pub fn default_fault_tolerance(replica_count: u32) -> u32 {
        if replica_count == 0 {
            0
        } else {
            (replica_count - 1) / 2
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack level classifier — classifies ack level from replica state
///////////////////////////////////////////////////////////////////////////////

/// Classifies the durability level based on durable replica counts.
pub struct AckLevelClassifier;

impl AckLevelClassifier {
    /// Create a new classifier.
    pub fn new() -> Self {
        AckLevelClassifier
    }

    /// Classify the ack level from durable/total replicas and threshold.
    pub fn classify(
        &self,
        durable_replicas: u32,
        total_replicas: u32,
        threshold: u32,
    ) -> AckLevel {
        if total_replicas == 0 || durable_replicas == 0 {
            return AckLevel::None;
        }
        if durable_replicas >= total_replicas {
            return AckLevel::All;
        }
        if durable_replicas >= Self::quorum_size(total_replicas) && durable_replicas >= threshold {
            return AckLevel::Quorum;
        }
        if durable_replicas >= 1 {
            return AckLevel::Primary;
        }
        AckLevel::None
    }

    /// Return the quorum size for n replicas.
    pub fn quorum_size(replica_count: u32) -> u32 {
        replica_count / 2 + 1
    }

    /// Return whether the given level satisfies the threshold.
    pub fn satisfies_threshold(level: AckLevel, durable: u32, threshold: u32) -> bool {
        durable >= threshold && level.is_durable()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack finality tracker — tracks whether acks are final
///////////////////////////////////////////////////////////////////////////////

/// A finality record for an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalityRecord {
    pub operation_id: OperationId,
    pub is_final: bool,
    pub final_ack_id: Option<AckOperationId>,
    pub finalized_at: Option<TimestampMillis>,
    pub durable_replicas_at_final: u32,
}

/// Tracks finality of durability acknowledgements.
pub struct AckFinalityTracker {
    /// Finality records per operation.
    records: HashMap<OperationId, FinalityRecord>,
    /// Total finalized operations.
    total_finalized: u64,
    /// Total non-final acks.
    total_non_final: u64,
}

impl AckFinalityTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        AckFinalityTracker {
            records: HashMap::new(),
            total_finalized: 0,
            total_non_final: 0,
        }
    }

    /// Record finality for an operation.
    pub fn record_finality(
        &mut self,
        operation_id: OperationId,
        is_final: bool,
        ack_id: Option<AckOperationId>,
        durable_replicas: u32,
    ) {
        if is_final {
            self.total_finalized += 1;
        } else {
            self.total_non_final += 1;
        }
        let record = FinalityRecord {
            operation_id,
            is_final,
            final_ack_id: ack_id,
            finalized_at: if is_final {
                Some(TimestampMillis(0))
            } else {
                None
            },
            durable_replicas_at_final: durable_replicas,
        };
        self.records.insert(operation_id, record);
    }

    /// Return whether an operation is finalized.
    pub fn is_finalized(&self, operation_id: OperationId) -> bool {
        self.records
            .get(&operation_id)
            .map(|r| r.is_final)
            .unwrap_or(false)
    }

    /// Return the finality record for an operation.
    pub fn record_for(&self, operation_id: OperationId) -> Option<&FinalityRecord> {
        self.records.get(&operation_id)
    }

    /// Return total finalized operations.
    pub fn total_finalized(&self) -> u64 {
        self.total_finalized
    }

    /// Return total non-final acks.
    pub fn total_non_final(&self) -> u64 {
        self.total_non_final
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack overstatement detector — detects when acks overstate durability
///////////////////////////////////////////////////////////////////////////////

/// An overstatement detection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverstatementResult {
    pub operation_id: OperationId,
    pub overstated: bool,
    pub claimed_level: AckLevel,
    pub actual_level: AckLevel,
    pub durable_replicas: u32,
    pub threshold: u32,
    pub severity: OverstatementSeverity,
}

/// Severity of an overstatement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OverstatementSeverity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Detects when an ack overstates the actual durability of an operation.
pub struct AckOverstatementDetector {
    /// Detection results per operation.
    results: HashMap<OperationId, OverstatementResult>,
    /// Total detections performed.
    total_detections: u64,
    /// Total overstatements found.
    total_overstatements: u64,
}

impl AckOverstatementDetector {
    /// Create a new detector.
    pub fn new() -> Self {
        AckOverstatementDetector {
            results: HashMap::new(),
            total_detections: 0,
            total_overstatements: 0,
        }
    }

    /// Detect whether an ack overstates durability.
    pub fn detect(
        &mut self,
        operation_id: OperationId,
        claimed_level: AckLevel,
        durable_replicas: u32,
        total_replicas: u32,
        threshold: u32,
    ) -> OverstatementResult {
        self.total_detections += 1;
        let classifier = AckLevelClassifier::new();
        let actual_level = classifier.classify(durable_replicas, total_replicas, threshold);
        let overstated = claimed_level.rank() > actual_level.rank();

        let severity = if !overstated {
            OverstatementSeverity::None
        } else {
            let diff = claimed_level.rank() as i32 - actual_level.rank() as i32;
            match diff {
                1 => OverstatementSeverity::Low,
                2 => OverstatementSeverity::Medium,
                3 => OverstatementSeverity::High,
                _ => OverstatementSeverity::Critical,
            }
        };

        if overstated {
            self.total_overstatements += 1;
        }

        let result = OverstatementResult {
            operation_id,
            overstated,
            claimed_level,
            actual_level,
            durable_replicas,
            threshold,
            severity,
        };

        self.results.insert(operation_id, result.clone());
        result
    }

    /// Return whether an operation was overstated.
    pub fn is_overstated(&self, operation_id: OperationId) -> bool {
        self.results
            .get(&operation_id)
            .map(|r| r.overstated)
            .unwrap_or(false)
    }

    /// Return the result for an operation.
    pub fn result_for(&self, operation_id: OperationId) -> Option<&OverstatementResult> {
        self.results.get(&operation_id)
    }

    /// Return total detections.
    pub fn total_detections(&self) -> u64 {
        self.total_detections
    }

    /// Return total overstatements.
    pub fn total_overstatements(&self) -> u64 {
        self.total_overstatements
    }

    /// Return overstatement rate.
    pub fn overstatement_rate(&self) -> f64 {
        if self.total_detections == 0 {
            return 0.0;
        }
        self.total_overstatements as f64 / self.total_detections as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack manager — core coordinator
///////////////////////////////////////////////////////////////////////////////

/// Configuration for durability acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityAckConfig {
    pub default_threshold: u32,
    pub default_fault_tolerance: u32,
    pub require_quorum_for_write: bool,
    pub reject_overstated_acks: bool,
}

impl Default for DurabilityAckConfig {
    fn default() -> Self {
        DurabilityAckConfig {
            default_threshold: 1,
            default_fault_tolerance: 0,
            require_quorum_for_write: false,
            reject_overstated_acks: true,
        }
    }
}

/// Core durability acknowledgement manager.
pub struct DurabilityAckManager {
    /// Configuration.
    config: DurabilityAckConfig,
    /// Ack tracker.
    tracker: AckTracker,
    /// Threshold calculator.
    threshold_calculator: DurabilityThresholdCalculator,
    /// Level classifier.
    classifier: AckLevelClassifier,
    /// Finality tracker.
    finality_tracker: AckFinalityTracker,
    /// Overstatement detector.
    overstatement_detector: AckOverstatementDetector,
    /// Total acks processed.
    total_acks_processed: u64,
    /// Total acks rejected.
    total_acks_rejected: u64,
}

impl DurabilityAckManager {
    /// Create a new manager with default config.
    pub fn new() -> Self {
        DurabilityAckManager {
            config: DurabilityAckConfig::default(),
            tracker: AckTracker::new(),
            threshold_calculator: DurabilityThresholdCalculator::new(),
            classifier: AckLevelClassifier::new(),
            finality_tracker: AckFinalityTracker::new(),
            overstatement_detector: AckOverstatementDetector::new(),
            total_acks_processed: 0,
            total_acks_rejected: 0,
        }
    }

    /// Create a manager with custom config.
    pub fn with_config(config: DurabilityAckConfig) -> Self {
        DurabilityAckManager {
            config,
            tracker: AckTracker::new(),
            threshold_calculator: DurabilityThresholdCalculator::new(),
            classifier: AckLevelClassifier::new(),
            finality_tracker: AckFinalityTracker::new(),
            overstatement_detector: AckOverstatementDetector::new(),
            total_acks_processed: 0,
            total_acks_rejected: 0,
        }
    }

    /// Process a durability acknowledgement.
    pub fn process_ack(
        &mut self,
        operation_id: OperationId,
        dataset_id: DatasetId,
        node_id: NodeId,
        level: AckLevel,
        durable_replicas: u32,
        total_replicas: u32,
        threshold: u32,
        is_final: bool,
        acknowledged_by: NodeId,
    ) -> Result<AckOperationId, Error> {
        self.total_acks_processed += 1;

        // Detect overstatement.
        let detection = self.overstatement_detector.detect(
            operation_id,
            level,
            durable_replicas,
            total_replicas,
            threshold,
        );

        if detection.overstated && self.config.reject_overstated_acks {
            self.total_acks_rejected += 1;
            return Err(Error::Internal {
                message: format!(
                    "overstated ack rejected: claimed {:?}, actual {:?}",
                    detection.claimed_level, detection.actual_level
                ),
            });
        }

        let ack_id = self.tracker.register_ack(
            operation_id,
            dataset_id,
            node_id,
            level,
            durable_replicas,
            total_replicas,
            threshold,
            is_final,
            acknowledged_by,
        );

        self.finality_tracker.record_finality(
            operation_id,
            is_final,
            Some(ack_id),
            durable_replicas,
        );

        Ok(ack_id)
    }

    /// Return the ack level for an operation.
    pub fn ack_level(&self, operation_id: AckOperationId) -> AckLevel {
        self.tracker
            .ack_for(operation_id)
            .map(|r| r.level)
            .unwrap_or(AckLevel::Unknown)
    }

    /// Return whether an ack is durable.
    pub fn is_durable(&self, operation_id: AckOperationId) -> bool {
        self.tracker
            .ack_for(operation_id)
            .map(|r| r.level.is_durable())
            .unwrap_or(false)
    }

    /// Return durable replica count for an operation's ack.
    pub fn durable_replica_count(&self, operation_id: AckOperationId) -> u32 {
        self.tracker
            .ack_for(operation_id)
            .map(|r| r.durable_replicas)
            .unwrap_or(0)
    }

    /// Return the durability threshold for a dataset.
    pub fn durability_threshold(&self, dataset_id: DatasetId) -> u32 {
        self.config.default_threshold
    }

    /// Return whether an ack meets the dataset's durability threshold.
    pub fn meets_threshold(
        &self,
        operation_id: AckOperationId,
        dataset_id: DatasetId,
    ) -> bool {
        let ack = match self.tracker.ack_for(operation_id) {
            Some(a) => a,
            None => return false,
        };
        let threshold = self.durability_threshold(dataset_id);
        ack.durable_replicas >= threshold
    }

    /// Return total acks for a dataset.
    pub fn ack_count_for_dataset(&self, dataset_id: DatasetId) -> u64 {
        self.tracker.ack_count_for_dataset(dataset_id)
    }

    /// Return whether an operation is finalized.
    pub fn is_finalized(&self, operation_id: OperationId) -> bool {
        self.finality_tracker.is_finalized(operation_id)
    }

    /// Return whether an ack was overstated.
    pub fn is_overstated(&self, operation_id: OperationId) -> bool {
        self.overstatement_detector.is_overstated(operation_id)
    }

    /// Return total acks processed.
    pub fn total_acks_processed(&self) -> u64 {
        self.total_acks_processed
    }

    /// Return total acks rejected.
    pub fn total_acks_rejected(&self) -> u64 {
        self.total_acks_rejected
    }

    /// Return overstatement rate.
    pub fn overstatement_rate(&self) -> f64 {
        self.overstatement_detector.overstatement_rate()
    }

    /// Return durable ack rate.
    pub fn durable_ack_rate(&self) -> f64 {
        self.tracker.durable_ack_rate()
    }

    /// Return the threshold calculator.
    pub fn threshold_calculator(&self) -> &DurabilityThresholdCalculator {
        &self.threshold_calculator
    }

    /// Return the classifier.
    pub fn classifier(&self) -> &AckLevelClassifier {
        &self.classifier
    }

    /// Return the tracker.
    pub fn tracker(&self) -> &AckTracker {
        &self.tracker
    }
}

///////////////////////////////////////////////////////////////////////////////
// Ack stats aggregator
///////////////////////////////////////////////////////////////////////////////

/// Aggregated durability ack statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckStats {
    pub total_acks: u64,
    pub durable_acks: u64,
    pub final_acks: u64,
    pub overstatements: u64,
    pub rejections: u64,
    pub durable_rate: f64,
    pub overstatement_rate: f64,
    pub rejection_rate: f64,
}

/// Aggregates durability ack statistics.
pub struct AckStatsAggregator;

impl AckStatsAggregator {
    /// Create a new aggregator.
    pub fn new() -> Self {
        AckStatsAggregator
    }

    /// Aggregate stats from a manager.
    pub fn aggregate(&self, manager: &DurabilityAckManager) -> AckStats {
        let total = manager.total_acks_processed();
        let durable = manager.tracker().total_durable_acks();
        let final_count = manager.tracker().total_final_acks();
        let overstatements = manager.overstatement_detector.total_overstatements();
        let rejections = manager.total_acks_rejected();

        let durable_rate = if total > 0 {
            durable as f64 / total as f64
        } else {
            1.0
        };
        let overstatement_rate = if total > 0 {
            overstatements as f64 / total as f64
        } else {
            0.0
        };
        let rejection_rate = if total > 0 {
            rejections as f64 / total as f64
        } else {
            0.0
        };

        AckStats {
            total_acks: total,
            durable_acks: durable,
            final_acks: final_count,
            overstatements,
            rejections,
            durable_rate,
            overstatement_rate,
            rejection_rate,
        }
    }

    /// Return whether the stats indicate healthy acknowledgement behavior.
    pub fn is_healthy(stats: &AckStats) -> bool {
        stats.overstatement_rate < 0.01 && stats.rejection_rate < 0.05
    }
}

///////////////////////////////////////////////////////////////////////////////
// AckBatcher — batches acks for efficiency
///////////////////////////////////////////////////////////////////////////////

/// A batch of acks ready to be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckBatch {
    pub batch_id: AckBatchId,
    pub acks: Vec<AckOperationId>,
    pub dataset_id: DatasetId,
    pub created_at: TimestampMillis,
    pub sent_at: Option<TimestampMillis>,
    pub size_bytes: u64,
}

/// Unique identifier for an ack batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AckBatchId(pub u64);

/// Batches durability acknowledgements for efficient transmission.
pub struct AckBatcher {
    /// Pending acks waiting to be batched.
    pending: Vec<AckOperationId>,
    /// Completed batches.
    batches: Vec<AckBatch>,
    /// Next batch ID.
    next_id: u64,
    /// Max batch size.
    max_batch_size: usize,
    /// Max batch delay in millis.
    max_delay_millis: u64,
    /// Total batches created.
    total_batches: u64,
    /// Total acks batched.
    total_acks_batched: u64,
}

impl AckBatcher {
    /// Create a new batcher.
    pub fn new() -> Self {
        AckBatcher {
            pending: Vec::new(),
            batches: Vec::new(),
            next_id: 0,
            max_batch_size: 100,
            max_delay_millis: 50,
            total_batches: 0,
            total_acks_batched: 0,
        }
    }

    /// Set max batch size.
    pub fn set_max_batch_size(&mut self, size: usize) {
        self.max_batch_size = size;
    }

    /// Set max batch delay.
    pub fn set_max_delay_millis(&mut self, millis: u64) {
        self.max_delay_millis = millis;
    }

    /// Add an ack to the pending queue.
    pub fn add_ack(&mut self, ack_id: AckOperationId) {
        self.pending.push(ack_id);
    }

    /// Create a batch from pending acks.
    pub fn create_batch(&mut self, dataset_id: DatasetId) -> Option<AckBatch> {
        if self.pending.is_empty() {
            return None;
        }
        let batch_size = self.pending.len().min(self.max_batch_size);
        let acks: Vec<AckOperationId> = self.pending.drain(..batch_size).collect();
        let size_bytes = (acks.len() * 16) as u64;
        let id = AckBatchId(self.next_id);
        self.next_id += 1;
        self.total_batches += 1;
        self.total_acks_batched += acks.len() as u64;
        let batch = AckBatch {
            batch_id: id,
            acks,
            dataset_id,
            created_at: TimestampMillis(0),
            sent_at: None,
            size_bytes,
        };
        self.batches.push(batch.clone());
        Some(batch)
    }

    /// Mark a batch as sent.
    pub fn mark_batch_sent(&mut self, batch_id: AckBatchId) -> bool {
        if let Some(batch) = self.batches.iter_mut().find(|b| b.batch_id == batch_id) {
            batch.sent_at = Some(TimestampMillis(0));
            true
        } else {
            false
        }
    }

    /// Return the number of pending acks.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Return total batches.
    pub fn total_batches(&self) -> u64 {
        self.total_batches
    }

    /// Return total acks batched.
    pub fn total_acks_batched(&self) -> u64 {
        self.total_acks_batched
    }

    /// Return whether batching is needed (pending >= max).
    pub fn should_batch(&self) -> bool {
        self.pending.len() >= self.max_batch_size
    }
}

///////////////////////////////////////////////////////////////////////////////
// AckLatencyTracker — tracks ack latency
///////////////////////////////////////////////////////////////////////////////

/// A latency measurement for an ack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckLatency {
    pub ack_id: AckOperationId,
    pub sent_at: TimestampMillis,
    pub received_at: Option<TimestampMillis>,
    pub latency_millis: Option<u64>,
}

/// Tracks latency of durability acknowledgements.
pub struct AckLatencyTracker {
    /// Latency measurements per ack.
    latencies: HashMap<AckOperationId, AckLatency>,
    /// Total latencies recorded.
    total_latencies: u64,
    /// Total completed latencies (received).
    total_completed: u64,
    /// Sum of all latencies in millis.
    total_latency_millis: u64,
}

impl AckLatencyTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        AckLatencyTracker {
            latencies: HashMap::new(),
            total_latencies: 0,
            total_completed: 0,
            total_latency_millis: 0,
        }
    }

    /// Record that an ack was sent.
    pub fn record_sent(&mut self, ack_id: AckOperationId, sent_at: TimestampMillis) {
        self.total_latencies += 1;
        let latency = AckLatency {
            ack_id,
            sent_at,
            received_at: None,
            latency_millis: None,
        };
        self.latencies.insert(ack_id, latency);
    }

    /// Record that an ack was received.
    pub fn record_received(&mut self, ack_id: AckOperationId, received_at: TimestampMillis) -> Option<u64> {
        let latency = self.latencies.get_mut(&ack_id)?;
        latency.received_at = Some(received_at);
        let latency_millis = received_at.millis().saturating_sub(latency.sent_at.millis());
        latency.latency_millis = Some(latency_millis);
        self.total_completed += 1;
        self.total_latency_millis += latency_millis;
        Some(latency_millis)
    }

    /// Return the latency for an ack.
    pub fn latency_for(&self, ack_id: AckOperationId) -> Option<&AckLatency> {
        self.latencies.get(&ack_id)
    }

    /// Return average latency in millis.
    pub fn average_latency_millis(&self) -> f64 {
        if self.total_completed == 0 {
            return 0.0;
        }
        self.total_latency_millis as f64 / self.total_completed as f64
    }

    /// Return total latencies recorded.
    pub fn total_latencies(&self) -> u64 {
        self.total_latencies
    }

    /// Return total completed latencies.
    pub fn total_completed(&self) -> u64 {
        self.total_completed
    }

    /// Return pending (sent but not received) count.
    pub fn pending_count(&self) -> u64 {
        self.total_latencies.saturating_sub(self.total_completed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// AckRetryManager — retries unacked operations
///////////////////////////////////////////////////////////////////////////////

/// A retry attempt for an unacked operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryAttempt {
    pub attempt_id: RetryAttemptId,
    pub operation_id: OperationId,
    pub attempt_number: u32,
    pub next_retry_at: TimestampMillis,
    pub max_attempts: u32,
    pub completed: bool,
}

/// Unique identifier for a retry attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RetryAttemptId(pub u64);

/// Manages retries for operations that have not been acknowledged.
pub struct AckRetryManager {
    /// Retry attempts per operation.
    attempts: HashMap<OperationId, Vec<RetryAttempt>>,
    /// Pending retries sorted by next retry time.
    pending: Vec<(TimestampMillis, OperationId)>,
    /// Next attempt ID.
    next_id: u64,
    /// Default max attempts.
    default_max_attempts: u32,
    /// Total retry attempts.
    total_attempts: u64,
    /// Total completed retries.
    total_completed: u64,
    /// Total failed retries (exceeded max).
    total_failed: u64,
}

impl AckRetryManager {
    /// Create a new manager.
    pub fn new() -> Self {
        AckRetryManager {
            attempts: HashMap::new(),
            pending: Vec::new(),
            next_id: 0,
            default_max_attempts: 5,
            total_attempts: 0,
            total_completed: 0,
            total_failed: 0,
        }
    }

    /// Set default max attempts.
    pub fn set_default_max_attempts(&mut self, attempts: u32) {
        self.default_max_attempts = attempts;
    }

    /// Schedule a retry for an unacked operation.
    pub fn schedule_retry(
        &mut self,
        operation_id: OperationId,
        next_retry_at: TimestampMillis,
    ) -> RetryAttemptId {
        let attempts_for_op = self.attempts.get(&operation_id).map(|v| v.len() as u32).unwrap_or(0);
        let attempt_number = attempts_for_op + 1;
        let id = RetryAttemptId(self.next_id);
        self.next_id += 1;
        self.total_attempts += 1;
        let attempt = RetryAttempt {
            attempt_id: id,
            operation_id,
            attempt_number,
            next_retry_at,
            max_attempts: self.default_max_attempts,
            completed: false,
        };
        self.attempts.entry(operation_id).or_default().push(attempt.clone());
        self.pending.push((next_retry_at, operation_id));
        self.pending.sort_by_key(|(t, _)| *t);
        id
    }

    /// Mark a retry attempt as completed.
    pub fn complete_retry(&mut self, attempt_id: RetryAttemptId) -> bool {
        for (_, attempts) in self.attempts.iter_mut() {
            if let Some(attempt) = attempts.iter_mut().find(|a| a.attempt_id == attempt_id) {
                attempt.completed = true;
                self.total_completed += 1;
                return true;
            }
        }
        false
    }

    /// Mark a retry attempt as failed (exceeded max attempts).
    pub fn fail_retry(&mut self, attempt_id: RetryAttemptId) -> bool {
        for (_, attempts) in self.attempts.iter_mut() {
            if let Some(attempt) = attempts.iter_mut().find(|a| a.attempt_id == attempt_id) {
                attempt.completed = true;
                self.total_failed += 1;
                return true;
            }
        }
        false
    }

    /// Return pending retries due at or before the given time.
    pub fn due_retries(&self, now: TimestampMillis) -> Vec<OperationId> {
        self.pending
            .iter()
            .filter(|(t, _)| t.millis() <= now.millis())
            .map(|(_, op)| *op)
            .collect()
    }

    /// Return attempts for an operation.
    pub fn attempts_for_operation(&self, operation_id: OperationId) -> Vec<RetryAttempt> {
        self.attempts
            .get(&operation_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Return total attempts.
    pub fn total_attempts(&self) -> u64 {
        self.total_attempts
    }

    /// Return total completed retries.
    pub fn total_completed(&self) -> u64 {
        self.total_completed
    }

    /// Return total failed retries.
    pub fn total_failed(&self) -> u64 {
        self.total_failed
    }
}

///////////////////////////////////////////////////////////////////////////////
// AckConvergenceChecker — checks ack convergence across replicas
///////////////////////////////////////////////////////////////////////////////

/// A convergence check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvergenceResult {
    pub operation_id: OperationId,
    pub converged: bool,
    pub replicas_acked: u32,
    pub total_replicas: u32,
    pub convergence_ratio: f64,
    pub missing_replicas: Vec<NodeId>,
}

/// Checks whether acks have converged across all replicas.
pub struct AckConvergenceChecker {
    /// Convergence results per operation.
    results: HashMap<OperationId, ConvergenceResult>,
    /// Total checks performed.
    total_checks: u64,
    /// Total converged operations.
    total_converged: u64,
}

impl AckConvergenceChecker {
    /// Create a new checker.
    pub fn new() -> Self {
        AckConvergenceChecker {
            results: HashMap::new(),
            total_checks: 0,
            total_converged: 0,
        }
    }

    /// Check convergence for an operation.
    pub fn check_convergence(
        &mut self,
        operation_id: OperationId,
        acked_replicas: BTreeSet<NodeId>,
        total_replicas: u32,
        all_replicas: BTreeSet<NodeId>,
    ) -> ConvergenceResult {
        self.total_checks += 1;
        let replicas_acked = acked_replicas.len() as u32;
        let convergence_ratio = if total_replicas > 0 {
            replicas_acked as f64 / total_replicas as f64
        } else {
            0.0
        };
        let converged = replicas_acked >= total_replicas;
        let missing: Vec<NodeId> = all_replicas
            .difference(&acked_replicas)
            .copied()
            .collect();

        if converged {
            self.total_converged += 1;
        }

        let result = ConvergenceResult {
            operation_id,
            converged,
            replicas_acked,
            total_replicas,
            convergence_ratio,
            missing_replicas: missing,
        };

        self.results.insert(operation_id, result.clone());
        result
    }

    /// Return whether an operation has converged.
    pub fn is_converged(&self, operation_id: OperationId) -> bool {
        self.results
            .get(&operation_id)
            .map(|r| r.converged)
            .unwrap_or(false)
    }

    /// Return the result for an operation.
    pub fn result_for(&self, operation_id: OperationId) -> Option<&ConvergenceResult> {
        self.results.get(&operation_id)
    }

    /// Return total checks.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Return total converged operations.
    pub fn total_converged(&self) -> u64 {
        self.total_converged
    }

    /// Return convergence rate.
    pub fn convergence_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 1.0;
        }
        self.total_converged as f64 / self.total_checks as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// AckThrottleController — throttles ack rate
///////////////////////////////////////////////////////////////////////////////

/// A throttle decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckThrottleDecision {
    Allow,
    Throttle,
    Reject,
}

/// Throttle configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckThrottleConfig {
    pub max_acks_per_second: u64,
    pub max_pending_acks: u64,
    pub backoff_factor: f64,
}

impl Default for AckThrottleConfig {
    fn default() -> Self {
        AckThrottleConfig {
            max_acks_per_second: 10_000,
            max_pending_acks: 100_000,
            backoff_factor: 2.0,
        }
    }
}

/// Controls the rate of durability acknowledgements.
pub struct AckThrottleController {
    /// Configuration.
    config: AckThrottleConfig,
    /// Current acks per second.
    current_rate: u64,
    /// Current pending ack count.
    pending_count: u64,
    /// Total throttled acks.
    total_throttled: u64,
    /// Total rejected acks.
    total_rejected: u64,
    /// Total allowed acks.
    total_allowed: u64,
}

impl AckThrottleController {
    /// Create a new controller.
    pub fn new() -> Self {
        AckThrottleController {
            config: AckThrottleConfig::default(),
            current_rate: 0,
            pending_count: 0,
            total_throttled: 0,
            total_rejected: 0,
            total_allowed: 0,
        }
    }

    /// Update the current rate and pending count.
    pub fn update(&mut self, rate: u64, pending: u64) {
        self.current_rate = rate;
        self.pending_count = pending;
    }

    /// Make a throttle decision for a new ack.
    pub fn decide(&mut self) -> AckThrottleDecision {
        if self.pending_count >= self.config.max_pending_acks {
            self.total_rejected += 1;
            return AckThrottleDecision::Reject;
        }
        if self.current_rate >= self.config.max_acks_per_second {
            self.total_throttled += 1;
            return AckThrottleDecision::Throttle;
        }
        self.total_allowed += 1;
        AckThrottleDecision::Allow
    }

    /// Return whether acks should be throttled.
    pub fn should_throttle(&self) -> bool {
        self.current_rate >= self.config.max_acks_per_second
    }

    /// Return total throttled.
    pub fn total_throttled(&self) -> u64 {
        self.total_throttled
    }

    /// Return total rejected.
    pub fn total_rejected(&self) -> u64 {
        self.total_rejected
    }

    /// Return total allowed.
    pub fn total_allowed(&self) -> u64 {
        self.total_allowed
    }

    /// Return the current config.
    pub fn config(&self) -> &AckThrottleConfig {
        &self.config
    }
}

// ========== AckAckNackTracker — tracks acks and nacks together
///////////////////////////////////////////////////////////////////////////////

/// An ack or nack record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckOrNack {
    Ack(AckLevel),
    Nack(NackReason),
}

/// Reason for a negative acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NackReason {
    NotDurable,
    Overstated,
    QuorumNotMet,
    NodeUnavailable,
    Throttled,
    Unknown,
}

/// Tracks acks and nacks together for a balanced view.
pub struct AckNackTracker {
    /// Records per operation.
    records: HashMap<OperationId, Vec<(AckOperationId, AckOrNack, TimestampMillis)>>,
    /// Total acks.
    total_acks: u64,
    /// Total nacks.
    total_nacks: u64,
    /// Total nacks by reason.
    nacks_by_reason: HashMap<NackReason, u64>,
}

impl AckNackTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        AckNackTracker {
            records: HashMap::new(),
            total_acks: 0,
            total_nacks: 0,
            nacks_by_reason: HashMap::new(),
        }
    }

    /// Record an ack.
    pub fn record_ack(
        &mut self,
        operation_id: OperationId,
        ack_id: AckOperationId,
        level: AckLevel,
        timestamp: TimestampMillis,
    ) {
        self.total_acks += 1;
        self.records
            .entry(operation_id)
            .or_default()
            .push((ack_id, AckOrNack::Ack(level), timestamp));
    }

    /// Record a nack.
    pub fn record_nack(
        &mut self,
        operation_id: OperationId,
        ack_id: AckOperationId,
        reason: NackReason,
        timestamp: TimestampMillis,
    ) {
        self.total_nacks += 1;
        *self.nacks_by_reason.entry(reason).or_insert(0) += 1;
        self.records
            .entry(operation_id)
            .or_default()
            .push((ack_id, AckOrNack::Nack(reason), timestamp));
    }

    /// Return whether an operation has any nacks.
    pub fn has_nack(&self, operation_id: OperationId) -> bool {
        self.records
            .get(&operation_id)
            .map(|r| r.iter().any(|(_, kind, _)| matches!(kind, AckOrNack::Nack(_))))
            .unwrap_or(false)
    }

    /// Return the nack count for an operation.
    pub fn nack_count_for_operation(&self, operation_id: OperationId) -> u64 {
        self.records
            .get(&operation_id)
            .map(|r| {
                r.iter()
                    .filter(|(_, kind, _)| matches!(kind, AckOrNack::Nack(_)))
                    .count() as u64
            })
            .unwrap_or(0)
    }

    /// Return total acks.
    pub fn total_acks(&self) -> u64 {
        self.total_acks
    }

    /// Return total nacks.
    pub fn total_nacks(&self) -> u64 {
        self.total_nacks
    }

    /// Return nack rate.
    pub fn nack_rate(&self) -> f64 {
        let total = self.total_acks + self.total_nacks;
        if total == 0 {
            return 0.0;
        }
        self.total_nacks as f64 / total as f64
    }

    /// Return nack count by reason.
    pub fn nack_count_by_reason(&self, reason: NackReason) -> u64 {
        self.nacks_by_reason.get(&reason).copied().unwrap_or(0)
    }

    /// Return records for an operation.
    pub fn records_for_operation(&self, operation_id: OperationId) -> Vec<(AckOperationId, AckOrNack, TimestampMillis)> {
        self.records.get(&operation_id).cloned().unwrap_or_default()
    }
}

// ========== AckDurabilityPolicy — policy enforcement for durability
///////////////////////////////////////////////////////////////////////////////

/// A durability policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityPolicyDecision {
    Accept,
    RequireMoreReplicas,
    Reject,
}

/// A durability policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityPolicy {
    pub min_durable_replicas: u32,
    pub min_ack_level: AckLevel,
    pub require_final_for_commit: bool,
    pub allow_unknown_level: bool,
}

impl Default for DurabilityPolicy {
    fn default() -> Self {
        DurabilityPolicy {
            min_durable_replicas: 1,
            min_ack_level: AckLevel::Primary,
            require_final_for_commit: false,
            allow_unknown_level: false,
        }
    }
}

/// Enforces durability policy on acknowledgements.
pub struct AckDurabilityPolicy {
    /// Policy per dataset.
    policies: HashMap<DatasetId, DurabilityPolicy>,
    /// Default policy.
    default_policy: DurabilityPolicy,
    /// Total decisions made.
    total_decisions: u64,
    /// Total rejections.
    total_rejections: u64,
    /// Total require-more-replicas decisions.
    total_require_more: u64,
}

impl AckDurabilityPolicy {
    /// Create a new enforcer.
    pub fn new() -> Self {
        AckDurabilityPolicy {
            policies: HashMap::new(),
            default_policy: DurabilityPolicy::default(),
            total_decisions: 0,
            total_rejections: 0,
            total_require_more: 0,
        }
    }

    /// Set policy for a dataset.
    pub fn set_policy(&mut self, dataset_id: DatasetId, policy: DurabilityPolicy) {
        self.policies.insert(dataset_id, policy);
    }

    /// Set default policy.
    pub fn set_default_policy(&mut self, policy: DurabilityPolicy) {
        self.default_policy = policy;
    }

    /// Make a durability policy decision.
    pub fn decide(
        &mut self,
        dataset_id: DatasetId,
        level: AckLevel,
        durable_replicas: u32,
        is_final: bool,
    ) -> DurabilityPolicyDecision {
        self.total_decisions += 1;
        let policy = self.policies.get(&dataset_id).unwrap_or(&self.default_policy);

        // Unknown level handling.
        if level == AckLevel::Unknown {
            if policy.allow_unknown_level {
                return DurabilityPolicyDecision::Accept;
            }
            self.total_rejections += 1;
            return DurabilityPolicyDecision::Reject;
        }

        // Check min ack level.
        if level.rank() < policy.min_ack_level.rank() {
            self.total_require_more += 1;
            return DurabilityPolicyDecision::RequireMoreReplicas;
        }

        // Check min durable replicas.
        if durable_replicas < policy.min_durable_replicas {
            self.total_require_more += 1;
            return DurabilityPolicyDecision::RequireMoreReplicas;
        }

        // Check final requirement for commit.
        if policy.require_final_for_commit && !is_final {
            self.total_require_more += 1;
            return DurabilityPolicyDecision::RequireMoreReplicas;
        }

        DurabilityPolicyDecision::Accept
    }

    /// Return total decisions.
    pub fn total_decisions(&self) -> u64 {
        self.total_decisions
    }

    /// Return total rejections.
    pub fn total_rejections(&self) -> u64 {
        self.total_rejections
    }

    /// Return total require-more decisions.
    pub fn total_require_more(&self) -> u64 {
        self.total_require_more
    }

    /// Return the default policy.
    pub fn default_policy(&self) -> &DurabilityPolicy {
        &self.default_policy
    }

    /// Return the policy for a dataset.
    pub fn policy_for(&self, dataset_id: DatasetId) -> Option<&DurabilityPolicy> {
        self.policies.get(&dataset_id)
    }
}

// ========== AckReplicaCoverage — tracks which replicas have acked
///////////////////////////////////////////////////////////////////////////////

/// A coverage record for an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageRecord {
    pub operation_id: OperationId,
    pub acked_replicas: BTreeSet<NodeId>,
    pub total_replicas: u32,
    pub last_updated: TimestampMillis,
}

/// Tracks which replicas have acknowledged each operation.
pub struct AckReplicaCoverage {
    /// Coverage per operation.
    coverage: HashMap<OperationId, CoverageRecord>,
    /// Total coverage updates.
    total_updates: u64,
    /// Total fully covered operations.
    total_fully_covered: u64,
}

impl AckReplicaCoverage {
    /// Create a new tracker.
    pub fn new() -> Self {
        AckReplicaCoverage {
            coverage: HashMap::new(),
            total_updates: 0,
            total_fully_covered: 0,
        }
    }

    /// Record that a replica has acked an operation.
    pub fn record_ack(
        &mut self,
        operation_id: OperationId,
        replica: NodeId,
        total_replicas: u32,
    ) -> bool {
        self.total_updates += 1;
        let entry = self.coverage.entry(operation_id).or_insert(CoverageRecord {
            operation_id,
            acked_replicas: BTreeSet::new(),
            total_replicas,
            last_updated: TimestampMillis(0),
        });
        entry.acked_replicas.insert(replica);
        entry.total_replicas = total_replicas;
        entry.last_updated = TimestampMillis(0);

        let fully_covered = entry.acked_replicas.len() as u32 >= total_replicas;
        if fully_covered && entry.acked_replicas.len() as u32 == total_replicas {
            // Only count once when reaching full coverage.
            // (Simplified: count each time it's fully covered.)
            self.total_fully_covered += 1;
        }
        fully_covered
    }

    /// Return the coverage record for an operation.
    pub fn coverage_for(&self, operation_id: OperationId) -> Option<&CoverageRecord> {
        self.coverage.get(&operation_id)
    }

    /// Return whether an operation is fully covered.
    pub fn is_fully_covered(&self, operation_id: OperationId) -> bool {
        self.coverage
            .get(&operation_id)
            .map(|c| c.acked_replicas.len() as u32 >= c.total_replicas)
            .unwrap_or(false)
    }

    /// Return the number of acked replicas for an operation.
    pub fn acked_replica_count(&self, operation_id: OperationId) -> u32 {
        self.coverage
            .get(&operation_id)
            .map(|c| c.acked_replicas.len() as u32)
            .unwrap_or(0)
    }

    /// Return the missing replicas for an operation.
    pub fn missing_replicas(
        &self,
        operation_id: OperationId,
        all_replicas: &BTreeSet<NodeId>,
    ) -> Vec<NodeId> {
        match self.coverage.get(&operation_id) {
            Some(c) => all_replicas
                .difference(&c.acked_replicas)
                .copied()
                .collect(),
            None => all_replicas.iter().copied().collect(),
        }
    }

    /// Return total updates.
    pub fn total_updates(&self) -> u64 {
        self.total_updates
    }

    /// Return total fully covered operations.
    pub fn total_fully_covered(&self) -> u64 {
        self.total_fully_covered
    }
}

// ========== AckDurabilityAuditor — audits durability compliance
///////////////////////////////////////////////////////////////////////////////

/// An audit result for durability compliance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityAuditResult {
    pub dataset_id: DatasetId,
    pub total_operations: u64,
    pub compliant_operations: u64,
    pub non_compliant_operations: u64,
    pub compliance_rate: f64,
    pub violations: Vec<DurabilityViolation>,
}

/// A durability violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilityViolation {
    pub operation_id: OperationId,
    pub violation_type: DurabilityViolationType,
    pub claimed_level: AckLevel,
    pub actual_level: AckLevel,
}

/// Type of durability violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityViolationType {
    OverstatedAck,
    BelowThreshold,
    MissingFinalAck,
    UnknownLevel,
}

/// Audits durability compliance across operations.
pub struct AckDurabilityAuditor {
    /// Audit results per dataset.
    results: HashMap<DatasetId, DurabilityAuditResult>,
    /// Total audits performed.
    total_audits: u64,
    /// Total violations found.
    total_violations: u64,
}

impl AckDurabilityAuditor {
    /// Create a new auditor.
    pub fn new() -> Self {
        AckDurabilityAuditor {
            results: HashMap::new(),
            total_audits: 0,
            total_violations: 0,
        }
    }

    /// Audit durability compliance for a dataset.
    pub fn audit_dataset(
        &mut self,
        dataset_id: DatasetId,
        operations: &[(OperationId, AckLevel, u32, u32, bool)],
        threshold: u32,
    ) -> DurabilityAuditResult {
        self.total_audits += 1;
        let total = operations.len() as u64;
        let mut compliant = 0u64;
        let mut non_compliant = 0u64;
        let mut violations: Vec<DurabilityViolation> = Vec::new();

        for (op_id, claimed_level, durable, total_replicas, is_final) in operations {
            let classifier = AckLevelClassifier::new();
            let actual_level = classifier.classify(*durable, *total_replicas, threshold);

            let mut is_compliant = true;

            // Check overstatement.
            if claimed_level.rank() > actual_level.rank() {
                is_compliant = false;
                violations.push(DurabilityViolation {
                    operation_id: *op_id,
                    violation_type: DurabilityViolationType::OverstatedAck,
                    claimed_level: *claimed_level,
                    actual_level,
                });
            }

            // Check threshold.
            if *durable < threshold {
                is_compliant = false;
                violations.push(DurabilityViolation {
                    operation_id: *op_id,
                    violation_type: DurabilityViolationType::BelowThreshold,
                    claimed_level: *claimed_level,
                    actual_level,
                });
            }

            // Check unknown level.
            if *claimed_level == AckLevel::Unknown {
                is_compliant = false;
                violations.push(DurabilityViolation {
                    operation_id: *op_id,
                    violation_type: DurabilityViolationType::UnknownLevel,
                    claimed_level: *claimed_level,
                    actual_level,
                });
            }

            if is_compliant {
                compliant += 1;
            } else {
                non_compliant += 1;
            }
        }

        self.total_violations += violations.len() as u64;

        let compliance_rate = if total > 0 {
            compliant as f64 / total as f64
        } else {
            1.0
        };

        let result = DurabilityAuditResult {
            dataset_id,
            total_operations: total,
            compliant_operations: compliant,
            non_compliant_operations: non_compliant,
            compliance_rate,
            violations,
        };

        self.results.insert(dataset_id, result.clone());
        result
    }

    /// Return whether a dataset is compliant.
    pub fn is_compliant(&self, dataset_id: DatasetId) -> bool {
        self.results
            .get(&dataset_id)
            .map(|r| r.compliance_rate >= 0.99)
            .unwrap_or(true)
    }

    /// Return the audit result for a dataset.
    pub fn result_for(&self, dataset_id: DatasetId) -> Option<&DurabilityAuditResult> {
        self.results.get(&dataset_id)
    }

    /// Return total audits.
    pub fn total_audits(&self) -> u64 {
        self.total_audits
    }

    /// Return total violations.
    pub fn total_violations(&self) -> u64 {
        self.total_violations
    }
}

// ========== AckDurabilityComplianceMonitor — monitors compliance trends
///////////////////////////////////////////////////////////////////////////////

/// A compliance snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceSnapshot {
    pub dataset_id: DatasetId,
    pub timestamp: TimestampMillis,
    pub compliance_rate: f64,
    pub total_operations: u64,
    pub compliant_operations: u64,
}

/// Monitors durability compliance trends over time.
pub struct AckDurabilityComplianceMonitor {
    /// Snapshots per dataset.
    snapshots: HashMap<DatasetId, Vec<ComplianceSnapshot>>,
    /// Total snapshots taken.
    total_snapshots: u64,
    /// Total compliance declines detected.
    total_declines: u64,
    /// Total improvements detected.
    total_improvements: u64,
}

impl AckDurabilityComplianceMonitor {
    /// Create a new monitor.
    pub fn new() -> Self {
        AckDurabilityComplianceMonitor {
            snapshots: HashMap::new(),
            total_snapshots: 0,
            total_declines: 0,
            total_improvements: 0,
        }
    }

    /// Record a compliance snapshot.
    pub fn record_snapshot(
        &mut self,
        dataset_id: DatasetId,
        compliance_rate: f64,
        total_operations: u64,
        compliant_operations: u64,
    ) {
        self.total_snapshots += 1;
        let snapshot = ComplianceSnapshot {
            dataset_id,
            timestamp: TimestampMillis(0),
            compliance_rate,
            total_operations,
            compliant_operations,
        };

        // Detect trend vs previous snapshot.
        if let Some(prev) = self
            .snapshots
            .get(&dataset_id)
            .and_then(|v| v.last())
        {
            if compliance_rate < prev.compliance_rate {
                self.total_declines += 1;
            } else if compliance_rate > prev.compliance_rate {
                self.total_improvements += 1;
            }
        }

        self.snapshots
            .entry(dataset_id)
            .or_default()
            .push(snapshot);
    }

    /// Return the latest snapshot for a dataset.
    pub fn latest_snapshot(&self, dataset_id: DatasetId) -> Option<&ComplianceSnapshot> {
        self.snapshots
            .get(&dataset_id)
            .and_then(|v| v.last())
    }

    /// Return all snapshots for a dataset.
    pub fn snapshots_for_dataset(&self, dataset_id: DatasetId) -> Vec<ComplianceSnapshot> {
        self.snapshots
            .get(&dataset_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Return total snapshots.
    pub fn total_snapshots(&self) -> u64 {
        self.total_snapshots
    }

    /// Return total compliance declines.
    pub fn total_declines(&self) -> u64 {
        self.total_declines
    }

    /// Return total improvements.
    pub fn total_improvements(&self) -> u64 {
        self.total_improvements
    }

    /// Return whether a dataset's compliance is trending down.
    pub fn is_trending_down(&self, dataset_id: DatasetId) -> bool {
        let snaps = match self.snapshots.get(&dataset_id) {
            Some(v) => v,
            None => return false,
        };
        if snaps.len() < 2 {
            return false;
        }
        let last = snaps.last().unwrap();
        let prev = snaps[snaps.len() - 2];
        last.compliance_rate < prev.compliance_rate
    }

    /// Return the average compliance rate across all snapshots for a dataset.
    pub fn average_compliance_rate(&self, dataset_id: DatasetId) -> f64 {
        let snaps = match self.snapshots.get(&dataset_id) {
            Some(v) => v,
            None => return 0.0,
        };
        if snaps.is_empty() {
            return 0.0;
        }
        let sum: f64 = snaps.iter().map(|s| s.compliance_rate).sum();
        sum / snaps.len() as f64
    }

    /// Return the total number of snapshots across all datasets.
    pub fn total_snapshot_count_all(&self) -> u64 {
        self.snapshots.values().map(|v| v.len() as u64).sum()
    }

    /// Return whether any dataset is trending down.
    pub fn any_trending_down(&self) -> bool {
        self.snapshots.keys().any(|&ds| self.is_trending_down(ds))
    }

    /// Return the dataset with the lowest compliance rate, if any.
    pub fn worst_dataset(&self) -> Option<(DatasetId, f64)> {
        let mut worst: Option<(DatasetId, f64)> = None;
        for (ds, snaps) in &self.snapshots {
            if let Some(last) = snaps.last() {
                let rate = last.compliance_rate;
                match worst {
                    Some((_, best_rate)) if rate < best_rate => worst = Some((*ds, rate)),
                    None => worst = Some((*ds, rate)),
                    _ => {}
                }
            }
        }
        worst
    }

    /// Return the total number of datasets tracked.
    pub fn tracked_dataset_count(&self) -> u64 {
        self.snapshots.len() as u64
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// Version bit position for the durability ack invariant in EdgeCaseBits.
pub const DURABILITY_ACK_BIT: u128 = 1 << 14;

/// Check whether the durability ack invariant is enabled in a bitset.
pub fn durability_ack_enabled(bits: &crate::versioning::EdgeCaseBits) -> bool {
    bits.has(DURABILITY_ACK_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// Tests
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod durability_ack_tests {
    use super::*;

    // ---- AckLevel tests ----

    #[test]
    fn ack_level_variants() {
        let levels = [
            AckLevel::None,
            AckLevel::Primary,
            AckLevel::Quorum,
            AckLevel::All,
            AckLevel::Unknown,
        ];
        for l in levels {
            let _ = format!("{:?}", l);
        }
    }

    #[test]
    fn ack_level_is_durable() {
        assert!(!AckLevel::None.is_durable());
        assert!(!AckLevel::Unknown.is_durable());
        assert!(AckLevel::Primary.is_durable());
        assert!(AckLevel::Quorum.is_durable());
        assert!(AckLevel::All.is_durable());
    }

    #[test]
    fn ack_level_is_full() {
        assert!(!AckLevel::None.is_full());
        assert!(!AckLevel::Primary.is_full());
        assert!(!AckLevel::Quorum.is_full());
        assert!(AckLevel::All.is_full());
    }

    #[test]
    fn ack_level_is_quorum() {
        assert!(!AckLevel::None.is_quorum());
        assert!(!AckLevel::Primary.is_quorum());
        assert!(AckLevel::Quorum.is_quorum());
        assert!(AckLevel::All.is_quorum());
    }

    #[test]
    fn ack_level_rank() {
        assert_eq!(AckLevel::None.rank(), 0);
        assert_eq!(AckLevel::Primary.rank(), 1);
        assert_eq!(AckLevel::Quorum.rank(), 2);
        assert_eq!(AckLevel::All.rank(), 3);
        assert_eq!(AckLevel::Unknown.rank(), 0);
    }

    #[test]
    fn ack_level_meets_or_exceeds() {
        assert!(AckLevel::All.meets_or_exceeds(AckLevel::Quorum));
        assert!(AckLevel::Quorum.meets_or_exceeds(AckLevel::Primary));
        assert!(AckLevel::Primary.meets_or_exceeds(AckLevel::None));
        assert!(!AckLevel::Primary.meets_or_exceeds(AckLevel::Quorum));
        assert!(!AckLevel::None.meets_or_exceeds(AckLevel::Primary));
    }

    // ---- AckOperationId tests ----

    #[test]
    fn ack_operation_id_creation() {
        let id = AckOperationId::new(42);
        assert_eq!(id.value(), 42);
    }

    #[test]
    fn ack_operation_id_ordering() {
        let a = AckOperationId::new(1);
        let b = AckOperationId::new(2);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- AckRecord tests ----

    #[test]
    fn ack_record_creation() {
        let record = AckRecord {
            ack_id: AckOperationId::new(1),
            operation_id: OperationId::new(10),
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            level: AckLevel::Quorum,
            durable_replicas: 3,
            total_replicas: 5,
            threshold: 3,
            is_final: true,
            sent_at: TimestampMillis::new(0),
            received_at: Some(TimestampMillis::new(100)),
            acknowledged_by: NodeId::new([3u8; 32]),
        };
        assert_eq!(record.level, AckLevel::Quorum);
        assert!(record.meets_threshold());
        assert!(!record.all_acked());
    }

    #[test]
    fn ack_record_replication_ratio() {
        let record = AckRecord {
            ack_id: AckOperationId::new(1),
            operation_id: OperationId::new(10),
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            level: AckLevel::Primary,
            durable_replicas: 2,
            total_replicas: 5,
            threshold: 1,
            is_final: false,
            sent_at: TimestampMillis::new(0),
            received_at: None,
            acknowledged_by: NodeId::new([3u8; 32]),
        };
        assert!((record.replication_ratio() - 0.4).abs() < 0.001);
    }

    #[test]
    fn ack_record_all_acked() {
        let record = AckRecord {
            ack_id: AckOperationId::new(1),
            operation_id: OperationId::new(10),
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            level: AckLevel::All,
            durable_replicas: 5,
            total_replicas: 5,
            threshold: 3,
            is_final: true,
            sent_at: TimestampMillis::new(0),
            received_at: None,
            acknowledged_by: NodeId::new([3u8; 32]),
        };
        assert!(record.all_acked());
    }

    #[test]
    fn ack_record_zero_total_replicas() {
        let record = AckRecord {
            ack_id: AckOperationId::new(1),
            operation_id: OperationId::new(10),
            dataset_id: DatasetId::new([1u8; 32]),
            node_id: NodeId::new([2u8; 32]),
            level: AckLevel::None,
            durable_replicas: 0,
            total_replicas: 0,
            threshold: 0,
            is_final: false,
            sent_at: TimestampMillis::new(0),
            received_at: None,
            acknowledged_by: NodeId::new([3u8; 32]),
        };
        assert_eq!(record.replication_ratio(), 0.0);
    }

    // ---- AckTracker tests ----

    #[test]
    fn tracker_register_ack() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let ack_id = tracker.register_ack(
            op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node,
        );
        assert_eq!(ack_id, AckOperationId::new(0));
        assert_eq!(tracker.total_acks(), 1);
        assert_eq!(tracker.total_durable_acks(), 1);
    }

    #[test]
    fn tracker_ack_level_for_operation() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op, dataset, node, AckLevel::None, 0, 3, 1, false, node);
        tracker.register_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node);
        assert_eq!(
            tracker.ack_level_for_operation(op),
            Some(AckLevel::Primary)
        );
    }

    #[test]
    fn tracker_is_durable_for_operation() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op, dataset, node, AckLevel::None, 0, 3, 1, false, node);
        assert!(!tracker.is_durable_for_operation(op));
        tracker.register_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node);
        assert!(tracker.is_durable_for_operation(op));
    }

    #[test]
    fn tracker_durable_replica_count_for_operation() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op, dataset, node, AckLevel::Quorum, 3, 5, 3, false, node);
        assert_eq!(tracker.durable_replica_count_for_operation(op), 3);
    }

    #[test]
    fn tracker_acks_for_dataset() {
        let mut tracker = AckTracker::new();
        let op1 = OperationId::new(1);
        let op2 = OperationId::new(2);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op1, dataset, node, AckLevel::Primary, 1, 3, 1, false, node);
        tracker.register_ack(op2, dataset, node, AckLevel::Quorum, 2, 3, 2, false, node);
        let acks = tracker.acks_for_dataset(dataset);
        assert_eq!(acks.len(), 2);
        assert_eq!(tracker.ack_count_for_dataset(dataset), 2);
    }

    #[test]
    fn tracker_total_durable_acks() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op, dataset, node, AckLevel::None, 0, 3, 1, false, node);
        tracker.register_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node);
        tracker.register_ack(op, dataset, node, AckLevel::All, 3, 3, 3, true, node);
        assert_eq!(tracker.total_durable_acks(), 2);
        assert_eq!(tracker.total_final_acks(), 1);
    }

    #[test]
    fn tracker_durable_ack_rate() {
        let mut tracker = AckTracker::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        tracker.register_ack(op, dataset, node, AckLevel::None, 0, 3, 1, false, node);
        tracker.register_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node);
        assert!((tracker.durable_ack_rate() - 0.5).abs() < 0.001);
    }

    // ---- DurabilityThresholdCalculation tests ----

    #[test]
    fn threshold_calculation_creation() {
        let calc = DurabilityThresholdCalculation {
            dataset_id: DatasetId::new([1u8; 32]),
            replica_count: 5,
            threshold: 3,
            ack_level_required: AckLevel::Quorum,
            fault_tolerance: 2,
        };
        assert_eq!(calc.replica_count, 5);
        assert_eq!(calc.threshold, 3);
    }

    // ---- DurabilityThresholdCalculator tests ----

    #[test]
    fn calculator_calculate() {
        let calc = DurabilityThresholdCalculator::new();
        let result = calc.calculate(DatasetId::new([1u8; 32]), 5, 2);
        assert_eq!(result.replica_count, 5);
        assert_eq!(result.threshold, 3);
        assert_eq!(result.fault_tolerance, 2);
        assert_eq!(result.ack_level_required, AckLevel::Quorum);
    }

    #[test]
    fn calculator_quorum_size() {
        assert_eq!(DurabilityThresholdCalculator::quorum_size(5), 3);
        assert_eq!(DurabilityThresholdCalculator::quorum_size(4), 3);
        assert_eq!(DurabilityThresholdCalculator::quorum_size(1), 1);
    }

    #[test]
    fn calculator_valid_threshold() {
        assert!(DurabilityThresholdCalculator::is_valid_threshold(5, 3));
        assert!(!DurabilityThresholdCalculator::is_valid_threshold(5, 6));
        assert!(!DurabilityThresholdCalculator::is_valid_threshold(5, 0));
    }

    #[test]
    fn calculator_default_fault_tolerance() {
        assert_eq!(DurabilityThresholdCalculator::default_fault_tolerance(5), 2);
        assert_eq!(DurabilityThresholdCalculator::default_fault_tolerance(1), 0);
    }

    #[test]
    fn calculator_all_replicas() {
        let calc = DurabilityThresholdCalculator::new();
        let result = calc.calculate(DatasetId::new([1u8; 32]), 3, 3);
        assert_eq!(result.threshold, 3);
        assert_eq!(result.ack_level_required, AckLevel::All);
    }

    #[test]
    fn calculator_zero_replicas() {
        let calc = DurabilityThresholdCalculator::new();
        let result = calc.calculate(DatasetId::new([1u8; 32]), 0, 0);
        assert_eq!(result.threshold, 0);
        assert_eq!(result.ack_level_required, AckLevel::None);
    }

    // ---- AckLevelClassifier tests ----

    #[test]
    fn classifier_none() {
        let classifier = AckLevelClassifier::new();
        assert_eq!(classifier.classify(0, 5, 3), AckLevel::None);
    }

    #[test]
    fn classifier_primary() {
        let classifier = AckLevelClassifier::new();
        assert_eq!(classifier.classify(1, 5, 3), AckLevel::Primary);
    }

    #[test]
    fn classifier_quorum() {
        let classifier = AckLevelClassifier::new();
        assert_eq!(classifier.classify(3, 5, 3), AckLevel::Quorum);
    }

    #[test]
    fn classifier_all() {
        let classifier = AckLevelClassifier::new();
        assert_eq!(classifier.classify(5, 5, 3), AckLevel::All);
    }

    #[test]
    fn classifier_quorum_size() {
        assert_eq!(AckLevelClassifier::quorum_size(5), 3);
        assert_eq!(AckLevelClassifier::quorum_size(6), 4);
    }

    #[test]
    fn classifier_satisfies_threshold() {
        assert!(AckLevelClassifier::satisfies_threshold(AckLevel::Quorum, 3, 3));
        assert!(!AckLevelClassifier::satisfies_threshold(AckLevel::Primary, 1, 3));
    }

    // ---- FinalityRecord tests ----

    #[test]
    fn finality_record_creation() {
        let record = FinalityRecord {
            operation_id: OperationId::new(1),
            is_final: true,
            final_ack_id: Some(AckOperationId::new(5)),
            finalized_at: Some(TimestampMillis::new(0)),
            durable_replicas_at_final: 3,
        };
        assert!(record.is_final);
        assert_eq!(record.durable_replicas_at_final, 3);
    }

    // ---- AckFinalityTracker tests ----

    #[test]
    fn finality_tracker_record_and_check() {
        let mut tracker = AckFinalityTracker::new();
        let op = OperationId::new(1);
        tracker.record_finality(op, false, None, 0);
        assert!(!tracker.is_finalized(op));
        tracker.record_finality(op, true, Some(AckOperationId::new(1)), 3);
        assert!(tracker.is_finalized(op));
        assert_eq!(tracker.total_finalized(), 1);
        assert_eq!(tracker.total_non_final(), 1);
    }

    #[test]
    fn finality_tracker_record_for() {
        let mut tracker = AckFinalityTracker::new();
        let op = OperationId::new(1);
        tracker.record_finality(op, true, Some(AckOperationId::new(1)), 3);
        let record = tracker.record_for(op).unwrap();
        assert!(record.is_final);
    }

    #[test]
    fn finality_tracker_no_record() {
        let tracker = AckFinalityTracker::new();
        assert!(!tracker.is_finalized(OperationId::new(99)));
        assert!(tracker.record_for(OperationId::new(99)).is_none());
    }

    // ---- OverstatementResult tests ----

    #[test]
    fn overstatement_result_creation() {
        let result = OverstatementResult {
            operation_id: OperationId::new(1),
            overstated: true,
            claimed_level: AckLevel::All,
            actual_level: AckLevel::Primary,
            durable_replicas: 1,
            threshold: 3,
            severity: OverstatementSeverity::High,
        };
        assert!(result.overstated);
        assert_eq!(result.severity, OverstatementSeverity::High);
    }

    // ---- OverstatementSeverity tests ----

    #[test]
    fn overstatement_severity_ordering() {
        assert!(OverstatementSeverity::Critical > OverstatementSeverity::High);
        assert!(OverstatementSeverity::High > OverstatementSeverity::Medium);
        assert!(OverstatementSeverity::Medium > OverstatementSeverity::Low);
        assert!(OverstatementSeverity::Low > OverstatementSeverity::None);
    }

    // ---- AckOverstatementDetector tests ----

    #[test]
    fn detector_no_overstatement() {
        let mut detector = AckOverstatementDetector::new();
        let result = detector.detect(OperationId::new(1), AckLevel::Primary, 1, 3, 1);
        assert!(!result.overstated);
        assert_eq!(result.severity, OverstatementSeverity::None);
    }

    #[test]
    fn detector_overstatement() {
        let mut detector = AckOverstatementDetector::new();
        let result = detector.detect(OperationId::new(1), AckLevel::All, 1, 5, 3);
        assert!(result.overstated);
        assert_eq!(result.claimed_level, AckLevel::All);
        assert_eq!(result.actual_level, AckLevel::Primary);
    }

    #[test]
    fn detector_severity_levels() {
        let mut detector = AckOverstatementDetector::new();
        let r1 = detector.detect(OperationId::new(1), AckLevel::All, 1, 5, 3);
        assert_eq!(r1.severity, OverstatementSeverity::Medium);
        let r2 = detector.detect(OperationId::new(2), AckLevel::Quorum, 0, 5, 3);
        assert_eq!(r2.severity, OverstatementSeverity::Medium);
    }

    #[test]
    fn detector_total_overstatements() {
        let mut detector = AckOverstatementDetector::new();
        detector.detect(OperationId::new(1), AckLevel::All, 1, 5, 3);
        detector.detect(OperationId::new(2), AckLevel::Primary, 1, 5, 3);
        assert_eq!(detector.total_detections(), 2);
        assert_eq!(detector.total_overstatements(), 1);
    }

    #[test]
    fn detector_overstatement_rate() {
        let mut detector = AckOverstatementDetector::new();
        detector.detect(OperationId::new(1), AckLevel::All, 1, 5, 3);
        detector.detect(OperationId::new(2), AckLevel::Primary, 1, 5, 3);
        assert!((detector.overstatement_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn detector_is_overstated() {
        let mut detector = AckOverstatementDetector::new();
        detector.detect(OperationId::new(1), AckLevel::All, 1, 5, 3);
        assert!(detector.is_overstated(OperationId::new(1)));
        assert!(!detector.is_overstated(OperationId::new(2)));
    }

    // ---- DurabilityAckConfig tests ----

    #[test]
    fn durability_ack_config_default() {
        let config = DurabilityAckConfig::default();
        assert_eq!(config.default_threshold, 1);
        assert_eq!(config.default_fault_tolerance, 0);
        assert!(!config.require_quorum_for_write);
        assert!(config.reject_overstated_acks);
    }

    // ---- DurabilityAckManager tests ----

    #[test]
    fn manager_process_ack() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let ack_id = manager.process_ack(
            op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node,
        ).unwrap();
        assert_eq!(ack_id, AckOperationId::new(0));
        assert_eq!(manager.total_acks_processed(), 1);
        assert_eq!(manager.total_acks_rejected(), 0);
    }

    #[test]
    fn manager_reject_overstated_ack() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let result = manager.process_ack(
            op, dataset, node, AckLevel::All, 1, 5, 3, false, node,
        );
        assert!(result.is_err());
        assert_eq!(manager.total_acks_rejected(), 1);
        assert_eq!(manager.total_acks_processed(), 1);
    }

    #[test]
    fn manager_ack_level_and_durable() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        let ack_id = manager.process_ack(
            op, dataset, node, AckLevel::None, 0, 3, 1, false, node,
        ).unwrap();
        assert_eq!(manager.ack_level(ack_id), AckLevel::None);
        assert!(!manager.is_durable(ack_id));

        let ack_id2 = manager.process_ack(
            op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node,
        ).unwrap();
        assert_eq!(manager.ack_level(ack_id2), AckLevel::Primary);
        assert!(manager.is_durable(ack_id2));
    }

    #[test]
    fn manager_durable_replica_count() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(
            op, dataset, node, AckLevel::Quorum, 3, 5, 3, false, node,
        ).unwrap();
        assert_eq!(manager.durable_replica_count(AckOperationId::new(0)), 3);
    }

    #[test]
    fn manager_meets_threshold() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(
            op, dataset, node, AckLevel::Quorum, 3, 5, 3, false, node,
        ).unwrap();
        assert!(manager.meets_threshold(AckOperationId::new(0), dataset));
    }

    #[test]
    fn manager_ack_count_for_dataset() {
        let mut manager = DurabilityAckManager::new();
        let op1 = OperationId::new(1);
        let op2 = OperationId::new(2);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(op1, dataset, node, AckLevel::Primary, 1, 3, 1, false, node).unwrap();
        manager.process_ack(op2, dataset, node, AckLevel::Quorum, 2, 3, 2, false, node).unwrap();
        assert_eq!(manager.ack_count_for_dataset(dataset), 2);
    }

    #[test]
    fn manager_is_finalized() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(
            op, dataset, node, AckLevel::All, 3, 3, 3, true, node,
        ).unwrap();
        assert!(manager.is_finalized(op));
    }

    #[test]
    fn manager_is_overstated() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(
            op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node,
        ).unwrap();
        assert!(!manager.is_overstated(op));
    }

    #[test]
    fn manager_overstatement_rate() {
        let mut manager = DurabilityAckManager::new();
        let op1 = OperationId::new(1);
        let op2 = OperationId::new(2);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(op1, dataset, node, AckLevel::All, 1, 5, 3, false, node).unwrap_err();
        manager.process_ack(op2, dataset, node, AckLevel::Primary, 1, 3, 1, false, node).unwrap();
        assert!((manager.overstatement_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn manager_durable_ack_rate() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(op, dataset, node, AckLevel::None, 0, 3, 1, false, node).unwrap();
        manager.process_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node).unwrap();
        assert!((manager.durable_ack_rate() - 0.5).abs() < 0.001);
    }

    // ---- AckStats tests ----

    #[test]
    fn ack_stats_creation() {
        let stats = AckStats {
            total_acks: 100,
            durable_acks: 80,
            final_acks: 50,
            overstatements: 2,
            rejections: 2,
            durable_rate: 0.8,
            overstatement_rate: 0.02,
            rejection_rate: 0.02,
        };
        assert_eq!(stats.total_acks, 100);
        assert_eq!(stats.durable_acks, 80);
    }

    // ---- AckStatsAggregator tests ----

    #[test]
    fn aggregator_aggregate() {
        let mut manager = DurabilityAckManager::new();
        let op = OperationId::new(1);
        let dataset = DatasetId::new([1u8; 32]);
        let node = NodeId::new([2u8; 32]);
        manager.process_ack(op, dataset, node, AckLevel::Primary, 1, 3, 1, false, node).unwrap();
        let aggregator = AckStatsAggregator::new();
        let stats = aggregator.aggregate(&manager);
        assert_eq!(stats.total_acks, 1);
        assert_eq!(stats.durable_acks, 1);
        assert!((stats.durable_rate - 1.0).abs() < 0.001);
    }

    #[test]
    fn aggregator_is_healthy() {
        let healthy = AckStats {
            total_acks: 1000,
            durable_acks: 990,
            final_acks: 950,
            overstatements: 5,
            rejections: 10,
            durable_rate: 0.99,
            overstatement_rate: 0.005,
            rejection_rate: 0.01,
        };
        assert!(AckStatsAggregator::is_healthy(&healthy));

        let unhealthy = AckStats {
            total_acks: 1000,
            durable_acks: 500,
            final_acks: 100,
            overstatements: 100,
            rejections: 200,
            durable_rate: 0.5,
            overstatement_rate: 0.1,
            rejection_rate: 0.2,
        };
        assert!(!AckStatsAggregator::is_healthy(&unhealthy));
    }

    // ---- AckBatcher tests ----

    #[test]
    fn batcher_add_and_create() {
        let mut batcher = AckBatcher::new();
        batcher.add_ack(AckOperationId::new(1));
        batcher.add_ack(AckOperationId::new(2));
        batcher.add_ack(AckOperationId::new(3));
        let dataset = DatasetId::new([1u8; 32]);
        let batch = batcher.create_batch(dataset).unwrap();
        assert_eq!(batch.acks.len(), 3);
        assert_eq!(batcher.total_batches(), 1);
        assert_eq!(batcher.total_acks_batched(), 3);
    }

    #[test]
    fn batcher_empty() {
        let mut batcher = AckBatcher::new();
        let dataset = DatasetId::new([1u8; 32]);
        assert!(batcher.create_batch(dataset).is_none());
    }

    #[test]
    fn batcher_max_size() {
        let mut batcher = AckBatcher::new();
        batcher.set_max_batch_size(2);
        batcher.add_ack(AckOperationId::new(1));
        batcher.add_ack(AckOperationId::new(2));
        batcher.add_ack(AckOperationId::new(3));
        let dataset = DatasetId::new([1u8; 32]);
        let batch = batcher.create_batch(dataset).unwrap();
        assert_eq!(batch.acks.len(), 2);
        assert_eq!(batcher.pending_count(), 1);
    }

    #[test]
    fn batcher_mark_sent() {
        let mut batcher = AckBatcher::new();
        batcher.add_ack(AckOperationId::new(1));
        let dataset = DatasetId::new([1u8; 32]);
        let batch = batcher.create_batch(dataset).unwrap();
        assert!(batcher.mark_batch_sent(batch.batch_id));
    }

    #[test]
    fn batcher_should_batch() {
        let mut batcher = AckBatcher::new();
        batcher.set_max_batch_size(3);
        batcher.add_ack(AckOperationId::new(1));
        batcher.add_ack(AckOperationId::new(2));
        assert!(!batcher.should_batch());
        batcher.add_ack(AckOperationId::new(3));
        assert!(batcher.should_batch());
    }

    // ---- AckLatencyTracker tests ----

    #[test]
    fn latency_tracker_send_and_receive() {
        let mut tracker = AckLatencyTracker::new();
        let ack_id = AckOperationId::new(1);
        tracker.record_sent(ack_id, TimestampMillis::new(0));
        let latency = tracker.record_received(ack_id, TimestampMillis::new(100));
        assert_eq!(latency, Some(100));
    }

    #[test]
    fn latency_tracker_average() {
        let mut tracker = AckLatencyTracker::new();
        tracker.record_sent(AckOperationId::new(1), TimestampMillis::new(0));
        tracker.record_received(AckOperationId::new(1), TimestampMillis::new(100));
        tracker.record_sent(AckOperationId::new(2), TimestampMillis::new(0));
        tracker.record_received(AckOperationId::new(2), TimestampMillis::new(200));
        assert!((tracker.average_latency_millis() - 150.0).abs() < 0.001);
    }

    #[test]
    fn latency_tracker_empty() {
        let tracker = AckLatencyTracker::new();
        assert_eq!(tracker.total_latencies(), 0);
        assert_eq!(tracker.total_completed(), 0);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(tracker.average_latency_millis(), 0.0);
    }

    // ---- AckRetryManager tests ----

    #[test]
    fn retry_manager_schedule_and_complete() {
        let mut manager = AckRetryManager::new();
        let op = OperationId::new(1);
        let id = manager.schedule_retry(op, TimestampMillis::new(1000));
        assert_eq!(id, RetryAttemptId::new(0));
        assert_eq!(manager.total_attempts(), 1);
        assert!(manager.complete_retry(id));
        assert_eq!(manager.total_completed(), 1);
    }

    #[test]
    fn retry_manager_due_retries() {
        let mut manager = AckRetryManager::new();
        let op1 = OperationId::new(1);
        let op2 = OperationId::new(2);
        manager.schedule_retry(op1, TimestampMillis::new(100));
        manager.schedule_retry(op2, TimestampMillis::new(200));
        let due = manager.due_retries(TimestampMillis::new(150));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0], op1);
    }

    #[test]
    fn retry_manager_fail_retry() {
        let mut manager = AckRetryManager::new();
        let op = OperationId::new(1);
        let id = manager.schedule_retry(op, TimestampMillis::new(100));
        assert!(manager.fail_retry(id));
        assert_eq!(manager.total_failed(), 1);
    }

    #[test]
    fn retry_manager_attempts_for_operation() {
        let mut manager = AckRetryManager::new();
        let op = OperationId::new(1);
        manager.schedule_retry(op, TimestampMillis::new(100));
        manager.schedule_retry(op, TimestampMillis::new(200));
        let attempts = manager.attempts_for_operation(op);
        assert_eq!(attempts.len(), 2);
    }

    // ---- AckConvergenceChecker tests ----

    #[test]
    fn convergence_checker_converged() {
        let mut checker = AckConvergenceChecker::new();
        let op = OperationId::new(1);
        let mut acked = BTreeSet::new();
        acked.insert(NodeId::new([1u8; 32]));
        acked.insert(NodeId::new([2u8; 32]));
        let mut all = BTreeSet::new();
        all.insert(NodeId::new([1u8; 32]));
        all.insert(NodeId::new([2u8; 32]));
        let result = checker.check_convergence(op, acked, 2, all);
        assert!(result.converged);
        assert_eq!(result.replicas_acked, 2);
    }

    #[test]
    fn convergence_checker_not_converged() {
        let mut checker = AckConvergenceChecker::new();
        let op = OperationId::new(1);
        let mut acked = BTreeSet::new();
        acked.insert(NodeId::new([1u8; 32]));
        let mut all = BTreeSet::new();
        all.insert(NodeId::new([1u8; 32]));
        all.insert(NodeId::new([2u8; 32]));
        let result = checker.check_convergence(op, acked, 2, all);
        assert!(!result.converged);
        assert_eq!(result.missing_replicas.len(), 1);
    }

    #[test]
    fn convergence_checker_convergence_rate() {
        let mut checker = AckConvergenceChecker::new();
        let op1 = OperationId::new(1);
        let op2 = OperationId::new(2);
        let mut acked = BTreeSet::new();
        acked.insert(NodeId::new([1u8; 32]));
        let mut all = BTreeSet::new();
        all.insert(NodeId::new([1u8; 32]));
        checker.check_convergence(op1, acked.clone(), 1, all.clone());
        checker.check_convergence(op2, acked, 2, all);
        assert!((checker.convergence_rate() - 0.5).abs() < 0.001);
    }

    // ---- AckThrottleController tests ----

    #[test]
    fn throttle_controller_allow() {
        let mut controller = AckThrottleController::new();
        controller.update(100, 10);
        assert_eq!(controller.decide(), AckThrottleDecision::Allow);
        assert_eq!(controller.total_allowed(), 1);
    }

    #[test]
    fn throttle_controller_throttle() {
        let mut controller = AckThrottleController::new();
        controller.update(10_000, 10);
        assert_eq!(controller.decide(), AckThrottleDecision::Throttle);
        assert_eq!(controller.total_throttled(), 1);
    }

    #[test]
    fn throttle_controller_reject() {
        let mut controller = AckThrottleController::new();
        controller.update(100, 100_000);
        assert_eq!(controller.decide(), AckThrottleDecision::Reject);
        assert_eq!(controller.total_rejected(), 1);
    }

    #[test]
    fn throttle_controller_should_throttle() {
        let mut controller = AckThrottleController::new();
        assert!(!controller.should_throttle());
        controller.update(10_000, 10);
        assert!(controller.should_throttle());
    }

    // ---- Sealed trait unit tests ----

    #[test]
    fn sealed_unit_impl() {
        let unit = ();
        assert_eq!(unit.ack_level(AckOperationId::new(1)), AckLevel::None);
        assert!(!unit.is_durable(AckOperationId::new(1)));
        assert_eq!(unit.durable_replica_count(AckOperationId::new(1)), 0);
        assert_eq!(unit.durability_threshold(DatasetId::new([1u8; 32])), 1);
        assert!(!unit.meets_threshold(AckOperationId::new(1), DatasetId::new([1u8; 32])));
        assert_eq!(unit.ack_count_for_dataset(DatasetId::new([1u8; 32])), 0);
    }

    // ---- DURABILITY_ACK_BIT tests ----

    #[test]
    fn durability_ack_bit_position() {
        assert_eq!(DURABILITY_ACK_BIT, 1 << 14);
    }

    #[test]
    fn durability_ack_enabled_checks_bit() {
        use crate::versioning::EdgeCaseBits;
        let mut bits = EdgeCaseBits(0);
        assert!(!durability_ack_enabled(&bits));
        bits.enable(DURABILITY_ACK_BIT);
        assert!(durability_ack_enabled(&bits));
    }

    // ---- Integration test: full workflow ==========

    #[test]
    fn full_durability_ack_workflow() {
        let mut manager = DurabilityAckManager::new();
        let dataset = DatasetId::new([1u8; 32]);
        let node1 = NodeId::new([2u8; 32]);
        let node2 = NodeId::new([3u8; 32]);
        let op = OperationId::new(1);

        // Primary ack.
        let ack1 = manager.process_ack(
            op, dataset, node1, AckLevel::Primary, 1, 3, 1, false, node1,
        ).unwrap();
        assert_eq!(manager.ack_level(ack1), AckLevel::Primary);
        assert!(manager.is_durable(ack1));

        // Quorum ack — meets threshold.
        let ack2 = manager.process_ack(
            op, dataset, node2, AckLevel::Quorum, 2, 3, 2, false, node2,
        ).unwrap();
        assert!(manager.meets_threshold(ack2, dataset));

        // Overstated ack — rejected.
        let overstated = manager.process_ack(
            op, dataset, node1, AckLevel::All, 1, 3, 3, false, node1,
        );
        assert!(overstated.is_err());
        assert_eq!(manager.total_acks_rejected(), 1);

        // Final ack.
        let ack3 = manager.process_ack(
            op, dataset, node2, AckLevel::All, 3, 3, 3, true, node2,
        ).unwrap();
        assert!(manager.is_finalized(op));
        assert!(manager.meets_threshold(ack3, dataset));

        // Stats.
        let aggregator = AckStatsAggregator::new();
        let stats = aggregator.aggregate(&manager);
        assert_eq!(stats.total_acks, 4);
        assert_eq!(stats.rejections, 1);
        assert!(AckStatsAggregator::is_healthy(&stats));
    }

    // ---- Threshold calculator integration ==========

    #[test]
    fn threshold_calculator_integration() {
        let calc = DurabilityThresholdCalculator::new();
        let result = calc.calculate(DatasetId::new([1u8; 32]), 7, 3);
        assert_eq!(result.threshold, 4);
        assert_eq!(result.ack_level_required, AckLevel::Quorum);
        assert_eq!(DurabilityThresholdCalculator::quorum_size(7), 4);
    }

    // ---- Overstatement detector integration ==========

    #[test]
    fn overstatement_detector_severity_progression() {
        let mut detector = AckOverstatementDetector::new();
        let r1 = detector.detect(OperationId::new(1), AckLevel::All, 0, 5, 3);
        assert_eq!(r1.severity, OverstatementSeverity::Critical);
        let r2 = detector.detect(OperationId::new(2), AckLevel::Quorum, 0, 5, 3);
        assert_eq!(r2.severity, OverstatementSeverity::Medium);
        let r3 = detector.detect(OperationId::new(3), AckLevel::Primary, 0, 5, 3);
        assert_eq!(r3.severity, OverstatementSeverity::Low);
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////