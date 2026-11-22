//! # ids-000-aa Sealed Trait Contract: Coordinator
//!
//! Distributed coordination contract for IDS.
//! Provides leader election, distributed locks, barriers, group membership,
//! heartbeats, quorum decisions, session management, and fencing.
//! All types are canonical and sealed — only ids-000-aa and its designated
//! implementation crates can implement this trait.
//!
//! Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, DNS, or any existing networking stack. Coordination messages are
//! opaque byte blobs carried on CarrierType + Vec<u8> Endpoints only.

use crate::errors::CoordinatorError;
use crate::types::{
    CheckpointId, DatasetId, Epoch, FragmentId, HUnitId, IdsTime, KeyId, LeaseId, MessageId,
    NodeId, OrderingDomain, RegionId, SagaId, SessionId, StepId, TransactionId, Version,
};
use std::collections::{HashMap, HashSet};

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Coordinator
///////////////////////////////////////////////////////////////////////////////

/// Sealed marker — only ids-000-aa and designated crates can implement.
pub mod private {
    pub trait Sealed {}
}

/// Sealed distributed coordination trait.
/// Implementations live downstream in ids-coordinator and ids-* crates.
pub trait Coordinator: private::Sealed {
    /// Elect a leader for a coordination domain.
    fn elect_leader(
        &self,
        domain: OrderingDomain,
        candidates: &[NodeId],
    ) -> Result<CoordinatorResult, CoordinatorError>;

    /// Get the current leader for a domain.
    fn leader(&self, domain: OrderingDomain) -> Option<NodeId>;

    /// Get the current epoch for a domain.
    fn domain_epoch(&self, domain: OrderingDomain) -> Epoch;

    /// Get the fencing epoch for a domain.
    fn fencing_epoch(&self, domain: OrderingDomain) -> Epoch;

    /// Challenge the current leader (trigger re-election).
    fn challenge_leader(
        &self,
        domain: OrderingDomain,
        challenger: NodeId,
    ) -> Result<CoordinatorResult, CoordinatorError>;

    /// Resign as leader of a domain.
    fn resign_leader(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<(), CoordinatorError>;

    /// Acquire a distributed lock.
    fn acquire_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
        ttl_ms: u64,
    ) -> Result<LockAcquisition, CoordinatorError>;

    /// Release a distributed lock.
    fn release_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
    ) -> Result<(), CoordinatorError>;

    /// Check if a lock is held.
    fn is_locked(&self, lock_id: KeyId) -> bool;

    /// Get the holder of a lock.
    fn lock_holder(&self, lock_id: KeyId) -> Option<NodeId>;

    /// Get the remaining TTL for a lock.
    fn lock_ttl_remaining(&self, lock_id: KeyId) -> Option<u64>;

    /// Extend a lock's TTL.
    fn extend_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
        additional_ms: u64,
    ) -> Result<u64, CoordinatorError>;

    /// Create a barrier for synchronized execution.
    fn create_barrier(
        &self,
        barrier_id: KeyId,
        participant_count: u32,
    ) -> Result<BarrierHandle, CoordinatorError>;

    /// Join a barrier (wait for all participants).
    fn join_barrier(
        &self,
        barrier_id: KeyId,
        node: NodeId,
    ) -> Result<BarrierResult, CoordinatorError>;

    /// Check if a barrier is satisfied.
    fn is_barrier_satisfied(&self, barrier_id: KeyId) -> bool;

    /// Get the number of participants at a barrier.
    fn barrier_participants(&self, barrier_id: KeyId) -> u32;

    /// Register a node in a coordination group.
    fn join_group(
        &self,
        group_id: KeyId,
        node: NodeId,
    ) -> Result<GroupMembership, CoordinatorError>;

    /// Leave a coordination group.
    fn leave_group(
        &self,
        group_id: KeyId,
        node: NodeId,
    ) -> Result<(), CoordinatorError>;

    /// Get group members.
    fn group_members(&self, group_id: KeyId) -> Vec<NodeId>;

    /// Check if a node is a group member.
    fn is_group_member(&self, group_id: KeyId, node: NodeId) -> bool;

    /// Record a heartbeat from a node.
    fn record_heartbeat(
        &self,
        node: NodeId,
        domain: OrderingDomain,
    ) -> Result<HeartbeatRecord, CoordinatorError>;

    /// Get the last heartbeat time for a node.
    fn last_heartbeat(&self, node: NodeId) -> Option<IdsTime>;

    /// Check if a node is alive (recent heartbeat).
    fn is_node_alive(&self, node: NodeId, timeout_ms: u64) -> bool;

    /// Get all alive nodes.
    fn alive_nodes(&self, timeout_ms: u64) -> Vec<NodeId>;

    /// Create a coordination session.
    fn create_session(
        &self,
        node: NodeId,
        domain: OrderingDomain,
        ttl_ms: u64,
    ) -> Result<SessionHandle, CoordinatorError>;

    /// Renew a session's TTL.
    fn renew_session(
        &self,
        session_id: SessionId,
        node: NodeId,
        additional_ms: u64,
    ) -> Result<SessionHandle, CoordinatorError>;

    /// Close a session.
    fn close_session(
        &self,
        session_id: SessionId,
        node: NodeId,
    ) -> Result<(), CoordinatorError>;

    /// Get session info.
    fn session_info(&self, session_id: SessionId) -> Option<SessionInfo>;

    /// Get all sessions for a node.
    fn node_sessions(&self, node: NodeId) -> Vec<SessionId>;

    /// Make a quorum decision.
    fn quorum_decision(
        &self,
        domain: OrderingDomain,
        proposal_id: MessageId,
        votes: &[(NodeId, bool)],
    ) -> Result<QuorumDecision, CoordinatorError>;

    /// Get quorum status for a domain.
    fn quorum_status(&self, domain: OrderingDomain) -> QuorumStatus;

    /// Issue a fencing token for a node.
    fn issue_fencing_token(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<FencingToken, CoordinatorError>;

    /// Validate a fencing token.
    fn validate_fencing_token(
        &self,
        token: &FencingToken,
    ) -> Result<bool, CoordinatorError>;

    /// Get the current fencing epoch for a node.
    fn node_fencing_epoch(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Epoch;

    /// Coordinator statistics.
    fn stats(&self, domain: OrderingDomain) -> CoordinatorStats;

    /// All known domains.
    fn domains(&self) -> Vec<OrderingDomain>;

    /// Validate coordinator configuration.
    fn validate_config(&self, config: &CoordinatorConfig) -> Result<(), CoordinatorError>;
}

///////////////////////////////////////////////////////////////////////////////
// CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Coordinator configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Leader election timeout in milliseconds.
    pub election_timeout_ms: u64,
    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval_ms: u64,
    /// Heartbeat timeout (node considered dead after this).
    pub heartbeat_timeout_ms: u64,
    /// Lock TTL in milliseconds.
    pub lock_ttl_ms: u64,
    /// Maximum lock extensions.
    pub max_lock_extensions: u32,
    /// Barrier wait timeout in milliseconds.
    pub barrier_timeout_ms: u64,
    /// Session TTL in milliseconds.
    pub session_ttl_ms: u64,
    /// Quorum size for decisions.
    pub quorum_size: u32,
    /// Maximum concurrent sessions.
    pub max_sessions: u32,
    /// Enable fencing.
    pub enable_fencing: bool,
    /// Enable automatic leader re-election.
    pub auto_reelect: bool,
}

impl CoordinatorConfig {
    pub fn new(election_timeout_ms: u64) -> Result<Self, CoordinatorError> {
        let config = Self {
            election_timeout_ms,
            heartbeat_interval_ms: 5000,
            heartbeat_timeout_ms: 30000,
            lock_ttl_ms: 60000,
            max_lock_extensions: 3,
            barrier_timeout_ms: 120000,
            session_ttl_ms: 300000,
            quorum_size: 3,
            max_sessions: 1000,
            enable_fencing: true,
            auto_reelect: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), CoordinatorError> {
        if self.election_timeout_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "election_timeout_ms".to_string(),
                reason: "election_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.heartbeat_interval_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "heartbeat_interval_ms".to_string(),
                reason: "heartbeat_interval_ms must be greater than 0".to_string(),
            });
        }
        if self.heartbeat_timeout_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "heartbeat_timeout_ms".to_string(),
                reason: "heartbeat_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.lock_ttl_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "lock_ttl_ms".to_string(),
                reason: "lock_ttl_ms must be greater than 0".to_string(),
            });
        }
        if self.barrier_timeout_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "barrier_timeout_ms".to_string(),
                reason: "barrier_timeout_ms must be greater than 0".to_string(),
            });
        }
        if self.session_ttl_ms == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "session_ttl_ms".to_string(),
                reason: "session_ttl_ms must be greater than 0".to_string(),
            });
        }
        if self.quorum_size == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "quorum_size".to_string(),
                reason: "quorum_size must be greater than 0".to_string(),
            });
        }
        if self.max_sessions == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "max_sessions".to_string(),
                reason: "max_sessions must be greater than 0".to_string(),
            });
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            election_timeout_ms: 10000,
            heartbeat_interval_ms: 5000,
            heartbeat_timeout_ms: 30000,
            lock_ttl_ms: 60000,
            max_lock_extensions: 3,
            barrier_timeout_ms: 120000,
            session_ttl_ms: 300000,
            quorum_size: 3,
            max_sessions: 1000,
            enable_fencing: true,
            auto_reelect: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            election_timeout_ms: 5000,
            heartbeat_interval_ms: 1000,
            heartbeat_timeout_ms: 5000,
            lock_ttl_ms: 10000,
            max_lock_extensions: 1,
            barrier_timeout_ms: 30000,
            session_ttl_ms: 60000,
            quorum_size: 5,
            max_sessions: 100,
            enable_fencing: true,
            auto_reelect: true,
        }
    }

    pub fn lenient() -> Self {
        Self {
            election_timeout_ms: 60000,
            heartbeat_interval_ms: 30000,
            heartbeat_timeout_ms: 300000,
            lock_ttl_ms: 600000,
            max_lock_extensions: 10,
            barrier_timeout_ms: 600000,
            session_ttl_ms: 3600000,
            quorum_size: 1,
            max_sessions: 10000,
            enable_fencing: false,
            auto_reelect: false,
        }
    }

    pub fn heartbeat_ratio(&self) -> f64 {
        self.heartbeat_timeout_ms as f64 / self.heartbeat_interval_ms as f64
    }
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self::default()
    }
}

///////////////////////////////////////////////////////////////////////////////
// COORDINATOR RESULT
///////////////////////////////////////////////////////////////////////////////

/// Result of a leader election.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorResult {
    pub domain: OrderingDomain,
    pub leader: NodeId,
    pub epoch: Epoch,
    pub fencing_epoch: Epoch,
    pub elected_at: IdsTime,
}

impl CoordinatorResult {
    pub fn new(
        domain: OrderingDomain,
        leader: NodeId,
        epoch: Epoch,
        fencing_epoch: Epoch,
        elected_at: IdsTime,
    ) -> Self {
        Self { domain, leader, epoch, fencing_epoch, elected_at }
    }

    pub fn domain(&self) -> OrderingDomain {
        self.domain
    }

    pub fn is_recent(&self, now_ms: u64, threshold_ms: u64) -> bool {
        self.elected_at.is_recent(now_ms, threshold_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOCK ACQUISITION
///////////////////////////////////////////////////////////////////////////////

/// Result of acquiring a distributed lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockAcquisition {
    pub lock_id: KeyId,
    pub holder: NodeId,
    pub acquired_at: IdsTime,
    pub ttl_ms: u64,
    pub expires_at: IdsTime,
    pub fencing_token: Epoch,
}

impl LockAcquisition {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.expires_at.0
    }

    pub fn ttl_remaining(&self, now_ms: u64) -> u64 {
        self.expires_at.0.saturating_sub(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BARRIER
///////////////////////////////////////////////////////////////////////////////

/// Handle for a synchronization barrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarrierHandle {
    pub barrier_id: KeyId,
    pub required_participants: u32,
    pub created_at: IdsTime,
    pub timeout_ms: u64,
}

impl BarrierHandle {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.created_at.0) > self.timeout_ms
    }
}

/// Result of joining a barrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarrierResult {
    pub barrier_id: KeyId,
    pub node: NodeId,
    pub satisfied: bool,
    pub participants: u32,
    pub joined_at: IdsTime,
}

impl BarrierResult {
    pub fn is_satisfied(&self) -> bool {
        self.satisfied
    }
}

///////////////////////////////////////////////////////////////////////////////
// GROUP MEMBERSHIP
///////////////////////////////////////////////////////////////////////////////

/// Result of joining a coordination group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembership {
    pub group_id: KeyId,
    pub node: NodeId,
    pub joined_at: IdsTime,
    pub member_count: u32,
    pub is_leader: bool,
}

impl GroupMembership {
    pub fn is_member(&self) -> bool {
        true
    }
}

///////////////////////////////////////////////////////////////////////////////
// HEARTBEAT
///////////////////////////////////////////////////////////////////////////////

/// Record of a node heartbeat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatRecord {
    pub node: NodeId,
    pub domain: OrderingDomain,
    pub received_at: IdsTime,
    pub sequence: u64,
}

impl HeartbeatRecord {
    pub fn is_stale(&self, now_ms: u64, timeout_ms: u64) -> bool {
        now_ms.saturating_sub(self.received_at.0) > timeout_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION
///////////////////////////////////////////////////////////////////////////////

/// Handle for a coordination session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionHandle {
    pub session_id: SessionId,
    pub node: NodeId,
    pub domain: OrderingDomain,
    pub created_at: IdsTime,
    pub ttl_ms: u64,
    pub expires_at: IdsTime,
}

impl SessionHandle {
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.expires_at.0
    }

    pub fn ttl_remaining(&self, now_ms: u64) -> u64 {
        self.expires_at.0.saturating_sub(now_ms)
    }
}

/// Information about a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub node: NodeId,
    pub domain: OrderingDomain,
    pub state: SessionState,
    pub created_at: IdsTime,
    pub last_renewal: Option<IdsTime>,
}

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionState {
    Active,
    Expired,
    Closed,
}

impl SessionState {
    pub fn is_active(&self) -> bool {
        matches!(self, SessionState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Expired | SessionState::Closed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM
///////////////////////////////////////////////////////////////////////////////

/// Result of a quorum decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumDecision {
    pub proposal_id: MessageId,
    pub domain: OrderingDomain,
    pub approved: bool,
    pub votes_for: u32,
    pub votes_against: u32,
    pub total_voters: u32,
    pub quorum_reached: bool,
    pub decided_at: IdsTime,
}

impl QuorumDecision {
    pub fn is_approved(&self) -> bool {
        self.approved
    }

    pub fn approval_rate(&self) -> f64 {
        if self.total_voters == 0 {
            return 0.0;
        }
        self.votes_for as f64 / self.total_voters as f64
    }

    pub fn quorum_met(&self) -> bool {
        self.quorum_reached
    }
}

/// Quorum status for a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumStatus {
    pub domain: OrderingDomain,
    pub total_nodes: u32,
    pub alive_nodes: u32,
    pub quorum_size: u32,
    pub quorum_available: bool,
}

impl QuorumStatus {
    pub fn quorum_achievable(&self) -> bool {
        self.alive_nodes >= self.quorum_size
    }
}

///////////////////////////////////////////////////////////////////////////////
// FENCING TOKEN
///////////////////////////////////////////////////////////////////////////////

/// A fencing token for isolating stale leaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FencingToken {
    pub domain: OrderingDomain,
    pub node: NodeId,
    pub epoch: Epoch,
    pub issued_at: IdsTime,
}

impl FencingToken {
    pub fn is_valid(&self, current_epoch: Epoch) -> bool {
        self.epoch >= current_epoch
    }

    pub fn is_stale(&self, current_epoch: Epoch) -> bool {
        self.epoch < current_epoch
    }
}

///////////////////////////////////////////////////////////////////////////////
// COORDINATOR STATS
///////////////////////////////////////////////////////////////////////////////

/// Statistics for a coordination domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorStats {
    pub domain: OrderingDomain,
    pub elections_held: u64,
    pub leader_changes: u64,
    pub locks_acquired: u64,
    pub locks_released: u64,
    pub barriers_created: u64,
    pub barriers_satisfied: u64,
    pub group_joins: u64,
    pub group_leaves: u64,
    pub heartbeats_received: u64,
    pub sessions_created: u64,
    pub sessions_closed: u64,
    pub quorum_decisions: u64,
    pub fencing_tokens_issued: u64,
}

impl CoordinatorStats {
    pub fn new(domain: OrderingDomain) -> Self {
        Self {
            domain,
            elections_held: 0,
            leader_changes: 0,
            locks_acquired: 0,
            locks_released: 0,
            barriers_created: 0,
            barriers_satisfied: 0,
            group_joins: 0,
            group_leaves: 0,
            heartbeats_received: 0,
            sessions_created: 0,
            sessions_closed: 0,
            quorum_decisions: 0,
            fencing_tokens_issued: 0,
        }
    }

    pub fn election_rate(&self) -> f64 {
        let total = self.elections_held + self.leader_changes;
        if total == 0 {
            return 0.0;
        }
        self.elections_held as f64 / total as f64
    }

    pub fn lock_success_rate(&self) -> f64 {
        let total = self.locks_acquired + self.locks_released;
        if total == 0 {
            return 1.0;
        }
        self.locks_acquired as f64 / total as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// COORDINATOR ERROR
///////////////////////////////////////////////////////////////////////////////

/// Distributed coordination errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum CoordinatorError {
    #[error("invalid config: {field} - {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("domain not found: {domain:?}")]
    DomainNotFound { domain: OrderingDomain },

    #[error("no candidates for election in domain: {domain:?}")]
    NoCandidates { domain: OrderingDomain },

    #[error("node is not a candidate: {node_id:?}")]
    NotACandidate { node_id: NodeId },

    #[error("lock not found: {lock_id:?}")]
    LockNotFound { lock_id: KeyId },

    #[error("lock already held by {holder:?}")]
    LockAlreadyHeld { holder: NodeId },

    #[error("lock not held by {node_id:?}")]
    LockNotHeld { node_id: NodeId },

    #[error("lock expired: {lock_id:?}")]
    LockExpired { lock_id: KeyId },

    #[error("barrier not found: {barrier_id:?}")]
    BarrierNotFound { barrier_id: KeyId },

    #[error("barrier already satisfied: {barrier_id:?}")]
    BarrierAlreadySatisfied { barrier_id: KeyId },

    #[error("barrier not satisfied: {barrier_id:?}")]
    BarrierNotSatisfied { barrier_id: KeyId },

    #[error("group not found: {group_id:?}")]
    GroupNotFound { group_id: KeyId },

    #[error("node already in group: {node_id:?}")]
    NodeAlreadyInGroup { node_id: NodeId },

    #[error("node not in group: {node_id:?}")]
    NodeNotInGroup { node_id: NodeId },

    #[error("session not found: {session_id:?}")]
    SessionNotFound { session_id: SessionId },

    #[error("session expired: {session_id:?}")]
    SessionExpired { session_id: SessionId },

    #[error("session not owned by {node_id:?}")]
    SessionNotOwned { node_id: NodeId },

    #[error("quorum not reached: {votes_for}/{total_voters}")]
    QuorumNotReached { votes_for: u32, total_voters: u32 },

    #[error("fencing token invalid: {reason}")]
    FencingTokenInvalid { reason: String },

    #[error("no leader for domain: {domain:?}")]
    NoLeader { domain: OrderingDomain },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl CoordinatorError {
    pub fn category(&self) -> &'static str {
        match self {
            CoordinatorError::InvalidConfig { .. } => "invalid_config",
            CoordinatorError::DomainNotFound { .. } => "domain_not_found",
            CoordinatorError::NoCandidates { .. } => "no_candidates",
            CoordinatorError::NotACandidate { .. } => "not_a_candidate",
            CoordinatorError::LockNotFound { .. } => "lock_not_found",
            CoordinatorError::LockAlreadyHeld { .. } => "lock_already_held",
            CoordinatorError::LockNotHeld { .. } => "lock_not_held",
            CoordinatorError::LockExpired { .. } => "lock_expired",
            CoordinatorError::BarrierNotFound { .. } => "barrier_not_found",
            CoordinatorError::BarrierAlreadySatisfied { .. } => "barrier_already_satisfied",
            CoordinatorError::BarrierNotSatisfied { .. } => "barrier_not_satisfied",
            CoordinatorError::GroupNotFound { .. } => "group_not_found",
            CoordinatorError::NodeAlreadyInGroup { .. } => "node_already_in_group",
            CoordinatorError::NodeNotInGroup { .. } => "node_not_in_group",
            CoordinatorError::SessionNotFound { .. } => "session_not_found",
            CoordinatorError::SessionExpired { .. } => "session_expired",
            CoordinatorError::SessionNotOwned { .. } => "session_not_owned",
            CoordinatorError::QuorumNotReached { .. } => "quorum_not_reached",
            CoordinatorError::FencingTokenInvalid { .. } => "fencing_token_invalid",
            CoordinatorError::NoLeader { .. } => "no_leader",
            CoordinatorError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CoordinatorError::LockExpired { .. }
                | CoordinatorError::SessionExpired { .. }
                | CoordinatorError::QuorumNotReached { .. }
                | CoordinatorError::NoLeader { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            CoordinatorError::InvalidConfig { .. } => "correct the coordinator configuration",
            CoordinatorError::DomainNotFound { .. } => "create the domain first",
            CoordinatorError::NoCandidates { .. } => "add candidates to the domain",
            CoordinatorError::NotACandidate { .. } => "register as a candidate",
            CoordinatorError::LockNotFound { .. } => "create the lock first",
            CoordinatorError::LockAlreadyHeld { .. } => "wait for lock release or use different lock",
            CoordinatorError::LockNotHeld { .. } => "acquire the lock first",
            CoordinatorError::LockExpired { .. } => "re-acquire the lock",
            CoordinatorError::BarrierNotFound { .. } => "create the barrier first",
            CoordinatorError::BarrierAlreadySatisfied { .. } => "create a new barrier",
            CoordinatorError::BarrierNotSatisfied { .. } => "wait for all participants",
            CoordinatorError::GroupNotFound { .. } => "create the group first",
            CoordinatorError::NodeAlreadyInGroup { .. } => "node is already a member",
            CoordinatorError::NodeNotInGroup { .. } => "join the group first",
            CoordinatorError::SessionNotFound { .. } => "create the session first",
            CoordinatorError::SessionExpired { .. } => "renew or create a new session",
            CoordinatorError::SessionNotOwned { .. } => "use your own session",
            CoordinatorError::QuorumNotReached { .. } => "wait for more votes or reduce quorum size",
            CoordinatorError::FencingTokenInvalid { .. } => "obtain a new fencing token",
            CoordinatorError::NoLeader { .. } => "wait for leader election",
            CoordinatorError::Internal { .. } => "contact system administrator",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// ELECTION STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a leader election for a domain.
#[derive(Debug, Clone)]
struct ElectionState {
    domain: OrderingDomain,
    leader: Option<NodeId>,
    epoch: Epoch,
    fencing_epoch: Epoch,
    candidates: HashSet<NodeId>,
    votes: HashMap<NodeId, u64>,
    last_election: Option<IdsTime>,
}

impl ElectionState {
    pub fn new(domain: OrderingDomain) -> Self {
        Self {
            domain,
            leader: None,
            epoch: Epoch::new(0),
            fencing_epoch: Epoch::new(0),
            candidates: HashSet::new(),
            votes: HashMap::new(),
            last_election: None,
        }
    }

    pub fn has_leader(&self) -> bool {
        self.leader.is_some()
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_candidate(&self, node: NodeId) -> bool {
        self.candidates.contains(&node)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOCK STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a distributed lock.
#[derive(Debug, Clone)]
struct LockState {
    lock_id: KeyId,
    holder: Option<NodeId>,
    acquired_at: Option<IdsTime>,
    ttl_ms: u64,
    extension_count: u32,
    fencing_epoch: Epoch,
}

impl LockState {
    pub fn new(lock_id: KeyId, ttl_ms: u64) -> Self {
        Self {
            lock_id,
            holder: None,
            acquired_at: None,
            ttl_ms,
            extension_count: 0,
            fencing_epoch: Epoch::new(0),
        }
    }

    pub fn is_held(&self) -> bool {
        self.holder.is_some()
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        if let Some(acquired) = self.acquired_at {
            now_ms.saturating_sub(acquired.0) > self.ttl_ms
        } else {
            false
        }
    }

    pub fn can_extend(&self, max_extensions: u32) -> bool {
        self.extension_count < max_extensions
    }
}

///////////////////////////////////////////////////////////////////////////////
// BARRIER STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a synchronization barrier.
#[derive(Debug, Clone)]
struct BarrierState {
    barrier_id: KeyId,
    required_participants: u32,
    participants: HashSet<NodeId>,
    satisfied: bool,
    created_at: IdsTime,
    timeout_ms: u64,
}

impl BarrierState {
    pub fn new(barrier_id: KeyId, required: u32, timeout_ms: u64) -> Self {
        Self {
            barrier_id,
            required_participants: required,
            participants: HashSet::new(),
            satisfied: false,
            created_at: IdsTime::now(),
            timeout_ms,
        }
    }

    pub fn add_participant(&mut self, node: NodeId) -> bool {
        if self.satisfied {
            return false;
        }
        let was_empty = self.participants.is_empty();
        self.participants.insert(node);
        let now_count = self.participants.len() as u32;
        if now_count >= self.required_participants {
            self.satisfied = true;
        }
        was_empty
    }

    pub fn participant_count(&self) -> u32 {
        self.participants.len() as u32
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.created_at.0) > self.timeout_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// GROUP STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a coordination group.
#[derive(Debug, Clone)]
struct GroupState {
    group_id: KeyId,
    members: HashSet<NodeId>,
    leader: Option<NodeId>,
    created_at: IdsTime,
}

impl GroupState {
    pub fn new(group_id: KeyId) -> Self {
        Self {
            group_id,
            members: HashSet::new(),
            leader: None,
            created_at: IdsTime::now(),
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn add_member(&mut self, node: NodeId) -> bool {
        self.members.insert(node)
    }

    pub fn remove_member(&mut self, node: NodeId) -> bool {
        let was_member = self.members.remove(&node);
        if was_member && self.leader == Some(node) {
            self.leader = None;
        }
        was_member
    }

    pub fn set_leader(&mut self, node: NodeId) {
        self.leader = Some(node);
    }

    pub fn is_member(&self, node: NodeId) -> bool {
        self.members.contains(&node)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a coordination session.
#[derive(Debug, Clone)]
struct SessionState {
    session_id: SessionId,
    node: NodeId,
    domain: OrderingDomain,
    state: SessionState,
    created_at: IdsTime,
    last_renewal: Option<IdsTime>,
    ttl_ms: u64,
}

impl SessionState {
    pub fn new(
        session_id: SessionId,
        node: NodeId,
        domain: OrderingDomain,
        ttl_ms: u64,
    ) -> Self {
        Self {
            session_id,
            node,
            domain,
            state: SessionState::Active,
            created_at: IdsTime::now(),
            last_renewal: None,
            ttl_ms,
        }
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        let reference = self.last_renewal.map(|t| t.0).unwrap_or(self.created_at.0);
        now_ms.saturating_sub(reference) > self.ttl_ms
    }

    pub fn renew(&mut self, additional_ms: u64) {
        self.ttl_ms += additional_ms;
        self.last_renewal = Some(IdsTime::now());
    }

    pub fn close(&mut self) {
        self.state = SessionState::Closed;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COORDINATOR ENGINE (REFERENCE IMPLEMENTATION)
///////////////////////////////////////////////////////////////////////////////

/// Reference coordinator engine implementing the Coordinator trait.
pub struct CoordinatorEngine {
    config: std::cell::RefCell<CoordinatorConfig>,
    elections: std::cell::RefCell<HashMap<OrderingDomain, ElectionState>>,
    locks: std::cell::RefCell<HashMap<KeyId, LockState>>,
    barriers: std::cell::RefCell<HashMap<KeyId, BarrierState>>,
    groups: std::cell::RefCell<HashMap<KeyId, GroupState>>,
    heartbeats: std::cell::RefCell<HashMap<NodeId, HeartbeatRecord>>,
    sessions: std::cell::RefCell<HashMap<SessionId, SessionState>>,
    quorum_decisions: std::cell::RefCell<HashMap<MessageId, QuorumDecision>>,
    fencing_tokens: std::cell::RefCell<HashMap<(OrderingDomain, NodeId), Epoch>>,
    stats: std::cell::RefCell<HashMap<OrderingDomain, CoordinatorStats>>,
    session_counter: std::cell::RefCell<u64>,
}

impl CoordinatorEngine {
    pub fn new() -> Self {
        Self {
            config: std::cell::RefCell::new(CoordinatorConfig::default()),
            elections: std::cell::RefCell::new(HashMap::new()),
            locks: std::cell::RefCell::new(HashMap::new()),
            barriers: std::cell::RefCell::new(HashMap::new()),
            groups: std::cell::RefCell::new(HashMap::new()),
            heartbeats: std::cell::RefCell::new(HashMap::new()),
            sessions: std::cell::RefCell::new(HashMap::new()),
            quorum_decisions: std::cell::RefCell::new(HashMap::new()),
            fencing_tokens: std::cell::RefCell::new(HashMap::new()),
            stats: std::cell::RefCell::new(HashMap::new()),
            session_counter: std::cell::RefCell::new(0),
        }
    }

    pub fn with_config(config: CoordinatorConfig) -> Result<Self, CoordinatorError> {
        config.validate()?;
        Ok(Self {
            config: std::cell::RefCell::new(config),
            elections: std::cell::RefCell::new(HashMap::new()),
            locks: std::cell::RefCell::new(HashMap::new()),
            barriers: std::cell::RefCell::new(HashMap::new()),
            groups: std::cell::RefCell::new(HashMap::new()),
            heartbeats: std::cell::RefCell::new(HashMap::new()),
            sessions: std::cell::RefCell::new(HashMap::new()),
            quorum_decisions: std::cell::RefCell::new(HashMap::new()),
            fencing_tokens: std::cell::RefCell::new(HashMap::new()),
            stats: std::cell::RefCell::new(HashMap::new()),
            session_counter: std::cell::RefCell::new(0),
        })
    }

    fn ensure_domain(&self, domain: OrderingDomain) {
        let mut elections = self.elections.borrow_mut();
        if !elections.contains_key(&domain) {
            let config = self.config.borrow().clone();
            elections.insert(
                domain,
                ElectionState::new(domain),
            );
            let mut stats = self.stats.borrow_mut();
            stats.insert(domain, CoordinatorStats::new(domain));
            drop(elections);
            drop(stats);
            let _ = config;
        }
    }

    fn stats_mut(&self, domain: OrderingDomain) -> CoordinatorStats {
        let mut stats = self.stats.borrow_mut();
        if let Some(s) = stats.get_mut(&domain) {
            s.clone()
        } else {
            CoordinatorStats::new(domain)
        }
    }

    fn update_stats<F>(&self, domain: OrderingDomain, f: F)
    where
        F: FnOnce(&mut CoordinatorStats),
    {
        let mut stats = self.stats.borrow_mut();
        let entry = stats.entry(domain).or_insert_with(|| CoordinatorStats::new(domain));
        f(entry);
    }
}

///////////////////////////////////////////////////////////////////////////////
// IMPL COORDINATOR FOR COORDINATORENGINE
///////////////////////////////////////////////////////////////////////////////

impl private::Sealed for CoordinatorEngine {}

impl Coordinator for CoordinatorEngine {
    fn elect_leader(
        &self,
        domain: OrderingDomain,
        candidates: &[NodeId],
    ) -> Result<CoordinatorResult, CoordinatorError> {
        self.ensure_domain(domain);

        if candidates.is_empty() {
            return Err(CoordinatorError::NoCandidates { domain });
        }

        let mut elections = self.elections.borrow_mut();
        let election = elections
            .get_mut(&domain)
            .ok_or_else(|| CoordinatorError::DomainNotFound { domain })?;

        // Simple election: first candidate becomes leader
        let leader = candidates[0];
        let epoch = election.epoch.next();
        let fencing_epoch = Epoch::new(epoch.0 + 1);
        let elected_at = IdsTime::now();

        election.leader = Some(leader);
        election.epoch = epoch;
        election.fencing_epoch = fencing_epoch;
        election.last_election = Some(elected_at);
        election.candidates = candidates.iter().cloned().collect();
        election.votes.clear();

        self.update_stats(domain, |s| {
            s.elections_held += 1;
            s.leader_changes += 1;
        });

        Ok(CoordinatorResult::new(
            domain,
            leader,
            epoch,
            fencing_epoch,
            elected_at,
        ))
    }

    fn leader(&self, domain: OrderingDomain) -> Option<NodeId> {
        let elections = self.elections.borrow();
        elections.get(&domain).and_then(|e| e.leader)
    }

    fn domain_epoch(&self, domain: OrderingDomain) -> Epoch {
        let elections = self.elections.borrow();
        elections.get(&domain).map(|e| e.epoch).unwrap_or(Epoch::new(0))
    }

    fn fencing_epoch(&self, domain: OrderingDomain) -> Epoch {
        let elections = self.elections.borrow();
        elections.get(&domain).map(|e| e.fencing_epoch).unwrap_or(Epoch::new(0))
    }

    fn challenge_leader(
        &self,
        domain: OrderingDomain,
        challenger: NodeId,
    ) -> Result<CoordinatorResult, CoordinatorError> {
        self.ensure_domain(domain);

        let mut elections = self.elections.borrow_mut();
        let election = elections
            .get_mut(&domain)
            .ok_or_else(|| CoordinatorError::DomainNotFound { domain })?;

        if !election.is_candidate(challenger) {
            return Err(CoordinatorError::NotACandidate { node_id: challenger });
        }

        let current_leader = election.leader;
        if current_leader == Some(challenger) {
            return Err(CoordinatorError::Internal {
                message: "challenger is already the leader".to_string(),
            });
        }

        let epoch = election.epoch.next();
        let fencing_epoch = Epoch::new(epoch.0 + 1);
        let elected_at = IdsTime::now();

        election.leader = Some(challenger);
        election.epoch = epoch;
        election.fencing_epoch = fencing_epoch;
        election.last_election = Some(elected_at);

        self.update_stats(domain, |s| {
            s.elections_held += 1;
            s.leader_changes += 1;
        });

        Ok(CoordinatorResult::new(
            domain,
            challenger,
            epoch,
            fencing_epoch,
            elected_at,
        ))
    }

    fn resign_leader(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<(), CoordinatorError> {
        let mut elections = self.elections.borrow_mut();
        let election = elections
            .get_mut(&domain)
            .ok_or_else(|| CoordinatorError::DomainNotFound { domain })?;

        if election.leader != Some(node) {
            return Err(CoordinatorError::Internal {
                message: "node is not the leader".to_string(),
            });
        }

        election.leader = None;
        election.fencing_epoch = election.fencing_epoch.next();

        self.update_stats(domain, |s| {
            s.leader_changes += 1;
        });

        Ok(())
    }

    fn acquire_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
        ttl_ms: u64,
    ) -> Result<LockAcquisition, CoordinatorError> {
        let mut locks = self.locks.borrow_mut();

        let lock = locks
            .entry(lock_id)
            .or_insert_with(|| LockState::new(lock_id, ttl_ms));

        if lock.is_held() && lock.holder != Some(node) {
            return Err(CoordinatorError::LockAlreadyHeld {
                holder: lock.holder.unwrap(),
            });
        }

        if lock.is_expired(IdsTime::now().0) {
            lock.holder = None;
            lock.acquired_at = None;
            lock.extension_count = 0;
        }

        lock.holder = Some(node);
        lock.acquired_at = Some(IdsTime::now());
        lock.ttl_ms = ttl_ms;
        lock.extension_count = 0;

        let fencing_token = self.fencing_epoch(domain_for_lock(&lock_id));
        lock.fencing_epoch = fencing_token;

        let expires_at = IdsTime::now().0 + ttl_ms;

        self.update_stats(domain_for_lock(&lock_id), |s| {
            s.locks_acquired += 1;
        });

        Ok(LockAcquisition {
            lock_id,
            holder: node,
            acquired_at: IdsTime::now(),
            ttl_ms,
            expires_at: IdsTime::new(expires_at),
            fencing_token,
        })
    }

    fn release_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
    ) -> Result<(), CoordinatorError> {
        let mut locks = self.locks.borrow_mut();

        let lock = locks
            .get_mut(&lock_id)
            .ok_or_else(|| CoordinatorError::LockNotFound { lock_id })?;

        if lock.holder != Some(node) {
            return Err(CoordinatorError::LockNotHeld { node_id: node });
        }

        lock.holder = None;
        lock.acquired_at = None;
        lock.extension_count = 0;

        self.update_stats(domain_for_lock(&lock_id), |s| {
            s.locks_released += 1;
        });

        Ok(())
    }

    fn is_locked(&self, lock_id: KeyId) -> bool {
        let locks = self.locks.borrow();
        if let Some(lock) = locks.get(&lock_id) {
            lock.is_held() && !lock.is_expired(IdsTime::now().0)
        } else {
            false
        }
    }

    fn lock_holder(&self, lock_id: KeyId) -> Option<NodeId> {
        let locks = self.locks.borrow();
        locks.get(&lock_id).and_then(|l| l.holder)
    }

    fn lock_ttl_remaining(&self, lock_id: KeyId) -> Option<u64> {
        let locks = self.locks.borrow();
        if let Some(lock) = locks.get(&lock_id) {
            if let Some(acquired) = lock.acquired_at {
                let now = IdsTime::now().0;
                if now.saturating_sub(acquired.0) < lock.ttl_ms {
                    return Some(lock.ttl_ms - (now - acquired.0));
                }
            }
        }
        None
    }

    fn extend_lock(
        &self,
        lock_id: KeyId,
        node: NodeId,
        additional_ms: u64,
    ) -> Result<u64, CoordinatorError> {
        let config = self.config.borrow().clone();
        let mut locks = self.locks.borrow_mut();

        let lock = locks
            .get_mut(&lock_id)
            .ok_or_else(|| CoordinatorError::LockNotFound { lock_id })?;

        if lock.holder != Some(node) {
            return Err(CoordinatorError::LockNotHeld { node_id: node });
        }

        if !lock.can_extend(config.max_lock_extensions) {
            return Err(CoordinatorError::Internal {
                message: "maximum lock extensions reached".to_string(),
            });
        }

        lock.ttl_ms += additional_ms;
        lock.extension_count += 1;

        let remaining = self.lock_ttl_remaining(lock_id).unwrap_or(0);

        Ok(remaining)
    }

    fn create_barrier(
        &self,
        barrier_id: KeyId,
        participant_count: u32,
    ) -> Result<BarrierHandle, CoordinatorError> {
        if participant_count == 0 {
            return Err(CoordinatorError::InvalidConfig {
                field: "participant_count".to_string(),
                reason: "participant_count must be greater than 0".to_string(),
            });
        }

        let mut barriers = self.barriers.borrow_mut();
        if barriers.contains_key(&barrier_id) {
            return Err(CoordinatorError::Internal {
                message: "barrier already exists".to_string(),
            });
        }

        let config = self.config.borrow().clone();
        let barrier = BarrierState::new(barrier_id, participant_count, config.barrier_timeout_ms);
        let handle = BarrierHandle {
            barrier_id,
            required_participants: participant_count,
            created_at: barrier.created_at,
            timeout_ms: config.barrier_timeout_ms,
        };
        barriers.insert(barrier_id, barrier);

        self.update_stats(OrderingDomain::default(), |s| {
            s.barriers_created += 1;
        });

        Ok(handle)
    }

    fn join_barrier(
        &self,
        barrier_id: KeyId,
        node: NodeId,
    ) -> Result<BarrierResult, CoordinatorError> {
        let mut barriers = self.barriers.borrow_mut();

        let barrier = barriers
            .get_mut(&barrier_id)
            .ok_or_else(|| CoordinatorError::BarrierNotFound { barrier_id })?;

        if barrier.satisfied {
            return Err(CoordinatorError::BarrierAlreadySatisfied { barrier_id });
        }

        let was_empty = barrier.add_participant(node);
        let satisfied = barrier.satisfied;
        let participants = barrier.participant_count();

        if satisfied {
            self.update_stats(OrderingDomain::default(), |s| {
                s.barriers_satisfied += 1;
            });
        }

        Ok(BarrierResult {
            barrier_id,
            node,
            satisfied,
            participants,
            joined_at: IdsTime::now(),
        })
    }

    fn is_barrier_satisfied(&self, barrier_id: KeyId) -> bool {
        let barriers = self.barriers.borrow();
        barriers
            .get(&barrier_id)
            .map(|b| b.satisfied)
            .unwrap_or(false)
    }

    fn barrier_participants(&self, barrier_id: KeyId) -> u32 {
        let barriers = self.barriers.borrow();
        barriers.get(&barrier_id).map(|b| b.participant_count()).unwrap_or(0)
    }

    fn join_group(
        &self,
        group_id: KeyId,
        node: NodeId,
    ) -> Result<GroupMembership, CoordinatorError> {
        let mut groups = self.groups.borrow_mut();

        let group = groups
            .entry(group_id)
            .or_insert_with(|| GroupState::new(group_id));

        let was_new_member = group.add_member(node);
        let member_count = group.member_count() as u32;
        let is_leader = group.leader == Some(node);

        self.update_stats(OrderingDomain::default(), |s| {
            s.group_joins += 1;
        });

        Ok(GroupMembership {
            group_id,
            node,
            joined_at: IdsTime::now(),
            member_count,
            is_leader,
        })
    }

    fn leave_group(
        &self,
        group_id: KeyId,
        node: NodeId,
    ) -> Result<(), CoordinatorError> {
        let mut groups = self.groups.borrow_mut();

        let group = groups
            .get_mut(&group_id)
            .ok_or_else(|| CoordinatorError::GroupNotFound { group_id })?;

        if !group.is_member(node) {
            return Err(CoordinatorError::NodeNotInGroup { node_id: node });
        }

        group.remove_member(node);

        self.update_stats(OrderingDomain::default(), |s| {
            s.group_leaves += 1;
        });

        Ok(())
    }

    fn group_members(&self, group_id: KeyId) -> Vec<NodeId> {
        let groups = self.groups.borrow();
        if let Some(group) = groups.get(&group_id) {
            group.members.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    fn is_group_member(&self, group_id: KeyId, node: NodeId) -> bool {
        let groups = self.groups.borrow();
        groups
            .get(&group_id)
            .map(|g| g.is_member(node))
            .unwrap_or(false)
    }

    fn record_heartbeat(
        &self,
        node: NodeId,
        domain: OrderingDomain,
    ) -> Result<HeartbeatRecord, CoordinatorError> {
        let mut heartbeats = self.heartbeats.borrow_mut();

        let record = HeartbeatRecord {
            node,
            domain,
            received_at: IdsTime::now(),
            sequence: heartbeats
                .get(&node)
                .map(|r| r.sequence + 1)
                .unwrap_or(1),
        };

        heartbeats.insert(node, record.clone());

        self.update_stats(domain, |s| {
            s.heartbeats_received += 1;
        });

        Ok(record)
    }

    fn last_heartbeat(&self, node: NodeId) -> Option<IdsTime> {
        let heartbeats = self.heartbeats.borrow();
        heartbeats.get(&node).map(|r| r.received_at)
    }

    fn is_node_alive(&self, node: NodeId, timeout_ms: u64) -> bool {
        let heartbeats = self.heartbeats.borrow();
        if let Some(record) = heartbeats.get(&node) {
            !record.is_stale(IdsTime::now().0, timeout_ms)
        } else {
            false
        }
    }

    fn alive_nodes(&self, timeout_ms: u64) -> Vec<NodeId> {
        let heartbeats = self.heartbeats.borrow();
        let now = IdsTime::now().0;
        heartbeats
            .iter()
            .filter(|(_, r)| !r.is_stale(now, timeout_ms))
            .map(|(node, _)| *node)
            .collect()
    }

    fn create_session(
        &self,
        node: NodeId,
        domain: OrderingDomain,
        ttl_ms: u64,
    ) -> Result<SessionHandle, CoordinatorError> {
        let mut sessions = self.sessions.borrow_mut();
        let mut counter = self.session_counter.borrow_mut();

        let session_id = {
            let mut bytes = [0u8; 16];
            bytes[15] = (*counter as u8).wrapping_add(1);
            SessionId::new(bytes)
        };
        *counter += 1;

        let expires_at = IdsTime::now().0 + ttl_ms;
        let session = SessionState::new(session_id, node, domain, ttl_ms);

        sessions.insert(session_id, session);

        self.update_stats(domain, |s| {
            s.sessions_created += 1;
        });

        Ok(SessionHandle {
            session_id,
            node,
            domain,
            created_at: IdsTime::now(),
            ttl_ms,
            expires_at: IdsTime::new(expires_at),
        })
    }

    fn renew_session(
        &self,
        session_id: SessionId,
        node: NodeId,
        additional_ms: u64,
    ) -> Result<SessionHandle, CoordinatorError> {
        let mut sessions = self.sessions.borrow_mut();

        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoordinatorError::SessionNotFound { session_id })?;

        if session.node != node {
            return Err(CoordinatorError::SessionNotOwned { node_id: node });
        }

        if session.is_expired(IdsTime::now().0) {
            return Err(CoordinatorError::SessionExpired { session_id });
        }

        session.renew(additional_ms);

        let expires_at = IdsTime::now().0 + session.ttl_ms;

        Ok(SessionHandle {
            session_id,
            node,
            domain: session.domain,
            created_at: session.created_at,
            ttl_ms: session.ttl_ms,
            expires_at: IdsTime::new(expires_at),
        })
    }

    fn close_session(
        &self,
        session_id: SessionId,
        node: NodeId,
    ) -> Result<(), CoordinatorError> {
        let mut sessions = self.sessions.borrow_mut();

        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| CoordinatorError::SessionNotFound { session_id })?;

        if session.node != node {
            return Err(CoordinatorError::SessionNotOwned { node_id: node });
        }

        session.close();

        self.update_stats(session.domain, |s| {
            s.sessions_closed += 1;
        });

        Ok(())
    }

    fn session_info(&self, session_id: SessionId) -> Option<SessionInfo> {
        let sessions = self.sessions.borrow();
        sessions.get(&session_id).map(|s| SessionInfo {
            session_id: s.session_id,
            node: s.node,
            domain: s.domain,
            state: s.state,
            created_at: s.created_at,
            last_renewal: s.last_renewal,
        })
    }

    fn node_sessions(&self, node: NodeId) -> Vec<SessionId> {
        let sessions = self.sessions.borrow();
        sessions
            .iter()
            .filter(|(_, s)| s.node == node)
            .map(|(id, _)| *id)
            .collect()
    }

    fn quorum_decision(
        &self,
        domain: OrderingDomain,
        proposal_id: MessageId,
        votes: &[(NodeId, bool)],
    ) -> Result<QuorumDecision, CoordinatorError> {
        let config = self.config.borrow().clone();

        let votes_for = votes.iter().filter(|(_, v)| *v).count() as u32;
        let votes_against = votes.iter().filter(|(_, v)| !*v).count() as u32;
        let total_voters = votes.len() as u32;
        let quorum_reached = total_voters >= config.quorum_size;
        let approved = quorum_reached && votes_for > votes_against;

        let decision = QuorumDecision {
            proposal_id,
            domain,
            approved,
            votes_for,
            votes_against,
            total_voters,
            quorum_reached,
            decided_at: IdsTime::now(),
        };

        self.quorum_decisions.borrow_mut().insert(proposal_id, decision.clone());

        self.update_stats(domain, |s| {
            s.quorum_decisions += 1;
        });

        Ok(decision)
    }

    fn quorum_status(&self, domain: OrderingDomain) -> QuorumStatus {
        let config = self.config.borrow().clone();
        let heartbeats = self.heartbeats.borrow();
        let alive = heartbeats.len() as u32;
        let quorum_available = alive >= config.quorum_size;

        QuorumStatus {
            domain,
            total_nodes: alive,
            alive_nodes: alive,
            quorum_size: config.quorum_size,
            quorum_available,
        }
    }

    fn issue_fencing_token(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Result<FencingToken, CoordinatorError> {
        let epoch = self.domain_epoch(domain);
        let token = FencingToken {
            domain,
            node,
            epoch,
            issued_at: IdsTime::now(),
        };

        self.fencing_tokens
            .borrow_mut()
            .insert((domain, node), epoch);

        self.update_stats(domain, |s| {
            s.fencing_tokens_issued += 1;
        });

        Ok(token)
    }

    fn validate_fencing_token(
        &self,
        token: &FencingToken,
    ) -> Result<bool, CoordinatorError> {
        let current_epoch = self.domain_epoch(token.domain);
        Ok(token.is_valid(current_epoch))
    }

    fn node_fencing_epoch(
        &self,
        domain: OrderingDomain,
        node: NodeId,
    ) -> Epoch {
        let tokens = self.fencing_tokens.borrow();
        tokens
            .get(&(domain, node))
            .copied()
            .unwrap_or(Epoch::new(0))
    }

    fn stats(&self, domain: OrderingDomain) -> CoordinatorStats {
        let stats = self.stats.borrow();
        stats.get(&domain).cloned().unwrap_or(CoordinatorStats::new(domain))
    }

    fn domains(&self) -> Vec<OrderingDomain> {
        let elections = self.elections.borrow();
        elections.keys().cloned().collect()
    }

    fn validate_config(&self, config: &CoordinatorConfig) -> Result<(), CoordinatorError> {
        config.validate()
    }
}

///////////////////////////////////////////////////////////////////////////////
// HELPER FUNCTIONS
///////////////////////////////////////////////////////////////////////////////

fn domain_for_lock(_lock_id: &KeyId) -> OrderingDomain {
    OrderingDomain::default()
}

///////////////////////////////////////////////////////////////////////////////
// ADVANCED COORDINATOR ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

/// Leader lease for time-bounded leadership authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderLease {
    pub domain: OrderingDomain,
    pub leader: NodeId,
    pub epoch: Epoch,
    pub granted_at: IdsTime,
    pub ttl_ms: u64,
    pub renewals: u32,
    pub max_renewals: u32,
}

impl LeaderLease {
    pub fn new(domain: OrderingDomain, leader: NodeId, epoch: Epoch, ttl_ms: u64, max_renewals: u32) -> Self {
        Self {
            domain,
            leader,
            epoch,
            granted_at: IdsTime::now(),
            ttl_ms,
            renewals: 0,
            max_renewals,
        }
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.granted_at.0) > self.ttl_ms
    }

    pub fn can_renew(&self) -> bool {
        self.renewals < self.max_renewals
    }

    pub fn renew(&mut self, additional_ms: u64) -> Result<(), CoordinatorError> {
        if !self.can_renew() {
            return Err(CoordinatorError::Internal {
                message: "maximum lease renewals reached".to_string(),
            });
        }
        if self.is_expired(IdsTime::now().0) {
            return Err(CoordinatorError::Internal {
                message: "lease already expired".to_string(),
            });
        }
        self.ttl_ms += additional_ms;
        self.granted_at = IdsTime::now();
        self.renewals += 1;
        Ok(())
    }

    pub fn ttl_remaining(&self, now_ms: u64) -> u64 {
        self.granted_at.0.saturating_add(self.ttl_ms).saturating_sub(now_ms)
    }
}

/// Lock waiter for fair lock acquisition ordering.
#[derive(Debug, Clone)]
pub struct LockWaiter {
    pub lock_id: KeyId,
    pub node: NodeId,
    pub requested_at: IdsTime,
    pub priority: u32,
    pub timeout_ms: u64,
}

impl LockWaiter {
    pub fn new(lock_id: KeyId, node: NodeId, priority: u32, timeout_ms: u64) -> Self {
        Self {
            lock_id,
            node,
            requested_at: IdsTime::now(),
            priority,
            timeout_ms,
        }
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.requested_at.0) > self.timeout_ms
    }
}

/// Quorum vote record for audit and history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumVote {
    pub proposal_id: MessageId,
    pub voter: NodeId,
    pub approved: bool,
    pub voted_at: IdsTime,
    pub domain: OrderingDomain,
}

impl QuorumVote {
    pub fn new(proposal_id: MessageId, voter: NodeId, approved: bool, domain: OrderingDomain) -> Self {
        Self {
            proposal_id,
            voter,
            approved,
            voted_at: IdsTime::now(),
            domain,
        }
    }
}

/// Domain lifecycle state for coordination domain management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainLifecycleState {
    Creating,
    Active,
    Degraded,
    Maintenance,
    Deleting,
    Deleted,
}

impl DomainLifecycleState {
    pub fn is_active(&self) -> bool {
        matches!(self, DomainLifecycleState::Active)
    }

    pub fn can_accept_operations(&self) -> bool {
        matches!(self, DomainLifecycleState::Active | DomainLifecycleState::Degraded)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DomainLifecycleState::Creating => "creating",
            DomainLifecycleState::Active => "active",
            DomainLifecycleState::Degraded => "degraded",
            DomainLifecycleState::Maintenance => "maintenance",
            DomainLifecycleState::Deleting => "deleting",
            DomainLifecycleState::Deleted => "deleted",
        }
    }
}

/// Domain metadata for lifecycle management.
#[derive(Debug, Clone)]
pub struct DomainMetadata {
    pub domain: OrderingDomain,
    pub lifecycle_state: DomainLifecycleState,
    pub created_at: IdsTime,
    pub node_count: u32,
    pub leader_epoch: Epoch,
    pub config_version: u32,
}

impl DomainMetadata {
    pub fn new(domain: OrderingDomain) -> Self {
        Self {
            domain,
            lifecycle_state: DomainLifecycleState::Creating,
            created_at: IdsTime::now(),
            node_count: 0,
            leader_epoch: Epoch::new(0),
            config_version: 1,
        }
    }

    pub fn activate(&mut self) {
        self.lifecycle_state = DomainLifecycleState::Active;
    }

    pub fn degrade(&mut self) {
        self.lifecycle_state = DomainLifecycleState::Degraded;
    }

    pub fn enter_maintenance(&mut self) {
        self.lifecycle_state = DomainLifecycleState::Maintenance;
    }

    pub fn mark_deleting(&mut self) {
        self.lifecycle_state = DomainLifecycleState::Deleting;
    }

    pub fn mark_deleted(&mut self) {
        self.lifecycle_state = DomainLifecycleState::Deleted;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADVANCED COORDINATOR ENGINE IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////

impl CoordinatorEngine {
    /// Get leader health status for a domain.
    pub fn leader_health(&self, domain: OrderingDomain) -> LeaderHealth {
        let elections = self.elections.borrow();
        let heartbeats = self.heartbeats.borrow();
        let election = elections.get(&domain);
        let leader = election.and_then(|e| e.leader);

        let health = match leader {
            Some(leader) => {
                let heartbeat = heartbeats.get(&leader);
                let is_alive = heartbeat.map(|h| !h.is_stale(IdsTime::now().0, 30000)).unwrap_or(false);
                let last_seen = heartbeat.map(|h| h.received_at);
                LeaderHealth {
                    domain,
                    leader,
                    is_alive,
                    last_seen,
                    epoch: election.map(|e| e.epoch).unwrap_or(Epoch::new(0)),
                }
            }
            None => LeaderHealth {
                domain,
                leader: NodeId::new([0u8; 32]),
                is_alive: false,
                last_seen: None,
                epoch: Epoch::new(0),
            },
        };
        health
    }

    /// Get lock waiters for a lock.
    pub fn lock_waiters(&self, _lock_id: KeyId) -> Vec<LockWaiter> {
        // Waiters tracked separately; return empty for reference implementation
        Vec::new()
    }

    /// Get quorum vote history for a proposal.
    pub fn quorum_vote_history(&self, proposal_id: MessageId) -> Vec<QuorumVote> {
        let decisions = self.quorum_decisions.borrow();
        if let Some(decision) = decisions.get(&proposal_id) {
            let mut votes = Vec::new();
            // Reconstruct from decision data (reference implementation)
            for i in 0..decision.votes_for {
                votes.push(QuorumVote::new(proposal_id, NodeId::new([i as u8; 32]), true, decision.domain));
            }
            for i in 0..decision.votes_against {
                votes.push(QuorumVote::new(proposal_id, NodeId::new([(i + 100) as u8; 32]), false, decision.domain));
            }
            votes
        } else {
            Vec::new()
        }
    }

    /// Get domain metadata.
    pub fn domain_metadata(&self, domain: OrderingDomain) -> Option<DomainMetadata> {
        let elections = self.elections.borrow();
        let election = elections.get(&domain)?;
        Some(DomainMetadata {
            domain,
            lifecycle_state: if election.has_leader() { DomainLifecycleState::Active } else { DomainLifecycleState::Creating },
            created_at: election.last_election.unwrap_or(IdsTime::now()),
            node_count: election.candidate_count() as u32,
            leader_epoch: election.epoch,
            config_version: 1,
        })
    }

    /// Get all domain metadata.
    pub fn all_domain_metadata(&self) -> Vec<DomainMetadata> {
        let elections = self.elections.borrow();
        elections
            .iter()
            .map(|(domain, election)| DomainMetadata {
                domain: *domain,
                lifecycle_state: if election.has_leader() { DomainLifecycleState::Active } else { DomainLifecycleState::Creating },
                created_at: election.last_election.unwrap_or(IdsTime::now()),
                node_count: election.candidate_count() as u32,
                leader_epoch: election.epoch,
                config_version: 1,
            })
            .collect()
    }

    /// Get total leader elections across all domains.
    pub fn total_elections(&self) -> u64 {
        let stats = self.stats.borrow();
        stats.values().map(|s| s.elections_held).sum()
    }

    /// Get total locks held across all domains.
    pub fn total_locks_held(&self) -> u32 {
        let locks = self.locks.borrow();
        locks.values().filter(|l| l.is_held()).count() as u32
    }

    /// Get total active barriers.
    pub fn total_active_barriers(&self) -> u32 {
        let barriers = self.barriers.borrow();
        barriers.values().filter(|b| !b.satisfied).count() as u32
    }

    /// Get total satisfied barriers.
    pub fn total_satisfied_barriers(&self) -> u32 {
        let barriers = self.barriers.borrow();
        barriers.values().filter(|b| b.satisfied).count() as u32
    }

    /// Get total active groups.
    pub fn total_active_groups(&self) -> u32 {
        let groups = self.groups.borrow();
        groups.values().filter(|g| !g.members.is_empty()).count() as u32
    }

    /// Get total active sessions.
    pub fn total_active_sessions(&self) -> u32 {
        let sessions = self.sessions.borrow();
        sessions.values().filter(|s| s.state == SessionState::Active).count() as u32
    }

    /// Get total expired sessions.
    pub fn total_expired_sessions(&self, now_ms: u64) -> u32 {
        let sessions = self.sessions.borrow();
        sessions.values().filter(|s| s.is_expired(now_ms)).count() as u32
    }

    /// Expire all expired sessions.
    pub fn expire_sessions(&self, now_ms: u64) -> u32 {
        let mut sessions = self.sessions.borrow_mut();
        let mut expired_count = 0u32;
        for session in sessions.values_mut() {
            if session.is_expired(now_ms) && session.state == SessionState::Active {
                session.state = SessionState::Expired;
                expired_count += 1;
            }
        }
        expired_count
    }

    /// Clean up expired locks.
    pub fn cleanup_expired_locks(&self, now_ms: u64) -> u32 {
        let mut locks = self.locks.borrow_mut();
        let mut cleaned = 0u32;
        let lock_ids: Vec<KeyId> = locks.keys().cloned().collect();
        for lock_id in lock_ids {
            if let Some(lock) = locks.get_mut(&lock_id) {
                if lock.is_expired(now_ms) && lock.is_held() {
                    lock.holder = None;
                    lock.acquired_at = None;
                    cleaned += 1;
                }
            }
        }
        cleaned
    }

    /// Clean up expired barriers.
    pub fn cleanup_expired_barriers(&self, now_ms: u64) -> u32 {
        let mut barriers = self.barriers.borrow_mut();
        let mut cleaned = 0u32;
        let barrier_ids: Vec<KeyId> = barriers.keys().cloned().collect();
        for barrier_id in barrier_ids {
            if let Some(barrier) = barriers.get_mut(&barrier_id) {
                if !barrier.satisfied && barrier.is_expired(now_ms) {
                    barriers.remove(&barrier_id);
                    cleaned += 1;
                }
            }
        }
        cleaned
    }

    /// Get fencing token count for a domain.
    pub fn fencing_token_count(&self, domain: OrderingDomain) -> u32 {
        let tokens = self.fencing_tokens.borrow();
        tokens.keys().filter(|(d, _)| *d == domain).count() as u32
    }

    /// Revoke all fencing tokens for a node.
    pub fn revoke_fencing_tokens_for_node(&self, node: NodeId) -> u32 {
        let mut tokens = self.fencing_tokens.borrow_mut();
        let keys: Vec<(OrderingDomain, NodeId)> = tokens.keys().filter(|(_, n)| *n == node).cloned().collect();
        let count = keys.len() as u32;
        for key in keys {
            tokens.remove(&key);
        }
        count
    }

    /// Get leader change history (simplified).
    pub fn leader_change_count(&self, domain: OrderingDomain) -> u64 {
        let stats = self.stats.borrow();
        stats.get(&domain).map(|s| s.leader_changes).unwrap_or(0)
    }

    /// Check if domain has quorum available.
    pub fn has_quorum(&self, domain: OrderingDomain) -> bool {
        let status = self.quorum_status(domain);
        status.quorum_available
    }

    /// Get domain election state.
    pub fn election_state(&self, domain: OrderingDomain) -> Option<ElectionStateSummary> {
        let elections = self.elections.borrow();
        let election = elections.get(&domain)?;
        Some(ElectionStateSummary {
            domain,
            has_leader: election.has_leader(),
            leader: election.leader,
            epoch: election.epoch,
            candidate_count: election.candidate_count(),
            last_election: election.last_election,
        })
    }
}

/// Leader health summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderHealth {
    pub domain: OrderingDomain,
    pub leader: NodeId,
    pub is_alive: bool,
    pub last_seen: Option<IdsTime>,
    pub epoch: Epoch,
}

/// Election state summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElectionStateSummary {
    pub domain: OrderingDomain,
    pub has_leader: bool,
    pub leader: Option<NodeId>,
    pub epoch: Epoch,
    pub candidate_count: usize,
    pub last_election: Option<IdsTime>,
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

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

    fn test_domain() -> OrderingDomain {
        OrderingDomain::new([1u8; 16])
    }

    fn test_domain2() -> OrderingDomain {
        OrderingDomain::new([2u8; 16])
    }

    fn test_key_id() -> KeyId {
        KeyId::new([1u8; 16])
    }

    fn test_key_id2() -> KeyId {
        KeyId::new([2u8; 16])
    }

    fn test_message_id() -> MessageId {
        MessageId::new([1u8; 16])
    }

    fn test_session_id() -> SessionId {
        SessionId::new([1u8; 16])
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_ids_time() -> IdsTime {
        IdsTime::now()
    }

    fn test_config() -> CoordinatorConfig {
        CoordinatorConfig::default()
    }

    // ========================================================================
    // CoordinatorConfig tests
    // ========================================================================

    #[test]
    fn coordinator_config_default() {
        let config = CoordinatorConfig::default();
        assert_eq!(config.election_timeout_ms, 10000);
        assert_eq!(config.heartbeat_interval_ms, 5000);
        assert_eq!(config.heartbeat_timeout_ms, 30000);
        assert_eq!(config.lock_ttl_ms, 60000);
        assert_eq!(config.max_lock_extensions, 3);
        assert_eq!(config.barrier_timeout_ms, 120000);
        assert_eq!(config.session_ttl_ms, 300000);
        assert_eq!(config.quorum_size, 3);
        assert_eq!(config.max_sessions, 1000);
        assert!(config.enable_fencing);
        assert!(config.auto_reelect);
    }

    #[test]
    fn coordinator_config_new_valid() {
        let config = CoordinatorConfig::new(5000).unwrap();
        assert_eq!(config.election_timeout_ms, 5000);
    }

    #[test]
    fn coordinator_config_new_zero_timeout() {
        assert!(CoordinatorConfig::new(0).is_err());
    }

    #[test]
    fn coordinator_config_validate() {
        let mut config = CoordinatorConfig::default();
        config.election_timeout_ms = 0;
        assert!(config.validate().is_err());
        config.election_timeout_ms = 10000;
        config.heartbeat_interval_ms = 0;
        assert!(config.validate().is_err());
        config.heartbeat_interval_ms = 5000;
        config.heartbeat_timeout_ms = 0;
        assert!(config.validate().is_err());
        config.heartbeat_timeout_ms = 30000;
        config.lock_ttl_ms = 0;
        assert!(config.validate().is_err());
        config.lock_ttl_ms = 60000;
        config.barrier_timeout_ms = 0;
        assert!(config.validate().is_err());
        config.barrier_timeout_ms = 120000;
        config.session_ttl_ms = 0;
        assert!(config.validate().is_err());
        config.session_ttl_ms = 300000;
        config.quorum_size = 0;
        assert!(config.validate().is_err());
        config.quorum_size = 3;
        config.max_sessions = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn coordinator_config_heartbeat_ratio() {
        let config = CoordinatorConfig::default();
        assert!((config.heartbeat_ratio() - 6.0).abs() < 0.01);
    }

    #[test]
    fn coordinator_config_strict() {
        let config = CoordinatorConfig::strict();
        assert_eq!(config.election_timeout_ms, 5000);
        assert_eq!(config.heartbeat_interval_ms, 1000);
        assert_eq!(config.quorum_size, 5);
        assert!(config.enable_fencing);
    }

    #[test]
    fn coordinator_config_lenient() {
        let config = CoordinatorConfig::lenient();
        assert_eq!(config.election_timeout_ms, 60000);
        assert_eq!(config.quorum_size, 1);
        assert!(!config.enable_fencing);
        assert!(!config.auto_reelect);
    }

    #[test]
    fn coordinator_config_serialization_roundtrip() {
        let config = CoordinatorConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CoordinatorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.election_timeout_ms, deserialized.election_timeout_ms);
        assert_eq!(config.quorum_size, deserialized.quorum_size);
    }

    // ========================================================================
    // CoordinatorResult tests
    // ========================================================================

    #[test]
    fn coordinator_result_new() {
        let result = CoordinatorResult::new(
            test_domain(),
            test_node_id(),
            Epoch::new(5),
            Epoch::new(6),
            IdsTime::now(),
        );
        assert_eq!(result.domain(), test_domain());
        assert_eq!(result.leader, test_node_id());
        assert_eq!(result.epoch, Epoch::new(5));
        assert_eq!(result.fencing_epoch, Epoch::new(6));
    }

    #[test]
    fn coordinator_result_is_recent() {
        let now = IdsTime::now().0;
        let result = CoordinatorResult::new(
            test_domain(),
            test_node_id(),
            Epoch::new(1),
            Epoch::new(2),
            IdsTime::new(now),
        );
        assert!(result.is_recent(now + 5000, 10000));
        assert!(!result.is_recent(now + 20000, 10000));
    }

    #[test]
    fn coordinator_result_serialization_roundtrip() {
        let result = CoordinatorResult::new(
            test_domain(),
            test_node_id(),
            Epoch::new(5),
            Epoch::new(6),
            IdsTime::now(),
        );
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CoordinatorResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.epoch, deserialized.epoch);
        assert_eq!(result.fencing_epoch, deserialized.fencing_epoch);
    }

    // ========================================================================
    // LockAcquisition tests
    // ========================================================================

    #[test]
    fn lock_acquisition_new() {
        let now = IdsTime::now().0;
        let acquisition = LockAcquisition {
            lock_id: test_key_id(),
            holder: test_node_id(),
            acquired_at: IdsTime::new(now),
            ttl_ms: 60000,
            expires_at: IdsTime::new(now + 60000),
            fencing_token: Epoch::new(1),
        };
        assert!(!acquisition.is_expired(now));
        assert!(acquisition.is_expired(now + 70000));
        assert_eq!(acquisition.ttl_remaining(now), 60000);
    }

    #[test]
    fn lock_acquisition_ttl_remaining() {
        let now = IdsTime::now().0;
        let acquisition = LockAcquisition {
            lock_id: test_key_id(),
            holder: test_node_id(),
            acquired_at: IdsTime::new(now),
            ttl_ms: 60000,
            expires_at: IdsTime::new(now + 60000),
            fencing_token: Epoch::new(1),
        };
        assert_eq!(acquisition.ttl_remaining(now + 30000), 30000);
    }

    // ========================================================================
    // BarrierHandle tests
    // ========================================================================

    #[test]
    fn barrier_handle_new() {
        let now = IdsTime::now().0;
        let handle = BarrierHandle {
            barrier_id: test_key_id(),
            required_participants: 3,
            created_at: IdsTime::new(now),
            timeout_ms: 120000,
        };
        assert!(!handle.is_expired(now));
        assert!(handle.is_expired(now + 200000));
    }

    // ========================================================================
    // BarrierResult tests
    // ========================================================================

    #[test]
    fn barrier_result_satisfied() {
        let result = BarrierResult {
            barrier_id: test_key_id(),
            node: test_node_id(),
            satisfied: true,
            participants: 3,
            joined_at: IdsTime::now(),
        };
        assert!(result.is_satisfied());
    }

    #[test]
    fn barrier_result_not_satisfied() {
        let result = BarrierResult {
            barrier_id: test_key_id(),
            node: test_node_id(),
            satisfied: false,
            participants: 1,
            joined_at: IdsTime::now(),
        };
        assert!(!result.is_satisfied());
    }

    // ========================================================================
    // GroupMembership tests
    // ========================================================================

    #[test]
    fn group_membership_new() {
        let membership = GroupMembership {
            group_id: test_key_id(),
            node: test_node_id(),
            joined_at: IdsTime::now(),
            member_count: 5,
            is_leader: true,
        };
        assert!(membership.is_member());
        assert!(membership.is_leader);
    }

    // ========================================================================
    // HeartbeatRecord tests
    // ========================================================================

    #[test]
    fn heartbeat_record_stale() {
        let now = IdsTime::now().0;
        let record = HeartbeatRecord {
            node: test_node_id(),
            domain: test_domain(),
            received_at: IdsTime::new(now),
            sequence: 1,
        };
        assert!(!record.is_stale(now + 10000, 30000));
        assert!(record.is_stale(now + 50000, 30000));
    }

    #[test]
    fn heartbeat_record_sequence() {
        let now = IdsTime::now().0;
        let record = HeartbeatRecord {
            node: test_node_id(),
            domain: test_domain(),
            received_at: IdsTime::new(now),
            sequence: 42,
        };
        assert_eq!(record.sequence, 42);
    }

    // ========================================================================
    // SessionHandle tests
    // ========================================================================

    #[test]
    fn session_handle_expired() {
        let now = IdsTime::now().0;
        let handle = SessionHandle {
            session_id: test_session_id(),
            node: test_node_id(),
            domain: test_domain(),
            created_at: IdsTime::new(now),
            ttl_ms: 60000,
            expires_at: IdsTime::new(now + 60000),
        };
        assert!(!handle.is_expired(now));
        assert!(handle.is_expired(now + 70000));
        assert_eq!(handle.ttl_remaining(now), 60000);
    }

    // ========================================================================
    // SessionInfo tests
    // ========================================================================

    #[test]
    fn session_info_active() {
        let info = SessionInfo {
            session_id: test_session_id(),
            node: test_node_id(),
            domain: test_domain(),
            state: SessionState::Active,
            created_at: IdsTime::now(),
            last_renewal: None,
        };
        assert!(info.state.is_active());
        assert!(!info.state.is_terminal());
    }

    #[test]
    fn session_info_terminal() {
        let info = SessionInfo {
            session_id: test_session_id(),
            node: test_node_id(),
            domain: test_domain(),
            state: SessionState::Expired,
            created_at: IdsTime::now(),
            last_renewal: None,
        };
        assert!(!info.state.is_active());
        assert!(info.state.is_terminal());
    }

    #[test]
    fn session_info_closed() {
        let info = SessionInfo {
            session_id: test_session_id(),
            node: test_node_id(),
            domain: test_domain(),
            state: SessionState::Closed,
            created_at: IdsTime::now(),
            last_renewal: None,
        };
        assert!(!info.state.is_active());
        assert!(info.state.is_terminal());
    }

    // ========================================================================
    // SessionState tests
    // ========================================================================

    #[test]
    fn session_state_variants() {
        assert!(SessionState::Active.is_active());
        assert!(!SessionState::Active.is_terminal());
        assert!(!SessionState::Expired.is_active());
        assert!(SessionState::Expired.is_terminal());
        assert!(!SessionState::Closed.is_active());
        assert!(SessionState::Closed.is_terminal());
    }

    // ========================================================================
    // QuorumDecision tests
    // ========================================================================

    #[test]
    fn quorum_decision_approved() {
        let decision = QuorumDecision {
            proposal_id: test_message_id(),
            domain: test_domain(),
            approved: true,
            votes_for: 3,
            votes_against: 1,
            total_voters: 4,
            quorum_reached: true,
            decided_at: IdsTime::now(),
        };
        assert!(decision.is_approved());
        assert!(decision.quorum_met());
        assert!((decision.approval_rate() - 0.75).abs() < 0.01);
    }

    #[test]
    fn quorum_decision_not_approved() {
        let decision = QuorumDecision {
            proposal_id: test_message_id(),
            domain: test_domain(),
            approved: false,
            votes_for: 1,
            votes_against: 3,
            total_voters: 4,
            quorum_reached: true,
            decided_at: IdsTime::now(),
        };
        assert!(!decision.is_approved());
        assert!((decision.approval_rate() - 0.25).abs() < 0.01);
    }

    #[test]
    fn quorum_decision_approval_rate_zero() {
        let decision = QuorumDecision {
            proposal_id: test_message_id(),
            domain: test_domain(),
            approved: false,
            votes_for: 0,
            votes_against: 0,
            total_voters: 0,
            quorum_reached: false,
            decided_at: IdsTime::now(),
        };
        assert!((decision.approval_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn quorum_decision_serialization_roundtrip() {
        let decision = QuorumDecision {
            proposal_id: test_message_id(),
            domain: test_domain(),
            approved: true,
            votes_for: 3,
            votes_against: 1,
            total_voters: 4,
            quorum_reached: true,
            decided_at: IdsTime::now(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        let deserialized: QuorumDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision.approved, deserialized.approved);
        assert_eq!(decision.votes_for, deserialized.votes_for);
    }

    // ========================================================================
    // QuorumStatus tests
    // ========================================================================

    #[test]
    fn quorum_status_available() {
        let status = QuorumStatus {
            domain: test_domain(),
            total_nodes: 5,
            alive_nodes: 4,
            quorum_size: 3,
            quorum_available: true,
        };
        assert!(status.quorum_achievable());
    }

    #[test]
    fn quorum_status_not_available() {
        let status = QuorumStatus {
            domain: test_domain(),
            total_nodes: 2,
            alive_nodes: 1,
            quorum_size: 3,
            quorum_available: false,
        };
        assert!(!status.quorum_achievable());
    }

    // ========================================================================
    // FencingToken tests
    // ========================================================================

    #[test]
    fn fencing_token_valid() {
        let token = FencingToken {
            domain: test_domain(),
            node: test_node_id(),
            epoch: Epoch::new(5),
            issued_at: IdsTime::now(),
        };
        assert!(token.is_valid(Epoch::new(3)));
        assert!(token.is_valid(Epoch::new(5)));
        assert!(!token.is_valid(Epoch::new(6)));
    }

    #[test]
    fn fencing_token_stale() {
        let token = FencingToken {
            domain: test_domain(),
            node: test_node_id(),
            epoch: Epoch::new(3),
            issued_at: IdsTime::now(),
        };
        assert!(token.is_stale(Epoch::new(5)));
        assert!(!token.is_stale(Epoch::new(3)));
    }

    #[test]
    fn fencing_token_serialization_roundtrip() {
        let token = FencingToken {
            domain: test_domain(),
            node: test_node_id(),
            epoch: Epoch::new(5),
            issued_at: IdsTime::now(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: FencingToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token.epoch, deserialized.epoch);
        assert_eq!(token.node, deserialized.node);
    }

    // ========================================================================
    // CoordinatorStats tests
    // ========================================================================

    #[test]
    fn coordinator_stats_new() {
        let stats = CoordinatorStats::new(test_domain());
        assert_eq!(stats.domain, test_domain());
        assert_eq!(stats.elections_held, 0);
        assert_eq!(stats.leader_changes, 0);
        assert_eq!(stats.locks_acquired, 0);
        assert!((stats.election_rate() - 0.0).abs() < 0.01);
        assert!((stats.lock_success_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn coordinator_stats_election_rate() {
        let mut stats = CoordinatorStats::new(test_domain());
        stats.elections_held = 7;
        stats.leader_changes = 3;
        assert!((stats.election_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn coordinator_stats_lock_success_rate() {
        let mut stats = CoordinatorStats::new(test_domain());
        stats.locks_acquired = 8;
        stats.locks_released = 2;
        assert!((stats.lock_success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn coordinator_stats_serialization_roundtrip() {
        let mut stats = CoordinatorStats::new(test_domain());
        stats.elections_held = 5;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CoordinatorStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.elections_held, deserialized.elections_held);
    }

    // ========================================================================
    // CoordinatorError tests
    // ========================================================================

    #[test]
    fn coordinator_error_category() {
        assert_eq!(
            CoordinatorError::InvalidConfig { field: "x".to_string(), reason: "y".to_string() }.category(),
            "invalid_config"
        );
        assert_eq!(
            CoordinatorError::DomainNotFound { domain: test_domain() }.category(),
            "domain_not_found"
        );
        assert_eq!(
            CoordinatorError::NoCandidates { domain: test_domain() }.category(),
            "no_candidates"
        );
        assert_eq!(
            CoordinatorError::LockNotFound { lock_id: test_key_id() }.category(),
            "lock_not_found"
        );
        assert_eq!(
            CoordinatorError::SessionExpired { session_id: test_session_id() }.category(),
            "session_expired"
        );
    }

    #[test]
    fn coordinator_error_is_recoverable() {
        assert!(CoordinatorError::LockExpired { lock_id: test_key_id() }.is_recoverable());
        assert!(CoordinatorError::SessionExpired { session_id: test_session_id() }.is_recoverable());
        assert!(CoordinatorError::QuorumNotReached { votes_for: 1, total_voters: 3 }.is_recoverable());
        assert!(!CoordinatorError::DomainNotFound { domain: test_domain() }.is_recoverable());
    }

    #[test]
    fn coordinator_error_suggested_action() {
        assert_eq!(
            CoordinatorError::DomainNotFound { domain: test_domain() }.suggested_action(),
            "create the domain first"
        );
    }

    // ========================================================================
    // CoordinatorEngine tests
    // ========================================================================

    #[test]
    fn coordinator_engine_new() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.domains().len(), 0);
        assert_eq!(engine.total_elections(), 0);
    }

    #[test]
    fn coordinator_engine_with_config() {
        let config = CoordinatorConfig::new(5000).unwrap();
        let engine = CoordinatorEngine::with_config(config).unwrap();
        assert_eq!(engine.domains().len(), 0);
    }

    #[test]
    fn coordinator_engine_with_config_invalid() {
        let result = CoordinatorConfig::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn coordinator_engine_elect_leader() {
        let engine = CoordinatorEngine::new();
        let result = engine.elect_leader(test_domain(), &[test_node_id(), test_node_id2()]).unwrap();
        assert_eq!(result.leader, test_node_id());
        assert_eq!(engine.leader(test_domain()), Some(test_node_id()));
        assert_eq!(engine.domains().len(), 1);
    }

    #[test]
    fn coordinator_engine_elect_leader_no_candidates() {
        let engine = CoordinatorEngine::new();
        let err = engine.elect_leader(test_domain(), &[]).unwrap_err();
        assert!(matches!(err, CoordinatorError::NoCandidates { .. }));
    }

    #[test]
    fn coordinator_engine_challenge_leader() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id(), test_node_id2()]).unwrap();
        let result = engine.challenge_leader(test_domain(), test_node_id2()).unwrap();
        assert_eq!(result.leader, test_node_id2());
        assert_eq!(engine.leader(test_domain()), Some(test_node_id2()));
    }

    #[test]
    fn coordinator_engine_challenge_not_candidate() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        let err = engine.challenge_leader(test_domain(), test_node_id2()).unwrap_err();
        assert!(matches!(err, CoordinatorError::NotACandidate { .. }));
    }

    #[test]
    fn coordinator_engine_resign_leader() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.resign_leader(test_domain(), test_node_id()).unwrap();
        assert_eq!(engine.leader(test_domain()), None);
    }

    #[test]
    fn coordinator_engine_acquire_and_release_lock() {
        let engine = CoordinatorEngine::new();
        let acquisition = engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        assert_eq!(acquisition.holder, test_node_id());
        assert!(engine.is_locked(test_key_id()));
        assert_eq!(engine.lock_holder(test_key_id()), Some(test_node_id()));
        engine.release_lock(test_key_id(), test_node_id()).unwrap();
        assert!(!engine.is_locked(test_key_id()));
    }

    #[test]
    fn coordinator_engine_lock_already_held() {
        let engine = CoordinatorEngine::new();
        engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        let err = engine.acquire_lock(test_key_id(), test_node_id2(), 60000).unwrap_err();
        assert!(matches!(err, CoordinatorError::LockAlreadyHeld { .. }));
    }

    #[test]
    fn coordinator_engine_release_lock_not_held() {
        let engine = CoordinatorEngine::new();
        let err = engine.release_lock(test_key_id(), test_node_id()).unwrap_err();
        assert!(matches!(err, CoordinatorError::LockNotHeld { .. }));
    }

    #[test]
    fn coordinator_engine_lock_ttl_remaining() {
        let engine = CoordinatorEngine::new();
        engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        let ttl = engine.lock_ttl_remaining(test_key_id()).unwrap();
        assert!(ttl > 0);
        assert!(ttl <= 60000);
    }

    #[test]
    fn coordinator_engine_extend_lock() {
        let engine = CoordinatorEngine::new();
        engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        let remaining = engine.extend_lock(test_key_id(), test_node_id(), 30000).unwrap();
        assert!(remaining > 0);
    }

    #[test]
    fn coordinator_engine_create_and_join_barrier() {
        let engine = CoordinatorEngine::new();
        let handle = engine.create_barrier(test_key_id(), 3).unwrap();
        assert_eq!(handle.required_participants, 3);
        assert!(!engine.is_barrier_satisfied(test_key_id()));
        assert_eq!(engine.barrier_participants(test_key_id()), 0);

        let result = engine.join_barrier(test_key_id(), test_node_id()).unwrap();
        assert!(!result.satisfied);
        assert_eq!(result.participants, 1);

        engine.join_barrier(test_key_id(), test_node_id2()).unwrap();
        assert_eq!(engine.barrier_participants(test_key_id()), 2);

        let result = engine.join_barrier(test_key_id(), test_node_id3()).unwrap();
        assert!(result.satisfied);
        assert!(engine.is_barrier_satisfied(test_key_id()));
    }

    #[test]
    fn coordinator_engine_join_barrier_already_satisfied() {
        let engine = CoordinatorEngine::new();
        engine.create_barrier(test_key_id(), 1).unwrap();
        engine.join_barrier(test_key_id(), test_node_id()).unwrap();
        let err = engine.join_barrier(test_key_id(), test_node_id2()).unwrap_err();
        assert!(matches!(err, CoordinatorError::BarrierAlreadySatisfied { .. }));
    }

    #[test]
    fn coordinator_engine_join_and_leave_group() {
        let engine = CoordinatorEngine::new();
        let membership = engine.join_group(test_key_id(), test_node_id()).unwrap();
        assert_eq!(membership.member_count, 1);
        assert!(membership.is_leader);
        assert!(engine.is_group_member(test_key_id(), test_node_id()));
        assert_eq!(engine.group_members(test_key_id()).len(), 1);

        engine.leave_group(test_key_id(), test_node_id()).unwrap();
        assert!(!engine.is_group_member(test_key_id(), test_node_id()));
        assert_eq!(engine.group_members(test_key_id()).len(), 0);
    }

    #[test]
    fn coordinator_engine_leave_group_not_member() {
        let engine = CoordinatorEngine::new();
        let err = engine.leave_group(test_key_id(), test_node_id()).unwrap_err();
        assert!(matches!(err, CoordinatorError::NodeNotInGroup { .. }));
    }

    #[test]
    fn coordinator_engine_record_and_check_heartbeat() {
        let engine = CoordinatorEngine::new();
        let record = engine.record_heartbeat(test_node_id(), test_domain()).unwrap();
        assert_eq!(record.node, test_node_id());
        assert_eq!(record.domain, test_domain());
        assert_eq!(record.sequence, 1);

        let last = engine.last_heartbeat(test_node_id()).unwrap();
        assert!(engine.is_node_alive(test_node_id(), 30000));
        assert_eq!(engine.alive_nodes(30000).len(), 1);
    }

    #[test]
    fn coordinator_engine_node_not_alive() {
        let engine = CoordinatorEngine::new();
        assert!(!engine.is_node_alive(test_node_id(), 30000));
        assert_eq!(engine.alive_nodes(30000).len(), 0);
    }

    #[test]
    fn coordinator_engine_create_and_renew_session() {
        let engine = CoordinatorEngine::new();
        let handle = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        assert_eq!(handle.node, test_node_id());
        assert_eq!(handle.domain, test_domain());

        let renewed = engine.renew_session(handle.session_id, test_node_id(), 30000).unwrap();
        assert_eq!(renewed.session_id, handle.session_id);
    }

    #[test]
    fn coordinator_engine_renew_session_not_owned() {
        let engine = CoordinatorEngine::new();
        let handle = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        let err = engine.renew_session(handle.session_id, test_node_id2(), 30000).unwrap_err();
        assert!(matches!(err, CoordinatorError::SessionNotOwned { .. }));
    }

    #[test]
    fn coordinator_engine_close_session() {
        let engine = CoordinatorEngine::new();
        let handle = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        engine.close_session(handle.session_id, test_node_id()).unwrap();
        let info = engine.session_info(handle.session_id).unwrap();
        assert_eq!(info.state, SessionState::Closed);
    }

    #[test]
    fn coordinator_engine_session_info_not_found() {
        let engine = CoordinatorEngine::new();
        assert!(engine.session_info(test_session_id()).is_none());
    }

    #[test]
    fn coordinator_engine_node_sessions() {
        let engine = CoordinatorEngine::new();
        let handle1 = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        let handle2 = engine.create_session(test_node_id(), test_domain2(), 60000).unwrap();
        let sessions = engine.node_sessions(test_node_id());
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&handle1.session_id));
        assert!(sessions.contains(&handle2.session_id));
    }

    #[test]
    fn coordinator_engine_quorum_decision() {
        let engine = CoordinatorEngine::new();
        let votes = vec![
            (test_node_id(), true),
            (test_node_id2(), true),
            (test_node_id3(), false),
        ];
        let decision = engine.quorum_decision(test_domain(), test_message_id(), &votes).unwrap();
        assert!(decision.approved);
        assert_eq!(decision.votes_for, 2);
        assert_eq!(decision.votes_against, 1);
        assert!(decision.quorum_reached);
    }

    #[test]
    fn coordinator_engine_quorum_not_reached() {
        let engine = CoordinatorEngine::new();
        let votes = vec![(test_node_id(), true)];
        let decision = engine.quorum_decision(test_domain(), test_message_id(), &votes).unwrap();
        assert!(!decision.quorum_reached);
        assert!(!decision.approved);
    }

    #[test]
    fn coordinator_engine_quorum_status() {
        let engine = CoordinatorEngine::new();
        let status = engine.quorum_status(test_domain());
        assert_eq!(status.quorum_size, 3);
    }

    #[test]
    fn coordinator_engine_issue_and_validate_fencing_token() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        let token = engine.issue_fencing_token(test_domain(), test_node_id()).unwrap();
        assert_eq!(token.node, test_node_id());
        assert!(engine.validate_fencing_token(&token).unwrap());
    }

    #[test]
    fn coordinator_engine_node_fencing_epoch() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.issue_fencing_token(test_domain(), test_node_id()).unwrap();
        let epoch = engine.node_fencing_epoch(test_domain(), test_node_id());
        assert!(epoch.0 > 0);
    }

    #[test]
    fn coordinator_engine_stats() {
        let engine = CoordinatorEngine::new();
        let stats = engine.stats(test_domain());
        assert_eq!(stats.domain, test_domain());
    }

    #[test]
    fn coordinator_engine_validate_config() {
        let engine = CoordinatorEngine::new();
        let config = CoordinatorConfig::default();
        assert!(engine.validate_config(&config).is_ok());
    }

    // ========================================================================
    // LeaderLease tests
    // ========================================================================

    #[test]
    fn leader_lease_new() {
        let lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 3);
        assert_eq!(lease.domain, test_domain());
        assert_eq!(lease.leader, test_node_id());
        assert_eq!(lease.epoch, Epoch::new(1));
        assert_eq!(lease.max_renewals, 3);
        assert_eq!(lease.renewals, 0);
    }

    #[test]
    fn leader_lease_is_expired() {
        let now = IdsTime::now().0;
        let mut lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 1000, 3);
        lease.granted_at = IdsTime::new(now);
        assert!(!lease.is_expired(now + 500));
        assert!(lease.is_expired(now + 2000));
    }

    #[test]
    fn leader_lease_can_renew() {
        let lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 3);
        assert!(lease.can_renew());
        let mut lease = lease;
        lease.renewals = 3;
        assert!(!lease.can_renew());
    }

    #[test]
    fn leader_lease_renew() {
        let mut lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 3);
        let now = IdsTime::now().0;
        lease.granted_at = IdsTime::new(now);
        lease.renew(30000).unwrap();
        assert_eq!(lease.renewals, 1);
        assert!(lease.ttl_remaining(now) > 60000);
    }

    #[test]
    fn leader_lease_renew_maxed() {
        let mut lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 0);
        let err = lease.renew(30000).unwrap_err();
        assert!(matches!(err, CoordinatorError::Internal { .. }));
    }

    #[test]
    fn leader_lease_ttl_remaining() {
        let now = IdsTime::now().0;
        let mut lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 3);
        lease.granted_at = IdsTime::new(now);
        assert!((lease.ttl_remaining(now + 10000) - 50000).abs() < 1000);
    }

    // ========================================================================
    // LockWaiter tests
    // ========================================================================

    #[test]
    fn lock_waiter_new() {
        let now = IdsTime::now().0;
        let waiter = LockWaiter::new(test_key_id(), test_node_id(), 5, 30000);
        assert_eq!(waiter.lock_id, test_key_id());
        assert_eq!(waiter.node, test_node_id());
        assert_eq!(waiter.priority, 5);
        assert!(!waiter.is_expired(now));
    }

    #[test]
    fn lock_waiter_is_expired() {
        let now = IdsTime::now().0;
        let mut waiter = LockWaiter::new(test_key_id(), test_node_id(), 1, 1000);
        waiter.requested_at = IdsTime::new(now);
        assert!(!waiter.is_expired(now + 500));
        assert!(waiter.is_expired(now + 2000));
    }

    // ========================================================================
    // QuorumVote tests
    // ========================================================================

    #[test]
    fn quorum_vote_new() {
        let vote = QuorumVote::new(test_message_id(), test_node_id(), true, test_domain());
        assert_eq!(vote.proposal_id, test_message_id());
        assert_eq!(vote.voter, test_node_id());
        assert!(vote.approved);
    }

    // ========================================================================
    // DomainLifecycleState tests
    // ========================================================================

    #[test]
    fn domain_lifecycle_state_is_active() {
        assert!(DomainLifecycleState::Active.is_active());
        assert!(DomainLifecycleState::Degraded.is_active());
        assert!(!DomainLifecycleState::Creating.is_active());
        assert!(!DomainLifecycleState::Deleted.is_active());
    }

    #[test]
    fn domain_lifecycle_state_can_accept_operations() {
        assert!(DomainLifecycleState::Active.can_accept_operations());
        assert!(DomainLifecycleState::Degraded.can_accept_operations());
        assert!(!DomainLifecycleState::Creating.can_accept_operations());
        assert!(!DomainLifecycleState::Maintenance.can_accept_operations());
    }

    #[test]
    fn domain_lifecycle_state_display_name() {
        assert_eq!(DomainLifecycleState::Active.display_name(), "active");
        assert_eq!(DomainLifecycleState::Deleted.display_name(), "deleted");
    }

    #[test]
    fn domain_lifecycle_state_serialization_roundtrip() {
        let state = DomainLifecycleState::Active;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: DomainLifecycleState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    // ========================================================================
    // DomainMetadata tests
    // ========================================================================

    #[test]
    fn domain_metadata_new() {
        let metadata = DomainMetadata::new(test_domain());
        assert_eq!(metadata.domain, test_domain());
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Creating);
        assert_eq!(metadata.node_count, 0);
    }

    #[test]
    fn domain_metadata_activate() {
        let mut metadata = DomainMetadata::new(test_domain());
        metadata.activate();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Active);
    }

    #[test]
    fn domain_metadata_degrade() {
        let mut metadata = DomainMetadata::new(test_domain());
        metadata.degrade();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Degraded);
    }

    #[test]
    fn domain_metadata_maintenance() {
        let mut metadata = DomainMetadata::new(test_domain());
        metadata.enter_maintenance();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Maintenance);
    }

    #[test]
    fn domain_metadata_deleting() {
        let mut metadata = DomainMetadata::new(test_domain());
        metadata.mark_deleting();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Deleting);
    }

    #[test]
    fn domain_metadata_deleted() {
        let mut metadata = DomainMetadata::new(test_domain());
        metadata.mark_deleted();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Deleted);
    }

    // ========================================================================
    // CoordinatorEngine advanced methods tests
    // ========================================================================

    #[test]
    fn coordinator_engine_leader_health() {
        let engine = CoordinatorEngine::new();
        let health = engine.leader_health(test_domain());
        assert!(!health.is_alive);
        assert_eq!(health.domain, test_domain());
    }

    #[test]
    fn coordinator_engine_lock_waiters() {
        let engine = CoordinatorEngine::new();
        let waiters = engine.lock_waiters(test_key_id());
        assert_eq!(waiters.len(), 0);
    }

    #[test]
    fn coordinator_engine_quorum_vote_history() {
        let engine = CoordinatorEngine::new();
        let votes = engine.quorum_vote_history(test_message_id());
        assert_eq!(votes.len(), 0);
    }

    #[test]
    fn coordinator_engine_domain_metadata() {
        let engine = CoordinatorEngine::new();
        assert!(engine.domain_metadata(test_domain()).is_none());
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        let metadata = engine.domain_metadata(test_domain()).unwrap();
        assert_eq!(metadata.domain, test_domain());
    }

    #[test]
    fn coordinator_engine_all_domain_metadata() {
        let engine = CoordinatorEngine::new();
        assert!(engine.all_domain_metadata().is_empty());
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.elect_leader(test_domain2(), &[test_node_id2()]).unwrap();
        let all = engine.all_domain_metadata();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn coordinator_engine_total_elections() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.total_elections(), 0);
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        assert_eq!(engine.total_elections(), 1);
    }

    #[test]
    fn coordinator_engine_total_locks_held() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.total_locks_held(), 0);
        engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        assert_eq!(engine.total_locks_held(), 1);
        engine.release_lock(test_key_id(), test_node_id()).unwrap();
        assert_eq!(engine.total_locks_held(), 0);
    }

    #[test]
    fn coordinator_engine_total_active_and_satisfied_barriers() {
        let engine = CoordinatorEngine::new();
        engine.create_barrier(test_key_id(), 2).unwrap();
        engine.create_barrier(test_key_id2(), 1).unwrap();
        assert_eq!(engine.total_active_barriers(), 2);
        engine.join_barrier(test_key_id2(), test_node_id()).unwrap();
        assert_eq!(engine.total_satisfied_barriers(), 1);
        assert_eq!(engine.total_active_barriers(), 1);
    }

    #[test]
    fn coordinator_engine_total_active_groups() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.total_active_groups(), 0);
        engine.join_group(test_key_id(), test_node_id()).unwrap();
        assert_eq!(engine.total_active_groups(), 1);
    }

    #[test]
    fn coordinator_engine_total_active_sessions() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.total_active_sessions(), 0);
        engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        assert_eq!(engine.total_active_sessions(), 1);
    }

    #[test]
    fn coordinator_engine_total_expired_sessions() {
        let engine = CoordinatorEngine::new();
        let now = IdsTime::now().0;
        assert_eq!(engine.total_expired_sessions(now), 0);
    }

    #[test]
    fn coordinator_engine_expire_sessions() {
        let engine = CoordinatorEngine::new();
        let handle = engine.create_session(test_node_id(), test_domain(), 1).unwrap();
        let now = IdsTime::new(handle.expires_at.0 + 1000);
        let expired = engine.expire_sessions(now.0);
        assert_eq!(expired, 1);
    }

    #[test]
    fn coordinator_engine_cleanup_expired_locks() {
        let engine = CoordinatorEngine::new();
        engine.acquire_lock(test_key_id(), test_node_id(), 1).unwrap();
        let now = IdsTime::new(IdsTime::now().0 + 2000);
        let cleaned = engine.cleanup_expired_locks(now.0);
        assert_eq!(cleaned, 1);
        assert!(!engine.is_locked(test_key_id()));
    }

    #[test]
    fn coordinator_engine_cleanup_expired_barriers() {
        let engine = CoordinatorEngine::new();
        let now = IdsTime::now().0;
        // Create a barrier with very short timeout
        engine.create_barrier(test_key_id(), 3).unwrap();
        let future_now = IdsTime::new(now + 200000);
        let cleaned = engine.cleanup_expired_barriers(future_now.0);
        assert_eq!(cleaned, 1);
    }

    #[test]
    fn coordinator_engine_fencing_token_count() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.fencing_token_count(test_domain()), 0);
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.issue_fencing_token(test_domain(), test_node_id()).unwrap();
        assert_eq!(engine.fencing_token_count(test_domain()), 1);
    }

    #[test]
    fn coordinator_engine_revoke_fencing_tokens_for_node() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.issue_fencing_token(test_domain(), test_node_id()).unwrap();
        engine.issue_fencing_token(test_domain2(), test_node_id()).unwrap();
        let revoked = engine.revoke_fencing_tokens_for_node(test_node_id());
        assert_eq!(revoked, 2);
    }

    #[test]
    fn coordinator_engine_leader_change_count() {
        let engine = CoordinatorEngine::new();
        assert_eq!(engine.leader_change_count(test_domain()), 0);
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        assert_eq!(engine.leader_change_count(test_domain()), 1);
    }

    #[test]
    fn coordinator_engine_has_quorum() {
        let engine = CoordinatorEngine::new();
        assert!(!engine.has_quorum(test_domain()));
    }

    #[test]
    fn coordinator_engine_election_state() {
        let engine = CoordinatorEngine::new();
        assert!(engine.election_state(test_domain()).is_none());
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        let state = engine.election_state(test_domain()).unwrap();
        assert!(state.has_leader);
        assert_eq!(state.leader, Some(test_node_id()));
        assert_eq!(state.candidate_count, 1);
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn coordinator_full_lifecycle() {
        let engine = CoordinatorEngine::new();

        // Elect leader
        let result = engine.elect_leader(test_domain(), &[test_node_id(), test_node_id2()]).unwrap();
        assert_eq!(result.leader, test_node_id());

        // Acquire lock
        engine.acquire_lock(test_key_id(), test_node_id(), 60000).unwrap();
        assert!(engine.is_locked(test_key_id()));

        // Create barrier
        engine.create_barrier(test_key_id2(), 2).unwrap();
        engine.join_barrier(test_key_id2(), test_node_id()).unwrap();
        engine.join_barrier(test_key_id2(), test_node_id2()).unwrap();
        assert!(engine.is_barrier_satisfied(test_key_id2()));

        // Join group
        engine.join_group(test_key_id(), test_node_id()).unwrap();
        engine.join_group(test_key_id(), test_node_id2()).unwrap();
        assert_eq!(engine.group_members(test_key_id()).len(), 2);

        // Record heartbeats
        engine.record_heartbeat(test_node_id(), test_domain()).unwrap();
        engine.record_heartbeat(test_node_id2(), test_domain()).unwrap();
        assert!(engine.is_node_alive(test_node_id(), 30000));
        assert_eq!(engine.alive_nodes(30000).len(), 2);

        // Create session
        let session = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        assert!(engine.total_active_sessions() > 0);

        // Quorum decision
        let votes = vec![(test_node_id(), true), (test_node_id2(), true)];
        let decision = engine.quorum_decision(test_domain(), test_message_id(), &votes).unwrap();
        assert!(decision.approved);

        // Issue fencing token
        let token = engine.issue_fencing_token(test_domain(), test_node_id()).unwrap();
        assert!(engine.validate_fencing_token(&token).unwrap());

        // Verify stats
        let stats = engine.stats(test_domain());
        assert!(stats.elections_held > 0);
        assert!(stats.locks_acquired > 0);
        assert!(stats.barriers_created > 0);
        assert!(stats.group_joins > 0);
        assert!(stats.heartbeats_received > 0);
        assert!(stats.sessions_created > 0);
        assert!(stats.quorum_decisions > 0);
        assert!(stats.fencing_tokens_issued > 0);
    }

    #[test]
    fn coordinator_multiple_domains() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.elect_leader(test_domain2(), &[test_node_id2()]).unwrap();
        assert_eq!(engine.domains().len(), 2);
        assert_eq!(engine.leader(test_domain()), Some(test_node_id()));
        assert_eq!(engine.leader(test_domain2()), Some(test_node_id2()));
    }

    #[test]
    fn coordinator_lock_acquire_release_cycle() {
        let engine = CoordinatorEngine::new();
        for i in 0..10 {
            let key = KeyId::new([i as u8; 16]);
            engine.acquire_lock(key, test_node_id(), 60000).unwrap();
        }
        assert_eq!(engine.total_locks_held(), 10);
        for i in 0..10 {
            let key = KeyId::new([i as u8; 16]);
            engine.release_lock(key, test_node_id()).unwrap();
        }
        assert_eq!(engine.total_locks_held(), 0);
    }

    #[test]
    fn coordinator_session_lifecycle() {
        let engine = CoordinatorEngine::new();
        let session = engine.create_session(test_node_id(), test_domain(), 60000).unwrap();
        assert_eq!(engine.total_active_sessions(), 1);
        let renewed = engine.renew_session(session.session_id, test_node_id(), 30000).unwrap();
        assert_eq!(engine.total_active_sessions(), 1);
        engine.close_session(session.session_id, test_node_id()).unwrap();
        assert_eq!(engine.total_active_sessions(), 0);
    }

    #[test]
    fn coordinator_leader_health_after_election() {
        let engine = CoordinatorEngine::new();
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        engine.record_heartbeat(test_node_id(), test_domain()).unwrap();
        let health = engine.leader_health(test_domain());
        assert!(health.is_alive);
        assert_eq!(health.leader, test_node_id());
    }

    #[test]
    fn coordinator_domain_metadata_lifecycle() {
        let engine = CoordinatorEngine::new();
        assert!(engine.domain_metadata(test_domain()).is_none());
        engine.elect_leader(test_domain(), &[test_node_id()]).unwrap();
        let metadata = engine.domain_metadata(test_domain()).unwrap();
        assert_eq!(metadata.lifecycle_state, DomainLifecycleState::Active);
    }

    #[test]
    fn coordinator_serialization_roundtrip_lock_acquisition() {
        let now = IdsTime::now().0;
        let acquisition = LockAcquisition {
            lock_id: test_key_id(),
            holder: test_node_id(),
            acquired_at: IdsTime::new(now),
            ttl_ms: 60000,
            expires_at: IdsTime::new(now + 60000),
            fencing_token: Epoch::new(1),
        };
        let json = serde_json::to_string(&acquisition).unwrap();
        let deserialized: LockAcquisition = serde_json::from_str(&json).unwrap();
        assert_eq!(acquisition.holder, deserialized.holder);
        assert_eq!(acquisition.ttl_ms, deserialized.ttl_ms);
    }

    #[test]
    fn coordinator_serialization_roundtrip_barrier_handle() {
        let now = IdsTime::now().0;
        let handle = BarrierHandle {
            barrier_id: test_key_id(),
            required_participants: 3,
            created_at: IdsTime::new(now),
            timeout_ms: 120000,
        };
        let json = serde_json::to_string(&handle).unwrap();
        let deserialized: BarrierHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(handle.required_participants, deserialized.required_participants);
        assert_eq!(handle.timeout_ms, deserialized.timeout_ms);
    }

    #[test]
    fn coordinator_serialization_roundtrip_heartbeat_record() {
        let record = HeartbeatRecord {
            node: test_node_id(),
            domain: test_domain(),
            received_at: IdsTime::now(),
            sequence: 1,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: HeartbeatRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.sequence, deserialized.sequence);
        assert_eq!(record.domain, deserialized.domain);
    }

    #[test]
    fn coordinator_serialization_roundtrip_session_handle() {
        let now = IdsTime::now().0;
        let handle = SessionHandle {
            session_id: test_session_id(),
            node: test_node_id(),
            domain: test_domain(),
            created_at: IdsTime::new(now),
            ttl_ms: 60000,
            expires_at: IdsTime::new(now + 60000),
        };
        let json = serde_json::to_string(&handle).unwrap();
        let deserialized: SessionHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(handle.session_id, deserialized.session_id);
        assert_eq!(handle.ttl_ms, deserialized.ttl_ms);
    }

    #[test]
    fn coordinator_quorum_decision_all_scenarios() {
        let engine = CoordinatorEngine::new();

        // All approve
        let votes = vec![(test_node_id(), true), (test_node_id2(), true), (test_node_id3(), true)];
        let decision = engine.quorum_decision(test_domain(), test_message_id(), &votes).unwrap();
        assert!(decision.approved);
        assert!((decision.approval_rate() - 1.0).abs() < 0.01);

        // All reject
        let votes = vec![(test_node_id(), false), (test_node_id2(), false), (test_node_id3(), false)];
        let decision = engine.quorum_decision(test_domain(), MessageId::new([2u8; 16]), &votes).unwrap();
        assert!(!decision.approved);
        assert!((decision.approval_rate() - 0.0).abs() < 0.01);

        // Tie
        let votes = vec![(test_node_id(), true), (test_node_id2(), false)];
        let decision = engine.quorum_decision(test_domain(), MessageId::new([3u8; 16]), &votes).unwrap();
        assert!(!decision.approved); // tie doesn't approve
        assert!((decision.approval_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn coordinator_error_display() {
        let err = CoordinatorError::DomainNotFound { domain: test_domain() };
        assert!(!err.to_string().is_empty());
        let err = CoordinatorError::LockAlreadyHeld { holder: test_node_id() };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn coordinator_clone_types() {
        let result = CoordinatorResult::new(test_domain(), test_node_id(), Epoch::new(1), Epoch::new(2), IdsTime::now());
        let cloned = result.clone();
        assert_eq!(result, cloned);

        let acquisition = LockAcquisition {
            lock_id: test_key_id(),
            holder: test_node_id(),
            acquired_at: IdsTime::now(),
            ttl_ms: 60000,
            expires_at: IdsTime::new(IdsTime::now().0 + 60000),
            fencing_token: Epoch::new(1),
        };
        let cloned = acquisition.clone();
        assert_eq!(acquisition, cloned);

        let lease = LeaderLease::new(test_domain(), test_node_id(), Epoch::new(1), 60000, 3);
        let cloned = lease.clone();
        assert_eq!(lease, cloned);
    }
}
