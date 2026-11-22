//! # Sealed Trait: Byzantine — Byzantine fault tolerance and malicious node handling
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Byzantine trait defines Byzantine fault tolerance primitives:
//! - Byzantine node detection via behavior scoring and anomaly thresholds
//! - Quarantine and isolation of suspected Byzantine peers
//! - Message integrity verification via signatures and challenge-response
//! - Byzantine agreement protocol (vote-based, multiple rounds)
//! - Double-spend and double-vote detection
//! - Slashing and reputation tracking for Byzantine participants
//! - Recovery and reintegration of quarantined nodes
//! - Byzantine state machine (healthy, suspected, quarantined, evicted, recovered)
//! - Byzantine stats and health monitoring
//! Implementations live downstream in ids-byzantine and ids-* crates.

use crate::types::{
    AuthToken, Capacity, Credential, DatasetId, Endpoint, FailureDomain,
    KeyId, NodeId, RegionId, Signature,
};
use crate::errors::ByzantineError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Byzantine
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::ByzantineEngine {}
impl private::Sealed for super::ByzantineConfig {}
impl private::Sealed for super::ByzantineState {}
impl private::Sealed for super::ByzantineNode {}
impl private::Sealed for super::ByzantineVote {}
impl private::Sealed for super::ByzantineRound {}
impl private::Sealed for super::ByzantineChallenge {}
impl private::Sealed for super::ByzantineResponse {}
impl private::Sealed for super::ByzantineCertificate {}
impl private::Sealed for super::ByzantineReputation {}
impl private::Sealed for super::ByzantineQuarantine {}
impl private::Sealed for super::ByzantineSlashRecord {}
impl private::Sealed for super::ByzantineStats {}
impl private::Sealed for super::ByzantineMonitor {}
impl private::Sealed for super::ByzantineValidator {}

// Sealed trait for Byzantine fault tolerance.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Byzantine: private::Sealed {
    /// Configure Byzantine fault tolerance parameters.
    fn configure(&self, config: ByzantineConfig) -> Result<(), ByzantineError>;

    /// Register a node for Byzantine monitoring.
    fn register_node(&self, node_id: NodeId, endpoint: Endpoint) -> Result<(), ByzantineError>;

    /// Update a node's behavior score based on observed actions.
    fn update_behavior_score(&self, node_id: NodeId, delta: i64, reason: &str) -> Result<i64, ByzantineError>;

    /// Get the current behavior score for a node.
    fn behavior_score(&self, node_id: NodeId) -> Result<i64, ByzantineError>;

    /// Mark a node as suspected Byzantine.
    fn suspect(&self, node_id: NodeId, evidence: Vec<u8>) -> Result<(), ByzantineError>;

    /// Quarantine a node (prevents participation).
    fn quarantine(&self, node_id: NodeId) -> Result<(), ByzantineError>;

    /// Evict a node permanently.
    fn evict(&self, node_id: NodeId) -> Result<(), ByzantineError>;

    /// Reintegrate a quarantined node after recovery.
    fn reintegrate(&self, node_id: NodeId) -> Result<(), ByzantineError>;

    /// Check if a node is Byzantine (suspected/quarantined/evicted).
    fn is_byzantine(&self, node_id: NodeId) -> Result<bool, ByzantineError>;

    /// Check if a node is quarantined.
    fn is_quarantined(&self, node_id: NodeId) -> Result<bool, ByzantineError>;

    /// Start a Byzantine agreement round.
    fn start_round(&self, round_id: u64, proposer: NodeId, payload: Vec<u8>) -> Result<ByzantineRound, ByzantineError>;

    /// Cast a vote in a Byzantine agreement round.
    fn cast_byzantine_vote(&self, round_id: u64, voter: NodeId, vote: ByzantineVote) -> Result<(), ByzantineError>;

    /// Check if Byzantine agreement is reached for a round.
    fn is_byzantine_agreement_reached(&self, round_id: u64) -> Result<bool, ByzantineError>;

    /// Commit a Byzantine agreement round.
    fn commit_round(&self, round_id: u64) -> Result<(), ByzantineError>;

    /// Issue a challenge to a node to verify liveness/integrity.
    fn issue_challenge(&self, node_id: NodeId, nonce: Vec<u8>) -> Result<ByzantineChallenge, ByzantineError>;

    /// Verify a response to a challenge.
    fn verify_response(&self, node_id: NodeId, response: ByzantineResponse) -> Result<bool, ByzantineError>;

    /// Detect double-vote by a node in the same round.
    fn detect_double_vote(&self, node_id: NodeId, round_id: u64) -> Result<bool, ByzantineError>;

    /// Slash a node's reputation for Byzantine behavior.
    fn slash(&self, node_id: NodeId, amount: u64, reason: &str) -> Result<ByzantineSlashRecord, ByzantineError>;

    /// Get reputation for a node.
    fn reputation(&self, node_id: NodeId) -> Result<ByzantineReputation, ByzantineError>;

    /// Generate a Byzantine certificate for an agreed round.
    fn generate_certificate(&self, round_id: u64) -> Result<ByzantineCertificate, ByzantineError>;

    /// Verify a Byzantine certificate.
    fn verify_certificate(&self, certificate: &ByzantineCertificate) -> Result<bool, ByzantineError>;

    /// Get Byzantine statistics.
    fn stats(&self) -> ByzantineStats;

    /// Check node health score.
    fn health_score(&self, node_id: NodeId) -> Result<f64, ByzantineError>;
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for Byzantine fault tolerance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineConfig {
    /// Suspicion threshold (nodes below this score are suspected).
    pub suspicion_threshold: i64,
    /// Quarantine threshold (nodes below this score are quarantined).
    pub quarantine_threshold: i64,
    /// Eviction threshold (nodes below this score are evicted).
    pub eviction_threshold: i64,
    /// Recovery threshold (nodes above this score can be reintegrated).
    pub recovery_threshold: i64,
    /// Maximum number of rounds for Byzantine agreement.
    pub max_rounds: u64,
    /// Quorum threshold for Byzantine agreement.
    pub agreement_quorum: f64,
    /// Challenge timeout in milliseconds.
    pub challenge_timeout_ms: u64,
    /// Reputation decay factor per hour.
    pub reputation_decay_factor: f64,
    /// Enable double-vote detection.
    pub enable_double_vote_detection: bool,
    /// Enable challenge-response verification.
    pub enable_challenge_response: bool,
}

impl Default for ByzantineConfig {
    fn default() -> Self {
        Self {
            suspicion_threshold: -10,
            quarantine_threshold: -50,
            eviction_threshold: -100,
            recovery_threshold: 0,
            max_rounds: 5,
            agreement_quorum: 0.67,
            challenge_timeout_ms: 30000,
            reputation_decay_factor: 0.95,
            enable_double_vote_detection: true,
            enable_challenge_response: true,
        }
    }
}

impl ByzantineConfig {
    /// Create a new ByzantineConfig with custom suspicion threshold.
    pub fn with_suspicion_threshold(mut self, threshold: i64) -> Self {
        self.suspicion_threshold = threshold;
        self
    }

    /// Create a new ByzantineConfig with custom quarantine threshold.
    pub fn with_quarantine_threshold(mut self, threshold: i64) -> Self {
        self.quarantine_threshold = threshold;
        self
    }

    /// Create a new ByzantineConfig with custom eviction threshold.
    pub fn with_eviction_threshold(mut self, threshold: i64) -> Self {
        self.eviction_threshold = threshold;
        self
    }

    /// Create a new ByzantineConfig with custom recovery threshold.
    pub fn with_recovery_threshold(mut self, threshold: i64) -> Self {
        self.recovery_threshold = threshold;
        self
    }

    /// Create a new ByzantineConfig with custom max rounds.
    pub fn with_max_rounds(mut self, rounds: u64) -> Self {
        self.max_rounds = rounds;
        self
    }

    /// Create a new ByzantineConfig with custom agreement quorum.
    pub fn with_agreement_quorum(mut self, quorum: f64) -> Self {
        self.agreement_quorum = quorum.clamp(0.0, 1.0);
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ByzantineError> {
        if self.suspicion_threshold > self.quarantine_threshold {
            return Err(ByzantineError::InvalidConfig {
                field: "suspicion_threshold".to_string(),
                reason: "suspicion threshold must be <= quarantine threshold".to_string(),
            });
        }
        if self.quarantine_threshold > self.eviction_threshold {
            return Err(ByzantineError::InvalidConfig {
                field: "quarantine_threshold".to_string(),
                reason: "quarantine threshold must be <= eviction threshold".to_string(),
            });
        }
        if self.recovery_threshold < self.quarantine_threshold {
            return Err(ByzantineError::InvalidConfig {
                field: "recovery_threshold".to_string(),
                reason: "recovery threshold must be >= quarantine threshold".to_string(),
            });
        }
        if self.agreement_quorum <= 0.0 || self.agreement_quorum > 1.0 {
            return Err(ByzantineError::InvalidConfig {
                field: "agreement_quorum".to_string(),
                reason: "agreement quorum must be in (0, 1]".to_string(),
            });
        }
        if self.max_rounds == 0 {
            return Err(ByzantineError::InvalidConfig {
                field: "max_rounds".to_string(),
                reason: "max rounds must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a node in the Byzantine fault tolerance system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByzantineState {
    /// Node is healthy and fully participating.
    Healthy,
    /// Node is suspected of Byzantine behavior.
    Suspected,
    /// Node is quarantined and cannot participate.
    Quarantined,
    /// Node is evicted permanently.
    Evicted,
    /// Node is recovering and can reintegrate.
    Recovering,
}

impl ByzantineState {
    /// Check if the state allows participation.
    pub fn can_participate(&self) -> bool {
        matches!(self, ByzantineState::Healthy | ByzantineState::Recovering)
    }

    /// Check if the state is final (no recovery possible).
    pub fn is_final(&self) -> bool {
        matches!(self, ByzantineState::Evicted)
    }

    /// Check if the state is quarantined or evicted.
    pub fn is_isolated(&self) -> bool {
        matches!(self, ByzantineState::Quarantined | ByzantineState::Evicted)
    }
}

impl Default for ByzantineState {
    fn default() -> Self {
        ByzantineState::Healthy
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE NODE
///////////////////////////////////////////////////////////////////////////////

/// A node tracked by the Byzantine fault tolerance system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineNode {
    /// Node identifier.
    pub node_id: NodeId,
    /// Node endpoint (carrier-agnostic).
    pub endpoint: Endpoint,
    /// Current Byzantine state.
    pub state: ByzantineState,
    /// Behavior score (higher = more trustworthy).
    pub behavior_score: i64,
    /// Reputation score (0.0 to 1.0).
    pub reputation: f64,
    /// Total votes cast.
    pub total_votes: u64,
    /// Total votes that were correct (agreed with majority).
    pub correct_votes: u64,
    /// Total double votes detected.
    pub double_votes: u64,
    /// Total challenges failed.
    pub failed_challenges: u64,
    /// Timestamp of last activity in milliseconds.
    pub last_activity_ms: u64,
    /// Timestamp when state was last changed.
    pub state_changed_at_ms: u64,
    /// Evidence collected against this node.
    pub evidence: Vec<Vec<u8>>,
}

impl ByzantineNode {
    /// Create a new ByzantineNode.
    pub fn new(node_id: NodeId, endpoint: Endpoint) -> Self {
        Self {
            node_id,
            endpoint,
            state: ByzantineState::Healthy,
            behavior_score: 0,
            reputation: 1.0,
            total_votes: 0,
            correct_votes: 0,
            double_votes: 0,
            failed_challenges: 0,
            last_activity_ms: 0,
            state_changed_at_ms: 0,
            evidence: Vec::new(),
        }
    }

    /// Update behavior score and potentially change state.
    pub fn update_score(&mut self, delta: i64, reason: &str) -> i64 {
        self.behavior_score = self.behavior_score.saturating_add(delta);
        self.last_activity_ms = crate::types::IdsTime::now_ms();
        self.evidence.push(reason.as_bytes().to_vec());
        self.behavior_score
    }

    /// Check if node should be suspected based on score.
    pub fn should_suspect(&self, threshold: i64) -> bool {
        self.behavior_score < threshold && !self.state.is_isolated()
    }

    /// Check if node should be quarantined based on score.
    pub fn should_quarantine(&self, threshold: i64) -> bool {
        self.behavior_score < threshold && self.state != ByzantineState::Evicted
    }

    /// Check if node should be evicted based on score.
    pub fn should_evict(&self, threshold: i64) -> bool {
        self.behavior_score < threshold
    }

    /// Check if node can be reintegrated based on score.
    pub fn can_reintegrate(&self, threshold: i64) -> bool {
        self.behavior_score >= threshold && self.state == ByzantineState::Quarantined
    }

    /// Vote accuracy ratio.
    pub fn vote_accuracy(&self) -> f64 {
        if self.total_votes == 0 {
            return 1.0;
        }
        self.correct_votes as f64 / self.total_votes as f64
    }

    /// Add evidence against this node.
    pub fn add_evidence(&mut self, evidence: Vec<u8>) {
        self.evidence.push(evidence);
    }

    /// Clear evidence.
    pub fn clear_evidence(&mut self) {
        self.evidence.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE VOTE
///////////////////////////////////////////////////////////////////////////////

/// A vote in a Byzantine agreement round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByzantineVote {
    /// Agree with the proposal.
    Agree,
    /// Disagree with the proposal.
    Disagree,
    /// Abstain from voting.
    Abstain,
}

/// A recorded vote with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineVoteRecord {
    /// Round ID.
    pub round_id: u64,
    /// Voter node ID.
    pub voter: NodeId,
    /// Vote cast.
    pub vote: ByzantineVote,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Signature of the vote.
    pub signature: Option<Signature>,
}

impl ByzantineVoteRecord {
    /// Create a new ByzantineVoteRecord.
    pub fn new(round_id: u64, voter: NodeId, vote: ByzantineVote, timestamp_ms: u64) -> Self {
        Self {
            round_id,
            voter,
            vote,
            timestamp_ms,
            signature: None,
        }
    }

    /// Set the signature on this vote record.
    pub fn with_signature(mut self, signature: Signature) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Check if the vote is positive (Agree).
    pub fn is_agree(&self) -> bool {
        matches!(self.vote, ByzantineVote::Agree)
    }

    /// Check if the vote is negative (Disagree).
    pub fn is_disagree(&self) -> bool {
        matches!(self.vote, ByzantineVote::Disagree)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ROUND
///////////////////////////////////////////////////////////////////////////////

/// A Byzantine agreement round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineRound {
    /// Round identifier.
    pub round_id: u64,
    /// Proposer node ID.
    pub proposer: NodeId,
    /// Proposal payload.
    pub payload: Vec<u8>,
    /// Current state of the round.
    pub state: ByzantineRoundState,
    /// Votes collected.
    pub votes: Vec<ByzantineVoteRecord>,
    /// Number of agrees.
    pub agree_count: u64,
    /// Number of disagrees.
    pub disagree_count: u64,
    /// Number of abstains.
    pub abstain_count: u64,
    /// Timestamp round was started.
    pub started_at_ms: u64,
    /// Timestamp round was committed.
    pub committed_at_ms: Option<u64>,
    /// Number of rounds attempted.
    pub round_attempt: u64,
}

/// State of a Byzantine agreement round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByzantineRoundState {
    /// Round is open for voting.
    Open,
    /// Round has reached agreement.
    Agreed,
    /// Round has been committed.
    Committed,
    /// Round has been aborted.
    Aborted,
    /// Round timed out.
    TimedOut,
}

impl Default for ByzantineRoundState {
    fn default() -> Self {
        ByzantineRoundState::Open
    }
}

impl ByzantineRound {
    /// Create a new ByzantineRound.
    pub fn new(round_id: u64, proposer: NodeId, payload: Vec<u8>, started_at_ms: u64) -> Self {
        Self {
            round_id,
            proposer,
            payload,
            state: ByzantineRoundState::Open,
            votes: Vec::new(),
            agree_count: 0,
            disagree_count: 0,
            abstain_count: 0,
            started_at_ms,
            committed_at_ms: None,
            round_attempt: 1,
        }
    }

    /// Add a vote to the round.
    pub fn add_vote(&mut self, vote: ByzantineVoteRecord) {
        match vote.vote {
            ByzantineVote::Agree => self.agree_count += 1,
            ByzantineVote::Disagree => self.disagree_count += 1,
            ByzantineVote::Abstain => self.abstain_count += 1,
        }
        self.votes.push(vote);
    }

    /// Check if agreement threshold is met.
    pub fn agreement_met(&self, quorum: f64) -> bool {
        let total = self.agree_count + self.disagree_count + self.abstain_count;
        if total == 0 {
            return false;
        }
        (self.agree_count as f64 / total as f64) >= quorum
    }

    /// Total votes cast.
    pub fn total_votes(&self) -> u64 {
        self.agree_count + self.disagree_count + self.abstain_count
    }

    /// Check if round is decided (agreed, committed, aborted, or timed out).
    pub fn is_decided(&self) -> bool {
        matches!(
            self.state,
            ByzantineRoundState::Agreed
                | ByzantineRoundState::Committed
                | ByzantineRoundState::Aborted
                | ByzantineRoundState::TimedOut
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CHALLENGE AND RESPONSE
///////////////////////////////////////////////////////////////////////////////

/// A challenge issued to a node for liveness/integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineChallenge {
    /// Challenge identifier.
    pub challenge_id: u64,
    /// Target node ID.
    pub target_node: NodeId,
    /// Nonce that the node must sign/respond to.
    pub nonce: Vec<u8>,
    /// Timestamp when issued.
    pub issued_at_ms: u64,
    /// Expiration timestamp.
    pub expires_at_ms: u64,
    /// Whether the challenge has been responded to.
    pub responded: bool,
}

impl ByzantineChallenge {
    /// Create a new ByzantineChallenge.
    pub fn new(challenge_id: u64, target_node: NodeId, nonce: Vec<u8>, issued_at_ms: u64, expires_at_ms: u64) -> Self {
        Self {
            challenge_id,
            target_node,
            nonce,
            issued_at_ms,
            expires_at_ms,
            responded: false,
        }
    }

    /// Check if the challenge is expired.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.expires_at_ms
    }

    /// Mark as responded.
    pub fn mark_responded(&mut self) {
        self.responded = true;
    }
}

/// A response to a Byzantine challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineResponse {
    /// Challenge identifier being responded to.
    pub challenge_id: u64,
    /// Responding node ID.
    pub responder: NodeId,
    /// Response payload (typically a signature over the nonce).
    pub response: Vec<u8>,
    /// Timestamp when responded.
    pub responded_at_ms: u64,
    /// Whether the response was valid.
    pub valid: bool,
}

impl ByzantineResponse {
    /// Create a new ByzantineResponse.
    pub fn new(challenge_id: u64, responder: NodeId, response: Vec<u8>, responded_at_ms: u64) -> Self {
        Self {
            challenge_id,
            responder,
            response,
            responded_at_ms,
            valid: false,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CERTIFICATE
///////////////////////////////////////////////////////////////////////////////

/// A certificate proving Byzantine agreement was reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineCertificate {
    /// Round ID this certificate covers.
    pub round_id: u64,
    /// Proposer node ID.
    pub proposer: NodeId,
    /// Agreed payload hash.
    pub payload_hash: [u8; 32],
    /// Number of agreeing votes.
    pub agree_count: u64,
    /// Total voting nodes.
    pub total_voters: u64,
    /// Signatures from agreeing nodes.
    pub signatures: Vec<(NodeId, Signature)>,
    /// Timestamp certificate was generated.
    pub generated_at_ms: u64,
}

impl ByzantineCertificate {
    /// Create a new ByzantineCertificate.
    pub fn new(
        round_id: u64,
        proposer: NodeId,
        payload_hash: [u8; 32],
        agree_count: u64,
        total_voters: u64,
        generated_at_ms: u64,
    ) -> Self {
        Self {
            round_id,
            proposer,
            payload_hash,
            agree_count,
            total_voters,
            signatures: Vec::new(),
            generated_at_ms,
        }
    }

    /// Add a signature to the certificate.
    pub fn add_signature(&mut self, node_id: NodeId, signature: Signature) {
        self.signatures.push((node_id, signature));
    }

    /// Check if certificate has enough signatures for quorum.
    pub fn has_quorum(&self, quorum: f64) -> bool {
        if self.total_voters == 0 {
            return false;
        }
        (self.signatures.len() as f64 / self.total_voters as f64) >= quorum
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE REPUTATION
///////////////////////////////////////////////////////////////////////////////

/// Reputation tracking for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineReputation {
    /// Node identifier.
    pub node_id: NodeId,
    /// Current reputation score (0.0 to 1.0).
    pub score: f64,
    /// Total positive interactions.
    pub positive_interactions: u64,
    /// Total negative interactions.
    pub negative_interactions: u64,
    /// Total slashes applied.
    pub total_slashes: u64,
    /// Total amount slashed.
    pub total_slashed: u64,
    /// Timestamp of last reputation update.
    pub last_updated_ms: u64,
}

impl Default for ByzantineReputation {
    fn default() -> Self {
        Self {
            node_id: NodeId::ZERO,
            score: 1.0,
            positive_interactions: 0,
            negative_interactions: 0,
            total_slashes: 0,
            total_slashed: 0,
            last_updated_ms: 0,
        }
    }
}

impl ByzantineReputation {
    /// Create a new ByzantineReputation.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            ..Default::default()
        }
    }

    /// Record a positive interaction.
    pub fn record_positive(&mut self, timestamp_ms: u64) {
        self.positive_interactions += 1;
        self.score = (self.score + 0.01).min(1.0);
        self.last_updated_ms = timestamp_ms;
    }

    /// Record a negative interaction.
    pub fn record_negative(&mut self, timestamp_ms: u64) {
        self.negative_interactions += 1;
        self.score = (self.score - 0.05).max(0.0);
        self.last_updated_ms = timestamp_ms;
    }

    /// Apply decay to reputation over time.
    pub fn decay(&mut self, factor: f64, timestamp_ms: u64) {
        self.score = (self.score * factor).max(0.0).min(1.0);
        self.last_updated_ms = timestamp_ms;
    }

    /// Interaction ratio (positive / total).
    pub fn interaction_ratio(&self) -> f64 {
        let total = self.positive_interactions + self.negative_interactions;
        if total == 0 {
            return 1.0;
        }
        self.positive_interactions as f64 / total as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE QUARANTINE
///////////////////////////////////////////////////////////////////////////////

/// Quarantine record for an isolated node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineQuarantine {
    /// Node identifier.
    pub node_id: NodeId,
    /// Timestamp when quarantined.
    pub quarantined_at_ms: u64,
    /// Reason for quarantine.
    pub reason: String,
    /// Evidence supporting quarantine.
    pub evidence: Vec<Vec<u8>>,
    /// Whether quarantine has been reviewed.
    pub reviewed: bool,
    /// Reviewer node ID (if reviewed).
    pub reviewer: Option<NodeId>,
}

impl ByzantineQuarantine {
    /// Create a new ByzantineQuarantine.
    pub fn new(node_id: NodeId, quarantined_at_ms: u64, reason: String) -> Self {
        Self {
            node_id,
            quarantined_at_ms,
            reason,
            evidence: Vec::new(),
            reviewed: false,
            reviewer: None,
        }
    }

    /// Add evidence to the quarantine record.
    pub fn add_evidence(&mut self, evidence: Vec<u8>) {
        self.evidence.push(evidence);
    }

    /// Mark as reviewed.
    pub fn mark_reviewed(&mut self, reviewer: NodeId) {
        self.reviewed = true;
        self.reviewer = Some(reviewer);
    }

    /// Duration of quarantine in milliseconds.
    pub fn duration_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.quarantined_at_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE SLASH RECORD
///////////////////////////////////////////////////////////////////////////////

/// A record of reputation slashing applied to a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineSlashRecord {
    /// Slash identifier.
    pub slash_id: u64,
    /// Node that was slashed.
    pub node_id: NodeId,
    /// Amount slashed.
    pub amount: u64,
    /// Reason for slashing.
    pub reason: String,
    /// Timestamp of slash.
    pub slashed_at_ms: u64,
    /// Node that initiated the slash.
    pub slasher: NodeId,
}

impl ByzantineSlashRecord {
    /// Create a new ByzantineSlashRecord.
    pub fn new(slash_id: u64, node_id: NodeId, amount: u64, reason: String, slashed_at_ms: u64, slasher: NodeId) -> Self {
        Self {
            slash_id,
            node_id,
            amount,
            reason,
            slashed_at_ms,
            slasher,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE STATS
///////////////////////////////////////////////////////////////////////////////

/// Aggregate statistics for Byzantine fault tolerance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineStats {
    /// Total nodes registered.
    pub total_nodes: u64,
    /// Total healthy nodes.
    pub healthy_nodes: u64,
    /// Total suspected nodes.
    pub suspected_nodes: u64,
    /// Total quarantined nodes.
    pub quarantined_nodes: u64,
    /// Total evicted nodes.
    pub evicted_nodes: u64,
    /// Total recovering nodes.
    pub recovering_nodes: u64,
    /// Total rounds started.
    pub total_rounds: u64,
    /// Total rounds committed.
    pub committed_rounds: u64,
    /// Total rounds aborted.
    pub aborted_rounds: u64,
    /// Total rounds timed out.
    pub timed_out_rounds: u64,
    /// Total votes cast.
    pub total_votes: u64,
    /// Total double votes detected.
    pub total_double_votes: u64,
    /// Total challenges issued.
    pub total_challenges: u64,
    /// Total challenges failed.
    pub total_challenges_failed: u64,
    /// Total slashes applied.
    pub total_slashes: u64,
    /// Total amount slashed.
    pub total_slashed: u64,
}

impl Default for ByzantineStats {
    fn default() -> Self {
        Self {
            total_nodes: 0,
            healthy_nodes: 0,
            suspected_nodes: 0,
            quarantined_nodes: 0,
            evicted_nodes: 0,
            recovering_nodes: 0,
            total_rounds: 0,
            committed_rounds: 0,
            aborted_rounds: 0,
            timed_out_rounds: 0,
            total_votes: 0,
            total_double_votes: 0,
            total_challenges: 0,
            total_challenges_failed: 0,
            total_slashes: 0,
            total_slashed: 0,
        }
    }
}

impl ByzantineStats {
    /// Byzantine ratio (quarantined + evicted / total).
    pub fn byzantine_ratio(&self) -> f64 {
        if self.total_nodes == 0 {
            return 0.0;
        }
        (self.quarantined_nodes + self.evicted_nodes) as f64 / self.total_nodes as f64
    }

    /// Commit rate (committed / total rounds).
    pub fn commit_rate(&self) -> f64 {
        if self.total_rounds == 0 {
            return 1.0;
        }
        self.committed_rounds as f64 / self.total_rounds as f64
    }

    /// Challenge failure rate.
    pub fn challenge_failure_rate(&self) -> f64 {
        if self.total_challenges == 0 {
            return 0.0;
        }
        self.total_challenges_failed as f64 / self.total_challenges as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Monitors node behavior and triggers Byzantine state transitions.
#[derive(Debug, Clone)]
pub struct ByzantineMonitor {
    /// Configuration.
    pub config: ByzantineConfig,
    /// Node states.
    pub nodes: std::cell::RefCell<std::collections::HashMap<NodeId, ByzantineNode>>,
    /// Quarantine records.
    pub quarantines: std::cell::RefCell<std::collections::HashMap<NodeId, ByzantineQuarantine>>,
    /// Reputation records.
    pub reputations: std::cell::RefCell<std::collections::HashMap<NodeId, ByzantineReputation>>,
    /// Slash records.
    pub slashes: std::cell::RefCell<Vec<ByzantineSlashRecord>>,
    /// Stats.
    pub stats: std::cell::RefCell<ByzantineStats>,
    /// Next slash ID.
    pub next_slash_id: std::cell::RefCell<u64>,
}

impl Default for ByzantineMonitor {
    fn default() -> Self {
        Self::new(ByzantineConfig::default())
    }
}

impl ByzantineMonitor {
    /// Create a new ByzantineMonitor.
    pub fn new(config: ByzantineConfig) -> Self {
        Self {
            config,
            nodes: std::cell::RefCell::new(std::collections::HashMap::new()),
            quarantines: std::cell::RefCell::new(std::collections::HashMap::new()),
            reputations: std::cell::RefCell::new(std::collections::HashMap::new()),
            slashes: std::cell::RefCell::new(Vec::new()),
            stats: std::cell::RefCell::new(ByzantineStats::default()),
            next_slash_id: std::cell::RefCell::new(1),
        }
    }

    /// Register a node.
    pub fn register_node(&self, node_id: NodeId, endpoint: Endpoint) {
        let mut nodes = self.nodes.borrow_mut();
        nodes.insert(
            node_id,
            ByzantineNode::new(node_id, endpoint),
        );
        let mut reputations = self.reputations.borrow_mut();
        reputations.insert(node_id, ByzantineReputation::new(node_id));
        let mut stats = self.stats.borrow_mut();
        stats.total_nodes += 1;
        stats.healthy_nodes += 1;
    }

    /// Update behavior score for a node.
    pub fn update_score(&self, node_id: NodeId, delta: i64, reason: &str) -> Result<i64, ByzantineError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        let new_score = node.update_score(delta, reason);
        drop(nodes);

        // Update reputation
        let mut reputations = self.reputations.borrow_mut();
        if let Some(rep) = reputations.get_mut(&node_id) {
            if delta > 0 {
                rep.record_positive(crate::types::IdsTime::now_ms());
            } else if delta < 0 {
                rep.record_negative(crate::types::IdsTime::now_ms());
            }
        }
        drop(reputations);

        // Check state transitions
        self._check_state_transitions(node_id);

        Ok(new_score)
    }

    /// Suspect a node.
    pub fn suspect(&self, node_id: NodeId, evidence: Vec<u8>) -> Result<(), ByzantineError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        if node.state == ByzantineState::Evicted {
            return Err(ByzantineError::NodeEvicted { node_id });
        }
        node.state = ByzantineState::Suspected;
        node.state_changed_at_ms = crate::types::IdsTime::now_ms();
        node.add_evidence(evidence);
        drop(nodes);

        let mut stats = self.stats.borrow_mut();
        stats.suspected_nodes += 1;
        stats.healthy_nodes = stats.healthy_nodes.saturating_sub(1);

        Ok(())
    }

    /// Quarantine a node.
    pub fn quarantine(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        if node.state == ByzantineState::Evicted {
            return Err(ByzantineError::NodeEvicted { node_id });
        }
        node.state = ByzantineState::Quarantined;
        node.state_changed_at_ms = crate::types::IdsTime::now_ms();
        drop(nodes);

        let mut quarantines = self.quarantines.borrow_mut();
        quarantines.insert(
            node_id,
            ByzantineQuarantine::new(node_id, crate::types::IdsTime::now_ms(), "quarantined".to_string()),
        );

        let mut stats = self.stats.borrow_mut();
        stats.quarantined_nodes += 1;
        stats.suspected_nodes = stats.suspected_nodes.saturating_sub(1);

        Ok(())
    }

    /// Evict a node.
    pub fn evict(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        node.state = ByzantineState::Evicted;
        node.state_changed_at_ms = crate::types::IdsTime::now_ms();
        drop(nodes);

        let mut stats = self.stats.borrow_mut();
        stats.evicted_nodes += 1;
        stats.quarantined_nodes = stats.quarantined_nodes.saturating_sub(1);
        stats.suspected_nodes = stats.suspected_nodes.saturating_sub(1);

        Ok(())
    }

    /// Reintegrate a quarantined node.
    pub fn reintegrate(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        let mut nodes = self.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        if node.state != ByzantineState::Quarantined {
            return Err(ByzantineError::NodeNotQuarantined { node_id });
        }
        node.state = ByzantineState::Recovering;
        node.state_changed_at_ms = crate::types::IdsTime::now_ms();
        drop(nodes);

        let mut stats = self.stats.borrow_mut();
        stats.recovering_nodes += 1;
        stats.quarantined_nodes = stats.quarantined_nodes.saturating_sub(1);

        Ok(())
    }

    /// Check if a node is Byzantine.
    pub fn is_byzantine(&self, node_id: NodeId) -> Result<bool, ByzantineError> {
        let nodes = self.nodes.borrow();
        let node = nodes.get(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        Ok(!node.state.can_participate())
    }

    /// Check if a node is quarantined.
    pub fn is_quarantined(&self, node_id: NodeId) -> Result<bool, ByzantineError> {
        let nodes = self.nodes.borrow();
        let node = nodes.get(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        Ok(node.state == ByzantineState::Quarantined)
    }

    /// Get behavior score.
    pub fn behavior_score(&self, node_id: NodeId) -> Result<i64, ByzantineError> {
        let nodes = self.nodes.borrow();
        let node = nodes.get(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        Ok(node.behavior_score)
    }

    /// Get reputation.
    pub fn reputation(&self, node_id: NodeId) -> Result<ByzantineReputation, ByzantineError> {
        let reputations = self.reputations.borrow();
        reputations.get(&node_id).cloned().ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })
    }

    /// Get stats.
    pub fn stats(&self) -> ByzantineStats {
        self.stats.borrow().clone()
    }

    /// Get health score for a node.
    pub fn health_score(&self, node_id: NodeId) -> Result<f64, ByzantineError> {
        let nodes = self.nodes.borrow();
        let node = nodes.get(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        let base = if node.behavior_score >= 0 { 1.0 } else { 0.5 };
        let reputation = self.reputations.borrow().get(&node_id).map(|r| r.score).unwrap_or(0.0);
        Ok((base + reputation) / 2.0)
    }

    fn _check_state_transitions(&self, node_id: NodeId) {
        // Internal: check if node should change state based on score
        let nodes = self.nodes.borrow();
        let node = match nodes.get(&node_id) {
            Some(n) => n.clone(),
            None => return,
        };
        drop(nodes);

        let mut need_update = false;
        let new_state = if node.behavior_score < self.config.eviction_threshold && node.state != ByzantineState::Evicted {
            need_update = true;
            ByzantineState::Evicted
        } else if node.behavior_score < self.config.quarantine_threshold && node.state == ByzantineState::Healthy {
            need_update = true;
            ByzantineState::Quarantined
        } else if node.behavior_score < self.config.suspicion_threshold && node.state == ByzantineState::Healthy {
            need_update = true;
            ByzantineState::Suspected
        } else {
            node.state
        };

        if need_update {
            drop(nodes);
            let mut nodes_mut = self.nodes.borrow_mut();
            if let Some(n) = nodes_mut.get_mut(&node_id) {
                n.state = new_state;
                n.state_changed_at_ms = crate::types::IdsTime::now_ms();
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates Byzantine operations (challenges, votes, certificates).
#[derive(Debug, Clone)]
pub struct ByzantineValidator {
    /// Enable double vote detection.
    pub enable_double_vote: bool,
    /// Enable challenge response.
    pub enable_challenge: bool,
    /// Challenges issued.
    pub challenges: std::cell::RefCell<std::collections::HashMap<u64, ByzantineChallenge>>,
    /// Responses received.
    pub responses: std::cell::RefCell<std::collections::HashMap<u64, ByzantineResponse>>,
    /// Votes per round per node (for double vote detection).
    pub votes_per_round: std::cell::RefCell<std::collections::HashMap<(u64, NodeId), ByzantineVote>>,
}

impl Default for ByzantineValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ByzantineValidator {
    /// Create a new ByzantineValidator.
    pub fn new() -> Self {
        Self {
            enable_double_vote: true,
            enable_challenge: true,
            challenges: std::cell::RefCell::new(std::collections::HashMap::new()),
            responses: std::cell::RefCell::new(std::collections::HashMap::new()),
            votes_per_round: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    /// Issue a challenge.
    pub fn issue_challenge(&self, challenge_id: u64, target_node: NodeId, nonce: Vec<u8>, issued_at_ms: u64, expires_at_ms: u64) -> ByzantineChallenge {
        let challenge = ByzantineChallenge::new(challenge_id, target_node, nonce, issued_at_ms, expires_at_ms);
        self.challenges.borrow_mut().insert(challenge_id, challenge.clone());
        challenge
    }

    /// Verify a response against a challenge.
    pub fn verify_response(&self, response: &ByzantineResponse) -> Result<bool, ByzantineError> {
        let challenges = self.challenges.borrow();
        let challenge = challenges.get(&response.challenge_id).ok_or_else(|| ByzantineError::ChallengeNotFound { challenge_id: response.challenge_id })?;
        if challenge.is_expired(crate::types::IdsTime::now_ms()) {
            return Ok(false);
        }
        if challenge.target_node != response.responder {
            return Ok(false);
        }
        if challenge.responded {
            return Ok(false);
        }
        Ok(!response.response.is_empty())
    }

    /// Record a response.
    pub fn record_response(&self, response: ByzantineResponse) {
        self.responses.borrow_mut().insert(response.challenge_id, response);
        if let Some(mut challenge) = self.challenges.borrow_mut().get_mut(&response.challenge_id) {
            challenge.mark_responded();
        }
    }

    /// Detect double vote.
    pub fn detect_double_vote(&self, round_id: u64, node_id: NodeId, vote: ByzantineVote) -> Result<bool, ByzantineError> {
        if !self.enable_double_vote {
            return Ok(false);
        }
        let mut votes = self.votes_per_round.borrow_mut();
        let key = (round_id, node_id);
        if let Some(existing) = votes.get(&key) {
            if *existing != vote {
                return Ok(true);
            }
            return Ok(false);
        }
        votes.insert(key, vote);
        Ok(false)
    }

    /// Clear expired challenges.
    pub fn clear_expired(&self, now_ms: u64) {
        let mut challenges = self.challenges.borrow_mut();
        challenges.retain(|_, c| !c.is_expired(now_ms));
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ENGINE
///////////////////////////////////////////////////////////////////////////////

/// The Byzantine fault tolerance engine — core implementation of the Byzantine trait.
#[derive(Debug, Clone)]
pub struct ByzantineEngine {
    /// Configuration.
    pub config: std::cell::RefCell<ByzantineConfig>,
    /// Monitor for node tracking.
    pub monitor: ByzantineMonitor,
    /// Validator for operations.
    pub validator: ByzantineValidator,
    /// Rounds in progress.
    pub rounds: std::cell::RefCell<std::collections::HashMap<u64, ByzantineRound>>,
    /// Certificates generated.
    pub certificates: std::cell::RefCell<std::collections::HashMap<u64, ByzantineCertificate>>,
    /// Next round ID.
    pub next_round_id: std::cell::RefCell<u64>,
    /// Next challenge ID.
    pub next_challenge_id: std::cell::RefCell<u64>,
}

impl Default for ByzantineEngine {
    fn default() -> Self {
        Self::new(ByzantineConfig::default())
    }
}

impl ByzantineEngine {
    /// Create a new ByzantineEngine with default config.
    pub fn new(config: ByzantineConfig) -> Self {
        config.validate().expect("Invalid ByzantineConfig");
        Self {
            config: std::cell::RefCell::new(config),
            monitor: ByzantineMonitor::new(config.clone()),
            validator: ByzantineValidator::new(),
            rounds: std::cell::RefCell::new(std::collections::HashMap::new()),
            certificates: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_round_id: std::cell::RefCell::new(1),
            next_challenge_id: std::cell::RefCell::new(1),
        }
    }

    /// Configure the engine.
    pub fn configure(&self, config: ByzantineConfig) -> Result<(), ByzantineError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        self.monitor = ByzantineMonitor::new(config);
        Ok(())
    }

    /// Register a node.
    pub fn register_node(&self, node_id: NodeId, endpoint: Endpoint) -> Result<(), ByzantineError> {
        self.monitor.register_node(node_id, endpoint);
        Ok(())
    }

    /// Update behavior score.
    pub fn update_behavior_score(&self, node_id: NodeId, delta: i64, reason: &str) -> Result<i64, ByzantineError> {
        self.monitor.update_score(node_id, delta, reason)
    }

    /// Get behavior score.
    pub fn behavior_score(&self, node_id: NodeId) -> Result<i64, ByzantineError> {
        self.monitor.behavior_score(node_id)
    }

    /// Suspect a node.
    pub fn suspect(&self, node_id: NodeId, evidence: Vec<u8>) -> Result<(), ByzantineError> {
        self.monitor.suspect(node_id, evidence)
    }

    /// Quarantine a node.
    pub fn quarantine(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        self.monitor.quarantine(node_id)
    }

    /// Evict a node.
    pub fn evict(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        self.monitor.evict(node_id)
    }

    /// Reintegrate a quarantined node.
    pub fn reintegrate(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        self.monitor.reintegrate(node_id)
    }

    /// Check if node is Byzantine.
    pub fn is_byzantine(&self, node_id: NodeId) -> Result<bool, ByzantineError> {
        self.monitor.is_byzantine(node_id)
    }

    /// Check if node is quarantined.
    pub fn is_quarantined(&self, node_id: NodeId) -> Result<bool, ByzantineError> {
        self.monitor.is_quarantined(node_id)
    }

    /// Start a Byzantine agreement round.
    pub fn start_round(&self, round_id: u64, proposer: NodeId, payload: Vec<u8>) -> Result<ByzantineRound, ByzantineError> {
        let rounds = self.rounds.borrow();
        if rounds.contains_key(&round_id) {
            return Err(ByzantineError::RoundAlreadyExists { round_id });
        }
        drop(rounds);

        let round = ByzantineRound::new(round_id, proposer, payload, crate::types::IdsTime::now_ms());
        self.rounds.borrow_mut().insert(round_id, round.clone());
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_rounds += 1;
        drop(stats);

        Ok(round)
    }

    /// Cast a vote in a Byzantine agreement round.
    pub fn cast_byzantine_vote(&self, round_id: u64, voter: NodeId, vote: ByzantineVote) -> Result<(), ByzantineError> {
        let mut rounds = self.rounds.borrow_mut();
        let round = rounds.get_mut(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if round.is_decided() {
            return Err(ByzantineError::RoundAlreadyDecided { round_id });
        }

        // Double vote detection
        let vote_type = match vote {
            ByzantineVote::Agree => crate::contracts::byzantine::ByzantineVote::Agree,
            ByzantineVote::Disagree => crate::contracts::byzantine::ByzantineVote::Disagree,
            ByzantineVote::Abstain => crate::contracts::byzantine::ByzantineVote::Abstain,
        };
        let _ = self.validator.detect_double_vote(round_id, voter, vote_type)?;

        let record = ByzantineVoteRecord::new(round_id, voter, vote, crate::types::IdsTime::now_ms());
        round.add_vote(record);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_votes += 1;
        drop(stats);

        Ok(())
    }

    /// Check if Byzantine agreement is reached.
    pub fn is_byzantine_agreement_reached(&self, round_id: u64) -> Result<bool, ByzantineError> {
        let rounds = self.rounds.borrow();
        let round = rounds.get(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        let config = self.config.borrow();
        Ok(round.agreement_met(config.agreement_quorum))
    }

    /// Commit a round.
    pub fn commit_round(&self, round_id: u64) -> Result<(), ByzantineError> {
        let mut rounds = self.rounds.borrow_mut();
        let round = rounds.get_mut(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if round.is_decided() {
            return Err(ByzantineError::RoundAlreadyDecided { round_id });
        }
        round.state = ByzantineRoundState::Committed;
        round.committed_at_ms = Some(crate::types::IdsTime::now_ms());
        drop(rounds);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.committed_rounds += 1;
        drop(stats);

        Ok(())
    }

    /// Issue a challenge.
    pub fn issue_challenge(&self, node_id: NodeId, nonce: Vec<u8>) -> Result<ByzantineChallenge, ByzantineError> {
        let config = self.config.borrow();
        let now = crate::types::IdsTime::now_ms();
        let challenge_id = *self.next_challenge_id.borrow();
        drop(config);

        let challenge = self.validator.issue_challenge(
            challenge_id,
            node_id,
            nonce,
            now,
            now + self.config.borrow().challenge_timeout_ms,
        );

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_challenges += 1;
        *self.next_challenge_id.borrow_mut() += 1;

        Ok(challenge)
    }

    /// Verify a response.
    pub fn verify_response(&self, node_id: NodeId, response: ByzantineResponse) -> Result<bool, ByzantineError> {
        let valid = self.validator.verify_response(&response)?;
        if valid {
            self.validator.record_response(response);
            let mut reputations = self.monitor.reputations.borrow_mut();
            if let Some(rep) = reputations.get_mut(&node_id) {
                rep.record_positive(crate::types::IdsTime::now_ms());
            }
        } else {
            let mut stats = self.monitor.stats.borrow_mut();
            stats.total_challenges_failed += 1;
            let mut reputations = self.monitor.reputations.borrow_mut();
            if let Some(rep) = reputations.get_mut(&node_id) {
                rep.record_negative(crate::types::IdsTime::now_ms());
            }
        }
        Ok(valid)
    }

    /// Detect double vote.
    pub fn detect_double_vote(&self, node_id: NodeId, round_id: u64) -> Result<bool, ByzantineError> {
        let rounds = self.rounds.borrow();
        let round = rounds.get(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        for record in &round.votes {
            if record.voter == node_id {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Slash a node.
    pub fn slash(&self, node_id: NodeId, amount: u64, reason: &str) -> Result<ByzantineSlashRecord, ByzantineError> {
        let slash_id = *self.next_slash_id.borrow();
        let now = crate::types::IdsTime::now_ms();
        let record = ByzantineSlashRecord::new(slash_id, node_id, amount, reason.to_string(), now, node_id);
        self.monitor.slashes.borrow_mut().push(record.clone());
        *self.next_slash_id.borrow_mut() += 1;

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_slashes += 1;
        stats.total_slashed += amount;

        // Also reduce reputation
        let mut reputations = self.monitor.reputations.borrow_mut();
        if let Some(rep) = reputations.get_mut(&node_id) {
            rep.total_slashes += 1;
            rep.total_slashed += amount;
            rep.score = (rep.score - 0.1).max(0.0);
        }

        Ok(record)
    }

    /// Get reputation.
    pub fn reputation(&self, node_id: NodeId) -> Result<ByzantineReputation, ByzantineError> {
        self.monitor.reputation(node_id)
    }

    /// Generate a certificate.
    pub fn generate_certificate(&self, round_id: u64) -> Result<ByzantineCertificate, ByzantineError> {
        let rounds = self.rounds.borrow();
        let round = rounds.get(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if !round.is_decided() {
            return Err(ByzantineError::RoundNotDecided { round_id });
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(&round.payload);
        let payload_hash = hasher.finalize().into();

        let cert = ByzantineCertificate::new(
            round_id,
            round.proposer,
            payload_hash,
            round.agree_count,
            round.total_voters(),
            crate::types::IdsTime::now_ms(),
        );
        self.certificates.borrow_mut().insert(round_id, cert.clone());
        Ok(cert)
    }

    /// Verify a certificate.
    pub fn verify_certificate(&self, certificate: &ByzantineCertificate) -> Result<bool, ByzantineError> {
        let config = self.config.borrow();
        Ok(certificate.has_quorum(config.agreement_quorum))
    }

    /// Get stats.
    pub fn stats(&self) -> ByzantineStats {
        self.monitor.stats()
    }

    /// Health score for a node.
    pub fn health_score(&self, node_id: NodeId) -> Result<f64, ByzantineError> {
        self.monitor.health_score(node_id)
    }

    /// Get all registered node IDs.
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.monitor.nodes.borrow().keys().cloned().collect()
    }

    /// Get quarantined node IDs.
    pub fn quarantined_ids(&self) -> Vec<NodeId> {
        self.monitor.quarantines.borrow().keys().cloned().collect()
    }

    /// Get evicted node IDs.
    pub fn evicted_ids(&self) -> Vec<NodeId> {
        self.monitor.nodes.borrow()
            .iter()
            .filter(|(_, n)| n.state == ByzantineState::Evicted)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get total double votes detected across all rounds.
    pub fn total_double_votes(&self) -> u64 {
        self.monitor.stats.borrow().total_double_votes
    }

    /// Clear expired challenges from the validator.
    pub fn clear_expired_challenges(&self, now_ms: u64) {
        self.validator.clear_expired(now_ms);
    }

    /// Get a round by ID.
    pub fn get_round(&self, round_id: u64) -> Option<ByzantineRound> {
        self.rounds.borrow().get(&round_id).cloned()
    }

    /// Get a certificate by round ID.
    pub fn get_certificate(&self, round_id: u64) -> Option<ByzantineCertificate> {
        self.certificates.borrow().get(&round_id).cloned()
    }

    /// Get total rounds started.
    pub fn total_rounds(&self) -> u64 {
        self.monitor.stats.borrow().total_rounds
    }

    /// Get total rounds committed.
    pub fn total_committed_rounds(&self) -> u64 {
        self.monitor.stats.borrow().committed_rounds
    }

    /// Get total rounds aborted.
    pub fn total_aborted_rounds(&self) -> u64 {
        self.monitor.stats.borrow().aborted_rounds
    }

    /// Get total rounds timed out.
    pub fn total_timed_out_rounds(&self) -> u64 {
        self.monitor.stats.borrow().timed_out_rounds
    }

    /// Get total votes cast.
    pub fn total_votes_cast(&self) -> u64 {
        self.monitor.stats.borrow().total_votes
    }

    /// Get total challenges issued.
    pub fn total_challenges_issued(&self) -> u64 {
        self.monitor.stats.borrow().total_challenges
    }

    /// Get total challenges that failed.
    pub fn total_challenges_failed(&self) -> u64 {
        self.monitor.stats.borrow().total_challenges_failed
    }

    /// Get total slashes applied.
    pub fn total_slashes_applied(&self) -> u64 {
        self.monitor.stats.borrow().total_slashes
    }

    /// Get total amount slashed.
    pub fn total_amount_slashed(&self) -> u64 {
        self.monitor.stats.borrow().total_slashed
    }

    /// Abort a round.
    pub fn abort_round(&self, round_id: u64) -> Result<(), ByzantineError> {
        let mut rounds = self.rounds.borrow_mut();
        let round = rounds.get_mut(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if round.is_decided() {
            return Err(ByzantineError::RoundAlreadyDecided { round_id });
        }
        round.state = ByzantineRoundState::Aborted;
        drop(rounds);
        let mut stats = self.monitor.stats.borrow_mut();
        stats.aborted_rounds += 1;
        Ok(())
    }

    /// Timeout a round.
    pub fn timeout_round(&self, round_id: u64) -> Result<(), ByzantineError> {
        let mut rounds = self.rounds.borrow_mut();
        let round = rounds.get_mut(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if round.is_decided() {
            return Err(ByzantineError::RoundAlreadyDecided { round_id });
        }
        round.state = ByzantineRoundState::TimedOut;
        drop(rounds);
        let mut stats = self.monitor.stats.borrow_mut();
        stats.timed_out_rounds += 1;
        Ok(())
    }

    /// Record a double vote detection.
    pub fn record_double_vote(&self, node_id: NodeId, round_id: u64) -> Result<(), ByzantineError> {
        let _ = self.detect_double_vote(node_id, round_id)?;
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_double_votes += 1;
        let _ = self.slash(node_id, 50, "double vote detected");
        Ok(())
    }

    /// Get the configuration.
    pub fn config(&self) -> ByzantineConfig {
        self.config.borrow().clone()
    }

    /// Decay all reputations by the configured factor.
    pub fn decay_all_reputations(&self, now_ms: u64) {
        let config = self.config.borrow();
        let factor = config.reputation_decay_factor;
        drop(config);
        let mut reputations = self.monitor.reputations.borrow_mut();
        for rep in reputations.values_mut() {
            rep.decay(factor, now_ms);
        }
    }

    /// Get nodes by state.
    pub fn nodes_by_state(&self, state: ByzantineState) -> Vec<NodeId> {
        self.monitor.nodes.borrow()
            .iter()
            .filter(|(_, n)| n.state == state)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Check if the system has too many Byzantine nodes (above quorum threshold).
    pub fn byzantine_quorum_compromised(&self) -> bool {
        let stats = self.stats();
        stats.byzantine_ratio() > (1.0 - self.config.borrow().agreement_quorum)
    }

    /// Get the next round ID that would be assigned.
    pub fn next_round_id(&self) -> u64 {
        *self.next_round_id.borrow()
    }

    /// Get the next challenge ID that would be assigned.
    pub fn next_challenge_id(&self) -> u64 {
        *self.next_challenge_id.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ROUND STATE EXTENSIONS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineRoundState {
    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ByzantineRoundState::Committed
                | ByzantineRoundState::Aborted
                | ByzantineRoundState::TimedOut
        )
    }

    /// Check if this represents successful agreement.
    pub fn is_success(&self) -> bool {
        matches!(self, ByzantineRoundState::Committed | ByzantineRoundState::Agreed)
    }

    /// Check if this represents failure.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            ByzantineRoundState::Aborted | ByzantineRoundState::TimedOut
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE VOTE EXTENSIONS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineVote {
    /// Check if this vote is positive agreement.
    pub fn is_positive(&self) -> bool {
        matches!(self, ByzantineVote::Agree)
    }

    /// Check if this vote is negative disagreement.
    pub fn is_negative(&self) -> bool {
        matches!(self, ByzantineVote::Disagree)
    }

    /// Check if this vote is neutral abstention.
    pub fn is_neutral(&self) -> bool {
        matches!(self, ByzantineVote::Abstain)
    }

    /// Convert to a numeric weight (+1 for agree, -1 for disagree, 0 for abstain).
    pub fn numeric_value(&self) -> i8 {
        match self {
            ByzantineVote::Agree => 1,
            ByzantineVote::Disagree => -1,
            ByzantineVote::Abstain => 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ByzantineError {
    #[error("node not registered: {node_id:?}")]
    NodeNotRegistered { node_id: NodeId },

    #[error("node evicted: {node_id:?}")]
    NodeEvicted { node_id: NodeId },

    #[error("node not quarantined: {node_id:?}")]
    NodeNotQuarantined { node_id: NodeId },

    #[error("round not found: {round_id}")]
    RoundNotFound { round_id: u64 },

    #[error("round already exists: {round_id}")]
    RoundAlreadyExists { round_id: u64 },

    #[error("round already decided: {round_id}")]
    RoundAlreadyDecided { round_id: u64 },

    #[error("round not decided: {round_id}")]
    RoundNotDecided { round_id: u64 },

    #[error("challenge not found: {challenge_id}")]
    ChallengeNotFound { challenge_id: u64 },

    #[error("invalid config: field={field}, reason={reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("internal byzantine error: {message}")]
    Internal { message: String },
}

impl ByzantineError {
    pub fn category(&self) -> &'static str {
        match self {
            ByzantineError::NodeNotRegistered { .. } => "node_not_registered",
            ByzantineError::NodeEvicted { .. } => "node_evicted",
            ByzantineError::NodeNotQuarantined { .. } => "node_not_quarantined",
            ByzantineError::RoundNotFound { .. } => "round_not_found",
            ByzantineError::RoundAlreadyExists { .. } => "round_already_exists",
            ByzantineError::RoundAlreadyDecided { .. } => "round_already_decided",
            ByzantineError::RoundNotDecided { .. } => "round_not_decided",
            ByzantineError::ChallengeNotFound { .. } => "challenge_not_found",
            ByzantineError::InvalidConfig { .. } => "invalid_config",
            ByzantineError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ByzantineError::NodeNotRegistered { .. }
                | ByzantineError::RoundNotFound { .. }
                | ByzantineError::ChallengeNotFound { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ByzantineError::NodeNotRegistered { .. } => "register the node first",
            ByzantineError::NodeEvicted { .. } => "node is evicted, cannot participate",
            ByzantineError::NodeNotQuarantined { .. } => "node is not quarantined",
            ByzantineError::RoundNotFound { .. } => "create the round first",
            ByzantineError::RoundAlreadyExists { .. } => "use existing round",
            ByzantineError::RoundAlreadyDecided { .. } => "round is already decided",
            ByzantineError::RoundNotDecided { .. } => "round not yet decided",
            ByzantineError::ChallengeNotFound { .. } => "issue a challenge first",
            ByzantineError::InvalidConfig { .. } => "fix configuration",
            ByzantineError::Internal { .. } => "contact support",
        }
    }

    pub fn is_byzantine_fault(&self) -> bool {
        matches!(self, ByzantineError::NodeEvicted { .. } | ByzantineError::NodeNotQuarantined { .. })
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ENGINE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineEngine {
    /// Process a challenge response and update node state accordingly.
    pub fn process_challenge_response(
        &self,
        node_id: NodeId,
        response: ByzantineResponse,
    ) -> Result<bool, ByzantineError> {
        let valid = self.verify_response(node_id, response.clone())?;
        if valid {
            self.monitor.update_score(node_id, 5, "challenge passed")?;
        } else {
            self.monitor.update_score(node_id, -10, "challenge failed")?;
            let mut stats = self.monitor.stats.borrow_mut();
            stats.total_challenges_failed += 1;
        }
        Ok(valid)
    }

    /// Finalize a round (mark as agreed if quorum met, otherwise abort).
    pub fn finalize_round(&self, round_id: u64) -> Result<ByzantineRoundState, ByzantineError> {
        let rounds = self.rounds.borrow();
        let round = rounds.get(&round_id).ok_or_else(|| ByzantineError::RoundNotFound { round_id })?;
        if round.is_decided() {
            return Ok(round.state);
        }
        drop(rounds);

        let config = self.config.borrow();
        let quorum = config.agreement_quorum;
        drop(config);

        let reached = self.is_byzantine_agreement_reached(round_id)?;
        if reached {
            self.commit_round(round_id)?;
            Ok(ByzantineRoundState::Agreed)
        } else {
            self.abort_round(round_id)?;
            Ok(ByzantineRoundState::Aborted)
        }
    }

    /// Get a health report for a node.
    pub fn get_health_report(&self, node_id: NodeId) -> Result<ByzantineHealthReport, ByzantineError> {
        let score = self.behavior_score(node_id)?;
        let reputation = self.reputation(node_id)?;
        let health = self.health_score(node_id)?;
        let is_byzantine = self.is_byzantine(node_id)?;
        let is_quarantined = self.is_quarantined(node_id)?;
        Ok(ByzantineHealthReport {
            node_id,
            behavior_score: score,
            reputation_score: reputation.score,
            health_score: health,
            is_byzantine,
            is_quarantined,
            positive_interactions: reputation.positive_interactions,
            negative_interactions: reputation.negative_interactions,
            total_slashes: reputation.total_slashes,
        })
    }

    /// Get a quarantine report for all quarantined nodes.
    pub fn get_quarantine_report(&self) -> Vec<ByzantineQuarantineReport> {
        self.monitor
            .quarantines
            .borrow()
            .iter()
            .map(|(node_id, quarantine)| ByzantineQuarantineReport {
                node_id: *node_id,
                quarantined_at_ms: quarantine.quarantined_at_ms,
                reason: quarantine.reason.clone(),
                evidence_count: quarantine.evidence.len(),
                reviewed: quarantine.reviewed,
                duration_ms: quarantine.duration_ms(crate::types::IdsTime::now_ms()),
            })
            .collect()
    }

    /// Revoke a slash record (administrative action).
    pub fn revoke_slash(&self, slash_id: u64) -> Result<(), ByzantineError> {
        let mut slashes = self.monitor.slashes.borrow_mut();
        let pos = slashes.iter().position(|s| s.slash_id == slash_id);
        match pos {
            Some(idx) => {
                let record = slashes.remove(idx);
                let mut stats = self.monitor.stats.borrow_mut();
                stats.total_slashes = stats.total_slashes.saturating_sub(1);
                stats.total_slashed = stats.total_slashed.saturating_sub(record.amount);
                // Restore reputation
                let mut reputations = self.monitor.reputations.borrow_mut();
                if let Some(rep) = reputations.get_mut(&record.node_id) {
                    rep.total_slashes = rep.total_slashes.saturating_sub(1);
                    rep.total_slashed = rep.total_slashed.saturating_sub(record.amount);
                }
                Ok(())
            }
            None => Err(ByzantineError::Internal {
                format: format!("slash record {} not found", slash_id),
            }),
        }
    }

    /// Clear all evidence for a node.
    pub fn clear_all_evidence(&self, node_id: NodeId) -> Result<(), ByzantineError> {
        let mut nodes = self.monitor.nodes.borrow_mut();
        let node = nodes.get_mut(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        node.clear_evidence();
        Ok(())
    }

    /// Get evidence for a node.
    pub fn get_evidence_for_node(&self, node_id: NodeId) -> Result<Vec<Vec<u8>>, ByzantineError> {
        let nodes = self.monitor.nodes.borrow();
        let node = nodes.get(&node_id).ok_or_else(|| ByzantineError::NodeNotRegistered { node_id })?;
        Ok(node.evidence.clone())
    }

    /// Check if a node exists.
    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.monitor.nodes.borrow().contains_key(&node_id)
    }

    /// Get total registered node count.
    pub fn node_count(&self) -> u64 {
        self.monitor.nodes.borrow().len() as u64
    }

    /// Get total round count.
    pub fn round_count(&self) -> u64 {
        self.rounds.borrow().len() as u64
    }

    /// Get total certificate count.
    pub fn certificate_count(&self) -> u64 {
        self.certificates.borrow().len() as u64
    }

    /// Get total slash count.
    pub fn slash_count(&self) -> u64 {
        self.monitor.slashes.borrow().len() as u64
    }

    /// Get all slash records.
    pub fn all_slashes(&self) -> Vec<ByzantineSlashRecord> {
        self.monitor.slashes.borrow().clone()
    }

    /// Get all quarantine records.
    pub fn all_quarantines(&self) -> Vec<ByzantineQuarantine> {
        self.monitor.quarantines.borrow().values().cloned().collect()
    }

    /// Get all registered nodes.
    pub fn all_nodes(&self) -> Vec<ByzantineNode> {
        self.monitor.nodes.borrow().values().cloned().collect()
    }

    /// Get all rounds.
    pub fn all_rounds(&self) -> Vec<ByzantineRound> {
        self.rounds.borrow().values().cloned().collect()
    }

    /// Get all certificates.
    pub fn all_certificates(&self) -> Vec<ByzantineCertificate> {
        self.certificates.borrow().values().cloned().collect()
    }

    /// Get reputation for all nodes.
    pub fn all_reputations(&self) -> Vec<ByzantineReputation> {
        self.monitor.reputations.borrow().values().cloned().collect()
    }

    /// Get all vote records across all rounds.
    pub fn all_votes(&self) -> Vec<ByzantineVoteRecord> {
        self.rounds
            .borrow()
            .values()
            .flat_map(|r| r.votes.clone())
            .collect()
    }

    /// Reset all stats to zero.
    pub fn reset_stats(&self) {
        *self.monitor.stats.borrow_mut() = ByzantineStats::default();
    }

    /// Clear all rounds.
    pub fn clear_rounds(&self) {
        self.rounds.borrow_mut().clear();
    }

    /// Clear all certificates.
    pub fn clear_certificates(&self) {
        self.certificates.borrow_mut().clear();
    }

    /// Clear all slashes.
    pub fn clear_slashes(&self) {
        self.monitor.slashes.borrow_mut().clear();
    }

    /// Clear all quarantines.
    pub fn clear_quarantines(&self) {
        self.monitor.quarantines.borrow_mut().clear();
    }

    /// Get the config (immutable borrow).
    pub fn config_borrow(&self) -> std::cell::Ref<'_, ByzantineConfig> {
        self.config.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE HEALTH REPORT
///////////////////////////////////////////////////////////////////////////////

/// A health report for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineHealthReport {
    /// Node identifier.
    pub node_id: NodeId,
    /// Behavior score.
    pub behavior_score: i64,
    /// Reputation score.
    pub reputation_score: f64,
    /// Composite health score.
    pub health_score: f64,
    /// Whether node is Byzantine.
    pub is_byzantine: bool,
    /// Whether node is quarantined.
    pub is_quarantined: bool,
    /// Positive interaction count.
    pub positive_interactions: u64,
    /// Negative interaction count.
    pub negative_interactions: u64,
    /// Total slashes.
    pub total_slashes: u64,
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE QUARANTINE REPORT
///////////////////////////////////////////////////////////////////////////////

/// A quarantine report entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByzantineQuarantineReport {
    /// Node identifier.
    pub node_id: NodeId,
    /// Timestamp when quarantined.
    pub quarantined_at_ms: u64,
    /// Reason for quarantine.
    pub reason: String,
    /// Number of evidence items.
    pub evidence_count: usize,
    /// Whether quarantine has been reviewed.
    pub reviewed: bool,
    /// Duration of quarantine in milliseconds.
    pub duration_ms: u64,
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE NODE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineNode {
    /// Mark activity at a given timestamp.
    pub fn mark_activity(&mut self, timestamp_ms: u64) {
        self.last_activity_ms = timestamp_ms;
    }

    /// Check if node is inactive beyond a threshold.
    pub fn is_inactive(&self, now_ms: u64, threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_activity_ms) > threshold_ms
    }

    /// Get time since last activity.
    pub fn time_since_activity(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.last_activity_ms)
    }

    /// Increment vote count.
    pub fn increment_votes(&mut self) {
        self.total_votes += 1;
    }

    /// Increment correct vote count.
    pub fn increment_correct_votes(&mut self) {
        self.correct_votes += 1;
    }

    /// Increment double vote count.
    pub fn increment_double_votes(&mut self) {
        self.double_votes += 1;
    }

    /// Increment failed challenge count.
    pub fn increment_failed_challenges(&mut self) {
        self.failed_challenges += 1;
    }

    /// Check if node is trustworthy (score above 0 and reputation above 0.5).
    pub fn is_trustworthy(&self) -> bool {
        self.behavior_score > 0 && self.reputation > 0.5
    }

    /// Get risk level (0.0 = safe, 1.0 = high risk).
    pub fn risk_level(&self) -> f64 {
        let score_risk = if self.behavior_score >= 0 { 0.0 } else { 0.5 };
        let rep_risk = 1.0 - self.reputation;
        let double_vote_risk = if self.double_votes > 0 { 0.3 } else { 0.0 };
        (score_risk + rep_risk + double_vote_risk).min(1.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ROUND EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineRound {
    /// Get the vote tally as (agree, disagree, abstain).
    pub fn vote_tally(&self) -> (u64, u64, u64) {
        (self.agree_count, self.disagree_count, self.abstain_count)
    }

    /// Get the agreement ratio.
    pub fn agreement_ratio(&self) -> f64 {
        let total = self.total_votes();
        if total == 0 {
            return 0.0;
        }
        self.agree_count as f64 / total as f64
    }

    /// Get the disagree ratio.
    pub fn disagree_ratio(&self) -> f64 {
        let total = self.total_votes();
        if total == 0 {
            return 0.0;
        }
        self.disagree_count as f64 / total as f64
    }

    /// Check if the round has any votes.
    pub fn has_votes(&self) -> bool {
        self.total_votes() > 0
    }

    /// Get round duration in milliseconds.
    pub fn duration_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.started_at_ms)
    }

    /// Check if round is older than a threshold.
    pub fn is_older_than(&self, now_ms: u64, threshold_ms: u64) -> bool {
        self.duration_ms(now_ms) > threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CERTIFICATE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineCertificate {
    /// Get the quorum ratio.
    pub fn quorum_ratio(&self) -> f64 {
        if self.total_voters == 0 {
            return 0.0;
        }
        self.signatures.len() as f64 / self.total_voters as f64
    }

    /// Check if certificate is valid (has quorum and signatures).
    pub fn is_valid(&self, quorum: f64) -> bool {
        !self.signatures.is_empty() && self.has_quorum(quorum)
    }

    /// Get the signers.
    pub fn signers(&self) -> Vec<NodeId> {
        self.signatures.iter().map(|(id, _)| *id).collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CHALLENGE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineChallenge {
    /// Get time remaining until expiry in milliseconds.
    pub fn time_remaining_ms(&self, now_ms: u64) -> u64 {
        self.expires_at_ms.saturating_sub(now_ms)
    }

    /// Check if challenge is still valid (not expired and not responded).
    pub fn is_valid(&self, now_ms: u64) -> bool {
        !self.responded && !self.is_expired(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE RESPONSE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineResponse {
    /// Check if response has data.
    pub fn has_data(&self) -> bool {
        !self.response.is_empty()
    }

    /// Get response length.
    pub fn len(&self) -> usize {
        self.response.len()
    }

    /// Check if response is empty.
    pub fn is_empty(&self) -> bool {
        self.response.is_empty()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE SLASH RECORD EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineSlashRecord {
    /// Check if slash is recent (within threshold).
    pub fn is_recent(&self, now_ms: u64, threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.slashed_at_ms) <= threshold_ms
    }

    /// Get time since slash in milliseconds.
    pub fn time_since_slash(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.slashed_at_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE QUARANTINE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineQuarantine {
    /// Check if quarantine is old enough for review.
    pub fn needs_review(&self, now_ms: u64, threshold_ms: u64) -> bool {
        !self.reviewed && self.duration_ms(now_ms) > threshold_ms
    }

    /// Get evidence count.
    pub fn evidence_count(&self) -> usize {
        self.evidence.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE REPUTATION EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineReputation {
    /// Check if reputation is critical (below 0.3).
    pub fn is_critical(&self) -> bool {
        self.score < 0.3
    }

    /// Check if reputation is good (above 0.8).
    pub fn is_good(&self) -> bool {
        self.score > 0.8
    }

    /// Get net interactions (positive - negative).
    pub fn net_interactions(&self) -> i64 {
        self.positive_interactions as i64 - self.negative_interactions as i64
    }

    /// Get slash ratio (slashed / positive interactions).
    pub fn slash_ratio(&self) -> f64 {
        if self.positive_interactions == 0 {
            return 0.0;
        }
        self.total_slashed as f64 / self.positive_interactions as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE CONFIG EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineConfig {
    /// Check if configuration is strict (low thresholds).
    pub fn is_strict(&self) -> bool {
        self.suspicion_threshold >= -5
            && self.quarantine_threshold >= -20
            && self.eviction_threshold >= -40
    }

    /// Check if configuration is lenient (high thresholds).
    pub fn is_lenient(&self) -> bool {
        self.suspicion_threshold <= -20
            && self.quarantine_threshold <= -80
            && self.eviction_threshold <= -150
    }

    /// Get the total allowed slashes per node (derived from config).
    pub fn max_slashes_per_node(&self) -> u64 {
        10
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE VOTE RECORD EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineVoteRecord {
    /// Get the numeric weight of this vote.
    pub fn weight(&self) -> i8 {
        self.vote.numeric_value()
    }

    /// Check if vote was signed.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Get the voter's weight contribution.
    pub fn voter_contribution(&self) -> i8 {
        if self.is_signed() {
            self.vote.numeric_value()
        } else {
            0
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE STATS EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineStats {
    /// Get total isolated nodes (quarantined + evicted).
    pub fn total_isolated(&self) -> u64 {
        self.quarantined_nodes + self.evicted_nodes
    }

    /// Get total participating nodes (healthy + recovering).
    pub fn total_participating(&self) -> u64 {
        self.healthy_nodes + self.recovering_nodes
    }

    /// Get participation ratio.
    pub fn participation_ratio(&self) -> f64 {
        if self.total_nodes == 0 {
            return 1.0;
        }
        self.total_participating() as f64 / self.total_nodes as f64
    }

    /// Get double vote rate.
    pub fn double_vote_rate(&self) -> f64 {
        if self.total_votes == 0 {
            return 0.0;
        }
        self.total_double_votes as f64 / self.total_votes as f64
    }

    /// Get slash rate (slashes per node).
    pub fn slash_rate(&self) -> f64 {
        if self.total_nodes == 0 {
            return 0.0;
        }
        self.total_slashes as f64 / self.total_nodes as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE MONITOR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineMonitor {
    /// Get all node IDs.
    pub fn all_node_ids(&self) -> Vec<NodeId> {
        self.nodes.borrow().keys().cloned().collect()
    }

    /// Get node count.
    pub fn node_count(&self) -> u64 {
        self.nodes.borrow().len() as u64
    }

    /// Get quarantined count.
    pub fn quarantined_count(&self) -> u64 {
        self.quarantines.borrow().len() as u64
    }

    /// Get slash count.
    pub fn slash_count(&self) -> u64 {
        self.slashes.borrow().len() as u64
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.nodes.borrow_mut().clear();
        self.quarantines.borrow_mut().clear();
        self.reputations.borrow_mut().clear();
        self.slashes.borrow_mut().clear();
        *self.stats.borrow_mut() = ByzantineStats::default();
        *self.next_slash_id.borrow_mut() = 1;
    }

    /// Decay all reputations.
    pub fn decay_all_reputations(&self, factor: f64, now_ms: u64) {
        let mut reputations = self.reputations.borrow_mut();
        for rep in reputations.values_mut() {
            rep.decay(factor, now_ms);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE VALIDATOR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineValidator {
    /// Get all challenge IDs.
    pub fn all_challenge_ids(&self) -> Vec<u64> {
        self.challenges.borrow().keys().cloned().collect()
    }

    /// Get all response IDs.
    pub fn all_response_ids(&self) -> Vec<u64> {
        self.responses.borrow().keys().cloned().collect()
    }

    /// Get challenge count.
    pub fn challenge_count(&self) -> u64 {
        self.challenges.borrow().len() as u64
    }

    /// Get response count.
    pub fn response_count(&self) -> u64 {
        self.responses.borrow().len() as u64
    }

    /// Clear all challenges and responses.
    pub fn clear_all(&self) {
        self.challenges.borrow_mut().clear();
        self.responses.borrow_mut().clear();
        self.votes_per_round.borrow_mut().clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// BYZANTINE ERROR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl ByzantineError {
    /// Check if this error is related to a node state.
    pub fn is_node_state_error(&self) -> bool {
        matches!(
            self,
            ByzantineError::NodeNotRegistered { .. }
                | ByzantineError::NodeEvicted { .. }
                | ByzantineError::NodeNotQuarantined { .. }
        )
    }

    /// Check if this error is related to a round.
    pub fn is_round_error(&self) -> bool {
        matches!(
            self,
            ByzantineError::RoundNotFound { .. }
                | ByzantineError::RoundAlreadyExists { .. }
                | ByzantineError::RoundAlreadyDecided { .. }
                | ByzantineError::RoundNotDecided { .. }
        )
    }

    /// Check if this error is related to a challenge.
    pub fn is_challenge_error(&self) -> bool {
        matches!(self, ByzantineError::ChallengeNotFound { .. })
    }
}

///////////////////////////////////////////////////////////////////////////////
// TEST HELPERS
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

    fn test_endpoint() -> Endpoint {
        Endpoint {
            carrier_type: crate::types::CarrierType::Direct,
            address: vec![1, 2, 3, 4],
        }
    }

    fn test_config() -> ByzantineConfig {
        ByzantineConfig::default()
    }

    // ========================================================================
    // ByzantineConfig tests
    // ========================================================================

    #[test]
    fn byzantine_config_default() {
        let config = ByzantineConfig::default();
        assert_eq!(config.suspicion_threshold, -10);
        assert_eq!(config.quarantine_threshold, -50);
        assert_eq!(config.eviction_threshold, -100);
        assert_eq!(config.recovery_threshold, 0);
        assert_eq!(config.max_rounds, 5);
        assert!((config.agreement_quorum - 0.67).abs() < 0.01);
        assert_eq!(config.challenge_timeout_ms, 30000);
        assert!((config.reputation_decay_factor - 0.95).abs() < 0.01);
        assert!(config.enable_double_vote_detection);
        assert!(config.enable_challenge_response);
    }

    #[test]
    fn byzantine_config_builder() {
        let config = ByzantineConfig::default()
            .with_suspicion_threshold(-5)
            .with_quarantine_threshold(-30)
            .with_eviction_threshold(-80)
            .with_recovery_threshold(10)
            .with_max_rounds(10)
            .with_agreement_quorum(0.8);
        assert_eq!(config.suspicion_threshold, -5);
        assert_eq!(config.quarantine_threshold, -30);
        assert_eq!(config.eviction_threshold, -80);
        assert_eq!(config.recovery_threshold, 10);
        assert_eq!(config.max_rounds, 10);
        assert!((config.agreement_quorum - 0.8).abs() < 0.01);
    }

    #[test]
    fn byzantine_config_validate_valid() {
        let config = ByzantineConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn byzantine_config_validate_suspicion_gt_quarantine() {
        let config = ByzantineConfig::default().with_suspicion_threshold(-10).with_quarantine_threshold(-20);
        assert!(config.validate().is_err());
    }

    #[test]
    fn byzantine_config_validate_quorum_out_of_range() {
        let config = ByzantineConfig::default().with_agreement_quorum(1.5);
        assert!(config.validate().is_err());
        let config = ByzantineConfig::default().with_agreement_quorum(0.0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn byzantine_config_validate_max_rounds_zero() {
        let config = ByzantineConfig::default().with_max_rounds(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn byzantine_config_validate_quarantine_gt_eviction() {
        let config = ByzantineConfig::default().with_quarantine_threshold(-80).with_eviction_threshold(-50);
        assert!(config.validate().is_err());
    }

    // ========================================================================
    // ByzantineState tests
    // ========================================================================

    #[test]
    fn byzantine_state_can_participate() {
        assert!(ByzantineState::Healthy.can_participate());
        assert!(ByzantineState::Recovering.can_participate());
        assert!(!ByzantineState::Suspected.can_participate());
        assert!(!ByzantineState::Quarantined.can_participate());
        assert!(!ByzantineState::Evicted.can_participate());
    }

    #[test]
    fn byzantine_state_is_final() {
        assert!(ByzantineState::Evicted.is_final());
        assert!(!ByzantineState::Healthy.is_final());
        assert!(!ByzantineState::Quarantined.is_final());
    }

    #[test]
    fn byzantine_state_is_isolated() {
        assert!(ByzantineState::Quarantined.is_isolated());
        assert!(ByzantineState::Evicted.is_isolated());
        assert!(!ByzantineState::Healthy.is_isolated());
        assert!(!ByzantineState::Suspected.is_isolated());
    }

    #[test]
    fn byzantine_state_default() {
        assert_eq!(ByzantineState::default(), ByzantineState::Healthy);
    }

    // ========================================================================
    // ByzantineNode tests
    // ========================================================================

    #[test]
    fn byzantine_node_new() {
        let node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert_eq!(node.node_id, test_node_id());
        assert_eq!(node.endpoint, test_endpoint());
        assert_eq!(node.state, ByzantineState::Healthy);
        assert_eq!(node.behavior_score, 0);
        assert!((node.reputation - 1.0).abs() < 0.01);
        assert_eq!(node.total_votes, 0);
        assert_eq!(node.correct_votes, 0);
        assert_eq!(node.double_votes, 0);
        assert_eq!(node.failed_challenges, 0);
    }

    #[test]
    fn byzantine_node_update_score() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        let score = node.update_score(10, "good behavior");
        assert_eq!(score, 10);
        assert_eq!(node.behavior_score, 10);
    }

    #[test]
    fn byzantine_node_score_saturates() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        node.update_score(i64::MAX, "overflow");
        assert_eq!(node.behavior_score, i64::MAX);
        node.update_score(1, "still adding");
        assert_eq!(node.behavior_score, i64::MAX);
    }

    #[test]
    fn byzantine_node_should_suspect() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert!(!node.should_suspect(-10));
        node.update_score(-20, "bad");
        assert!(node.should_suspect(-10));
    }

    #[test]
    fn byzantine_node_should_quarantine() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        node.update_score(-60, "very bad");
        assert!(node.should_quarantine(-50));
        assert!(!node.should_quarantine(-70));
    }

    #[test]
    fn byzantine_node_should_evict() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        node.update_score(-110, "terrible");
        assert!(node.should_evict(-100));
    }

    #[test]
    fn byzantine_node_can_reintegrate() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        node.state = ByzantineState::Quarantined;
        node.update_score(10, "recovered");
        assert!(node.can_reintegrate(0));
    }

    #[test]
    fn byzantine_node_vote_accuracy() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert!((node.vote_accuracy() - 1.0).abs() < 0.01);
        node.total_votes = 10;
        node.correct_votes = 7;
        assert!((node.vote_accuracy() - 0.7).abs() < 0.01);
    }

    #[test]
    fn byzantine_node_evidence() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        node.add_evidence(b"evidence1".to_vec());
        node.add_evidence(b"evidence2".to_vec());
        assert_eq!(node.evidence.len(), 2);
        node.clear_evidence();
        assert!(node.evidence.is_empty());
    }

    // ========================================================================
    // ByzantineVote tests
    // ========================================================================

    #[test]
    fn byzantine_vote_is_agree() {
        assert!(ByzantineVote::Agree.is_agree() if false); // enum, use record
        let record = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 1000);
        assert!(record.is_agree());
        assert!(!record.is_disagree());
    }

    #[test]
    fn byzantine_vote_is_disagree() {
        let record = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Disagree, 1000);
        assert!(record.is_disagree());
        assert!(!record.is_agree());
    }

    #[test]
    fn byzantine_vote_record_with_signature() {
        let sig = Signature([0u8; 64]);
        let record = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 1000).with_signature(sig);
        assert!(record.signature.is_some());
        assert_eq!(record.signature.unwrap(), sig);
    }

    // ========================================================================
    // ByzantineRound tests
    // ========================================================================

    #[test]
    fn byzantine_round_new() {
        let round = ByzantineRound::new(1, test_node_id(), vec![1, 2, 3], 1000);
        assert_eq!(round.round_id, 1);
        assert_eq!(round.proposer, test_node_id());
        assert_eq!(round.payload, vec![1, 2, 3]);
        assert_eq!(round.state, ByzantineRoundState::Open);
        assert_eq!(round.total_votes(), 0);
    }

    #[test]
    fn byzantine_round_add_vote() {
        let mut round = ByzantineRound::new(1, test_node_id(), vec![], 1000);
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id2(), ByzantineVote::Disagree, 2000));
        assert_eq!(round.agree_count, 1);
        assert_eq!(round.disagree_count, 1);
        assert_eq!(round.total_votes(), 2);
    }

    #[test]
    fn byzantine_round_agreement_met() {
        let mut round = ByzantineRound::new(1, test_node_id(), vec![], 1000);
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id2(), ByzantineVote::Agree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id3(), ByzantineVote::Abstain, 2000));
        // 2/3 agree = 0.67, meets 0.67 quorum
        assert!(round.agreement_met(0.67));
        assert!(!round.agreement_met(0.8));
    }

    #[test]
    fn byzantine_round_is_decided() {
        let mut round = ByzantineRound::new(1, test_node_id(), vec![], 1000);
        assert!(!round.is_decided());
        round.state = ByzantineRoundState::Committed;
        assert!(round.is_decided());
    }

    // ========================================================================
    // ByzantineChallenge tests
    // ========================================================================

    #[test]
    fn byzantine_challenge_new() {
        let challenge = ByzantineChallenge::new(1, test_node_id(), vec![1, 2, 3], 1000, 5000);
        assert_eq!(challenge.challenge_id, 1);
        assert_eq!(challenge.target_node, test_node_id());
        assert!(!challenge.responded);
        assert!(!challenge.is_expired(2000));
        assert!(challenge.is_expired(6000));
    }

    #[test]
    fn byzantine_challenge_mark_responded() {
        let mut challenge = ByzantineChallenge::new(1, test_node_id(), vec![], 1000, 5000);
        challenge.mark_responded();
        assert!(challenge.responded);
    }

    // ========================================================================
    // ByzantineResponse tests
    // ========================================================================

    #[test]
    fn byzantine_response_new() {
        let response = ByzantineResponse::new(1, test_node_id(), vec![1, 2, 3], 2000);
        assert_eq!(response.challenge_id, 1);
        assert_eq!(response.responder, test_node_id());
        assert_eq!(response.response, vec![1, 2, 3]);
        assert!(!response.valid);
    }

    // ========================================================================
    // ByzantineCertificate tests
    // ========================================================================

    #[test]
    fn byzantine_certificate_new() {
        let cert = ByzantineCertificate::new(1, test_node_id(), [42u8; 32], 2, 3, 1000);
        assert_eq!(cert.round_id, 1);
        assert_eq!(cert.agree_count, 2);
        assert_eq!(cert.total_voters, 3);
        assert!(cert.signatures.is_empty());
    }

    #[test]
    fn byzantine_certificate_add_signature() {
        let mut cert = ByzantineCertificate::new(1, test_node_id(), [0u8; 32], 0, 0, 0);
        cert.add_signature(test_node_id(), Signature([1u8; 64]));
        cert.add_signature(test_node_id2(), Signature([2u8; 64]));
        assert_eq!(cert.signatures.len(), 2);
    }

    #[test]
    fn byzantine_certificate_has_quorum() {
        let mut cert = ByzantineCertificate::new(1, test_node_id(), [0u8; 32], 0, 10, 0);
        assert!(!cert.has_quorum(0.67));
        for i in 0..7 {
            cert.add_signature(NodeId::new([i; 32]), Signature([i as u8; 64]));
        }
        assert!(cert.has_quorum(0.67));
    }

    // ========================================================================
    // ByzantineReputation tests
    // ========================================================================

    #[test]
    fn byzantine_reputation_new() {
        let rep = ByzantineReputation::new(test_node_id());
        assert_eq!(rep.node_id, test_node_id());
        assert!((rep.score - 1.0).abs() < 0.01);
        assert_eq!(rep.positive_interactions, 0);
        assert_eq!(rep.negative_interactions, 0);
    }

    #[test]
    fn byzantine_reputation_record_positive() {
        let mut rep = ByzantineReputation::new(test_node_id());
        rep.record_positive(1000);
        assert_eq!(rep.positive_interactions, 1);
        assert!(rep.score > 0.0);
        assert!(rep.score <= 1.0);
    }

    #[test]
    fn byzantine_reputation_record_negative() {
        let mut rep = ByzantineReputation::new(test_node_id());
        rep.record_negative(1000);
        assert_eq!(rep.negative_interactions, 1);
        assert!(rep.score < 1.0);
        assert!(rep.score >= 0.0);
    }

    #[test]
    fn byzantine_reputation_decay() {
        let mut rep = ByzantineReputation::new(test_node_id());
        rep.score = 1.0;
        rep.decay(0.5, 2000);
        assert!((rep.score - 0.5).abs() < 0.01);
    }

    #[test]
    fn byzantine_reputation_interaction_ratio() {
        let mut rep = ByzantineReputation::new(test_node_id());
        assert!((rep.interaction_ratio() - 1.0).abs() < 0.01);
        rep.record_positive(1000);
        rep.record_negative(2000);
        assert!((rep.interaction_ratio() - 0.5).abs() < 0.01);
    }

    // ========================================================================
    // ByzantineQuarantine tests
    // ========================================================================

    #[test]
    fn byzantine_quarantine_new() {
        let q = ByzantineQuarantine::new(test_node_id(), 1000, "bad behavior".to_string());
        assert_eq!(q.node_id, test_node_id());
        assert_eq!(q.reason, "bad behavior");
        assert!(!q.reviewed);
        assert!(q.reviewer.is_none());
    }

    #[test]
    fn byzantine_quarantine_add_evidence() {
        let mut q = ByzantineQuarantine::new(test_node_id(), 1000, "bad".to_string());
        q.add_evidence(b"e1".to_vec());
        q.add_evidence(b"e2".to_vec());
        assert_eq!(q.evidence.len(), 2);
    }

    #[test]
    fn byzantine_quarantine_mark_reviewed() {
        let mut q = ByzantineQuarantine::new(test_node_id(), 1000, "bad".to_string());
        q.mark_reviewed(test_node_id2());
        assert!(q.reviewed);
        assert_eq!(q.reviewer, Some(test_node_id2()));
    }

    #[test]
    fn byzantine_quarantine_duration() {
        let q = ByzantineQuarantine::new(test_node_id(), 1000, "bad".to_string());
        assert_eq!(q.duration_ms(5000), 4000);
        assert_eq!(q.duration_ms(500), 0);
    }

    // ========================================================================
    // ByzantineSlashRecord tests
    // ========================================================================

    #[test]
    fn byzantine_slash_record_new() {
        let record = ByzantineSlashRecord::new(1, test_node_id(), 50, "double vote".to_string(), 1000, test_node_id2());
        assert_eq!(record.slash_id, 1);
        assert_eq!(record.node_id, test_node_id());
        assert_eq!(record.amount, 50);
        assert_eq!(record.reason, "double vote");
        assert_eq!(record.slasher, test_node_id2());
    }

    // ========================================================================
    // ByzantineStats tests
    // ========================================================================

    #[test]
    fn byzantine_stats_default() {
        let stats = ByzantineStats::default();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.healthy_nodes, 0);
        assert!((stats.byzantine_ratio() - 0.0).abs() < 0.01);
        assert!((stats.commit_rate() - 1.0).abs() < 0.01);
        assert!((stats.challenge_failure_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn byzantine_stats_byzantine_ratio() {
        let mut stats = ByzantineStats::default();
        stats.total_nodes = 10;
        stats.quarantined_nodes = 3;
        stats.evicted_nodes = 2;
        assert!((stats.byzantine_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn byzantine_stats_commit_rate() {
        let mut stats = ByzantineStats::default();
        stats.total_rounds = 10;
        stats.committed_rounds = 7;
        assert!((stats.commit_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn byzantine_stats_challenge_failure_rate() {
        let mut stats = ByzantineStats::default();
        stats.total_challenges = 100;
        stats.total_challenges_failed = 10;
        assert!((stats.challenge_failure_rate() - 0.1).abs() < 0.01);
    }

    // ========================================================================
    // ByzantineMonitor tests
    // ========================================================================

    #[test]
    fn byzantine_monitor_register_node() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        assert_eq!(monitor.stats().total_nodes, 1);
        assert_eq!(monitor.stats().healthy_nodes, 1);
    }

    #[test]
    fn byzantine_monitor_update_score() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        let score = monitor.update_score(test_node_id(), 10, "good").unwrap();
        assert_eq!(score, 10);
        assert_eq!(monitor.behavior_score(test_node_id()).unwrap(), 10);
    }

    #[test]
    fn byzantine_monitor_suspect() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        monitor.suspect(test_node_id(), b"evidence".to_vec()).unwrap();
        assert!(monitor.is_byzantine(test_node_id()).unwrap());
        assert_eq!(monitor.stats().suspected_nodes, 1);
    }

    #[test]
    fn byzantine_monitor_quarantine() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        monitor.quarantine(test_node_id()).unwrap();
        assert!(monitor.is_quarantined(test_node_id()).unwrap());
        assert_eq!(monitor.stats().quarantined_nodes, 1);
    }

    #[test]
    fn byzantine_monitor_evict() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        monitor.evict(test_node_id()).unwrap();
        assert!(monitor.is_byzantine(test_node_id()).unwrap());
        assert_eq!(monitor.stats().evicted_nodes, 1);
    }

    #[test]
    fn byzantine_monitor_reintegrate() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        monitor.quarantine(test_node_id()).unwrap();
        monitor.reintegrate(test_node_id()).unwrap();
        assert_eq!(monitor.stats().recovering_nodes, 1);
    }

    #[test]
    fn byzantine_monitor_health_score() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        let score = monitor.health_score(test_node_id()).unwrap();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn byzantine_monitor_node_not_registered() {
        let monitor = ByzantineMonitor::new(ByzantineConfig::default());
        let err = monitor.behavior_score(test_node_id()).unwrap_err();
        assert!(matches!(err, ByzantineError::NodeNotRegistered { .. }));
    }

    // ========================================================================
    // ByzantineValidator tests
    // ========================================================================

    #[test]
    fn byzantine_validator_issue_challenge() {
        let validator = ByzantineValidator::new();
        let challenge = validator.issue_challenge(1, test_node_id(), vec![1, 2], 1000, 5000);
        assert_eq!(challenge.challenge_id, 1);
        assert!(!challenge.responded);
    }

    #[test]
    fn byzantine_validator_verify_response() {
        let validator = ByzantineValidator::new();
        let challenge = validator.issue_challenge(1, test_node_id(), vec![1, 2], 1000, 5000);
        let response = ByzantineResponse::new(1, test_node_id(), vec![9, 9], 2000);
        assert!(validator.verify_response(&response).unwrap());
    }

    #[test]
    fn byzantine_validator_verify_response_wrong_node() {
        let validator = ByzantineValidator::new();
        validator.issue_challenge(1, test_node_id(), vec![1, 2], 1000, 5000);
        let response = ByzantineResponse::new(1, test_node_id2(), vec![9, 9], 2000);
        assert!(!validator.verify_response(&response).unwrap());
    }

    #[test]
    fn byzantine_validator_detect_double_vote() {
        let validator = ByzantineValidator::new();
        assert!(!validator.detect_double_vote(1, test_node_id(), ByzantineVote::Agree).unwrap());
        assert!(validator.detect_double_vote(1, test_node_id(), ByzantineVote::Disagree).unwrap());
    }

    #[test]
    fn byzantine_validator_clear_expired() {
        let validator = ByzantineValidator::new();
        validator.issue_challenge(1, test_node_id(), vec![1], 1000, 2000);
        validator.clear_expired(3000);
        // Challenge should be removed
        let challenges = validator.challenges.borrow();
        assert!(challenges.is_empty());
    }

    // ========================================================================
    // ByzantineEngine tests
    // ========================================================================

    #[test]
    fn byzantine_engine_new() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        assert_eq!(engine.stats().total_nodes, 0);
        assert_eq!(engine.next_round_id.borrow().clone(), 1);
    }

    #[test]
    fn byzantine_engine_configure() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let config = ByzantineConfig::default().with_max_rounds(10);
        assert!(engine.configure(config).is_ok());
    }

    #[test]
    fn byzantine_engine_register_node() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        assert_eq!(engine.stats().total_nodes, 1);
    }

    #[test]
    fn byzantine_engine_update_score() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        let score = engine.update_behavior_score(test_node_id(), -60, "bad").unwrap();
        assert_eq!(score, -60);
    }

    #[test]
    fn byzantine_engine_suspect_and_quarantine() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.suspect(test_node_id(), b"evidence".to_vec()).unwrap();
        assert!(engine.is_byzantine(test_node_id()).unwrap());
        engine.quarantine(test_node_id()).unwrap();
        assert!(engine.is_quarantined(test_node_id()).unwrap());
    }

    #[test]
    fn byzantine_engine_evict() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.evict(test_node_id()).unwrap();
        assert!(engine.is_byzantine(test_node_id()).unwrap());
    }

    #[test]
    fn byzantine_engine_reintegrate() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.quarantine(test_node_id()).unwrap();
        engine.reintegrate(test_node_id()).unwrap();
        assert_eq!(engine.stats().recovering_nodes, 1);
    }

    #[test]
    fn byzantine_engine_start_round() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let round = engine.start_round(1, test_node_id(), vec![1, 2, 3]).unwrap();
        assert_eq!(round.round_id, 1);
        assert_eq!(round.proposer, test_node_id());
        assert_eq!(engine.stats().total_rounds, 1);
    }

    #[test]
    fn byzantine_engine_cast_vote() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id2(), ByzantineVote::Agree).unwrap();
        assert_eq!(engine.stats().total_votes, 2);
    }

    #[test]
    fn byzantine_engine_agreement_reached() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id2(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id3(), ByzantineVote::Abstain).unwrap();
        assert!(engine.is_byzantine_agreement_reached(1).unwrap());
    }

    #[test]
    fn byzantine_engine_commit_round() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![]).unwrap();
        engine.commit_round(1).unwrap();
        assert_eq!(engine.stats().committed_rounds, 1);
    }

    #[test]
    fn byzantine_engine_issue_challenge() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let challenge = engine.issue_challenge(test_node_id(), vec![1, 2, 3]).unwrap();
        assert_eq!(challenge.target_node, test_node_id());
        assert_eq!(engine.stats().total_challenges, 1);
    }

    #[test]
    fn byzantine_engine_verify_response() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let challenge = engine.issue_challenge(test_node_id(), vec![1, 2]).unwrap();
        let response = ByzantineResponse::new(challenge.challenge_id, test_node_id(), vec![9, 9], 2000);
        assert!(engine.verify_response(test_node_id(), response).unwrap());
    }

    #[test]
    fn byzantine_engine_detect_double_vote() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        assert!(engine.detect_double_vote(test_node_id(), 1).unwrap());
    }

    #[test]
    fn byzantine_engine_slash() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        let record = engine.slash(test_node_id(), 100, "double vote").unwrap();
        assert_eq!(record.amount, 100);
        assert_eq!(engine.stats().total_slashes, 1);
        assert_eq!(engine.stats().total_slashed, 100);
    }

    #[test]
    fn byzantine_engine_generate_certificate() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![1, 2, 3]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id2(), ByzantineVote::Agree).unwrap();
        engine.commit_round(1).unwrap();
        let cert = engine.generate_certificate(1).unwrap();
        assert_eq!(cert.round_id, 1);
        assert_eq!(cert.agree_count, 2);
    }

    #[test]
    fn byzantine_engine_verify_certificate() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let mut cert = ByzantineCertificate::new(1, test_node_id(), [0u8; 32], 7, 10, 0);
        for i in 0..7 {
            cert.add_signature(NodeId::new([i; 32]), Signature([i as u8; 64]));
        }
        assert!(engine.verify_certificate(&cert).unwrap());
    }

    #[test]
    fn byzantine_engine_stats() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.healthy_nodes, 1);
    }

    #[test]
    fn byzantine_engine_health_score() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        let score = engine.health_score(test_node_id()).unwrap();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn byzantine_engine_node_ids() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        let ids = engine.node_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn byzantine_engine_quarantined_and_evicted() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        engine.quarantine(test_node_id()).unwrap();
        engine.evict(test_node_id2()).unwrap();
        assert_eq!(engine.quarantined_ids().len(), 1);
        assert_eq!(engine.evicted_ids().len(), 1);
    }

    #[test]
    fn byzantine_error_category() {
        assert_eq!(ByzantineError::NodeNotRegistered { node_id: test_node_id() }.category(), "node_not_registered");
        assert_eq!(ByzantineError::RoundNotFound { round_id: 1 }.category(), "round_not_found");
        assert_eq!(ByzantineError::Internal { message: "x".to_string() }.category(), "internal");
    }

    #[test]
    fn byzantine_error_is_recoverable() {
        assert!(ByzantineError::NodeNotRegistered { node_id: test_node_id() }.is_recoverable());
        assert!(!ByzantineError::NodeEvicted { node_id: test_node_id() }.is_recoverable());
    }

    #[test]
    fn byzantine_error_suggested_action() {
        assert_eq!(ByzantineError::NodeNotRegistered { node_id: test_node_id() }.suggested_action(), "register the node first");
        assert_eq!(ByzantineError::NodeEvicted { node_id: test_node_id() }.suggested_action(), "node is evicted, cannot participate");
    }

    #[test]
    fn byzantine_error_is_byzantine_fault() {
        assert!(ByzantineError::NodeEvicted { node_id: test_node_id() }.is_byzantine_fault());
        assert!(!ByzantineError::RoundNotFound { round_id: 1 }.is_byzantine_fault());
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn byzantine_full_flow() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());

        // Register nodes
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        engine.register_node(test_node_id3(), test_endpoint()).unwrap();

        // Update scores
        engine.update_behavior_score(test_node_id(), -30, "suspicious").unwrap();
        engine.update_behavior_score(test_node_id2(), -80, "very bad").unwrap();

        // Quarantine bad node
        engine.quarantine(test_node_id2()).unwrap();

        // Start agreement round
        engine.start_round(1, test_node_id(), vec![1, 2, 3]).unwrap();

        // Cast votes from healthy nodes only
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id3(), ByzantineVote::Agree).unwrap();

        // Check agreement
        assert!(engine.is_byzantine_agreement_reached(1).unwrap());

        // Commit
        engine.commit_round(1).unwrap();

        // Generate certificate
        let cert = engine.generate_certificate(1).unwrap();
        assert!(engine.verify_certificate(&cert).unwrap());

        // Challenge and response
        let challenge = engine.issue_challenge(test_node_id(), vec![1, 2, 3]).unwrap();
        let response = ByzantineResponse::new(challenge.challenge_id, test_node_id(), vec![9, 9], 2000);
        assert!(engine.verify_response(test_node_id(), response).unwrap());

        // Slash
        engine.slash(test_node_id2(), 100, "byzantine behavior").unwrap();

        // Final state
        let stats = engine.stats();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.quarantined_nodes, 1);
        assert_eq!(stats.committed_rounds, 1);
        assert_eq!(stats.total_slashes, 1);
        assert!(stats.byzantine_ratio() > 0.0);
    }

    #[test]
    fn byzantine_multiple_rounds() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();

        for i in 1..=3 {
            engine.start_round(i, test_node_id(), vec![i as u8]).unwrap();
            engine.cast_byzantine_vote(i, test_node_id(), ByzantineVote::Agree).unwrap();
            engine.cast_byzantine_vote(i, test_node_id2(), ByzantineVote::Agree).unwrap();
            engine.commit_round(i).unwrap();
        }

        assert_eq!(engine.stats().total_rounds, 3);
        assert_eq!(engine.stats().committed_rounds, 3);
    }

    #[test]
    fn byzantine_challenge_expiry() {
        let engine = ByzantineEngine::new(ByzantineConfig::default().with_challenge_timeout_ms(100));
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        let challenge = engine.issue_challenge(test_node_id(), vec![1]).unwrap();
        assert!(challenge.is_expired(5000));
    }

    // ========================================================================
    // Extended Edge Case Tests
    // ========================================================================

    #[test]
    fn byzantine_config_strict_and_lenient() {
        let strict = ByzantineConfig::default()
            .with_suspicion_threshold(-5)
            .with_quarantine_threshold(-20)
            .with_eviction_threshold(-40);
        assert!(strict.is_strict());
        assert!(!strict.is_lenient());

        let lenient = ByzantineConfig::default()
            .with_suspicion_threshold(-20)
            .with_quarantine_threshold(-80)
            .with_eviction_threshold(-150);
        assert!(lenient.is_lenient());
        assert!(!lenient.is_strict());
    }

    #[test]
    fn byzantine_config_max_slashes_per_node() {
        let config = ByzantineConfig::default();
        assert_eq!(config.max_slashes_per_node(), 10);
    }

    #[test]
    fn byzantine_state_transitions_full_cycle() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert_eq!(node.state, ByzantineState::Healthy);
        assert!(node.state.can_participate());

        // Suspect
        node.state = ByzantineState::Suspected;
        assert!(!node.state.can_participate());

        // Quarantine
        node.state = ByzantineState::Quarantined;
        assert!(!node.state.can_participate());
        assert!(node.state.is_isolated());

        // Evict (final)
        node.state = ByzantineState::Evicted;
        assert!(!node.state.can_participate());
        assert!(node.state.is_final());
        assert!(node.state.is_isolated());

        // Recovering
        node.state = ByzantineState::Recovering;
        assert!(node.state.can_participate());
        assert!(!node.state.is_final());
    }

    #[test]
    fn byzantine_node_trustworthy_and_risk() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        // Default: score 0, reputation 1.0 -> not trustworthy (score not > 0)
        assert!(!node.is_trustworthy());
        assert!((node.risk_level() - 0.0).abs() < 0.01);

        // High score, high reputation -> trustworthy, low risk
        node.behavior_score = 50;
        node.reputation = 0.9;
        assert!(node.is_trustworthy());
        assert!(node.risk_level() < 0.2);

        // Negative score -> high risk
        node.behavior_score = -50;
        node.reputation = 0.2;
        assert!(!node.is_trustworthy());
        assert!(node.risk_level() > 0.5);

        // Double votes -> high risk
        node.behavior_score = 0;
        node.reputation = 1.0;
        node.double_votes = 1;
        assert!(node.risk_level() > 0.2);
    }

    #[test]
    fn byzantine_node_activity_tracking() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert!(!node.is_inactive(1000, 5000));
        node.mark_activity(1000);
        assert!(!node.is_inactive(2000, 5000));
        assert!(node.is_inactive(7000, 5000));
        assert_eq!(node.time_since_activity(7000), 6000);
    }

    #[test]
    fn byzantine_node_vote_counting() {
        let mut node = ByzantineNode::new(test_node_id(), test_endpoint());
        assert_eq!(node.total_votes, 0);
        assert_eq!(node.correct_votes, 0);
        assert!((node.vote_accuracy() - 1.0).abs() < 0.01);

        node.increment_votes();
        node.increment_votes();
        node.increment_correct_votes();
        assert_eq!(node.total_votes, 2);
        assert_eq!(node.correct_votes, 1);
        assert!((node.vote_accuracy() - 0.5).abs() < 0.01);
    }

    #[test]
    fn byzantine_vote_record_weight_and_signature() {
        let agree = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 1000);
        assert_eq!(agree.weight(), 1);
        assert!(!agree.is_signed());
        assert_eq!(agree.voter_contribution(), 0);

        let signed = agree.with_signature(Signature([1u8; 64]));
        assert!(signed.is_signed());
        assert_eq!(signed.voter_contribution(), 1);

        let disagree = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Disagree, 1000);
        assert_eq!(disagree.weight(), -1);
        assert_eq!(disagree.voter_contribution(), 0);

        let abstain = ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Abstain, 1000);
        assert_eq!(abstain.weight(), 0);
    }

    #[test]
    fn byzantine_round_tally_and_ratio() {
        let mut round = ByzantineRound::new(1, test_node_id(), vec![], 1000);
        assert_eq!(round.vote_tally(), (0, 0, 0));
        assert!((round.agreement_ratio() - 0.0).abs() < 0.01);
        assert!((round.disagree_ratio() - 0.0).abs() < 0.01);
        assert!(!round.has_votes());

        round.add_vote(ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Agree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id2(), ByzantineVote::Agree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id3(), ByzantineVote::Disagree, 2000));
        round.add_vote(ByzantineVoteRecord::new(1, test_node_id(), ByzantineVote::Abstain, 2000));

        assert_eq!(round.vote_tally(), (2, 1, 1));
        assert_eq!(round.total_votes(), 4);
        assert!((round.agreement_ratio() - 0.5).abs() < 0.01);
        assert!((round.disagree_ratio() - 0.25).abs() < 0.01);
        assert!(round.has_votes());
    }

    #[test]
    fn byzantine_round_duration_and_age() {
        let round = ByzantineRound::new(1, test_node_id(), vec![], 1000);
        assert_eq!(round.duration_ms(1000), 0);
        assert_eq!(round.duration_ms(5000), 4000);
        assert!(!round.is_older_than(2000, 5000));
        assert!(round.is_older_than(7000, 5000));
    }

    #[test]
    fn byzantine_certificate_quorum_and_signers() {
        let mut cert = ByzantineCertificate::new(1, test_node_id(), [0u8; 32], 0, 10, 0);
        assert!(!cert.is_valid(0.67));
        assert!((cert.quorum_ratio() - 0.0).abs() < 0.01);
        assert!(cert.signers().is_empty());

        for i in 0..7 {
            cert.add_signature(NodeId::new([i as u8; 32]), Signature([i as u8; 64]));
        }
        assert!(cert.is_valid(0.67));
        assert!((cert.quorum_ratio() - 0.7).abs() < 0.01);
        assert_eq!(cert.signers().len(), 7);
    }

    #[test]
    fn byzantine_challenge_validity_and_remaining() {
        let challenge = ByzantineChallenge::new(1, test_node_id(), vec![1, 2], 1000, 5000);
        assert!(challenge.is_valid(2000));
        assert!(!challenge.is_valid(6000)); // expired
        assert_eq!(challenge.time_remaining_ms(2000), 3000);
        assert_eq!(challenge.time_remaining_ms(6000), 0);
    }

    #[test]
    fn byzantine_response_data_methods() {
        let response = ByzantineResponse::new(1, test_node_id(), vec![1, 2, 3], 2000);
        assert!(response.has_data());
        assert_eq!(response.len(), 3);
        assert!(!response.is_empty());

        let empty = ByzantineResponse::new(2, test_node_id(), vec![], 2000);
        assert!(!empty.has_data());
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn byzantine_slash_record_recent_and_age() {
        let record = ByzantineSlashRecord::new(1, test_node_id(), 50, "test".to_string(), 1000, test_node_id2());
        assert!(record.is_recent(2000, 5000));
        assert!(!record.is_recent(7000, 5000));
        assert_eq!(record.time_since_slash(6000), 5000);
    }

    #[test]
    fn byzantine_quarantine_needs_review_and_evidence() {
        let mut q = ByzantineQuarantine::new(test_node_id(), 1000, "bad".to_string());
        assert!(!q.needs_review(2000, 5000));
        assert!(q.needs_review(7000, 5000));
        assert_eq!(q.evidence_count(), 0);
        q.add_evidence(b"e1".to_vec());
        q.add_evidence(b"e2".to_vec());
        assert_eq!(q.evidence_count(), 2);
    }

    #[test]
    fn byzantine_reputation_critical_good_and_ratios() {
        let mut rep = ByzantineReputation::new(test_node_id());
        assert!(rep.is_good());
        assert!(!rep.is_critical());
        assert_eq!(rep.net_interactions(), 0);
        assert!((rep.slash_ratio() - 0.0).abs() < 0.01);

        rep.record_negative(1000);
        rep.record_negative(2000);
        rep.total_slashed = 100;
        assert!(rep.is_critical());
        assert!(!rep.is_good());
        assert_eq!(rep.net_interactions(), -2);
        assert!((rep.slash_ratio() - 50.0).abs() < 0.01);
    }

    #[test]
    fn byzantine_stats_extended_ratios() {
        let mut stats = ByzantineStats::default();
        stats.total_nodes = 10;
        stats.healthy_nodes = 6;
        stats.recovering_nodes = 1;
        stats.quarantined_nodes = 2;
        stats.evicted_nodes = 1;
        stats.total_votes = 100;
        stats.total_double_votes = 5;
        stats.total_slashes = 10;

        assert_eq!(stats.total_isolated(), 3);
        assert_eq!(stats.total_participating(), 7);
        assert!((stats.participation_ratio() - 0.7).abs() < 0.01);
        assert!((stats.double_vote_rate() - 0.05).abs() < 0.01);
        assert!((stats.slash_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn byzantine_monitor_extended() {
        let mut monitor = ByzantineMonitor::new(ByzantineConfig::default());
        monitor.register_node(test_node_id(), test_endpoint());
        monitor.register_node(test_node_id2(), test_endpoint());
        assert_eq!(monitor.node_count(), 2);
        assert_eq!(monitor.quarantined_count(), 0);
        assert_eq!(monitor.slash_count(), 0);

        let ids = monitor.all_node_ids();
        assert_eq!(ids.len(), 2);

        monitor.reset();
        assert_eq!(monitor.node_count(), 0);
        assert_eq!(monitor.quarantined_count(), 0);
    }

    #[test]
    fn byzantine_validator_extended() {
        let validator = ByzantineValidator::new();
        validator.issue_challenge(1, test_node_id(), vec![1], 1000, 5000);
        validator.issue_challenge(2, test_node_id2(), vec![2], 1000, 5000);
        assert_eq!(validator.challenge_count(), 2);
        assert_eq!(validator.all_challenge_ids(), vec![1, 2]);
        assert_eq!(validator.response_count(), 0);

        validator.record_response(ByzantineResponse::new(1, test_node_id(), vec![9], 2000));
        assert_eq!(validator.response_count(), 1);

        validator.clear_all();
        assert_eq!(validator.challenge_count(), 0);
        assert_eq!(validator.response_count(), 0);
    }

    #[test]
    fn byzantine_error_classification() {
        assert!(ByzantineError::NodeNotRegistered { node_id: test_node_id() }.is_node_state_error());
        assert!(ByzantineError::NodeEvicted { node_id: test_node_id() }.is_node_state_error());
        assert!(ByzantineError::RoundNotFound { round_id: 1 }.is_round_error());
        assert!(ByzantineError::ChallengeNotFound { challenge_id: 1 }.is_challenge_error());
        assert!(!ByzantineError::NodeNotRegistered { node_id: test_node_id() }.is_round_error());
    }

    // ========================================================================
    // Engine Extended Method Tests
    // ========================================================================

    #[test]
    fn byzantine_engine_process_challenge_response() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();

        // Valid response -> score goes up
        let challenge = engine.issue_challenge(test_node_id(), vec![1, 2]).unwrap();
        let response = ByzantineResponse::new(challenge.challenge_id, test_node_id(), vec![9, 9], 2000);
        assert!(engine.process_challenge_response(test_node_id(), response).unwrap());
        assert_eq!(engine.behavior_score(test_node_id()).unwrap(), 5);

        // Expired challenge -> invalid -> score goes down
        let challenge2 = engine.issue_challenge(test_node_id(), vec![3, 4]).unwrap();
        let response2 = ByzantineResponse::new(challenge2.challenge_id, test_node_id(), vec![9, 9], 50000);
        assert!(!engine.process_challenge_response(test_node_id(), response2).unwrap());
        assert_eq!(engine.behavior_score(test_node_id()).unwrap(), -5);
    }

    #[test]
    fn byzantine_engine_finalize_round() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        engine.register_node(test_node_id3(), test_endpoint()).unwrap();

        engine.start_round(1, test_node_id(), vec![1, 2, 3]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id2(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id3(), ByzantineVote::Abstain).unwrap();

        let state = engine.finalize_round(1).unwrap();
        assert_eq!(state, ByzantineRoundState::Agreed);
        assert_eq!(engine.stats().committed_rounds, 1);
    }

    #[test]
    fn byzantine_engine_finalize_round_no_quorum() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();

        engine.start_round(1, test_node_id(), vec![1, 2, 3]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, test_node_id2(), ByzantineVote::Disagree).unwrap();

        let state = engine.finalize_round(1).unwrap();
        assert_eq!(state, ByzantineRoundState::Aborted);
        assert_eq!(engine.stats().aborted_rounds, 1);
    }

    #[test]
    fn byzantine_engine_abort_and_timeout() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.start_round(1, test_node_id(), vec![]).unwrap();

        engine.abort_round(1).unwrap();
        assert_eq!(engine.stats().aborted_rounds, 1);

        engine.start_round(2, test_node_id(), vec![]).unwrap();
        engine.timeout_round(2).unwrap();
        assert_eq!(engine.stats().timed_out_rounds, 1);

        // Already decided round cannot be aborted
        let err = engine.abort_round(1).unwrap_err();
        assert!(matches!(err, ByzantineError::RoundAlreadyDecided { .. }));
    }

    #[test]
    fn byzantine_engine_record_double_vote() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.start_round(1, test_node_id(), vec![]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();

        assert!(engine.record_double_vote(test_node_id(), 1).unwrap());
        assert_eq!(engine.stats().total_double_votes, 1);
        assert_eq!(engine.stats().total_slashes, 1);
    }

    #[test]
    fn byzantine_engine_health_report() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();

        let report = engine.get_health_report(test_node_id()).unwrap();
        assert_eq!(report.node_id, test_node_id());
        assert_eq!(report.behavior_score, 0);
        assert!((report.reputation_score - 1.0).abs() < 0.01);
        assert!((report.health_score - 1.0).abs() < 0.01);
        assert!(!report.is_byzantine);
        assert!(!report.is_quarantined);
        assert_eq!(report.total_slashes, 0);
    }

    #[test]
    fn byzantine_engine_quarantine_report() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.quarantine(test_node_id()).unwrap();

        let reports = engine.get_quarantine_report();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].node_id, test_node_id());
        assert_eq!(reports[0].reason, "quarantined");
        assert_eq!(reports[0].evidence_count, 0);
        assert!(!reports[0].reviewed);
    }

    #[test]
    fn byzantine_engine_revoke_slash() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();

        let record = engine.slash(test_node_id(), 100, "test").unwrap();
        assert_eq!(engine.stats().total_slashes, 1);
        assert_eq!(engine.stats().total_slashed, 100);

        assert!(engine.revoke_slash(record.slash_id).unwrap());
        assert_eq!(engine.stats().total_slashes, 0);
        assert_eq!(engine.stats().total_slashed, 0);

        // Revoke non-existent
        let err = engine.revoke_slash(999).unwrap_err();
        assert!(matches!(err, ByzantineError::Internal { .. }));
    }

    #[test]
    fn byzantine_engine_clear_all_evidence() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.suspect(test_node_id(), b"evidence1".to_vec()).unwrap();
        engine.suspect(test_node_id(), b"evidence2".to_vec()).unwrap();

        let evidence = engine.get_evidence_for_node(test_node_id()).unwrap();
        assert_eq!(evidence.len(), 2);

        engine.clear_all_evidence(test_node_id()).unwrap();
        let evidence = engine.get_evidence_for_node(test_node_id()).unwrap();
        assert!(evidence.is_empty());
    }

    #[test]
    fn byzantine_engine_has_node_and_counts() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        assert!(!engine.has_node(test_node_id()));
        assert_eq!(engine.node_count(), 0);

        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        assert!(engine.has_node(test_node_id()));
        assert_eq!(engine.node_count(), 1);
    }

    #[test]
    fn byzantine_engine_all_collections() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        engine.start_round(1, test_node_id(), vec![1]).unwrap();
        engine.cast_byzantine_vote(1, test_node_id(), ByzantineVote::Agree).unwrap();
        engine.commit_round(1).unwrap();
        let cert = engine.generate_certificate(1).unwrap();
        engine.slash(test_node_id(), 50, "test").unwrap();

        assert_eq!(engine.all_nodes().len(), 2);
        assert_eq!(engine.all_rounds().len(), 1);
        assert_eq!(engine.all_certificates().len(), 1);
        assert_eq!(engine.all_slashes().len(), 1);
        assert_eq!(engine.all_reputations().len(), 2);
        assert_eq!(engine.all_votes().len(), 1);
        assert_eq!(engine.all_quarantines().len(), 0);
    }

    #[test]
    fn byzantine_engine_reset_and_clear() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.slash(test_node_id(), 100, "test").unwrap();

        engine.reset_stats();
        assert_eq!(engine.stats().total_slashes, 0);

        engine.clear_rounds();
        assert_eq!(engine.round_count(), 0);

        engine.clear_certificates();
        assert_eq!(engine.certificate_count(), 0);

        engine.clear_slashes();
        assert_eq!(engine.slash_count(), 0);
    }

    #[test]
    fn byzantine_engine_byzantine_quorum_compromised() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        // Default config: agreement_quorum = 0.67, so threshold = 0.33
        // With 0 Byzantine nodes, ratio = 0, not compromised
        assert!(!engine.byzantine_quorum_compromised());

        engine.register_node(test_node_id(), test_endpoint()).unwrap();
        engine.register_node(test_node_id2(), test_endpoint()).unwrap();
        engine.register_node(test_node_id3(), test_endpoint()).unwrap();

        // Evict 1 of 3 = 0.33 ratio, threshold = 1 - 0.67 = 0.33, not strictly greater
        engine.evict(test_node_id()).unwrap();
        assert!(!engine.byzantine_quorum_compromised());
    }

    #[test]
    fn byzantine_engine_next_ids() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        assert_eq!(engine.next_round_id(), 1);
        assert_eq!(engine.next_challenge_id(), 1);

        engine.start_round(1, test_node_id(), vec![]).unwrap();
        assert_eq!(engine.next_round_id(), 1); // next_round_id not incremented by start_round

        engine.issue_challenge(test_node_id(), vec![1]).unwrap();
        assert_eq!(engine.next_challenge_id(), 2);
    }

    #[test]
    fn byzantine_engine_config_borrow() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());
        let config = engine.config_borrow();
        assert_eq!(config.max_rounds, 5);
        drop(config);

        let new_config = ByzantineConfig::default().with_max_rounds(10);
        engine.configure(new_config).unwrap();
        let config = engine.config_borrow();
        assert_eq!(config.max_rounds, 10);
    }

    #[test]
    fn byzantine_node_id_zero_vs_real() {
        let zero = NodeId::ZERO;
        let real = test_node_id();
        assert_ne!(zero, real);
        assert_ne!(zero, test_node_id2());
        assert_ne!(test_node_id(), test_node_id2());
    }

    #[test]
    fn byzantine_endpoint_clone() {
        let ep1 = test_endpoint();
        let ep2 = ep1.clone();
        assert_eq!(ep1.carrier_type, ep2.carrier_type);
        assert_eq!(ep1.address, ep2.address);
    }

    #[test]
    fn byzantine_signature_clone() {
        let sig1 = Signature([42u8; 64]);
        let sig2 = sig1.clone();
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn byzantine_full_adversarial_scenario() {
        let engine = ByzantineEngine::new(ByzantineConfig::default());

        // Register 5 nodes
        for i in 0..5 {
            let mut bytes = [0u8; 32];
            bytes[0] = i as u8;
            let nid = NodeId::new(bytes);
            let ep = Endpoint {
                carrier_type: crate::types::CarrierType::Direct,
                address: vec![i as u8, 0, 0, 0],
            };
            engine.register_node(nid, ep).unwrap();
        }

        // Node 1 becomes suspicious
        let mut bytes1 = [0u8; 32];
        bytes1[0] = 1;
        engine.update_behavior_score(NodeId::new(bytes1), -20, "suspicious").unwrap();

        // Node 2 becomes very bad -> quarantined
        let mut bytes2 = [0u8; 32];
        bytes2[0] = 2;
        engine.update_behavior_score(NodeId::new(bytes2), -60, "malicious").unwrap();
        engine.quarantine(NodeId::new(bytes2)).unwrap();

        // Node 3 double votes
        let mut bytes3 = [0u8; 32];
        bytes3[0] = 3;
        engine.start_round(1, NodeId::new(bytes1), vec![1, 2, 3]).unwrap();
        engine.cast_byzantine_vote(1, NodeId::new(bytes1), ByzantineVote::Agree).unwrap();
        engine.cast_byzantine_vote(1, NodeId::new(bytes3), ByzantineVote::Agree).unwrap();
        assert!(engine.record_double_vote(NodeId::new(bytes3), 1).unwrap());

        // Node 4 fails challenge
        let mut bytes4 = [0u8; 32];
        bytes4[0] = 4;
        engine.issue_challenge(NodeId::new(bytes4), vec![1]).unwrap();
        // expired challenge response -> invalid
        let challenge = engine.issue_challenge(NodeId::new(bytes4), vec![2]).unwrap();
        let response = ByzantineResponse::new(challenge.challenge_id, NodeId::new(bytes4), vec![], 50000);
        assert!(!engine.verify_response(NodeId::new(bytes4), response).unwrap());

        // Check final state
        assert!(engine.is_quarantined(NodeId::new(bytes2)).unwrap());
        assert_eq!(engine.stats().total_double_votes, 1);
        assert_eq!(engine.stats().total_slashes, 1);
        assert!(engine.stats().byzantine_ratio() > 0.0);
        assert!(engine.stats().total_isolated() >= 1);
    }

    #[test]
    fn byzantine_config_serialization_roundtrip() {
        let config = ByzantineConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ByzantineConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.suspicion_threshold, deserialized.suspicion_threshold);
        assert_eq!(config.max_rounds, deserialized.max_rounds);
        assert!((config.agreement_quorum - deserialized.agreement_quorum).abs() < 0.001);
    }

    #[test]
    fn byzantine_node_serialization_roundtrip() {
        let node = ByzantineNode::new(test_node_id(), test_endpoint());
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: ByzantineNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node.node_id, deserialized.node_id);
        assert_eq!(node.state, deserialized.state);
        assert_eq!(node.behavior_score, deserialized.behavior_score);
    }

    #[test]
    fn byzantine_round_serialization_roundtrip() {
        let round = ByzantineRound::new(1, test_node_id(), vec![1, 2, 3], 1000);
        let json = serde_json::to_string(&round).unwrap();
        let deserialized: ByzantineRound = serde_json::from_str(&json).unwrap();
        assert_eq!(round.round_id, deserialized.round_id);
        assert_eq!(round.proposer, deserialized.proposer);
        assert_eq!(round.payload, deserialized.payload);
    }

    #[test]
    fn byzantine_certificate_serialization_roundtrip() {
        let mut cert = ByzantineCertificate::new(1, test_node_id(), [1u8; 32], 2, 3, 1000);
        cert.add_signature(test_node_id2(), Signature([2u8; 64]));
        let json = serde_json::to_string(&cert).unwrap();
        let deserialized: ByzantineCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert.round_id, deserialized.round_id);
        assert_eq!(cert.signatures.len(), deserialized.signatures.len());
    }

    #[test]
    fn byzantine_stats_serialization_roundtrip() {
        let mut stats = ByzantineStats::default();
        stats.total_nodes = 5;
        stats.committed_rounds = 3;
        stats.total_slashes = 2;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ByzantineStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_nodes, deserialized.total_nodes);
        assert_eq!(stats.committed_rounds, deserialized.committed_rounds);
        assert_eq!(stats.total_slashes, deserialized.total_slashes);
    }

    #[test]
    fn byzantine_reputation_serialization_roundtrip() {
        let mut rep = ByzantineReputation::new(test_node_id());
        rep.record_positive(1000);
        rep.record_negative(2000);
        rep.total_slashes = 1;
        let json = serde_json::to_string(&rep).unwrap();
        let deserialized: ByzantineReputation = serde_json::from_str(&json).unwrap();
        assert_eq!(rep.score, deserialized.score);
        assert_eq!(rep.positive_interactions, deserialized.positive_interactions);
        assert_eq!(rep.total_slashes, deserialized.total_slashes);
    }

    #[test]
    fn byzantine_slash_record_serialization_roundtrip() {
        let record = ByzantineSlashRecord::new(1, test_node_id(), 50, "test".to_string(), 1000, test_node_id2());
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ByzantineSlashRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.slash_id, deserialized.slash_id);
        assert_eq!(record.amount, deserialized.amount);
        assert_eq!(record.reason, deserialized.reason);
    }

    #[test]
    fn byzantine_quarantine_serialization_roundtrip() {
        let mut q = ByzantineQuarantine::new(test_node_id(), 1000, "test".to_string());
        q.add_evidence(b"e1".to_vec());
        let json = serde_json::to_string(&q).unwrap();
        let deserialized: ByzantineQuarantine = serde_json::from_str(&json).unwrap();
        assert_eq!(q.node_id, deserialized.node_id);
        assert_eq!(q.reason, deserialized.reason);
        assert_eq!(q.evidence.len(), deserialized.evidence.len());
    }

    #[test]
    fn byzantine_health_report_serialization_roundtrip() {
        let report = ByzantineHealthReport {
            node_id: test_node_id(),
            behavior_score: 10,
            reputation_score: 0.8,
            health_score: 0.9,
            is_byzantine: false,
            is_quarantined: false,
            positive_interactions: 5,
            negative_interactions: 1,
            total_slashes: 0,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: ByzantineHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.behavior_score, deserialized.behavior_score);
        assert!((report.reputation_score - deserialized.reputation_score).abs() < 0.001);
    }

    #[test]
    fn byzantine_quarantine_report_serialization_roundtrip() {
        let report = ByzantineQuarantineReport {
            node_id: test_node_id(),
            quarantined_at_ms: 1000,
            reason: "test".to_string(),
            evidence_count: 2,
            reviewed: true,
            duration_ms: 5000,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: ByzantineQuarantineReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.node_id, deserialized.node_id);
        assert_eq!(report.evidence_count, deserialized.evidence_count);
        assert_eq!(report.reviewed, deserialized.reviewed);
    }
}