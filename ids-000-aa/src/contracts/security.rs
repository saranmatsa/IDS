//! # Sealed Trait: Security — authentication, authorization, encryption, key management
//!
//! Carrier-agnostic: operates on opaque Endpoints and NodeIds only.
//! No std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, or DNS.
//!
//! The Security trait defines mutual authentication, dataset-scoped authorization,
//! data encryption (in transit + at rest), cryptographic key management with rotation
//! and revocation, tamper-evident audit trails, nonce management, replay protection,
//! constant-time channel binding, ephemeral public key validation (reject all-zeros),
//! endpoint address size limits (1024 bytes), zero nonce rejection, and regional
//! security boundaries. There is no global security master — each dataset/region
//! manages its own security context.
//! Implementations live downstream in ids-security and ids-* crates.

use crate::types::{
    AuthToken, Credential, DatasetId, Endpoint, KeyId, NodeId, OpType, RegionId,
    RotatingNodeId, Signature,
};
use crate::errors::SecurityError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Security
///////////////////////////////////////////////////////////////////////////////

mod private {
    pub trait Sealed {}
}

impl private::Sealed for SecurityEngine {}
impl private::Sealed for AuthManager {}
impl private::Sealed for AuthzManager {}
impl private::Sealed for EncryptionManager {}
impl private::Sealed for KeyManager {}
impl private::Sealed for KeyRotationManager {}
impl private::Sealed for KeyRevocationManager {}
impl private::Sealed for AuditManager {}
impl private::Sealed for NonceManager {}
impl private::Sealed for ReplayProtection {}
impl private::Sealed for ChannelBinding {}
impl private::Sealed for EphemeralKeyValidator {}
impl private::Sealed for AddressLimitValidator {}
impl private::Sealed for ZeroNonceValidator {}
impl private::Sealed for RegionalBoundary {}
impl private::Sealed for SecurityStats {}
impl private::Sealed for SecurityMonitor {}
impl private::Sealed for SecurityValidator {}

// Sealed trait for security — no global master.
// Downstream crates can call methods and use as trait bounds but cannot implement.
pub trait Security: private::Sealed {
    /// Mutual authentication — returns an auth token on success.
    fn authenticate(
        &self,
        identity: NodeId,
        credential: Credential,
    ) -> Result<AuthToken, SecurityError>;

    /// Authorization — token-based dataset/operation-level access control.
    fn authorize(
        &self,
        token: AuthToken,
        dataset: DatasetId,
        op: OpType,
    ) -> Result<(), SecurityError>;

    /// Encrypt data for a given key (in transit + at rest).
    fn encrypt(
        &self,
        data: Vec<u8>,
        key_id: KeyId,
    ) -> Result<Vec<u8>, SecurityError>;

    /// Decrypt data for a given key.
    fn decrypt(
        &self,
        data: Vec<u8>,
        key_id: KeyId,
    ) -> Result<Vec<u8>, SecurityError>;

    /// Rotate a cryptographic key, returning the new key id.
    fn rotate_key(&self, key_id: KeyId) -> Result<KeyId, SecurityError>;

    /// Revoke a cryptographic key.
    fn revoke_key(&self, key_id: KeyId) -> Result<(), SecurityError>;

    /// Record an audit event (tamper-evident).
    fn audit_event(
        &self,
        node_id: NodeId,
        event_type: &'static str,
        detail: Vec<u8>,
    ) -> Result<(), SecurityError>;

    /// Generate a nonce for a given context.
    fn generate_nonce(&self, context: &[u8]) -> Result<[u8; 32], SecurityError>;

    /// Check replay protection for an operation reference.
    fn replay_check(&self, op_ref: &[u8; 9]) -> Result<bool, SecurityError>;

    /// Constant-time channel binding comparison.
    fn verify_channel_binding(
        &self,
        binding_a: &[u8],
        binding_b: &[u8],
    ) -> Result<bool, SecurityError>;

    /// Validate an ephemeral public key (reject all-zeros).
    fn validate_ephemeral_key(&self, public_key: &[u8; 32]) -> Result<bool, SecurityError>;

    /// Validate endpoint address size limit (max 1024 bytes).
    fn validate_endpoint_address_size(&self, address: &[u8]) -> Result<(), SecurityError>;

    /// Reject zero nonce (nonce.0 != 0 required).
    fn reject_zero_nonce(&self, nonce: &[u8; 32]) -> Result<(), SecurityError>;

    /// Regional security boundary check.
    fn check_regional_boundary(
        &self,
        source: RegionId,
        target: RegionId,
    ) -> Result<bool, SecurityError>;

    /// Get security stats.
    fn stats(&self) -> SecurityStats;

    /// Get the security name for debugging.
    fn security_name(&self) -> &'static str;

    /// Get total authentication count.
    fn auth_count(&self) -> u64;

    /// Get total authorization count.
    fn authz_count(&self) -> u64;

    /// Get total encryption count.
    fn encrypt_count(&self) -> u64;

    /// Get total decryption count.
    fn decrypt_count(&self) -> u64;
}

///////////////////////////////////////////////////////////////////////////////
// SECURITY ERROR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SecurityError {
    #[error("authentication failed: {identity:?}")]
    AuthFailed { identity: NodeId },

    #[error("invalid credential: {identity:?}")]
    InvalidCredential { identity: NodeId },

    #[error("token not found or expired: {token:?}")]
    TokenNotFound { token: AuthToken },

    #[error("authorization denied: dataset={dataset:?}, op={op:?}")]
    AuthorizationDenied { dataset: DatasetId, op: OpType },

    #[error("encryption failed: key_id={key_id:?}")]
    EncryptionFailed { key_id: KeyId },

    #[error("decryption failed: key_id={key_id:?}")]
    DecryptionFailed { key_id: KeyId },

    #[error("key not found: {key_id:?}")]
    KeyNotFound { key_id: KeyId },

    #[error("key already revoked: {key_id:?}")]
    KeyRevoked { key_id: KeyId },

    #[error("key rotation failed: {key_id:?}")]
    KeyRotationFailed { key_id: KeyId },

    #[error("audit write failed: {message}")]
    AuditFailed { message: String },

    #[error("nonce generation failed: {message}")]
    NonceFailed { message: String },

    #[error("replay detected: op_ref={op_ref:?}")]
    ReplayDetected { op_ref: [u8; 9] },

    #[error("channel binding mismatch")]
    ChannelBindingMismatch,

    #[error("invalid ephemeral key: all-zeros or malformed")]
    InvalidEphemeralKey,

    #[error("endpoint address exceeds size limit: {size} bytes")]
    EndpointAddressTooLarge { size: usize },

    #[error("zero nonce rejected")]
    ZeroNonceRejected,

    #[error("regional boundary violation: source={source:?}, target={target:?}")]
    RegionalBoundaryViolation { source: RegionId, target: RegionId },

    #[error("invalid security state: {message}")]
    InvalidState { message: String },

    #[error("internal security error: {message}")]
    Internal { message: String },
}

impl SecurityError {
    pub fn category(&self) -> &'static str {
        match self {
            SecurityError::AuthFailed { .. } => "auth_failed",
            SecurityError::InvalidCredential { .. } => "invalid_credential",
            SecurityError::TokenNotFound { .. } => "token_not_found",
            SecurityError::AuthorizationDenied { .. } => "authorization_denied",
            SecurityError::EncryptionFailed { .. } => "encryption_failed",
            SecurityError::DecryptionFailed { .. } => "decryption_failed",
            SecurityError::KeyNotFound { .. } => "key_not_found",
            SecurityError::KeyRevoked { .. } => "key_revoked",
            SecurityError::KeyRotationFailed { .. } => "key_rotation_failed",
            SecurityError::AuditFailed { .. } => "audit_failed",
            SecurityError::NonceFailed { .. } => "nonce_failed",
            SecurityError::ReplayDetected { .. } => "replay_detected",
            SecurityError::ChannelBindingMismatch => "channel_binding_mismatch",
            SecurityError::InvalidEphemeralKey => "invalid_ephemeral_key",
            SecurityError::EndpointAddressTooLarge { .. } => "endpoint_address_too_large",
            SecurityError::ZeroNonceRejected => "zero_nonce_rejected",
            SecurityError::RegionalBoundaryViolation { .. } => "regional_boundary_violation",
            SecurityError::InvalidState { .. } => "invalid_state",
            SecurityError::Internal { .. } => "internal",
        }
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SecurityError::InvalidCredential { .. }
                | SecurityError::EndpointAddressTooLarge { .. }
                | SecurityError::ZeroNonceRejected
                | SecurityError::RegionalBoundaryViolation { .. }
        )
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            SecurityError::AuthFailed { .. } => "verify credential and retry",
            SecurityError::InvalidCredential { .. } => "provide valid credential",
            SecurityError::TokenNotFound { .. } => "re-authenticate",
            SecurityError::AuthorizationDenied { .. } => "request access grant",
            SecurityError::EncryptionFailed { .. } => "check key validity",
            SecurityError::DecryptionFailed { .. } => "check key validity",
            SecurityError::KeyNotFound { .. } => "create key first",
            SecurityError::KeyRevoked { .. } => "use active key",
            SecurityError::KeyRotationFailed { .. } => "retry key rotation",
            SecurityError::AuditFailed { .. } => "check audit storage",
            SecurityError::NonceFailed { .. } => "retry nonce generation",
            SecurityError::ReplayDetected { .. } => "reject duplicate operation",
            SecurityError::ChannelBindingMismatch => "re-establish channel",
            SecurityError::InvalidEphemeralKey => "provide valid ephemeral key",
            SecurityError::EndpointAddressTooLarge { .. } => "reduce address size",
            SecurityError::ZeroNonceRejected => "use non-zero nonce",
            SecurityError::RegionalBoundaryViolation { .. } => "use authorized region pair",
            SecurityError::InvalidState { .. } => "reset security state",
            SecurityError::Internal { .. } => "contact support",
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHENTICATION STATE
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    Pending,
    Authenticated,
    Expired,
    Revoked,
}

impl AuthState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthState::Pending => "pending",
            AuthState::Authenticated => "authenticated",
            AuthState::Expired => "expired",
            AuthState::Revoked => "revoked",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, AuthState::Authenticated)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, AuthState::Expired | AuthState::Revoked)
    }
}

pub struct AuthSession {
    pub identity: NodeId,
    pub token: AuthToken,
    pub state: AuthState,
    pub authenticated_at_ms: u64,
    pub expires_at_ms: u64,
    pub auth_count: u64,
}

impl AuthSession {
    pub fn new(
        identity: NodeId,
        token: AuthToken,
        authenticated_at_ms: u64,
        ttl_ms: u64,
    ) -> Self {
        Self {
            identity,
            token,
            state: AuthState::Pending,
            authenticated_at_ms,
            expires_at_ms: authenticated_at_ms + ttl_ms,
            auth_count: 0,
        }
    }

    pub fn activate(&mut self) {
        self.state = AuthState::Authenticated;
        self.auth_count += 1;
    }

    pub fn expire(&mut self) {
        self.state = AuthState::Expired;
    }

    pub fn revoke(&mut self) {
        self.state = AuthState::Revoked;
    }

    pub fn is_valid(&self, at_ms: u64) -> bool {
        self.state == AuthState::Authenticated && at_ms < self.expires_at_ms
    }

    pub fn time_to_expiry(&self, at_ms: u64) -> u64 {
        if at_ms < self.expires_at_ms {
            self.expires_at_ms - at_ms
        } else {
            0
        }
    }

    pub fn state(&self) -> AuthState {
        self.state
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTH MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct AuthManager {
    pub sessions: std::cell::RefCell<std::collections::HashMap<AuthToken, AuthSession>>,
    pub identity_to_token: std::cell::RefCell<std::collections::HashMap<NodeId, AuthToken>>,
    pub auth_count: u64,
    pub failed_auth_count: u64,
    pub active_session_count: u64,
    pub expired_session_count: u64,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            sessions: std::cell::RefCell::new(std::collections::HashMap::new()),
            identity_to_token: std::cell::RefCell::new(std::collections::HashMap::new()),
            auth_count: 0,
            failed_auth_count: 0,
            active_session_count: 0,
            expired_session_count: 0,
        }
    }

    pub fn authenticate(
        &self,
        identity: NodeId,
        credential: Credential,
        at_ms: u64,
        ttl_ms: u64,
    ) -> Result<AuthToken, SecurityError> {
        // Validate credential: token must be non-empty
        if credential.token.is_empty() {
            self.failed_auth_count += 1;
            return Err(SecurityError::InvalidCredential { identity });
        }
        // Reject all-zeros token
        if credential.token.iter().all(|b| *b == 0) {
            self.failed_auth_count += 1;
            return Err(SecurityError::InvalidCredential { identity });
        }

        let token = AuthToken::new(blake3_hash_32(&credential.token));
        let session = AuthSession::new(identity.clone(), token, at_ms, ttl_ms);

        let mut sessions = self.sessions.borrow_mut();
        let mut identity_map = self.identity_to_token.borrow_mut();

        // Revoke existing token for this identity if any
        if let Some(old_token) = identity_map.get(&identity) {
            if let Some(old_session) = sessions.get_mut(old_token) {
                old_session.revoke();
                self.expired_session_count += 1;
            }
        }

        sessions.insert(token, session);
        identity_map.insert(identity, token);
        self.auth_count += 1;
        self.active_session_count += 1;

        Ok(token)
    }

    pub fn get_session(&self, token: &AuthToken) -> Option<AuthSession> {
        self.sessions.borrow().get(token).cloned()
    }

    pub fn get_token_for_identity(&self, identity: &NodeId) -> Option<AuthToken> {
        self.identity_to_token.borrow().get(identity).copied()
    }

    pub fn expire_session(&self, token: &AuthToken) -> bool {
        if let Some(session) = self.sessions.borrow_mut().get_mut(token) {
            session.expire();
            self.expired_session_count += 1;
            self.active_session_count = self.active_session_count.saturating_sub(1);
            true
        } else {
            false
        }
    }

    pub fn revoke_session(&self, token: &AuthToken) -> bool {
        if let Some(session) = self.sessions.borrow_mut().get_mut(token) {
            session.revoke();
            self.active_session_count = self.active_session_count.saturating_sub(1);
            true
        } else {
            false
        }
    }

    pub fn is_token_valid(&self, token: &AuthToken, at_ms: u64) -> bool {
        self.sessions
            .borrow()
            .get(token)
            .map(|s| s.is_valid(at_ms))
            .unwrap_or(false)
    }

    pub fn auth_count(&self) -> u64 {
        self.auth_count
    }

    pub fn failed_auth_count(&self) -> u64 {
        self.failed_auth_count
    }

    pub fn active_session_count(&self) -> u64 {
        self.active_session_count
    }

    pub fn session_count(&self) -> usize {
        self.sessions.borrow().len()
    }

    pub fn auth_failure_rate(&self) -> f64 {
        let total = self.auth_count + self.failed_auth_count;
        if total == 0 {
            return 0.0;
        }
        self.failed_auth_count as f64 / total as f64
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUTHORIZATION MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Write,
    Delete,
    Replicate,
    Recover,
    Admin,
}

impl Permission {
    pub fn from_op_type(op: OpType) -> Self {
        match op {
            OpType::Read => Permission::Read,
            OpType::Write => Permission::Write,
            OpType::Delete => Permission::Delete,
            OpType::Replicate => Permission::Replicate,
            OpType::Recover => Permission::Recover,
            OpType::Admin => Permission::Admin,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Write => "write",
            Permission::Delete => "delete",
            Permission::Replicate => "replicate",
            Permission::Recover => "recover",
            Permission::Admin => "admin",
        }
    }
}

pub struct AuthzGrant {
    pub token: AuthToken,
    pub dataset_id: DatasetId,
    pub permission: Permission,
    pub granted_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub grant_count: u64,
}

impl AuthzGrant {
    pub fn new(
        token: AuthToken,
        dataset_id: DatasetId,
        permission: Permission,
        granted_at_ms: u64,
        ttl_ms: Option<u64>,
    ) -> Self {
        Self {
            token,
            dataset_id,
            permission,
            granted_at_ms,
            expires_at_ms: ttl_ms.map(|t| granted_at_ms + t),
            grant_count: 0,
        }
    }

    pub fn is_valid(&self, at_ms: u64) -> bool {
        match self.expires_at_ms {
            Some(expires) => at_ms < expires,
            None => true,
        }
    }

    pub fn increment(&mut self) {
        self.grant_count += 1;
    }
}

pub struct AuthzManager {
    pub grants: std::cell::RefCell<
        std::collections::HashMap<(AuthToken, DatasetId), AuthzGrant>,
    >,
    pub token_grants: std::cell::RefCell<std::collections::HashMap<AuthToken, Vec<DatasetId>>>,
    pub authz_count: u64,
    pub denied_count: u64,
    pub active_grant_count: u64,
    pub expired_grant_count: u64,
}

impl AuthzManager {
    pub fn new() -> Self {
        Self {
            grants: std::cell::RefCell::new(std::collections::HashMap::new()),
            token_grants: std::cell::RefCell::new(std::collections::HashMap::new()),
            authz_count: 0,
            denied_count: 0,
            active_grant_count: 0,
            expired_grant_count: 0,
        }
    }

    pub fn grant(
        &self,
        token: AuthToken,
        dataset_id: DatasetId,
        permission: Permission,
        at_ms: u64,
        ttl_ms: Option<u64>,
    ) {
        let grant = AuthzGrant::new(token, dataset_id, permission, at_ms, ttl_ms);
        self.grants.borrow_mut().insert((token, dataset_id), grant);
        self.token_grants
            .borrow_mut()
            .entry(token)
            .or_default()
            .push(dataset_id);
        self.active_grant_count += 1;
    }

    pub fn authorize(
        &self,
        token: AuthToken,
        dataset_id: &DatasetId,
        op: OpType,
        at_ms: u64,
    ) -> Result<(), SecurityError> {
        self.authz_count += 1;
        let permission = Permission::from_op_type(op);

        let grants = self.grants.borrow();
        let grant = grants.get(&(token, *dataset_id));

        match grant {
            Some(g) if g.is_valid(at_ms) && g.permission == Permission::Admin => {
                // Admin grants all permissions
                Ok(())
            }
            Some(g) if g.is_valid(at_ms) && g.permission == permission => {
                drop(grants);
                self.grants.borrow_mut().get_mut(&(token, *dataset_id)).unwrap().increment();
                Ok(())
            }
            _ => {
                self.denied_count += 1;
                Err(SecurityError::AuthorizationDenied {
                    dataset: *dataset_id,
                    op,
                })
            }
        }
    }

    pub fn revoke_grant(&self, token: &AuthToken, dataset_id: &DatasetId) -> bool {
        if self.grants.borrow_mut().remove(&(*token, *dataset_id)).is_some() {
            if let Some(datasets) = self.token_grants.borrow_mut().get_mut(token) {
                datasets.retain(|d| d != dataset_id);
            }
            self.active_grant_count = self.active_grant_count.saturating_sub(1);
            true
        } else {
            false
        }
    }

    pub fn get_grant(
        &self,
        token: &AuthToken,
        dataset_id: &DatasetId,
    ) -> Option<AuthzGrant> {
        self.grants.borrow().get(&(*token, *dataset_id)).cloned()
    }

    pub fn authz_count(&self) -> u64 {
        self.authz_count
    }

    pub fn denied_count(&self) -> u64 {
        self.denied_count
    }

    pub fn active_grant_count(&self) -> u64 {
        self.active_grant_count
    }

    pub fn grant_count(&self) -> usize {
        self.grants.borrow().len()
    }

    pub fn denial_rate(&self) -> f64 {
        if self.authz_count == 0 {
            return 0.0;
        }
        self.denied_count as f64 / self.authz_count as f64
    }
}

impl Default for AuthzManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ENCRYPTION MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    XChaCha20Poly1305,
    Chacha20Poly1305,
}

impl EncryptionAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            EncryptionAlgorithm::XChaCha20Poly1305 => "xchacha20poly1305",
            EncryptionAlgorithm::Chacha20Poly1305 => "chacha20poly1305",
        }
    }

    pub fn key_size(&self) -> usize {
        match self {
            EncryptionAlgorithm::XChaCha20Poly1305 => 32,
            EncryptionAlgorithm::Chacha20Poly1305 => 32,
        }
    }

    pub fn nonce_size(&self) -> usize {
        match self {
            EncryptionAlgorithm::XChaCha20Poly1305 => 24,
            EncryptionAlgorithm::Chacha20Poly1305 => 12,
        }
    }
}

pub struct EncryptedPayload {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub algorithm: EncryptionAlgorithm,
    pub key_id: KeyId,
    pub encrypted_at_ms: u64,
}

impl EncryptedPayload {
    pub fn new(
        ciphertext: Vec<u8>,
        nonce: Vec<u8>,
        algorithm: EncryptionAlgorithm,
        key_id: KeyId,
        encrypted_at_ms: u64,
    ) -> Self {
        Self {
            ciphertext,
            nonce,
            algorithm,
            key_id,
            encrypted_at_ms,
        }
    }

    pub fn payload_size(&self) -> usize {
        self.ciphertext.len()
    }
}

pub struct EncryptionManager {
    pub keys: std::cell::RefCell<std::collections::HashMap<KeyId, Vec<u8>>>,
    pub algorithm: EncryptionAlgorithm,
    pub encrypt_count: u64,
    pub decrypt_count: u64,
    pub encrypt_failure_count: u64,
    pub decrypt_failure_count: u64,
    pub total_bytes_encrypted: u64,
    pub total_bytes_decrypted: u64,
}

impl EncryptionManager {
    pub fn new(algorithm: EncryptionAlgorithm) -> Self {
        Self {
            keys: std::cell::RefCell::new(std::collections::HashMap::new()),
            algorithm,
            encrypt_count: 0,
            decrypt_count: 0,
            encrypt_failure_count: 0,
            decrypt_failure_count: 0,
            total_bytes_encrypted: 0,
            total_bytes_decrypted: 0,
        }
    }

    pub fn register_key(&self, key_id: KeyId, key_material: Vec<u8>) -> Result<(), SecurityError> {
        let expected_size = self.algorithm.key_size();
        if key_material.len() != expected_size {
            return Err(SecurityError::InvalidState {
                message: format!(
                    "key material size {} != expected {}",
                    key_material.len(),
                    expected_size
                ),
            });
        }
        if key_material.iter().all(|b| *b == 0) {
            return Err(SecurityError::InvalidState {
                message: "all-zeros key material rejected".to_string(),
            });
        }
        self.keys.borrow_mut().insert(key_id, key_material);
        Ok(())
    }

    pub fn encrypt(
        &self,
        data: Vec<u8>,
        key_id: KeyId,
        at_ms: u64,
    ) -> Result<EncryptedPayload, SecurityError> {
        let keys = self.keys.borrow();
        let key_material = keys.get(&key_id).ok_or_else(|| {
            self.encrypt_failure_count += 1;
            SecurityError::KeyNotFound { key_id }
        })?;

        if key_material.iter().all(|b| *b == 0) {
            drop(keys);
            self.encrypt_failure_count += 1;
            return Err(SecurityError::EncryptionFailed { key_id });
        }

        let nonce_size = self.algorithm.nonce_size();
        let nonce: Vec<u8> = (0..nonce_size).map(|i| ((at_ms ^ (i as u64 * 31)) & 0xff) as u8).collect();

        // XOR-based stream cipher placeholder — real implementation uses xchacha20poly1305
        let ciphertext: Vec<u8> = data
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let key_byte = key_material[i % key_material.len()];
                let nonce_byte = nonce[i % nonce.len()];
                b ^ key_byte ^ nonce_byte
            })
            .collect();

        self.encrypt_count += 1;
        self.total_bytes_encrypted += data.len() as u64;

        Ok(EncryptedPayload::new(
            ciphertext,
            nonce,
            self.algorithm,
            key_id,
            at_ms,
        ))
    }

    pub fn decrypt(
        &self,
        payload: &EncryptedPayload,
        key_id: KeyId,
        at_ms: u64,
    ) -> Result<Vec<u8>, SecurityError> {
        let keys = self.keys.borrow();
        let key_material = keys.get(&key_id).ok_or_else(|| {
            self.decrypt_failure_count += 1;
            SecurityError::KeyNotFound { key_id }
        })?;

        if key_material.iter().all(|b| *b == 0) {
            drop(keys);
            self.decrypt_failure_count += 1;
            return Err(SecurityError::DecryptionFailed { key_id });
        }

        if payload.key_id != key_id {
            drop(keys);
            self.decrypt_failure_count += 1;
            return Err(SecurityError::DecryptionFailed { key_id });
        }

        let plaintext: Vec<u8> = payload
            .ciphertext
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let key_byte = key_material[i % key_material.len()];
                let nonce_byte = payload.nonce[i % payload.nonce.len()];
                b ^ key_byte ^ nonce_byte
            })
            .collect();

        self.decrypt_count += 1;
        self.total_bytes_decrypted += plaintext.len() as u64;

        Ok(plaintext)
    }

    pub fn get_key_material(&self, key_id: &KeyId) -> Option<Vec<u8>> {
        self.keys.borrow().get(key_id).cloned()
    }

    pub fn key_count(&self) -> usize {
        self.keys.borrow().len()
    }

    pub fn encrypt_count(&self) -> u64 {
        self.encrypt_count
    }

    pub fn decrypt_count(&self) -> u64 {
        self.decrypt_count
    }

    pub fn encrypt_failure_count(&self) -> u64 {
        self.encrypt_failure_count
    }

    pub fn decrypt_failure_count(&self) -> u64 {
        self.decrypt_failure_count
    }

    pub fn total_bytes_encrypted(&self) -> u64 {
        self.total_bytes_encrypted
    }

    pub fn total_bytes_decrypted(&self) -> u64 {
        self.total_bytes_decrypted
    }

    pub fn algorithm(&self) -> EncryptionAlgorithm {
        self.algorithm
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new(EncryptionAlgorithm::XChaCha20Poly1305)
    }
}

///////////////////////////////////////////////////////////////////////////////
// KEY MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pending,
    Active,
    Rotating,
    Revoked,
    Expired,
}

impl KeyState {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyState::Pending => "pending",
            KeyState::Active => "active",
            KeyState::Rotating => "rotating",
            KeyState::Revoked => "revoked",
            KeyState::Expired => "expired",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, KeyState::Active)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, KeyState::Revoked | KeyState::Expired)
    }
}

pub struct KeyInfo {
    pub key_id: KeyId,
    pub state: KeyState,
    pub created_at_ms: u64,
    pub activated_at_ms: Option<u64>,
    pub rotated_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
    pub usage_count: u64,
    pub max_usage: Option<u64>,
    pub algorithm: EncryptionAlgorithm,
}

impl KeyInfo {
    pub fn new(
        key_id: KeyId,
        created_at_ms: u64,
        algorithm: EncryptionAlgorithm,
    ) -> Self {
        Self {
            key_id,
            state: KeyState::Pending,
            created_at_ms,
            activated_at_ms: None,
            rotated_at_ms: None,
            revoked_at_ms: None,
            usage_count: 0,
            max_usage: None,
            algorithm,
        }
    }

    pub fn activate(&mut self, at_ms: u64) {
        self.state = KeyState::Active;
        self.activated_at_ms = Some(at_ms);
    }

    pub fn rotate(&mut self, at_ms: u64) -> KeyId {
        self.state = KeyState::Rotating;
        self.rotated_at_ms = Some(at_ms);
        KeyId::new(blake3_hash_16(&self.key_id.0))
    }

    pub fn revoke(&mut self, at_ms: u64) {
        self.state = KeyState::Revoked;
        self.revoked_at_ms = Some(at_ms);
    }

    pub fn expire(&mut self) {
        self.state = KeyState::Expired;
    }

    pub fn increment_usage(&mut self) {
        self.usage_count += 1;
    }

    pub fn at_usage_limit(&self) -> bool {
        match self.max_usage {
            Some(limit) => self.usage_count >= limit,
            None => false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }
}

pub struct KeyManager {
    pub keys: std::cell::RefCell<std::collections::HashMap<KeyId, KeyInfo>>,
    pub active_key: std::cell::Cell<Option<KeyId>>,
    pub key_count: u64,
    pub rotation_count: u64,
    pub revocation_count: u64,
    pub expired_key_count: u64,
}

impl KeyManager {
    pub fn new() -> Self {
        Self {
            keys: std::cell::RefCell::new(std::collections::HashMap::new()),
            active_key: std::cell::Cell::new(None),
            key_count: 0,
            rotation_count: 0,
            revocation_count: 0,
            expired_key_count: 0,
        }
    }

    pub fn create_key(
        &self,
        key_id: KeyId,
        algorithm: EncryptionAlgorithm,
        at_ms: u64,
    ) -> Result<(), SecurityError> {
        if self.keys.borrow().contains_key(&key_id) {
            return Err(SecurityError::InvalidState {
                message: "key already exists".to_string(),
            });
        }
        let mut key_info = KeyInfo::new(key_id, at_ms, algorithm);
        key_info.activate(at_ms);
        self.keys.borrow_mut().insert(key_id, key_info);
        self.active_key.set(Some(key_id));
        self.key_count += 1;
        Ok(())
    }

    pub fn get_key_info(&self, key_id: &KeyId) -> Option<KeyInfo> {
        self.keys.borrow().get(key_id).cloned()
    }

    pub fn set_active_key(&self, key_id: KeyId) {
        if self.keys.borrow().contains_key(&key_id) {
            self.active_key.set(Some(key_id));
        }
    }

    pub fn active_key(&self) -> Option<KeyId> {
        self.active_key.get()
    }

    pub fn rotate_key(&self, key_id: KeyId, at_ms: u64) -> Result<KeyId, SecurityError> {
        let mut keys = self.keys.borrow_mut();
        let key_info = keys.get_mut(&key_id).ok_or_else(|| {
            SecurityError::KeyNotFound { key_id }
        })?;

        if !key_info.is_active() {
            return Err(SecurityError::KeyRotationFailed { key_id });
        }

        let new_key_id = key_info.rotate(at_ms);
        self.rotation_count += 1;

        if self.active_key.get() == Some(key_id) {
            self.active_key.set(Some(new_key_id));
        }

        Ok(new_key_id)
    }

    pub fn revoke_key(&self, key_id: KeyId, at_ms: u64) -> Result<(), SecurityError> {
        let mut keys = self.keys.borrow_mut();
        let key_info = keys.get_mut(&key_id).ok_or_else(|| {
            SecurityError::KeyNotFound { key_id }
        })?;

        if key_info.is_terminal() {
            return Err(SecurityError::KeyRevoked { key_id });
        }

        key_info.revoke(at_ms);
        self.revocation_count += 1;

        if self.active_key.get() == Some(key_id) {
            self.active_key.set(None);
        }

        Ok(())
    }

    pub fn expire_key(&self, key_id: KeyId) -> Result<(), SecurityError> {
        let mut keys = self.keys.borrow_mut();
        let key_info = keys.get_mut(&key_id).ok_or_else(|| {
            SecurityError::KeyNotFound { key_id }
        })?;
        key_info.expire();
        self.expired_key_count += 1;
        Ok(())
    }

    pub fn increment_key_usage(&self, key_id: &KeyId) -> Result<(), SecurityError> {
        let mut keys = self.keys.borrow_mut();
        let key_info = keys.get_mut(key_id).ok_or_else(|| {
            SecurityError::KeyNotFound { key_id: *key_id }
        })?;
        key_info.increment_usage();
        if key_info.at_usage_limit() {
            key_info.state = KeyState::Expired;
            self.expired_key_count += 1;
        }
        Ok(())
    }

    pub fn key_count(&self) -> u64 {
        self.key_count
    }

    pub fn tracked_key_count(&self) -> usize {
        self.keys.borrow().len()
    }

    pub fn rotation_count(&self) -> u64 {
        self.rotation_count
    }

    pub fn revocation_count(&self) -> u64 {
        self.revocation_count
    }

    pub fn active_key_count(&self) -> usize {
        self.keys
            .borrow()
            .values()
            .filter(|k| k.is_active())
            .count()
    }

    pub fn revoked_key_count(&self) -> usize {
        self.keys
            .borrow()
            .values()
            .filter(|k| matches!(k.state, KeyState::Revoked))
            .count()
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// KEY ROTATION MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct RotationSchedule {
    pub key_id: KeyId,
    pub next_rotation_at_ms: u64,
    pub rotation_interval_ms: u64,
    pub rotation_count: u64,
    pub max_rotations: u64,
}

impl RotationSchedule {
    pub fn new(
        key_id: KeyId,
        next_rotation_at_ms: u64,
        rotation_interval_ms: u64,
        max_rotations: u64,
    ) -> Self {
        Self {
            key_id,
            next_rotation_at_ms,
            rotation_interval_ms,
            rotation_count: 0,
            max_rotations,
        }
    }

    pub fn is_due(&self, at_ms: u64) -> bool {
        at_ms >= self.next_rotation_at_ms && self.rotation_count < self.max_rotations
    }

    pub fn rotate(&mut self, at_ms: u64) -> u64 {
        self.rotation_count += 1;
        self.next_rotation_at_ms = at_ms + self.rotation_interval_ms;
        self.rotation_count
    }

    pub fn at_limit(&self) -> bool {
        self.rotation_count >= self.max_rotations
    }
}

pub struct KeyRotationManager {
    pub schedules: std::cell::RefCell<std::collections::HashMap<KeyId, RotationSchedule>>,
    pub scheduled_rotation_count: u64,
    pub executed_rotation_count: u64,
    pub missed_rotation_count: u64,
}

impl KeyRotationManager {
    pub fn new() -> Self {
        Self {
            schedules: std::cell::RefCell::new(std::collections::HashMap::new()),
            scheduled_rotation_count: 0,
            executed_rotation_count: 0,
            missed_rotation_count: 0,
        }
    }

    pub fn schedule_rotation(
        &self,
        key_id: KeyId,
        next_rotation_at_ms: u64,
        rotation_interval_ms: u64,
        max_rotations: u64,
    ) {
        let schedule = RotationSchedule::new(
            key_id,
            next_rotation_at_ms,
            rotation_interval_ms,
            max_rotations,
        );
        self.schedules.borrow_mut().insert(key_id, schedule);
        self.scheduled_rotation_count += 1;
    }

    pub fn is_rotation_due(&self, key_id: &KeyId, at_ms: u64) -> bool {
        self.schedules
            .borrow()
            .get(key_id)
            .map(|s| s.is_due(at_ms))
            .unwrap_or(false)
    }

    pub fn execute_rotation(&self, key_id: &KeyId, at_ms: u64) -> Option<u64> {
        let mut schedules = self.schedules.borrow_mut();
        if let Some(schedule) = schedules.get_mut(key_id) {
            if schedule.is_due(at_ms) {
                let count = schedule.rotate(at_ms);
                self.executed_rotation_count += 1;
                return Some(count);
            } else {
                self.missed_rotation_count += 1;
            }
        }
        None
    }

    pub fn unschedule_rotation(&self, key_id: &KeyId) -> bool {
        self.schedules.borrow_mut().remove(key_id).is_some()
    }

    pub fn scheduled_rotation_count(&self) -> u64 {
        self.scheduled_rotation_count
    }

    pub fn executed_rotation_count(&self) -> u64 {
        self.executed_rotation_count
    }

    pub fn missed_rotation_count(&self) -> u64 {
        self.missed_rotation_count
    }

    pub fn schedule_count(&self) -> usize {
        self.schedules.borrow().len()
    }
}

impl Default for KeyRotationManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// KEY REVOCATION MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct RevocationEntry {
    pub key_id: KeyId,
    pub revoked_at_ms: u64,
    pub reason: &'static str,
    pub revoked_by: NodeId,
}

impl RevocationEntry {
    pub fn new(
        key_id: KeyId,
        revoked_at_ms: u64,
        reason: &'static str,
        revoked_by: NodeId,
    ) -> Self {
        Self {
            key_id,
            revoked_at_ms,
            reason,
            revoked_by,
        }
    }
}

pub struct KeyRevocationManager {
    pub revocations: std::cell::RefCell<std::collections::HashMap<KeyId, RevocationEntry>>,
    pub revocation_count: u64,
    pub reinstatement_count: u64,
}

impl KeyRevocationManager {
    pub fn new() -> Self {
        Self {
            revocations: std::cell::RefCell::new(std::collections::HashMap::new()),
            revocation_count: 0,
            reinstatement_count: 0,
        }
    }

    pub fn revoke(
        &self,
        key_id: KeyId,
        revoked_at_ms: u64,
        reason: &'static str,
        revoked_by: NodeId,
    ) -> Result<(), SecurityError> {
        if self.revocations.borrow().contains_key(&key_id) {
            return Err(SecurityError::KeyRevoked { key_id });
        }
        let entry = RevocationEntry::new(key_id, revoked_at_ms, reason, revoked_by);
        self.revocations.borrow_mut().insert(key_id, entry);
        self.revocation_count += 1;
        Ok(())
    }

    pub fn is_revoked(&self, key_id: &KeyId) -> bool {
        self.revocations.borrow().contains_key(key_id)
    }

    pub fn get_revocation(&self, key_id: &KeyId) -> Option<RevocationEntry> {
        self.revocations.borrow().get(key_id).cloned()
    }

    pub fn reinstate(&self, key_id: &KeyId) -> bool {
        if self.revocations.borrow_mut().remove(key_id).is_some() {
            self.reinstatement_count += 1;
            true
        } else {
            false
        }
    }

    pub fn revocation_count(&self) -> u64 {
        self.revocation_count
    }

    pub fn reinstatement_count(&self) -> u64 {
        self.reinstatement_count
    }

    pub fn revoked_key_count(&self) -> usize {
        self.revocations.borrow().len()
    }
}

impl Default for KeyRevocationManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// AUDIT MANAGER
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event_id: [u8; 16],
    pub node_id: NodeId,
    pub event_type: &'static str,
    pub detail: Vec<u8>,
    pub timestamp_ms: u64,
    pub sequence_number: u64,
    pub previous_hash: [u8; 32],
    pub event_hash: [u8; 32],
}

impl AuditEvent {
    pub fn new(
        node_id: NodeId,
        event_type: &'static str,
        detail: Vec<u8>,
        timestamp_ms: u64,
        sequence_number: u64,
        previous_hash: [u8; 32],
    ) -> Self {
        let event_hash = blake3_hash_32(&format!(
            "{:?}:{}:{:?}:{}:{}",
            node_id, event_type, detail, timestamp_ms, sequence_number
        ));
        Self {
            event_id: blake3_hash_16(&event_hash),
            node_id,
            event_type,
            detail,
            timestamp_ms,
            sequence_number,
            previous_hash,
            event_hash,
        }
    }

    pub fn verify_chain_link(&self, previous_hash: [u8; 32]) -> bool {
        self.previous_hash == previous_hash
    }
}

pub struct AuditManager {
    pub events: std::cell::RefCell<std::collections::VecDeque<AuditEvent>>,
    pub last_hash: std::cell::Cell<[u8; 32]>,
    pub event_count: u64,
    pub max_events: u64,
    pub tamper_detection_count: u64,
}

impl AuditManager {
    pub fn new(max_events: u64) -> Self {
        Self {
            events: std::cell::RefCell::new(std::collections::VecDeque::new()),
            last_hash: std::cell::Cell::new([0u8; 32]),
            event_count: 0,
            max_events,
            tamper_detection_count: 0,
        }
    }

    pub fn record_event(
        &self,
        node_id: NodeId,
        event_type: &'static str,
        detail: Vec<u8>,
        timestamp_ms: u64,
    ) -> Result<(), SecurityError> {
        let sequence_number = self.event_count;
        let previous_hash = self.last_hash.get();
        let event = AuditEvent::new(
            node_id,
            event_type,
            detail,
            timestamp_ms,
            sequence_number,
            previous_hash,
        );

        let mut events = self.events.borrow_mut();
        if events.len() as u64 >= self.max_events {
            events.pop_front();
        }
        events.push_back(event);
        self.last_hash.set(
            events
                .back()
                .map(|e| e.event_hash)
                .unwrap_or(previous_hash),
        );
        self.event_count += 1;

        Ok(())
    }

    pub fn verify_chain(&self) -> Result<(), SecurityError> {
        let events = self.events.borrow();
        let mut prev_hash = [0u8; 32];
        for event in events.iter() {
            if !event.verify_chain_link(prev_hash) {
                self.tamper_detection_count += 1;
                return Err(SecurityError::AuditFailed {
                    message: "audit chain tamper detected".to_string(),
                });
            }
            prev_hash = event.event_hash;
        }
        Ok(())
    }

    pub fn get_events(&self) -> Vec<AuditEvent> {
        self.events.borrow().iter().cloned().collect()
    }

    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    pub fn tamper_detection_count(&self) -> u64 {
        self.tamper_detection_count
    }

    pub fn stored_event_count(&self) -> usize {
        self.events.borrow().len()
    }
}

impl Default for AuditManager {
    fn default() -> Self {
        Self::new(10000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// NONCE MANAGER
///////////////////////////////////////////////////////////////////////////////

pub struct NonceEntry {
    pub nonce: [u8; 32],
    pub context: Vec<u8>,
    pub generated_at_ms: u64,
    pub used: bool,
}

impl NonceEntry {
    pub fn new(nonce: [u8; 32], context: Vec<u8>, generated_at_ms: u64) -> Self {
        Self {
            nonce,
            context,
            generated_at_ms,
            used: false,
        }
    }

    pub fn mark_used(&mut self) {
        self.used = true;
    }

    pub fn is_zero(&self) -> bool {
        self.nonce.iter().all(|b| *b == 0)
    }
}

pub struct NonceManager {
    pub nonces: std::cell::RefCell<std::collections::HashMap<[u8; 32], NonceEntry>>,
    pub generation_count: u64,
    pub usage_count: u64,
    pub zero_nonce_rejections: u64,
    pub max_nonces: u64,
}

impl NonceManager {
    pub fn new(max_nonces: u64) -> Self {
        Self {
            nonces: std::cell::RefCell::new(std::collections::HashMap::new()),
            generation_count: 0,
            usage_count: 0,
            zero_nonce_rejections: 0,
            max_nonces,
        }
    }

    pub fn generate_nonce(&self, context: &[u8]) -> Result<[u8; 32], SecurityError> {
        let nonce = blake3_hash_32(context);
        if nonce.iter().all(|b| *b == 0) {
            self.zero_nonce_rejections += 1;
            return Err(SecurityError::NonceFailed {
                message: "generated zero nonce, regenerating".to_string(),
            });
        }

        let mut nonces = self.nonces.borrow_mut();
        if nonces.len() as u64 >= self.max_nonces {
            // Evict oldest — simple approach: clear a portion
            let keys: Vec<_> = nonces.keys().cloned().take(100).collect();
            for k in keys {
                nonces.remove(&k);
            }
        }

        let entry = NonceEntry::new(nonce, context.to_vec(), 0);
        nonces.insert(nonce, entry);
        self.generation_count += 1;

        Ok(nonce)
    }

    pub fn mark_nonce_used(&self, nonce: &[u8; 32]) -> Result<(), SecurityError> {
        let mut nonces = self.nonces.borrow_mut();
        if let Some(entry) = nonces.get_mut(nonce) {
            if entry.used {
                return Err(SecurityError::ReplayDetected { op_ref: *nonce });
            }
            entry.mark_used();
            self.usage_count += 1;
            Ok(())
        } else {
            Err(SecurityError::ReplayDetected { op_ref: *nonce })
        }
    }

    pub fn is_nonce_used(&self, nonce: &[u8; 32]) -> bool {
        self.nonces
            .borrow()
            .get(nonce)
            .map(|e| e.used)
            .unwrap_or(false)
    }

    pub fn reject_zero_nonce(&self, nonce: &[u8; 32]) -> Result<(), SecurityError> {
        if nonce.iter().all(|b| *b == 0) {
            return Err(SecurityError::ZeroNonceRejected);
        }
        Ok(())
    }

    pub fn generation_count(&self) -> u64 {
        self.generation_count
    }

    pub fn usage_count(&self) -> u64 {
        self.usage_count
    }

    pub fn zero_nonce_rejection_count(&self) -> u64 {
        self.zero_nonce_rejections
    }

    pub fn tracked_nonce_count(&self) -> usize {
        self.nonces.borrow().len()
    }
}

impl Default for NonceManager {
    fn default() -> Self {
        Self::new(100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// REPLAY PROTECTION
///////////////////////////////////////////////////////////////////////////////

pub struct ReplayWindow {
    pub op_refs: std::cell::RefCell<std::collections::HashSet<[u8; 9]>>,
    pub check_count: u64,
    pub replay_count: u64,
    pub max_window_size: u64,
}

impl ReplayWindow {
    pub fn new(max_window_size: u64) -> Self {
        Self {
            op_refs: std::cell::RefCell::new(std::collections::HashSet::new()),
            check_count: 0,
            replay_count: 0,
            max_window_size,
        }
    }

    pub fn check(&self, op_ref: &[u8; 9]) -> Result<bool, SecurityError> {
        self.check_count += 1;
        let mut refs = self.op_refs.borrow_mut();

        if refs.contains(op_ref) {
            self.replay_count += 1;
            return Err(SecurityError::ReplayDetected { op_ref: *op_ref });
        }

        if refs.len() as u64 >= self.max_window_size {
            // Clear half the window to make room
            let keys: Vec<[u8; 9]> = refs.iter().cloned().take(refs.len() / 2).collect();
            for k in keys {
                refs.remove(&k);
            }
        }

        refs.insert(*op_ref);
        Ok(true)
    }

    pub fn replay_count(&self) -> u64 {
        self.replay_count
    }

    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    pub fn replay_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 0.0;
        }
        self.replay_count as f64 / self.check_count as f64
    }

    pub fn window_size(&self) -> usize {
        self.op_refs.borrow().len()
    }
}

pub struct ReplayProtection {
    pub window: ReplayWindow,
    pub nonce_manager: std::cell::RefCell<NonceManager>,
}

impl ReplayProtection {
    pub fn new(max_window_size: u64, max_nonces: u64) -> Self {
        Self {
            window: ReplayWindow::new(max_window_size),
            nonce_manager: std::cell::RefCell::new(NonceManager::new(max_nonces)),
        }
    }

    pub fn check_op_ref(&self, op_ref: &[u8; 9]) -> Result<bool, SecurityError> {
        self.window.check(op_ref)
    }

    pub fn check_nonce(&self, nonce: &[u8; 32]) -> Result<(), SecurityError> {
        self.nonce_manager.borrow_mut().mark_nonce_used(nonce)
    }

    pub fn generate_nonce(&self, context: &[u8]) -> Result<[u8; 32], SecurityError> {
        self.nonce_manager.borrow_mut().generate_nonce(context)
    }

    pub fn replay_count(&self) -> u64 {
        self.window.replay_count()
    }

    pub fn replay_rate(&self) -> f64 {
        self.window.replay_rate()
    }
}

impl Default for ReplayProtection {
    fn default() -> Self {
        Self::new(100000, 100000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHANNEL BINDING
///////////////////////////////////////////////////////////////////////////////

pub struct ChannelBinding {
    pub bindings: std::cell::RefCell<std::collections::HashMap<NodeId, Vec<u8>>>,
    pub comparison_count: u64,
    pub mismatch_count: u64,
}

impl ChannelBinding {
    pub fn new() -> Self {
        Self {
            bindings: std::cell::RefCell::new(std::collections::HashMap::new()),
            comparison_count: 0,
            mismatch_count: 0,
        }
    }

    pub fn store_binding(&self, node_id: NodeId, binding: Vec<u8>) {
        self.bindings.borrow_mut().insert(node_id, binding);
    }

    pub fn get_binding(&self, node_id: &NodeId) -> Option<Vec<u8>> {
        self.bindings.borrow().get(node_id).cloned()
    }

    pub fn verify_channel_binding(
        &self,
        node_id: &NodeId,
        binding_b: &[u8],
    ) -> Result<bool, SecurityError> {
        self.comparison_count += 1;
        let binding_a = self.bindings.borrow().get(node_id);

        match binding_a {
            Some(a) => {
                if constant_time_eq(a.as_slice(), binding_b) {
                    Ok(true)
                } else {
                    self.mismatch_count += 1;
                    Err(SecurityError::ChannelBindingMismatch)
                }
            }
            None => {
                self.mismatch_count += 1;
                Err(SecurityError::ChannelBindingMismatch)
            }
        }
    }

    pub fn comparison_count(&self) -> u64 {
        self.comparison_count
    }

    pub fn mismatch_count(&self) -> u64 {
        self.mismatch_count
    }

    pub fn mismatch_rate(&self) -> f64 {
        if self.comparison_count == 0 {
            return 0.0;
        }
        self.mismatch_count as f64 / self.comparison_count as f64
    }

    pub fn tracked_binding_count(&self) -> usize {
        self.bindings.borrow().len()
    }
}

impl Default for ChannelBinding {
    fn default() -> Self {
        Self::new()
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

///////////////////////////////////////////////////////////////////////////////
// EPHEMERAL KEY VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct EphemeralKeyValidator {
    pub validated_keys: std::cell::RefCell<std::collections::HashSet<[u8; 32]>>,
    pub validation_count: u64,
    pub rejection_count: u64,
    pub all_zeros_rejection_count: u64,
}

impl EphemeralKeyValidator {
    pub fn new() -> Self {
        Self {
            validated_keys: std::cell::RefCell::new(std::collections::HashSet::new()),
            validation_count: 0,
            rejection_count: 0,
            all_zeros_rejection_count: 0,
        }
    }

    pub fn validate_ephemeral_key(&self, public_key: &[u8; 32]) -> Result<bool, SecurityError> {
        self.validation_count += 1;

        // Reject all-zeros key
        if public_key.iter().all(|b| *b == 0) {
            self.all_zeros_rejection_count += 1;
            self.rejection_count += 1;
            return Err(SecurityError::InvalidEphemeralKey);
        }

        // Reject keys with only high bit set (weak)
        if public_key[0] & 0x80 == 0x80 && public_key[1..].iter().all(|b| *b == 0) {
            self.rejection_count += 1;
            return Err(SecurityError::InvalidEphemeralKey);
        }

        // Check for duplicate validation
        if self.validated_keys.borrow().contains(public_key) {
            self.rejection_count += 1;
            return Err(SecurityError::InvalidEphemeralKey);
        }

        self.validated_keys.borrow_mut().insert(*public_key);
        Ok(true)
    }

    pub fn validation_count(&self) -> u64 {
        self.validation_count
    }

    pub fn rejection_count(&self) -> u64 {
        self.rejection_count
    }

    pub fn all_zeros_rejection_count(&self) -> u64 {
        self.all_zeros_rejection_count
    }

    pub fn validated_key_count(&self) -> usize {
        self.validated_keys.borrow().len()
    }
}

impl Default for EphemeralKeyValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// ADDRESS LIMIT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct AddressLimitValidator {
    pub max_address_size: usize,
    pub validation_count: u64,
    pub rejection_count: u64,
    pub oversized_address_count: u64,
}

impl AddressLimitValidator {
    pub fn new(max_address_size: usize) -> Self {
        Self {
            max_address_size,
            validation_count: 0,
            rejection_count: 0,
            oversized_address_count: 0,
        }
    }

    pub fn validate_endpoint_address_size(&self, address: &[u8]) -> Result<(), SecurityError> {
        self.validation_count += 1;

        if address.len() > self.max_address_size {
            self.oversized_address_count += 1;
            self.rejection_count += 1;
            return Err(SecurityError::EndpointAddressTooLarge {
                size: address.len(),
            });
        }

        Ok(())
    }

    pub fn validation_count(&self) -> u64 {
        self.validation_count
    }

    pub fn rejection_count(&self) -> u64 {
        self.rejection_count
    }

    pub fn oversized_address_count(&self) -> u64 {
        self.oversized_address_count
    }

    pub fn rejection_rate(&self) -> f64 {
        if self.validation_count == 0 {
            return 0.0;
        }
        self.rejection_count as f64 / self.validation_count as f64
    }
}

impl Default for AddressLimitValidator {
    fn default() -> Self {
        Self::new(1024)
    }
}

///////////////////////////////////////////////////////////////////////////////
// ZERO NONCE VALIDATOR
///////////////////////////////////////////////////////////////////////////////

pub struct ZeroNonceValidator {
    pub validation_count: u64,
    pub rejection_count: u64,
}

impl ZeroNonceValidator {
    pub fn new() -> Self {
        Self {
            validation_count: 0,
            rejection_count: 0,
        }
    }

    pub fn reject_zero_nonce(&self, nonce: &[u8; 32]) -> Result<(), SecurityError> {
        self.validation_count += 1;
        if nonce.iter().all(|b| *b == 0) {
            self.rejection_count += 1;
            return Err(SecurityError::ZeroNonceRejected);
        }
        Ok(())
    }

    pub fn validation_count(&self) -> u64 {
        self.validation_count
    }

    pub fn rejection_count(&self) -> u64 {
        self.rejection_count
    }

    pub fn rejection_rate(&self) -> f64 {
        if self.validation_count == 0 {
            return 0.0;
        }
        self.rejection_count as f64 / self.validation_count as f64
    }
}

impl Default for ZeroNonceValidator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// REGIONAL BOUNDARY
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryAction {
    Allow,
    Deny,
    RequireAuthentication,
    RequireEncryption,
}

impl BoundaryAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            BoundaryAction::Allow => "allow",
            BoundaryAction::Deny => "deny",
            BoundaryAction::RequireAuthentication => "require_authentication",
            BoundaryAction::RequireEncryption => "require_encryption",
        }
    }
}

pub struct RegionalBoundaryRule {
    pub source: RegionId,
    pub target: RegionId,
    pub action: BoundaryAction,
    pub requires_encrypted_channel: bool,
    pub requires_mutual_auth: bool,
}

impl RegionalBoundaryRule {
    pub fn new(
        source: RegionId,
        target: RegionId,
        action: BoundaryAction,
    ) -> Self {
        Self {
            source,
            target,
            action,
            requires_encrypted_channel: matches!(
                action,
                BoundaryAction::RequireEncryption | BoundaryAction::RequireAuthentication
            ),
            requires_mutual_auth: matches!(
                action,
                BoundaryAction::RequireAuthentication
            ),
        }
    }

    pub fn matches(&self, source: &RegionId, target: &RegionId) -> bool {
        self.source == *source && self.target == *target
    }
}

pub struct RegionalBoundary {
    pub rules: std::cell::RefCell<std::collections::HashMap<(RegionId, RegionId), RegionalBoundaryRule>>,
    pub check_count: u64,
    pub allow_count: u64,
    pub deny_count: u64,
    pub auth_required_count: u64,
    pub encryption_required_count: u64,
}

impl RegionalBoundary {
    pub fn new() -> Self {
        Self {
            rules: std::cell::RefCell::new(std::collections::HashMap::new()),
            check_count: 0,
            allow_count: 0,
            deny_count: 0,
            auth_required_count: 0,
            encryption_required_count: 0,
        }
    }

    pub fn add_rule(
        &self,
        source: RegionId,
        target: RegionId,
        action: BoundaryAction,
    ) {
        let rule = RegionalBoundaryRule::new(source, target, action);
        self.rules.borrow_mut().insert((source, target), rule);
    }

    pub fn check_regional_boundary(
        &self,
        source: RegionId,
        target: RegionId,
    ) -> Result<bool, SecurityError> {
        self.check_count += 1;

        let rule = self.rules.borrow().get(&(source, target));

        match rule {
            Some(rule) => match rule.action {
                BoundaryAction::Allow => {
                    self.allow_count += 1;
                    Ok(true)
                }
                BoundaryAction::Deny => {
                    self.deny_count += 1;
                    Err(SecurityError::RegionalBoundaryViolation { source, target })
                }
                BoundaryAction::RequireAuthentication => {
                    self.auth_required_count += 1;
                    Ok(true)
                }
                BoundaryAction::RequireEncryption => {
                    self.encryption_required_count += 1;
                    Ok(true)
                }
            },
            None => {
                // Default: deny cross-region without explicit rule
                self.deny_count += 1;
                Err(SecurityError::RegionalBoundaryViolation { source, target })
            }
        }
    }

    pub fn remove_rule(&self, source: &RegionId, target: &RegionId) -> bool {
        self.rules.borrow_mut().remove(&(*source, *target)).is_some()
    }

    pub fn check_count(&self) -> u64 {
        self.check_count
    }

    pub fn allow_count(&self) -> u64 {
        self.allow_count
    }

    pub fn deny_count(&self) -> u64 {
        self.deny_count
    }

    pub fn rule_count(&self) -> usize {
        self.rules.borrow().len()
    }

    pub fn deny_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 0.0;
        }
        self.deny_count as f64 / self.check_count as f64
    }
}

impl Default for RegionalBoundary {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECURITY STATS
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default)]
pub struct SecurityStats {
    pub auth_count: u64,
    pub auth_failure_count: u64,
    pub authz_count: u64,
    pub authz_denied_count: u64,
    pub encrypt_count: u64,
    pub decrypt_count: u64,
    pub key_count: u64,
    pub key_rotation_count: u64,
    pub key_revocation_count: u64,
    pub audit_event_count: u64,
    pub tamper_detection_count: u64,
    pub replay_count: u64,
    pub channel_binding_mismatch_count: u64,
    pub regional_violation_count: u64,
    pub zero_nonce_rejection_count: u64,
}

impl SecurityStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn auth_success_rate(&self) -> f64 {
        let total = self.auth_count + self.auth_failure_count;
        if total == 0 {
            return 1.0;
        }
        self.auth_count as f64 / total as f64
    }

    pub fn authz_success_rate(&self) -> f64 {
        if self.authz_count == 0 {
            return 1.0;
        }
        (self.authz_count - self.authz_denied_count) as f64 / self.authz_count as f64
    }

    pub fn security_health_score(&self) -> f64 {
        let mut score = 100.0;
        score -= self.auth_failure_count as f64 * 0.1;
        score -= self.authz_denied_count as f64 * 0.1;
        score -= self.tamper_detection_count as f64 * 10.0;
        score -= self.replay_count as f64 * 0.5;
        score -= self.channel_binding_mismatch_count as f64 * 1.0;
        score -= self.regional_violation_count as f64 * 2.0;
        score.max(0.0).min(100.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECURITY MONITOR
///////////////////////////////////////////////////////////////////////////////

pub struct SecurityMonitor {
    pub node_health: std::cell::RefCell<std::collections::HashMap<NodeId, bool>>,
    pub alert_count: u64,
    pub health_check_count: u64,
    pub security_incident_count: u64,
}

impl SecurityMonitor {
    pub fn new() -> Self {
        Self {
            node_health: std::cell::RefCell::new(std::collections::HashMap::new()),
            alert_count: 0,
            health_check_count: 0,
            security_incident_count: 0,
        }
    }

    pub fn report_health(&self, node_id: NodeId, healthy: bool) {
        let mut health = self.node_health.borrow_mut();
        let prev = health.get(&node_id).copied().unwrap_or(true);
        health.insert(node_id, healthy);
        self.health_check_count += 1;
        if prev && !healthy {
            self.alert_count += 1;
            self.security_incident_count += 1;
        }
    }

    pub fn is_node_healthy(&self, node_id: &NodeId) -> bool {
        self.node_health.borrow().get(node_id).copied().unwrap_or(true)
    }

    pub fn report_incident(&self, node_id: NodeId) {
        self.security_incident_count += 1;
        self.node_health.borrow_mut().insert(node_id, false);
        self.alert_count += 1;
    }

    pub fn alert_count(&self) -> u64 {
        self.alert_count
    }

    pub fn health_check_count(&self) -> u64 {
        self.health_check_count
    }

    pub fn security_incident_count(&self) -> u64 {
        self.security_incident_count
    }

    pub fn monitored_node_count(&self) -> usize {
        self.node_health.borrow().len()
    }

    pub fn unhealthy_node_count(&self) -> usize {
        self.node_health.borrow().values().filter(|h| !*h).count()
    }
}

impl Default for SecurityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// SECURITY VALIDATOR
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityValidationResult {
    Valid,
    InvalidCredential,
    InvalidKey,
    InvalidNonce,
    ChannelBindingFailed,
    RegionalViolation,
}

impl SecurityValidationResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityValidationResult::Valid => "valid",
            SecurityValidationResult::InvalidCredential => "invalid_credential",
            SecurityValidationResult::InvalidKey => "invalid_key",
            SecurityValidationResult::InvalidNonce => "invalid_nonce",
            SecurityValidationResult::ChannelBindingFailed => "channel_binding_failed",
            SecurityValidationResult::RegionalViolation => "regional_violation",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, SecurityValidationResult::Valid)
    }
}

pub struct SecurityValidator {
    pub validations: std::cell::RefCell<
        std::collections::HashMap<NodeId, SecurityValidationResult>,
    >,
    pub validation_count: u64,
    pub valid_count: u64,
    pub invalid_count: u64,
}

impl SecurityValidator {
    pub fn new() -> Self {
        Self {
            validations: std::cell::RefCell::new(std::collections::HashMap::new()),
            validation_count: 0,
            valid_count: 0,
            invalid_count: 0,
        }
    }

    pub fn validate_credential(
        &self,
        identity: NodeId,
        credential: &Credential,
    ) -> SecurityValidationResult {
        self.validation_count += 1;
        let result = if credential.token.is_empty()
            || credential.token.iter().all(|b| *b == 0)
        {
            SecurityValidationResult::InvalidCredential
        } else {
            SecurityValidationResult::Valid
        };
        match result {
            SecurityValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations.borrow_mut().insert(identity, result);
        result
    }

    pub fn validate_key(&self, key_id: &KeyId, key_material: &[u8]) -> SecurityValidationResult {
        self.validation_count += 1;
        let result = if key_material.is_empty()
            || key_material.iter().all(|b| *b == 0)
        {
            SecurityValidationResult::InvalidKey
        } else {
            SecurityValidationResult::Valid
        };
        match result {
            SecurityValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::from_key_id(*key_id), result);
        result
    }

    pub fn validate_nonce(&self, nonce: &[u8; 32]) -> SecurityValidationResult {
        self.validation_count += 1;
        let result = if nonce.iter().all(|b| *b == 0) {
            SecurityValidationResult::InvalidNonce
        } else {
            SecurityValidationResult::Valid
        };
        match result {
            SecurityValidationResult::Valid => self.valid_count += 1,
            _ => self.invalid_count += 1,
        }
        self.validations
            .borrow_mut()
            .insert(NodeId::from_nonce(*nonce), result);
        result
    }

    pub fn get_result(&self, node_id: &NodeId) -> Option<SecurityValidationResult> {
        self.validations.borrow().get(node_id).copied()
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

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

// Helper: derive NodeId from key id bytes (for validator key storage)
impl NodeId {
    pub fn from_key_id(key_id: KeyId) -> Self {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&key_id.0);
        Self(bytes)
    }

    pub fn from_nonce(nonce: [u8; 32]) -> Self {
        Self(nonce)
    }
}

///////////////////////////////////////////////////////////////////////////////
// HASH HELPERS (BLAKE3-based, no external crypto dependency beyond allowed primitives)
///////////////////////////////////////////////////////////////////////////////

fn blake3_hash_32(input: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(input);
    let mut out = [0u8; 32];
    hasher.finalize_xof().fill(&mut out);
    out
}

fn blake3_hash_16(input: &[u8]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(input);
    let mut out = [0u8; 16];
    hasher.finalize_xof().fill(&mut out);
    out
}

///////////////////////////////////////////////////////////////////////////////
// SECURITY ENGINE
///////////////////////////////////////////////////////////////////////////////

pub struct SecurityEngine {
    pub auth_manager: std::cell::RefCell<AuthManager>,
    pub authz_manager: std::cell::RefCell<AuthzManager>,
    pub encryption_manager: std::cell::RefCell<EncryptionManager>,
    pub key_manager: std::cell::RefCell<KeyManager>,
    pub key_rotation_manager: std::cell::RefCell<KeyRotationManager>,
    pub key_revocation_manager: std::cell::RefCell<KeyRevocationManager>,
    pub audit_manager: std::cell::RefCell<AuditManager>,
    pub nonce_manager: std::cell::RefCell<NonceManager>,
    pub replay_protection: std::cell::RefCell<ReplayProtection>,
    pub channel_binding: std::cell::RefCell<ChannelBinding>,
    pub ephemeral_key_validator: std::cell::RefCell<EphemeralKeyValidator>,
    pub address_limit_validator: std::cell::RefCell<AddressLimitValidator>,
    pub zero_nonce_validator: std::cell::RefCell<ZeroNonceValidator>,
    pub regional_boundary: std::cell::RefCell<RegionalBoundary>,
    pub stats: std::cell::RefCell<SecurityStats>,
    pub monitor: std::cell::RefCell<SecurityMonitor>,
    pub validator: std::cell::RefCell<SecurityValidator>,
}

impl SecurityEngine {
    pub fn new() -> Self {
        Self {
            auth_manager: std::cell::RefCell::new(AuthManager::new()),
            authz_manager: std::cell::RefCell::new(AuthzManager::new()),
            encryption_manager: std::cell::RefCell::new(EncryptionManager::default()),
            key_manager: std::cell::RefCell::new(KeyManager::new()),
            key_rotation_manager: std::cell::RefCell::new(KeyRotationManager::new()),
            key_revocation_manager: std::cell::RefCell::new(KeyRevocationManager::new()),
            audit_manager: std::cell::RefCell::new(AuditManager::default()),
            nonce_manager: std::cell::RefCell::new(NonceManager::default()),
            replay_protection: std::cell::RefCell::new(ReplayProtection::default()),
            channel_binding: std::cell::RefCell::new(ChannelBinding::default()),
            ephemeral_key_validator: std::cell::RefCell::new(EphemeralKeyValidator::default()),
            address_limit_validator: std::cell::RefCell::new(AddressLimitValidator::default()),
            zero_nonce_validator: std::cell::RefCell::new(ZeroNonceValidator::default()),
            regional_boundary: std::cell::RefCell::new(RegionalBoundary::default()),
            stats: std::cell::RefCell::new(SecurityStats::new()),
            monitor: std::cell::RefCell::new(SecurityMonitor::new()),
            validator: std::cell::RefCell::new(SecurityValidator::new()),
        }
    }

    pub fn authenticate(
        &self,
        identity: NodeId,
        credential: Credential,
    ) -> Result<AuthToken, SecurityError> {
        let token = self.auth_manager.borrow_mut().authenticate(
            identity.clone(),
            credential,
            0,
            3600000,
        )?;
        self.stats.borrow_mut().auth_count += 1;
        self.monitor.borrow_mut().report_health(identity, true);
        Ok(token)
    }

    pub fn authorize(
        &self,
        token: AuthToken,
        dataset: DatasetId,
        op: OpType,
    ) -> Result<(), SecurityError> {
        self.authz_manager.borrow_mut().authorize(token, &dataset, op, 0)?;
        self.stats.borrow_mut().authz_count += 1;
        Ok(())
    }

    pub fn encrypt(
        &self,
        data: Vec<u8>,
        key_id: KeyId,
    ) -> Result<Vec<u8>, SecurityError> {
        let payload = self
            .encryption_manager
            .borrow_mut()
            .encrypt(data, key_id, 0)?;
        self.stats.borrow_mut().encrypt_count += 1;
        Ok(payload.ciphertext)
    }

    pub fn decrypt(
        &self,
        data: Vec<u8>,
        key_id: KeyId,
    ) -> Result<Vec<u8>, SecurityError> {
        let payload = self
            .encryption_manager
            .borrow()
            .get_key_material(&key_id)
            .ok_or_else(|| SecurityError::KeyNotFound { key_id })?;
        let _ = payload; // key material exists
        // Reconstruct encrypted payload for decrypt (nonce would need to be stored separately in real impl)
        let nonce = [0u8; 24]; // placeholder — real impl stores nonce with ciphertext
        let encrypted_payload = crate::contracts::security::EncryptedPayload {
            ciphertext: data,
            nonce: nonce.to_vec(),
            algorithm: crate::contracts::security::EncryptionAlgorithm::XChaCha20Poly1305,
            key_id,
            encrypted_at_ms: 0,
        };
        let plaintext = self
            .encryption_manager
            .borrow_mut()
            .decrypt(&encrypted_payload, key_id, 0)?;
        self.stats.borrow_mut().decrypt_count += 1;
        Ok(plaintext)
    }

    pub fn rotate_key(&self, key_id: KeyId) -> Result<KeyId, SecurityError> {
        let new_key_id = self.key_manager.borrow_mut().rotate_key(key_id, 0)?;
        self.stats.borrow_mut().key_rotation_count += 1;
        Ok(new_key_id)
    }

    pub fn revoke_key(&self, key_id: KeyId) -> Result<(), SecurityError> {
        self.key_manager.borrow_mut().revoke_key(key_id, 0)?;
        self.stats.borrow_mut().key_revocation_count += 1;
        Ok(())
    }

    pub fn audit_event(
        &self,
        node_id: NodeId,
        event_type: &'static str,
        detail: Vec<u8>,
    ) -> Result<(), SecurityError> {
        self.audit_manager
            .borrow_mut()
            .record_event(node_id, event_type, detail, 0)?;
        self.stats.borrow_mut().audit_event_count += 1;
        Ok(())
    }

    pub fn generate_nonce(&self, context: &[u8]) -> Result<[u8; 32], SecurityError> {
        self.nonce_manager.borrow_mut().generate_nonce(context)
    }

    pub fn replay_check(&self, op_ref: &[u8; 9]) -> Result<bool, SecurityError> {
        let result = self.replay_protection.borrow_mut().check_op_ref(op_ref)?;
        if result {
            self.stats.borrow_mut().replay_count += 1;
        }
        Ok(result)
    }

    pub fn verify_channel_binding(
        &self,
        binding_a: &[u8],
        binding_b: &[u8],
    ) -> Result<bool, SecurityError> {
        let result = constant_time_eq(binding_a, binding_b);
        if !result {
            self.stats.borrow_mut().channel_binding_mismatch_count += 1;
        }
        Ok(result)
    }

    pub fn validate_ephemeral_key(&self, public_key: &[u8; 32]) -> Result<bool, SecurityError> {
        self.ephemeral_key_validator
            .borrow_mut()
            .validate_ephemeral_key(public_key)
    }

    pub fn validate_endpoint_address_size(&self, address: &[u8]) -> Result<(), SecurityError> {
        self.address_limit_validator
            .borrow_mut()
            .validate_endpoint_address_size(address)
    }

    pub fn reject_zero_nonce(&self, nonce: &[u8; 32]) -> Result<(), SecurityError> {
        self.zero_nonce_validator
            .borrow_mut()
            .reject_zero_nonce(nonce)?;
        self.stats.borrow_mut().zero_nonce_rejection_count += 1;
        Ok(())
    }

    pub fn check_regional_boundary(
        &self,
        source: RegionId,
        target: RegionId,
    ) -> Result<bool, SecurityError> {
        let result = self.regional_boundary.borrow_mut().check_regional_boundary(source, target)?;
        if !result {
            self.stats.borrow_mut().regional_violation_count += 1;
        }
        Ok(result)
    }

    pub fn stats(&self) -> SecurityStats {
        self.stats.borrow().clone()
    }

    pub fn security_name(&self) -> &'static str {
        "security_engine"
    }

    pub fn auth_count(&self) -> u64 {
        self.auth_manager.borrow().auth_count()
    }

    pub fn authz_count(&self) -> u64 {
        self.authz_manager.borrow().authz_count()
    }

    pub fn encrypt_count(&self) -> u64 {
        self.encryption_manager.borrow().encrypt_count()
    }

    pub fn decrypt_count(&self) -> u64 {
        self.encryption_manager.borrow().decrypt_count()
    }

    pub fn register_encryption_key(
        &self,
        key_id: KeyId,
        key_material: Vec<u8>,
    ) -> Result<(), SecurityError> {
        self.encryption_manager.borrow_mut().register_key(key_id, key_material)
    }

    pub fn create_key(
        &self,
        key_id: KeyId,
        algorithm: crate::contracts::security::EncryptionAlgorithm,
    ) -> Result<(), SecurityError> {
        self.key_manager
            .borrow_mut()
            .create_key(key_id, algorithm, 0)
    }

    pub fn key_count(&self) -> u64 {
        self.key_manager.borrow().key_count()
    }

    pub fn grant_authz(
        &self,
        token: AuthToken,
        dataset: DatasetId,
        permission: Permission,
        ttl_ms: Option<u64>,
    ) {
        self.authz_manager
            .borrow_mut()
            .grant(token, dataset, permission, 0, ttl_ms);
    }

    pub fn verify_audit_chain(&self) -> Result<(), SecurityError> {
        self.audit_manager.borrow_mut().verify_chain()
    }

    pub fn tamper_count(&self) -> u64 {
        self.audit_manager.borrow().tamper_detection_count()
    }

    pub fn security_health_score(&self) -> f64 {
        self.stats.borrow().security_health_score()
    }

    pub fn monitor_node_health(&self, node_id: NodeId, healthy: bool) {
        self.monitor.borrow_mut().report_health(node_id, healthy);
    }

    pub fn is_node_healthy(&self, node_id: &NodeId) -> bool {
        self.monitor.borrow().is_node_healthy(node_id)
    }

    pub fn validate_credential(
        &self,
        identity: NodeId,
        credential: &Credential,
    ) -> SecurityValidationResult {
        self.validator
            .borrow_mut()
            .validate_credential(identity, credential)
    }

    pub fn validate_nonce(&self, nonce: &[u8; 32]) -> SecurityValidationResult {
        self.validator.borrow_mut().validate_nonce(nonce)
    }

    pub fn valid_rate(&self) -> f64 {
        self.validator.borrow().valid_rate()
    }
}

impl Default for SecurityEngine {
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

    fn test_credential() -> Credential {
        Credential::new(test_node_id(), vec![1u8; 32])
    }

    fn test_dataset_id() -> DatasetId {
        DatasetId::new([1; 32])
    }

    fn test_key_id() -> KeyId {
        KeyId::new([1; 16])
    }

    fn test_region_id() -> RegionId {
        RegionId::new([1; 32])
    }

    fn test_auth_token() -> AuthToken {
        AuthToken::new([1; 32])
    }

    fn test_op_ref() -> [u8; 9] {
        [1u8; 9]
    }

    fn test_nonce() -> [u8; 32] {
        [1u8; 32]
    }

    // === SecurityError tests ===
    #[test]
    fn security_error_category() {
        assert_eq!(
            SecurityError::AuthFailed { identity: test_node_id() }.category(),
            "auth_failed"
        );
        assert_eq!(
            SecurityError::InvalidCredential { identity: test_node_id() }.category(),
            "invalid_credential"
        );
        assert_eq!(
            SecurityError::TokenNotFound { token: test_auth_token() }.category(),
            "token_not_found"
        );
        assert_eq!(
            SecurityError::AuthorizationDenied {
                dataset: test_dataset_id(),
                op: OpType::Read,
            }
            .category(),
            "authorization_denied"
        );
        assert_eq!(
            SecurityError::EncryptionFailed { key_id: test_key_id() }.category(),
            "encryption_failed"
        );
        assert_eq!(
            SecurityError::DecryptionFailed { key_id: test_key_id() }.category(),
            "decryption_failed"
        );
        assert_eq!(
            SecurityError::KeyNotFound { key_id: test_key_id() }.category(),
            "key_not_found"
        );
        assert_eq!(
            SecurityError::KeyRevoked { key_id: test_key_id() }.category(),
            "key_revoked"
        );
        assert_eq!(
            SecurityError::KeyRotationFailed { key_id: test_key_id() }.category(),
            "key_rotation_failed"
        );
        assert_eq!(
            SecurityError::AuditFailed { message: "x".to_string() }.category(),
            "audit_failed"
        );
        assert_eq!(
            SecurityError::NonceFailed { message: "x".to_string() }.category(),
            "nonce_failed"
        );
        assert_eq!(
            SecurityError::ReplayDetected { op_ref: test_op_ref() }.category(),
            "replay_detected"
        );
        assert_eq!(
            SecurityError::ChannelBindingMismatch.category(),
            "channel_binding_mismatch"
        );
        assert_eq!(
            SecurityError::InvalidEphemeralKey.category(),
            "invalid_ephemeral_key"
        );
        assert_eq!(
            SecurityError::EndpointAddressTooLarge { size: 2048 }.category(),
            "endpoint_address_too_large"
        );
        assert_eq!(
            SecurityError::ZeroNonceRejected.category(),
            "zero_nonce_rejected"
        );
        assert_eq!(
            SecurityError::RegionalBoundaryViolation {
                source: test_region_id(),
                target: test_region_id(),
            }
            .category(),
            "regional_boundary_violation"
        );
        assert_eq!(
            SecurityError::InvalidState { message: "x".to_string() }.category(),
            "invalid_state"
        );
        assert_eq!(
            SecurityError::Internal { message: "x".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn security_error_is_recoverable() {
        assert!(SecurityError::InvalidCredential { identity: test_node_id() }.is_recoverable());
        assert!(SecurityError::EndpointAddressTooLarge { size: 2048 }.is_recoverable());
        assert!(SecurityError::ZeroNonceRejected.is_recoverable());
        assert!(!SecurityError::AuthFailed { identity: test_node_id() }.is_recoverable());
        assert!(!SecurityError::KeyNotFound { key_id: test_key_id() }.is_recoverable());
    }

    #[test]
    fn security_error_suggested_action() {
        assert_eq!(
            SecurityError::AuthFailed { identity: test_node_id() }.suggested_action(),
            "verify credential and retry"
        );
        assert_eq!(
            SecurityError::ZeroNonceRejected.suggested_action(),
            "use non-zero nonce"
        );
        assert_eq!(
            SecurityError::KeyNotFound { key_id: test_key_id() }.suggested_action(),
            "create key first"
        );
    }

    // === AuthState tests ===
    #[test]
    fn auth_state_as_str() {
        assert_eq!(AuthState::Pending.as_str(), "pending");
        assert_eq!(AuthState::Authenticated.as_str(), "authenticated");
        assert_eq!(AuthState::Expired.as_str(), "expired");
        assert_eq!(AuthState::Revoked.as_str(), "revoked");
        assert!(AuthState::Authenticated.is_active());
        assert!(AuthState::Expired.is_terminal());
        assert!(AuthState::Revoked.is_terminal());
    }

    // === AuthSession tests ===
    #[test]
    fn auth_session_activate_and_expire() {
        let mut session = AuthSession::new(
            test_node_id(),
            test_auth_token(),
            1000,
            3600000,
        );
        assert_eq!(session.state, AuthState::Pending);
        session.activate();
        assert_eq!(session.state, AuthState::Authenticated);
        assert_eq!(session.auth_count, 1);
        assert!(session.is_valid(2000));
        session.expire();
        assert_eq!(session.state, AuthState::Expired);
        assert!(!session.is_valid(2000));
    }

    #[test]
    fn auth_session_revoke() {
        let mut session = AuthSession::new(test_node_id(), test_auth_token(), 1000, 3600000);
        session.activate();
        session.revoke();
        assert_eq!(session.state, AuthState::Revoked);
        assert!(session.state.is_terminal());
    }

    #[test]
    fn auth_session_time_to_expiry() {
        let session = AuthSession::new(test_node_id(), test_auth_token(), 1000, 3600000);
        assert_eq!(session.time_to_expiry(1000), 3600000);
        assert_eq!(session.time_to_expiry(3700000), 0);
    }

    // === AuthManager tests ===
    #[test]
    fn auth_manager_authenticate() {
        let am = AuthManager::default();
        let token = am.authenticate(
            test_node_id(),
            test_credential(),
            1000,
            3600000,
        ).unwrap();
        assert_eq!(am.auth_count(), 1);
        assert_eq!(am.failed_auth_count(), 0);
        assert!(am.is_token_valid(&token, 2000));
    }

    #[test]
    fn auth_manager_invalid_credential() {
        let am = AuthManager::default();
        let bad_cred = Credential::new(test_node_id(), vec![]);
        let err = am.authenticate(test_node_id(), bad_cred, 1000, 3600000).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidCredential { .. }));
        assert_eq!(am.failed_auth_count(), 1);
    }

    #[test]
    fn auth_manager_all_zeros_credential() {
        let am = AuthManager::default();
        let bad_cred = Credential::new(test_node_id(), vec![0u8; 32]);
        let err = am.authenticate(test_node_id(), bad_cred, 1000, 3600000).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidCredential { .. }));
    }

    #[test]
    fn auth_manager_expire_and_revoke() {
        let am = AuthManager::default();
        let token = am.authenticate(test_node_id(), test_credential(), 1000, 3600000).unwrap();
        assert!(am.expire_session(&token));
        assert!(!am.is_token_valid(&token, 2000));

        let token2 = am.authenticate(test_node_id(), test_credential(), 2000, 3600000).unwrap();
        assert!(am.revoke_session(&token2));
    }

    #[test]
    fn auth_manager_session_count() {
        let am = AuthManager::default();
        am.authenticate(test_node_id(), test_credential(), 1000, 3600000).unwrap();
        am.authenticate(NodeId::new([2; 32]), test_credential(), 2000, 3600000).unwrap();
        assert_eq!(am.session_count(), 2);
        assert_eq!(am.active_session_count(), 2);
    }

    #[test]
    fn auth_manager_auth_failure_rate() {
        let am = AuthManager::default();
        am.authenticate(test_node_id(), test_credential(), 1000, 3600000).unwrap();
        let _ = am.authenticate(test_node_id(), Credential::new(test_node_id(), vec![]), 2000, 3600000);
        let rate = am.auth_failure_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === Permission tests ===
    #[test]
    fn permission_from_op_type() {
        assert_eq!(Permission::from_op_type(OpType::Read), Permission::Read);
        assert_eq!(Permission::from_op_type(OpType::Write), Permission::Write);
        assert_eq!(Permission::from_op_type(OpType::Admin), Permission::Admin);
    }

    #[test]
    fn permission_as_str() {
        assert_eq!(Permission::Read.as_str(), "read");
        assert_eq!(Permission::Write.as_str(), "write");
        assert_eq!(Permission::Admin.as_str(), "admin");
    }

    // === AuthzGrant tests ===
    #[test]
    fn authz_grant_validity() {
        let grant = AuthzGrant::new(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            Some(3600000),
        );
        assert!(grant.is_valid(2000));
        assert!(!grant.is_valid(3700000));
    }

    #[test]
    fn authz_grant_no_ttl() {
        let grant = AuthzGrant::new(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            None,
        );
        assert!(grant.is_valid(1000000));
        assert!(grant.is_valid(999999999));
    }

    // === AuthzManager tests ===
    #[test]
    fn authz_manager_grant_and_authorize() {
        let azm = AuthzManager::default();
        azm.grant(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            Some(3600000),
        );
        assert!(azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Read, 2000).is_ok());
        assert_eq!(azm.authz_count(), 1);
        assert_eq!(azm.denied_count(), 0);
    }

    #[test]
    fn authz_manager_deny() {
        let azm = AuthzManager::default();
        azm.grant(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            Some(3600000),
        );
        let err = azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Write, 2000).unwrap_err();
        assert!(matches!(err, SecurityError::AuthorizationDenied { .. }));
        assert_eq!(azm.denied_count(), 1);
    }

    #[test]
    fn authz_manager_admin_grants_all() {
        let azm = AuthzManager::default();
        azm.grant(
            test_auth_token(),
            test_dataset_id(),
            Permission::Admin,
            1000,
            Some(3600000),
        );
        assert!(azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Delete, 2000).is_ok());
    }

    #[test]
    fn authz_manager_expired_grant() {
        let azm = AuthzManager::default();
        azm.grant(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            Some(1000),
        );
        let err = azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Read, 2000).unwrap_err();
        assert!(matches!(err, SecurityError::AuthorizationDenied { .. }));
    }

    #[test]
    fn authz_manager_revoke_grant() {
        let azm = AuthzManager::default();
        azm.grant(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            1000,
            Some(3600000),
        );
        assert!(azm.revoke_grant(&test_auth_token(), &test_dataset_id()));
        assert!(!azm.revoke_grant(&test_auth_token(), &test_dataset_id()));
    }

    #[test]
    fn authz_manager_denial_rate() {
        let azm = AuthzManager::default();
        azm.grant(test_auth_token(), test_dataset_id(), Permission::Read, 1000, Some(3600000));
        let _ = azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Write, 2000);
        assert!(azm.authorize(test_auth_token(), &test_dataset_id(), OpType::Read, 2000).is_ok());
        let rate = azm.denial_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === EncryptionAlgorithm tests ===
    #[test]
    fn encryption_algorithm_as_str() {
        assert_eq!(EncryptionAlgorithm::XChaCha20Poly1305.as_str(), "xchacha20poly1305");
        assert_eq!(EncryptionAlgorithm::Chacha20Poly1305.as_str(), "chacha20poly1305");
    }

    #[test]
    fn encryption_algorithm_sizes() {
        assert_eq!(EncryptionAlgorithm::XChaCha20Poly1305.key_size(), 32);
        assert_eq!(EncryptionAlgorithm::XChaCha20Poly1305.nonce_size(), 24);
        assert_eq!(EncryptionAlgorithm::Chacha20Poly1305.key_size(), 32);
        assert_eq!(EncryptionAlgorithm::Chacha20Poly1305.nonce_size(), 12);
    }

    // === EncryptedPayload tests ===
    #[test]
    fn encrypted_payload_new() {
        let payload = EncryptedPayload::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            EncryptionAlgorithm::XChaCha20Poly1305,
            test_key_id(),
            1000,
        );
        assert_eq!(payload.payload_size(), 3);
        assert_eq!(payload.key_id, test_key_id());
    }

    // === EncryptionManager tests ===
    #[test]
    fn encryption_manager_register_and_encrypt() {
        let em = EncryptionManager::default();
        em.register_key(test_key_id(), vec![1u8; 32]).unwrap();
        let payload = em.encrypt(vec![1, 2, 3, 4], test_key_id(), 1000).unwrap();
        assert_eq!(payload.algorithm, EncryptionAlgorithm::XChaCha20Poly1305);
        assert_eq!(em.encrypt_count(), 1);
        assert_eq!(em.total_bytes_encrypted(), 4);
    }

    #[test]
    fn encryption_manager_encrypt_key_not_found() {
        let em = EncryptionManager::default();
        let err = em.encrypt(vec![1, 2, 3], test_key_id(), 1000).unwrap_err();
        assert!(matches!(err, SecurityError::KeyNotFound { .. }));
    }

    #[test]
    fn encryption_manager_all_zeros_key() {
        let em = EncryptionManager::default();
        em.register_key(test_key_id(), vec![0u8; 32]).unwrap_err();
    }

    #[test]
    fn encryption_manager_encrypt_decrypt_roundtrip() {
        let em = EncryptionManager::default();
        em.register_key(test_key_id(), vec![1u8; 32]).unwrap();
        let payload = em.encrypt(vec![1, 2, 3, 4, 5], test_key_id(), 1000).unwrap();
        let decrypted = em.decrypt(&payload, test_key_id(), 2000).unwrap();
        assert_eq!(decrypted, vec![1, 2, 3, 4, 5]);
        assert_eq!(em.decrypt_count(), 1);
    }

    #[test]
    fn encryption_manager_wrong_key_decrypt() {
        let em = EncryptionManager::default();
        em.register_key(test_key_id(), vec![1u8; 32]).unwrap();
        let payload = em.encrypt(vec![1, 2, 3], test_key_id(), 1000).unwrap();
        let err = em.decrypt(&payload, KeyId::new([2; 16]), 2000).unwrap_err();
        assert!(matches!(err, SecurityError::KeyNotFound { .. }));
    }

    #[test]
    fn encryption_manager_counts() {
        let em = EncryptionManager::default();
        em.register_key(test_key_id(), vec![1u8; 32]).unwrap();
        em.encrypt(vec![1], test_key_id(), 1000).unwrap();
        em.encrypt(vec![2], test_key_id(), 2000).unwrap();
        assert_eq!(em.encrypt_count(), 2);
        assert_eq!(em.key_count(), 1);
    }

    // === KeyState tests ===
    #[test]
    fn key_state_as_str() {
        assert_eq!(KeyState::Pending.as_str(), "pending");
        assert_eq!(KeyState::Active.as_str(), "active");
        assert_eq!(KeyState::Rotating.as_str(), "rotating");
        assert_eq!(KeyState::Revoked.as_str(), "revoked");
        assert_eq!(KeyState::Expired.as_str(), "expired");
        assert!(KeyState::Active.is_active());
        assert!(KeyState::Revoked.is_terminal());
        assert!(KeyState::Expired.is_terminal());
    }

    // === KeyInfo tests ===
    #[test]
    fn key_info_lifecycle() {
        let mut info = KeyInfo::new(test_key_id(), 1000, EncryptionAlgorithm::XChaCha20Poly1305);
        assert_eq!(info.state, KeyState::Pending);
        info.activate(2000);
        assert_eq!(info.state, KeyState::Active);
        assert!(info.is_active());
        let new_id = info.rotate(3000);
        assert_ne!(new_id, test_key_id());
        info.revoke(4000);
        assert_eq!(info.state, KeyState::Revoked);
        assert!(info.state.is_terminal());
    }

    // === KeyManager tests ===
    #[test]
    fn key_manager_create_and_active() {
        let km = KeyManager::default();
        km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 1000).unwrap();
        assert_eq!(km.active_key(), Some(test_key_id()));
        assert_eq!(km.key_count(), 1);
    }

    #[test]
    fn key_manager_duplicate_key() {
        let km = KeyManager::default();
        km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 1000).unwrap();
        let err = km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 2000).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidState { .. }));
    }

    #[test]
    fn key_manager_rotate() {
        let km = KeyManager::default();
        km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 1000).unwrap();
        let new_id = km.rotate_key(test_key_id(), 2000).unwrap();
        assert_ne!(new_id, test_key_id());
        assert_eq!(km.active_key(), Some(new_id));
        assert_eq!(km.rotation_count(), 1);
    }

    #[test]
    fn key_manager_revoke() {
        let km = KeyManager::default();
        km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 1000).unwrap();
        km.revoke_key(test_key_id(), 2000).unwrap();
        assert_eq!(km.active_key(), None);
        assert_eq!(km.revocation_count(), 1);
    }

    #[test]
    fn key_manager_increment_usage() {
        let km = KeyManager::default();
        km.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305, 1000).unwrap();
        km.increment_key_usage(&test_key_id()).unwrap();
        assert_eq!(km.active_key_count(), 1);
    }

    // === RotationSchedule tests ===
    #[test]
    fn rotation_schedule_due() {
        let mut sched = RotationSchedule::new(test_key_id(), 1000, 3600000, 5);
        assert!(!sched.is_due(500));
        assert!(sched.is_due(1000));
        let count = sched.rotate(1000);
        assert_eq!(count, 1);
        assert!(!sched.at_limit());
    }

    #[test]
    fn rotation_schedule_at_limit() {
        let mut sched = RotationSchedule::new(test_key_id(), 1000, 3600000, 1);
        sched.rotate(1000);
        assert!(sched.at_limit());
    }

    // === KeyRotationManager tests ===
    #[test]
    fn key_rotation_manager_schedule_and_execute() {
        let krm = KeyRotationManager::default();
        krm.schedule_rotation(test_key_id(), 1000, 3600000, 5);
        assert!(krm.is_rotation_due(&test_key_id(), 1000));
        let count = krm.execute_rotation(&test_key_id(), 1000).unwrap();
        assert_eq!(count, 1);
        assert_eq!(krm.executed_rotation_count(), 1);
        assert_eq!(krm.schedule_count(), 1);
    }

    #[test]
    fn key_rotation_manager_unschedule() {
        let krm = KeyRotationManager::default();
        krm.schedule_rotation(test_key_id(), 1000, 3600000, 5);
        assert!(krm.unschedule_rotation(&test_key_id()));
        assert!(!krm.unschedule_rotation(&test_key_id()));
    }

    // === RevocationEntry tests ===
    #[test]
    fn revocation_entry_new() {
        let entry = RevocationEntry::new(test_key_id(), 1000, "compromised", test_node_id());
        assert_eq!(entry.key_id, test_key_id());
        assert_eq!(entry.reason, "compromised");
        assert_eq!(entry.revoked_by, test_node_id());
    }

    // === KeyRevocationManager tests ===
    #[test]
    fn key_revocation_manager_revoke() {
        let krm = KeyRevocationManager::default();
        krm.revoke(test_key_id(), 1000, "compromised", test_node_id()).unwrap();
        assert!(krm.is_revoked(&test_key_id()));
        assert_eq!(krm.revocation_count(), 1);
    }

    #[test]
    fn key_revocation_manager_double_revoke() {
        let krm = KeyRevocationManager::default();
        krm.revoke(test_key_id(), 1000, "compromised", test_node_id()).unwrap();
        let err = krm.revoke(test_key_id(), 2000, "compromised", test_node_id()).unwrap_err();
        assert!(matches!(err, SecurityError::KeyRevoked { .. }));
    }

    #[test]
    fn key_revocation_manager_reinstate() {
        let krm = KeyRevocationManager::default();
        krm.revoke(test_key_id(), 1000, "compromised", test_node_id()).unwrap();
        assert!(krm.reinstate(&test_key_id()));
        assert!(!krm.is_revoked(&test_key_id()));
        assert_eq!(krm.reinstatement_count(), 1);
    }

    // === AuditEvent tests ===
    #[test]
    fn audit_event_new_and_verify() {
        let event = AuditEvent::new(
            test_node_id(),
            "auth",
            vec![1, 2, 3],
            1000,
            1,
            [0u8; 32],
        );
        assert!(event.verify_chain_link([0u8; 32]));
        assert!(!event.verify_chain_link([1u8; 32]));
    }

    // === AuditManager tests ===
    #[test]
    fn audit_manager_record_and_verify() {
        let am = AuditManager::default();
        am.record_event(test_node_id(), "auth", vec![1, 2, 3], 1000).unwrap();
        am.record_event(test_node_id(), "encrypt", vec![4, 5, 6], 2000).unwrap();
        assert_eq!(am.event_count(), 2);
        assert_eq!(am.stored_event_count(), 2);
        assert!(am.verify_chain().is_ok());
    }

    #[test]
    fn audit_manager_tamper_detection() {
        let am = AuditManager::default();
        am.record_event(test_node_id(), "auth", vec![1], 1000).unwrap();
        // Directly tamper with last hash to break chain
        am.last_hash.set([9u8; 32]);
        let err = am.verify_chain().unwrap_err();
        assert!(matches!(err, SecurityError::AuditFailed { .. }));
        assert_eq!(am.tamper_detection_count(), 1);
    }

    // === NonceEntry tests ===
    #[test]
    fn nonce_entry_is_zero() {
        let entry = NonceEntry::new([0u8; 32], vec![], 1000);
        assert!(entry.is_zero());
        let entry2 = NonceEntry::new([1u8; 32], vec![], 1000);
        assert!(!entry2.is_zero());
    }

    // === NonceManager tests ===
    #[test]
    fn nonce_manager_generate_and_use() {
        let nm = NonceManager::default();
        let nonce = nm.generate_nonce(b"context").unwrap();
        assert!(!nm.is_nonce_used(&nonce));
        nm.mark_nonce_used(&nonce).unwrap();
        assert!(nm.is_nonce_used(&nonce));
    }

    #[test]
    fn nonce_manager_reject_zero_nonce() {
        let nm = NonceManager::default();
        let err = nm.reject_zero_nonce(&[0u8; 32]).unwrap_err();
        assert!(matches!(err, SecurityError::ZeroNonceRejected));
        assert_eq!(nm.zero_nonce_rejection_count(), 1);
    }

    #[test]
    fn nonce_manager_replay_detect() {
        let nm = NonceManager::default();
        let nonce = nm.generate_nonce(b"ctx").unwrap();
        nm.mark_nonce_used(&nonce).unwrap();
        let err = nm.mark_nonce_used(&nonce).unwrap_err();
        assert!(matches!(err, SecurityError::ReplayDetected { .. }));
    }

    // === ReplayWindow tests ===
    #[test]
    fn replay_window_check_and_detect() {
        let rw = ReplayWindow::new(100);
        assert!(rw.check(&test_op_ref()).unwrap());
        let err = rw.check(&test_op_ref()).unwrap_err();
        assert!(matches!(err, SecurityError::ReplayDetected { .. }));
        assert_eq!(rw.replay_count(), 1);
        assert_eq!(rw.check_count(), 2);
    }

    // === ReplayProtection tests ===
    #[test]
    fn replay_protection_check_op_ref() {
        let rp = ReplayProtection::default();
        assert!(rp.check_op_ref(&test_op_ref()).unwrap());
        let err = rp.check_op_ref(&test_op_ref()).unwrap_err();
        assert!(matches!(err, SecurityError::ReplayDetected { .. }));
    }

    #[test]
    fn replay_protection_nonce() {
        let rp = ReplayProtection::default();
        let nonce = rp.generate_nonce(b"ctx").unwrap();
        rp.check_nonce(&nonce).unwrap();
        let err = rp.check_nonce(&nonce).unwrap_err();
        assert!(matches!(err, SecurityError::ReplayDetected { .. }));
    }

    // === ChannelBinding tests ===
    #[test]
    fn channel_binding_verify() {
        let cb = ChannelBinding::default();
        cb.store_binding(test_node_id(), vec![1, 2, 3]);
        assert!(cb.verify_channel_binding(&test_node_id(), &[1, 2, 3]).unwrap());
        let err = cb.verify_channel_binding(&test_node_id(), &[4, 5, 6]).unwrap_err();
        assert!(matches!(err, SecurityError::ChannelBindingMismatch));
    }

    #[test]
    fn channel_binding_unknown_node() {
        let cb = ChannelBinding::default();
        let err = cb.verify_channel_binding(&NodeId::new([2; 32]), &[1, 2, 3]).unwrap_err();
        assert!(matches!(err, SecurityError::ChannelBindingMismatch));
    }

    #[test]
    fn channel_binding_mismatch_rate() {
        let cb = ChannelBinding::default();
        cb.store_binding(test_node_id(), vec![1]);
        let _ = cb.verify_channel_binding(&test_node_id(), &[1]);
        let _ = cb.verify_channel_binding(&test_node_id(), &[2]);
        let rate = cb.mismatch_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    // === constant_time_eq tests ===
    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"helloo"));
    }

    // === EphemeralKeyValidator tests ===
    #[test]
    fn ephemeral_key_validator_accept_valid() {
        let ekv = EphemeralKeyValidator::default();
        let key = [1u8; 32];
        assert!(ekv.validate_ephemeral_key(&key).unwrap());
        assert_eq!(ekv.validation_count(), 1);
    }

    #[test]
    fn ephemeral_key_validator_reject_all_zeros() {
        let ekv = EphemeralKeyValidator::default();
        let err = ekv.validate_ephemeral_key(&[0u8; 32]).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidEphemeralKey));
        assert_eq!(ekv.all_zeros_rejection_count(), 1);
    }

    #[test]
    fn ephemeral_key_validator_reject_duplicate() {
        let ekv = EphemeralKeyValidator::default();
        let key = [1u8; 32];
        ekv.validate_ephemeral_key(&key).unwrap();
        let err = ekv.validate_ephemeral_key(&key).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidEphemeralKey));
    }

    // === AddressLimitValidator tests ===
    #[test]
    fn address_limit_validator_accept() {
        let alv = AddressLimitValidator::default();
        let addr = vec![1u8; 500];
        alv.validate_endpoint_address_size(&addr).unwrap();
        assert_eq!(alv.validation_count(), 1);
    }

    #[test]
    fn address_limit_validator_reject_oversized() {
        let alv = AddressLimitValidator::default();
        let addr = vec![1u8; 2048];
        let err = alv.validate_endpoint_address_size(&addr).unwrap_err();
        assert!(matches!(err, SecurityError::EndpointAddressTooLarge { .. }));
        assert_eq!(alv.oversized_address_count(), 1);
    }

    // === ZeroNonceValidator tests ===
    #[test]
    fn zero_nonce_validator_accept() {
        let znv = ZeroNonceValidator::default();
        znv.reject_zero_nonce(&[1u8; 32]).unwrap();
        assert_eq!(znv.validation_count(), 1);
    }

    #[test]
    fn zero_nonce_validator_reject() {
        let znv = ZeroNonceValidator::default();
        let err = znv.reject_zero_nonce(&[0u8; 32]).unwrap_err();
        assert!(matches!(err, SecurityError::ZeroNonceRejected));
        assert_eq!(znv.rejection_count(), 1);
    }

    // === BoundaryAction tests ===
    #[test]
    fn boundary_action_as_str() {
        assert_eq!(BoundaryAction::Allow.as_str(), "allow");
        assert_eq!(BoundaryAction::Deny.as_str(), "deny");
        assert_eq!(BoundaryAction::RequireAuthentication.as_str(), "require_authentication");
        assert_eq!(BoundaryAction::RequireEncryption.as_str(), "require_encryption");
    }

    // === RegionalBoundaryRule tests ===
    #[test]
    fn regional_boundary_rule_matches() {
        let rule = RegionalBoundaryRule::new(
            test_region_id(),
            RegionId::new([2; 32]),
            BoundaryAction::RequireEncryption,
        );
        assert!(rule.matches(&test_region_id(), &RegionId::new([2; 32])));
        assert!(!rule.matches(&RegionId::new([2; 32]), &test_region_id()));
        assert!(rule.requires_encrypted_channel);
    }

    // === RegionalBoundary tests ===
    #[test]
    fn regional_boundary_allow() {
        let rb = RegionalBoundary::default();
        rb.add_rule(test_region_id(), RegionId::new([2; 32]), BoundaryAction::Allow);
        assert!(rb.check_regional_boundary(test_region_id(), RegionId::new([2; 32])).unwrap());
        assert_eq!(rb.allow_count(), 1);
    }

    #[test]
    fn regional_boundary_deny() {
        let rb = RegionalBoundary::default();
        rb.add_rule(test_region_id(), RegionId::new([2; 32]), BoundaryAction::Deny);
        let err = rb.check_regional_boundary(test_region_id(), RegionId::new([2; 32])).unwrap_err();
        assert!(matches!(err, SecurityError::RegionalBoundaryViolation { .. }));
        assert_eq!(rb.deny_count(), 1);
    }

    #[test]
    fn regional_boundary_default_deny() {
        let rb = RegionalBoundary::default();
        let err = rb.check_regional_boundary(test_region_id(), RegionId::new([2; 32])).unwrap_err();
        assert!(matches!(err, SecurityError::RegionalBoundaryViolation { .. }));
    }

    #[test]
    fn regional_boundary_remove_rule() {
        let rb = RegionalBoundary::default();
        rb.add_rule(test_region_id(), RegionId::new([2; 32]), BoundaryAction::Allow);
        assert!(rb.remove_rule(&test_region_id(), &RegionId::new([2; 32])));
        assert!(!rb.remove_rule(&test_region_id(), &RegionId::new([2; 32])));
    }

    // === SecurityStats tests ===
    #[test]
    fn security_stats_health_score() {
        let mut s = SecurityStats::default();
        s.auth_count = 100;
        s.auth_failure_count = 5;
        s.tamper_detection_count = 0;
        let score = s.security_health_score();
        assert!(score > 90.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn security_stats_auth_success_rate() {
        let mut s = SecurityStats::default();
        s.auth_count = 90;
        s.auth_failure_count = 10;
        assert!((s.auth_success_rate() - 0.9).abs() < 0.01);
    }

    #[test]
    fn security_stats_authz_success_rate() {
        let mut s = SecurityStats::default();
        s.authz_count = 100;
        s.authz_denied_count = 20;
        assert!((s.authz_success_rate() - 0.8).abs() < 0.01);
    }

    // === SecurityMonitor tests ===
    #[test]
    fn security_monitor_report_health() {
        let sm = SecurityMonitor::default();
        sm.report_health(test_node_id(), true);
        assert!(sm.is_node_healthy(&test_node_id()));
        assert_eq!(sm.health_check_count(), 1);
    }

    #[test]
    fn security_monitor_incident() {
        let sm = SecurityMonitor::default();
        sm.report_health(test_node_id(), true);
        sm.report_incident(test_node_id());
        assert!(!sm.is_node_healthy(&test_node_id()));
        assert_eq!(sm.security_incident_count(), 1);
        assert_eq!(sm.alert_count(), 1);
    }

    // === SecurityValidationResult tests ===
    #[test]
    fn security_validation_result_as_str() {
        assert_eq!(SecurityValidationResult::Valid.as_str(), "valid");
        assert_eq!(SecurityValidationResult::InvalidCredential.as_str(), "invalid_credential");
        assert_eq!(SecurityValidationResult::InvalidKey.as_str(), "invalid_key");
        assert_eq!(SecurityValidationResult::InvalidNonce.as_str(), "invalid_nonce");
        assert_eq!(SecurityValidationResult::ChannelBindingFailed.as_str(), "channel_binding_failed");
        assert_eq!(SecurityValidationResult::RegionalViolation.as_str(), "regional_violation");
        assert!(SecurityValidationResult::Valid.is_ok());
        assert!(!SecurityValidationResult::InvalidKey.is_ok());
    }

    // === SecurityValidator tests ===
    #[test]
    fn security_validator_validate_credential() {
        let sv = SecurityValidator::default();
        let good = Credential::new(test_node_id(), vec![1u8; 32]);
        let bad = Credential::new(test_node_id(), vec![]);
        assert_eq!(sv.validate_credential(test_node_id(), &good), SecurityValidationResult::Valid);
        assert_eq!(sv.validate_credential(test_node_id(), &bad), SecurityValidationResult::InvalidCredential);
        assert_eq!(sv.validation_count(), 2);
        assert_eq!(sv.valid_count(), 1);
        assert_eq!(sv.invalid_count(), 1);
    }

    #[test]
    fn security_validator_validate_nonce() {
        let sv = SecurityValidator::default();
        assert_eq!(sv.validate_nonce(&[1u8; 32]), SecurityValidationResult::Valid);
        assert_eq!(sv.validate_nonce(&[0u8; 32]), SecurityValidationResult::InvalidNonce);
    }

    #[test]
    fn security_validator_valid_rate() {
        let sv = SecurityValidator::default();
        sv.validate_credential(test_node_id(), &Credential::new(test_node_id(), vec![1u8; 32]));
        sv.validate_credential(test_node_id(), &Credential::new(test_node_id(), vec![]));
        assert!((sv.valid_rate() - 0.5).abs() < 0.01);
    }

    // === SecurityEngine tests ===
    #[test]
    fn security_engine_authenticate() {
        let engine = SecurityEngine::default();
        let token = engine.authenticate(test_node_id(), test_credential()).unwrap();
        assert_eq!(engine.auth_count(), 1);
        assert_eq!(token, AuthToken::new(blake3_hash_32(&vec![1u8; 32])));
    }

    #[test]
    fn security_engine_auth_failed() {
        let engine = SecurityEngine::default();
        let err = engine.authenticate(test_node_id(), Credential::new(test_node_id(), vec![])).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidCredential { .. }));
    }

    #[test]
    fn security_engine_authorize() {
        let engine = SecurityEngine::default();
        engine.grant_authz(
            test_auth_token(),
            test_dataset_id(),
            Permission::Read,
            None,
        );
        assert!(engine.authorize(test_auth_token(), test_dataset_id(), OpType::Read).is_ok());
        assert_eq!(engine.authz_count(), 1);
    }

    #[test]
    fn security_engine_encrypt_decrypt() {
        let engine = SecurityEngine::default();
        engine.register_encryption_key(test_key_id(), vec![1u8; 32]).unwrap();
        let ciphertext = engine.encrypt(vec![1, 2, 3], test_key_id()).unwrap();
        let plaintext = engine.decrypt(ciphertext.clone(), test_key_id()).unwrap();
        assert_eq!(plaintext, vec![1, 2, 3]);
        assert_eq!(engine.encrypt_count(), 1);
        assert_eq!(engine.decrypt_count(), 1);
    }

    #[test]
    fn security_engine_key_rotation() {
        let engine = SecurityEngine::default();
        engine.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305).unwrap();
        let new_id = engine.rotate_key(test_key_id()).unwrap();
        assert_ne!(new_id, test_key_id());
        assert_eq!(engine.key_count(), 1);
    }

    #[test]
    fn security_engine_revoke_key() {
        let engine = SecurityEngine::default();
        engine.create_key(test_key_id(), EncryptionAlgorithm::XChaCha20Poly1305).unwrap();
        engine.revoke_key(test_key_id()).unwrap();
    }

    #[test]
    fn security_engine_audit_event() {
        let engine = SecurityEngine::default();
        engine.audit_event(test_node_id(), "login", vec![1, 2, 3]).unwrap();
        assert_eq!(engine.stats().audit_event_count, 1);
        assert!(engine.verify_audit_chain().is_ok());
    }

    #[test]
    fn security_engine_replay_check() {
        let engine = SecurityEngine::default();
        assert!(engine.replay_check(&test_op_ref()).unwrap());
        let err = engine.replay_check(&test_op_ref()).unwrap_err();
        assert!(matches!(err, SecurityError::ReplayDetected { .. }));
    }

    #[test]
    fn security_engine_channel_binding() {
        let engine = SecurityEngine::default();
        assert!(engine.verify_channel_binding(&[1, 2, 3], &[1, 2, 3]).unwrap());
        let err = engine.verify_channel_binding(&[1, 2, 3], &[4, 5, 6]).unwrap_err();
        assert!(matches!(err, SecurityError::ChannelBindingMismatch));
    }

    #[test]
    fn security_engine_validate_ephemeral_key() {
        let engine = SecurityEngine::default();
        assert!(engine.validate_ephemeral_key(&[1u8; 32]).unwrap());
        let err = engine.validate_ephemeral_key(&[0u8; 32]).unwrap_err();
        assert!(matches!(err, SecurityError::InvalidEphemeralKey));
    }

    #[test]
    fn security_engine_validate_endpoint_address_size() {
        let engine = SecurityEngine::default();
        let small = vec![1u8; 100];
        engine.validate_endpoint_address_size(&small).unwrap();
        let large = vec![1u8; 2048];
        let err = engine.validate_endpoint_address_size(&large).unwrap_err();
        assert!(matches!(err, SecurityError::EndpointAddressTooLarge { .. }));
    }

    #[test]
    fn security_engine_reject_zero_nonce() {
        let engine = SecurityEngine::default();
        engine.reject_zero_nonce(&[1u8; 32]).unwrap();
        let err = engine.reject_zero_nonce(&[0u8; 32]).unwrap_err();
        assert!(matches!(err, SecurityError::ZeroNonceRejected));
    }

    #[test]
    fn security_engine_regional_boundary() {
        let engine = SecurityEngine::default();
        engine.regional_boundary.borrow_mut().add_rule(
            test_region_id(),
            RegionId::new([2; 32]),
            BoundaryAction::Deny,
        );
        let err = engine.check_regional_boundary(test_region_id(), RegionId::new([2; 32])).unwrap_err();
        assert!(matches!(err, SecurityError::RegionalBoundaryViolation { .. }));
    }

    #[test]
    fn security_engine_stats_and_name() {
        let engine = SecurityEngine::default();
        assert_eq!(engine.security_name(), "security_engine");
        assert_eq!(engine.auth_count(), 0);
        assert_eq!(engine.authz_count(), 0);
        assert_eq!(engine.encrypt_count(), 0);
        assert_eq!(engine.decrypt_count(), 0);
    }

    #[test]
    fn security_engine_monitor_health() {
        let engine = SecurityEngine::default();
        engine.monitor_node_health(test_node_id(), false);
        assert!(!engine.is_node_healthy(&test_node_id()));
    }

    #[test]
    fn security_engine_validate_credential() {
        let engine = SecurityEngine::default();
        let good = Credential::new(test_node_id(), vec![1u8; 32]);
        assert_eq!(
            engine.validate_credential(test_node_id(), &good),
            SecurityValidationResult::Valid
        );
        let bad = Credential::new(test_node_id(), vec![]);
        assert_eq!(
            engine.validate_credential(test_node_id(), &bad),
            SecurityValidationResult::InvalidCredential
        );
    }

    #[test]
    fn security_engine_valid_rate() {
        let engine = SecurityEngine::default();
        engine.validate_credential(test_node_id(), &Credential::new(test_node_id(), vec![1u8; 32]));
        engine.validate_credential(test_node_id(), &Credential::new(test_node_id(), vec![]));
        assert!((engine.valid_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn security_engine_security_health_score() {
        let engine = SecurityEngine::default();
        engine.audit_event(test_node_id(), "login", vec![]).unwrap();
        let score = engine.security_health_score();
        assert!(score >= 0.0);
        assert!(score <= 100.0);
    }

    // === Integration: full security flow ===
    #[test]
    fn security_full_flow() {
        let engine = SecurityEngine::default();

        // Authenticate
        let token = engine.authenticate(test_node_id(), test_credential()).unwrap();
        assert!(engine.is_node_healthy(&test_node_id()));

        // Authorize
        engine.grant_authz(token, test_dataset_id(), Permission::Read, None);
        assert!(engine.authorize(token, test_dataset_id(), OpType::Read).is_ok());

        // Encrypt/decrypt
        engine.register_encryption_key(test_key_id(), vec![1u8; 32]).unwrap();
        let ciphertext = engine.encrypt(b"secret data".to_vec(), test_key_id()).unwrap();
        let plaintext = engine.decrypt(ciphertext, test_key_id()).unwrap();
        assert_eq!(plaintext, b"secret data");

        // Rotate key
        let new_key = engine.rotate_key(test_key_id()).unwrap();
        assert_ne!(new_key, test_key_id());

        // Audit
        engine.audit_event(test_node_id(), "key_rotation", vec![]).unwrap();
        assert!(engine.verify_audit_chain().is_ok());

        // Replay check
        assert!(engine.replay_check(&test_op_ref()).unwrap());
        let _ = engine.replay_check(&test_op_ref());

        // Regional boundary
        engine.regional_boundary.borrow_mut().add_rule(
            test_region_id(),
            RegionId::new([2; 32]),
            BoundaryAction::RequireEncryption,
        );
        assert!(engine.check_regional_boundary(test_region_id(), RegionId::new([2; 32])).unwrap());

        // Stats
        let s = engine.stats();
        assert!(s.auth_count >= 1);
        assert!(s.authz_count >= 1);
        assert!(s.encrypt_count >= 1);
        assert!(s.decrypt_count >= 1);
        assert!(s.audit_event_count >= 1);
    }

    // === NodeId helper tests ===
    #[test]
    fn node_id_from_key_id() {
        let nid = NodeId::from_key_id(test_key_id());
        assert_eq!(nid.0[..16], test_key_id().0);
    }

    #[test]
    fn node_id_from_nonce() {
        let nid = NodeId::from_nonce(test_nonce());
        assert_eq!(nid.0, test_nonce());
    }
}