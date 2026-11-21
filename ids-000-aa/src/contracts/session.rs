//! # ids-000-aa Contract: Session
//!
//! `trait Session` — THE ONE CALL binding: client ↔ IDS session management.
//! Carrier-agnostic: operates on opaque IDs and endpoints only.

use crate::types::{
    DatasetId, Endpoint, HUnitId, NodeId, ObjectID, OpRef, RegionId, SessionID,
};
use crate::errors::SessionError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Session
///////////////////////////////////////////////////////////////////////////////

/// THE ONE CALL binding: client-to-IDS session management with ids_submit.
/// Sealed: only ids-000-aa and designated impl crates can implement.
pub trait Session: private::Sealed {
    /// Create a new session for a client node.
    fn create(&self, client_id: NodeId) -> Result<SessionID, SessionError>;

    /// Expire/terminate a session by ID.
    fn expire(&self, session_id: SessionID) -> Result<(), SessionError>;

    /// Send a heartbeat to keep a session alive.
    fn heartbeat(&self, session_id: SessionID) -> Result<(), SessionError>;

    /// Submit an operation — THE ONE CALL (ids_submit).
    fn submit(&self, session_id: SessionID, op: Vec<u8>)
        -> Result<OpRef, SessionError>;

    /// Get the session's associated client node ID.
    fn client_id(&self, session_id: SessionID) -> Result<NodeId, SessionError>;

    /// Get the session's current region.
    fn region(&self, session_id: SessionID) -> Result<RegionId, SessionError>;

    /// Get the session's associated HUnit (if any).
    fn hunit(&self, session_id: SessionID) -> Result<Option<HUnitId>, SessionError>;

    /// Get the session's creation timestamp (IDS standard time ms).
    fn created_at_ms(&self, session_id: SessionID) -> Result<u64, SessionError>;

    /// Get the session's last heartbeat timestamp.
    fn last_heartbeat_ms(&self, session_id: SessionID) -> Result<u64, SessionError>;

    /// Check if a session exists and is active.
    fn is_active(&self, session_id: SessionID) -> Result<bool, SessionError>;

    /// Get the total operations submitted in this session.
    fn op_count(&self, session_id: SessionID) -> Result<u64, SessionError>;

    /// Get the total bytes submitted in this session.
    fn byte_count(&self, session_id: SessionID) -> Result<u64, SessionError>;

    /// Get the associated endpoint for this session.
    fn endpoint(&self, session_id: SessionID) -> Result<Option<Endpoint>, SessionError>;

    /// Associate a dataset with this session.
    fn associate_dataset(
        &self,
        session_id: SessionID,
        dataset: DatasetId,
    ) -> Result<(), SessionError>;

    /// List datasets associated with this session.
    fn datasets(&self, session_id: SessionID) -> Result<Vec<DatasetId>, SessionError>;

    /// Get the session's priority for scheduling.
    fn priority(&self, session_id: SessionID) -> Result<u32, SessionError>;

    /// Set the session's priority for scheduling.
    fn set_priority(
        &self,
        session_id: SessionID,
        priority: u32,
    ) -> Result<(), SessionError>;

    /// Get the session's name for debugging.
    fn session_name(&self) -> &'static str {
        "session"
    }

    /// Get total active sessions.
    fn active_session_count(&self) -> u64 {
        0
    }

    /// Get total sessions ever created.
    fn total_sessions_created(&self) -> u64 {
        0
    }

    /// Get total sessions expired.
    fn total_sessions_expired(&self) -> u64 {
        0
    }

    /// Reset all session state, clearing all sessions.
    fn reset(&self) -> Result<(), SessionError> {
        Ok(())
    }

    /// Flush pending session operations to durable storage.
    fn flush(&self) -> Result<(), SessionError> {
        Ok(())
    }

    /// Check if the session manager is healthy.
    fn is_healthy(&self) -> bool {
        true
    }

    /// Get the maximum number of concurrent sessions.
    fn max_sessions(&self) -> u64 {
        100000
    }

    /// Get the default session TTL in milliseconds.
    fn default_ttl_ms(&self) -> u64 {
        300000
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED SUPERTRAIT MODULE
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed supertrait — prevents downstream crates from implementing Session.
    pub trait Sealed {}
    impl Sealed for crate::types::SessionID {}
}

///////////////////////////////////////////////////////////////////////////////
// ERROR TYPE: SessionError
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by session operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: SessionID },

    #[error("session already exists: {session_id}")]
    SessionAlreadyExists { session_id: SessionID },

    #[error("session expired: {session_id}")]
    SessionExpired { session_id: SessionID },

    #[error("session not active: {session_id}")]
    SessionNotActive { session_id: SessionID },

    #[error("client not found: {client_id}")]
    ClientNotFound { client_id: NodeId },

    #[error("client already has active session: {client_id}")]
    ClientAlreadyHasSession { client_id: NodeId },

    #[error("submit failed: session={session_id}, reason={reason}")]
    SubmitFailed { session_id: SessionID, reason: String },

    #[error("heartbeat failed: session={session_id}, reason={reason}")]
    HeartbeatFailed { session_id: SessionID, reason: String },

    #[error("expire failed: session={session_id}, reason={reason}")]
    ExpireFailed { session_id: SessionID, reason: String },

    #[error("dataset association failed: session={session_id}, dataset={dataset}")]
    DatasetAssociationFailed {
        session_id: SessionID,
        dataset: DatasetId,
    },

    #[error("dataset not associated with session: session={session_id}, dataset={dataset}")]
    DatasetNotAssociated {
        session_id: SessionID,
        dataset: DatasetId,
    },

    #[error("session limit exceeded: max {max}")]
    SessionLimitExceeded { max: u64 },

    #[error("invalid session ID: {session_id}")]
    InvalidSessionId { session_id: SessionID },

    #[error("flush failed: {reason}")]
    FlushFailed { reason: String },

    #[error("reset failed: {reason}")]
    ResetFailed { reason: String },

    #[error("session internal error: {message}")]
    Internal { message: String },
}

impl SessionError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SessionError::SubmitFailed { .. }
                | SessionError::HeartbeatFailed { .. }
                | SessionError::FlushFailed { .. }
                | SessionError::ResetFailed { .. }
                | SessionError::Internal { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        !self.is_retryable()
    }

    pub fn category(&self) -> &'static str {
        match self {
            SessionError::SessionNotFound { .. } => "session_not_found",
            SessionError::SessionAlreadyExists { .. } => "session_already_exists",
            SessionError::SessionExpired { .. } => "session_expired",
            SessionError::SessionNotActive { .. } => "session_not_active",
            SessionError::ClientNotFound { .. } => "client_not_found",
            SessionError::ClientAlreadyHasSession { .. } => "client_already_has_session",
            SessionError::SubmitFailed { .. } => "submit_failed",
            SessionError::HeartbeatFailed { .. } => "heartbeat_failed",
            SessionError::ExpireFailed { .. } => "expire_failed",
            SessionError::DatasetAssociationFailed { .. } => "dataset_association_failed",
            SessionError::DatasetNotAssociated { .. } => "dataset_not_associated",
            SessionError::SessionLimitExceeded { .. } => "session_limit_exceeded",
            SessionError::InvalidSessionId { .. } => "invalid_session_id",
            SessionError::FlushFailed { .. } => "flush_failed",
            SessionError::ResetFailed { .. } => "reset_failed",
            SessionError::Internal { .. } => "internal",
        }
    }

    pub fn plain_message(&self) -> String {
        match self {
            SessionError::SessionNotFound { session_id } => {
                format!("session not found: {session_id}")
            }
            SessionError::SessionAlreadyExists { session_id } => {
                format!("session already exists: {session_id}")
            }
            SessionError::SessionExpired { session_id } => {
                format!("session expired: {session_id}")
            }
            SessionError::SessionNotActive { session_id } => {
                format!("session not active: {session_id}")
            }
            SessionError::ClientNotFound { client_id } => {
                format!("client not found: {client_id}")
            }
            SessionError::ClientAlreadyHasSession { client_id } => {
                format!("client already has active session: {client_id}")
            }
            SessionError::SubmitFailed { session_id, reason } => {
                format!("submit failed: session={session_id}, reason={reason}")
            }
            SessionError::HeartbeatFailed { session_id, reason } => {
                format!("heartbeat failed: session={session_id}, reason={reason}")
            }
            SessionError::ExpireFailed { session_id, reason } => {
                format!("expire failed: session={session_id}, reason={reason}")
            }
            SessionError::DatasetAssociationFailed {
                session_id,
                dataset,
            } => format!(
                "dataset association failed: session={session_id}, dataset={dataset}"
            ),
            SessionError::DatasetNotAssociated {
                session_id,
                dataset,
            } => format!(
                "dataset not associated with session: session={session_id}, dataset={dataset}"
            ),
            SessionError::SessionLimitExceeded { max } => {
                format!("session limit exceeded: max {max}")
            }
            SessionError::InvalidSessionId { session_id } => {
                format!("invalid session ID: {session_id}")
            }
            SessionError::FlushFailed { reason } => {
                format!("flush failed: {reason}")
            }
            SessionError::ResetFailed { reason } => {
                format!("reset failed: {reason}")
            }
            SessionError::Internal { message } => {
                format!("session internal error: {message}")
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Creating,
    Active,
    Expired,
    Failed,
}

impl SessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionState::Creating => "creating",
            SessionState::Active => "active",
            SessionState::Expired => "expired",
            SessionState::Failed => "failed",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, SessionState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Expired | SessionState::Failed)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION INFO
///////////////////////////////////////////////////////////////////////////////

/// Information about an active session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionID,
    pub client_id: NodeId,
    pub state: SessionState,
    pub region: RegionId,
    pub hunit: Option<HUnitId>,
    pub created_at_ms: u64,
    pub last_heartbeat_ms: u64,
    pub op_count: u64,
    pub byte_count: u64,
    pub endpoint: Option<Endpoint>,
    pub priority: u32,
    pub datasets: Vec<DatasetId>,
}

impl SessionInfo {
    pub fn new(
        session_id: SessionID,
        client_id: NodeId,
        region: RegionId,
        created_at_ms: u64,
    ) -> Self {
        Self {
            session_id,
            client_id,
            state: SessionState::Active,
            region,
            hunit: None,
            created_at_ms,
            last_heartbeat_ms: created_at_ms,
            op_count: 0,
            byte_count: 0,
            endpoint: None,
            priority: 0,
            datasets: Vec::new(),
        }
    }

    pub fn with_hunit(mut self, hunit: HUnitId) -> Self {
        self.hunit = Some(hunit);
        self
    }

    pub fn with_endpoint(mut self, endpoint: Endpoint) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn record_op(&mut self, bytes: u64) {
        self.op_count += 1;
        self.byte_count += bytes;
    }

    pub fn record_heartbeat(&mut self, now_ms: u64) {
        self.last_heartbeat_ms = now_ms;
    }

    pub fn associate_dataset(&mut self, dataset: DatasetId) {
        if !self.datasets.contains(&dataset) {
            self.datasets.push(dataset);
        }
    }

    pub fn expire(&mut self) {
        self.state = SessionState::Expired;
    }

    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.created_at_ms)
    }

    pub fn idle_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.last_heartbeat_ms)
    }

    pub fn is_idle(&self, now_ms: u64, threshold_ms: u64) -> bool {
        self.idle_ms(now_ms) > threshold_ms
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION MANAGER STATE
///////////////////////////////////////////////////////////////////////////////

pub struct SessionManagerState {
    pub sessions: std::cell::RefCell<std::collections::HashMap<SessionID, SessionInfo>>,
    pub client_sessions: std::cell::RefCell<std::collections::HashMap<NodeId, SessionID>>,
    pub next_session_id: std::cell::Cell<u64>,
    pub total_created: u64,
    pub total_expired: u64,
    pub total_submits: u64,
    pub total_bytes: u64,
}

impl SessionManagerState {
    pub fn new() -> Self {
        Self {
            sessions: std::cell::RefCell::new(std::collections::HashMap::new()),
            client_sessions: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_session_id: std::cell::Cell::new(1),
            total_created: 0,
            total_expired: 0,
            total_submits: 0,
            total_bytes: 0,
        }
    }

    pub fn create_session(
        &self,
        client_id: NodeId,
        region: RegionId,
        now_ms: u64,
    ) -> Result<SessionID, SessionError> {
        if self.client_sessions.borrow().contains_key(&client_id) {
            return Err(SessionError::ClientAlreadyHasSession { client_id });
        }
        let id = SessionID::new(self.next_session_id.get());
        self.next_session_id.set(id.value() + 1);
        let info = SessionInfo::new(id, client_id, region, now_ms);
        self.sessions.borrow_mut().insert(id, info.clone());
        self.client_sessions
            .borrow_mut()
            .insert(client_id, id);
        self.total_created += 1;
        Ok(id)
    }

    pub fn get_session(&self, session_id: SessionID) -> Option<SessionInfo> {
        self.sessions.borrow().get(&session_id).cloned()
    }

    pub fn expire_session(&self, session_id: SessionID) -> Result<(), SessionError> {
        let mut sessions = self.sessions.borrow_mut();
        let info = sessions.get(&session_id).ok_or_else(|| {
            SessionError::SessionNotFound { session_id }
        })?;
        let client_id = info.client_id;
        drop(sessions);
        self.sessions.borrow_mut().remove(&session_id);
        self.client_sessions.borrow_mut().remove(&client_id);
        self.total_expired += 1;
        Ok(())
    }

    pub fn heartbeat_session(&self, session_id: SessionID, now_ms: u64) -> Result<(), SessionError> {
        let mut sessions = self.sessions.borrow_mut();
        let info = sessions.get_mut(&session_id).ok_or_else(|| {
            SessionError::SessionNotFound { session_id }
        })?;
        info.record_heartbeat(now_ms);
        Ok(())
    }

    pub fn record_submit(
        &self,
        session_id: SessionID,
        bytes: u64,
    ) -> Result<(), SessionError> {
        let mut sessions = self.sessions.borrow_mut();
        let info = sessions.get_mut(&session_id).ok_or_else(|| {
            SessionError::SessionNotFound { session_id }
        })?;
        info.record_op(bytes);
        drop(sessions);
        self.total_submits += 1;
        self.total_bytes += bytes;
        Ok(())
    }

    pub fn active_count(&self) -> u64 {
        self.sessions.borrow().len() as u64
    }

    pub fn is_healthy(&self) -> bool {
        true
    }

    pub fn reset(&self) {
        self.sessions.borrow_mut().clear();
        self.client_sessions.borrow_mut().clear();
        self.next_session_id.set(1);
        self.total_created = 0;
        self.total_expired = 0;
        self.total_submits = 0;
        self.total_bytes = 0;
    }
}

impl Default for SessionManagerState {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION CONFIG
///////////////////////////////////////////////////////////////////////////////

/// Configuration for session behavior.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub max_sessions: u64,
    pub default_ttl_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub max_ops_per_session: u64,
    pub max_bytes_per_session: u64,
    pub enable_heartbeat: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_sessions: 100000,
            default_ttl_ms: 300000,
            heartbeat_interval_ms: 30000,
            max_ops_per_session: 10000000,
            max_bytes_per_session: 1073741824,
            enable_heartbeat: true,
        }
    }
}

impl SessionConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_sessions(mut self, max: u64) -> Self {
        self.max_sessions = max;
        self
    }

    pub fn with_default_ttl(mut self, ms: u64) -> Self {
        self.default_ttl_ms = ms;
        self
    }

    pub fn with_heartbeat_interval(mut self, ms: u64) -> Self {
        self.heartbeat_interval_ms = ms;
        self
    }

    pub fn validate(&self) -> Result<(), SessionError> {
        if self.max_sessions == 0 {
            return Err(SessionError::Internal {
                message: "max sessions must be non-zero".to_string(),
            });
        }
        if self.default_ttl_ms == 0 {
            return Err(SessionError::Internal {
                message: "default TTL must be non-zero".to_string(),
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STATISTICS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub sessions_created: u64,
    pub sessions_expired: u64,
    pub total_submits: u64,
    pub total_bytes: u64,
    pub failed_submits: u64,
    pub failed_heartbeats: u64,
}

impl SessionStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_sessions(&self) -> u64 {
        self.sessions_created.saturating_sub(self.sessions_expired)
    }

    pub fn submit_success_rate(&self) -> f64 {
        let total = self.total_submits + self.failed_submits;
        if total == 0 {
            return 1.0;
        }
        self.total_submits as f64 / total as f64
    }

    pub fn heartbeat_success_rate(&self) -> f64 {
        let total = self.total_submits + self.failed_heartbeats;
        if total == 0 {
            return 1.0;
        }
        self.total_submits as f64 / total as f64
    }

    pub fn average_bytes_per_submit(&self) -> f64 {
        if self.total_submits == 0 {
            return 0.0;
        }
        self.total_bytes as f64 / self.total_submits as f64
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION ENDPOINT BINDING
///////////////////////////////////////////////////////////////////////////////

pub struct SessionEndpointBinding {
    pub session_id: SessionID,
    pub endpoint: Option<Endpoint>,
    pub bound: bool,
    pub bound_at_ms: Option<u64>,
}

impl SessionEndpointBinding {
    pub fn new(session_id: SessionID) -> Self {
        Self {
            session_id,
            endpoint: None,
            bound: false,
            bound_at_ms: None,
        }
    }

    pub fn bind(&mut self, endpoint: Endpoint, now_ms: u64) {
        self.endpoint = Some(endpoint);
        self.bound = true;
        self.bound_at_ms = Some(now_ms);
    }

    pub fn unbind(&mut self) {
        self.endpoint = None;
        self.bound = false;
        self.bound_at_ms = None;
    }

    pub fn is_bound(&self) -> bool {
        self.bound
    }

    pub fn bound_duration_ms(&self, now_ms: u64) -> Option<u64> {
        self.bound_at_ms.map(|start| now_ms.saturating_sub(start))
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION OPERATION TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionOpTracker {
    pub ops: std::cell::RefCell<std::collections::HashMap<OpRef, u64>>,
    pub total_tracked: u64,
}

impl SessionOpTracker {
    pub fn new() -> Self {
        Self {
            ops: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_tracked: 0,
        }
    }

    pub fn track_op(&self, op_ref: OpRef, timestamp_ms: u64) {
        self.ops.borrow_mut().insert(op_ref, timestamp_ms);
        self.total_tracked += 1;
    }

    pub fn get_op_timestamp(&self, op_ref: OpRef) -> Option<u64> {
        self.ops.borrow().get(&op_ref).copied()
    }

    pub fn remove_op(&self, op_ref: OpRef) -> bool {
        self.ops.borrow_mut().remove(&op_ref).is_some()
    }

    pub fn op_count(&self) -> usize {
        self.ops.borrow().len()
    }

    pub fn total_tracked(&self) -> u64 {
        self.total_tracked
    }
}

impl Default for SessionOpTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION PRIORITY QUEUE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionQueueItem {
    pub priority: u32,
    pub session_id: SessionID,
    pub op_bytes: u64,
}

pub struct SessionPriorityQueue {
    pub items: std::cell::RefCell<Vec<SessionQueueItem>>,
    pub max_size: usize,
    pub total_enqueued: u64,
    pub total_dequeued: u64,
}

impl SessionPriorityQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: std::cell::RefCell::new(Vec::new()),
            max_size,
            total_enqueued: 0,
            total_dequeued: 0,
        }
    }

    pub fn enqueue(&self, item: SessionQueueItem) -> bool {
        if self.items.borrow().len() >= self.max_size {
            return false;
        }
        self.items.borrow_mut().push(item);
        self.items.borrow_mut().sort_by_key(|i| i.priority);
        self.total_enqueued += 1;
        true
    }

    pub fn dequeue(&self) -> Option<SessionQueueItem> {
        let item = self.items.borrow_mut().pop();
        if item.is_some() {
            self.total_dequeued += 1;
        }
        item
    }

    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.items.borrow().len() >= self.max_size
    }
}

impl Default for SessionPriorityQueue {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION WINDOW MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionWindowManager {
    pub window_size: u64,
    pub ops_in_flight: u64,
    pub max_window_size: u64,
    pub min_window_size: u64,
}

impl SessionWindowManager {
    pub fn new() -> Self {
        Self {
            window_size: 65536,
            ops_in_flight: 0,
            max_window_size: 1048576,
            min_window_size: 1024,
        }
    }

    pub fn can_submit(&self, bytes: u64) -> bool {
        self.ops_in_flight + bytes <= self.window_size
    }

    pub fn add_in_flight(&mut self, bytes: u64) {
        self.ops_in_flight = self.ops_in_flight.saturating_add(bytes);
    }

    pub fn remove_in_flight(&mut self, bytes: u64) {
        self.ops_in_flight = self.ops_in_flight.saturating_sub(bytes);
    }

    pub fn increase_window(&mut self, factor: f64) {
        self.window_size = (self.window_size as f64 * factor) as u64;
        self.window_size = self.window_size.min(self.max_window_size);
        self.window_size = self.window_size.max(self.min_window_size);
    }

    pub fn decrease_window(&mut self, factor: f64) {
        self.window_size = (self.window_size as f64 * factor) as u64;
        self.window_size = self.window_size.min(self.max_window_size);
        self.window_size = self.window_size.max(self.min_window_size);
    }

    pub fn window_utilization(&self) -> f64 {
        if self.window_size == 0 {
            return 0.0;
        }
        self.ops_in_flight as f64 / self.window_size as f64
    }

    pub fn is_window_full(&self) -> bool {
        self.window_utilization() >= 1.0
    }
}

impl Default for SessionWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionRateLimiter {
    pub max_ops_per_sec: u64,
    pub current_ops: std::cell::Cell<u64>,
    pub window_start_ms: std::cell::Cell<u64>,
    pub throttled_ops: u64,
}

impl SessionRateLimiter {
    pub fn new(max_ops_per_sec: u64) -> Self {
        Self {
            max_ops_per_sec,
            current_ops: std::cell::Cell::new(0),
            window_start_ms: std::cell::Cell::new(0),
            throttled_ops: 0,
        }
    }

    pub fn try_acquire(&self, now_ms: u64) -> bool {
        let window_start = self.window_start_ms.get();
        if now_ms.saturating_sub(window_start) >= 1000 {
            self.current_ops.set(0);
            self.window_start_ms.set(now_ms);
        }
        let current = self.current_ops.get();
        if current >= self.max_ops_per_sec {
            self.throttled_ops += 1;
            false
        } else {
            self.current_ops.set(current + 1);
            true
        }
    }

    pub fn throttled_count(&self) -> u64 {
        self.throttled_ops
    }

    pub fn current_ops(&self) -> u64 {
        self.current_ops.get()
    }

    pub fn is_rate_limited(&self, now_ms: u64) -> bool {
        let window_start = self.window_start_ms.get();
        if now_ms.saturating_sub(window_start) >= 1000 {
            return false;
        }
        self.current_ops.get() >= self.max_ops_per_sec
    }
}

impl Default for SessionRateLimiter {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl SessionHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionHealth::Healthy => "healthy",
            SessionHealth::Degraded => "degraded",
            SessionHealth::Unhealthy => "unhealthy",
            SessionHealth::Unknown => "unknown",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, SessionHealth::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, SessionHealth::Degraded)
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, SessionHealth::Unhealthy)
    }
}

pub struct SessionHealthMonitor {
    pub state: SessionState,
    pub health: SessionHealth,
    pub consecutive_failures: u32,
    pub total_submits: u64,
    pub total_heartbeats: u64,
    pub total_errors: u64,
}

impl SessionHealthMonitor {
    pub fn new() -> Self {
        Self {
            state: SessionState::Creating,
            health: SessionHealth::Unknown,
            consecutive_failures: 0,
            total_submits: 0,
            total_heartbeats: 0,
            total_errors: 0,
        }
    }

    pub fn record_success(&mut self, bytes: u64) {
        self.state = SessionState::Active;
        self.health = if self.consecutive_failures > 0 {
            SessionHealth::Degraded
        } else {
            SessionHealth::Healthy
        };
        self.consecutive_failures = 0;
        self.total_submits += 1;
    }

    pub fn record_heartbeat(&mut self) {
        self.total_heartbeats += 1;
        if self.health == SessionHealth::Unknown {
            self.health = SessionHealth::Healthy;
        }
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.total_errors += 1;
        self.health = if self.consecutive_failures >= 5 {
            SessionHealth::Unhealthy
        } else if self.consecutive_failures >= 2 {
            SessionHealth::Degraded
        } else {
            SessionHealth::Healthy
        };
        if self.consecutive_failures >= 10 {
            self.state = SessionState::Failed;
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }

    pub fn is_unhealthy(&self) -> bool {
        self.health.is_unhealthy()
    }

    pub fn reset(&mut self) {
        self.state = SessionState::Creating;
        self.health = SessionHealth::Unknown;
        self.consecutive_failures = 0;
        self.total_submits = 0;
        self.total_heartbeats = 0;
        self.total_errors = 0;
    }
}

impl Default for SessionHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION POOL
///////////////////////////////////////////////////////////////////////////////

pub struct SessionPool {
    pub sessions: std::cell::RefCell<std::collections::HashMap<SessionID, SessionInfo>>,
    pub max_sessions: u64,
    pub total_created: u64,
    pub total_expired: u64,
}

impl SessionPool {
    pub fn new(max_sessions: u64) -> Self {
        Self {
            sessions: std::cell::RefCell::new(std::collections::HashMap::new()),
            max_sessions,
            total_created: 0,
            total_expired: 0,
        }
    }

    pub fn add(&self, info: SessionInfo) -> bool {
        if self.sessions.borrow().len() >= self.max_sessions as usize {
            return false;
        }
        self.sessions.borrow_mut().insert(info.session_id, info);
        self.total_created += 1;
        true
    }

    pub fn remove(&self, session_id: SessionID) -> bool {
        if self.sessions.borrow_mut().remove(&session_id).is_some() {
            self.total_expired += 1;
            true
        } else {
            false
        }
    }

    pub fn get(&self, session_id: SessionID) -> Option<SessionInfo> {
        self.sessions.borrow().get(&session_id).cloned()
    }

    pub fn len(&self) -> usize {
        self.sessions.borrow().len()
    }

    pub fn is_full(&self) -> bool {
        self.sessions.borrow().len() >= self.max_sessions as usize
    }

    pub fn total_created(&self) -> u64 {
        self.total_created
    }

    pub fn total_expired(&self) -> u64 {
        self.total_expired
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION CREDIT MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionCreditManager {
    pub available_credits: u64,
    pub total_credits_issued: u64,
    pub total_credits_consumed: u64,
    pub credit_unit_ops: u64,
}

impl SessionCreditManager {
    pub fn new(initial_credits: u64, credit_unit_ops: u64) -> Self {
        Self {
            available_credits: initial_credits,
            total_credits_issued: initial_credits,
            total_credits_consumed: 0,
            credit_unit_ops,
        }
    }

    pub fn consume_credit(&mut self, ops: u64) -> bool {
        let credits_needed = ops.div_ceil(self.credit_unit_ops);
        if self.available_credits >= credits_needed {
            self.available_credits -= credits_needed;
            self.total_credits_consumed += credits_needed;
            true
        } else {
            false
        }
    }

    pub fn issue_credit(&mut self, credits: u64) {
        self.available_credits += credits;
        self.total_credits_issued += credits;
    }

    pub fn credit_ratio(&self) -> f64 {
        if self.total_credits_issued == 0 {
            return 1.0;
        }
        self.available_credits as f64 / self.total_credits_issued as f64
    }

    pub fn is_exhausted(&self) -> bool {
        self.available_credits == 0
    }
}

impl Default for SessionCreditManager {
    fn default() -> Self {
        Self::new(10000, 100)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION BACKPRESSURE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionBackpressureManager {
    pub queue_depth: u64,
    pub max_queue_depth: u64,
    pub throttled: std::cell::Cell<bool>,
}

impl SessionBackpressureManager {
    pub fn new(max_queue_depth: u64) -> Self {
        Self {
            queue_depth: 0,
            max_queue_depth,
            throttled: std::cell::Cell::new(false),
        }
    }

    pub fn update_queue_depth(&self, depth: u64) {
        self.queue_depth = depth;
        self.throttled.set(depth >= self.max_queue_depth);
    }

    pub fn is_throttled(&self) -> bool {
        self.throttled.get()
    }

    pub fn queue_utilization(&self) -> f64 {
        if self.max_queue_depth == 0 {
            return 0.0;
        }
        self.queue_depth as f64 / self.max_queue_depth as f64
    }

    pub fn can_submit(&self) -> bool {
        !self.is_throttled()
    }
}

impl Default for SessionBackpressureManager {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION CLOCK
///////////////////////////////////////////////////////////////////////////////

pub struct SessionClock {
    pub epoch_ms: u64,
    pub ticks: u64,
}

impl SessionClock {
    pub fn new() -> Self {
        Self {
            epoch_ms: 0,
            ticks: 0,
        }
    }

    pub fn now_ms(&self) -> u64 {
        self.epoch_ms
    }

    pub fn tick(&mut self, ms: u64) {
        self.epoch_ms += ms;
        self.ticks += 1;
    }

    pub fn tick_count(&self) -> u64 {
        self.ticks
    }
}

impl Default for SessionClock {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION EVENT LOGGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub event_id: u64,
    pub event_type: SessionEventType,
    pub session_id: Option<SessionID>,
    pub client_id: Option<NodeId>,
    pub timestamp_ms: u64,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEventType {
    Create,
    Expire,
    Heartbeat,
    Submit,
    AssociateDataset,
    Error,
}

impl SessionEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionEventType::Create => "create",
            SessionEventType::Expire => "expire",
            SessionEventType::Heartbeat => "heartbeat",
            SessionEventType::Submit => "submit",
            SessionEventType::AssociateDataset => "associate_dataset",
            SessionEventType::Error => "error",
        }
    }
}

pub struct SessionEventLogger {
    pub events: std::cell::RefCell<Vec<SessionEvent>>,
    pub max_events: usize,
    pub next_event_id: std::cell::Cell<u64>,
}

impl SessionEventLogger {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: std::cell::RefCell::new(Vec::new()),
            max_events,
            next_event_id: std::cell::Cell::new(0),
        }
    }

    pub fn log(
        &self,
        event_type: SessionEventType,
        session_id: Option<SessionID>,
        client_id: Option<NodeId>,
        timestamp_ms: u64,
        details: &str,
    ) {
        let id = self.next_event_id.get();
        self.next_event_id.set(id + 1);
        let event = SessionEvent {
            event_id: id,
            event_type,
            session_id,
            client_id,
            timestamp_ms,
            details: details.to_string(),
        };
        let mut events = self.events.borrow_mut();
        if events.len() >= self.max_events {
            events.remove(0);
        }
        events.push(event);
    }

    pub fn event_count(&self) -> usize {
        self.events.borrow().len()
    }

    pub fn events_by_type(&self, event_type: SessionEventType) -> usize {
        self.events
            .borrow()
            .iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    pub fn clear(&self) {
        self.events.borrow_mut().clear();
    }
}

impl Default for SessionEventLogger {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct SessionValidator {
    pub validations: u64,
    pub validation_failures: u64,
    pub invalid_session_ids: u64,
}

impl SessionValidator {
    pub fn new() -> Self {
        Self {
            validations: 0,
            validation_failures: 0,
            invalid_session_ids: 0,
        }
    }

    pub fn validate_session_id(&mut self, session_id: SessionID) -> bool {
        self.validations += 1;
        if session_id.is_zero() {
            self.validation_failures += 1;
            self.invalid_session_ids += 1;
            false
        } else {
            true
        }
    }

    pub fn validate_client_id(&mut self, client_id: NodeId) -> bool {
        self.validations += 1;
        let all_zero = client_id.0.iter().all(|b| *b == 0);
        if all_zero {
            self.validation_failures += 1;
            false
        } else {
            true
        }
    }

    pub fn pass_rate(&self) -> f64 {
        if self.validations == 0 {
            return 1.0;
        }
        (self.validations - self.validation_failures) as f64 / self.validations as f64
    }
}

impl Default for SessionValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STATE TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransition {
    Create,
    Activate,
    Expire,
    Fail,
    Recover,
}

pub struct SessionStateTracker {
    pub transitions: std::cell::RefCell<Vec<(SessionID, SessionTransition, u64)>>,
    pub total_transitions: u64,
}

impl SessionStateTracker {
    pub fn new() -> Self {
        Self {
            transitions: std::cell::RefCell::new(Vec::new()),
            total_transitions: 0,
        }
    }

    pub fn record_transition(
        &self,
        session_id: SessionID,
        transition: SessionTransition,
        timestamp_ms: u64,
    ) {
        self.transitions
            .borrow_mut()
            .push((session_id, transition, timestamp_ms));
        self.total_transitions += 1;
    }

    pub fn transitions_for_session(
        &self,
        session_id: SessionID,
    ) -> Vec<(SessionTransition, u64)> {
        self.transitions
            .borrow()
            .iter()
            .filter(|(sid, _, _)| *sid == session_id)
            .map(|(_, t, ts)| (*t, *ts))
            .collect()
    }

    pub fn transition_count(&self) -> usize {
        self.transitions.borrow().len()
    }

    pub fn total_transitions(&self) -> u64 {
        self.total_transitions
    }
}

impl Default for SessionStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION QUOTA MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionQuotaManager {
    pub max_ops_per_session: u64,
    pub max_bytes_per_session: u64,
    pub session_quotas: std::cell::RefCell<
        std::collections::HashMap<SessionID, (u64, u64)>,
    >,
}

impl SessionQuotaManager {
    pub fn new(max_ops_per_session: u64, max_bytes_per_session: u64) -> Self {
        Self {
            max_ops_per_session,
            max_bytes_per_session,
            session_quotas: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn check_quota(
        &self,
        session_id: SessionID,
        ops: u64,
        bytes: u64,
    ) -> bool {
        let quotas = self.session_quotas.borrow();
        if let Some((current_ops, current_bytes)) = quotas.get(&session_id) {
            current_ops + ops <= self.max_ops_per_session
                && current_bytes + bytes <= self.max_bytes_per_session
        } else {
            ops <= self.max_ops_per_session && bytes <= self.max_bytes_per_session
        }
    }

    pub fn record_usage(
        &self,
        session_id: SessionID,
        ops: u64,
        bytes: u64,
    ) {
        let mut quotas = self.session_quotas.borrow_mut();
        let entry = quotas.entry(session_id).or_insert((0, 0));
        entry.0 += ops;
        entry.1 += bytes;
    }

    pub fn get_usage(&self, session_id: SessionID) -> Option<(u64, u64)> {
        self.session_quotas.borrow().get(&session_id).copied()
    }

    pub fn reset_quota(&self, session_id: SessionID) -> bool {
        self.session_quotas.borrow_mut().remove(&session_id).is_some()
    }
}

impl Default for SessionQuotaManager {
    fn default() -> Self {
        Self::new(10000000, 1073741824)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION RECOVERY MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRecoveryState {
    Idle,
    Recovering,
    Recovered,
    Failed,
}

impl SessionRecoveryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionRecoveryState::Idle => "idle",
            SessionRecoveryState::Recovering => "recovering",
            SessionRecoveryState::Recovered => "recovered",
            SessionRecoveryState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SessionRecoveryState::Recovered | SessionRecoveryState::Failed
        )
    }
}

pub struct SessionRecoveryManager {
    pub state: std::cell::Cell<SessionRecoveryState>,
    pub sessions_recovered: u64,
    pub sessions_failed: u64,
    pub last_recovery_ms: u64,
}

impl SessionRecoveryManager {
    pub fn new() -> Self {
        Self {
            state: std::cell::Cell::new(SessionRecoveryState::Idle),
            sessions_recovered: 0,
            sessions_failed: 0,
            last_recovery_ms: 0,
        }
    }

    pub fn start_recovery(&self) {
        self.state.set(SessionRecoveryState::Recovering);
    }

    pub fn complete_recovery(&self, now_ms: u64, success: bool) {
        if success {
            self.sessions_recovered += 1;
            self.state.set(SessionRecoveryState::Recovered);
        } else {
            self.sessions_failed += 1;
            self.state.set(SessionRecoveryState::Failed);
        }
        self.last_recovery_ms = now_ms;
    }

    pub fn recovery_success_rate(&self) -> f64 {
        let total = self.sessions_recovered + self.sessions_failed;
        if total == 0 {
            return 1.0;
        }
        self.sessions_recovered as f64 / total as f64
    }
}

impl Default for SessionRecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION ID EXTENDED METHODS
///////////////////////////////////////////////////////////////////////////////

impl SessionID {
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn next(&self) -> SessionID {
        SessionID::new(self.0 + 1)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION AFFINITY MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionAffinityManager {
    pub client_region: std::cell::RefCell<std::collections::HashMap<NodeId, RegionId>>,
    pub session_region: std::cell::RefCell<std::collections::HashMap<SessionID, RegionId>>,
}

impl SessionAffinityManager {
    pub fn new() -> Self {
        Self {
            client_region: std::cell::RefCell::new(std::collections::HashMap::new()),
            session_region: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_client_region(&self, client_id: NodeId, region: RegionId) {
        self.client_region.borrow_mut().insert(client_id, region);
    }

    pub fn get_client_region(&self, client_id: NodeId) -> Option<RegionId> {
        self.client_region.borrow().get(&client_id).copied()
    }

    pub fn set_session_region(&self, session_id: SessionID, region: RegionId) {
        self.session_region.borrow_mut().insert(session_id, region);
    }

    pub fn get_session_region(&self, session_id: SessionID) -> Option<RegionId> {
        self.session_region.borrow().get(&session_id).copied()
    }

    pub fn affinity_match(&self, client_id: NodeId, session_id: SessionID) -> bool {
        let client_region = self.get_client_region(client_id);
        let session_region = self.get_session_region(session_id);
        client_region.is_some() && client_region == session_region
    }

    pub fn client_count(&self) -> usize {
        self.client_region.borrow().len()
    }

    pub fn session_count(&self) -> usize {
        self.session_region.borrow().len()
    }
}

impl Default for SessionAffinityManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION LOAD BALANCER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLoadStrategy {
    RoundRobin,
    LeastLoaded,
    Random,
    Affinity,
}

impl SessionLoadStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionLoadStrategy::RoundRobin => "round_robin",
            SessionLoadStrategy::LeastLoaded => "least_loaded",
            SessionLoadStrategy::Random => "random",
            SessionLoadStrategy::Affinity => "affinity",
        }
    }
}

pub struct SessionLoadBalancer {
    pub strategy: SessionLoadStrategy,
    pub node_loads: std::cell::RefCell<std::collections::HashMap<NodeId, u64>>,
    pub round_robin_index: std::cell::Cell<usize>,
    pub nodes: std::cell::RefCell<Vec<NodeId>>,
}

impl SessionLoadBalancer {
    pub fn new(strategy: SessionLoadStrategy) -> Self {
        Self {
            strategy,
            node_loads: std::cell::RefCell::new(std::collections::HashMap::new()),
            round_robin_index: std::cell::Cell::new(0),
            nodes: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn add_node(&self, node: NodeId) {
        self.nodes.borrow_mut().push(node);
        self.node_loads.borrow_mut().insert(node, 0);
    }

    pub fn select_node(&self) -> Option<NodeId> {
        let nodes = self.nodes.borrow();
        if nodes.is_empty() {
            return None;
        }
        match self.strategy {
            SessionLoadStrategy::RoundRobin => {
                let idx = self.round_robin_index.get() % nodes.len();
                self.round_robin_index.set(idx + 1);
                Some(nodes[idx])
            }
            SessionLoadStrategy::LeastLoaded => {
                let loads = self.node_loads.borrow();
                nodes.iter()
                    .min_by_key(|n| loads.get(n).copied().unwrap_or(0))
                    .copied()
            }
            SessionLoadStrategy::Random => {
                // Deterministic pseudo-random selection based on index
                let idx = (self.round_robin_index.get() as u64 * 1103515245 + 12345) % nodes.len() as u64;
                Some(nodes[idx as usize])
            }
            SessionLoadStrategy::Affinity => {
                // Return first available node (affinity handled elsewhere)
                nodes.first().copied()
            }
        }
    }

    pub fn record_load(&self, node: NodeId, load: u64) {
        self.node_loads.borrow_mut().insert(node, load);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.borrow().len()
    }
}

impl Default for SessionLoadBalancer {
    fn default() -> Self {
        Self::new(SessionLoadStrategy::RoundRobin)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STREAM
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct SessionStream {
    pub stream_id: u64,
    pub session_id: SessionID,
    pub endpoint: Endpoint,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

impl SessionStream {
    pub fn new(stream_id: u64, session_id: SessionID, endpoint: Endpoint) -> Self {
        Self {
            stream_id,
            session_id,
            endpoint,
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.messages_sent += 1;
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.messages_received += 1;
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    pub fn total_messages(&self) -> u64 {
        self.messages_sent + self.messages_received
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION STREAM MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionStreamManager {
    pub streams: std::cell::RefCell<std::collections::HashMap<u64, SessionStream>>,
    pub next_stream_id: std::cell::Cell<u64>,
}

impl SessionStreamManager {
    pub fn new() -> Self {
        Self {
            streams: std::cell::RefCell::new(std::collections::HashMap::new()),
            next_stream_id: std::cell::Cell::new(1),
        }
    }

    pub fn create_stream(&self, session_id: SessionID, endpoint: Endpoint) -> u64 {
        let id = self.next_stream_id.get();
        self.next_stream_id.set(id + 1);
        let stream = SessionStream::new(id, session_id, endpoint);
        self.streams.borrow_mut().insert(id, stream);
        id
    }

    pub fn get_stream(&self, stream_id: u64) -> Option<SessionStream> {
        self.streams.borrow().get(&stream_id).cloned()
    }

    pub fn remove_stream(&self, stream_id: u64) -> bool {
        self.streams.borrow_mut().remove(&stream_id).is_some()
    }

    pub fn stream_count(&self) -> usize {
        self.streams.borrow().len()
    }
}

impl Default for SessionStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION TTL MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionTtlManager {
    pub default_ttl_ms: u64,
    pub session_ttls: std::cell::RefCell<std::collections::HashMap<SessionID, u64>>,
    pub expiry_times: std::cell::RefCell<std::collections::HashMap<SessionID, u64>>,
}

impl SessionTtlManager {
    pub fn new(default_ttl_ms: u64) -> Self {
        Self {
            default_ttl_ms,
            session_ttls: std::cell::RefCell::new(std::collections::HashMap::new()),
            expiry_times: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn set_ttl(&self, session_id: SessionID, ttl_ms: u64, now_ms: u64) {
        self.session_ttls.borrow_mut().insert(session_id, ttl_ms);
        self.expiry_times
            .borrow_mut()
            .insert(session_id, now_ms + ttl_ms);
    }

    pub fn is_expired(&self, session_id: SessionID, now_ms: u64) -> bool {
        self.expiry_times
            .borrow()
            .get(&session_id)
            .map(|&expiry| now_ms >= expiry)
            .unwrap_or(true)
    }

    pub fn time_to_expiry(&self, session_id: SessionID, now_ms: u64) -> Option<u64> {
        self.expiry_times
            .borrow()
            .get(&session_id)
            .map(|&expiry| expiry.saturating_sub(now_ms))
    }

    pub fn extend_ttl(&self, session_id: SessionID, additional_ms: u64, now_ms: u64) {
        if let Some(&expiry) = self.expiry_times.borrow().get(&session_id) {
            let new_expiry = expiry + additional_ms;
            self.expiry_times.borrow_mut().insert(session_id, new_expiry);
            let ttl = new_expiry.saturating_sub(now_ms);
            self.session_ttls.borrow_mut().insert(session_id, ttl);
        }
    }

    pub fn default_ttl(&self) -> u64 {
        self.default_ttl_ms
    }
}

impl Default for SessionTtlManager {
    fn default() -> Self {
        Self::new(300000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION FLOW CONTROL
///////////////////////////////////////////////////////////////////////////////

pub struct SessionFlowControl {
    pub window_size: u64,
    pub bytes_in_flight: u64,
    pub credits_available: u64,
    pub max_credits: u64,
}

impl SessionFlowControl {
    pub fn new(window_size: u64, max_credits: u64) -> Self {
        Self {
            window_size,
            bytes_in_flight: 0,
            credits_available: max_credits,
            max_credits,
        }
    }

    pub fn can_send(&self, bytes: u64) -> bool {
        self.bytes_in_flight + bytes <= self.window_size && self.credits_available > 0
    }

    pub fn send(&mut self, bytes: u64) -> bool {
        if self.can_send(bytes) {
            self.bytes_in_flight += bytes;
            self.credits_available = self.credits_available.saturating_sub(1);
            true
        } else {
            false
        }
    }

    pub fn acknowledge(&mut self, bytes: u64) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
        self.credits_available = (self.credits_available + 1).min(self.max_credits);
    }

    pub fn window_utilization(&self) -> f64 {
        if self.window_size == 0 {
            return 0.0;
        }
        self.bytes_in_flight as f64 / self.window_size as f64
    }

    pub fn credit_ratio(&self) -> f64 {
        if self.max_credits == 0 {
            return 0.0;
        }
        self.credits_available as f64 / self.max_credits as f64
    }
}

impl Default for SessionFlowControl {
    fn default() -> Self {
        Self::new(65536, 1000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SESSION MESSAGE SEQUENCER
///////////////////////////////////////////////////////////////////////////////

pub struct SessionMessageSequencer {
    pub next_send_seq: u64,
    pub next_expected_seq: u64,
    pub last_acked_seq: u64,
    pub sent_unacked: std::collections::HashMap<u64, Vec<u8>>,
}

impl SessionMessageSequencer {
    pub fn new() -> Self {
        Self {
            next_send_seq: 0,
            next_expected_seq: 0,
            last_acked_seq: 0,
            sent_unacked: std::collections::HashMap::new(),
        }
    }

    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.next_send_seq;
        self.next_send_seq += 1;
        seq
    }

    pub fn mark_sent(&mut self, seq: u64, data: Vec<u8>) {
        self.sent_unacked.insert(seq, data);
    }

    pub fn mark_acked(&mut self, seq: u64) {
        self.sent_unacked.remove(&seq);
        if seq > self.last_acked_seq {
            self.last_acked_seq = seq;
        }
    }

    pub fn is_acked(&self, seq: u64) -> bool {
        seq <= self.last_acked_seq
    }

    pub fn unacked_count(&self) -> usize {
        self.sent_unacked.len()
    }

    pub fn receive_sequence(&mut self, seq: u64) -> bool {
        if seq == self.next_expected_seq {
            self.next_expected_seq += 1;
            true
        } else if seq < self.next_expected_seq {
            false
        } else {
            false
        }
    }
}

impl Default for SessionMessageSequencer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn session_validate_session_id(session_id: SessionID) -> bool {
    !session_id.is_zero()
}

pub fn session_validate_client_id(client_id: NodeId) -> bool {
    !client_id.0.iter().all(|b| *b == 0)
}

pub fn session_validate_endpoint(endpoint: &Endpoint) -> bool {
    !endpoint.address.is_empty() && endpoint.carrier_type != CarrierType::Loopback || endpoint.address.len() > 0
}

pub const SESSION_PROTOCOL_VERSION: u32 = 1;

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation of Session for testing the trait contract.
    struct TestSession {
        name: &'static str,
        state: std::cell::Cell<SessionState>,
        created: std::cell::Cell<u64>,
        submits: std::cell::Cell<u64>,
        bytes: std::cell::Cell<u64>,
        heartbeats: std::cell::Cell<u64>,
    }

    impl TestSession {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                state: std::cell::Cell::new(SessionState::Active),
                created: std::cell::Cell::new(0),
                submits: std::cell::Cell::new(0),
                bytes: std::cell::Cell::new(0),
                heartbeats: std::cell::Cell::new(0),
            }
        }
    }

    impl Session for TestSession {
        fn create(&self, client_id: NodeId) -> Result<SessionID, SessionError> {
            let _ = client_id;
            self.state.set(SessionState::Active);
            self.created.set(100);
            Ok(SessionID::new(1))
        }

        fn expire(&self, session_id: SessionID) -> Result<(), SessionError> {
            self.state.set(SessionState::Expired);
            Ok(())
        }

        fn heartbeat(&self, session_id: SessionID) -> Result<(), SessionError> {
            self.heartbeats.set(self.heartbeats.get() + 1);
            Ok(())
        }

        fn submit(
            &self,
            session_id: SessionID,
            op: Vec<u8>,
        ) -> Result<OpRef, SessionError> {
            self.submits.set(self.submits.get() + 1);
            self.bytes.set(self.bytes.get() + op.len() as u64);
            Ok(OpRef::new([0u8; 9]))
        }

        fn client_id(&self, session_id: SessionID) -> Result<NodeId, SessionError> {
            Ok(NodeId::new([1; 32]))
        }

        fn region(&self, session_id: SessionID) -> Result<RegionId, SessionError> {
            Ok(RegionId::new([2; 32]))
        }

        fn hunit(&self, session_id: SessionID) -> Result<Option<HUnitId>, SessionError> {
            Ok(Some(HUnitId::new([3; 32])))
        }

        fn created_at_ms(&self, session_id: SessionID) -> Result<u64, SessionError> {
            Ok(self.created.get())
        }

        fn last_heartbeat_ms(&self, session_id: SessionID) -> Result<u64, SessionError> {
            Ok(200)
        }

        fn is_active(&self, session_id: SessionID) -> Result<bool, SessionError> {
            Ok(self.state.get().is_active())
        }

        fn op_count(&self, session_id: SessionID) -> Result<u64, SessionError> {
            Ok(self.submits.get())
        }

        fn byte_count(&self, session_id: SessionID) -> Result<u64, SessionError> {
            Ok(self.bytes.get())
        }

        fn endpoint(&self, session_id: SessionID) -> Result<Option<Endpoint>, SessionError> {
            Ok(None)
        }

        fn session_name(&self) -> &'static str {
            self.name
        }

        fn active_session_count(&self) -> u64 {
            1
        }
    }

    #[test]
    fn session_create_expire_heartbeat() {
        let session = TestSession::new("test");
        let sid = session.create(NodeId::new([1; 32])).unwrap();
        assert_eq!(sid.value(), 1);
        assert!(session.is_active(sid).unwrap());
        session.heartbeat(sid).unwrap();
        assert_eq!(session.heartbeats.get(), 1);
        session.expire(sid).unwrap();
        assert!(!session.is_active(sid).unwrap());
    }

    #[test]
    fn session_submit() {
        let session = TestSession::new("test");
        let sid = session.create(NodeId::new([1; 32])).unwrap();
        let opref = session.submit(sid, vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(opref.value(), [0u8; 9]);
        assert_eq!(session.submits.get(), 1);
        assert_eq!(session.bytes.get(), 5);
    }

    #[test]
    fn session_client_and_region() {
        let session = TestSession::new("test");
        let sid = session.create(NodeId::new([1; 32])).unwrap();
        assert_eq!(session.client_id(sid).unwrap(), NodeId::new([1; 32]));
        assert_eq!(session.region(sid).unwrap(), RegionId::new([2; 32]));
    }

    #[test]
    fn session_hunit_and_timestamps() {
        let session = TestSession::new("test");
        let sid = session.create(NodeId::new([1; 32])).unwrap();
        assert_eq!(
            session.hunit(sid).unwrap(),
            Some(HUnitId::new([3; 32]))
        );
        assert_eq!(session.created_at_ms(sid).unwrap(), 100);
        assert_eq!(session.last_heartbeat_ms(sid).unwrap(), 200);
    }

    #[test]
    fn session_op_and_byte_count() {
        let session = TestSession::new("test");
        let sid = session.create(NodeId::new([1; 32])).unwrap();
        session.submit(sid, vec![1, 2, 3]).unwrap();
        session.submit(sid, vec![4, 5, 6, 7]).unwrap();
        assert_eq!(session.op_count(sid).unwrap(), 2);
        assert_eq!(session.byte_count(sid).unwrap(), 7);
    }

    #[test]
    fn session_name_and_counts() {
        let session = TestSession::new("testsession");
        assert_eq!(session.session_name(), "testsession");
        assert_eq!(session.active_session_count(), 1);
        assert_eq!(session.default_ttl_ms(), 300000);
        assert_eq!(session.max_sessions(), 100000);
    }

    #[test]
    fn session_state_variants() {
        assert_eq!(SessionState::Creating.as_str(), "creating");
        assert_eq!(SessionState::Active.as_str(), "active");
        assert_eq!(SessionState::Expired.as_str(), "expired");
        assert_eq!(SessionState::Failed.as_str(), "failed");
        assert!(SessionState::Active.is_active());
        assert!(!SessionState::Expired.is_active());
        assert!(SessionState::Expired.is_terminal());
        assert!(SessionState::Failed.is_terminal());
        assert!(!SessionState::Active.is_terminal());
    }

    // === SessionInfo tests ===
    #[test]
    fn session_info_new_and_record() {
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let mut info = SessionInfo::new(sid, cid, rid, 100);
        assert_eq!(info.session_id, sid);
        assert_eq!(info.client_id, cid);
        assert_eq!(info.state, SessionState::Active);
        assert_eq!(info.op_count, 0);
        assert_eq!(info.byte_count, 0);

        info.record_op(500);
        assert_eq!(info.op_count, 1);
        assert_eq!(info.byte_count, 500);

        info.record_heartbeat(200);
        assert_eq!(info.last_heartbeat_ms, 200);
    }

    #[test]
    fn session_info_with_options() {
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let info = SessionInfo::new(sid, cid, rid, 100)
            .with_hunit(HUnitId::new([3; 32]))
            .with_endpoint(Endpoint::new(
                crate::types::CarrierType::Loopback,
                vec![1, 2, 3],
            ))
            .unwrap()
            .with_priority(5);
        assert!(info.hunit.is_some());
        assert!(info.endpoint.is_some());
        assert_eq!(info.priority, 5);
    }

    #[test]
    fn session_info_expire_and_age() {
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let mut info = SessionInfo::new(sid, cid, rid, 100);
        assert!(!info.is_idle(150, 100));
        info.expire();
        assert_eq!(info.state, SessionState::Expired);
        assert!(info.is_terminal());
        assert_eq!(info.age_ms(500), 400);
    }

    #[test]
    fn session_info_dataset() {
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let mut info = SessionInfo::new(sid, cid, rid, 100);
        info.associate_dataset(DatasetId::new([1]));
        info.associate_dataset(DatasetId::new([2]));
        assert_eq!(info.datasets.len(), 2);
        info.associate_dataset(DatasetId::new([1]));
        assert_eq!(info.datasets.len(), 2);
    }

    // === SessionManagerState tests ===
    #[test]
    fn session_manager_create_and_get() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let sid = state.create_session(cid, rid, 100).unwrap();
        assert_eq!(sid.value(), 1);
        assert_eq!(state.active_count(), 1);
        let info = state.get_session(sid).unwrap();
        assert_eq!(info.client_id, cid);
        assert_eq!(info.region, rid);
    }

    #[test]
    fn session_manager_client_already_has_session() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        state.create_session(cid, rid, 100).unwrap();
        let err = state.create_session(cid, rid, 200).unwrap_err();
        assert!(matches!(err, SessionError::ClientAlreadyHasSession { .. }));
    }

    #[test]
    fn session_manager_expire() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let sid = state.create_session(cid, rid, 100).unwrap();
        state.expire_session(sid).unwrap();
        assert_eq!(state.active_count(), 0);
        assert!(state.get_session(sid).is_none());
    }

    #[test]
    fn session_manager_heartbeat() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let sid = state.create_session(cid, rid, 100).unwrap();
        state.heartbeat_session(sid, 200).unwrap();
        let info = state.get_session(sid).unwrap();
        assert_eq!(info.last_heartbeat_ms, 200);
    }

    #[test]
    fn session_manager_record_submit() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let sid = state.create_session(cid, rid, 100).unwrap();
        state.record_submit(sid, 500).unwrap();
        state.record_submit(sid, 300).unwrap();
        let info = state.get_session(sid).unwrap();
        assert_eq!(info.op_count, 2);
        assert_eq!(info.byte_count, 800);
        assert_eq!(state.total_submits, 2);
        assert_eq!(state.total_bytes, 800);
    }

    #[test]
    fn session_manager_reset() {
        let state = SessionManagerState::new();
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        state.create_session(cid, rid, 100).unwrap();
        state.reset();
        assert_eq!(state.active_count(), 0);
        assert_eq!(state.total_created, 0);
    }

    // === SessionError tests ===
    #[test]
    fn session_error_category() {
        assert_eq!(
            SessionError::SessionNotFound { session_id: SessionID::new(1) }
                .category(),
            "session_not_found"
        );
        assert_eq!(
            SessionError::SessionExpired { session_id: SessionID::new(1) }
                .category(),
            "session_expired"
        );
        assert_eq!(
            SessionError::SubmitFailed {
                session_id: SessionID::new(1),
                reason: "x".to_string()
            }
            .category(),
            "submit_failed"
        );
        assert_eq!(
            SessionError::HeartbeatFailed {
                session_id: SessionID::new(1),
                reason: "x".to_string()
            }
            .category(),
            "heartbeat_failed"
        );
        assert_eq!(
            SessionError::ClientNotFound {
                client_id: NodeId::new([1; 32])
            }
            .category(),
            "client_not_found"
        );
        assert_eq!(
            SessionError::SessionLimitExceeded { max: 100 }.category(),
            "session_limit_exceeded"
        );
        assert_eq!(
            SessionError::Internal { message: "x".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn session_error_retryable_and_fatal() {
        assert!(SessionError::SubmitFailed {
            session_id: SessionID::new(1),
            reason: "x".to_string()
        }
        .is_retryable());
        assert!(SessionError::Internal { message: "x".to_string() }.is_retryable());
        assert!(!SessionError::SessionNotFound { session_id: SessionID::new(1) }.is_retryable());
        assert!(SessionError::SessionNotFound { session_id: SessionID::new(1) }.is_fatal());
    }

    #[test]
    fn session_error_plain_message() {
        let msg = SessionError::SubmitFailed {
            session_id: SessionID::new(1),
            reason: "timeout".to_string(),
        }
        .plain_message();
        assert!(msg.contains("submit failed"));
        assert!(msg.contains("session=1"));
    }

    #[test]
    fn session_error_session_and_client() {
        assert!(SessionError::SessionExpired { session_id: SessionID::new(1) }.is_session_error());
        assert!(SessionError::InvalidSessionId { session_id: SessionID::new(1) }.is_session_error());
        assert!(SessionError::ClientNotFound { client_id: NodeId::new([1; 32]) }.is_client_error());
        assert!(!SessionError::SubmitFailed {
            session_id: SessionID::new(1),
            reason: "x".to_string()
        }
        .is_session_error());
    }

    #[test]
    fn session_error_suggested_action() {
        assert_eq!(
            SessionError::SessionNotFound { session_id: SessionID::new(1) }
                .suggested_action(),
            "create the session first"
        );
        assert_eq!(
            SessionError::SessionLimitExceeded { max: 100 }.suggested_action(),
            "expire old sessions"
        );
        assert_eq!(
            SessionError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === SessionConfig tests ===
    #[test]
    fn session_config_default_and_validate() {
        let config = SessionConfig::default();
        assert_eq!(config.max_sessions, 100000);
        assert_eq!(config.default_ttl_ms, 300000);
        assert_eq!(config.heartbeat_interval_ms, 30000);
        assert!(config.enable_heartbeat);
        assert!(config.validate().is_ok());

        let mut bad = SessionConfig::default();
        bad.max_sessions = 0;
        assert!(bad.validate().is_err());
        let mut bad2 = SessionConfig::default();
        bad2.default_ttl_ms = 0;
        assert!(bad2.validate().is_err());
    }

    #[test]
    fn session_config_builder() {
        let config = SessionConfig::new()
            .with_max_sessions(50000)
            .with_default_ttl(600000)
            .with_heartbeat_interval(10000);
        assert_eq!(config.max_sessions, 50000);
        assert_eq!(config.default_ttl_ms, 600000);
        assert_eq!(config.heartbeat_interval_ms, 10000);
    }

    // === SessionStats tests ===
    #[test]
    fn session_stats_default_and_rates() {
        let stats = SessionStats::default();
        assert_eq!(stats.active_sessions(), 0);
        assert!((stats.submit_success_rate() - 1.0).abs() < f64::EPSILON);
        assert!((stats.heartbeat_success_rate() - 1.0).abs() < f64::EPSILON);
        assert!((stats.average_bytes_per_submit() - 0.0).abs() < f64::EPSILON);

        let mut stats = stats;
        stats.sessions_created = 100;
        stats.sessions_expired = 20;
        stats.total_submits = 500;
        stats.failed_submits = 10;
        stats.total_bytes = 50000;
        assert_eq!(stats.active_sessions(), 80);
        assert!((stats.submit_success_rate() - (500.0 / 510.0)).abs() < f64::EPSILON);
        assert!((stats.average_bytes_per_submit() - 100.0).abs() < f64::EPSILON);
    }

    // === SessionEndpointBinding tests ===
    #[test]
    fn session_endpoint_binding_bind_unbind() {
        let sid = SessionID::new(1);
        let ep = Endpoint::new(crate::types::CarrierType::Loopback, vec![1]).unwrap();
        let mut binding = SessionEndpointBinding::new(sid);
        assert!(!binding.is_bound());
        binding.bind(ep, 100);
        assert!(binding.is_bound());
        assert_eq!(binding.bound_duration_ms(200), Some(100));
        binding.unbind();
        assert!(!binding.is_bound());
    }

    // === SessionOpTracker tests ===
    #[test]
    fn session_op_tracker_track_and_get() {
        let tracker = SessionOpTracker::new();
        let opref = OpRef::new([1u8; 9]);
        tracker.track_op(opref, 100);
        assert_eq!(tracker.op_count(), 1);
        assert_eq!(tracker.get_op_timestamp(opref), Some(100));
        assert!(tracker.remove_op(opref));
        assert_eq!(tracker.op_count(), 0);
        assert_eq!(tracker.total_tracked(), 1);
    }

    // === SessionPriorityQueue tests ===
    #[test]
    fn session_priority_queue_enqueue_dequeue() {
        let pq = SessionPriorityQueue::new(10);
        assert_eq!(pq.len(), 0);
        assert!(pq.is_empty());
        pq.enqueue(SessionQueueItem {
            priority: 3,
            session_id: SessionID::new(1),
            op_bytes: 100,
        });
        pq.enqueue(SessionQueueItem {
            priority: 1,
            session_id: SessionID::new(2),
            op_bytes: 200,
        });
        pq.enqueue(SessionQueueItem {
            priority: 2,
            session_id: SessionID::new(3),
            op_bytes: 300,
        });
        assert_eq!(pq.len(), 3);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 1);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 2);
        let item = pq.dequeue().unwrap();
        assert_eq!(item.priority, 3);
    }

    #[test]
    fn session_priority_queue_full() {
        let pq = SessionPriorityQueue::new(1);
        pq.enqueue(SessionQueueItem {
            priority: 1,
            session_id: SessionID::new(1),
            op_bytes: 10,
        });
        assert!(pq.is_full());
        assert!(!pq.enqueue(SessionQueueItem {
            priority: 2,
            session_id: SessionID::new(2),
            op_bytes: 20,
        }));
    }

    // === SessionID tests ===
    #[test]
    fn session_id_variants() {
        let sid = SessionID::new(42);
        assert_eq!(sid.value(), 42);
        assert!(!sid.is_zero());
        assert!(SessionID::new(0).is_zero());
    }

    // === SessionWindowManager tests ===
    #[test]
    fn session_window_manager_can_submit() {
        let wm = SessionWindowManager::new();
        assert!(wm.can_submit(100));
        assert!(!wm.can_submit(70000));
    }

    #[test]
    fn session_window_manager_add_remove() {
        let mut wm = SessionWindowManager::new();
        wm.add_in_flight(1000);
        assert_eq!(wm.ops_in_flight, 1000);
        wm.remove_in_flight(500);
        assert_eq!(wm.ops_in_flight, 500);
    }

    #[test]
    fn session_window_manager_increase_decrease() {
        let mut wm = SessionWindowManager::new();
        wm.increase_window(2.0);
        assert_eq!(wm.window_size, 131072);
        wm.decrease_window(0.5);
        assert_eq!(wm.window_size, 65536);
    }

    #[test]
    fn session_window_manager_utilization() {
        let mut wm = SessionWindowManager::new();
        assert!((wm.window_utilization() - 0.0).abs() < f64::EPSILON);
        wm.add_in_flight(32768);
        assert!((wm.window_utilization() - 0.5).abs() < f64::EPSILON);
        assert!(!wm.is_window_full());
        wm.add_in_flight(32768);
        assert!(wm.is_window_full());
    }

    // === SessionRateLimiter tests ===
    #[test]
    fn session_rate_limiter_acquire() {
        let rl = SessionRateLimiter::new(3);
        assert!(rl.try_acquire(0));
        assert!(rl.try_acquire(0));
        assert!(rl.try_acquire(0));
        assert!(!rl.try_acquire(0));
        assert_eq!(rl.throttled_count(), 1);
    }

    #[test]
    fn session_rate_limiter_window_reset() {
        let rl = SessionRateLimiter::new(1);
        assert!(rl.try_acquire(0));
        assert!(!rl.try_acquire(0));
        assert!(rl.try_acquire(1500));
        assert_eq!(rl.current_ops(), 1);
    }

    // === SessionHealthMonitor tests ===
    #[test]
    fn session_health_monitor_record() {
        let mut hm = SessionHealthMonitor::new();
        assert_eq!(hm.health, SessionHealth::Unknown);
        hm.record_success(100);
        assert_eq!(hm.health, SessionHealth::Healthy);
        assert!(hm.is_healthy());
    }

    #[test]
    fn session_health_monitor_unhealthy() {
        let mut hm = SessionHealthMonitor::new();
        for _ in 0..10 {
            hm.record_failure();
        }
        assert_eq!(hm.health, SessionHealth::Unhealthy);
        assert!(hm.is_unhealthy());
    }

    #[test]
    fn session_health_monitor_reset() {
        let mut hm = SessionHealthMonitor::new();
        hm.record_success(100);
        hm.record_failure();
        hm.reset();
        assert_eq!(hm.state, SessionState::Creating);
        assert_eq!(hm.health, SessionHealth::Unknown);
    }

    // === SessionPool tests ===
    #[test]
    fn session_pool_add_remove() {
        let pool = SessionPool::new(2);
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let info = SessionInfo::new(sid, cid, rid, 100);
        assert!(pool.add(info));
        assert_eq!(pool.len(), 1);
        assert!(pool.remove(sid));
        assert_eq!(pool.len(), 0);
        assert!(!pool.remove(SessionID::new(999)));
    }

    #[test]
    fn session_pool_full() {
        let pool = SessionPool::new(1);
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        let rid = RegionId::new([2; 32]);
        let info = SessionInfo::new(sid, cid, rid, 100);
        assert!(pool.add(info));
        assert!(pool.is_full());
        let info2 = SessionInfo::new(SessionID::new(2), cid, rid, 100);
        assert!(!pool.add(info2));
    }

    // === SessionCreditManager tests ===
    #[test]
    fn session_credit_manager_consume() {
        let mut cm = SessionCreditManager::new(10, 100);
        assert!(cm.consume_credit(500));
        assert_eq!(cm.available_credits, 5);
        assert!(!cm.consume_credit(1000));
    }

    #[test]
    fn session_credit_manager_issue() {
        let mut cm = SessionCreditManager::new(10, 100);
        cm.issue_credit(3);
        assert_eq!(cm.available_credits, 13);
    }

    #[test]
    fn session_credit_manager_ratio() {
        let mut cm = SessionCreditManager::new(100, 100);
        assert!((cm.credit_ratio() - 1.0).abs() < f64::EPSILON);
        cm.consume_credit(5000);
        assert!((cm.credit_ratio() - 0.5).abs() < f64::EPSILON);
    }

    // === SessionBackpressureManager tests ===
    #[test]
    fn session_backpressure_manager_throttle() {
        let bpm = SessionBackpressureManager::new(100);
        bpm.update_queue_depth(50);
        assert!(!bpm.is_throttled());
        bpm.update_queue_depth(100);
        assert!(bpm.is_throttled());
        assert!(!bpm.can_submit());
    }

    #[test]
    fn session_backpressure_manager_utilization() {
        let bpm = SessionBackpressureManager::new(1000);
        bpm.update_queue_depth(250);
        assert!((bpm.queue_utilization() - 0.25).abs() < f64::EPSILON);
    }

    // === SessionClock tests ===
    #[test]
    fn session_clock_tick() {
        let mut clock = SessionClock::new();
        assert_eq!(clock.now_ms(), 0);
        clock.tick(100);
        assert_eq!(clock.now_ms(), 100);
        assert_eq!(clock.tick_count(), 1);
    }

    // === SessionEventLogger tests ===
    #[test]
    fn session_event_logger_log() {
        let el = SessionEventLogger::new(100);
        let sid = SessionID::new(1);
        let cid = NodeId::new([1; 32]);
        el.log(SessionEventType::Create, Some(sid), Some(cid), 100, "create");
        el.log(SessionEventType::Submit, None, None, 200, "submit");
        assert_eq!(el.event_count(), 2);
        assert_eq!(el.events_by_type(SessionEventType::Create), 1);
        assert_eq!(el.events_by_type(SessionEventType::Submit), 1);
    }

    #[test]
    fn session_event_logger_clear() {
        let el = SessionEventLogger::new(100);
        el.log(SessionEventType::Create, None, None, 100, "x");
        assert_eq!(el.event_count(), 1);
        el.clear();
        assert_eq!(el.event_count(), 0);
    }

    // === SessionEventType tests ===
    #[test]
    fn session_event_type_as_str() {
        assert_eq!(SessionEventType::Create.as_str(), "create");
        assert_eq!(SessionEventType::Expire.as_str(), "expire");
        assert_eq!(SessionEventType::Heartbeat.as_str(), "heartbeat");
        assert_eq!(SessionEventType::Submit.as_str(), "submit");
        assert_eq!(SessionEventType::AssociateDataset.as_str(), "associate_dataset");
        assert_eq!(SessionEventType::Error.as_str(), "error");
    }

    // === SessionValidator tests ===
    #[test]
    fn session_validator_validate() {
        let mut validator = SessionValidator::new();
        assert!(validator.validate_session_id(SessionID::new(1)));
        assert!(!validator.validate_session_id(SessionID::new(0)));
        assert_eq!(validator.invalid_session_ids, 1);
        assert_eq!(validator.validations, 2);
    }

    #[test]
    fn session_validator_client_id() {
        let mut validator = SessionValidator::new();
        assert!(validator.validate_client_id(NodeId::new([1; 32])));
        assert!(!validator.validate_client_id(NodeId::new([0; 32])));
    }

    #[test]
    fn session_validator_pass_rate() {
        let mut validator = SessionValidator::new();
        validator.validate_session_id(SessionID::new(1));
        validator.validate_session_id(SessionID::new(0));
        assert!((validator.pass_rate() - 0.5).abs() < f64::EPSILON);
    }

    // === SessionStateTracker tests ===
    #[test]
    fn session_state_tracker_record() {
        let tracker = SessionStateTracker::new();
        let sid = SessionID::new(1);
        tracker.record_transition(sid, SessionTransition::Create, 100);
        tracker.record_transition(sid, SessionTransition::Activate, 200);
        assert_eq!(tracker.transition_count(), 2);
        assert_eq!(tracker.total_transitions(), 2);
        let transitions = tracker.transitions_for_session(sid);
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].0, SessionTransition::Create);
    }

    // === SessionQuotaManager tests ===
    #[test]
    fn session_quota_manager_check() {
        let qm = SessionQuotaManager::new(10, 1000);
        let sid = SessionID::new(1);
        assert!(qm.check_quota(sid, 5, 500));
        qm.record_usage(sid, 5, 500);
        assert!(!qm.check_quota(sid, 6, 1));
        assert!(qm.check_quota(sid, 5, 1));
    }

    #[test]
    fn session_quota_manager_reset() {
        let qm = SessionQuotaManager::new(10, 1000);
        let sid = SessionID::new(1);
        qm.record_usage(sid, 5, 500);
        assert!(qm.get_usage(sid).is_some());
        assert!(qm.reset_quota(sid));
        assert!(qm.get_usage(sid).is_none());
    }

    // === SessionRecoveryManager tests ===
    #[test]
    fn session_recovery_manager_complete() {
        let mut rm = SessionRecoveryManager::new();
        assert_eq!(rm.state.get(), SessionRecoveryState::Idle);
        rm.start_recovery();
        assert_eq!(rm.state.get(), SessionRecoveryState::Recovering);
        rm.complete_recovery(1000, true);
        assert_eq!(rm.state.get(), SessionRecoveryState::Recovered);
        assert_eq!(rm.sessions_recovered, 1);
    }

    #[test]
    fn session_recovery_manager_fail() {
        let mut rm = SessionRecoveryManager::new();
        rm.complete_recovery(1000, false);
        assert_eq!(rm.state.get(), SessionRecoveryState::Failed);
        assert_eq!(rm.sessions_failed, 1);
    }

    #[test]
    fn session_recovery_manager_success_rate() {
        let mut rm = SessionRecoveryManager::new();
        rm.complete_recovery(100, true);
        rm.complete_recovery(200, true);
        rm.complete_recovery(300, false);
        assert!((rm.recovery_success_rate() - (2.0 / 3.0)).abs() < f64::EPSILON);
    }

    // === SessionAffinityManager tests ===
    #[test]
    fn session_affinity_manager_match() {
        let am = SessionAffinityManager::new();
        let cid = NodeId::new([1; 32]);
        let sid = SessionID::new(1);
        let rid = RegionId::new([2; 32]);
        am.set_client_region(cid, rid.clone());
        am.set_session_region(sid, rid.clone());
        assert!(am.affinity_match(cid, sid));
        assert_eq!(am.client_count(), 1);
        assert_eq!(am.session_count(), 1);
    }

    // === SessionLoadStrategy tests ===
    #[test]
    fn session_load_strategy_as_str() {
        assert_eq!(SessionLoadStrategy::RoundRobin.as_str(), "round_robin");
        assert_eq!(SessionLoadStrategy::LeastLoaded.as_str(), "least_loaded");
        assert_eq!(SessionLoadStrategy::Random.as_str(), "random");
        assert_eq!(SessionLoadStrategy::Affinity.as_str(), "affinity");
    }

    // === SessionLoadBalancer tests ===
    #[test]
    fn session_load_balancer_select() {
        let lb = SessionLoadBalancer::new(SessionLoadStrategy::RoundRobin);
        lb.add_node(NodeId::new([1; 32]));
        lb.add_node(NodeId::new([2; 32]));
        assert_eq!(lb.node_count(), 2);
        let node1 = lb.select_node().unwrap();
        let node2 = lb.select_node().unwrap();
        assert_ne!(node1, node2);
    }

    // === SessionStream tests ===
    #[test]
    fn session_stream_record() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut stream = SessionStream::new(1, SessionID::new(1), ep.clone());
        stream.record_sent(100);
        stream.record_received(50);
        assert_eq!(stream.bytes_sent, 100);
        assert_eq!(stream.bytes_received, 50);
        assert_eq!(stream.messages_sent, 1);
        assert_eq!(stream.messages_received, 1);
        assert_eq!(stream.total_bytes(), 150);
        assert_eq!(stream.total_messages(), 2);
    }

    // === SessionStreamManager tests ===
    #[test]
    fn session_stream_manager_create() {
        let sm = SessionStreamManager::new();
        let sid = SessionID::new(1);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let id = sm.create_stream(sid, ep);
        assert_eq!(id, 1);
        assert_eq!(sm.stream_count(), 1);
        assert!(sm.get_stream(1).is_some());
        assert!(sm.remove_stream(1));
        assert!(sm.get_stream(1).is_none());
    }

    // === SessionTtlManager tests ===
    #[test]
    fn session_ttl_manager_set_and_expire() {
        let ttm = SessionTtlManager::new(1000);
        let sid = SessionID::new(1);
        ttm.set_ttl(sid, 1000, 100);
        assert!(!ttm.is_expired(sid, 500));
        assert!(ttm.is_expired(sid, 1101));
        assert_eq!(ttm.time_to_expiry(sid, 100), Some(1000));
    }

    #[test]
    fn session_ttl_manager_extend() {
        let ttm = SessionTtlManager::new(1000);
        let sid = SessionID::new(1);
        ttm.set_ttl(sid, 1000, 100);
        ttm.extend_ttl(sid, 500, 100);
        assert_eq!(ttm.time_to_expiry(sid, 100), Some(1500));
    }

    // === SessionFlowControl tests ===
    #[test]
    fn session_flow_control_send_ack() {
        let mut fc = SessionFlowControl::new(1000, 10);
        assert!(fc.can_send(500));
        assert!(fc.send(500));
        assert!(!fc.can_send(600));
        fc.acknowledge(500);
        assert_eq!(fc.bytes_in_flight, 0);
    }

    #[test]
    fn session_flow_control_credit_ratio() {
        let fc = SessionFlowControl::new(1000, 10);
        assert!((fc.credit_ratio() - 1.0).abs() < f64::EPSILON);
    }

    // === SessionMessageSequencer tests ===
    #[test]
    fn session_message_sequencer_next() {
        let mut seq = SessionMessageSequencer::new();
        assert_eq!(seq.next_sequence(), 0);
        assert_eq!(seq.next_sequence(), 1);
        assert_eq!(seq.next_sequence(), 2);
    }

    #[test]
    fn session_message_sequencer_mark_ack() {
        let mut seq = SessionMessageSequencer::new();
        let s1 = seq.next_sequence();
        seq.mark_sent(s1, vec![1, 2, 3]);
        assert_eq!(seq.unacked_count(), 1);
        assert!(!seq.is_acked(s1));
        seq.mark_acked(s1);
        assert_eq!(seq.unacked_count(), 0);
        assert!(seq.is_acked(s1));
    }

    #[test]
    fn session_message_sequencer_receive() {
        let mut seq = SessionMessageSequencer::new();
        assert!(seq.receive_sequence(0));
        assert!(!seq.receive_sequence(0));
        assert!(!seq.receive_sequence(5));
    }

    // === session_validate functions tests ===
    #[test]
    fn session_validate_functions() {
        assert!(session_validate_session_id(SessionID::new(1)));
        assert!(!session_validate_session_id(SessionID::new(0)));
        assert!(session_validate_client_id(NodeId::new([1; 32])));
        assert!(!session_validate_client_id(NodeId::new([0; 32])));
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        assert!(session_validate_endpoint(&ep));
        let ep_empty = Endpoint::new(CarrierType::Loopback, vec![]).unwrap();
        assert!(!session_validate_endpoint(&ep_empty));
    }
}