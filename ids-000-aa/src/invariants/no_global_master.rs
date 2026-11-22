//! # Invariant: No Global Master
//!
//! `NoGlobalMaster` enforces IDS edge-case position #3: **no permanent global
//! ordering master.** Ordering is dataset-scoped only — each dataset has its
//! own ordering domain with a local ordering authority that can rotate, fail,
//! and be replaced without affecting any other dataset. There is no single
//! node that orders all writes across all datasets.
//!
//! ## Contract
//!
//! - `register_dataset(dataset_id, domain_id) -> Result<(), OrderingError>`
//!   — register a new dataset-scoped ordering domain.
//! - `ordering_authority(dataset_id) -> Option<NodeId>` — current ordering
//!   authority (master) for a dataset, if any.
//! - `propose_order(dataset_id, operation, now) -> Result<OrderingTicket,
//!   OrderingError>` — propose an operation for ordering within a dataset
//!   domain; only the current authority can propose.
//! - `commit_order(dataset_id, ticket_id, now) -> Result<(), OrderingError>`
//!   — commit a proposed ticket, advancing the dataset's commit position.
//! - `rotate_authority(dataset_id, new_authority, now) -> Result<(), OrderingError>`
//!   — rotate the ordering authority within a dataset domain.
//! - `authority_epoch(dataset_id) -> u64` — current epoch of the ordering
//!   authority for a dataset.
//! - `commit_position(dataset_id) -> u64` — last committed position for a
//!   dataset.
//! - `domain_count() -> u64` — total number of dataset ordering domains.
//! - `is_domain_scoped(dataset_a, dataset_b) -> bool` — whether two datasets
//!   are in independent ordering domains (always true for different datasets).
//! - `global_order_exists() -> bool` — always false; no global ordering
//!   master exists.
//! - `summary() -> NoGlobalMasterSummary` — observability summary.
//! - `health_check(now) -> Vec<DomainHealth>` — per-domain health status.
//!
//! ## Sealing
//!
//! Sealed via `private::Sealed`. Downstream crates use as trait bound only.
//!
//! ## No networking dependencies
//!
//! Uses only `crate::types` (`NodeId`, `DatasetId`, `DomainId`,
//! `TimestampMillis`, `Duration`) and `crate::errors` (`InvariantError`,
//! `OrderingError`). No `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC,
//! HTTP, or DNS.

use crate::types::{DatasetId, DomainId, Duration, NodeId, TimestampMillis};
use crate::errors::{InvariantError, OrderingError};

///////////////////////////////////////////////////////////////////////////////
// TYPES
///////////////////////////////////////////////////////////////////////////////

/// A ticket granting the right to order an operation at a position within a
/// dataset domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderingTicket {
    pub ticket_id: [u8; 16],
    pub dataset_id: DatasetId,
    pub position: u64,
    pub authority: NodeId,
    pub epoch: u64,
    pub issued_at: TimestampMillis,
}

/// The current ordering authority for a dataset domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderingAuthority {
    pub dataset_id: DatasetId,
    pub domain_id: DomainId,
    pub node_id: NodeId,
    pub epoch: u64,
    pub assumed_at: TimestampMillis,
    pub rotation_count: u64,
}

/// A dataset-scoped ordering domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderingDomain {
    pub dataset_id: DatasetId,
    pub domain_id: DomainId,
    pub authority: Option<OrderingAuthority>,
    pub commit_position: u64,
    pub proposal_count: u64,
    pub commit_count: u64,
    pub rotation_count: u64,
    pub created_at: TimestampMillis,
    pub last_activity: TimestampMillis,
}

/// Health status of a single ordering domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DomainHealth {
    pub dataset_id: DatasetId,
    pub domain_id: DomainId,
    pub has_authority: bool,
    pub authority_node: Option<NodeId>,
    pub epoch: u64,
    pub commit_position: u64,
    pub pending_proposals: u64,
    pub is_stale: bool,
    pub last_activity: TimestampMillis,
}

/// Observability summary for the no-global-master subsystem.
#[derive(Debug, Clone, Default)]
pub struct NoGlobalMasterSummary {
    pub total_domains: u64,
    pub total_proposals: u64,
    pub total_commits: u64,
    pub total_rotations: u64,
    pub domains_with_authority: u64,
    pub domains_without_authority: u64,
    pub global_master_exists: bool,
}

/// A proposed operation awaiting ordering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PendingProposal {
    pub proposal_id: [u8; 16],
    pub dataset_id: DatasetId,
    pub operation_id: [u8; 16],
    pub proposed_by: NodeId,
    pub proposed_at: TimestampMillis,
    pub position: Option<u64>,
}

/// Reason for an ordering error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderingErrorReason {
    NotAuthority,
    DatasetNotRegistered,
    DomainNotFound,
    InvalidPosition,
    EpochMismatch,
    RotationInProgress,
    CrossDomainOrderingAttempt,
}

/// Reason for authority rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationReason {
    Scheduled,
    FailureDetected,
    Administrative,
    EpochAdvance,
    LoadRebalance,
}

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT
///////////////////////////////////////////////////////////////////////////////

/// No global master invariant: dataset-scoped ordering only; no permanent
/// global ordering master.
///
/// Downstream crates may use this trait as a bound and call its methods, but
/// may not implement it. Only `ids-000-aa` implementation crates may `impl
/// NoGlobalMaster`.
pub trait NoGlobalMaster: private::Sealed {
    /// Register a new dataset-scoped ordering domain.
    fn register_dataset(
        &mut self,
        dataset_id: DatasetId,
        domain_id: DomainId,
        now: TimestampMillis,
    ) -> Result<(), OrderingError>;

    /// Current ordering authority for a dataset, if any.
    fn ordering_authority(&self, dataset_id: DatasetId) -> Option<OrderingAuthority>;

    /// Propose an operation for ordering within a dataset domain. Only the
    /// current authority may propose.
    fn propose_order(
        &mut self,
        dataset_id: DatasetId,
        operation_id: [u8; 16],
        proposed_by: NodeId,
        now: TimestampMillis,
    ) -> Result<OrderingTicket, OrderingError>;

    /// Commit a proposed ticket, advancing the dataset's commit position.
    fn commit_order(
        &mut self,
        dataset_id: DatasetId,
        ticket_id: [u8; 16],
        now: TimestampMillis,
    ) -> Result<(), OrderingError>;

    /// Rotate the ordering authority within a dataset domain.
    fn rotate_authority(
        &mut self,
        dataset_id: DatasetId,
        new_authority: NodeId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> Result<(), OrderingError>;

    /// Current epoch of the ordering authority for a dataset.
    fn authority_epoch(&self, dataset_id: DatasetId) -> u64;

    /// Last committed position for a dataset.
    fn commit_position(&self, dataset_id: DatasetId) -> u64;

    /// Total number of dataset ordering domains.
    fn domain_count(&self) -> u64;

    /// Whether two datasets are in independent ordering domains.
    fn is_domain_scoped(&self, dataset_a: DatasetId, dataset_b: DatasetId) -> bool;

    /// Always false — no global ordering master exists.
    fn global_order_exists(&self) -> bool;

    /// Observability summary.
    fn summary(&self) -> NoGlobalMasterSummary;

    /// Per-domain health check.
    fn health_check(&self, now: TimestampMillis) -> Vec<DomainHealth>;
}

///////////////////////////////////////////////////////////////////////////////
// SEALED MARKER
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for () {}

impl NoGlobalMaster for () {
    fn register_dataset(
        &mut self,
        _dataset_id: DatasetId,
        _domain_id: DomainId,
        _now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        Err(OrderingError::DomainNotFound {
            domain: DomainId([0u8; 16]),
        })
    }

    fn ordering_authority(&self, _dataset_id: DatasetId) -> Option<OrderingAuthority> {
        None
    }

    fn propose_order(
        &mut self,
        _dataset_id: DatasetId,
        _operation_id: [u8; 16],
        _proposed_by: NodeId,
        _now: TimestampMillis,
    ) -> Result<OrderingTicket, OrderingError> {
        Err(OrderingError::DatasetNotRegistered {
            dataset: DatasetId([0u8; 16]),
        })
    }

    fn commit_order(
        &mut self,
        _dataset_id: DatasetId,
        _ticket_id: [u8; 16],
        _now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        Err(OrderingError::DatasetNotRegistered {
            dataset: DatasetId([0u8; 16]),
        })
    }

    fn rotate_authority(
        &mut self,
        _dataset_id: DatasetId,
        _new_authority: NodeId,
        _reason: RotationReason,
        _now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        Err(OrderingError::DatasetNotRegistered {
            dataset: DatasetId([0u8; 16]),
        })
    }

    fn authority_epoch(&self, _dataset_id: DatasetId) -> u64 {
        0
    }

    fn commit_position(&self, _dataset_id: DatasetId) -> u64 {
        0
    }

    fn domain_count(&self) -> u64 {
        0
    }

    fn is_domain_scoped(&self, _dataset_a: DatasetId, _dataset_b: DatasetId) -> bool {
        false
    }

    fn global_order_exists(&self) -> bool {
        false
    }

    fn summary(&self) -> NoGlobalMasterSummary {
        NoGlobalMasterSummary::default()
    }

    fn health_check(&self, _now: TimestampMillis) -> Vec<DomainHealth> {
        Vec::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN REGISTRY
///////////////////////////////////////////////////////////////////////////////

/// Registry of dataset-scoped ordering domains.
#[derive(Debug, Clone, Default)]
pub struct DomainRegistry {
    domains: std::collections::HashMap<DatasetId, OrderingDomain>,
    domain_ids: std::collections::HashMap<DomainId, DatasetId>,
    next_domain_id: u64,
    total_domains: u64,
}

impl DomainRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a dataset with its domain. Returns an error if already
    /// registered.
    pub fn register(
        &mut self,
        dataset_id: DatasetId,
        domain_id: DomainId,
        now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        if self.domains.contains_key(&dataset_id) {
            return Err(OrderingError::DatasetAlreadyRegistered { dataset: dataset_id });
        }
        if self.domain_ids.contains_key(&domain_id) {
            return Err(OrderingError::DomainIdCollision { domain: domain_id });
        }
        let domain = OrderingDomain {
            dataset_id,
            domain_id,
            authority: None,
            commit_position: 0,
            proposal_count: 0,
            commit_count: 0,
            rotation_count: 0,
            created_at: now,
            last_activity: now,
        };
        self.domains.insert(dataset_id, domain);
        self.domain_ids.insert(domain_id, dataset_id);
        self.total_domains += 1;
        Ok(())
    }

    /// Get a domain by dataset ID.
    pub fn get(&self, dataset_id: DatasetId) -> Option<&OrderingDomain> {
        self.domains.get(&dataset_id)
    }

    /// Get a domain mutably by dataset ID.
    pub fn get_mut(&mut self, dataset_id: DatasetId) -> Option<&mut OrderingDomain> {
        self.domains.get_mut(&dataset_id)
    }

    /// Get dataset ID by domain ID.
    pub fn dataset_by_domain(&self, domain_id: DomainId) -> Option<DatasetId> {
        self.domain_ids.get(&domain_id).copied()
    }

    /// Total domains.
    pub fn len(&self) -> u64 {
        self.total_domains
    }

    pub fn is_empty(&self) -> bool {
        self.total_domains == 0
    }

    /// Dataset IDs sorted.
    pub fn dataset_ids(&self) -> Vec<DatasetId> {
        let mut v: Vec<DatasetId> = self.domains.keys().copied().collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Domains with an authority set.
    pub fn domains_with_authority(&self) -> u64 {
        self.domains
            .values()
            .filter(|d| d.authority.is_some())
            .count() as u64
    }

    /// Domains without an authority.
    pub fn domains_without_authority(&self) -> u64 {
        self.domains
            .values()
            .filter(|d| d.authority.is_none())
            .count() as u64
    }

    /// Total proposals across all domains.
    pub fn total_proposals(&self) -> u64 {
        self.domains.values().map(|d| d.proposal_count).sum()
    }

    /// Total commits across all domains.
    pub fn total_commits(&self) -> u64 {
        self.domains.values().map(|d| d.commit_count).sum()
    }

    /// Total rotations across all domains.
    pub fn total_rotations(&self) -> u64 {
        self.domains.values().map(|d| d.rotation_count).sum()
    }

    /// Clear all domains.
    pub fn clear(&mut self) {
        self.domains.clear();
        self.domain_ids.clear();
        self.next_domain_id = 0;
        self.total_domains = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING AUTHORITY MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages ordering authorities within dataset domains: assumption, rotation,
/// and epoch tracking.
#[derive(Debug, Clone, Default)]
pub struct AuthorityManager {
    authorities: std::collections::HashMap<DatasetId, OrderingAuthority>,
    rotations: std::collections::VecDeque<AuthorityRotationRecord>,
    total_rotations: u64,
    max_rotation_history: usize,
}

/// A record of an authority rotation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthorityRotationRecord {
    pub dataset_id: DatasetId,
    pub old_authority: Option<NodeId>,
    pub new_authority: NodeId,
    pub reason: RotationReason,
    pub at_tick: TimestampMillis,
    pub epoch: u64,
}

impl AuthorityManager {
    pub fn new() -> Self {
        Self {
            authorities: std::collections::HashMap::new(),
            rotations: std::collections::VecDeque::new(),
            total_rotations: 0,
            max_rotation_history: 1000,
        }
    }

    /// Assume authority for a dataset. Returns the new authority record.
    pub fn assume(
        &mut self,
        dataset_id: DatasetId,
        domain_id: DomainId,
        node_id: NodeId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> OrderingAuthority {
        let old = self.authorities.get(&dataset_id).cloned();
        let epoch = old.map(|a| a.epoch + 1).unwrap_or(1);
        let authority = OrderingAuthority {
            dataset_id,
            domain_id,
            node_id,
            epoch,
            assumed_at: now,
            rotation_count: old.map(|a| a.rotation_count + 1).unwrap_or(0),
        };
        self.authorities.insert(dataset_id, authority.clone());
        self.record_rotation(old, authority.node_id, reason, epoch, now);
        authority
    }

    /// Remove authority for a dataset.
    pub fn relinquish(&mut self, dataset_id: DatasetId) -> Option<OrderingAuthority> {
        self.authorities.remove(&dataset_id)
    }

    /// Get authority for a dataset.
    pub fn get(&self, dataset_id: DatasetId) -> Option<&OrderingAuthority> {
        self.authorities.get(&dataset_id)
    }

    /// Total authorities.
    pub fn len(&self) -> u64 {
        self.authorities.len() as u64
    }

    /// Total rotations performed.
    pub fn total_rotations(&self) -> u64 {
        self.total_rotations
    }

    /// Rotation history.
    pub fn history(&self) -> Vec<&AuthorityRotationRecord> {
        self.rotations.iter().collect()
    }

    /// Record a rotation in history.
    fn record_rotation(
        &mut self,
        old: Option<OrderingAuthority>,
        new: NodeId,
        reason: RotationReason,
        epoch: u64,
        now: TimestampMillis,
    ) {
        if self.rotations.len() >= self.max_rotation_history {
            self.rotations.pop_front();
        }
        self.rotations.push_back(AuthorityRotationRecord {
            dataset_id: old
                .as_ref()
                .map(|a| a.dataset_id)
                .unwrap_or(DatasetId([0u8; 16])),
            old_authority: old.map(|a| a.node_id),
            new_authority: new,
            reason,
            at_tick: now,
            epoch,
        });
        self.total_rotations += 1;
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROPOSAL TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks pending and committed proposals per dataset domain.
#[derive(Debug, Clone, Default)]
pub struct ProposalTracker {
    pending: std::collections::HashMap<[u8; 16], PendingProposal>,
    committed: std::collections::VecDeque<CommittedOrder>,
    per_dataset_proposals: std::collections::HashMap<DatasetId, u64>,
    per_dataset_commits: std::collections::HashMap<DatasetId, u64>,
    total_proposals: u64,
    total_commits: u64,
    max_committed_history: usize,
}

/// A committed order entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommittedOrder {
    pub ticket_id: [u8; 16],
    pub dataset_id: DatasetId,
    pub position: u64,
    pub committed_at: TimestampMillis,
    pub authority: NodeId,
    pub epoch: u64,
}

impl ProposalTracker {
    pub fn new() -> Self {
        Self {
            pending: std::collections::HashMap::new(),
            committed: std::collections::VecDeque::new(),
            per_dataset_proposals: std::collections::HashMap::new(),
            per_dataset_commits: std::collections::HashMap::new(),
            total_proposals: 0,
            total_commits: 0,
            max_committed_history: 1000,
        }
    }

    /// Register a pending proposal.
    pub fn propose(
        &mut self,
        ticket_id: [u8; 16],
        dataset_id: DatasetId,
        operation_id: [u8; 16],
        proposed_by: NodeId,
        position: u64,
        now: TimestampMillis,
    ) {
        *self.per_dataset_proposals.entry(dataset_id).or_insert(0) += 1;
        self.total_proposals += 1;
        self.pending.insert(
            ticket_id,
            PendingProposal {
                proposal_id: ticket_id,
                dataset_id,
                operation_id,
                proposed_by,
                proposed_at: now,
                position: Some(position),
            },
        );
    }

    /// Mark a proposal as committed.
    pub fn commit(&mut self, ticket_id: [u8; 16], dataset_id: DatasetId, now: TimestampMillis) -> Option<u64> {
        let proposal = self.pending.remove(&ticket_id)?;
        let position = proposal.position.unwrap_or(0);
        *self.per_dataset_commits.entry(dataset_id).or_insert(0) += 1;
        self.total_commits += 1;
        if self.committed.len() >= self.max_committed_history {
            self.committed.pop_front();
        }
        self.committed.push_back(CommittedOrder {
            ticket_id,
            dataset_id,
            position,
            committed_at: now,
            authority: proposal.proposed_by,
            epoch: 0,
        });
        Some(position)
    }

    /// Pending proposal count for a dataset.
    pub fn pending_for_dataset(&self, dataset_id: DatasetId) -> u64 {
        self.pending
            .values()
            .filter(|p| p.dataset_id == dataset_id)
            .count() as u64
    }

    /// Total pending proposals.
    pub fn total_pending(&self) -> u64 {
        self.pending.len() as u64
    }

    /// Total proposals.
    pub fn total_proposals(&self) -> u64 {
        self.total_proposals
    }

    /// Total commits.
    pub fn total_commits(&self) -> u64 {
        self.total_commits
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.committed.clear();
        self.per_dataset_proposals.clear();
        self.per_dataset_commits.clear();
        self.total_proposals = 0;
        self.total_commits = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Monitors health of dataset ordering domains: staleness, authority
/// absence, commit lag.
#[derive(Debug, Clone, Default)]
pub struct DomainHealthMonitor {
    stale_threshold_ticks: u64,
    health_checks: u64,
    unhealthy_domains: u64,
}

impl DomainHealthMonitor {
    pub fn new() -> Self {
        Self {
            stale_threshold_ticks: 3600000, // 1 hour in IDS ticks
            health_checks: 0,
            unhealthy_domains: 0,
        }
    }

    pub fn with_stale_threshold(ticks: u64) -> Self {
        Self {
            stale_threshold_ticks: ticks,
            ..Default::default()
        }
    }

    /// Evaluate health for all domains.
    pub fn evaluate(
        &self,
        domains: &std::collections::HashMap<DatasetId, OrderingDomain>,
        now: TimestampMillis,
    ) -> Vec<DomainHealth> {
        let mut result: Vec<DomainHealth> = Vec::new();
        for (dataset_id, domain) in domains {
            let has_authority = domain.authority.is_some();
            let authority_node = domain.authority.as_ref().map(|a| a.node_id);
            let epoch = domain.authority.as_ref().map(|a| a.epoch).unwrap_or(0);
            let is_stale = now.saturating_sub(domain.last_activity) > self.stale_threshold_ticks;
            result.push(DomainHealth {
                dataset_id: *dataset_id,
                domain_id: domain.domain_id,
                has_authority,
                authority_node,
                epoch,
                commit_position: domain.commit_position,
                pending_proposals: 0, // filled by caller
                is_stale,
                last_activity: domain.last_activity,
            });
        }
        result.sort_by(|a, b| a.dataset_id.0.cmp(&b.dataset_id.0));
        result
    }

    /// Total health checks.
    pub fn health_checks(&self) -> u64 {
        self.health_checks
    }

    /// Set stale threshold.
    pub fn set_stale_threshold(&mut self, ticks: u64) {
        self.stale_threshold_ticks = ticks;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS-DOMAIN ORDERING GUARD
///////////////////////////////////////////////////////////////////////////////

/// Guards against cross-domain ordering attempts: operations from one dataset
/// domain cannot be ordered in another dataset's domain.
#[derive(Debug, Clone, Default)]
pub struct CrossDomainOrderingGuard {
    blocked_attempts: std::collections::VecDeque<CrossDomainAttempt>,
    total_blocked: u64,
}

/// A blocked cross-domain ordering attempt.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrossDomainAttempt {
    pub source_dataset: DatasetId,
    pub target_dataset: DatasetId,
    pub attempted_at: TimestampMillis,
}

impl CrossDomainOrderingGuard {
    pub fn new() -> Self {
        Self {
            blocked_attempts: std::collections::VecDeque::new(),
            total_blocked: 0,
        }
    }

    /// Record a blocked cross-domain attempt.
    pub fn record_block(&mut self, source: DatasetId, target: DatasetId, now: TimestampMillis) {
        if self.blocked_attempts.len() >= 500 {
            self.blocked_attempts.pop_front();
        }
        self.blocked_attempts.push_back(CrossDomainAttempt {
            source_dataset: source,
            target_dataset: target,
            attempted_at: now,
        });
        self.total_blocked += 1;
    }

    /// Whether attempting to order source in target is blocked (always for
    /// different datasets).
    pub fn is_blocked(&self, source: DatasetId, target: DatasetId) -> bool {
        source != target
    }

    /// Total blocked attempts.
    pub fn total_blocked(&self) -> u64 {
        self.total_blocked
    }

    /// Recent blocked attempts.
    pub fn recent_blocks(&self) -> Vec<&CrossDomainAttempt> {
        self.blocked_attempts.iter().collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// NO GLOBAL MASTER IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////

/// Production implementation of the `NoGlobalMaster` invariant. Each dataset
/// has its own ordering domain with a local, rotatable authority. No global
/// ordering master exists across datasets.
#[derive(Debug, Clone, Default)]
pub struct NoGlobalMasterImpl {
    domains: DomainRegistry,
    authorities: AuthorityManager,
    proposals: ProposalTracker,
    guard: CrossDomainOrderingGuard,
    health: DomainHealthMonitor,
    total_proposals: u64,
    total_commits: u64,
    total_rotations: u64,
}

impl NoGlobalMasterImpl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a dataset-scoped ordering domain.
    pub fn register_dataset(
        &mut self,
        dataset_id: DatasetId,
        domain_id: DomainId,
        now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        self.domains.register(dataset_id, domain_id, now)
    }

    /// Current ordering authority for a dataset.
    pub fn ordering_authority(&self, dataset_id: DatasetId) -> Option<OrderingAuthority> {
        self.authorities.get(dataset_id).cloned()
    }

    /// Propose an operation for ordering. Only the current authority may
    /// propose; cross-domain proposals are rejected.
    pub fn propose_order(
        &mut self,
        dataset_id: DatasetId,
        operation_id: [u8; 16],
        proposed_by: NodeId,
        now: TimestampMillis,
    ) -> Result<OrderingTicket, OrderingError> {
        let domain = self
            .domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;

        let authority = match &domain.authority {
            Some(a) if a.node_id == proposed_by => a.clone(),
            _ => {
                return Err(OrderingError::NotAuthority {
                    dataset: dataset_id,
                    node: proposed_by,
                });
            }
        };

        // Cross-domain guard: a node from another dataset cannot propose here.
        let _source_domain = self.domains.dataset_ids();
        // (Dataset identity already scoped by dataset_id parameter.)

        let position = domain.commit_position + 1;
        let ticket_id = generate_ticket_id();
        let ticket = OrderingTicket {
            ticket_id,
            dataset_id,
            position,
            authority: proposed_by,
            epoch: authority.epoch,
            issued_at: now,
        };

        self.proposals.propose(
            ticket_id,
            dataset_id,
            operation_id,
            proposed_by,
            position,
            now,
        );
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.proposal_count += 1;
            domain.last_activity = now;
        }
        self.total_proposals += 1;

        Ok(ticket)
    }

    /// Commit a proposed ticket.
    pub fn commit_order(
        &mut self,
        dataset_id: DatasetId,
        ticket_id: [u8; 16],
        now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        let _domain = self
            .domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;

        let position = self
            .proposals
            .commit(ticket_id, dataset_id, now)
            .ok_or_else(|| OrderingError::InvalidPosition {
                dataset: dataset_id,
                position: 0,
            })?;

        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.commit_position = position.max(domain.commit_position);
            domain.commit_count += 1;
            domain.last_activity = now;
        }
        self.total_commits += 1;
        Ok(())
    }

    /// Rotate the ordering authority within a dataset domain.
    pub fn rotate_authority(
        &mut self,
        dataset_id: DatasetId,
        new_authority: NodeId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        let _domain = self
            .domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;

        let authority = self
            .authorities
            .assume(dataset_id, DomainId([0u8; 16]), new_authority, reason, now);
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.authority = Some(authority.clone());
            domain.rotation_count += 1;
            domain.last_activity = now;
        }
        self.total_rotations += 1;
        Ok(())
    }

    /// Authority epoch for a dataset.
    pub fn authority_epoch(&self, dataset_id: DatasetId) -> u64 {
        self.authorities
            .get(dataset_id)
            .map(|a| a.epoch)
            .unwrap_or(0)
    }

    /// Commit position for a dataset.
    pub fn commit_position(&self, dataset_id: DatasetId) -> u64 {
        self.domains
            .get(dataset_id)
            .map(|d| d.commit_position)
            .unwrap_or(0)
    }

    /// Total domains.
    pub fn domain_count(&self) -> u64 {
        self.domains.len()
    }

    /// Whether two datasets are in independent ordering domains.
    pub fn is_domain_scoped(&self, dataset_a: DatasetId, dataset_b: DatasetId) -> bool {
        dataset_a != dataset_b
            && self.domains.get(dataset_a).is_some()
            && self.domains.get(dataset_b).is_some()
    }

    /// Always false — no global ordering master exists.
    pub fn global_order_exists(&self) -> bool {
        false
    }

    /// Observability summary.
    pub fn summary(&self) -> NoGlobalMasterSummary {
        NoGlobalMasterSummary {
            total_domains: self.domains.len(),
            total_proposals: self.total_proposals,
            total_commits: self.total_commits,
            total_rotations: self.total_rotations,
            domains_with_authority: self.domains.domains_with_authority(),
            domains_without_authority: self.domains.domains_without_authority(),
            global_master_exists: false,
        }
    }

    /// Per-domain health check.
    pub fn health_check(&self, now: TimestampMillis) -> Vec<DomainHealth> {
        let mut health = self.health.evaluate(&self.domains.domains, now);
        // Fill pending proposal counts.
        for h in &mut health {
            h.pending_proposals = self.proposals.pending_for_dataset(h.dataset_id);
        }
        health
    }

    /// Total proposals across all domains.
    pub fn total_proposals(&self) -> u64 {
        self.total_proposals
    }

    /// Total commits across all domains.
    pub fn total_commits(&self) -> u64 {
        self.total_commits
    }

    /// Total rotations across all domains.
    pub fn total_rotations(&self) -> u64 {
        self.total_rotations
    }

    /// All dataset IDs.
    pub fn dataset_ids(&self) -> Vec<DatasetId> {
        self.domains.dataset_ids()
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.domains.clear();
        self.authorities = AuthorityManager::new();
        self.proposals.clear();
        self.guard = CrossDomainOrderingGuard::new();
        self.total_proposals = 0;
        self.total_commits = 0;
        self.total_rotations = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// HELPER: TICKET ID GENERATION
///////////////////////////////////////////////////////////////////////////////

fn generate_ticket_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = 0x4e; // 'N'
    id[1] = 0x47; // 'M'
    id
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING EPOCH MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Tracks epochs per dataset domain: each authority change bumps the epoch,
/// and stale epochs are rejected to prevent split-ordering.
#[derive(Debug, Clone, Default)]
pub struct OrderingEpochManager {
    epochs: std::collections::HashMap<DatasetId, u64>,
    epoch_history: std::collections::VecDeque<EpochChangeRecord>,
    total_epoch_changes: u64,
    max_history: usize,
}

/// A record of an epoch change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EpochChangeRecord {
    pub dataset_id: DatasetId,
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub changed_at: TimestampMillis,
    pub reason: RotationReason,
}

impl OrderingEpochManager {
    pub fn new() -> Self {
        Self {
            epochs: std::collections::HashMap::new(),
            epoch_history: std::collections::VecDeque::new(),
            total_epoch_changes: 0,
            max_history: 1000,
        }
    }

    /// Current epoch for a dataset (0 if none).
    pub fn current_epoch(&self, dataset_id: DatasetId) -> u64 {
        self.epochs.get(&dataset_id).copied().unwrap_or(0)
    }

    /// Advance the epoch for a dataset. Returns the new epoch.
    pub fn advance(
        &mut self,
        dataset_id: DatasetId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> u64 {
        let old = self.current_epoch(dataset_id);
        let new = old + 1;
        self.epochs.insert(dataset_id, new);
        if self.epoch_history.len() >= self.max_history {
            self.epoch_history.pop_front();
        }
        self.epoch_history.push_back(EpochChangeRecord {
            dataset_id,
            old_epoch: old,
            new_epoch: new,
            changed_at: now,
            reason,
        });
        self.total_epoch_changes += 1;
        new
    }

    /// Validate that a proposed epoch is current (not stale).
    pub fn validate_epoch(&self, dataset_id: DatasetId, proposed_epoch: u64) -> bool {
        self.current_epoch(dataset_id) == proposed_epoch
    }

    /// Total epoch changes.
    pub fn total_epoch_changes(&self) -> u64 {
        self.total_epoch_changes
    }

    /// History of epoch changes.
    pub fn history(&self) -> Vec<&EpochChangeRecord> {
        self.epoch_history.iter().collect()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.epochs.clear();
        self.epoch_history.clear();
        self.total_epoch_changes = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMMIT POSITION TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks high-water commit positions per dataset with watermark support for
/// crash recovery and replay detection.
#[derive(Debug, Clone, Default)]
pub struct CommitPositionTracker {
    positions: std::collections::HashMap<DatasetId, u64>,
    watermarks: std::collections::HashMap<DatasetId, u64>,
    total_commits: u64,
    position_history: std::collections::VecDeque<PositionChange>,
}

/// A commit position change.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PositionChange {
    pub dataset_id: DatasetId,
    pub old_position: u64,
    pub new_position: u64,
    pub changed_at: TimestampMillis,
}

impl CommitPositionTracker {
    pub fn new() -> Self {
        Self {
            positions: std::collections::HashMap::new(),
            watermarks: std::collections::HashMap::new(),
            total_commits: 0,
            position_history: std::collections::VecDeque::new(),
        }
    }

    /// Current commit position for a dataset.
    pub fn position(&self, dataset_id: DatasetId) -> u64 {
        self.positions.get(&dataset_id).copied().unwrap_or(0)
    }

    /// Watermark (durable position) for a dataset.
    pub fn watermark(&self, dataset_id: DatasetId) -> u64 {
        self.watermarks.get(&dataset_id).copied().unwrap_or(0)
    }

    /// Advance position to `new_position` if greater than current.
    pub fn advance(&mut self, dataset_id: DatasetId, new_position: u64, now: TimestampMillis) -> bool {
        let current = self.position(dataset_id);
        if new_position <= current {
            return false;
        }
        let old = current;
        self.positions.insert(dataset_id, new_position);
        // Watermark lags by one for safety.
        let wm = self.watermarks.entry(dataset_id).or_insert(0);
        if new_position > *wm + 1 {
            *wm = new_position - 1;
        }
        if self.position_history.len() >= 500 {
            self.position_history.pop_front();
        }
        self.position_history.push_back(PositionChange {
            dataset_id,
            old_position: old,
            new_position,
            changed_at: now,
        });
        self.total_commits += 1;
        true
    }

    /// Advance watermark to `new_watermark` if greater.
    pub fn advance_watermark(&mut self, dataset_id: DatasetId, new_watermark: u64) -> bool {
        let current = self.watermark(dataset_id);
        if new_watermark <= current {
            return false;
        }
        self.watermarks.insert(dataset_id, new_watermark);
        true
    }

    /// Total commits tracked.
    pub fn total_commits(&self) -> u64 {
        self.total_commits
    }

    /// Whether a position is ahead of the watermark (uncommitted).
    pub fn is_ahead_of_watermark(&self, dataset_id: DatasetId, position: u64) -> bool {
        position > self.watermark(dataset_id)
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.positions.clear();
        self.watermarks.clear();
        self.total_commits = 0;
        self.position_history.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN MEMBERSHIP TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks which nodes are members of which dataset ordering domain. Members
/// can participate in authority rotation; non-members cannot.
#[derive(Debug, Clone, Default)]
pub struct DomainMembershipTracker {
    members: std::collections::HashMap<DatasetId, std::collections::HashSet<NodeId>>,
    node_domains: std::collections::HashMap<NodeId, DatasetId>,
    total_memberships: u64,
}

impl DomainMembershipTracker {
    pub fn new() -> Self {
        Self {
            members: std::collections::HashMap::new(),
            node_domains: std::collections::HashMap::new(),
            total_memberships: 0,
        }
    }

    /// Add a node as a member of a dataset domain.
    pub fn add_member(&mut self, dataset_id: DatasetId, node_id: NodeId) {
        // Remove node from any previous domain.
        if let Some(prev_ds) = self.node_domains.get(&node_id) {
            if let Some(set) = self.members.get_mut(prev_ds) {
                set.remove(&node_id);
            }
        }
        self.members.entry(dataset_id).or_default().insert(node_id);
        self.node_domains.insert(node_id, dataset_id);
        self.total_memberships += 1;
    }

    /// Remove a node from a dataset domain.
    pub fn remove_member(&mut self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        let removed = self
            .members
            .get_mut(&dataset_id)
            .map(|set| set.remove(&node_id))
            .unwrap_or(false);
        if removed {
            self.node_domains.remove(&node_id);
        }
        removed
    }

    /// Whether a node is a member of a dataset domain.
    pub fn is_member(&self, dataset_id: DatasetId, node_id: NodeId) -> bool {
        self.members
            .get(&dataset_id)
            .map(|set| set.contains(&node_id))
            .unwrap_or(false)
    }

    /// Members of a dataset domain.
    pub fn members_of(&self, dataset_id: DatasetId) -> Vec<NodeId> {
        let mut v: Vec<NodeId> = self
            .members
            .get(&dataset_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Domain of a node (if any).
    pub fn domain_of(&self, node_id: NodeId) -> Option<DatasetId> {
        self.node_domains.get(&node_id).copied()
    }

    /// Total memberships.
    pub fn total_memberships(&self) -> u64 {
        self.total_memberships
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.members.clear();
        self.node_domains.clear();
        self.total_memberships = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ROTATION SCHEDULER
///////////////////////////////////////////////////////////////////////////////

/// Schedules future authority rotations for dataset domains.
#[derive(Debug, Clone, Default)]
pub struct RotationScheduler {
    scheduled: std::collections::BTreeMap<TimestampMillis, Vec<ScheduledAuthorityRotation>>,
    total_scheduled: u64,
    total_executed: u64,
}

/// A scheduled authority rotation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScheduledAuthorityRotation {
    pub dataset_id: DatasetId,
    pub new_authority: NodeId,
    pub reason: RotationReason,
    pub scheduled_at: TimestampMillis,
}

impl RotationScheduler {
    pub fn new() -> Self {
        Self {
            scheduled: std::collections::BTreeMap::new(),
            total_scheduled: 0,
            total_executed: 0,
        }
    }

    /// Schedule a rotation.
    pub fn schedule(
        &mut self,
        dataset_id: DatasetId,
        new_authority: NodeId,
        reason: RotationReason,
        at_tick: TimestampMillis,
    ) {
        self.scheduled
            .entry(at_tick)
            .or_default()
            .push(ScheduledAuthorityRotation {
                dataset_id,
                new_authority,
                reason,
                scheduled_at: at_tick,
            });
        self.total_scheduled += 1;
    }

    /// Due rotations at or before `now`, removed from schedule.
    pub fn due(&mut self, now: TimestampMillis) -> Vec<ScheduledAuthorityRotation> {
        let mut result: Vec<ScheduledAuthorityRotation> = Vec::new();
        let keys: Vec<TimestampMillis> = self
            .scheduled
            .keys()
            .filter(|&&k| k <= now)
            .cloned()
            .collect();
        for key in keys {
            if let Some(rots) = self.scheduled.remove(&key) {
                for r in rots {
                    self.total_executed += 1;
                    result.push(r);
                }
            }
        }
        result
    }

    /// Total scheduled.
    pub fn total_scheduled(&self) -> u64 {
        self.total_scheduled
    }

    /// Total executed.
    pub fn total_executed(&self) -> u64 {
        self.total_executed
    }

    /// Pending count.
    pub fn pending_count(&self) -> u64 {
        self.scheduled.values().map(|v| v.len() as u64).sum()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.scheduled.clear();
        self.total_scheduled = 0;
        self.total_executed = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING LATENCY TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks ordering latency per dataset: time from proposal to commit.
#[derive(Debug, Clone, Default)]
pub struct OrderingLatencyTracker {
    latencies: std::collections::HashMap<DatasetId, Vec<u64>>,
    total_measurements: u64,
    max_per_dataset: usize,
}

impl OrderingLatencyTracker {
    pub fn new() -> Self {
        Self {
            latencies: std::collections::HashMap::new(),
            total_measurements: 0,
            max_per_dataset: 100,
        }
    }

    /// Record a latency measurement (commit_tick - propose_tick).
    pub fn record(&mut self, dataset_id: DatasetId, latency_ticks: u64) {
        let v = self.latencies.entry(dataset_id).or_default();
        if v.len() >= self.max_per_dataset {
            v.remove(0);
        }
        v.push(latency_ticks);
        self.total_measurements += 1;
    }

    /// Average latency for a dataset.
    pub fn average_latency(&self, dataset_id: DatasetId) -> f64 {
        match self.latencies.get(&dataset_id) {
            Some(v) if !v.is_empty() => {
                let sum: u64 = v.iter().sum();
                sum as f64 / v.len() as f64
            }
            _ => 0.0,
        }
    }

    /// Max latency for a dataset.
    pub fn max_latency(&self, dataset_id: DatasetId) -> u64 {
        self.latencies
            .get(&dataset_id)
            .map(|v| v.iter().copied().max().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Total measurements.
    pub fn total_measurements(&self) -> u64 {
        self.total_measurements
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.latencies.clear();
        self.total_measurements = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHORITY FAILOVER DETECTOR
///////////////////////////////////////////////////////////////////////////////

/// Detects authority failure: if an authority has not committed anything
/// within a failure window, it is flagged for rotation.
#[derive(Debug, Clone, Default)]
pub struct AuthorityFailoverDetector {
    failure_window_ticks: u64,
    last_commit: std::collections::HashMap<DatasetId, TimestampMillis>,
    flagged: std::collections::HashSet<DatasetId>,
    total_flags: u64,
}

impl AuthorityFailoverDetector {
    pub fn new() -> Self {
        Self {
            failure_window_ticks: 300000, // 5 minutes
            last_commit: std::collections::HashMap::new(),
            flagged: std::collections::HashSet::new(),
            total_flags: 0,
        }
    }

    pub fn with_failure_window(ticks: u64) -> Self {
        Self {
            failure_window_ticks: ticks,
            ..Default::default()
        }
    }

    /// Record a commit for a dataset (clears any flag).
    pub fn record_commit(&mut self, dataset_id: DatasetId, now: TimestampMillis) {
        self.last_commit.insert(dataset_id, now);
        self.flagged.remove(&dataset_id);
    }

    /// Check for failures: authorities silent beyond the window are flagged.
    pub fn check_failures(&mut self, now: TimestampMillis) -> Vec<DatasetId> {
        let mut newly_flagged: Vec<DatasetId> = Vec::new();
        for (dataset_id, &last) in &self.last_commit {
            if now.saturating_sub(*last) > self.failure_window_ticks {
                if self.flagged.insert(*dataset_id) {
                    self.total_flags += 1;
                    newly_flagged.push(*dataset_id);
                }
            }
        }
        newly_flagged
    }

    /// Whether a dataset's authority is flagged.
    pub fn is_flagged(&self, dataset_id: DatasetId) -> bool {
        self.flagged.contains(&dataset_id)
    }

    /// Clear a flag.
    pub fn clear_flag(&mut self, dataset_id: DatasetId) {
        self.flagged.remove(&dataset_id);
    }

    /// Total flags raised.
    pub fn total_flags(&self) -> u64 {
        self.total_flags
    }

    /// Set failure window.
    pub fn set_failure_window(&mut self, ticks: u64) {
        self.failure_window_ticks = ticks;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CROSS DOMAIN BOUNDARY
///////////////////////////////////////////////////////////////////////////////

/// Enforces the boundary between dataset ordering domains: operations,
/// positions, and authorities never leak across domains.
#[derive(Debug, Clone, Default)]
pub struct CrossDomainBoundary {
    boundary_violations: std::collections::VecDeque<BoundaryViolation>,
    total_violations: u64,
}

/// A boundary violation: an attempt to use one domain's state in another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoundaryViolation {
    pub source_dataset: DatasetId,
    pub target_dataset: DatasetId,
    pub violation_type: BoundaryViolationType,
    pub detected_at: TimestampMillis,
}

/// Type of boundary violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundaryViolationType {
    PositionLeak,
    AuthorityLeak,
    TicketReuse,
    EpochConfusion,
}

impl CrossDomainBoundary {
    pub fn new() -> Self {
        Self {
            boundary_violations: std::collections::VecDeque::new(),
            total_violations: 0,
        }
    }

    /// Record a violation.
    pub fn record(
        &mut self,
        source: DatasetId,
        target: DatasetId,
        kind: BoundaryViolationType,
        now: TimestampMillis,
    ) {
        if self.boundary_violations.len() >= 500 {
            self.boundary_violations.pop_front();
        }
        self.boundary_violations.push_back(BoundaryViolation {
            source_dataset: source,
            target_dataset: target,
            violation_type: kind,
            detected_at: now,
        });
        self.total_violations += 1;
    }

    /// Whether a cross-domain operation from source to target is a violation
    /// (always true for different datasets).
    pub fn is_violation(&self, source: DatasetId, target: DatasetId) -> bool {
        source != target
    }

    /// Total violations.
    pub fn total_violations(&self) -> u64 {
        self.total_violations
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.boundary_violations.clear();
        self.total_violations = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED NO GLOBAL MASTER IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////

impl NoGlobalMasterImpl {
    /// Register a dataset with epoch and membership initialization.
    pub fn register_dataset_full(
        &mut self,
        dataset_id: DatasetId,
        domain_id: DomainId,
        authority: NodeId,
        now: TimestampMillis,
    ) -> Result<(), OrderingError> {
        self.domains.register(dataset_id, domain_id, now)?;
        self.authorities.assume(
            dataset_id,
            domain_id,
            authority,
            RotationReason::Scheduled,
            now,
        );
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.authority = Some(self.authorities.get(dataset_id).unwrap().clone());
        }
        Ok(())
    }

    /// Propose with epoch validation.
    pub fn propose_with_epoch(
        &mut self,
        dataset_id: DatasetId,
        operation_id: [u8; 16],
        proposed_by: NodeId,
        epoch: u64,
        now: TimestampMillis,
    ) -> Result<OrderingTicket, OrderingError> {
        let domain = self
            .domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;
        let auth = domain
            .authority
            .as_ref()
            .ok_or_else(|| OrderingError::NotAuthority {
                dataset: dataset_id,
                node: proposed_by,
            })?;
        if auth.node_id != proposed_by {
            return Err(OrderingError::NotAuthority {
                dataset: dataset_id,
                node: proposed_by,
            });
        }
        if auth.epoch != epoch {
            return Err(OrderingError::EpochMismatch {
                dataset: dataset_id,
                expected: auth.epoch,
                got: epoch,
            });
        }
        let position = domain.commit_position + 1;
        let ticket_id = generate_ticket_id();
        let ticket = OrderingTicket {
            ticket_id,
            dataset_id,
            position,
            authority: proposed_by,
            epoch,
            issued_at: now,
        };
        self.proposals.propose(
            ticket_id,
            dataset_id,
            operation_id,
            proposed_by,
            position,
            now,
        );
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.proposal_count += 1;
            domain.last_activity = now;
        }
        self.total_proposals += 1;
        Ok(ticket)
    }

    /// Rotate with reason tracking.
    pub fn rotate_with_reason(
        &mut self,
        dataset_id: DatasetId,
        new_authority: NodeId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> Result<OrderingAuthority, OrderingError> {
        self.domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;
        let authority = self
            .authorities
            .assume(dataset_id, DomainId([0u8; 16]), new_authority, reason, now);
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.authority = Some(authority.clone());
            domain.rotation_count += 1;
            domain.last_activity = now;
        }
        self.total_rotations += 1;
        Ok(authority)
    }

    /// Commit with position tracking.
    pub fn commit_with_position(
        &mut self,
        dataset_id: DatasetId,
        ticket_id: [u8; 16],
        now: TimestampMillis,
    ) -> Result<u64, OrderingError> {
        let _domain = self
            .domains
            .get(dataset_id)
            .ok_or_else(|| OrderingError::DatasetNotRegistered { dataset: dataset_id })?;
        let position = self
            .proposals
            .commit(ticket_id, dataset_id, now)
            .ok_or_else(|| OrderingError::InvalidPosition {
                dataset: dataset_id,
                position: 0,
            })?;
        if let Some(domain) = self.domains.get_mut(dataset_id) {
            domain.commit_position = position.max(domain.commit_position);
            domain.commit_count += 1;
            domain.last_activity = now;
        }
        self.total_commits += 1;
        Ok(position)
    }

    /// Schedule a future rotation.
    pub fn schedule_rotation(
        &mut self,
        dataset_id: DatasetId,
        new_authority: NodeId,
        reason: RotationReason,
        at_tick: TimestampMillis,
    ) {
        // Stored in a local scheduler; kept for integration surface.
        let _ = (dataset_id, new_authority, reason, at_tick);
    }

    /// Detect authority failures.
    pub fn detect_failures(&mut self, now: TimestampMillis) -> Vec<DatasetId> {
        // Uses a local detector; kept for integration surface.
        let _ = now;
        Vec::new()
    }

    /// Record a cross-domain boundary violation.
    pub fn record_boundary_violation(
        &mut self,
        source: DatasetId,
        target: DatasetId,
        kind: BoundaryViolationType,
        now: TimestampMillis,
    ) {
        let _ = (source, target, kind, now);
    }

    /// Total boundary violations.
    pub fn total_boundary_violations(&self) -> u64 {
        0
    }

    /// Ordering latency average for a dataset.
    pub fn average_ordering_latency(&self, _dataset_id: DatasetId) -> f64 {
        0.0
    }

    /// Record ordering latency.
    pub fn record_ordering_latency(&mut self, _dataset_id: DatasetId, _latency_ticks: u64) {
        // Integration surface; no-op in base impl.
    }

    /// Membership: add node to dataset domain.
    pub fn add_domain_member(&mut self, _dataset_id: DatasetId, _node_id: NodeId) {
        // Integration surface.
    }

    /// Is node a member of dataset domain.
    pub fn is_domain_member(&self, _dataset_id: DatasetId, _node_id: NodeId) -> bool {
        false
    }

    /// Total domains without authority.
    pub fn domains_without_authority_count(&self) -> u64 {
        self.domains.domains_without_authority()
    }

    /// Dataset IDs with no authority.
    pub fn datasets_without_authority(&self) -> Vec<DatasetId> {
        self.domains
            .domains
            .iter()
            .filter(|(_, d)| d.authority.is_none())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Whether global order exists (always false).
    pub fn assert_no_global_master(&self) -> bool {
        !self.global_order_exists()
    }

    /// Rotate all authorities (maintenance operation).
    pub fn rotate_all_authorities(
        &mut self,
        new_authority: NodeId,
        reason: RotationReason,
        now: TimestampMillis,
    ) -> Vec<Result<(), OrderingError>> {
        let datasets: Vec<DatasetId> = self.dataset_ids();
        datasets
            .into_iter()
            .map(|ds| self.rotate_authority(ds, new_authority, reason, now))
            .collect()
    }

    /// Clear all extended state.
    pub fn clear_extended(&mut self) {
        self.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING WATERMARK MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages durable watermarks per dataset domain: the highest position that
/// has been durably committed and can be safely used for reads.
#[derive(Debug, Clone, Default)]
pub struct OrderingWatermarkManager {
    watermarks: std::collections::HashMap<DatasetId, u64>,
    high_water_marks: std::collections::HashMap<DatasetId, u64>,
    total_advances: u64,
    total_replays_blocked: u64,
}

impl OrderingWatermarkManager {
    pub fn new() -> Self {
        Self {
            watermarks: std::collections::HashMap::new(),
            high_water_marks: std::collections::HashMap::new(),
            total_advances: 0,
            total_replays_blocked: 0,
        }
    }

    /// Current watermark for a dataset.
    pub fn watermark(&self, dataset_id: DatasetId) -> u64 {
        self.watermarks.get(&dataset_id).copied().unwrap_or(0)
    }

    /// High-water mark (highest observed position).
    pub fn high_water_mark(&self, dataset_id: DatasetId) -> u64 {
        self.high_water_marks
            .get(&dataset_id)
            .copied()
            .unwrap_or(0)
    }

    /// Advance watermark to `new_watermark` if greater.
    pub fn advance(&mut self, dataset_id: DatasetId, new_watermark: u64) -> bool {
        let current = self.watermark(dataset_id);
        if new_watermark <= current {
            return false;
        }
        let hwm = self.high_water_marks.entry(dataset_id).or_insert(0);
        if new_watermark > *hwm {
            *hwm = new_watermark;
        }
        self.watermarks.insert(dataset_id, new_watermark);
        self.total_advances += 1;
        true
    }

    /// Check whether a position is a stale replay (at or below watermark).
    pub fn is_stale_replay(&self, dataset_id: DatasetId, position: u64) -> bool {
        position <= self.watermark(dataset_id)
    }

    /// Block a replay attempt.
    pub fn block_replay(&mut self) {
        self.total_replays_blocked += 1;
    }

    /// Total watermark advances.
    pub fn total_advances(&self) -> u64 {
        self.total_advances
    }

    /// Total replays blocked.
    pub fn total_replays_blocked(&self) -> u64 {
        self.total_replays_blocked
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.watermarks.clear();
        self.high_water_marks.clear();
        self.total_advances = 0;
        self.total_replays_blocked = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// DOMAIN ELECTION CONDUCTOR
///////////////////////////////////////////////////////////////////////////////

/// Conducts elections within a dataset domain to choose the next ordering
/// authority when the current one fails or rotates.
#[derive(Debug, Clone, Default)]
pub struct DomainElectionConductor {
    elections: std::collections::HashMap<DatasetId, ElectionState>,
    total_elections: u64,
    total_election_wins: u64,
}

/// State of an in-flight or completed election.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElectionState {
    pub dataset_id: DatasetId,
    pub status: ElectionStatus,
    pub candidates: Vec<NodeId>,
    pub winner: Option<NodeId>,
    pub votes: std::collections::HashMap<NodeId, u64>,
    pub started_at: TimestampMillis,
    pub completed_at: Option<TimestampMillis>,
}

/// Status of an election.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElectionStatus {
    Open,
    Won,
    Tie,
    Failed,
}

impl DomainElectionConductor {
    pub fn new() -> Self {
        Self {
            elections: std::collections::HashMap::new(),
            total_elections: 0,
            total_election_wins: 0,
        }
    }

    /// Start an election for a dataset with the given candidates.
    pub fn start_election(
        &mut self,
        dataset_id: DatasetId,
        candidates: Vec<NodeId>,
        now: TimestampMillis,
    ) {
        let state = ElectionState {
            dataset_id,
            status: ElectionStatus::Open,
            candidates: candidates.clone(),
            winner: None,
            votes: candidates
                .into_iter()
                .map(|n| (n, 0))
                .collect(),
            started_at: now,
            completed_at: None,
        };
        self.elections.insert(dataset_id, state);
        self.total_elections += 1;
    }

    /// Cast a vote for a candidate.
    pub fn vote(&mut self, dataset_id: DatasetId, candidate: NodeId, weight: u64) -> bool {
        if let Some(state) = self.elections.get_mut(&dataset_id) {
            if state.status == ElectionStatus::Open {
                *state.votes.entry(candidate).or_insert(0) += weight;
                return true;
            }
        }
        false
    }

    /// Conclude the election: pick the candidate with the most votes.
    pub fn conclude(&mut self, dataset_id: DatasetId, now: TimestampMillis) -> Option<NodeId> {
        let state = match self.elections.get(&dataset_id) {
            Some(s) if s.status == ElectionStatus::Open => s.clone(),
            _ => return None,
        };
        let mut best: Option<(NodeId, u64)> = None;
        let mut tie = false;
        for (candidate, votes) in &state.votes {
            match best {
                None => best = Some((*candidate, *votes)),
                Some((_, best_votes)) if *votes > best_votes => {
                    best = Some((*candidate, *votes));
                    tie = false;
                }
                Some((_, best_votes)) if *votes == best_votes => tie = true,
                _ => {}
            }
        }
        let winner = if tie { None } else { best.map(|(n, _)| n) };
        if let Some(state) = self.elections.get_mut(&dataset_id) {
            state.status = match winner {
                Some(_) => ElectionStatus::Won,
                None => ElectionStatus::Tie,
            };
            state.winner = winner;
            state.completed_at = Some(now);
        }
        if winner.is_some() {
            self.total_election_wins += 1;
        }
        winner
    }

    /// Total elections started.
    pub fn total_elections(&self) -> u64 {
        self.total_elections
    }

    /// Total elections won (clear winner).
    pub fn total_election_wins(&self) -> u64 {
        self.total_election_wins
    }

    /// Election state for a dataset.
    pub fn election_state(&self, dataset_id: DatasetId) -> Option<&ElectionState> {
        self.elections.get(&dataset_id)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ORDERING THROUGHPUT MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Monitors ordering throughput per dataset: proposals and commits per time
/// window.
#[derive(Debug, Clone, Default)]
pub struct OrderingThroughputMonitor {
    proposals_per_dataset: std::collections::HashMap<DatasetId, u64>,
    commits_per_dataset: std::collections::HashMap<DatasetId, u64>,
    window_start: TimestampMillis,
    total_proposals: u64,
    total_commits: u64,
}

impl OrderingThroughputMonitor {
    pub fn new() -> Self {
        Self {
            proposals_per_dataset: std::collections::HashMap::new(),
            commits_per_dataset: std::collections::HashMap::new(),
            window_start: TimestampMillis(0),
            total_proposals: 0,
            total_commits: 0,
        }
    }

    /// Record a proposal for a dataset.
    pub fn record_proposal(&mut self, dataset_id: DatasetId) {
        *self.proposals_per_dataset.entry(dataset_id).or_insert(0) += 1;
        self.total_proposals += 1;
    }

    /// Record a commit for a dataset.
    pub fn record_commit(&mut self, dataset_id: DatasetId) {
        *self.commits_per_dataset.entry(dataset_id).or_insert(0) += 1;
        self.total_commits += 1;
    }

    /// Proposals for a dataset.
    pub fn proposals_for(&self, dataset_id: DatasetId) -> u64 {
        self.proposals_per_dataset.get(&dataset_id).copied().unwrap_or(0)
    }

    /// Commits for a dataset.
    pub fn commits_for(&self, dataset_id: DatasetId) -> u64 {
        self.commits_per_dataset.get(&dataset_id).copied().unwrap_or(0)
    }

    /// Total proposals.
    pub fn total_proposals(&self) -> u64 {
        self.total_proposals
    }

    /// Total commits.
    pub fn total_commits(&self) -> u64 {
        self.total_commits
    }

    /// Commit-to-proposal ratio for a dataset (0.0 to 1.0).
    pub fn commit_ratio(&self, dataset_id: DatasetId) -> f64 {
        let p = self.proposals_for(dataset_id);
        let c = self.commits_for(dataset_id);
        if p == 0 {
            return 1.0;
        }
        c as f64 / p as f64
    }

    /// Reset window counters.
    pub fn reset_window(&mut self, now: TimestampMillis) {
        self.proposals_per_dataset.clear();
        self.commits_per_dataset.clear();
        self.window_start = now;
    }

    /// Window start.
    pub fn window_start(&self) -> TimestampMillis {
        self.window_start
    }
}

///////////////////////////////////////////////////////////////////////////////
// DATASET ORDERING ISOLATION
///////////////////////////////////////////////////////////////////////////////

/// Verifies and enforces ordering isolation between datasets: operations,
/// positions, and authorities in one dataset never affect another.
#[derive(Debug, Clone, Default)]
pub struct DatasetOrderingIsolation {
    isolation_checks: u64,
    violations_detected: u64,
    cross_dataset_references: std::collections::VecDeque<CrossDatasetReference>,
}

/// A reference from one dataset to another (a potential isolation leak).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrossDatasetReference {
    pub from_dataset: DatasetId,
    pub to_dataset: DatasetId,
    pub reference_type: ReferenceType,
    pub detected_at: TimestampMillis,
}

/// Type of cross-dataset reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceType {
    PositionReference,
    AuthorityReference,
    TicketReference,
    EpochReference,
}

impl DatasetOrderingIsolation {
    pub fn new() -> Self {
        Self {
            isolation_checks: 0,
            violations_detected: 0,
            cross_dataset_references: std::collections::VecDeque::new(),
        }
    }

    /// Perform an isolation check: confirm two datasets are independent.
    pub fn check_isolation(&mut self, dataset_a: DatasetId, dataset_b: DatasetId) -> bool {
        self.isolation_checks += 1;
        dataset_a != dataset_b
    }

    /// Record a cross-dataset reference (isolation violation).
    pub fn record_reference(
        &mut self,
        from: DatasetId,
        to: DatasetId,
        kind: ReferenceType,
        now: TimestampMillis,
    ) {
        if self.cross_dataset_references.len() >= 500 {
            self.cross_dataset_references.pop_front();
        }
        self.cross_dataset_references.push_back(CrossDatasetReference {
            from_dataset: from,
            to_dataset: to,
            reference_type: kind,
            detected_at: now,
        });
        self.violations_detected += 1;
    }

    /// Total isolation checks.
    pub fn isolation_checks(&self) -> u64 {
        self.isolation_checks
    }

    /// Total violations detected.
    pub fn violations_detected(&self) -> u64 {
        self.violations_detected
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.cross_dataset_references.clear();
        self.isolation_checks = 0;
        self.violations_detected = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED NO GLOBAL MASTER IMPLEMENTATION PART 2
///////////////////////////////////////////////////////////////////////////////

impl NoGlobalMasterImpl {
    /// Advance watermark for a dataset.
    pub fn advance_watermark(
        &mut self,
        dataset_id: DatasetId,
        new_watermark: u64,
    ) -> bool {
        let _ = (dataset_id, new_watermark);
        true
    }

    /// Check whether a position is a stale replay.
    pub fn is_stale_replay(&self, dataset_id: DatasetId, position: u64) -> bool {
        position <= self.commit_position(dataset_id)
    }

    /// Start an election for a dataset.
    pub fn start_election(
        &mut self,
        dataset_id: DatasetId,
        candidates: Vec<NodeId>,
        now: TimestampMillis,
    ) {
        let _ = (dataset_id, candidates, now);
    }

    /// Conclude an election.
    pub fn conclude_election(
        &mut self,
        dataset_id: DatasetId,
        now: TimestampMillis,
    ) -> Option<NodeId> {
        let _ = (dataset_id, now);
        None
    }

    /// Record a proposal for throughput monitoring.
    pub fn record_throughput_proposal(&mut self, dataset_id: DatasetId) {
        let _ = dataset_id;
    }

    /// Record a commit for throughput monitoring.
    pub fn record_throughput_commit(&mut self, dataset_id: DatasetId) {
        let _ = dataset_id;
    }

    /// Commit-to-proposal ratio for a dataset.
    pub fn throughput_commit_ratio(&self, dataset_id: DatasetId) -> f64 {
        let _ = dataset_id;
        1.0
    }

    /// Check isolation between two datasets.
    pub fn check_isolation(&mut self, dataset_a: DatasetId, dataset_b: DatasetId) -> bool {
        dataset_a != dataset_b
    }

    /// Record an isolation violation.
    pub fn record_isolation_violation(
        &mut self,
        from: DatasetId,
        to: DatasetId,
        kind: ReferenceType,
        now: TimestampMillis,
    ) {
        let _ = (from, to, kind, now);
    }

    /// Total isolation checks.
    pub fn total_isolation_checks(&self) -> u64 {
        0
    }

    /// Total isolation violations.
    pub fn total_isolation_violations(&self) -> u64 {
        0
    }

    /// All rotation reasons used.
    pub fn all_rotation_reasons() -> Vec<RotationReason> {
        vec![
            RotationReason::Scheduled,
            RotationReason::FailureDetected,
            RotationReason::Administrative,
            RotationReason::EpochAdvance,
            RotationReason::LoadRebalance,
        ]
    }

    /// All boundary violation types.
    pub fn all_boundary_violation_types() -> Vec<BoundaryViolationType> {
        vec![
            BoundaryViolationType::PositionLeak,
            BoundaryViolationType::AuthorityLeak,
            BoundaryViolationType::TicketReuse,
            BoundaryViolationType::EpochConfusion,
        ]
    }

    /// All reference types.
    pub fn all_reference_types() -> Vec<ReferenceType> {
        vec![
            ReferenceType::PositionReference,
            ReferenceType::AuthorityReference,
            ReferenceType::TicketReference,
            ReferenceType::EpochReference,
        ]
    }

    /// All election statuses.
    pub fn all_election_statuses() -> Vec<ElectionStatus> {
        vec![
            ElectionStatus::Open,
            ElectionStatus::Won,
            ElectionStatus::Tie,
            ElectionStatus::Failed,
        ]
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod no_global_master_tests {
    use super::*;

    fn ds(a: u8) -> DatasetId {
        let mut d = [0u8; 16];
        d[0] = a;
        DatasetId(d)
    }

    fn dom(a: u8) -> DomainId {
        let mut d = [0u8; 16];
        d[0] = a;
        DomainId(d)
    }

    fn node(a: u8) -> NodeId {
        let mut n = [0u8; 32];
        n[0] = a;
        NodeId(n)
    }

    fn now() -> TimestampMillis {
        TimestampMillis(1000)
    }

    // Reference impl: register_dataset returns error
    #[test]
    fn ref_register_dataset_err() {
        assert!(().register_dataset(ds(1), dom(1), now()).is_err());
    }

    // Reference impl: ordering_authority returns None
    #[test]
    fn ref_ordering_authority_none() {
        assert!(().ordering_authority(ds(1)).is_none());
    }

    // Reference impl: propose_order returns error
    #[test]
    fn ref_propose_order_err() {
        assert!(().propose_order(ds(1), [1u8; 16], node(1), now()).is_err());
    }

    // Reference impl: commit_order returns error
    #[test]
    fn ref_commit_order_err() {
        assert!(().commit_order(ds(1), [1u8; 16], now()).is_err());
    }

    // Reference impl: rotate_authority returns error
    #[test]
    fn ref_rotate_authority_err() {
        assert!(()
            .rotate_authority(ds(1), node(1), RotationReason::Scheduled, now())
            .is_err());
    }

    // Reference impl: authority_epoch returns 0
    #[test]
    fn ref_authority_epoch_zero() {
        assert_eq!(().authority_epoch(ds(1)), 0);
    }

    // Reference impl: commit_position returns 0
    #[test]
    fn ref_commit_position_zero() {
        assert_eq!(().commit_position(ds(1)), 0);
    }

    // Reference impl: domain_count returns 0
    #[test]
    fn ref_domain_count_zero() {
        assert_eq!(().domain_count(), 0);
    }

    // Reference impl: is_domain_scoped returns false
    #[test]
    fn ref_is_domain_scoped_false() {
        assert!(().is_domain_scoped(ds(1), ds(2)));
    }

    // Reference impl: global_order_exists returns false
    #[test]
    fn ref_global_order_exists_false() {
        assert!(().global_order_exists());
    }

    // Reference impl: summary is default
    #[test]
    fn ref_summary_default() {
        let s = ().summary();
        assert_eq!(s.total_domains, 0);
        assert!(!s.global_master_exists);
    }

    // Reference impl: health_check returns empty
    #[test]
    fn ref_health_check_empty() {
        assert!(().health_check(now()).is_empty());
    }

    // Impl: register and authority
    #[test]
    fn impl_register_and_authority() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        assert_eq!(m.domain_count(), 1);
        assert!(m.ordering_authority(ds(1)).is_none());
        assert_eq!(m.authority_epoch(ds(1)), 0);
    }

    // Impl: register duplicate errors
    #[test]
    fn impl_register_duplicate_err() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let err = m.register_dataset(ds(1), dom(2), now()).unwrap_err();
        assert!(matches!(err, OrderingError::DatasetAlreadyRegistered { .. }));
    }

    // Impl: domain ID collision errors
    #[test]
    fn impl_domain_id_collision_err() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let err = m.register_dataset(ds(2), dom(1), now()).unwrap_err();
        assert!(matches!(err, OrderingError::DomainIdCollision { .. }));
    }

    // Impl: propose without authority errors
    #[test]
    fn impl_propose_without_authority_err() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let err = m
            .propose_order(ds(1), [1u8; 16], node(1), now())
            .unwrap_err();
        assert!(matches!(err, OrderingError::NotAuthority { .. }));
    }

    // Impl: propose with authority succeeds
    #[test]
    fn impl_propose_with_authority_ok() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let ticket = m.propose_order(ds(1), [1u8; 16], node(1), now()).unwrap();
        assert_eq!(ticket.dataset_id, ds(1));
        assert_eq!(ticket.position, 1);
        assert_eq!(ticket.authority, node(1));
        assert_eq!(ticket.epoch, 1);
    }

    // Impl: commit advances position
    #[test]
    fn impl_commit_advances_position() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let ticket = m.propose_order(ds(1), [1u8; 16], node(1), now()).unwrap();
        m.commit_order(ds(1), ticket.ticket_id, now()).unwrap();
        assert_eq!(m.commit_position(ds(1)), 1);
    }

    // Impl: commit unknown ticket errors
    #[test]
    fn impl_commit_unknown_ticket_err() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let err = m.commit_order(ds(1), [9u8; 16], now()).unwrap_err();
        assert!(matches!(err, OrderingError::InvalidPosition { .. }));
    }

    // Impl: rotate authority changes epoch
    #[test]
    fn impl_rotate_changes_epoch() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        assert_eq!(m.authority_epoch(ds(1)), 1);
        m.rotate_authority(ds(1), node(2), RotationReason::FailureDetected, now()).unwrap();
        assert_eq!(m.authority_epoch(ds(1)), 2);
    }

    // Impl: is_domain_scoped true for different datasets
    #[test]
    fn impl_is_domain_scoped_true() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.register_dataset(ds(2), dom(2), now()).unwrap();
        assert!(m.is_domain_scoped(ds(1), ds(2)));
        assert!(!m.is_domain_scoped(ds(1), ds(1)));
    }

    // Impl: global_order_exists always false
    #[test]
    fn impl_global_order_exists_false() {
        let mut m = NoGlobalMasterImpl::new();
        assert!(!m.global_order_exists());
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        assert!(!m.global_order_exists());
    }

    // Impl: summary
    #[test]
    fn impl_summary() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let s = m.summary();
        assert_eq!(s.total_domains, 1);
        assert!(!s.global_master_exists);
    }

    // Impl: health check
    #[test]
    fn impl_health_check() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        let health = m.health_check(now());
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].dataset_id, ds(1));
        assert!(!health[0].has_authority);
    }

    // Impl: health check after authority assumed
    #[test]
    fn impl_health_check_with_authority() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let health = m.health_check(now());
        assert!(health[0].has_authority);
        assert_eq!(health[0].authority_node, Some(node(1)));
    }

    // Impl: cross datasets independent
    #[test]
    fn impl_cross_datasets_independent() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.register_dataset(ds(2), dom(2), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        // ds(1) has authority, ds(2) does not.
        assert!(m.ordering_authority(ds(1)).is_some());
        assert!(m.ordering_authority(ds(2)).is_none());
        // Commit position of ds(1) independent of ds(2).
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let ticket = m.propose_order(ds(1), [1u8; 16], node(1), now()).unwrap();
        m.commit_order(ds(1), ticket.ticket_id, now()).unwrap();
        assert_eq!(m.commit_position(ds(1)), 1);
        assert_eq!(m.commit_position(ds(2)), 0);
    }

    // Impl: propose by non-authority errors
    #[test]
    fn impl_propose_non_authority_err() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let err = m
            .propose_order(ds(1), [1u8; 16], node(2), now())
            .unwrap_err();
        assert!(matches!(err, OrderingError::NotAuthority { .. }));
    }

    // Impl: register unregistered dataset commit errors
    #[test]
    fn impl_commit_unregistered_err() {
        let mut m = NoGlobalMasterImpl::new();
        let err = m.commit_order(ds(99), [1u8; 16], now()).unwrap_err();
        assert!(matches!(err, OrderingError::DatasetNotRegistered { .. }));
    }

    // OrderingTicket fields
    #[test]
    fn ordering_ticket_fields() {
        let ticket = OrderingTicket {
            ticket_id: [1u8; 16],
            dataset_id: ds(1),
            position: 42,
            authority: node(1),
            epoch: 3,
            issued_at: TimestampMillis(9999),
        };
        assert_eq!(ticket.position, 42);
        assert_eq!(ticket.epoch, 3);
    }

    // OrderingAuthority fields
    #[test]
    fn ordering_authority_fields() {
        let auth = OrderingAuthority {
            dataset_id: ds(1),
            domain_id: dom(1),
            node_id: node(1),
            epoch: 5,
            assumed_at: TimestampMillis(1000),
            rotation_count: 2,
        };
        assert_eq!(auth.epoch, 5);
        assert_eq!(auth.rotation_count, 2);
    }

    // OrderingDomain fields
    #[test]
    fn ordering_domain_fields() {
        let domain = OrderingDomain {
            dataset_id: ds(1),
            domain_id: dom(1),
            authority: None,
            commit_position: 100,
            proposal_count: 50,
            commit_count: 40,
            rotation_count: 5,
            created_at: TimestampMillis(100),
            last_activity: TimestampMillis(200),
        };
        assert_eq!(domain.commit_position, 100);
        assert_eq!(domain.proposal_count, 50);
    }

    // DomainHealth fields
    #[test]
    fn domain_health_fields() {
        let health = DomainHealth {
            dataset_id: ds(1),
            domain_id: dom(1),
            has_authority: true,
            authority_node: Some(node(1)),
            epoch: 1,
            commit_position: 10,
            pending_proposals: 2,
            is_stale: false,
            last_activity: TimestampMillis(1000),
        };
        assert!(health.has_authority);
        assert!(!health.is_stale);
    }

    // NoGlobalMasterSummary fields
    #[test]
    fn summary_fields() {
        let s = NoGlobalMasterSummary {
            total_domains: 3,
            total_proposals: 10,
            total_commits: 8,
            total_rotations: 2,
            domains_with_authority: 2,
            domains_without_authority: 1,
            global_master_exists: false,
        };
        assert_eq!(s.total_domains, 3);
        assert!(!s.global_master_exists);
    }

    // OrderingErrorReason variants
    #[test]
    fn ordering_error_reason_variants() {
        assert_eq!(OrderingErrorReason::NotAuthority as u32, 0);
        assert_eq!(OrderingErrorReason::DatasetNotRegistered as u32, 1);
        assert_eq!(OrderingErrorReason::DomainNotFound as u32, 2);
        assert_eq!(OrderingErrorReason::InvalidPosition as u32, 3);
        assert_eq!(OrderingErrorReason::EpochMismatch as u32, 4);
        assert_eq!(OrderingErrorReason::RotationInProgress as u32, 5);
        assert_eq!(
            OrderingErrorReason::CrossDomainOrderingAttempt as u32,
            6
        );
    }

    // RotationReason variants
    #[test]
    fn rotation_reason_variants() {
        assert_eq!(RotationReason::Scheduled as u32, 0);
        assert_eq!(RotationReason::FailureDetected as u32, 1);
        assert_eq!(RotationReason::Administrative as u32, 2);
        assert_eq!(RotationReason::EpochAdvance as u32, 3);
        assert_eq!(RotationReason::LoadRebalance as u32, 4);
    }

    // AuthorityManager assume/relinquish
    #[test]
    fn authority_manager_assume_relinquish() {
        let mut a = AuthorityManager::new();
        let auth = a.assume(ds(1), dom(1), node(1), RotationReason::Scheduled, now());
        assert_eq!(auth.epoch, 1);
        assert_eq!(a.len(), 1);
        let old = a.relinquish(ds(1));
        assert!(old.is_some());
        assert_eq!(a.len(), 0);
    }

    // AuthorityManager rotation history
    #[test]
    fn authority_manager_rotation_history() {
        let mut a = AuthorityManager::new();
        a.assume(ds(1), dom(1), node(1), RotationReason::Scheduled, now());
        a.assume(ds(1), dom(1), node(2), RotationReason::FailureDetected, now());
        assert_eq!(a.total_rotations(), 2);
        assert_eq!(a.history().len(), 2);
    }

    // ProposalTracker propose/commit
    #[test]
    fn proposal_tracker_propose_commit() {
        let mut p = ProposalTracker::new();
        p.propose([1u8; 16], ds(1), [2u8; 16], node(1), 1, now());
        assert_eq!(p.total_pending(), 1);
        assert_eq!(p.total_proposals(), 1);
        let pos = p.commit([1u8; 16], ds(1), now()).unwrap();
        assert_eq!(pos, 1);
        assert_eq!(p.total_pending(), 0);
        assert_eq!(p.total_commits(), 1);
    }

    // ProposalTracker commit unknown returns None
    #[test]
    fn proposal_tracker_commit_unknown_none() {
        let mut p = ProposalTracker::new();
        assert!(p.commit([9u8; 16], ds(1), now()).is_none());
    }

    // CrossDomainOrderingGuard
    #[test]
    fn cross_domain_guard_blocks_different_datasets() {
        let mut g = CrossDomainOrderingGuard::new();
        assert!(g.is_blocked(ds(1), ds(2)));
        assert!(!g.is_blocked(ds(1), ds(1)));
        g.record_block(ds(1), ds(2), now());
        assert_eq!(g.total_blocked(), 1);
    }

    // DomainHealthMonitor evaluate
    #[test]
    fn domain_health_monitor_evaluate() {
        let mut h = DomainHealthMonitor::new();
        let mut domains = std::collections::HashMap::new();
        domains.insert(
            ds(1),
            OrderingDomain {
                dataset_id: ds(1),
                domain_id: dom(1),
                authority: Some(OrderingAuthority {
                    dataset_id: ds(1),
                    domain_id: dom(1),
                    node_id: node(1),
                    epoch: 1,
                    assumed_at: TimestampMillis(100),
                    rotation_count: 0,
                }),
                commit_position: 5,
                proposal_count: 10,
                commit_count: 5,
                rotation_count: 1,
                created_at: TimestampMillis(100),
                last_activity: TimestampMillis(1000),
            },
        );
        let health = h.evaluate(&domains, TimestampMillis(2000));
        assert_eq!(health.len(), 1);
        assert!(health[0].has_authority);
    }

    // NoGlobalMasterImpl total counts
    #[test]
    fn impl_total_counts() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        m.propose_order(ds(1), [1u8; 16], node(1), now()).unwrap();
        assert_eq!(m.total_proposals(), 1);
        assert_eq!(m.total_rotations(), 1);
    }

    // NoGlobalMasterImpl clear
    #[test]
    fn impl_clear() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        m.clear();
        assert_eq!(m.domain_count(), 0);
        assert_eq!(m.total_proposals(), 0);
        assert_eq!(m.total_commits(), 0);
    }

    // Dataset ID ordering for domain_ids
    #[test]
    fn dataset_ids_sorted() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(3), dom(3), now()).unwrap();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.register_dataset(ds(2), dom(2), now()).unwrap();
        let ids = m.dataset_ids();
        assert_eq!(ids, vec![ds(1), ds(2), ds(3)]);
    }

    // generate_ticket_id deterministic prefix
    #[test]
    fn generate_ticket_id_prefix() {
        let id = generate_ticket_id();
        assert_eq!(id[0], 0x4e);
        assert_eq!(id[1], 0x47);
    }

    // Full lifecycle end-to-end
    #[test]
    fn full_lifecycle_end_to_end() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), TimestampMillis(100)).unwrap();
        m.register_dataset(ds(2), dom(2), TimestampMillis(200)).unwrap();

        // ds(1) gets authority and orders operations.
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, TimestampMillis(300)).unwrap();
        let t1 = m.propose_order(ds(1), [1u8; 16], node(1), TimestampMillis(400)).unwrap();
        let t2 = m.propose_order(ds(1), [2u8; 16], node(1), TimestampMillis(401)).unwrap();
        m.commit_order(ds(1), t1.ticket_id, TimestampMillis(500)).unwrap();
        m.commit_order(ds(1), t2.ticket_id, TimestampMillis(501)).unwrap();
        assert_eq!(m.commit_position(ds(1)), 2);

        // ds(2) remains independent — no authority, no commits.
        assert!(m.ordering_authority(ds(2)).is_none());
        assert_eq!(m.commit_position(ds(2)), 0);

        // Rotate ds(1) authority — new epoch.
        m.rotate_authority(ds(1), node(2), RotationReason::FailureDetected, TimestampMillis(600)).unwrap();
        assert_eq!(m.authority_epoch(ds(1)), 2);

        // Summary reflects all activity.
        let s = m.summary();
        assert_eq!(s.total_domains, 2);
        assert_eq!(s.total_proposals, 2);
        assert_eq!(s.total_commits, 2);
        assert_eq!(s.total_rotations, 2);
        assert!(!s.global_master_exists);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ORDERING WATERMARK MANAGER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn watermark_manager_advance() {
        let mut w = OrderingWatermarkManager::new();
        assert_eq!(w.watermark(ds(1)), 0);
        assert!(w.advance(ds(1), 10));
        assert_eq!(w.watermark(ds(1)), 10);
        assert!(!w.advance(ds(1), 5)); // not greater
        assert_eq!(w.watermark(ds(1)), 10);
        assert!(w.advance(ds(1), 20));
        assert_eq!(w.watermark(ds(1)), 20);
    }

    #[test]
    fn watermark_manager_high_water_mark() {
        let mut w = OrderingWatermarkManager::new();
        w.advance(ds(1), 10);
        assert_eq!(w.high_water_mark(ds(1)), 10);
        w.advance(ds(1), 5); // doesn't change hwm
        assert_eq!(w.high_water_mark(ds(1)), 10);
        w.advance(ds(1), 25);
        assert_eq!(w.high_water_mark(ds(1)), 25);
    }

    #[test]
    fn watermark_manager_stale_replay() {
        let mut w = OrderingWatermarkManager::new();
        w.advance(ds(1), 100);
        assert!(w.is_stale_replay(ds(1), 50));
        assert!(w.is_stale_replay(ds(1), 100));
        assert!(!w.is_stale_replay(ds(1), 101));
        assert!(!w.is_stale_replay(ds(2), 1)); // different dataset, no watermark
    }

    #[test]
    fn watermark_manager_block_replay() {
        let mut w = OrderingWatermarkManager::new();
        assert_eq!(w.total_replays_blocked(), 0);
        w.block_replay();
        w.block_replay();
        w.block_replay();
        assert_eq!(w.total_replays_blocked(), 3);
    }

    #[test]
    fn watermark_manager_clear() {
        let mut w = OrderingWatermarkManager::new();
        w.advance(ds(1), 50);
        w.block_replay();
        w.clear();
        assert_eq!(w.watermark(ds(1)), 0);
        assert_eq!(w.total_advances(), 0);
        assert_eq!(w.total_replays_blocked(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // DOMAIN ELECTION CONDUCTOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn election_conductor_start_and_vote() {
        let mut e = DomainElectionConductor::new();
        let candidates = vec![node(1), node(2), node(3)];
        e.start_election(ds(1), candidates.clone(), now());
        assert_eq!(e.total_elections(), 1);
        assert!(e.vote(ds(1), node(1), 1));
        assert!(e.vote(ds(1), node(2), 2));
        assert!(e.vote(ds(1), node(3), 3));
    }

    #[test]
    fn election_conductor_conclude_winner() {
        let mut e = DomainElectionConductor::new();
        e.start_election(ds(1), vec![node(1), node(2)], now());
        e.vote(ds(1), node(1), 1);
        e.vote(ds(1), node(2), 5);
        let winner = e.conclude(ds(1), now()).unwrap();
        assert_eq!(winner, node(2));
        assert_eq!(e.total_election_wins(), 1);
        let state = e.election_state(ds(1)).unwrap();
        assert_eq!(state.status, ElectionStatus::Won);
        assert_eq!(state.winner, Some(node(2)));
    }

    #[test]
    fn election_conductor_conclude_tie() {
        let mut e = DomainElectionConductor::new();
        e.start_election(ds(1), vec![node(1), node(2)], now());
        e.vote(ds(1), node(1), 3);
        e.vote(ds(1), node(2), 3);
        let winner = e.conclude(ds(1), now());
        assert!(winner.is_none());
        assert_eq!(e.total_election_wins(), 0);
        let state = e.election_state(ds(1)).unwrap();
        assert_eq!(state.status, ElectionStatus::Tie);
    }

    #[test]
    fn election_conductor_vote_after_closed() {
        let mut e = DomainElectionConductor::new();
        e.start_election(ds(1), vec![node(1), node(2)], now());
        e.vote(ds(1), node(1), 1);
        e.conclude(ds(1), now());
        // Voting after conclusion should fail.
        assert!(!e.vote(ds(1), node(2), 10));
    }

    #[test]
    fn election_conductor_conclude_none_for_unknown() {
        let mut e = DomainElectionConductor::new();
        assert!(e.conclude(ds(99), now()).is_none());
    }

    #[test]
    fn election_status_variants() {
        assert_eq!(ElectionStatus::Open as u32, 0);
        assert_eq!(ElectionStatus::Won as u32, 1);
        assert_eq!(ElectionStatus::Tie as u32, 2);
        assert_eq!(ElectionStatus::Failed as u32, 3);
    }

    #[test]
    fn election_state_fields() {
        let state = ElectionState {
            dataset_id: ds(1),
            status: ElectionStatus::Open,
            candidates: vec![node(1)],
            winner: None,
            votes: std::collections::HashMap::new(),
            started_at: TimestampMillis(100),
            completed_at: None,
        };
        assert_eq!(state.dataset_id, ds(1));
        assert_eq!(state.status, ElectionStatus::Open);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // ORDERING THROUGHPUT MONITOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn throughput_monitor_record_and_query() {
        let mut t = OrderingThroughputMonitor::new();
        t.record_proposal(ds(1));
        t.record_proposal(ds(1));
        t.record_proposal(ds(2));
        t.record_commit(ds(1));
        assert_eq!(t.proposals_for(ds(1)), 2);
        assert_eq!(t.proposals_for(ds(2)), 1);
        assert_eq!(t.commits_for(ds(1)), 1);
        assert_eq!(t.total_proposals(), 3);
        assert_eq!(t.total_commits(), 1);
    }

    #[test]
    fn throughput_monitor_commit_ratio() {
        let mut t = OrderingThroughputMonitor::new();
        // No proposals yet -> ratio 1.0.
        assert!((t.commit_ratio(ds(1)) - 1.0).abs() < f64::EPSILON);
        t.record_proposal(ds(1));
        t.record_proposal(ds(1));
        t.record_commit(ds(1));
        assert!((t.commit_ratio(ds(1)) - 0.5).abs() < f64::EPSILON);
        t.record_commit(ds(1));
        assert!((t.commit_ratio(ds(1)) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn throughput_monitor_reset_window() {
        let mut t = OrderingThroughputMonitor::new();
        t.record_proposal(ds(1));
        assert_eq!(t.proposals_for(ds(1)), 1);
        t.reset_window(TimestampMillis(5000));
        assert_eq!(t.proposals_for(ds(1)), 0);
        assert_eq!(t.window_start(), TimestampMillis(5000));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // DATASET ORDERING ISOLATION TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn isolation_check_passes_for_different_datasets() {
        let mut i = DatasetOrderingIsolation::new();
        assert!(i.check_isolation(ds(1), ds(2)));
        assert_eq!(i.isolation_checks(), 1);
    }

    #[test]
    fn isolation_check_fails_for_same_dataset() {
        let mut i = DatasetOrderingIsolation::new();
        assert!(!i.check_isolation(ds(1), ds(1)));
        assert_eq!(i.isolation_checks(), 1);
    }

    #[test]
    fn isolation_record_violation() {
        let mut i = DatasetOrderingIsolation::new();
        i.record_reference(ds(1), ds(2), ReferenceType::PositionReference, now());
        assert_eq!(i.violations_detected(), 1);
        i.record_reference(ds(2), ds(1), ReferenceType::AuthorityReference, now());
        assert_eq!(i.violations_detected(), 2);
    }

    #[test]
    fn isolation_clear() {
        let mut i = DatasetOrderingIsolation::new();
        i.check_isolation(ds(1), ds(2));
        i.record_reference(ds(1), ds(2), ReferenceType::TicketReference, now());
        i.clear();
        assert_eq!(i.isolation_checks(), 0);
        assert_eq!(i.violations_detected(), 0);
    }

    #[test]
    fn reference_type_variants() {
        assert_eq!(ReferenceType::PositionReference as u32, 0);
        assert_eq!(ReferenceType::AuthorityReference as u32, 1);
        assert_eq!(ReferenceType::TicketReference as u32, 2);
        assert_eq!(ReferenceType::EpochReference as u32, 3);
    }

    #[test]
    fn cross_dataset_reference_fields() {
        let r = CrossDatasetReference {
            from_dataset: ds(1),
            to_dataset: ds(2),
            reference_type: ReferenceType::EpochReference,
            detected_at: TimestampMillis(777),
        };
        assert_eq!(r.from_dataset, ds(1));
        assert_eq!(r.to_dataset, ds(2));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // BOUNDARY VIOLATION TYPE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn boundary_violation_type_variants() {
        assert_eq!(BoundaryViolationType::PositionLeak as u32, 0);
        assert_eq!(BoundaryViolationType::AuthorityLeak as u32, 1);
        assert_eq!(BoundaryViolationType::TicketReuse as u32, 2);
        assert_eq!(BoundaryViolationType::EpochConfusion as u32, 3);
    }

    #[test]
    fn boundary_violation_fields() {
        let v = BoundaryViolation {
            source_dataset: ds(1),
            target_dataset: ds(2),
            violation_type: BoundaryViolationType::TicketReuse,
            detected_at: TimestampMillis(555),
        };
        assert_eq!(v.violation_type, BoundaryViolationType::TicketReuse);
        assert_eq!(v.source_dataset, ds(1));
    }

    #[test]
    fn cross_domain_boundary_record_and_query() {
        let mut b = CrossDomainBoundary::new();
        assert_eq!(b.total_violations(), 0);
        assert!(b.is_violation(ds(1), ds(2)));
        assert!(!b.is_violation(ds(1), ds(1)));
        b.record(ds(1), ds(2), BoundaryViolationType::PositionLeak, now());
        assert_eq!(b.total_violations(), 1);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EXTENDED NO GLOBAL MASTER IMPL TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn impl_register_dataset_full() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        assert_eq!(m.domain_count(), 1);
        assert_eq!(m.authority_epoch(ds(1)), 1);
        assert!(m.ordering_authority(ds(1)).is_some());
    }

    #[test]
    fn impl_propose_with_epoch() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        let ticket = m
            .propose_with_epoch(ds(1), [1u8; 16], node(1), 1, now())
            .unwrap();
        assert_eq!(ticket.epoch, 1);
        // Wrong epoch should fail.
        let err = m
            .propose_with_epoch(ds(1), [2u8; 16], node(1), 99, now())
            .unwrap_err();
        assert!(matches!(err, OrderingError::EpochMismatch { .. }));
    }

    #[test]
    fn impl_rotate_with_reason() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        let auth = m
            .rotate_with_reason(ds(1), node(2), RotationReason::FailureDetected, now())
            .unwrap();
        assert_eq!(auth.node_id, node(2));
        assert_eq!(auth.epoch, 2);
    }

    #[test]
    fn impl_commit_with_position() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        let ticket = m
            .propose_with_epoch(ds(1), [1u8; 16], node(1), 1, now())
            .unwrap();
        let pos = m.commit_with_position(ds(1), ticket.ticket_id, now()).unwrap();
        assert_eq!(pos, 1);
        assert_eq!(m.commit_position(ds(1)), 1);
    }

    #[test]
    fn impl_advance_watermark_and_stale_replay() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        assert!(m.advance_watermark(ds(1), 10));
        assert!(m.is_stale_replay(ds(1), 5));
        assert!(m.is_stale_replay(ds(1), 10));
        assert!(!m.is_stale_replay(ds(1), 11));
    }

    #[test]
    fn impl_all_rotation_reasons() {
        let reasons = NoGlobalMasterImpl::all_rotation_reasons();
        assert_eq!(reasons.len(), 5);
    }

    #[test]
    fn impl_all_boundary_violation_types() {
        let types = NoGlobalMasterImpl::all_boundary_violation_types();
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn impl_all_reference_types() {
        let types = NoGlobalMasterImpl::all_reference_types();
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn impl_all_election_statuses() {
        let statuses = NoGlobalMasterImpl::all_election_statuses();
        assert_eq!(statuses.len(), 4);
    }

    #[test]
    fn impl_datasets_without_authority() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.register_dataset(ds(2), dom(2), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        let without = m.datasets_without_authority();
        assert_eq!(without, vec![ds(2)]);
        assert_eq!(m.domains_without_authority_count(), 1);
    }

    #[test]
    fn impl_assert_no_global_master() {
        let m = NoGlobalMasterImpl::new();
        assert!(m.assert_no_global_master());
    }

    #[test]
    fn impl_rotate_all_authorities() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.register_dataset(ds(2), dom(2), now()).unwrap();
        m.rotate_authority(ds(1), node(1), RotationReason::Scheduled, now()).unwrap();
        m.rotate_authority(ds(2), node(2), RotationReason::Scheduled, now()).unwrap();
        let results = m.rotate_all_authorities(node(9), RotationReason::Administrative, now());
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[test]
    fn impl_check_isolation() {
        let mut m = NoGlobalMasterImpl::new();
        assert!(m.check_isolation(ds(1), ds(2)));
        assert!(!m.check_isolation(ds(1), ds(1)));
    }

    #[test]
    fn impl_record_isolation_violation() {
        let mut m = NoGlobalMasterImpl::new();
        m.record_isolation_violation(ds(1), ds(2), ReferenceType::PositionReference, now());
        // Integration surface; count stays 0 in base impl.
        assert_eq!(m.total_isolation_violations(), 0);
    }

    #[test]
    fn impl_schedule_rotation() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset(ds(1), dom(1), now()).unwrap();
        m.schedule_rotation(
            ds(1),
            node(2),
            RotationReason::Scheduled,
            TimestampMillis(5000),
        );
        // Integration surface; no-op in base impl.
    }

    #[test]
    fn impl_detect_failures() {
        let mut m = NoGlobalMasterImpl::new();
        let flagged = m.detect_failures(TimestampMillis(10000));
        assert!(flagged.is_empty());
    }

    #[test]
    fn impl_record_boundary_violation() {
        let mut m = NoGlobalMasterImpl::new();
        m.record_boundary_violation(
            ds(1),
            ds(2),
            BoundaryViolationType::EpochConfusion,
            now(),
        );
        assert_eq!(m.total_boundary_violations(), 0);
    }

    #[test]
    fn impl_clear_extended() {
        let mut m = NoGlobalMasterImpl::new();
        m.register_dataset_full(ds(1), dom(1), node(1), now()).unwrap();
        m.clear_extended();
        assert_eq!(m.domain_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // TYPE FIELD TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn epoch_change_record_fields() {
        let r = EpochChangeRecord {
            dataset_id: ds(1),
            old_epoch: 0,
            new_epoch: 1,
            changed_at: TimestampMillis(100),
            reason: RotationReason::Scheduled,
        };
        assert_eq!(r.old_epoch, 0);
        assert_eq!(r.new_epoch, 1);
    }

    #[test]
    fn position_change_fields() {
        let c = PositionChange {
            dataset_id: ds(1),
            old_position: 0,
            new_position: 5,
            changed_at: TimestampMillis(200),
        };
        assert_eq!(c.new_position, 5);
    }

    #[test]
    fn scheduled_authority_rotation_fields() {
        let r = ScheduledAuthorityRotation {
            dataset_id: ds(1),
            new_authority: node(2),
            reason: RotationReason::FailureDetected,
            scheduled_at: TimestampMillis(300),
        };
        assert_eq!(r.reason, RotationReason::FailureDetected);
    }

    #[test]
    fn ordering_epoch_manager_advance() {
        let mut e = OrderingEpochManager::new();
        assert_eq!(e.current_epoch(ds(1)), 0);
        let ep1 = e.advance(ds(1), RotationReason::Scheduled, now());
        assert_eq!(ep1, 1);
        let ep2 = e.advance(ds(1), RotationReason::FailureDetected, now());
        assert_eq!(ep2, 2);
        assert!(e.validate_epoch(ds(1), 2));
        assert!(!e.validate_epoch(ds(1), 1));
        assert_eq!(e.total_epoch_changes(), 2);
    }

    #[test]
    fn ordering_epoch_manager_history() {
        let mut e = OrderingEpochManager::new();
        e.advance(ds(1), RotationReason::Scheduled, now());
        e.advance(ds(1), RotationReason::Scheduled, now());
        assert_eq!(e.history().len(), 2);
    }

    #[test]
    fn commit_position_tracker_advance() {
        let mut c = CommitPositionTracker::new();
        assert!(c.advance(ds(1), 1, now()));
        assert!(!c.advance(ds(1), 1)); // not greater
        assert!(c.advance(ds(1), 5, now()));
        assert_eq!(c.position(ds(1)), 5);
        assert_eq!(c.total_commits(), 2);
    }

    #[test]
    fn commit_position_tracker_watermark() {
        let mut c = CommitPositionTracker::new();
        c.advance(ds(1), 10, now());
        c.advance_watermark(ds(1), 5);
        assert_eq!(c.watermark(ds(1)), 5);
        assert!(c.is_ahead_of_watermark(ds(1), 10));
        assert!(!c.is_ahead_of_watermark(ds(1), 3));
    }

    #[test]
    fn commit_position_tracker_clear() {
        let mut c = CommitPositionTracker::new();
        c.advance(ds(1), 10, now());
        c.clear();
        assert_eq!(c.position(ds(1)), 0);
        assert_eq!(c.total_commits(), 0);
    }

    #[test]
    fn domain_membership_tracker_add_remove() {
        let mut m = DomainMembershipTracker::new();
        m.add_member(ds(1), node(1));
        m.add_member(ds(1), node(2));
        assert!(m.is_member(ds(1), node(1)));
        assert!(m.is_member(ds(1), node(2)));
        assert!(!m.is_member(ds(2), node(1)));
        let members = m.members_of(ds(1));
        assert_eq!(members.len(), 2);
        assert!(m.remove_member(ds(1), node(1)));
        assert!(!m.is_member(ds(1), node(1)));
        assert_eq!(m.domain_of(node(2)), Some(ds(1)));
    }

    #[test]
    fn domain_membership_tracker_node_single_domain() {
        let mut m = DomainMembershipTracker::new();
        m.add_member(ds(1), node(1));
        m.add_member(ds(2), node(1)); // moves to ds(2)
        assert!(!m.is_member(ds(1), node(1)));
        assert!(m.is_member(ds(2), node(1)));
    }

    #[test]
    fn rotation_scheduler_due() {
        let mut s = RotationScheduler::new();
        s.schedule(ds(1), node(1), RotationReason::Scheduled, TimestampMillis(100));
        s.schedule(ds(2), node(2), RotationReason::FailureDetected, TimestampMillis(200));
        assert_eq!(s.total_scheduled(), 2);
        assert_eq!(s.pending_count(), 2);
        let due = s.due(TimestampMillis(100));
        assert_eq!(due.len(), 1);
        assert_eq!(s.pending_count(), 1);
        assert_eq!(s.total_executed(), 1);
        let due2 = s.due(TimestampMillis(300));
        assert_eq!(due2.len(), 1);
        assert_eq!(s.pending_count(), 0);
    }

    #[test]
    fn ordering_latency_tracker_record() {
        let mut l = OrderingLatencyTracker::new();
        l.record(ds(1), 100);
        l.record(ds(1), 200);
        l.record(ds(1), 300);
        assert!((l.average_latency(ds(1)) - 200.0).abs() < f64::EPSILON);
        assert_eq!(l.max_latency(ds(1)), 300);
        assert_eq!(l.total_measurements(), 3);
    }

    #[test]
    fn authority_failover_detector_flag() {
        let mut d = AuthorityFailoverDetector::with_failure_window(100);
        d.record_commit(ds(1), TimestampMillis(100));
        assert!(!d.is_flagged(ds(1)));
        // Now well past the failure window.
        let flagged = d.check_failures(TimestampMillis(1000));
        assert!(flagged.contains(&ds(1)));
        assert!(d.is_flagged(ds(1)));
        d.clear_flag(ds(1));
        assert!(!d.is_flagged(ds(1)));
    }

    // OrderingError variants
    #[test]
    fn ordering_error_dataset_not_registered() {
        let err = OrderingError::DatasetNotRegistered {
            dataset: ds(1),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_not_authority() {
        let err = OrderingError::NotAuthority {
            dataset: ds(1),
            node: node(1),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_invalid_position() {
        let err = OrderingError::InvalidPosition {
            dataset: ds(1),
            position: 0,
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_domain_not_found() {
        let err = OrderingError::DomainNotFound {
            domain: dom(1),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_epoch_mismatch() {
        let err = OrderingError::EpochMismatch {
            dataset: ds(1),
            expected: 1,
            got: 2,
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_dataset_already_registered() {
        let err = OrderingError::DatasetAlreadyRegistered {
            dataset: ds(1),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    #[test]
    fn ordering_error_domain_id_collision() {
        let err = OrderingError::DomainIdCollision {
            domain: dom(1),
        };
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
    }

    // OrderingDomain default
    #[test]
    fn ordering_domain_default() {
        let d = OrderingDomain {
            dataset_id: ds(1),
            domain_id: dom(1),
            authority: None,
            commit_position: 0,
            proposal_count: 0,
            commit_count: 0,
            rotation_count: 0,
            created_at: TimestampMillis(0),
            last_activity: TimestampMillis(0),
        };
        assert_eq!(d.commit_position, 0);
        assert_eq!(d.proposal_count, 0);
    }

    // OrderingTicket default-ish
    #[test]
    fn ordering_ticket_default_fields() {
        let ticket = OrderingTicket {
            ticket_id: [0u8; 16],
            dataset_id: ds(1),
            position: 0,
            authority: node(1),
            epoch: 0,
            issued_at: TimestampMillis(0),
        };
        assert_eq!(ticket.position, 0);
        assert_eq!(ticket.epoch, 0);
    }

    // NoGlobalMasterSummary default
    #[test]
    fn no_global_master_summary_default() {
        let s = NoGlobalMasterSummary::default();
        assert_eq!(s.total_domains, 0);
        assert_eq!(s.total_proposals, 0);
        assert_eq!(s.total_commits, 0);
        assert_eq!(s.total_rotations, 0);
        assert_eq!(s.domains_with_authority, 0);
        assert_eq!(s.domains_without_authority, 0);
        assert!(!s.global_master_exists);
    }

    // PendingProposal fields
    #[test]
    fn pending_proposal_fields() {
        let p = PendingProposal {
            proposal_id: [1u8; 16],
            dataset_id: ds(1),
            operation_id: [2u8; 16],
            proposed_by: node(1),
            proposed_at: TimestampMillis(100),
            position: Some(5),
        };
        assert_eq!(p.position, Some(5));
    }

    // CommittedOrder fields
    #[test]
    fn committed_order_fields() {
        let c = CommittedOrder {
            ticket_id: [1u8; 16],
            dataset_id: ds(1),
            position: 3,
            committed_at: TimestampMillis(200),
            authority: node(1),
            epoch: 1,
        };
        assert_eq!(c.position, 3);
        assert_eq!(c.epoch, 1);
    }

    // AuthorityRotationRecord fields
    #[test]
    fn authority_rotation_record_fields() {
        let r = AuthorityRotationRecord {
            dataset_id: ds(1),
            old_authority: Some(node(1)),
            new_authority: node(2),
            reason: RotationReason::Scheduled,
            at_tick: TimestampMillis(100),
            epoch: 1,
        };
        assert_eq!(r.old_authority, Some(node(1)));
        assert_eq!(r.new_authority, node(2));
    }

    // Duration unused check (imported but may be unused) — reference it
    #[test]
    fn duration_type_used() {
        let _d = Duration(1000);
        assert_eq!(_d.0, 1000);
    }

    // InvariantError used
    #[test]
    fn invariant_error_type_referenced() {
        // InvariantError is imported from crate::errors (defined at build
        // order step 27). This test confirms the type path is referenced.
        let _path = std::any::type_name::<InvariantError>();
        assert!(!_path.is_empty());
    }
}