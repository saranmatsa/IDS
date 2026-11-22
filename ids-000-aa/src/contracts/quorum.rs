//! # Sealed Trait: Quorum — quorum-based distributed decision making
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Quorum trait defines quorum-based consensus primitives:
//! - Quorum configuration (threshold, total members, weight)
//! - Proposal creation and lifecycle (proposed, voting, committed, aborted)
//! - Vote casting and tracking (yes, no, abstain)
//! - Quorum threshold checking (simple majority, supermajority, unanimous, weighted)
//! - Quorum certificate generation and verification
//! - View change and leader rotation
//! - Conflict detection and resolution
//! - Quorum stats and health monitoring
//! Implementations live downstream in ids-quorum and ids-* crates.

use crate::types::{
    DatasetId, Endpoint, FailureDomain, NodeId, RegionId,
};
use crate::errors::QuorumError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Quorum
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::QuorumEngine {}
impl private::Sealed for super::QuorumConfig {}
impl private::Sealed for super::Proposal {}
impl private::Sealed for super::ProposalState {}
impl private::Sealed for super::Vote {}
impl private::Sealed for super::VoteType {}
impl private::Sealed for super::VoteRecord {}
impl private::Sealed for super::QuorumThreshold {}
impl private::Sealed for super::QuorumCertificate {}
impl private::Sealed for super::ViewChange {}
impl private::Sealed for super::ConflictResolver {}
impl private::Sealed for super::QuorumStats {}
impl private::Sealed for super::QuorumMonitor {}
impl private::Sealed for super::QuorumValidator {}

// Sealed trait for quorum.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Quorum: private::Sealed {
    /// Configure a quorum group.
    fn configure_quorum(&self, config: QuorumConfig) -> Result<(), QuorumError>;

    /// Create a new proposal.
    fn create_proposal(
        &self,
        proposal_id: u64,
        proposer: NodeId,
        payload: Vec<u8>,
    ) -> Result<Proposal, QuorumError>;

    /// Cast a vote on a proposal.
    fn cast_vote(&self, proposal_id: u64, voter: NodeId, vote: Vote) -> Result<(), QuorumError>;

    /// Check if quorum is reached for a proposal.
    fn is_quorum_reached(&self, proposal_id: u64) -> Result<bool, QuorumError>;

    /// Commit a proposal if quorum reached.
    fn commit_proposal(&self, proposal_id: u64) -> Result<(), QuorumError>;

    /// Abort a proposal.
    fn abort_proposal(&self, proposal_id: u64) -> Result<(), QuorumError>;

    /// Get proposal state.
    fn proposal_state(&self, proposal_id: u64) -> Result<ProposalState, QuorumError>;

    /// Get vote tally for a proposal.
    fn vote_tally(&self, proposal_id: u64) -> Result<(u64, u64, u64), QuorumError>;

    /// Generate a quorum certificate.
    fn generate_certificate(&self, proposal_id: u64) -> Result<QuorumCertificate, QuorumError>;

    /// Verify a quorum certificate.
    fn verify_certificate(&self, cert: &QuorumCertificate) -> Result<bool, QuorumError>;

    /// Initiate a view change.
    fn view_change(&self, new_view: u64, leader: NodeId) -> Result<(), QuorumError>;

    /// Get current view number.
    fn current_view(&self) -> Result<u64, QuorumError>;

    /// Get current leader.
    fn current_leader(&self) -> Result<NodeId, QuorumError>;

    /// Resolve a conflict between proposals.
    fn resolve_conflict(
        &self,
        proposal_id_a: u64,
        proposal_id_b: u64,
    ) -> Result<ResolutionResult, QuorumError>;

    /// Get quorum stats.
    fn stats(&self) -> QuorumStats;

    /// Get quorum name for debugging.
    fn quorum_name(&self) -> &'static str;

    /// Get total proposals created.
    fn total_proposals(&self) -> u64;

    /// Get total proposals committed.
    fn total_committed(&self) -> u64;

    /// Get total proposals aborted.
    fn total_aborted(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum QuorumError {
    #[error("quorum not configured: {group_id:?}")]
    QuorumNotConfigured { group_id: u64 },

    #[error("proposal not found: {proposal_id}")]
    ProposalNotFound { proposal_id: u64 },

    #[error("proposal already exists: {proposal_id}")]
    ProposalAlreadyExists { proposal_id: u64 },

    #[error("proposal already decided: {proposal_id}, state={state:?}")]
    ProposalAlreadyDecided { proposal_id: u64, state: ProposalState },

    #[error("vote already cast: {voter:?} on {proposal_id}")]
    VoteAlreadyCast { voter: NodeId, proposal_id: u64 },

    #[error("voter not in quorum: {voter:?}")]
    VoterNotInQuorum { voter: NodeId },

    #[error("insufficient votes: have={have}, needed={needed}")]
    InsufficientVotes { have: u64, needed: u64 },

    #[error("quorum threshold not met: {threshold:?}")]
    ThresholdNotMet { threshold: QuorumThreshold },

    #[error("invalid view: current={current}, requested={requested}")]
    InvalidView { current: u64, requested: u64 },

    #[error("leader not authorized: {leader:?}")]
    LeaderNotAuthorized { leader: NodeId },

    #[error("conflict unresolved: {proposal_id_a} vs {proposal_id_b}")]
    ConflictUnresolved { proposal_id_a: u64, proposal_id_b: u64 },

    #[error("certificate invalid: {reason}")]
    CertificateInvalid { reason: String },

    #[error("internal quorum error: {message}")]
    Internal { message: String },
}

impl QuorumError {
    pub fn category(&self) -> &'static str {
        match self {
            QuorumError::QuorumNotConfigured { .. } => "quorum_not_configured",
            QuorumError::ProposalNotFound { .. } => "proposal_not_found",
            QuorumError::ProposalAlreadyExists { .. } => "proposal_already_exists",
            QuorumError::ProposalAlreadyDecided { .. } => "proposal_already_decided",
            QuorumError::VoteAlreadyCast { .. } => "vote_already_cast",
            QuorumError::VoterNotInQuorum { .. } => "voter_not_in_quorum",
            QuorumError::InsufficientVotes { .. } => "insufficient_votes",
            QuorumError::ThresholdNotMet { .. } => "threshold_not_met",
            QuorumError::InvalidView { .. } => "invalid_view",
            QuorumError::LeaderNotAuthorized { .. } => "leader_not_authorized",
            QuorumError::ConflictUnresolved { .. } => "conflict_unresolved",
            QuorumError::CertificateInvalid { .. } => "certificate_invalid",
            QuorumError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            QuorumError::InsufficientVotes { .. }
                | QuorumError::ThresholdNotMet { .. }
                | QuorumError::InvalidView { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            QuorumError::QuorumNotConfigured { .. } => "configure quorum first",
            QuorumError::ProposalNotFound { .. } => "create the proposal first",
            QuorumError::ProposalAlreadyExists { .. } => "use existing proposal",
            QuorumError::ProposalAlreadyDecided { .. } => "proposal is already decided",
            QuorumError::VoteAlreadyCast { .. } => "vote already recorded",
            QuorumError::VoterNotInQuorum { .. } => "add voter to quorum group",
            QuorumError::InsufficientVotes { .. } => "wait for more votes",
            QuorumError::ThresholdNotMet { .. } => "adjust threshold or gather more votes",
            QuorumError::InvalidView { .. } => "use valid view number",
            QuorumError::LeaderNotAuthorized { .. } => "authorize the leader",
            QuorumError::ConflictUnresolved { .. } => "resolve conflict manually",
            QuorumError::CertificateInvalid { .. } => "regenerate certificate",
            QuorumError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROPOSAL STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    Proposed,
    Voting,
    Committed,
    Aborted,
    Expired,
}

impl ProposalState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalState::Proposed => "proposed",
            ProposalState::Voting => "voting",
            ProposalState::Committed => "committed",
            ProposalState::Aborted => "aborted",
            ProposalState::Expired => "expired",
        }
    }

    pub fn is_decided(&self) -> bool {
        matches!(
            self,
            ProposalState::Committed | ProposalState::Aborted | ProposalState::Expired
        )
    }

    pub fn can_transition_to(&self, next: ProposalState) -> bool {
        match self {
            ProposalState::Proposed => matches!(
                next,
                ProposalState::Voting | ProposalState::Aborted | ProposalState::Expired
            ),
            ProposalState::Voting => matches!(
                next,
                ProposalState::Committed | ProposalState::Aborted | ProposalState::Expired
            ),
            ProposalState::Committed => false,
            ProposalState::Aborted => false,
            ProposalState::Expired => false,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// VOTE TYPE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteType {
    Yes,
    No,
    Abstain,
}

impl VoteType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VoteType::Yes => "yes",
            VoteType::No => "no",
            VoteType::Abstain => "abstain",
        }
    }

    pub fn is_positive(&self) -> bool {
        matches!(self, VoteType::Yes)
    }

    pub fn is_negative(&self) -> bool {
        matches!(self, VoteType::No)
    }
}

///////////////////////////////////////////////////////////////////////////////
// VOTE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct Vote {
    pub voter: NodeId,
    pub vote_type: VoteType,
    pub weight: u64,
    pub timestamp_ms: u64,
    pub signature: Option<Vec<u8>>,
}

impl Vote {
    pub fn new(voter: NodeId, vote_type: VoteType, weight: u64, timestamp_ms: u64) -> Self {
        Self {
            voter,
            vote_type,
            weight,
            timestamp_ms,
            signature: None,
        }
    }

    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }

    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    pub fn weight(&self) -> u64 {
        self.weight
    }
}

///////////////////////////////////////////////////////////////////////////////
// VOTE RECORD
///////////////////////////////////////////////////////////////////////////////

pub struct VoteRecord {
    pub proposal_id: u64,
    pub voter: NodeId,
    pub vote: Vote,
    pub received_at_ms: u64,
}

impl VoteRecord {
    pub fn new(
        proposal_id: u64,
        voter: NodeId,
        vote: Vote,
        received_at_ms: u64,
    ) -> Self {
        Self {
            proposal_id,
            voter,
            vote,
            received_at_ms,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM THRESHOLD
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuorumThreshold {
    SimpleMajority,
    SuperMajority2_3,
    SuperMajority3_4,
    Unanimous,
    Weighted(f64),
    Custom(u64, u64), // needed, total
}

impl QuorumThreshold {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuorumThreshold::SimpleMajority => "simple_majority",
            QuorumThreshold::SuperMajority2_3 => "super_majority_2_3",
            QuorumThreshold::SuperMajority3_4 => "super_majority_3_4",
            QuorumThreshold::Unanimous => "unanimous",
            QuorumThreshold::Weighted(_) => "weighted",
            QuorumThreshold::Custom { .. } => "custom",
        }
    }

    pub fn is_met(&self, yes_votes: u64, total_votes: u64, total_weight: u64) -> bool {
        match self {
            QuorumThreshold::SimpleMajority => yes_votes > total_votes / 2,
            QuorumThreshold::SuperMajority2_3 => yes_votes * 3 >= total_votes * 2,
            QuorumThreshold::SuperMajority3_4 => yes_votes * 4 >= total_votes * 3,
            QuorumThreshold::Unanimous => yes_votes == total_votes && total_votes > 0,
            QuorumThreshold::Weighted(ratio) => {
                if total_weight == 0 {
                    return false;
                }
                (yes_votes as f64 / total_weight as f64) >= *ratio
            }
            QuorumThreshold::Custom(needed, total) => yes_votes >= *needed && total_votes >= *total,
        }
    }

    pub fn needed_votes(&self, total_voters: u64) -> u64 {
        match self {
            QuorumThreshold::SimpleMajority => total_voters / 2 + 1,
            QuorumThreshold::SuperMajority2_3 => (total_voters * 2 + 2) / 3,
            QuorumThreshold::SuperMajority3_4 => (total_voters * 3 + 3) / 4,
            QuorumThreshold::Unanimous => total_voters,
            QuorumThreshold::Weighted(ratio) => {
                (total_voters as f64 * ratio).ceil() as u64
            }
            QuorumThreshold::Custom(needed, _) => *needed,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM CONFIG
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct QuorumConfig {
    pub group_id: u64,
    pub name: String,
    pub members: Vec<NodeId>,
    pub threshold: QuorumThreshold,
    pub total_weight: u64,
    pub view_timeout_ms: u64,
    pub proposal_timeout_ms: u64,
    pub max_proposals: u64,
    pub leader_id: Option<NodeId>,
}

impl QuorumConfig {
    pub fn new(
        group_id: u64,
        name: String,
        members: Vec<NodeId>,
        threshold: QuorumThreshold,
    ) -> Self {
        let total_weight = members.len() as u64;
        Self {
            group_id,
            name,
            members,
            threshold,
            total_weight,
            view_timeout_ms: 30_000,
            proposal_timeout_ms: 60_000,
            max_proposals: 10_000,
            leader_id: None,
        }
    }

    pub fn with_timeouts(mut self, view_timeout_ms: u64, proposal_timeout_ms: u64) -> Self {
        self.view_timeout_ms = view_timeout_ms;
        self.proposal_timeout_ms = proposal_timeout_ms;
        self
    }

    pub fn with_max_proposals(mut self, max: u64) -> Self {
        self.max_proposals = max;
        self
    }

    pub fn with_leader(mut self, leader: NodeId) -> Self {
        self.leader_id = Some(leader);
        self
    }

    pub fn member_count(&self) -> u64 {
        self.members.len() as u64
    }

    pub fn is_member(&self, node: &NodeId) -> bool {
        self.members.iter().any(|m| m == node)
    }

    pub fn needed_votes(&self) -> u64 {
        self.threshold.needed_votes(self.member_count())
    }

    pub fn validate(&self) -> Result<(), QuorumError> {
        if self.members.is_empty() {
            return Err(QuorumError::Internal {
                message: "quorum must have at least one member".to_string(),
            });
        }
        if self.max_proposals == 0 {
            return Err(QuorumError::Internal {
                message: "max_proposals must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROPOSAL
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct Proposal {
    pub proposal_id: u64,
    pub proposer: NodeId,
    pub payload: Vec<u8>,
    pub state: ProposalState,
    pub created_at_ms: u64,
    pub decided_at_ms: Option<u64>,
    pub votes: std::cell::RefCell<Vec<VoteRecord>>,
    pub quorum_threshold: QuorumThreshold,
    pub total_weight: u64,
}

impl Proposal {
    pub fn new(
        proposal_id: u64,
        proposer: NodeId,
        payload: Vec<u8>,
        quorum_threshold: QuorumThreshold,
        total_weight: u64,
        created_at_ms: u64,
    ) -> Self {
        Self {
            proposal_id,
            proposer,
            payload,
            state: ProposalState::Proposed,
            created_at_ms,
            decided_at_ms: None,
            votes: std::cell::RefCell::new(Vec::new()),
            quorum_threshold,
            total_weight,
        }
    }

    pub fn add_vote(&self, record: VoteRecord) -> Result<(), QuorumError> {
        if self.state.is_decided() {
            return Err(QuorumError::ProposalAlreadyDecided {
                proposal_id: self.proposal_id,
                state: self.state,
            });
        }
        // Check duplicate voter
        if self.votes.borrow().iter().any(|v| v.voter == record.voter) {
            return Err(QuorumError::VoteAlreadyCast {
                voter: record.voter,
                proposal_id: self.proposal_id,
            });
        }
        self.votes.borrow_mut().push(record);
        Ok(())
    }

    pub fn vote_count(&self) -> u64 {
        self.votes.borrow().len() as u64
    }

    pub fn yes_votes(&self) -> u64 {
        self.votes
            .borrow()
            .iter()
            .filter(|v| v.vote.vote_type == VoteType::Yes)
            .map(|v| v.vote.weight)
            .sum()
    }

    pub fn no_votes(&self) -> u64 {
        self.votes
            .borrow()
            .iter()
            .filter(|v| v.vote.vote_type == VoteType::No)
            .map(|v| v.vote.weight)
            .sum()
    }

    pub fn abstain_votes(&self) -> u64 {
        self.votes
            .borrow()
            .iter()
            .filter(|v| v.vote.vote_type == VoteType::Abstain)
            .map(|v| v.vote.weight)
            .sum()
    }

    pub fn total_vote_weight(&self) -> u64 {
        self.votes.borrow().iter().map(|v| v.vote.weight).sum()
    }

    pub fn is_quorum_reached(&self) -> bool {
        let yes = self.yes_votes();
        let total = self.total_vote_weight();
        self.quorum_threshold.is_met(yes, self.vote_count(), total)
    }

    pub fn transition_to(&mut self, next: ProposalState) -> Result<(), QuorumError> {
        if !self.state.can_transition_to(next) {
            return Err(QuorumError::Internal {
                message: format!("invalid transition from {:?} to {:?}", self.state, next),
            });
        }
        if next.is_decided() {
            self.decided_at_ms = Some(0);
        }
        self.state = next;
        Ok(())
    }

    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM CERTIFICATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct QuorumCertificate {
    pub proposal_id: u64,
    pub view: u64,
    pub yes_votes: u64,
    pub total_votes: u64,
    pub threshold: QuorumThreshold,
    pub signatures: Vec<(NodeId, Vec<u8>)>,
    pub created_at_ms: u64,
    pub valid: bool,
}

impl QuorumCertificate {
    pub fn new(
        proposal_id: u64,
        view: u64,
        yes_votes: u64,
        total_votes: u64,
        threshold: QuorumThreshold,
        created_at_ms: u64,
    ) -> Self {
        Self {
            proposal_id,
            view,
            yes_votes,
            total_votes,
            threshold,
            signatures: Vec::new(),
            created_at_ms,
            valid: false,
        }
    }

    pub fn add_signature(&mut self, node: NodeId, signature: Vec<u8>) {
        self.signatures.push((node, signature));
    }

    pub fn mark_valid(&mut self) {
        self.valid = true;
    }

    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub fn is_valid(&self) -> bool {
        self.valid && self.threshold.is_met(self.yes_votes, self.total_votes, self.total_votes)
    }
}

///////////////////////////////////////////////////////////////////////////////
// VIEW CHANGE
///////////////////////////////////////////////////////////////////////////////

pub struct ViewChangeRecord {
    pub old_view: u64,
    pub new_view: u64,
    pub old_leader: Option<NodeId>,
    pub new_leader: NodeId,
    pub initiated_at_ms: u64,
    pub completed: bool,
}

impl ViewChangeRecord {
    pub fn new(
        old_view: u64,
        new_view: u64,
        old_leader: Option<NodeId>,
        new_leader: NodeId,
        initiated_at_ms: u64,
    ) -> Self {
        Self {
            old_view,
            new_view,
            old_leader,
            new_leader,
            initiated_at_ms,
            completed: false,
        }
    }

    pub fn complete(&mut self) {
        self.completed = true;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CONFLICT RESOLUTION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionResult {
    ProposalAWins,
    ProposalBWins,
    Merge,
    AbortBoth,
    ManualIntervention,
}

impl ResolutionResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionResult::ProposalAWins => "proposal_a_wins",
            ResolutionResult::ProposalBWins => "proposal_b_wins",
            ResolutionResult::Merge => "merge",
            ResolutionResult::AbortBoth => "abort_both",
            ResolutionResult::ManualIntervention => "manual_intervention",
        }
    }
}

pub struct ConflictResolver {
    pub conflicts: std::cell::RefCell<std::collections::HashMap<(u64, u64), ResolutionResult>>,
    pub total_resolved: u64,
    pub total_pending: u64,
}

impl ConflictResolver {
    pub fn new() -> Self {
        Self {
            conflicts: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_resolved: 0,
            total_pending: 0,
        }
    }

    pub fn resolve(
        &self,
        proposal_id_a: u64,
        proposal_id_b: u64,
        result: ResolutionResult,
    ) {
        self.conflicts
            .borrow_mut()
            .insert((proposal_id_a, proposal_id_b), result);
        self.total_resolved += 1;
    }

    pub fn get_resolution(
        &self,
        proposal_id_a: u64,
        proposal_id_b: u64,
    ) -> Option<ResolutionResult> {
        self.conflicts
            .borrow()
            .get(&(proposal_id_a, proposal_id_b))
            .copied()
    }

    pub fn total_resolved(&self) -> u64 {
        self.total_resolved
    }

    pub fn tracked_conflicts(&self) -> usize {
        self.conflicts.borrow().len()
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct QuorumStats {
    pub total_proposals: u64,
    pub total_committed: u64,
    pub total_aborted: u64,
    pub total_expired: u64,
    pub total_votes_cast: u64,
    pub total_quorums_reached: u64,
    pub total_view_changes: u64,
    pub total_conflicts_resolved: u64,
    pub total_certificates_generated: u64,
    pub total_certificates_verified: u64,
}

impl QuorumStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commit_rate(&self) -> f64 {
        if self.total_proposals == 0 {
            return 1.0;
        }
        self.total_committed as f64 / self.total_proposals as f64
    }

    pub fn abort_rate(&self) -> f64 {
        if self.total_proposals == 0 {
            return 0.0;
        }
        self.total_aborted as f64 / self.total_proposals as f64
    }

    pub fn quorum_rate(&self) -> f64 {
        if self.total_proposals == 0 {
            return 0.0;
        }
        self.total_quorums_reached as f64 / self.total_proposals as f64
    }

    pub fn system_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.total_aborted as f64 * 2.0;
        score -= self.total_expired as f64 * 1.0;
        score += self.commit_rate() * 20.0;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct QuorumMonitor {
    pub node_activity: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub quorum_incident_count: u64,
}

impl QuorumMonitor {
    pub fn new() -> Self {
        Self {
            node_activity: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            quorum_incident_count: 0,
        }
    }

    pub fn record_activity(&self, node: NodeId, actions: u64) {
        let mut activity = self.node_activity.borrow_mut();
        let prev = activity.get(&node).copied().unwrap_or(0);
        activity.insert(node, prev + actions);
        self.health_check_count += 1;
        if actions == 0 && prev > 0 {
            self.alert_count += 1;
            self.quorum_incident_count += 1;
        }
    }

    pub fn node_action_count(&self, node: &NodeId) -> u64 {
        self.node_activity.borrow().get(node).copied().unwrap_or(0)
    }

    pub fn report_incident(&self, node: NodeId) {
        self.quorum_incident_count += 1;
        self.alert_count += 1;
        self.node_activity.borrow_mut().insert(node, 0);
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn quorum_incident_count(&self) -> u64 {
        self.quorum_incident_count
    }

    pub fn monitored_node_count(&self) -> usize {
        self.node_activity.borrow().len()
    }

    pub fn inactive_node_count(&self) -> usize {
        self.node_activity.borrow().values().filter(|c| *c == 0).count()
    }
}

impl Default for QuorumMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuorumValidationResult {
    Valid,
    InvalidProposalID,
    InvalidVoter,
    InvalidThreshold,
    InvalidWeight,
    InvalidView,
}

impl QuorumValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuorumValidationResult::Valid => "valid",
            QuorumValidationResult::InvalidProposalID => "invalid_proposal_id",
            QuorumValidationResult::InvalidVoter => "invalid_voter",
            QuorumValidationResult::InvalidThreshold => "invalid_threshold",
            QuorumValidationResult::InvalidWeight => "invalid_weight",
            QuorumValidationResult::InvalidView => "invalid_view",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, QuorumValidationResult::Valid)
    }
}

pub struct QuorumValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<NodeId, QuorumValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl QuorumValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_proposal_id(&self, proposal_id: u64) -> QuorumValidationResult {
        self.validation_count += 1;
        let result = if proposal_id == 0 {
            QuorumValidationResult::InvalidProposalID
        } else {
            QuorumValidationResult::Valid
        };
        match result {
            QuorumValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([proposal_id as u8; 32]), result);
        result
    }

    pub fn validate_voter(&self, voter: NodeId) -> QuorumValidationResult {
        self.validation_count += 1;
        let result = if voter == NodeId::new([0u8; 32]) {
            QuorumValidationResult::InvalidVoter
        } else {
            QuorumValidationResult::Valid
        };
        match result {
            QuorumValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(voter, result);
        result
    }

    pub fn validate_threshold(
        &self,
        threshold: QuorumThreshold,
        total_voters: u64,
    ) -> QuorumValidationResult {
        self.validation_count += 1;
        let needed = threshold.needed_votes(total_voters);
        let result = if needed == 0 && total_voters > 0 {
            QuorumValidationResult::InvalidThreshold
        } else {
            QuorumValidationResult::Valid
        };
        match result {
            QuorumValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::new([needed as u8; 32]), result);
        result
    }

    pub fn validate_weight(&self, voter: NodeId, weight: u64) -> QuorumValidationResult {
        self.validation_count += 1;
        let result = if weight == 0 {
            QuorumValidationResult::InvalidWeight
        } else {
            QuorumValidationResult::Valid
        };
        match result {
            QuorumValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(voter, result);
        result
    }

    pub fn validate_view(&self, voter: NodeId, current: u64, requested: u64) -> QuorumValidationResult {
        self.validation_count += 1;
        let result = if requested < current {
            QuorumValidationResult::InvalidView
        } else {
            QuorumValidationResult::Valid
        };
        match result {
            QuorumValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(voter, result);
        result
    }

    pub fn get_result(&self, voter: &NodeId) -> Option<QuorumValidationResult> {
        self.validations.borrow().get(voter).copied()
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

impl Default for QuorumValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct QuorumEngine {
    pub configs: std::cell::RefCell<std::collections::HashMap<u64, QuorumConfig>>,
    pub proposals: std::cell::RefCell<std::collections::HashMap<u64, Proposal>>,
    pub vote_records: std::cell::RefCell<std::collections::HashMap<u64, Vec<VoteRecord>>>,
    pub certificates: std::cell::RefCell<std::collections::HashMap<u64, QuorumCertificate>>,
    pub view_changes: std::cell::RefCell<Vec<ViewChangeRecord>>,
    pub conflict_resolver: std::cell::RefCell<ConflictResolver>,
    pub current_view: std::cell::RefCell<u64>,
    pub current_leader: std::cell::RefCell<Option<NodeId>>,
    pub stats: std::cell::RefCell<QuorumStats>,
    pub monitor: std::cell::RefCell<QuorumMonitor>,
    pub validator: std::cell::RefCell<QuorumValidator>,
}

impl QuorumEngine {
    pub fn new() -> Self {
        Self {
            configs: std::cell::RefCell::new(std::collections::HashMap::new()),
            proposals: std::cell::RefCell::new(std::collections::HashMap::new()),
            vote_records: std::cell::RefCell::new(std::collections::HashMap::new()),
            certificates: std::cell::RefCell::new(std::collections::HashMap::new()),
            view_changes: std::cell::RefCell::new(Vec::new()),
            conflict_resolver: std::cell::RefCell::new(ConflictResolver::default()),
            current_view: std::cell::RefCell::new(0),
            current_leader: std::cell::RefCell::new(None),
            stats: std::cell::RefCell::new(QuorumStats::default()),
            monitor: std::cell::RefCell::new(QuorumMonitor::new()),
            validator: std::cell::RefCell::new(QuorumValidator::new()),
        }
    }

    pub fn configure_quorum(&self, config: QuorumConfig) -> Result<(), QuorumError> {
        config.validate()?;
        let mut configs = self.configs.borrow_mut();
        if configs.contains_key(&config.group_id) {
            return Err(QuorumError::Internal {
                message: "quorum already configured".to_string(),
            });
        }
        configs.insert(config.group_id, config);
        Ok(())
    }

    pub fn create_proposal(
        &self,
        proposal_id: u64,
        proposer: NodeId,
        payload: Vec<u8>,
    ) -> Result<Proposal, QuorumError> {
        let configs = self.configs.borrow();
        let config = configs.values().next().ok_or_else(|| QuorumError::QuorumNotConfigured { group_id: 0 })?;
        drop(configs);

        if !config.is_member(&proposer) {
            return Err(QuorumError::VoterNotInQuorum { voter: proposer });
        }

        let mut proposals = self.proposals.borrow_mut();
        if proposals.contains_key(&proposal_id) {
            return Err(QuorumError::ProposalAlreadyExists { proposal_id });
        }
        if proposals.len() as u64 >= config.max_proposals {
            return Err(QuorumError::Internal {
                message: "max proposals reached".to_string(),
            });
        }

        let proposal = Proposal::new(
            proposal_id,
            proposer,
            payload,
            config.threshold,
            config.total_weight,
            0,
        );
        proposals.insert(proposal_id, proposal.clone());
        self.stats.borrow_mut().total_proposals += 1;
        self.monitor.borrow_mut().record_activity(proposer, 1);
        Ok(proposal)
    }

    pub fn cast_vote(
        &self,
        proposal_id: u64,
        voter: NodeId,
        vote: Vote,
    ) -> Result<(), QuorumError> {
        let configs = self.configs.borrow();
        let config = configs.values().next().ok_or_else(|| QuorumError::QuorumNotConfigured { group_id: 0 })?;
        drop(configs);

        if !config.is_member(&voter) {
            return Err(QuorumError::VoterNotInQuorum { voter });
        }

        let mut proposals = self.proposals.borrow_mut();
        let proposal = proposals.get_mut(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;

        if proposal.state.is_decided() {
            return Err(QuorumError::ProposalAlreadyDecided {
                proposal_id,
                state: proposal.state,
            });
        }

        let record = VoteRecord::new(proposal_id, voter, vote.clone(), 0);
        proposal.add_vote(record)?;

        let mut vote_records = self.vote_records.borrow_mut();
        vote_records.entry(proposal_id).or_default().push(VoteRecord::new(proposal_id, voter, vote, 0));
        self.stats.borrow_mut().total_votes_cast += 1;
        self.monitor.borrow_mut().record_activity(voter, 1);
        Ok(())
    }

    pub fn is_quorum_reached(&self, proposal_id: u64) -> Result<bool, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        Ok(proposal.is_quorum_reached())
    }

    pub fn commit_proposal(&self, proposal_id: u64) -> Result<(), QuorumError> {
        let reached = self.is_quorum_reached(proposal_id)?;
        if !reached {
            return Err(QuorumError::InsufficientVotes {
                have: 0,
                needed: 1,
            });
        }

        let mut proposals = self.proposals.borrow_mut();
        let proposal = proposals.get_mut(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        proposal.transition_to(ProposalState::Committed)?;
        self.stats.borrow_mut().total_committed += 1;
        Ok(())
    }

    pub fn abort_proposal(&self, proposal_id: u64) -> Result<(), QuorumError> {
        let mut proposals = self.proposals.borrow_mut();
        let proposal = proposals.get_mut(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        proposal.transition_to(ProposalState::Aborted)?;
        self.stats.borrow_mut().total_aborted += 1;
        Ok(())
    }

    pub fn proposal_state(&self, proposal_id: u64) -> Result<ProposalState, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        Ok(proposal.state)
    }

    pub fn vote_tally(&self, proposal_id: u64) -> Result<(u64, u64, u64), QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        Ok((proposal.yes_votes(), proposal.no_votes(), proposal.abstain_votes()))
    }

    pub fn generate_certificate(&self, proposal_id: u64) -> Result<QuorumCertificate, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        if proposal.state != ProposalState::Committed {
            return Err(QuorumError::CertificateInvalid {
                reason: "proposal not committed".to_string(),
            });
        }
        let view = *self.current_view.borrow();
        let mut cert = QuorumCertificate::new(
            proposal_id,
            view,
            proposal.yes_votes(),
            proposal.vote_count(),
            proposal.quorum_threshold,
            0,
        );
        // Add signatures from voters
        for record in proposal.votes.borrow().iter() {
            cert.add_signature(record.voter, vec![1u8; 32]);
        }
        cert.mark_valid();
        self.certificates.borrow_mut().insert(proposal_id, cert.clone());
        self.stats.borrow_mut().total_certificates_generated += 1;
        Ok(cert)
    }

    pub fn verify_certificate(&self, cert: &QuorumCertificate) -> Result<bool, QuorumError> {
        if !cert.is_valid() {
            self.stats.borrow_mut().total_certificates_verified += 1;
            return Ok(false);
        }
        let proposals = self.proposals.borrow();
        let _proposal = proposals.get(&cert.proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id: cert.proposal_id })?;
        self.stats.borrow_mut().total_certificates_verified += 1;
        Ok(true)
    }

    pub fn view_change(&self, new_view: u64, leader: NodeId) -> Result<(), QuorumError> {
        let current = *self.current_view.borrow();
        if new_view < current {
            return Err(QuorumError::InvalidView { current, requested: new_view });
        }
        let configs = self.configs.borrow();
        let config = configs.values().next().ok_or_else(|| QuorumError::QuorumNotConfigured { group_id: 0 })?;
        drop(configs);

        if !config.is_member(&leader) {
            return Err(QuorumError::LeaderNotAuthorized { leader });
        }

        let old_leader = self.current_leader.borrow().clone();
        let record = ViewChangeRecord::new(current, new_view, old_leader, leader, 0);
        self.view_changes.borrow_mut().push(record);
        *self.current_view.borrow_mut() = new_view;
        *self.current_leader.borrow_mut() = Some(leader);
        self.stats.borrow_mut().total_view_changes += 1;
        Ok(())
    }

    pub fn current_view(&self) -> Result<u64, QuorumError> {
        Ok(*self.current_view.borrow())
    }

    pub fn current_leader(&self) -> Result<NodeId, QuorumError> {
        self.current_leader.borrow().clone().ok_or_else(|| QuorumError::LeaderNotAuthorized { leader: NodeId::new([0u8; 32]) })
    }

    pub fn resolve_conflict(
        &self,
        proposal_id_a: u64,
        proposal_id_b: u64,
    ) -> Result<ResolutionResult, QuorumError> {
        // Simple resolution: higher proposal ID wins
        let result = if proposal_id_a > proposal_id_b {
            ResolutionResult::ProposalAWins
        } else if proposal_id_b > proposal_id_a {
            ResolutionResult::ProposalBWins
        } else {
            ResolutionResult::AbortBoth
        };
        self.conflict_resolver
            .borrow_mut()
            .resolve(proposal_id_a, proposal_id_b, result);
        self.stats.borrow_mut().total_conflicts_resolved += 1;
        Ok(result)
    }

    pub fn stats(&self) -> QuorumStats {
        self.stats.borrow().clone()
    }

    pub fn quorum_name(&self) -> &'static str {
        "quorum_engine"
    }

    pub fn total_proposals(&self) -> u64 {
        self.stats.borrow().total_proposals
    }

    pub fn total_committed(&self) -> u64 {
        self.stats.borrow().total_committed
    }

    pub fn total_aborted(&self) -> u64 {
        self.stats.borrow().total_aborted
    }

    pub fn get_config(&self, group_id: u64) -> Result<QuorumConfig, QuorumError> {
        self.configs
            .borrow()
            .get(&group_id)
            .cloned()
            .ok_or_else(|| QuorumError::QuorumNotConfigured { group_id })
    }

    pub fn get_proposal(&self, proposal_id: u64) -> Result<Proposal, QuorumError> {
        self.proposals
            .borrow()
            .get(&proposal_id)
            .cloned()
            .ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })
    }

    pub fn monitor_node(&self, node: NodeId, actions: u64) {
        self.monitor.borrow_mut().record_activity(node, actions);
    }

    pub fn is_node_active(&self, node: &NodeId) -> bool {
        self.monitor.borrow().node_action_count(node) > 0
    }

    pub fn validate_proposal_id(&self, proposal_id: u64) -> QuorumValidationResult {
        self.validator.borrow_mut().validate_proposal_id(proposal_id)
    }

    pub fn validate_voter(&self, voter: NodeId) -> QuorumValidationResult {
        self.validator.borrow_mut().validate_voter(voter)
    }

    pub fn validate_threshold(&self, threshold: QuorumThreshold, total_voters: u64) -> QuorumValidationResult {
        self.validator.borrow_mut().validate_threshold(threshold, total_voters)
    }

    pub fn validate_weight(&self, voter: NodeId, weight: u64) -> QuorumValidationResult {
        self.validator.borrow_mut().validate_weight(voter, weight)
    }

    pub fn validate_view(&self, voter: NodeId, current: u64, requested: u64) -> QuorumValidationResult {
        self.validator.borrow_mut().validate_view(voter, current, requested)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    pub fn quorum_health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }
}

impl Default for QuorumEngine {
    fn default() -> Self {
        Self::new()
    }
}


///////////////////////////////////////////////////////////////////////////////
// QUORUM GROUP — weighted group of quorum members
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct QuorumMember {
    pub node_id: NodeId,
    pub weight: u64,
    pub joined_at_ms: u64,
    pub active: bool,
}

impl QuorumMember {
    pub fn new(node_id: NodeId, weight: u64, joined_at_ms: u64) -> Self {
        Self {
            node_id,
            weight,
            joined_at_ms,
            active: true,
        }
    }

    pub fn total_weight(&self) -> u64 {
        if self.active { self.weight } else { 0 }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }
}

pub struct QuorumGroup {
    pub group_id: u64,
    pub name: String,
    pub members: std::cell::RefCell<Vec<QuorumMember>>,
    pub threshold: QuorumThreshold,
    pub created_at_ms: u64,
    pub version: u64,
}

impl QuorumGroup {
    pub fn new(
        group_id: u64,
        name: String,
        threshold: QuorumThreshold,
        created_at_ms: u64,
    ) -> Self {
        Self {
            group_id,
            name,
            members: std::cell::RefCell::new(Vec::new()),
            threshold,
            created_at_ms,
            version: 1,
        }
    }

    pub fn add_member(&self, member: QuorumMember) {
        self.members.borrow_mut().push(member);
        self.version += 1;
    }

    pub fn remove_member(&self, node_id: &NodeId) -> bool {
        let mut members = self.members.borrow_mut();
        let pos = members.iter().position(|m| m.node_id == *node_id);
        if let Some(idx) = pos {
            members.remove(idx);
            self.version += 1;
            true
        } else {
            false
        }
    }

    pub fn get_member(&self, node_id: &NodeId) -> Option<QuorumMember> {
        self.members.borrow().iter().find(|m| &m.node_id == node_id).cloned()
    }

    pub fn member_count(&self) -> usize {
        self.members.borrow().len()
    }

    pub fn active_member_count(&self) -> usize {
        self.members.borrow().iter().filter(|m| m.active).count()
    }

    pub fn total_weight(&self) -> u64 {
        self.members.borrow().iter().map(|m| m.total_weight()).sum()
    }

    pub fn active_weight(&self) -> u64 {
        self.members.borrow().iter().map(|m| m.total_weight()).sum()
    }

    pub fn is_member(&self, node_id: &NodeId) -> bool {
        self.members.borrow().iter().any(|m| m.node_id == *node_id && m.active)
    }

    pub fn threshold_met(&self, yes_weight: u64) -> bool {
        self.threshold.is_met(yes_weight, self.active_member_count() as u64, self.active_weight())
    }

    pub fn needed_weight(&self) -> u64 {
        self.threshold.needed_votes(self.active_member_count() as u64)
    }

    pub fn version(&self) -> u64 {
        self.version
    }
}

///////////////////////////////////////////////////////////////////////////////
// WEIGHTED VOTE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct WeightedVote {
    pub proposal_id: u64,
    pub voter: NodeId,
    pub vote_type: VoteType,
    pub weight: u64,
    pub timestamp_ms: u64,
}

impl WeightedVote {
    pub fn new(
        proposal_id: u64,
        voter: NodeId,
        vote_type: VoteType,
        weight: u64,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            proposal_id,
            voter,
            vote_type,
            weight,
            timestamp_ms,
        }
    }

    pub fn is_positive(&self) -> bool {
        self.vote_type.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.vote_type.is_negative()
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROPOSAL BATCH
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ProposalBatch {
    pub batch_id: u64,
    pub proposals: std::cell::RefCell<Vec<u64>>,
    pub created_at_ms: u64,
    pub committed_count: u64,
    pub aborted_count: u64,
    pub completed: bool,
}

impl ProposalBatch {
    pub fn new(batch_id: u64, created_at_ms: u64) -> Self {
        Self {
            batch_id,
            proposals: std::cell::RefCell::new(Vec::new()),
            created_at_ms,
            committed_count: 0,
            aborted_count: 0,
            completed: false,
        }
    }

    pub fn add_proposal(&self, proposal_id: u64) {
        self.proposals.borrow_mut().push(proposal_id);
    }

    pub fn proposal_count(&self) -> usize {
        self.proposals.borrow().len()
    }

    pub fn mark_committed(&mut self) {
        self.committed_count += 1;
    }

    pub fn mark_aborted(&mut self) {
        self.aborted_count += 1;
    }

    pub fn complete(&mut self) {
        self.completed = true;
    }

    pub fn completion_rate(&self) -> f64 {
        let total = self.committed_count + self.aborted_count;
        if total == 0 {
            return 0.0;
        }
        self.committed_count as f64 / total as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct QuorumSnapshot {
    pub snapshot_id: u64,
    pub view: u64,
    pub total_proposals: u64,
    pub committed_proposals: u64,
    pub aborted_proposals: u64,
    pub total_votes: u64,
    pub taken_at_ms: u64,
}

impl QuorumSnapshot {
    pub fn new(
        snapshot_id: u64,
        view: u64,
        taken_at_ms: u64,
    ) -> Self {
        Self {
            snapshot_id,
            view,
            total_proposals: 0,
            committed_proposals: 0,
            aborted_proposals: 0,
            total_votes: 0,
            taken_at_ms,
        }
    }

    pub fn update_stats(&mut self, total: u64, committed: u64, aborted: u64, votes: u64) {
        self.total_proposals = total;
        self.committed_proposals = committed;
        self.aborted_proposals = aborted;
        self.total_votes = votes;
    }

    pub fn commit_rate(&self) -> f64 {
        if self.total_proposals == 0 {
            return 1.0;
        }
        self.committed_proposals as f64 / self.total_proposals as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED QUORUM ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

impl QuorumEngine {
    /// Create a proposal with a specific quorum group.
    pub fn create_proposal_with_group(
        &self,
        proposal_id: u64,
        proposer: NodeId,
        payload: Vec<u8>,
        group_id: u64,
    ) -> Result<Proposal, QuorumError> {
        let configs = self.configs.borrow();
        let config = configs.get(&group_id).ok_or_else(|| QuorumError::QuorumNotConfigured { group_id })?;
        drop(configs);

        if !config.is_member(&proposer) {
            return Err(QuorumError::VoterNotInQuorum { voter: proposer });
        }

        let mut proposals = self.proposals.borrow_mut();
        if proposals.contains_key(&proposal_id) {
            return Err(QuorumError::ProposalAlreadyExists { proposal_id });
        }
        if proposals.len() as u64 >= config.max_proposals {
            return Err(QuorumError::Internal {
                message: "max proposals reached".to_string(),
            });
        }

        let proposal = Proposal::new(
            proposal_id,
            proposer,
            payload,
            config.threshold,
            config.total_weight,
            0,
        );
        proposals.insert(proposal_id, proposal.clone());
        self.stats.borrow_mut().total_proposals += 1;
        self.monitor.borrow_mut().record_activity(proposer, 1);
        Ok(proposal)
    }

    /// Expire a proposal that has not been decided.
    pub fn expire_proposal(&self, proposal_id: u64) -> Result<(), QuorumError> {
        let mut proposals = self.proposals.borrow_mut();
        let proposal = proposals.get_mut(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        if proposal.state.is_decided() {
            return Err(QuorumError::ProposalAlreadyDecided {
                proposal_id,
                state: proposal.state,
            });
        }
        proposal.transition_to(ProposalState::Expired)?;
        self.stats.borrow_mut().total_expired += 1;
        Ok(())
    }

    /// Get total expired proposals.
    pub fn total_expired(&self) -> u64 {
        self.stats.borrow().total_expired
    }

    /// Get total votes cast.
    pub fn total_votes_cast(&self) -> u64 {
        self.stats.borrow().total_votes_cast
    }

    /// Get total quorums reached.
    pub fn total_quorums_reached(&self) -> u64 {
        self.stats.borrow().total_quorums_reached
    }

    /// Increment quorums reached counter.
    pub fn record_quorum_reached(&self) {
        self.stats.borrow_mut().total_quorums_reached += 1;
    }

    /// Get total view changes.
    pub fn total_view_changes(&self) -> u64 {
        self.stats.borrow().total_view_changes
    }

    /// Get total conflicts resolved.
    pub fn total_conflicts_resolved(&self) -> u64 {
        self.stats.borrow().total_conflicts_resolved
    }

    /// Get total certificates generated.
    pub fn total_certificates_generated(&self) -> u64 {
        self.stats.borrow().total_certificates_generated
    }

    /// Get total certificates verified.
    pub fn total_certificates_verified(&self) -> u64 {
        self.stats.borrow().total_certificates_verified
    }

    /// Get proposal count.
    pub fn proposal_count(&self) -> usize {
        self.proposals.borrow().len()
    }

    /// Get config count.
    pub fn config_count(&self) -> usize {
        self.configs.borrow().len()
    }

    /// Get certificate count.
    pub fn certificate_count(&self) -> usize {
        self.certificates.borrow().len()
    }

    /// Check if a proposal is expired.
    pub fn is_expired(&self, proposal_id: u64) -> Result<bool, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        Ok(proposal.state == ProposalState::Expired)
    }

    /// Get proposals by state.
    pub fn proposals_by_state(&self, state: ProposalState) -> Vec<u64> {
        self.proposals
            .borrow()
            .iter()
            .filter(|(_, p)| p.state == state)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get committed proposal IDs.
    pub fn committed_proposals(&self) -> Vec<u64> {
        self.proposals_by_state(ProposalState::Committed)
    }

    /// Get aborted proposal IDs.
    pub fn aborted_proposals(&self) -> Vec<u64> {
        self.proposals_by_state(ProposalState::Aborted)
    }

    /// Get expired proposal IDs.
    pub fn expired_proposals(&self) -> Vec<u64> {
        self.proposals_by_state(ProposalState::Expired)
    }

    /// Get pending (non-decided) proposal IDs.
    pub fn pending_proposals(&self) -> Vec<u64> {
        self.proposals
            .borrow()
            .iter()
            .filter(|(_, p)| !p.state.is_decided())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Take a snapshot of current quorum state.
    pub fn take_snapshot(&self, snapshot_id: u64) -> QuorumSnapshot {
        let stats = self.stats.borrow();
        let mut snapshot = QuorumSnapshot::new(snapshot_id, *self.current_view.borrow(), 0);
        snapshot.update_stats(
            stats.total_proposals,
            stats.total_committed,
            stats.total_aborted,
            stats.total_votes_cast,
        );
        snapshot
    }

    /// Get quorum health score.
    pub fn health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }

    /// Get commit rate.
    pub fn commit_rate(&self) -> f64 {
        self.stats.borrow().commit_rate()
    }

    /// Get abort rate.
    pub fn abort_rate(&self) -> f64 {
        self.stats.borrow().abort_rate()
    }

    /// Get quorum rate.
    pub fn quorum_rate(&self) -> f64 {
        self.stats.borrow().quorum_rate()
    }

}

///////////////////////////////////////////////////////////////////////////////
// QUORUM PROPOSAL RECORD — record of a proposal decision
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct QuorumProposalRecord {
    pub proposal_id: u64,
    pub proposer: NodeId,
    pub state: ProposalState,
    pub yes_weight: u64,
    pub no_weight: u64,
    pub abstain_weight: u64,
    pub total_voters: u64,
    pub decided_at_ms: Option<u64>,
    pub view: u64,
}

impl QuorumProposalRecord {
    pub fn new(
        proposal_id: u64,
        proposer: NodeId,
        state: ProposalState,
        view: u64,
    ) -> Self {
        Self {
            proposal_id,
            proposer,
            state,
            yes_weight: 0,
            no_weight: 0,
            abstain_weight: 0,
            total_voters: 0,
            decided_at_ms: None,
            view,
        }
    }

    pub fn update_weights(&mut self, yes: u64, no: u64, abstain: u64, total_voters: u64) {
        self.yes_weight = yes;
        self.no_weight = no;
        self.abstain_weight = abstain;
        self.total_voters = total_voters;
    }

    pub fn mark_decided(&mut self, at_ms: u64) {
        self.decided_at_ms = Some(at_ms);
    }

    pub fn net_weight(&self) -> i64 {
        self.yes_weight as i64 - self.no_weight as i64
    }

    pub fn participation_rate(&self) -> f64 {
        if self.total_voters == 0 {
            return 0.0;
        }
        (self.yes_weight + self.no_weight + self.abstain_weight) as f64 / self.total_voters as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// VOTE RECEIPT
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct VoteReceipt {
    pub receipt_id: u64,
    pub proposal_id: u64,
    pub voter: NodeId,
    pub vote_type: VoteType,
    pub weight: u64,
    pub received_at_ms: u64,
    pub acknowledged: bool,
}

impl VoteReceipt {
    pub fn new(
        receipt_id: u64,
        proposal_id: u64,
        voter: NodeId,
        vote_type: VoteType,
        weight: u64,
        received_at_ms: u64,
    ) -> Self {
        Self {
            receipt_id,
            proposal_id,
            voter,
            vote_type,
            weight,
            received_at_ms,
            acknowledged: false,
        }
    }

    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    pub fn is_acknowledged(&self) -> bool {
        self.acknowledged
    }
}

///////////////////////////////////////////////////////////////////////////////
// QUORUM DECISION LOG — append-only log of quorum decisions
///////////////////////////////////////////////////////////////////////////////

pub struct QuorumDecisionLog {
    pub decisions: std::cell::RefCell<Vec<QuorumProposalRecord>>,
    pub log_id: u64,
    pub total_entries: u64,
    pub committed_entries: u64,
    pub aborted_entries: u64,
}

impl QuorumDecisionLog {
    pub fn new(log_id: u64) -> Self {
        Self {
            decisions: std::cell::RefCell::new(Vec::new()),
            log_id,
            total_entries: 0,
            committed_entries: 0,
            aborted_entries: 0,
        }
    }

    pub fn append(&self, record: QuorumProposalRecord) {
        self.decisions.borrow_mut().push(record);
        self.total_entries += 1;
    }

    pub fn entry_count(&self) -> usize {
        self.decisions.borrow().len()
    }

    pub fn get_entry(&self, index: usize) -> Option<QuorumProposalRecord> {
        self.decisions.borrow().get(index).cloned()
    }

    pub fn committed_count(&self) -> u64 {
        self.decisions.borrow().iter().filter(|r| r.state == ProposalState::Committed).count() as u64
    }

    pub fn aborted_count(&self) -> u64 {
        self.decisions.borrow().iter().filter(|r| r.state == ProposalState::Aborted).count() as u64
    }

    pub fn pending_count(&self) -> u64 {
        self.decisions.borrow().iter().filter(|r| !r.state.is_decided()).count() as u64
    }

    pub fn commit_rate(&self) -> f64 {
        if self.total_entries == 0 {
            return 1.0;
        }
        self.committed_count() as f64 / self.total_entries as f64
    }

    pub fn clear(&self) {
        self.decisions.borrow_mut().clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// MORE QUORUM ENGINE METHODS
///////////////////////////////////////////////////////////////////////////////

impl QuorumEngine {
    /// Record a proposal decision in the decision log (via snapshot).
    pub fn record_proposal_decision(&self, record: QuorumProposalRecord) -> QuorumProposalRecord {
        // Track via stats
        match record.state {
            ProposalState::Committed => self.stats.borrow_mut().total_committed += 1,
            ProposalState::Aborted => self.stats.borrow_mut().total_aborted += 1,
            ProposalState::Expired => self.stats.borrow_mut().total_expired += 1,
            _ => {}
        }
        record
    }

    /// Create a vote receipt.
    pub fn create_vote_receipt(
        &self,
        receipt_id: u64,
        proposal_id: u64,
        voter: NodeId,
        vote_type: VoteType,
        weight: u64,
        received_at_ms: u64,
    ) -> VoteReceipt {
        VoteReceipt::new(receipt_id, proposal_id, voter, vote_type, weight, received_at_ms)
    }

    /// Get proposal record from a proposal.
    pub fn proposal_record(&self, proposal_id: u64, view: u64) -> Result<QuorumProposalRecord, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        let mut record = QuorumProposalRecord::new(proposal_id, proposal.proposer, proposal.state, view);
        record.update_weights(proposal.yes_votes(), proposal.no_votes(), proposal.abstain_votes(), proposal.vote_count());
        if proposal.decided_at_ms.is_some() {
            record.mark_decided(0);
        }
        Ok(record)
    }

    /// Get all committed proposal records.
    pub fn all_committed_records(&self, view: u64) -> Vec<QuorumProposalRecord> {
        let committed = self.committed_proposals();
        committed.into_iter().filter_map(|id| self.proposal_record(id, view).ok()).collect()
    }

    /// Get all aborted proposal records.
    pub fn all_aborted_records(&self, view: u64) -> Vec<QuorumProposalRecord> {
        let aborted = self.aborted_proposals();
        aborted.into_iter().filter_map(|id| self.proposal_record(id, view).ok()).collect()
    }

    /// Reset all stats.
    pub fn reset_stats(&self) {
        *self.stats.borrow_mut() = QuorumStats::default();
    }

    /// Get the number of tracked configs.
    pub fn tracked_configs(&self) -> Vec<u64> {
        self.configs.borrow().keys().copied().collect()
    }

    /// Check if quorum group exists.
    pub fn has_config(&self, group_id: u64) -> bool {
        self.configs.borrow().contains_key(&group_id)
    }

    /// Get proposal payload size.
    pub fn proposal_payload_size(&self, proposal_id: u64) -> Result<usize, QuorumError> {
        let proposals = self.proposals.borrow();
        let proposal = proposals.get(&proposal_id).ok_or_else(|| QuorumError::ProposalNotFound { proposal_id })?;
        Ok(proposal.payload_size())
    }

    /// Get total proposal payload bytes.
    pub fn total_proposal_payload_bytes(&self) -> usize {
        self.proposals.borrow().values().map(|p| p.payload_size()).sum()
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

    fn test_peer3_id() -> NodeId {
        NodeId::new([3; 32])
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

    fn test_failure_domain() -> FailureDomain {
        FailureDomain {
            power: Some([1u8; 16]),
            facility: Some([2u8; 16]),
            network_segment: Some([3u8; 16]),
            operator: Some([4u8; 16]),
            geographic: Some([5u8; 16]),
        }
    }

    fn test_quorum_config() -> QuorumConfig {
        QuorumConfig::new(
            1,
            "test_quorum".to_string(),
            vec![test_node_id(), test_peer_id(), test_peer3_id()],
            QuorumThreshold::SimpleMajority,
        )
    }

    // === QuorumError tests ===
    #[test]
    fn quorum_error_category() {
        assert_eq!(
            QuorumError::QuorumNotConfigured { group_id: 1 }.category(),
            "quorum_not_configured"
        );
        assert_eq!(
            QuorumError::ProposalNotFound { proposal_id: 42 }.category(),
            "proposal_not_found"
        );
        assert_eq!(
            QuorumError::ProposalAlreadyExists { proposal_id: 42 }.category(),
            "proposal_already_exists"
        );
        assert_eq!(
            QuorumError::ProposalAlreadyDecided { proposal_id: 42, state: ProposalState::Committed }.category(),
            "proposal_already_decided"
        );
        assert_eq!(
            QuorumError::VoteAlreadyCast { voter: test_node_id(), proposal_id: 42 }.category(),
            "vote_already_cast"
        );
        assert_eq!(
            QuorumError::VoterNotInQuorum { voter: test_node_id() }.category(),
            "voter_not_in_quorum"
        );
        assert_eq!(
            QuorumError::InsufficientVotes { have: 1, needed: 3 }.category(),
            "insufficient_votes"
        );
        assert_eq!(
            QuorumError::ThresholdNotMet { threshold: QuorumThreshold::Unanimous }.category(),
            "threshold_not_met"
        );
        assert_eq!(
            QuorumError::InvalidView { current: 1, requested: 0 }.category(),
            "invalid_view"
        );
        assert_eq!(
            QuorumError::LeaderNotAuthorized { leader: test_node_id() }.category(),
            "leader_not_authorized"
        );
        assert_eq!(
            QuorumError::ConflictUnresolved { proposal_id_a: 1, proposal_id_b: 2 }.category(),
            "conflict_unresolved"
        );
        assert_eq!(
            QuorumError::CertificateInvalid { reason: "bad".to_string() }.category(),
            "certificate_invalid"
        );
        assert_eq!(
            QuorumError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn quorum_error_is_recoverable() {
        assert!(QuorumError::InsufficientVotes { have: 1, needed: 3 }.is_recoverable());
        assert!(QuorumError::ThresholdNotMet { threshold: QuorumThreshold::Unanimous }.is_recoverable());
        assert!(QuorumError::InvalidView { current: 1, requested: 0 }.is_recoverable());
        assert!(!QuorumError::ProposalNotFound { proposal_id: 1 }.is_recoverable());
    }

    #[test]
    fn quorum_error_suggested_action() {
        assert_eq!(
            QuorumError::InsufficientVotes { have: 1, needed: 3 }.suggested_action(),
            "wait for more votes"
        );
        assert_eq!(
            QuorumError::CertificateInvalid { reason: "x".to_string() }.suggested_action(),
            "regenerate certificate"
        );
        assert_eq!(
            QuorumError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === ProposalState tests ===
    #[test]
    fn proposal_state_as_str() {
        assert_eq!(ProposalState::Proposed.as_str(), "proposed");
        assert_eq!(ProposalState::Voting.as_str(), "voting");
        assert_eq!(ProposalState::Committed.as_str(), "committed");
        assert_eq!(ProposalState::Aborted.as_str(), "aborted");
        assert_eq!(ProposalState::Expired.as_str(), "expired");
    }

    #[test]
    fn proposal_state_is_decided() {
        assert!(ProposalState::Committed.is_decided());
        assert!(ProposalState::Aborted.is_decided());
        assert!(ProposalState::Expired.is_decided());
        assert!(!ProposalState::Proposed.is_decided());
        assert!(!ProposalState::Voting.is_decided());
    }

    #[test]
    fn proposal_state_can_transition() {
        assert!(ProposalState::Proposed.can_transition_to(ProposalState::Voting));
        assert!(ProposalState::Voting.can_transition_to(ProposalState::Committed));
        assert!(!ProposalState::Committed.can_transition_to(ProposalState::Aborted));
    }

    // === VoteType tests ===
    #[test]
    fn vote_type_as_str() {
        assert_eq!(VoteType::Yes.as_str(), "yes");
        assert_eq!(VoteType::No.as_str(), "no");
        assert_eq!(VoteType::Abstain.as_str(), "abstain");
        assert!(VoteType::Yes.is_positive());
        assert!(VoteType::No.is_negative());
        assert!(!VoteType::Abstain.is_positive());
    }

    // === Vote tests ===
    #[test]
    fn vote_new_and_signature() {
        let vote = Vote::new(test_node_id(), VoteType::Yes, 1, 1000);
        assert_eq!(vote.voter, test_node_id());
        assert_eq!(vote.vote_type, VoteType::Yes);
        assert_eq!(vote.weight, 1);
        assert!(!vote.is_signed());
        let vote = vote.with_signature(vec![1, 2, 3]);
        assert!(vote.is_signed());
    }

    // === QuorumThreshold tests ===
    #[test]
    fn quorum_threshold_simple_majority() {
        assert!(QuorumThreshold::SimpleMajority.is_met(2, 3, 3));
        assert!(!QuorumThreshold::SimpleMajority.is_met(1, 3, 3));
    }

    #[test]
    fn quorum_threshold_super_majority_2_3() {
        assert!(QuorumThreshold::SuperMajority2_3.is_met(2, 3, 3));
        assert!(!QuorumThreshold::SuperMajority2_3.is_met(1, 3, 3));
    }

    #[test]
    fn quorum_threshold_unanimous() {
        assert!(QuorumThreshold::Unanimous.is_met(3, 3, 3));
        assert!(!QuorumThreshold::Unanimous.is_met(2, 3, 3));
    }

    #[test]
    fn quorum_threshold_weighted() {
        assert!(QuorumThreshold::Weighted(0.5).is_met(2, 4, 4));
        assert!(!QuorumThreshold::Weighted(0.8).is_met(2, 4, 4));
    }

    #[test]
    fn quorum_threshold_custom() {
        assert!(QuorumThreshold::Custom(2, 5).is_met(2, 5, 5));
        assert!(!QuorumThreshold::Custom(3, 5).is_met(2, 5, 5));
    }

    #[test]
    fn quorum_threshold_needed_votes() {
        assert_eq!(QuorumThreshold::SimpleMajority.needed_votes(3), 2);
        assert_eq!(QuorumThreshold::Unanimous.needed_votes(3), 3);
        assert_eq!(QuorumThreshold::SuperMajority2_3.needed_votes(3), 2);
    }

    #[test]
    fn quorum_threshold_as_str() {
        assert_eq!(QuorumThreshold::SimpleMajority.as_str(), "simple_majority");
        assert_eq!(QuorumThreshold::Unanimous.as_str(), "unanimous");
        assert_eq!(QuorumThreshold::Weighted(0.5).as_str(), "weighted");
    }

    // === QuorumConfig tests ===
    #[test]
    fn quorum_config_member_count() {
        let config = test_quorum_config();
        assert_eq!(config.member_count(), 3);
        assert!(config.is_member(&test_node_id()));
        assert!(!config.is_member(&NodeId::new([9; 32])));
    }

    #[test]
    fn quorum_config_needed_votes() {
        let config = test_quorum_config();
        assert_eq!(config.needed_votes(), 2);
    }

    #[test]
    fn quorum_config_validate() {
        let config = test_quorum_config();
        assert!(config.validate().is_ok());
        let config = QuorumConfig::new(1, "empty".to_string(), Vec::new(), QuorumThreshold::SimpleMajority);
        assert!(config.validate().is_err());
    }

    #[test]
    fn quorum_config_builder() {
        let config = test_quorum_config()
            .with_timeouts(10000, 20000)
            .with_max_proposals(500)
            .with_leader(test_node_id());
        assert_eq!(config.view_timeout_ms, 10000);
        assert_eq!(config.proposal_timeout_ms, 20000);
        assert_eq!(config.max_proposals, 500);
        assert_eq!(config.leader_id, Some(test_node_id()));
    }

    // === Proposal tests ===
    #[test]
    fn proposal_add_vote() {
        let proposal = Proposal::new(
            1,
            test_node_id(),
            vec![1, 2, 3],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        let record = VoteRecord::new(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 2000), 2000);
        proposal.add_vote(record).unwrap();
        assert_eq!(proposal.vote_count(), 1);
        assert_eq!(proposal.yes_votes(), 1);
    }

    #[test]
    fn proposal_duplicate_vote() {
        let proposal = Proposal::new(
            1,
            test_node_id(),
            vec![],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        let record = VoteRecord::new(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 2000), 2000);
        proposal.add_vote(record.clone()).unwrap();
        let err = proposal.add_vote(record).unwrap_err();
        assert!(matches!(err, QuorumError::VoteAlreadyCast { .. }));
    }

    #[test]
    fn proposal_decided_cannot_vote() {
        let mut proposal = Proposal::new(
            1,
            test_node_id(),
            vec![],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        proposal.transition_to(ProposalState::Committed).unwrap();
        let record = VoteRecord::new(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 2000), 2000);
        let err = proposal.add_vote(record).unwrap_err();
        assert!(matches!(err, QuorumError::ProposalAlreadyDecided { .. }));
    }

    #[test]
    fn proposal_is_quorum_reached() {
        let proposal = Proposal::new(
            1,
            test_node_id(),
            vec![],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        let record1 = VoteRecord::new(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 2000), 2000);
        let record2 = VoteRecord::new(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 3000), 3000);
        proposal.add_vote(record1).unwrap();
        proposal.add_vote(record2).unwrap();
        assert!(proposal.is_quorum_reached());
    }

    #[test]
    fn proposal_tally() {
        let proposal = Proposal::new(
            1,
            test_node_id(),
            vec![],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        let record1 = VoteRecord::new(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 2000), 2000);
        let record2 = VoteRecord::new(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::No, 1, 3000), 3000);
        proposal.add_vote(record1).unwrap();
        proposal.add_vote(record2).unwrap();
        assert_eq!(proposal.yes_votes(), 1);
        assert_eq!(proposal.no_votes(), 1);
        assert_eq!(proposal.abstain_votes(), 0);
        assert_eq!(proposal.total_vote_weight(), 2);
    }

    #[test]
    fn proposal_transition() {
        let mut proposal = Proposal::new(
            1,
            test_node_id(),
            vec![],
            QuorumThreshold::SimpleMajority,
            3,
            1000,
        );
        proposal.transition_to(ProposalState::Voting).unwrap();
        assert_eq!(proposal.state, ProposalState::Voting);
        let err = proposal.transition_to(ProposalState::Proposed).unwrap_err();
        assert!(matches!(err, QuorumError::Internal { .. }));
    }

    // === QuorumCertificate tests ===
    #[test]
    fn quorum_certificate_add_signature() {
        let mut cert = QuorumCertificate::new(1, 0, 2, 3, QuorumThreshold::SimpleMajority, 1000);
        cert.add_signature(test_node_id(), vec![1, 2, 3]);
        assert_eq!(cert.signature_count(), 1);
    }

    #[test]
    fn quorum_certificate_is_valid() {
        let mut cert = QuorumCertificate::new(1, 0, 2, 2, QuorumThreshold::SimpleMajority, 1000);
        cert.mark_valid();
        assert!(cert.is_valid());
    }

    #[test]
    fn quorum_certificate_invalid_threshold() {
        let mut cert = QuorumCertificate::new(1, 0, 1, 3, QuorumThreshold::Unanimous, 1000);
        cert.mark_valid();
        assert!(!cert.is_valid());
    }

    // === ViewChangeRecord tests ===
    #[test]
    fn view_change_record_complete() {
        let mut vcr = ViewChangeRecord::new(0, 1, None, test_node_id(), 1000);
        assert!(!vcr.completed);
        vcr.complete();
        assert!(vcr.completed);
    }

    // === ConflictResolver tests ===
    #[test]
    fn conflict_resolver_resolve() {
        let cr = ConflictResolver::default();
        cr.resolve(1, 2, ResolutionResult::ProposalAWins);
        assert_eq!(cr.get_resolution(1, 2), Some(ResolutionResult::ProposalAWins));
        assert_eq!(cr.total_resolved(), 1);
    }

    #[test]
    fn conflict_resolution_as_str() {
        assert_eq!(ResolutionResult::ProposalAWins.as_str(), "proposal_a_wins");
        assert_eq!(ResolutionResult::Merge.as_str(), "merge");
        assert_eq!(ResolutionResult::ManualIntervention.as_str(), "manual_intervention");
    }

    // === QuorumStats tests ===
    #[test]
    fn quorum_stats_rates() {
        let mut s = QuorumStats::default();
        s.total_proposals = 10;
        s.total_committed = 7;
        s.total_aborted = 2;
        assert!((s.commit_rate() - 0.7).abs() < 0.01);
        assert!((s.abort_rate() - 0.2).abs() < 0.01);
    }

    #[test]
    fn quorum_stats_health_score() {
        let mut s = QuorumStats::default();
        assert_eq!(s.system_health_score(), 100.0);
        s.total_aborted = 10;
        let score = s.system_health_score();
        assert!(score < 100.0);
        assert!(score >= 0.0);
    }

    // === QuorumMonitor tests ===
    #[test]
    fn quorum_monitor_record_activity() {
        let qm = QuorumMonitor::default();
        qm.record_activity(test_node_id(), 5);
        assert_eq!(qm.node_action_count(&test_node_id()), 5);
        assert_eq!(qm.health_check_count(), 1);
    }

    #[test]
    fn quorum_monitor_incident() {
        let qm = QuorumMonitor::default();
        qm.record_activity(test_node_id(), 5);
        qm.report_incident(test_node_id());
        assert_eq!(qm.quorum_incident_count(), 1);
        assert_eq!(qm.alert_count(), 1);
        assert_eq!(qm.inactive_node_count(), 1);
    }

    // === QuorumValidationResult tests ===
    #[test]
    fn quorum_validation_result_as_str() {
        assert_eq!(QuorumValidationResult::Valid.as_str(), "valid");
        assert_eq!(QuorumValidationResult::InvalidProposalID.as_str(), "invalid_proposal_id");
        assert_eq!(QuorumValidationResult::InvalidVoter.as_str(), "invalid_voter");
        assert_eq!(QuorumValidationResult::InvalidThreshold.as_str(), "invalid_threshold");
        assert_eq!(QuorumValidationResult::InvalidWeight.as_str(), "invalid_weight");
        assert_eq!(QuorumValidationResult::InvalidView.as_str(), "invalid_view");
        assert!(QuorumValidationResult::Valid.is_ok());
        assert!(!QuorumValidationResult::InvalidVoter.is_ok());
    }

    // === QuorumValidator tests ===
    #[test]
    fn quorum_validator_validate_proposal_id() {
        let qv = QuorumValidator::default();
        assert_eq!(qv.validate_proposal_id(1), QuorumValidationResult::Valid);
        assert_eq!(qv.validate_proposal_id(0), QuorumValidationResult::InvalidProposalID);
    }

    #[test]
    fn quorum_validator_validate_voter() {
        let qv = QuorumValidator::default();
        assert_eq!(qv.validate_voter(test_node_id()), QuorumValidationResult::Valid);
        assert_eq!(
            qv.validate_voter(NodeId::new([0u8; 32])),
            QuorumValidationResult::InvalidVoter
        );
    }

    #[test]
    fn quorum_validator_validate_threshold() {
        let qv = QuorumValidator::default();
        assert_eq!(
            qv.validate_threshold(QuorumThreshold::SimpleMajority, 3),
            QuorumValidationResult::Valid
        );
    }

    #[test]
    fn quorum_validator_validate_weight() {
        let qv = QuorumValidator::default();
        assert_eq!(qv.validate_weight(test_node_id(), 1), QuorumValidationResult::Valid);
        assert_eq!(
            qv.validate_weight(test_node_id(), 0),
            QuorumValidationResult::InvalidWeight
        );
    }

    #[test]
    fn quorum_validator_validate_view() {
        let qv = QuorumValidator::default();
        assert_eq!(
            qv.validate_view(test_node_id(), 1, 2),
            QuorumValidationResult::Valid
        );
        assert_eq!(
            qv.validate_view(test_node_id(), 2, 1),
            QuorumValidationResult::InvalidView
        );
    }

    #[test]
    fn quorum_validator_valid_rate() {
        let qv = QuorumValidator::default();
        qv.validate_proposal_id(1);
        qv.validate_proposal_id(0);
        assert!((qv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === QuorumEngine tests ===
    #[test]
    fn quorum_engine_configure_and_create() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        let proposal = engine.create_proposal(1, test_node_id(), vec![1, 2, 3]).unwrap();
        assert_eq!(proposal.proposal_id, 1);
        assert_eq!(engine.total_proposals(), 1);
    }

    #[test]
    fn quorum_engine_quorum_not_configured() {
        let engine = QuorumEngine::default();
        let err = engine.create_proposal(1, test_node_id(), vec![]).unwrap_err();
        assert!(matches!(err, QuorumError::QuorumNotConfigured { .. }));
    }

    #[test]
    fn quorum_engine_proposal_already_exists() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        let err = engine.create_proposal(1, test_node_id(), vec![]).unwrap_err();
        assert!(matches!(err, QuorumError::ProposalAlreadyExists { .. }));
    }

    #[test]
    fn quorum_engine_cast_vote() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        assert_eq!(engine.stats().borrow().total_votes_cast, 1);
    }

    #[test]
    fn quorum_engine_voter_not_in_quorum() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        let err = engine.cast_vote(1, NodeId::new([9; 32]), Vote::new(NodeId::new([9; 32]), VoteType::Yes, 1, 1000)).unwrap_err();
        assert!(matches!(err, QuorumError::VoterNotInQuorum { .. }));
    }

    #[test]
    fn quorum_engine_is_quorum_reached() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        assert!(engine.is_quorum_reached(1).unwrap());
    }

    #[test]
    fn quorum_engine_commit_proposal() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        engine.commit_proposal(1).unwrap();
        assert_eq!(engine.proposal_state(1).unwrap(), ProposalState::Committed);
        assert_eq!(engine.total_committed(), 1);
    }

    #[test]
    fn quorum_engine_abort_proposal() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.abort_proposal(1).unwrap();
        assert_eq!(engine.proposal_state(1).unwrap(), ProposalState::Aborted);
        assert_eq!(engine.total_aborted(), 1);
    }

    #[test]
    fn quorum_engine_vote_tally() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::No, 1, 2000)).unwrap();
        let (yes, no, abstain) = engine.vote_tally(1).unwrap();
        assert_eq!(yes, 1);
        assert_eq!(no, 1);
        assert_eq!(abstain, 0);
    }

    #[test]
    fn quorum_engine_generate_and_verify_certificate() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        engine.commit_proposal(1).unwrap();
        let cert = engine.generate_certificate(1).unwrap();
        assert!(cert.is_valid());
        assert!(engine.verify_certificate(&cert).unwrap());
    }

    #[test]
    fn quorum_engine_view_change() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.view_change(1, test_node_id()).unwrap();
        assert_eq!(engine.current_view().unwrap(), 1);
        assert_eq!(engine.current_leader().unwrap(), test_node_id());
    }

    #[test]
    fn quorum_engine_invalid_view_change() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.view_change(1, test_node_id()).unwrap();
        let err = engine.view_change(0, test_node_id()).unwrap_err();
        assert!(matches!(err, QuorumError::InvalidView { .. }));
    }

    #[test]
    fn quorum_engine_leader_not_authorized() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        let err = engine.view_change(1, NodeId::new([9; 32])).unwrap_err();
        assert!(matches!(err, QuorumError::LeaderNotAuthorized { .. }));
    }

    #[test]
    fn quorum_engine_resolve_conflict() {
        let engine = QuorumEngine::default();
        let result = engine.resolve_conflict(1, 2).unwrap();
        assert_eq!(result, ResolutionResult::ProposalBWins);
        assert_eq!(engine.stats().borrow().total_conflicts_resolved, 1);
    }

    #[test]
    fn quorum_engine_stats_and_name() {
        let engine = QuorumEngine::default();
        assert_eq!(engine.quorum_name(), "quorum_engine");
        assert_eq!(engine.total_proposals(), 0);
        assert_eq!(engine.total_committed(), 0);
        assert_eq!(engine.total_aborted(), 0);
    }

    #[test]
    fn quorum_engine_monitor_node() {
        let engine = QuorumEngine::default();
        engine.monitor_node(test_node_id(), 5);
        assert!(engine.is_node_active(&test_node_id()));
        assert_eq!(engine.monitor.borrow().monitored_node_count(), 1);
    }

    #[test]
    fn quorum_engine_validate_all() {
        let engine = QuorumEngine::default();
        assert_eq!(engine.validate_proposal_id(1), QuorumValidationResult::Valid);
        assert_eq!(engine.validate_voter(test_node_id()), QuorumValidationResult::Valid);
        assert_eq!(
            engine.validate_threshold(QuorumThreshold::SimpleMajority, 3),
            QuorumValidationResult::Valid
        );
        assert_eq!(engine.validate_weight(test_node_id(), 1), QuorumValidationResult::Valid);
        assert_eq!(
            engine.validate_view(test_node_id(), 1, 2),
            QuorumValidationResult::Valid
        );
    }

    #[test]
    fn quorum_engine_health_score() {
        let engine = QuorumEngine::default();
        let score = engine.quorum_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full quorum flow ===
    #[test]
    fn quorum_full_flow() {
        let engine = QuorumEngine::default();

        // Configure quorum
        engine.configure_quorum(test_quorum_config()).unwrap();

        // Create proposal
        let proposal = engine.create_proposal(1, test_node_id(), vec![1, 2, 3]).unwrap();
        assert_eq!(proposal.state, ProposalState::Proposed);

        // Cast votes
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();

        // Check quorum
        assert!(engine.is_quorum_reached(1).unwrap());

        // Commit
        engine.commit_proposal(1).unwrap();
        assert_eq!(engine.proposal_state(1).unwrap(), ProposalState::Committed);

        // Generate certificate
        let cert = engine.generate_certificate(1).unwrap();
        assert!(cert.is_valid());
        assert!(engine.verify_certificate(&cert).unwrap());

        // View change
        engine.view_change(1, test_peer_id()).unwrap();
        assert_eq!(engine.current_view().unwrap(), 1);

        // Stats
        let s = engine.stats();
        assert_eq!(s.total_proposals, 1);
        assert_eq!(s.total_committed, 1);
        assert_eq!(s.total_votes_cast, 2);

        // Health score
        let score = engine.quorum_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === NodeId zero peer test ===
    #[test]
    fn node_id_zero_peer() {
        let zero = NodeId::new([0u8; 32]);
        assert_ne!(test_node_id(), zero);
    }

    // === Endpoint clone test ===
    #[test]
    fn endpoint_clone() {
        let ep1 = test_endpoint();
        let ep2 = ep1.clone();
        assert_eq!(ep1.carrier_type, ep2.carrier_type);
        assert_eq!(ep1.address, ep2.address);
    }

    // === FailureDomain clone test ===
    #[test]
    fn failure_domain_clone() {
        let fd1 = test_failure_domain();
        let fd2 = fd1.clone();
        assert_eq!(fd1.power, fd2.power);
        assert_eq!(fd1.facility, fd2.facility);
    }

    // === QuorumConfig clone test ===
    #[test]
    fn quorum_config_clone() {
        let cfg1 = test_quorum_config();
        let cfg2 = cfg1.clone();
        assert_eq!(cfg1.group_id, cfg2.group_id);
        assert_eq!(cfg1.name, cfg2.name);
        assert_eq!(cfg1.member_count(), cfg2.member_count());
    }

    // === QuorumMember tests ===
    #[test]
    fn quorum_member_new_and_total_weight() {
        let member = QuorumMember::new(test_node_id(), 10, 1000);
        assert_eq!(member.node_id, test_node_id());
        assert_eq!(member.weight, 10);
        assert_eq!(member.total_weight(), 10);
        assert!(member.active);
    }

    #[test]
    fn quorum_member_deactivate_activate() {
        let mut member = QuorumMember::new(test_node_id(), 10, 1000);
        member.deactivate();
        assert_eq!(member.total_weight(), 0);
        assert!(!member.active);
        member.activate();
        assert_eq!(member.total_weight(), 10);
        assert!(member.active);
    }

    // === QuorumGroup tests ===
    #[test]
    fn quorum_group_add_and_remove_member() {
        let group = QuorumGroup::new(1, "group1".to_string(), QuorumThreshold::SimpleMajority, 1000);
        group.add_member(QuorumMember::new(test_node_id(), 10, 1000));
        group.add_member(QuorumMember::new(test_peer_id(), 20, 2000));
        assert_eq!(group.member_count(), 2);
        assert_eq!(group.active_member_count(), 2);
        assert_eq!(group.total_weight(), 30);
        assert!(group.is_member(&test_node_id()));
        assert!(!group.is_member(&NodeId::new([9; 32])));
    }

    #[test]
    fn quorum_group_remove_member() {
        let group = QuorumGroup::new(1, "group1".to_string(), QuorumThreshold::SimpleMajority, 1000);
        group.add_member(QuorumMember::new(test_node_id(), 10, 1000));
        assert!(group.remove_member(&test_node_id()));
        assert!(!group.remove_member(&test_node_id()));
        assert_eq!(group.member_count(), 0);
    }

    #[test]
    fn quorum_group_threshold_met() {
        let group = QuorumGroup::new(1, "group1".to_string(), QuorumThreshold::SimpleMajority, 1000);
        group.add_member(QuorumMember::new(test_node_id(), 10, 1000));
        group.add_member(QuorumMember::new(test_peer_id(), 10, 2000));
        group.add_member(QuorumMember::new(test_peer3_id(), 10, 3000));
        // Simple majority of 3 = 2 needed
        assert!(!group.threshold_met(10));
        assert!(group.threshold_met(20));
    }

    #[test]
    fn quorum_group_needed_weight() {
        let group = QuorumGroup::new(1, "group1".to_string(), QuorumThreshold::Unanimous, 1000);
        group.add_member(QuorumMember::new(test_node_id(), 10, 1000));
        group.add_member(QuorumMember::new(test_peer_id(), 10, 2000));
        assert_eq!(group.needed_weight(), 2);
    }

    #[test]
    fn quorum_group_version() {
        let group = QuorumGroup::new(1, "group1".to_string(), QuorumThreshold::SimpleMajority, 1000);
        assert_eq!(group.version(), 1);
        group.add_member(QuorumMember::new(test_node_id(), 10, 1000));
        assert_eq!(group.version(), 2);
    }

    // === WeightedVote tests ===
    #[test]
    fn weighted_vote_new() {
        let vote = WeightedVote::new(1, test_node_id(), VoteType::Yes, 5, 1000);
        assert_eq!(vote.proposal_id, 1);
        assert_eq!(vote.voter, test_node_id());
        assert_eq!(vote.vote_type, VoteType::Yes);
        assert_eq!(vote.weight, 5);
        assert_eq!(vote.timestamp_ms, 1000);
    }

    #[test]
    fn weighted_vote_positive_negative() {
        let yes = WeightedVote::new(1, test_node_id(), VoteType::Yes, 5, 1000);
        assert!(yes.is_positive());
        assert!(!yes.is_negative());
        let no = WeightedVote::new(1, test_node_id(), VoteType::No, 5, 1000);
        assert!(!no.is_positive());
        assert!(no.is_negative());
    }

    // === ProposalBatch tests ===
    #[test]
    fn proposal_batch_add_and_count() {
        let mut batch = ProposalBatch::new(1, 1000);
        batch.add_proposal(10);
        batch.add_proposal(20);
        batch.add_proposal(30);
        assert_eq!(batch.proposal_count(), 3);
    }

    #[test]
    fn proposal_batch_completion_rate() {
        let mut batch = ProposalBatch::new(1, 1000);
        batch.mark_committed();
        batch.mark_committed();
        batch.mark_aborted();
        assert_eq!(batch.committed_count, 2);
        assert_eq!(batch.aborted_count, 1);
        assert!((batch.completion_rate() - (2.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn proposal_batch_complete() {
        let mut batch = ProposalBatch::new(1, 1000);
        assert!(!batch.completed);
        batch.complete();
        assert!(batch.completed);
    }

    // === QuorumSnapshot tests ===
    #[test]
    fn quorum_snapshot_new_and_update() {
        let mut snap = QuorumSnapshot::new(1, 0, 1000);
        snap.update_stats(10, 7, 2, 15);
        assert_eq!(snap.total_proposals, 10);
        assert_eq!(snap.committed_proposals, 7);
        assert_eq!(snap.aborted_proposals, 2);
        assert_eq!(snap.total_votes, 15);
    }

    #[test]
    fn quorum_snapshot_commit_rate() {
        let mut snap = QuorumSnapshot::new(1, 0, 1000);
        snap.update_stats(10, 7, 2, 15);
        assert!((snap.commit_rate() - 0.7).abs() < 0.01);
        let snap = QuorumSnapshot::new(2, 0, 2000);
        assert_eq!(snap.commit_rate(), 1.0);
    }

    // === QuorumProposalRecord tests ===
    #[test]
    fn quorum_proposal_record_new() {
        let record = QuorumProposalRecord::new(1, test_node_id(), ProposalState::Committed, 0);
        assert_eq!(record.proposal_id, 1);
        assert_eq!(record.proposer, test_node_id());
        assert_eq!(record.state, ProposalState::Committed);
        assert_eq!(record.net_weight(), 0);
    }

    #[test]
    fn quorum_proposal_record_update_weights() {
        let mut record = QuorumProposalRecord::new(1, test_node_id(), ProposalState::Voting, 0);
        record.update_weights(10, 5, 2, 17);
        assert_eq!(record.yes_weight, 10);
        assert_eq!(record.no_weight, 5);
        assert_eq!(record.abstain_weight, 2);
        assert_eq!(record.total_voters, 17);
        assert_eq!(record.net_weight(), 5);
    }

    #[test]
    fn quorum_proposal_record_mark_decided() {
        let mut record = QuorumProposalRecord::new(1, test_node_id(), ProposalState::Proposed, 0);
        assert!(record.decided_at_ms.is_none());
        record.mark_decided(5000);
        assert_eq!(record.decided_at_ms, Some(5000));
    }

    #[test]
    fn quorum_proposal_record_participation_rate() {
        let mut record = QuorumProposalRecord::new(1, test_node_id(), ProposalState::Voting, 0);
        record.update_weights(10, 5, 5, 100);
        assert!((record.participation_rate() - 0.2).abs() < 0.01);
        let record = QuorumProposalRecord::new(2, test_node_id(), ProposalState::Voting, 0);
        assert_eq!(record.participation_rate(), 0.0);
    }

    // === VoteReceipt tests ===
    #[test]
    fn vote_receipt_new_and_acknowledge() {
        let mut receipt = VoteReceipt::new(1, 10, test_node_id(), VoteType::Yes, 5, 1000);
        assert_eq!(receipt.receipt_id, 1);
        assert_eq!(receipt.proposal_id, 10);
        assert!(!receipt.is_acknowledged());
        receipt.acknowledge();
        assert!(receipt.is_acknowledged());
    }

    // === QuorumDecisionLog tests ===
    #[test]
    fn quorum_decision_log_append_and_count() {
        let log = QuorumDecisionLog::new(1);
        let rec1 = QuorumProposalRecord::new(1, test_node_id(), ProposalState::Committed, 0);
        let rec2 = QuorumProposalRecord::new(2, test_peer_id(), ProposalState::Aborted, 0);
        log.append(rec1);
        log.append(rec2);
        assert_eq!(log.entry_count(), 2);
        assert_eq!(log.total_entries, 2);
        assert_eq!(log.committed_count(), 1);
        assert_eq!(log.aborted_count(), 1);
        assert_eq!(log.pending_count(), 0);
    }

    #[test]
    fn quorum_decision_log_get_entry() {
        let log = QuorumDecisionLog::new(1);
        log.append(QuorumProposalRecord::new(1, test_node_id(), ProposalState::Committed, 0));
        let entry = log.get_entry(0).unwrap();
        assert_eq!(entry.proposal_id, 1);
        assert_eq!(log.get_entry(999), None);
    }

    #[test]
    fn quorum_decision_log_commit_rate() {
        let log = QuorumDecisionLog::new(1);
        log.append(QuorumProposalRecord::new(1, test_node_id(), ProposalState::Committed, 0));
        log.append(QuorumProposalRecord::new(2, test_peer_id(), ProposalState::Aborted, 0));
        assert!((log.commit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn quorum_decision_log_clear() {
        let log = QuorumDecisionLog::new(1);
        log.append(QuorumProposalRecord::new(1, test_node_id(), ProposalState::Committed, 0));
        assert_eq!(log.entry_count(), 1);
        log.clear();
        assert_eq!(log.entry_count(), 0);
    }

    // === QuorumEngine extended tests ===
    #[test]
    fn quorum_engine_expire_proposal() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.expire_proposal(1).unwrap();
        assert_eq!(engine.proposal_state(1).unwrap(), ProposalState::Expired);
        assert_eq!(engine.total_expired(), 1);
    }

    #[test]
    fn quorum_engine_expire_already_decided() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.commit_proposal(1).unwrap();
        let err = engine.expire_proposal(1).unwrap_err();
        assert!(matches!(err, QuorumError::ProposalAlreadyDecided { .. }));
    }

    #[test]
    fn quorum_engine_create_proposal_with_group() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        let proposal = engine.create_proposal_with_group(2, test_node_id(), vec![1, 2], 1).unwrap();
        assert_eq!(proposal.proposal_id, 2);
        assert_eq!(engine.total_proposals(), 2);
    }

    #[test]
    fn quorum_engine_proposal_payload_size() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(engine.proposal_payload_size(1).unwrap(), 5);
    }

    #[test]
    fn quorum_engine_total_proposal_payload_bytes() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![1, 2, 3]).unwrap();
        engine.create_proposal(2, test_node_id(), vec![4, 5]).unwrap();
        assert_eq!(engine.total_proposal_payload_bytes(), 5);
    }

    #[test]
    fn quorum_engine_has_config() {
        let engine = QuorumEngine::default();
        assert!(!engine.has_config(1));
        engine.configure_quorum(test_quorum_config()).unwrap();
        assert!(engine.has_config(1));
    }

    #[test]
    fn quorum_engine_tracked_configs() {
        let engine = QuorumEngine::default();
        assert_eq!(engine.tracked_configs().len(), 0);
        engine.configure_quorum(test_quorum_config()).unwrap();
        assert_eq!(engine.tracked_configs(), vec![1]);
    }

    #[test]
    fn quorum_engine_committed_proposals() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.create_proposal(2, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        engine.commit_proposal(1).unwrap();
        let committed = engine.committed_proposals();
        assert_eq!(committed, vec![1]);
    }

    #[test]
    fn quorum_engine_aborted_proposals() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.create_proposal(2, test_node_id(), vec![]).unwrap();
        engine.abort_proposal(1).unwrap();
        let aborted = engine.aborted_proposals();
        assert_eq!(aborted, vec![1]);
    }

    #[test]
    fn quorum_engine_expired_proposals() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.create_proposal(2, test_node_id(), vec![]).unwrap();
        engine.expire_proposal(1).unwrap();
        let expired = engine.expired_proposals();
        assert_eq!(expired, vec![1]);
    }

    #[test]
    fn quorum_engine_pending_proposals() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.create_proposal(2, test_node_id(), vec![]).unwrap();
        engine.abort_proposal(1).unwrap();
        let pending = engine.pending_proposals();
        assert_eq!(pending, vec![2]);
    }

    #[test]
    fn quorum_engine_proposals_by_state() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.abort_proposal(1).unwrap();
        let aborted = engine.proposals_by_state(ProposalState::Aborted);
        assert_eq!(aborted, vec![1]);
        let proposed = engine.proposals_by_state(ProposalState::Proposed);
        assert!(proposed.is_empty());
    }

    #[test]
    fn quorum_engine_take_snapshot() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        let snapshot = engine.take_snapshot(1);
        assert_eq!(snap.snapshot_id, 1);
        assert_eq!(snap.total_proposals, 1);
        assert_eq!(snap.view, 0);
    }

    #[test]
    fn quorum_engine_reset_stats() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        assert_eq!(engine.total_proposals(), 1);
        engine.reset_stats();
        assert_eq!(engine.total_proposals(), 0);
    }

    #[test]
    fn quorum_engine_health_and_rates() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        assert!((engine.commit_rate() - 1.0).abs() < 0.01);
        assert!((engine.abort_rate() - 0.0).abs() < 0.01);
        assert!((engine.quorum_rate() - 0.0).abs() < 0.01);
        let score = engine.health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn quorum_engine_create_vote_receipt() {
        let engine = QuorumEngine::default();
        let receipt = engine.create_vote_receipt(1, 10, test_node_id(), VoteType::Yes, 5, 1000);
        assert_eq!(receipt.receipt_id, 1);
        assert_eq!(receipt.proposal_id, 10);
        assert_eq!(receipt.voter, test_node_id());
        assert_eq!(receipt.weight, 5);
    }

    #[test]
    fn quorum_engine_proposal_record() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        let record = engine.proposal_record(1, 0).unwrap();
        assert_eq!(record.proposal_id, 1);
        assert_eq!(record.proposer, test_node_id());
    }

    #[test]
    fn quorum_engine_all_committed_records() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        engine.commit_proposal(1).unwrap();
        let records = engine.all_committed_records(0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal_id, 1);
    }

    #[test]
    fn quorum_engine_all_aborted_records() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.abort_proposal(1).unwrap();
        let records = engine.all_aborted_records(0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal_id, 1);
    }

    #[test]
    fn quorum_engine_is_expired() {
        let engine = QuorumEngine::default();
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        assert!(!engine.is_expired(1).unwrap());
        engine.expire_proposal(1).unwrap();
        assert!(engine.is_expired(1).unwrap());
    }

    #[test]
    fn quorum_engine_proposal_count_and_config_count() {
        let engine = QuorumEngine::default();
        assert_eq!(engine.proposal_count(), 0);
        assert_eq!(engine.config_count(), 0);
        engine.configure_quorum(test_quorum_config()).unwrap();
        assert_eq!(engine.config_count(), 1);
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        assert_eq!(engine.proposal_count(), 1);
    }

    #[test]
    fn quorum_engine_certificate_count() {
        let engine = QuorumEngine::default();
        assert_eq!(engine.certificate_count(), 0);
        engine.configure_quorum(test_quorum_config()).unwrap();
        engine.create_proposal(1, test_node_id(), vec![]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::Yes, 1, 2000)).unwrap();
        engine.commit_proposal(1).unwrap();
        engine.generate_certificate(1).unwrap();
        assert_eq!(engine.certificate_count(), 1);
    }

    // === Integration: weighted quorum flow ===
    #[test]
    fn quorum_weighted_flow() {
        let engine = QuorumEngine::default();

        // Configure weighted quorum
        let config = QuorumConfig::new(
            1,
            "weighted".to_string(),
            vec![test_node_id(), test_peer_id(), test_peer3_id()],
            QuorumThreshold::Weighted(0.6),
        ).with_leader(test_node_id());
        engine.configure_quorum(config).unwrap();

        // Create and vote on proposal
        engine.create_proposal(1, test_node_id(), vec![1, 2, 3]).unwrap();
        engine.cast_vote(1, test_peer_id(), Vote::new(test_peer_id(), VoteType::Yes, 1, 1000)).unwrap();
        engine.cast_vote(1, test_peer3_id(), Vote::new(test_peer3_id(), VoteType::No, 1, 2000)).unwrap();

        // Tallies
        let (yes, no, _) = engine.vote_tally(1).unwrap();
        assert_eq!(yes, 1);
        assert_eq!(no, 1);

        // Commit
        engine.commit_proposal(1).unwrap();
        assert_eq!(engine.proposal_state(1).unwrap(), ProposalState::Committed);

        // Certificate
        let cert = engine.generate_certificate(1).unwrap();
        assert!(cert.is_valid());

        // View change
        engine.view_change(1, test_peer_id()).unwrap();
        assert_eq!(engine.current_view().unwrap(), 1);

        // Snapshot
        let snapshot = engine.take_snapshot(1);
        assert_eq!(snap.total_proposals, 1);
        assert_eq!(snap.committed_proposals, 1);

        // Final stats
        let s = engine.stats();
        assert_eq!(s.total_proposals, 1);
        assert_eq!(s.total_committed, 1);
        assert_eq!(s.total_view_changes, 1);
        let score = engine.health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === NodeId zero peer test ===
    #[test]
    fn node_id_zero_peer_quorum() {
        let zero = NodeId::new([0u8; 32]);
        assert_ne!(test_node_id(), zero);
    }

    // === Endpoint clone test ===
    #[test]
    fn endpoint_clone_quorum() {
        let ep1 = test_endpoint();
        let ep2 = ep1.clone();
        assert_eq!(ep1.carrier_type, ep2.carrier_type);
        assert_eq!(ep1.address, ep2.address);
    }

    // === FailureDomain clone test ===
    #[test]
    fn failure_domain_clone_quorum() {
        let fd1 = test_failure_domain();
        let fd2 = fd1.clone();
        assert_eq!(fd1.power, fd2.power);
        assert_eq!(fd1.facility, fd2.facility);
    }

    // === QuorumGroup clone test ===
    #[test]
    fn quorum_group_clone() {
        let g1 = QuorumGroup::new(1, "g".to_string(), QuorumThreshold::SimpleMajority, 1000);
        let g2 = g1.clone();
        assert_eq!(g1.group_id, g2.group_id);
        assert_eq!(g1.name, g2.name);
    }

    // === ProposalBatch clone test ===
    #[test]
    fn proposal_batch_clone() {
        let b1 = ProposalBatch::new(1, 1000);
        let b2 = b1.clone();
        assert_eq!(b1.batch_id, b2.batch_id);
    }

    // === QuorumSnapshot clone test ===
    #[test]
    fn quorum_snapshot_clone() {
        let s1 = QuorumSnapshot::new(1, 0, 1000);
        let s2 = s1.clone();
        assert_eq!(s1.snapshot_id, s2.snapshot_id);
    }

    // === VoteReceipt clone test ===
    #[test]
    fn vote_receipt_clone() {
        let r1 = VoteReceipt::new(1, 10, test_node_id(), VoteType::Yes, 5, 1000);
        let r2 = r1.clone();
        assert_eq!(r1.receipt_id, r2.receipt_id);
    }
}