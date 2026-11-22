//! # Sealed Trait: FaultTolerance — circuit breaker, bulkhead, backpressure, graceful degradation
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The FaultTolerance trait defines failure isolation mechanisms: circuit breakers
//! (closed/open/half-open), bulkhead isolation, credit-based backpressure, load shedding,
//! graceful degradation hierarchy, retry storm prevention, thundering herd prevention,
//! failure isolation, and degradation hierarchy states. There is no global fault master
//! — each domain/peer manages its own fault tolerance context.
//! Implementations live downstream in ids-fault and ids-* crates.

use crate::types::{
    Capacity, DegradationLevel, Endpoint, FailureDomain, NodeId,
};
use crate::errors::FaultError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: FaultTolerance
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::FaultEngine {}
impl private::Sealed for super::CircuitBreaker {}
impl private::Sealed for super::CircuitBreakerMgr {}
impl private::Sealed for super::Bulkhead {}
impl private::Sealed for super::BulkheadMgr {}
impl private::Sealed for super::BackpressureMgr {}
impl private::Sealed for super::LoadSheddingMgr {}
impl private::Sealed for super::GracefulDegradeMgr {}
impl private::Sealed for super::RetryStormPrevention {}
impl private::Sealed for super::ThunderingHerdPrevention {}
impl private::Sealed for super::FailureIsolation {}
impl private::Sealed for super::DegradationHierarchy {}
impl private::Sealed for super::FaultStats {}
impl private::Sealed for super::FaultMonitor {}
impl private::Sealed for super::FaultValidator {}

// Sealed trait for fault tolerance.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait FaultTolerance: private::Sealed {
    /// Get circuit breaker state for a peer.
    fn circuit_breaker_state(&self, peer: NodeId) -> Result<CircuitState, FaultError>;

    /// Record a success for a peer's circuit breaker.
    fn record_success(&self, peer: NodeId) -> Result<(), FaultError>;

    /// Record a failure for a peer's circuit breaker.
    fn record_failure(&self, peer: NodeId) -> Result<(), FaultError>;

    /// Isolate a bulkhead domain.
    fn bulkhead_isolate(&self, domain: FailureDomain) -> Result<(), FaultError>;

    /// Release a bulkhead domain.
    fn bulkhead_release(&self, domain: FailureDomain) -> Result<(), FaultError>;

    /// Apply credit-based backpressure to an endpoint.
    fn backpressure_apply(&self, endpoint: Endpoint, credit: u64) -> Result<(), FaultError>;

    /// Consume backpressure credit.
    fn backpressure_consume(&self, endpoint: Endpoint, credit: u64) -> Result<(), FaultError>;

    /// Check if load shedding should occur.
    fn should_shed_load(&self, endpoint: Endpoint) -> Result<bool, FaultError>;

    /// Record a shed load event.
    fn record_shed_load(&self, endpoint: Endpoint) -> Result<(), FaultError>;

    /// Apply graceful degradation level.
    fn graceful_degrade(&self, level: DegradationLevel) -> Result<(), FaultError>;

    /// Get current degradation level.
    fn current_degradation_level(&self) -> Result<DegradationLevel, FaultError>;

    /// Check if retry storm should be prevented.
    fn should_prevent_retry_storm(&self, peer: NodeId) -> Result<bool, FaultError>;

    /// Record a retry attempt.
    fn record_retry_attempt(&self, peer: NodeId) -> Result<(), FaultError>;

    /// Check if thundering herd should be prevented.
    fn should_prevent_thundering_herd(&self, endpoint: Endpoint) -> Result<bool, FaultError>;

    /// Record a thundering herd wake event.
    fn record_thundering_herd_wake(&self, endpoint: Endpoint) -> Result<(), FaultError>;

    /// Isolate a failure.
    fn isolate_failure(&self, node_id: NodeId) -> Result<(), FaultError>;

    /// Get fault stats.
    fn stats(&self) -> FaultStats;

    /// Get the fault tolerance name for debugging.
    fn fault_name(&self) -> &'static str;

    /// Get total circuit breaker trips.
    fn circuit_trip_count(&self) -> u64;

    /// Get total bulkhead isolations.
    fn bulkhead_isolation_count(&self) -> u64;

    /// Get total backpressure applications.
    fn backpressure_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// FAULT ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FaultError {
    #[error("circuit breaker not found: {peer:?}")]
    CircuitBreakerNotFound { peer: NodeId },

    #[error("circuit breaker already open: {peer:?}")]
    CircuitBreakerOpen { peer: NodeId },

    #[error("bulkhead not found: {domain:?}")]
    BulkheadNotFound { domain: FailureDomain },

    #[error("bulkhead already isolated: {domain:?}")]
    BulkheadAlreadyIsolated { domain: FailureDomain },

    #[error("backpressure not found: {endpoint:?}")]
    BackpressureNotFound { endpoint: Endpoint },

    #[error("insufficient credit: {endpoint:?}, needed={needed}, available={available}")]
    InsufficientCredit {
        endpoint: Endpoint,
        needed: u64,
        available: u64,
    },

    #[error("load shedding not configured: {endpoint:?}")]
    LoadSheddingNotConfigured { endpoint: Endpoint },

    #[error("degradation not supported: {level:?}")]
    DegradationNotSupported { level: DegradationLevel },

    #[error("retry storm detected: {peer:?}")]
    RetryStormDetected { peer: NodeId },

    #[error("thundering herd detected: {endpoint:?}")]
    ThunderingHerdDetected { endpoint: Endpoint },

    #[error("failure isolation failed: {node_id:?}, reason={reason}")]
    FailureIsolationFailed { node_id: NodeId, reason: String },

    #[error("invalid fault state: {message}")]
    InvalidState { message: String },

    #[error("internal fault error: {message}")]
    Internal { message: String },
}

impl FaultError {
    pub fn category(&self) -> &'static str {
        match self {
            FaultError::CircuitBreakerNotFound { .. } => "circuit_breaker_not_found",
            FaultError::CircuitBreakerOpen { .. } => "circuit_breaker_open",
            FaultError::BulkheadNotFound { .. } => "bulkhead_not_found",
            FaultError::BulkheadAlreadyIsolated { .. } => "bulkhead_already_isolated",
            FaultError::BackpressureNotFound { .. } => "backpressure_not_found",
            FaultError::InsufficientCredit { .. } => "insufficient_credit",
            FaultError::LoadSheddingNotConfigured { .. } => "load_shedding_not_configured",
            FaultError::DegradationNotSupported { .. } => "degradation_not_supported",
            FaultError::RetryStormDetected { .. } => "retry_storm_detected",
            FaultError::ThunderingHerdDetected { .. } => "thundering_herd_detected",
            FaultError::FailureIsolationFailed { .. } => "failure_isolation_failed",
            FaultError::InvalidState { .. } => "invalid_state",
            FaultError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            FaultError::CircuitBreakerOpen { .. }
                | FaultError::InsufficientCredit { .. }
                | FaultError::RetryStormDetected { .. }
                | FaultError::ThunderingHerdDetected { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            FaultError::CircuitBreakerNotFound { .. } => "create circuit breaker first",
            FaultError::CircuitBreakerOpen { .. } => "wait for half-open or reset",
            FaultError::BulkheadNotFound { .. } => "create bulkhead first",
            FaultError::BulkheadAlreadyIsolated { .. } => "release bulkhead first",
            FaultError::BackpressureNotFound { .. } => "configure backpressure first",
            FaultError::InsufficientCredit { .. } => "wait for credit to replenish",
            FaultError::LoadSheddingNotConfigured { .. } => "configure load shedding",
            FaultError::DegradationNotSupported { .. } => "use supported degradation level",
            FaultError::RetryStormDetected { .. } => "back off and reduce retries",
            FaultError::ThunderingHerdDetected { .. } => "stagger wake-up times",
            FaultError::FailureIsolationFailed { .. } => "check isolation prerequisites",
            FaultError::InvalidState { .. } => "reset fault state",
            FaultError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CIRCUIT STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CircuitState::Closed => "closed",
            CircuitState::Open => "open",
            CircuitState::HalfOpen => "half_open",
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, CircuitState::Closed)
    }

    pub fn is_open(&self) -> bool {
        matches!(self, CircuitState::Open)
    }

    pub fn is_half_open(&self) -> bool {
        matches!(self, CircuitState::HalfOpen)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CIRCUIT BREAKER
///////////////////////////////////////////////////////////////////////////////

pub struct CircuitBreaker {
    pub peer: NodeId,
    pub state: CircuitState,
    pub failure_count: u64,
    pub success_count: u64,
    pub failure_threshold: u64,
    pub success_threshold: u64,
    pub opened_at_ms: Option<u64>,
    pub half_open_at_ms: Option<u64>,
    pub open_duration_ms: u64,
    pub trip_count: u64,
    pub reset_count: u64,
}

impl CircuitBreaker {
    pub fn new(
        peer: NodeId,
        failure_threshold: u64,
        success_threshold: u64,
        open_duration_ms: u64,
    ) -> Self {
        Self {
            peer,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            success_threshold,
            opened_at_ms: None,
            half_open_at_ms: None,
            open_duration_ms,
            trip_count: 0,
            reset_count: 0,
        }
    }

    pub fn record_success(&mut self) -> Result<(), FaultError> {
        match self.state {
            CircuitState::Closed => {
                self.success_count += 1;
                Ok(())
            }
            CircuitState::Open => {
                Err(FaultError::CircuitBreakerOpen { peer: self.peer })
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                    self.half_open_at_ms = None;
                    self.reset_count += 1;
                }
                Ok(())
            }
        }
    }

    pub fn record_failure(&mut self) -> Result<(), FaultError> {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at_ms = Some(0);
                    self.trip_count += 1;
                }
                Ok(())
            }
            CircuitState::Open => {
                Err(FaultError::CircuitBreakerOpen { peer: self.peer })
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.opened_at_ms = Some(0);
                self.half_open_at_ms = None;
                self.trip_count += 1;
                Err(FaultError::CircuitBreakerOpen { peer: self.peer })
            }
        }
    }

    pub fn open(&mut self, at_ms: u64) {
        self.state = CircuitState::Open;
        self.opened_at_ms = Some(at_ms);
        self.trip_count += 1;
    }

    pub fn half_open(&mut self, at_ms: u64) {
        self.state = CircuitState::HalfOpen;
        self.half_open_at_ms = Some(at_ms);
        self.success_count = 0;
    }

    pub fn close(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.opened_at_ms = None;
        self.half_open_at_ms = None;
        self.reset_count += 1;
    }

    pub fn can_attempt(&self, at_ms: u64) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(opened) = self.opened_at_ms {
                    at_ms - opened >= self.open_duration_ms
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn failure_count(&self) -> u64 {
        self.failure_count
    }

    pub fn success_count(&self) -> u64 {
        self.success_count
    }

    pub fn trip_count(&self) -> u64 {
        self.trip_count
    }

    pub fn reset_count(&self) -> u64 {
        self.reset_count
    }
}

///////////////////////////////////////////////////////////////////////////////
// CIRCUIT BREAKER MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct CircuitBreakerMgr {
    pub breakers: std::cell::RefCell<std::collections::HashMap<NodeId, CircuitBreaker>>,
    pub total_trips: u64,
    pub total_resets: u64,
    pub total_opens: u64,
    pub total_closes: u64,
    pub max_breakers: u64,
}

impl CircuitBreakerMgr {
    pub fn new(max_breakers: u64) -> Self {
        Self {
            breakers: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_trips: 0,
            total_resets: 0,
            total_opens: 0,
            total_closes: 0,
            max_breakers,
        }
    }

    pub fn create_breaker(
        &self,
        peer: NodeId,
        failure_threshold: u64,
        success_threshold: u64,
        open_duration_ms: u64,
    ) -> Result<CircuitBreaker, FaultError> {
        let breakers = self.breakers.borrow();
        if breakers.contains_key(&peer) {
            drop(breakers);
            return Err(FaultError::InvalidState {
                message: "circuit breaker already exists".to_string(),
            });
        }
        if breakers.len() as u64 >= self.max_breakers {
            drop(breakers);
            return Err(FaultError::InvalidState {
                message: "circuit breaker table full".to_string(),
            });
        }
        drop(breakers);

        let mut breakers = self.breakers.borrow_mut();
        let breaker = CircuitBreaker::new(
            peer, failure_threshold, success_threshold, open_duration_ms,
        );
        breakers.insert(peer, breaker.clone());
        Ok(breaker)
    }

    pub fn get_breaker(&self, peer: &NodeId) -> Option<CircuitBreaker> {
        self.breakers.borrow().get(peer).cloned()
    }

    pub fn record_success(&self, peer: &NodeId) -> Result<(), FaultError> {
        if let Some(breaker) = self.breakers.borrow_mut().get_mut(peer) {
            breaker.record_success()
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn record_failure(&self, peer: &NodeId) -> Result<(), FaultError> {
        if let Some(breaker) = self.breakers.borrow_mut().get_mut(peer) {
            let result = breaker.record_failure();
            if breaker.state().is_open() {
                self.total_opens += 1;
            }
            result
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn open_breaker(&self, peer: &NodeId, at_ms: u64) -> Result<(), FaultError> {
        if let Some(breaker) = self.breakers.borrow_mut().get_mut(peer) {
            breaker.open(at_ms);
            self.total_opens += 1;
            self.total_trips += 1;
            Ok(())
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn half_open_breaker(&self, peer: &NodeId, at_ms: u64) -> Result<(), FaultError> {
        if let Some(breaker) = self.breakers.borrow_mut().get_mut(peer) {
            breaker.half_open(at_ms);
            Ok(())
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn close_breaker(&self, peer: &NodeId) -> Result<(), FaultError> {
        if let Some(breaker) = self.breakers.borrow_mut().get_mut(peer) {
            breaker.close();
            self.total_closes += 1;
            self.total_resets += 1;
            Ok(())
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn can_attempt(&self, peer: &NodeId, at_ms: u64) -> Result<bool, FaultError> {
        if let Some(breaker) = self.breakers.borrow().get(peer) {
            Ok(breaker.can_attempt(at_ms))
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn get_state(&self, peer: &NodeId) -> Result<CircuitState, FaultError> {
        if let Some(breaker) = self.breakers.borrow().get(peer) {
            Ok(breaker.state())
        } else {
            Err(FaultError::CircuitBreakerNotFound { peer: *peer })
        }
    }

    pub fn total_trips(&self) -> u64 {
        self.total_trips
    }

    pub fn total_resets(&self) -> u64 {
        self.total_resets
    }

    pub fn total_opens(&self) -> u64 {
        self.total_opens
    }

    pub fn total_closes(&self) -> u64 {
        self.total_closes
    }

    pub fn tracked_breaker_count(&self) -> usize {
        self.breakers.borrow().len()
    }

    pub fn open_breaker_count(&self) -> usize {
        self.breakers
            .borrow()
            .values()
            .filter(|b| b.state.is_open())
            .count()
    }

    pub fn closed_breaker_count(&self) -> usize {
        self.breakers
            .borrow()
            .values()
            .filter(|b| b.state.is_closed())
            .count()
    }
}

impl Default for CircuitBreakerMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BULKHEAD
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkheadState {
    Open,
    Isolated,
}

impl BulkheadState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BulkheadState::Open => "open",
            BulkheadState::Isolated => "isolated",
        }
    }

    pub fn is_isolated(&self) -> bool {
        matches!(self, BulkheadState::Isolated)
    }
}

pub struct Bulkhead {
    pub domain: FailureDomain,
    pub state: BulkheadState,
    pub isolated_at_ms: Option<u64>,
    pub isolation_count: u64,
    pub release_count: u64,
}

impl Bulkhead {
    pub fn new(domain: FailureDomain) -> Self {
        Self {
            domain,
            state: BulkheadState::Open,
            isolated_at_ms: None,
            isolation_count: 0,
            release_count: 0,
        }
    }

    pub fn isolate(&mut self, at_ms: u64) {
        self.state = BulkheadState::Isolated;
        self.isolated_at_ms = Some(at_ms);
        self.isolation_count += 1;
    }

    pub fn release(&mut self) {
        self.state = BulkheadState::Open;
        self.isolated_at_ms = None;
        self.release_count += 1;
    }

    pub fn state(&self) -> BulkheadState {
        self.state
    }

    pub fn isolation_count(&self) -> u64 {
        self.isolation_count
    }

    pub fn release_count(&self) -> u64 {
        self.release_count
    }
}

pub struct BulkheadMgr {
    pub bulkheads: std::cell::RefCell<std::collections::HashMap<FailureDomain, Bulkhead>>,
    pub total_isolations: u64,
    pub total_releases: u64,
    pub max_bulkheads: u64,
}

impl BulkheadMgr {
    pub fn new(max_bulkheads: u64) -> Self {
        Self {
            bulkheads: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_isolations: 0,
            total_releases: 0,
            max_bulkheads,
        }
    }

    pub fn create_bulkhead(&self, domain: FailureDomain) -> Result<Bulkhead, FaultError> {
        let bulkheads = self.bulkheads.borrow();
        if bulkheads.contains_key(&domain) {
            drop(bulkheads);
            return Err(FaultError::InvalidState {
                message: "bulkhead already exists".to_string(),
            });
        }
        if bulkheads.len() as u64 >= self.max_bulkheads {
            drop(bulkheads);
            return Err(FaultError::InvalidState {
                message: "bulkhead table full".to_string(),
            });
        }
        drop(bulkheads);

        let mut bulkheads = self.bulkheads.borrow_mut();
        let bulkhead = Bulkhead::new(domain);
        bulkheads.insert(domain, bulkhead.clone());
        Ok(bulkhead)
    }

    pub fn get_bulkhead(&self, domain: &FailureDomain) -> Option<Bulkhead> {
        self.bulkheads.borrow().get(domain).cloned()
    }

    pub fn isolate(&self, domain: &FailureDomain, at_ms: u64) -> Result<(), FaultError> {
        if let Some(bulkhead) = self.bulkheads.borrow_mut().get_mut(domain) {
            if bulkhead.state().is_isolated() {
                return Err(FaultError::BulkheadAlreadyIsolated { domain: *domain });
            }
            bulkhead.isolate(at_ms);
            self.total_isolations += 1;
            Ok(())
        } else {
            Err(FaultError::BulkheadNotFound { domain: *domain })
        }
    }

    pub fn release(&self, domain: &FailureDomain) -> Result<(), FaultError> {
        if let Some(bulkhead) = self.bulkheads.borrow_mut().get_mut(domain) {
            bulkhead.release();
            self.total_releases += 1;
            Ok(())
        } else {
            Err(FaultError::BulkheadNotFound { domain: *domain })
        }
    }

    pub fn get_state(&self, domain: &FailureDomain) -> Result<BulkheadState, FaultError> {
        if let Some(bulkhead) = self.bulkheads.borrow().get(domain) {
            Ok(bulkhead.state())
        } else {
            Err(FaultError::BulkheadNotFound { domain: *domain })
        }
    }

    pub fn total_isolations(&self) -> u64 {
        self.total_isolations
    }

    pub fn total_releases(&self) -> u64 {
        self.total_releases
    }

    pub fn tracked_bulkhead_count(&self) -> usize {
        self.bulkheads.borrow().len()
    }

    pub fn isolated_bulkhead_count(&self) -> usize {
        self.bulkheads
            .borrow()
            .values()
            .filter(|b| b.state.is_isolated())
            .count()
    }
}

impl Default for BulkheadMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// BACKPRESSURE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct BackpressureWindow {
    pub endpoint: Endpoint,
    pub available_credit: u64,
    pub max_credit: u64,
    pub consumed_credit: u64,
    pub replenished_credit: u64,
}

impl BackpressureWindow {
    pub fn new(endpoint: Endpoint, max_credit: u64) -> Self {
        Self {
            endpoint,
            available_credit: max_credit,
            max_credit,
            consumed_credit: 0,
            replenished_credit: 0,
        }
    }

    pub fn apply_credit(&mut self, credit: u64) {
        self.available_credit = self.available_credit.saturating_add(credit).min(self.max_credit);
        self.replenished_credit += credit;
    }

    pub fn consume_credit(&mut self, credit: u64) -> Result<(), FaultError> {
        if credit > self.available_credit {
            return Err(FaultError::InsufficientCredit {
                endpoint: self.endpoint.clone(),
                needed: credit,
                available: self.available_credit,
            });
        }
        self.available_credit -= credit;
        self.consumed_credit += credit;
        Ok(())
    }

    pub fn available_credit(&self) -> u64 {
        self.available_credit
    }

    pub fn consumed_credit(&self) -> u64 {
        self.consumed_credit
    }

    pub fn replenished_credit(&self) -> u64 {
        self.replenished_credit
    }
}

pub struct BackpressureMgr {
    pub windows: std::cell::RefCell<std::collections::HashMap<Endpoint, BackpressureWindow>>,
    pub total_credit_applied: u64,
    pub total_credit_consumed: u64,
    pub insufficient_credit_count: u64,
    pub max_windows: u64,
}

impl BackpressureMgr {
    pub fn new(max_windows: u64) -> Self {
        Self {
            windows: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_credit_applied: 0,
            total_credit_consumed: 0,
            insufficient_credit_count: 0,
            max_windows,
        }
    }

    pub fn create_window(
        &self,
        endpoint: Endpoint,
        max_credit: u64,
    ) -> Result<BackpressureWindow, FaultError> {
        let windows = self.windows.borrow();
        if windows.contains_key(&endpoint) {
            drop(windows);
            return Err(FaultError::InvalidState {
                message: "backpressure window already exists".to_string(),
            });
        }
        if windows.len() as u64 >= self.max_windows {
            drop(windows);
            return Err(FaultError::InvalidState {
                message: "backpressure table full".to_string(),
            });
        }
        drop(windows);

        let mut windows = self.windows.borrow_mut();
        let window = BackpressureWindow::new(endpoint.clone(), max_credit);
        windows.insert(endpoint, window.clone());
        self.total_credit_applied += max_credit;
        Ok(window)
    }

    pub fn get_window(&self, endpoint: &Endpoint) -> Option<BackpressureWindow> {
        self.windows.borrow().get(endpoint).cloned()
    }

    pub fn apply_credit(
        &self,
        endpoint: &Endpoint,
        credit: u64,
    ) -> Result<(), FaultError> {
        if let Some(window) = self.windows.borrow_mut().get_mut(endpoint) {
            window.apply_credit(credit);
            self.total_credit_applied += credit;
            Ok(())
        } else {
            Err(FaultError::BackpressureNotFound { endpoint: endpoint.clone() })
        }
    }

    pub fn consume_credit(
        &self,
        endpoint: &Endpoint,
        credit: u64,
    ) -> Result<(), FaultError> {
        if let Some(window) = self.windows.borrow_mut().get_mut(endpoint) {
            let result = window.consume_credit(credit);
            if result.is_ok() {
                self.total_credit_consumed += credit;
            } else {
                self.insufficient_credit_count += 1;
            }
            result
        } else {
            Err(FaultError::BackpressureNotFound { endpoint: endpoint.clone() })
        }
    }

    pub fn available_credit(&self, endpoint: &Endpoint) -> Result<u64, FaultError> {
        if let Some(window) = self.windows.borrow().get(endpoint) {
            Ok(window.available_credit())
        } else {
            Err(FaultError::BackpressureNotFound { endpoint: endpoint.clone() })
        }
    }

    pub fn total_credit_applied(&self) -> u64 {
        self.total_credit_applied
    }

    pub fn total_credit_consumed(&self) -> u64 {
        self.total_credit_consumed
    }

    pub fn insufficient_credit_count(&self) -> u64 {
        self.insufficient_credit_count
    }

    pub fn tracked_window_count(&self) -> usize {
        self.windows.borrow().len()
    }
}

impl Default for BackpressureMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// LOAD SHEDDING MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct LoadSheddingConfig {
    pub endpoint: Endpoint,
    pub shed_threshold: f64,
    pub shed_probability: f64,
    pub enabled: bool,
}

impl LoadSheddingConfig {
    pub fn new(endpoint: Endpoint, shed_threshold: f64, shed_probability: f64) -> Self {
        Self {
            endpoint,
            shed_threshold,
            shed_probability,
            enabled: true,
        }
    }
}

pub struct LoadSheddingMgr {
    pub configs: std::cell::RefCell<std::collections::HashMap<Endpoint, LoadSheddingConfig>>,
    pub shed_events: u64,
    pub total_requests: u64,
    pub max_configs: u64,
}

impl LoadSheddingMgr {
    pub fn new(max_configs: u64) -> Self {
        Self {
            configs: std::cell::RefCell::new(std::collections::HashMap::new()),
            shed_events: 0,
            total_requests: 0,
            max_configs,
        }
    }

    pub fn configure(
        &self,
        endpoint: Endpoint,
        shed_threshold: f64,
        shed_probability: f64,
    ) -> Result<(), FaultError> {
        let configs = self.configs.borrow();
        if configs.contains_key(&endpoint) {
            drop(configs);
            return Err(FaultError::InvalidState {
                message: "load shedding already configured".to_string(),
            });
        }
        if configs.len() as u64 >= self.max_configs {
            drop(configs);
            return Err(FaultError::InvalidState {
                message: "load shedding table full".to_string(),
            });
        }
        drop(configs);

        let mut configs = self.configs.borrow_mut();
        configs.insert(
            endpoint,
            LoadSheddingConfig::new(endpoint, shed_threshold, shed_probability),
        );
        Ok(())
    }

    pub fn should_shed_load(
        &self,
        endpoint: &Endpoint,
        current_load: f64,
    ) -> Result<bool, FaultError> {
        self.total_requests += 1;
        if let Some(config) = self.configs.borrow().get(endpoint) {
            if !config.enabled {
                return Ok(false);
            }
            if current_load >= config.shed_threshold {
                // Simple deterministic shed based on load
                let shed = (current_load * 1000.0) as u64 % 100 < (config.shed_probability * 100.0) as u64;
                if shed {
                    self.shed_events += 1;
                }
                Ok(shed)
            } else {
                Ok(false)
            }
        } else {
            Err(FaultError::LoadSheddingNotConfigured { endpoint: endpoint.clone() })
        }
    }

    pub fn record_shed_load(&self, endpoint: &Endpoint) -> Result<(), FaultError> {
        if self.configs.borrow().contains_key(endpoint) {
            self.shed_events += 1;
            Ok(())
        } else {
            Err(FaultError::LoadSheddingNotConfigured { endpoint: endpoint.clone() })
        }
    }

    pub fn enable(&self, endpoint: &Endpoint) -> Result<(), FaultError> {
        if let Some(config) = self.configs.borrow_mut().get_mut(endpoint) {
            config.enabled = true;
            Ok(())
        } else {
            Err(FaultError::LoadSheddingNotConfigured { endpoint: endpoint.clone() })
        }
    }

    pub fn disable(&self, endpoint: &Endpoint) -> Result<(), FaultError> {
        if let Some(config) = self.configs.borrow_mut().get_mut(endpoint) {
            config.enabled = false;
            Ok(())
        } else {
            Err(FaultError::LoadSheddingNotConfigured { endpoint: endpoint.clone() })
        }
    }

    pub fn shed_events(&self) -> u64 {
        self.shed_events
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    pub fn shed_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.shed_events as f64 / self.total_requests as f64
    }

    pub fn tracked_config_count(&self) -> usize {
        self.configs.borrow().len()
    }
}

impl Default for LoadSheddingMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// GRACEFUL DEGRADE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct DegradationLevelMgr {
    pub current_level: DegradationLevel,
    pub level_changes: u64,
    pub degradation_events: u64,
    pub recovery_events: u64,
}

impl DegradationLevelMgr {
    pub fn new() -> Self {
        Self {
            current_level: DegradationLevel::Normal,
            level_changes: 0,
            degradation_events: 0,
            recovery_events: 0,
        }
    }

    pub fn degrade(&mut self, level: DegradationLevel) -> Result<(), FaultError> {
        let current_rank = self.current_level.rank();
        let new_rank = level.rank();
        if new_rank <= current_rank {
            return Err(FaultError::DegradationNotSupported { level });
        }
        self.current_level = level;
        self.level_changes += 1;
        self.degradation_events += 1;
        Ok(())
    }

    pub fn recover(&mut self, level: DegradationLevel) -> Result<(), FaultError> {
        let current_rank = self.current_level.rank();
        let new_rank = level.rank();
        if new_rank >= current_rank {
            return Err(FaultError::DegradationNotSupported { level });
        }
        self.current_level = level;
        self.level_changes += 1;
        self.recovery_events += 1;
        Ok(())
    }

    pub fn get_level(&self) -> DegradationLevel {
        self.current_level
    }

    pub fn level_changes(&self) -> u64 {
        self.level_changes
    }

    pub fn degradation_events(&self) -> u64 {
        self.degradation_events
    }

    pub fn recovery_events(&self) -> u64 {
        self.recovery_events
    }
}

impl Default for DegradationLevelMgr {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// RETRY STORM PREVENTION
///////////////////////////////////////////////////////////////////////////////

pub struct RetryWindow {
    pub peer: NodeId,
    pub retry_count: u64,
    pub window_start_ms: u64,
    pub window_duration_ms: u64,
    pub max_retries: u64,
    pub blocked_count: u64,
}

impl RetryWindow {
    pub fn new(
        peer: NodeId,
        window_start_ms: u64,
        window_duration_ms: u64,
        max_retries: u64,
    ) -> Self {
        Self {
            peer,
            retry_count: 0,
            window_start_ms,
            window_duration_ms,
            max_retries,
            blocked_count: 0,
        }
    }

    pub fn is_within_window(&self, at_ms: u64) -> bool {
        at_ms >= self.window_start_ms && at_ms < self.window_start_ms + self.window_duration_ms
    }

    pub fn record_retry(&mut self, at_ms: u64) -> Result<(), FaultError> {
        if !self.is_within_window(at_ms) {
            self.retry_count = 0;
            self.window_start_ms = at_ms;
        }
        if self.retry_count >= self.max_retries {
            self.blocked_count += 1;
            return Err(FaultError::RetryStormDetected { peer: self.peer });
        }
        self.retry_count += 1;
        Ok(())
    }

    pub fn retry_count(&self) -> u64 {
        self.retry_count
    }

    pub fn blocked_count(&self) -> u64 {
        self.blocked_count
    }
}

pub struct RetryStormPrevention {
    pub windows: std::cell::RefCell<std::collections::HashMap<NodeId, RetryWindow>>,
    pub total_retries: u64,
    pub total_blocked: u64,
    pub default_window_ms: u64,
    pub default_max_retries: u64,
}

impl RetryStormPrevention {
    pub fn new(default_window_ms: u64, default_max_retries: u64) -> Self {
        Self {
            windows: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_retries: 0,
            total_blocked: 0,
            default_window_ms,
            default_max_retries,
        }
    }

    pub fn record_retry(
        &self,
        peer: NodeId,
        at_ms: u64,
    ) -> Result<(), FaultError> {
        let mut windows = self.windows.borrow_mut();
        let window = windows.entry(peer).or_insert_with(|| {
            RetryWindow::new(peer, at_ms, self.default_window_ms, self.default_max_retries)
        });
        let result = window.record_retry(at_ms);
        if result.is_ok() {
            self.total_retries += 1;
        } else {
            self.total_blocked += 1;
        }
        result
    }

    pub fn get_window(&self, peer: &NodeId) -> Option<RetryWindow> {
        self.windows.borrow().get(peer).cloned()
    }

    pub fn total_retries(&self) -> u64 {
        self.total_retries
    }

    pub fn total_blocked(&self) -> u64 {
        self.total_blocked
    }

    pub fn block_rate(&self) -> f64 {
        let total = self.total_retries + self.total_blocked;
        if total == 0 {
            return 0.0;
        }
        self.total_blocked as f64 / total as f64
    }

    pub fn tracked_peer_count(&self) -> usize {
        self.windows.borrow().len()
    }
}

impl Default for RetryStormPrevention {
    fn default() -> Self {
        Self::new(60000, 100)
    }
}

///////////////////////////////////////////////////////////////////////////////
// THUNDERING HERD PREVENTION
///////////////////////////////////////////////////////////////////////////////

pub struct ThunderingHerdWindow {
    pub endpoint: Endpoint,
    pub wake_count: u64,
    pub window_start_ms: u64,
    pub window_duration_ms: u64,
    pub max_wakes: u64,
    pub blocked_count: u64,
}

impl ThunderingHerdWindow {
    pub fn new(
        endpoint: Endpoint,
        window_start_ms: u64,
        window_duration_ms: u64,
        max_wakes: u64,
    ) -> Self {
        Self {
            endpoint,
            wake_count: 0,
            window_start_ms,
            window_duration_ms,
            max_wakes,
            blocked_count: 0,
        }
    }

    pub fn is_within_window(&self, at_ms: u64) -> bool {
        at_ms >= self.window_start_ms && at_ms < self.window_start_ms + self.window_duration_ms
    }

    pub fn record_wake(&mut self, at_ms: u64) -> Result<(), FaultError> {
        if !self.is_within_window(at_ms) {
            self.wake_count = 0;
            self.window_start_ms = at_ms;
        }
        if self.wake_count >= self.max_wakes {
            self.blocked_count += 1;
            return Err(FaultError::ThunderingHerdDetected { endpoint: self.endpoint.clone() });
        }
        self.wake_count += 1;
        Ok(())
    }

    pub fn wake_count(&self) -> u64 {
        self.wake_count
    }

    pub fn blocked_count(&self) -> u64 {
        self.blocked_count
    }
}

pub struct ThunderingHerdPrevention {
    pub windows: std::cell::RefCell<std::collections::HashMap<Endpoint, ThunderingHerdWindow>>,
    pub total_wakes: u64,
    pub total_blocked: u64,
    pub default_window_ms: u64,
    pub default_max_wakes: u64,
}

impl ThunderingHerdPrevention {
    pub fn new(default_window_ms: u64, default_max_wakes: u64) -> Self {
        Self {
            windows: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_wakes: 0,
            total_blocked: 0,
            default_window_ms,
            default_max_wakes,
        }
    }

    pub fn record_wake(
        &self,
        endpoint: Endpoint,
        at_ms: u64,
    ) -> Result<(), FaultError> {
        let mut windows = self.windows.borrow_mut();
        let window = windows.entry(endpoint.clone()).or_insert_with(|| {
            ThunderingHerdWindow::new(endpoint, at_ms, self.default_window_ms, self.default_max_wakes)
        });
        let result = window.record_wake(at_ms);
        if result.is_ok() {
            self.total_wakes += 1;
        } else {
            self.total_blocked += 1;
        }
        result
    }

    pub fn get_window(&self, endpoint: &Endpoint) -> Option<ThunderingHerdWindow> {
        self.windows.borrow().get(endpoint).cloned()
    }

    pub fn total_wakes(&self) -> u64 {
        self.total_wakes
    }

    pub fn total_blocked(&self) -> u64 {
        self.total_blocked
    }

    pub fn block_rate(&self) -> f64 {
        let total = self.total_wakes + self.total_blocked;
        if total == 0 {
            return 0.0;
        }
        self.total_blocked as f64 / total as f64
    }

    pub fn tracked_endpoint_count(&self) -> usize {
        self.windows.borrow().len()
    }
}

impl Default for ThunderingHerdPrevention {
    fn default() -> Self {
        Self::new(60000, 1000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAILURE ISOLATION
///////////////////////////////////////////////////////////////////////////////

pub struct FailureIsolationRecord {
    pub node_id: NodeId,
    pub isolated_at_ms: u64,
    pub reason: String,
    pub released: bool,
}

impl FailureIsolationRecord {
    pub fn new(node_id: NodeId, isolated_at_ms: u64, reason: String) -> Self {
        Self {
            node_id,
            isolated_at_ms,
            reason,
            released: false,
        }
    }

    pub fn release(&mut self) {
        self.released = true;
    }
}

pub struct FailureIsolation {
    pub records: std::cell::RefCell<std::collections::HashMap<NodeId, FailureIsolationRecord>>,
    pub total_isolations: u64,
    pub total_releases: u64,
    pub max_records: u64,
}

impl FailureIsolation {
    pub fn new(max_records: u64) -> Self {
        Self {
            records: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_isolations: 0,
            total_releases: 0,
            max_records,
        }
    }

    pub fn isolate(
        &self,
        node_id: NodeId,
        isolated_at_ms: u64,
        reason: String,
    ) -> Result<(), FaultError> {
        let records = self.records.borrow();
        if records.contains_key(&node_id) {
            drop(records);
            return Err(FaultError::FailureIsolationFailed {
                node_id,
                reason: "already isolated".to_string(),
            });
        }
        if records.len() as u64 >= self.max_records {
            drop(records);
            return Err(FaultError::FailureIsolationFailed {
                node_id,
                reason: "isolation table full".to_string(),
            });
        }
        drop(records);

        let mut records = self.records.borrow_mut();
        let record = FailureIsolationRecord::new(node_id, isolated_at_ms, reason);
        records.insert(node_id, record);
        self.total_isolations += 1;
        Ok(())
    }

    pub fn release(&self, node_id: &NodeId) -> Result<(), FaultError> {
        if let Some(record) = self.records.borrow_mut().get_mut(node_id) {
            if record.released {
                return Err(FaultError::FailureIsolationFailed {
                    node_id: *node_id,
                    reason: "already released".to_string(),
                });
            }
            record.release();
            self.total_releases += 1;
            Ok(())
        } else {
            Err(FaultError::FailureIsolationFailed {
                node_id: *node_id,
                reason: "not isolated".to_string(),
            })
        }
    }

    pub fn is_isolated(&self, node_id: &NodeId) -> bool {
        self.records
            .borrow()
            .get(node_id)
            .map(|r| !r.released)
            .unwrap_or(false)
    }

    pub fn get_record(&self, node_id: &NodeId) -> Option<FailureIsolationRecord> {
        self.records.borrow().get(node_id).cloned()
    }

    pub fn total_isolations(&self) -> u64 {
        self.total_isolations
    }

    pub fn total_releases(&self) -> u64 {
        self.total_releases
    }

    pub fn tracked_record_count(&self) -> usize {
        self.records.borrow().len()
    }

    pub fn active_isolation_count(&self) -> usize {
        self.records.borrow().values().filter(|r| !r.released).count()
    }
}

impl Default for FailureIsolation {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DEGRADATION HIERARCHY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyLevel {
    FullService,
    ReducedService,
    ReadOnly,
    CriticalOnly,
    Emergency,
}

impl HierarchyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            HierarchyLevel::FullService => "full_service",
            HierarchyLevel::ReducedService => "reduced_service",
            HierarchyLevel::ReadOnly => "read_only",
            HierarchyLevel::CriticalOnly => "critical_only",
            HierarchyLevel::Emergency => "emergency",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            HierarchyLevel::FullService => 0,
            HierarchyLevel::ReducedService => 1,
            HierarchyLevel::ReadOnly => 2,
            HierarchyLevel::CriticalOnly => 3,
            HierarchyLevel::Emergency => 4,
        }
    }

    pub fn can_transition_to(&self, next: HierarchyLevel) -> bool {
        self.rank() <= next.rank()
    }
}

pub struct DegradationHierarchy {
    pub current_level: HierarchyLevel,
    pub level_changes: u64,
    pub degradation_events: u64,
    pub recovery_events: u64,
}

impl DegradationHierarchy {
    pub fn new() -> Self {
        Self {
            current_level: HierarchyLevel::FullService,
            level_changes: 0,
            degradation_events: 0,
            recovery_events: 0,
        }
    }

    pub fn degrade(&mut self, level: HierarchyLevel) -> Result<(), FaultError> {
        if !self.current_level.can_transition_to(level) {
            return Err(FaultError::DegradationNotSupported {
                level: DegradationLevel::Normal,
            });
        }
        self.current_level = level;
        self.level_changes += 1;
        self.degradation_events += 1;
        Ok(())
    }

    pub fn recover(&mut self, level: HierarchyLevel) -> Result<(), FaultError> {
        if !level.can_transition_to(self.current_level) {
            return Err(FaultError::DegradationNotSupported {
                level: DegradationLevel::Normal,
            });
        }
        self.current_level = level;
        self.level_changes += 1;
        self.recovery_events += 1;
        Ok(())
    }

    pub fn get_level(&self) -> HierarchyLevel {
        self.current_level
    }

    pub fn level_changes(&self) -> u64 {
        self.level_changes
    }

    pub fn degradation_events(&self) -> u64 {
        self.degradation_events
    }

    pub fn recovery_events(&self) -> u64 {
        self.recovery_events
    }
}

impl Default for DegradationHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAULT STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct FaultStats {
    pub circuit_trip_count: u64,
    pub circuit_reset_count: u64,
    pub bulkhead_isolation_count: u64,
    pub bulkhead_release_count: u64,
    pub backpressure_apply_count: u64,
    pub backpressure_consume_count: u64,
    pub insufficient_credit_count: u64,
    pub load_shed_events: u64,
    pub retry_storm_blocks: u64,
    pub thundering_herd_blocks: u64,
    pub failure_isolation_count: u64,
    pub degradation_level_changes: u64,
}

impl FaultStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fault_rate(&self) -> f64 {
        let total = self.circuit_trip_count + self.bulkhead_isolation_count + self.retry_storm_blocks + self.thundering_herd_blocks;
        if total == 0 {
            return 0.0;
        }
        total as f64 / 1000.0
    }

    pub fn system_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.circuit_trip_count as f64 * 2.0;
        score -= self.bulkhead_isolation_count as f64 * 1.5;
        score -= self.insufficient_credit_count as f64 * 0.5;
        score -= self.load_shed_events as f64 * 1.0;
        score -= self.retry_storm_blocks as f64 * 3.0;
        score -= self.thundering_herd_blocks as f64 * 2.0;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAULT MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct FaultMonitor {
    pub peer_health: std::cell::RefCell<std::collections::HashMap<NodeId, bool>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub fault_incident_count: u64,
}

impl FaultMonitor {
    pub fn new() -> Self {
        Self {
            peer_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            fault_incident_count: 0,
        }
    }

    pub fn report_health(&self, peer: NodeId, healthy: bool) {
        let mut health = self.peer_health.borrow_mut();
        let prev = health.get(&peer).copied().unwrap_or(true);
        health.insert(peer, healthy);
        self.health_check_count += 1;
        if prev && !healthy {
            self.alert_count += 1;
            self.fault_incident_count += 1;
        }
    }

    pub fn is_peer_healthy(&self, peer: &NodeId) -> bool {
        self.peer_health.borrow().get(peer).copied().unwrap_or(true)
    }

    pub fn report_incident(&self, peer: NodeId) {
        self.fault_incident_count += 1;
        self.peer_health.borrow_mut().insert(peer, false);
        self.alert_count += 1;
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn fault_incident_count(&self) -> u64 {
        self.fault_incident_count
    }

    pub fn monitored_peer_count(&self) -> usize {
        self.peer_health.borrow().len()
    }

    pub fn unhealthy_peer_count(&self) -> usize {
        self.peer_health.borrow().values().filter(|h| !*h).count()
    }
}

impl Default for FaultMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAULT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultValidationResult {
    Valid,
    InvalidPeer,
    InvalidThreshold,
    InvalidCredit,
    InvalidLevel,
}

impl FaultValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            FaultValidationResult::Valid => "valid",
            FaultValidationResult::InvalidPeer => "invalid_peer",
            FaultValidationResult::InvalidThreshold => "invalid_threshold",
            FaultValidationResult::InvalidCredit => "invalid_credit",
            FaultValidationResult::InvalidLevel => "invalid_level",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, FaultValidationResult::Valid)
    }
}

pub struct FaultValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<NodeId, FaultValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl FaultValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_peer(&self, peer: NodeId) -> FaultValidationResult {
        self.validation_count += 1;
        let result = if peer == NodeId::new([0u8; 32]) {
            FaultValidationResult::InvalidPeer
        } else {
            FaultValidationResult::Valid
        };
        match result {
            FaultValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(peer, result);
        result
    }

    pub fn validate_threshold(&self, peer: NodeId, threshold: u64) -> FaultValidationResult {
        self.validation_count += 1;
        let result = if threshold == 0 {
            FaultValidationResult::InvalidThreshold
        } else {
            FaultValidationResult::Valid
        };
        match result {
            FaultValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(peer, result);
        result
    }

    pub fn validate_credit(&self, peer: NodeId, credit: u64, available: u64) -> FaultValidationResult {
        self.validation_count += 1;
        let result = if credit > available {
            FaultValidationResult::InvalidCredit
        } else {
            FaultValidationResult::Valid
        };
        match result {
            FaultValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(peer, result);
        result
    }

    pub fn validate_level(&self, peer: NodeId, level: DegradationLevel) -> FaultValidationResult {
        self.validation_count += 1;
        let result = match level {
            DegradationLevel::Normal => FaultValidationResult::Valid,
            _ => FaultValidationResult::Valid,
        };
        match result {
            FaultValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(peer, result);
        result
    }

    pub fn get_result(&self, peer: &NodeId) -> Option<FaultValidationResult> {
        self.validations.borrow().get(peer).copied()
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

impl Default for FaultValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// FAULT ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct FaultEngine {
    pub circuit_mgr: std::cell::RefCell<CircuitBreakerMgr>,
    pub bulkhead_mgr: std::cell::RefCell<BulkheadMgr>,
    pub backpressure_mgr: std::cell::RefCell<BackpressureMgr>,
    pub load_shedding_mgr: std::cell::RefCell<LoadSheddingMgr>,
    pub degradation_mgr: std::cell::RefCell<DegradationLevelMgr>,
    pub retry_storm: std::cell::RefCell<RetryStormPrevention>,
    pub thundering_herd: std::cell::RefCell<ThunderingHerdPrevention>,
    pub failure_isolation: std::cell::RefCell<FailureIsolation>,
    pub degradation_hierarchy: std::cell::RefCell<DegradationHierarchy>,
    pub stats: std::cell::RefCell<FaultStats>,
    pub monitor: std::cell::RefCell<FaultMonitor>,
    pub validator: std::cell::RefCell<FaultValidator>,
}

impl FaultEngine {
    pub fn new() -> Self {
        Self {
            circuit_mgr: std::cell::RefCell::new(CircuitBreakerMgr::default()),
            bulkhead_mgr: std::cell::RefCell::new(BulkheadMgr::default()),
            backpressure_mgr: std::cell::RefCell::new(BackpressureMgr::default()),
            load_shedding_mgr: std::cell::RefCell::new(LoadSheddingMgr::default()),
            degradation_mgr: std::cell::RefCell::new(DegradationLevelMgr::new()),
            retry_storm: std::cell::RefCell::new(RetryStormPrevention::default()),
            thundering_herd: std::cell::RefCell::new(ThunderingHerdPrevention::default()),
            failure_isolation: std::cell::RefCell::new(FailureIsolation::default()),
            degradation_hierarchy: std::cell::RefCell::new(DegradationHierarchy::new()),
            stats: std::cell::RefCell::new(FaultStats::new()),
            monitor: std::cell::RefCell::new(FaultMonitor::new()),
            validator: std::cell::RefCell::new(FaultValidator::new()),
        }
    }

    pub fn circuit_breaker_state(&self, peer: NodeId) -> Result<CircuitState, FaultError> {
        self.circuit_mgr.borrow_mut().get_state(&peer)
    }

    pub fn record_success(&self, peer: NodeId) -> Result<(), FaultError> {
        self.circuit_mgr.borrow_mut().record_success(&peer)
    }

    pub fn record_failure(&self, peer: NodeId) -> Result<(), FaultError> {
        let result = self.circuit_mgr.borrow_mut().record_failure(&peer);
        if result.is_ok() {
            self.stats.borrow_mut().circuit_trip_count += 1;
        }
        result
    }

    pub fn bulkhead_isolate(&self, domain: FailureDomain) -> Result<(), FaultError> {
        self.bulkhead_mgr.borrow_mut().isolate(&domain, 0)?;
        self.stats.borrow_mut().bulkhead_isolation_count += 1;
        Ok(())
    }

    pub fn bulkhead_release(&self, domain: FailureDomain) -> Result<(), FaultError> {
        self.bulkhead_mgr.borrow_mut().release(&domain)?;
        self.stats.borrow_mut().bulkhead_release_count += 1;
        Ok(())
    }

    pub fn backpressure_apply(&self, endpoint: Endpoint, credit: u64) -> Result<(), FaultError> {
        self.backpressure_mgr.borrow_mut().apply_credit(&endpoint, credit)?;
        self.stats.borrow_mut().backpressure_apply_count += 1;
        Ok(())
    }

    pub fn backpressure_consume(&self, endpoint: Endpoint, credit: u64) -> Result<(), FaultError> {
        let result = self.backpressure_mgr.borrow_mut().consume_credit(&endpoint, credit);
        if result.is_ok() {
            self.stats.borrow_mut().backpressure_consume_count += 1;
        } else {
            self.stats.borrow_mut().insufficient_credit_count += 1;
        }
        result
    }

    pub fn should_shed_load(&self, endpoint: Endpoint) -> Result<bool, FaultError> {
        self.load_shedding_mgr.borrow_mut().should_shed_load(&endpoint, 0.8)
    }

    pub fn record_shed_load(&self, endpoint: Endpoint) -> Result<(), FaultError> {
        self.load_shedding_mgr.borrow_mut().record_shed_load(&endpoint)?;
        self.stats.borrow_mut().load_shed_events += 1;
        Ok(())
    }

    pub fn graceful_degrade(&self, level: DegradationLevel) -> Result<(), FaultError> {
        self.degradation_mgr.borrow_mut().degrade(level)?;
        self.stats.borrow_mut().degradation_level_changes += 1;
        Ok(())
    }

    pub fn current_degradation_level(&self) -> Result<DegradationLevel, FaultError> {
        Ok(self.degradation_mgr.borrow().get_level())
    }

    pub fn should_prevent_retry_storm(&self, peer: NodeId) -> Result<bool, FaultError> {
        let result = self.retry_storm.borrow_mut().record_retry(peer, 0);
        if result.is_err() {
            self.stats.borrow_mut().retry_storm_blocks += 1;
        }
        Ok(result.is_err())
    }

    pub fn record_retry_attempt(&self, peer: NodeId) -> Result<(), FaultError> {
        self.retry_storm.borrow_mut().record_retry(peer, 0)
    }

    pub fn should_prevent_thundering_herd(&self, endpoint: Endpoint) -> Result<bool, FaultError> {
        let result = self.thundering_herd.borrow_mut().record_wake(endpoint.clone(), 0);
        if result.is_err() {
            self.stats.borrow_mut().thundering_herd_blocks += 1;
        }
        Ok(result.is_err())
    }

    pub fn record_thundering_herd_wake(&self, endpoint: Endpoint) -> Result<(), FaultError> {
        self.thundering_herd.borrow_mut().record_wake(endpoint, 0)
    }

    pub fn isolate_failure(&self, node_id: NodeId) -> Result<(), FaultError> {
        self.failure_isolation
            .borrow_mut()
            .isolate(node_id, 0, "fault detected".to_string())?;
        self.stats.borrow_mut().failure_isolation_count += 1;
        Ok(())
    }

    pub fn stats(&self) -> FaultStats {
        self.stats.borrow().clone()
    }

    pub fn fault_name(&self) -> &'static str {
        "fault_engine"
    }

    pub fn circuit_trip_count(&self) -> u64 {
        self.stats.borrow().circuit_trip_count
    }

    pub fn bulkhead_isolation_count(&self) -> u64 {
        self.stats.borrow().bulkhead_isolation_count
    }

    pub fn backpressure_count(&self) -> u64 {
        self.stats.borrow().backpressure_apply_count
    }

    pub fn create_circuit_breaker(
        &self,
        peer: NodeId,
        failure_threshold: u64,
        success_threshold: u64,
        open_duration_ms: u64,
    ) -> Result<(), FaultError> {
        self.circuit_mgr
            .borrow_mut()
            .create_breaker(peer, failure_threshold, success_threshold, open_duration_ms)?;
        Ok(())
    }

    pub fn create_bulkhead(&self, domain: FailureDomain) -> Result<(), FaultError> {
        self.bulkhead_mgr.borrow_mut().create_bulkhead(domain)?;
        Ok(())
    }

    pub fn create_backpressure_window(
        &self,
        endpoint: Endpoint,
        max_credit: u64,
    ) -> Result<(), FaultError> {
        self.backpressure_mgr.borrow_mut().create_window(endpoint, max_credit)?;
        Ok(())
    }

    pub fn configure_load_shedding(
        &self,
        endpoint: Endpoint,
        shed_threshold: f64,
        shed_probability: f64,
    ) -> Result<(), FaultError> {
        self.load_shedding_mgr
            .borrow_mut()
            .configure(endpoint, shed_threshold, shed_probability)?;
        Ok(())
    }

    pub fn monitor_peer_health(&self, peer: NodeId, healthy: bool) {
        self.monitor.borrow_mut().report_health(peer, healthy);
    }

    pub fn is_peer_healthy(&self, peer: &NodeId) -> bool {
        self.monitor.borrow().is_peer_healthy(peer)
    }

    pub fn validate_peer(&self, peer: NodeId) -> FaultValidationResult {
        self.validator.borrow_mut().validate_peer(peer)
    }

    pub fn validate_threshold(&self, peer: NodeId, threshold: u64) -> FaultValidationResult {
        self.validator.borrow_mut().validate_threshold(peer, threshold)
    }

    pub fn validate_credit(&self, peer: NodeId, credit: u64, available: u64) -> FaultValidationResult {
        self.validator.borrow_mut().validate_credit(peer, credit, available)
    }

    pub fn validate_level(&self, peer: NodeId, level: DegradationLevel) -> FaultValidationResult {
        self.validator.borrow_mut().validate_level(peer, level)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    pub fn fault_health_score(&self) -> f64 {
        self.stats.borrow().system_health_score()
    }
}

impl Default for FaultEngine {
    fn default() -> Self {
        Self::new()
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

    fn test_failure_domain() -> FailureDomain {
        FailureDomain {
            power: Some([1u8; 16]),
            facility: Some([2u8; 16]),
            network_segment: Some([3u8; 16]),
            operator: Some([4u8; 16]),
            geographic: Some([5u8; 16]),
        }
    }

    fn test_endpoint() -> Endpoint {
        Endpoint {
            carrier_type: crate::types::CarrierType::Loopback,
            address: vec![1, 2, 3],
        }
    }

    // === FaultError tests ===
    #[test]
    fn fault_error_category() {
        assert_eq!(
            FaultError::CircuitBreakerNotFound { peer: test_node_id() }.category(),
            "circuit_breaker_not_found"
        );
        assert_eq!(
            FaultError::CircuitBreakerOpen { peer: test_node_id() }.category(),
            "circuit_breaker_open"
        );
        assert_eq!(
            FaultError::BulkheadNotFound { domain: test_failure_domain() }.category(),
            "bulkhead_not_found"
        );
        assert_eq!(
            FaultError::BulkheadAlreadyIsolated { domain: test_failure_domain() }.category(),
            "bulkhead_already_isolated"
        );
        assert_eq!(
            FaultError::BackpressureNotFound { endpoint: test_endpoint() }.category(),
            "backpressure_not_found"
        );
        assert_eq!(
            FaultError::InsufficientCredit {
                endpoint: test_endpoint(),
                needed: 100,
                available: 50,
            }
            .category(),
            "insufficient_credit"
        );
        assert_eq!(
            FaultError::LoadSheddingNotConfigured { endpoint: test_endpoint() }.category(),
            "load_shedding_not_configured"
        );
        assert_eq!(
            FaultError::DegradationNotSupported { level: DegradationLevel::Normal }.category(),
            "degradation_not_supported"
        );
        assert_eq!(
            FaultError::RetryStormDetected { peer: test_node_id() }.category(),
            "retry_storm_detected"
        );
        assert_eq!(
            FaultError::ThunderingHerdDetected { endpoint: test_endpoint() }.category(),
            "thundering_herd_detected"
        );
        assert_eq!(
            FaultError::FailureIsolationFailed {
                node_id: test_node_id(),
                reason: "test".to_string(),
            }
            .category(),
            "failure_isolation_failed"
        );
        assert_eq!(
            FaultError::InvalidState { message: "bad".to_string() }.category(),
            "invalid_state"
        );
        assert_eq!(
            FaultError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn fault_error_is_recoverable() {
        assert!(FaultError::CircuitBreakerOpen { peer: test_node_id() }.is_recoverable());
        assert!(FaultError::InsufficientCredit {
            endpoint: test_endpoint(), needed: 100, available: 50,
        }.is_recoverable());
        assert!(FaultError::RetryStormDetected { peer: test_node_id() }.is_recoverable());
        assert!(FaultError::ThunderingHerdDetected { endpoint: test_endpoint() }.is_recoverable());
        assert!(!FaultError::CircuitBreakerNotFound { peer: test_node_id() }.is_recoverable());
    }

    #[test]
    fn fault_error_suggested_action() {
        assert_eq!(
            FaultError::CircuitBreakerOpen { peer: test_node_id() }.suggested_action(),
            "wait for half-open or reset"
        );
        assert_eq!(
            FaultError::RetryStormDetected { peer: test_node_id() }.suggested_action(),
            "back off and reduce retries"
        );
        assert_eq!(
            FaultError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === CircuitState tests ===
    #[test]
    fn circuit_state_as_str() {
        assert_eq!(CircuitState::Closed.as_str(), "closed");
        assert_eq!(CircuitState::Open.as_str(), "open");
        assert_eq!(CircuitState::HalfOpen.as_str(), "half_open");
        assert!(CircuitState::Closed.is_closed());
        assert!(CircuitState::Open.is_open());
        assert!(CircuitState::HalfOpen.is_half_open());
    }

    // === CircuitBreaker tests ===
    #[test]
    fn circuit_breaker_record_success_failure() {
        let mut cb = CircuitBreaker::new(test_node_id(), 3, 2, 60000);
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_success().unwrap();
        assert_eq!(cb.success_count, 1);
        cb.record_failure().unwrap();
        assert_eq!(cb.failure_count, 1);
        cb.record_failure().unwrap();
        cb.record_failure().unwrap();
        assert_eq!(cb.state, CircuitState::Open);
        assert_eq!(cb.trip_count, 1);
    }

    #[test]
    fn circuit_breaker_open_rejects() {
        let mut cb = CircuitBreaker::new(test_node_id(), 1, 2, 60000);
        cb.record_failure().unwrap();
        assert_eq!(cb.state, CircuitState::Open);
        let err = cb.record_failure().unwrap_err();
        assert!(matches!(err, FaultError::CircuitBreakerOpen { .. }));
        let err = cb.record_success().unwrap_err();
        assert!(matches!(err, FaultError::CircuitBreakerOpen { .. }));
    }

    #[test]
    fn circuit_breaker_half_open_and_close() {
        let mut cb = CircuitBreaker::new(test_node_id(), 1, 1, 60000);
        cb.record_failure().unwrap();
        cb.half_open(1000);
        assert_eq!(cb.state, CircuitState::HalfOpen);
        cb.record_success().unwrap();
        assert_eq!(cb.state, CircuitState::Closed);
        cb.close();
        assert_eq!(cb.reset_count, 1);
    }

    #[test]
    fn circuit_breaker_can_attempt() {
        let mut cb = CircuitBreaker::new(test_node_id(), 1, 2, 60000);
        assert!(cb.can_attempt(0));
        cb.open(0);
        assert!(!cb.can_attempt(30000));
        assert!(cb.can_attempt(60000));
    }

    // === CircuitBreakerMgr tests ===
    #[test]
    fn circuit_breaker_mgr_create_and_get() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap();
        assert_eq!(mgr.tracked_breaker_count(), 1);
        let breaker = mgr.get_breaker(&test_node_id()).unwrap();
        assert_eq!(breaker.peer, test_node_id());
    }

    #[test]
    fn circuit_breaker_mgr_duplicate() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap();
        let err = mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap_err();
        assert!(matches!(err, FaultError::InvalidState { .. }));
    }

    #[test]
    fn circuit_breaker_mgr_record_success_failure() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 1, 2, 60000).unwrap();
        mgr.record_success(&test_node_id()).unwrap();
        mgr.record_failure(&test_node_id()).unwrap();
        assert_eq!(mgr.total_opens(), 1);
        assert_eq!(mgr.total_trips(), 1);
    }

    #[test]
    fn circuit_breaker_mgr_open_close() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap();
        mgr.open_breaker(&test_node_id(), 0).unwrap();
        assert_eq!(mgr.get_state(&test_node_id()).unwrap(), CircuitState::Open);
        mgr.close_breaker(&test_node_id()).unwrap();
        assert_eq!(mgr.get_state(&test_node_id()).unwrap(), CircuitState::Closed);
    }

    #[test]
    fn circuit_breaker_mgr_counts() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap();
        mgr.create_breaker(test_peer_id(), 3, 2, 60000).unwrap();
        assert_eq!(mgr.tracked_breaker_count(), 2);
        assert_eq!(mgr.closed_breaker_count(), 2);
        mgr.open_breaker(&test_node_id(), 0).unwrap();
        assert_eq!(mgr.open_breaker_count(), 1);
        assert_eq!(mgr.closed_breaker_count(), 1);
    }

    // === BulkheadState tests ===
    #[test]
    fn bulkhead_state_as_str() {
        assert_eq!(BulkheadState::Open.as_str(), "open");
        assert_eq!(BulkheadState::Isolated.as_str(), "isolated");
        assert!(BulkheadState::Isolated.is_isolated());
        assert!(!BulkheadState::Open.is_isolated());
    }

    // === Bulkhead tests ===
    #[test]
    fn bulkhead_isolate_and_release() {
        let mut bh = Bulkhead::new(test_failure_domain());
        assert_eq!(bh.state, BulkheadState::Open);
        bh.isolate(1000);
        assert_eq!(bh.state, BulkheadState::Isolated);
        assert_eq!(bh.isolation_count, 1);
        bh.release();
        assert_eq!(bh.state, BulkheadState::Open);
        assert_eq!(bh.release_count, 1);
    }

    // === BulkheadMgr tests ===
    #[test]
    fn bulkhead_mgr_create_and_isolate() {
        let mgr = BulkheadMgr::default();
        mgr.create_bulkhead(test_failure_domain()).unwrap();
        mgr.isolate(&test_failure_domain(), 1000).unwrap();
        assert_eq!(mgr.total_isolations(), 1);
        assert_eq!(mgr.isolated_bulkhead_count(), 1);
        mgr.release(&test_failure_domain()).unwrap();
        assert_eq!(mgr.total_releases(), 1);
        assert_eq!(mgr.isolated_bulkhead_count(), 0);
    }

    #[test]
    fn bulkhead_mgr_already_isolated() {
        let mgr = BulkheadMgr::default();
        mgr.create_bulkhead(test_failure_domain()).unwrap();
        mgr.isolate(&test_failure_domain(), 1000).unwrap();
        let err = mgr.isolate(&test_failure_domain(), 2000).unwrap_err();
        assert!(matches!(err, FaultError::BulkheadAlreadyIsolated { .. }));
    }

    #[test]
    fn bulkhead_mgr_not_found() {
        let mgr = BulkheadMgr::default();
        let err = mgr.isolate(&test_failure_domain(), 1000).unwrap_err();
        assert!(matches!(err, FaultError::BulkheadNotFound { .. }));
    }

    // === BackpressureWindow tests ===
    #[test]
    fn backpressure_window_consume() {
        let mut bw = BackpressureWindow::new(test_endpoint(), 100);
        assert_eq!(bw.available_credit(), 100);
        bw.consume_credit(30).unwrap();
        assert_eq!(bw.available_credit(), 70);
        assert_eq!(bw.consumed_credit(), 30);
    }

    #[test]
    fn backpressure_window_insufficient_credit() {
        let mut bw = BackpressureWindow::new(test_endpoint(), 10);
        let err = bw.consume_credit(20).unwrap_err();
        assert!(matches!(err, FaultError::InsufficientCredit { .. }));
    }

    #[test]
    fn backpressure_window_apply_credit() {
        let mut bw = BackpressureWindow::new(test_endpoint(), 100);
        bw.consume_credit(50).unwrap();
        bw.apply_credit(30);
        assert_eq!(bw.available_credit(), 80);
        assert_eq!(bw.replenished_credit(), 30);
    }

    // === BackpressureMgr tests ===
    #[test]
    fn backpressure_mgr_create_and_consume() {
        let mgr = BackpressureMgr::default();
        mgr.create_window(test_endpoint(), 100).unwrap();
        mgr.apply_credit(&test_endpoint(), 50).unwrap();
        mgr.consume_credit(&test_endpoint(), 30).unwrap();
        assert_eq!(mgr.total_credit_applied(), 150);
        assert_eq!(mgr.total_credit_consumed(), 30);
        assert_eq!(mgr.available_credit(&test_endpoint()).unwrap(), 120);
    }

    #[test]
    fn backpressure_mgr_insufficient_credit() {
        let mgr = BackpressureMgr::default();
        mgr.create_window(test_endpoint(), 10).unwrap();
        let err = mgr.consume_credit(&test_endpoint(), 20).unwrap_err();
        assert!(matches!(err, FaultError::InsufficientCredit { .. }));
        assert_eq!(mgr.insufficient_credit_count(), 1);
    }

    #[test]
    fn backpressure_mgr_not_found() {
        let mgr = BackpressureMgr::default();
        let err = mgr.consume_credit(&test_endpoint(), 10).unwrap_err();
        assert!(matches!(err, FaultError::BackpressureNotFound { .. }));
    }

    // === LoadSheddingMgr tests ===
    #[test]
    fn load_shedding_mgr_configure_and_shed() {
        let mgr = LoadSheddingMgr::default();
        mgr.configure(test_endpoint(), 0.5, 0.8).unwrap();
        assert!(mgr.should_shed_load(&test_endpoint(), 0.9).unwrap());
        assert_eq!(mgr.shed_events(), 1);
    }

    #[test]
    fn load_shedding_mgr_not_configured() {
        let mgr = LoadSheddingMgr::default();
        let err = mgr.should_shed_load(&test_endpoint(), 0.9).unwrap_err();
        assert!(matches!(err, FaultError::LoadSheddingNotConfigured { .. }));
    }

    #[test]
    fn load_shedding_mgr_enable_disable() {
        let mgr = LoadSheddingMgr::default();
        mgr.configure(test_endpoint(), 0.5, 0.8).unwrap();
        mgr.disable(&test_endpoint()).unwrap();
        assert!(!mgr.should_shed_load(&test_endpoint(), 0.9).unwrap());
        mgr.enable(&test_endpoint()).unwrap();
    }

    #[test]
    fn load_shedding_mgr_shed_rate() {
        let mgr = LoadSheddingMgr::default();
        mgr.configure(test_endpoint(), 0.1, 1.0).unwrap();
        mgr.should_shed_load(&test_endpoint(), 0.9).unwrap();
        mgr.should_shed_load(&test_endpoint(), 0.9).unwrap();
        let rate = mgr.shed_rate();
        assert!(rate >= 0.0);
        assert!(rate <= 1.0);
    }

    // === DegradationLevelMgr tests ===
    #[test]
    fn degradation_mgr_degrade_and_recover() {
        let mut mgr = DegradationLevelMgr::new();
        assert_eq!(mgr.get_level(), DegradationLevel::Normal);
        mgr.degrade(DegradationLevel::ReducedService).unwrap();
        assert_eq!(mgr.get_level(), DegradationLevel::ReducedService);
        assert_eq!(mgr.level_changes(), 1);
        assert_eq!(mgr.degradation_events(), 1);
        mgr.recover(DegradationLevel::Normal).unwrap();
        assert_eq!(mgr.get_level(), DegradationLevel::Normal);
        assert_eq!(mgr.recovery_events(), 1);
    }

    #[test]
    fn degradation_mgr_invalid_degrade() {
        let mut mgr = DegradationLevelMgr::new();
        let err = mgr.degrade(DegradationLevel::Normal).unwrap_err();
        assert!(matches!(err, FaultError::DegradationNotSupported { .. }));
    }

    // === RetryWindow tests ===
    #[test]
    fn retry_window_record_and_block() {
        let mut rw = RetryWindow::new(test_node_id(), 0, 60000, 3);
        rw.record_retry(1000).unwrap();
        rw.record_retry(2000).unwrap();
        rw.record_retry(3000).unwrap();
        let err = rw.record_retry(4000).unwrap_err();
        assert!(matches!(err, FaultError::RetryStormDetected { .. }));
        assert_eq!(rw.blocked_count(), 1);
    }

    // === RetryStormPrevention tests ===
    #[test]
    fn retry_storm_prevention_record() {
        let rsp = RetryStormPrevention::default();
        rsp.record_retry(test_node_id(), 1000).unwrap();
        assert_eq!(rsp.total_retries(), 1);
        assert_eq!(rsp.tracked_peer_count(), 1);
    }

    #[test]
    fn retry_storm_prevention_block_rate() {
        let rsp = RetryStormPrevention::new(60000, 2);
        rsp.record_retry(test_node_id(), 1000).unwrap();
        rsp.record_retry(test_node_id(), 2000).unwrap();
        let _ = rsp.record_retry(test_node_id(), 3000);
        let rate = rsp.block_rate();
        assert!((rate - (1.0 / 3.0)).abs() < 0.01);
    }

    // === ThunderingHerdWindow tests ===
    #[test]
    fn thundering_herd_window_record() {
        let mut wh = ThunderingHerdWindow::new(test_endpoint(), 0, 60000, 3);
        wh.record_wake(1000).unwrap();
        wh.record_wake(2000).unwrap();
        assert_eq!(wh.wake_count(), 2);
    }

    // === ThunderingHerdPrevention tests ===
    #[test]
    fn thundering_herd_prevention_record() {
        let thp = ThunderingHerdPrevention::default();
        thp.record_wake(test_endpoint(), 1000).unwrap();
        assert_eq!(thp.total_wakes(), 1);
        assert_eq!(thp.tracked_endpoint_count(), 1);
    }

    // === FailureIsolationRecord tests ===
    #[test]
    fn failure_isolation_record_release() {
        let mut rec = FailureIsolationRecord::new(test_node_id(), 1000, "test".to_string());
        assert!(!rec.released);
        rec.release();
        assert!(rec.released);
    }

    // === FailureIsolation tests ===
    #[test]
    fn failure_isolation_isolate_and_release() {
        let fi = FailureIsolation::default();
        fi.isolate(test_node_id(), 1000, "test".to_string()).unwrap();
        assert!(fi.is_isolated(&test_node_id()));
        assert_eq!(fi.total_isolations(), 1);
        fi.release(&test_node_id()).unwrap();
        assert!(!fi.is_isolated(&test_node_id()));
        assert_eq!(fi.total_releases(), 1);
    }

    #[test]
    fn failure_isolation_double_isolate() {
        let fi = FailureIsolation::default();
        fi.isolate(test_node_id(), 1000, "test".to_string()).unwrap();
        let err = fi.isolate(test_node_id(), 2000, "test2".to_string()).unwrap_err();
        assert!(matches!(err, FaultError::FailureIsolationFailed { .. }));
    }

    #[test]
    fn failure_isolation_active_count() {
        let fi = FailureIsolation::default();
        fi.isolate(test_node_id(), 1000, "test".to_string()).unwrap();
        fi.isolate(test_peer_id(), 2000, "test".to_string()).unwrap();
        assert_eq!(fi.active_isolation_count(), 2);
        fi.release(&test_node_id()).unwrap();
        assert_eq!(fi.active_isolation_count(), 1);
    }

    // === HierarchyLevel tests ===
    #[test]
    fn hierarchy_level_rank_and_transition() {
        assert_eq!(HierarchyLevel::FullService.rank(), 0);
        assert_eq!(HierarchyLevel::Emergency.rank(), 4);
        assert!(HierarchyLevel::FullService.can_transition_to(HierarchyLevel::Emergency));
        assert!(!HierarchyLevel::Emergency.can_transition_to(HierarchyLevel::FullService));
    }

    // === DegradationHierarchy tests ===
    #[test]
    fn degradation_hierarchy_degrade_and_recover() {
        let mut dh = DegradationHierarchy::new();
        assert_eq!(dh.get_level(), HierarchyLevel::FullService);
        dh.degrade(HierarchyLevel::ReadOnly).unwrap();
        assert_eq!(dh.get_level(), HierarchyLevel::ReadOnly);
        assert_eq!(dh.level_changes(), 1);
        dh.recover(HierarchyLevel::FullService).unwrap();
        assert_eq!(dh.get_level(), HierarchyLevel::FullService);
        assert_eq!(dh.recovery_events(), 1);
    }

    #[test]
    fn degradation_hierarchy_invalid_degrade() {
        let mut dh = DegradationHierarchy::new();
        dh.degrade(HierarchyLevel::Emergency).unwrap();
        let err = dh.degrade(HierarchyLevel::FullService).unwrap_err();
        assert!(matches!(err, FaultError::DegradationNotSupported { .. }));
    }

    // === FaultStats tests ===
    #[test]
    fn fault_stats_health_score() {
        let mut s = FaultStats::default();
        s.circuit_trip_count = 0;
        s.bulkhead_isolation_count = 0;
        assert_eq!(s.system_health_score(), 100.0);
        s.circuit_trip_count = 10;
        s.retry_storm_blocks = 5;
        let score = s.system_health_score();
        assert!(score < 100.0);
        assert!(score >= 0.0);
    }

    #[test]
    fn fault_stats_fault_rate() {
        let mut s = FaultStats::default();
        s.circuit_trip_count = 100;
        assert!((s.fault_rate() - 0.1).abs() < 0.01);
    }

    // === FaultMonitor tests ===
    #[test]
    fn fault_monitor_report_health() {
        let fm = FaultMonitor::default();
        fm.report_health(test_node_id(), true);
        assert!(fm.is_peer_healthy(&test_node_id()));
        assert_eq!(fm.health_check_count(), 1);
    }

    #[test]
    fn fault_monitor_incident() {
        let fm = FaultMonitor::default();
        fm.report_health(test_node_id(), true);
        fm.report_incident(test_node_id());
        assert!(!fm.is_peer_healthy(&test_node_id()));
        assert_eq!(fm.fault_incident_count(), 1);
        assert_eq!(fm.alert_count(), 1);
    }

    // === FaultValidationResult tests ===
    #[test]
    fn fault_validation_result_as_str() {
        assert_eq!(FaultValidationResult::Valid.as_str(), "valid");
        assert_eq!(FaultValidationResult::InvalidPeer.as_str(), "invalid_peer");
        assert_eq!(FaultValidationResult::InvalidThreshold.as_str(), "invalid_threshold");
        assert_eq!(FaultValidationResult::InvalidCredit.as_str(), "invalid_credit");
        assert_eq!(FaultValidationResult::InvalidLevel.as_str(), "invalid_level");
        assert!(FaultValidationResult::Valid.is_ok());
        assert!(!FaultValidationResult::InvalidPeer.is_ok());
    }

    // === FaultValidator tests ===
    #[test]
    fn fault_validator_validate_peer() {
        let fv = FaultValidator::default();
        assert_eq!(fv.validate_peer(test_node_id()), FaultValidationResult::Valid);
        assert_eq!(
            fv.validate_peer(NodeId::new([0u8; 32])),
            FaultValidationResult::InvalidPeer
        );
        assert_eq!(fv.validation_count(), 2);
        assert_eq!(fv.valid_count(), 1);
        assert_eq!(fv.invalid_count(), 1);
    }

    #[test]
    fn fault_validator_validate_threshold() {
        let fv = FaultValidator::default();
        assert_eq!(fv.validate_threshold(test_node_id(), 5), FaultValidationResult::Valid);
        assert_eq!(
            fv.validate_threshold(test_node_id(), 0),
            FaultValidationResult::InvalidThreshold
        );
    }

    #[test]
    fn fault_validator_validate_credit() {
        let fv = FaultValidator::default();
        assert_eq!(fv.validate_credit(test_node_id(), 50, 100), FaultValidationResult::Valid);
        assert_eq!(
            fv.validate_credit(test_node_id(), 200, 100),
            FaultValidationResult::InvalidCredit
        );
    }

    #[test]
    fn fault_validator_valid_rate() {
        let fv = FaultValidator::default();
        fv.validate_peer(test_node_id());
        fv.validate_peer(NodeId::new([0u8; 32]));
        assert!((fv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === FaultEngine tests ===
    #[test]
    fn fault_engine_circuit_breaker() {
        let engine = FaultEngine::default();
        engine.create_circuit_breaker(test_node_id(), 1, 2, 60000).unwrap();
        engine.record_failure(test_node_id()).unwrap();
        assert_eq!(
            engine.circuit_breaker_state(test_node_id()).unwrap(),
            CircuitState::Open
        );
        assert_eq!(engine.circuit_trip_count(), 1);
    }

    #[test]
    fn fault_engine_bulkhead() {
        let engine = FaultEngine::default();
        engine.create_bulkhead(test_failure_domain()).unwrap();
        engine.bulkhead_isolate(test_failure_domain()).unwrap();
        assert_eq!(engine.bulkhead_isolation_count(), 1);
        engine.bulkhead_release(test_failure_domain()).unwrap();
        assert_eq!(engine.stats().borrow().bulkhead_release_count, 1);
    }

    #[test]
    fn fault_engine_backpressure() {
        let engine = FaultEngine::default();
        engine.create_backpressure_window(test_endpoint(), 100).unwrap();
        engine.backpressure_apply(test_endpoint().clone(), 50).unwrap();
        engine.backpressure_consume(test_endpoint().clone(), 30).unwrap();
        assert_eq!(engine.backpressure_count(), 1);
    }

    #[test]
    fn fault_engine_backpressure_insufficient() {
        let engine = FaultEngine::default();
        engine.create_backpressure_window(test_endpoint(), 10).unwrap();
        let err = engine.backpressure_consume(test_endpoint().clone(), 20).unwrap_err();
        assert!(matches!(err, FaultError::InsufficientCredit { .. }));
    }

    #[test]
    fn fault_engine_load_shedding() {
        let engine = FaultEngine::default();
        engine.configure_load_shedding(test_endpoint().clone(), 0.5, 0.8).unwrap();
        assert!(engine.should_shed_load(test_endpoint().clone()).unwrap());
        assert_eq!(engine.stats().borrow().load_shed_events, 1);
    }

    #[test]
    fn fault_engine_graceful_degrade() {
        let engine = FaultEngine::default();
        engine.graceful_degrade(DegradationLevel::ReducedService).unwrap();
        assert_eq!(
            engine.current_degradation_level().unwrap(),
            DegradationLevel::ReducedService
        );
    }

    #[test]
    fn fault_engine_retry_storm() {
        let engine = FaultEngine::default();
        assert!(engine.should_prevent_retry_storm(test_node_id()).unwrap());
        assert_eq!(engine.stats().borrow().retry_storm_blocks, 0);
    }

    #[test]
    fn fault_engine_thundering_herd() {
        let engine = FaultEngine::default();
        assert!(!engine.should_prevent_thundering_herd(test_endpoint().clone()).unwrap());
    }

    #[test]
    fn fault_engine_isolate_failure() {
        let engine = FaultEngine::default();
        engine.isolate_failure(test_node_id()).unwrap();
        assert_eq!(engine.stats().borrow().failure_isolation_count, 1);
    }

    #[test]
    fn fault_engine_stats_and_name() {
        let engine = FaultEngine::default();
        assert_eq!(engine.fault_name(), "fault_engine");
        assert_eq!(engine.circuit_trip_count(), 0);
        assert_eq!(engine.bulkhead_isolation_count(), 0);
        assert_eq!(engine.backpressure_count(), 0);
    }

    #[test]
    fn fault_engine_monitor_health() {
        let engine = FaultEngine::default();
        engine.monitor_peer_health(test_node_id(), false);
        assert!(!engine.is_peer_healthy(&test_node_id()));
    }

    #[test]
    fn fault_engine_validate() {
        let engine = FaultEngine::default();
        assert_eq!(engine.validate_peer(test_node_id()), FaultValidationResult::Valid);
        assert_eq!(
            engine.validate_threshold(test_node_id(), 0),
            FaultValidationResult::InvalidThreshold
        );
        assert_eq!(engine.valid_rate(), 0.5);
    }

    #[test]
    fn fault_engine_health_score() {
        let engine = FaultEngine::default();
        let score = engine.fault_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full fault tolerance flow ===
    #[test]
    fn fault_full_flow() {
        let engine = FaultEngine::default();

        // Create circuit breaker and record failures
        engine.create_circuit_breaker(test_node_id(), 2, 2, 60000).unwrap();
        engine.record_failure(test_node_id()).unwrap();
        engine.record_failure(test_node_id()).unwrap();
        assert_eq!(
            engine.circuit_breaker_state(test_node_id()).unwrap(),
            CircuitState::Open
        );

        // Create bulkhead and isolate
        engine.create_bulkhead(test_failure_domain()).unwrap();
        engine.bulkhead_isolate(test_failure_domain()).unwrap();

        // Backpressure
        engine.create_backpressure_window(test_endpoint().clone(), 100).unwrap();
        engine.backpressure_apply(test_endpoint().clone(), 50).unwrap();
        engine.backpressure_consume(test_endpoint().clone(), 30).unwrap();

        // Load shedding
        engine.configure_load_shedding(test_endpoint().clone(), 0.5, 0.8).unwrap();
        assert!(engine.should_shed_load(test_endpoint().clone()).unwrap());

        // Degrade
        engine.graceful_degrade(DegradationLevel::ReducedService).unwrap();

        // Isolate failure
        engine.isolate_failure(test_peer_id()).unwrap();

        // Stats
        let s = engine.stats();
        assert!(s.circuit_trip_count >= 1);
        assert!(s.bulkhead_isolation_count >= 1);
        assert!(s.backpressure_apply_count >= 1);
        assert!(s.load_shed_events >= 1);
        assert!(s.failure_isolation_count >= 1);

        // Health score
        let score = engine.fault_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === CircuitBreakerMgr tests: max breakers ===
    #[test]
    fn circuit_breaker_mgr_max_breakers() {
        let mgr = CircuitBreakerMgr::new(1);
        mgr.create_breaker(test_node_id(), 3, 2, 60000).unwrap();
        let err = mgr.create_breaker(test_peer_id(), 3, 2, 60000).unwrap_err();
        assert!(matches!(err, FaultError::InvalidState { .. }));
    }

    #[test]
    fn circuit_breaker_mgr_can_attempt() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 1, 2, 60000).unwrap();
        assert!(mgr.can_attempt(&test_node_id(), 0).unwrap());
        mgr.open_breaker(&test_node_id(), 0).unwrap();
        assert!(!mgr.can_attempt(&test_node_id(), 30000).unwrap());
        assert!(mgr.can_attempt(&test_node_id(), 60000).unwrap());
    }

    #[test]
    fn circuit_breaker_mgr_half_open() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 1, 1, 60000).unwrap();
        mgr.record_failure(&test_node_id()).unwrap();
        mgr.half_open_breaker(&test_node_id(), 1000).unwrap();
        assert_eq!(mgr.get_state(&test_node_id()).unwrap(), CircuitState::HalfOpen);
        mgr.record_success(&test_node_id()).unwrap();
        assert_eq!(mgr.get_state(&test_node_id()).unwrap(), CircuitState::Closed);
    }

    #[test]
    fn circuit_breaker_mgr_open_closed_counts() {
        let mgr = CircuitBreakerMgr::default();
        mgr.create_breaker(test_node_id(), 1, 2, 60000).unwrap();
        mgr.open_breaker(&test_node_id(), 0).unwrap();
        assert_eq!(mgr.total_opens(), 1);
        mgr.close_breaker(&test_node_id()).unwrap();
        assert_eq!(mgr.total_closes(), 1);
        assert_eq!(mgr.total_resets(), 1);
    }

    // === BulkheadMgr tests: max bulkheads ===
    #[test]
    fn bulkhead_mgr_max_bulkheads() {
        let mgr = BulkheadMgr::new(1);
        mgr.create_bulkhead(test_failure_domain()).unwrap();
        let err = mgr.create_bulkhead(FailureDomain {
            power: Some([2u8; 16]),
            facility: None,
            network_segment: None,
            operator: None,
            geographic: None,
        }).unwrap_err();
        assert!(matches!(err, FaultError::InvalidState { .. }));
    }

    #[test]
    fn bulkhead_mgr_release_not_isolated() {
        let mgr = BulkheadMgr::default();
        mgr.create_bulkhead(test_failure_domain()).unwrap();
        let err = mgr.release(&test_failure_domain()).unwrap_err();
        assert!(matches!(err, FaultError::BulkheadNotFound { .. }));
    }

    // === BackpressureMgr tests: max windows ===
    #[test]
    fn backpressure_mgr_max_windows() {
        let mgr = BackpressureMgr::new(1);
        mgr.create_window(test_endpoint(), 100).unwrap();
        let err = mgr.create_window(
            Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![2] },
            100,
        ).unwrap_err();
        assert!(matches!(err, FaultError::InvalidState { .. }));
    }

    #[test]
    fn backpressure_mgr_replenish_credit() {
        let mgr = BackpressureMgr::default();
        mgr.create_window(test_endpoint(), 100).unwrap();
        mgr.consume_credit(&test_endpoint(), 80).unwrap();
        mgr.apply_credit(&test_endpoint(), 50).unwrap();
        assert_eq!(mgr.available_credit(&test_endpoint()).unwrap(), 70);
    }

    // === LoadSheddingMgr tests: max configs ===
    #[test]
    fn load_shedding_mgr_max_configs() {
        let mgr = LoadSheddingMgr::new(1);
        mgr.configure(test_endpoint(), 0.5, 0.8).unwrap();
        let err = mgr.configure(
            Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![2] },
            0.5,
            0.8,
        ).unwrap_err();
        assert!(matches!(err, FaultError::InvalidState { .. }));
    }

    #[test]
    fn load_shedding_mgr_below_threshold() {
        let mgr = LoadSheddingMgr::default();
        mgr.configure(test_endpoint(), 0.9, 0.8).unwrap();
        assert!(!mgr.should_shed_load(&test_endpoint(), 0.5).unwrap());
        assert_eq!(mgr.shed_events(), 0);
    }

    // === DegradationLevelMgr tests: invalid recover ===
    #[test]
    fn degradation_mgr_invalid_recover() {
        let mut mgr = DegradationLevelMgr::new();
        let err = mgr.recover(DegradationLevel::ReducedService).unwrap_err();
        assert!(matches!(err, FaultError::DegradationNotSupported { .. }));
    }

    // === RetryWindow tests: window reset ===
    #[test]
    fn retry_window_reset() {
        let mut rw = RetryWindow::new(test_node_id(), 0, 1000, 2);
        rw.record_retry(500).unwrap();
        rw.record_retry(600).unwrap();
        assert!(rw.record_retry(700).is_err());
        // New window after 1000ms
        assert!(rw.record_retry(1500).is_ok());
        assert_eq!(rw.retry_count(), 1);
    }

    // === RetryStormPrevention tests: block rate ===
    #[test]
    fn retry_storm_prevention_block() {
        let rsp = RetryStormPrevention::new(60000, 1);
        rsp.record_retry(test_node_id(), 1000).unwrap();
        let err = rsp.record_retry(test_node_id(), 2000).unwrap_err();
        assert!(matches!(err, FaultError::RetryStormDetected { .. }));
        assert_eq!(rsp.total_blocked(), 1);
    }

    // === ThunderingHerdWindow tests: window reset ===
    #[test]
    fn thundering_herd_window_reset() {
        let mut wh = ThunderingHerdWindow::new(test_endpoint(), 0, 1000, 2);
        wh.record_wake(500).unwrap();
        wh.record_wake(600).unwrap();
        assert!(wh.record_wake(700).is_err());
        assert!(wh.record_wake(1500).is_ok());
        assert_eq!(wh.wake_count(), 1);
    }

    // === ThunderingHerdPrevention tests: block rate ===
    #[test]
    fn thundering_herd_prevention_block_rate() {
        let thp = ThunderingHerdPrevention::new(60000, 1);
        thp.record_wake(test_endpoint(), 1000).unwrap();
        let _ = thp.record_wake(test_endpoint(), 2000);
        let rate = thp.block_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === FailureIsolation tests: max records ===
    #[test]
    fn failure_isolation_max_records() {
        let fi = FailureIsolation::new(1);
        fi.isolate(test_node_id(), 1000, "test".to_string()).unwrap();
        let err = fi.isolate(test_peer_id(), 2000, "test".to_string()).unwrap_err();
        assert!(matches!(err, FaultError::FailureIsolationFailed { .. }));
    }

    #[test]
    fn failure_isolation_release_not_isolated() {
        let fi = FailureIsolation::default();
        let err = fi.release(&test_node_id()).unwrap_err();
        assert!(matches!(err, FaultError::FailureIsolationFailed { .. }));
    }

    #[test]
    fn failure_isolation_release_already_released() {
        let fi = FailureIsolation::default();
        fi.isolate(test_node_id(), 1000, "test".to_string()).unwrap();
        fi.release(&test_node_id()).unwrap();
        let err = fi.release(&test_node_id()).unwrap_err();
        assert!(matches!(err, FaultError::FailureIsolationFailed { .. }));
    }

    // === DegradationHierarchy tests: max level ===
    #[test]
    fn degradation_hierarchy_emergency() {
        let mut dh = DegradationHierarchy::new();
        dh.degrade(HierarchyLevel::Emergency).unwrap();
        assert_eq!(dh.get_level(), HierarchyLevel::Emergency);
        let err = dh.degrade(HierarchyLevel::FullService).unwrap_err();
        assert!(matches!(err, FaultError::DegradationNotSupported { .. }));
    }

    // === FaultStats tests: fault rate ===
    #[test]
    fn fault_stats_zero_rate() {
        let s = FaultStats::default();
        assert_eq!(s.fault_rate(), 0.0);
        assert_eq!(s.system_health_score(), 100.0);
    }

    // === FaultMonitor tests: multiple peers ===
    #[test]
    fn fault_monitor_multiple_peers() {
        let fm = FaultMonitor::default();
        fm.report_health(test_node_id(), false);
        fm.report_health(test_peer_id(), false);
        assert_eq!(fm.monitored_peer_count(), 2);
        assert_eq!(fm.unhealthy_peer_count(), 2);
        assert_eq!(fm.alert_count(), 2);
    }

    #[test]
    fn fault_monitor_report_incident() {
        let fm = FaultMonitor::default();
        fm.report_incident(test_node_id());
        assert!(!fm.is_peer_healthy(&test_node_id()));
        assert_eq!(fm.fault_incident_count(), 1);
    }

    // === FaultValidator tests: validate_level ===
    #[test]
    fn fault_validator_validate_level() {
        let fv = FaultValidator::default();
        assert_eq!(
            fv.validate_level(test_node_id(), DegradationLevel::Normal),
            FaultValidationResult::Valid
        );
    }

    #[test]
    fn fault_validator_invalid_count() {
        let fv = FaultValidator::default();
        fv.validate_peer(NodeId::new([0u8; 32]));
        fv.validate_threshold(test_node_id(), 0);
        fv.validate_credit(test_node_id(), 200, 100);
        assert_eq!(fv.invalid_count(), 3);
        assert_eq!(fv.valid_count(), 0);
    }

    // === FaultEngine tests: validate methods ===
    #[test]
    fn fault_engine_validate_all() {
        let engine = FaultEngine::default();
        assert_eq!(engine.validate_peer(test_node_id()), FaultValidationResult::Valid);
        assert_eq!(
            engine.validate_threshold(test_node_id(), 5),
            FaultValidationResult::Valid
        );
        assert_eq!(
            engine.validate_credit(test_node_id(), 50, 100),
            FaultValidationResult::Valid
        );
        assert_eq!(
            engine.validate_level(test_node_id(), DegradationLevel::Normal),
            FaultValidationResult::Valid
        );
    }

    // === FaultEngine tests: load shedding disabled ===
    #[test]
    fn fault_engine_load_shedding_disabled() {
        let engine = FaultEngine::default();
        engine.configure_load_shedding(test_endpoint().clone(), 0.5, 0.8).unwrap();
        engine.load_shedding_mgr.borrow_mut().disable(&test_endpoint()).unwrap();
        assert!(!engine.should_shed_load(test_endpoint().clone()).unwrap());
    }

    // === FaultEngine tests: retry storm blocked ===
    #[test]
    fn fault_engine_retry_storm_blocked() {
        let engine = FaultEngine::default();
        engine.retry_storm.borrow_mut().windows.borrow_mut().insert(
            test_node_id(),
            RetryWindow::new(test_node_id(), 0, 60000, 0),
        );
        let result = engine.should_prevent_retry_storm(test_node_id());
        assert!(result.is_ok() && result.unwrap());
    }

    // === FaultEngine tests: thundering herd blocked ===
    #[test]
    fn fault_engine_thundering_herd_blocked() {
        let engine = FaultEngine::default();
        engine.thundering_herd.borrow_mut().windows.borrow_mut().insert(
            test_endpoint(),
            ThunderingHerdWindow::new(test_endpoint(), 0, 60000, 0),
        );
        let result = engine.should_prevent_thundering_herd(test_endpoint().clone());
        assert!(result.is_ok() && result.unwrap());
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
}