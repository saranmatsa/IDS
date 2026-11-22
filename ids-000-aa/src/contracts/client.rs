//! # Sealed Trait: Client — THE ONE CALL (ids_submit) and session binding
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Client trait defines THE ONE CALL binding (`ids_submit`) through which
//! all client operations enter the IDS system. It includes session creation,
//! session expiration, session heartbeat, idempotency key management, operation
//! reference generation, op lifecycle tracking, and result handling. There is
//! no global client master — each dataset/region manages its own client context.
//! Implementations live downstream in ids-client and ids-* crates.

use crate::types::{
    AuthToken, CommitState, ConsistencyProfile, DatasetId, DurabilityCondition, Endpoint,
    IdempotencyKey, IdsTime, NodeId, OpRef, OpState, OpType, RecordId, RegionId,
};
use crate::errors::ClientError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Client
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for super::ClientEngine {}
impl private::Sealed for super::ClientSession {}
impl private::Sealed for super::ClientSessionMgr {}
impl private::Sealed for super::IdempotencyManager {}
impl private::Sealed for super::OpRefAllocator {}
impl private::Sealed for super::OpLifecycleTracker {}
impl private::Sealed for super::OpResultMgr {}
impl private::Sealed for super::ClientStats {}
impl private::Sealed for super::ClientMonitor {}
impl private::Sealed for super::ClientValidator {}

// Sealed trait for client — THE ONE CALL binding.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Client: private::Sealed {
    /// THE ONE CALL — submit an operation and receive an OpRef.
    fn ids_submit(
        &self,
        op: crate::types::Operation,
    ) -> Result<OpRef, ClientError>;

    /// Create a client session.
    fn session_create(
        &self,
        client_id: NodeId,
    ) -> Result<crate::types::SessionId, ClientError>;

    /// Expire a client session.
    fn session_expire(
        &self,
        session_id: crate::types::SessionId,
    ) -> Result<(), ClientError>;

    /// Heartbeat a client session to keep it alive.
    fn session_heartbeat(
        &self,
        session_id: crate::types::SessionId,
    ) -> Result<(), ClientError>;

    /// Generate an idempotency key for an operation.
    fn idempotency_key(
        &self,
        op: &crate::types::Operation,
    ) -> IdempotencyKey;

    /// Generate an operation reference.
    fn op_reference(
        &self,
        op: &crate::types::Operation,
    ) -> OpRef;

    /// Get the current state of an operation.
    fn op_state(&self, op_ref: &OpRef) -> Result<OpState, ClientError>;

    /// Get the result of a completed operation.
    fn op_result(&self, op_ref: &OpRef) -> Result<OpResult, ClientError>;

    /// Get client stats.
    fn stats(&self) -> ClientStats;

    /// Get the client name for debugging.
    fn client_name(&self) -> &'static str;

    /// Get total submit count.
    fn submit_count(&self) -> u64;

    /// Get total session count.
    fn session_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ClientError {
    #[error("session not found: {session_id:?}")]
    SessionNotFound { session_id: crate::types::SessionId },

    #[error("session expired: {session_id:?}")]
    SessionExpired { session_id: crate::types::SessionId },

    #[error("invalid operation: {message}")]
    InvalidOperation { message: String },

    #[error("operation not found: {op_ref:?}")]
    OperationNotFound { op_ref: OpRef },

    #[error("idempotency conflict: {op_ref:?}")]
    IdempotencyConflict { op_ref: OpRef },

    #[error("duplicate idempotency key: {key:?}")]
    DuplicateIdempotencyKey { key: IdempotencyKey },

    #[error("submit failed: {message}")]
    SubmitFailed { message: String },

    #[error("client not authorized: {client_id:?}")]
    ClientNotAuthorized { client_id: NodeId },

    #[error("invalid session state: {message}")]
    InvalidSessionState { message: String },

    #[error("session limit reached: {limit}")]
    SessionLimitReached { limit: u64 },

    #[error("invalid client state: {message}")]
    InvalidState { message: String },

    #[error("internal client error: {message}")]
    Internal { message: String },
}

impl ClientError {
    pub fn category(&self) -> &'static str {
        match self {
            ClientError::SessionNotFound { .. } => "session_not_found",
            ClientError::SessionExpired { .. } => "session_expired",
            ClientError::InvalidOperation { .. } => "invalid_operation",
            ClientError::OperationNotFound { .. } => "operation_not_found",
            ClientError::IdempotencyConflict { .. } => "idempotency_conflict",
            ClientError::DuplicateIdempotencyKey { .. } => "duplicate_idempotency_key",
            ClientError::SubmitFailed { .. } => "submit_failed",
            ClientError::ClientNotAuthorized { .. } => "client_not_authorized",
            ClientError::InvalidSessionState { .. } => "invalid_session_state",
            ClientError::SessionLimitReached { .. } => "session_limit_reached",
            ClientError::InvalidState { .. } => "invalid_state",
            ClientError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ClientError::SessionExpired { .. }
                | ClientError::IdempotencyConflict { .. }
                | ClientError::SubmitFailed { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            ClientError::SessionNotFound { .. } => "create session first",
            ClientError::SessionExpired { .. } => "renew or create new session",
            ClientError::InvalidOperation { .. } => "fix operation payload",
            ClientError::OperationNotFound { .. } => "submit operation first",
            ClientError::IdempotencyConflict { .. } => "use different operation",
            ClientError::DuplicateIdempotencyKey { .. } => "use unique idempotency key",
            ClientError::SubmitFailed { .. } => "retry submission",
            ClientError::ClientNotAuthorized { .. } => "authenticate client",
            ClientError::InvalidSessionState { .. } => "reset session state",
            ClientError::SessionLimitReached { .. } => "expire old sessions",
            ClientError::InvalidState { .. } => "reset client state",
            ClientError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT SESSION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSessionState {
    Creating,
    Active,
    Expired,
    Revoked,
}

impl ClientSessionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientSessionState::Creating => "creating",
            ClientSessionState::Active => "active",
            ClientSessionState::Expired => "expired",
            ClientSessionState::Revoked => "revoked",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ClientSessionState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ClientSessionState::Expired | ClientSessionState::Revoked)
    }
}

pub struct ClientSession {
    pub session_id: crate::types::SessionId,
    pub client_id: NodeId,
    pub state: ClientSessionState,
    pub created_at_ms: u64,
    pub last_heartbeat_at_ms: u64,
    pub expires_at_ms: u64,
    pub heartbeat_count: u64,
    pub submit_count: u64,
    pub auth_token: Option<AuthToken>,
}

impl ClientSession {
    pub fn new(
        session_id: crate::types::SessionId,
        client_id: NodeId,
        created_at_ms: u64,
        ttl_ms: u64,
    ) -> Self {
        Self {
            session_id,
            client_id,
            state: ClientSessionState::Creating,
            created_at_ms,
            last_heartbeat_at_ms: created_at_ms,
            expires_at_ms: created_at_ms + ttl_ms,
            heartbeat_count: 0,
            submit_count: 0,
            auth_token: None,
        }
    }

    pub fn activate(&mut self, auth_token: Option<AuthToken>) {
        self.state = ClientSessionState::Active;
        self.auth_token = auth_token;
    }

    pub fn heartbeat(&mut self, at_ms: u64, ttl_ms: u64) {
        self.last_heartbeat_at_ms = at_ms;
        self.expires_at_ms = at_ms + ttl_ms;
        self.heartbeat_count += 1;
    }

    pub fn expire(&mut self) {
        self.state = ClientSessionState::Expired;
    }

    pub fn revoke(&mut self) {
        self.state = ClientSessionState::Revoked;
    }

    pub fn is_valid(&self, at_ms: u64) -> bool {
        self.state == ClientSessionState::Active && at_ms < self.expires_at_ms
    }

    pub fn time_to_expiry(&self, at_ms: u64) -> u64 {
        if at_ms < self.expires_at_ms {
            self.expires_at_ms - at_ms
        } else {
            0
        }
    }

    pub fn state(&self) -> ClientSessionState {
        self.state
    }

    pub fn increment_submit(&mut self) {
        self.submit_count += 1;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT SESSION MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct ClientSessionMgr {
    pub sessions: std::cell::RefCell<std::collections::HashMap<crate::types::SessionId, ClientSession>>,
    pub client_to_session: std::cell::RefCell<std::collections::HashMap<NodeId, crate::types::SessionId>>,
    pub session_counter: u64,
    pub session_count: u64,
    pub expired_session_count: u64,
    pub revoked_session_count: u64,
    pub max_sessions: u64,
}

impl ClientSessionMgr {
    pub fn new(max_sessions: u64) -> Self {
        Self {
            sessions: std::cell::RefCell::new(std::collections::HashMap::new()),
            client_to_session: std::cell::RefCell::new(std::collections::HashMap::new()),
            session_counter: 0,
            session_count: 0,
            expired_session_count: 0,
            revoked_session_count: 0,
            max_sessions,
        }
    }

    pub fn create_session(
        &self,
        client_id: NodeId,
        created_at_ms: u64,
        ttl_ms: u64,
    ) -> Result<crate::types::SessionId, ClientError> {
        let sessions = self.sessions.borrow();
        if sessions.len() as u64 >= self.max_sessions {
            drop(sessions);
            return Err(ClientError::SessionLimitReached { limit: self.max_sessions });
        }
        drop(sessions);

        let mut sessions = self.sessions.borrow_mut();
        let mut client_map = self.client_to_session.borrow_mut();

        // Revoke existing session for this client if any
        if let Some(old_session_id) = client_map.get(&client_id) {
            if let Some(old_session) = sessions.get_mut(old_session_id) {
                old_session.revoke();
                self.revoked_session_count += 1;
            }
        }

        self.session_counter += 1;
        let session_id = crate::types::SessionId::new([
            self.session_counter as u8,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        let mut session = ClientSession::new(session_id, client_id, created_at_ms, ttl_ms);
        session.activate(None);
        sessions.insert(session_id, session);
        client_map.insert(client_id, session_id);
        self.session_count += 1;

        Ok(session_id)
    }

    pub fn get_session(&self, session_id: &crate::types::SessionId) -> Option<ClientSession> {
        self.sessions.borrow().get(session_id).cloned()
    }

    pub fn get_session_for_client(&self, client_id: &NodeId) -> Option<crate::types::SessionId> {
        self.client_to_session.borrow().get(client_id).copied()
    }

    pub fn expire_session(&self, session_id: &crate::types::SessionId) -> Result<(), ClientError> {
        if let Some(session) = self.sessions.borrow_mut().get_mut(session_id) {
            session.expire();
            self.expired_session_count += 1;
            Ok(())
        } else {
            Err(ClientError::SessionNotFound { session_id: *session_id })
        }
    }

    pub fn revoke_session(&self, session_id: &crate::types::SessionId) -> Result<(), ClientError> {
        if let Some(session) = self.sessions.borrow_mut().get_mut(session_id) {
            session.revoke();
            self.revoked_session_count += 1;
            Ok(())
        } else {
            Err(ClientError::SessionNotFound { session_id: *session_id })
        }
    }

    pub fn heartbeat_session(
        &self,
        session_id: &crate::types::SessionId,
        at_ms: u64,
        ttl_ms: u64,
    ) -> Result<(), ClientError> {
        if let Some(session) = self.sessions.borrow_mut().get_mut(session_id) {
            if !session.is_valid(at_ms) {
                return Err(ClientError::SessionExpired { session_id: *session_id });
            }
            session.heartbeat(at_ms, ttl_ms);
            Ok(())
        } else {
            Err(ClientError::SessionNotFound { session_id: *session_id })
        }
    }

    pub fn is_session_valid(
        &self,
        session_id: &crate::types::SessionId,
        at_ms: u64,
    ) -> bool {
        self.sessions
            .borrow()
            .get(session_id)
            .map(|s| s.is_valid(at_ms))
            .unwrap_or(false)
    }

    pub fn session_count(&self) -> u64 {
        self.session_count
    }

    pub fn expired_session_count(&self) -> u64 {
        self.expired_session_count
    }

    pub fn revoked_session_count(&self) -> u64 {
        self.revoked_session_count
    }

    pub fn active_session_count(&self) -> u64 {
        self.sessions
            .borrow()
            .values()
            .filter(|s| s.state.is_active())
            .count() as u64
    }

    pub fn tracked_session_count(&self) -> usize {
        self.sessions.borrow().len()
    }

    pub fn session_limit_reached(&self) -> bool {
        self.sessions.borrow().len() as u64 >= self.max_sessions
    }
}

impl Default for ClientSessionMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// IDEMPOTENCY MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyStatus {
    Pending,
    Processed,
    Completed,
    Failed,
}

impl IdempotencyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdempotencyStatus::Pending => "pending",
            IdempotencyStatus::Processed => "processed",
            IdempotencyStatus::Completed => "completed",
            IdempotencyStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, IdempotencyStatus::Completed | IdempotencyStatus::Failed)
    }
}

pub struct IdempotencyEntry {
    pub key: IdempotencyKey,
    pub op_ref: OpRef,
    pub status: IdempotencyStatus,
    pub created_at_ms: u64,
    pub processed_at_ms: Option<u64>,
    pub result: Option<crate::types::CommitState>,
}

impl IdempotencyEntry {
    pub fn new(
        key: IdempotencyKey,
        op_ref: OpRef,
        created_at_ms: u64,
    ) -> Self {
        Self {
            key,
            op_ref,
            status: IdempotencyStatus::Pending,
            created_at_ms,
            processed_at_ms: None,
            result: None,
        }
    }

    pub fn mark_processed(&mut self, op_ref: OpRef, at_ms: u64) {
        self.status = IdempotencyStatus::Processed;
        self.processed_at_ms = Some(at_ms);
    }

    pub fn mark_completed(&mut self, result: crate::types::CommitState, at_ms: u64) {
        self.status = IdempotencyStatus::Completed;
        self.processed_at_ms = Some(at_ms);
        self.result = Some(result);
    }

    pub fn mark_failed(&mut self, at_ms: u64) {
        self.status = IdempotencyStatus::Failed;
        self.processed_at_ms = Some(at_ms);
    }

    pub fn is_duplicate(&self, key: &IdempotencyKey) -> bool {
        self.key == *key
    }
}

pub struct IdempotencyManager {
    pub entries: std::cell::RefCell<std::collections::HashMap<IdempotencyKey, IdempotencyEntry>>,
    pub op_ref_to_key: std::cell::RefCell<std::collections::HashMap<OpRef, IdempotencyKey>>,
    pub key_count: u64,
    pub duplicate_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub max_entries: u64,
}

impl IdempotencyManager {
    pub fn new(max_entries: u64) -> Self {
        Self {
            entries: std::cell::RefCell::new(std::collections::HashMap::new()),
            op_ref_to_key: std::cell::RefCell::new(std::collections::HashMap::new()),
            key_count: 0,
            duplicate_count: 0,
            completed_count: 0,
            failed_count: 0,
            max_entries,
        }
    }

    pub fn register_key(
        &self,
        key: IdempotencyKey,
        op_ref: OpRef,
        created_at_ms: u64,
    ) -> Result<(), ClientError> {
        let entries = self.entries.borrow();
        if entries.contains_key(&key) {
            drop(entries);
            self.duplicate_count += 1;
            return Err(ClientError::DuplicateIdempotencyKey { key });
        }
        if entries.len() as u64 >= self.max_entries {
            drop(entries);
            return Err(ClientError::InvalidState {
                message: "idempotency table full".to_string(),
            });
        }
        drop(entries);

        let mut entries = self.entries.borrow_mut();
        let mut op_ref_map = self.op_ref_to_key.borrow_mut();

        let entry = IdempotencyEntry::new(key, op_ref, created_at_ms);
        entries.insert(key, entry);
        op_ref_map.insert(op_ref, key);
        self.key_count += 1;

        Ok(())
    }

    pub fn get_entry(&self, key: &IdempotencyKey) -> Option<IdempotencyEntry> {
        self.entries.borrow().get(key).cloned()
    }

    pub fn get_key_for_op_ref(&self, op_ref: &OpRef) -> Option<IdempotencyKey> {
        self.op_ref_to_key.borrow().get(op_ref).copied()
    }

    pub fn check_duplicate(&self, key: &IdempotencyKey) -> bool {
        self.entries.borrow().contains_key(key)
    }

    pub fn mark_completed(
        &self,
        key: &IdempotencyKey,
        result: crate::types::CommitState,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        if let Some(entry) = self.entries.borrow_mut().get_mut(key) {
            entry.mark_completed(result, at_ms);
            self.completed_count += 1;
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "idempotency key not found".to_string(),
            })
        }
    }

    pub fn mark_failed(&self, key: &IdempotencyKey, at_ms: u64) -> Result<(), ClientError> {
        if let Some(entry) = self.entries.borrow_mut().get_mut(key) {
            entry.mark_failed(at_ms);
            self.failed_count += 1;
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "idempotency key not found".to_string(),
            })
        }
    }

    pub fn key_count(&self) -> u64 {
        self.key_count
    }

    pub fn duplicate_count(&self) -> u64 {
        self.duplicate_count
    }

    pub fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn tracked_entry_count(&self) -> usize {
        self.entries.borrow().len()
    }

    pub fn duplicate_rate(&self) -> f64 {
        let total = self.key_count + self.duplicate_count;
        if total == 0 {
            return 0.0;
        }
        self.duplicate_count as f64 / total as f64
    }
}

impl Default for IdempotencyManager {
    fn default() -> Self {
        Self::new(1000000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OP REF ALLOCATOR
///////////////////////////////////////////////////////////////////////////////

pub struct OpRefAllocator {
    pub counter: u64,
    pub allocated_count: u64,
    pub op_refs: std::cell::RefCell<std::collections::HashSet<OpRef>>,
}

impl OpRefAllocator {
    pub fn new() -> Self {
        Self {
            counter: 0,
            allocated_count: 0,
            op_refs: std::cell::RefCell::new(std::collections::HashSet::new()),
        }
    }

    pub fn allocate(&mut self) -> OpRef {
        loop {
            self.counter += 1;
            let bytes = self.counter.to_be_bytes();
            let mut op_ref_bytes = [0u8; 9];
            op_ref_bytes[..8].copy_from_slice(&bytes);
            op_ref_bytes[8] = (self.counter & 0xff) as u8;
            let op_ref = OpRef(op_ref_bytes);
            if self.op_refs.borrow().insert(op_ref) {
                self.allocated_count += 1;
                return op_ref;
            }
        }
    }

    pub fn contains(&self, op_ref: &OpRef) -> bool {
        self.op_refs.borrow().contains(op_ref)
    }

    pub fn allocated_count(&self) -> u64 {
        self.allocated_count
    }

    pub fn tracked_count(&self) -> usize {
        self.op_refs.borrow().len()
    }
}

impl Default for OpRefAllocator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// OP LIFECYCLE TRACKER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpLifecycleStage {
    Received,
    Ordered,
    Executing,
    Committed,
    Synchronized,
    Verified,
    Failed,
}

impl OpLifecycleStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpLifecycleStage::Received => "received",
            OpLifecycleStage::Ordered => "ordered",
            OpLifecycleStage::Executing => "executing",
            OpLifecycleStage::Committed => "committed",
            OpLifecycleStage::Synchronized => "synchronized",
            OpLifecycleStage::Verified => "verified",
            OpLifecycleStage::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, OpLifecycleStage::Verified | OpLifecycleStage::Failed)
    }

    pub fn can_transition_to(&self, next: OpLifecycleStage) -> bool {
        match self {
            OpLifecycleStage::Received => matches!(next, OpLifecycleStage::Ordered | OpLifecycleStage::Failed),
            OpLifecycleStage::Ordered => matches!(next, OpLifecycleStage::Executing | OpLifecycleStage::Failed),
            OpLifecycleStage::Executing => matches!(next, OpLifecycleStage::Committed | OpLifecycleStage::Failed),
            OpLifecycleStage::Committed => matches!(next, OpLifecycleStage::Synchronized | OpLifecycleStage::Failed),
            OpLifecycleStage::Synchronized => matches!(next, OpLifecycleStage::Verified | OpLifecycleStage::Failed),
            OpLifecycleStage::Verified => false,
            OpLifecycleStage::Failed => false,
        }
    }
}

pub struct OpLifecycleEntry {
    pub op_ref: OpRef,
    pub stage: OpLifecycleStage,
    pub dataset_id: DatasetId,
    pub op_type: OpType,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub transition_count: u64,
}

impl OpLifecycleEntry {
    pub fn new(
        op_ref: OpRef,
        dataset_id: DatasetId,
        op_type: OpType,
        at_ms: u64,
    ) -> Self {
        Self {
            op_ref,
            stage: OpLifecycleStage::Received,
            dataset_id,
            op_type,
            created_at_ms: at_ms,
            updated_at_ms: at_ms,
            transition_count: 0,
        }
    }

    pub fn transition_to(&mut self, next: OpLifecycleStage, at_ms: u64) -> Result<(), ClientError> {
        if !self.stage.can_transition_to(next) {
            return Err(ClientError::InvalidState {
                message: format!(
                    "invalid lifecycle transition: {:?} -> {:?}",
                    self.stage, next
                ),
            });
        }
        self.stage = next;
        self.updated_at_ms = at_ms;
        self.transition_count += 1;
        Ok(())
    }

    pub fn is_terminal(&self) -> bool {
        self.stage.is_terminal()
    }
}

pub struct OpLifecycleTracker {
    pub ops: std::cell::RefCell<std::collections::HashMap<OpRef, OpLifecycleEntry>>,
    pub transition_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub max_ops: u64,
}

impl OpLifecycleTracker {
    pub fn new(max_ops: u64) -> Self {
        Self {
            ops: std::cell::RefCell::new(std::collections::HashMap::new()),
            transition_count: 0,
            completed_count: 0,
            failed_count: 0,
            max_ops,
        }
    }

    pub fn register_op(
        &self,
        op_ref: OpRef,
        dataset_id: DatasetId,
        op_type: OpType,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        let ops = self.ops.borrow();
        if ops.contains_key(&op_ref) {
            drop(ops);
            return Err(ClientError::OperationNotFound { op_ref });
        }
        if ops.len() as u64 >= self.max_ops {
            drop(ops);
            return Err(ClientError::InvalidState {
                message: "op lifecycle table full".to_string(),
            });
        }
        drop(ops);

        let mut ops = self.ops.borrow_mut();
        let entry = OpLifecycleEntry::new(op_ref, dataset_id, op_type, at_ms);
        ops.insert(op_ref, entry);
        Ok(())
    }

    pub fn transition_op(
        &self,
        op_ref: &OpRef,
        next: OpLifecycleStage,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        if let Some(entry) = self.ops.borrow_mut().get_mut(op_ref) {
            entry.transition_to(next, at_ms)?;
            self.transition_count += 1;
            match next {
                OpLifecycleStage::Verified => self.completed_count += 1,
                OpLifecycleStage::Failed => self.failed_count += 1,
                _ => {}
            }
            Ok(())
        } else {
            Err(ClientError::OperationNotFound { op_ref: *op_ref })
        }
    }

    pub fn get_stage(&self, op_ref: &OpRef) -> Option<OpLifecycleStage> {
        self.ops.borrow().get(op_ref).map(|e| e.stage)
    }

    pub fn is_terminal(&self, op_ref: &OpRef) -> bool {
        self.ops
            .borrow()
            .get(op_ref)
            .map(|e| e.is_terminal())
            .unwrap_or(false)
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn completed_count(&self) -> u64 {
        self.completed_count
    }

    pub fn failed_count(&self) -> u64 {
        self.failed_count
    }

    pub fn tracked_op_count(&self) -> usize {
        self.ops.borrow().len()
    }

    pub fn completion_rate(&self) -> f64 {
        let total = self.completed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_count as f64 / total as f64
    }
}

impl Default for OpLifecycleTracker {
    fn default() -> Self {
        Self::new(1000000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OP RESULT MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct OpResult {
    pub op_ref: OpRef,
    pub commit_state: CommitState,
    pub dataset_id: DatasetId,
    pub completed_at_ms: u64,
    pub result_payload: Vec<u8>,
}

impl OpResult {
    pub fn new(
        op_ref: OpRef,
        commit_state: CommitState,
        dataset_id: DatasetId,
        completed_at_ms: u64,
        result_payload: Vec<u8>,
    ) -> Self {
        Self {
            op_ref,
            commit_state,
            dataset_id,
            completed_at_ms,
            result_payload,
        }
    }

    pub fn is_committed(&self) -> bool {
        matches!(self.commit_state, CommitState::Committed)
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self.commit_state, CommitState::Aborted)
    }
}

pub struct OpResultMgr {
    pub results: std::cell::RefCell<std::collections::HashMap<OpRef, OpResult>>,
    pub result_count: u64,
    pub committed_count: u64,
    pub aborted_count: u64,
    pub max_results: u64,
}

impl OpResultMgr {
    pub fn new(max_results: u64) -> Self {
        Self {
            results: std::cell::RefCell::new(std::collections::HashMap::new()),
            result_count: 0,
            committed_count: 0,
            aborted_count: 0,
            max_results,
        }
    }

    pub fn record_result(
        &self,
        op_ref: OpRef,
        commit_state: CommitState,
        dataset_id: DatasetId,
        completed_at_ms: u64,
        result_payload: Vec<u8>,
    ) -> Result<(), ClientError> {
        if self.results.borrow().contains_key(&op_ref) {
            return Err(ClientError::InvalidState {
                message: "result already recorded for op_ref".to_string(),
            });
        }
        if self.results.borrow().len() as u64 >= self.max_results {
            return Err(ClientError::InvalidState {
                message: "op result table full".to_string(),
            });
        }

        let result = OpResult::new(op_ref, commit_state, dataset_id, completed_at_ms, result_payload);
        self.results.borrow_mut().insert(op_ref, result);
        self.result_count += 1;
        match commit_state {
            CommitState::Committed => self.committed_count += 1,
            CommitState::Aborted => self.aborted_count += 1,
            _ => {}
        }
        Ok(())
    }

    pub fn get_result(&self, op_ref: &OpRef) -> Option<OpResult> {
        self.results.borrow().get(op_ref).cloned()
    }

    pub fn result_count(&self) -> u64 {
        self.result_count
    }

    pub fn committed_count(&self) -> u64 {
        self.committed_count
    }

    pub fn aborted_count(&self) -> u64 {
        self.aborted_count
    }

    pub fn tracked_result_count(&self) -> usize {
        self.results.borrow().len()
    }

    pub fn commit_rate(&self) -> f64 {
        let total = self.committed_count + self.aborted_count;
        if total == 0 {
            return 1.0;
        }
        self.committed_count as f64 / total as f64
    }
}

impl Default for OpResultMgr {
    fn default() -> Self {
        Self::new(1000000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    pub submit_count: u64,
    pub submit_failure_count: u64,
    pub session_count: u64,
    pub active_session_count: u64,
    pub idempotency_key_count: u64,
    pub duplicate_idempotency_count: u64,
    pub op_count: u64,
    pub completed_op_count: u64,
    pub failed_op_count: u64,
    pub committed_result_count: u64,
    pub aborted_result_count: u64,
}

impl ClientStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit_success_rate(&self) -> f64 {
        let total = self.submit_count + self.submit_failure_count;
        if total == 0 {
            return 1.0;
        }
        self.submit_count as f64 / total as f64
    }

    pub fn op_completion_rate(&self) -> f64 {
        let total = self.completed_op_count + self.failed_op_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_op_count as f64 / total as f64
    }

    pub fn client_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.submit_failure_count as f64 * 0.5;
        score -= self.duplicate_idempotency_count as f64 * 0.1;
        score -= self.failed_op_count as f64 * 1.0;
        score -= self.aborted_result_count as f64 * 1.0;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct ClientMonitor {
    pub client_health: std::cell::RefCell<std::collections::HashMap<NodeId, bool>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub submit_error_count: u64,
}

impl ClientMonitor {
    pub fn new() -> Self {
        Self {
            client_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            submit_error_count: 0,
        }
    }

    pub fn report_health(&self, client_id: NodeId, healthy: bool) {
        let mut health = self.client_health.borrow_mut();
        let prev = health.get(&client_id).copied().unwrap_or(true);
        health.insert(client_id, healthy);
        self.health_check_count += 1;
        if prev && !healthy {
            self.alert_count += 1;
        }
    }

    pub fn report_submit_error(&self) {
        self.submit_error_count += 1;
    }

    pub fn is_client_healthy(&self, client_id: &NodeId) -> bool {
        self.client_health.borrow().get(client_id).copied().unwrap_or(true)
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn submit_error_count(&self) -> u64 {
        self.submit_error_count
    }

    pub fn monitored_client_count(&self) -> usize {
        self.client_health.borrow().len()
    }

    pub fn unhealthy_client_count(&self) -> usize {
        self.client_health.borrow().values().filter(|h| !*h).count()
    }
}

impl Default for ClientMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientValidationResult {
    Valid,
    InvalidOperation,
    InvalidSession,
    InvalidIdempotencyKey,
}

impl ClientValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientValidationResult::Valid => "valid",
            ClientValidationResult::InvalidOperation => "invalid_operation",
            ClientValidationResult::InvalidSession => "invalid_session",
            ClientValidationResult::InvalidIdempotencyKey => "invalid_idempotency_key",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ClientValidationResult::Valid)
    }
}

pub struct ClientValidator {
    pub validations: std::cell::RefCell<std::collections::HashMap<NodeId, ClientValidationResult>>,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl ClientValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_operation(
        &self,
        client_id: NodeId,
        op: &crate::types::Operation,
    ) -> ClientValidationResult {
        self.validation_count += 1;
        let result = if op.payload.is_empty() && op.op_type != OpType::Delete {
            ClientValidationResult::InvalidOperation
        } else if op.dataset_id == DatasetId::new([0u8; 32]) {
            ClientValidationResult::InvalidOperation
        } else {
            ClientValidationResult::Valid
        };
        match result {
            ClientValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(client_id, result);
        result
    }

    pub fn validate_session(
        &self,
        client_id: NodeId,
        session_valid: bool,
    ) -> ClientValidationResult {
        self.validation_count += 1;
        let result = if session_valid {
            ClientValidationResult::Valid
        } else {
            ClientValidationResult::InvalidSession
        };
        match result {
            ClientValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(client_id, result);
        result
    }

    pub fn validate_idempotency_key(
        &self,
        client_id: NodeId,
        key_valid: bool,
    ) -> ClientValidationResult {
        self.validation_count += 1;
        let result = if key_valid {
            ClientValidationResult::Valid
        } else {
            ClientValidationResult::InvalidIdempotencyKey
        };
        match result {
            ClientValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(client_id, result);
        result
    }

    pub fn get_result(&self, client_id: &NodeId) -> Option<ClientValidationResult> {
        self.validations.borrow().get(client_id).copied()
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

impl Default for ClientValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct ClientEngine {
    pub session_mgr: std::cell::RefCell<ClientSessionMgr>,
    pub idempotency_mgr: std::cell::RefCell<IdempotencyManager>,
    pub op_ref_allocator: std::cell::RefCell<OpRefAllocator>,
    pub op_lifecycle: std::cell::RefCell<OpLifecycleTracker>,
    pub op_result_mgr: std::cell::RefCell<OpResultMgr>,
    pub stats: std::cell::RefCell<ClientStats>,
    pub monitor: std::cell::RefCell<ClientMonitor>,
    pub validator: std::cell::RefCell<ClientValidator>,
}

impl ClientEngine {
    pub fn new() -> Self {
        Self {
            session_mgr: std::cell::RefCell::new(ClientSessionMgr::default()),
            idempotency_mgr: std::cell::RefCell::new(IdempotencyManager::default()),
            op_ref_allocator: std::cell::RefCell::new(OpRefAllocator::new()),
            op_lifecycle: std::cell::RefCell::new(OpLifecycleTracker::default()),
            op_result_mgr: std::cell::RefCell::new(OpResultMgr::default()),
            stats: std::cell::RefCell::new(ClientStats::new()),
            monitor: std::cell::RefCell::new(ClientMonitor::new()),
            validator: std::cell::RefCell::new(ClientValidator::new()),
        }
    }

    pub fn ids_submit(
        &self,
        op: crate::types::Operation,
    ) -> Result<OpRef, ClientError> {
        // Validate operation
        self.validator.borrow_mut().validate_operation(op.client_id, &op)?;
        self.stats.borrow_mut().submit_count += 1;

        // Check idempotency
        if self.idempotency_mgr.borrow().check_duplicate(&op.idempotency_key) {
            self.stats.borrow_mut().submit_failure_count += 1;
            self.monitor.borrow_mut().report_submit_error();
            return Err(ClientError::IdempotencyConflict { op_ref: op.op_ref });
        }

        // Register idempotency key
        self.idempotency_mgr.borrow_mut().register_key(
            op.idempotency_key,
            op.op_ref,
            op.ids_time.0,
        )?;

        // Register op lifecycle
        self.op_lifecycle.borrow_mut().register_op(
            op.op_ref,
            op.dataset_id,
            op.op_type,
            op.ids_time.0,
        )?;

        // Transition to ordered
        self.op_lifecycle.borrow_mut().transition_op(
            &op.op_ref,
            OpLifecycleStage::Ordered,
            op.ids_time.0,
        )?;

        // Record result as committed (placeholder — real impl delegates to store)
        self.op_result_mgr.borrow_mut().record_result(
            op.op_ref,
            CommitState::Committed,
            op.dataset_id,
            op.ids_time.0,
            op.payload.clone(),
        )?;

        Ok(op.op_ref)
    }

    pub fn session_create(
        &self,
        client_id: NodeId,
    ) -> Result<crate::types::SessionId, ClientError> {
        let session_id = self.session_mgr.borrow_mut().create_session(
            client_id,
            0,
            3600000,
        )?;
        self.stats.borrow_mut().session_count += 1;
        self.monitor.borrow_mut().report_health(client_id, true);
        Ok(session_id)
    }

    pub fn session_expire(
        &self,
        session_id: crate::types::SessionId,
    ) -> Result<(), ClientError> {
        self.session_mgr.borrow_mut().expire_session(&session_id)?;
        Ok(())
    }

    pub fn session_heartbeat(
        &self,
        session_id: crate::types::SessionId,
    ) -> Result<(), ClientError> {
        self.session_mgr
            .borrow_mut()
            .heartbeat_session(&session_id, 0, 3600000)?;
        Ok(())
    }

    pub fn idempotency_key(
        &self,
        op: &crate::types::Operation,
    ) -> IdempotencyKey {
        op.idempotency_key
    }

    pub fn op_reference(
        &self,
        op: &crate::types::Operation,
    ) -> OpRef {
        op.op_ref
    }

    pub fn op_state(&self, op_ref: &OpRef) -> Result<OpState, ClientError> {
        let stage = self
            .op_lifecycle
            .borrow()
            .get_stage(op_ref)
            .ok_or_else(|| ClientError::OperationNotFound { op_ref: *op_ref })?;
        Ok(match stage {
            OpLifecycleStage::Received => OpState::Received,
            OpLifecycleStage::Ordered => OpState::Ordered,
            OpLifecycleStage::Executing => OpState::Executing,
            OpLifecycleStage::Committed => OpState::Committed,
            OpLifecycleStage::Synchronized => OpState::Synchronized,
            OpLifecycleStage::Verified => OpState::Verified,
            OpLifecycleStage::Failed => OpState::Failed,
        })
    }

    pub fn op_result(&self, op_ref: &OpRef) -> Result<OpResult, ClientError> {
        self.op_result_mgr
            .borrow()
            .get_result(op_ref)
            .ok_or_else(|| ClientError::OperationNotFound { op_ref: *op_ref })
    }

    pub fn stats(&self) -> ClientStats {
        self.stats.borrow().clone()
    }

    pub fn client_name(&self) -> &'static str {
        "client_engine"
    }

    pub fn submit_count(&self) -> u64 {
        self.stats.borrow().submit_count
    }

    pub fn session_count(&self) -> u64 {
        self.stats.borrow().session_count
    }

    pub fn transition_op(
        &self,
        op_ref: &OpRef,
        next: OpLifecycleStage,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        self.op_lifecycle.borrow_mut().transition_op(op_ref, next, at_ms)
    }

    pub fn record_result(
        &self,
        op_ref: OpRef,
        commit_state: CommitState,
        dataset_id: DatasetId,
        completed_at_ms: u64,
        result_payload: Vec<u8>,
    ) -> Result<(), ClientError> {
        self.op_result_mgr
            .borrow_mut()
            .record_result(op_ref, commit_state, dataset_id, completed_at_ms, result_payload)
    }

    pub fn register_idempotency_key(
        &self,
        key: IdempotencyKey,
        op_ref: OpRef,
        created_at_ms: u64,
    ) -> Result<(), ClientError> {
        self.idempotency_mgr
            .borrow_mut()
            .register_key(key, op_ref, created_at_ms)
    }

    pub fn check_idempotency_duplicate(&self, key: &IdempotencyKey) -> bool {
        self.idempotency_mgr.borrow().check_duplicate(key)
    }

    pub fn monitor_client_health(&self, client_id: NodeId, healthy: bool) {
        self.monitor.borrow_mut().report_health(client_id, healthy);
    }

    pub fn is_client_healthy(&self, client_id: &NodeId) -> bool {
        self.monitor.borrow().is_client_healthy(client_id)
    }

    pub fn validate_operation(
        &self,
        client_id: NodeId,
        op: &crate::types::Operation,
    ) -> ClientValidationResult {
        self.validator
            .borrow_mut()
            .validate_operation(client_id, op)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }

    pub fn client_health_score(&self) -> f64 {
        self.stats.borrow().client_health_score()
    }
}

impl Default for ClientEngine {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// OPERATION BATCH SUBMISSION
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct BatchSubmission {
    pub batch_id: OpRef,
    pub operations: Vec<crate::types::Operation>,
    pub submitted_at_ms: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub status: BatchStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    Pending,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
}

impl BatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BatchStatus::Pending => "pending",
            BatchStatus::InProgress => "in_progress",
            BatchStatus::Completed => "completed",
            BatchStatus::PartiallyCompleted => "partially_completed",
            BatchStatus::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            BatchStatus::Completed | BatchStatus::PartiallyCompleted | BatchStatus::Failed
        )
    }
}

impl BatchSubmission {
    pub fn new(
        batch_id: OpRef,
        operations: Vec<crate::types::Operation>,
        submitted_at_ms: u64,
    ) -> Self {
        Self {
            batch_id,
            operations,
            submitted_at_ms,
            completed_count: 0,
            failed_count: 0,
            status: BatchStatus::Pending,
        }
    }

    pub fn mark_in_progress(&mut self) {
        self.status = BatchStatus::InProgress;
    }

    pub fn record_completion(&mut self, success: bool) {
        if success {
            self.completed_count += 1;
        } else {
            self.failed_count += 1;
        }
        if self.completed_count + self.failed_count >= self.operations.len() as u64 {
            if self.failed_count == 0 {
                self.status = BatchStatus::Completed;
            } else if self.completed_count > 0 {
                self.status = BatchStatus::PartiallyCompleted;
            } else {
                self.status = BatchStatus::Failed;
            }
        }
    }

    pub fn total_operations(&self) -> u64 {
        self.operations.len() as u64
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.completed_count + self.failed_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_count as f64 / total as f64
    }
}

pub struct OperationBatchMgr {
    pub batches: std::cell::RefCell<std::collections::HashMap<OpRef, BatchSubmission>>,
    pub batch_count: u64,
    pub completed_batch_count: u64,
    pub failed_batch_count: u64,
    pub partially_completed_count: u64,
    pub max_batches: u64,
}

impl OperationBatchMgr {
    pub fn new(max_batches: u64) -> Self {
        Self {
            batches: std::cell::RefCell::new(std::collections::HashMap::new()),
            batch_count: 0,
            completed_batch_count: 0,
            failed_batch_count: 0,
            partially_completed_count: 0,
            max_batches,
        }
    }

    pub fn create_batch(
        &self,
        batch_id: OpRef,
        operations: Vec<crate::types::Operation>,
        submitted_at_ms: u64,
    ) -> Result<BatchSubmission, ClientError> {
        let batches = self.batches.borrow();
        if batches.contains_key(&batch_id) {
            drop(batches);
            return Err(ClientError::InvalidState {
                message: "batch already exists".to_string(),
            });
        }
        if batches.len() as u64 >= self.max_batches {
            drop(batches);
            return Err(ClientError::InvalidState {
                message: "batch table full".to_string(),
            });
        }
        drop(batches);

        let mut batches = self.batches.borrow_mut();
        let batch = BatchSubmission::new(batch_id, operations, submitted_at_ms);
        batches.insert(batch_id, batch.clone());
        self.batch_count += 1;
        Ok(batch)
    }

    pub fn get_batch(&self, batch_id: &OpRef) -> Option<BatchSubmission> {
        self.batches.borrow().get(batch_id).cloned()
    }

    pub fn record_batch_completion(
        &self,
        batch_id: &OpRef,
        success: bool,
    ) -> Result<(), ClientError> {
        if let Some(batch) = self.batches.borrow_mut().get_mut(batch_id) {
            batch.record_completion(success);
            match batch.status {
                BatchStatus::Completed => self.completed_batch_count += 1,
                BatchStatus::Failed => self.failed_batch_count += 1,
                BatchStatus::PartiallyCompleted => self.partially_completed_count += 1,
                _ => {}
            }
            Ok(())
        } else {
            Err(ClientError::OperationNotFound { op_ref: *batch_id })
        }
    }

    pub fn batch_count(&self) -> u64 {
        self.batch_count
    }

    pub fn completed_batch_count(&self) -> u64 {
        self.completed_batch_count
    }

    pub fn failed_batch_count(&self) -> u64 {
        self.failed_batch_count
    }

    pub fn tracked_batch_count(&self) -> usize {
        self.batches.borrow().len()
    }

    pub fn batch_success_rate(&self) -> f64 {
        let total = self.completed_batch_count + self.failed_batch_count + self.partially_completed_count;
        if total == 0 {
            return 1.0;
        }
        self.completed_batch_count as f64 / total as f64
    }
}

impl Default for OperationBatchMgr {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitAction {
    Allow,
    Delay,
    Reject,
}

impl RateLimitAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RateLimitAction::Allow => "allow",
            RateLimitAction::Delay => "delay",
            RateLimitAction::Reject => "reject",
        }
    }
}

pub struct RateLimitWindow {
    pub client_id: NodeId,
    pub request_count: u64,
    pub window_start_ms: u64,
    pub window_duration_ms: u64,
    pub max_requests: u64,
    pub rejected_count: u64,
}

impl RateLimitWindow {
    pub fn new(
        client_id: NodeId,
        window_start_ms: u64,
        window_duration_ms: u64,
        max_requests: u64,
    ) -> Self {
        Self {
            client_id,
            request_count: 0,
            window_start_ms,
            window_duration_ms,
            max_requests,
            rejected_count: 0,
        }
    }

    pub fn is_within_window(&self, at_ms: u64) -> bool {
        at_ms >= self.window_start_ms && at_ms < self.window_start_ms + self.window_duration_ms
    }

    pub fn check(&mut self, at_ms: u64) -> RateLimitAction {
        if !self.is_within_window(at_ms) {
            // Reset window
            self.request_count = 0;
            self.window_start_ms = at_ms;
        }

        if self.request_count >= self.max_requests {
            self.rejected_count += 1;
            RateLimitAction::Reject
        } else {
            self.request_count += 1;
            RateLimitAction::Allow
        }
    }

    pub fn request_count(&self) -> u64 {
        self.request_count
    }

    pub fn rejected_count(&self) -> u64 {
        self.rejected_count
    }

    pub fn remaining(&self) -> u64 {
        self.max_requests.saturating_sub(self.request_count)
    }
}

pub struct ClientRateLimiter {
    pub windows: std::cell::RefCell<std::collections::HashMap<NodeId, RateLimitWindow>>,
    pub total_requests: u64,
    pub total_rejected: u64,
    pub default_window_ms: u64,
    pub default_max_requests: u64,
}

impl ClientRateLimiter {
    pub fn new(default_window_ms: u64, default_max_requests: u64) -> Self {
        Self {
            windows: std::cell::RefCell::new(std::collections::HashMap::new()),
            total_requests: 0,
            total_rejected: 0,
            default_window_ms,
            default_max_requests,
        }
    }

    pub fn check_rate_limit(
        &self,
        client_id: NodeId,
        at_ms: u64,
    ) -> RateLimitAction {
        let mut windows = self.windows.borrow_mut();
        let window = windows.entry(client_id).or_insert_with(|| {
            RateLimitWindow::new(
                client_id,
                at_ms,
                self.default_window_ms,
                self.default_max_requests,
            )
        });
        let action = window.check(at_ms);
        self.total_requests += 1;
        if action == RateLimitAction::Reject {
            self.total_rejected += 1;
        }
        action
    }

    pub fn get_window(&self, client_id: &NodeId) -> Option<RateLimitWindow> {
        self.windows.borrow().get(client_id).cloned()
    }

    pub fn remove_window(&self, client_id: &NodeId) -> bool {
        self.windows.borrow_mut().remove(client_id).is_some()
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    pub fn total_rejected(&self) -> u64 {
        self.total_rejected
    }

    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_rejected as f64 / self.total_requests as f64
    }

    pub fn tracked_client_count(&self) -> usize {
        self.windows.borrow().len()
    }
}

impl Default for ClientRateLimiter {
    fn default() -> Self {
        Self::new(60000, 10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OPERATION PRIORITY QUEUE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl OpPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpPriority::Low => "low",
            OpPriority::Normal => "normal",
            OpPriority::High => "high",
            OpPriority::Critical => "critical",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            OpPriority::Low => 0,
            OpPriority::Normal => 1,
            OpPriority::High => 2,
            OpPriority::Critical => 3,
        }
    }
}

pub struct QueuedOperation {
    pub op_ref: OpRef,
    pub priority: OpPriority,
    pub enqueued_at_ms: u64,
    pub attempts: u64,
    pub max_attempts: u64,
}

impl QueuedOperation {
    pub fn new(
        op_ref: OpRef,
        priority: OpPriority,
        enqueued_at_ms: u64,
        max_attempts: u64,
    ) -> Self {
        Self {
            op_ref,
            priority,
            enqueued_at_ms,
            attempts: 0,
            max_attempts,
        }
    }

    pub fn increment_attempts(&mut self) {
        self.attempts += 1;
    }

    pub fn has_exceeded_max_attempts(&self) -> bool {
        self.attempts >= self.max_attempts
    }

    pub fn wait_time_ms(&self, at_ms: u64) -> u64 {
        at_ms.saturating_sub(self.enqueued_at_ms)
    }
}

pub struct OperationPriorityQueue {
    pub queues: std::cell::RefCell<
        std::collections::BTreeMap<OpPriority, std::collections::VecDeque<QueuedOperation>>,
    >,
    pub enqueue_count: u64,
    pub dequeue_count: u64,
    pub dropped_count: u64,
    pub max_queue_size: u64,
}

impl OperationPriorityQueue {
    pub fn new(max_queue_size: u64) -> Self {
        let mut queues = std::collections::BTreeMap::new();
        queues.insert(OpPriority::Critical, std::collections::VecDeque::new());
        queues.insert(OpPriority::High, std::collections::VecDeque::new());
        queues.insert(OpPriority::Normal, std::collections::VecDeque::new());
        queues.insert(OpPriority::Low, std::collections::VecDeque::new());
        Self {
            queues: std::cell::RefCell::new(queues),
            enqueue_count: 0,
            dequeue_count: 0,
            dropped_count: 0,
            max_queue_size,
        }
    }

    pub fn enqueue(
        &self,
        op_ref: OpRef,
        priority: OpPriority,
        at_ms: u64,
        max_attempts: u64,
    ) -> Result<(), ClientError> {
        let total_size: usize = self
            .queues
            .borrow()
            .values()
            .map(|q| q.len())
            .sum();
        if total_size as u64 >= self.max_queue_size {
            self.dropped_count += 1;
            return Err(ClientError::InvalidState {
                message: "priority queue full".to_string(),
            });
        }

        let op = QueuedOperation::new(op_ref, priority, at_ms, max_attempts);
        self.queues.borrow_mut().get_mut(&priority).unwrap().push_back(op);
        self.enqueue_count += 1;
        Ok(())
    }

    pub fn dequeue(&self) -> Option<QueuedOperation> {
        let mut queues = self.queues.borrow_mut();
        // Critical first, then high, normal, low
        for priority in [
            OpPriority::Critical,
            OpPriority::High,
            OpPriority::Normal,
            OpPriority::Low,
        ]
        .iter()
        {
            if let Some(op) = queues.get_mut(priority).and_then(|q| q.pop_front()) {
                self.dequeue_count += 1;
                return Some(op);
            }
        }
        None
    }

    pub fn peek(&self, priority: OpPriority) -> Option<QueuedOperation> {
        self.queues
            .borrow()
            .get(&priority)
            .and_then(|q| q.front().cloned())
    }

    pub fn remove_op(&self, op_ref: &OpRef) -> bool {
        let mut queues = self.queues.borrow_mut();
        for queue in queues.values_mut() {
            if let Some(pos) = queue.iter().position(|o| o.op_ref == *op_ref) {
                queue.remove(pos);
                return true;
            }
        }
        false
    }

    pub fn queue_size(&self, priority: OpPriority) -> usize {
        self.queues
            .borrow()
            .get(&priority)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    pub fn total_queue_size(&self) -> usize {
        self.queues
            .borrow()
            .values()
            .map(|q| q.len())
            .sum()
    }

    pub fn enqueue_count(&self) -> u64 {
        self.enqueue_count
    }

    pub fn dequeue_count(&self) -> u64 {
        self.dequeue_count
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }
}

impl Default for OperationPriorityQueue {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CLIENT CONNECTION POOL MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Idle,
    Closing,
    Closed,
    Failed,
}

impl ConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionState::Connecting => "connecting",
            ConnectionState::Connected => "connected",
            ConnectionState::Idle => "idle",
            ConnectionState::Closing => "closing",
            ConnectionState::Closed => "closed",
            ConnectionState::Failed => "failed",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ConnectionState::Connected | ConnectionState::Idle)
    }
}

pub struct ClientConnection {
    pub connection_id: NodeId,
    pub endpoint: Endpoint,
    pub state: ConnectionState,
    pub connected_at_ms: Option<u64>,
    pub last_used_at_ms: Option<u64>,
    pub request_count: u64,
    pub error_count: u64,
    pub max_idle_ms: u64,
}

impl ClientConnection {
    pub fn new(
        connection_id: NodeId,
        endpoint: Endpoint,
        max_idle_ms: u64,
    ) -> Self {
        Self {
            connection_id,
            endpoint,
            state: ConnectionState::Connecting,
            connected_at_ms: None,
            last_used_at_ms: None,
            request_count: 0,
            error_count: 0,
            max_idle_ms,
        }
    }

    pub fn connect(&mut self, at_ms: u64) {
        self.state = ConnectionState::Connected;
        self.connected_at_ms = Some(at_ms);
        self.last_used_at_ms = Some(at_ms);
    }

    pub fn mark_idle(&mut self) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Idle;
        }
    }

    pub fn record_request(&mut self, at_ms: u64) {
        self.request_count += 1;
        self.last_used_at_ms = Some(at_ms);
        if self.state == ConnectionState::Idle {
            self.state = ConnectionState::Connected;
        }
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    pub fn close(&mut self) {
        self.state = ConnectionState::Closed;
    }

    pub fn fail(&mut self) {
        self.state = ConnectionState::Failed;
    }

    pub fn is_idle_too_long(&self, at_ms: u64) -> bool {
        match self.last_used_at_ms {
            Some(last) => at_ms - last > self.max_idle_ms,
            None => false,
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }
}

pub struct ClientConnectionPool {
    pub connections: std::cell::RefCell<std::collections::HashMap<NodeId, ClientConnection>>,
    pub connection_count: u64,
    pub active_connection_count: u64,
    pub closed_connection_count: u64,
    pub failed_connection_count: u64,
    pub max_connections: u64,
}

impl ClientConnectionPool {
    pub fn new(max_connections: u64) -> Self {
        Self {
            connections: std::cell::RefCell::new(std::collections::HashMap::new()),
            connection_count: 0,
            active_connection_count: 0,
            closed_connection_count: 0,
            failed_connection_count: 0,
            max_connections,
        }
    }

    pub fn add_connection(
        &self,
        connection_id: NodeId,
        endpoint: Endpoint,
        max_idle_ms: u64,
    ) -> Result<(), ClientError> {
        let connections = self.connections.borrow();
        if connections.contains_key(&connection_id) {
            drop(connections);
            return Err(ClientError::InvalidState {
                message: "connection already exists".to_string(),
            });
        }
        if connections.len() as u64 >= self.max_connections {
            drop(connections);
            return Err(ClientError::InvalidState {
                message: "connection pool full".to_string(),
            });
        }
        drop(connections);

        let mut connections = self.connections.borrow_mut();
        let conn = ClientConnection::new(connection_id, endpoint, max_idle_ms);
        connections.insert(connection_id, conn);
        self.connection_count += 1;
        Ok(())
    }

    pub fn connect(
        &self,
        connection_id: &NodeId,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        if let Some(conn) = self.connections.borrow_mut().get_mut(connection_id) {
            conn.connect(at_ms);
            self.active_connection_count += 1;
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "connection not found".to_string(),
            })
        }
    }

    pub fn record_request(
        &self,
        connection_id: &NodeId,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        if let Some(conn) = self.connections.borrow_mut().get_mut(connection_id) {
            conn.record_request(at_ms);
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "connection not found".to_string(),
            })
        }
    }

    pub fn record_error(&self, connection_id: &NodeId) -> Result<(), ClientError> {
        if let Some(conn) = self.connections.borrow_mut().get_mut(connection_id) {
            conn.record_error();
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "connection not found".to_string(),
            })
        }
    }

    pub fn close_connection(&self, connection_id: &NodeId) -> Result<(), ClientError> {
        if let Some(conn) = self.connections.borrow_mut().get_mut(connection_id) {
            conn.close();
            self.closed_connection_count += 1;
            self.active_connection_count = self.active_connection_count.saturating_sub(1);
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "connection not found".to_string(),
            })
        }
    }

    pub fn fail_connection(&self, connection_id: &NodeId) -> Result<(), ClientError> {
        if let Some(conn) = self.connections.borrow_mut().get_mut(connection_id) {
            conn.fail();
            self.failed_connection_count += 1;
            self.active_connection_count = self.active_connection_count.saturating_sub(1);
            Ok(())
        } else {
            Err(ClientError::InvalidState {
                message: "connection not found".to_string(),
            })
        }
    }

    pub fn get_connection(&self, connection_id: &NodeId) -> Option<ClientConnection> {
        self.connections.borrow().get(connection_id).cloned()
    }

    pub fn connection_count(&self) -> u64 {
        self.connection_count
    }

    pub fn active_connection_count(&self) -> u64 {
        self.active_connection_count
    }

    pub fn tracked_connection_count(&self) -> usize {
        self.connections.borrow().len()
    }

    pub fn idle_too_long_count(&self, at_ms: u64) -> usize {
        self.connections
            .borrow()
            .values()
            .filter(|c| c.is_idle_too_long(at_ms))
            .count()
    }
}

impl Default for ClientConnectionPool {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// OPERATION TIMEOUT TRACKER
///////////////////////////////////////////////////////////////////////////////

pub struct OperationTimeoutEntry {
    pub op_ref: OpRef,
    pub timeout_at_ms: u64,
    pub expired: bool,
}

impl OperationTimeoutEntry {
    pub fn new(op_ref: OpRef, timeout_at_ms: u64) -> Self {
        Self {
            op_ref,
            timeout_at_ms,
            expired: false,
        }
    }

    pub fn mark_expired(&mut self) {
        self.expired = true;
    }

    pub fn is_expired(&self, at_ms: u64) -> bool {
        self.expired || at_ms >= self.timeout_at_ms
    }
}

pub struct OperationTimeoutTracker {
    pub timeouts: std::cell::RefCell<std::collections::HashMap<OpRef, OperationTimeoutEntry>>,
    pub timeout_count: u64,
    pub expired_count: u64,
    pub cleared_count: u64,
}

impl OperationTimeoutTracker {
    pub fn new() -> Self {
        Self {
            timeouts: std::cell::RefCell::new(std::collections::HashMap::new()),
            timeout_count: 0,
            expired_count: 0,
            cleared_count: 0,
        }
    }

    pub fn register_timeout(&self, op_ref: OpRef, timeout_at_ms: u64) {
        let entry = OperationTimeoutEntry::new(op_ref, timeout_at_ms);
        self.timeouts.borrow_mut().insert(op_ref, entry);
        self.timeout_count += 1;
    }

    pub fn check_expired(&self, at_ms: u64) -> Vec<OpRef> {
        let mut expired = Vec::new();
        let mut timeouts = self.timeouts.borrow_mut();
        for entry in timeouts.values_mut() {
            if entry.is_expired(at_ms) && !entry.expired {
                entry.mark_expired();
                self.expired_count += 1;
                expired.push(entry.op_ref);
            }
        }
        expired
    }

    pub fn clear_timeout(&self, op_ref: &OpRef) -> bool {
        if self.timeouts.borrow_mut().remove(op_ref).is_some() {
            self.cleared_count += 1;
            true
        } else {
            false
        }
    }

    pub fn is_expired(&self, op_ref: &OpRef, at_ms: u64) -> bool {
        self.timeouts
            .borrow()
            .get(op_ref)
            .map(|e| e.is_expired(at_ms))
            .unwrap_or(false)
    }

    pub fn timeout_count(&self) -> u64 {
        self.timeout_count
    }

    pub fn expired_count(&self) -> u64 {
        self.expired_count
    }

    pub fn tracked_timeout_count(&self) -> usize {
        self.timeouts.borrow().len()
    }
}

impl Default for OperationTimeoutTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// EXTENDED CLIENT ENGINE
///////////////////////////////////////////////////////////////////////////////

impl ClientEngine {
    pub fn new_with_features() -> Self {
        Self {
            session_mgr: std::cell::RefCell::new(ClientSessionMgr::default()),
            idempotency_mgr: std::cell::RefCell::new(IdempotencyManager::default()),
            op_ref_allocator: std::cell::RefCell::new(OpRefAllocator::new()),
            op_lifecycle: std::cell::RefCell::new(OpLifecycleTracker::default()),
            op_result_mgr: std::cell::RefCell::new(OpResultMgr::default()),
            stats: std::cell::RefCell::new(ClientStats::new()),
            monitor: std::cell::RefCell::new(ClientMonitor::new()),
            validator: std::cell::RefCell::new(ClientValidator::new()),
            batch_mgr: std::cell::RefCell::new(OperationBatchMgr::default()),
            rate_limiter: std::cell::RefCell::new(ClientRateLimiter::default()),
            priority_queue: std::cell::RefCell::new(OperationPriorityQueue::default()),
            connection_pool: std::cell::RefCell::new(ClientConnectionPool::default()),
            timeout_tracker: std::cell::RefCell::new(OperationTimeoutTracker::new()),
        }
    }

    pub fn submit_batch(
        &self,
        batch_id: OpRef,
        operations: Vec<crate::types::Operation>,
        at_ms: u64,
    ) -> Result<BatchSubmission, ClientError> {
        let batch = self
            .batch_mgr
            .borrow_mut()
            .create_batch(batch_id, operations, at_ms)?;
        Ok(batch)
    }

    pub fn complete_batch_operation(
        &self,
        batch_id: &OpRef,
        success: bool,
    ) -> Result<(), ClientError> {
        self.batch_mgr.borrow_mut().record_batch_completion(batch_id, success)
    }

    pub fn check_rate_limit(
        &self,
        client_id: NodeId,
        at_ms: u64,
    ) -> RateLimitAction {
        self.rate_limiter
            .borrow_mut()
            .check_rate_limit(client_id, at_ms)
    }

    pub fn enqueue_operation(
        &self,
        op_ref: OpRef,
        priority: OpPriority,
        at_ms: u64,
        max_attempts: u64,
    ) -> Result<(), ClientError> {
        self.priority_queue
            .borrow_mut()
            .enqueue(op_ref, priority, at_ms, max_attempts)
    }

    pub fn dequeue_operation(&self) -> Option<QueuedOperation> {
        self.priority_queue.borrow_mut().dequeue()
    }

    pub fn add_connection(
        &self,
        connection_id: NodeId,
        endpoint: Endpoint,
        max_idle_ms: u64,
    ) -> Result<(), ClientError> {
        self.connection_pool
            .borrow_mut()
            .add_connection(connection_id, endpoint, max_idle_ms)
    }

    pub fn connect_connection(
        &self,
        connection_id: &NodeId,
        at_ms: u64,
    ) -> Result<(), ClientError> {
        self.connection_pool.borrow_mut().connect(connection_id, at_ms)
    }

    pub fn register_timeout(
        &self,
        op_ref: OpRef,
        timeout_at_ms: u64,
    ) {
        self.timeout_tracker.borrow_mut().register_timeout(op_ref, timeout_at_ms);
    }

    pub fn check_expired_timeouts(&self, at_ms: u64) -> Vec<OpRef> {
        self.timeout_tracker.borrow_mut().check_expired(at_ms)
    }

    pub fn batch_count(&self) -> u64 {
        self.batch_mgr.borrow().batch_count()
    }

    pub fn rate_limit_rejection_rate(&self) -> f64 {
        self.rate_limiter.borrow().rejection_rate()
    }

    pub fn priority_queue_size(&self) -> usize {
        self.priority_queue.borrow().total_queue_size()
    }

    pub fn connection_count(&self) -> u64 {
        self.connection_pool.borrow().connection_count()
    }

    pub fn active_connection_count(&self) -> u64 {
        self.connection_pool.borrow().active_connection_count()
    }

    pub fn timeout_count(&self) -> u64 {
        self.timeout_tracker.borrow().timeout_count()
    }

    pub fn expired_timeout_count(&self) -> u64 {
        self.timeout_tracker.borrow().expired_count()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client_id() -> NodeId {
        NodeId::new([1; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1; 32])
    }

    fn test_op_type() -> OpType {
        OpType::Write
    }

    fn test_op_ref() -> OpRef {
        OpRef::new([1u8; 9])
    }

    fn test_session_id() -> crate::types::SessionId {
        crate::types::SessionId::new([1; 16])
    }

    fn test_ids_time() -> IdsTime {
        IdsTime::new(1000)
    }

    fn test_operation() -> crate::types::Operation {
        crate::types::Operation {
            op_ref: test_op_ref(),
            dataset_id: test_dataset_id(),
            record_id: Some(RecordId::new([1; 32])),
            op_type: test_op_type(),
            payload: vec![1, 2, 3, 4, 5],
            durability: DurabilityCondition::LocalCommitted,
            consistency: ConsistencyProfile::Strong,
            ids_time: test_ids_time(),
            idempotency_key: IdempotencyKey::new([1; 32]),
            client_id: test_client_id(),
        }
    }

    // === ClientError tests ===
    #[test]
    fn client_error_category() {
        assert_eq!(
            ClientError::SessionNotFound { session_id: test_session_id() }.category(),
            "session_not_found"
        );
        assert_eq!(
            ClientError::SessionExpired { session_id: test_session_id() }.category(),
            "session_expired"
        );
        assert_eq!(
            ClientError::InvalidOperation { message: "bad".to_string() }.category(),
            "invalid_operation"
        );
        assert_eq!(
            ClientError::OperationNotFound { op_ref: test_op_ref() }.category(),
            "operation_not_found"
        );
        assert_eq!(
            ClientError::IdempotencyConflict { op_ref: test_op_ref() }.category(),
            "idempotency_conflict"
        );
        assert_eq!(
            ClientError::DuplicateIdempotencyKey { key: IdempotencyKey::new([1; 32]) }.category(),
            "duplicate_idempotency_key"
        );
        assert_eq!(
            ClientError::SubmitFailed { message: "fail".to_string() }.category(),
            "submit_failed"
        );
        assert_eq!(
            ClientError::ClientNotAuthorized { client_id: test_client_id() }.category(),
            "client_not_authorized"
        );
        assert_eq!(
            ClientError::InvalidSessionState { message: "bad".to_string() }.category(),
            "invalid_session_state"
        );
        assert_eq!(
            ClientError::SessionLimitReached { limit: 100 }.category(),
            "session_limit_reached"
        );
        assert_eq!(
            ClientError::InvalidState { message: "bad".to_string() }.category(),
            "invalid_state"
        );
        assert_eq!(
            ClientError::Internal { message: "panic".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn client_error_is_recoverable() {
        assert!(ClientError::SessionExpired { session_id: test_session_id() }.is_recoverable());
        assert!(ClientError::IdempotencyConflict { op_ref: test_op_ref() }.is_recoverable());
        assert!(ClientError::SubmitFailed { message: "fail".to_string() }.is_recoverable());
        assert!(!ClientError::SessionNotFound { session_id: test_session_id() }.is_recoverable());
        assert!(!ClientError::OperationNotFound { op_ref: test_op_ref() }.is_recoverable());
    }

    #[test]
    fn client_error_suggested_action() {
        assert_eq!(
            ClientError::SessionNotFound { session_id: test_session_id() }.suggested_action(),
            "create session first"
        );
        assert_eq!(
            ClientError::IdempotencyConflict { op_ref: test_op_ref() }.suggested_action(),
            "use different operation"
        );
        assert_eq!(
            ClientError::DuplicateIdempotencyKey { key: IdempotencyKey::new([1; 32]) }.suggested_action(),
            "use unique idempotency key"
        );
        assert_eq!(
            ClientError::Internal { message: "x".to_string() }.suggested_action(),
            "contact support"
        );
    }

    // === ClientSessionState tests ===
    #[test]
    fn client_session_state_as_str() {
        assert_eq!(ClientSessionState::Creating.as_str(), "creating");
        assert_eq!(ClientSessionState::Active.as_str(), "active");
        assert_eq!(ClientSessionState::Expired.as_str(), "expired");
        assert_eq!(ClientSessionState::Revoked.as_str(), "revoked");
        assert!(ClientSessionState::Active.is_active());
        assert!(ClientSessionState::Expired.is_terminal());
        assert!(ClientSessionState::Revoked.is_terminal());
    }

    // === ClientSession tests ===
    #[test]
    fn client_session_activate_and_heartbeat() {
        let mut session = ClientSession::new(
            test_session_id(),
            test_client_id(),
            1000,
            3600000,
        );
        assert_eq!(session.state, ClientSessionState::Creating);
        session.activate(None);
        assert_eq!(session.state, ClientSessionState::Active);
        assert!(session.is_valid(2000));
        session.heartbeat(2000, 3600000);
        assert_eq!(session.heartbeat_count, 1);
        assert_eq!(session.last_heartbeat_at_ms, 2000);
    }

    #[test]
    fn client_session_expire_and_revoke() {
        let mut session = ClientSession::new(test_session_id(), test_client_id(), 1000, 3600000);
        session.activate(None);
        session.expire();
        assert_eq!(session.state, ClientSessionState::Expired);
        assert!(session.state.is_terminal());
        assert!(!session.is_valid(2000));
    }

    #[test]
    fn client_session_time_to_expiry() {
        let session = ClientSession::new(test_session_id(), test_client_id(), 1000, 3600000);
        session.activate(None);
        assert_eq!(session.time_to_expiry(1000), 3600000);
        assert_eq!(session.time_to_expiry(3700000), 0);
    }

    #[test]
    fn client_session_increment_submit() {
        let mut session = ClientSession::new(test_session_id(), test_client_id(), 1000, 3600000);
        session.activate(None);
        session.increment_submit();
        session.increment_submit();
        assert_eq!(session.submit_count, 2);
    }

    // === ClientSessionMgr tests ===
    #[test]
    fn client_session_mgr_create_and_get() {
        let mgr = ClientSessionMgr::default();
        let sid = mgr.create_session(test_client_id(), 1000, 3600000).unwrap();
        assert_eq!(mgr.session_count(), 1);
        assert_eq!(mgr.active_session_count(), 1);
        let session = mgr.get_session(&sid).unwrap();
        assert_eq!(session.client_id, test_client_id());
        assert_eq!(mgr.get_session_for_client(&test_client_id()), Some(sid));
    }

    #[test]
    fn client_session_mgr_session_limit() {
        let mgr = ClientSessionMgr::new(1);
        mgr.create_session(test_client_id(), 1000, 3600000).unwrap();
        let err = mgr.create_session(NodeId::new([2; 32]), 2000, 3600000).unwrap_err();
        assert!(matches!(err, ClientError::SessionLimitReached { .. }));
    }

    #[test]
    fn client_session_mgr_expire_and_revoke() {
        let mgr = ClientSessionMgr::default();
        let sid = mgr.create_session(test_client_id(), 1000, 3600000).unwrap();
        assert!(mgr.expire_session(&sid).is_ok());
        assert_eq!(mgr.expired_session_count(), 1);

        let sid2 = mgr.create_session(NodeId::new([2; 32]), 2000, 3600000).unwrap();
        assert!(mgr.revoke_session(&sid2).is_ok());
        assert_eq!(mgr.revoked_session_count(), 1);
    }

    #[test]
    fn client_session_mgr_heartbeat() {
        let mgr = ClientSessionMgr::default();
        let sid = mgr.create_session(test_client_id(), 1000, 3600000).unwrap();
        assert!(mgr.heartbeat_session(&sid, 2000, 3600000).is_ok());
        let session = mgr.get_session(&sid).unwrap();
        assert_eq!(session.heartbeat_count, 1);
    }

    #[test]
    fn client_session_mgr_expired_session_heartbeat() {
        let mgr = ClientSessionMgr::default();
        let sid = mgr.create_session(test_client_id(), 1000, 100).unwrap();
        // Session expires at 1100, heartbeat at 2000 should fail
        let err = mgr.heartbeat_session(&sid, 2000, 3600000).unwrap_err();
        assert!(matches!(err, ClientError::SessionExpired { .. }));
    }

    #[test]
    fn client_session_mgr_counts() {
        let mgr = ClientSessionMgr::default();
        mgr.create_session(test_client_id(), 1000, 3600000).unwrap();
        mgr.create_session(NodeId::new([2; 32]), 2000, 3600000).unwrap();
        assert_eq!(mgr.session_count(), 2);
        assert_eq!(mgr.active_session_count(), 2);
        assert_eq!(mgr.tracked_session_count(), 2);
    }

    // === IdempotencyStatus tests ===
    #[test]
    fn idempotency_status_as_str() {
        assert_eq!(IdempotencyStatus::Pending.as_str(), "pending");
        assert_eq!(IdempotencyStatus::Processed.as_str(), "processed");
        assert_eq!(IdempotencyStatus::Completed.as_str(), "completed");
        assert_eq!(IdempotencyStatus::Failed.as_str(), "failed");
        assert!(IdempotencyStatus::Completed.is_terminal());
        assert!(IdempotencyStatus::Failed.is_terminal());
        assert!(!IdempotencyStatus::Pending.is_terminal());
    }

    // === IdempotencyEntry tests ===
    #[test]
    fn idempotency_entry_lifecycle() {
        let mut entry = IdempotencyEntry::new(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            1000,
        );
        assert_eq!(entry.status, IdempotencyStatus::Pending);
        entry.mark_processed(test_op_ref(), 2000);
        assert_eq!(entry.status, IdempotencyStatus::Processed);
        entry.mark_completed(CommitState::Committed, 3000);
        assert_eq!(entry.status, IdempotencyStatus::Completed);
        assert_eq!(entry.result, Some(CommitState::Committed));
    }

    #[test]
    fn idempotency_entry_mark_failed() {
        let mut entry = IdempotencyEntry::new(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            1000,
        );
        entry.mark_failed(2000);
        assert_eq!(entry.status, IdempotencyStatus::Failed);
        assert!(entry.status.is_terminal());
    }

    // === IdempotencyManager tests ===
    #[test]
    fn idempotency_manager_register_and_duplicate() {
        let im = IdempotencyManager::default();
        im.register_key(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            1000,
        ).unwrap();
        assert_eq!(im.key_count(), 1);
        assert!(im.check_duplicate(&IdempotencyKey::new([1; 32])));
        let err = im.register_key(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            2000,
        ).unwrap_err();
        assert!(matches!(err, ClientError::DuplicateIdempotencyKey { .. }));
        assert_eq!(im.duplicate_count(), 1);
    }

    #[test]
    fn idempotency_manager_mark_completed() {
        let im = IdempotencyManager::default();
        im.register_key(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            1000,
        ).unwrap();
        im.mark_completed(
            &IdempotencyKey::new([1; 32]),
            CommitState::Committed,
            2000,
        ).unwrap();
        assert_eq!(im.completed_count(), 1);
    }

    #[test]
    fn idempotency_manager_mark_failed() {
        let im = IdempotencyManager::default();
        im.register_key(
            IdempotencyKey::new([1; 32]),
            test_op_ref(),
            1000,
        ).unwrap();
        im.mark_failed(&IdempotencyKey::new([1; 32]), 2000).unwrap();
        assert_eq!(im.failed_count(), 1);
    }

    #[test]
    fn idempotency_manager_duplicate_rate() {
        let im = IdempotencyManager::default();
        im.register_key(IdempotencyKey::new([1; 32]), test_op_ref(), 1000).unwrap();
        let _ = im.register_key(IdempotencyKey::new([1; 32]), test_op_ref(), 2000);
        let rate = im.duplicate_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === OpRefAllocator tests ===
    #[test]
    fn op_ref_allocator_allocate() {
        let mut alloc = OpRefAllocator::new();
        let op_ref1 = alloc.allocate();
        let op_ref2 = alloc.allocate();
        assert_ne!(op_ref1, op_ref2);
        assert_eq!(alloc.allocated_count(), 2);
        assert!(alloc.contains(&op_ref1));
        assert!(alloc.contains(&op_ref2));
    }

    #[test]
    fn op_ref_allocator_unique() {
        let mut alloc = OpRefAllocator::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10000 {
            let op_ref = alloc.allocate();
            assert!(seen.insert(op_ref));
        }
        assert_eq!(alloc.tracked_count(), 10000);
    }

    // === OpLifecycleStage tests ===
    #[test]
    fn op_lifecycle_stage_as_str() {
        assert_eq!(OpLifecycleStage::Received.as_str(), "received");
        assert_eq!(OpLifecycleStage::Ordered.as_str(), "ordered");
        assert_eq!(OpLifecycleStage::Executing.as_str(), "executing");
        assert_eq!(OpLifecycleStage::Committed.as_str(), "committed");
        assert_eq!(OpLifecycleStage::Synchronized.as_str(), "synchronized");
        assert_eq!(OpLifecycleStage::Verified.as_str(), "verified");
        assert_eq!(OpLifecycleStage::Failed.as_str(), "failed");
        assert!(OpLifecycleStage::Verified.is_terminal());
        assert!(OpLifecycleStage::Failed.is_terminal());
        assert!(!OpLifecycleStage::Received.is_terminal());
    }

    #[test]
    fn op_lifecycle_stage_can_transition() {
        assert!(OpLifecycleStage::Received.can_transition_to(OpLifecycleStage::Ordered));
        assert!(!OpLifecycleStage::Received.can_transition_to(OpLifecycleStage::Committed));
        assert!(!OpLifecycleStage::Verified.can_transition_to(OpLifecycleStage::Ordered));
        assert!(!OpLifecycleStage::Failed.can_transition_to(OpLifecycleStage::Executing));
    }

    // === OpLifecycleEntry tests ===
    #[test]
    fn op_lifecycle_entry_transition() {
        let mut entry = OpLifecycleEntry::new(
            test_op_ref(),
            test_dataset_id(),
            OpType::Write,
            1000,
        );
        assert_eq!(entry.stage, OpLifecycleStage::Received);
        entry.transition_to(OpLifecycleStage::Ordered, 2000).unwrap();
        assert_eq!(entry.stage, OpLifecycleStage::Ordered);
        assert_eq!(entry.transition_count, 1);
        entry.transition_to(OpLifecycleStage::Executing, 3000).unwrap();
        assert_eq!(entry.stage, OpLifecycleStage::Executing);
    }

    #[test]
    fn op_lifecycle_entry_invalid_transition() {
        let mut entry = OpLifecycleEntry::new(
            test_op_ref(),
            test_dataset_id(),
            OpType::Write,
            1000,
        );
        let err = entry.transition_to(OpLifecycleStage::Committed, 2000).unwrap_err();
        assert!(matches!(err, ClientError::InvalidState { .. }));
    }

    // === OpLifecycleTracker tests ===
    #[test]
    fn op_lifecycle_tracker_register_and_transition() {
        let tracker = OpLifecycleTracker::default();
        tracker.register_op(test_op_ref(), test_dataset_id(), OpType::Write, 1000).unwrap();
        tracker.transition_op(&test_op_ref(), OpLifecycleStage::Ordered, 2000).unwrap();
        assert_eq!(tracker.transition_count(), 1);
        assert_eq!(tracker.get_stage(&test_op_ref()), Some(OpLifecycleStage::Ordered));
    }

    #[test]
    fn op_lifecycle_tracker_register_duplicate() {
        let tracker = OpLifecycleTracker::default();
        tracker.register_op(test_op_ref(), test_dataset_id(), OpType::Write, 1000).unwrap();
        let err = tracker.register_op(test_op_ref(), test_dataset_id(), OpType::Write, 2000).unwrap_err();
        assert!(matches!(err, ClientError::OperationNotFound { .. }));
    }

    #[test]
    fn op_lifecycle_tracker_transition_not_found() {
        let tracker = OpLifecycleTracker::default();
        let err = tracker.transition_op(&test_op_ref(), OpLifecycleStage::Ordered, 2000).unwrap_err();
        assert!(matches!(err, ClientError::OperationNotFound { .. }));
    }

    #[test]
    fn op_lifecycle_tracker_completion_rate() {
        let tracker = OpLifecycleTracker::default();
        tracker.register_op(OpRef::new([1u8; 9]), test_dataset_id(), OpType::Write, 1000).unwrap();
        tracker.transition_op(&OpRef::new([1u8; 9]), OpLifecycleStage::Verified, 2000).unwrap();
        tracker.register_op(OpRef::new([2u8; 9]), test_dataset_id(), OpType::Write, 1000).unwrap();
        tracker.transition_op(&OpRef::new([2u8; 9]), OpLifecycleStage::Failed, 2000).unwrap();
        assert!((tracker.completion_rate() - 0.5).abs() < 0.01);
    }

    // === OpResult tests ===
    #[test]
    fn op_result_committed_and_aborted() {
        let result = OpResult::new(
            test_op_ref(),
            CommitState::Committed,
            test_dataset_id(),
            1000,
            vec![1, 2, 3],
        );
        assert!(result.is_committed());
        assert!(!result.is_aborted());

        let result2 = OpResult::new(
            test_op_ref(),
            CommitState::Aborted,
            test_dataset_id(),
            1000,
            vec![],
        );
        assert!(!result2.is_committed());
        assert!(result2.is_aborted());
    }

    // === OpResultMgr tests ===
    #[test]
    fn op_result_mgr_record_and_get() {
        let mgr = OpResultMgr::default();
        mgr.record_result(
            test_op_ref(),
            CommitState::Committed,
            test_dataset_id(),
            1000,
            vec![1, 2, 3],
        ).unwrap();
        assert_eq!(mgr.result_count(), 1);
        assert_eq!(mgr.committed_count(), 1);
        let result = mgr.get_result(&test_op_ref()).unwrap();
        assert!(result.is_committed());
    }

    #[test]
    fn op_result_mgr_duplicate_result() {
        let mgr = OpResultMgr::default();
        mgr.record_result(
            test_op_ref(),
            CommitState::Committed,
            test_dataset_id(),
            1000,
            vec![1, 2, 3],
        ).unwrap();
        let err = mgr.record_result(
            test_op_ref(),
            CommitState::Aborted,
            test_dataset_id(),
            2000,
            vec![],
        ).unwrap_err();
        assert!(matches!(err, ClientError::InvalidState { .. }));
    }

    #[test]
    fn op_result_mgr_commit_rate() {
        let mgr = OpResultMgr::default();
        mgr.record_result(
            OpRef::new([1u8; 9]),
            CommitState::Committed,
            test_dataset_id(),
            1000,
            vec![],
        ).unwrap();
        mgr.record_result(
            OpRef::new([2u8; 9]),
            CommitState::Aborted,
            test_dataset_id(),
            2000,
            vec![],
        ).unwrap();
        assert!((mgr.commit_rate() - 0.5).abs() < 0.01);
    }

    // === ClientStats tests ===
    #[test]
    fn client_stats_submit_success_rate() {
        let mut s = ClientStats::default();
        s.submit_count = 90;
        s.submit_failure_count = 10;
        assert!((s.submit_success_rate() - 0.9).abs() < 0.01);
    }

    #[test]
    fn client_stats_op_completion_rate() {
        let mut s = ClientStats::default();
        s.completed_op_count = 80;
        s.failed_op_count = 20;
        assert!((s.op_completion_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn client_stats_health_score() {
        let mut s = ClientStats::default();
        s.submit_count = 100;
        s.submit_failure_count = 0;
        s.failed_op_count = 0;
        s.aborted_result_count = 0;
        assert_eq!(s.client_health_score(), 100.0);
    }

    // === ClientMonitor tests ===
    #[test]
    fn client_monitor_report_health() {
        let cm = ClientMonitor::default();
        cm.report_health(test_client_id(), true);
        assert!(cm.is_client_healthy(&test_client_id()));
        assert_eq!(cm.health_check_count(), 1);
    }

    #[test]
    fn client_monitor_submit_error() {
        let cm = ClientMonitor::default();
        cm.report_submit_error();
        cm.report_submit_error();
        assert_eq!(cm.submit_error_count(), 2);
    }

    // === ClientValidationResult tests ===
    #[test]
    fn client_validation_result_as_str() {
        assert_eq!(ClientValidationResult::Valid.as_str(), "valid");
        assert_eq!(ClientValidationResult::InvalidOperation.as_str(), "invalid_operation");
        assert_eq!(ClientValidationResult::InvalidSession.as_str(), "invalid_session");
        assert_eq!(ClientValidationResult::InvalidIdempotencyKey.as_str(), "invalid_idempotency_key");
        assert!(ClientValidationResult::Valid.is_ok());
        assert!(!ClientValidationResult::InvalidOperation.is_ok());
    }

    // === ClientValidator tests ===
    #[test]
    fn client_validator_validate_operation() {
        let cv = ClientValidator::default();
        let good_op = test_operation();
        assert_eq!(
            cv.validate_operation(test_client_id(), &good_op),
            ClientValidationResult::Valid
        );
        let bad_op = crate::types::Operation {
            op_ref: test_op_ref(),
            dataset_id: DatasetId::new([0; 32]),
            record_id: Some(RecordId::new([1; 32])),
            op_type: OpType::Write,
            payload: vec![],
            durability: DurabilityCondition::LocalCommitted,
            consistency: ConsistencyProfile::Strong,
            ids_time: test_ids_time(),
            idempotency_key: IdempotencyKey::new([1; 32]),
            client_id: test_client_id(),
        };
        assert_eq!(
            cv.validate_operation(test_client_id(), &bad_op),
            ClientValidationResult::InvalidOperation
        );
    }

    #[test]
    fn client_validator_validate_session() {
        let cv = ClientValidator::default();
        assert_eq!(
            cv.validate_session(test_client_id(), true),
            ClientValidationResult::Valid
        );
        assert_eq!(
            cv.validate_session(test_client_id(), false),
            ClientValidationResult::InvalidSession
        );
    }

    #[test]
    fn client_validator_valid_rate() {
        let cv = ClientValidator::default();
        cv.validate_operation(test_client_id(), &test_operation());
        cv.validate_session(test_client_id(), false);
        assert!((cv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === ClientEngine tests ===
    #[test]
    fn client_engine_ids_submit() {
        let engine = ClientEngine::default();
        let op_ref = engine.ids_submit(test_operation()).unwrap();
        assert_eq!(op_ref, test_op_ref());
        assert_eq!(engine.submit_count(), 1);
    }

    #[test]
    fn client_engine_ids_submit_duplicate_idempotency() {
        let engine = ClientEngine::default();
        let op = test_operation();
        engine.ids_submit(op.clone()).unwrap();
        let err = engine.ids_submit(op).unwrap_err();
        assert!(matches!(err, ClientError::IdempotencyConflict { .. }));
    }

    #[test]
    fn client_engine_ids_submit_empty_payload_write() {
        let engine = ClientEngine::default();
        let mut bad_op = test_operation();
        bad_op.payload = vec![];
        let err = engine.ids_submit(bad_op).unwrap_err();
        assert!(matches!(err, ClientError::InvalidOperation { .. }));
    }

    #[test]
    fn client_engine_session_create_and_expire() {
        let engine = ClientEngine::default();
        let sid = engine.session_create(test_client_id()).unwrap();
        assert_eq!(engine.session_count(), 1);
        assert!(engine.session_expire(sid).is_ok());
    }

    #[test]
    fn client_engine_session_heartbeat() {
        let engine = ClientEngine::default();
        let sid = engine.session_create(test_client_id()).unwrap();
        assert!(engine.session_heartbeat(sid).is_ok());
    }

    #[test]
    fn client_engine_op_state() {
        let engine = ClientEngine::default();
        let op_ref = engine.ids_submit(test_operation()).unwrap();
        let state = engine.op_state(&op_ref).unwrap();
        assert_eq!(state, OpState::Ordered);
    }

    #[test]
    fn client_engine_op_state_not_found() {
        let engine = ClientEngine::default();
        let err = engine.op_state(&test_op_ref()).unwrap_err();
        assert!(matches!(err, ClientError::OperationNotFound { .. }));
    }

    #[test]
    fn client_engine_op_result() {
        let engine = ClientEngine::default();
        let op_ref = engine.ids_submit(test_operation()).unwrap();
        let result = engine.op_result(&op_ref).unwrap();
        assert!(result.is_committed());
    }

    #[test]
    fn client_engine_transition_op() {
        let engine = ClientEngine::default();
        let op_ref = engine.ids_submit(test_operation()).unwrap();
        engine.transition_op(&op_ref, OpLifecycleStage::Executing, 2000).unwrap();
        assert_eq!(
            engine.op_state(&op_ref).unwrap(),
            OpState::Executing
        );
    }

    #[test]
    fn client_engine_record_result() {
        let engine = ClientEngine::default();
        engine.record_result(
            test_op_ref(),
            CommitState::Committed,
            test_dataset_id(),
            1000,
            vec![1, 2, 3],
        ).unwrap();
        let result = engine.op_result(&test_op_ref()).unwrap();
        assert!(result.is_committed());
    }

    #[test]
    fn client_engine_stats_and_name() {
        let engine = ClientEngine::default();
        assert_eq!(engine.client_name(), "client_engine");
        assert_eq!(engine.submit_count(), 0);
        assert_eq!(engine.session_count(), 0);
    }

    #[test]
    fn client_engine_monitor_health() {
        let engine = ClientEngine::default();
        engine.monitor_client_health(test_client_id(), false);
        assert!(!engine.is_client_healthy(&test_client_id()));
    }

    #[test]
    fn client_engine_validate_operation() {
        let engine = ClientEngine::default();
        assert_eq!(
            engine.validate_operation(test_client_id(), &test_operation()),
            ClientValidationResult::Valid
        );
    }

    #[test]
    fn client_engine_valid_rate() {
        let engine = ClientEngine::default();
        engine.validate_operation(test_client_id(), &test_operation());
        engine.validate_session(test_client_id(), false);
        assert!((engine.valid_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn client_engine_client_health_score() {
        let engine = ClientEngine::default();
        engine.ids_submit(test_operation()).unwrap();
        let score = engine.client_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full client flow ===
    #[test]
    fn client_full_flow() {
        let engine = ClientEngine::default();

        // Create session
        let sid = engine.session_create(test_client_id()).unwrap();
        assert_eq!(engine.session_count(), 1);

        // Submit operation
        let op_ref = engine.ids_submit(test_operation()).unwrap();
        assert_eq!(engine.submit_count(), 1);

        // Check op state
        assert_eq!(engine.op_state(&op_ref).unwrap(), OpState::Ordered);

        // Get result
        let result = engine.op_result(&op_ref).unwrap();
        assert!(result.is_committed());

        // Transition to verified
        engine.transition_op(&op_ref, OpLifecycleStage::Executing, 2000).unwrap();
        engine.transition_op(&op_ref, OpLifecycleStage::Committed, 3000).unwrap();
        engine.transition_op(&op_ref, OpLifecycleStage::Synchronized, 4000).unwrap();
        engine.transition_op(&op_ref, OpLifecycleStage::Verified, 5000).unwrap();
        assert_eq!(engine.op_state(&op_ref).unwrap(), OpState::Verified);

        // Heartbeat session
        engine.session_heartbeat(sid).unwrap();

        // Check idempotency duplicate
        assert!(engine.check_idempotency_duplicate(&IdempotencyKey::new([1; 32])));

        // Stats
        let s = engine.stats();
        assert!(s.submit_count >= 1);
        assert!(s.session_count >= 1);

        // Health score
        let score = engine.client_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);

        // Expire session
        engine.session_expire(sid).unwrap();
    }

    // === BatchSubmission tests ===
    #[test]
    fn batch_submission_new_and_status() {
        let op1 = test_operation();
        let op2 = crate::types::Operation {
            op_ref: OpRef::new([2u8; 9]),
            dataset_id: test_dataset_id(),
            record_id: Some(RecordId::new([2; 32])),
            op_type: OpType::Read,
            payload: vec![6, 7, 8],
            durability: DurabilityCondition::Received,
            consistency: ConsistencyProfile::Eventual,
            ids_time: test_ids_time(),
            idempotency_key: IdempotencyKey::new([2; 32]),
            client_id: test_client_id(),
        };
        let batch = BatchSubmission::new(
            test_op_ref(),
            vec![op1, op2],
            1000,
        );
        assert_eq!(batch.status, BatchStatus::Pending);
        assert_eq!(batch.total_operations(), 2);
        assert_eq!(batch.success_rate(), 1.0);
    }

    #[test]
    fn batch_submission_record_completion() {
        let mut batch = BatchSubmission::new(
            test_op_ref(),
            vec![test_operation(), test_operation()],
            1000,
        );
        batch.mark_in_progress();
        assert_eq!(batch.status, BatchStatus::InProgress);
        batch.record_completion(true);
        assert_eq!(batch.completed_count, 1);
        assert_eq!(batch.status, BatchStatus::Pending);
        batch.record_completion(true);
        assert_eq!(batch.status, BatchStatus::Completed);
        assert!((batch.success_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn batch_submission_partial_completion() {
        let mut batch = BatchSubmission::new(
            test_op_ref(),
            vec![test_operation(), test_operation(), test_operation()],
            1000,
        );
        batch.record_completion(true);
        batch.record_completion(false);
        batch.record_completion(false);
        assert_eq!(batch.status, BatchStatus::PartiallyCompleted);
        assert!((batch.success_rate() - (1.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn batch_status_as_str_and_terminal() {
        assert_eq!(BatchStatus::Pending.as_str(), "pending");
        assert_eq!(BatchStatus::InProgress.as_str(), "in_progress");
        assert_eq!(BatchStatus::Completed.as_str(), "completed");
        assert_eq!(BatchStatus::PartiallyCompleted.as_str(), "partially_completed");
        assert_eq!(BatchStatus::Failed.as_str(), "failed");
        assert!(BatchStatus::Completed.is_terminal());
        assert!(BatchStatus::Failed.is_terminal());
        assert!(!BatchStatus::Pending.is_terminal());
        assert!(!BatchStatus::InProgress.is_terminal());
    }

    // === OperationBatchMgr tests ===
    #[test]
    fn operation_batch_mgr_create_and_get() {
        let mgr = OperationBatchMgr::default();
        let ops = vec![test_operation(), test_operation()];
        let batch = mgr.create_batch(test_op_ref(), ops, 1000).unwrap();
        assert_eq!(mgr.batch_count(), 1);
        assert_eq!(mgr.tracked_batch_count(), 1);
        let retrieved = mgr.get_batch(&test_op_ref()).unwrap();
        assert_eq!(retrieved.batch_id, test_op_ref());
    }

    #[test]
    fn operation_batch_mgr_duplicate_batch() {
        let mgr = OperationBatchMgr::default();
        mgr.create_batch(test_op_ref(), vec![test_operation()], 1000).unwrap();
        let err = mgr.create_batch(test_op_ref(), vec![test_operation()], 2000).unwrap_err();
        assert!(matches!(err, ClientError::InvalidState { .. }));
    }

    #[test]
    fn operation_batch_mgr_record_completion() {
        let mgr = OperationBatchMgr::default();
        mgr.create_batch(test_op_ref(), vec![test_operation()], 1000).unwrap();
        mgr.record_batch_completion(&test_op_ref(), true).unwrap();
        assert_eq!(mgr.completed_batch_count(), 1);
        let retrieved = mgr.get_batch(&test_op_ref()).unwrap();
        assert_eq!(retrieved.status, BatchStatus::Completed);
    }

    #[test]
    fn operation_batch_mgr_batch_not_found() {
        let mgr = OperationBatchMgr::default();
        let err = mgr.record_batch_completion(&test_op_ref(), true).unwrap_err();
        assert!(matches!(err, ClientError::OperationNotFound { .. }));
    }

    #[test]
    fn operation_batch_mgr_success_rate() {
        let mgr = OperationBatchMgr::default();
        mgr.create_batch(OpRef::new([1u8; 9]), vec![test_operation()], 1000).unwrap();
        mgr.record_batch_completion(&OpRef::new([1u8; 9]), true).unwrap();
        mgr.create_batch(OpRef::new([2u8; 9]), vec![test_operation()], 2000).unwrap();
        mgr.record_batch_completion(&OpRef::new([2u8; 9]), false).unwrap();
        assert!((mgr.batch_success_rate() - 0.5).abs() < 0.01);
    }

    // === RateLimitAction tests ===
    #[test]
    fn rate_limit_action_as_str() {
        assert_eq!(RateLimitAction::Allow.as_str(), "allow");
        assert_eq!(RateLimitAction::Delay.as_str(), "delay");
        assert_eq!(RateLimitAction::Reject.as_str(), "reject");
    }

    // === RateLimitWindow tests ===
    #[test]
    fn rate_limit_window_allow_and_reject() {
        let mut window = RateLimitWindow::new(test_client_id(), 0, 60000, 3);
        assert_eq!(window.check(1000), RateLimitAction::Allow);
        assert_eq!(window.check(2000), RateLimitAction::Allow);
        assert_eq!(window.check(3000), RateLimitAction::Allow);
        assert_eq!(window.check(4000), RateLimitAction::Reject);
        assert_eq!(window.request_count(), 3);
        assert_eq!(window.rejected_count(), 1);
        assert_eq!(window.remaining(), 0);
    }

    #[test]
    fn rate_limit_window_reset() {
        let mut window = RateLimitWindow::new(test_client_id(), 0, 1000, 2);
        window.check(500);
        window.check(600);
        assert_eq!(window.check(700), RateLimitAction::Reject);
        // New window after 1000ms
        assert_eq!(window.check(1500), RateLimitAction::Allow);
        assert_eq!(window.request_count(), 1);
    }

    // === ClientRateLimiter tests ===
    #[test]
    fn client_rate_limiter_check() {
        let rl = ClientRateLimiter::default();
        assert_eq!(rl.check_rate_limit(test_client_id(), 1000), RateLimitAction::Allow);
        assert_eq!(rl.total_requests(), 1);
        assert_eq!(rl.total_rejected(), 0);
    }

    #[test]
    fn client_rate_limiter_rejection_rate() {
        let rl = ClientRateLimiter::new(60000, 2);
        rl.check_rate_limit(test_client_id(), 1000);
        rl.check_rate_limit(test_client_id(), 2000);
        assert_eq!(rl.check_rate_limit(test_client_id(), 3000), RateLimitAction::Reject);
        let rate = rl.rejection_rate();
        assert!((rate - (1.0 / 3.0)).abs() < 0.01);
    }

    #[test]
    fn client_rate_limiter_window() {
        let rl = ClientRateLimiter::default();
        let window = rl.get_window(&test_client_id());
        assert!(window.is_none());
        rl.check_rate_limit(test_client_id(), 1000);
        let window = rl.get_window(&test_client_id()).unwrap();
        assert_eq!(window.request_count(), 1);
        assert!(rl.remove_window(&test_client_id()));
        assert!(!rl.remove_window(&test_client_id()));
    }

    // === OpPriority tests ===
    #[test]
    fn op_priority_rank_and_str() {
        assert_eq!(OpPriority::Low.rank(), 0);
        assert_eq!(OpPriority::Normal.rank(), 1);
        assert_eq!(OpPriority::High.rank(), 2);
        assert_eq!(OpPriority::Critical.rank(), 3);
        assert_eq!(OpPriority::Critical.as_str(), "critical");
        assert!(OpPriority::Critical > OpPriority::Low);
    }

    // === QueuedOperation tests ===
    #[test]
    fn queued_operation_attempts() {
        let mut op = QueuedOperation::new(
            test_op_ref(),
            OpPriority::High,
            1000,
            3,
        );
        assert!(!op.has_exceeded_max_attempts());
        op.increment_attempts();
        op.increment_attempts();
        op.increment_attempts();
        assert!(op.has_exceeded_max_attempts());
        assert_eq!(op.wait_time_ms(5000), 4000);
    }

    // === OperationPriorityQueue tests ===
    #[test]
    fn priority_queue_enqueue_and_dequeue() {
        let pq = OperationPriorityQueue::default();
        pq.enqueue(OpRef::new([1u8; 9]), OpPriority::Low, 1000, 3).unwrap();
        pq.enqueue(OpRef::new([2u8; 9]), OpPriority::Critical, 2000, 3).unwrap();
        pq.enqueue(OpRef::new([3u8; 9]), OpPriority::High, 3000, 3).unwrap();
        assert_eq!(pq.enqueue_count(), 3);
        assert_eq!(pq.total_queue_size(), 3);

        let first = pq.dequeue().unwrap();
        assert_eq!(first.priority, OpPriority::Critical);
        let second = pq.dequeue().unwrap();
        assert_eq!(second.priority, OpPriority::High);
        let third = pq.dequeue().unwrap();
        assert_eq!(third.priority, OpPriority::Low);
        assert!(pq.dequeue().is_none());
        assert_eq!(pq.dequeue_count(), 3);
    }

    #[test]
    fn priority_queue_full() {
        let pq = OperationPriorityQueue::new(1);
        pq.enqueue(OpRef::new([1u8; 9]), OpPriority::Low, 1000, 3).unwrap();
        let err = pq.enqueue(OpRef::new([2u8; 9]), OpPriority::Critical, 2000, 3).unwrap_err();
        assert!(matches!(err, ClientError::InvalidState { .. }));
        assert_eq!(pq.dropped_count(), 1);
    }

    #[test]
    fn priority_queue_remove_op() {
        let pq = OperationPriorityQueue::default();
        pq.enqueue(OpRef::new([1u8; 9]), OpPriority::Low, 1000, 3).unwrap();
        assert!(pq.remove_op(&OpRef::new([1u8; 9])));
        assert!(!pq.remove_op(&OpRef::new([1u8; 9])));
        assert_eq!(pq.total_queue_size(), 0);
    }

    #[test]
    fn priority_queue_peek() {
        let pq = OperationPriorityQueue::default();
        assert!(pq.peek(OpPriority::Low).is_none());
        pq.enqueue(OpRef::new([1u8; 9]), OpPriority::Low, 1000, 3).unwrap();
        let peeked = pq.peek(OpPriority::Low).unwrap();
        assert_eq!(peeked.op_ref, OpRef::new([1u8; 9]));
    }

    // === ConnectionState tests ===
    #[test]
    fn connection_state_as_str() {
        assert_eq!(ConnectionState::Connecting.as_str(), "connecting");
        assert_eq!(ConnectionState::Connected.as_str(), "connected");
        assert_eq!(ConnectionState::Idle.as_str(), "idle");
        assert_eq!(ConnectionState::Closing.as_str(), "closing");
        assert_eq!(ConnectionState::Closed.as_str(), "closed");
        assert_eq!(ConnectionState::Failed.as_str(), "failed");
        assert!(ConnectionState::Connected.is_active());
        assert!(ConnectionState::Idle.is_active());
        assert!(!ConnectionState::Closed.is_active());
    }

    // === ClientConnection tests ===
    #[test]
    fn client_connection_lifecycle() {
        let mut conn = ClientConnection::new(
            test_client_id(),
            Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1, 2, 3] },
            60000,
        );
        assert_eq!(conn.state, ConnectionState::Connecting);
        conn.connect(1000);
        assert_eq!(conn.state, ConnectionState::Connected);
        assert!(conn.state.is_active());
        conn.record_request(2000);
        assert_eq!(conn.request_count, 1);
        conn.mark_idle();
        assert_eq!(conn.state, ConnectionState::Idle);
        conn.close();
        assert_eq!(conn.state, ConnectionState::Closed);
        assert!(!conn.state.is_active());
    }

    #[test]
    fn client_connection_idle_too_long() {
        let mut conn = ClientConnection::new(
            test_client_id(),
            Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![] },
            1000,
        );
        conn.connect(100);
        conn.record_request(200);
        assert!(!conn.is_idle_too_long(500));
        assert!(conn.is_idle_too_long(2000));
    }

    // === ClientConnectionPool tests ===
    #[test]
    fn client_connection_pool_add_and_connect() {
        let pool = ClientConnectionPool::default();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        pool.add_connection(test_client_id(), endpoint.clone(), 60000).unwrap();
        assert_eq!(pool.connection_count(), 1);
        assert_eq!(pool.tracked_connection_count(), 1);
        pool.connect(&test_client_id(), 1000).unwrap();
        assert_eq!(pool.active_connection_count(), 1);
    }

    #[test]
    fn client_connection_pool_duplicate() {
        let pool = ClientConnectionPool::default();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        pool.add_connection(test_client_id(), endpoint.clone(), 60000).unwrap();
        let err = pool.add_connection(test_client_id(), endpoint, 60000).unwrap_err();
        assert!(matches!(err, ClientError::InvalidState { .. }));
    }

    #[test]
    fn client_connection_pool_close_and_fail() {
        let pool = ClientConnectionPool::default();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        pool.add_connection(test_client_id(), endpoint.clone(), 60000).unwrap();
        pool.connect(&test_client_id(), 1000).unwrap();
        pool.close_connection(&test_client_id()).unwrap();
        assert_eq!(pool.closed_connection_count(), 1);
        assert_eq!(pool.active_connection_count(), 0);

        let endpoint2 = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![2] };
        pool.add_connection(NodeId::new([2; 32]), endpoint2, 60000).unwrap();
        pool.connect(&NodeId::new([2; 32]), 2000).unwrap();
        pool.fail_connection(&NodeId::new([2; 32])).unwrap();
        assert_eq!(pool.failed_connection_count(), 1);
    }

    #[test]
    fn client_connection_pool_record_request_and_error() {
        let pool = ClientConnectionPool::default();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        pool.add_connection(test_client_id(), endpoint, 60000).unwrap();
        pool.connect(&test_client_id(), 1000).unwrap();
        pool.record_request(&test_client_id(), 2000).unwrap();
        pool.record_error(&test_client_id()).unwrap();
        let conn = pool.get_connection(&test_client_id()).unwrap();
        assert_eq!(conn.request_count, 1);
        assert_eq!(conn.error_count, 1);
    }

    #[test]
    fn client_connection_pool_idle_too_long() {
        let pool = ClientConnectionPool::default();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        pool.add_connection(test_client_id(), endpoint, 1000).unwrap();
        pool.connect(&test_client_id(), 100).unwrap();
        pool.record_request(&test_client_id(), 200).unwrap();
        assert_eq!(pool.idle_too_long_count(500), 0);
        assert_eq!(pool.idle_too_long_count(2000), 1);
    }

    // === OperationTimeoutEntry tests ===
    #[test]
    fn operation_timeout_entry_expiry() {
        let mut entry = OperationTimeoutEntry::new(test_op_ref(), 5000);
        assert!(!entry.is_expired(1000));
        assert!(!entry.is_expired(5000));
        assert!(entry.is_expired(5001));
        entry.mark_expired();
        assert!(entry.is_expired(0));
    }

    // === OperationTimeoutTracker tests ===
    #[test]
    fn operation_timeout_tracker_register_and_check() {
        let tracker = OperationTimeoutTracker::default();
        tracker.register_timeout(test_op_ref(), 5000);
        assert_eq!(tracker.timeout_count(), 1);
        assert_eq!(tracker.tracked_timeout_count(), 1);
        let expired = tracker.check_expired(1000);
        assert!(expired.is_empty());
        let expired = tracker.check_expired(6000);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], test_op_ref());
        assert_eq!(tracker.expired_count(), 1);
    }

    #[test]
    fn operation_timeout_tracker_clear() {
        let tracker = OperationTimeoutTracker::default();
        tracker.register_timeout(test_op_ref(), 5000);
        assert!(tracker.clear_timeout(&test_op_ref()));
        assert!(!tracker.clear_timeout(&test_op_ref()));
        assert_eq!(tracker.cleared_count(), 1);
        assert_eq!(tracker.tracked_timeout_count(), 0);
    }

    #[test]
    fn operation_timeout_tracker_is_expired() {
        let tracker = OperationTimeoutTracker::default();
        tracker.register_timeout(test_op_ref(), 5000);
        assert!(!tracker.is_expired(&test_op_ref(), 1000));
        assert!(tracker.is_expired(&test_op_ref(), 6000));
    }

    // === Extended ClientEngine tests ===
    #[test]
    fn client_engine_new_with_features() {
        let engine = ClientEngine::new_with_features();
        assert_eq!(engine.batch_count(), 0);
        assert_eq!(engine.priority_queue_size(), 0);
        assert_eq!(engine.connection_count(), 0);
        assert_eq!(engine.active_connection_count(), 0);
        assert_eq!(engine.timeout_count(), 0);
        assert_eq!(engine.expired_timeout_count(), 0);
    }

    #[test]
    fn client_engine_submit_batch() {
        let engine = ClientEngine::new_with_features();
        let ops = vec![test_operation(), test_operation()];
        let batch = engine.submit_batch(test_op_ref(), ops, 1000).unwrap();
        assert_eq!(batch.batch_id, test_op_ref());
        assert_eq!(engine.batch_count(), 1);
    }

    #[test]
    fn client_engine_complete_batch_operation() {
        let engine = ClientEngine::new_with_features();
        let ops = vec![test_operation(), test_operation()];
        engine.submit_batch(test_op_ref(), ops, 1000).unwrap();
        engine.complete_batch_operation(&test_op_ref(), true).unwrap();
        engine.complete_batch_operation(&test_op_ref(), true).unwrap();
        let batch = engine.batch_mgr.borrow().get_batch(&test_op_ref()).unwrap();
        assert_eq!(batch.status, BatchStatus::Completed);
    }

    #[test]
    fn client_engine_rate_limit() {
        let engine = ClientEngine::new_with_features();
        assert_eq!(
            engine.check_rate_limit(test_client_id(), 1000),
            RateLimitAction::Allow
        );
        let rate = engine.rate_limit_rejection_rate();
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn client_engine_priority_queue() {
        let engine = ClientEngine::new_with_features();
        engine.enqueue_operation(OpRef::new([1u8; 9]), OpPriority::Critical, 1000, 3).unwrap();
        engine.enqueue_operation(OpRef::new([2u8; 9]), OpPriority::Low, 2000, 3).unwrap();
        assert_eq!(engine.priority_queue_size(), 2);
        let dequeued = engine.dequeue_operation().unwrap();
        assert_eq!(dequeued.priority, OpPriority::Critical);
        assert_eq!(engine.priority_queue_size(), 1);
    }

    #[test]
    fn client_engine_connection_pool() {
        let engine = ClientEngine::new_with_features();
        let endpoint = Endpoint { carrier_type: crate::types::CarrierType::Loopback, address: vec![1] };
        engine.add_connection(test_client_id(), endpoint.clone(), 60000).unwrap();
        assert_eq!(engine.connection_count(), 1);
        engine.connect_connection(&test_client_id(), 1000).unwrap();
        assert_eq!(engine.active_connection_count(), 1);
    }

    #[test]
    fn client_engine_timeout_tracker() {
        let engine = ClientEngine::new_with_features();
        engine.register_timeout(test_op_ref(), 5000);
        assert_eq!(engine.timeout_count(), 1);
        let expired = engine.check_expired_timeouts(1000);
        assert!(expired.is_empty());
        let expired = engine.check_expired_timeouts(6000);
        assert_eq!(expired.len(), 1);
        assert_eq!(engine.expired_timeout_count(), 1);
    }

    // === NodeId helper tests ===
    #[test]
    fn node_id_from_key_id() {
        let nid = NodeId::from_key_id(KeyId::new([1; 16]));
        assert_eq!(nid.0[..16], [1u8; 16]);
    }

    #[test]
    fn node_id_from_nonce() {
        let nid = NodeId::from_nonce([1u8; 32]);
        assert_eq!(nid.0, [1u8; 32]);
    }
}