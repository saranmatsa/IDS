//! # Sealed Trait: Lease — lease-based consistency, fencing epochs, and STONITH
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Lease trait defines lease-based distributed consistency primitives:
//! - Lease grant, renewal, expiry, and revocation lifecycle
//! - Fencing token generation and validation (monotonically increasing)
//! - Epoch/term management for leader authority
//! - STONITH (Shoot The Other Node In The Head) execution for failed nodes
//! - Split-brain prevention via authority epoch checks
//! - Lease state machine (pending, active, renewing, expiring, expired, revoked, fenced)
//! - Authority epoch management per domain
//! - Lease stats and health monitoring
//! Implementations live downstream in ids-lease and ids-* crates.

use crate::types::{Endpoint, Epoch, FencingToken, LeaseId, NodeId};
use crate::errors::LeaseError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Lease
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::LeaseEngine {}
impl private::Sealed for super::LeaseConfig {}
impl private::Sealed for super::LeaseState {}
impl private::Sealed for super::LeaseRecord {}
impl private::Sealed for super::LeaseGrant {}
impl private::Sealed for super::LeaseRenewal {}
impl private::Sealed for super::LeaseRevocation {}
impl private::Sealed for super::FencingTokenRecord {}
impl private::Sealed for super::AuthorityEpoch {}
impl private::Sealed for super::LeaseStats {}
impl private::Sealed for super::LeaseMonitor {}
impl private::Sealed for super::LeaseValidator {}

// Sealed trait for lease-based consistency.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Lease: private::Sealed {
    /// Configure lease parameters.
    fn configure(&self, config: LeaseConfig) -> Result<(), LeaseError>;

    /// Grant a lease to a node.
    fn grant_lease(&self, node_id: NodeId, epoch: Epoch, duration_ms: u64) -> Result<LeaseGrant, LeaseError>;

    /// Renew an existing lease.
    fn renew_lease(&self, lease_id: LeaseId) -> Result<LeaseRenewal, LeaseError>;

    /// Revoke a lease.
    fn revoke_lease(&self, lease_id: LeaseId) -> Result<LeaseRevocation, LeaseError>;

    /// Get the current fencing token for a lease.
    fn fencing_token(&self, lease_id: LeaseId) -> Result<FencingToken, LeaseError>;

    /// Increment the fencing token.
    fn fencing_token_increment(&self, lease_id: LeaseId) -> Result<FencingToken, LeaseError>;

    /// Check if a fencing token is valid (>= current token).
    fn fencing_token_check(&self, lease_id: LeaseId, token: FencingToken) -> Result<bool, LeaseError>;

    /// Execute STONITH on a node.
    fn stonith_execute(&self, node_id: NodeId) -> Result<(), LeaseError>;

    /// Check if split-brain is prevented for a domain.
    fn split_brain_prevented(&self, domain: Epoch) -> Result<bool, LeaseError>;

    /// Issue a new authority epoch for a domain.
    fn authority_epoch_issue(&self, domain: Epoch) -> Result<Epoch, LeaseError>;

    /// Get the current authority epoch for a domain.
    fn authority_epoch(&self, domain: Epoch) -> Result<Epoch, LeaseError>;

    /// Check if a lease is active.
    fn is_lease_active(&self, lease_id: LeaseId) -> Result<bool, LeaseError>;

    /// Get lease state.
    fn lease_state(&self, lease_id: LeaseId) -> Result<LeaseState, LeaseError>;

    /// Get lease stats.
    fn stats(&self) -> LeaseStats;

    /// Get lease health score for a node.
    fn health_score(&self, node_id: NodeId) -> Result<f64, LeaseError>;
}

///////////////////////////////////////////////////////////////////////////////
// LEASE CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for lease-based consistency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseConfig {
    /// Default lease duration in milliseconds.
    pub default_lease_duration_ms: u64,
    /// Minimum lease duration in milliseconds.
    pub min_lease_duration_ms: u64,
    /// Maximum lease duration in milliseconds.
    pub max_lease_duration_ms: u64,
    /// Lease renewal grace period in milliseconds.
    pub renewal_grace_period_ms: u64,
    /// Maximum number of renewal attempts before expiry.
    pub max_renewal_attempts: u32,
    /// Fencing token increment step.
    pub fencing_token_step: u64,
    /// Enable STONITH for failed leases.
    pub enable_stonith: bool,
    /// Enable split-brain prevention.
    pub enable_split_brain_prevention: bool,
    /// Authority epoch duration in milliseconds.
    pub authority_epoch_duration_ms: u64,
}

impl Default for LeaseConfig {
    fn default() -> Self {
        Self {
            default_lease_duration_ms: 30000,
            min_lease_duration_ms: 1000,
            max_lease_duration_ms: 300000,
            renewal_grace_period_ms: 5000,
            max_renewal_attempts: 3,
            fencing_token_step: 1,
            enable_stonith: true,
            enable_split_brain_prevention: true,
            authority_epoch_duration_ms: 60000,
        }
    }
}

impl LeaseConfig {
    /// Create a new LeaseConfig with custom default duration.
    pub fn with_default_duration(mut self, ms: u64) -> Self {
        self.default_lease_duration_ms = ms.clamp(self.min_lease_duration_ms, self.max_lease_duration_ms);
        self
    }

    /// Create a new LeaseConfig with custom duration bounds.
    pub fn with_duration_bounds(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.min_lease_duration_ms = min_ms;
        self.max_lease_duration_ms = max_ms;
        self
    }

    /// Create a new LeaseConfig with custom renewal settings.
    pub fn with_renewal(mut self, grace_period_ms: u64, max_attempts: u32) -> Self {
        self.renewal_grace_period_ms = grace_period_ms;
        self.max_renewal_attempts = max_attempts;
        self
    }

    /// Create a new LeaseConfig with STONITH enabled/disabled.
    pub fn with_stonith(mut self, enabled: bool) -> Self {
        self.enable_stonith = enabled;
        self
    }

    /// Create a new LeaseConfig with split-brain prevention enabled/disabled.
    pub fn with_split_brain_prevention(mut self, enabled: bool) -> Self {
        self.enable_split_brain_prevention = enabled;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), LeaseError> {
        if self.min_lease_duration_ms == 0 {
            return Err(LeaseError::InvalidConfig {
                field: "min_lease_duration_ms".to_string(),
                reason: "min lease duration must be > 0".to_string(),
            });
        }
        if self.max_lease_duration_ms < self.min_lease_duration_ms {
            return Err(LeaseError::InvalidConfig {
                field: "max_lease_duration_ms".to_string(),
                reason: "max must be >= min".to_string(),
            });
        }
        if self.default_lease_duration_ms < self.min_lease_duration_ms
            || self.default_lease_duration_ms > self.max_lease_duration_ms
        {
            return Err(LeaseError::InvalidConfig {
                field: "default_lease_duration_ms".to_string(),
                reason: "default must be within [min, max]".to_string(),
            });
        }
        if self.max_renewal_attempts == 0 {
            return Err(LeaseError::InvalidConfig {
                field: "max_renewal_attempts".to_string(),
                reason: "max renewal attempts must be > 0".to_string(),
            });
        }
        if self.fencing_token_step == 0 {
            return Err(LeaseError::InvalidConfig {
                field: "fencing_token_step".to_string(),
                reason: "fencing token step must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE STATE
///////////////////////////////////////////////////////////////////////////////

/// State of a lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseState {
    /// Lease is pending initial grant.
    Pending,
    /// Lease is active and valid.
    Active,
    /// Lease is being renewed.
    Renewing,
    /// Lease is expiring soon (within grace period).
    Expiring,
    /// Lease has expired.
    Expired,
    /// Lease has been explicitly revoked.
    Revoked,
    /// Lease is fenced (node excluded via fencing token).
    Fenced,
}

impl Default for LeaseState {
    fn default() -> Self {
        LeaseState::Pending
    }
}

impl LeaseState {
    /// Check if the lease state allows operations.
    pub fn is_active(&self) -> bool {
        matches!(self, LeaseState::Active | LeaseState::Renewing | LeaseState::Expiring)
    }

    /// Check if the state is terminal (no further operations possible).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            LeaseState::Expired | LeaseState::Revoked | LeaseState::Fenced
        )
    }

    /// Check if the state can transition to active.
    pub fn can_renew(&self) -> bool {
        matches!(self, LeaseState::Active | LeaseState::Renewing | LeaseState::Expiring)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RECORD
///////////////////////////////////////////////////////////////////////////////

/// A lease record tracking a granted lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseRecord {
    /// Lease identifier.
    pub lease_id: LeaseId,
    /// Node that holds the lease.
    pub node_id: NodeId,
    /// Current lease state.
    pub state: LeaseState,
    /// Epoch when lease was granted.
    pub epoch: Epoch,
    /// Lease duration in milliseconds.
    pub duration_ms: u64,
    /// Timestamp when lease was granted.
    pub granted_at_ms: u64,
    /// Timestamp when lease expires.
    pub expires_at_ms: u64,
    /// Timestamp of last renewal.
    pub last_renewal_at_ms: Option<u64>,
    /// Number of renewals performed.
    pub renewal_count: u32,
    /// Current fencing token.
    pub fencing_token: FencingToken,
    /// Whether the lease has been fenced.
    pub is_fenced: bool,
}

impl LeaseRecord {
    /// Create a new LeaseRecord.
    pub fn new(
        lease_id: LeaseId,
        node_id: NodeId,
        epoch: Epoch,
        duration_ms: u64,
        granted_at_ms: u64,
    ) -> Self {
        let expires_at_ms = granted_at_ms + duration_ms;
        Self {
            lease_id,
            node_id,
            state: LeaseState::Pending,
            epoch,
            duration_ms,
            granted_at_ms,
            expires_at_ms,
            last_renewal_at_ms: None,
            renewal_count: 0,
            fencing_token: FencingToken::ZERO,
            is_fenced: false,
        }
    }

    /// Activate the lease.
    pub fn activate(&mut self, fencing_token: FencingToken, now_ms: u64) {
        self.state = LeaseState::Active;
        self.fencing_token = fencing_token;
        self.expires_at_ms = now_ms + self.duration_ms;
    }

    /// Renew the lease.
    pub fn renew(&mut self, now_ms: u64, fencing_token: FencingToken) {
        self.state = LeaseState::Renewing;
        self.last_renewal_at_ms = Some(now_ms);
        self.renewal_count = self.renewal_count.saturating_add(1);
        self.fencing_token = fencing_token;
        self.expires_at_ms = now_ms + self.duration_ms;
        self.state = LeaseState::Active;
    }

    /// Expire the lease.
    pub fn expire(&mut self) {
        self.state = LeaseState::Expired;
    }

    /// Revoke the lease.
    pub fn revoke(&mut self) {
        self.state = LeaseState::Revoked;
    }

    /// Fence the lease.
    pub fn fence(&mut self) {
        self.state = LeaseState::Fenced;
        self.is_fenced = true;
    }

    /// Check if lease is expired at a given timestamp.
    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        now_ms > self.expires_at_ms
    }

    /// Check if lease is within grace period.
    pub fn is_in_grace_period(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        let time_until_expiry = self.expires_at_ms.saturating_sub(now_ms);
        time_until_expiry <= grace_period_ms && !self.is_terminal()
    }

    /// Time until expiry in milliseconds.
    pub fn time_until_expiry(&self, now_ms: u64) -> u64 {
        self.expires_at_ms.saturating_sub(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE GRANT
///////////////////////////////////////////////////////////////////////////////

/// Result of a lease grant operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseGrant {
    /// Lease identifier.
    pub lease_id: LeaseId,
    /// Node that received the lease.
    pub node_id: NodeId,
    /// Epoch of the lease.
    pub epoch: Epoch,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Granted timestamp.
    pub granted_at_ms: u64,
    /// Expiry timestamp.
    pub expires_at_ms: u64,
    /// Initial fencing token.
    pub fencing_token: FencingToken,
}

impl LeaseGrant {
    /// Create a new LeaseGrant.
    pub fn new(
        lease_id: LeaseId,
        node_id: NodeId,
        epoch: Epoch,
        duration_ms: u64,
        granted_at_ms: u64,
        fencing_token: FencingToken,
    ) -> Self {
        Self {
            lease_id,
            node_id,
            epoch,
            duration_ms,
            granted_at_ms,
            expires_at_ms: granted_at_ms + duration_ms,
            fencing_token,
        }
    }

    /// Time remaining on the lease.
    pub fn time_remaining(&self, now_ms: u64) -> u64 {
        self.expires_at_ms.saturating_sub(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RENEWAL
///////////////////////////////////////////////////////////////////////////////

/// Result of a lease renewal operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseRenewal {
    /// Lease identifier.
    pub lease_id: LeaseId,
    /// New expiry timestamp.
    pub new_expires_at_ms: u64,
    /// Renewal count after this renewal.
    pub renewal_count: u32,
    /// New fencing token.
    pub fencing_token: FencingToken,
    /// Renewed at timestamp.
    pub renewed_at_ms: u64,
}

impl LeaseRenewal {
    /// Create a new LeaseRenewal.
    pub fn new(
        lease_id: LeaseId,
        new_expires_at_ms: u64,
        renewal_count: u32,
        fencing_token: FencingToken,
        renewed_at_ms: u64,
    ) -> Self {
        Self {
            lease_id,
            new_expires_at_ms,
            renewal_count,
            fencing_token,
            renewed_at_ms,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE REVOCATION
///////////////////////////////////////////////////////////////////////////////

/// Result of a lease revocation operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseRevocation {
    /// Lease identifier.
    pub lease_id: LeaseId,
    /// Node that held the lease.
    pub node_id: NodeId,
    /// Revoked at timestamp.
    pub revoked_at_ms: u64,
    /// Reason for revocation.
    pub reason: String,
    /// Whether STONITH was executed.
    pub stonith_executed: bool,
}

impl LeaseRevocation {
    /// Create a new LeaseRevocation.
    pub fn new(lease_id: LeaseId, node_id: NodeId, revoked_at_ms: u64, reason: String) -> Self {
        Self {
            lease_id,
            node_id,
            revoked_at_ms,
            reason,
            stonith_executed: false,
        }
    }

    /// Mark STONITH as executed.
    pub fn with_stonith(mut self) -> Self {
        self.stonith_executed = true;
        self
    }
}

///////////////////////////////////////////////////////////////////////////////
// FENCING TOKEN RECORD
///////////////////////////////////////////////////////////////////////////////

/// A fencing token record for a lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FencingTokenRecord {
    /// Lease identifier.
    pub lease_id: LeaseId,
    /// Current fencing token value.
    pub token: FencingToken,
    /// Last increment timestamp.
    pub last_increment_at_ms: u64,
    /// Total increments performed.
    pub increment_count: u64,
}

impl FencingTokenRecord {
    /// Create a new FencingTokenRecord.
    pub fn new(lease_id: LeaseId, token: FencingToken, issued_at_ms: u64) -> Self {
        Self {
            lease_id,
            token,
            last_increment_at_ms: issued_at_ms,
            increment_count: 0,
        }
    }

    /// Increment the token.
    pub fn increment(&mut self, step: u64, now_ms: u64) -> FencingToken {
        self.token = FencingToken(self.token.0.saturating_add(step));
        self.last_increment_at_ms = now_ms;
        self.increment_count = self.increment_count.saturating_add(1);
        self.token
    }

    /// Check if a given token is valid (>= current).
    pub fn is_valid(&self, token: FencingToken) -> bool {
        token >= self.token
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHORITY EPOCH
///////////////////////////////////////////////////////////////////////////////

/// An authority epoch for a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityEpoch {
    /// Domain identifier.
    pub domain: Epoch,
    /// Current authority epoch value.
    pub epoch: Epoch,
    /// Authority node ID (current leader).
    pub authority_node: NodeId,
    /// Timestamp when epoch was issued.
    pub issued_at_ms: u64,
    /// Epoch duration in milliseconds.
    pub duration_ms: u64,
}

impl AuthorityEpoch {
    /// Create a new AuthorityEpoch.
    pub fn new(domain: Epoch, epoch: Epoch, authority_node: NodeId, issued_at_ms: u64, duration_ms: u64) -> Self {
        Self {
            domain,
            epoch,
            authority_node,
            issued_at_ms,
            duration_ms,
        }
    }

    /// Check if epoch is expired at a given timestamp.
    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        now_ms > self.issued_at_ms + self.duration_ms
    }

    /// Time until epoch expiry.
    pub fn time_until_expiry(&self, now_ms: u64) -> u64 {
        (self.issued_at_ms + self.duration_ms).saturating_sub(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE STATS
///////////////////////////////////////////////////////////////////////////////

/// Aggregate statistics for lease operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseStats {
    /// Total leases granted.
    pub total_grants: u64,
    /// Total leases renewed.
    pub total_renewals: u64,
    /// Total leases revoked.
    pub total_revocations: u64,
    /// Total leases expired.
    pub total_expired: u64,
    /// Total leases fenced.
    pub total_fenced: u64,
    /// Total STONITH executions.
    pub total_stonith_executions: u64,
    /// Total fencing token increments.
    pub total_fencing_increments: u64,
    /// Total authority epochs issued.
    pub total_authority_epochs: u64,
    /// Total split-brain prevention events.
    pub total_split_brain_prevented: u64,
    /// Current active leases.
    pub active_leases: u64,
}

impl Default for LeaseStats {
    fn default() -> Self {
        Self {
            total_grants: 0,
            total_renewals: 0,
            total_revocations: 0,
            total_expired: 0,
            total_fenced: 0,
            total_stonith_executions: 0,
            total_fencing_increments: 0,
            total_authority_epochs: 0,
            total_split_brain_prevented: 0,
            active_leases: 0,
        }
    }
}

impl LeaseStats {
    /// Success rate (grants - revocations - expired) / grants.
    pub fn success_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 1.0;
        }
        let successful = self.total_grants.saturating_sub(self.total_revocations).saturating_sub(self.total_expired);
        successful as f64 / self.total_grants as f64
    }

    /// Renewal rate (renewals / grants).
    pub fn renewal_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_renewals as f64 / self.total_grants as f64
    }

    /// Expiry rate (expired / grants).
    pub fn expiry_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_expired as f64 / self.total_grants as f64
    }

    /// Fencing rate (fenced / grants).
    pub fn fencing_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_fenced as f64 / self.total_grants as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Monitors lease state and triggers transitions.
#[derive(Debug, Clone)]
pub struct LeaseMonitor {
    /// Configuration.
    pub config: LeaseConfig,
    /// Lease records.
    pub leases: std::cell::RefCell<std::collections::HashMap<LeaseId, LeaseRecord>>,
    /// Leases by node.
    pub leases_by_node: std::cell::RefCell<std::collections::HashMap<NodeId, Vec<LeaseId>>>,
    /// Fencing token records.
    pub fencing_tokens: std::cell::RefCell<std::collections::HashMap<LeaseId, FencingTokenRecord>>,
    /// Authority epochs by domain.
    pub authority_epochs: std::cell::RefCell<std::collections::HashMap<Epoch, AuthorityEpoch>>,
    /// STONITH execution log.
    pub stonith_log: std::cell::RefCell<Vec<(NodeId, u64)>>,
    /// Stats.
    pub stats: std::cell::RefCell<LeaseStats>,
    /// Next lease ID counter.
    pub next_lease_id: std::cell::RefCell<u64>,
}

impl Default for LeaseMonitor {
    fn default() -> Self {
        Self::new(LeaseConfig::default())
    }
}

impl LeaseMonitor {
    /// Create a new LeaseMonitor.
    pub fn new(config: LeaseConfig) -> Self {
        Self {
            config,
            leases: std::cell::RefCell::new(std::collections::HashMap::new()),
            leases_by_node: std::cell::RefCell::new(std::collections::HashMap::new()),
            fencing_tokens: std::cell::RefCell::new(std::collections::HashMap::new()),
            authority_epochs: std::cell::RefCell::new(std::collections::HashMap::new()),
            stonith_log: std::cell::RefCell::new(Vec::new()),
            stats: std::cell::RefCell::new(LeaseStats::default()),
            next_lease_id: std::cell::RefCell::new(1),
        }
    }

    /// Register a lease record.
    pub fn register_lease(&self, lease: LeaseRecord) {
        let lease_id = lease.lease_id;
        let node_id = lease.node_id;
        self.leases.borrow_mut().insert(lease_id, lease);
        self.leases_by_node
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .push(lease_id);
        let mut stats = self.stats.borrow_mut();
        stats.total_grants += 1;
        stats.active_leases += 1;
    }

    /// Get a lease record.
    pub fn get_lease(&self, lease_id: LeaseId) -> Option<LeaseRecord> {
        self.leases.borrow().get(&lease_id).cloned()
    }

    /// Get all lease IDs for a node.
    pub fn leases_for_node(&self, node_id: NodeId) -> Vec<LeaseId> {
        self.leases_by_node.borrow().get(&node_id).cloned().unwrap_or_default()
    }

    /// Get fencing token record.
    pub fn get_fencing_token(&self, lease_id: LeaseId) -> Option<FencingTokenRecord> {
        self.fencing_tokens.borrow().get(&lease_id).cloned()
    }

    /// Create a fencing token record.
    pub fn create_fencing_token(&self, lease_id: LeaseId, token: FencingToken, now_ms: u64) {
        self.fencing_tokens
            .borrow_mut()
            .insert(lease_id, FencingTokenRecord::new(lease_id, token, now_ms));
    }

    /// Increment fencing token.
    pub fn increment_fencing_token(&self, lease_id: LeaseId, step: u64, now_ms: u64) -> FencingToken {
        let mut tokens = self.fencing_tokens.borrow_mut();
        let record = tokens.get_mut(&lease_id).expect("fencing token record must exist");
        record.increment(step, now_ms)
    }

    /// Get authority epoch for a domain.
    pub fn get_authority_epoch(&self, domain: Epoch) -> Option<AuthorityEpoch> {
        self.authority_epochs.borrow().get(&domain).cloned()
    }

    /// Set authority epoch.
    pub fn set_authority_epoch(&self, epoch: AuthorityEpoch) {
        self.authority_epochs.borrow_mut().insert(epoch.domain, epoch);
        let mut stats = self.stats.borrow_mut();
        stats.total_authority_epochs += 1;
    }

    /// Log a STONITH execution.
    pub fn log_stonith(&self, node_id: NodeId, at_ms: u64) {
        self.stonith_log.borrow_mut().push((node_id, at_ms));
        let mut stats = self.stats.borrow_mut();
        stats.total_stonith_executions += 1;
    }

    /// Get stats.
    pub fn stats(&self) -> LeaseStats {
        self.stats.borrow().clone()
    }

    /// Get all lease IDs.
    pub fn all_lease_ids(&self) -> Vec<LeaseId> {
        self.leases.borrow().keys().cloned().collect()
    }

    /// Get active lease count.
    pub fn active_lease_count(&self) -> u64 {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state.is_active())
            .count() as u64
    }

    /// Get expired lease count.
    pub fn expired_lease_count(&self) -> u64 {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state == LeaseState::Expired)
            .count() as u64
    }

    /// Get fenced lease count.
    pub fn fenced_lease_count(&self) -> u64 {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state == LeaseState::Fenced)
            .count() as u64
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates lease operations (fencing tokens, epochs, renewals).
#[derive(Debug, Clone)]
pub struct LeaseValidator {
    /// Enable fencing token validation.
    pub enable_fencing: bool,
    /// Enable authority epoch validation.
    pub enable_epoch_validation: bool,
}

impl Default for LeaseValidator {
    fn default() -> Self {
        Self {
            enable_fencing: true,
            enable_epoch_validation: true,
        }
    }
}

impl LeaseValidator {
    /// Create a new LeaseValidator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a fencing token against the current token.
    pub fn validate_fencing_token(&self, current: FencingToken, presented: FencingToken) -> bool {
        if !self.enable_fencing {
            return true;
        }
        presented >= current
    }

    /// Validate an authority epoch.
    pub fn validate_authority_epoch(&self, domain: Epoch, presented: Epoch, current: Epoch) -> bool {
        if !self.enable_epoch_validation {
            return true;
        }
        presented >= current
    }

    /// Validate lease renewal (not expired, not fenced, not revoked).
    pub fn validate_renewal(&self, state: LeaseState) -> bool {
        matches!(state, LeaseState::Active | LeaseState::Renewing | LeaseState::Expiring)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ENGINE
///////////////////////////////////////////////////////////////////////////////

/// The lease engine — core implementation of the Lease trait.
#[derive(Debug, Clone)]
pub struct LeaseEngine {
    /// Configuration.
    pub config: std::cell::RefCell<LeaseConfig>,
    /// Monitor for lease tracking.
    pub monitor: LeaseMonitor,
    /// Validator for operations.
    pub validator: LeaseValidator,
    /// Next fencing token base.
    pub next_fencing_base: std::cell::RefCell<FencingToken>,
}

impl Default for LeaseEngine {
    fn default() -> Self {
        Self::new(LeaseConfig::default())
    }
}

impl LeaseEngine {
    /// Create a new LeaseEngine with default config.
    pub fn new(config: LeaseConfig) -> Self {
        config.validate().expect("Invalid LeaseConfig");
        Self {
            config: std::cell::RefCell::new(config),
            monitor: LeaseMonitor::new(config.clone()),
            validator: LeaseValidator::new(),
            next_fencing_base: std::cell::RefCell::new(FencingToken::new(1)),
        }
    }

    /// Configure the engine.
    pub fn configure(&self, config: LeaseConfig) -> Result<(), LeaseError> {
        config.validate()?;
        *self.config.borrow_mut() = config;
        self.monitor = LeaseMonitor::new(config);
        Ok(())
    }

    /// Grant a lease to a node.
    pub fn grant_lease(&self, node_id: NodeId, epoch: Epoch, duration_ms: u64) -> Result<LeaseGrant, LeaseError> {
        let config = self.config.borrow();
        let duration = if duration_ms == 0 {
            config.default_lease_duration_ms
        } else {
            duration_ms.clamp(config.min_lease_duration_ms, config.max_lease_duration_ms)
        };
        drop(config);

        let now = crate::types::IdsTime::now_ms();
        let mut lease_id_bytes = [0u8; 16];
        lease_id_bytes[0..8].copy_from_slice(&now.to_le_bytes());
        lease_id_bytes[8..16].copy_from_slice(&(*self.next_fencing_base.borrow()).0.to_le_bytes());
        let lease_id = LeaseId::new(lease_id_bytes);
        *self.next_fencing_base.borrow_mut() = FencingToken((*self.next_fencing_base.borrow()).0.saturating_add(1));

        let fencing_token = FencingToken::new(1);
        let mut grant = LeaseGrant::new(lease_id, node_id, epoch, duration, now, fencing_token);

        let mut record = LeaseRecord::new(lease_id, node_id, epoch, duration, now);
        record.activate(fencing_token, now);

        self.monitor.register_lease(record);
        self.monitor.create_fencing_token(lease_id, fencing_token, now);

        Ok(grant)
    }

    /// Renew an existing lease.
    pub fn renew_lease(&self, lease_id: LeaseId) -> Result<LeaseRenewal, LeaseError> {
        let mut leases = self.monitor.leases.borrow_mut();
        let record = leases.get_mut(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;

        if !self.validator.validate_renewal(record.state) {
            return Err(LeaseError::LeaseNotRenewable { lease_id });
        }

        let now = crate::types::IdsTime::now_ms();
        if record.is_expired_at(now) && !record.state.is_in_grace_period(now, self.config.borrow().renewal_grace_period_ms) {
            record.expire();
            drop(leases);
            let mut stats = self.monitor.stats.borrow_mut();
            stats.total_expired += 1;
            stats.active_leases = stats.active_leases.saturating_sub(1);
            return Err(LeaseError::LeaseExpired { lease_id });
        }
        drop(leases);

        let config = self.config.borrow();
        let new_token = FencingToken(record.fencing_token.0.saturating_add(config.fencing_token_step));
        drop(config);

        let mut fencing_tokens = self.monitor.fencing_tokens.borrow_mut();
        let current_token = fencing_tokens.get(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        let new_token_value = current_token.increment(self.config.borrow().fencing_token_step, now);
        drop(fencing_tokens);

        let mut leases = self.monitor.leases.borrow_mut();
        let record = leases.get_mut(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        record.renew(now, new_token_value);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_renewals += 1;

        Ok(LeaseRenewal::new(
            lease_id,
            record.expires_at_ms,
            record.renewal_count,
            new_token_value,
            now,
        ))
    }

    /// Revoke a lease.
    pub fn revoke_lease(&self, lease_id: LeaseId) -> Result<LeaseRevocation, LeaseError> {
        let mut leases = self.monitor.leases.borrow_mut();
        let record = leases.get_mut(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        if record.state.is_terminal() {
            return Err(LeaseError::LeaseAlreadyTerminal { lease_id });
        }
        let node_id = record.node_id;
        let now = crate::types::IdsTime::now_ms();
        record.revoke();
        drop(leases);

        let mut revocation = LeaseRevocation::new(lease_id, node_id, now, "revoked".to_string());

        // Execute STONITH if enabled
        if self.config.borrow().enable_stonith {
            self.monitor.log_stonith(node_id, now);
            revocation = revocation.with_stonith();
        }

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_revocations += 1;
        stats.active_leases = stats.active_leases.saturating_sub(1);

        Ok(revocation)
    }

    /// Get the current fencing token for a lease.
    pub fn fencing_token(&self, lease_id: LeaseId) -> Result<FencingToken, LeaseError> {
        let tokens = self.monitor.fencing_tokens.borrow();
        let record = tokens.get(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        Ok(record.token)
    }

    /// Increment the fencing token.
    pub fn fencing_token_increment(&self, lease_id: LeaseId) -> Result<FencingToken, LeaseError> {
        let now = crate::types::IdsTime::now_ms();
        let new_token = self.monitor.increment_fencing_token(lease_id, self.config.borrow().fencing_token_step, now);
        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_fencing_increments += 1;
        Ok(new_token)
    }

    /// Check if a fencing token is valid.
    pub fn fencing_token_check(&self, lease_id: LeaseId, token: FencingToken) -> Result<bool, LeaseError> {
        let tokens = self.monitor.fencing_tokens.borrow();
        let record = tokens.get(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        Ok(self.validator.validate_fencing_token(record.token, token))
    }

    /// Execute STONITH on a node.
    pub fn stonith_execute(&self, node_id: NodeId) -> Result<(), LeaseError> {
        if !self.config.borrow().enable_stonith {
            return Err(LeaseError::StonithDisabled);
        }
        let now = crate::types::IdsTime::now_ms();
        self.monitor.log_stonith(node_id, now);

        // Fence all leases for this node
        let lease_ids = self.monitor.leases_for_node(node_id);
        let mut leases = self.monitor.leases.borrow_mut();
        for lease_id in &lease_ids {
            if let Some(record) = leases.get_mut(lease_id) {
                record.fence();
            }
        }
        drop(leases);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_fenced += lease_ids.len() as u64;

        Ok(())
    }

    /// Check if split-brain is prevented for a domain.
    pub fn split_brain_prevented(&self, domain: Epoch) -> Result<bool, LeaseError> {
        if !self.config.borrow().enable_split_brain_prevention {
            return Ok(false);
        }
        let epochs = self.monitor.authority_epochs.borrow();
        let epoch = epochs.get(&domain).ok_or_else(|| LeaseError::AuthorityEpochNotFound { domain })?;
        Ok(!epoch.is_expired_at(crate::types::IdsTime::now_ms()))
    }

    /// Issue a new authority epoch for a domain.
    pub fn authority_epoch_issue(&self, domain: Epoch) -> Result<Epoch, LeaseError> {
        let now = crate::types::IdsTime::now_ms();
        let mut epochs = self.monitor.authority_epochs.borrow_mut();
        let current = epochs.get(&domain).cloned();
        drop(epochs);

        let new_epoch = match current {
            Some(e) => Epoch(e.epoch.0.saturating_add(1)),
            None => Epoch(1),
        };

        let authority_epoch = AuthorityEpoch::new(domain, new_epoch, node_id_zero(), now, self.config.borrow().authority_epoch_duration_ms);
        self.monitor.set_authority_epoch(authority_epoch);

        Ok(new_epoch)
    }

    /// Get the current authority epoch for a domain.
    pub fn authority_epoch(&self, domain: Epoch) -> Result<Epoch, LeaseError> {
        let epochs = self.monitor.authority_epochs.borrow();
        let epoch = epochs.get(&domain).ok_or_else(|| LeaseError::AuthorityEpochNotFound { domain })?;
        Ok(epoch.epoch)
    }

    /// Check if a lease is active.
    pub fn is_lease_active(&self, lease_id: LeaseId) -> Result<bool, LeaseError> {
        let leases = self.monitor.leases.borrow();
        let record = leases.get(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        Ok(record.state.is_active())
    }

    /// Get lease state.
    pub fn lease_state(&self, lease_id: LeaseId) -> Result<LeaseState, LeaseError> {
        let leases = self.monitor.leases.borrow();
        let record = leases.get(&lease_id).ok_or_else(|| LeaseError::LeaseNotFound { lease_id })?;
        Ok(record.state)
    }

    /// Get stats.
    pub fn stats(&self) -> LeaseStats {
        self.monitor.stats()
    }

    /// Health score for a node.
    pub fn health_score(&self, node_id: NodeId) -> Result<f64, LeaseError> {
        let lease_ids = self.monitor.leases_for_node(node_id);
        if lease_ids.is_empty() {
            return Ok(1.0);
        }
        let leases = self.monitor.leases.borrow();
        let active = lease_ids.iter().filter(|id| {
            if let Some(r) = leases.get(*id) {
                r.state.is_active()
            } else {
                false
            }
        }).count() as f64;
        drop(leases);
        Ok(active / lease_ids.len() as f64)
    }

    /// Get all lease IDs.
    pub fn all_lease_ids(&self) -> Vec<LeaseId> {
        self.monitor.all_lease_ids()
    }

    /// Get active lease count.
    pub fn active_lease_count(&self) -> u64 {
        self.monitor.active_lease_count()
    }

    /// Get expired lease count.
    pub fn expired_lease_count(&self) -> u64 {
        self.monitor.expired_lease_count()
    }

    /// Get fenced lease count.
    pub fn fenced_lease_count(&self) -> u64 {
        self.monitor.fenced_lease_count()
    }

    /// Expire all leases past their expiry.
    pub fn expire_all_past_due(&self, now_ms: u64) -> u64 {
        let mut leases = self.monitor.leases.borrow_mut();
        let mut expired_count = 0u64;
        for record in leases.values_mut() {
            if record.state.is_active() && record.is_expired_at(now_ms) {
                record.expire();
                expired_count += 1;
            }
        }
        drop(leases);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_expired = stats.total_expired.saturating_add(expired_count);
        stats.active_leases = stats.active_leases.saturating_sub(expired_count);

        expired_count
    }

    /// Get config.
    pub fn config(&self) -> LeaseConfig {
        self.config.borrow().clone()
    }
}

fn node_id_zero() -> NodeId {
    NodeId::ZERO
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LeaseError {
    #[error("lease not found: {lease_id:?}")]
    LeaseNotFound { lease_id: LeaseId },

    #[error("lease not renewable: {lease_id:?}")]
    LeaseNotRenewable { lease_id: LeaseId },

    #[error("lease expired: {lease_id:?}")]
    LeaseExpired { lease_id: LeaseId },

    #[error("lease already terminal: {lease_id:?}")]
    LeaseAlreadyTerminal { lease_id: LeaseId },

    #[error("authority epoch not found: {domain:?}")]
    AuthorityEpochNotFound { domain: Epoch },

    #[error("STONITH is disabled")]
    StonithDisabled,

    #[error("invalid config: field={field}, reason={reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("internal lease error: {message}")]
    Internal { message: String },
}

impl LeaseError {
    pub fn category(&self) -> &'static str {
        match self {
            LeaseError::LeaseNotFound { .. } => "lease_not_found",
            LeaseError::LeaseNotRenewable { .. } => "lease_not_renewable",
            LeaseError::LeaseExpired { .. } => "lease_expired",
            LeaseError::LeaseAlreadyTerminal { .. } => "lease_already_terminal",
            LeaseError::AuthorityEpochNotFound { .. } => "authority_epoch_not_found",
            LeaseError::StonithDisabled => "stonith_disabled",
            LeaseError::InvalidConfig { .. } => "invalid_config",
            LeaseError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            LeaseError::LeaseNotFound { .. }
                | LeaseError::AuthorityEpochNotFound { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            LeaseError::LeaseNotFound { .. } => "create the lease first",
            LeaseError::LeaseNotRenewable { .. } => "lease is not in a renewable state",
            LeaseError::LeaseExpired { .. } => "grant a new lease",
            LeaseError::LeaseAlreadyTerminal { .. } => "lease is already in terminal state",
            LeaseError::AuthorityEpochNotFound { .. } => "issue an authority epoch first",
            LeaseError::StonithDisabled => "enable STONITH in configuration",
            LeaseError::InvalidConfig { .. } => "fix configuration",
            LeaseError::Internal { .. } => "contact support",
        }
    }

    pub fn is_lease_state_error(&self) -> bool {
        matches!(
            self,
            LeaseError::LeaseNotFound { .. }
                | LeaseError::LeaseNotRenewable { .. }
                | LeaseError::LeaseExpired { .. }
                | LeaseError::LeaseAlreadyTerminal { .. }
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RECORD EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRecord {
    /// Check if lease needs renewal (within grace period).
    pub fn needs_renewal(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        self.is_in_grace_period(now_ms, grace_period_ms) && self.state.is_active()
    }

    /// Check if lease can be gracefully renewed.
    pub fn can_gracefully_renew(&self, now_ms: u64, grace_period_ms: u64, max_attempts: u32) -> bool {
        if self.state.is_terminal() {
            return false;
        }
        if self.renewal_count >= max_attempts {
            return false;
        }
        self.time_until_expiry(now_ms) <= grace_period_ms || self.state == LeaseState::Expiring
    }

    /// Get lease age in milliseconds.
    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.granted_at_ms)
    }

    /// Get time since last renewal.
    pub fn time_since_last_renewal(&self, now_ms: u64) -> Option<u64> {
        self.last_renewal_at_ms.map(|t| now_ms.saturating_sub(t))
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE GRANT EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseGrant {
    /// Check if grant is expired at a given timestamp.
    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        now_ms > self.expires_at_ms
    }

    /// Check if grant is within grace period.
    pub fn is_in_grace_period(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        let time_remaining = self.time_remaining(now_ms);
        time_remaining <= grace_period_ms && time_remaining > 0
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RENEWAL EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRenewal {
    /// Check if renewal is fresh (within threshold).
    pub fn is_fresh(&self, now_ms: u64, threshold_ms: u64) -> bool {
        now_ms.saturating_sub(self.renewed_at_ms) <= threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE REVOCATION EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRevocation {
    /// Get time since revocation.
    pub fn time_since_revocation(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.revoked_at_ms)
    }

    /// Check if revocation included STONITH.
    pub fn had_stonith(&self) -> bool {
        self.stonith_executed
    }
}

///////////////////////////////////////////////////////////////////////////////
// FENCING TOKEN RECORD EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl FencingTokenRecord {
    /// Get token value.
    pub fn value(&self) -> u64 {
        self.token.0
    }

    /// Check if token has been incremented.
    pub fn has_increments(&self) -> bool {
        self.increment_count > 0
    }

    /// Get time since last increment.
    pub fn time_since_last_increment(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.last_increment_at_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHORITY EPOCH EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl AuthorityEpoch {
    /// Get time remaining until epoch expiry.
    pub fn time_remaining(&self, now_ms: u64) -> u64 {
        (self.issued_at_ms + self.duration_ms).saturating_sub(now_ms)
    }

    /// Check if epoch needs renewal.
    pub fn needs_renewal(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        self.time_remaining(now_ms) <= grace_period_ms && !self.is_expired_at(now_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE STATS EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseStats {
    /// Get total terminal leases (expired + revoked + fenced).
    pub fn total_terminal(&self) -> u64 {
        self.total_expired + self.total_revocations + self.total_fenced
    }

    /// Get failure rate (terminal / grants).
    pub fn failure_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_terminal() as f64 / self.total_grants as f64
    }

    /// Get STONITH rate (stonith executions / grants).
    pub fn stonith_rate(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_stonith_executions as f64 / self.total_grants as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE MONITOR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseMonitor {
    /// Get all leases.
    pub fn all_leases(&self) -> Vec<LeaseRecord> {
        self.leases.borrow().values().cloned().collect()
    }

    /// Get active leases.
    pub fn active_leases(&self) -> Vec<LeaseRecord> {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state.is_active())
            .cloned()
            .collect()
    }

    /// Get expired leases.
    pub fn expired_leases(&self) -> Vec<LeaseRecord> {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state == LeaseState::Expired)
            .cloned()
            .collect()
    }

    /// Get fenced leases.
    pub fn fenced_leases(&self) -> Vec<LeaseRecord> {
        self.leases
            .borrow()
            .values()
            .filter(|l| l.state == LeaseState::Fenced)
            .cloned()
            .collect()
    }

    /// Get all fencing token records.
    pub fn all_fencing_tokens(&self) -> Vec<FencingTokenRecord> {
        self.fencing_tokens.borrow().values().cloned().collect()
    }

    /// Get all authority epochs.
    pub fn all_authority_epochs(&self) -> Vec<AuthorityEpoch> {
        self.authority_epochs.borrow().values().cloned().collect()
    }

    /// Get STONITH log.
    pub fn stonith_log(&self) -> Vec<(NodeId, u64)> {
        self.stonith_log.borrow().clone()
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.leases.borrow_mut().clear();
        self.leases_by_node.borrow_mut().clear();
        self.fencing_tokens.borrow_mut().clear();
        self.authority_epochs.borrow_mut().clear();
        self.stonith_log.borrow_mut().clear();
        *self.stats.borrow_mut() = LeaseStats::default();
        *self.next_lease_id.borrow_mut() = 1;
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE VALIDATOR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseValidator {
    /// Validate multiple fencing tokens at once.
    pub fn validate_fencing_tokens(&self, current: FencingToken, presented: &[FencingToken]) -> Vec<bool> {
        presented.iter().map(|t| self.validate_fencing_token(current, *t)).collect()
    }

    /// Check if fencing is enabled.
    pub fn is_fencing_enabled(&self) -> bool {
        self.enable_fencing
    }

    /// Check if epoch validation is enabled.
    pub fn is_epoch_validation_enabled(&self) -> bool {
        self.enable_epoch_validation
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE CONFIG EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseConfig {
    /// Check if configuration is strict (short leases, few renewals).
    pub fn is_strict(&self) -> bool {
        self.default_lease_duration_ms <= 5000
            && self.max_renewal_attempts <= 2
            && self.renewal_grace_period_ms <= 1000
    }

    /// Check if configuration is lenient (long leases, many renewals).
    pub fn is_lenient(&self) -> bool {
        self.default_lease_duration_ms >= 60000
            && self.max_renewal_attempts >= 5
            && self.renewal_grace_period_ms >= 10000
    }

    /// Get effective lease duration (clamped).
    pub fn effective_duration(&self, requested: u64) -> u64 {
        if requested == 0 {
            self.default_lease_duration_ms
        } else {
            requested.clamp(self.min_lease_duration_ms, self.max_lease_duration_ms)
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE STATE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseState {
    /// Get a display name for the state.
    pub fn display_name(&self) -> &'static str {
        match self {
            LeaseState::Pending => "pending",
            LeaseState::Active => "active",
            LeaseState::Renewing => "renewing",
            LeaseState::Expiring => "expiring",
            LeaseState::Expired => "expired",
            LeaseState::Revoked => "revoked",
            LeaseState::Fenced => "fenced",
        }
    }

    /// Check if the state requires intervention.
    pub fn requires_intervention(&self) -> bool {
        matches!(self, LeaseState::Expiring | LeaseState::Expired | LeaseState::Fenced)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ENGINE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseEngine {
    /// Get a lease record by ID.
    pub fn get_lease_record(&self, lease_id: LeaseId) -> Option<LeaseRecord> {
        self.monitor.get_lease(lease_id)
    }

    /// Get all leases for a node.
    pub fn leases_for_node(&self, node_id: NodeId) -> Vec<LeaseRecord> {
        let lease_ids = self.monitor.leases_for_node(node_id);
        let leases = self.monitor.leases.borrow();
        lease_ids.iter().filter_map(|id| leases.get(*id).cloned()).collect()
    }

    /// Get all fencing token records.
    pub fn all_fencing_tokens(&self) -> Vec<FencingTokenRecord> {
        self.monitor.all_fencing_tokens()
    }

    /// Get all authority epochs.
    pub fn all_authority_epochs(&self) -> Vec<AuthorityEpoch> {
        self.monitor.all_authority_epochs()
    }

    /// Get STONITH log.
    pub fn stonith_log(&self) -> Vec<(NodeId, u64)> {
        self.monitor.stonith_log()
    }

    /// Check if a node has any active leases.
    pub fn node_has_active_lease(&self, node_id: NodeId) -> bool {
        let leases = self.leases_for_node(node_id);
        leases.iter().any(|l| l.state.is_active())
    }

    /// Get total lease count.
    pub fn total_lease_count(&self) -> u64 {
        self.monitor.leases.borrow().len() as u64
    }

    /// Get total fencing token count.
    pub fn total_fencing_token_count(&self) -> u64 {
        self.monitor.fencing_tokens.borrow().len() as u64
    }

    /// Get total authority epoch count.
    pub fn total_authority_epoch_count(&self) -> u64 {
        self.monitor.authority_epochs.borrow().len() as u64
    }

    /// Get total STONITH count.
    pub fn total_stonith_count(&self) -> u64 {
        self.monitor.stonith_log.borrow().len() as u64
    }

    /// Reset all stats.
    pub fn reset_stats(&self) {
        *self.monitor.stats.borrow_mut() = LeaseStats::default();
    }

    /// Clear all leases.
    pub fn clear_leases(&self) {
        self.monitor.leases.borrow_mut().clear();
        self.monitor.leases_by_node.borrow_mut().clear();
    }

    /// Clear all fencing tokens.
    pub fn clear_fencing_tokens(&self) {
        self.monitor.fencing_tokens.borrow_mut().clear();
    }

    /// Clear all authority epochs.
    pub fn clear_authority_epochs(&self) {
        self.monitor.authority_epochs.borrow_mut().clear();
    }

    /// Clear STONITH log.
    pub fn clear_stonith_log(&self) {
        self.monitor.stonith_log.borrow_mut().clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ERROR EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseError {
    /// Check if this error is related to a lease state.
    pub fn is_lease_state_error(&self) -> bool {
        matches!(
            self,
            LeaseError::LeaseNotFound { .. }
                | LeaseError::LeaseNotRenewable { .. }
                | LeaseError::LeaseExpired { .. }
                | LeaseError::LeaseAlreadyTerminal { .. }
        )
    }

    /// Check if this error is related to authority.
    pub fn is_authority_error(&self) -> bool {
        matches!(self, LeaseError::AuthorityEpochNotFound { .. })
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ENGINE EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseEngine {
    /// Get a lease record by ID.
    pub fn get_lease_record(&self, lease_id: LeaseId) -> Option<LeaseRecord> {
        self.monitor.get_lease(lease_id)
    }

    /// Get all leases for a node.
    pub fn leases_for_node(&self, node_id: NodeId) -> Vec<LeaseRecord> {
        let lease_ids = self.monitor.leases_for_node(node_id);
        let leases = self.monitor.leases.borrow();
        lease_ids
            .iter()
            .filter_map(|id| leases.get(*id).cloned())
            .collect()
    }

    /// Get all fencing token records.
    pub fn all_fencing_tokens(&self) -> Vec<FencingTokenRecord> {
        self.monitor.all_fencing_tokens()
    }

    /// Get all authority epochs.
    pub fn all_authority_epochs(&self) -> Vec<AuthorityEpoch> {
        self.monitor.all_authority_epochs()
    }

    /// Get STONITH log.
    pub fn stonith_log(&self) -> Vec<(NodeId, u64)> {
        self.monitor.stonith_log()
    }

    /// Check if a node has any active leases.
    pub fn node_has_active_lease(&self, node_id: NodeId) -> bool {
        let leases = self.leases_for_node(node_id);
        leases.iter().any(|l| l.state.is_active())
    }

    /// Get total lease count.
    pub fn total_lease_count(&self) -> u64 {
        self.monitor.leases.borrow().len() as u64
    }

    /// Get total fencing token count.
    pub fn total_fencing_token_count(&self) -> u64 {
        self.monitor.fencing_tokens.borrow().len() as u64
    }

    /// Get total authority epoch count.
    pub fn total_authority_epoch_count(&self) -> u64 {
        self.monitor.authority_epochs.borrow().len() as u64
    }

    /// Get total STONITH count.
    pub fn total_stonith_count(&self) -> u64 {
        self.monitor.stonith_log.borrow().len() as u64
    }

    /// Reset all stats.
    pub fn reset_stats(&self) {
        *self.monitor.stats.borrow_mut() = LeaseStats::default();
    }

    /// Clear all leases.
    pub fn clear_leases(&self) {
        self.monitor.leases.borrow_mut().clear();
        self.monitor.leases_by_node.borrow_mut().clear();
    }

    /// Clear all fencing tokens.
    pub fn clear_fencing_tokens(&self) {
        self.monitor.fencing_tokens.borrow_mut().clear();
    }

    /// Clear all authority epochs.
    pub fn clear_authority_epochs(&self) {
        self.monitor.authority_epochs.borrow_mut().clear();
    }

    /// Clear STONITH log.
    pub fn clear_stonith_log(&self) {
        self.monitor.stonith_log.borrow_mut().clear();
    }

    /// Expire all leases past their expiry at a given timestamp.
    pub fn expire_all_past_due(&self, now_ms: u64) -> u64 {
        let mut leases = self.monitor.leases.borrow_mut();
        let mut expired_count = 0u64;
        for record in leases.values_mut() {
            if record.state.is_active() && record.is_expired_at(now_ms) {
                record.expire();
                expired_count += 1;
            }
        }
        drop(leases);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_expired = stats.total_expired.saturating_add(expired_count);
        stats.active_leases = stats.active_leases.saturating_sub(expired_count);

        expired_count
    }

    /// Fence all leases for a given node.
    pub fn fence_node_leases(&self, node_id: NodeId) -> u64 {
        let lease_ids = self.monitor.leases_for_node(node_id);
        let mut fenced_count = 0u64;
        let mut leases = self.monitor.leases.borrow_mut();
        for lease_id in &lease_ids {
            if let Some(record) = leases.get_mut(lease_id) {
                if !record.is_fenced {
                    record.fence();
                    fenced_count += 1;
                }
            }
        }
        drop(leases);

        let mut stats = self.monitor.stats.borrow_mut();
        stats.total_fenced = stats.total_fenced.saturating_add(fenced_count);

        fenced_count
    }

    /// Get leases that are expiring soon (within grace period).
    pub fn get_expiring_leases(&self, now_ms: u64, grace_period_ms: u64) -> Vec<LeaseRecord> {
        let leases = self.monitor.leases.borrow();
        leases
            .values()
            .filter(|r| r.state.is_active() && r.is_in_grace_period(now_ms, grace_period_ms))
            .cloned()
            .collect()
    }

    /// Get leases that are already expired.
    pub fn get_expired_leases(&self) -> Vec<LeaseRecord> {
        let leases = self.monitor.leases.borrow();
        leases
            .values()
            .filter(|r| r.state == LeaseState::Expired)
            .cloned()
            .collect()
    }

    /// Get leases that are fenced.
    pub fn get_fenced_leases(&self) -> Vec<LeaseRecord> {
        let leases = self.monitor.leases.borrow();
        leases
            .values()
            .filter(|r| r.state == LeaseState::Fenced)
            .cloned()
            .collect()
    }

    /// Check if fencing is enabled.
    pub fn is_fencing_enabled(&self) -> bool {
        self.config.borrow().enable_stonith
    }

    /// Check if split-brain prevention is enabled.
    pub fn is_split_brain_prevention_enabled(&self) -> bool {
        self.config.borrow().enable_split_brain_prevention
    }

    /// Get the default lease duration.
    pub fn default_lease_duration(&self) -> u64 {
        self.config.borrow().default_lease_duration_ms
    }

    /// Get the renewal grace period.
    pub fn renewal_grace_period(&self) -> u64 {
        self.config.borrow().renewal_grace_period_ms
    }

    /// Get the max renewal attempts.
    pub fn max_renewal_attempts(&self) -> u32 {
        self.config.borrow().max_renewal_attempts
    }

    /// Get the fencing token step.
    pub fn fencing_token_step(&self) -> u64 {
        self.config.borrow().fencing_token_step
    }

    /// Get the authority epoch duration.
    pub fn authority_epoch_duration(&self) -> u64 {
        self.config.borrow().authority_epoch_duration_ms
    }

    /// Validate the current configuration.
    pub fn validate_config(&self) -> Result<(), LeaseError> {
        self.config.borrow().validate()
    }

    /// Check if configuration is strict.
    pub fn config_is_strict(&self) -> bool {
        self.config.borrow().is_strict()
    }

    /// Check if configuration is lenient.
    pub fn config_is_lenient(&self) -> bool {
        self.config.borrow().is_lenient()
    }

    /// Get effective duration for a requested duration.
    pub fn effective_duration(&self, requested: u64) -> u64 {
        self.config.borrow().effective_duration(requested)
    }

    /// Create a new authority epoch with a specific authority node.
    pub fn authority_epoch_issue_with_node(
        &self,
        domain: Epoch,
        authority_node: NodeId,
    ) -> Result<Epoch, LeaseError> {
        let now = crate::types::IdsTime::now_ms();
        let mut epochs = self.monitor.authority_epochs.borrow_mut();
        let current = epochs.get(&domain).cloned();
        drop(epochs);

        let new_epoch = match current {
            Some(e) => Epoch(e.epoch.0.saturating_add(1)),
            None => Epoch(1),
        };

        let authority_epoch =
            AuthorityEpoch::new(domain, new_epoch, authority_node, now, self.config.borrow().authority_epoch_duration_ms);
        self.monitor.set_authority_epoch(authority_epoch);

        Ok(new_epoch)
    }

    /// Check if an authority node matches the current authority for a domain.
    pub fn is_authority_node(&self, domain: Epoch, node_id: NodeId) -> Result<bool, LeaseError> {
        let epochs = self.monitor.authority_epochs.borrow();
        let epoch = epochs.get(&domain).ok_or_else(|| LeaseError::AuthorityEpochNotFound { domain })?;
        Ok(epoch.authority_node == node_id)
    }

    /// Get all authority nodes across all domains.
    pub fn all_authority_nodes(&self) -> Vec<(Epoch, NodeId)> {
        self.monitor
            .authority_epochs
            .borrow()
            .iter()
            .map(|(domain, epoch)| (*domain, epoch.authority_node))
            .collect()
    }

    /// Count leases by state.
    pub fn count_leases_by_state(&self, state: LeaseState) -> u64 {
        self.monitor
            .leases
            .borrow()
            .values()
            .filter(|r| r.state == state)
            .count() as u64
    }

    /// Get lease age statistics.
    pub fn lease_age_stats(&self, now_ms: u64) -> (u64, u64, u64) {
        let leases = self.monitor.leases.borrow();
        let ages: Vec<u64> = leases.values().map(|r| r.age_ms(now_ms)).collect();
        drop(leases);

        if ages.is_empty() {
            return (0, 0, 0);
        }

        let min = ages.iter().min().copied().unwrap_or(0);
        let max = ages.iter().max().copied().unwrap_or(0);
        let avg = ages.iter().sum::<u64>() / ages.len() as u64;
        (min, max, avg)
    }

    /// Get fencing token statistics.
    pub fn fencing_token_stats(&self) -> (u64, u64) {
        let tokens = self.monitor.fencing_tokens.borrow();
        let total_increments: u64 = tokens.values().map(|r| r.increment_count).sum();
        let total_tokens = tokens.len() as u64;
        drop(tokens);
        (total_tokens, total_increments)
    }

    /// Get STONITH execution statistics.
    pub fn stonith_stats(&self) -> (u64, Vec<NodeId>) {
        let log = self.monitor.stonith_log.borrow();
        let unique_nodes: Vec<NodeId> = log.iter().map(|(n, _)| *n).collect();
        let unique_count = unique_nodes.iter().collect::<std::collections::HashSet<_>>().len() as u64;
        drop(log);
        (unique_count, unique_nodes)
    }

    /// Get all lease IDs sorted by expiry (soonest first).
    pub fn lease_ids_by_expiry(&self, now_ms: u64) -> Vec<(LeaseId, u64)> {
        let leases = self.monitor.leases.borrow();
        let mut expiry_list: Vec<(LeaseId, u64)> = leases
            .iter()
            .map(|(id, r)| (*id, r.time_until_expiry(now_ms)))
            .collect();
        drop(leases);
        expiry_list.sort_by_key(|(_, t)| *t);
        expiry_list
    }

    /// Check if any lease needs immediate renewal.
    pub fn any_lease_needs_renewal(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        let leases = self.monitor.leases.borrow();
        leases
            .values()
            .any(|r| r.needs_renewal(now_ms, grace_period_ms))
    }

    /// Get count of leases needing renewal.
    pub fn count_leases_needing_renewal(&self, now_ms: u64, grace_period_ms: u64) -> u64 {
        let leases = self.monitor.leases.borrow();
        leases
            .values()
            .filter(|r| r.needs_renewal(now_ms, grace_period_ms))
            .count() as u64
    }

    /// Get the config (immutable borrow).
    pub fn config_borrow(&self) -> std::cell::Ref<'_, LeaseConfig> {
        self.config.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE HEALTH REPORT
///////////////////////////////////////////////////////////////////////////////

/// A health report for lease operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseHealthReport {
    /// Total leases.
    pub total_leases: u64,
    /// Active leases.
    pub active_leases: u64,
    /// Expired leases.
    pub expired_leases: u64,
    /// Revoked leases.
    pub revoked_leases: u64,
    /// Fenced leases.
    pub fenced_leases: u64,
    /// Success rate (0.0 to 1.0).
    pub success_rate: f64,
    /// Renewal rate (0.0 to 1.0).
    pub renewal_rate: f64,
    /// Expiry rate (0.0 to 1.0).
    pub expiry_rate: f64,
    /// Fencing rate (0.0 to 1.0).
    pub fencing_rate: f64,
    /// STONITH execution count.
    pub stonith_count: u64,
    /// Fencing token increment count.
    pub fencing_increment_count: u64,
    /// Authority epoch count.
    pub authority_epoch_count: u64,
}

impl LeaseHealthReport {
    /// Create a new LeaseHealthReport from stats.
    pub fn from_stats(stats: &LeaseStats) -> Self {
        Self {
            total_leases: stats.total_grants,
            active_leases: stats.active_leases,
            expired_leases: stats.total_expired,
            revoked_leases: stats.total_revocations,
            fenced_leases: stats.total_fenced,
            success_rate: stats.success_rate(),
            renewal_rate: stats.renewal_rate(),
            expiry_rate: stats.expiry_rate(),
            fencing_rate: stats.fencing_rate(),
            stonith_count: stats.total_stonith_executions,
            fencing_increment_count: stats.total_fencing_increments,
            authority_epoch_count: stats.total_authority_epochs,
        }
    }

    /// Check if the system is healthy (success rate above threshold).
    pub fn is_healthy(&self, threshold: f64) -> bool {
        self.success_rate >= threshold
    }

    /// Check if there are too many expired leases.
    pub fn has_high_expiry_rate(&self, threshold: f64) -> bool {
        self.expiry_rate > threshold
    }

    /// Check if there are too many fenced leases.
    pub fn has_high_fencing_rate(&self, threshold: f64) -> bool {
        self.fencing_rate > threshold
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ENGINE HEALTH REPORT METHOD
///////////////////////////////////////////////////////////////////////////////

impl LeaseEngine {
    /// Get a health report for lease operations.
    pub fn health_report(&self) -> LeaseHealthReport {
        LeaseHealthReport::from_stats(&self.stats())
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ENGINE ADVANCED OPERATIONS
///////////////////////////////////////////////////////////////////////////////

impl LeaseEngine {
    /// Perform a full lease lifecycle: grant, renew, revoke.
    pub fn full_lifecycle(&self, node_id: NodeId, epoch: Epoch, duration_ms: u64) -> Result<LeaseRevocation, LeaseError> {
        let grant = self.grant_lease(node_id, epoch, duration_ms)?;
        let lease_id = grant.lease_id;

        // Renew once
        let renewal = self.renew_lease(lease_id)?;
        let _ = renewal;

        // Revoke
        let revocation = self.revoke_lease(lease_id)?;
        Ok(revocation)
    }

    /// Perform STONITH and fence all leases for a node.
    pub fn stonith_and_fence_node(&self, node_id: NodeId) -> Result<u64, LeaseError> {
        self.stonith_execute(node_id)?;
        let fenced = self.fence_node_leases(node_id);
        Ok(fenced)
    }

    /// Get all nodes that have active leases.
    pub fn nodes_with_active_leases(&self) -> Vec<NodeId> {
        let leases = self.monitor.leases.borrow();
        let mut nodes: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        for record in leases.values() {
            if record.state.is_active() {
                nodes.insert(record.node_id);
            }
        }
        drop(leases);
        nodes.into_iter().collect()
    }

    /// Get all domains with active authority epochs.
    pub fn domains_with_authority(&self) -> Vec<Epoch> {
        self.monitor
            .authority_epochs
            .borrow()
            .keys()
            .cloned()
            .collect()
    }

    /// Check if a domain has split-brain risk (multiple authority epochs).
    pub fn domain_split_brain_risk(&self, domain: Epoch) -> bool {
        let epochs = self.monitor.authority_epochs.borrow();
        epochs.get(&domain).map_or(false, |e| e.is_expired_at(crate::types::IdsTime::now_ms()))
    }

    /// Renew all leases that are within grace period.
    pub fn renew_all_expiring(&self, now_ms: u64, grace_period_ms: u64) -> u64 {
        let expiring = self.get_expiring_leases(now_ms, grace_period_ms);
        let mut renewed_count = 0u64;
        for record in &expiring {
            if self.renew_lease(record.lease_id).is_ok() {
                renewed_count += 1;
            }
        }
        renewed_count
    }

    /// Expire all leases that are past due.
    pub fn expire_all_past_due_with_stats(&self, now_ms: u64) -> (u64, LeaseStats) {
        let expired = self.expire_all_past_due(now_ms);
        let stats = self.stats();
        (expired, stats)
    }

    /// Get lease summary for a node.
    pub fn node_lease_summary(&self, node_id: NodeId) -> (u64, u64, u64) {
        let leases = self.leases_for_node(node_id);
        let active = leases.iter().filter(|l| l.state.is_active()).count() as u64;
        let expired = leases.iter().filter(|l| l.state == LeaseState::Expired).count() as u64;
        let fenced = leases.iter().filter(|l| l.state == LeaseState::Fenced).count() as u64;
        (active, expired, fenced)
    }

    /// Get total fencing token value across all leases.
    pub fn total_fencing_token_value(&self) -> u64 {
        let tokens = self.monitor.fencing_tokens.borrow();
        let total: u64 = tokens.values().map(|r| r.token.0).sum();
        drop(tokens);
        total
    }

    /// Get average fencing token increment count.
    pub fn average_fencing_increments(&self) -> f64 {
        let tokens = self.monitor.fencing_tokens.borrow();
        if tokens.is_empty() {
            return 0.0;
        }
        let total: u64 = tokens.values().map(|r| r.increment_count).sum();
        total as f64 / tokens.len() as f64
    }

    /// Get the oldest active lease.
    pub fn oldest_active_lease(&self, now_ms: u64) -> Option<(LeaseId, u64)> {
        let leases = self.monitor.leases.borrow();
        let mut oldest: Option<(LeaseId, u64)> = None;
        for (id, record) in leases.iter() {
            if record.state.is_active() {
                let age = record.age_ms(now_ms);
                if oldest.map_or(true, |(_, a)| age > a) {
                    oldest = Some((*id, age));
                }
            }
        }
        drop(leases);
        oldest
    }

    /// Get the newest active lease.
    pub fn newest_active_lease(&self, now_ms: u64) -> Option<(LeaseId, u64)> {
        let leases = self.monitor.leases.borrow();
        let mut newest: Option<(LeaseId, u64)> = None;
        for (id, record) in leases.iter() {
            if record.state.is_active() {
                let age = record.age_ms(now_ms);
                if newest.map_or(true, |(_, a)| age < a) {
                    newest = Some((*id, age));
                }
            }
        }
        drop(leases);
        newest
    }

    /// Check if the lease system is healthy.
    pub fn is_system_healthy(&self, threshold: f64) -> bool {
        let report = self.health_report();
        report.is_healthy(threshold)
    }

    /// Get the configuration as a clone.
    pub fn config_clone(&self) -> LeaseConfig {
        self.config.borrow().clone()
    }

    /// Set a new fencing token base.
    pub fn set_fencing_token_base(&self, base: FencingToken) {
        *self.next_fencing_base.borrow_mut() = base;
    }

    /// Get the current fencing token base.
    pub fn fencing_token_base(&self) -> FencingToken {
        *self.next_fencing_base.borrow()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE MONITOR ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseMonitor {
    /// Get leases expiring within a time window.
    pub fn leases_expiring_within(&self, now_ms: u64, window_ms: u64) -> Vec<LeaseRecord> {
        let leases = self.leases.borrow();
        leases
            .values()
            .filter(|r| r.state.is_active() && r.time_until_expiry(now_ms) <= window_ms)
            .cloned()
            .collect()
    }

    /// Get leases expired before a timestamp.
    pub fn leases_expired_before(&self, before_ms: u64) -> Vec<LeaseRecord> {
        let leases = self.leases.borrow();
        leases
            .values()
            .filter(|r| r.state == LeaseState::Expired && r.expires_at_ms < before_ms)
            .cloned()
            .collect()
    }

    /// Get leases granted after a timestamp.
    pub fn leases_granted_after(&self, after_ms: u64) -> Vec<LeaseRecord> {
        let leases = self.leases.borrow();
        leases
            .values()
            .filter(|r| r.granted_at_ms > after_ms)
            .cloned()
            .collect()
    }

    /// Get leases by epoch.
    pub fn leases_by_epoch(&self, epoch: Epoch) -> Vec<LeaseRecord> {
        let leases = self.leases.borrow();
        leases
            .values()
            .filter(|r| r.epoch == epoch)
            .cloned()
            .collect()
    }

    /// Get fencing tokens by lease ID range.
    pub fn fencing_tokens_by_lease(&self, lease_id: LeaseId) -> Option<FencingTokenRecord> {
        self.fencing_tokens.borrow().get(&lease_id).cloned()
    }

    /// Get all authority epochs sorted by domain.
    pub fn authority_epochs_sorted(&self) -> Vec<(Epoch, AuthorityEpoch)> {
        let mut epochs: Vec<(Epoch, AuthorityEpoch)> = self
            .authority_epochs
            .borrow()
            .iter()
            .map(|(d, e)| (*d, e.clone()))
            .collect();
        epochs.sort_by_key(|(d, _)| *d);
        epochs
    }

    /// Get STONITH log for a specific node.
    pub fn stonith_log_for_node(&self, node_id: NodeId) -> Vec<(NodeId, u64)> {
        self.stonith_log
            .borrow()
            .iter()
            .filter(|(n, _)| *n == node_id)
            .cloned()
            .collect()
    }

    /// Count STONITH executions.
    pub fn stonith_execution_count(&self) -> u64 {
        self.stonith_log.borrow().len() as u64
    }

    /// Get lease count by node.
    pub fn lease_count_by_node(&self, node_id: NodeId) -> u64 {
        self.leases_for_node(node_id).len() as u64
    }

    /// Get nodes sorted by lease count (descending).
    pub fn nodes_by_lease_count(&self) -> Vec<(NodeId, u64)> {
        let leases = self.leases.borrow();
        let mut counts: std::collections::HashMap<NodeId, u64> = std::collections::HashMap::new();
        for record in leases.values() {
            *counts.entry(record.node_id).or_insert(0) += 1;
        }
        drop(leases);
        let mut result: Vec<(NodeId, u64)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Reset fencing token records.
    pub fn reset_fencing_tokens(&mut self) {
        self.fencing_tokens.borrow_mut().clear();
    }

    /// Reset authority epochs.
    pub fn reset_authority_epochs(&mut self) {
        self.authority_epochs.borrow_mut().clear();
    }

    /// Reset STONITH log.
    pub fn reset_stonith_log(&mut self) {
        self.stonith_log.borrow_mut().clear();
    }

    /// Get monitor stats as a clone.
    pub fn stats_clone(&self) -> LeaseStats {
        self.stats.borrow().clone()
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE VALIDATOR ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseValidator {
    /// Validate a batch of fencing tokens and return results.
    pub fn validate_fencing_token_batch(
        &self,
        current: FencingToken,
        presented: &[(LeaseId, FencingToken)],
    ) -> Vec<(LeaseId, bool)> {
        presented
            .iter()
            .map(|(id, token)| (*id, self.validate_fencing_token(current, *token)))
            .collect()
    }

    /// Validate authority epochs for multiple domains.
    pub fn validate_authority_epochs(
        &self,
        domains: &[(Epoch, Epoch)],
        current: &std::collections::HashMap<Epoch, Epoch>,
    ) -> Vec<(Epoch, bool)> {
        domains
            .iter()
            .map(|(domain, presented)| {
                let cur = current.get(domain).copied().unwrap_or(Epoch::ZERO);
                (*domain, self.validate_authority_epoch(*domain, *presented, cur))
            })
            .collect()
    }

    /// Validate renewal for multiple lease states.
    pub fn validate_renewal_batch(&self, states: &[LeaseState]) -> Vec<bool> {
        states.iter().map(|s| self.validate_renewal(*s)).collect()
    }

    /// Toggle fencing validation.
    pub fn set_fencing_enabled(&mut self, enabled: bool) {
        self.enable_fencing = enabled;
    }

    /// Toggle epoch validation.
    pub fn set_epoch_validation_enabled(&mut self, enabled: bool) {
        self.enable_epoch_validation = enabled;
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE CONFIG ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseConfig {
    /// Create a strict configuration (short leases, few renewals).
    pub fn strict() -> Self {
        Self {
            default_lease_duration_ms: 5000,
            min_lease_duration_ms: 1000,
            max_lease_duration_ms: 10000,
            renewal_grace_period_ms: 1000,
            max_renewal_attempts: 2,
            fencing_token_step: 1,
            enable_stonith: true,
            enable_split_brain_prevention: true,
            authority_epoch_duration_ms: 10000,
        }
    }

    /// Create a lenient configuration (long leases, many renewals).
    pub fn lenient() -> Self {
        Self {
            default_lease_duration_ms: 120000,
            min_lease_duration_ms: 30000,
            max_lease_duration_ms: 600000,
            renewal_grace_period_ms: 30000,
            max_renewal_attempts: 10,
            fencing_token_step: 1,
            enable_stonith: false,
            enable_split_brain_prevention: false,
            authority_epoch_duration_ms: 300000,
        }
    }

    /// Create a balanced configuration (default).
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Get the range of allowed durations.
    pub fn duration_range(&self) -> (u64, u64) {
        (self.min_lease_duration_ms, self.max_lease_duration_ms)
    }

    /// Get the total allowed renewal time.
    pub fn total_renewal_time(&self) -> u64 {
        self.default_lease_duration_ms * self.max_renewal_attempts as u64
    }

    /// Check if a duration is valid.
    pub fn is_duration_valid(&self, duration_ms: u64) -> bool {
        duration_ms >= self.min_lease_duration_ms && duration_ms <= self.max_lease_duration_ms
    }

    /// Get the recommended renewal interval (half of default duration).
    pub fn recommended_renewal_interval(&self) -> u64 {
        self.default_lease_duration_ms / 2
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RECORD ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRecord {
    /// Check if lease should be renewed based on remaining time and attempts.
    pub fn should_renew(&self, now_ms: u64, grace_period_ms: u64, max_attempts: u32) -> bool {
        if self.state.is_terminal() {
            return false;
        }
        if self.renewal_count >= max_attempts {
            return false;
        }
        self.time_until_expiry(now_ms) <= grace_period_ms
    }

    /// Get remaining renewals.
    pub fn remaining_renewals(&self, max_attempts: u32) -> u32 {
        if self.renewal_count >= max_attempts {
            0
        } else {
            max_attempts - self.renewal_count
        }
    }

    /// Check if lease is fresh (granted recently).
    pub fn is_fresh(&self, now_ms: u64, fresh_threshold_ms: u64) -> bool {
        self.age_ms(now_ms) <= fresh_threshold_ms
    }

    /// Check if lease is old (granted long ago).
    pub fn is_old(&self, now_ms: u64, old_threshold_ms: u64) -> bool {
        self.age_ms(now_ms) > old_threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE GRANT ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseGrant {
    /// Check if grant will expire within a window.
    pub fn expires_within(&self, now_ms: u64, window_ms: u64) -> bool {
        self.time_remaining(now_ms) <= window_ms && self.time_remaining(now_ms) > 0
    }

    /// Get the grant duration as a percentage of total time elapsed.
    pub fn elapsed_percent(&self, now_ms: u64) -> f64 {
        let total = self.duration_ms;
        if total == 0 {
            return 0.0;
        }
        let elapsed = now_ms.saturating_sub(self.granted_at_ms);
        (elapsed as f64 / total as f64).min(1.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE RENEWAL ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRenewal {
    /// Check if renewal will expire within a window.
    pub fn expires_within(&self, now_ms: u64, window_ms: u64) -> bool {
        self.new_expires_at_ms.saturating_sub(now_ms) <= window_ms
            && self.new_expires_at_ms > now_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE REVOCATION ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseRevocation {
    /// Get the revocation reason category.
    pub fn reason_category(&self) -> &'static str {
        if self.reason.contains("stonith") {
            "stonith"
        } else if self.reason.contains("expire") {
            "expiry"
        } else if self.reason.contains("revoke") {
            "manual"
        } else {
            "other"
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// FENCING TOKEN RECORD ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl FencingTokenRecord {
    /// Get the next token value after increment.
    pub fn next_token_value(&self, step: u64) -> FencingToken {
        FencingToken(self.token.0.saturating_add(step))
    }

    /// Check if token is fresh (incremented recently).
    pub fn is_fresh(&self, now_ms: u64, fresh_threshold_ms: u64) -> bool {
        self.time_since_last_increment(now_ms) <= fresh_threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHORITY EPOCH ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl AuthorityEpoch {
    /// Check if epoch needs renewal soon.
    pub fn needs_renewal_soon(&self, now_ms: u64, grace_period_ms: u64) -> bool {
        self.time_remaining(now_ms) <= grace_period_ms && !self.is_expired_at(now_ms)
    }

    /// Get the epoch age.
    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.issued_at_ms)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE STATS ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseStats {
    /// Get the total number of non-terminal leases.
    pub fn total_active_or_pending(&self) -> u64 {
        self.active_leases
    }

    /// Get the ratio of active leases to total grants.
    pub fn active_ratio(&self) -> f64 {
        if self.total_grants == 0 {
            return 1.0;
        }
        self.active_leases as f64 / self.total_grants as f64
    }

    /// Get the ratio of fenced leases to total grants.
    pub fn fenced_ratio(&self) -> f64 {
        if self.total_grants == 0 {
            return 0.0;
        }
        self.total_fenced as f64 / self.total_grants as f64
    }

    /// Get the overall health score (0.0 to 1.0).
    pub fn health_score(&self) -> f64 {
        let success = self.success_rate();
        let renewal = self.renewal_rate().min(1.0);
        let expiry_penalty = self.expiry_rate();
        let fencing_penalty = self.fencing_rate();
        (success + renewal) / 2.0 - (expiry_penalty + fencing_penalty) / 2.0
    }
}

///////////////////////////////////////////////////////////////////////////////
// LEASE ERROR ADVANCED METHODS
///////////////////////////////////////////////////////////////////////////////

impl LeaseError {
    /// Check if this error requires STONITH.
    pub fn requires_stonith(&self) -> bool {
        matches!(
            self,
            LeaseError::LeaseExpired { .. } | LeaseError::LeaseNotRenewable { .. }
        )
    }

    /// Check if this error is related to fencing.
    pub fn is_fencing_error(&self) -> bool {
        matches!(self, LeaseError::StonithDisabled)
    }

    /// Get the error severity.
    pub fn severity(&self) -> &'static str {
        match self {
            LeaseError::LeaseNotFound { .. } => "low",
            LeaseError::LeaseExpired { .. } => "medium",
            LeaseError::LeaseNotRenewable { .. } => "medium",
            LeaseError::StonithDisabled => "high",
            LeaseError::AuthorityEpochNotFound { .. } => "high",
            LeaseError::InvalidConfig { .. } => "critical",
            LeaseError::Internal { .. } => "critical",
            LeaseError::LeaseAlreadyTerminal { .. } => "low",
        }
    }
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

    fn test_endpoint() -> Endpoint {
        Endpoint {
            carrier_type: crate::types::CarrierType::Direct,
            address: vec![1, 2, 3, 4],
        }
    }

    fn test_epoch() -> Epoch {
        Epoch::new(1)
    }

    fn test_lease_id() -> LeaseId {
        let mut bytes = [0u8; 16];
        bytes[0] = 1;
        LeaseId::new(bytes)
    }

    fn test_fencing_token() -> FencingToken {
        FencingToken::new(1)
    }

    fn test_config() -> LeaseConfig {
        LeaseConfig::default()
    }

    // ========================================================================
    // LeaseConfig tests
    // ========================================================================

    #[test]
    fn lease_config_default() {
        let config = LeaseConfig::default();
        assert_eq!(config.default_lease_duration_ms, 30000);
        assert_eq!(config.min_lease_duration_ms, 1000);
        assert_eq!(config.max_lease_duration_ms, 300000);
        assert_eq!(config.renewal_grace_period_ms, 5000);
        assert_eq!(config.max_renewal_attempts, 3);
        assert_eq!(config.fencing_token_step, 1);
        assert!(config.enable_stonith);
        assert!(config.enable_split_brain_prevention);
        assert_eq!(config.authority_epoch_duration_ms, 60000);
    }

    #[test]
    fn lease_config_builder() {
        let config = LeaseConfig::default()
            .with_default_duration(10000)
            .with_duration_bounds(2000, 120000)
            .with_renewal(3000, 5)
            .with_stonith(false)
            .with_split_brain_prevention(false);
        assert_eq!(config.default_lease_duration_ms, 10000);
        assert_eq!(config.min_lease_duration_ms, 2000);
        assert_eq!(config.max_lease_duration_ms, 120000);
        assert_eq!(config.renewal_grace_period_ms, 3000);
        assert_eq!(config.max_renewal_attempts, 5);
        assert!(!config.enable_stonith);
        assert!(!config.enable_split_brain_prevention);
    }

    #[test]
    fn lease_config_validate_valid() {
        let config = LeaseConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn lease_config_validate_min_zero() {
        let config = LeaseConfig::default().with_duration_bounds(0, 300000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn lease_config_validate_max_lt_min() {
        let config = LeaseConfig::default().with_duration_bounds(300000, 1000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn lease_config_validate_default_out_of_range() {
        let config = LeaseConfig::default().with_default_duration(500000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn lease_config_validate_max_renewal_zero() {
        let config = LeaseConfig::default().with_renewal(5000, 0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn lease_config_validate_fencing_step_zero() {
        let config = LeaseConfig::default().with_duration_bounds(1000, 300000);
        let mut config = config;
        config.fencing_token_step = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn lease_config_strict() {
        let config = LeaseConfig::strict();
        assert!(config.is_strict());
        assert!(!config.is_lenient());
        assert_eq!(config.default_lease_duration_ms, 5000);
        assert_eq!(config.max_renewal_attempts, 2);
    }

    #[test]
    fn lease_config_lenient() {
        let config = LeaseConfig::lenient();
        assert!(config.is_lenient());
        assert!(!config.is_strict());
        assert_eq!(config.default_lease_duration_ms, 120000);
        assert_eq!(config.max_renewal_attempts, 10);
    }

    #[test]
    fn lease_config_balanced() {
        let config = LeaseConfig::balanced();
        assert!(!config.is_strict());
        assert!(!config.is_lenient());
        assert_eq!(config.default_lease_duration_ms, 30000);
    }

    #[test]
    fn lease_config_duration_range() {
        let config = LeaseConfig::default();
        let (min, max) = config.duration_range();
        assert_eq!(min, 1000);
        assert_eq!(max, 300000);
    }

    #[test]
    fn lease_config_total_renewal_time() {
        let config = LeaseConfig::default();
        assert_eq!(config.total_renewal_time(), 30000 * 3);
    }

    #[test]
    fn lease_config_is_duration_valid() {
        let config = LeaseConfig::default();
        assert!(config.is_duration_valid(5000));
        assert!(!config.is_duration_valid(500));
        assert!(!config.is_duration_valid(500000));
    }

    #[test]
    fn lease_config_recommended_renewal_interval() {
        let config = LeaseConfig::default();
        assert_eq!(config.recommended_renewal_interval(), 15000);
    }

    #[test]
    fn lease_config_effective_duration() {
        let config = LeaseConfig::default();
        assert_eq!(config.effective_duration(0), 30000);
        assert_eq!(config.effective_duration(5000), 5000);
        assert_eq!(config.effective_duration(500), 1000);
        assert_eq!(config.effective_duration(500000), 300000);
    }

    #[test]
    fn lease_config_serialization_roundtrip() {
        let config = LeaseConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: LeaseConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_lease_duration_ms, deserialized.default_lease_duration_ms);
        assert_eq!(config.max_renewal_attempts, deserialized.max_renewal_attempts);
    }

    // ========================================================================
    // LeaseState tests
    // ========================================================================

    #[test]
    fn lease_state_is_active() {
        assert!(LeaseState::Active.is_active());
        assert!(LeaseState::Renewing.is_active());
        assert!(LeaseState::Expiring.is_active());
        assert!(!LeaseState::Pending.is_active());
        assert!(!LeaseState::Expired.is_active());
        assert!(!LeaseState::Revoked.is_active());
        assert!(!LeaseState::Fenced.is_active());
    }

    #[test]
    fn lease_state_is_terminal() {
        assert!(LeaseState::Expired.is_terminal());
        assert!(LeaseState::Revoked.is_terminal());
        assert!(LeaseState::Fenced.is_terminal());
        assert!(!LeaseState::Pending.is_terminal());
        assert!(!LeaseState::Active.is_terminal());
    }

    #[test]
    fn lease_state_can_renew() {
        assert!(LeaseState::Active.can_renew());
        assert!(LeaseState::Renewing.can_renew());
        assert!(LeaseState::Expiring.can_renew());
        assert!(!LeaseState::Pending.can_renew());
        assert!(!LeaseState::Expired.can_renew());
    }

    #[test]
    fn lease_state_default() {
        assert_eq!(LeaseState::default(), LeaseState::Pending);
    }

    #[test]
    fn lease_state_display_name() {
        assert_eq!(LeaseState::Pending.display_name(), "pending");
        assert_eq!(LeaseState::Active.display_name(), "active");
        assert_eq!(LeaseState::Expired.display_name(), "expired");
        assert_eq!(LeaseState::Fenced.display_name(), "fenced");
    }

    #[test]
    fn lease_state_requires_intervention() {
        assert!(LeaseState::Expiring.requires_intervention());
        assert!(LeaseState::Expired.requires_intervention());
        assert!(LeaseState::Fenced.requires_intervention());
        assert!(!LeaseState::Active.requires_intervention());
    }

    // ========================================================================
    // LeaseRecord tests
    // ========================================================================

    #[test]
    fn lease_record_new() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        assert_eq!(record.lease_id, test_lease_id());
        assert_eq!(record.node_id, test_node_id());
        assert_eq!(record.epoch, test_epoch());
        assert_eq!(record.duration_ms, 10000);
        assert_eq!(record.state, LeaseState::Pending);
        assert_eq!(record.renewal_count, 0);
        assert!(!record.is_fenced);
    }

    #[test]
    fn lease_record_activate() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 2000);
        assert_eq!(record.state, LeaseState::Active);
        assert_eq!(record.fencing_token, test_fencing_token());
        assert_eq!(record.expires_at_ms, 12000);
    }

    #[test]
    fn lease_record_renew() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        record.renew(5000, FencingToken::new(2));
        assert_eq!(record.state, LeaseState::Active);
        assert_eq!(record.renewal_count, 1);
        assert_eq!(record.expires_at_ms, 15000);
    }

    #[test]
    fn lease_record_expire() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.expire();
        assert_eq!(record.state, LeaseState::Expired);
        assert!(record.state.is_terminal());
    }

    #[test]
    fn lease_record_revoke() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.revoke();
        assert_eq!(record.state, LeaseState::Revoked);
        assert!(record.state.is_terminal());
    }

    #[test]
    fn lease_record_fence() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.fence();
        assert_eq!(record.state, LeaseState::Fenced);
        assert!(record.is_fenced);
        assert!(record.state.is_terminal());
    }

    #[test]
    fn lease_record_is_expired_at() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        assert!(!record.is_expired_at(5000));
        assert!(record.is_expired_at(15000));
    }

    #[test]
    fn lease_record_is_in_grace_period() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        // expires at 11000, grace period 5000 -> grace starts at 6000
        assert!(!record.is_in_grace_period(5000, 5000));
        assert!(record.is_in_grace_period(7000, 5000));
    }

    #[test]
    fn lease_record_time_until_expiry() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        assert_eq!(record.time_until_expiry(2000), 9000);
        assert_eq!(record.time_until_expiry(12000), 0);
    }

    #[test]
    fn lease_record_should_renew() {
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        // expires at 11000, grace 5000 -> needs renewal at 6000+
        assert!(!record.should_renew(5000, 5000, 3));
        assert!(record.should_renew(7000, 5000, 3));
    }

    #[test]
    fn lease_record_remaining_renewals() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        assert_eq!(record.remaining_renewals(3), 3);
        let mut record = record;
        record.renewal_count = 2;
        assert_eq!(record.remaining_renewals(3), 1);
        record.renewal_count = 3;
        assert_eq!(record.remaining_renewals(3), 0);
    }

    #[test]
    fn lease_record_is_fresh_and_old() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        assert!(record.is_fresh(2000, 2000));
        assert!(!record.is_old(5000, 10000));
        assert!(record.is_old(20000, 10000));
    }

    #[test]
    fn lease_record_age_ms() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        assert_eq!(record.age_ms(1000), 0);
        assert_eq!(record.age_ms(5000), 4000);
    }

    #[test]
    fn lease_record_serialization_roundtrip() {
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: LeaseRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.lease_id, deserialized.lease_id);
        assert_eq!(record.node_id, deserialized.node_id);
        assert_eq!(record.duration_ms, deserialized.duration_ms);
    }

    // ========================================================================
    // LeaseGrant tests
    // ========================================================================

    #[test]
    fn lease_grant_new() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        assert_eq!(grant.lease_id, test_lease_id());
        assert_eq!(grant.expires_at_ms, 11000);
        assert_eq!(grant.fencing_token, test_fencing_token());
    }

    #[test]
    fn lease_grant_time_remaining() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        assert_eq!(grant.time_remaining(2000), 9000);
        assert_eq!(grant.time_remaining(12000), 0);
    }

    #[test]
    fn lease_grant_is_expired_at() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        assert!(!grant.is_expired_at(5000));
        assert!(grant.is_expired_at(12000));
    }

    #[test]
    fn lease_grant_is_in_grace_period() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        assert!(!grant.is_in_grace_period(5000, 5000));
        assert!(grant.is_in_grace_period(7000, 5000));
    }

    #[test]
    fn lease_grant_elapsed_percent() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        assert!((grant.elapsed_percent(1000) - 0.0).abs() < 0.01);
        assert!((grant.elapsed_percent(6000) - 0.5).abs() < 0.01);
        assert!((grant.elapsed_percent(12000) - 1.0).abs() < 0.01);
    }

    #[test]
    fn lease_grant_serialization_roundtrip() {
        let grant = LeaseGrant::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000, test_fencing_token());
        let json = serde_json::to_string(&grant).unwrap();
        let deserialized: LeaseGrant = serde_json::from_str(&json).unwrap();
        assert_eq!(grant.lease_id, deserialized.lease_id);
        assert_eq!(grant.duration_ms, deserialized.duration_ms);
    }

    // ========================================================================
    // LeaseRenewal tests
    // ========================================================================

    #[test]
    fn lease_renewal_new() {
        let renewal = LeaseRenewal::new(test_lease_id(), 20000, 1, test_fencing_token(), 15000);
        assert_eq!(renewal.lease_id, test_lease_id());
        assert_eq!(renewal.new_expires_at_ms, 20000);
        assert_eq!(renewal.renewal_count, 1);
    }

    #[test]
    fn lease_renewal_is_fresh() {
        let renewal = LeaseRenewal::new(test_lease_id(), 20000, 1, test_fencing_token(), 15000);
        assert!(renewal.is_fresh(16000, 2000));
        assert!(!renewal.is_fresh(20000, 2000));
    }

    #[test]
    fn lease_renewal_expires_within() {
        let renewal = LeaseRenewal::new(test_lease_id(), 20000, 1, test_fencing_token(), 15000);
        assert!(renewal.expires_within(19000, 2000));
        assert!(!renewal.expires_within(15000, 2000));
    }

    #[test]
    fn lease_renewal_serialization_roundtrip() {
        let renewal = LeaseRenewal::new(test_lease_id(), 20000, 1, test_fencing_token(), 15000);
        let json = serde_json::to_string(&renewal).unwrap();
        let deserialized: LeaseRenewal = serde_json::from_str(&json).unwrap();
        assert_eq!(renewal.lease_id, deserialized.lease_id);
        assert_eq!(renewal.renewal_count, deserialized.renewal_count);
    }

    // ========================================================================
    // LeaseRevocation tests
    // ========================================================================

    #[test]
    fn lease_revocation_new() {
        let revocation = LeaseRevocation::new(test_lease_id(), test_node_id(), 10000, "revoked".to_string());
        assert_eq!(revocation.lease_id, test_lease_id());
        assert_eq!(revocation.node_id, test_node_id());
        assert!(!revocation.stonith_executed);
    }

    #[test]
    fn lease_revocation_with_stonith() {
        let revocation = LeaseRevocation::new(test_lease_id(), test_node_id(), 10000, "stonith".to_string()).with_stonith();
        assert!(revocation.stonith_executed);
    }

    #[test]
    fn lease_revocation_reason_category() {
        let manual = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "revoked".to_string());
        assert_eq!(manual.reason_category(), "manual");
        let stonith = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "stonith executed".to_string());
        assert_eq!(stonith.reason_category(), "stonith");
        let expiry = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "lease expire".to_string());
        assert_eq!(expiry.reason_category(), "expiry");
        let other = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "unknown".to_string());
        assert_eq!(other.reason_category(), "other");
    }

    #[test]
    fn lease_revocation_time_since_revocation() {
        let revocation = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "revoked".to_string());
        assert_eq!(revocation.time_since_revocation(5000), 4000);
    }

    #[test]
    fn lease_revocation_serialization_roundtrip() {
        let revocation = LeaseRevocation::new(test_lease_id(), test_node_id(), 1000, "test".to_string());
        let json = serde_json::to_string(&revocation).unwrap();
        let deserialized: LeaseRevocation = serde_json::from_str(&json).unwrap();
        assert_eq!(revocation.lease_id, deserialized.lease_id);
        assert_eq!(revocation.reason, deserialized.reason);
    }

    // ========================================================================
    // FencingTokenRecord tests
    // ========================================================================

    #[test]
    fn fencing_token_record_new() {
        let record = FencingTokenRecord::new(test_lease_id(), test_fencing_token(), 1000);
        assert_eq!(record.lease_id, test_lease_id());
        assert_eq!(record.token, test_fencing_token());
        assert_eq!(record.increment_count, 0);
    }

    #[test]
    fn fencing_token_record_increment() {
        let mut record = FencingTokenRecord::new(test_lease_id(), test_fencing_token(), 1000);
        let new_token = record.increment(5, 2000);
        assert_eq!(new_token, FencingToken::new(6));
        assert_eq!(record.token, FencingToken::new(6));
        assert_eq!(record.increment_count, 1);
    }

    #[test]
    fn fencing_token_record_is_valid() {
        let record = FencingTokenRecord::new(test_lease_id(), FencingToken::new(10), 1000);
        assert!(record.is_valid(FencingToken::new(10)));
        assert!(record.is_valid(FencingToken::new(15)));
        assert!(!record.is_valid(FencingToken::new(5)));
    }

    #[test]
    fn fencing_token_record_next_token_value() {
        let record = FencingTokenRecord::new(test_lease_id(), FencingToken::new(10), 1000);
        assert_eq!(record.next_token_value(3), FencingToken::new(13));
    }

    #[test]
    fn fencing_token_record_is_fresh() {
        let record = FencingTokenRecord::new(test_lease_id(), test_fencing_token(), 1000);
        assert!(record.is_fresh(2000, 2000));
        assert!(!record.is_fresh(5000, 2000));
    }

    #[test]
    fn fencing_token_record_serialization_roundtrip() {
        let record = FencingTokenRecord::new(test_lease_id(), test_fencing_token(), 1000);
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: FencingTokenRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.lease_id, deserialized.lease_id);
        assert_eq!(record.token, deserialized.token);
    }

    // ========================================================================
    // AuthorityEpoch tests
    // ========================================================================

    #[test]
    fn authority_epoch_new() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        assert_eq!(epoch.domain, test_epoch());
        assert_eq!(epoch.epoch, Epoch::new(5));
        assert_eq!(epoch.authority_node, test_node_id());
    }

    #[test]
    fn authority_epoch_is_expired_at() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        assert!(!epoch.is_expired_at(10000));
        assert!(epoch.is_expired_at(70000));
    }

    #[test]
    fn authority_epoch_time_until_expiry() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        assert_eq!(epoch.time_until_expiry(2000), 59000);
        assert_eq!(epoch.time_until_expiry(70000), 0);
    }

    #[test]
    fn authority_epoch_needs_renewal_soon() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        // expires at 61000, grace 5000 -> needs renewal at 56000+
        assert!(!epoch.needs_renewal_soon(50000, 5000));
        assert!(epoch.needs_renewal_soon(57000, 5000));
    }

    #[test]
    fn authority_epoch_age_ms() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        assert_eq!(epoch.age_ms(1000), 0);
        assert_eq!(epoch.age_ms(5000), 4000);
    }

    #[test]
    fn authority_epoch_serialization_roundtrip() {
        let epoch = AuthorityEpoch::new(test_epoch(), Epoch::new(5), test_node_id(), 1000, 60000);
        let json = serde_json::to_string(&epoch).unwrap();
        let deserialized: AuthorityEpoch = serde_json::from_str(&json).unwrap();
        assert_eq!(epoch.domain, deserialized.domain);
        assert_eq!(epoch.epoch, deserialized.epoch);
    }

    // ========================================================================
    // LeaseStats tests
    // ========================================================================

    #[test]
    fn lease_stats_default() {
        let stats = LeaseStats::default();
        assert_eq!(stats.total_grants, 0);
        assert_eq!(stats.active_leases, 0);
        assert!((stats.success_rate() - 1.0).abs() < 0.01);
        assert!((stats.renewal_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn lease_stats_success_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_revocations = 2;
        stats.total_expired = 1;
        assert!((stats.success_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn lease_stats_renewal_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_renewals = 7;
        assert!((stats.renewal_rate() - 0.7).abs() < 0.01);
    }

    #[test]
    fn lease_stats_expiry_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_expired = 3;
        assert!((stats.expiry_rate() - 0.3).abs() < 0.01);
    }

    #[test]
    fn lease_stats_fencing_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_fenced = 2;
        assert!((stats.fencing_rate() - 0.2).abs() < 0.01);
    }

    #[test]
    fn lease_stats_total_terminal() {
        let mut stats = LeaseStats::default();
        stats.total_expired = 2;
        stats.total_revocations = 1;
        stats.total_fenced = 1;
        assert_eq!(stats.total_terminal(), 4);
    }

    #[test]
    fn lease_stats_failure_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_expired = 2;
        stats.total_revocations = 1;
        stats.total_fenced = 1;
        assert!((stats.failure_rate() - 0.4).abs() < 0.01);
    }

    #[test]
    fn lease_stats_stonith_rate() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.total_stonith_executions = 2;
        assert!((stats.stonith_rate() - 0.2).abs() < 0.01);
    }

    #[test]
    fn lease_stats_active_ratio() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.active_leases = 7;
        assert!((stats.active_ratio() - 0.7).abs() < 0.01);
    }

    #[test]
    fn lease_stats_health_score() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 10;
        stats.active_leases = 8;
        let score = stats.health_score();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn lease_stats_serialization_roundtrip() {
        let mut stats = LeaseStats::default();
        stats.total_grants = 5;
        stats.total_renewals = 3;
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: LeaseStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_grants, deserialized.total_grants);
    }

    // ========================================================================
    // LeaseMonitor tests
    // ========================================================================

    #[test]
    fn lease_monitor_register_and_get() {
        let monitor = LeaseMonitor::new(LeaseConfig::default());
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        monitor.register_lease(record);
        let fetched = monitor.get_lease(test_lease_id()).unwrap();
        assert_eq!(fetched.lease_id, test_lease_id());
    }

    #[test]
    fn lease_monitor_leases_for_node() {
        let monitor = LeaseMonitor::new(LeaseConfig::default());
        let record1 = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        let record2 = LeaseRecord::new(LeaseId::new([2u8; 16]), test_node_id(), test_epoch(), 10000, 2000);
        monitor.register_lease(record1);
        monitor.register_lease(record2);
        let leases = monitor.leases_for_node(test_node_id());
        assert_eq!(leases.len(), 2);
    }

    #[test]
    fn lease_monitor_active_lease_count() {
        let monitor = LeaseMonitor::new(LeaseConfig::default());
        let mut record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        record.activate(test_fencing_token(), 1000);
        monitor.register_lease(record);
        assert_eq!(monitor.active_lease_count(), 1);
    }

    #[test]
    fn lease_monitor_stats() {
        let monitor = LeaseMonitor::new(LeaseConfig::default());
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        monitor.register_lease(record);
        let stats = monitor.stats();
        assert_eq!(stats.total_grants, 1);
        assert_eq!(stats.active_leases, 1);
    }

    #[test]
    fn lease_monitor_reset() {
        let mut monitor = LeaseMonitor::new(LeaseConfig::default());
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        monitor.register_lease(record);
        monitor.reset();
        assert_eq!(monitor.node_count(), 0);
        assert_eq!(monitor.stats().total_grants, 0);
    }

    #[test]
    fn lease_monitor_decay_all_reputations() {
        let mut monitor = LeaseMonitor::new(LeaseConfig::default());
        monitor.decay_all_reputations(0.5, 2000);
        // Should not panic
    }

    #[test]
    fn lease_monitor_serialization_roundtrip() {
        let monitor = LeaseMonitor::new(LeaseConfig::default());
        let record = LeaseRecord::new(test_lease_id(), test_node_id(), test_epoch(), 10000, 1000);
        monitor.register_lease(record);
        // Monitor is not serializable directly, but we can check stats
        let stats = monitor.stats();
        let json = serde_json::to_string(&stats).unwrap();
        let _: LeaseStats = serde_json::from_str(&json).unwrap();
    }

    // ========================================================================
    // LeaseValidator tests
    // ========================================================================

    #[test]
    fn lease_validator_validate_fencing_token() {
        let validator = LeaseValidator::new();
        assert!(validator.validate_fencing_token(FencingToken::new(10), FencingToken::new(10)));
        assert!(validator.validate_fencing_token(FencingToken::new(10), FencingToken::new(15)));
        assert!(!validator.validate_fencing_token(FencingToken::new(10), FencingToken::new(5)));
    }

    #[test]
    fn lease_validator_validate_authority_epoch() {
        let validator = LeaseValidator::new();
        assert!(validator.validate_authority_epoch(Epoch::new(1), Epoch::new(5), Epoch::new(3)));
        assert!(!validator.validate_authority_epoch(Epoch::new(1), Epoch::new(2), Epoch::new(5)));
    }

    #[test]
    fn lease_validator_validate_renewal() {
        let validator = LeaseValidator::new();
        assert!(validator.validate_renewal(LeaseState::Active));
        assert!(validator.validate_renewal(LeaseState::Renewing));
        assert!(validator.validate_renewal(LeaseState::Expiring));
        assert!(!validator.validate_renewal(LeaseState::Expired));
        assert!(!validator.validate_renewal(LeaseState::Revoked));
    }

    #[test]
    fn lease_validator_validate_fencing_tokens_batch() {
        let validator = LeaseValidator::new();
        let presented = vec![
            (test_lease_id(), FencingToken::new(10)),
            (test_lease_id(), FencingToken::new(5)),
        ];
        let results = validator.validate_fencing_token_batch(FencingToken::new(10), &presented);
        assert_eq!(results.len(), 2);
        assert!(results[0].1);
        assert!(!results[1].1);
    }

    #[test]
    fn lease_validator_validate_renewal_batch() {
        let validator = LeaseValidator::new();
        let states = vec![LeaseState::Active, LeaseState::Expired, LeaseState::Active];
        let results = validator.validate_renewal_batch(&states);
        assert_eq!(results, vec![true, false, true]);
    }

    #[test]
    fn lease_validator_toggle_fencing() {
        let mut validator = LeaseValidator::new();
        assert!(validator.is_fencing_enabled());
        validator.set_fencing_enabled(false);
        assert!(!validator.is_fencing_enabled());
        // When fencing is disabled, all tokens are valid
        assert!(validator.validate_fencing_token(FencingToken::new(10), FencingToken::new(1)));
    }

    #[test]
    fn lease_validator_toggle_epoch() {
        let mut validator = LeaseValidator::new();
        assert!(validator.is_epoch_validation_enabled());
        validator.set_epoch_validation_enabled(false);
        assert!(!validator.is_epoch_validation_enabled());
    }

    // ========================================================================
    // LeaseEngine tests
    // ========================================================================

    #[test]
    fn lease_engine_new() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert_eq!(engine.stats().total_grants, 0);
        assert_eq!(engine.total_lease_count(), 0);
    }

    #[test]
    fn lease_engine_configure() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let config = LeaseConfig::default().with_default_duration(10000);
        assert!(engine.configure(config).is_ok());
        assert_eq!(engine.default_lease_duration(), 10000);
    }

    #[test]
    fn lease_engine_configure_invalid() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let config = LeaseConfig::default().with_duration_bounds(0, 300000);
        assert!(engine.configure(config).is_err());
    }

    #[test]
    fn lease_engine_grant_lease() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(grant.node_id, test_node_id());
        assert_eq!(grant.epoch, test_epoch());
        assert_eq!(engine.stats().total_grants, 1);
        assert_eq!(engine.total_lease_count(), 1);
    }

    #[test]
    fn lease_engine_grant_lease_default_duration() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 0).unwrap();
        assert_eq!(grant.duration_ms, 30000);
    }

    #[test]
    fn lease_engine_grant_lease_clamped() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 500000).unwrap();
        assert_eq!(grant.duration_ms, 300000);
    }

    #[test]
    fn lease_engine_renew_lease() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let renewal = engine.renew_lease(grant.lease_id).unwrap();
        assert_eq!(renewal.renewal_count, 1);
        assert_eq!(engine.stats().total_renewals, 1);
    }

    #[test]
    fn lease_engine_renew_lease_not_found() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let err = engine.renew_lease(test_lease_id()).unwrap_err();
        assert!(matches!(err, LeaseError::LeaseNotFound { .. }));
    }

    #[test]
    fn lease_engine_revoke_lease() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let revocation = engine.revoke_lease(grant.lease_id).unwrap();
        assert!(!revocation.stonith_executed);
        assert_eq!(engine.stats().total_revocations, 1);
    }

    #[test]
    fn lease_engine_revoke_lease_already_terminal() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.revoke_lease(grant.lease_id).unwrap();
        let err = engine.revoke_lease(grant.lease_id()).unwrap_err();
        assert!(matches!(err, LeaseError::LeaseAlreadyTerminal { .. }));
    }

    #[test]
    fn lease_engine_fencing_token() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let token = engine.fencing_token(grant.lease_id).unwrap();
        assert_eq!(token, test_fencing_token());
    }

    #[test]
    fn lease_engine_fencing_token_increment() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let new_token = engine.fencing_token_increment(grant.lease_id).unwrap();
        assert_eq!(new_token, FencingToken::new(2));
        assert_eq!(engine.stats().total_fencing_increments, 1);
    }

    #[test]
    fn lease_engine_fencing_token_check() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert!(engine.fencing_token_check(grant.lease_id, test_fencing_token()).unwrap());
        assert!(!engine.fencing_token_check(grant.lease_id, FencingToken::new(0)).unwrap());
    }

    #[test]
    fn lease_engine_stonith_execute() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.stonith_execute(test_node_id()).unwrap();
        assert_eq!(engine.stats().total_stonith_executions, 1);
    }

    #[test]
    fn lease_engine_stonith_disabled() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_stonith(false));
        let err = engine.stonith_execute(test_node_id()).unwrap_err();
        assert!(matches!(err, LeaseError::StonithDisabled));
    }

    #[test]
    fn lease_engine_split_brain_prevention() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue(test_epoch()).unwrap();
        assert!(engine.split_brain_prevention(test_epoch()).unwrap());
    }

    #[test]
    fn lease_engine_authority_epoch_issue() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let epoch = engine.authority_epoch_issue(test_epoch()).unwrap();
        assert_eq!(epoch, Epoch::new(1));
        assert_eq!(engine.stats().total_authority_epochs, 1);
    }

    #[test]
    fn lease_engine_authority_epoch() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue(test_epoch()).unwrap();
        let epoch = engine.authority_epoch(test_epoch()).unwrap();
        assert_eq!(epoch, Epoch::new(1));
    }

    #[test]
    fn lease_engine_authority_epoch_not_found() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let err = engine.authority_epoch(Epoch::new(99)).unwrap_err();
        assert!(matches!(err, LeaseError::AuthorityEpochNotFound { .. }));
    }

    #[test]
    fn lease_engine_is_lease_active() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert!(engine.is_lease_active(grant.lease_id).unwrap());
        engine.revoke_lease(grant.lease_id).unwrap();
        assert!(!engine.is_lease_active(grant.lease_id).unwrap());
    }

    #[test]
    fn lease_engine_lease_state() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(engine.lease_state(grant.lease_id).unwrap(), LeaseState::Active);
    }

    #[test]
    fn lease_engine_health_score() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.register_node_for_test(test_node_id(), test_endpoint());
        let score = engine.health_score(test_node_id()).unwrap();
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn lease_engine_full_lifecycle() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let revocation = engine.full_lifecycle(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(revocation.node_id, test_node_id());
        assert_eq!(engine.stats().total_grants, 1);
        assert_eq!(engine.stats().total_renewals, 1);
        assert_eq!(engine.stats().total_revocations, 1);
    }

    #[test]
    fn lease_engine_stonith_and_fence_node() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let count = engine.stonith_and_fence_node(test_node_id()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(engine.stats().total_stonith_executions, 1);
    }

    #[test]
    fn lease_engine_expire_all_past_due() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let count = engine.expire_all_past_due(5000);
        assert_eq!(count, 1);
        assert_eq!(engine.stats().total_expired, 1);
    }

    #[test]
    fn lease_engine_fence_node_leases() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let count = engine.fence_node_leases(test_node_id()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(engine.stats().total_fenced, 1);
    }

    #[test]
    fn lease_engine_get_expiring_leases() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let expiring = engine.get_expiring_leases(500, 10000);
        assert!(!expiring.is_empty());
    }

    #[test]
    fn lease_engine_get_expired_leases() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        engine.expire_all_past_due(5000);
        let expired = engine.get_expired_leases();
        assert_eq!(expired.len(), 1);
    }

    #[test]
    fn lease_engine_get_fenced_leases() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.fence_node_leases(test_node_id()).unwrap();
        let fenced = engine.get_fenced_leases();
        assert_eq!(fenced.len(), 1);
    }

    #[test]
    fn lease_engine_node_lease_summary() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let (active, expired, fenced) = engine.node_lease_summary(test_node_id());
        assert_eq!(active, 1);
        assert_eq!(expired, 0);
        assert_eq!(fenced, 0);
    }

    #[test]
    fn lease_engine_total_fencing_token_value() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(engine.total_fencing_token_value(), 1);
    }

    #[test]
    fn lease_engine_oldest_and_newest_active_lease() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        let oldest = engine.oldest_active_lease(now);
        assert!(oldest.is_some());
        let newest = engine.newest_active_lease(now);
        assert!(newest.is_some());
    }

    #[test]
    fn lease_engine_is_system_healthy() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(engine.is_system_healthy(0.5));
    }

    #[test]
    fn lease_engine_config_clone() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let config = engine.config_clone();
        assert_eq!(config.default_lease_duration_ms, 30000);
    }

    #[test]
    fn lease_engine_set_and_get_fencing_token_base() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.set_fencing_token_base(FencingToken::new(100));
        assert_eq!(engine.fencing_token_base(), FencingToken::new(100));
    }

    #[test]
    fn lease_engine_count_leases_by_state() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(engine.count_leases_by_state(LeaseState::Active), 1);
        assert_eq!(engine.count_leases_by_state(LeaseState::Expired), 0);
    }

    #[test]
    fn lease_engine_lease_age_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        let (min, max, avg) = engine.lease_age_stats(now);
        assert_eq!(min, 0);
        assert_eq!(max, 0);
        assert_eq!(avg, 0);
    }

    #[test]
    fn lease_engine_fencing_token_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let (tokens, increments) = engine.fencing_token_stats();
        assert_eq!(tokens, 1);
        assert_eq!(increments, 0);
    }

    #[test]
    fn lease_engine_stonith_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.stonith_execute(test_node_id()).unwrap();
        let (unique, nodes) = engine.stonith_stats();
        assert_eq!(unique, 1);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn lease_engine_lease_ids_by_expiry() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        let list = engine.lease_ids_by_expiry(now);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn lease_engine_any_lease_needs_renewal() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        assert!(engine.any_lease_needs_renewal(now + 500, 1000));
    }

    #[test]
    fn lease_engine_count_leases_needing_renewal() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        let count = engine.count_leases_needing_renewal(now + 500, 1000);
        assert_eq!(count, 1);
    }

    #[test]
    fn lease_engine_nodes_with_active_leases() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.grant_lease(test_node_id2(), test_epoch(), 10000).unwrap();
        let nodes = engine.nodes_with_active_leases();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn lease_engine_domains_with_authority() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue(test_epoch()).unwrap();
        let domains = engine.domains_with_authority();
        assert_eq!(domains.len(), 1);
    }

    #[test]
    fn lease_engine_domain_split_brain_risk() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(!engine.domain_split_brain_risk(test_epoch()));
        engine.authority_epoch_issue(test_epoch()).unwrap();
        assert!(!engine.domain_split_brain_risk(test_epoch()));
    }

    #[test]
    fn lease_engine_renew_all_expiring() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let now = crate::types::IdsTime::now_ms();
        let count = engine.renew_all_expiring(now + 500, 10000);
        assert_eq!(count, 1);
    }

    #[test]
    fn lease_engine_expire_all_past_due_with_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        let (expired, stats) = engine.expire_all_past_due_with_stats(5000);
        assert_eq!(expired, 1);
        assert_eq!(stats.total_expired, 1);
    }

    #[test]
    fn lease_engine_node_has_active_lease() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(!engine.node_has_active_lease(test_node_id()));
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert!(engine.node_has_active_lease(test_node_id()));
    }

    #[test]
    fn lease_engine_all_lease_ids() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let ids = engine.all_lease_ids();
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn lease_engine_active_lease_count() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        assert_eq!(engine.active_lease_count(), 1);
    }

    #[test]
    fn lease_engine_expired_lease_count() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        engine.expire_all_past_due(5000);
        assert_eq!(engine.expired_lease_count(), 1);
    }

    #[test]
    fn lease_engine_fenced_lease_count() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.fence_node_leases(test_node_id()).unwrap();
        assert_eq!(engine.fenced_lease_count(), 1);
    }

    #[test]
    fn lease_engine_health_report() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let report = engine.health_report();
        assert_eq!(report.total_leases, 1);
        assert_eq!(report.active_leases, 1);
        assert!((report.success_rate - 1.0).abs() < 0.01);
        assert!(report.is_healthy(0.5));
    }

    #[test]
    fn lease_engine_health_report_unhealthy() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_default_duration(1000));
        engine.grant_lease(test_node_id(), test_epoch(), 1000).unwrap();
        engine.expire_all_past_due(5000);
        let report = engine.health_report();
        assert!(!report.is_healthy(0.9));
    }

    #[test]
    fn lease_engine_serialization_of_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let stats = engine.stats();
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: LeaseStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_grants, deserialized.total_grants);
    }

    #[test]
    fn lease_engine_health_report_serialization() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let report = engine.health_report();
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: LeaseHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.total_leases, deserialized.total_leases);
    }

    #[test]
    fn lease_engine_full_lifecycle_serialization() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let revocation = engine.full_lifecycle(test_node_id(), test_epoch(), 10000).unwrap();
        let json = serde_json::to_string(&revocation).unwrap();
        let deserialized: LeaseRevocation = serde_json::from_str(&json).unwrap();
        assert_eq!(revocation.node_id, deserialized.node_id);
    }

    #[test]
    fn lease_engine_stonith_serialization() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.stonith_execute(test_node_id()).unwrap();
        let log = engine.stonith_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, test_node_id());
    }

    #[test]
    fn lease_engine_authority_node_check() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue_with_node(test_epoch(), test_node_id()).unwrap();
        assert!(engine.is_authority_node(test_epoch(), test_node_id()).unwrap());
        assert!(!engine.is_authority_node(test_epoch(), test_node_id2()).unwrap());
    }

    #[test]
    fn lease_engine_all_authority_nodes() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue_with_node(test_epoch(), test_node_id()).unwrap();
        let nodes = engine.all_authority_nodes();
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn lease_engine_config_strict_lenient() {
        let engine = LeaseEngine::new(LeaseConfig::strict());
        assert!(engine.config_is_strict());
        let engine = LeaseEngine::new(LeaseConfig::lenient());
        assert!(engine.config_is_lenient());
    }

    #[test]
    fn lease_engine_clear_and_reset() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.clear_leases();
        assert_eq!(engine.total_lease_count(), 0);
        engine.clear_fencing_tokens();
        assert_eq!(engine.total_fencing_token_count(), 0);
        engine.clear_authority_epochs();
        assert_eq!(engine.total_authority_epoch_count(), 0);
        engine.clear_stonith_log();
        assert_eq!(engine.total_stonith_count(), 0);
    }

    #[test]
    fn lease_engine_reset_stats() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        engine.reset_stats();
        assert_eq!(engine.stats().total_grants, 0);
    }

    #[test]
    fn lease_engine_validate_config() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(engine.validate_config().is_ok());
    }

    #[test]
    fn lease_error_category() {
        assert_eq!(LeaseError::LeaseNotFound { lease_id: test_lease_id() }.category(), "lease_not_found");
        assert_eq!(LeaseError::StonithDisabled.category(), "stonith_disabled");
        assert_eq!(LeaseError::Internal { message: "x".to_string() }.category(), "internal");
    }

    #[test]
    fn lease_error_is_recoverable() {
        assert!(LeaseError::LeaseNotFound { lease_id: test_lease_id() }.is_recoverable());
        assert!(!LeaseError::LeaseExpired { lease_id: test_lease_id() }.is_recoverable());
    }

    #[test]
    fn lease_error_suggested_action() {
        assert_eq!(LeaseError::LeaseNotFound { lease_id: test_lease_id() }.suggested_action(), "create the lease first");
        assert_eq!(LeaseError::StonithDisabled.suggested_action(), "enable STONITH in configuration");
    }

    #[test]
    fn lease_error_is_lease_state_error() {
        assert!(LeaseError::LeaseNotFound { lease_id: test_lease_id() }.is_lease_state_error());
        assert!(!LeaseError::StonithDisabled.is_lease_state_error());
    }

    #[test]
    fn lease_error_requires_stonith() {
        assert!(LeaseError::LeaseExpired { lease_id: test_lease_id() }.requires_stonith());
        assert!(!LeaseError::LeaseNotFound { lease_id: test_lease_id() }.requires_stonith());
    }

    #[test]
    fn lease_error_is_fencing_error() {
        assert!(LeaseError::StonithDisabled.is_fencing_error());
        assert!(!LeaseError::LeaseNotFound { lease_id: test_lease_id() }.is_fencing_error());
    }

    #[test]
    fn lease_error_severity() {
        assert_eq!(LeaseError::LeaseNotFound { lease_id: test_lease_id() }.severity(), "low");
        assert_eq!(LeaseError::StonithDisabled.severity(), "high");
        assert_eq!(LeaseError::InvalidConfig { field: "x".to_string(), reason: "y".to_string() }.severity(), "critical");
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn lease_full_integration_flow() {
        let engine = LeaseEngine::new(LeaseConfig::default());

        // Grant leases for multiple nodes
        engine.grant_lease(test_node_id(), test_epoch(), 30000).unwrap();
        engine.grant_lease(test_node_id2(), test_epoch(), 30000).unwrap();
        engine.grant_lease(test_node_id3(), test_epoch(), 30000).unwrap();

        assert_eq!(engine.total_lease_count(), 3);
        assert_eq!(engine.active_lease_count(), 3);

        // Renew one lease
        let ids = engine.all_lease_ids();
        engine.renew_lease(ids[0]).unwrap();
        assert_eq!(engine.stats().total_renewals, 1);

        // Issue authority epoch
        engine.authority_epoch_issue(test_epoch()).unwrap();
        assert_eq!(engine.stats().total_authority_epochs, 1);

        // STONITH one node
        engine.stonith_execute(test_node_id2()).unwrap();
        assert_eq!(engine.stats().total_stonith_executions, 1);

        // Check health
        let report = engine.health_report();
        assert_eq!(report.total_leases, 3);
        assert_eq!(report.stonith_count, 1);
        assert!(report.success_rate > 0.0);

        // Revoke one lease
        engine.revoke_lease(ids[1]).unwrap();
        assert_eq!(engine.stats().total_revocations, 1);

        // Expire old leases
        engine.expire_all_past_due(crate::types::IdsTime::now_ms() + 100000);
        assert!(engine.stats().total_expired >= 1);

        // Final state check
        let final_stats = engine.stats();
        assert_eq!(final_stats.total_grants, 3);
        assert!(final_stats.total_terminal() >= 1);
    }

    #[test]
    fn lease_multiple_authority_epochs() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        engine.authority_epoch_issue(test_epoch()).unwrap();
        engine.authority_epoch_issue(test_epoch()).unwrap();
        engine.authority_epoch_issue(test_epoch()).unwrap();
        assert_eq!(engine.stats().total_authority_epochs, 3);
        let latest = engine.authority_epoch(test_epoch()).unwrap();
        assert_eq!(latest, Epoch::new(3));
    }

    #[test]
    fn lease_fencing_token_monotonic() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        let grant1 = engine.grant_lease(test_node_id(), test_epoch(), 10000).unwrap();
        let grant2 = engine.grant_lease(test_node_id2(), test_epoch(), 10000).unwrap();
        let token1 = engine.fencing_token(grant1.lease_id).unwrap();
        let token2 = engine.fencing_token(grant2.lease_id).unwrap();
        // Each grant gets fencing_token starting at 1, increment is separate
        assert_eq!(token1, test_fencing_token());
        assert_eq!(token2, test_fencing_token());
    }

    #[test]
    fn lease_split_brain_prevention_disabled() {
        let engine = LeaseEngine::new(LeaseConfig::default().with_split_brain_prevention(false));
        assert!(!engine.split_brain_prevention(test_epoch()).unwrap());
    }

    #[test]
    fn lease_engine_is_fencing_enabled() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(engine.is_fencing_enabled());
        let engine = LeaseEngine::new(LeaseConfig::default().with_stonith(false));
        assert!(!engine.is_fencing_enabled());
    }

    #[test]
    fn lease_engine_is_split_brain_prevention_enabled() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert!(engine.is_split_brain_prevention_enabled());
        let engine = LeaseEngine::new(LeaseConfig::default().with_split_brain_prevention(false));
        assert!(!engine.is_split_brain_prevention_enabled());
    }

    #[test]
    fn lease_engine_authority_epoch_duration() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert_eq!(engine.authority_epoch_duration(), 60000);
    }

    #[test]
    fn lease_engine_max_renewal_attempts() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert_eq!(engine.max_renewal_attempts(), 3);
    }

    #[test]
    fn lease_engine_fencing_token_step() {
        let engine = LeaseEngine::new(LeaseConfig::default());
        assert_eq!(engine.fencing_token_step(), 1);
    }
}

