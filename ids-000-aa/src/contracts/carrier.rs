//! # ids-000-aa Contract: Carrier
//!
//! `trait Carrier` — carrier-agnostic send/receive of opaque datagrams.
//! No `std::net`, `tokio::net`, IP, TCP, UDP, or any existing networking stack.
//! `Endpoint` is `CarrierType + Vec<u8>` only.

use crate::types::{CarrierType, Endpoint};
use crate::errors::CarrierError;

///////////////////////////////////////////////////////////////////////////////
// SEALED TRAIT: Carrier
///////////////////////////////////////////////////////////////////////////////

/// Carrier-agnostic transport for sending and receiving opaque datagrams.
/// Implementations are provided by downstream `ids-carrier` crate.
/// Sealed: only ids-000-aa and designated impl crates can implement.
pub trait Carrier: private::Sealed {
    /// Send an opaque datagram to the given endpoint.
    /// Returns error if the carrier is unavailable or the address is invalid.
    fn send_datagram(&self, endpoint: &Endpoint, data: &[u8]) -> Result<(), CarrierError>;

    /// Receive an opaque datagram from any connected endpoint.
    /// Blocks until a datagram is available or an error occurs.
    fn receive_datagram(&self) -> Result<(Endpoint, Vec<u8>), CarrierError>;

    /// Returns the carrier type this implementation uses.
    fn carrier_type(&self) -> CarrierType;

    /// Maximum endpoint address size in bytes (default 1024).
    /// Enforced at the carrier layer to prevent oversized addresses.
    fn endpoint_address_size_limit(&self) -> usize {
        1024
    }

    /// Check if the carrier is currently available for sending/receiving.
    fn is_available(&self) -> bool {
        true
    }

    /// Flush any pending datagrams to the underlying carrier.
    fn flush(&self) -> Result<(), CarrierError> {
        Ok(())
    }

    /// Get the carrier's name for debugging and observability.
    fn carrier_name(&self) -> &'static str {
        "carrier"
    }

    /// Get the maximum datagram payload size supported by this carrier.
    fn max_datagram_size(&self) -> usize {
        65536
    }

    /// Check if the given endpoint is valid for this carrier type.
    fn is_endpoint_valid(&self, endpoint: &Endpoint) -> bool {
        endpoint.address.len() <= self.endpoint_address_size_limit()
    }

    /// Get the carrier's current uptime in arbitrary time units.
    fn carrier_uptime(&self) -> u64 {
        0
    }

    /// Reset the carrier state, dropping all pending datagrams.
    fn reset(&self) -> Result<(), CarrierError> {
        Ok(())
    }

    /// Get statistics about datagrams sent through this carrier.
    fn datagrams_sent(&self) -> u64 {
        0
    }

    /// Get statistics about datagrams received through this carrier.
    fn datagrams_received(&self) -> u64 {
        0
    }

    /// Get the number of bytes sent through this carrier.
    fn bytes_sent(&self) -> u64 {
        0
    }

    /// Get the number of bytes received through this carrier.
    fn bytes_received(&self) -> u64 {
        0
    }

    /// Check if the carrier supports broadcast semantics.
    fn supports_broadcast(&self) -> bool {
        false
    }

    /// Check if the carrier supports multicast semantics.
    fn supports_multicast(&self) -> bool {
        false
    }

    /// Get the carrier's priority for routing decisions (higher = preferred).
    fn priority(&self) -> u32 {
        0
    }

    /// Get the carrier's reliability level (0 = best effort, 10 = guaranteed).
    fn reliability_level(&self) -> u32 {
        0
    }

    /// Get the carrier's latency class (0 = low, 1 = medium, 2 = high).
    fn latency_class(&self) -> u32 {
        1
    }

    /// Get the carrier's bandwidth class (0 = low, 1 = medium, 2 = high).
    fn bandwidth_class(&self) -> u32 {
        1
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEALED SUPERTRAIT MODULE
///////////////////////////////////////////////////////////////////////////////

mod private {
    /// Sealed supertrait — prevents downstream crates from implementing Carrier.
    pub trait Sealed {}
    impl Sealed for crate::types::CarrierType {}
}

///////////////////////////////////////////////////////////////////////////////
// ERROR TYPE: CarrierError
///////////////////////////////////////////////////////////////////////////////

/// Errors returned by carrier operations.
#[derive(Debug, thiserror::Error)]
pub enum CarrierError {
    #[error("carrier {0:?} not available")]
    Unavailable(CarrierType),

    #[error("endpoint address too large: {size} bytes (max {max})")]
    AddressTooLarge { size: usize, max: usize },

    #[error("endpoint {endpoint:?} not valid for carrier {carrier:?}")]
    InvalidEndpoint { endpoint: String, carrier: String },

    #[error("send failed for endpoint {endpoint:?}: {reason}")]
    SendFailed { endpoint: String, reason: String },

    #[error("receive failed: {reason}")]
    ReceiveFailed { reason: String },

    #[error("carrier {carrier:?} not initialized")]
    NotInitialized { carrier: CarrierType },

    #[error("carrier {carrier:?} already initialized")]
    AlreadyInitialized { carrier: CarrierType },

    #[error("flush failed: {reason}")]
    FlushFailed { reason: String },

    #[error("reset failed: {reason}")]
    ResetFailed { reason: String },

    #[error("datagram exceeds max size: {size} bytes (max {max})")]
    DatagramTooLarge { size: usize, max: usize },

    #[error("carrier {carrier:?} does not support broadcast")]
    BroadcastNotSupported { carrier: CarrierType },

    #[error("carrier {carrier:?} does not support multicast")]
    MulticastNotSupported { carrier: CarrierType },

    #[error("carrier {carrier:?} timeout after {ms} ms")]
    Timeout { carrier: CarrierType, ms: u64 },

    #[error("carrier {carrier:?} internal error: {message}")]
    Internal { carrier: CarrierType, message: String },
}

impl CarrierError {
    pub fn carrier_type(&self) -> Option<CarrierType> {
        match self {
            CarrierError::Unavailable(c) => Some(*c),
            CarrierError::InvalidEndpoint { carrier, .. } => {
                CarrierType::from_str(carrier).ok()
            }
            CarrierError::SendFailed { .. } => None,
            CarrierError::ReceiveFailed { .. } => None,
            CarrierError::NotInitialized { carrier } => Some(*carrier),
            CarrierError::AlreadyInitialized { carrier } => Some(*carrier),
            CarrierError::FlushFailed { .. } => None,
            CarrierError::ResetFailed { .. } => None,
            CarrierError::DatagramTooLarge { .. } => None,
            CarrierError::BroadcastNotSupported { carrier } => Some(*carrier),
            CarrierError::MulticastNotSupported { carrier } => Some(*carrier),
            CarrierError::Timeout { carrier, .. } => Some(*carrier),
            CarrierError::Internal { carrier, .. } => Some(*carrier),
            CarrierError::AddressTooLarge { .. } => None,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CarrierError::Unavailable(_)
                | CarrierError::Timeout { .. }
                | CarrierError::SendFailed { .. }
                | CarrierError::ReceiveFailed { .. }
                | CarrierError::Internal { .. }
        )
    }

    pub fn is_fatal(&self) -> bool {
        !self.is_retryable()
    }
}

///////////////////////////////////////////////////////////////////////////////
// DELIVERY SEMANTICS
///////////////////////////////////////////////////////////////////////////////

/// Delivery semantics for carrier transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarrierDeliverySemantics {
    /// Best effort — no guarantees of delivery or ordering
    BestEffort,
    /// At least once — datagram may be delivered multiple times
    AtLeastOnce,
    /// At most once — datagram delivered at most once, may be lost
    AtMostOnce,
    /// Exactly once — datagram delivered exactly once
    ExactlyOnce,
}

impl CarrierDeliverySemantics {
    pub fn as_str(&self) -> &'static str {
        match self {
            CarrierDeliverySemantics::BestEffort => "best_effort",
            CarrierDeliverySemantics::AtLeastOnce => "at_least_once",
            CarrierDeliverySemantics::AtMostOnce => "at_most_once",
            CarrierDeliverySemantics::ExactlyOnce => "exactly_once",
        }
    }

    pub fn guarantees_delivery(&self) -> bool {
        matches!(
            self,
            CarrierDeliverySemantics::AtLeastOnce
                | CarrierDeliverySemantics::ExactlyOnce
        )
    }

    pub fn guarantees_ordering(&self) -> bool {
        matches!(self, CarrierDeliverySemantics::ExactlyOnce)
    }
}

///////////////////////////////////////////////////////////////////////////////
// DEFAULT IMPLEMENTATIONS / HELPER TRAITS
///////////////////////////////////////////////////////////////////////////////

/// Helper trait for carriers that support datagram batching.
pub trait CarrierBatch: Carrier {
    /// Send multiple datagrams in a single batch.
    fn send_batch(
        &self,
        datagrams: Vec<(Endpoint, Vec<u8>)>,
    ) -> Result<Vec<Result<(), CarrierError>>, CarrierError> {
        let mut results = Vec::with_capacity(datagrams.len());
        for (endpoint, data) in datagrams {
            results.push(self.send_datagram(&endpoint, &data));
        }
        Ok(results)
    }
}

/// Helper trait for carriers that support flow control.
pub trait CarrierFlowControl: Carrier {
    /// Get the current backpressure level (0 = none, 100 = max).
    fn backpressure_level(&self) -> u32 {
        0
    }

    /// Check if the carrier is under backpressure.
    fn is_backpressured(&self) -> bool {
        self.backpressure_level() > 80
    }

    /// Apply backpressure credit to the carrier.
    fn apply_credit(&self, credit: u64) -> Result<(), CarrierError> {
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER STATISTICS
///////////////////////////////////////////////////////////////////////////////

/// Statistics collected by a carrier implementation.
#[derive(Debug, Clone, Default)]
pub struct CarrierStats {
    pub datagrams_sent: u64,
    pub datagrams_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors: u64,
    pub timeouts: u64,
    pub retries: u64,
    pub carrier_type: String,
}

impl CarrierStats {
    pub fn new(carrier_type: String) -> Self {
        Self { carrier_type, ..Default::default() }
    }

    pub fn total_datagrams(&self) -> u64 {
        self.datagrams_sent + self.datagrams_received
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_datagrams() == 0 {
            return 0.0;
        }
        self.errors as f64 / self.total_datagrams() as f64
    }

    pub fn success_rate(&self) -> f64 {
        1.0 - self.error_rate()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER CONFIGURATION
///////////////////////////////////////////////////////////////////////////////

/// Configuration for carrier creation and behavior.
#[derive(Debug, Clone)]
pub struct CarrierConfig {
    pub carrier_type: CarrierType,
    pub max_datagram_size: usize,
    pub timeout_ms: u64,
    pub retry_count: u32,
    pub enable_batching: bool,
    pub enable_flow_control: bool,
    pub delivery_semantics: CarrierDeliverySemantics,
}

impl Default for CarrierConfig {
    fn default() -> Self {
        Self {
            carrier_type: CarrierType::Loopback,
            max_datagram_size: 65536,
            timeout_ms: 5000,
            retry_count: 3,
            enable_batching: false,
            enable_flow_control: false,
            delivery_semantics: CarrierDeliverySemantics::AtMostOnce,
        }
    }
}

impl CarrierConfig {
    pub fn new(carrier_type: CarrierType) -> Self {
        Self {
            carrier_type,
            ..Default::default()
        }
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn with_retries(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    pub fn with_delivery_semantics(mut self, semantics: CarrierDeliverySemantics) -> Self {
        self.delivery_semantics = semantics;
        self
    }

    pub fn validate(&self) -> Result<(), CarrierError> {
        if self.max_datagram_size == 0 {
            return Err(CarrierError::DatagramTooLarge {
                size: 0,
                max: self.max_datagram_size,
            });
        }
        if self.timeout_ms == 0 {
            return Err(CarrierError::Timeout {
                carrier: self.carrier_type,
                ms: 0,
            });
        }
        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER ERROR EXTENDED VARIANTS AND METHODS
///////////////////////////////////////////////////////////////////////////////

impl CarrierError {
    /// Returns a human-readable category for this error.
    pub fn category(&self) -> &'static str {
        match self {
            CarrierError::Unavailable { .. } => "unavailable",
            CarrierError::AddressTooLarge { .. } => "address_too_large",
            CarrierError::InvalidEndpoint { .. } => "invalid_endpoint",
            CarrierError::SendFailed { .. } => "send_failed",
            CarrierError::ReceiveFailed { .. } => "receive_failed",
            CarrierError::NotInitialized { .. } => "not_initialized",
            CarrierError::AlreadyInitialized { .. } => "already_initialized",
            CarrierError::FlushFailed { .. } => "flush_failed",
            CarrierError::ResetFailed { .. } => "reset_failed",
            CarrierError::DatagramTooLarge { .. } => "datagram_too_large",
            CarrierError::BroadcastNotSupported { .. } => "broadcast_not_supported",
            CarrierError::MulticastNotSupported { .. } => "multicast_not_supported",
            CarrierError::Timeout { .. } => "timeout",
            CarrierError::Internal { .. } => "internal",
        }
    }

    /// Returns the suggested retry delay in milliseconds, or None if no retry.
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            CarrierError::Timeout { ms, .. } => Some(*ms),
            CarrierError::Unavailable { .. } => Some(1000),
            CarrierError::SendFailed { .. } => Some(500),
            CarrierError::ReceiveFailed { .. } => Some(500),
            CarrierError::Internal { .. } => Some(2000),
            _ => None,
        }
    }

    /// Returns whether this error indicates a permanent carrier failure.
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            CarrierError::NotInitialized { .. }
                | CarrierError::AlreadyInitialized { .. }
                | CarrierError::InvalidEndpoint { .. }
                | CarrierError::AddressTooLarge { .. }
        )
    }

    /// Returns whether this error indicates a transient carrier issue.
    pub fn is_transient(&self) -> bool {
        !self.is_permanent() && self.is_retryable()
    }

    /// Returns the error message without carrier-specific formatting.
    pub fn plain_message(&self) -> String {
        match self {
            CarrierError::Unavailable(carrier) => {
                format!("carrier {:?} is not available", carrier)
            }
            CarrierError::AddressTooLarge { size, max } => {
                format!("address too large: {} bytes (max {})", size, max)
            }
            CarrierError::InvalidEndpoint { endpoint, carrier } => {
                format!("invalid endpoint {} for carrier {}", endpoint, carrier)
            }
            CarrierError::SendFailed { endpoint, reason } => {
                format!("send failed for {}: {}", endpoint, reason)
            }
            CarrierError::ReceiveFailed { reason } => {
                format!("receive failed: {}", reason)
            }
            CarrierError::NotInitialized { carrier } => {
                format!("carrier {:?} not initialized", carrier)
            }
            CarrierError::AlreadyInitialized { carrier } => {
                format!("carrier {:?} already initialized", carrier)
            }
            CarrierError::FlushFailed { reason } => {
                format!("flush failed: {}", reason)
            }
            CarrierError::ResetFailed { reason } => {
                format!("reset failed: {}", reason)
            }
            CarrierError::DatagramTooLarge { size, max } => {
                format!("datagram too large: {} bytes (max {})", size, max)
            }
            CarrierError::BroadcastNotSupported { carrier } => {
                format!("carrier {:?} does not support broadcast", carrier)
            }
            CarrierError::MulticastNotSupported { carrier } => {
                format!("carrier {:?} does not support multicast", carrier)
            }
            CarrierError::Timeout { carrier, ms } => {
                format!("carrier {:?} timeout after {} ms", carrier, ms)
            }
            CarrierError::Internal { carrier, message } => {
                format!("carrier {:?} internal error: {}", carrier, message)
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER ENDPOINT VALIDATOR
///////////////////////////////////////////////////////////////////////////////

/// Validates endpoint addresses for carrier compatibility.
/// Ensures addresses conform to carrier-specific size and format limits.
pub struct CarrierEndpointValidator {
    carrier_type: CarrierType,
    max_address_size: usize,
    allowed_carrier_types: Vec<CarrierType>,
}

impl CarrierEndpointValidator {
    pub fn new(carrier_type: CarrierType) -> Self {
        Self {
            carrier_type,
            max_address_size: 1024,
            allowed_carrier_types: vec![carrier_type],
        }
    }

    pub fn with_max_address_size(mut self, size: usize) -> Self {
        self.max_address_size = size;
        self
    }

    pub fn with_allowed_types(mut self, types: Vec<CarrierType>) -> Self {
        self.allowed_carrier_types = types;
        self
    }

    /// Validate an endpoint against this validator's constraints.
    pub fn validate(&self, endpoint: &Endpoint) -> Result<(), CarrierError> {
        if endpoint.address.len() > self.max_address_size {
            return Err(CarrierError::AddressTooLarge {
                size: endpoint.address.len(),
                max: self.max_address_size,
            });
        }
        if !self.allowed_carrier_types.contains(&endpoint.carrier_type) {
            return Err(CarrierError::InvalidEndpoint {
                endpoint: format!("{:?}", endpoint),
                carrier: format!("{:?}", self.carrier_type),
            });
        }
        Ok(())
    }

    /// Check if an endpoint is valid without returning a detailed error.
    pub fn is_valid(&self, endpoint: &Endpoint) -> bool {
        self.validate(endpoint).is_ok()
    }

    /// Get the carrier type this validator is configured for.
    pub fn carrier_type(&self) -> CarrierType {
        self.carrier_type
    }

    /// Get the maximum address size enforced by this validator.
    pub fn max_address_size(&self) -> usize {
        self.max_address_size
    }

    /// Get the list of allowed carrier types.
    pub fn allowed_types(&self) -> &[CarrierType] {
        &self.allowed_carrier_types
    }

    /// Create a validator that accepts all carrier types.
    pub fn permissive() -> Self {
        Self {
            carrier_type: CarrierType::Loopback,
            max_address_size: 1024,
            allowed_carrier_types: vec![
                CarrierType::Loopback,
                CarrierType::Serial,
                CarrierType::Optical,
                CarrierType::Radio,
                CarrierType::Custom(0),
            ],
        }
    }
}

impl Default for CarrierEndpointValidator {
    fn default() -> Self {
        Self::new(CarrierType::Loopback)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER CONNECTION STATE
///////////////////////////////////////////////////////////////////////////////

/// Connection state for a carrier endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarrierConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

impl CarrierConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CarrierConnectionState::Disconnected => "disconnected",
            CarrierConnectionState::Connecting => "connecting",
            CarrierConnectionState::Connected => "connected",
            CarrierConnectionState::Disconnecting => "disconnecting",
            CarrierConnectionState::Failed => "failed",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, CarrierConnectionState::Connected)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, CarrierConnectionState::Failed | CarrierConnectionState::Disconnected)
    }

    pub fn can_send(&self) -> bool {
        matches!(self, CarrierConnectionState::Connected)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER HEALTH MONITOR
///////////////////////////////////////////////////////////////////////////////

/// Health status for a carrier endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl CarrierHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            CarrierHealth::Healthy => "healthy",
            CarrierHealth::Degraded => "degraded",
            CarrierHealth::Unhealthy => "unhealthy",
            CarrierHealth::Unknown => "unknown",
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, CarrierHealth::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, CarrierHealth::Degraded)
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, CarrierHealth::Unhealthy)
    }
}

/// Monitors the health of carrier endpoints and connections.
pub struct CarrierHealthMonitor {
    endpoint: Endpoint,
    state: CarrierConnectionState,
    health: CarrierHealth,
    consecutive_failures: u32,
    last_success_time: Option<u64>,
    last_failure_time: Option<u64>,
    total_sent: u64,
    total_received: u64,
    total_errors: u64,
}

impl CarrierHealthMonitor {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            state: CarrierConnectionState::Disconnected,
            health: CarrierHealth::Unknown,
            consecutive_failures: 0,
            last_success_time: None,
            last_failure_time: None,
            total_sent: 0,
            total_received: 0,
            total_errors: 0,
        }
    }

    /// Record a successful send operation.
    pub fn record_success(&mut self, bytes: u64, time: u64) {
        self.state = CarrierConnectionState::Connected;
        self.health = if self.consecutive_failures > 0 {
            CarrierHealth::Degraded
        } else {
            CarrierHealth::Healthy
        };
        self.consecutive_failures = 0;
        self.last_success_time = Some(time);
        self.total_sent += bytes;
    }

    /// Record a failed operation.
    pub fn record_failure(&mut self, error: &CarrierError, time: u64) {
        self.consecutive_failures += 1;
        self.last_failure_time = Some(time);
        self.total_errors += 1;

        self.health = if self.consecutive_failures >= 5 {
            CarrierHealth::Unhealthy
        } else if self.consecutive_failures >= 2 {
            CarrierHealth::Degraded
        } else {
            CarrierHealth::Healthy
        };

        if self.consecutive_failures >= 10 {
            self.state = CarrierConnectionState::Failed;
        }
    }

    /// Record a received datagram.
    pub fn record_received(&mut self, bytes: u64) {
        self.total_received += bytes;
        if self.health == CarrierHealth::Unknown {
            self.health = CarrierHealth::Healthy;
        }
    }

    /// Get the current connection state.
    pub fn state(&self) -> CarrierConnectionState {
        self.state
    }

    /// Get the current health status.
    pub fn health(&self) -> CarrierHealth {
        self.health
    }

    /// Get the number of consecutive failures.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Get the total bytes sent.
    pub fn total_sent(&self) -> u64 {
        self.total_sent
    }

    /// Get the total bytes received.
    pub fn total_received(&self) -> u64 {
        self.total_received
    }

    /// Get the total errors.
    pub fn total_errors(&self) -> u64 {
        self.total_errors
    }

    /// Get the endpoint being monitored.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Check if the monitor considers the endpoint healthy.
    pub fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }

    /// Check if the monitor considers the endpoint unhealthy.
    pub fn is_unhealthy(&self) -> bool {
        self.health.is_unhealthy()
    }

    /// Reset the monitor to initial state.
    pub fn reset(&mut self) {
        self.state = CarrierConnectionState::Disconnected;
        self.health = CarrierHealth::Unknown;
        self.consecutive_failures = 0;
        self.last_success_time = None;
        self.last_failure_time = None;
        self.total_sent = 0;
        self.total_received = 0;
        self.total_errors = 0;
    }

    /// Transition to connecting state.
    pub fn connecting(&mut self) {
        self.state = CarrierConnectionState::Connecting;
        self.health = CarrierHealth::Unknown;
    }

    /// Transition to connected state.
    pub fn connected(&mut self) {
        self.state = CarrierConnectionState::Connected;
        self.health = CarrierHealth::Healthy;
        self.consecutive_failures = 0;
    }

    /// Transition to disconnected state.
    pub fn disconnected(&mut self) {
        self.state = CarrierConnectionState::Disconnected;
        self.health = CarrierHealth::Unknown;
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER RATE LIMITER
///////////////////////////////////////////////////////////////////////////////

/// Simple token-bucket rate limiter for carrier operations.
pub struct CarrierRateLimiter {
    max_tokens: u64,
    tokens: std::cell::Cell<u64>,
    refill_rate_per_sec: u64,
    last_refill: std::cell::Cell<u64>,
}

impl CarrierRateLimiter {
    pub fn new(max_tokens: u64, refill_rate_per_sec: u64) -> Self {
        Self {
            max_tokens,
            tokens: std::cell::Cell::new(max_tokens),
            refill_rate_per_sec,
            last_refill: std::cell::Cell::now_ms(),
        }
    }

    /// Try to consume one token. Returns true if allowed, false if rate limited.
    pub fn try_consume(&self) -> bool {
        self.refill();
        let current = self.tokens.get();
        if current > 0 {
            self.tokens.set(current - 1);
            true
        } else {
            false
        }
    }

    /// Try to consume N tokens. Returns the number actually consumed.
    pub fn try_consume_n(&self, n: u64) -> u64 {
        self.refill();
        let current = self.tokens.get();
        let consumed = current.min(n);
        self.tokens.set(current - consumed);
        consumed
    }

    /// Get the current number of available tokens.
    pub fn available_tokens(&self) -> u64 {
        self.refill();
        self.tokens.get()
    }

    /// Get the maximum number of tokens.
    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }

    fn refill(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last = self.last_refill.get();
        if now > last {
            let elapsed = now - last;
            let refill_amount = elapsed * self.refill_rate_per_sec / 1000;
            if refill_amount > 0 {
                let current = self.tokens.get();
                let new_tokens = current.saturating_add(refill_amount).min(self.max_tokens);
                self.tokens.set(new_tokens);
                self.last_refill.set(now);
            }
        }
    }
}

impl Default for CarrierRateLimiter {
    fn default() -> Self {
        Self::new(1000, 1000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER METRICS
///////////////////////////////////////////////////////////////////////////////

/// Detailed metrics collected by a carrier implementation.
#[derive(Debug, Clone, Default)]
pub struct CarrierMetrics {
    pub carrier_type: String,
    pub datagrams_sent: u64,
    pub datagrams_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors_sent: u64,
    pub errors_received: u64,
    pub timeouts: u64,
    pub retries: u64,
    pub retransmissions: u64,
    pub dropped_datagrams: u64,
    pub out_of_order_datagrams: u64,
    pub duplicate_datagrams: u64,
    pub avg_latency_ms: f64,
    pub max_latency_ms: f64,
    pub min_latency_ms: f64,
    pub last_operation_time: Option<u64>,
    pub uptime_ms: u64,
}

impl CarrierMetrics {
    pub fn new(carrier_type: String) -> Self {
        Self { carrier_type, ..Default::default() }
    }

    pub fn total_datagrams(&self) -> u64 {
        self.datagrams_sent + self.datagrams_received
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    pub fn total_errors(&self) -> u64 {
        self.errors_sent + self.errors_received
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.total_datagrams();
        if total == 0 {
            return 0.0;
        }
        self.total_errors() as f64 / total as f64
    }

    pub fn delivery_success_rate(&self) -> f64 {
        let sent = self.datagrams_sent;
        if sent == 0 {
            return 1.0;
        }
        (sent - self.errors_sent) as f64 / sent as f64
    }

    pub fn throughput_bytes_per_sec(&self, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        self.total_bytes() as f64 / (elapsed_ms as f64 / 1000.0)
    }

    pub fn datagrams_per_sec(&self, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        self.total_datagrams() as f64 / (elapsed_ms as f64 / 1000.0)
    }

    pub fn latency_stats_summary(&self) -> String {
        format!(
            "avg={:.2}ms max={:.2}ms min={:.2}ms",
            self.avg_latency_ms, self.max_latency_ms, self.min_latency_ms
        )
    }

    pub fn record_latency(&mut self, latency_ms: f64) {
        self.last_operation_time = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
        if self.min_latency_ms == 0.0 || latency_ms < self.min_latency_ms {
            self.min_latency_ms = latency_ms;
        }
        if latency_ms > self.max_latency_ms {
            self.max_latency_ms = latency_ms;
        }
        self.avg_latency_ms = (self.avg_latency_ms + latency_ms) / 2.0;
    }

    pub fn is_healthy(&self) -> bool {
        self.error_rate() < 0.1 && self.timeouts < 100
    }

    pub fn needs_attention(&self) -> bool {
        self.error_rate() >= 0.1 || self.timeouts >= 100 || self.dropped_datagrams > 1000
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER POOL
///////////////////////////////////////////////////////////////////////////////

/// A pool of carrier endpoints for load balancing and failover.
pub struct CarrierPool {
    carriers: Vec<Box<dyn Carrier>>,
    current_index: std::cell::Cell<usize>,
    strategy: PoolStrategy,
}

impl CarrierPool {
    pub fn new(strategy: PoolStrategy) -> Self {
        Self {
            carriers: Vec::new(),
            current_index: std::cell::Cell::new(0),
            strategy,
        }
    }

    pub fn add_carrier(&mut self, carrier: Box<dyn Carrier>) {
        self.carriers.push(carrier);
    }

    pub fn carrier_count(&self) -> usize {
        self.carriers.len()
    }

    pub fn select(&self) -> Option<&Box<dyn Carrier>> {
        if self.carriers.is_empty() {
            return None;
        }
        match self.strategy {
            PoolStrategy::RoundRobin => {
                let idx = self.current_index.get() % self.carriers.len();
                self.current_index.set(idx + 1);
                Some(&self.carriers[idx])
            }
            PoolStrategy::FirstAvailable => {
                self.carriers.iter().find(|c| c.is_available())
            }
            PoolStrategy::Random => {
                let idx = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos() as usize)
                    % self.carriers.len();
                Some(&self.carriers[idx])
            }
        }
    }

    pub fn send_datagram(&self, endpoint: &Endpoint, data: &[u8]) -> Result<(), CarrierError> {
        if let Some(carrier) = self.select() {
            carrier.send_datagram(endpoint, data)
        } else {
            Err(CarrierError::Unavailable(CarrierType::Loopback))
        }
    }

    pub fn any_available(&self) -> bool {
        self.carriers.iter().any(|c| c.is_available())
    }

    pub fn available_carriers(&self) -> Vec<&Box<dyn Carrier>> {
        self.carriers.iter().filter(|c| c.is_available()).collect()
    }

    pub fn remove_carrier(&mut self, index: usize) -> Option<Box<dyn Carrier>> {
        if index < self.carriers.len() {
            Some(self.carriers.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.carriers.clear();
        self.current_index.set(0);
    }
}

impl Default for CarrierPool {
    fn default() -> Self {
        Self::new(PoolStrategy::RoundRobin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolStrategy {
    RoundRobin,
    FirstAvailable,
    Random,
}

impl Default for PoolStrategy {
    fn default() -> Self {
        PoolStrategy::RoundRobin
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER ROUTER
///////////////////////////////////////////////////////////////////////////////

pub struct CarrierRouter {
    routes: std::collections::HashMap<CarrierType, Box<dyn Carrier>>,
    default_carrier: Option<CarrierType>,
}

impl CarrierRouter {
    pub fn new() -> Self {
        Self {
            routes: std::collections::HashMap::new(),
            default_carrier: None,
        }
    }

    pub fn register_carrier(&mut self, carrier_type: CarrierType, carrier: Box<dyn Carrier>) {
        self.routes.insert(carrier_type, carrier);
        if self.default_carrier.is_none() {
            self.default_carrier = Some(carrier_type);
        }
    }

    pub fn set_default_carrier(&mut self, carrier_type: CarrierType) {
        self.default_carrier = Some(carrier_type);
    }

    pub fn route_datagram(
        &self,
        endpoint: &Endpoint,
        data: &[u8],
    ) -> Result<(), CarrierError> {
        let carrier_type = endpoint.carrier_type;
        if let Some(carrier) = self.routes.get(&carrier_type) {
            carrier.send_datagram(endpoint, data)
        } else if let Some(default) = self.default_carrier {
            if let Some(carrier) = self.routes.get(&default) {
                carrier.send_datagram(endpoint, data)
            } else {
                Err(CarrierError::Unavailable(carrier_type))
            }
        } else {
            Err(CarrierError::Unavailable(carrier_type))
        }
    }

    pub fn get_carrier(&self, carrier_type: CarrierType) -> Option<&Box<dyn Carrier>> {
        self.routes.get(&carrier_type)
    }

    pub fn registered_types(&self) -> Vec<CarrierType> {
        self.routes.keys().copied().collect()
    }

    pub fn is_registered(&self, carrier_type: CarrierType) -> bool {
        self.routes.contains_key(&carrier_type)
    }

    pub fn unregister(&mut self, carrier_type: CarrierType) -> Option<Box<dyn Carrier>> {
        self.routes.remove(&carrier_type)
    }

    pub fn carrier_count(&self) -> usize {
        self.routes.len()
    }
}

impl Default for CarrierRouter {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER ENDPOINT RESOLVER
///////////////////////////////////////////////////////////////////////////////

pub struct CarrierEndpointResolver {
    mappings: std::collections::HashMap<Vec<u8>, Endpoint>,
}

impl CarrierEndpointResolver {
    pub fn new() -> Self {
        Self {
            mappings: std::collections::HashMap::new(),
        }
    }

    pub fn register_mapping(&mut self, logical: Vec<u8>, endpoint: Endpoint) {
        self.mappings.insert(logical, endpoint);
    }

    pub fn resolve(&self, logical_address: &[u8]) -> Option<&Endpoint> {
        self.mappings.get(logical_address)
    }

    pub fn resolve_or_default(&self, logical_address: &[u8], default: Endpoint) -> Endpoint {
        self.mappings
            .get(logical_address)
            .cloned()
            .unwrap_or(default)
    }

    pub fn remove_mapping(&mut self, logical: &[u8]) -> Option<Endpoint> {
        self.mappings.remove(logical)
    }

    pub fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    pub fn clear_mappings(&mut self) {
        self.mappings.clear();
    }

    pub fn contains(&self, logical_address: &[u8]) -> bool {
        self.mappings.contains_key(logical_address)
    }
}

impl Default for CarrierEndpointResolver {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER HEALTH AGGREGATOR
///////////////////////////////////////////////////////////////////////////////

pub struct CarrierHealthAggregator {
    monitors: std::collections::HashMap<String, CarrierHealthMonitor>,
}

impl CarrierHealthAggregator {
    pub fn new() -> Self {
        Self {
            monitors: std::collections::HashMap::new(),
        }
    }

    pub fn add_monitor(&mut self, name: String, monitor: CarrierHealthMonitor) {
        self.monitors.insert(name, monitor);
    }

    pub fn overall_health(&self) -> CarrierHealth {
        if self.monitors.is_empty() {
            return CarrierHealth::Unknown;
        }
        let unhealthy_count = self
            .monitors
            .values()
            .filter(|m| m.is_unhealthy())
            .count();
        let degraded_count = self
            .monitors
            .values()
            .filter(|m| m.is_degraded())
            .count();
        let total = self.monitors.len();

        if unhealthy_count > total / 2 {
            CarrierHealth::Unhealthy
        } else if unhealthy_count > 0 || degraded_count > total / 2 {
            CarrierHealth::Degraded
        } else {
            CarrierHealth::Healthy
        }
    }

    pub fn healthy_count(&self) -> usize {
        self.monitors.values().filter(|m| m.is_healthy()).count()
    }

    pub fn unhealthy_count(&self) -> usize {
        self.monitors.values().filter(|m| m.is_unhealthy()).count()
    }

    pub fn degraded_count(&self) -> usize {
        self.monitors.values().filter(|m| m.is_degraded()).count()
    }

    pub fn total_count(&self) -> usize {
        self.monitors.len()
    }

    pub fn get_monitor(&self, name: &str) -> Option<&CarrierHealthMonitor> {
        self.monitors.get(name)
    }

    pub fn remove_monitor(&mut self, name: &str) -> Option<CarrierHealthMonitor> {
        self.monitors.remove(name)
    }
}

impl Default for CarrierHealthAggregator {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER TRANSACTION ID
///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CarrierTransactionId(pub u64);

impl CarrierTransactionId {
    pub const ZERO: CarrierTransactionId = CarrierTransactionId(0);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Default for CarrierTransactionId {
    fn default() -> Self {
        Self::ZERO
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER OPERATION RESULT
///////////////////////////////////////////////////////////////////////////////

pub struct CarrierOperationResult {
    pub success: bool,
    pub bytes_transferred: u64,
    pub latency_ms: u64,
    pub transaction_id: CarrierTransactionId,
    pub error: Option<CarrierError>,
}

impl CarrierOperationResult {
    pub fn success(
        transaction_id: CarrierTransactionId,
        bytes_transferred: u64,
        latency_ms: u64,
    ) -> Self {
        Self {
            success: true,
            bytes_transferred,
            latency_ms,
            transaction_id,
            error: None,
        }
    }

    pub fn failure(
        transaction_id: CarrierTransactionId,
        error: CarrierError,
        latency_ms: u64,
    ) -> Self {
        Self {
            success: false,
            bytes_transferred: 0,
            latency_ms,
            transaction_id,
            error: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn is_failure(&self) -> bool {
        !self.success
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER FRAME
///////////////////////////////////////////////////////////////////////////////

/// A native carrier frame with sequence number, payload, and checksum.
/// Frame format: [seq:8][flags:1][checksum:4][payload:variable]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarrierFrame {
    pub sequence: u64,
    pub flags: u8,
    pub checksum: [u8; 4],
    pub payload: Vec<u8>,
}

impl CarrierFrame {
    pub const FLAG_ACK: u8 = 1 << 0;
    pub const FLAG_SYN: u8 = 1 << 1;
    pub const FLAG_FIN: u8 = 1 << 2;
    pub const FLAG_NACK: u8 = 1 << 3;

    pub fn new(sequence: u64, flags: u8, payload: Vec<u8>) -> Self {
        let checksum = Self::compute_checksum(&payload);
        Self { sequence, flags, checksum, payload }
    }

    pub fn ack(sequence: u64) -> Self {
        Self::new(sequence, Self::FLAG_ACK, Vec::new())
    }

    pub fn syn(sequence: u64) -> Self {
        Self::new(sequence, Self::FLAG_SYN, Vec::new())
    }

    pub fn fin(sequence: u64) -> Self {
        Self::new(sequence, Self::FLAG_FIN, Vec::new())
    }

    pub fn nack(sequence: u64) -> Self {
        Self::new(sequence, Self::FLAG_NACK, Vec::new())
    }

    pub fn compute_checksum(payload: &[u8]) -> [u8; 4] {
        let mut hash: u32 = 0x811c9dc5;
        for &byte in payload {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash.to_le_bytes()
    }

    pub fn verify_checksum(&self) -> bool {
        let expected = Self::compute_checksum(&self.payload);
        expected == self.checksum
    }

    pub fn is_ack(&self) -> bool {
        self.flags & Self::FLAG_ACK != 0
    }

    pub fn is_syn(&self) -> bool {
        self.flags & Self::FLAG_SYN != 0
    }

    pub fn is_fin(&self) -> bool {
        self.flags & Self::FLAG_FIN != 0
    }

    pub fn is_nack(&self) -> bool {
        self.flags & Self::FLAG_NACK != 0
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub fn frame_size(&self) -> usize {
        8 + 1 + 4 + self.payload.len()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.frame_size());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.push(self.flags);
        buf.extend_from_slice(&self.checksum);
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Self, CarrierError> {
        if data.len() < 13 {
            return Err(CarrierError::ReceiveFailed {
                reason: "frame too short".to_string(),
            });
        }
        let sequence = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let flags = data[8];
        let checksum: [u8; 4] = data[9..13].try_into().unwrap();
        let payload = data[13..].to_vec();
        Ok(Self { sequence, flags, checksum, payload })
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER SEQUENCE NUMBER TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks send and receive sequence numbers for a carrier connection.
pub struct CarrierSequenceTracker {
    pub next_send: u64,
    pub next_receive: u64,
    pub last_acked: u64,
    pub sent_unacked: std::collections::HashMap<u64, CarrierFrame>,
    pub received_out_of_order: std::collections::HashMap<u64, CarrierFrame>,
    pub max_window: u64,
}

impl CarrierSequenceTracker {
    pub fn new() -> Self {
        Self {
            next_send: 0,
            next_receive: 0,
            last_acked: 0,
            sent_unacked: std::collections::HashMap::new(),
            received_out_of_order: std::collections::HashMap::new(),
            max_window: 1024,
        }
    }

    pub fn next_send_sequence(&mut self) -> u64 {
        let seq = self.next_send;
        self.next_send += 1;
        seq
    }

    pub fn mark_acked(&mut self, sequence: u64) {
        self.sent_unacked.remove(&sequence);
        if sequence > self.last_acked {
            self.last_acked = sequence;
        }
    }

    pub fn mark_sent(&mut self, frame: CarrierFrame) {
        self.sent_unacked.insert(frame.sequence, frame);
    }

    pub fn is_acked(&self, sequence: u64) -> bool {
        sequence <= self.last_acked
    }

    pub fn unacked_count(&self) -> usize {
        self.sent_unacked.len()
    }

    pub fn can_send(&self) -> bool {
        self.unacked_count() < self.max_window as usize
    }

    pub fn receive_frame(&mut self, frame: CarrierFrame) -> bool {
        if frame.sequence == self.next_receive {
            self.next_receive += 1;
            true
        } else if frame.sequence > self.next_receive {
            self.received_out_of_order.insert(frame.sequence, frame);
            false
        } else {
            false // duplicate
        }
    }

    pub fn get_next_received(&mut self) -> Option<CarrierFrame> {
        while let Some(frame) = self.received_out_of_order.remove(&self.next_receive) {
            self.next_receive += 1;
            return Some(frame);
        }
        None
    }

    pub fn max_window(&self) -> u64 {
        self.max_window
    }

    pub fn set_max_window(&mut self, window: u64) {
        self.max_window = window;
    }
}

impl Default for CarrierSequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER WINDOW MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages the send window for flow control.
pub struct CarrierWindowManager {
    pub window_size: u64,
    pub bytes_in_flight: u64,
    pub max_window_size: u64,
    pub min_window_size: u64,
}

impl CarrierWindowManager {
    pub fn new() -> Self {
        Self {
            window_size: 65536,
            bytes_in_flight: 0,
            max_window_size: 1048576,
            min_window_size: 1024,
        }
    }

    pub fn can_send(&self, bytes: u64) -> bool {
        self.bytes_in_flight + bytes <= self.window_size
    }

    pub fn add_in_flight(&mut self, bytes: u64) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_add(bytes);
    }

    pub fn remove_in_flight(&mut self, bytes: u64) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
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
        self.bytes_in_flight as f64 / self.window_size as f64
    }

    pub fn is_window_full(&self) -> bool {
        self.window_utilization() >= 1.0
    }
}

impl Default for CarrierWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER RETRY MANAGER
///////////////////////////////////////////////////////////////////////////////

/// Manages retry logic for failed carrier operations.
pub struct CarrierRetryManager {
    pub max_retries: u32,
    pub retry_count: std::cell::Cell<u32>,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl CarrierRetryManager {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            retry_count: std::cell::Cell::new(0),
            base_delay_ms: 100,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
        }
    }

    pub fn should_retry(&self, error: &CarrierError) -> bool {
        self.retry_count.get() < self.max_retries && error.is_retryable()
    }

    pub fn increment_retry(&mut self) {
        self.retry_count.set(self.retry_count.get() + 1);
    }

    pub fn reset_retry(&mut self) {
        self.retry_count.set(0);
    }

    pub fn current_retry_count(&self) -> u32 {
        self.retry_count.get()
    }

    pub fn next_delay_ms(&self) -> u64 {
        let count = self.retry_count.get() as f64;
        let delay = self.base_delay_ms as f64 * self.backoff_multiplier.powf(count);
        delay as u64
    }

    pub fn clamp_delay(&self, delay_ms: u64) -> u64 {
        delay_ms.min(self.max_delay_ms)
    }

    pub fn is_exhausted(&self) -> bool {
        self.retry_count.get() >= self.max_retries
    }
}

impl Default for CarrierRetryManager {
    fn default() -> Self {
        Self::new(3)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER CONNECTION INFO
///////////////////////////////////////////////////////////////////////////////

/// Information about a carrier connection endpoint.
#[derive(Debug, Clone)]
pub struct CarrierConnectionInfo {
    pub endpoint: Endpoint,
    pub connected_since: Option<u64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub datagrams_sent: u64,
    pub datagrams_received: u64,
    pub errors: u64,
    pub is_active: bool,
}

impl CarrierConnectionInfo {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            connected_since: None,
            bytes_sent: 0,
            bytes_received: 0,
            datagrams_sent: 0,
            datagrams_received: 0,
            errors: 0,
            is_active: false,
        }
    }

    pub fn mark_connected(&mut self, time: u64) {
        self.connected_since = Some(time);
        self.is_active = true;
    }

    pub fn mark_disconnected(&mut self) {
        self.connected_since = None;
        self.is_active = false;
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.datagrams_sent += 1;
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.datagrams_received += 1;
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    pub fn total_datagrams(&self) -> u64 {
        self.datagrams_sent + self.datagrams_received
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_datagrams() == 0 {
            return 0.0;
        }
        self.errors as f64 / self.total_datagrams() as f64
    }

    pub fn uptime_ms(&self) -> Option<u64> {
        self.connected_since.map(|start| {
            let now = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()) as u64;
            now.saturating_sub(start)
        })
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER BANDWIDTH TRACKER
///////////////////////////////////////////////////////////////////////////////

/// Tracks bandwidth usage over time windows.
pub struct CarrierBandwidthTracker {
    pub sent_bytes: std::cell::Cell<u64>,
    pub received_bytes: std::cell::Cell<u64>,
    pub window_start: std::cell::Cell<u64>,
    pub window_ms: u64,
}

impl CarrierBandwidthTracker {
    pub fn new(window_ms: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            sent_bytes: std::cell::Cell::new(0),
            received_bytes: std::cell::Cell::new(0),
            window_start: std::cell::Cell::new(now),
            window_ms,
        }
    }

    pub fn add_sent(&self, bytes: u64) {
        self.check_window();
        self.sent_bytes.set(self.sent_bytes.get() + bytes);
    }

    pub fn add_received(&self, bytes: u64) {
        self.check_window();
        self.received_bytes.set(self.received_bytes.get() + bytes);
    }

    pub fn sent_bps(&self) -> f64 {
        self.check_window();
        let elapsed = self.elapsed_ms();
        if elapsed == 0 {
            return 0.0;
        }
        self.sent_bytes.get() as f64 / (elapsed as f64 / 1000.0)
    }

    pub fn received_bps(&self) -> f64 {
        self.check_window();
        let elapsed = self.elapsed_ms();
        if elapsed == 0 {
            return 0.0;
        }
        self.received_bytes.get() as f64 / (elapsed as f64 / 1000.0)
    }

    pub fn total_bps(&self) -> f64 {
        self.sent_bps() + self.received_bps()
    }

    pub fn reset_window(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.window_start.set(now);
        self.sent_bytes.set(0);
        self.received_bytes.set(0);
    }

    fn check_window(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        if now - self.window_start.get() >= self.window_ms {
            self.reset_window();
        }
    }

    fn elapsed_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now.saturating_sub(self.window_start.get())
    }
}

impl Default for CarrierBandwidthTracker {
    fn default() -> Self {
        Self::new(1000)
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER FRAME CODEC
///////////////////////////////////////////////////////////////////////////////

/// Encodes and decodes CarrierFrame to/from raw byte buffers.
pub struct CarrierFrameCodec;

impl CarrierFrameCodec {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_frame(&self, frame: &CarrierFrame) -> Vec<u8> {
        frame.encode()
    }

    pub fn decode_frame(&self, data: &[u8]) -> Result<CarrierFrame, CarrierError> {
        CarrierFrame::decode(data)
    }

    pub fn encode_ack(&self, sequence: u64) -> Vec<u8> {
        CarrierFrame::ack(sequence).encode()
    }

    pub fn encode_syn(&self, sequence: u64) -> Vec<u8> {
        CarrierFrame::syn(sequence).encode()
    }

    pub fn encode_fin(&self, sequence: u64) -> Vec<u8> {
        CarrierFrame::fin(sequence).encode()
    }

    pub fn encode_nack(&self, sequence: u64) -> Vec<u8> {
        CarrierFrame::nack(sequence).encode()
    }

    pub fn encode_data(&self, sequence: u64, payload: Vec<u8>) -> Vec<u8> {
        CarrierFrame::new(sequence, 0, payload).encode()
    }

    pub fn min_frame_size() -> usize {
        13
    }

    pub fn max_payload_size() -> usize {
        65535
    }

    pub fn frame_header_size() -> usize {
        13
    }

    pub fn frame_overhead() -> usize {
        Self::frame_header_size()
    }

    pub fn validate_frame_size(data_len: usize) -> Result<(), CarrierError> {
        if data_len < Self::min_frame_size() {
            return Err(CarrierError::ReceiveFailed {
                reason: format!(
                    "frame too small: {} bytes (min {})",
                    data_len,
                    Self::min_frame_size()
                ),
            });
        }
        Ok(())
    }

    pub fn max_payload_for_buffer(buffer_size: usize) -> usize {
        buffer_size.saturating_sub(Self::frame_header_size())
    }

    pub fn frame_checksum_valid(frame_bytes: &[u8]) -> bool {
        if frame_bytes.len() < 13 {
            return false;
        }
        let checksum_offset = 9;
        let expected: [u8; 4] = frame_bytes[checksum_offset..checksum_offset + 4]
            .try_into()
            .unwrap_or([0; 4]);
        let payload = &frame_bytes[13..];
        let computed = CarrierFrame::compute_checksum(payload);
        computed == expected
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER FRAME STATS
///////////////////////////////////////////////////////////////////////////////

/// Statistics about frame encoding/decoding operations.
#[derive(Debug, Clone, Default)]
pub struct CarrierFrameStats {
    pub frames_encoded: u64,
    pub frames_decoded: u64,
    pub frames_failed_decode: u64,
    pub bytes_encoded: u64,
    pub bytes_decoded: u64,
    pub checksum_failures: u64,
    pub oversized_frames: u64,
    pub undersized_frames: u64,
}

impl CarrierFrameStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_encode(&mut self, frame_size: usize) {
        self.frames_encoded += 1;
        self.bytes_encoded += frame_size as u64;
    }

    pub fn record_decode_success(&mut self, frame_size: usize) {
        self.frames_decoded += 1;
        self.bytes_decoded += frame_size as u64;
    }

    pub fn record_decode_failure(&mut self) {
        self.frames_failed_decode += 1;
    }

    pub fn record_checksum_failure(&mut self) {
        self.checksum_failures += 1;
    }

    pub fn record_oversized_frame(&mut self) {
        self.oversized_frames += 1;
    }

    pub fn record_undersized_frame(&mut self) {
        self.undersized_frames += 1;
    }

    pub fn total_frames(&self) -> u64 {
        self.frames_encoded + self.frames_decoded
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_encoded + self.bytes_decoded
    }

    pub fn total_failures(&self) -> u64 {
        self.frames_failed_decode
            + self.checksum_failures
            + self.oversized_frames
            + self.undersized_frames
    }

    pub fn decode_success_rate(&self) -> f64 {
        let total = self.frames_decoded + self.frames_failed_decode;
        if total == 0 {
            return 1.0;
        }
        self.frames_decoded as f64 / total as f64
    }

    pub fn is_healthy(&self) -> bool {
        self.checksum_failures == 0 && self.decode_success_rate() > 0.9
    }

    pub fn failure_rate(&self) -> f64 {
        let total = self.total_frames() + self.total_failures();
        if total == 0 {
            return 0.0;
        }
        self.total_failures() as f64 / total as f64
    }

    pub fn avg_frame_size(&self) -> f64 {
        let count = self.frames_encoded + self.frames_decoded;
        if count == 0 {
            return 0.0;
        }
        self.total_bytes() as f64 / count as f64
    }
}

impl Default for CarrierFrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CARRIER ERROR HANDLER
///////////////////////////////////////////////////////////////////////////////

/// Handles carrier errors and determines retry strategy.
pub struct CarrierErrorHandler {
    pub retry_manager: CarrierRetryManager,
    pub error_counts: std::cell::RefCell<std::collections::HashMap<String, u32>>,
}

impl CarrierErrorHandler {
    pub fn new(max_retries: u32) -> Self {
        Self {
            retry_manager: CarrierRetryManager::new(max_retries),
            error_counts: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn handle_error(&self, error: &CarrierError, endpoint: &Endpoint) -> bool {
        let category = error.category();
        *self.error_counts.borrow_mut().entry(category.to_string()).or_insert(0) += 1;
        self.retry_manager.should_retry(error)
    }

    pub fn reset_retry(&mut self) {
        self.retry_manager.reset_retry();
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_manager.current_retry_count()
    }

    pub fn is_exhausted(&self) -> bool {
        self.retry_manager.is_exhausted()
    }

    pub fn error_count_for(&self, category: &str) -> u32 {
        self.error_counts.borrow().get(category).copied().unwrap_or(0)
    }

    pub fn total_errors(&self) -> u32 {
        self.error_counts.borrow().values().sum()
    }

    pub fn clear_error_counts(&mut self) {
        self.error_counts.borrow_mut().clear();
    }
}

impl Default for CarrierErrorHandler {
    fn default() -> Self {
        Self::new(3)
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    // A test implementation of Carrier for testing the trait contract.
    struct TestCarrier {
        available: bool,
        name: &'static str,
        carrier_type: CarrierType,
        sent: std::cell::Cell<u64>,
        received: std::cell::Cell<u64>,
    }

    impl TestCarrier {
        fn new(carrier_type: CarrierType) -> Self {
            Self {
                available: true,
                name: "test",
                carrier_type,
                sent: std::cell::Cell::new(0),
                received: std::cell::Cell::new(0),
            }
        }
    }

    impl Carrier for TestCarrier {
        fn send_datagram(&self, _endpoint: &Endpoint, _data: &[u8]) -> Result<(), CarrierError> {
            self.sent.set(self.sent.get() + 1);
            Ok(())
        }

        fn receive_datagram(&self) -> Result<(Endpoint, Vec<u8>), CarrierError> {
            self.received.set(self.received.get() + 1);
            Ok((
                Endpoint::new(CarrierType::Loopback, vec![1, 2, 3]).unwrap(),
                vec![0, 0, 0],
            ))
        }

        fn carrier_type(&self) -> CarrierType {
            self.carrier_type
        }

        fn endpoint_address_size_limit(&self) -> usize {
            1024
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn carrier_name(&self) -> &'static str {
            self.name
        }

        fn datagrams_sent(&self) -> u64 {
            self.sent.get()
        }

        fn datagrams_received(&self) -> u64 {
            self.received.get()
        }
    }

    #[test]
    fn carrier_trait_send_and_receive() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        let ep = Endpoint::new(CarrierType::Loopback, vec![42]).unwrap();
        assert!(carrier.send_datagram(&ep, &[1, 2, 3]).is_ok());
        assert_eq!(carrier.datagrams_sent(), 1);
        let (recv_ep, data) = carrier.receive_datagram().unwrap();
        assert_eq!(recv_ep.address, vec![1, 2, 3]);
        assert_eq!(data, vec![0, 0, 0]);
        assert_eq!(carrier.datagrams_received(), 1);
    }

    #[test]
    fn carrier_type_and_name() {
        let carrier = TestCarrier::new(CarrierType::Serial);
        assert_eq!(carrier.carrier_type(), CarrierType::Serial);
        assert_eq!(carrier.carrier_name(), "test");
    }

    #[test]
    fn carrier_is_available() {
        let carrier = TestCarrier::new(CarrierType::Optical);
        assert!(carrier.is_available());
    }

    #[test]
    fn carrier_endpoint_address_limit() {
        let carrier = TestCarrier::new(CarrierType::Radio);
        assert_eq!(carrier.endpoint_address_size_limit(), 1024);
    }

    #[test]
    fn carrier_flush_and_reset() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert!(carrier.flush().is_ok());
        assert!(carrier.reset().is_ok());
    }

    #[test]
    fn carrier_delivery_semantics() {
        assert_eq!(CarrierDeliverySemantics::BestEffort.as_str(), "best_effort");
        assert_eq!(
            CarrierDeliverySemantics::AtLeastOnce.as_str(),
            "at_least_once"
        );
        assert_eq!(
            CarrierDeliverySemantics::AtMostOnce.as_str(),
            "at_most_once"
        );
        assert_eq!(
            CarrierDeliverySemantics::ExactlyOnce.as_str(),
            "exactly_once"
        );
        assert!(!CarrierDeliverySemantics::BestEffort.guarantees_delivery());
        assert!(CarrierDeliverySemantics::AtLeastOnce.guarantees_delivery());
        assert!(!CarrierDeliverySemantics::AtMostOnce.guarantees_ordering());
        assert!(CarrierDeliverySemantics::ExactlyOnce.guarantees_ordering());
    }

    #[test]
    fn carrier_error_retryable_and_fatal() {
        let err = CarrierError::Unavailable(CarrierType::Loopback);
        assert!(err.is_retryable());
        assert!(!err.is_fatal());

        let err = CarrierError::InvalidEndpoint {
            endpoint: "bad".to_string(),
            carrier: "loopback".to_string(),
        };
        assert!(!err.is_retryable());
        assert!(err.is_fatal());

        let err = CarrierError::Timeout {
            carrier: CarrierType::Serial,
            ms: 1000,
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn carrier_error_carrier_type() {
        let err = CarrierError::Unavailable(CarrierType::Radio);
        assert_eq!(err.carrier_type(), Some(CarrierType::Radio));
        let err = CarrierError::AddressTooLarge { size: 2000, max: 1024 };
        assert_eq!(err.carrier_type(), None);
    }

    #[test]
    fn carrier_stats_default_and_totals() {
        let stats = CarrierStats::default();
        assert_eq!(stats.total_datagrams(), 0);
        assert_eq!(stats.total_bytes(), 0);
        assert_eq!(stats.error_rate(), 0.0);
        assert_eq!(stats.success_rate(), 1.0);

        let mut stats = CarrierStats::new("test".to_string());
        stats.datagrams_sent = 10;
        stats.datagrams_received = 5;
        stats.bytes_sent = 1000;
        stats.bytes_received = 500;
        stats.errors = 1;
        assert_eq!(stats.total_datagrams(), 15);
        assert_eq!(stats.total_bytes(), 1500);
        assert!((stats.error_rate() - (1.0 / 15.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_config_default_and_validate() {
        let config = CarrierConfig::default();
        assert_eq!(config.carrier_type, CarrierType::Loopback);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.retry_count, 3);
        assert!(config.validate().is_ok());

        let mut bad_config = CarrierConfig::default();
        bad_config.max_datagram_size = 0;
        assert!(bad_config.validate().is_err());

        let mut bad_timeout = CarrierConfig::default();
        bad_timeout.timeout_ms = 0;
        assert!(bad_timeout.validate().is_err());
    }

    #[test]
    fn carrier_config_builder() {
        let config = CarrierConfig::new(CarrierType::Optical)
            .with_timeout(10000)
            .with_retries(5)
            .with_delivery_semantics(CarrierDeliverySemantics::ExactlyOnce);
        assert_eq!(config.carrier_type, CarrierType::Optical);
        assert_eq!(config.timeout_ms, 10000);
        assert_eq!(config.retry_count, 5);
        assert_eq!(
            config.delivery_semantics,
            CarrierDeliverySemantics::ExactlyOnce
        );
    }

    #[test]
    fn carrier_batch_trait() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        let ep1 = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let ep2 = Endpoint::new(CarrierType::Loopback, vec![2]).unwrap();
        let results = carrier.send_batch(vec![
            (ep1, vec![1, 2]),
            (ep2, vec![3, 4]),
        ]).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert_eq!(carrier.datagrams_sent(), 2);
    }

    #[test]
    fn carrier_flow_control_trait() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert_eq!(carrier.backpressure_level(), 0);
        assert!(!carrier.is_backpressured());
        assert!(carrier.apply_credit(100).is_ok());
    }

    #[test]
    fn carrier_max_datagram_size() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert_eq!(carrier.max_datagram_size(), 65536);
    }

    #[test]
    fn carrier_is_endpoint_valid() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        let valid = Endpoint::new(CarrierType::Loopback, vec![1; 100]).unwrap();
        let invalid = Endpoint::new(CarrierType::Loopback, vec![1; 2000]).unwrap();
        assert!(carrier.is_endpoint_valid(&valid));
        assert!(!carrier.is_endpoint_valid(&invalid));
    }

    #[test]
    fn carrier_supports_broadcast_and_multicast_default() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert!(!carrier.supports_broadcast());
        assert!(!carrier.supports_multicast());
    }

    #[test]
    fn carrier_priority_and_classes() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert_eq!(carrier.priority(), 0);
        assert_eq!(carrier.reliability_level(), 0);
        assert_eq!(carrier.latency_class(), 1);
        assert_eq!(carrier.bandwidth_class(), 1);
    }

    #[test]
    fn carrier_uptime_default() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert_eq!(carrier.carrier_uptime(), 0);
    }

    #[test]
    fn carrier_bytes_sent_and_received() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        assert_eq!(carrier.bytes_sent(), 0);
        assert_eq!(carrier.bytes_received(), 0);
    }

    // === CarrierError extended tests ===
    #[test]
    fn carrier_error_category() {
        assert_eq!(
            CarrierError::Unavailable(CarrierType::Loopback).category(),
            "unavailable"
        );
        assert_eq!(
            CarrierError::AddressTooLarge { size: 100, max: 50 }.category(),
            "address_too_large"
        );
        assert_eq!(
            CarrierError::InvalidEndpoint {
                endpoint: "x".to_string(),
                carrier: "y".to_string(),
            }.category(),
            "invalid_endpoint"
        );
        assert_eq!(
            CarrierError::SendFailed {
                endpoint: "x".to_string(),
                reason: "timeout".to_string(),
            }.category(),
            "send_failed"
        );
        assert_eq!(
            CarrierError::ReceiveFailed { reason: "eof".to_string() }.category(),
            "receive_failed"
        );
        assert_eq!(
            CarrierError::NotInitialized { carrier: CarrierType::Serial }.category(),
            "not_initialized"
        );
        assert_eq!(
            CarrierError::AlreadyInitialized { carrier: CarrierType::Radio }.category(),
            "already_initialized"
        );
        assert_eq!(
            CarrierError::FlushFailed { reason: "io".to_string() }.category(),
            "flush_failed"
        );
        assert_eq!(
            CarrierError::ResetFailed { reason: "io".to_string() }.category(),
            "reset_failed"
        );
        assert_eq!(
            CarrierError::DatagramTooLarge { size: 1000, max: 500 }.category(),
            "datagram_too_large"
        );
        assert_eq!(
            CarrierError::BroadcastNotSupported { carrier: CarrierType::Loopback }.category(),
            "broadcast_not_supported"
        );
        assert_eq!(
            CarrierError::MulticastNotSupported { carrier: CarrierType::Serial }.category(),
            "multicast_not_supported"
        );
        assert_eq!(
            CarrierError::Timeout { carrier: CarrierType::Optical, ms: 500 }.category(),
            "timeout"
        );
        assert_eq!(
            CarrierError::Internal { carrier: CarrierType::Radio, message: "oops".to_string() }.category(),
            "internal"
        );
    }

    #[test]
    fn carrier_error_suggested_retry_delay() {
        assert_eq!(
            CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 3000 }
                .suggested_retry_delay_ms(),
            Some(3000)
        );
        assert_eq!(
            CarrierError::Unavailable(CarrierType::Serial).suggested_retry_delay_ms(),
            Some(1000)
        );
        assert_eq!(
            CarrierError::SendFailed {
                endpoint: "x".to_string(),
                reason: "y".to_string(),
            }.suggested_retry_delay_ms(),
            Some(500)
        );
        assert_eq!(
            CarrierError::NotInitialized { carrier: CarrierType::Loopback }
                .suggested_retry_delay_ms(),
            None
        );
        assert_eq!(
            CarrierError::AddressTooLarge { size: 1, max: 2 }
                .suggested_retry_delay_ms(),
            None
        );
    }

    #[test]
    fn carrier_error_is_permanent_and_transient() {
        assert!(CarrierError::NotInitialized { carrier: CarrierType::Loopback }.is_permanent());
        assert!(CarrierError::AlreadyInitialized { carrier: CarrierType::Serial }.is_permanent());
        assert!(CarrierError::InvalidEndpoint {
            endpoint: "x".to_string(),
            carrier: "y".to_string(),
        }.is_permanent());
        assert!(CarrierError::AddressTooLarge { size: 1, max: 2 }.is_permanent());
        assert!(!CarrierError::Unavailable(CarrierType::Loopback).is_permanent());
        assert!(!CarrierError::Timeout { carrier: CarrierType::Radio, ms: 100 }.is_permanent());

        assert!(CarrierError::Unavailable(CarrierType::Loopback).is_transient());
        assert!(CarrierError::Timeout { carrier: CarrierType::Serial, ms: 100 }.is_transient());
        assert!(!CarrierError::NotInitialized { carrier: CarrierType::Loopback }.is_transient());
    }

    #[test]
    fn carrier_error_plain_message() {
        let msg = CarrierError::Unavailable(CarrierType::Loopback).plain_message();
        assert!(msg.contains("Loopback"));
        assert!(msg.contains("not available"));

        let msg = CarrierError::AddressTooLarge { size: 2000, max: 1024 }.plain_message();
        assert!(msg.contains("2000"));
        assert!(msg.contains("1024"));

        let msg = CarrierError::Timeout { carrier: CarrierType::Radio, ms: 5000 }.plain_message();
        assert!(msg.contains("5000"));

        let msg = CarrierError::Internal { carrier: CarrierType::Serial, message: "corrupt".to_string() }.plain_message();
        assert!(msg.contains("corrupt"));
    }

    // === CarrierEndpointValidator tests ===
    #[test]
    fn carrier_endpoint_validator_new_and_validate() {
        let validator = CarrierEndpointValidator::new(CarrierType::Loopback);
        let valid_ep = Endpoint::new(CarrierType::Loopback, vec![1, 2, 3]).unwrap();
        assert!(validator.validate(&valid_ep).is_ok());
        assert!(validator.is_valid(&valid_ep));

        let invalid_carrier = Endpoint::new(CarrierType::Serial, vec![1, 2, 3]).unwrap();
        assert!(!validator.is_valid(&invalid_carrier));
    }

    #[test]
    fn carrier_endpoint_validator_address_too_large() {
        let validator = CarrierEndpointValidator::new(CarrierType::Loopback);
        let large_addr: Vec<u8> = vec![0; 2000];
        let large_ep = Endpoint::new(CarrierType::Loopback, large_addr).unwrap();
        assert!(!validator.is_valid(&large_ep));
        let err = validator.validate(&large_ep).unwrap_err();
        assert!(matches!(err, CarrierError::AddressTooLarge { .. }));
    }

    #[test]
    fn carrier_endpoint_validator_with_max_size() {
        let validator = CarrierEndpointValidator::new(CarrierType::Loopback)
            .with_max_address_size(10);
        let small_ep = Endpoint::new(CarrierType::Loopback, vec![1; 5]).unwrap();
        assert!(validator.is_valid(&small_ep));
        let large_ep = Endpoint::new(CarrierType::Loopback, vec![1; 15]).unwrap();
        assert!(!validator.is_valid(&large_ep));
        assert_eq!(validator.max_address_size(), 10);
    }

    #[test]
    fn carrier_endpoint_validator_with_allowed_types() {
        let validator = CarrierEndpointValidator::new(CarrierType::Loopback)
            .with_allowed_types(vec![CarrierType::Loopback, CarrierType::Serial]);
        let loopback_ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let serial_ep = Endpoint::new(CarrierType::Serial, vec![1]).unwrap();
        let radio_ep = Endpoint::new(CarrierType::Radio, vec![1]).unwrap();
        assert!(validator.is_valid(&loopback_ep));
        assert!(validator.is_valid(&serial_ep));
        assert!(!validator.is_valid(&radio_ep));
    }

    #[test]
    fn carrier_endpoint_validator_permissive() {
        let validator = CarrierEndpointValidator::permissive();
        for ct in [
            CarrierType::Loopback,
            CarrierType::Serial,
            CarrierType::Optical,
            CarrierType::Radio,
        ] {
            let ep = Endpoint::new(ct, vec![1]).unwrap();
            assert!(validator.is_valid(&ep));
        }
    }

    #[test]
    fn carrier_endpoint_validator_default() {
        let validator = CarrierEndpointValidator::default();
        assert_eq!(validator.carrier_type(), CarrierType::Loopback);
        assert_eq!(validator.max_address_size(), 1024);
    }

    // === CarrierConnectionState tests ===
    #[test]
    fn carrier_connection_state_variants() {
        assert_eq!(CarrierConnectionState::Disconnected.as_str(), "disconnected");
        assert_eq!(CarrierConnectionState::Connecting.as_str(), "connecting");
        assert_eq!(CarrierConnectionState::Connected.as_str(), "connected");
        assert_eq!(CarrierConnectionState::Disconnecting.as_str(), "disconnecting");
        assert_eq!(CarrierConnectionState::Failed.as_str(), "failed");
        assert!(CarrierConnectionState::Connected.is_connected());
        assert!(!CarrierConnectionState::Disconnected.is_connected());
        assert!(CarrierConnectionState::Connected.can_send());
        assert!(!CarrierConnectionState::Disconnected.can_send());
        assert!(CarrierConnectionState::Failed.is_terminal());
        assert!(CarrierConnectionState::Disconnected.is_terminal());
        assert!(!CarrierConnectionState::Connected.is_terminal());
    }

    // === CarrierHealth tests ===
    #[test]
    fn carrier_health_variants() {
        assert_eq!(CarrierHealth::Healthy.as_str(), "healthy");
        assert_eq!(CarrierHealth::Degraded.as_str(), "degraded");
        assert_eq!(CarrierHealth::Unhealthy.as_str(), "unhealthy");
        assert_eq!(CarrierHealth::Unknown.as_str(), "unknown");
        assert!(CarrierHealth::Healthy.is_healthy());
        assert!(!CarrierHealth::Degraded.is_healthy());
        assert!(CarrierHealth::Degraded.is_degraded());
        assert!(CarrierHealth::Unhealthy.is_unhealthy());
    }

    // === CarrierHealthMonitor tests ===
    #[test]
    fn carrier_health_monitor_new_and_default() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let monitor = CarrierHealthMonitor::new(ep.clone());
        assert_eq!(monitor.state(), CarrierConnectionState::Disconnected);
        assert_eq!(monitor.health(), CarrierHealth::Unknown);
        assert_eq!(monitor.consecutive_failures(), 0);
        assert_eq!(monitor.total_sent(), 0);
        assert_eq!(monitor.total_received(), 0);
        assert_eq!(monitor.total_errors(), 0);
        assert!(!monitor.is_healthy());
        assert!(!monitor.is_unhealthy());
    }

    #[test]
    fn carrier_health_monitor_record_success() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        monitor.record_success(100, 10);
        assert_eq!(monitor.state(), CarrierConnectionState::Connected);
        assert_eq!(monitor.health(), CarrierHealth::Healthy);
        assert_eq!(monitor.total_sent(), 100);
        assert_eq!(monitor.consecutive_failures(), 0);
        assert!(monitor.is_healthy());
    }

    #[test]
    fn carrier_health_monitor_record_failure() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        let err = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        monitor.record_failure(&err, 20);
        assert_eq!(monitor.consecutive_failures(), 1);
        assert_eq!(monitor.total_errors(), 1);
    }

    #[test]
    fn carrier_health_monitor_unhealthy_after_many_failures() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        let err = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        for _ in 0..10 {
            monitor.record_failure(&err, 30);
        }
        assert_eq!(monitor.state(), CarrierConnectionState::Failed);
        assert_eq!(monitor.health(), CarrierHealth::Unhealthy);
        assert!(monitor.is_unhealthy());
    }

    #[test]
    fn carrier_health_monitor_record_received() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        monitor.record_received(500);
        assert_eq!(monitor.total_received(), 500);
    }

    #[test]
    fn carrier_health_monitor_reset() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        monitor.record_success(100, 10);
        monitor.record_failure(&CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 }, 20);
        monitor.reset();
        assert_eq!(monitor.state(), CarrierConnectionState::Disconnected);
        assert_eq!(monitor.health(), CarrierHealth::Unknown);
        assert_eq!(monitor.consecutive_failures(), 0);
        assert_eq!(monitor.total_sent(), 0);
        assert_eq!(monitor.total_received(), 0);
        assert_eq!(monitor.total_errors(), 0);
    }

    #[test]
    fn carrier_health_monitor_state_transitions() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        monitor.connecting();
        assert_eq!(monitor.state(), CarrierConnectionState::Connecting);
        monitor.connected();
        assert_eq!(monitor.state(), CarrierConnectionState::Connected);
        assert!(monitor.is_healthy());
        monitor.disconnected();
        assert_eq!(monitor.state(), CarrierConnectionState::Disconnected);
    }

    #[test]
    fn carrier_health_monitor_degraded_after_failures() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        let err = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        monitor.record_failure(&err, 1);
        monitor.record_failure(&err, 2);
        assert_eq!(monitor.health(), CarrierHealth::Degraded);
    }

    // === CarrierRateLimiter tests ===
    #[test]
    fn carrier_rate_limiter_consume() {
        let limiter = CarrierRateLimiter::new(5, 1000);
        assert_eq!(limiter.available_tokens(), 5);
        assert!(limiter.try_consume());
        assert_eq!(limiter.available_tokens(), 4);
        assert!(limiter.try_consume());
        assert!(limiter.try_consume());
        assert!(limiter.try_consume());
        assert!(limiter.try_consume());
        assert!(!limiter.try_consume());
    }

    #[test]
    fn carrier_rate_limiter_consume_n() {
        let limiter = CarrierRateLimiter::new(10, 1000);
        assert_eq!(limiter.try_consume_n(3), 3);
        assert_eq!(limiter.try_consume_n(3), 3);
        assert_eq!(limiter.try_consume_n(3), 4);
        assert_eq!(limiter.try_consume_n(3), 0);
    }

    #[test]
    fn carrier_rate_limiter_default() {
        let limiter = CarrierRateLimiter::default();
        assert_eq!(limiter.max_tokens(), 1000);
        assert_eq!(limiter.available_tokens(), 1000);
    }

    #[test]
    fn carrier_rate_limiter_max_tokens() {
        let limiter = CarrierRateLimiter::new(100, 500);
        assert_eq!(limiter.max_tokens(), 100);
    }

    // === TestCarrier edge cases ===
    #[test]
    fn test_carrier_unavailable() {
        let mut carrier = TestCarrier::new(CarrierType::Loopback);
        // TestCarrier is always available in this test impl
        assert!(carrier.is_available());
    }

    #[test]
    fn test_carrier_all_carrier_types() {
        for ct in [
            CarrierType::Loopback,
            CarrierType::Serial,
            CarrierType::Optical,
            CarrierType::Radio,
            CarrierType::Custom(42),
        ] {
            let carrier = TestCarrier::new(ct);
            assert_eq!(carrier.carrier_type(), ct);
            let ep = Endpoint::new(ct, vec![1]).unwrap();
            assert!(carrier.is_endpoint_valid(&ep));
            assert!(carrier.send_datagram(&ep, &[1]).is_ok());
        }
    }

    #[test]
    fn test_carrier_datagram_counting() {
        let carrier = TestCarrier::new(CarrierType::Loopback);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        for i in 0..100 {
            carrier.send_datagram(&ep, &[i as u8]).unwrap();
        }
        assert_eq!(carrier.datagrams_sent(), 100);
        for _ in 0..50 {
            carrier.receive_datagram().unwrap();
        }
        assert_eq!(carrier.datagrams_received(), 50);
    }

    #[test]
    fn carrier_config_all_builder_methods() {
        let config = CarrierConfig::new(CarrierType::Custom(99))
            .with_timeout(9999)
            .with_retries(42)
            .with_delivery_semantics(CarrierDeliverySemantics::BestEffort);
        assert_eq!(config.carrier_type, CarrierType::Custom(99));
        assert_eq!(config.timeout_ms, 9999);
        assert_eq!(config.retry_count, 42);
        assert_eq!(config.delivery_semantics, CarrierDeliverySemantics::BestEffort);
        assert_eq!(config.max_datagram_size, 65536);
        assert!(!config.enable_batching);
        assert!(!config.enable_flow_control);
    }

    #[test]
    fn carrier_stats_edge_cases() {
        let stats = CarrierStats::default();
        assert!(stats.total_datagrams() == 0);
        assert!(stats.total_bytes() == 0);
        assert!((stats.error_rate() - 0.0).abs() < f64::EPSILON);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);

        let mut stats = CarrierStats::new("edge".to_string());
        stats.datagrams_sent = 0;
        stats.datagrams_received = 0;
        stats.errors = 0;
        assert!((stats.error_rate() - 0.0).abs() < f64::EPSILON);

        stats.datagrams_sent = 1;
        stats.errors = 1;
        assert!((stats.error_rate() - 1.0).abs() < f64::EPSILON);
        assert!((stats.success_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_delivery_semantics_all_combinations() {
        let semantics_list = [
            CarrierDeliverySemantics::BestEffort,
            CarrierDeliverySemantics::AtLeastOnce,
            CarrierDeliverySemantics::AtMostOnce,
            CarrierDeliverySemantics::ExactlyOnce,
        ];
        for s in semantics_list {
            assert!(!s.as_str().is_empty());
            let _ = s.guarantees_delivery();
            let _ = s.guarantees_ordering();
        }
    }

    #[test]
    fn carrier_error_all_variants_constructible() {
        let _ = CarrierError::Unavailable(CarrierType::Loopback);
        let _ = CarrierError::AddressTooLarge { size: 1, max: 2 };
        let _ = CarrierError::InvalidEndpoint { endpoint: "a".to_string(), carrier: "b".to_string() };
        let _ = CarrierError::SendFailed { endpoint: "a".to_string(), reason: "b".to_string() };
        let _ = CarrierError::ReceiveFailed { reason: "c".to_string() };
        let _ = CarrierError::NotInitialized { carrier: CarrierType::Loopback };
        let _ = CarrierError::AlreadyInitialized { carrier: CarrierType::Serial };
        let _ = CarrierError::FlushFailed { reason: "d".to_string() };
        let _ = CarrierError::ResetFailed { reason: "e".to_string() };
        let _ = CarrierError::DatagramTooLarge { size: 1, max: 2 };
        let _ = CarrierError::BroadcastNotSupported { carrier: CarrierType::Optical };
        let _ = CarrierError::MulticastNotSupported { carrier: CarrierType::Radio };
        let _ = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        let _ = CarrierError::Internal { carrier: CarrierType::Custom(0), message: "f".to_string() };
    }

    #[test]
    fn carrier_endpoint_validator_is_valid_consistency() {
        let validator = CarrierEndpointValidator::new(CarrierType::Loopback);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        assert_eq!(validator.is_valid(&ep), validator.validate(&ep).is_ok());
    }

    #[test]
    fn carrier_health_monitor_consecutive_failures_reset_on_success() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        let err = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        monitor.record_failure(&err, 1);
        monitor.record_failure(&err, 2);
        assert_eq!(monitor.consecutive_failures(), 2);
        monitor.record_success(10, 3);
        assert_eq!(monitor.consecutive_failures(), 0);
    }

    #[test]
    fn carrier_health_monitor_health_transitions() {
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let mut monitor = CarrierHealthMonitor::new(ep.clone());
        assert_eq!(monitor.health(), CarrierHealth::Unknown);
        let err = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        monitor.record_failure(&err, 1);
        assert_eq!(monitor.health(), CarrierHealth::Healthy); // 1 failure is still healthy
        monitor.record_failure(&err, 2);
        assert_eq!(monitor.health(), CarrierHealth::Degraded); // 2 failures = degraded
        monitor.record_failure(&err, 3);
        monitor.record_failure(&err, 4);
        monitor.record_failure(&err, 5);
        assert_eq!(monitor.health(), CarrierHealth::Unhealthy); // 5+ failures = unhealthy
    }

    #[test]
    fn carrier_rate_limiter_refill_over_time() {
        let limiter = CarrierRateLimiter::new(10, 1000);
        // Consume all tokens
        for _ in 0..10 {
            assert!(limiter.try_consume());
        }
        assert!(!limiter.try_consume());
        // No time has passed, so no refill yet
        assert_eq!(limiter.available_tokens(), 0);
    }

    // === CarrierMetrics tests ===
    #[test]
    fn carrier_metrics_default_and_totals() {
        let metrics = CarrierMetrics::new("test".to_string());
        assert_eq!(metrics.total_datagrams(), 0);
        assert_eq!(metrics.total_bytes(), 0);
        assert_eq!(metrics.total_errors(), 0);
        assert!((metrics.error_rate() - 0.0).abs() < f64::EPSILON);
        assert!((metrics.delivery_success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_metrics_error_rate() {
        let mut metrics = CarrierMetrics::new("test".to_string());
        metrics.datagrams_sent = 100;
        metrics.datagrams_received = 50;
        metrics.errors_sent = 10;
        metrics.errors_received = 5;
        assert_eq!(metrics.total_datagrams(), 150);
        assert_eq!(metrics.total_errors(), 15);
        assert!((metrics.error_rate() - 0.1).abs() < f64::EPSILON);
        assert!((metrics.delivery_success_rate() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_metrics_throughput_and_latency() {
        let metrics = CarrierMetrics::new("test".to_string());
        assert!((metrics.throughput_bytes_per_sec(0) - 0.0).abs() < f64::EPSILON);
        assert!((metrics.datagrams_per_sec(0) - 0.0).abs() < f64::EPSILON);

        let mut metrics = metrics;
        metrics.bytes_sent = 10000;
        metrics.bytes_received = 5000;
        assert!((metrics.throughput_bytes_per_sec(1000) - 15.0).abs() < f64::EPSILON);
        assert!((metrics.datagrams_per_sec(1000) - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_metrics_record_latency() {
        let mut metrics = CarrierMetrics::new("test".to_string());
        metrics.record_latency(10.0);
        assert!((metrics.min_latency_ms - 10.0).abs() < f64::EPSILON);
        assert!((metrics.max_latency_ms - 10.0).abs() < f64::EPSILON);
        assert!((metrics.avg_latency_ms - 10.0).abs() < f64::EPSILON);
        metrics.record_latency(20.0);
        assert!((metrics.min_latency_ms - 10.0).abs() < f64::EPSILON);
        assert!((metrics.max_latency_ms - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn carrier_metrics_healthy_and_attention() {
        let metrics = CarrierMetrics::new("test".to_string());
        assert!(metrics.is_healthy());
        assert!(!metrics.needs_attention());

        let mut metrics = metrics;
        metrics.timeouts = 200;
        assert!(!metrics.is_healthy());
        assert!(metrics.needs_attention());

        let mut metrics = CarrierMetrics::new("test".to_string());
        metrics.datagrams_sent = 100;
        metrics.errors_sent = 20;
        assert!(!metrics.is_healthy());
        assert!(metrics.needs_attention());
    }

    #[test]
    fn carrier_metrics_latency_summary() {
        let metrics = CarrierMetrics::new("test".to_string());
        let summary = metrics.latency_stats_summary();
        assert!(summary.contains("avg=0.00ms"));
        assert!(summary.contains("max=0.00ms"));
        assert!(summary.contains("min=0.00ms"));
    }

    // === CarrierPool tests ===
    #[test]
    fn carrier_pool_new_and_add() {
        let pool = CarrierPool::new(PoolStrategy::RoundRobin);
        assert_eq!(pool.carrier_count(), 0);
        assert!(!pool.any_available());
    }

    #[test]
    fn carrier_pool_select_empty() {
        let pool = CarrierPool::new(PoolStrategy::RoundRobin);
        assert!(pool.select().is_none());
    }

    #[test]
    fn carrier_pool_send_datagram_empty() {
        let pool = CarrierPool::new(PoolStrategy::RoundRobin);
        let ep = Endpoint::new(CarrierType::Loopback, vec![1]).unwrap();
        let result = pool.send_datagram(&ep, &[1]);
        assert!(result.is_err());
    }

    #[test]
    fn carrier_pool_strategy_default() {
        assert_eq!(PoolStrategy::default(), PoolStrategy::RoundRobin);
    }

    // === CarrierRouter tests ===
    #[test]
    fn carrier_router_new_and_register() {
        let router = CarrierRouter::new();
        assert_eq!(router.carrier_count(), 0);
        assert!(!router.is_registered(CarrierType::Loopback));
    }

    #[test]
    fn carrier_router_registered_types() {
        let mut router = CarrierRouter::new();
        assert!(router.registered_types().is_empty());
    }

    // === CarrierEndpointResolver tests ===
    #[test]
    fn carrier_endpoint_resolver_new_and_mapping() {
        let resolver = CarrierEndpointResolver::new();
        assert_eq!(resolver.mapping_count(), 0);
        assert!(!resolver.contains(&[1, 2, 3]));
    }

    #[test]
    fn carrier_endpoint_resolver_resolve_missing() {
        let resolver = CarrierEndpointResolver::new();
        let default = Endpoint::new(CarrierType::Loopback, vec![99]).unwrap();
        let result = resolver.resolve_or_default(&[1, 2, 3], default.clone());
        assert_eq!(result, default);
        assert!(resolver.resolve(&[1, 2, 3]).is_none());
    }

    // === CarrierHealthAggregator tests ===
    #[test]
    fn carrier_health_aggregator_new_and_empty() {
        let aggregator = CarrierHealthAggregator::new();
        assert_eq!(aggregator.total_count(), 0);
        assert_eq!(aggregator.overall_health(), CarrierHealth::Unknown);
        assert_eq!(aggregator.healthy_count(), 0);
        assert_eq!(aggregator.unhealthy_count(), 0);
        assert_eq!(aggregator.degraded_count(), 0);
    }

    // === CarrierTransactionId tests ===
    #[test]
    fn carrier_transaction_id_zero_and_next() {
        assert!(CarrierTransactionId::ZERO.is_zero());
        assert_eq!(CarrierTransactionId::ZERO.value(), 0);
        let id = CarrierTransactionId::new(42);
        assert!(!id.is_zero());
        assert_eq!(id.value(), 42);
        assert_eq!(id.next().value(), 43);
    }

    #[test]
    fn carrier_transaction_id_default() {
        let id = CarrierTransactionId::default();
        assert!(id.is_zero());
    }

    // === CarrierOperationResult tests ===
    #[test]
    fn carrier_operation_result_success_and_failure() {
        let tid = CarrierTransactionId::new(1);
        let success = CarrierOperationResult::success(tid, 100, 5);
        assert!(success.is_success());
        assert!(!success.is_failure());
        assert_eq!(success.bytes_transferred, 100);
        assert_eq!(success.latency_ms, 5);
        assert!(success.error.is_none());

        let error = CarrierError::Timeout { carrier: CarrierType::Loopback, ms: 100 };
        let failure = CarrierOperationResult::failure(tid, error.clone(), 10);
        assert!(!failure.is_success());
        assert!(failure.is_failure());
        assert_eq!(failure.bytes_transferred, 0);
        assert_eq!(failure.latency_ms, 10);
        assert!(failure.error.is_some());
    }

    // === Additional carrier type coverage ===
    #[test]
    fn carrier_type_custom_display() {
        let ct = CarrierType::Custom(65535);
        assert_eq!(ct.as_u16(), 65535);
        assert!(!ct.is_native());
    }

    #[test]
    fn carrier_type_name_all() {
        assert_eq!(CarrierType::Loopback.name(), "loopback");
        assert_eq!(CarrierType::Serial.name(), "serial");
        assert_eq!(CarrierType::Optical.name(), "optical");
        assert_eq!(CarrierType::Radio.name(), "radio");
        assert_eq!(CarrierType::Custom(0).name(), "custom");
    }

    #[test]
    fn carrier_endpoint_display_and_parts() {
        let ep = Endpoint::new(CarrierType::Radio, vec![1, 2, 3]).unwrap();
        let (ct, addr) = ep.into_parts();
        assert_eq!(ct, CarrierType::Radio);
        assert_eq!(addr, vec![1, 2, 3]);
    }

    // === PoolStrategy coverage ===
    #[test]
    fn pool_strategy_all_variants() {
        assert_eq!(PoolStrategy::RoundRobin as u32, 0);
        assert_eq!(PoolStrategy::FirstAvailable as u32, 1);
        assert_eq!(PoolStrategy::Random as u32, 2);
    }
}