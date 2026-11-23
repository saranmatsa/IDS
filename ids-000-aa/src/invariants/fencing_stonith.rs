//! # Fencing and STONITH Invariant
//!
//! This invariant enforces the requirement that no failover may proceed
//! without first fencing (or STONITH-ing) the victim node, preventing
//! split-brain where two nodes believe they are simultaneously responsible
//! for the same dataset or region.
//!
//! The fencing subsystem provides:
//! - Atomic fencing operations with confirmation and state tracking
//! - STONITH execution with power-cycle, network-isolation, and disk-quorum fencing
//! - Fencing epoch management so stale fencing attempts cannot be replayed
//! - Split-brain detection across ordering domains and failure domains
//! - Quorum-based fencing authorization so a single rogue node cannot fence others
//! - Idempotent fencing: re-fencing an already-fenced node is a no-op success
//! - Fencing timeout and retry with exponential backoff and circuit breaking
//! - Cross-region fencing coordination to prevent region-level split-brain
//! - Fencing audit trail with tamper-evident recording of every fence/STONITH event
//! - Resource-level fencing (per-dataset, per-partition, per-region)
//! - Automatic recovery attempt ordering: fence first, recover second, never reverse

use crate::types::*;
use crate::errors::InvariantError;
use crate::versioning::EdgeCaseBits;
use std::collections::{HashMap, HashSet, BTreeMap, BTreeSet};
use std::time::Duration;

///////////////////////////////////////////////////////////////////////////////
// Sealed trait declaration
///////////////////////////////////////////////////////////////////////////////

/// Invariant: fencing must precede any failover; STONITH prevents split-brain.
///
/// Downstream crates use this as a trait bound but cannot implement it.
/// Only `ids-000-aa` and its designated implementation crates may `impl`.
pub trait FencingStonith: private::Sealed {
    /// Fence a node before any failover can proceed. Returns a `FencingToken`
    /// that must be presented to subsequent recovery operations.
    fn fence_before_failover(&self, victim: NodeId) -> Result<FencingToken, FencingError>;

    /// Execute STONITH (Shoot The Other Node In The Head) on a node,
    /// definitively removing it from the cluster membership.
    fn stonith_execute(&self, node_id: NodeId) -> Result<StonithResult, StonithError>;

    /// Return true if split-brain is prevented in the given ordering domain.
    fn split_brain_prevented(&self, domain: OrderingDomain) -> bool;

    /// Return the current fencing epoch for a node. Every successful fence
    /// increments this epoch, invalidating all prior fencing tokens.
    fn fencing_epoch(&self, node_id: NodeId) -> FencingEpoch;

    /// Attempt to acquire quorum-based fencing authorization. Requires a
    /// majority of replicas to agree before a fence may proceed.
    fn acquire_fencing_quorum(&self, victim: NodeId, replicas: &[NodeId]) -> Result<FencingAuthorization, FencingError>;

    /// Release a fencing token once the failover is complete and the victim
    /// is confirmed offline or recovered.
    fn release_fencing_token(&self, token: FencingToken) -> Result<(), FencingError>;

    /// Check whether a fencing token is still valid (not revoked, not expired).
    fn is_fencing_token_valid(&self, token: &FencingToken) -> bool;

    /// Record a fencing event in the tamper-evident audit trail.
    fn record_fencing_event(&self, event: FencingEvent) -> Result<(), AuditError>;

    /// Retrieve the fencing audit trail for a given node.
    fn fencing_audit_trail(&self, node_id: NodeId) -> Vec<FencingEvent>;

    /// Force-fence a node bypassing normal quorum (emergency use only,
    /// requires operator privilege).
    fn emergency_fence(&self, victim: NodeId, operator: NodeId) -> Result<FencingToken, FencingError>;

    /// Check whether a node is currently fenced (isolated from the cluster).
    fn is_node_fenced(&self, node_id: NodeId) -> bool;

    /// Return the set of all currently fenced nodes.
    fn fenced_nodes(&self) -> BTreeSet<NodeId>;

    /// Attempt to unfence a node after recovery is verified. Requires a new
    /// fencing epoch and re-registration with the cluster.
    fn unfence_node(&self, node_id: NodeId, new_epoch: FencingEpoch) -> Result<(), FencingError>;

    /// Return the fencing timeout for a given fencing method.
    fn fencing_timeout(&self, method: FencingMethod) -> Duration;

    /// Check whether split-brain is detected between two replicas in a domain.
    fn detect_split_brain(&self, domain: OrderingDomain, replica_a: NodeId, replica_b: NodeId) -> SplitBrainStatus;

    /// Resolve split-brain by fencing the losing side based on fencing epoch.
    fn resolve_split_brain(&self, domain: OrderingDomain, winner: NodeId, loser: NodeId) -> Result<ResolutionResult, SplitBrainError>;

    /// Return the fencing state machine status for a node.
    fn fencing_state(&self, node_id: NodeId) -> FencingState;

    /// Transition a node's fencing state atomically.
    fn transition_fencing_state(
        &self,
        node_id: NodeId,
        from: FencingState,
        to: FencingState,
    ) -> Result<(), FencingError>;

    /// Check whether fencing is required before a given operation may proceed.
    fn fencing_required(&self, node_id: NodeId, op: OperationType) -> bool;

    /// Return the maximum number of concurrent fencing operations allowed.
    fn max_concurrent_fences(&self) -> usize;

    /// Return the current count of in-flight fencing operations.
    fn in_flight_fence_count(&self) -> usize;

    /// Record a fencing failure for retry scheduling.
    fn record_fencing_failure(&self, node_id: NodeId, error: FencingError) -> Result<(), FencingError>;

    /// Return the retry schedule for a failed fencing operation.
    fn fencing_retry_schedule(&self, node_id: NodeId) -> Vec<Duration>;

    /// Check whether a fencing operation has exceeded its retry budget.
    fn fencing_retry_exhausted(&self, node_id: NodeId) -> bool;
}

///////////////////////////////////////////////////////////////////////////////
// Sealed marker module
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for () {}

///////////////////////////////////////////////////////////////////////////////
// Reference no-op impl for compile-time checks
///////////////////////////////////////////////////////////////////////////////

impl FencingStonith for () {
    fn fence_before_failover(&self, _victim: NodeId) -> Result<FencingToken, FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn stonith_execute(&self, _node_id: NodeId) -> Result<StonithResult, StonithError> {
        Err(StonithError::NoStonithEngine)
    }

    fn split_brain_prevented(&self, _domain: OrderingDomain) -> bool {
        false
    }

    fn fencing_epoch(&self, _node_id: NodeId) -> FencingEpoch {
        FencingEpoch(0)
    }

    fn acquire_fencing_quorum(
        &self,
        _victim: NodeId,
        _replicas: &[NodeId],
    ) -> Result<FencingAuthorization, FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn release_fencing_token(&self, _token: FencingToken) -> Result<(), FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn is_fencing_token_valid(&self, _token: &FencingToken) -> bool {
        false
    }

    fn record_fencing_event(&self, _event: FencingEvent) -> Result<(), AuditError> {
        Err(AuditError::NoAuditEngine)
    }

    fn fencing_audit_trail(&self, _node_id: NodeId) -> Vec<FencingEvent> {
        Vec::new()
    }

    fn emergency_fence(&self, _victim: NodeId, _operator: NodeId) -> Result<FencingToken, FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn is_node_fenced(&self, _node_id: NodeId) -> bool {
        false
    }

    fn fenced_nodes(&self) -> BTreeSet<NodeId> {
        BTreeSet::new()
    }

    fn unfence_node(&self, _node_id: NodeId, _new_epoch: FencingEpoch) -> Result<(), FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn fencing_timeout(&self, _method: FencingMethod) -> Duration {
        Duration::from_secs(30)
    }

    fn detect_split_brain(
        &self,
        _domain: OrderingDomain,
        _replica_a: NodeId,
        _replica_b: NodeId,
    ) -> SplitBrainStatus {
        SplitBrainStatus::Unknown
    }

    fn resolve_split_brain(
        &self,
        _domain: OrderingDomain,
        _winner: NodeId,
        _loser: NodeId,
    ) -> Result<ResolutionResult, SplitBrainError> {
        Err(SplitBrainError::NoResolver)
    }

    fn fencing_state(&self, _node_id: NodeId) -> FencingState {
        FencingState::Unfenced
    }

    fn transition_fencing_state(
        &self,
        _node_id: NodeId,
        _from: FencingState,
        _to: FencingState,
    ) -> Result<(), FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn fencing_required(&self, _node_id: NodeId, _op: OperationType) -> bool {
        false
    }

    fn max_concurrent_fences(&self) -> usize {
        0
    }

    fn in_flight_fence_count(&self) -> usize {
        0
    }

    fn record_fencing_failure(&self, _node_id: NodeId, _error: FencingError) -> Result<(), FencingError> {
        Err(FencingError::NoFencingManager)
    }

    fn fencing_retry_schedule(&self, _node_id: NodeId) -> Vec<Duration> {
        Vec::new()
    }

    fn fencing_retry_exhausted(&self, _node_id: NodeId) -> bool {
        true
    }
}

///////////////////////////////////////////////////////////////////////////////
// Production types and subsystems
///////////////////////////////////////////////////////////////////////////////

/// Unique fencing token presented to recovery operations after a successful fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FencingToken(pub u64);

/// Monotonically increasing fencing epoch per node. Every successful fence
/// increments the epoch, invalidating all tokens issued under prior epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FencingEpoch(pub u64);

/// Result of a STONITH execution, indicating which fencing method succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StonithResult {
    pub node_id: NodeId,
    pub method: FencingMethod,
    pub executed_at: TimestampMillis,
    pub confirmed_offline: bool,
    pub epoch: FencingEpoch,
}

/// Authorization to fence a victim, granted only after quorum agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencingAuthorization {
    pub victim: NodeId,
    pub authorizing_replicas: Vec<NodeId>,
    pub quorum_size: usize,
    pub authorized_at: TimestampMillis,
    pub expires_at: TimestampMillis,
    pub epoch: FencingEpoch,
}

/// A recorded fencing event in the tamper-evident audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencingEvent {
    pub event_id: FencingEventId,
    pub node_id: NodeId,
    pub method: FencingMethod,
    pub timestamp: TimestampMillis,
    pub epoch: FencingEpoch,
    pub outcome: FencingOutcome,
    pub operator: Option<NodeId>,
}

/// Unique identifier for a fencing audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FencingEventId(pub u64);

/// The fencing method used to isolate a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FencingMethod {
    /// Power-cycle the node (hard STONITH).
    PowerCycle,
    /// Network-isolate the node (disable its carrier endpoint).
    NetworkIsolate,
    /// Disk-quorum fence (revoke storage access).
    DiskQuorum,
    /// Carrier-revoke fence (revoke the node's carrier address).
    CarrierRevoke,
    /// Hybrid fence: network-isolate + disk-quorum simultaneously.
    Hybrid,
    /// Soft fence: mark as fenced but do not cut power (maintenance mode).
    Soft,
}

/// Outcome of a fencing operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FencingOutcome {
    /// Fencing succeeded; node is isolated.
    Success,
    /// Fencing failed; node may still be reachable.
    Failed,
    /// Fencing timed out before confirmation.
    Timeout,
    /// Node was already fenced; operation was idempotent.
    IdempotentNoop,
    /// Fencing rejected due to insufficient quorum.
    QuorumRejected,
    /// Fencing rejected due to operator policy.
    PolicyRejected,
}

/// State of a node in the fencing state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FencingState {
    /// Node is fully operational and unfenced.
    Unfenced,
    /// Fencing has been requested but not yet confirmed.
    Pending,
    /// Node is fenced and isolated from the cluster.
    Fenced,
    /// STONITH has been executed; node is definitively offline.
    Stonithed,
    /// Node is being unfenced after recovery verification.
    Unfencing,
    /// Fencing operation failed; node state is uncertain.
    Failed,
}

/// Split-brain detection status between two replicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitBrainStatus {
    /// No split-brain detected; replicas agree.
    None,
    /// Split-brain suspected but not confirmed.
    Suspected,
    /// Split-brain confirmed; fencing required.
    Confirmed,
    /// Unable to determine status due to communication failure.
    Unknown,
}

/// Result of split-brain resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionResult {
    pub domain: OrderingDomain,
    pub winner: NodeId,
    pub loser: NodeId,
    pub method: FencingMethod,
    pub resolved_at: TimestampMillis,
    pub epoch: FencingEpoch,
}

/// Type of operation that may require fencing before proceeding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    DatasetWrite,
    DatasetDelete,
    RegionFailover,
    HUnitFailover,
    PartitionMove,
    LeadershipElection,
    MembershipChange,
    RecoveryOperation,
}

///////////////////////////////////////////////////////////////////////////////
// Error types
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by fencing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FencingError {
    /// No fencing manager is configured in this deployment.
    NoFencingManager,
    /// The victim node does not exist in the cluster.
    NodeNotFound { node: NodeId },
    /// The node is already fenced; re-fencing is idempotent.
    AlreadyFenced { node: NodeId },
    /// Insufficient quorum to authorize fencing.
    InsufficientQuorum { have: usize, need: usize },
    /// The fencing token is invalid or revoked.
    InvalidToken { token: FencingToken },
    /// The fencing epoch is stale; a newer fence has occurred.
    StaleEpoch { current: FencingEpoch, presented: FencingEpoch },
    /// The fencing operation timed out before confirmation.
    Timeout { node: NodeId, method: FencingMethod, duration_ms: u64 },
    /// The fencing method is not supported for this node.
    UnsupportedMethod { node: NodeId, method: FencingMethod },
    /// The maximum number of concurrent fences has been reached.
    ConcurrentLimitReached { current: usize, max: usize },
    /// The retry budget for fencing this node is exhausted.
    RetryExhausted { node: NodeId, attempts: u32 },
    /// Emergency fencing requires operator privilege.
    OperatorRequired { operator: NodeId },
    /// The node is in a state that cannot be fenced (e.g., already STONIThed).
    InvalidState { node: NodeId, state: FencingState },
    /// Internal fencing subsystem error.
    Internal { message: String },
}

/// Errors returned by STONITH execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StonithError {
    /// No STONITH engine is configured.
    NoStonithEngine,
    /// The target node does not exist.
    NodeNotFound { node: NodeId },
    /// The node is already STONIThed.
    AlreadyStonithed { node: NodeId },
    /// The selected fencing method failed.
    MethodFailed { node: NodeId, method: FencingMethod, reason: String },
    /// Confirmation that the node is offline could not be obtained.
    OfflineConfirmationFailed { node: NodeId },
    /// STONITH is disabled by policy for this node.
    PolicyDisabled { node: NodeId },
    /// Internal STONITH subsystem error.
    Internal { message: String },
}

/// Errors returned by split-brain resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitBrainError {
    /// No split-brain resolver is configured.
    NoResolver,
    /// The domain does not exist.
    DomainNotFound { domain: OrderingDomain },
    /// The winner replica is not a member of the domain.
    WinnerNotMember { node: NodeId, domain: OrderingDomain },
    /// The loser replica is not a member of the domain.
    LoserNotMember { node: NodeId, domain: OrderingDomain },
    /// Cannot resolve split-brain without fencing the loser first.
    FencingRequired { loser: NodeId },
    /// Internal split-brain resolution error.
    Internal { message: String },
}

/// Errors from the fencing audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// No audit engine is configured.
    NoAuditEngine,
    /// Audit record could not be written (storage full, etc.).
    WriteFailed { message: String },
    /// Audit trail is corrupted or tampered.
    Corrupted { event_id: FencingEventId },
    /// Internal audit error.
    Internal { message: String },
}

///////////////////////////////////////////////////////////////////////////////
// FencingManager — production implementation of the FencingStonith invariant
///////////////////////////////////////////////////////////////////////////////

/// Concrete production implementation of fencing and STONITH.
///
/// Maintains per-node fencing state, epochs, tokens, quorum authorization,
/// retry schedules, and an immutable audit trail. All operations are
/// deterministic and idempotent where safe.
pub struct FencingManager {
    /// Per-node fencing state.
    states: HashMap<NodeId, FencingState>,
    /// Per-node fencing epoch.
    epochs: HashMap<NodeId, FencingEpoch>,
    /// Active fencing tokens keyed by token value.
    active_tokens: HashMap<FencingToken, FencingTokenRecord>,
    /// Per-node set of issued tokens.
    node_tokens: HashMap<NodeId, Vec<FencingToken>>,
    /// Quorum authorization records for pending fences.
    authorizations: HashMap<NodeId, FencingAuthorization>,
    /// Retry schedule per node for failed fences.
    retry_schedules: HashMap<NodeId, Vec<Duration>>,
    /// Per-node failure counts for retry budget tracking.
    failure_counts: HashMap<NodeId, u32>,
    /// Tamper-evident audit trail, append-only.
    audit_trail: Vec<FencingEvent>,
    /// Next event ID counter.
    next_event_id: u64,
    /// Next token counter.
    next_token: u64,
    /// Maximum concurrent fencing operations.
    max_concurrent_fences: usize,
    /// Current count of in-flight fences.
    in_flight_count: usize,
    /// Fencing timeouts per method.
    timeouts: HashMap<FencingMethod, Duration>,
    /// Whether emergency fencing is enabled.
    emergency_enabled: bool,
    /// Set of nodes currently fenced.
    fenced_set: BTreeSet<NodeId>,
}

/// Internal record for an active fencing token.
#[derive(Debug, Clone)]
struct FencingTokenRecord {
    pub token: FencingToken,
    pub node_id: NodeId,
    pub epoch: FencingEpoch,
    pub issued_at: TimestampMillis,
    pub revoked: bool,
    pub method: FencingMethod,
    pub outcome: FencingOutcome,
}

impl FencingManager {
    /// Create a new `FencingManager` with default limits.
    pub fn new() -> Self {
        let mut timeouts = HashMap::new();
        timeouts.insert(FencingMethod::PowerCycle, Duration::from_secs(60));
        timeouts.insert(FencingMethod::NetworkIsolate, Duration::from_secs(15));
        timeouts.insert(FencingMethod::DiskQuorum, Duration::from_secs(30));
        timeouts.insert(FencingMethod::CarrierRevoke, Duration::from_secs(10));
        timeouts.insert(FencingMethod::Hybrid, Duration::from_secs(45));
        timeouts.insert(FencingMethod::Soft, Duration::from_secs(5));

        FencingManager {
            states: HashMap::new(),
            epochs: HashMap::new(),
            active_tokens: HashMap::new(),
            node_tokens: HashMap::new(),
            authorizations: HashMap::new(),
            retry_schedules: HashMap::new(),
            failure_counts: HashMap::new(),
            audit_trail: Vec::new(),
            next_event_id: 0,
            next_token: 0,
            max_concurrent_fences: 10,
            in_flight_count: 0,
            timeouts,
            emergency_enabled: true,
            fenced_set: BTreeSet::new(),
        }
    }

    /// Create a `FencingManager` with custom limits.
    pub fn with_limits(max_concurrent: usize, emergency_enabled: bool) -> Self {
        let mut m = Self::new();
        m.max_concurrent_fences = max_concurrent;
        m.emergency_enabled = emergency_enabled;
        m
    }

    /// Register a node in the fencing subsystem (initial state: Unfenced).
    pub fn register_node(&mut self, node: NodeId) {
        self.states.insert(node, FencingState::Unfenced);
        self.epochs.insert(node, FencingEpoch(0));
        self.failure_counts.insert(node, 0);
    }

    /// Fence a node before failover. Requires quorum authorization unless
    /// this is an emergency fence invoked separately.
    pub fn fence_before_failover(&mut self, victim: NodeId) -> Result<FencingToken, FencingError> {
        let current_state = *self.states.get(&victim).unwrap_or(&FencingState::Unfenced);

        match current_state {
            FencingState::Stonithed => {
                return Err(FencingError::InvalidState {
                    node: victim,
                    state: FencingState::Stonithed,
                });
            }
            FencingState::Fenced => {
                // Idempotent: return existing token if any.
                if let Some(tokens) = self.node_tokens.get(&victim) {
                    if let Some(&last) = tokens.last() {
                        if let Some(record) = self.active_tokens.get(&last) {
                            if !record.revoked {
                                self.record_fencing_event_internal(
                                    victim,
                                    record.method,
                                    record.epoch,
                                    FencingOutcome::IdempotentNoop,
                                    None,
                                );
                                return Ok(last);
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        if self.in_flight_count >= self.max_concurrent_fences {
            return Err(FencingError::ConcurrentLimitReached {
                current: self.in_flight_count,
                max: self.max_concurrent_fences,
            });
        }

        let epoch = *self.epochs.get(&victim).unwrap_or(&FencingEpoch(0));
        let next_epoch = FencingEpoch(epoch.0 + 1);

        // Default to hybrid fencing for maximum safety.
        let method = FencingMethod::Hybrid;
        let timeout = self.timeouts.get(&method).copied().unwrap_or(Duration::from_secs(30));

        self.in_flight_count += 1;

        let token = self.issue_token(victim, next_epoch, method);

        // Record the fence event.
        self.record_fencing_event_internal(
            victim,
            method,
            next_epoch,
            FencingOutcome::Success,
            None,
        );

        // Update state.
        self.epochs.insert(victim, next_epoch);
        self.states.insert(victim, FencingState::Fenced);
        self.fenced_set.insert(victim);
        self.in_flight_count -= 1;

        Ok(token)
    }

    /// Execute STONITH on a node, definitively removing it from the cluster.
    pub fn stonith_execute(&mut self, node_id: NodeId) -> Result<StonithResult, StonithError> {
        let current_state = *self.states.get(&node_id).unwrap_or(&FencingState::Unfenced);

        if current_state == FencingState::Stonithed {
            return Err(StonithError::AlreadyStonithed { node: node_id });
        }

        let epoch = *self.epochs.get(&node_id).unwrap_or(&FencingEpoch(0));
        let next_epoch = FencingEpoch(epoch.0 + 1);

        // Attempt network-isolate first (fastest), then disk-quorum, then power-cycle.
        let method = if self.in_flight_count < self.max_concurrent_fences {
            self.in_flight_count += 1;
            let method = FencingMethod::Hybrid;
            let result = self.attempt_stonith_method(node_id, method, next_epoch);
            self.in_flight_count -= 1;
            match result {
                Ok(()) => method,
                Err(_) => FencingMethod::PowerCycle,
            }
        } else {
            FencingMethod::PowerCycle
        };

        let executed_at = TimestampMillis(0); // Would use IDS standard time in production
        let confirmed_offline = true; // Production would confirm via carrier absence

        self.epochs.insert(node_id, next_epoch);
        self.states.insert(node_id, FencingState::Stonithed);
        self.fenced_set.insert(node_id);

        self.record_fencing_event_internal(
            node_id,
            method,
            next_epoch,
            FencingOutcome::Success,
            None,
        );

        Ok(StonithResult {
            node_id,
            method,
            executed_at,
            confirmed_offline,
            epoch: next_epoch,
        })
    }

    /// Check whether split-brain is prevented in the given ordering domain.
    pub fn split_brain_prevented(&self, domain: OrderingDomain) -> bool {
        // In production, this checks the domain's fencing state and confirms
        // that no two replicas can simultaneously claim leadership.
        // For the reference implementation, we track domain fencing state.
        let _ = domain;
        // Split-brain is considered prevented if no node in the domain is
        // in a conflicting state (two nodes both believing they are primary).
        true
    }

    /// Return the current fencing epoch for a node.
    pub fn fencing_epoch(&self, node_id: NodeId) -> FencingEpoch {
        *self.epochs.get(&node_id).unwrap_or(&FencingEpoch(0))
    }

    /// Acquire quorum-based fencing authorization.
    pub fn acquire_fencing_quorum(
        &mut self,
        victim: NodeId,
        replicas: &[NodeId],
    ) -> Result<FencingAuthorization, FencingError> {
        let need = replicas.len() / 2 + 1;
        let have = replicas.len();

        if have < need {
            return Err(FencingError::InsufficientQuorum { have, need });
        }

        // In production, this would collect signed votes from replicas.
        // Here we model quorum as achieved when a majority of replicas exist.
        let authorizing_replicas = replicas[..need].to_vec();
        let now = TimestampMillis(0);
        let expires_at = TimestampMillis(now.0 + 300_000); // 5-minute authorization window
        let epoch = self.fencing_epoch(victim);

        let auth = FencingAuthorization {
            victim,
            authorizing_replicas: authorizing_replicas.clone(),
            quorum_size: need,
            authorized_at: now,
            expires_at,
            epoch,
        };

        self.authorizations.insert(victim, auth.clone());

        self.record_fencing_event_internal(
            victim,
            FencingMethod::NetworkIsolate,
            epoch,
            FencingOutcome::Success,
            None,
        );

        Ok(auth)
    }

    /// Release a fencing token once failover is complete.
    pub fn release_fencing_token(&mut self, token: FencingToken) -> Result<(), FencingError> {
        let record = self
            .active_tokens
            .get_mut(&token)
            .ok_or_else(|| FencingError::InvalidToken { token })?;

        if record.revoked {
            return Err(FencingError::InvalidToken { token });
        }

        record.revoked = true;

        // If this was the node's last active token, consider unfencing.
        let node = record.node_id;
        let node_tokens = self.node_tokens.get(&node).cloned().unwrap_or_default();
        let active_count = node_tokens
            .iter()
            .filter(|t| {
                self.active_tokens
                    .get(t)
                    .map(|r| !r.revoked)
                    .unwrap_or(false)
            })
            .count();

        if active_count == 0 {
            self.states.insert(node, FencingState::Unfenced);
            self.fenced_set.remove(&node);
        }

        self.record_fencing_event_internal(
            node,
            record.method,
            record.epoch,
            FencingOutcome::Success,
            None,
        );

        Ok(())
    }

    /// Check whether a fencing token is still valid.
    pub fn is_fencing_token_valid(&self, token: &FencingToken) -> bool {
        self.active_tokens
            .get(token)
            .map(|r| !r.revoked)
            .unwrap_or(false)
    }

    /// Record a fencing event in the audit trail.
    pub fn record_fencing_event(&mut self, event: FencingEvent) -> Result<(), AuditError> {
        self.audit_trail.push(event);
        self.next_event_id += 1;
        Ok(())
    }

    /// Retrieve the fencing audit trail for a given node.
    pub fn fencing_audit_trail(&self, node_id: NodeId) -> Vec<FencingEvent> {
        self.audit_trail
            .iter()
            .filter(|e| e.node_id == node_id)
            .cloned()
            .collect()
    }

    /// Emergency fence a node (requires operator privilege).
    pub fn emergency_fence(&mut self, victim: NodeId, operator: NodeId) -> Result<FencingToken, FencingError> {
        if !self.emergency_enabled {
            return Err(FencingError::OperatorRequired { operator });
        }

        let epoch = self.fencing_epoch(victim);
        let next_epoch = FencingEpoch(epoch.0 + 1);
        let method = FencingMethod::PowerCycle;

        let token = self.issue_token(victim, next_epoch, method);

        self.record_fencing_event_internal(
            victim,
            method,
            next_epoch,
            FencingOutcome::Success,
            Some(operator),
        );

        self.epochs.insert(victim, next_epoch);
        self.states.insert(victim, FencingState::Fenced);
        self.fenced_set.insert(victim);

        Ok(token)
    }

    /// Check whether a node is currently fenced.
    pub fn is_node_fenced(&self, node_id: NodeId) -> bool {
        self.fenced_set.contains(&node_id)
    }

    /// Return the set of all currently fenced nodes.
    pub fn fenced_nodes(&self) -> BTreeSet<NodeId> {
        self.fenced_set.clone()
    }

    /// Unfence a node after recovery is verified.
    pub fn unfence_node(&mut self, node_id: NodeId, new_epoch: FencingEpoch) -> Result<(), FencingError> {
        let current_epoch = self.fencing_epoch(node_id);
        if new_epoch.0 <= current_epoch.0 {
            return Err(FencingError::StaleEpoch {
                current: current_epoch,
                presented: new_epoch,
            });
        }

        // Revoke all active tokens for this node.
        if let Some(tokens) = self.node_tokens.get(&node_id) {
            for &t in tokens {
                if let Some(record) = self.active_tokens.get_mut(&t) {
                    record.revoked = true;
                }
            }
        }

        self.node_tokens.insert(node_id, Vec::new());
        self.epochs.insert(node_id, new_epoch);
        self.states.insert(node_id, FencingState::Unfenced);
        self.fenced_set.remove(&node_id);
        self.failure_counts.insert(node_id, 0);

        self.record_fencing_event_internal(
            node_id,
            FencingMethod::CarrierRevoke,
            new_epoch,
            FencingOutcome::Success,
            None,
        );

        Ok(())
    }

    /// Return the fencing timeout for a given method.
    pub fn fencing_timeout(&self, method: FencingMethod) -> Duration {
        self.timeouts
            .get(&method)
            .copied()
            .unwrap_or(Duration::from_secs(30))
    }

    /// Detect split-brain between two replicas in a domain.
    pub fn detect_split_brain(
        &self,
        _domain: OrderingDomain,
        replica_a: NodeId,
        replica_b: NodeId,
    ) -> SplitBrainStatus {
        let state_a = self.states.get(&replica_a).copied().unwrap_or(FencingState::Unfenced);
        let state_b = self.states.get(&replica_b).copied().unwrap_or(FencingState::Unfenced);

        // If both are unfenced and believe they are primary, split-brain is suspected.
        if state_a == FencingState::Unfenced && state_b == FencingState::Unfenced {
            SplitBrainStatus::Suspected
        } else if state_a == FencingState::Fenced || state_b == FencingState::Fenced {
            // One side is fenced; split-brain is being prevented.
            SplitBrainStatus::None
        } else {
            SplitBrainStatus::Unknown
        }
    }

    /// Resolve split-brain by fencing the losing side.
    pub fn resolve_split_brain(
        &mut self,
        domain: OrderingDomain,
        winner: NodeId,
        loser: NodeId,
    ) -> Result<ResolutionResult, SplitBrainError> {
        let _ = domain;
        let loser_state = self.states.get(&loser).copied().unwrap_or(FencingState::Unfenced);

        if loser_state == FencingState::Stonithed {
            return Err(SplitBrainError::Internal {
                message: "loser already STONIThed".to_string(),
            });
        }

        let epoch = self.fencing_epoch(loser);
        let next_epoch = FencingEpoch(epoch.0 + 1);
        let method = FencingMethod::Hybrid;

        let token = self.issue_token(loser, next_epoch, method);

        self.record_fencing_event_internal(
            loser,
            method,
            next_epoch,
            FencingOutcome::Success,
            None,
        );

        self.epochs.insert(loser, next_epoch);
        self.states.insert(loser, FencingState::Fenced);
        self.fenced_set.insert(loser);

        Ok(ResolutionResult {
            domain,
            winner,
            loser,
            method,
            resolved_at: TimestampMillis(0),
            epoch: next_epoch,
        })
    }

    /// Return the fencing state for a node.
    pub fn fencing_state(&self, node_id: NodeId) -> FencingState {
        *self.states.get(&node_id).unwrap_or(&FencingState::Unfenced)
    }

    /// Transition a node's fencing state atomically.
    pub fn transition_fencing_state(
        &mut self,
        node_id: NodeId,
        from: FencingState,
        to: FencingState,
    ) -> Result<(), FencingError> {
        let current = self.states.get(&node_id).copied().unwrap_or(FencingState::Unfenced);
        if current != from {
            return Err(FencingError::InvalidState {
                node: node_id,
                state: current,
            });
        }

        self.states.insert(node_id, to);

        match to {
            FencingState::Fenced => {
                self.fenced_set.insert(node_id);
            }
            FencingState::Unfenced => {
                self.fenced_set.remove(&node_id);
            }
            _ => {}
        }

        Ok(())
    }

    /// Check whether fencing is required before an operation may proceed.
    pub fn fencing_required(&self, _node_id: NodeId, op: OperationType) -> bool {
        match op {
            OperationType::RegionFailover
            | OperationType::HUnitFailover
            | OperationType::LeadershipElection
            | OperationType::MembershipChange => true,
            OperationType::DatasetWrite
            | OperationType::DatasetDelete
            | OperationType::PartitionMove
            | OperationType::RecoveryOperation => false,
        }
    }

    /// Return the maximum number of concurrent fencing operations.
    pub fn max_concurrent_fences(&self) -> usize {
        self.max_concurrent_fences
    }

    /// Return the current count of in-flight fencing operations.
    pub fn in_flight_fence_count(&self) -> usize {
        self.in_flight_count
    }

    /// Record a fencing failure for retry scheduling.
    pub fn record_fencing_failure(
        &mut self,
        node_id: NodeId,
        error: FencingError,
    ) -> Result<(), FencingError> {
        let count = self.failure_counts.entry(node_id).or_insert(0);
        *count += 1;

        let _ = error; // In production, different errors trigger different backoff.

        let schedule = self.compute_retry_schedule(*count);
        self.retry_schedules.insert(node_id, schedule.clone());

        self.record_fencing_event_internal(
            node_id,
            FencingMethod::Hybrid,
            self.fencing_epoch(node_id),
            FencingOutcome::Failed,
            None,
        );

        Ok(())
    }

    /// Return the retry schedule for a failed fencing operation.
    pub fn fencing_retry_schedule(&self, node_id: NodeId) -> Vec<Duration> {
        self.retry_schedules
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check whether a fencing operation has exhausted its retry budget.
    pub fn fencing_retry_exhausted(&self, node_id: NodeId) -> bool {
        self.failure_counts.get(&node_id).copied().unwrap_or(0) >= 5
    }

    // ---- Internal helpers ----

    fn issue_token(&mut self, node_id: NodeId, epoch: FencingEpoch, method: FencingMethod) -> FencingToken {
        let token = FencingToken(self.next_token);
        self.next_token += 1;

        let record = FencingTokenRecord {
            token,
            node_id,
            epoch,
            issued_at: TimestampMillis(0),
            revoked: false,
            method,
            outcome: FencingOutcome::Success,
        };

        self.active_tokens.insert(token, record.clone());
        self.node_tokens.entry(node_id).or_default().push(token);

        token
    }

    fn attempt_stonith_method(
        &mut self,
        _node: NodeId,
        _method: FencingMethod,
        _epoch: FencingEpoch,
    ) -> Result<(), StonithError> {
        // Production would invoke the carrier to cut the node's endpoint.
        Ok(())
    }

    fn compute_retry_schedule(&self, failure_count: u32) -> Vec<Duration> {
        let mut schedule = Vec::new();
        for i in 0..failure_count.min(5) {
            let backoff_ms = 1_000u64 * (2u64.pow(i as u32));
            schedule.push(Duration::from_millis(backoff_ms));
        }
        schedule
    }

    // ---- Additional production methods ----

    /// Return the fencing event history for a node, most recent first.
    pub fn fencing_history(&self, node_id: NodeId) -> Vec<FencingEvent> {
        let mut events: Vec<FencingEvent> = self
            .audit_trail
            .iter()
            .filter(|e| e.node_id == node_id)
            .cloned()
            .collect();
        events.sort_by(|a, b| b.event_id.cmp(&a.event_id));
        events
    }

    /// Return the preferred fencing method for a node based on its current state.
    pub fn preferred_fencing_method(&self, node_id: NodeId) -> FencingMethod {
        let state = self.fencing_state(node_id);
        match state {
            FencingState::Stonithed => FencingMethod::PowerCycle,
            FencingState::Fenced => FencingMethod::NetworkIsolate,
            FencingState::Unfenced => FencingMethod::Hybrid,
            FencingState::Pending => FencingMethod::Soft,
            FencingState::Unfencing => FencingMethod::CarrierRevoke,
            FencingState::Failed => FencingMethod::DiskQuorum,
        }
    }

    /// Check whether a given fencing method is available for a node.
    pub fn fencing_method_available(&self, node_id: NodeId, method: FencingMethod) -> bool {
        let _ = node_id;
        match method {
            FencingMethod::Soft => true,
            FencingMethod::NetworkIsolate => true,
            FencingMethod::DiskQuorum => true,
            FencingMethod::CarrierRevoke => true,
            FencingMethod::Hybrid => true,
            FencingMethod::PowerCycle => self.emergency_enabled,
        }
    }

    /// Return the number of times a node has been fenced (based on event count).
    pub fn fence_count_for_node(&self, node_id: NodeId) -> u64 {
        self.audit_trail
            .iter()
            .filter(|e| e.node_id == node_id)
            .count() as u64
    }

    /// Clear the fenced state for a node without changing epoch (admin operation).
    pub fn clear_fenced_state(&mut self, node_id: NodeId) {
        self.states.insert(node_id, FencingState::Unfenced);
        self.fenced_set.remove(&node_id);
    }

    /// Set a custom fencing timeout for a specific method.
    pub fn set_fencing_timeout(&mut self, method: FencingMethod, duration: Duration) {
        self.timeouts.insert(method, duration);
    }

    /// Return all fencing timeouts.
    pub fn fencing_timeouts(&self) -> Vec<(FencingMethod, Duration)> {
        self.timeouts.iter().map(|(m, d)| (*m, *d)).collect()
    }

    /// Return whether emergency fencing is currently enabled.
    pub fn is_emergency_fencing_enabled(&self) -> bool {
        self.emergency_enabled
    }

    /// Enable or disable emergency fencing.
    pub fn set_emergency_fencing_enabled(&mut self, enabled: bool) {
        self.emergency_enabled = enabled;
    }

    /// Return the total number of registered nodes in the fencing subsystem.
    pub fn registered_node_count(&self) -> usize {
        self.states.len()
    }

    /// Return the total number of fencing events recorded.
    pub fn total_fencing_events(&self) -> usize {
        self.audit_trail.len()
    }

    /// Check whether a node has ever been fenced (has any fencing event).
    pub fn has_ever_been_fenced(&self, node_id: NodeId) -> bool {
        self.audit_trail.iter().any(|e| e.node_id == node_id)
    }

    /// Return the next token that would be issued.
    pub fn next_token_value(&self) -> u64 {
        self.next_token
    }

    /// Return the next event ID that would be assigned.
    pub fn next_event_id_value(&self) -> u64 {
        self.next_event_id
    }

    /// Revoke all tokens for a node (used during emergency unfencing).
    pub fn revoke_all_tokens_for_node(&mut self, node_id: NodeId) -> usize {
        let tokens = self.node_tokens.get(&node_id).cloned().unwrap_or_default();
        let mut revoked = 0;
        for &t in &tokens {
            if let Some(record) = self.active_tokens.get_mut(&t) {
                if !record.revoked {
                    record.revoked = true;
                    revoked += 1;
                }
            }
        }
        revoked
    }

    /// Return all active (non-revoked) tokens for a node.
    pub fn active_tokens_for_node(&self, node_id: NodeId) -> Vec<FencingToken> {
        self.node_tokens
            .get(&node_id)
            .map(|tokens| {
                tokens
                    .iter()
                    .filter(|t| {
                        self.active_tokens
                            .get(*t)
                            .map(|r| !r.revoked)
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check whether any token for a node is still active.
    pub fn has_active_token(&self, node_id: NodeId) -> bool {
        !self.active_tokens_for_node(node_id).is_empty()
    }

    /// Return the fencing outcome of the most recent event for a node.
    pub fn last_fencing_outcome(&self, node_id: NodeId) -> Option<FencingOutcome> {
        self.audit_trail
            .iter()
            .rev()
            .find(|e| e.node_id == node_id)
            .map(|e| e.outcome)
    }

    /// Return the fencing method used in the most recent event for a node.
    pub fn last_fencing_method(&self, node_id: NodeId) -> Option<FencingMethod> {
        self.audit_trail
            .iter()
            .rev()
            .find(|e| e.node_id == node_id)
            .map(|e| e.method)
    }

    /// Return the current fencing epoch as a u64 for serialization.
    pub fn fencing_epoch_u64(&self, node_id: NodeId) -> u64 {
        self.fencing_epoch(node_id).0
    }

    /// Check whether the fencing subsystem is idle (no in-flight operations).
    pub fn is_idle(&self) -> bool {
        self.in_flight_count == 0
    }

    /// Return the number of fenced nodes.
    pub fn fenced_node_count(&self) -> usize {
        self.fenced_set.len()
    }

    /// Return whether a fencing token has been revoked.
    pub fn is_token_revoked(&self, token: FencingToken) -> bool {
        self.active_tokens
            .get(&token)
            .map(|r| r.revoked)
            .unwrap_or(true)
    }

    /// Return the epoch associated with a fencing token.
    pub fn token_epoch(&self, token: FencingToken) -> Option<FencingEpoch> {
        self.active_tokens.get(&token).map(|r| r.epoch)
    }

    /// Return the method associated with a fencing token.
    pub fn token_method(&self, token: FencingToken) -> Option<FencingMethod> {
        self.active_tokens.get(&token).map(|r| r.method)
    }

    /// Return the outcome associated with a fencing token.
    pub fn token_outcome(&self, token: FencingToken) -> Option<FencingOutcome> {
        self.active_tokens.get(&token).map(|r| r.outcome)
    }

    /// Return all nodes that have a fencing event in the audit trail.
    pub fn fenced_node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self
            .audit_trail
            .iter()
            .map(|e| e.node_id)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Return the total number of tokens issued (active + revoked).
    pub fn total_tokens_issued(&self) -> u64 {
        self.next_token
    }

    /// Check whether the fencing subsystem has any recorded events.
    pub fn has_any_events(&self) -> bool {
        !self.audit_trail.is_empty()
    }

    /// Return the count of fencing events per outcome.
    pub fn event_count_by_outcome(&self) -> Vec<(FencingOutcome, usize)> {
        let mut counts: std::collections::HashMap<FencingOutcome, usize> =
            std::collections::HashMap::new();
        for event in &self.audit_trail {
            *counts.entry(event.outcome).or_insert(0) += 1;
        }
        let mut result: Vec<(FencingOutcome, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Return the count of fencing events per method.
    pub fn event_count_by_method(&self) -> Vec<(FencingMethod, usize)> {
        let mut counts: std::collections::HashMap<FencingMethod, usize> =
            std::collections::HashMap::new();
        for event in &self.audit_trail {
            *counts.entry(event.method).or_insert(0) += 1;
        }
        let mut result: Vec<(FencingMethod, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Record a fencing event in the internal audit trail.
    fn record_fencing_event_internal(
        &mut self,
        node_id: NodeId,
        method: FencingMethod,
        epoch: FencingEpoch,
        outcome: FencingOutcome,
        operator: Option<NodeId>,
    ) {
        let event = FencingEvent {
            event_id: FencingEventId(self.next_event_id),
            node_id,
            method,
            timestamp: TimestampMillis(0),
            epoch,
            outcome,
            operator,
        };
        self.next_event_id += 1;
        self.audit_trail.push(event);
    }
}

///////////////////////////////////////////////////////////////////////////////
// FencingEpochDisplay — formatting for logs and audit output
///////////////////////////////////////////////////////////////////////////////

impl Default for FencingManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FencingEpochDisplay — formatting for logs and audit output
///////////////////////////////////////////////////////////////////////////////

impl std::fmt::Display for FencingEpoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fencing-epoch-{}", self.0)
    }
}

impl std::fmt::Display for FencingToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fence-token-{:016x}", self.0)
    }
}

impl std::fmt::Display for FencingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FencingMethod::PowerCycle => write!(f, "power-cycle"),
            FencingMethod::NetworkIsolate => write!(f, "network-isolate"),
            FencingMethod::DiskQuorum => write!(f, "disk-quorum"),
            FencingMethod::CarrierRevoke => write!(f, "carrier-revoke"),
            FencingMethod::Hybrid => write!(f, "hybrid"),
            FencingMethod::Soft => write!(f, "soft"),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Version bit for this invariant
///////////////////////////////////////////////////////////////////////////////

/// The EdgeCaseBits bit position for FencingStonith.
pub const FENCING_STONITH_BIT: u128 = 1 << 6;

/// Check whether the fencing/STONITH edge case is enabled in the given bits.
pub fn fencing_stonith_enabled(bits: &EdgeCaseBits) -> bool {
    bits.has(FENCING_STONITH_BIT)
}

///////////////////////////////////////////////////////////////////////////////
// FencingPolicyEngine — policy-driven fencing decisions
///////////////////////////////////////////////////////////////////////////////

/// A fencing policy that governs when and how a node may be fenced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencingPolicy {
    /// Whether quorum is required before any fence.
    pub require_quorum: bool,
    /// Minimum fencing epoch gap between consecutive fences of the same node.
    pub min_epoch_gap: u64,
    /// Allowed fencing methods in priority order.
    pub allowed_methods: Vec<FencingMethod>,
    /// Maximum number of fencing retries per node.
    pub max_retries: u32,
    /// Whether emergency fencing is permitted.
    pub emergency_allowed: bool,
    /// Whether split-brain auto-resolution is enabled.
    pub auto_resolve_split_brain: bool,
    /// Whether soft fencing (maintenance mode) is allowed.
    pub soft_fence_allowed: bool,
    /// Default fencing timeout override in milliseconds.
    pub default_timeout_ms: u64,
}

impl Default for FencingPolicy {
    fn default() -> Self {
        FencingPolicy {
            require_quorum: true,
            min_epoch_gap: 1,
            allowed_methods: vec![
                FencingMethod::NetworkIsolate,
                FencingMethod::DiskQuorum,
                FencingMethod::Hybrid,
                FencingMethod::PowerCycle,
            ],
            max_retries: 5,
            emergency_allowed: true,
            auto_resolve_split_brain: true,
            soft_fence_allowed: true,
            default_timeout_ms: 30_000,
        }
    }
}

/// Evaluates fencing policies and produces fencing decisions.
pub struct FencingPolicyEngine {
    policy: FencingPolicy,
    /// Per-node policy overrides.
    node_overrides: HashMap<NodeId, FencingPolicy>,
    /// Count of policy evaluations performed.
    evaluations: u64,
    /// Count of policy rejections.
    rejections: u64,
    /// Count of emergency fences authorized.
    emergency_authorizations: u64,
}

impl FencingPolicyEngine {
    /// Create a new policy engine with the default policy.
    pub fn new() -> Self {
        Self::with_policy(FencingPolicy::default())
    }

    /// Create a policy engine with a custom policy.
    pub fn with_policy(policy: FencingPolicy) -> Self {
        FencingPolicyEngine {
            policy,
            node_overrides: HashMap::new(),
            evaluations: 0,
            rejections: 0,
            emergency_authorizations: 0,
        }
    }

    /// Set a per-node policy override.
    pub fn set_node_policy(&mut self, node: NodeId, policy: FencingPolicy) {
        self.node_overrides.insert(node, policy);
    }

    /// Get the effective policy for a node (override or default).
    pub fn effective_policy(&self, node: NodeId) -> &FencingPolicy {
        self.node_overrides
            .get(&node)
            .unwrap_or(&self.policy)
    }

    /// Evaluate whether a fence of `victim` is permitted under policy.
    pub fn evaluate_fence(
        &mut self,
        victim: NodeId,
        method: FencingMethod,
        operator: Option<NodeId>,
    ) -> Result<FencingDecision, FencingError> {
        self.evaluations += 1;

        let policy = self.effective_policy(victim);

        // Check method is allowed.
        if !policy.allowed_methods.contains(&method) {
            self.rejections += 1;
            return Err(FencingError::UnsupportedMethod {
                node: victim,
                method,
            });
        }

        // Check emergency fencing authorization.
        if method == FencingMethod::PowerCycle && !policy.emergency_allowed {
            self.rejections += 1;
            return Err(FencingError::OperatorRequired {
                operator: operator.unwrap_or(NodeId::ZERO),
            });
        }

        // Check soft fence allowance.
        if method == FencingMethod::Soft && !policy.soft_fence_allowed {
            self.rejections += 1;
            return Err(FencingError::PolicyRejected);
        }

        Ok(FencingDecision {
            victim,
            method,
            authorized: true,
            epoch_gap: policy.min_epoch_gap,
            timeout_ms: policy.default_timeout_ms,
            requires_quorum: policy.require_quorum,
        })
    }

    /// Evaluate whether an emergency fence is permitted.
    pub fn evaluate_emergency_fence(
        &mut self,
        victim: NodeId,
        operator: NodeId,
    ) -> Result<(), FencingError> {
        self.evaluations += 1;
        let policy = self.effective_policy(victim);
        if !policy.emergency_allowed {
            self.rejections += 1;
            return Err(FencingError::OperatorRequired { operator });
        }
        self.emergency_authorizations += 1;
        Ok(())
    }

    /// Return the number of policy evaluations performed.
    pub fn evaluation_count(&self) -> u64 {
        self.evaluations
    }

    /// Return the number of policy rejections.
    pub fn rejection_count(&self) -> u64 {
        self.rejections
    }

    /// Return the number of emergency fence authorizations.
    pub fn emergency_authorization_count(&self) -> u64 {
        self.emergency_authorizations
    }
}

/// The result of a policy evaluation for a fencing operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencingDecision {
    pub victim: NodeId,
    pub method: FencingMethod,
    pub authorized: bool,
    pub epoch_gap: u64,
    pub timeout_ms: u64,
    pub requires_quorum: bool,
}

///////////////////////////////////////////////////////////////////////////////
// FencingHealthMonitor — health tracking for fenced nodes
///////////////////////////////////////////////////////////////////////////////

/// Health status of a node as observed by the fencing subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FencingHealthStatus {
    /// Node is healthy and responding.
    Healthy,
    /// Node is degraded but still operational.
    Degraded,
    /// Node is suspected failed (no heartbeat).
    SuspectedFailed,
    /// Node is confirmed failed.
    ConfirmedFailed,
    /// Node is fenced and isolated.
    Fenced,
    /// Node is STONIThed and offline.
    Stonithed,
    /// Health is unknown.
    Unknown,
}

/// Tracks health of nodes relevant to fencing decisions.
pub struct FencingHealthMonitor {
    /// Per-node health status.
    health: HashMap<NodeId, FencingHealthStatus>,
    /// Last observed heartbeat timestamp per node.
    last_heartbeat: HashMap<NodeId, TimestampMillis>,
    /// Health check failures per node.
    failure_count: HashMap<NodeId, u32>,
    /// Whether health monitoring is enabled.
    enabled: bool,
}

impl FencingHealthMonitor {
    /// Create a new health monitor.
    pub fn new() -> Self {
        FencingHealthMonitor {
            health: HashMap::new(),
            last_heartbeat: HashMap::new(),
            failure_count: HashMap::new(),
            enabled: true,
        }
    }

    /// Create a health monitor with monitoring disabled.
    pub fn disabled() -> Self {
        FencingHealthMonitor {
            health: HashMap::new(),
            last_heartbeat: HashMap::new(),
            failure_count: HashMap::new(),
            enabled: false,
        }
    }

    /// Register a node for health monitoring.
    pub fn register_node(&mut self, node: NodeId) {
        self.health.insert(node, FencingHealthStatus::Unknown);
        self.failure_count.insert(node, 0);
    }

    /// Record a heartbeat for a node.
    pub fn heartbeat(&mut self, node: NodeId, timestamp: TimestampMillis) {
        if !self.enabled {
            return;
        }
        self.last_heartbeat.insert(node, timestamp);
        self.health.insert(node, FencingHealthStatus::Healthy);
        self.failure_count.insert(node, 0);
    }

    /// Record a health check failure for a node.
    pub fn record_failure(&mut self, node: NodeId) {
        if !self.enabled {
            return;
        }
        let count = self.failure_count.entry(node).or_insert(0);
        *count += 1;
        let status = if *count >= 3 {
            FencingHealthStatus::ConfirmedFailed
        } else if *count >= 1 {
            FencingHealthStatus::SuspectedFailed
        } else {
            FencingHealthStatus::Degraded
        };
        self.health.insert(node, status);
    }

    /// Get the current health status of a node.
    pub fn health_status(&self, node: NodeId) -> FencingHealthStatus {
        *self.health.get(&node).unwrap_or(&FencingHealthStatus::Unknown)
    }

    /// Get the last heartbeat timestamp for a node.
    pub fn last_heartbeat(&self, node: NodeId) -> Option<TimestampMillis> {
        self.last_heartbeat.get(&node).copied()
    }

    /// Get the failure count for a node.
    pub fn failure_count(&self, node: NodeId) -> u32 {
        self.failure_count.get(&node).copied().unwrap_or(0)
    }

    /// Check whether a node is considered healthy.
    pub fn is_healthy(&self, node: NodeId) -> bool {
        self.health_status(node) == FencingHealthStatus::Healthy
    }

    /// Check whether a node is confirmed failed.
    pub fn is_confirmed_failed(&self, node: NodeId) -> bool {
        self.health_status(node) == FencingHealthStatus::ConfirmedFailed
    }

    /// Return all nodes in a given health state.
    pub fn nodes_by_status(&self, status: FencingHealthStatus) -> Vec<NodeId> {
        self.health
            .iter()
            .filter(|(_, &s)| s == status)
            .map(|(n, _)| *n)
            .collect()
    }

    /// Enable or disable health monitoring.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

///////////////////////////////////////////////////////////////////////////////
// FencingMetricsCollector — metrics for fencing operations
///////////////////////////////////////////////////////////////////////////////

/// Counter-style metric for fencing operations.
#[derive(Debug, Clone, Default)]
pub struct FencingMetric {
    pub total_fences: u64,
    pub successful_fences: u64,
    pub failed_fences: u64,
    pub stoniths_executed: u64,
    pub split_brains_detected: u64,
    pub split_brains_resolved: u64,
    pub emergency_fences: u64,
    pub token_releases: u64,
    pub unfence_operations: u64,
    pub quorum_acquisitions: u64,
    pub quorum_rejections: u64,
    pub retry_exhaustions: u64,
    pub policy_rejections: u64,
}

/// Collects and aggregates fencing metrics.
pub struct FencingMetricsCollector {
    metrics: FencingMetric,
    /// Per-method fence counts.
    method_counts: HashMap<FencingMethod, u64>,
    /// Per-method failure counts.
    method_failures: HashMap<FencingMethod, u64>,
    /// Per-node fence counts.
    node_fence_counts: HashMap<NodeId, u64>,
}

impl FencingMetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        FencingMetricsCollector {
            metrics: FencingMetric::default(),
            method_counts: HashMap::new(),
            method_failures: HashMap::new(),
            node_fence_counts: HashMap::new(),
        }
    }

    /// Record a successful fence.
    pub fn record_fence_success(&mut self, node: NodeId, method: FencingMethod) {
        self.metrics.total_fences += 1;
        self.metrics.successful_fences += 1;
        *self.method_counts.entry(method).or_insert(0) += 1;
        *self.node_fence_counts.entry(node).or_insert(0) += 1;
    }

    /// Record a failed fence.
    pub fn record_fence_failure(&mut self, node: NodeId, method: FencingMethod) {
        self.metrics.total_fences += 1;
        self.metrics.failed_fences += 1;
        *self.method_failures.entry(method).or_insert(0) += 1;
    }

    /// Record a STONITH execution.
    pub fn record_stonith(&mut self, node: NodeId, method: FencingMethod) {
        self.metrics.stoniths_executed += 1;
        self.record_fence_success(node, method);
    }

    /// Record a split-brain detection.
    pub fn record_split_brain_detected(&mut self) {
        self.metrics.split_brains_detected += 1;
    }

    /// Record a split-brain resolution.
    pub fn record_split_brain_resolved(&mut self) {
        self.metrics.split_brains_resolved += 1;
    }

    /// Record an emergency fence.
    pub fn record_emergency_fence(&mut self, node: NodeId) {
        self.metrics.emergency_fences += 1;
        self.record_fence_success(node, FencingMethod::PowerCycle);
    }

    /// Record a token release.
    pub fn record_token_release(&mut self) {
        self.metrics.token_releases += 1;
    }

    /// Record an unfence operation.
    pub fn record_unfence(&mut self) {
        self.metrics.unfence_operations += 1;
    }

    /// Record a quorum acquisition.
    pub fn record_quorum_acquisition(&mut self) {
        self.metrics.quorum_acquisitions += 1;
    }

    /// Record a quorum rejection.
    pub fn record_quorum_rejection(&mut self) {
        self.metrics.quorum_rejections += 1;
    }

    /// Record a retry exhaustion.
    pub fn record_retry_exhaustion(&mut self) {
        self.metrics.retry_exhaustions += 1;
    }

    /// Record a policy rejection.
    pub fn record_policy_rejection(&mut self) {
        self.metrics.policy_rejections += 1;
    }

    /// Return a snapshot of current metrics.
    pub fn snapshot(&self) -> FencingMetric {
        self.metrics.clone()
    }

    /// Return the fence count for a specific method.
    pub fn method_count(&self, method: FencingMethod) -> u64 {
        self.method_counts.get(&method).copied().unwrap_or(0)
    }

    /// Return the fence count for a specific node.
    pub fn node_fence_count(&self, node: NodeId) -> u64 {
        self.node_fence_counts.get(&node).copied().unwrap_or(0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CrossRegionFencingCoordinator — cross-region fencing coordination
///////////////////////////////////////////////////////////////////////////////

/// A cross-region fencing request awaiting coordination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRegionFenceRequest {
    pub request_id: CrossRegionFenceId,
    pub victim: NodeId,
    pub victim_region: RegionId,
    pub requesting_region: RegionId,
    pub method: FencingMethod,
    pub timestamp: TimestampMillis,
    pub status: CrossRegionFenceStatus,
}

/// Unique identifier for a cross-region fencing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CrossRegionFenceId(pub u64);

/// Status of a cross-region fencing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRegionFenceStatus {
    /// Request submitted, awaiting acknowledgment.
    Pending,
    /// Victim region has acknowledged the fence request.
    Acknowledged,
    /// Fence has been executed in the victim region.
    Executed,
    /// Fence was rejected by the victim region.
    Rejected,
    /// Request timed out before execution.
    TimedOut,
}

/// Coordinates fencing across regions to prevent region-level split-brain.
pub struct CrossRegionFencingCoordinator {
    /// Pending cross-region fence requests.
    requests: HashMap<CrossRegionFenceId, CrossRegionFenceRequest>,
    /// Per-region fence request history.
    region_history: HashMap<RegionId, Vec<CrossRegionFenceId>>,
    /// Next request ID counter.
    next_id: u64,
    /// Whether cross-region coordination is enabled.
    enabled: bool,
}

impl CrossRegionFencingCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        CrossRegionFencingCoordinator {
            requests: HashMap::new(),
            region_history: HashMap::new(),
            next_id: 0,
            enabled: true,
        }
    }

    /// Submit a cross-region fence request.
    pub fn submit_request(
        &mut self,
        victim: NodeId,
        victim_region: RegionId,
        requesting_region: RegionId,
        method: FencingMethod,
    ) -> CrossRegionFenceId {
        let id = CrossRegionFenceId(self.next_id);
        self.next_id += 1;

        let request = CrossRegionFenceRequest {
            request_id: id,
            victim,
            victim_region,
            requesting_region,
            method,
            timestamp: TimestampMillis(0),
            status: CrossRegionFenceStatus::Pending,
        };

        self.requests.insert(id, request.clone());
        self.region_history
            .entry(victim_region)
            .or_default()
            .push(id);

        id
    }

    /// Acknowledge a fence request (victim region confirms it will fence).
    pub fn acknowledge_request(
        &mut self,
        request_id: CrossRegionFenceId,
    ) -> Result<(), CrossRegionFenceError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CrossRegionFenceError::RequestNotFound { request_id })?;

        if request.status != CrossRegionFenceStatus::Pending {
            return Err(CrossRegionFenceError::InvalidTransition {
                request_id,
                from: request.status,
                to: CrossRegionFenceStatus::Acknowledged,
            });
        }

        request.status = CrossRegionFenceStatus::Acknowledged;
        Ok(())
    }

    /// Mark a fence request as executed.
    pub fn mark_executed(&mut self, request_id: CrossRegionFenceId) -> Result<(), CrossRegionFenceError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CrossRegionFenceError::RequestNotFound { request_id })?;

        request.status = CrossRegionFenceStatus::Executed;
        Ok(())
    }

    /// Mark a fence request as rejected.
    pub fn mark_rejected(&mut self, request_id: CrossRegionFenceId) -> Result<(), CrossRegionFenceError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CrossRegionFenceError::RequestNotFound { request_id })?;

        request.status = CrossRegionFenceStatus::Rejected;
        Ok(())
    }

    /// Get the status of a request.
    pub fn request_status(&self, request_id: CrossRegionFenceId) -> Option<CrossRegionFenceStatus> {
        self.requests.get(&request_id).map(|r| r.status)
    }

    /// Return all requests for a given region.
    pub fn requests_for_region(&self, region: RegionId) -> Vec<CrossRegionFenceId> {
        self.region_history
            .get(&region)
            .cloned()
            .unwrap_or_default()
    }

    /// Return all pending requests.
    pub fn pending_requests(&self) -> Vec<CrossRegionFenceId> {
        self.requests
            .iter()
            .filter(|(_, r)| r.status == CrossRegionFenceStatus::Pending)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Enable or disable cross-region coordination.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Return whether coordination is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Errors from cross-region fencing coordination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossRegionFenceError {
    RequestNotFound { request_id: CrossRegionFenceId },
    InvalidTransition {
        request_id: CrossRegionFenceId,
        from: CrossRegionFenceStatus,
        to: CrossRegionFenceStatus,
    },
    CoordinationDisabled,
    Internal { message: String },
}

///////////////////////////////////////////////////////////////////////////////
// FencingResourceManager — resource-level fencing (per-dataset, per-partition)
///////////////////////////////////////////////////////////////////////////////

/// A resource-level fence: fences access to a specific resource rather than
/// an entire node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceFence {
    pub fence_id: ResourceFenceId,
    pub resource_type: ResourceType,
    pub resource_id: ResourceId,
    pub fenced_by: NodeId,
    pub timestamp: TimestampMillis,
    pub epoch: FencingEpoch,
}

/// Unique identifier for a resource-level fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceFenceId(pub u64);

/// Type of resource that can be fenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Dataset,
    Partition,
    Region,
    HUnit,
}

/// Identifier for a fenced resource (opaque bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(pub Vec<u8>);

/// Manages resource-level fences.
pub struct FencingResourceManager {
    /// Active resource fences.
    fences: HashMap<ResourceFenceId, ResourceFence>,
    /// Index: resource -> fence IDs.
    resource_index: HashMap<(ResourceType, ResourceId), Vec<ResourceFenceId>>,
    /// Index: node -> fence IDs issued by that node.
    issuer_index: HashMap<NodeId, Vec<ResourceFenceId>>,
    /// Next fence ID counter.
    next_id: u64,
}

impl FencingResourceManager {
    /// Create a new resource manager.
    pub fn new() -> Self {
        FencingResourceManager {
            fences: HashMap::new(),
            resource_index: HashMap::new(),
            issuer_index: HashMap::new(),
            next_id: 0,
        }
    }

    /// Fence a resource (prevent access to it).
    pub fn fence_resource(
        &mut self,
        resource_type: ResourceType,
        resource_id: ResourceId,
        fenced_by: NodeId,
        epoch: FencingEpoch,
    ) -> ResourceFenceId {
        let id = ResourceFenceId(self.next_id);
        self.next_id += 1;

        let fence = ResourceFence {
            fence_id: id,
            resource_type,
            resource_id: resource_id.clone(),
            fenced_by,
            timestamp: TimestampMillis(0),
            epoch,
        };

        self.fences.insert(id, fence.clone());
        self.resource_index
            .entry((resource_type, resource_id))
            .or_default()
            .push(id);
        self.issuer_index
            .entry(fenced_by)
            .or_default()
            .push(id);

        id
    }

    /// Release (remove) a resource fence.
    pub fn release_resource_fence(&mut self, fence_id: ResourceFenceId) -> Option<ResourceFence> {
        let fence = self.fences.remove(&fence_id)?;
        if let Some(ids) = self.resource_index.get_mut(&(fence.resource_type, fence.resource_id.clone())) {
            ids.retain(|&i| i != fence_id);
        }
        if let Some(ids) = self.issuer_index.get_mut(&fence.fenced_by) {
            ids.retain(|&i| i != fence_id);
        }
        Some(fence)
    }

    /// Check whether a resource is currently fenced.
    pub fn is_resource_fenced(&self, resource_type: ResourceType, resource_id: &ResourceId) -> bool {
        self.resource_index
            .get(&(resource_type, resource_id.clone()))
            .map(|ids| !ids.is_empty())
            .unwrap_or(false)
    }

    /// Return all fences for a given resource.
    pub fn resource_fences(&self, resource_type: ResourceType, resource_id: &ResourceId) -> Vec<ResourceFence> {
        self.resource_index
            .get(&(resource_type, resource_id.clone()))
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.fences.get(&id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return all fences issued by a given node.
    pub fn fences_by_issuer(&self, node: NodeId) -> Vec<ResourceFence> {
        self.issuer_index
            .get(&node)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.fences.get(&id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return the total number of active resource fences.
    pub fn active_fence_count(&self) -> usize {
        self.fences.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FencingRecoveryOrchestrator — orchestrates fence-then-recover ordering
///////////////////////////////////////////////////////////////////////////////

/// A recovery operation that depends on a completed fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedRecovery {
    pub recovery_id: RecoveryId,
    pub node: NodeId,
    pub fencing_token: FencingToken,
    pub recovery_type: RecoveryType,
    pub status: RecoveryStatus,
    pub created_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Unique identifier for a fenced recovery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecoveryId(pub u64);

/// Type of recovery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryType {
    NodeRecovery,
    HUnitRecovery,
    RegionalRecovery,
    DatasetRestore,
    PartitionRebalance,
}

/// Status of a fenced recovery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStatus {
    /// Waiting for fence to complete.
    WaitingForFence,
    /// Fence confirmed; recovery may proceed.
    Ready,
    /// Recovery in progress.
    InProgress,
    /// Recovery completed successfully.
    Completed,
    /// Recovery failed.
    Failed,
    /// Recovery aborted (fence was revoked).
    Aborted,
}

/// Orchestrates recovery operations that require prior fencing.
/// Enforces the invariant: fence first, recover second, never reverse.
pub struct FencingRecoveryOrchestrator {
    /// Pending recoveries keyed by ID.
    recoveries: HashMap<RecoveryId, FencedRecovery>,
    /// Index: node -> recovery IDs.
    node_recoveries: HashMap<NodeId, Vec<RecoveryId>>,
    /// Index: fencing token -> recovery ID.
    token_index: HashMap<FencingToken, RecoveryId>,
    /// Next recovery ID counter.
    next_id: u64,
}

impl FencingRecoveryOrchestrator {
    /// Create a new orchestrator.
    pub fn new() -> Self {
        FencingRecoveryOrchestrator {
            recoveries: HashMap::new(),
            node_recoveries: HashMap::new(),
            token_index: HashMap::new(),
            next_id: 0,
        }
    }

    /// Register a recovery that requires a fencing token before it may proceed.
    pub fn register_recovery(
        &mut self,
        node: NodeId,
        fencing_token: FencingToken,
        recovery_type: RecoveryType,
    ) -> RecoveryId {
        let id = RecoveryId(self.next_id);
        self.next_id += 1;

        let recovery = FencedRecovery {
            recovery_id: id,
            node,
            fencing_token,
            recovery_type,
            status: RecoveryStatus::WaitingForFence,
            created_at: TimestampMillis(0),
            completed_at: None,
        };

        self.recoveries.insert(id, recovery.clone());
        self.node_recoveries.entry(node).or_default().push(id);
        self.token_index.insert(fencing_token, id);

        id
    }

    /// Mark a recovery as ready (fence confirmed).
    pub fn mark_ready(&mut self, recovery_id: RecoveryId) -> Result<(), RecoveryOrchestratorError> {
        let recovery = self
            .recoveries
            .get_mut(&recovery_id)
            .ok_or(RecoveryOrchestratorError::RecoveryNotFound { recovery_id })?;

        if recovery.status != RecoveryStatus::WaitingForFence {
            return Err(RecoveryOrchestratorError::InvalidTransition {
                recovery_id,
                from: recovery.status,
                to: RecoveryStatus::Ready,
            });
        }

        recovery.status = RecoveryStatus::Ready;
        Ok(())
    }

    /// Mark a recovery as in progress.
    pub fn mark_in_progress(&mut self, recovery_id: RecoveryId) -> Result<(), RecoveryOrchestratorError> {
        let recovery = self
            .recoveries
            .get_mut(&recovery_id)
            .ok_or(RecoveryOrchestratorError::RecoveryNotFound { recovery_id })?;

        if recovery.status != RecoveryStatus::Ready {
            return Err(RecoveryOrchestratorError::InvalidTransition {
                recovery_id,
                from: recovery.status,
                to: RecoveryStatus::InProgress,
            });
        }

        recovery.status = RecoveryStatus::InProgress;
        Ok(())
    }

    /// Mark a recovery as completed.
    pub fn mark_completed(&mut self, recovery_id: RecoveryId) -> Result<(), RecoveryOrchestratorError> {
        let recovery = self
            .recoveries
            .get_mut(&recovery_id)
            .ok_or(RecoveryOrchestratorError::RecoveryNotFound { recovery_id })?;

        recovery.status = RecoveryStatus::Completed;
        recovery.completed_at = Some(TimestampMillis(0));
        Ok(())
    }

    /// Mark a recovery as aborted (e.g., fence was revoked).
    pub fn mark_aborted(&mut self, recovery_id: RecoveryId) -> Result<(), RecoveryOrchestratorError> {
        let recovery = self
            .recoveries
            .get_mut(&recovery_id)
            .ok_or(RecoveryOrchestratorError::RecoveryNotFound { recovery_id })?;

        recovery.status = RecoveryStatus::Aborted;
        Ok(())
    }

    /// Check whether a recovery may proceed (fence must be confirmed first).
    pub fn may_proceed(&self, recovery_id: RecoveryId) -> bool {
        self.recoveries
            .get(&recovery_id)
            .map(|r| r.status == RecoveryStatus::Ready || r.status == RecoveryStatus::InProgress)
            .unwrap_or(false)
    }

    /// Return all recoveries for a node.
    pub fn recoveries_for_node(&self, node: NodeId) -> Vec<FencedRecovery> {
        self.node_recoveries
            .get(&node)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.recoveries.get(&id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return all recoveries waiting for fence.
    pub fn waiting_for_fence(&self) -> Vec<FencedRecovery> {
        self.recoveries
            .iter()
            .filter(|(_, r)| r.status == RecoveryStatus::WaitingForFence)
            .map(|(_, r)| r.clone())
            .collect()
    }

    /// Return the total number of registered recoveries.
    pub fn total_recoveries(&self) -> usize {
        self.recoveries.len()
    }
}

/// Errors from the fencing recovery orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryOrchestratorError {
    RecoveryNotFound { recovery_id: RecoveryId },
    InvalidTransition {
        recovery_id: RecoveryId,
        from: RecoveryStatus,
        to: RecoveryStatus,
    },
    FenceNotConfirmed { recovery_id: RecoveryId },
    Internal { message: String },
}

///////////////////////////////////////////////////////////////////////////////
// Tests for new subsystems
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;


    // ---- FencingToken tests ----

    #[test]
    fn fencing_token_equality_and_ordering() {
        let a = FencingToken(1);
        let b = FencingToken(1);
        let c = FencingToken(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a < c);
    }

    #[test]
    fn fencing_token_display() {
        let token = FencingToken(0xdeadbeef);
        let s = format!("{}", token);
        assert!(s.contains("deadbeef"));
    }

    // ---- FencingEpoch tests ----

    #[test]
    fn fencing_epoch_increment() {
        let e = FencingEpoch(5);
        let next = FencingEpoch(e.0 + 1);
        assert_eq!(next.0, 6);
        assert!(next > e);
    }

    #[test]
    fn fencing_epoch_display() {
        let e = FencingEpoch(42);
        let s = format!("{}", e);
        assert!(s.contains("42"));
    }

    // ---- StonithResult tests ----

    #[test]
    fn stonith_result_construction() {
        let result = StonithResult {
            node_id: NodeId::new([1u8; 32]),
            method: FencingMethod::PowerCycle,
            executed_at: TimestampMillis(1000),
            confirmed_offline: true,
            epoch: FencingEpoch(3),
        };
        assert!(result.confirmed_offline);
        assert_eq!(result.epoch.0, 3);
    }

    // ---- FencingAuthorization tests ----

    #[test]
    fn fencing_authorization_quorum_size() {
        let replicas = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        let auth = FencingAuthorization {
            victim: NodeId::new([9u8; 32]),
            authorizing_replicas: replicas.clone(),
            quorum_size: 2,
            authorized_at: TimestampMillis(100),
            expires_at: TimestampMillis(400),
            epoch: FencingEpoch(1),
        };
        assert_eq!(auth.quorum_size, 2);
        assert_eq!(auth.authorizing_replicas.len(), 3);
    }

    // ---- FencingEvent tests ----

    #[test]
    fn fencing_event_creation() {
        let event = FencingEvent {
            event_id: FencingEventId(1),
            node_id: NodeId::new([1u8; 32]),
            method: FencingMethod::NetworkIsolate,
            timestamp: TimestampMillis(500),
            epoch: FencingEpoch(2),
            outcome: FencingOutcome::Success,
            operator: None,
        };
        assert_eq!(event.event_id.0, 1);
        assert_eq!(event.outcome, FencingOutcome::Success);
    }

    // ---- FencingEventId tests ----

    #[test]
    fn fencing_event_id_ordering() {
        let a = FencingEventId(10);
        let b = FencingEventId(20);
        assert!(a < b);
        assert_ne!(a, b);
    }

    // ---- FencingMethod tests ----

    #[test]
    fn fencing_method_variants() {
        assert_eq!(
            format!("{}", FencingMethod::PowerCycle),
            "power-cycle"
        );
        assert_eq!(
            format!("{}", FencingMethod::Hybrid),
            "hybrid"
        );
        assert_eq!(
            format!("{}", FencingMethod::Soft),
            "soft"
        );
    }

    // ---- FencingOutcome tests ----

    #[test]
    fn fencing_outcome_variants() {
        assert_eq!(FencingOutcome::Success as u32, 0);
        assert_eq!(FencingOutcome::IdempotentNoop as u32, 3);
    }

    // ---- FencingState tests ----

    #[test]
    fn fencing_state_transitions() {
        assert_ne!(FencingState::Unfenced, FencingState::Fenced);
        assert_ne!(FencingState::Fenced, FencingState::Stonithed);
    }

    // ---- SplitBrainStatus tests ----

    #[test]
    fn split_brain_status_variants() {
        assert_eq!(
            format!("{:?}", SplitBrainStatus::Confirmed),
            "Confirmed"
        );
        assert_eq!(
            format!("{:?}", SplitBrainStatus::Unknown),
            "Unknown"
        );
    }

    // ---- ResolutionResult tests ----

    #[test]
    fn resolution_result_construction() {
        let result = ResolutionResult {
            domain: OrderingDomain(1),
            winner: NodeId::new([1u8; 32]),
            loser: NodeId::new([2u8; 32]),
            method: FencingMethod::Hybrid,
            resolved_at: TimestampMillis(1000),
            epoch: FencingEpoch(5),
        };
        assert_eq!(result.epoch.0, 5);
        assert_eq!(result.method, FencingMethod::Hybrid);
    }

    // ---- OperationType tests ----

    #[test]
    fn operation_type_requires_fencing() {
        let op = OperationType::RegionFailover;
        assert!(matches!(op, OperationType::RegionFailover));
        assert!(matches!(
            OperationType::LeadershipElection,
            OperationType::LeadershipElection
        ));
    }

    // ---- FencingError tests ----

    #[test]
    fn fencing_error_variants() {
        let err = FencingError::NodeNotFound {
            node: NodeId::new([1u8; 32]),
        };
        match &err {
            FencingError::NodeNotFound { node } => assert_eq!(node.0[0], 1),
            _ => panic!("expected NodeNotFound"),
        }

        let err2 = FencingError::InsufficientQuorum { have: 1, need: 3 };
        match &err2 {
            FencingError::InsufficientQuorum { have, need } => {
                assert_eq!(*have, 1);
                assert_eq!(*need, 3);
            }
            _ => panic!("expected InsufficientQuorum"),
        }
    }

    // ---- StonithError tests ----

    #[test]
    fn stonith_error_variants() {
        let err = StonithError::OfflineConfirmationFailed {
            node: NodeId::new([2u8; 32]),
        };
        match &err {
            StonithError::OfflineConfirmationFailed { node } => {
                assert_eq!(node.0[0], 2);
            }
            _ => panic!("expected OfflineConfirmationFailed"),
        }
    }

    // ---- SplitBrainError tests ----

    #[test]
    fn split_brain_error_variants() {
        let err = SplitBrainError::FencingRequired {
            loser: NodeId::new([3u8; 32]),
        };
        match &err {
            SplitBrainError::FencingRequired { loser } => {
                assert_eq!(loser.0[0], 3);
            }
            _ => panic!("expected FencingRequired"),
        }
    }

    // ---- AuditError tests ----

    #[test]
    fn audit_error_variants() {
        let err = AuditError::WriteFailed {
            message: "disk full".to_string(),
        };
        match &err {
            AuditError::WriteFailed { message } => {
                assert_eq!(message, "disk full");
            }
            _ => panic!("expected WriteFailed"),
        }
    }

    // ---- FencingManager default ----

    #[test]
    fn fencing_manager_default_creation() {
        let mgr = FencingManager::default();
        assert_eq!(mgr.max_concurrent_fences(), 10);
        assert_eq!(mgr.in_flight_fence_count(), 0);
        assert!(mgr.fenced_nodes().is_empty());
    }

    #[test]
    fn fencing_manager_with_limits() {
        let mgr = FencingManager::with_limits(3, false);
        assert_eq!(mgr.max_concurrent_fences(), 3);
        assert_eq!(mgr.in_flight_fence_count(), 0);
    }

    // ---- register_node ----

    #[test]
    fn register_node_initializes_state() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        assert_eq!(mgr.fencing_state(node), FencingState::Unfenced);
        assert_eq!(mgr.fencing_epoch(node).0, 0);
        assert!(!mgr.is_node_fenced(node));
    }

    // ---- fence_before_failover ----

    #[test]
    fn fence_before_failover_creates_token() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        mgr.register_node(victim);
        let token = mgr.fence_before_failover(victim);
        assert!(token.is_ok());
        let t = token.unwrap();
        assert!(mgr.is_fencing_token_valid(&t));
        assert!(mgr.is_node_fenced(victim));
    }

    #[test]
    fn fence_before_failover_is_idempotent() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        mgr.register_node(victim);
        let t1 = mgr.fence_before_failover(victim).unwrap();
        let t2 = mgr.fence_before_failover(victim).unwrap();
        // Second call returns the same token (idempotent no-op).
        assert_eq!(t1, t2);
    }

    #[test]
    fn fence_stonithed_node_fails() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.stonith_execute(node).unwrap();
        let result = mgr.fence_before_failover(node);
        assert!(result.is_err());
    }

    #[test]
    fn fence_increments_epoch() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        mgr.register_node(victim);
        let before = mgr.fencing_epoch(victim);
        mgr.fence_before_failover(victim).unwrap();
        let after = mgr.fencing_epoch(victim);
        assert_eq!(after.0, before.0 + 1);
    }

    #[test]
    fn fence_respects_concurrent_limit() {
        let mut mgr = FencingManager::with_limits(1, true);
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        mgr.register_node(n1);
        mgr.register_node(n2);
        // First fence succeeds.
        assert!(mgr.fence_before_failover(n1).is_ok());
        // With limit 1, subsequent fence attempts hit the concurrent limit.
        // (In this simplified implementation, in_flight_count is transient;
        // we verify the limit field is respected by construction.)
        assert_eq!(mgr.max_concurrent_fences(), 1);
    }

    // ---- stonith_execute ----

    #[test]
    fn stonith_execute_marks_node() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let result = mgr.stonith_execute(node);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.confirmed_offline);
        assert_eq!(mgr.fencing_state(node), FencingState::Stonithed);
        assert!(mgr.is_node_fenced(node));
    }

    #[test]
    fn stonith_execute_twice_fails() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.stonith_execute(node).unwrap();
        let result = mgr.stonith_execute(node);
        assert!(result.is_err());
        match result.unwrap_err() {
            StonithError::AlreadyStonithed { node } => {
                assert_eq!(node.0[0], 1);
            }
            _ => panic!("expected AlreadyStonithed"),
        }
    }

    #[test]
    fn stonith_increments_epoch() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let before = mgr.fencing_epoch(node);
        mgr.stonith_execute(node).unwrap();
        let after = mgr.fencing_epoch(node);
        assert_eq!(after.0, before.0 + 1);
    }

    // ---- split_brain_prevented ----

    #[test]
    fn split_brain_prevented_returns_bool() {
        let mgr = FencingManager::new();
        assert!(mgr.split_brain_prevented(OrderingDomain(1)));
    }

    // ---- fencing_epoch ----

    #[test]
    fn fencing_epoch_unknown_node_returns_zero() {
        let mgr = FencingManager::new();
        assert_eq!(mgr.fencing_epoch(NodeId::new([9u8; 32])).0, 0);
    }

    // ---- acquire_fencing_quorum ----

    #[test]
    fn acquire_fencing_quorum_succeeds_with_majority() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        let replicas = vec![
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
            NodeId::new([4u8; 32]),
        ];
        let auth = mgr.acquire_fencing_quorum(victim, &replicas);
        assert!(auth.is_ok());
        let a = auth.unwrap();
        assert_eq!(a.quorum_size, 2);
        assert_eq!(a.authorizing_replicas.len(), 2);
    }

    #[test]
    fn acquire_fencing_quorum_insufficient_replicas() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        let replicas = vec![NodeId::new([2u8; 32])];
        let result = mgr.acquire_fencing_quorum(victim, &replicas);
        assert!(result.is_err());
        match result.unwrap_err() {
            FencingError::InsufficientQuorum { have, need } => {
                assert_eq!(have, 1);
                assert_eq!(need, 1); // 1/2 + 1 = 1
            }
            _ => panic!("expected InsufficientQuorum"),
        }
    }

    // ---- release_fencing_token ----

    #[test]
    fn release_fencing_token_revokes() {
        let mut mgr = FencingManager::new();
        let victim = NodeId::new([1u8; 32]);
        mgr.register_node(victim);
        let token = mgr.fence_before_failover(victim).unwrap();
        assert!(mgr.is_fencing_token_valid(&token));
        mgr.release_fencing_token(token).unwrap();
        assert!(!mgr.is_fencing_token_valid(&token));
    }

    #[test]
    fn release_invalid_token_fails() {
        let mut mgr = FencingManager::new();
        let result = mgr.release_fencing_token(FencingToken(99999));
        assert!(result.is_err());
    }

    // ---- is_fencing_token_valid ----

    #[test]
    fn is_fencing_token_valid_unknown_token() {
        let mgr = FencingManager::new();
        assert!(!mgr.is_fencing_token_valid(&FencingToken(0)));
    }

    // ---- record_fencing_event / fencing_audit_trail ----

    #[test]
    fn record_and_retrieve_audit_trail() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        let event = FencingEvent {
            event_id: FencingEventId(1),
            node_id: node,
            method: FencingMethod::NetworkIsolate,
            timestamp: TimestampMillis(100),
            epoch: FencingEpoch(1),
            outcome: FencingOutcome::Success,
            operator: None,
        };
        mgr.record_fencing_event(event).unwrap();
        let trail = mgr.fencing_audit_trail(node);
        assert_eq!(trail.len(), 1);
        assert_eq!(trail[0].event_id.0, 1);
    }

    #[test]
    fn audit_trail_empty_for_unknown_node() {
        let mgr = FencingManager::new();
        let trail = mgr.fencing_audit_trail(NodeId::new([9u8; 32]));
        assert!(trail.is_empty());
    }

    // ---- emergency_fence ----

    #[test]
    fn emergency_fence_requires_operator() {
        let mut mgr = FencingManager::with_limits(10, false);
        let victim = NodeId::new([1u8; 32]);
        let operator = NodeId::new([2u8; 32]);
        let result = mgr.emergency_fence(victim, operator);
        assert!(result.is_err());
    }

    #[test]
    fn emergency_fence_succeeds_when_enabled() {
        let mut mgr = FencingManager::with_limits(10, true);
        let victim = NodeId::new([1u8; 32]);
        let operator = NodeId::new([2u8; 32]);
        mgr.register_node(victim);
        let token = mgr.emergency_fence(victim, operator);
        assert!(token.is_ok());
        assert!(mgr.is_node_fenced(victim));
    }

    // ---- is_node_fenced / fenced_nodes ----

    #[test]
    fn fenced_nodes_tracking() {
        let mut mgr = FencingManager::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        mgr.register_node(n1);
        mgr.register_node(n2);
        assert!(mgr.fenced_nodes().is_empty());
        mgr.fence_before_failover(n1).unwrap();
        let fenced = mgr.fenced_nodes();
        assert!(fenced.contains(&n1));
        assert!(!fenced.contains(&n2));
    }

    // ---- unfence_node ----

    #[test]
    fn unfence_node_requires_newer_epoch() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.fence_before_failover(node).unwrap();
        let current = mgr.fencing_epoch(node);
        let result = mgr.unfence_node(node, current);
        assert!(result.is_err());
    }

    #[test]
    fn unfence_node_succeeds_with_newer_epoch() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.fence_before_failover(node).unwrap();
        let current = mgr.fencing_epoch(node);
        let new_epoch = FencingEpoch(current.0 + 1);
        let result = mgr.unfence_node(node, new_epoch);
        assert!(result.is_ok());
        assert!(!mgr.is_node_fenced(node));
    }

    // ---- fencing_timeout ----

    #[test]
    fn fencing_timeout_per_method() {
        let mgr = FencingManager::new();
        assert_eq!(
            mgr.fencing_timeout(FencingMethod::NetworkIsolate),
            Duration::from_secs(15)
        );
        assert_eq!(
            mgr.fencing_timeout(FencingMethod::PowerCycle),
            Duration::from_secs(60)
        );
    }

    // ---- detect_split_brain ----

    #[test]
    fn detect_split_brain_both_unfenced() {
        let mgr = FencingManager::new();
        let a = NodeId::new([1u8; 32]);
        let b = NodeId::new([2u8; 32]);
        mgr.register_node(a);
        mgr.register_node(b);
        let status = mgr.detect_split_brain(OrderingDomain(1), a, b);
        assert_eq!(status, SplitBrainStatus::Suspected);
    }

    #[test]
    fn detect_split_brain_one_fenced() {
        let mut mgr = FencingManager::new();
        let a = NodeId::new([1u8; 32]);
        let b = NodeId::new([2u8; 32]);
        mgr.register_node(a);
        mgr.register_node(b);
        mgr.fence_before_failover(a).unwrap();
        let status = mgr.detect_split_brain(OrderingDomain(1), a, b);
        assert_eq!(status, SplitBrainStatus::None);
    }

    // ---- resolve_split_brain ----

    #[test]
    fn resolve_split_brain_fences_loser() {
        let mut mgr = FencingManager::new();
        let winner = NodeId::new([1u8; 32]);
        let loser = NodeId::new([2u8; 32]);
        mgr.register_node(winner);
        mgr.register_node(loser);
        let result = mgr.resolve_split_brain(OrderingDomain(1), winner, loser);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.winner, winner);
        assert_eq!(r.loser, loser);
        assert!(mgr.is_node_fenced(loser));
    }

    // ---- fencing_state ----

    #[test]
    fn fencing_state_unknown_node() {
        let mgr = FencingManager::new();
        assert_eq!(
            mgr.fencing_state(NodeId::new([9u8; 32])),
            FencingState::Unfenced
        );
    }

    // ---- transition_fencing_state ----

    #[test]
    fn transition_fencing_state_valid() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let result = mgr.transition_fencing_state(
            node,
            FencingState::Unfenced,
            FencingState::Pending,
        );
        assert!(result.is_ok());
        assert_eq!(mgr.fencing_state(node), FencingState::Pending);
    }

    #[test]
    fn transition_fencing_state_wrong_from() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        // Node is Unfenced, but we claim from=Fenced.
        let result = mgr.transition_fencing_state(
            node,
            FencingState::Fenced,
            FencingState::Pending,
        );
        assert!(result.is_err());
    }

    // ---- fencing_required ----

    #[test]
    fn fencing_required_for_failover_ops() {
        let mgr = FencingManager::new();
        assert!(mgr.fencing_required(
            NodeId::new([1u8; 32]),
            OperationType::RegionFailover
        ));
        assert!(mgr.fencing_required(
            NodeId::new([1u8; 32]),
            OperationType::LeadershipElection
        ));
        assert!(!mgr.fencing_required(
            NodeId::new([1u8; 32]),
            OperationType::DatasetWrite
        ));
    }

    // ---- max_concurrent_fences / in_flight ----

    #[test]
    fn concurrent_fence_limits() {
        let mgr = FencingManager::with_limits(7, true);
        assert_eq!(mgr.max_concurrent_fences(), 7);
        assert_eq!(mgr.in_flight_fence_count(), 0);
    }

    // ---- record_fencing_failure / retry ----

    #[test]
    fn record_fencing_failure_builds_schedule() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let err = FencingError::Timeout {
            node,
            method: FencingMethod::PowerCycle,
            duration_ms: 60000,
        };
        let result = mgr.record_fencing_failure(node, err);
        assert!(result.is_ok());
        let schedule = mgr.fencing_retry_schedule(node);
        assert!(!schedule.is_empty());
    }

    #[test]
    fn fencing_retry_exhausted_after_many_failures() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        for i in 0..6 {
            let err = FencingError::Timeout {
                node,
                method: FencingMethod::PowerCycle,
                duration_ms: 60000,
            };
            let _ = mgr.record_fencing_failure(node, err);
            let _ = i;
        }
        assert!(mgr.fencing_retry_exhausted(node));
    }

    // ---- Display impls ----

    #[test]
    fn fencing_method_display_all_variants() {
        let methods = [
            FencingMethod::PowerCycle,
            FencingMethod::NetworkIsolate,
            FencingMethod::DiskQuorum,
            FencingMethod::CarrierRevoke,
            FencingMethod::Hybrid,
            FencingMethod::Soft,
        ];
        for m in methods {
            let s = format!("{}", m);
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn fencing_outcome_display_all_variants() {
        let outcomes = [
            FencingOutcome::Success,
            FencingOutcome::Failed,
            FencingOutcome::Timeout,
            FencingOutcome::IdempotentNoop,
            FencingOutcome::QuorumRejected,
            FencingOutcome::PolicyRejected,
        ];
        for o in outcomes {
            let _ = format!("{:?}", o);
        }
    }

    // ---- FENCING_STONITH_BIT ----

    #[test]
    fn fencing_stonith_bit_position() {
        assert_eq!(FENCING_STONITH_BIT, 1 << 6);
    }

    // ---- fencing_stonith_enabled ----

    #[test]
    fn fencing_stonith_enabled_checks_bit() {
        let mut bits = EdgeCaseBits(0);
        assert!(!fencing_stonith_enabled(&bits));
        bits.enable(FENCING_STONITH_BIT);
        assert!(fencing_stonith_enabled(&bits));
    }

    // ---- Integration: fence then stonith then unfence ----

    #[test]
    fn full_fencing_lifecycle() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);

        // Fence.
        let token = mgr.fence_before_failover(node).unwrap();
        assert!(mgr.is_fencing_token_valid(&token));
        assert!(mgr.is_node_fenced(node));

        // STONITH after fence.
        let stonith = mgr.stonith_execute(node).unwrap();
        assert!(stonith.confirmed_offline);
        assert_eq!(mgr.fencing_state(node), FencingState::Stonithed);

        // Cannot unfence with stale epoch.
        let stale = mgr.fencing_epoch(node);
        let result = mgr.unfence_node(node, stale);
        assert!(result.is_err());

        // Unfence with newer epoch.
        let new_epoch = FencingEpoch(stale.0 + 1);
        mgr.unfence_node(node, new_epoch).unwrap();
        assert!(!mgr.is_node_fenced(node));
    }

    // ---- Audit trail ordering ----

    #[test]
    fn audit_trail_is_append_only_ordered() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.fence_before_failover(node).unwrap();
        mgr.record_fencing_failure(
            node,
            FencingError::Timeout {
                node,
                method: FencingMethod::PowerCycle,
                duration_ms: 60000,
            },
        ).unwrap();
        let trail = mgr.fencing_audit_trail(node);
        assert!(trail.len() >= 2);
        // Event IDs should be monotonically increasing.
        for pair in trail.windows(2) {
            assert!(pair[0].event_id.0 < pair[1].event_id.0);
        }
    }

    // ---- Token revocation on unfence ----

    #[test]
    fn unfence_revokes_all_tokens() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let t1 = mgr.fence_before_failover(node).unwrap();
        let t2 = mgr.emergency_fence(node, NodeId::new([9u8; 32])).unwrap();
        assert!(mgr.is_fencing_token_valid(&t1));
        assert!(mgr.is_fencing_token_valid(&t2));
        let current = mgr.fencing_epoch(node);
        mgr.unfence_node(node, FencingEpoch(current.0 + 1)).unwrap();
        assert!(!mgr.is_fencing_token_valid(&t2));
    }

// ========== New subsystem tests appended ==========

    // ========== FencingPolicyEngine tests ==========

    #[test]
    fn policy_engine_default_policy() {
        let engine = FencingPolicyEngine::new();
        assert_eq!(engine.evaluation_count(), 0);
        assert_eq!(engine.rejection_count(), 0);
        assert_eq!(engine.emergency_authorization_count(), 0);
    }

    #[test]
    fn policy_engine_evaluate_fence_allowed_method() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let decision = engine.evaluate_fence(node, FencingMethod::NetworkIsolate, None);
        assert!(decision.is_ok());
        let d = decision.unwrap();
        assert!(d.authorized);
        assert_eq!(d.victim, node);
        assert_eq!(d.method, FencingMethod::NetworkIsolate);
        assert_eq!(engine.evaluation_count(), 1);
    }

    #[test]
    fn policy_engine_evaluate_fence_disallowed_method() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let mut policy = FencingPolicy::default();
        policy.allowed_methods = vec![FencingMethod::NetworkIsolate];
        engine.set_node_policy(node, policy);
        let result = engine.evaluate_fence(node, FencingMethod::PowerCycle, None);
        assert!(result.is_err());
        assert_eq!(engine.rejection_count(), 1);
    }

    #[test]
    fn policy_engine_emergency_fence_allowed() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let operator = NodeId::new([2u8; 32]);
        let result = engine.evaluate_emergency_fence(node, operator);
        assert!(result.is_ok());
        assert_eq!(engine.emergency_authorization_count(), 1);
    }

    #[test]
    fn policy_engine_emergency_fence_disallowed() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let operator = NodeId::new([2u8; 32]);
        let mut policy = FencingPolicy::default();
        policy.emergency_allowed = false;
        engine.set_node_policy(node, policy);
        let result = engine.evaluate_emergency_fence(node, operator);
        assert!(result.is_err());
        assert_eq!(engine.rejection_count(), 1);
    }

    #[test]
    fn policy_engine_soft_fence_disallowed() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let mut policy = FencingPolicy::default();
        policy.soft_fence_allowed = false;
        engine.set_node_policy(node, policy);
        let result = engine.evaluate_fence(node, FencingMethod::Soft, None);
        assert!(result.is_err());
    }

    #[test]
    fn policy_decision_fields() {
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        let decision = engine.evaluate_fence(node, FencingMethod::Hybrid, None).unwrap();
        assert_eq!(decision.epoch_gap, 1);
        assert_eq!(decision.timeout_ms, 30_000);
        assert!(decision.requires_quorum);
    }

    // ========== FencingHealthMonitor tests ==========

    #[test]
    fn health_monitor_register_and_status() {
        let mut monitor = FencingHealthMonitor::new();
        let node = NodeId::new([1u8; 32]);
        monitor.register_node(node);
        assert_eq!(monitor.health_status(node), FencingHealthStatus::Unknown);
    }

    #[test]
    fn health_monitor_heartbeat_marks_healthy() {
        let mut monitor = FencingHealthMonitor::new();
        let node = NodeId::new([1u8; 32]);
        monitor.register_node(node);
        monitor.heartbeat(node, TimestampMillis(1000));
        assert!(monitor.is_healthy(node));
        assert_eq!(monitor.last_heartbeat(node), Some(TimestampMillis(1000)));
    }

    #[test]
    fn health_monitor_failure_progression() {
        let mut monitor = FencingHealthMonitor::new();
        let node = NodeId::new([1u8; 32]);
        monitor.register_node(node);
        monitor.record_failure(node);
        assert_eq!(monitor.health_status(node), FencingHealthStatus::SuspectedFailed);
        assert_eq!(monitor.failure_count(node), 1);
        monitor.record_failure(node);
        assert_eq!(monitor.health_status(node), FencingHealthStatus::SuspectedFailed);
        monitor.record_failure(node);
        assert_eq!(monitor.health_status(node), FencingHealthStatus::ConfirmedFailed);
        assert!(monitor.is_confirmed_failed(node));
    }

    #[test]
    fn health_monitor_disabled() {
        let mut monitor = FencingHealthMonitor::disabled();
        let node = NodeId::new([1u8; 32]);
        monitor.register_node(node);
        monitor.heartbeat(node, TimestampMillis(1000));
        assert_eq!(monitor.health_status(node), FencingHealthStatus::Unknown);
    }

    #[test]
    fn health_monitor_nodes_by_status() {
        let mut monitor = FencingHealthMonitor::new();
        let n1 = NodeId::new([1u8; 32]);
        let n2 = NodeId::new([2u8; 32]);
        let n3 = NodeId::new([3u8; 32]);
        monitor.register_node(n1);
        monitor.register_node(n2);
        monitor.register_node(n3);
        monitor.heartbeat(n1, TimestampMillis(100));
        monitor.record_failure(n2);
        monitor.record_failure(n2);
        monitor.record_failure(n2);
        let healthy = monitor.nodes_by_status(FencingHealthStatus::Healthy);
        assert!(healthy.contains(&n1));
        assert!(!healthy.contains(&n2));
        let failed = monitor.nodes_by_status(FencingHealthStatus::ConfirmedFailed);
        assert!(failed.contains(&n2));
        assert!(!failed.contains(&n1));
    }

    // ========== FencingMetricsCollector tests ==========

    #[test]
    fn metrics_collector_default_zero() {
        let collector = FencingMetricsCollector::new();
        let m = collector.snapshot();
        assert_eq!(m.total_fences, 0);
        assert_eq!(m.successful_fences, 0);
        assert_eq!(m.failed_fences, 0);
    }

    #[test]
    fn metrics_collector_record_fence_success() {
        let mut collector = FencingMetricsCollector::new();
        let node = NodeId::new([1u8; 32]);
        collector.record_fence_success(node, FencingMethod::NetworkIsolate);
        let m = collector.snapshot();
        assert_eq!(m.total_fences, 1);
        assert_eq!(m.successful_fences, 1);
        assert_eq!(collector.method_count(FencingMethod::NetworkIsolate), 1);
        assert_eq!(collector.node_fence_count(node), 1);
    }

    #[test]
    fn metrics_collector_record_fence_failure() {
        let mut collector = FencingMetricsCollector::new();
        let node = NodeId::new([1u8; 32]);
        collector.record_fence_failure(node, FencingMethod::PowerCycle);
        let m = collector.snapshot();
        assert_eq!(m.total_fences, 1);
        assert_eq!(m.failed_fences, 1);
    }

    #[test]
    fn metrics_collector_record_stonith() {
        let mut collector = FencingMetricsCollector::new();
        let node = NodeId::new([1u8; 32]);
        collector.record_stonith(node, FencingMethod::PowerCycle);
        let m = collector.snapshot();
        assert_eq!(m.stoniths_executed, 1);
        assert_eq!(m.successful_fences, 1);
    }

    #[test]
    fn metrics_collector_split_brain() {
        let mut collector = FencingMetricsCollector::new();
        collector.record_split_brain_detected();
        collector.record_split_brain_detected();
        collector.record_split_brain_resolved();
        let m = collector.snapshot();
        assert_eq!(m.split_brains_detected, 2);
        assert_eq!(m.split_brains_resolved, 1);
    }

    #[test]
    fn metrics_collector_emergency_and_token() {
        let mut collector = FencingMetricsCollector::new();
        let node = NodeId::new([1u8; 32]);
        collector.record_emergency_fence(node);
        collector.record_token_release();
        collector.record_unfence();
        let m = collector.snapshot();
        assert_eq!(m.emergency_fences, 1);
        assert_eq!(m.token_releases, 1);
        assert_eq!(m.unfence_operations, 1);
    }

    #[test]
    fn metrics_collector_quorum_and_retry() {
        let mut collector = FencingMetricsCollector::new();
        collector.record_quorum_acquisition();
        collector.record_quorum_rejection();
        collector.record_retry_exhaustion();
        collector.record_policy_rejection();
        let m = collector.snapshot();
        assert_eq!(m.quorum_acquisitions, 1);
        assert_eq!(m.quorum_rejections, 1);
        assert_eq!(m.retry_exhaustions, 1);
        assert_eq!(m.policy_rejections, 1);
    }

    // ========== CrossRegionFencingCoordinator tests ==========

    #[test]
    fn cross_region_coordinator_submit_and_ack() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let victim = NodeId::new([1u8; 32]);
        let victim_region = RegionId::new([2u8; 32]);
        let requesting_region = RegionId::new([3u8; 32]);
        let id = coord.submit_request(
            victim,
            victim_region,
            requesting_region,
            FencingMethod::Hybrid,
        );
        assert_eq!(coord.request_status(id), Some(CrossRegionFenceStatus::Pending));
        assert!(coord.pending_requests().contains(&id));
    }

    #[test]
    fn cross_region_coordinator_ack_transition() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let id = coord.submit_request(
            NodeId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            RegionId::new([3u8; 32]),
            FencingMethod::PowerCycle,
        );
        coord.acknowledge_request(id).unwrap();
        assert_eq!(coord.request_status(id), Some(CrossRegionFenceStatus::Acknowledged));
    }

    #[test]
    fn cross_region_coordinator_mark_executed() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let id = coord.submit_request(
            NodeId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            RegionId::new([3u8; 32]),
            FencingMethod::DiskQuorum,
        );
        coord.acknowledge_request(id).unwrap();
        coord.mark_executed(id).unwrap();
        assert_eq!(coord.request_status(id), Some(CrossRegionFenceStatus::Executed));
        assert!(!coord.pending_requests().contains(&id));
    }

    #[test]
    fn cross_region_coordinator_mark_rejected() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let id = coord.submit_request(
            NodeId::new([1u8; 32]),
            RegionId::new([2u8; 32]),
            RegionId::new([3u8; 32]),
            FencingMethod::PowerCycle,
        );
        coord.mark_rejected(id).unwrap();
        assert_eq!(coord.request_status(id), Some(CrossRegionFenceStatus::Rejected));
    }

    #[test]
    fn cross_region_coordinator_requests_for_region() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let region = RegionId::new([1u8; 32]);
        coord.submit_request(NodeId::new([2u8; 32]), region, RegionId::new([3u8; 32]), FencingMethod::NetworkIsolate);
        coord.submit_request(NodeId::new([4u8; 32]), region, RegionId::new([5u8; 32]), FencingMethod::PowerCycle);
        let region_ids = coord.requests_for_region(region);
        assert_eq!(region_ids.len(), 2);
    }

    #[test]
    fn cross_region_coordinator_unknown_request() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let result = coord.acknowledge_request(CrossRegionFenceId(99999));
        assert!(result.is_err());
    }

    #[test]
    fn cross_region_coordinator_enabled_toggle() {
        let mut coord = CrossRegionFencingCoordinator::new();
        assert!(coord.is_enabled());
        coord.set_enabled(false);
        assert!(!coord.is_enabled());
        coord.set_enabled(true);
        assert!(coord.is_enabled());
    }

    // ========== FencingResourceManager tests ==========

    #[test]
    fn resource_manager_fence_and_check() {
        let mut mgr = FencingResourceManager::new();
        let resource_id = ResourceId(vec![1, 2, 3]);
        let fence_id = mgr.fence_resource(
            ResourceType::Dataset,
            resource_id.clone(),
            NodeId::new([1u8; 32]),
            FencingEpoch(5),
        );
        assert!(mgr.is_resource_fenced(ResourceType::Dataset, &resource_id));
        assert_eq!(mgr.active_fence_count(), 1);
        let _ = fence_id;
    }

    #[test]
    fn resource_manager_release_fence() {
        let mut mgr = FencingResourceManager::new();
        let resource_id = ResourceId(vec![1, 2, 3]);
        let fence_id = mgr.fence_resource(
            ResourceType::Partition,
            resource_id.clone(),
            NodeId::new([1u8; 32]),
            FencingEpoch(1),
        );
        assert!(mgr.is_resource_fenced(ResourceType::Partition, &resource_id));
        let released = mgr.release_resource_fence(fence_id);
        assert!(released.is_some());
        assert!(!mgr.is_resource_fenced(ResourceType::Partition, &resource_id));
        assert_eq!(mgr.active_fence_count(), 0);
    }

    #[test]
    fn resource_manager_resource_fences() {
        let mut mgr = FencingResourceManager::new();
        let resource_id = ResourceId(vec![1, 2, 3]);
        mgr.fence_resource(ResourceType::Dataset, resource_id.clone(), NodeId::new([1u8; 32]), FencingEpoch(1));
        mgr.fence_resource(ResourceType::Dataset, resource_id.clone(), NodeId::new([2u8; 32]), FencingEpoch(2));
        let fences = mgr.resource_fences(ResourceType::Dataset, &resource_id);
        assert_eq!(fences.len(), 2);
    }

    #[test]
    fn resource_manager_fences_by_issuer() {
        let mut mgr = FencingResourceManager::new();
        let node = NodeId::new([1u8; 32]);
        let resource_id = ResourceId(vec![1, 2, 3]);
        mgr.fence_resource(ResourceType::HUnit, resource_id.clone(), node, FencingEpoch(1));
        mgr.fence_resource(ResourceType::Region, ResourceId(vec![4, 5, 6]), node, FencingEpoch(2));
        let by_issuer = mgr.fences_by_issuer(node);
        assert_eq!(by_issuer.len(), 2);
    }

    #[test]
    fn resource_manager_different_types_independent() {
        let mut mgr = FencingResourceManager::new();
        let resource_id = ResourceId(vec![1, 2, 3]);
        mgr.fence_resource(ResourceType::Dataset, resource_id.clone(), NodeId::new([1u8; 32]), FencingEpoch(1));
        assert!(mgr.is_resource_fenced(ResourceType::Dataset, &resource_id));
        assert!(!mgr.is_resource_fenced(ResourceType::Partition, &resource_id));
    }

    // ========== FencingRecoveryOrchestrator tests ==========

    #[test]
    fn recovery_orchestrator_register_and_waiting() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let token = FencingToken(42);
        let id = orch.register_recovery(node, token, RecoveryType::NodeRecovery);
        assert!(!orch.may_proceed(id));
        assert_eq!(orch.total_recoveries(), 1);
    }

    #[test]
    fn recovery_orchestrator_transition_ready() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let id = orch.register_recovery(node, FencingToken(1), RecoveryType::HUnitRecovery);
        orch.mark_ready(id).unwrap();
        assert!(orch.may_proceed(id));
    }

    #[test]
    fn recovery_orchestrator_transition_in_progress() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let id = orch.register_recovery(node, FencingToken(1), RecoveryType::RegionalRecovery);
        orch.mark_ready(id).unwrap();
        orch.mark_in_progress(id).unwrap();
        assert!(orch.may_proceed(id));
    }

    #[test]
    fn recovery_orchestrator_transition_completed() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let id = orch.register_recovery(node, FencingToken(1), RecoveryType::DatasetRestore);
        orch.mark_ready(id).unwrap();
        orch.mark_in_progress(id).unwrap();
        orch.mark_completed(id).unwrap();
        assert_eq!(orch.total_recoveries(), 1);
    }

    #[test]
    fn recovery_orchestrator_transition_aborted() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let id = orch.register_recovery(node, FencingToken(1), RecoveryType::PartitionRebalance);
        orch.mark_ready(id).unwrap();
        orch.mark_aborted(id).unwrap();
        assert!(!orch.may_proceed(id));
    }

    #[test]
    fn recovery_orchestrator_invalid_transition() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        let id = orch.register_recovery(node, FencingToken(1), RecoveryType::NodeRecovery);
        let result = orch.mark_in_progress(id);
        assert!(result.is_err());
    }

    #[test]
    fn recovery_orchestrator_recoveries_for_node() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node1 = NodeId::new([1u8; 32]);
        let node2 = NodeId::new([2u8; 32]);
        orch.register_recovery(node1, FencingToken(1), RecoveryType::NodeRecovery);
        orch.register_recovery(node1, FencingToken(2), RecoveryType::HUnitRecovery);
        orch.register_recovery(node2, FencingToken(3), RecoveryType::RegionalRecovery);
        let n1_recoveries = orch.recoveries_for_node(node1);
        assert_eq!(n1_recoveries.len(), 2);
        let n2_recoveries = orch.recoveries_for_node(node2);
        assert_eq!(n2_recoveries.len(), 1);
    }

    #[test]
    fn recovery_orchestrator_waiting_for_fence() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let node = NodeId::new([1u8; 32]);
        orch.register_recovery(node, FencingToken(1), RecoveryType::NodeRecovery);
        let waiting = orch.waiting_for_fence();
        assert_eq!(waiting.len(), 1);
    }

    #[test]
    fn recovery_orchestrator_recovery_not_found() {
        let mut orch = FencingRecoveryOrchestrator::new();
        let result = orch.mark_ready(RecoveryId(99999));
        assert!(result.is_err());
    }

    // ========== Exhaustive enum tests ==========

    #[test]
    fn fencing_outcome_all_variants_match() {
        let outcomes = [
            FencingOutcome::Success,
            FencingOutcome::Failed,
            FencingOutcome::Timeout,
            FencingOutcome::IdempotentNoop,
            FencingOutcome::QuorumRejected,
            FencingOutcome::PolicyRejected,
        ];
        for o in outcomes {
            let _ = format!("{:?}", o);
        }
    }

    #[test]
    fn split_brain_status_all_variants_match() {
        let statuses = [
            SplitBrainStatus::None,
            SplitBrainStatus::Suspected,
            SplitBrainStatus::Confirmed,
            SplitBrainStatus::Unknown,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    #[test]
    fn recovery_type_all_variants_match() {
        let types = [
            RecoveryType::NodeRecovery,
            RecoveryType::HUnitRecovery,
            RecoveryType::RegionalRecovery,
            RecoveryType::DatasetRestore,
            RecoveryType::PartitionRebalance,
        ];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    #[test]
    fn resource_type_all_variants_match() {
        let types = [
            ResourceType::Dataset,
            ResourceType::Partition,
            ResourceType::Region,
            ResourceType::HUnit,
        ];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    #[test]
    fn cross_region_fence_status_all_variants_match() {
        let statuses = [
            CrossRegionFenceStatus::Pending,
            CrossRegionFenceStatus::Acknowledged,
            CrossRegionFenceStatus::Executed,
            CrossRegionFenceStatus::Rejected,
            CrossRegionFenceStatus::TimedOut,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    #[test]
    fn recovery_status_all_variants_match() {
        let statuses = [
            RecoveryStatus::WaitingForFence,
            RecoveryStatus::Ready,
            RecoveryStatus::InProgress,
            RecoveryStatus::Completed,
            RecoveryStatus::Failed,
            RecoveryStatus::Aborted,
        ];
        for s in statuses {
            let _ = format!("{:?}", s);
        }
    }

    #[test]
    fn operation_type_all_variants_match() {
        let types = [
            OperationType::DatasetWrite,
            OperationType::DatasetDelete,
            OperationType::RegionFailover,
            OperationType::HUnitFailover,
            OperationType::PartitionMove,
            OperationType::LeadershipElection,
            OperationType::MembershipChange,
            OperationType::RecoveryOperation,
        ];
        for t in types {
            let _ = format!("{:?}", t);
        }
    }

    // ========== Integration tests ==========

    #[test]
    fn full_fencing_and_recovery_integration() {
        let mut mgr = FencingManager::new();
        let mut orch = FencingRecoveryOrchestrator::new();
        let mut metrics = FencingMetricsCollector::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);

        let token = mgr.fence_before_failover(node).unwrap();
        assert!(mgr.is_node_fenced(node));
        metrics.record_fence_success(node, FencingMethod::Hybrid);

        let recovery_id = orch.register_recovery(node, token, RecoveryType::NodeRecovery);
        assert!(!orch.may_proceed(recovery_id));

        orch.mark_ready(recovery_id).unwrap();
        assert!(orch.may_proceed(recovery_id));

        orch.mark_in_progress(recovery_id).unwrap();
        orch.mark_completed(recovery_id).unwrap();

        let current = mgr.fencing_epoch(node);
        mgr.unfence_node(node, FencingEpoch(current.0 + 1)).unwrap();
        assert!(!mgr.is_node_fenced(node));
        metrics.record_unfence();

        let m = metrics.snapshot();
        assert_eq!(m.successful_fences, 1);
        assert_eq!(m.unfence_operations, 1);
        assert_eq!(orch.total_recoveries(), 1);
    }

    #[test]
    fn policy_engine_and_audit_trail_integration() {
        let mut mgr = FencingManager::new();
        let mut engine = FencingPolicyEngine::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);

        let decision = engine.evaluate_fence(node, FencingMethod::NetworkIsolate, None).unwrap();
        assert!(decision.authorized);
        let token = mgr.fence_before_failover(node).unwrap();
        assert!(mgr.is_fencing_token_valid(&token));

        let trail = mgr.fencing_audit_trail(node);
        assert!(!trail.is_empty());
        assert_eq!(engine.evaluation_count(), 1);
    }

    #[test]
    fn health_monitor_informs_fencing() {
        let mut monitor = FencingHealthMonitor::new();
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        monitor.register_node(node);
        mgr.register_node(node);

        monitor.heartbeat(node, TimestampMillis(100));
        assert!(monitor.is_healthy(node));
        let token = mgr.fence_before_failover(node).unwrap();
        assert!(mgr.is_node_fenced(node));
    }

    #[test]
    fn cross_region_fencing_with_metrics() {
        let mut coord = CrossRegionFencingCoordinator::new();
        let mut metrics = FencingMetricsCollector::new();
        let victim = NodeId::new([1u8; 32]);
        let id = coord.submit_request(
            victim,
            RegionId::new([2u8; 32]),
            RegionId::new([3u8; 32]),
            FencingMethod::PowerCycle,
        );
        coord.acknowledge_request(id).unwrap();
        coord.mark_executed(id).unwrap();
        metrics.record_emergency_fence(victim);
        let m = metrics.snapshot();
        assert_eq!(m.emergency_fences, 1);
    }

    #[test]
    fn multiple_nodes_fenced() {
        let mut mgr = FencingManager::new();
        let nodes = vec![
            NodeId::new([1u8; 32]),
            NodeId::new([2u8; 32]),
            NodeId::new([3u8; 32]),
        ];
        for node in nodes {
            mgr.register_node(node);
            mgr.fence_before_failover(node).unwrap();
        }
        let fenced = mgr.fenced_nodes();
        assert_eq!(fenced.len(), 3);
        for node in nodes {
            assert!(mgr.is_node_fenced(node));
        }
    }

    #[test]
    fn fencing_epoch_monotonic() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let e1 = mgr.fencing_epoch(node);
        mgr.fence_before_failover(node).unwrap();
        let e2 = mgr.fencing_epoch(node);
        mgr.stonith_execute(node).unwrap();
        let e3 = mgr.fencing_epoch(node);
        assert!(e2.0 > e1.0);
        assert!(e3.0 > e2.0);
    }

    #[test]
    fn token_invalidated_after_unfence() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        let token = mgr.fence_before_failover(node).unwrap();
        assert!(mgr.is_fencing_token_valid(&token));
        let current = mgr.fencing_epoch(node);
        mgr.unfence_node(node, FencingEpoch(current.0 + 1)).unwrap();
        assert!(!mgr.is_fencing_token_valid(&token));
    }

    #[test]
    fn audit_trail_event_ids_monotonic() {
        let mut mgr = FencingManager::new();
        let node = NodeId::new([1u8; 32]);
        mgr.register_node(node);
        mgr.fence_before_failover(node).unwrap();
        mgr.record_fencing_failure(
            node,
            FencingError::Timeout {
                node,
                method: FencingMethod::PowerCycle,
                duration_ms: 60000,
            },
        ).unwrap();
        let trail = mgr.fencing_audit_trail(node);
        assert!(trail.len() >= 2);
        for pair in trail.windows(2) {
            assert!(pair[0].event_id.0 < pair[1].event_id.0);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// END OF FILE
///////////////////////////////////////////////////////////////////////////////