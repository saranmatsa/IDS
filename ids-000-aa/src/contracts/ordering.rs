//! # Sealed Trait: Ordering — dataset-scoped ordering domains (no global master)
//!
//! Carrier-agnostic: operates on opaque Endpoints and IDs only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Ordering trait defines dataset-scoped ordering domains, per-domain
//! sequencing, causal ordering, total order broadcast within a domain, and
//! distributed snapshot isolation. Each dataset belongs to exactly one
//! OrderingDomain; there is no global ordering master.
//! Implementations live downstream in ids-ordering and ids-* crates.

use crate::types::{
    Capacity, ConsistencyProfile, DatasetId, DurabilityCondition, Endpoint, FailureDomain,
    HUnitId, IdsTime, NodeId, OrderingDomain, OrderingPos, OrderingPosSnapshot, PartitionId,
    RecordId, RedundancyMode, RegionId,
};
use crate::errors::OrderingError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Ordering
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::OrderingEngine {}
impl private::Sealed for super::OrderingDomainMgr {}
impl private::Sealed for super::DomainSequencer {}
impl private::Sealed for super::OrderingClock {}
impl private::Sealed for super::CausalOrderTracker {}
impl private::Sealed for super::TotalOrderSequencer {}
impl private::Seated for super::SnapshotIsolationOrdering {}
impl private::Seated for super::VectorClock {}
impl private::Sealed for super::OrderingStats {}
impl private::Sealed for super::OrderingMonitor {}
impl private::Sealed for super::OrderingValidator {}

// Sealed trait for dataset-scoped ordering — no global master.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Ordering: private::Sealed {
    /// Create a new ordering domain for a dataset.
    fn create_domain(
        &self,
        dataset_id: DatasetId,
        domain_id: OrderingDomain,
    ) -> Result<OrderingDomain, OrderingError>;

    /// Get the ordering domain for a dataset.
    fn get_domain(&self, dataset_id: &DatasetId) -> Option<OrderingDomain>;

    /// Advance the ordering clock for a domain and return the new position.
    fn advance_position(
        &self,
        domain: OrderingDomain,
    ) -> Result<OrderingPos, OrderingError>;

    /// Get the current position for a domain.
    fn current_position(&self, domain: &OrderingDomain) -> OrderingPos;

    /// Check if position_a happens-before position_b in the same domain.
    fn happens_before(
        &self,
        domain: OrderingDomain,
        position_a: OrderingPos,
        position_b: OrderingPos,
    ) -> Result<bool, OrderingError>;

    /// Record a total order entry for a position.
    fn record_total_order(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        record_id: RecordId,
    ) -> Result<(), OrderingError>;

    /// Get the record at a position in total order.
    fn record_at_position(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
    ) -> Result<Option<RecordId>, OrderingError>;

    /// Create a snapshot at the current position.
    fn create_snapshot(
        &self,
        domain: OrderingDomain,
        ids_time: IdsTime,
    ) -> Result<OrderingPosSnapshot, OrderingError>;

    /// Get ordering stats.
    fn stats(&self) -> OrderingStats;

    /// Get the ordering name for debugging.
    fn ordering_name(&self) -> &'static str;

    /// Get total domain count.
    fn domain_count(&self) -> u64;

    /// Get total position advances.
    fn position_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum OrderingError {
    #[error("domain not found: {domain_id:?}")]
    DomainNotFound { domain_id: OrderingDomain },

    #[error("dataset not found: {dataset_id:?}")]
    DatasetNotFound { dataset_id: DatasetId },

    #[error("position not found: {position:?}")]
    PositionNotFound { position: OrderingPos },

    #[error("record not found: {record_id:?}")]
    RecordNotFound { record_id: RecordId },

    #[error("domain already exists: {domain_id:?}")]
    DomainAlreadyExists { domain_id: OrderingDomain },

    #[error("position out of order: expected={expected}, got={got}")]
    PositionOutOfOrder { expected: u64, got: u64 },

    #[error("causal violation: position={position}, depends_on={depends_on}")]
    CausalViolation { position: u64, depends_on: u64 },

    #[error("snapshot not found: {snapshot_id:?}")]
    SnapshotNotFound { snapshot_id: OrderingDomain },

    #[error("cross-domain ordering not allowed: domain_a={domain_a:?}, domain_b={domain_b:?}")]
    CrossDomainOrdering { domain_a: OrderingDomain, domain_b: OrderingDomain },

    #[error("invalid ordering state: {message}")]
    InvalidState { message: String },

    #[error("internal ordering error: {message}")]
    Internal { message: String },
}

impl OrderingError {
    pub fn category(&self) -> &'static str {
        match self {
            OrderingError::DomainNotFound { .. } => "domain_not_found",
            OrderingError::DatasetNotFound { .. } => "dataset_not_found",
            OrderingError::PositionNotFound { .. } => "position_not_found",
            OrderingError::RecordNotFound { .. } => "record_not_found",
            OrderingError::DomainAlreadyExists { .. } => "domain_already_exists",
            OrderingError::PositionOutOfOrder { .. } => "position_out_of_order",
            OrderingError::CausalViolation { .. } => "causal_violation",
            OrderingError::SnapshotNotFound { .. } => "snapshot_not_found",
            OrderingError::CrossDomainOrdering { .. } => "cross_domain_ordering",
            OrderingError::InvalidState { .. } => "invalid_state",
            OrderingError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            OrderingError::PositionOutOfOrder { .. }
                | OrderingError::CausalViolation { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            OrderingError::DomainNotFound { .. } => "create domain first",
            OrderingError::DatasetNotFound { .. } => "create dataset first",
            OrderingError::PositionNotFound { .. } => "advance position first",
            OrderingError::RecordNotFound { .. } => "record entry first",
            OrderingError::DomainAlreadyExists { .. } => "use existing domain",
            OrderingError::PositionOutOfOrder { .. } => "use next valid position",
            OrderingError::CausalViolation { .. } => "wait for dependency to complete",
            OrderingError::SnapshotNotFound { .. } => "create snapshot first",
            OrderingError::CrossDomainOrdering { .. } => "order within domain only",
            OrderingError::InvalidState { .. } => "reset ordering state",
            OrderingError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING DOMAIN MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainLifecycleState {
    Creating,
    Active,
    Frozen,
    Retired,
}

impl DomainLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DomainLifecycleState::Creating => "creating",
            DomainLifecycleState::Active => "active",
            DomainLifecycleState::Frozen => "frozen",
            DomainLifecycleState::Retired => "retired",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, DomainLifecycleState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, DomainLifecycleState::Retired)
    }
}

pub struct OrderingDomainEntry {
    pub domain_id: OrderingDomain,
    pub dataset_id: DatasetId,
    pub state: DomainLifecycleState,
    pub created_at_ms: u64,
    pub current_position: OrderingPos,
    pub entry_count: u64,
}

impl OrderingDomainEntry {
    pub fn new(domain_id: OrderingDomain, dataset_id: DatasetId, created_at_ms: u64) -> Self {
        Self {
            domain_id,
            dataset_id,
            state: DomainLifecycleState::Creating,
            created_at_ms,
            current_position: OrderingPos::ZERO,
            entry_count: 0,
        }
    }

    pub fn activate(&mut self) {
        self.state = DomainLifecycleState::Active;
    }

    pub fn freeze(&mut self) {
        self.state = DomainLifecycleState::Frozen;
    }

    pub fn retire(&mut self) {
        self.state = DomainLifecycleState::Retired;
    }

    pub fn advance(&mut self, steps: u64) {
        self.current_position = OrderingPos::new(self.current_position.value() + steps);
        self.entry_count += 1;
    }
}

pub struct OrderingDomainMgr {
    pub domains: std::cell::RefCell<std::collections::HashMap<OrderingDomain, OrderingDomainEntry>>,
    pub dataset_to_domain: std::cell::RefCell<std::collections::HashMap<DatasetId, OrderingDomain>>,
    pub domain_count: u64,
    pub active_domain_count: u64,
    pub retired_domain_count: u64,
}

impl OrderingDomainMgr {
    pub fn new() -> Self {
        Self {
            domains: std::cell::RefCell::new(std::collections::HashMap::new()),
            dataset_to_domain: std::cell::RefCell::new(std::collections::HashMap::new()),
            domain_count: 0,
            active_domain_count: 0,
            retired_domain_count: 0,
        }
    }

    pub fn create_domain(
        &self,
        dataset_id: DatasetId,
        domain_id: OrderingDomain,
        at_ms: u64,
    ) -> Result<OrderingDomainEntry, OrderingError> {
        let mut domains = self.domains.borrow_mut();
        let mut dataset_map = self.dataset_to_domain.borrow_mut();

        if domains.contains_key(&domain_id) {
            return Err(OrderingError::DomainAlreadyExists { domain_id });
        }
        if dataset_map.contains_key(&dataset_id) {
            return Err(OrderingError::InvalidState {
                message: "dataset already has an ordering domain".to_string(),
            });
        }

        let mut entry = OrderingDomainEntry::new(domain_id, dataset_id, at_ms);
        entry.activate();
        domains.insert(domain_id, entry.clone());
        dataset_map.insert(dataset_id, domain_id);
        self.domain_count += 1;
        self.active_domain_count += 1;

        Ok(entry)
    }

    pub fn get_domain(&self, domain_id: &OrderingDomain) -> Option<OrderingDomainEntry> {
        self.domains.borrow().get(domain_id).cloned()
    }

    pub fn get_domain_for_dataset(&self, dataset_id: &DatasetId) -> Option<OrderingDomain> {
        self.dataset_to_domain.borrow().get(dataset_id).copied()
    }

    pub fn freeze_domain(&self, domain_id: &OrderingDomain) -> bool {
        if let Some(entry) = self.domains.borrow_mut().get_mut(domain_id) {
            entry.freeze();
            true
        } else {
            false
        }
    }

    pub fn retire_domain(&self, domain_id: &OrderingDomain) -> bool {
        if let Some(entry) = self.domains.borrow_mut().get_mut(domain_id) {
            entry.retire();
            self.retired_domain_count += 1;
            true
        } else {
            false
        }
    }

    pub fn domain_count(&self) -> u64 {
        self.domain_count
    }

    pub fn active_domain_count(&self) -> u64 {
        self.active_domain_count
    }

    pub fn retired_domain_count(&self) -> u64 {
        self.retired_domain_count
    }

    pub fn track_count(&self) -> usize {
        self.domains.borrow().len()
    }
}

impl Default for OrderingDomainMgr {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN SEQUENCER
///////////////////////////////////////////////////////////////////////////////

pub struct DomainSequencer {
    pub domain_positions: std::cell::RefCell<std::collections::HashMap<OrderingDomain, OrderingPos>>,
    pub advance_count: u64,
    pub total_advances: u64,
    pub skipped_advances: u64,
}

impl DomainSequencer {
    pub fn new() -> Self {
        Self {
            domain_positions: std::cell::RefCell::new(std::collections::HashMap::new()),
            advance_count: 0,
            total_advances: 0,
            skipped_advances: 0,
        }
    }

    pub fn initialize_position(&self, domain: OrderingDomain, position: OrderingPos) {
        self.domain_positions.borrow_mut().insert(domain, position);
    }

    pub fn advance(&self, domain: OrderingDomain, steps: u64) -> Result<OrderingPos, OrderingError> {
        let mut positions = self.domain_positions.borrow_mut();
        let current = positions.get(&domain).copied().unwrap_or(OrderingPos::ZERO);
        let new_pos = OrderingPos::new(current.value() + steps);
        positions.insert(domain, new_pos);
        self.advance_count += 1;
        self.total_advances += steps;
        Ok(new_pos)
    }

    pub fn advance_one(&self, domain: OrderingDomain) -> Result<OrderingPos, OrderingError> {
        self.advance(domain, 1)
    }

    pub fn get_position(&self, domain: &OrderingDomain) -> OrderingPos {
        self.domain_positions.borrow().get(domain).copied().unwrap_or(OrderingPos::ZERO)
    }

    pub fn set_position(&self, domain: OrderingDomain, position: OrderingPos) {
        self.domain_positions.borrow_mut().insert(domain, position);
    }

    pub fn is_monotonic(&self, domain: OrderingDomain, candidate: OrderingPos) -> bool {
        let current = self.get_position(&domain);
        candidate.value() > current.value()
    }

    pub fn advance_count(&self) -> u64 {
        self.advance_count
    }

    pub fn total_advances(&self) -> u64 {
        self.total_advances
    }

    pub fn tracked_domain_count(&self) -> usize {
        self.domain_positions.borrow().len()
    }
}

impl Default for DomainSequencer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING CLOCK
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockDriftPolicy {
    Reject,
    Warn,
    Allow,
}

impl ClockDriftPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClockDriftPolicy::Reject => "reject",
            ClockDriftPolicy::Warn => "warn",
            ClockDriftPolicy::Allow => "allow",
        }
    }
}

pub struct OrderingClock {
    pub domain_clocks: std::cell::RefCell<std::collections::HashMap<OrderingDomain, u64>>,
    pub tick_count: u64,
    pub drift_rejections: u64,
    pub drift_policy: ClockDriftPolicy,
    pub max_drift_ms: u64,
}

impl OrderingClock {
    pub fn new(drift_policy: ClockDriftPolicy, max_drift_ms: u64) -> Self {
        Self {
            domain_clocks: std::cell::RefCell::new(std::collections::HashMap::new()),
            tick_count: 0,
            drift_rejections: 0,
            drift_policy,
            max_drift_ms,
        }
    }

    pub fn tick(&self, domain: OrderingDomain, at_ms: u64) -> Result<u64, OrderingError> {
        let mut clocks = self.domain_clocks.borrow_mut();
        let last = clocks.get(&domain).copied().unwrap_or(0);
        if at_ms < last {
            match self.drift_policy {
                ClockDriftPolicy::Reject => {
                    self.drift_rejections += 1;
                    return Err(OrderingError::InvalidState {
                        message: format!("clock drift detected: last={}, current={}", last, at_ms),
                    });
                }
                ClockDriftPolicy::Warn => {
                    self.drift_rejections += 1;
                }
                ClockDriftPolicy::Allow => {}
            }
        }
        clocks.insert(domain, at_ms);
        self.tick_count += 1;
        Ok(at_ms)
    }

    pub fn get_time(&self, domain: &OrderingDomain) -> u64 {
        self.domain_clocks.borrow().get(domain).copied().unwrap_or(0)
    }

    pub fn set_time(&self, domain: OrderingDomain, at_ms: u64) {
        self.domain_clocks.borrow_mut().insert(domain, at_ms);
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn drift_rejection_count(&self) -> u64 {
        self.drift_rejections
    }

    pub fn tracked_clock_count(&self) -> usize {
        self.domain_clocks.borrow().len()
    }
}

impl Default for OrderingClock {
    fn default() -> Self {
        Self::new(ClockDriftPolicy::Warn, 5000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CAUSAL ORDER TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct CausalOrderTracker {
    pub domain_dependencies: std::cell::RefCell<
        std::collections::HashMap<OrderingDomain, Vec<(OrderingPos, OrderingPos)>>,
    >,
    pub dependency_count: u64,
    pub violation_count: u64,
}

impl CausalOrderTracker {
    pub fn new() -> Self {
        Self {
            domain_dependencies: std::cell::RefCell::new(std::collections::HashMap::new()),
            dependency_count: 0,
            violation_count: 0,
        }
    }

    pub fn record_dependency(
        &self,
        domain: OrderingDomain,
        cause: OrderingPos,
        effect: OrderingPos,
    ) -> Result<(), OrderingError> {
        if effect.value() <= cause.value() {
            self.violation_count += 1;
            return Err(OrderingError::CausalViolation {
                position: effect.value(),
                depends_on: cause.value(),
            });
        }
        self.domain_dependencies
            .borrow_mut()
            .entry(domain)
            .or_default()
            .push((cause, effect));
        self.dependency_count += 1;
        Ok(())
    }

    pub fn check_causal_order(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        depends_on: OrderingPos,
    ) -> Result<bool, OrderingError> {
        if position.value() <= depends_on.value() {
            self.violation_count += 1;
            return Err(OrderingError::CausalViolation {
                position: position.value(),
                depends_on: depends_on.value(),
            });
        }
        Ok(true)
    }

    pub fn get_dependencies(&self, domain: &OrderingDomain) -> Vec<(OrderingPos, OrderingPos)> {
        self.domain_dependencies
            .borrow()
            .get(domain)
            .cloned()
            .unwrap_or_default()
    }

    pub fn dependency_count(&self) -> u64 {
        self.dependency_count
    }

    pub fn violation_count(&self) -> u64 {
        self.violation_count
    }

    pub fn causal_violation_rate(&self) -> f64 {
        if self.dependency_count == 0 {
            return 0.0;
        }
        self.violation_count as f64 / self.dependency_count as f64
    }

    pub fn tracked_domain_count(&self) -> usize {
        self.domain_dependencies.borrow().len()
    }
}

impl Default for CausalOrderTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TOTAL ORDER SEQUENCER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct TotalOrderEntry {
    pub position: OrderingPos,
    pub record_id: RecordId,
    pub domain: OrderingDomain,
    pub sequenced_at_ms: u64,
    pub sequence_number: u64,
}

impl TotalOrderEntry {
    pub fn new(
        position: OrderingPos,
        record_id: RecordId,
        domain: OrderingDomain,
        sequenced_at_ms: u64,
        sequence_number: u64,
    ) -> Self {
        Self {
            position,
            record_id,
            domain,
            sequenced_at_ms,
            sequence_number,
        }
    }
}

pub struct TotalOrderSequencer {
    pub entries: std::cell::RefCell<std::collections::BTreeMap<OrderingPos, TotalOrderEntry>>,
    pub position_to_record: std::cell::RefCell<std::collections::HashMap<OrderingPos, RecordId>>,
    pub record_to_position: std::cell::RefCell<std::collections::HashMap<RecordId, OrderingPos>>,
    pub sequence_counter: u64,
    pub sequence_count: u64,
}

impl TotalOrderSequencer {
    pub fn new() -> Self {
        Self {
            entries: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            position_to_record: std::cell::RefCell::new(std::collections::HashMap::new()),
            record_to_position: std::cell::RefCell::new(std::collections::HashMap::new()),
            sequence_counter: 0,
            sequence_count: 0,
        }
    }

    pub fn sequence(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        record_id: RecordId,
        at_ms: u64,
    ) -> Result<(), OrderingError> {
        let mut entries = self.entries.borrow_mut();
        let mut pos_to_rec = self.position_to_record.borrow_mut();
        let mut rec_to_pos = self.record_to_position.borrow_mut();

        if pos_to_rec.contains_key(&position) {
            return Err(OrderingError::PositionOutOfOrder {
                expected: position.value() + 1,
                got: position.value(),
            });
        }
        if rec_to_pos.contains_key(&record_id) {
            return Err(OrderingError::InvalidState {
                message: "record already sequenced at another position".to_string(),
            });
        }

        self.sequence_counter += 1;
        let entry = TotalOrderEntry::new(position, record_id, domain, at_ms, self.sequence_counter);
        entries.insert(position, entry);
        pos_to_rec.insert(position, record_id);
        rec_to_pos.insert(record_id, position);
        self.sequence_count += 1;

        Ok(())
    }

    pub fn get_entry_at(&self, position: OrderingPos) -> Option<TotalOrderEntry> {
        self.entries.borrow().get(&position).cloned()
    }

    pub fn get_record_at(&self, position: OrderingPos) -> Option<RecordId> {
        self.position_to_record.borrow().get(&position).copied()
    }

    pub fn get_position_for_record(&self, record_id: &RecordId) -> Option<OrderingPos> {
        self.record_to_position.borrow().get(record_id).copied()
    }

    pub fn sequence_count(&self) -> u64 {
        self.sequence_count
    }

    pub fn entry_count(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn first_position(&self) -> Option<OrderingPos> {
        self.entries.borrow().keys().next().copied()
    }

    pub fn last_position(&self) -> Option<OrderingPos> {
        self.entries.borrow().keys().next_back().copied()
    }
}

impl Default for TotalOrderSequencer {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SNAPSHOT ISOLATION ORDERING
///////////////////////////////////////////////////////////////////////////////

pub struct SnapshotIsolationOrdering {
    pub snapshots: std::cell::RefCell<std::collections::HashMap<OrderingDomain, Vec<OrderingPosSnapshot>>>,
    pub snapshot_counter: u64,
    pub snapshot_count: u64,
    pub restored_count: u64,
}

impl SnapshotIsolationOrdering {
    pub fn new() -> Self {
        Self {
            snapshots: std::cell::RefCell::new(std::collections::HashMap::new()),
            snapshot_counter: 0,
            snapshot_count: 0,
            restored_count: 0,
        }
    }

    pub fn create_snapshot(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        ids_time: IdsTime,
    ) -> Result<OrderingPosSnapshot, OrderingError> {
        self.snapshot_counter += 1;
        let snapshot = OrderingPosSnapshot::new(domain, position, ids_time);
        self.snapshots.borrow_mut().entry(domain).or_default().push(snapshot);
        self.snapshot_count += 1;
        Ok(snapshot)
    }

    pub fn get_snapshots(&self, domain: &OrderingDomain) -> Vec<OrderingPosSnapshot> {
        self.snapshots.borrow().get(domain).cloned().unwrap_or_default()
    }

    pub fn get_latest_snapshot(&self, domain: &OrderingDomain) -> Option<OrderingPosSnapshot> {
        self.snapshots
            .borrow()
            .get(domain)
            .and_then(|v| v.last().copied())
    }

    pub fn restore_snapshot(&self, domain: OrderingDomain, snapshot: OrderingPosSnapshot) -> bool {
        let snapshots = self.snapshots.borrow_mut();
        let domain_snaps = snapshots.entry(domain).or_default();
        if domain_snaps.iter().any(|s| s.position == snapshot.position) {
            self.restored_count += 1;
            true
        } else {
            false
        }
    }

    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count
    }

    pub fn restored_count(&self) -> u64 {
        self.restored_count
    }

    pub fn snapshot_domain_count(&self) -> usize {
        self.snapshots.borrow().len()
    }
}

impl Default for SnapshotIsolationOrdering {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// VECTOR CLOCK
///////////////////////////////////////////////////////////////////////////////

pub struct VectorClock {
    pub clocks: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub tick_count: u64,
    pub merge_count: u64,
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            clocks: std::cell::RefCell::new(std::collections::HashMap::new()),
            tick_count: 0,
            merge_count: 0,
        }
    }

    pub fn tick(&self, node_id: NodeId) -> u64 {
        let mut clocks = self.clocks.borrow_mut();
        let current = clocks.get(&node_id).copied().unwrap_or(0);
        let new_val = current + 1;
        clocks.insert(node_id, new_val);
        self.tick_count += 1;
        new_val
    }

    pub fn get(&self, node_id: &NodeId) -> u64 {
        self.clocks.borrow().get(node_id).copied().unwrap_or(0)
    }

    pub fn merge(&self, other: &std::collections::HashMap<NodeId, u64>) {
        let mut clocks = self.clocks.borrow_mut();
        for (node_id, val) in other.iter() {
            let current = clocks.get(node_id).copied().unwrap_or(0);
            if *val > current {
                clocks.insert(*node_id, *val);
            }
        }
        self.merge_count += 1;
    }

    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let a = self.clocks.borrow();
        let b = other.clocks.borrow();
        let mut all_less_equal = true;
        let mut some_strict = false;
        for (node, val_a) in a.iter() {
            let val_b = b.get(node).copied().unwrap_or(0);
            if val_a > val_b {
                all_less_equal = false;
                break;
            }
            if val_a < val_b {
                some_strict = true;
            }
        }
        all_less_equal && some_strict
    }

    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn merge_count(&self) -> u64 {
        self.merge_count
    }

    pub fn node_count(&self) -> usize {
        self.clocks.borrow().len()
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct OrderingStats {
    pub total_domains: u64,
    pub total_positions: u64,
    pub total_sequences: u64,
    pub total_snapshots: u64,
    pub total_causal_violations: u64,
    pub avg_positions_per_domain: f64,
    pub sequencing_rate: f64,
}

impl OrderingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn domain_efficiency(&self) -> f64 {
        if self.total_domains == 0 {
            return 1.0;
        }
        self.total_positions as f64 / self.total_domains as f64 / 1000.0
    }

    pub fn causal_violation_rate(&self) -> f64 {
        if self.total_positions == 0 {
            return 0.0;
        }
        self.total_causal_violations as f64 / self.total_positions as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct OrderingMonitor {
    pub domain_health: std::cell::RefCell<std::collections::HashMap<OrderingDomain, bool>>,
    pub alert_count: u64,
    pub health_check_count: u64,
}

impl OrderingMonitor {
    pub fn new() -> Self {
        Self {
            domain_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
        }
    }

    pub fn report_health(&self, domain: OrderingDomain, healthy: bool) {
        let mut health = self.domain_health.borrow_mut();
        let prev = health.get(&domain).copied().unwrap_or(true);
        health.insert(domain, healthy);
        self.health_check_count += 1;
        if prev && !healthy {
            self.alert_count += 1;
        }
    }

    pub fn is_healthy(&self, domain: &OrderingDomain) -> bool {
        self.domain_health.borrow().get(domain).copied().unwrap_or(true)
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn monitored_count(&self) -> usize {
        self.domain_health.borrow().len()
    }

    pub fn unhealthy_count(&self) -> usize {
        self.domain_health.borrow().values().filter(|h| !*h).count()
    }
}

impl Default for OrderingMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResult {
    Valid,
    InvalidPosition,
    InvalidDomain,
    CausalViolation,
}

impl ValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationResult::Valid => "valid",
            ValidationResult::InvalidPosition => "invalid_position",
            ValidationResult::InvalidDomain => "invalid_domain",
            ValidationResult::CausalViolation => "causal_violation",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }
}

pub struct OrderingValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<OrderingDomain, ValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl OrderingValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_position(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        current: OrderingPos,
    ) -> ValidationResult {
        self.validation_count += 1;
        let result = if position.value() <= current.value() {
            ValidationResult::InvalidPosition
        } else {
            ValidationResult::Valid
        };
        match result {
            ValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(domain, result);
        result
    }

    pub fn validate_domain(&self, domain: OrderingDomain, exists: bool) -> ValidationResult {
        self.validation_count += 1;
        let result = if exists {
            ValidationResult::Valid
        } else {
            ValidationResult::InvalidDomain
        };
        match result {
            ValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(domain, result);
        result
    }

    pub fn get_result(&self, domain: &OrderingDomain) -> Option<ValidationResult> {
        self.validations.borrow().get(domain).copied()
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

impl Default for OrderingValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct OrderingEngine {
    pub domain_mgr: std::cell::RefCell<OrderingDomainMgr>,
    pub sequencer: std::cell::RefCell<DomainSequencer>,
    pub clock: std::cell::RefCell<OrderingClock>,
    pub causal_tracker: std::cell::RefCell<CausalOrderTracker>,
    pub total_order: std::cell::RefCell<TotalOrderSequencer>,
    pub snapshot_isolation: std::cell::RefCell<SnapshotIsolationOrdering>,
    pub vector_clock: std::cell::RefCell<VectorClock>,
    pub stats: std::cell::RefCell<OrderingStats>,
    pub monitor: std::cell::RefCell<OrderingMonitor>,
    pub validator: std::cell::RefCell<OrderingValidator>,
}

impl OrderingEngine {
    pub fn new() -> Self {
        Self {
            domain_mgr: std::cell::RefCell::new(OrderingDomainMgr::new()),
            sequencer: std::cell::RefCell::new(DomainSequencer::new()),
            clock: std::cell::RefCell::new(OrderingClock::default()),
            causal_tracker: std::cell::RefCell::new(CausalOrderTracker::new()),
            total_order: std::cell::RefCell::new(TotalOrderSequencer::new()),
            snapshot_isolation: std::cell::RefCell::new(SnapshotIsolationOrdering::new()),
            vector_clock: std::cell::RefCell::new(VectorClock::new()),
            stats: std::cell::RefCell::new(OrderingStats::new()),
            monitor: std::cell::RefCell::new(OrderingMonitor::new()),
            validator: std::cell::RefCell::new(OrderingValidator::new()),
        }
    }

    pub fn create_domain(
        &self,
        dataset_id: DatasetId,
        domain_id: OrderingDomain,
    ) -> Result<OrderingDomain, OrderingError> {
        let entry = self.domain_mgr.borrow_mut().create_domain(dataset_id, domain_id, 0)?;
        self.sequencer.borrow_mut().initialize_position(domain_id, OrderingPos::ZERO);
        self.stats.borrow_mut().total_domains += 1;
        self.monitor.borrow_mut().report_health(domain_id, true);
        Ok(domain_id)
    }

    pub fn get_domain(&self, dataset_id: &DatasetId) -> Option<OrderingDomain> {
        self.domain_mgr.borrow().get_domain_for_dataset(dataset_id)
    }

    pub fn advance_position(
        &self,
        domain: OrderingDomain,
    ) -> Result<OrderingPos, OrderingError> {
        if !self.domain_mgr.borrow().get_domain(&domain).is_some() {
            return Err(OrderingError::DomainNotFound { domain_id: domain });
        }
        let pos = self.sequencer.borrow_mut().advance_one(domain)?;
        self.stats.borrow_mut().total_positions += 1;
        Ok(pos)
    }

    pub fn current_position(&self, domain: &OrderingDomain) -> OrderingPos {
        self.sequencer.borrow().get_position(domain)
    }

    pub fn happens_before(
        &self,
        domain: OrderingDomain,
        position_a: OrderingPos,
        position_b: OrderingPos,
    ) -> Result<bool, OrderingError> {
        if !self.domain_mgr.borrow().get_domain(&domain).is_some() {
            return Err(OrderingError::DomainNotFound { domain_id: domain });
        }
        Ok(position_a.value() < position_b.value())
    }

    pub fn record_total_order(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        record_id: RecordId,
    ) -> Result<(), OrderingError> {
        if !self.domain_mgr.borrow().get_domain(&domain).is_some() {
            return Err(OrderingError::DomainNotFound { domain_id: domain });
        }
        self.total_order.borrow_mut().sequence(domain, position, record_id, 0)?;
        self.stats.borrow_mut().total_sequences += 1;
        Ok(())
    }

    pub fn record_at_position(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
    ) -> Result<Option<RecordId>, OrderingError> {
        if !self.domain_mgr.borrow().get_domain(&domain).is_some() {
            return Err(OrderingError::DomainNotFound { domain_id: domain });
        }
        Ok(self.total_order.borrow().get_record_at(position))
    }

    pub fn create_snapshot(
        &self,
        domain: OrderingDomain,
        ids_time: IdsTime,
    ) -> Result<OrderingPosSnapshot, OrderingError> {
        if !self.domain_mgr.borrow().get_domain(&domain).is_some() {
            return Err(OrderingError::DomainNotFound { domain_id: domain });
        }
        let pos = self.current_position(&domain);
        let snapshot = self.snapshot_isolation.borrow_mut().create_snapshot(domain, pos, ids_time)?;
        self.stats.borrow_mut().total_snapshots += 1;
        Ok(snapshot)
    }

    pub fn stats(&self) -> OrderingStats {
        self.stats.borrow().clone()
    }

    pub fn ordering_name(&self) -> &'static str {
        "ordering_engine"
    }

    pub fn domain_count(&self) -> u64 {
        self.domain_mgr.borrow().domain_count()
    }

    pub fn position_count(&self) -> u64 {
        self.stats.borrow().total_positions
    }

    pub fn validate_position(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        current: OrderingPos,
    ) -> ValidationResult {
        self.validator.borrow_mut().validate_position(domain, position, current)
    }

    pub fn validate_domain_exists(&self, domain: OrderingDomain) -> ValidationResult {
        let exists = self.domain_mgr.borrow().get_domain(&domain).is_some();
        self.validator.borrow_mut().validate_domain(domain, exists)
    }

    pub fn record_causal_dependency(
        &self,
        domain: OrderingDomain,
        cause: OrderingPos,
        effect: OrderingPos,
    ) -> Result<(), OrderingError> {
        self.causal_tracker.borrow_mut().record_dependency(domain, cause, effect)?;
        if cause.value() >= effect.value() {
            self.stats.borrow_mut().total_causal_violations += 1;
        }
        Ok(())
    }

    pub fn tick_clock(&self, domain: OrderingDomain, at_ms: u64) -> Result<u64, OrderingError> {
        self.clock.borrow_mut().tick(domain, at_ms)
    }

    pub fn vector_clock_tick(&self, node_id: NodeId) -> u64 {
        self.vector_clock.borrow_mut().tick(node_id)
    }

    pub fn vector_clock_merge(&self, other: &std::collections::HashMap<NodeId, u64>) {
        self.vector_clock.borrow_mut().merge(other);
    }

    pub fn monitor_health(&self, domain: OrderingDomain, healthy: bool) {
        self.monitor.borrow_mut().report_health(domain, healthy);
    }

    pub fn is_domain_healthy(&self, domain: &OrderingDomain) -> bool {
        self.monitor.borrow().is_healthy(domain)
    }
}

impl Default for OrderingEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING CONFLICT RESOLVER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolutionOutcome {
    PositionWins,
    RecordWins,
    TimestampWins,
    ManualResolution,
}

impl ConflictResolutionOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictResolutionOutcome::PositionWins => "position_wins",
            ConflictResolutionOutcome::RecordWins => "record_wins",
            ConflictResolutionOutcome::TimestampWins => "timestamp_wins",
            ConflictResolutionOutcome::ManualResolution => "manual_resolution",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderingConflict {
    pub domain: OrderingDomain,
    pub position_a: OrderingPos,
    pub position_b: OrderingPos,
    pub record_a: RecordId,
    pub record_b: RecordId,
    pub detected_at_ms: u64,
    pub resolved: bool,
    pub resolution: Option<ConflictResolutionOutcome>,
}

impl OrderingConflict {
    pub fn new(
        domain: OrderingDomain,
        position_a: OrderingPos,
        position_b: OrderingPos,
        record_a: RecordId,
        record_b: RecordId,
        detected_at_ms: u64,
    ) -> Self {
        Self {
            domain,
            position_a,
            position_b,
            record_a,
            record_b,
            detected_at_ms,
            resolved: false,
            resolution: None,
        }
    }

    pub fn mark_resolved(&mut self, resolution: ConflictResolutionOutcome) {
        self.resolved = true;
        self.resolution = Some(resolution);
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved
    }
}

pub struct OrderingConflictResolver {
    pub conflicts: std::cell::RefCell<std::collections::HashMap<OrderingPos, OrderingConflict>>,
    pub conflict_count: u64,
    pub resolved_count: u64,
    pub auto_resolved_count: u64,
    pub manual_resolved_count: u64,
}

impl OrderingConflictResolver {
    pub fn new() -> Self {
        Self {
            conflicts: std::cell::RefCell::new(std::collections::HashMap::new()),
            conflict_count: 0,
            resolved_count: 0,
            auto_resolved_count: 0,
            manual_resolved_count: 0,
        }
    }

    pub fn detect_conflict(
        &self,
        domain: OrderingDomain,
        position_a: OrderingPos,
        position_b: OrderingPos,
        record_a: RecordId,
        record_b: RecordId,
        detected_at_ms: u64,
    ) -> OrderingPos {
        let conflict_pos = if position_a.value() < position_b.value() {
            position_b
        } else {
            position_a
        };
        let conflict = OrderingConflict::new(
            domain, position_a, position_b, record_a, record_b, detected_at_ms,
        );
        self.conflicts.borrow_mut().insert(conflict_pos, conflict);
        self.conflict_count += 1;
        conflict_pos
    }

    pub fn resolve_conflict(
        &self,
        position: OrderingPos,
        outcome: ConflictResolutionOutcome,
    ) -> bool {
        if let Some(conflict) = self.conflicts.borrow_mut().get_mut(&position) {
            if !conflict.is_resolved() {
                conflict.mark_resolved(outcome);
                self.resolved_count += 1;
                match outcome {
                    ConflictResolutionOutcome::ManualResolution => {
                        self.manual_resolved_count += 1;
                    }
                    _ => {
                        self.auto_resolved_count += 1;
                    }
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn auto_resolve_by_position(&self, position: OrderingPos) -> bool {
        self.resolve_conflict(position, ConflictResolutionOutcome::PositionWins)
    }

    pub fn get_conflict(&self, position: &OrderingPos) -> Option<OrderingConflict> {
        self.conflicts.borrow().get(position).cloned()
    }

    pub fn conflict_count(&self) -> u64 {
        self.conflict_count
    }

    pub fn resolved_count(&self) -> u64 {
        self.resolved_count
    }

    pub fn unresolved_count(&self) -> u64 {
        self.conflict_count - self.resolved_count
    }

    pub fn resolution_rate(&self) -> f64 {
        if self.conflict_count == 0 {
            return 1.0;
        }
        self.resolved_count as f64 / self.conflict_count as f64
    }

    pub fn active_conflict_count(&self) -> usize {
        self.conflicts
            .borrow()
            .values()
            .filter(|c| !c.is_resolved())
            .count()
    }
}

impl Default for OrderingConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING BARRIER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierState {
    Waiting,
    Reached,
    Broken,
}

impl BarrierState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BarrierState::Waiting => "waiting",
            BarrierState::Reached => "reached",
            BarrierState::Broken => "broken",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, BarrierState::Reached | BarrierState::Broken)
    }
}

pub struct OrderingBarrier {
    pub barrier_id: OrderingDomain,
    pub target_position: OrderingPos,
    pub current_position: std::cell::Cell<OrderingPos>,
    pub state: std::cell::Cell<BarrierState>,
    pub participant_count: u64,
    pub arrived_count: u64,
    pub barrier_reached_at_ms: Option<u64>,
}

impl OrderingBarrier {
    pub fn new(barrier_id: OrderingDomain, target_position: OrderingPos) -> Self {
        Self {
            barrier_id,
            target_position,
            current_position: std::cell::Cell::new(OrderingPos::ZERO),
            state: std::cell::Cell::new(BarrierState::Waiting),
            participant_count: 0,
            arrived_count: 0,
            barrier_reached_at_ms: None,
        }
    }

    pub fn arrive(&self, position: OrderingPos, at_ms: u64) -> bool {
        if self.state.get().is_terminal() {
            return false;
        }
        self.current_position.set(position);
        self.arrived_count += 1;
        if position.value() >= self.target_position.value() {
            self.state.set(BarrierState::Reached);
            self.barrier_reached_at_ms = Some(at_ms);
            true
        } else {
            false
        }
    }

    pub fn break_barrier(&self, at_ms: u64) {
        self.state.set(BarrierState::Broken);
    }

    pub fn register_participant(&self) {
        self.participant_count += 1;
    }

    pub fn is_reached(&self) -> bool {
        matches!(self.state.get(), BarrierState::Reached)
    }

    pub fn is_broken(&self) -> bool {
        matches!(self.state.get(), BarrierState::Broken)
    }

    pub fn current_position(&self) -> OrderingPos {
        self.current_position.get()
    }

    pub fn target_position(&self) -> OrderingPos {
        self.target_position
    }

    pub fn participant_count(&self) -> u64 {
        self.participant_count
    }

    pub fn arrived_count(&self) -> u64 {
        self.arrived_count
    }

    pub fn progress_percent(&self) -> u8 {
        if self.target_position.value() == 0 {
            return 100;
        }
        (self.current_position.value() as u32 * 100 / self.target_position.value() as u32)
            .min(100)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING CHECKPOINT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct OrderingCheckpoint {
    pub checkpoint_id: OrderingDomain,
    pub domain: OrderingDomain,
    pub position: OrderingPos,
    pub created_at_ms: u64,
    pub entry_count: u64,
    pub verified: bool,
}

impl OrderingCheckpoint {
    pub fn new(
        checkpoint_id: OrderingDomain,
        domain: OrderingDomain,
        position: OrderingPos,
        created_at_ms: u64,
    ) -> Self {
        Self {
            checkpoint_id,
            domain,
            position,
            created_at_ms,
            entry_count: 0,
            verified: false,
        }
    }

    pub fn set_entry_count(&mut self, count: u64) {
        self.entry_count = count;
    }

    pub fn verify(&mut self) {
        self.verified = true;
    }

    pub fn is_verified(&self) -> bool {
        self.verified
    }
}

pub struct OrderingCheckpointMgr {
    pub checkpoints: std::cell::RefCell<std::collections::HashMap<OrderingDomain, OrderingCheckpoint>>,
    pub checkpoint_count: u64,
    pub verified_count: u64,
    pub restored_count: u64,
}

impl OrderingCheckpointMgr {
    pub fn new() -> Self {
        Self {
            checkpoints: std::cell::RefCell::new(std::collections::HashMap::new()),
            checkpoint_count: 0,
            verified_count: 0,
            restored_count: 0,
        }
    }

    pub fn create_checkpoint(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        created_at_ms: u64,
    ) -> OrderingCheckpoint {
        let checkpoint = OrderingCheckpoint::new(domain, domain, position, created_at_ms);
        self.checkpoints.borrow_mut().insert(domain, checkpoint.clone());
        self.checkpoint_count += 1;
        checkpoint
    }

    pub fn get_checkpoint(&self, domain: &OrderingDomain) -> Option<OrderingCheckpoint> {
        self.checkpoints.borrow().get(domain).cloned()
    }

    pub fn verify_checkpoint(&self, domain: &OrderingDomain) -> bool {
        if let Some(cp) = self.checkpoints.borrow_mut().get_mut(domain) {
            if !cp.is_verified() {
                cp.verify();
                self.verified_count += 1;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn restore_checkpoint(&self, domain: &OrderingDomain) -> Option<OrderingCheckpoint> {
        if let Some(cp) = self.checkpoints.borrow().get(domain) {
            if cp.is_verified() {
                self.restored_count += 1;
                return Some(cp.clone());
            }
        }
        None
    }

    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoint_count
    }

    pub fn verified_count(&self) -> u64 {
        self.verified_count
    }

    pub fn restored_count(&self) -> u64 {
        self.restored_count
    }

    pub fn tracked_count(&self) -> usize {
        self.checkpoints.borrow().len()
    }
}

impl Default for OrderingCheckpointMgr {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING REPLICATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicaSyncState {
    Synced,
    Lagging,
    Behind,
    Disconnected,
}

impl ReplicaSyncState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplicaSyncState::Synced => "synced",
            ReplicaSyncState::Lagging => "lagging",
            ReplicaSyncState::Behind => "behind",
            ReplicaSyncState::Disconnected => "disconnected",
        }
    }

    pub fn is_synced(&self) -> bool {
        matches!(self, ReplicaSyncState::Synced)
    }
}

pub struct ReplicaOrderingInfo {
    pub node_id: NodeId,
    pub position: OrderingPos,
    pub state: ReplicaSyncState,
    pub last_sync_at_ms: u64,
    pub lag_ms: u64,
}

impl ReplicaOrderingInfo {
    pub fn new(node_id: NodeId, position: OrderingPos, last_sync_at_ms: u64) -> Self {
        Self {
            node_id,
            position,
            state: ReplicaSyncState::Synced,
            last_sync_at_ms,
            lag_ms: 0,
        }
    }

    pub fn update_position(&mut self, position: OrderingPos, at_ms: u64) {
        self.position = position;
        self.last_sync_at_ms = at_ms;
        self.lag_ms = at_ms.saturating_sub(self.last_sync_at_ms);
        self.state = if self.lag_ms > 10000 {
            ReplicaSyncState::Behind
        } else if self.lag_ms > 1000 {
            ReplicaSyncState::Lagging
        } else {
            ReplicaSyncState::Synced
        };
    }

    pub fn disconnect(&mut self) {
        self.state = ReplicaSyncState::Disconnected;
    }

    pub fn reconnect(&mut self, at_ms: u64) {
        self.state = ReplicaSyncState::Synced;
        self.last_sync_at_ms = at_ms;
    }
}

pub struct OrderingReplicator {
    pub replicas: std::cell::RefCell<std::collections::HashMap<NodeId, ReplicaOrderingInfo>>,
    pub sync_count: u64,
    pub lag_warning_count: u64,
    pub disconnection_count: u64,
}

impl OrderingReplicator {
    pub fn new() -> Self {
        Self {
            replicas: std::cell::RefCell::new(std::collections::HashMap::new()),
            sync_count: 0,
            lag_warning_count: 0,
            disconnection_count: 0,
        }
    }

    pub fn register_replica(&self, node_id: NodeId, position: OrderingPos, at_ms: u64) {
        self.replicas
            .borrow_mut()
            .insert(node_id, ReplicaOrderingInfo::new(node_id, position, at_ms));
        self.sync_count += 1;
    }

    pub fn update_replica(
        &self,
        node_id: NodeId,
        position: OrderingPos,
        at_ms: u64,
    ) -> Option<ReplicaSyncState> {
        if let Some(info) = self.replicas.borrow_mut().get_mut(&node_id) {
            let old_state = info.state;
            info.update_position(position, at_ms);
            if old_state != info.state && info.state == ReplicaSyncState::Lagging {
                self.lag_warning_count += 1;
            }
            Some(info.state)
        } else {
            None
        }
    }

    pub fn disconnect_replica(&self, node_id: &NodeId) -> bool {
        if let Some(info) = self.replicas.borrow_mut().get_mut(node_id) {
            info.disconnect();
            self.disconnection_count += 1;
            true
        } else {
            false
        }
    }

    pub fn reconnect_replica(&self, node_id: &NodeId, at_ms: u64) -> bool {
        if let Some(info) = self.replicas.borrow_mut().get_mut(node_id) {
            info.reconnect(at_ms);
            true
        } else {
            false
        }
    }

    pub fn get_replica_state(&self, node_id: &NodeId) -> Option<ReplicaSyncState> {
        self.replicas.borrow().get(node_id).map(|i| i.state)
    }

    pub fn replica_count(&self) -> usize {
        self.replicas.borrow().len()
    }

    pub fn synced_count(&self) -> usize {
        self.replicas
            .borrow()
            .values()
            .filter(|i| i.is_synced())
            .count()
    }

    pub fn lagging_count(&self) -> usize {
        self.replicas
            .borrow()
            .values()
            .filter(|i| matches!(i.state, ReplicaSyncState::Lagging | ReplicaSyncState::Behind))
            .count()
    }

    pub fn disconnected_count(&self) -> usize {
        self.replicas
            .borrow()
            .values()
            .filter(|i| matches!(i.state, ReplicaSyncState::Disconnected))
            .count()
    }

    pub fn sync_count(&self) -> u64 {
        self.sync_count
    }

    pub fn lag_warning_count(&self) -> u64 {
        self.lag_warning_count
    }

    pub fn disconnection_count(&self) -> u64 {
        self.disconnection_count
    }
}

impl Default for OrderingReplicator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING COMPACTOR
///////////////////////////////////////////////////////////////////////////////

pub struct OrderingCompactor {
    pub compaction_threshold: u64,
    pub compaction_count: u64,
    pub entries_compacted: u64,
    pub entries_retained: u64,
    pub last_compacted_position: OrderingPos,
}

impl OrderingCompactor {
    pub fn new(compaction_threshold: u64) -> Self {
        Self {
            compaction_threshold,
            compaction_count: 0,
            entries_compacted: 0,
            entries_retained: 0,
            last_compacted_position: OrderingPos::ZERO,
        }
    }

    pub fn should_compact(&self, current_position: OrderingPos) -> bool {
        current_position.value() - self.last_compacted_position.value()
            >= self.compaction_threshold
    }

    pub fn compact(&mut self, current_position: OrderingPos, retain_count: u64) -> u64 {
        if !self.should_compact(current_position) {
            return 0;
        }
        let compacted = current_position.value() - self.last_compacted_position.value() - retain_count;
        let compacted = compacted.max(0);
        self.entries_compacted += compacted;
        self.entries_retained += retain_count;
        self.last_compacted_position = current_position;
        self.compaction_count += 1;
        compacted
    }

    pub fn compaction_count(&self) -> u64 {
        self.compaction_count
    }

    pub fn entries_compacted(&self) -> u64 {
        self.entries_compacted
    }

    pub fn entries_retained(&self) -> u64 {
        self.entries_retained
    }

    pub fn last_compacted_position(&self) -> OrderingPos {
        self.last_compacted_position
    }
}

impl Default for OrderingCompactor {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING LEASE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    Pending,
    Active,
    Expired,
    Revoked,
}

impl LeaseState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeaseState::Pending => "pending",
            LeaseState::Active => "active",
            LeaseState::Expired => "expired",
            LeaseState::Revoked => "revoked",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, LeaseState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, LeaseState::Expired | LeaseState::Revoked)
    }
}

pub struct OrderingLease {
    pub lease_id: OrderingDomain,
    pub holder: NodeId,
    pub domain: OrderingDomain,
    pub state: std::cell::Cell<LeaseState>,
    pub granted_at_ms: u64,
    pub expires_at_ms: u64,
    pub renewal_count: u64,
}

impl OrderingLease {
    pub fn new(
        lease_id: OrderingDomain,
        holder: NodeId,
        domain: OrderingDomain,
        granted_at_ms: u64,
        ttl_ms: u64,
    ) -> Self {
        Self {
            lease_id,
            holder,
            domain,
            state: std::cell::Cell::new(LeaseState::Pending),
            granted_at_ms,
            expires_at_ms: granted_at_ms + ttl_ms,
            renewal_count: 0,
        }
    }

    pub fn activate(&self) {
        self.state.set(LeaseState::Active);
    }

    pub fn renew(&self, at_ms: u64, ttl_ms: u64) -> bool {
        if self.state.get() == LeaseState::Active && at_ms < self.expires_at_ms.get() {
            self.expires_at_ms.set(at_ms + ttl_ms);
            self.renewal_count += 1;
            true
        } else {
            false
        }
    }

    pub fn expire(&self) {
        self.state.set(LeaseState::Expired);
    }

    pub fn revoke(&self) {
        self.state.set(LeaseState::Revoked);
    }

    pub fn is_valid(&self, at_ms: u64) -> bool {
        self.state.get() == LeaseState::Active && at_ms < self.expires_at_ms.get()
    }

    pub fn time_to_expiry(&self, at_ms: u64) -> u64 {
        if at_ms < self.expires_at_ms.get() {
            self.expires_at_ms.get() - at_ms
        } else {
            0
        }
    }

    pub fn state(&self) -> LeaseState {
        self.state.get()
    }

    pub fn renewal_count(&self) -> u64 {
        self.renewal_count
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING PARTITION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionOrderingState {
    Forming,
    Ordered,
    Recovering,
    Failed,
}

impl PartitionOrderingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionOrderingState::Forming => "forming",
            PartitionOrderingState::Ordered => "ordered",
            PartitionOrderingState::Recovering => "recovering",
            PartitionOrderingState::Failed => "failed",
        }
    }

    pub fn is_ordered(&self) -> bool {
        matches!(self, PartitionOrderingState::Ordered)
    }
}

pub struct OrderingPartition {
    pub partition_id: PartitionId,
    pub domain: OrderingDomain,
    pub state: std::cell::Cell<PartitionOrderingState>,
    pub position: OrderingPos,
    pub member_count: u64,
    pub quorum_size: u64,
}

impl OrderingPartition {
    pub fn new(
        partition_id: PartitionId,
        domain: OrderingDomain,
        quorum_size: u64,
    ) -> Self {
        Self {
            partition_id,
            domain,
            state: std::cell::Cell::new(PartitionOrderingState::Forming),
            position: OrderingPos::ZERO,
            member_count: 0,
            quorum_size,
        }
    }

    pub fn order(&self, position: OrderingPos) {
        self.state.set(PartitionOrderingState::Ordered);
        self.position = position;
    }

    pub fn recover(&self) {
        self.state.set(PartitionOrderingState::Recovering);
    }

    pub fn fail(&self) {
        self.state.set(PartitionOrderingState::Failed);
    }

    pub fn add_member(&self) {
        self.member_count += 1;
    }

    pub fn has_quorum(&self) -> bool {
        self.member_count >= self.quorum_size
    }

    pub fn state(&self) -> PartitionOrderingState {
        self.state.get()
    }

    pub fn is_ordered(&self) -> bool {
        self.state().is_ordered()
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED ORDERING ENGINE
///////////////////////////////////////////////////////////////////////////////

impl OrderingEngine {
    pub fn new_with_features() -> Self {
        Self {
            domain_mgr: std::cell::RefCell::new(OrderingDomainMgr::new()),
            sequencer: std::cell::RefCell::new(DomainSequencer::new()),
            clock: std::cell::RefCell::new(OrderingClock::default()),
            causal_tracker: std::cell::RefCell::new(CausalOrderTracker::new()),
            total_order: std::cell::RefCell::new(TotalOrderSequencer::new()),
            snapshot_isolation: std::cell::RefCell::new(SnapshotIsolationOrdering::new()),
            vector_clock: std::cell::RefCell::new(VectorClock::new()),
            stats: std::cell::RefCell::new(OrderingStats::new()),
            monitor: std::cell::RefCell::new(OrderingMonitor::new()),
            validator: std::cell::RefCell::new(OrderingValidator::new()),
            conflict_resolver: std::cell::RefCell::new(OrderingConflictResolver::new()),
            barrier: std::cell::RefCell::new(OrderingBarrier::new(
                OrderingDomain::ZERO,
                OrderingPos::ZERO,
            )),
            checkpoint_mgr: std::cell::RefCell::new(OrderingCheckpointMgr::new()),
            replicator: std::cell::RefCell::new(OrderingReplicator::new()),
            compactor: std::cell::RefCell::new(OrderingCompactor::new(10000)),
            ordering_lease: std::cell::RefCell::new(OrderingLease::new(
                OrderingDomain::ZERO,
                NodeId::new([0; 32]),
                OrderingDomain::ZERO,
                0,
                60000,
            )),
            ordering_partition: std::cell::RefCell::new(OrderingPartition::new(
                PartitionId::new([0; 32]),
                OrderingDomain::ZERO,
                1,
            )),
        }
    }

    pub fn detect_conflict(
        &self,
        domain: OrderingDomain,
        position_a: OrderingPos,
        position_b: OrderingPos,
        record_a: RecordId,
        record_b: RecordId,
        at_ms: u64,
    ) -> OrderingPos {
        self.conflict_resolver
            .borrow_mut()
            .detect_conflict(domain, position_a, position_b, record_a, record_b, at_ms)
    }

    pub fn resolve_conflict(
        &self,
        position: OrderingPos,
        outcome: ConflictResolutionOutcome,
    ) -> bool {
        self.conflict_resolver.borrow_mut().resolve_conflict(position, outcome)
    }

    pub fn auto_resolve_by_position(&self, position: OrderingPos) -> bool {
        self.conflict_resolver.borrow_mut().auto_resolve_by_position(position)
    }

    pub fn conflict_count(&self) -> u64 {
        self.conflict_resolver.borrow().conflict_count()
    }

    pub fn conflict_resolution_rate(&self) -> f64 {
        self.conflict_resolver.borrow().resolution_rate()
    }

    pub fn active_conflict_count(&self) -> usize {
        self.conflict_resolver.borrow().active_conflict_count()
    }

    pub fn create_barrier(
        &self,
        barrier_id: OrderingDomain,
        target_position: OrderingPos,
    ) {
        self.barrier
            .borrow_mut()
            .replace(OrderingBarrier::new(barrier_id, target_position));
    }

    pub fn barrier_arrive(&self, position: OrderingPos, at_ms: u64) -> bool {
        self.barrier.borrow_mut().arrive(position, at_ms)
    }

    pub fn barrier_reached(&self) -> bool {
        self.barrier.borrow().is_reached()
    }

    pub fn barrier_progress(&self) -> u8 {
        self.barrier.borrow().progress_percent()
    }

    pub fn register_barrier_participant(&self) {
        self.barrier.borrow_mut().participant_count += 1;
    }

    pub fn create_ordering_checkpoint(
        &self,
        domain: OrderingDomain,
        position: OrderingPos,
        at_ms: u64,
    ) -> OrderingCheckpoint {
        self.checkpoint_mgr
            .borrow_mut()
            .create_checkpoint(domain, position, at_ms)
    }

    pub fn verify_checkpoint(&self, domain: &OrderingDomain) -> bool {
        self.checkpoint_mgr.borrow_mut().verify_checkpoint(domain)
    }

    pub fn restore_checkpoint(&self, domain: &OrderingDomain) -> Option<OrderingCheckpoint> {
        self.checkpoint_mgr.borrow_mut().restore_checkpoint(domain)
    }

    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoint_mgr.borrow().checkpoint_count()
    }

    pub fn register_replica(
        &self,
        node_id: NodeId,
        position: OrderingPos,
        at_ms: u64,
    ) {
        self.replicator.borrow_mut().register_replica(node_id, position, at_ms);
    }

    pub fn update_replica_position(
        &self,
        node_id: NodeId,
        position: OrderingPos,
        at_ms: u64,
    ) -> Option<ReplicaSyncState> {
        self.replicator.borrow_mut().update_replica(node_id, position, at_ms)
    }

    pub fn replica_state(&self, node_id: &NodeId) -> Option<ReplicaSyncState> {
        self.replicator.borrow().get_replica_state(node_id)
    }

    pub fn replica_count(&self) -> usize {
        self.replicator.borrow().replica_count()
    }

    pub fn synced_replica_count(&self) -> usize {
        self.replicator.borrow().synced_count()
    }

    pub fn compact(
        &mut self,
        current_position: OrderingPos,
        retain_count: u64,
    ) -> u64 {
        self.compactor.borrow_mut().compact(current_position, retain_count)
    }

    pub fn compaction_count(&self) -> u64 {
        self.compactor.borrow().compaction_count()
    }

    pub fn entries_compacted(&self) -> u64 {
        self.compactor.borrow().entries_compacted()
    }

    pub fn create_lease(
        &self,
        lease_id: OrderingDomain,
        holder: NodeId,
        domain: OrderingDomain,
        at_ms: u64,
        ttl_ms: u64,
    ) -> OrderingLease {
        self.ordering_lease
            .borrow_mut()
            .replace(OrderingLease::new(lease_id, holder, domain, at_ms, ttl_ms));
        self.ordering_lease.borrow().clone()
    }

    pub fn activate_lease(&self) {
        self.ordering_lease.borrow_mut().activate();
    }

    pub fn renew_lease(&self, at_ms: u64, ttl_ms: u64) -> bool {
        self.ordering_lease.borrow_mut().renew(at_ms, ttl_ms)
    }

    pub fn lease_state(&self) -> LeaseState {
        self.ordering_lease.borrow().state()
    }

    pub fn lease_is_valid(&self, at_ms: u64) -> bool {
        self.ordering_lease.borrow().is_valid(at_ms)
    }

    pub fn create_ordering_partition(
        &self,
        partition_id: PartitionId,
        domain: OrderingDomain,
        quorum_size: u64,
    ) -> OrderingPartition {
        self.ordering_partition
            .borrow_mut()
            .replace(OrderingPartition::new(partition_id, domain, quorum_size));
        self.ordering_partition.borrow().clone()
    }

    pub fn order_partition(&self, partition_id: PartitionId, position: OrderingPos) {
        let mut p = self.ordering_partition.borrow_mut();
        if p.partition_id == partition_id {
            p.order(position);
        }
    }

    pub fn partition_state(&self, partition_id: PartitionId) -> Option<PartitionOrderingState> {
        let p = self.ordering_partition.borrow();
        if p.partition_id == partition_id {
            Some(p.state())
        } else {
            None
        }
    }

    pub fn partition_has_quorum(&self, partition_id: PartitionId) -> bool {
        let p = self.ordering_partition.borrow();
        p.partition_id == partition_id && p.has_quorum()
    }

    pub fn add_partition_member(&self, partition_id: PartitionId) {
        let mut p = self.ordering_partition.borrow_mut();
        if p.partition_id == partition_id {
            p.add_member();
        }
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

    fn test_domain_id() -> OrderingDomain {
        OrderingDomain::new([1; 32])
    }

    fn test_node_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_record_id() -> RecordId {
        RecordId::new([1; 32])
    }

    fn test_position() -> OrderingPos {
        OrderingPos::new(100)
    }

    fn test_ids_time() -> IdsTime {
        IdsTime::new(1000)
    }

    // === OrderingError tests ===
    #[test]
    fn ordering_error_category() {
        assert_eq!(
            OrderingError::DomainNotFound { domain_id: test_domain_id() }.category(),
            "domain_not_found"
        );
        assert_eq!(
            OrderingError::DatasetNotFound { dataset_id: test_dataset_id() }.category(),
            "dataset_not_found"
        );
        assert_eq!(
            OrderingError::PositionNotFound { position: test_position() }.category(),
            "position_not_found"
        );
        assert_eq!(
            OrderingError::RecordNotFound { record_id: test_record_id() }.category(),
            "record_not_found"
        );
        assert_eq!(
            OrderingError::DomainAlreadyExists { domain_id: test_domain_id() }.category(),
            "domain_already_exists"
        );
        assert_eq!(
            OrderingError::PositionOutOfOrder { expected: 100, got: 50 }.category(),
            "position_out_of_order"
        );
        assert_eq!(
            OrderingError::CausalViolation { position: 50, depends_on: 100 }.category(),
            "causal_violation"
        );
        assert_eq!(
            OrderingError::SnapshotNotFound { snapshot_id: test_domain_id() }.category(),
            "snapshot_not_found"
        );
        assert_eq!(
            OrderingError::CrossDomainOrdering {
                domain_a: test_domain_id(),
                domain_b: OrderingDomain::new([2; 32]),
            }
            .category(),
            "cross_domain_ordering"
        );
        assert_eq!(
            OrderingError::InvalidState { message: "bad".to_string() }.category(),
            "invalid_state"
        );
        assert_eq!(
            OrderingError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn ordering_error_is_recoverable() {
        assert!(OrderingError::PositionOutOfOrder { expected: 100, got: 50 }.is_recoverable());
        assert!(OrderingError::CausalViolation { position: 50, depends_on: 100 }.is_recoverable());
        assert!(!OrderingError::DomainNotFound { domain_id: test_domain_id() }.is_recoverable());
        assert!(!OrderingError::SnapshotNotFound { snapshot_id: test_domain_id() }.is_recoverable());
    }

    #[test]
    fn ordering_error_suggested_action() {
        assert_eq!(
            OrderingError::DomainNotFound { domain_id: test_domain_id() }.suggested_action(),
            "create domain first"
        );
        assert_eq!(
            OrderingError::PositionOutOfOrder { expected: 100, got: 50 }.suggested_action(),
            "use next valid position"
        );
        assert_eq!(
            OrderingError::CausalViolation { position: 50, depends_on: 100 }.suggested_action(),
            "wait for dependency to complete"
        );
        assert_eq!(
            OrderingError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === DomainLifecycleState tests ===
    #[test]
    fn domain_lifecycle_state_as_str() {
        assert_eq!(DomainLifecycleState::Creating.as_str(), "creating");
        assert_eq!(DomainLifecycleState::Active.as_str(), "active");
        assert_eq!(DomainLifecycleState::Frozen.as_str(), "frozen");
        assert_eq!(DomainLifecycleState::Retired.as_str(), "retired");
    }

    #[test]
    fn domain_lifecycle_state_active_and_terminal() {
        assert!(DomainLifecycleState::Active.is_active());
        assert!(!DomainLifecycleState::Creating.is_active());
        assert!(!DomainLifecycleState::Frozen.is_active());
        assert!(DomainLifecycleState::Retired.is_terminal());
        assert!(!DomainLifecycleState::Active.is_terminal());
    }

    // === OrderingDomainEntry tests ===
    #[test]
    fn ordering_domain_entry_new_and_activate() {
        let entry = OrderingDomainEntry::new(test_domain_id(), test_dataset_id(), 1000);
        assert_eq!(entry.domain_id, test_domain_id());
        assert_eq!(entry.dataset_id, test_dataset_id());
        assert_eq!(entry.state, DomainLifecycleState::Creating);
        assert_eq!(entry.current_position, OrderingPos::ZERO);
        assert_eq!(entry.entry_count, 0);
    }

    #[test]
    fn ordering_domain_entry_transitions() {
        let mut entry = OrderingDomainEntry::new(test_domain_id(), test_dataset_id(), 1000);
        entry.activate();
        assert_eq!(entry.state, DomainLifecycleState::Active);
        entry.freeze();
        assert_eq!(entry.state, DomainLifecycleState::Frozen);
        entry.retire();
        assert_eq!(entry.state, DomainLifecycleState::Retired);
    }

    #[test]
    fn ordering_domain_entry_advance() {
        let mut entry = OrderingDomainEntry::new(test_domain_id(), test_dataset_id(), 1000);
        entry.advance(5);
        assert_eq!(entry.current_position, OrderingPos::new(5));
        assert_eq!(entry.entry_count, 1);
        entry.advance(3);
        assert_eq!(entry.current_position, OrderingPos::new(8));
        assert_eq!(entry.entry_count, 2);
    }

    // === OrderingDomainMgr tests ===
    #[test]
    fn ordering_domain_mgr_create_and_get() {
        let mgr = OrderingDomainMgr::default();
        let entry = mgr.create_domain(test_dataset_id(), test_domain_id(), 1000).unwrap();
        assert_eq!(entry.domain_id, test_domain_id());
        assert_eq!(mgr.domain_count(), 1);
        assert_eq!(mgr.active_domain_count(), 1);
        assert_eq!(mgr.track_count(), 1);
    }

    #[test]
    fn ordering_domain_mgr_domain_for_dataset() {
        let mgr = OrderingDomainMgr::default();
        mgr.create_domain(test_dataset_id(), test_domain_id(), 1000).unwrap();
        assert_eq!(mgr.get_domain_for_dataset(&test_dataset_id()), Some(test_domain_id()));
        assert_eq!(mgr.get_domain_for_dataset(&DatasetId::new([2; 32])), None);
    }

    #[test]
    fn ordering_domain_mgr_duplicate_domain() {
        let mgr = OrderingDomainMgr::default();
        mgr.create_domain(test_dataset_id(), test_domain_id(), 1000).unwrap();
        let err = mgr.create_domain(test_dataset_id(), test_domain_id(), 2000).unwrap_err();
        assert!(matches!(err, OrderingError::DomainAlreadyExists { .. }));
    }

    #[test]
    fn ordering_domain_mgr_freeze_retire() {
        let mgr = OrderingDomainMgr::default();
        mgr.create_domain(test_dataset_id(), test_domain_id(), 1000).unwrap();
        assert!(mgr.freeze_domain(&test_domain_id()));
        assert!(mgr.retire_domain(&test_domain_id()));
        assert_eq!(mgr.retired_domain_count(), 1);
    }

    #[test]
    fn ordering_domain_mgr_get_domain() {
        let mgr = OrderingDomainMgr::default();
        mgr.create_domain(test_dataset_id(), test_domain_id(), 1000).unwrap();
        let entry = mgr.get_domain(&test_domain_id()).unwrap();
        assert_eq!(entry.domain_id, test_domain_id());
        assert_eq!(mgr.get_domain(&OrderingDomain::new([2; 32])), None);
    }

    // === DomainSequencer tests ===
    #[test]
    fn domain_sequencer_advance() {
        let seq = DomainSequencer::default();
        seq.initialize_position(test_domain_id(), OrderingPos::new(100));
        let pos = seq.advance(test_domain_id(), 10).unwrap();
        assert_eq!(pos, OrderingPos::new(110));
        assert_eq!(seq.get_position(&test_domain_id()), OrderingPos::new(110));
    }

    #[test]
    fn domain_sequencer_advance_one() {
        let seq = DomainSequencer::default();
        seq.initialize_position(test_domain_id(), OrderingPos::new(0));
        let pos = seq.advance_one(test_domain_id()).unwrap();
        assert_eq!(pos, OrderingPos::new(1));
        assert_eq!(seq.advance_count(), 1);
        assert_eq!(seq.total_advances(), 1);
    }

    #[test]
    fn domain_sequencer_is_monotonic() {
        let seq = DomainSequencer::default();
        seq.initialize_position(test_domain_id(), OrderingPos::new(100));
        assert!(seq.is_monotonic(test_domain_id(), OrderingPos::new(101)));
        assert!(!seq.is_monotonic(test_domain_id(), OrderingPos::new(100)));
        assert!(!seq.is_monotonic(test_domain_id(), OrderingPos::new(50)));
    }

    #[test]
    fn domain_sequencer_counts() {
        let seq = DomainSequencer::default();
        seq.initialize_position(test_domain_id(), OrderingPos::new(0));
        seq.advance(test_domain_id(), 5).unwrap();
        assert_eq!(seq.advance_count(), 1);
        assert_eq!(seq.total_advances(), 5);
        assert_eq!(seq.tracked_domain_count(), 1);
    }

    // === OrderingClock tests ===
    #[test]
    fn ordering_clock_tick() {
        let oc = OrderingClock::default();
        assert_eq!(oc.tick(test_domain_id(), 1000).unwrap(), 1000);
        assert_eq!(oc.get_time(&test_domain_id()), 1000);
        assert_eq!(oc.tick_count(), 1);
    }

    #[test]
    fn ordering_clock_drift_reject() {
        let oc = OrderingClock::new(ClockDriftPolicy::Reject, 5000);
        oc.tick(test_domain_id(), 1000).unwrap();
        let err = oc.tick(test_domain_id(), 500).unwrap_err();
        assert!(matches!(err, OrderingError::InvalidState { .. }));
        assert_eq!(oc.drift_rejection_count(), 1);
    }

    #[test]
    fn ordering_clock_drift_allow() {
        let oc = OrderingClock::new(ClockDriftPolicy::Allow, 5000);
        oc.tick(test_domain_id(), 1000).unwrap();
        assert_eq!(oc.tick(test_domain_id(), 500).unwrap(), 500);
        assert_eq!(oc.drift_rejection_count(), 0);
    }

    #[test]
    fn ordering_clock_drift_warn() {
        let oc = OrderingClock::new(ClockDriftPolicy::Warn, 5000);
        oc.tick(test_domain_id(), 1000).unwrap();
        assert_eq!(oc.tick(test_domain_id(), 500).unwrap(), 500);
        assert_eq!(oc.drift_rejection_count(), 1);
    }

    #[test]
    fn ordering_clock_policy_as_str() {
        assert_eq!(ClockDriftPolicy::Reject.as_str(), "reject");
        assert_eq!(ClockDriftPolicy::Warn.as_str(), "warn");
        assert_eq!(ClockDriftPolicy::Allow.as_str(), "allow");
    }

    // === CausalOrderTracker tests ===
    #[test]
    fn causal_order_tracker_record_valid() {
        let cot = CausalOrderTracker::default();
        assert!(cot.record_dependency(test_domain_id(), OrderingPos::new(10), OrderingPos::new(20)).is_ok());
        assert_eq!(cot.dependency_count(), 1);
        assert_eq!(cot.violation_count(), 0);
    }

    #[test]
    fn causal_order_tracker_record_violation() {
        let cot = CausalOrderTracker::default();
        let err = cot.record_dependency(test_domain_id(), OrderingPos::new(20), OrderingPos::new(10)).unwrap_err();
        assert!(matches!(err, OrderingError::CausalViolation { .. }));
        assert_eq!(cot.violation_count(), 1);
    }

    #[test]
    fn causal_order_tracker_check() {
        let cot = CausalOrderTracker::default();
        assert!(cot.check_causal_order(test_domain_id(), OrderingPos::new(20), OrderingPos::new(10)).unwrap());
        let err = cot.check_causal_order(test_domain_id(), OrderingPos::new(10), OrderingPos::new(20)).unwrap_err();
        assert!(matches!(err, OrderingError::CausalViolation { .. }));
    }

    #[test]
    fn causal_order_tracker_counts() {
        let cot = CausalOrderTracker::default();
        cot.record_dependency(test_domain_id(), OrderingPos::new(1), OrderingPos::new(2)).unwrap();
        cot.record_dependency(test_domain_id(), OrderingPos::new(2), OrderingPos::new(3)).unwrap();
        assert_eq!(cot.dependency_count(), 2);
        assert_eq!(cot.causal_violation_rate(), 0.0);
        assert_eq!(cot.tracked_domain_count(), 1);
    }

    // === TotalOrderEntry tests ===
    #[test]
    fn total_order_entry_new() {
        let entry = TotalOrderEntry::new(
            OrderingPos::new(100),
            test_record_id(),
            test_domain_id(),
            5000,
            1,
        );
        assert_eq!(entry.position, OrderingPos::new(100));
        assert_eq!(entry.record_id, test_record_id());
        assert_eq!(entry.domain, test_domain_id());
        assert_eq!(entry.sequence_number, 1);
    }

    // === TotalOrderSequencer tests ===
    #[test]
    fn total_order_sequencer_sequence_and_get() {
        let tos = TotalOrderSequencer::default();
        tos.sequence(test_domain_id(), OrderingPos::new(1), test_record_id(), 1000).unwrap();
        assert_eq!(tos.sequence_count(), 1);
        assert_eq!(tos.get_record_at(OrderingPos::new(1)), Some(test_record_id()));
        assert_eq!(tos.get_position_for_record(&test_record_id()), Some(OrderingPos::new(1)));
    }

    #[test]
    fn total_order_sequencer_duplicate_position() {
        let tos = TotalOrderSequencer::default();
        tos.sequence(test_domain_id(), OrderingPos::new(1), test_record_id(), 1000).unwrap();
        let err = tos.sequence(test_domain_id(), OrderingPos::new(1), RecordId::new([2; 32]), 2000).unwrap_err();
        assert!(matches!(err, OrderingError::PositionOutOfOrder { .. }));
    }

    #[test]
    fn total_order_sequencer_first_last() {
        let tos = TotalOrderSequencer::default();
        tos.sequence(test_domain_id(), OrderingPos::new(5), test_record_id(), 1000).unwrap();
        tos.sequence(test_domain_id(), OrderingPos::new(10), RecordId::new([2; 32]), 2000).unwrap();
        assert_eq!(tos.first_position(), Some(OrderingPos::new(5)));
        assert_eq!(tos.last_position(), Some(OrderingPos::new(10)));
        assert_eq!(tos.entry_count(), 2);
    }

    // === SnapshotIsolationOrdering tests ===
    #[test]
    fn snapshot_isolation_create_and_get() {
        let sio = SnapshotIsolationOrdering::default();
        let snap = sio.create_snapshot(test_domain_id(), OrderingPos::new(100), test_ids_time()).unwrap();
        assert_eq!(snap.domain, test_domain_id());
        assert_eq!(snap.position, OrderingPos::new(100));
        assert_eq!(sio.snapshot_count(), 1);
        assert_eq!(sio.get_latest_snapshot(&test_domain_id()), Some(snap));
    }

    #[test]
    fn snapshot_isolation_restore() {
        let sio = SnapshotIsolationOrdering::default();
        let snap = sio.create_snapshot(test_domain_id(), OrderingPos::new(100), test_ids_time()).unwrap();
        assert!(sio.restore_snapshot(test_domain_id(), snap));
        assert_eq!(sio.restored_count(), 1);
    }

    #[test]
    fn snapshot_isolation_counts() {
        let sio = SnapshotIsolationOrdering::default();
        sio.create_snapshot(test_domain_id(), OrderingPos::new(100), test_ids_time()).unwrap();
        sio.create_snapshot(OrderingDomain::new([2; 32]), OrderingPos::new(200), test_ids_time()).unwrap();
        assert_eq!(sio.snapshot_count(), 2);
        assert_eq!(sio.snapshot_domain_count(), 2);
    }

    // === VectorClock tests ===
    #[test]
    fn vector_clock_tick_and_get() {
        let vc = VectorClock::default();
        assert_eq!(vc.tick(test_node_id()), 1);
        assert_eq!(vc.tick(test_node_id()), 2);
        assert_eq!(vc.get(&test_node_id()), 2);
        assert_eq!(vc.get(&NodeId::new([2; 32])), 0);
    }

    #[test]
    fn vector_clock_merge() {
        let vc1 = VectorClock::default();
        vc1.tick(test_node_id());
        vc1.tick(test_node_id());
        let mut vc2_map = std::collections::HashMap::new();
        vc2_map.insert(NodeId::new([2; 32]), 3);
        vc1.merge(&vc2_map);
        assert_eq!(vc1.get(&test_node_id()), 2);
        assert_eq!(vc1.get(&NodeId::new([2; 32])), 3);
        assert_eq!(vc1.merge_count(), 1);
    }

    #[test]
    fn vector_clock_happens_before() {
        let vc1 = VectorClock::default();
        vc1.tick(test_node_id());
        let vc2 = VectorClock::default();
        vc2.tick(test_node_id());
        vc2.tick(NodeId::new([2; 32]));
        assert!(vc1.happens_before(&vc2));
        assert!(!vc2.happens_before(&vc1));
        assert!(vc1.is_concurrent(&vc2) == false);
    }

    #[test]
    fn vector_clock_concurrent() {
        let vc1 = VectorClock::default();
        vc1.tick(test_node_id());
        let vc2 = VectorClock::default();
        vc2.tick(NodeId::new([2; 32]));
        assert!(vc1.is_concurrent(&vc2));
    }

    // === OrderingStats tests ===
    #[test]
    fn ordering_stats_domain_efficiency() {
        let mut s = OrderingStats::default();
        s.total_domains = 10;
        s.total_positions = 10000;
        assert!((s.domain_efficiency() - 1.0).abs() < 0.01);
    }

    #[test]
    fn ordering_stats_causal_violation_rate() {
        let mut s = OrderingStats::default();
        s.total_positions = 100;
        s.total_causal_violations = 5;
        assert!((s.causal_violation_rate() - 0.05).abs() < 0.01);
    }

    #[test]
    fn ordering_stats_default() {
        let s = OrderingStats::default();
        assert_eq!(s.total_domains, 0);
        assert_eq!(s.domain_efficiency(), 1.0);
        assert_eq!(s.causal_violation_rate(), 0.0);
    }

    // === OrderingMonitor tests ===
    #[test]
    fn ordering_monitor_report_health() {
        let om = OrderingMonitor::default();
        om.report_health(test_domain_id(), true);
        assert!(om.is_healthy(&test_domain_id()));
        assert_eq!(om.health_check_count(), 1);
        assert_eq!(om.alert_count(), 0);
    }

    #[test]
    fn ordering_monitor_alert() {
        let om = OrderingMonitor::default();
        om.report_health(test_domain_id(), true);
        om.report_health(test_domain_id(), false);
        assert_eq!(om.alert_count(), 1);
        assert!(!om.is_healthy(&test_domain_id()));
        assert_eq!(om.monitored_count(), 1);
        assert_eq!(om.unhealthy_count(), 1);
    }

    // === ValidationResult tests ===
    #[test]
    fn validation_result_as_str() {
        assert_eq!(ValidationResult::Valid.as_str(), "valid");
        assert_eq!(ValidationResult::InvalidPosition.as_str(), "invalid_position");
        assert_eq!(ValidationResult::InvalidDomain.as_str(), "invalid_domain");
        assert_eq!(ValidationResult::CausalViolation.as_str(), "causal_violation");
        assert!(ValidationResult::Valid.is_ok());
        assert!(!ValidationResult::InvalidPosition.is_ok());
    }

    // === OrderingValidator tests ===
    #[test]
    fn ordering_validator_validate_position() {
        let ov = OrderingValidator::default();
        assert_eq!(
            ov.validate_position(test_domain_id(), OrderingPos::new(10), OrderingPos::new(5)),
            ValidationResult::Valid
        );
        assert_eq!(
            ov.validate_position(test_domain_id(), OrderingPos::new(3), OrderingPos::new(5)),
            ValidationResult::InvalidPosition
        );
        assert_eq!(ov.validation_count(), 2);
        assert_eq!(ov.valid_count(), 1);
        assert_eq!(ov.invalid_count(), 1);
        assert!((ov.valid_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn ordering_validator_validate_domain() {
        let ov = OrderingValidator::default();
        assert_eq!(ov.validate_domain(test_domain_id(), true), ValidationResult::Valid);
        assert_eq!(ov.validate_domain(OrderingDomain::new([2; 32]), false), ValidationResult::InvalidDomain);
        assert_eq!(ov.valid_rate(), 0.5);
    }

    // === OrderingEngine tests ===
    #[test]
    fn ordering_engine_create_domain() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        assert_eq!(engine.domain_count(), 1);
        assert_eq!(engine.get_domain(&test_dataset_id()), Some(test_domain_id()));
        assert_eq!(engine.stats().total_domains, 1);
    }

    #[test]
    fn ordering_engine_create_domain_duplicate() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        let err = engine.create_domain(test_dataset_id(), test_domain_id()).unwrap_err();
        assert!(matches!(err, OrderingError::DomainAlreadyExists { .. }));
    }

    #[test]
    fn ordering_engine_advance_position() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        let pos = engine.advance_position(test_domain_id()).unwrap();
        assert_eq!(pos, OrderingPos::new(1));
        assert_eq!(engine.current_position(&test_domain_id()), OrderingPos::new(1));
        assert_eq!(engine.position_count(), 1);
    }

    #[test]
    fn ordering_engine_advance_domain_not_found() {
        let engine = OrderingEngine::default();
        let err = engine.advance_position(test_domain_id()).unwrap_err();
        assert!(matches!(err, OrderingError::DomainNotFound { .. }));
    }

    #[test]
    fn ordering_engine_happens_before() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        assert!(engine.happens_before(test_domain_id(), OrderingPos::new(1), OrderingPos::new(2)).unwrap());
        assert!(!engine.happens_before(test_domain_id(), OrderingPos::new(2), OrderingPos::new(1)).unwrap());
    }

    #[test]
    fn ordering_engine_record_total_order() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        engine.record_total_order(test_domain_id(), OrderingPos::new(1), test_record_id()).unwrap();
        assert_eq!(engine.record_at_position(test_domain_id(), OrderingPos::new(1)).unwrap(), Some(test_record_id()));
        assert_eq!(engine.stats().total_sequences, 1);
    }

    #[test]
    fn ordering_engine_create_snapshot() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        engine.advance_position(test_domain_id()).unwrap();
        let snap = engine.create_snapshot(test_domain_id(), test_ids_time()).unwrap();
        assert_eq!(snap.domain, test_domain_id());
        assert_eq!(engine.stats().total_snapshots, 1);
    }

    #[test]
    fn ordering_engine_stats_and_name() {
        let engine = OrderingEngine::default();
        assert_eq!(engine.ordering_name(), "ordering_engine");
        assert_eq!(engine.domain_count(), 0);
        assert_eq!(engine.position_count(), 0);
    }

    #[test]
    fn ordering_engine_validate() {
        let engine = OrderingEngine::default();
        assert_eq!(engine.validate_position(test_domain_id(), OrderingPos::new(10), OrderingPos::new(5)), ValidationResult::Valid);
        assert_eq!(engine.validate_domain_exists(test_domain_id()), ValidationResult::InvalidDomain);
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        assert_eq!(engine.validate_domain_exists(test_domain_id()), ValidationResult::Valid);
    }

    #[test]
    fn ordering_engine_causal_dependency() {
        let engine = OrderingEngine::default();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        engine.record_causal_dependency(test_domain_id(), OrderingPos::new(10), OrderingPos::new(20)).unwrap();
        assert_eq!(engine.stats().total_causal_violations, 0);
    }

    #[test]
    fn ordering_engine_clock_and_vector_clock() {
        let engine = OrderingEngine::default();
        engine.tick_clock(test_domain_id(), 5000).unwrap();
        assert_eq!(engine.vector_clock_tick(test_node_id()), 1);
        let mut map = std::collections::HashMap::new();
        map.insert(NodeId::new([2; 32]), 5);
        engine.vector_clock_merge(&map);
        assert_eq!(engine.vector_clock_tick(test_node_id()), 2);
    }

    #[test]
    fn ordering_engine_monitor() {
        let engine = OrderingEngine::default();
        engine.monitor_health(test_domain_id(), false);
        assert!(!engine.is_domain_healthy(&test_domain_id()));
    }

    // === OrderingPos tests ===
    #[test]
    fn ordering_pos_next_prev() {
        assert_eq!(OrderingPos::new(5).next(), OrderingPos::new(6));
        assert_eq!(OrderingPos::new(5).prev(), OrderingPos::new(4));
        assert_eq!(OrderingPos::new(0).prev(), OrderingPos::new(0));
    }

    // === OrderingDomain tests ===
    #[test]
    fn ordering_domain_is_zero() {
        assert!(OrderingDomain::new([0; 32]).is_zero());
        assert!(!test_domain_id().is_zero());
    }

    // === OrderingConflictResolver tests ===
    #[test]
    fn conflict_resolver_detect_and_resolve() {
        let cr = OrderingConflictResolver::default();
        let pos = cr.detect_conflict(
            test_domain_id(),
            OrderingPos::new(10),
            OrderingPos::new(15),
            RecordId::new([1; 32]),
            RecordId::new([2; 32]),
            5000,
        );
        assert_eq!(pos, OrderingPos::new(15));
        assert_eq!(cr.conflict_count(), 1);
        assert_eq!(cr.unresolved_count(), 1);
    }

    #[test]
    fn conflict_resolver_resolve() {
        let cr = OrderingConflictResolver::default();
        cr.detect_conflict(
            test_domain_id(),
            OrderingPos::new(10),
            OrderingPos::new(15),
            RecordId::new([1; 32]),
            RecordId::new([2; 32]),
            5000,
        );
        assert!(cr.resolve_conflict(OrderingPos::new(15), ConflictResolutionOutcome::PositionWins));
        assert_eq!(cr.resolved_count(), 1);
        assert_eq!(cr.unresolved_count(), 0);
        assert!((cr.resolution_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn conflict_resolver_auto_resolve_by_position() {
        let cr = OrderingConflictResolver::default();
        cr.detect_conflict(
            test_domain_id(),
            OrderingPos::new(10),
            OrderingPos::new(15),
            RecordId::new([1; 32]),
            RecordId::new([2; 32]),
            5000,
        );
        assert!(cr.auto_resolve_by_position(OrderingPos::new(15)));
        assert_eq!(cr.auto_resolved_count(), 1);
        assert_eq!(cr.manual_resolved_count(), 0);
    }

    #[test]
    fn conflict_resolver_active_conflict_count() {
        let cr = OrderingConflictResolver::default();
        cr.detect_conflict(
            test_domain_id(),
            OrderingPos::new(10),
            OrderingPos::new(15),
            RecordId::new([1; 32]),
            RecordId::new([2; 32]),
            5000,
        );
        cr.detect_conflict(
            test_domain_id(),
            OrderingPos::new(20),
            OrderingPos::new(25),
            RecordId::new([3; 32]),
            RecordId::new([4; 32]),
            6000,
        );
        assert_eq!(cr.active_conflict_count(), 2);
        cr.resolve_conflict(OrderingPos::new(15), ConflictResolutionOutcome::PositionWins);
        assert_eq!(cr.active_conflict_count(), 1);
    }

    // === OrderingBarrier tests ===
    #[test]
    fn ordering_barrier_arrive_and_reach() {
        let barrier = OrderingBarrier::new(test_domain_id(), OrderingPos::new(100));
        assert!(!barrier.is_reached());
        assert_eq!(barrier.progress_percent(), 0);
        assert!(barrier.arrive(OrderingPos::new(50), 1000));
        assert!(!barrier.is_reached());
        assert_eq!(barrier.progress_percent(), 50);
        assert!(barrier.arrive(OrderingPos::new(100), 2000));
        assert!(barrier.is_reached());
        assert_eq!(barrier.progress_percent(), 100);
    }

    #[test]
    fn ordering_barrier_break() {
        let barrier = OrderingBarrier::new(test_domain_id(), OrderingPos::new(100));
        barrier.break_barrier(5000);
        assert!(barrier.is_broken());
        assert!(!barrier.is_reached());
        assert!(!barrier.arrive(OrderingPos::new(200), 6000));
    }

    #[test]
    fn ordering_barrier_participants() {
        let barrier = OrderingBarrier::new(test_domain_id(), OrderingPos::new(100));
        barrier.register_participant();
        barrier.register_participant();
        barrier.register_participant();
        assert_eq!(barrier.participant_count(), 3);
        assert_eq!(barrier.arrived_count(), 0);
        barrier.arrive(OrderingPos::new(100), 1000);
        assert_eq!(barrier.arrived_count(), 1);
    }

    #[test]
    fn ordering_barrier_state_as_str() {
        assert_eq!(BarrierState::Waiting.as_str(), "waiting");
        assert_eq!(BarrierState::Reached.as_str(), "reached");
        assert_eq!(BarrierState::Broken.as_str(), "broken");
        assert!(BarrierState::Reached.is_terminal());
        assert!(BarrierState::Broken.is_terminal());
        assert!(!BarrierState::Waiting.is_terminal());
    }

    // === OrderingCheckpoint tests ===
    #[test]
    fn ordering_checkpoint_create_and_verify() {
        let mgr = OrderingCheckpointMgr::default();
        let cp = mgr.create_checkpoint(test_domain_id(), OrderingPos::new(100), 5000);
        assert_eq!(cp.position, OrderingPos::new(100));
        assert!(!cp.is_verified());
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    #[test]
    fn ordering_checkpoint_verify_and_restore() {
        let mgr = OrderingCheckpointMgr::default();
        let cp = mgr.create_checkpoint(test_domain_id(), OrderingPos::new(100), 5000);
        assert!(mgr.verify_checkpoint(&test_domain_id()));
        assert_eq!(mgr.verified_count(), 1);
        let restored = mgr.restore_checkpoint(&test_domain_id()).unwrap();
        assert_eq!(restored.position, OrderingPos::new(100));
        assert_eq!(mgr.restored_count(), 1);
    }

    #[test]
    fn ordering_checkpoint_restore_without_verify() {
        let mgr = OrderingCheckpointMgr::default();
        mgr.create_checkpoint(test_domain_id(), OrderingPos::new(100), 5000);
        assert!(mgr.restore_checkpoint(&test_domain_id()).is_none());
        assert_eq!(mgr.restored_count(), 0);
    }

    #[test]
    fn ordering_checkpoint_tracked_count() {
        let mgr = OrderingCheckpointMgr::default();
        mgr.create_checkpoint(test_domain_id(), OrderingPos::new(100), 5000);
        mgr.create_checkpoint(OrderingDomain::new([2; 32]), OrderingPos::new(200), 6000);
        assert_eq!(mgr.tracked_count(), 2);
    }

    // === ReplicaOrderingInfo tests ===
    #[test]
    fn replica_info_new_and_update() {
        let info = ReplicaOrderingInfo::new(test_node_id(), OrderingPos::new(50), 1000);
        assert_eq!(info.position, OrderingPos::new(50));
        assert!(info.is_synced());
    }

    #[test]
    fn replica_info_lag_states() {
        let mut info = ReplicaOrderingInfo::new(test_node_id(), OrderingPos::new(50), 1000);
        info.update_position(OrderingPos::new(100), 5000);
        assert!(matches!(info.state, ReplicaSyncState::Lagging));
        info.update_position(OrderingPos::new(200), 20000);
        assert!(matches!(info.state, ReplicaSyncState::Behind));
    }

    #[test]
    fn replica_info_disconnect_reconnect() {
        let mut info = ReplicaOrderingInfo::new(test_node_id(), OrderingPos::new(50), 1000);
        info.disconnect();
        assert!(matches!(info.state, ReplicaSyncState::Disconnected));
        info.reconnect(5000);
        assert!(matches!(info.state, ReplicaSyncState::Synced));
    }

    #[test]
    fn replica_sync_state_as_str() {
        assert_eq!(ReplicaSyncState::Synced.as_str(), "synced");
        assert_eq!(ReplicaSyncState::Lagging.as_str(), "lagging");
        assert_eq!(ReplicaSyncState::Behind.as_str(), "behind");
        assert_eq!(ReplicaSyncState::Disconnected.as_str(), "disconnected");
        assert!(ReplicaSyncState::Synced.is_synced());
        assert!(!ReplicaSyncState::Behind.is_synced());
    }

    // === OrderingReplicator tests ===
    #[test]
    fn replicator_register_and_update() {
        let rep = OrderingReplicator::default();
        rep.register_replica(test_node_id(), OrderingPos::new(50), 1000);
        assert_eq!(rep.replica_count(), 1);
        assert_eq!(rep.sync_count(), 1);
        let state = rep.update_replica(test_node_id(), OrderingPos::new(100), 2000);
        assert_eq!(state, Some(ReplicaSyncState::Synced));
    }

    #[test]
    fn replicator_disconnect_reconnect() {
        let rep = OrderingReplicator::default();
        rep.register_replica(test_node_id(), OrderingPos::new(50), 1000);
        assert!(rep.disconnect_replica(&test_node_id()));
        assert!(matches!(rep.get_replica_state(&test_node_id()), Some(ReplicaSyncState::Disconnected)));
        assert_eq!(rep.disconnection_count(), 1);
        assert!(rep.reconnect_replica(&test_node_id(), 5000));
        assert_eq!(rep.get_replica_state(&test_node_id()), Some(ReplicaSyncState::Synced));
    }

    #[test]
    fn replicator_counts() {
        let rep = OrderingReplicator::default();
        rep.register_replica(test_node_id(), OrderingPos::new(50), 1000);
        rep.register_replica(NodeId::new([2; 32]), OrderingPos::new(60), 2000);
        assert_eq!(rep.replica_count(), 2);
        assert_eq!(rep.synced_count(), 2);
        assert_eq!(rep.lagging_count(), 0);
        assert_eq!(rep.disconnected_count(), 0);
    }

    // === OrderingCompactor tests ===
    #[test]
    fn compactor_should_and_compact() {
        let mut comp = OrderingCompactor::new(1000);
        assert!(!comp.should_compact(OrderingPos::new(500)));
        assert!(comp.should_compact(OrderingPos::new(2000)));
        let c = comp.compact(OrderingPos::new(2000), 100);
        assert!(c > 0);
        assert_eq!(comp.compaction_count(), 1);
        assert_eq!(comp.last_compacted_position(), OrderingPos::new(2000));
    }

    #[test]
    fn compactor_no_compact_below_threshold() {
        let mut comp = OrderingCompactor::new(10000);
        assert_eq!(comp.compact(OrderingPos::new(5000), 100), 0);
        assert_eq!(comp.compaction_count(), 0);
    }

    #[test]
    fn compactor_counts() {
        let mut comp = OrderingCompactor::new(1000);
        comp.compact(OrderingPos::new(2000), 100);
        comp.compact(OrderingPos::new(4000), 100);
        assert_eq!(comp.compaction_count(), 2);
        assert!(comp.entries_compacted() > 0);
    }

    // === LeaseState tests ===
    #[test]
    fn lease_state_as_str() {
        assert_eq!(LeaseState::Pending.as_str(), "pending");
        assert_eq!(LeaseState::Active.as_str(), "active");
        assert_eq!(LeaseState::Expired.as_str(), "expired");
        assert_eq!(LeaseState::Revoked.as_str(), "revoked");
        assert!(LeaseState::Active.is_active());
        assert!(!LeaseState::Pending.is_active());
        assert!(LeaseState::Expired.is_terminal());
        assert!(LeaseState::Revoked.is_terminal());
    }

    // === OrderingLease tests ===
    #[test]
    fn ordering_lease_activate_and_renew() {
        let lease = OrderingLease::new(test_domain_id(), test_node_id(), test_domain_id(), 1000, 60000);
        lease.activate();
        assert_eq!(lease.state(), LeaseState::Active);
        assert!(lease.is_valid(2000));
        assert!(lease.renew(2000, 60000));
        assert_eq!(lease.renewal_count(), 1);
    }

    #[test]
    fn ordering_lease_expire_and_revoke() {
        let lease = OrderingLease::new(test_domain_id(), test_node_id(), test_domain_id(), 1000, 60000);
        lease.activate();
        lease.expire();
        assert_eq!(lease.state(), LeaseState::Expired);
        assert!(!lease.is_valid(2000));
        assert!(!lease.renew(2000, 60000));
    }

    #[test]
    fn ordering_lease_time_to_expiry() {
        let lease = OrderingLease::new(test_domain_id(), test_node_id(), test_domain_id(), 1000, 60000);
        lease.activate();
        assert_eq!(lease.time_to_expiry(1000), 60000);
        assert_eq!(lease.time_to_expiry(31000), 30000);
        assert_eq!(lease.time_to_expiry(70000), 0);
    }

    // === PartitionOrderingState tests ===
    #[test]
    fn partition_state_as_str() {
        assert_eq!(PartitionOrderingState::Forming.as_str(), "forming");
        assert_eq!(PartitionOrderingState::Ordered.as_str(), "ordered");
        assert_eq!(PartitionOrderingState::Recovering.as_str(), "recovering");
        assert_eq!(PartitionOrderingState::Failed.as_str(), "failed");
        assert!(PartitionOrderingState::Ordered.is_ordered());
        assert!(!PartitionOrderingState::Forming.is_ordered());
    }

    // === OrderingPartition tests ===
    #[test]
    fn ordering_partition_order_and_quorum() {
        let part = OrderingPartition::new(PartitionId::new([1; 32]), test_domain_id(), 3);
        assert!(!part.is_ordered());
        assert!(!part.has_quorum());
        part.add_member();
        part.add_member();
        assert!(!part.has_quorum());
        part.add_member();
        assert!(part.has_quorum());
        part.order(OrderingPos::new(100));
        assert!(part.is_ordered());
    }

    #[test]
    fn ordering_partition_fail_and_recover() {
        let part = OrderingPartition::new(PartitionId::new([1; 32]), test_domain_id(), 1);
        part.fail();
        assert_eq!(part.state(), PartitionOrderingState::Failed);
        part.recover();
        assert_eq!(part.state(), PartitionOrderingState::Recovering);
        part.order(OrderingPos::new(50));
        assert!(part.is_ordered());
    }

    // === Extended OrderingEngine tests ===
    #[test]
    fn ordering_engine_conflict_detection_and_resolution() {
        let engine = OrderingEngine::new_with_features();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        let pos = engine.detect_conflict(
            test_domain_id(),
            OrderingPos::new(10),
            OrderingPos::new(15),
            RecordId::new([1; 32]),
            RecordId::new([2; 32]),
            5000,
        );
        assert_eq!(pos, OrderingPos::new(15));
        assert_eq!(engine.conflict_count(), 1);
        assert!(engine.auto_resolve_by_position(OrderingPos::new(15)));
        assert_eq!(engine.active_conflict_count(), 0);
    }

    #[test]
    fn ordering_engine_barrier() {
        let engine = OrderingEngine::new_with_features();
        engine.create_barrier(test_domain_id(), OrderingPos::new(100));
        engine.register_barrier_participant();
        assert!(!engine.barrier_reached());
        assert_eq!(engine.barrier_progress(), 0);
        assert!(engine.barrier_arrive(OrderingPos::new(100), 1000));
        assert!(engine.barrier_reached());
        assert_eq!(engine.barrier_progress(), 100);
    }

    #[test]
    fn ordering_engine_checkpoint() {
        let engine = OrderingEngine::new_with_features();
        let cp = engine.create_ordering_checkpoint(test_domain_id(), OrderingPos::new(100), 5000);
        assert_eq!(cp.position, OrderingPos::new(100));
        assert!(engine.verify_checkpoint(&test_domain_id()));
        let restored = engine.restore_checkpoint(&test_domain_id()).unwrap();
        assert_eq!(restored.position, OrderingPos::new(100));
        assert_eq!(engine.checkpoint_count(), 1);
    }

    #[test]
    fn ordering_engine_replica() {
        let engine = OrderingEngine::new_with_features();
        engine.register_replica(test_node_id(), OrderingPos::new(50), 1000);
        assert_eq!(engine.replica_count(), 1);
        assert_eq!(engine.synced_replica_count(), 1);
        let state = engine.update_replica_position(test_node_id(), OrderingPos::new(100), 5000);
        assert_eq!(state, Some(ReplicaSyncState::Synced));
        assert_eq!(engine.replica_state(&test_node_id()), Some(ReplicaSyncState::Synced));
    }

    #[test]
    fn ordering_engine_compact() {
        let mut engine = OrderingEngine::new_with_features();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();
        let c = engine.compact(OrderingPos::new(20000), 100);
        assert!(c > 0);
        assert_eq!(engine.compaction_count(), 1);
    }

    #[test]
    fn ordering_engine_lease() {
        let engine = OrderingEngine::new_with_features();
        let lease = engine.create_lease(test_domain_id(), test_node_id(), test_domain_id(), 1000, 60000);
        assert_eq!(lease.state(), LeaseState::Pending);
        engine.activate_lease();
        assert_eq!(engine.lease_state(), LeaseState::Active);
        assert!(engine.lease_is_valid(2000));
        assert!(engine.renew_lease(2000, 60000));
    }

    #[test]
    fn ordering_engine_partition() {
        let engine = OrderingEngine::new_with_features();
        let part = engine.create_ordering_partition(PartitionId::new([1; 32]), test_domain_id(), 2);
        assert_eq!(part.state(), PartitionOrderingState::Forming);
        engine.add_partition_member(PartitionId::new([1; 32]));
        engine.add_partition_member(PartitionId::new([1; 32]));
        assert!(engine.partition_has_quorum(PartitionId::new([1; 32])));
        engine.order_partition(PartitionId::new([1; 32]), OrderingPos::new(100));
        assert_eq!(engine.partition_state(PartitionId::new([1; 32])), Some(PartitionOrderingState::Ordered));
    }

    #[test]
    fn ordering_engine_stats() {
        let engine = OrderingEngine::new_with_features();
        let s = engine.stats();
        assert_eq!(s.total_domains, 0);
        assert_eq!(s.domain_efficiency(), 1.0);
    }

    #[test]
    fn ordering_engine_new_vs_default() {
        let default_engine = OrderingEngine::default();
        let features_engine = OrderingEngine::new_with_features();
        assert_eq!(default_engine.domain_count(), 0);
        assert_eq!(features_engine.domain_count(), 0);
    }

    // === Integration: full ordering flow ===
    #[test]
    fn ordering_full_flow() {
        let engine = OrderingEngine::new_with_features();
        engine.create_domain(test_dataset_id(), test_domain_id()).unwrap();

        // Advance positions and record total order
        engine.advance_position(test_domain_id()).unwrap();
        engine.advance_position(test_domain_id()).unwrap();
        engine.record_total_order(test_domain_id(), OrderingPos::new(1), test_record_id()).unwrap();

        // Check causal ordering
        assert!(engine
            .happens_before(test_domain_id(), OrderingPos::new(1), OrderingPos::new(2))
            .unwrap());

        // Create snapshot
        engine.create_snapshot(test_domain_id(), test_ids_time()).unwrap();

        // Record causal dependency
        engine
            .record_causal_dependency(test_domain_id(), OrderingPos::new(1), OrderingPos::new(2))
            .unwrap();

        // Monitor health
        engine.monitor_health(test_domain_id(), true);
        assert!(engine.is_domain_healthy(&test_domain_id()));

        // Validate
        assert_eq!(
            engine.validate_domain_exists(test_domain_id()),
            ValidationResult::Valid
        );

        // Register replica
        engine.register_replica(test_node_id(), OrderingPos::new(1), 1000);
        assert_eq!(engine.replica_count(), 1);

        // Check stats
        let s = engine.stats();
        assert!(s.total_domains >= 1);
        assert!(s.total_sequences >= 1);
        assert!(s.total_positions >= 2);
    }
}